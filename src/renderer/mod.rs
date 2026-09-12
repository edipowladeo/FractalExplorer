use crate::config::RendererConfig;
use crate::geometry::{ComplexEnvelope, ComplexPoint, ScreenPoint, ScreenSize};
use crate::{input::ZoomDirection, InputEvent, InputState, Sprite, Tile, TileSprite};
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use serde::{Deserialize, Deserializer, Serialize};

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

/// Displays one rendered sprite in a native window.
pub fn run(tile_sprite: &mut TileSprite, config: &RendererConfig) -> Result<(), minifb::Error> {
    let width = config.width;
    let height = config.height;
    let sprite = sprite_from_tile(
        tile_sprite.tile(),
        config.max_iterations as u64,
        config.palette,
        config.palette_period,
    );
    let screen_size = ScreenSize::new(width, height);
    let allocation = allocation_screen_rect(screen_size, config.effective_allocation_ratio());
    let window_envelope = window_envelope(screen_size);
    let mut window = Window::new(
        "FractalExplorer - Mandelbrot",
        width,
        height,
        WindowOptions::default(),
    )?;
    let mut input = InputState::new();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut framebuffer = vec![0x101820; width * height];
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
                InputEvent::Drag { delta } => tile_sprite.set_position(ScreenPoint::new(
                    tile_sprite.position().x + delta.x,
                    tile_sprite.position().y + delta.y,
                )),
                InputEvent::MiddleClick(cursor) => {
                    let complex = window_envelope.screen_to_complex(cursor, screen_size);
                    copy_coordinates(&format_coordinates(complex));
                }
                InputEvent::Zoom { direction, cursor } => {
                    let multiplier = config.zoom_multiplier;
                    assert!(multiplier > 1.0, "zoom_multiplier must be greater than 1");
                    let zoom = match direction {
                        ZoomDirection::In => tile_sprite.zoom() * multiplier,
                        ZoomDirection::Out => tile_sprite.zoom() / multiplier,
                    };
                    tile_sprite.zoom_at(cursor, zoom);
                }
            }
        }
        if let Some(cursor) = mouse_position {
            let complex = window_envelope.screen_to_complex(cursor, screen_size);
            draw_status_bar(&mut framebuffer, screen_size, &format_coordinates(complex));
        }
        let (sprite_width, sprite_height) = tile_sprite.screen_size();
        sprite.draw_into_scaled(
            &mut framebuffer,
            width,
            tile_sprite.position().x as isize,
            tile_sprite.position().y as isize,
            sprite_width as usize,
            sprite_height as usize,
        );
        if config.debug.show_allocation_envelope {
            draw_rectangle_outline(&mut framebuffer, screen_size, allocation, 0xff0000);
        }
        window.update_with_buffer(&framebuffer, width, height)?;
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

fn copy_coordinates(coordinates: &str) {
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(coordinates)) {
        Ok(()) => println!("Coordenadas copiadas: {coordinates}"),
        Err(error) => eprintln!("Não foi possível copiar as coordenadas: {error}"),
    }
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
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
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
        allocation_screen_rect, draw_rectangle_outline, format_coordinates, sprite_from_tile,
        Palette, ScreenRect,
    };
    use crate::geometry::{ComplexPoint, ScreenSize};
    use crate::{Mandelbrot, Orchestrator, Tile};

    #[test]
    fn converts_tile_iterations_to_a_sprite() {
        let tile = Tile::new(ComplexPoint::new(0.0, 0.0), 3, 3, 1.0);
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
    fn formats_the_complex_coordinates_for_the_status_bar() {
        let text = format_coordinates(ComplexPoint::new(-0.743643887037151, 0.131825904205330));

        assert_eq!(text, "x: -0.743643887037151   y: 0.131825904205330");
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
}
