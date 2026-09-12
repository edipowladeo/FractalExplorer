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
pub fn run(image: IterationBuffer) -> Result<(), minifb::Error> {
    let width = image.width();
    let height = image.height();
    let sprite = sprite_from_iterations(&image);
    let mut framebuffer = vec![0x101820; width * height];
    sprite.draw_into(&mut framebuffer, width, 0, 0);
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

fn color(iterations: u32, max_iterations: u32) -> u32 {
    if iterations == max_iterations {
        return 0;
    }

    let shade = 255 - (iterations * 255 / max_iterations);
    (shade << 16) | (shade << 8) | 0xff
}

#[cfg(test)]
mod tests {
    use super::sprite_from_iterations;
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
}
