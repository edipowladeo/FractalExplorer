#[cfg(test)]
mod tests {
    use super::{Orchestrator, RenderConfig};
    use crate::Mandelbrot;

    #[test]
    fn scans_the_configured_width_and_height() {
        let config = RenderConfig::centered(9, 5, 4.0);
        let image = Orchestrator::new(Mandelbrot::new(32)).render(config);

        assert_eq!(image.width(), 9);
        assert_eq!(image.height(), 5);
        assert_eq!(image.iterations().len(), 45);
    }

    #[test]
    fn maps_the_center_pixel_to_the_origin() {
        let config = RenderConfig::centered(9, 9, 4.0);
        let image = Orchestrator::new(Mandelbrot::new(32)).render(config);

        assert_eq!(image.iterations()[4 * 9 + 4], 32);
    }

    #[test]
    fn maps_the_center_pixel_to_the_configured_center() {
        let config =
            RenderConfig::centered_at(9, 9, 4.0, crate::geometry::ComplexPoint::new(2.0, 0.0));
        let image = Orchestrator::new(Mandelbrot::new(32)).render(config);

        assert!(image.iterations()[4 * 9 + 4] < 32);
    }
}

/// Viewport and output dimensions used by the orchestrator.
pub struct RenderConfig {
    pub width: usize,
    pub height: usize,
    pub view_width: f64,
    pub center: crate::geometry::ComplexPoint<f64>,
}

impl RenderConfig {
    pub fn centered(width: usize, height: usize, view_width: f64) -> Self {
        Self::centered_at(
            width,
            height,
            view_width,
            crate::geometry::ComplexPoint::new(0.0, 0.0),
        )
    }

    pub fn centered_at(
        width: usize,
        height: usize,
        view_width: f64,
        center: crate::geometry::ComplexPoint<f64>,
    ) -> Self {
        assert!(width > 0 && height > 0, "a render target must have a size");
        assert!(view_width > 0.0, "view_width must be positive");
        Self {
            width,
            height,
            view_width,
            center,
        }
    }
}

/// Walks the output pixels and delegates each coordinate to the calculator.
pub struct Orchestrator {
    calculator: crate::Mandelbrot,
}

impl Orchestrator {
    pub fn new(calculator: crate::Mandelbrot) -> Self {
        Self { calculator }
    }

    pub fn render(&self, config: RenderConfig) -> IterationBuffer {
        let aspect_ratio = config.height as f64 / config.width as f64;
        let mut iterations = Vec::with_capacity(config.width * config.height);

        for pixel_y in 0..config.height {
            for pixel_x in 0..config.width {
                let real = config.center.x
                    + ((pixel_x as f64 + 0.5) / config.width as f64 - 0.5) * config.view_width;
                let imaginary = ((pixel_y as f64 + 0.5) / config.height as f64 - 0.5)
                    * config.view_width
                    * aspect_ratio
                    + config.center.y;
                iterations.push(self.calculator.escape_iterations(real, imaginary));
            }
        }

        IterationBuffer {
            width: config.width,
            height: config.height,
            max_iterations: self.calculator.max_iterations(),
            iterations,
        }
    }
}

pub struct IterationBuffer {
    width: usize,
    height: usize,
    max_iterations: u32,
    iterations: Vec<u32>,
}

impl IterationBuffer {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn max_iterations(&self) -> u32 {
        self.max_iterations
    }

    pub fn iterations(&self) -> &[u32] {
        &self.iterations
    }
}
