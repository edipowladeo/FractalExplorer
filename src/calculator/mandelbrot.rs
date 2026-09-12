use crate::Fixed;

/// CPU Mandelbrot calculator using `f64` coordinates.
pub struct Mandelbrot {
    max_iterations: u32,
}

/// CPU Mandelbrot calculator using a signed fixed-point coordinate type.
pub struct MandelbrotFixed<const N: usize> {
    max_iterations: u32,
}

impl<const N: usize> MandelbrotFixed<N> {
    pub fn new(max_iterations: u32) -> Self {
        assert!(max_iterations > 0, "max_iterations must be positive");
        Self { max_iterations }
    }

    /// Returns the iteration at which `c` escapes, or the limit if it does not.
    pub fn escape_iterations(&self, real: Fixed<N>, imaginary: Fixed<N>) -> u32 {
        let zero = Fixed::zero();
        let escape_radius_squared = Fixed::from_i64(4);
        let mut z_real = zero;
        let mut z_imaginary = zero;

        for iteration in 0..self.max_iterations {
            let magnitude_squared = z_real.square().add(z_imaginary.square());
            if magnitude_squared.compare(escape_radius_squared).is_gt() {
                return iteration;
            }

            let next_real = z_real.square().sub(z_imaginary.square()).add(real);
            z_imaginary = z_real
                .mul(z_imaginary)
                .mul(Fixed::from_i64(2))
                .add(imaginary);
            z_real = next_real;
        }

        self.max_iterations
    }

    /// Calculates every pixel in a centered row-major tile using fixed-point
    /// coordinate arithmetic throughout the traversal.
    pub fn render_tile(
        &self,
        center_real: Fixed<N>,
        center_imaginary: Fixed<N>,
        delta: Fixed<N>,
        width: usize,
        height: usize,
    ) -> Vec<u32> {
        assert!(width > 0 && height > 0, "a tile must have a size");
        let center_x = Fixed::from_i64((width.saturating_sub(1) / 2) as i64);
        let center_y = Fixed::from_i64((height.saturating_sub(1) / 2) as i64);
        let mut iterations = Vec::with_capacity(width * height);

        for y in 0..height {
            for x in 0..width {
                let x_offset = Fixed::from_i64(x as i64).sub(center_x);
                let y_offset = center_y.sub(Fixed::from_i64(y as i64));
                let real = center_real.add(delta.mul(x_offset));
                let imaginary = center_imaginary.add(delta.mul(y_offset));
                iterations.push(self.escape_iterations(real, imaginary));
            }
        }

        iterations
    }
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
    use super::{Mandelbrot, MandelbrotFixed};
    use crate::Fixed;

    #[test]
    fn fixed_origin_is_inside_the_set() {
        let fractal = MandelbrotFixed::<2>::new(32);

        assert_eq!(fractal.escape_iterations(Fixed::zero(), Fixed::zero()), 32);
    }

    #[test]
    fn fixed_point_outside_the_set_escapes() {
        let fractal = MandelbrotFixed::<2>::new(32);

        assert!(fractal.escape_iterations(Fixed::from_i64(2), Fixed::zero()) < 32);
    }

    #[test]
    fn fixed_render_tile_calculates_every_pixel_without_float_coordinates() {
        let fractal = MandelbrotFixed::<2>::new(32);
        let iterations =
            fractal.render_tile(Fixed::zero(), Fixed::zero(), Fixed::from_i64(1), 3, 3);

        assert_eq!(iterations.len(), 9);
        assert_eq!(iterations[4], 32);
        assert!(iterations[0] < 32);
    }

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
