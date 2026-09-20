use crate::config::RendererConfig;
use crate::geometry::{ComplexEnvelope, ComplexPoint, ScreenPoint, ScreenSize};
use crate::orchestrator::CanvasNavigationEvent;
use crate::{
    input::ZoomDirection, InputEvent, InputState, Orchestrator, PrecisionDecisionManager, Sprite,
    Tile, TiledInfiniteCanvas,
};
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Palette {
    Shade,
    Rainbow,
}

impl Default for Palette {
    fn default() -> Self {
        Self::Rainbow
    }
}

impl<'de> Deserialize<'de> for Palette {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.to_ascii_lowercase().as_str() {
            "shade" => Ok(Self::Shade),
            "rainbow" => Ok(Self::Rainbow),
            invalid => {
                eprintln!("Aviso: paleta inválida '{invalid}'; usando fallback 'rainbow'");
                Ok(Self::Rainbow)
            }
        }
    }
}

fn sprite_from_tile(
    tile: &Tile,
    max_iterations: u64,
    palette: Palette,
    palette_period: f64,
) -> Sprite {
    let iteration_storage = tile.iterations();
    let iterations = iteration_storage
        .lock()
        .expect("tile iterations mutex poisoned");
    let pixels = iterations
        .iter()
        .map(|&iteration| color(iteration, max_iterations, palette, palette_period))
        .collect();
    Sprite::from_pixels(tile.width() as usize, tile.height() as usize, pixels)
}

/// Framebuffer and viewport-derived regions for the current native window size.
struct RenderSurface {
    screen_size: ScreenSize,
    framebuffer: Vec<u32>,
    allocation: ScreenRect,
    deallocation: ScreenRect,
    allocation_ratio: f64,
    deallocation_ratio: f64,
}

impl RenderSurface {
    fn new(width: usize, height: usize, allocation_ratio: f64, deallocation_ratio: f64) -> Self {
        let screen_size = ScreenSize::new(width, height);
        Self {
            screen_size,
            framebuffer: vec![0x101820; width * height],
            allocation: allocation_screen_rect(screen_size, allocation_ratio),
            deallocation: deallocation_screen_rect(screen_size, deallocation_ratio),
            allocation_ratio,
            deallocation_ratio,
        }
    }

    /// Applies a size reported by the window immediately, ignoring transient zero-sized states.
    fn update_window_size(&mut self, (width, height): (usize, usize)) -> bool {
        if width == 0
            || height == 0
            || (width, height) == (self.screen_size.width, self.screen_size.height)
        {
            return false;
        }

        self.screen_size = ScreenSize::new(width, height);
        self.framebuffer = vec![0x101820; width * height];
        self.allocation = allocation_screen_rect(self.screen_size, self.allocation_ratio);
        self.deallocation = deallocation_screen_rect(self.screen_size, self.deallocation_ratio);
        true
    }

    fn clear(&mut self) {
        self.framebuffer.fill(0x101820);
    }
}

/// Displays one rendered sprite in a native window.
pub fn run(
    canvas: &mut TiledInfiniteCanvas,
    orchestrator: &Orchestrator,
    config: &RendererConfig,
) -> Result<(), minifb::Error> {
    let (_sender, receiver) = std::sync::mpsc::channel();
    run_with_updates(canvas, orchestrator, config, receiver)
}

