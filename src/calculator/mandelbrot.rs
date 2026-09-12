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

    pub fn max_iterations(&self) -> u32 {
        self.max_iterations
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
}
