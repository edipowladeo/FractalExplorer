#[cfg(test)]
mod tests {
    use super::{Orchestrator, RenderConfig, Tile, TileStatus};
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

    #[test]
    fn tile_starts_not_started_and_owns_u64_iteration_storage() {
        let tile = Tile::new(crate::geometry::ComplexPoint::new(1.0, -2.0), 3, 2, 0.25);

        assert_eq!(tile.status(), TileStatus::NotStarted);
        assert_eq!(tile.width(), 3);
        assert_eq!(tile.height(), 2);
        assert_eq!(tile.delta(), 0.25);
        assert_eq!(tile.iterations().lock().unwrap().len(), 6);
        assert_eq!(tile.iterations().lock().unwrap()[0], 0u64);
    }

    #[test]
    fn orchestrator_dispatches_and_completes_a_tile() {
        let tile = Tile::new(crate::geometry::ComplexPoint::new(0.0, 0.0), 3, 3, 1.0);
        Orchestrator::new(Mandelbrot::new(32)).render_tile(&tile);

        assert_eq!(tile.status(), TileStatus::Completed);
        assert_eq!(tile.iterations().lock().unwrap()[4], 32u64);
    }
}

use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Mutex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TileStatus {
    NotStarted = 0,
    Dispatched = 1,
    Completed = 2,
}

impl TileStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Dispatched,
            2 => Self::Completed,
            _ => Self::NotStarted,
        }
    }
}

/// A fixed-size region of the complex plane and its calculation result.
pub struct Tile {
    coordinate: crate::geometry::ComplexPoint<f64>,
    width: u32,
    height: u32,
    delta: f64,
    status: AtomicU8,
    iterations: Arc<Mutex<Vec<u64>>>,
}

impl Tile {
    pub fn new(
        coordinate: crate::geometry::ComplexPoint<f64>,
        width: u32,
        height: u32,
        delta: f64,
    ) -> Self {
        assert!(width > 0 && height > 0, "a tile must have a size");
        assert!(delta > 0.0, "tile delta must be positive");
        Self {
            coordinate,
            width,
            height,
            delta,
            status: AtomicU8::new(TileStatus::NotStarted as u8),
            iterations: Arc::new(Mutex::new(vec![0; width as usize * height as usize])),
        }
    }

    pub fn coordinate(&self) -> &crate::geometry::ComplexPoint<f64> {
        &self.coordinate
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn delta(&self) -> f64 {
        self.delta
    }

    pub fn status(&self) -> TileStatus {
        TileStatus::from_u8(self.status.load(Ordering::Acquire))
    }

    pub fn iterations(&self) -> Arc<Mutex<Vec<u64>>> {
        Arc::clone(&self.iterations)
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
                iterations.push(self.calculator.escape_iterations(real, imaginary) as u64);
            }
        }

        IterationBuffer {
            width: config.width,
            height: config.height,
            max_iterations: self.calculator.max_iterations() as u64,
            iterations,
        }
    }

    pub fn render_tile(&self, tile: &Tile) {
        tile.status
            .store(TileStatus::Dispatched as u8, Ordering::Release);
        let mut iterations = tile
            .iterations
            .lock()
            .expect("tile iterations mutex poisoned");
        let center_x = (tile.width - 1) as f64 / 2.0;
        let center_y = (tile.height - 1) as f64 / 2.0;
        for y in 0..tile.height {
            for x in 0..tile.width {
                let real = tile.coordinate.x + (x as f64 - center_x) * tile.delta;
                let imaginary = tile.coordinate.y - (y as f64 - center_y) * tile.delta;
                iterations[y as usize * tile.width as usize + x as usize] =
                    self.calculator.escape_iterations(real, imaginary) as u64;
            }
        }
        tile.status
            .store(TileStatus::Completed as u8, Ordering::Release);
    }
}

pub struct IterationBuffer {
    width: usize,
    height: usize,
    max_iterations: u64,
    iterations: Vec<u64>,
}

impl IterationBuffer {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn max_iterations(&self) -> u64 {
        self.max_iterations
    }

    pub fn iterations(&self) -> &[u64] {
        &self.iterations
    }
}