pub fn run_with_updates(
    canvas: &mut TiledInfiniteCanvas,
    orchestrator: &Orchestrator,
    initial_config: &RendererConfig,
    receiver: Receiver<RendererConfig>,
) -> Result<(), minifb::Error> {
    let mut config = initial_config.clone();
    let mut surface = RenderSurface::new(
        config.width,
        config.height,
        config.effective_allocation_ratio(),
        config.effective_deallocation_ratio(),
    );
    let mut window = Window::new(
        "FractalExplorer - Mandelbrot",
        config.width,
        config.height,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )?;
    let mut input = InputState::new();
    let mut render_plan = PrecisionDecisionManager::from_config(initial_config).map_err(|_| {
        minifb::Error::WindowCreate("invalid renderer precision configuration".to_string())
    })?;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        while let Ok(next_config) = receiver.try_recv() {
            if next_config.width == config.width && next_config.height == config.height {
                if let Ok(next_plan) = PrecisionDecisionManager::from_config(&next_config) {
                    if next_plan != render_plan {
                        render_plan = next_plan;
                        orchestrator.set_render_plan(next_plan);
                        canvas.set_render_plan(next_plan);
                        canvas.invalidate_tiles();
                    }
                }
                config = next_config;
            }
        }
        // `get_size` changes while the resize gesture is in progress, not only when it ends.
        surface.update_window_size(window.get_size());
        surface.clear();
        canvas.trim_outside_allocation((
            surface.deallocation.left,
            surface.deallocation.top,
            surface.deallocation.right,
            surface.deallocation.bottom,
        ));
        canvas.ensure_screen_coverage((
            surface.allocation.left,
            surface.allocation.top,
            surface.allocation.right,
            surface.allocation.bottom,
        ));
        for layer in canvas.layers() {
            orchestrator.render_layer(layer);
        }
        let mouse_position = window
            .get_mouse_pos(MouseMode::Clamp)
            .map(|(x, y)| ScreenPoint::new(x.round() as i32, y.round() as i32));
        let events = input.update(
            mouse_position,
            window.get_mouse_down(MouseButton::Left),
            window.get_mouse_down(MouseButton::Middle),
            window.get_scroll_wheel().map_or(0.0, |(_, y)| y),
        );
        for event in events {
            match event {
                InputEvent::Drag { delta } => canvas.drag(delta),
                InputEvent::MiddleClick(cursor) => {
                    let complex = canvas.screen_to_complex(cursor);
                    copy_coordinates(&format_copied_coordinates(
                        complex.clone(),
                        canvas.apparent_pixel_size(),
                    ));
                    if config.debug.middle_click_coordinate_report {
                        println!(
                            "{}",
                            middle_click_coordinate_report(canvas, cursor, complex)
                        );
                    }
                }
                InputEvent::Zoom { direction, cursor } => {
                    let multiplier = config.zoom_multiplier;
                    assert!(multiplier > 1.0, "zoom_multiplier must be greater than 1");
                    let current_zoom = canvas
                        .layer(0)
                        .map_or(config.max_apparent_pixel_size(), |layer| layer.zoom());
                    let zoom = match direction {
                        ZoomDirection::In => current_zoom * multiplier,
                        ZoomDirection::Out => current_zoom / multiplier,
                    };
                    canvas.zoom_at(cursor, zoom);
                }
            }
        }
        if config.debug.text_overlay_global {
            if let Some(cursor) = mouse_position {
                let complex = canvas.screen_to_complex(cursor);
                draw_status_bar(
                    &mut surface.framebuffer,
                    surface.screen_size,
                    &format_coordinates(complex),
                );
            }
        }
        for layer in canvas.layers() {
            let (tile_width, tile_height) = layer.tile_screen_size();
            for row in 0..layer.row_count() {
                for column in 0..layer.column_count() {
                    let tile = layer.tile(row, column).expect("layer grid is rectangular");
                    if tile.status() != crate::orchestrator::TileStatus::Completed {
                        continue;
                    }
                    let sprite = tile.sprite().unwrap_or_else(|| {
                        let sprite = std::sync::Arc::new(sprite_from_tile(
                            tile,
                            config.effective_max_iterations() as u64,
                            config.palette,
                            config.palette_period,
                        ));
                        tile.set_sprite(std::sync::Arc::clone(&sprite));
                        tile.sprite().expect("tile sprite should exist")
                    });
                    let tile_position = canvas.complex_to_screen(tile.coordinate().clone());
                    sprite.draw_into_scaled(
                        &mut surface.framebuffer,
                        surface.screen_size.width,
                        tile_position.x as isize,
                        tile_position.y as isize,
                        tile_width as usize,
                        tile_height as usize,
                    );
                }
            }
        }
        if let Some(cursor) = mouse_position {
            draw_mouse_marker(
                &mut surface.framebuffer,
                surface.screen_size,
                cursor,
                0x00ffff,
            );
        }
        if config.debug.show_allocation_envelope {
            draw_rectangle_outline(
                &mut surface.framebuffer,
                surface.screen_size,
                surface.allocation,
                0xff0000,
            );
            draw_rectangle_outline(
                &mut surface.framebuffer,
                surface.screen_size,
                surface.deallocation,
                0xffff00,
            );
        }
        if config.debug.text_overlay_layers {
            let overlays: Vec<_> = canvas
                .layers()
                .iter()
                .enumerate()
                .map(|(index, layer)| {
                    format_layer_overlay(
                        index,
                        layer.zoom(),
                        delta_exponent(layer.delta()),
                        layer.column_count(),
                        layer.row_count(),
                        mouse_position.map(|cursor| canvas.screen_to_complex(cursor)),
                    )
                })
                .collect();
            let first_overlay_y = surface
                .screen_size
                .height
                .saturating_sub(24 + overlays.len() * 8 + 4)
                as i32;
            for (line, overlay) in overlays.iter().enumerate() {
                draw_text(
                    &mut surface.framebuffer,
                    surface.screen_size,
                    8,
                    first_overlay_y + line as i32 * 8,
                    overlay,
                    0xffffff,
                );
            }
        }
        if config.debug.text_overlay_queue {
            let queue_lines: Vec<_> = canvas
                .layers()
                .iter()
                .enumerate()
                .flat_map(|(layer_index, layer)| {
                    layer
                        .pending_work_positions()
                        .into_iter()
                        .map(move |(row, column)| {
                            format_worker_queue_line(
                                layer_index,
                                row,
                                column,
                                delta_exponent(layer.delta()),
                            )
                        })
                })
                .collect();
            for (line, queue_line) in queue_lines.iter().enumerate() {
                let x = surface
                    .screen_size
                    .width
                    .saturating_sub(queue_line.chars().count() * 6 + 8)
                    as i32;
                draw_text(
                    &mut surface.framebuffer,
                    surface.screen_size,
                    x,
                    8 + line as i32 * 8,
                    queue_line,
                    0xffffff,
                );
            }
        }
        if config.debug.text_overlay_workers {
            for (line, worker) in orchestrator.worker_statuses().iter().enumerate() {
                let worker_line = format_worker_status_line(worker.id, worker.tile.as_ref());
                draw_text(
                    &mut surface.framebuffer,
                    surface.screen_size,
                    8,
                    8 + line as i32 * 8,
                    &worker_line,
                    0xffffff,
                );
            }
        }
        window.update_with_buffer(
            &surface.framebuffer,
            surface.screen_size.width,
            surface.screen_size.height,
        )?;
    }

    // Closing the native window leaves the loop and releases the renderer
    // before the application returns from `main`.
    drop(window);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn allocation_screen_rect(size: ScreenSize, allocation_ratio: f64) -> ScreenRect {
    assert!(allocation_ratio > 0.0, "allocation_ratio must be positive");

    let window_envelope = window_envelope(size);
    let top_left = window_envelope.screen_to_complex(ScreenPoint::new(0, 0), size);
    let bottom_right = window_envelope.screen_to_complex(
        ScreenPoint::new(size.width as i32 - 1, size.height as i32 - 1),
        size,
    );
    let window_envelope =
        ComplexEnvelope::new(top_left.x, bottom_right.x, bottom_right.y, top_left.y);
    let center = ComplexPoint::new(
        (window_envelope.xmin() + window_envelope.xmax()) / 2.0,
        (window_envelope.ymin() + window_envelope.ymax()) / 2.0,
    );
    let half_width = (window_envelope.xmax() - window_envelope.xmin()) * allocation_ratio / 2.0;
    let half_height = (window_envelope.ymax() - window_envelope.ymin()) * allocation_ratio / 2.0;
    let allocation_envelope = ComplexEnvelope::new(
        center.x - half_width,
        center.x + half_width,
        center.y - half_height,
        center.y + half_height,
    );
    let top_left = complex_to_screen(
        &window_envelope,
        ComplexPoint::new(*allocation_envelope.xmin(), *allocation_envelope.ymax()),
        size,
    );
    let bottom_right = complex_to_screen(
        &window_envelope,
        ComplexPoint::new(*allocation_envelope.xmax(), *allocation_envelope.ymin()),
        size,
    );
    ScreenRect {
        left: top_left.x,
        top: top_left.y,
        right: bottom_right.x,
        bottom: bottom_right.y,
    }
}

