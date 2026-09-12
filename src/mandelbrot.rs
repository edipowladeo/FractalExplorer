/// CPU Mandelbrot calculator using `f64` coordinates.
pub struct Mandelbrot {
    max_iterations: u32,
}

impl Mandelbrot {
    pub fn new(max_iterations: u32) -> Self {
        assert!(max_iterations > 0, "max_iterations must be positive");
        Self { max_iterations }
    }

    /// Returns the iteration at which `c` escapes, or `max_iterations` if it does not.
    pub fn escape_iterations(&self, real: f64, imaginary: f64) -> u32 {
        let mut z_real = 0.0;
        let mut z_imaginary = 0.0;

        for iteration in 0..self.max_iterations {
            if z_real * z_real + z_imaginary * z_imaginary > 4.0 {
                return iteration;
            }

            let next_real = z_real * z_real - z_imaginary * z_imaginary + real;
            z_imaginary = 2.0 * z_real * z_imaginary + imaginary;
            z_real = next_real;
        }

        self.max_iterations
    }

    /// Renders a square complex-plane view centered on `(0, 0)` into a sprite.
    pub fn render(&self, width: usize, height: usize, view_width: f64) -> crate::Sprite {
        assert!(width > 0 && height > 0, "a render target must have a size");
        assert!(view_width > 0.0, "view_width must be positive");

        let aspect_ratio = height as f64 / width as f64;
        let mut pixels = Vec::with_capacity(width * height);

        for pixel_y in 0..height {
            for pixel_x in 0..width {
                let real = ((pixel_x as f64 + 0.5) / width as f64 - 0.5) * view_width;
                let imaginary =
                    ((pixel_y as f64 + 0.5) / height as f64 - 0.5) * view_width * aspect_ratio;
                let iterations = self.escape_iterations(real, imaginary);
                pixels.push(self.color(iterations));
            }
        }

        crate::Sprite::from_pixels(width, height, pixels)
    }

    fn color(&self, iterations: u32) -> u32 {
        if iterations == self.max_iterations {
            return 0;
        }

        let shade = 255 - (iterations * 255 / self.max_iterations);
        (shade << 16) | (shade << 8) | 0xff
    }
}

#[cfg(test)]
mod tests {
    use super::Mandelbrot;

    #[test]
    fn origin_is_inside_the_set() {
        let fractal = Mandelbrot::new(32);

        assert_eq!(fractal.escape_iterations(0.0, 0.0), 32);
    }

    #[test]
    fn point_outside_the_set_escapes() {
        let fractal = Mandelbrot::new(32);

        assert!(fractal.escape_iterations(2.0, 0.0) < 32);
    }

    #[test]
    fn render_is_centered_on_origin() {
        let fractal = Mandelbrot::new(32);
        let sprite = fractal.render(9, 9, 4.0);

        assert_eq!(sprite.pixels()[4 * 9 + 4], 0);
        assert_ne!(sprite.pixels()[0], 0);
    }
}
