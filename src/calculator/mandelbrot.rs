use crate::Fixed;

/// CPU Mandelbrot calculator using `f64` coordinates.
pub struct Mandelbrot {
    max_iterations: u32,
}

/// CPU Mandelbrot calculator using a signed fixed-point coordinate type.
pub struct MandelbrotFixed<const N: usize> {
    max_iterations: u32,
}

#[derive(Debug, Clone)]
pub struct ReferenceOrbit<const N: usize> {
    pub coordinate_real: Fixed<N>,
    pub coordinate_imaginary: Fixed<N>,
    pub real: Vec<Fixed<N>>,
    pub imaginary: Vec<Fixed<N>>,
    pub valid_iterations: u32,
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

    pub fn reference_orbit(
        &self,
        coordinate_real: Fixed<N>,
        coordinate_imaginary: Fixed<N>,
    ) -> ReferenceOrbit<N> {
        let zero = Fixed::zero();
        let escape_radius_squared = Fixed::from_i64(4);
        let mut real = Vec::with_capacity(self.max_iterations as usize + 1);
        let mut imaginary = Vec::with_capacity(self.max_iterations as usize + 1);
        let mut z_real = zero;
        let mut z_imaginary = zero;

        for iteration in 0..self.max_iterations {
            real.push(z_real);
            imaginary.push(z_imaginary);
            if z_real
                .square()
                .add(z_imaginary.square())
                .compare(escape_radius_squared)
                .is_gt()
            {
                return ReferenceOrbit {
                    coordinate_real,
                    coordinate_imaginary,
                    real,
                    imaginary,
                    valid_iterations: iteration,
                };
            }

            let next_real = z_real
                .square()
                .sub(z_imaginary.square())
                .add(coordinate_real);
            z_imaginary = z_real
                .mul(z_imaginary)
                .mul(Fixed::from_i64(2))
                .add(coordinate_imaginary);
            z_real = next_real;
        }

        real.push(z_real);
        imaginary.push(z_imaginary);
        ReferenceOrbit {
            coordinate_real,
            coordinate_imaginary,
            real,
            imaginary,
            valid_iterations: self.max_iterations,
        }
    }

    pub fn perturbation_iterations(
        &self,
        real: Fixed<N>,
        imaginary: Fixed<N>,
        reference: &ReferenceOrbit<N>,
    ) -> Option<u32> {
        let delta_c_real = real.sub(reference.coordinate_real).to_f64();
        let delta_c_imaginary = imaginary.sub(reference.coordinate_imaginary).to_f64();
        if !delta_c_real.is_finite() || !delta_c_imaginary.is_finite() {
            return None;
        }

        let mut delta_z_real = 0.0;
        let mut delta_z_imaginary = 0.0;
        for iteration in 0..self.max_iterations {
            if iteration >= reference.valid_iterations {
                return None;
            }
            let reference_real = reference.real[iteration as usize].to_f64();
            let reference_imaginary = reference.imaginary[iteration as usize].to_f64();
            if !reference_real.is_finite() || !reference_imaginary.is_finite() {
                return None;
            }

            let current_real = reference_real + delta_z_real;
            let current_imaginary = reference_imaginary + delta_z_imaginary;
            let magnitude = current_real * current_real + current_imaginary * current_imaginary;
            if !magnitude.is_finite() || magnitude > 4.0 {
                return Some(iteration);
            }

            let next_real = 2.0
                * (reference_real * delta_z_real - reference_imaginary * delta_z_imaginary)
                + (delta_z_real * delta_z_real - delta_z_imaginary * delta_z_imaginary)
                + delta_c_real;
            let next_imaginary = 2.0
                * (reference_real * delta_z_imaginary + reference_imaginary * delta_z_real)
                + 2.0 * delta_z_real * delta_z_imaginary
                + delta_c_imaginary;
            delta_z_real = next_real;
            delta_z_imaginary = next_imaginary;

            if !delta_z_real.is_finite() || !delta_z_imaginary.is_finite() {
                return None;
            }
            let reference_magnitude =
                reference_real * reference_real + reference_imaginary * reference_imaginary;
            if (delta_z_real.abs() + delta_z_imaginary.abs())
                > 1.0e6 * (1.0 + reference_magnitude.sqrt())
            {
                return None;
            }
        }
        Some(self.max_iterations)
    }

    pub fn render_tile_perturbation(
        &self,
        center_real: Fixed<N>,
        center_imaginary: Fixed<N>,
        delta: Fixed<N>,
        width: usize,
        height: usize,
        reference: &ReferenceOrbit<N>,
        fallback: bool,
    ) -> Vec<Option<u32>> {
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
                let result = self.perturbation_iterations(real, imaginary, reference);
                iterations.push(if fallback {
                    result.or_else(|| Some(self.escape_iterations(real, imaginary)))
                } else {
                    result
                });
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
    fn perturbation_matches_direct_calculation_at_reference_and_nearby_point() {
        let fractal = MandelbrotFixed::<2>::new(64);
        let reference_real = Fixed::zero();
        let reference_imaginary = Fixed::zero();
        let orbit = fractal.reference_orbit(reference_real, reference_imaginary);

        assert_eq!(
            fractal.perturbation_iterations(reference_real, reference_imaginary, &orbit),
            Some(fractal.escape_iterations(reference_real, reference_imaginary))
        );

        let nearby_real = Fixed::from_f64(0.001);
        let nearby_imaginary = Fixed::from_f64(0.001);
        assert_eq!(
            fractal.perturbation_iterations(nearby_real, nearby_imaginary, &orbit),
            Some(fractal.escape_iterations(nearby_real, nearby_imaginary))
        );
    }

    #[test]
    fn perturbation_fallback_option_controls_unstable_pixels() {
        let fractal = MandelbrotFixed::<2>::new(32);
        let reference = fractal.reference_orbit(Fixed::from_i64(2), Fixed::zero());

        let without_fallback = fractal.render_tile_perturbation(
            Fixed::zero(),
            Fixed::zero(),
            Fixed::from_i64(1),
            1,
            1,
            &reference,
            false,
        );
        let with_fallback = fractal.render_tile_perturbation(
            Fixed::zero(),
            Fixed::zero(),
            Fixed::from_i64(1),
            1,
            1,
            &reference,
            true,
        );

        assert_eq!(without_fallback, vec![None]);
        assert_eq!(with_fallback, vec![Some(32)]);
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