fn deallocation_screen_rect(size: ScreenSize, deallocation_ratio: f64) -> ScreenRect {
    allocation_screen_rect(size, deallocation_ratio)
}

fn window_envelope(size: ScreenSize) -> ComplexEnvelope<f64> {
    ComplexEnvelope::new(
        -2.0,
        2.0,
        -2.0 * size.height as f64 / size.width as f64,
        2.0 * size.height as f64 / size.width as f64,
    )
}

fn complex_to_screen(
    envelope: &ComplexEnvelope<f64>,
    point: ComplexPoint<f64>,
    size: ScreenSize,
) -> ScreenPoint {
    let x = ((point.x - *envelope.xmin()) / (*envelope.xmax() - *envelope.xmin())
        * (size.width - 1) as f64)
        .round() as i32;
    let y = ((*envelope.ymax() - point.y) / (*envelope.ymax() - *envelope.ymin())
        * (size.height - 1) as f64)
        .round() as i32;
    ScreenPoint::new(x, y)
}

fn draw_rectangle_outline(
    framebuffer: &mut [u32],
    size: ScreenSize,
    rectangle: ScreenRect,
    color: u32,
) {
    for x in rectangle.left.max(0)..=rectangle.right.min(size.width as i32 - 1) {
        set_pixel(framebuffer, size, x, rectangle.top, color);
        set_pixel(framebuffer, size, x, rectangle.bottom, color);
    }
    for y in rectangle.top.max(0)..=rectangle.bottom.min(size.height as i32 - 1) {
        set_pixel(framebuffer, size, rectangle.left, y, color);
        set_pixel(framebuffer, size, rectangle.right, y, color);
    }
}

