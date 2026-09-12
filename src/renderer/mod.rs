use crate::config::RendererConfig;
use crate::geometry::{ComplexEnvelope, ComplexPoint, ScreenPoint, ScreenSize};
use crate::{IterationBuffer, Sprite};
use minifb::{Key, Window, WindowOptions};

/// Converts calculator results into display pixels.
pub fn sprite_from_iterations(image: &IterationBuffer) -> Sprite {
    let pixels = image
        .iterations()
        .iter()
        .map(|&iterations| color(iterations, image.max_iterations()))
        .collect();
    Sprite::from_pixels(image.width(), image.height(), pixels)
}

/// Displays one rendered sprite in a native window.
pub fn run(image: IterationBuffer, config: &RendererConfig) -> Result<(), minifb::Error> {
    let width = image.width();
    let height = image.height();
    let sprite = sprite_from_iterations(&image);
    let mut framebuffer = vec![0x101820; width * height];
    sprite.draw_into(&mut framebuffer, width, 0, 0);
    let screen_size = ScreenSize::new(width, height);
    let allocation = allocation_screen_rect(screen_size, config.effective_allocation_ratio());
    if config.debug.show_allocation_envelope {
        draw_rectangle_outline(&mut framebuffer, screen_size, allocation, 0xff0000);
    }
    let mut window = Window::new(
        "FractalExplorer - Mandelbrot",
        width,
        height,
        WindowOptions::default(),
    )?;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update_with_buffer(&framebuffer, width, height)?;
    }

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

    let window_envelope = ComplexEnvelope::new(
        -2.0,
        2.0,
        -2.0 * size.height as f64 / size.width as f64,
        2.0 * size.height as f64 / size.width as f64,
    );
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

fn color(iterations: u32, max_iterations: u32) -> u32 {
    if iterations == max_iterations {
        return 0;
    }

    let shade = 255 - (iterations * 255 / max_iterations);
    (shade << 16) | (shade << 8) | 0xff
}

#[cfg(test)]
mod tests {
    use super::{
        allocation_screen_rect, draw_rectangle_outline, sprite_from_iterations, ScreenRect,
    };
    use crate::geometry::ScreenSize;
    use crate::{Mandelbrot, Orchestrator, RenderConfig};

    #[test]
    fn converts_iteration_buffer_to_a_sprite() {
        let image =
            Orchestrator::new(Mandelbrot::new(32)).render(RenderConfig::centered(3, 3, 4.0));
        let sprite = sprite_from_iterations(&image);

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