fn set_pixel(framebuffer: &mut [u32], size: ScreenSize, x: i32, y: i32, color: u32) {
    if x >= 0 && y >= 0 && x < size.width as i32 && y < size.height as i32 {
        framebuffer[y as usize * size.width + x as usize] = color;
    }
}

fn format_coordinates(point: ComplexPoint<f64>) -> String {
    format!("x: {:.15}   y: {:.15}", point.x, point.y)
}

fn format_copied_coordinates(point: ComplexPoint<f64>, zoom: f64) -> String {
    format!("{}   zoom: {:.15}", format_coordinates(point), zoom.log2())
}

fn format_layer_overlay(
    index: usize,
    zoom: f64,
    delta: f64,
    columns: usize,
    rows: usize,
    mouse: Option<ComplexPoint<f64>>,
) -> String {
    let mouse_text = mouse.map_or_else(String::new, |point| {
        format!(" mouse={:.15}x{:.15}", point.x, point.y)
    });
    format!(
        "Camada {index}: zoom={:.3} delta={delta:.3} {columns}x{rows} tiles{mouse_text}",
        zoom.log2()
    )
}

fn format_worker_queue_line(layer: usize, row: usize, column: usize, delta: f64) -> String {
    format!("Camada={layer} pos {row}x{column} delta={delta:.3}")
}

fn delta_exponent(delta: f64) -> f64 {
    -delta.log2()
}

fn format_worker_status_line(id: usize, tile: Option<&crate::orchestrator::WorkerTile>) -> String {
    match tile {
        Some(tile) => format!(
            "Worker {id}: tile pos {:.3}x{:.3} delta={:.3}",
            tile.coordinate.x,
            tile.coordinate.y,
            delta_exponent(tile.delta)
        ),
        None => format!("Worker {id}: ocioso"),
    }
}

fn draw_status_bar(framebuffer: &mut [u32], size: ScreenSize, text: &str) {
    let bar_height = 24usize;
    let top = size.height.saturating_sub(bar_height);
    for y in top..size.height {
        for x in 0..size.width {
            framebuffer[y * size.width + x] = 0x202020;
        }
    }
    draw_text(framebuffer, size, 8, top as i32 + 8, &text, 0xffffff);
}

fn draw_mouse_marker(framebuffer: &mut [u32], size: ScreenSize, position: ScreenPoint, color: u32) {
    for offset in -4..=4 {
        set_pixel(framebuffer, size, position.x + offset, position.y, color);
        set_pixel(framebuffer, size, position.x, position.y + offset, color);
    }
}

fn copy_coordinates(coordinates: &str) {
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(coordinates)) {
        Ok(()) => println!("Coordenadas copiadas: {coordinates}"),
        Err(error) => eprintln!("Não foi possível copiar as coordenadas: {error}"),
    }
}

fn middle_click_coordinate_report(
    canvas: &TiledInfiniteCanvas,
    cursor: ScreenPoint,
    global: ComplexPoint<f64>,
) -> String {
    let initial = canvas.initial_state();
    let mut report = format!(
        "Canvas inicial: pos={:.15}x{:.15} tile={}x{} delta={:.15} tela={}x{} zoom_max={:.15} zoom_min={:.15}\nMouse tela: {}x{}\nMouse complexo: {:.15}x{:.15}",
        initial.position.x,
        initial.position.y,
        initial.tile_width,
        initial.tile_height,
        initial.delta,
        initial.screen_position.x,
        initial.screen_position.y,
        initial.max_apparent_pixel_size,
        initial.min_apparent_pixel_size,
        cursor.x,
        cursor.y,
        global.x,
        global.y
    );
    let mut discrepancy_count = 0;
    for (index, layer) in canvas.layers().iter().enumerate() {
        let point = layer.screen_to_complex(cursor);
        let expected_screen = layer.complex_to_screen(point.clone());
        let dx = point.x - global.x;
        let dy = point.y - global.y;
        let discrepancy = (dx.abs() >= 1e-12 || dy.abs() >= 1e-12).then(|| {
            discrepancy_count += 1;
            format!(" DESALINHADA dx={dx:.15} dy={dy:.15}")
        });
        report.push_str(&format!(
            "\nCamada {index}: {:.15}x{:.15} tela_esperada {}x{}{}",
            point.x,
            point.y,
            expected_screen.x,
            expected_screen.y,
            discrepancy.unwrap_or_default()
        ));
    }
    if discrepancy_count > 0 {
        report.push_str(&format!("\nDiscrepancias detectadas: {discrepancy_count}"));
    }
    if !canvas.navigation_history().is_empty() {
        report.push_str("\nPercurso de navegacao:");
        for (index, event) in canvas.navigation_history().iter().enumerate() {
            let command = match event {
                CanvasNavigationEvent::Drag { delta } => {
                    format!("arrasto {}x{}", delta.x, delta.y)
                }
                CanvasNavigationEvent::Zoom { cursor, zoom } => {
                    format!("zoom {}x{} escala={zoom:.15}", cursor.x, cursor.y)
                }
            };
            report.push_str(&format!("\n{}. {command}", index + 1));
        }
    }
    report
}

fn draw_text(framebuffer: &mut [u32], size: ScreenSize, x: i32, y: i32, text: &str, color: u32) {
    let mut cursor_x = x;
    for character in text.chars() {
        if let Some(glyph) = glyph(character) {
            for (row, bits) in glyph.iter().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        set_pixel(framebuffer, size, cursor_x + column, y + row as i32, color);
                    }
                }
            }
        }
        cursor_x += 6;
    }
}

fn glyph(character: char) -> Option<[u8; 7]> {
    let glyph = match character {
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100,
        ],
        'x' => [
            0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b00000,
        ],
        'y' => [
            0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100,
        ],
        'p' => [
            0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000,
        ],
        'z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        'o' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'a' => [
            0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111,
        ],
        'm' => [
            0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101,
        ],
        'd' => [
            0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10011, 0b01101,
        ],
        'e' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01111,
        ],
        't' => [
            0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00101, 0b00010,
        ],
        'i' => [
            0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'l' => [
            0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        's' => [
            0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110,
        ],
        '*' => [
            0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '=' => [
            0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        ' ' => [0; 7],
        _ => return None,
    };
    Some(glyph)
}

fn color(iterations: u64, max_iterations: u64, palette: Palette, palette_period: f64) -> u32 {
    if iterations == max_iterations {
        return 0;
    }

    match palette {
        Palette::Shade => shade_color(iterations, max_iterations),
        Palette::Rainbow => rainbow_color(iterations, palette_period),
    }
}

fn shade_color(iterations: u64, max_iterations: u64) -> u32 {
    let shade = 255 - (iterations * 255 / max_iterations);
    ((shade << 16) | (shade << 8) | 0xff) as u32
}

fn rainbow_color(iterations: u64, palette_period: f64) -> u32 {
    assert!(palette_period > 0.0, "palette_period must be positive");
    let hue = (0.7 + iterations as f64 / palette_period).rem_euclid(1.0);
    let sector = hue * 6.0;
    let fraction = sector.fract();
    let p = 0.1;
    let q = 1.0 - 0.9 * fraction;
    let t = 1.0 - 0.9 * (1.0 - fraction);
    let (red, green, blue) = match sector as u32 {
        0 => (1.0, t, p),
        1 => (q, 1.0, p),
        2 => (p, 1.0, t),
        3 => (p, q, 1.0),
        4 => (t, p, 1.0),
        _ => (1.0, p, q),
    };
    ((red * 255.0) as u32) << 16 | ((green * 255.0) as u32) << 8 | (blue * 255.0) as u32
}

#[cfg(test)]
mod tests {
    use super::{
        allocation_screen_rect, draw_mouse_marker, draw_rectangle_outline, format_coordinates,
        format_copied_coordinates, format_layer_overlay, format_worker_queue_line,
        format_worker_status_line, middle_click_coordinate_report, sprite_from_tile, Palette,
        RenderSurface, ScreenRect,
    };
    use crate::geometry::{ComplexPoint, ScreenPoint, ScreenSize};
    use crate::{Mandelbrot, Orchestrator, Tile, TiledInfiniteCanvas};

    #[test]
    fn converts_tile_iterations_to_a_sprite() {
        let tile = std::sync::Arc::new(Tile::new(ComplexPoint::new(-1.0, 1.0), 3, 3, 1.0));
        Orchestrator::new(Mandelbrot::new(32)).render_tile(&tile);
        let sprite = sprite_from_tile(&tile, 32, Palette::Shade, 5.0);

        assert_eq!((sprite.width(), sprite.height()), (3, 3));
        assert_eq!(sprite.pixels()[4], 0);
        assert_ne!(sprite.pixels()[0], 0);
    }

    #[test]
    fn calculates_a_centered_half_size_allocation_rectangle() {
        let rectangle = allocation_screen_rect(ScreenSize::new(5, 5), 0.5);

        assert_eq!(
            rectangle,
            ScreenRect {
                left: 1,
                top: 1,
                right: 3,
                bottom: 3,
            }
        );
    }

    #[test]
    fn render_surface_reallocates_the_framebuffer_for_each_live_window_size() {
        let mut surface = RenderSurface::new(800, 600, 1.2, 1.5);

        assert!(surface.update_window_size((960, 540)));
        assert_eq!(surface.screen_size, ScreenSize::new(960, 540));
        assert_eq!(surface.framebuffer.len(), 960 * 540);
        assert_eq!(
            surface.allocation,
            allocation_screen_rect(ScreenSize::new(960, 540), 1.2)
        );
        assert_eq!(
            surface.deallocation,
            allocation_screen_rect(ScreenSize::new(960, 540), 1.5)
        );

        assert!(!surface.update_window_size((960, 540)));
        assert!(!surface.update_window_size((0, 540)));
    }

    #[test]
    fn invalid_palette_falls_back_to_rainbow() {
        let config: crate::config::RendererConfig =
            toml::from_str("palette = \"unknown\"").unwrap();

        assert_eq!(config.palette, Palette::Rainbow);
        assert_eq!(
            crate::config::RendererConfig::default().palette,
            Palette::Rainbow
        );
    }

    #[test]
    fn formats_layer_overlay_with_grid_dimensions() {
        assert_eq!(
            format_layer_overlay(2, 4.0, 0.005, 3, 4, None),
            "Camada 2: zoom=2.000 delta=0.005 3x4 tiles"
        );
    }

    #[test]
    fn formats_layer_overlay_with_high_precision_mouse_coordinate() {
        assert_eq!(
            format_layer_overlay(
                0,
                8.0,
                7.644,
                1,
                1,
                Some(ComplexPoint::new(-0.743643887037151, 0.131825904205330)),
            ),
            "Camada 0: zoom=3.000 delta=7.644 1x1 tiles mouse=-0.743643887037151x0.131825904205330"
        );
    }

    #[test]
    fn formats_worker_queue_line_with_layer_and_tile_position() {
        assert_eq!(
            format_worker_queue_line(1, 2, 4, 0.005),
            "Camada=1 pos 2x4 delta=0.005"
        );
    }

    #[test]
    fn formats_worker_status_with_tile_position_and_delta() {
        let tile = crate::orchestrator::WorkerTile {
            coordinate: ComplexPoint::new(-2.0, 3.0),
            delta: 0.5,
        };

        assert_eq!(
            format_worker_status_line(2, Some(&tile)),
            "Worker 2: tile pos -2.000x3.000 delta=1.000"
        );
        assert_eq!(format_worker_status_line(3, None), "Worker 3: ocioso");
    }

    #[test]
    fn formats_the_complex_coordinates_for_the_status_bar() {
        let text = format_coordinates(ComplexPoint::new(-0.743643887037151, 0.131825904205330));

        assert_eq!(text, "x: -0.743643887037151   y: 0.131825904205330");
    }

    #[test]
    fn formats_copied_coordinates_with_the_current_zoom() {
        let text = format_copied_coordinates(
            ComplexPoint::new(-0.743643887037151, 0.131825904205330),
            2.5,
        );

        assert_eq!(
            text,
            "x: -0.743643887037151   y: 0.131825904205330   zoom: 1.321928094887362"
        );
    }

    #[test]
    fn formats_middle_click_report_with_only_coordinates() {
        let mut canvas = TiledInfiniteCanvas::new(
            ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );
        canvas.expand_one_layer_per_frame();

        let report = middle_click_coordinate_report(
            &canvas,
            ScreenPoint::new(300, 225),
            ComplexPoint::new(-1.0, 1.0),
        );

        assert_eq!(
            report,
            "Canvas inicial: pos=-1.000000000000000x1.000000000000000 tile=30x20 delta=0.010000000000000 tela=300x225 zoom_max=8.000000000000000 zoom_min=0.800000000000000\nMouse tela: 300x225\nMouse complexo: -1.000000000000000x1.000000000000000\nCamada 0: -1.005000000000000x1.005000000000000 tela_esperada 300x225 DESALINHADA dx=-0.005000000000000 dy=0.005000000000000\nDiscrepancias detectadas: 1"
        );
    }

    #[test]
    fn middle_click_report_records_the_navigation_path_needed_to_reproduce_it() {
        let mut canvas = TiledInfiniteCanvas::new(
            ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );
        canvas.drag(ScreenPoint::new(12, -7));
        canvas.zoom_at(ScreenPoint::new(484, 357), 10.4);

        let report = middle_click_coordinate_report(
            &canvas,
            ScreenPoint::new(484, 357),
            canvas.screen_to_complex(ScreenPoint::new(484, 357)),
        );

        assert!(report.contains(
            "Percurso de navegacao:\n1. arrasto 12x-7\n2. zoom 484x357 escala=10.400000000000000"
        ));
    }

    #[test]
    fn middle_click_report_uses_each_layers_own_transform() {
        let mut canvas = TiledInfiniteCanvas::new(
            ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );
        canvas.expand_one_layer_per_frame();
        canvas
            .layer_mut(0)
            .unwrap()
            .set_screen_position(ScreenPoint::new(0, 0));
        let cursor = ScreenPoint::new(300, 225);

        let report =
            middle_click_coordinate_report(&canvas, cursor, canvas.screen_to_complex(cursor));

        assert!(
            report.contains("Camada 0: -0.625000000000000x0.718750000000000 tela_esperada 300x225")
        );
    }

    #[test]
    fn middle_click_report_records_the_canvas_initial_state_for_replay() {
        let canvas = TiledInfiniteCanvas::new(
            ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            ScreenPoint::new(385, 290),
            8.0,
            0.8,
        );

        let report = middle_click_coordinate_report(
            &canvas,
            ScreenPoint::new(353, 318),
            canvas.screen_to_complex(ScreenPoint::new(353, 318)),
        );

        assert!(report.contains(
            "Canvas inicial: pos=-1.572500000000000x0.047500000000000 tile=30x20 delta=0.005000000000000 tela=385x290 zoom_max=8.000000000000000 zoom_min=0.800000000000000"
        ));
    }

    #[test]
    fn middle_click_report_does_not_flag_layers_aligned_with_the_camera() {
        let mut canvas = TiledInfiniteCanvas::new(
            ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            ScreenPoint::new(385, 290),
            8.0,
            0.5,
        );
        for _ in 0..5 {
            canvas.ensure_screen_coverage((0, 0, 799, 599));
        }
        let cursor = ScreenPoint::new(415, 368);

        let report =
            middle_click_coordinate_report(&canvas, cursor, canvas.screen_to_complex(cursor));

        assert!(!report.contains("DESALINHADA"));
        assert!(!report.contains("Discrepancias detectadas:"));
    }

    #[test]
    fn draws_only_the_one_pixel_red_outline_when_requested() {
        let size = ScreenSize::new(5, 5);
        let mut framebuffer = vec![0; 25];

        draw_rectangle_outline(
            &mut framebuffer,
            size,
            ScreenRect {
                left: 1,
                top: 1,
                right: 3,
                bottom: 3,
            },
            0xff0000,
        );

        assert_eq!(framebuffer[1 + 5], 0xff0000);
        assert_eq!(framebuffer[2 + 5], 0xff0000);
        assert_eq!(framebuffer[2 + 5 * 2], 0);
    }

    #[test]
    fn draws_a_mouse_marker_as_a_cross() {
        let size = ScreenSize::new(9, 9);
        let mut framebuffer = vec![0; 81];

        draw_mouse_marker(
            &mut framebuffer,
            size,
            crate::geometry::ScreenPoint::new(4, 4),
            0x00ffff,
        );

        assert_eq!(framebuffer[4 * 9 + 0], 0x00ffff);
        assert_eq!(framebuffer[0 * 9 + 4], 0x00ffff);
        assert_eq!(framebuffer[4 * 9 + 4], 0x00ffff);
    }
}
