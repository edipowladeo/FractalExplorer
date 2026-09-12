#[cfg(test)]
mod tests {
    use super::{Orchestrator, Tile, TileLayer, TileSprite, TileStatus};
    use crate::Mandelbrot;

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

    #[test]
    fn layer_top_left_maps_to_its_declared_complex_position() {
        let tile = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(99.0, 99.0),
            2,
            2,
            1.0,
        ));
        let mut row = std::collections::VecDeque::new();
        row.push_back(tile);
        let mut tiles = std::collections::VecDeque::new();
        tiles.push_back(row);
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-2.0, 1.5),
            0.5,
            tiles,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );

        assert_eq!(
            layer.complex_point_for_pixel(0, 0),
            crate::geometry::ComplexPoint::new(-2.0, 1.5)
        );
        assert_eq!(
            layer.complex_point_for_pixel(1, 1),
            crate::geometry::ComplexPoint::new(-1.5, 1.0)
        );
    }

    #[test]
    fn adjacent_tiles_share_the_complex_coordinates_at_their_boundary() {
        let left = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            2,
            2,
            1.0,
        ));
        let right = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            2,
            2,
            1.0,
        ));
        let mut row = std::collections::VecDeque::new();
        row.push_back(left);
        row.push_back(right);
        let mut tiles = std::collections::VecDeque::new();
        tiles.push_back(row);
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-2.0, 1.0),
            0.5,
            tiles,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );

        assert_eq!(
            layer.complex_point_for_pixel(2, 0),
            crate::geometry::ComplexPoint::new(-1.0, 1.0)
        );
    }

    #[test]
    fn rendering_a_layer_calculates_its_first_pixel_from_the_layer_top_left() {
        let tile = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            3,
            3,
            1.0,
        ));
        let mut row = std::collections::VecDeque::new();
        row.push_back(std::sync::Arc::clone(&tile));
        let mut tiles = std::collections::VecDeque::new();
        tiles.push_back(row);
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(2.0, 0.0),
            1.0,
            tiles,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );

        Orchestrator::new(Mandelbrot::new(32)).render_layer(&layer);

        assert_eq!(tile.iterations().lock().unwrap()[0], 2);
    }

    #[test]
    fn tile_sprite_keeps_tile_reference_and_top_left_screen_position() {
        let tile = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            200,
            150,
            1.0,
        ));
        let sprite = TileSprite::new(
            std::sync::Arc::clone(&tile),
            crate::geometry::ScreenPoint::new(10, 20),
            1.5,
        );

        assert!(std::sync::Arc::ptr_eq(sprite.tile(), &tile));
        assert_eq!(sprite.position(), crate::geometry::ScreenPoint::new(10, 20));
        assert_eq!(sprite.zoom(), 1.5);
        assert_eq!(sprite.screen_size(), (300, 225));
    }

    #[test]
    fn tile_sprite_zoom_keeps_cursor_position_invariant() {
        let tile = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            100,
            50,
            1.0,
        ));
        let mut sprite = TileSprite::new(tile, crate::geometry::ScreenPoint::new(100, 80), 1.0);

        sprite.zoom_at(crate::geometry::ScreenPoint::new(150, 100), 2.0);

        assert_eq!(sprite.position(), crate::geometry::ScreenPoint::new(50, 60));
        assert_eq!(sprite.zoom(), 2.0);
    }

    #[test]
    fn centered_layer_derives_its_top_left_from_the_center_of_its_pixel_grid() {
        let tile = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            4,
            2,
            1.0,
        ));
        let mut row = std::collections::VecDeque::new();
        row.push_back(tile);
        let mut tiles = std::collections::VecDeque::new();
        tiles.push_back(row);

        let layer = TileLayer::new_centered(
            crate::geometry::ComplexPoint::new(10.0, -5.0),
            0.5,
            tiles,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );

        assert_eq!(
            layer.position(),
            &crate::geometry::ComplexPoint::new(9.25, -4.75)
        );
        assert_eq!(
            layer.complex_point_for_pixel(3, 1),
            crate::geometry::ComplexPoint::new(10.75, -5.25)
        );
    }
}

use std::collections::VecDeque;
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

/// Places one calculated tile in screen space.
pub struct TileSprite {
    tile: Arc<Tile>,
    position: crate::geometry::ScreenPoint,
    zoom: f64,
}

/// A grid of tiles with one shared complex-plane transform.
pub struct TileLayer {
    position: crate::geometry::ComplexPoint<f64>,
    delta: f64,
    tiles: VecDeque<VecDeque<Arc<Tile>>>,
    screen_position: crate::geometry::ScreenPoint,
    zoom: f64,
}

impl TileLayer {
    pub fn new(
        position: crate::geometry::ComplexPoint<f64>,
        delta: f64,
        tiles: VecDeque<VecDeque<Arc<Tile>>>,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        assert!(delta > 0.0, "layer delta must be positive");
        assert!(zoom > 0.0, "layer zoom must be positive");
        assert!(!tiles.is_empty(), "a layer must have at least one row");
        let columns = tiles.front().map_or(0, VecDeque::len);
        assert!(columns > 0, "a layer must have at least one column");
        assert!(
            tiles.iter().all(|row| row.len() == columns),
            "layer rows must have equal lengths"
        );
        Self {
            position,
            delta,
            tiles,
            screen_position,
            zoom,
        }
    }

    /// Creates a layer whose geometric center is the supplied complex coordinate.
    pub fn new_centered(
        center: crate::geometry::ComplexPoint<f64>,
        delta: f64,
        tiles: VecDeque<VecDeque<Arc<Tile>>>,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        let width = tiles
            .front()
            .map(|row| row.iter().map(|tile| tile.width()).sum::<u32>())
            .unwrap_or(0);
        let height = tiles
            .iter()
            .map(|row| row.front().map(|tile| tile.height()).unwrap_or(0))
            .sum::<u32>();
        let position = crate::geometry::ComplexPoint::new(
            center.x - (width.saturating_sub(1) as f64 / 2.0) * delta,
            center.y + (height.saturating_sub(1) as f64 / 2.0) * delta,
        );
        Self::new(position, delta, tiles, screen_position, zoom)
    }

    pub fn position(&self) -> &crate::geometry::ComplexPoint<f64> {
        &self.position
    }
    pub fn delta(&self) -> f64 {
        self.delta
    }

    /// Maps a logical pixel offset from the layer's top-left corner to the complex plane.
    pub fn complex_point_for_pixel(&self, x: u32, y: u32) -> crate::geometry::ComplexPoint<f64> {
        crate::geometry::ComplexPoint::new(
            self.position.x + x as f64 * self.delta,
            self.position.y - y as f64 * self.delta,
        )
    }
    pub fn row_count(&self) -> usize {
        self.tiles.len()
    }
    pub fn column_count(&self) -> usize {
        self.tiles.front().map_or(0, VecDeque::len)
    }
    pub fn tile(&self, row: usize, column: usize) -> Option<&Arc<Tile>> {
        self.tiles.get(row)?.get(column)
    }
    pub fn tile_pixel_origin(&self, row: usize, column: usize) -> Option<(u32, u32)> {
        self.tile(row, column)?;
        let x = self
            .tiles
            .get(row)?
            .iter()
            .take(column)
            .map(|tile| tile.width())
            .sum();
        let y = self
            .tiles
            .iter()
            .take(row)
            .map(|row| row.front().expect("layer rows must not be empty").height())
            .sum();
        Some((x, y))
    }
    pub fn screen_position(&self) -> crate::geometry::ScreenPoint {
        self.screen_position
    }
    pub fn zoom(&self) -> f64 {
        self.zoom
    }
    pub fn screen_size(&self) -> (u32, u32) {
        let width = self
            .tiles
            .front()
            .expect("layer must contain a tile")
            .iter()
            .map(|tile| tile.width())
            .sum::<u32>();
        let height = self
            .tiles
            .iter()
            .map(|row| row.front().expect("layer rows must not be empty").height())
            .sum::<u32>();
        (
            ((width as f64 * self.zoom).round() as u32).max(1),
            ((height as f64 * self.zoom).round() as u32).max(1),
        )
    }
    pub fn set_screen_position(&mut self, position: crate::geometry::ScreenPoint) {
        self.screen_position = position;
    }

    pub fn zoom_at(&mut self, cursor: crate::geometry::ScreenPoint, zoom: f64) {
        assert!(zoom > 0.0, "layer zoom must be positive");
        let scale = zoom / self.zoom;
        self.screen_position = crate::geometry::ScreenPoint::new(
            (cursor.x as f64 - (cursor.x - self.screen_position.x) as f64 * scale).round() as i32,
            (cursor.y as f64 - (cursor.y - self.screen_position.y) as f64 * scale).round() as i32,
        );
        self.zoom = zoom;
    }
}

impl TileSprite {
    pub fn new(tile: Arc<Tile>, position: crate::geometry::ScreenPoint, zoom: f64) -> Self {
        assert!(zoom > 0.0, "sprite zoom must be positive");
        Self {
            tile,
            position,
            zoom,
        }
    }

    pub fn tile(&self) -> &Arc<Tile> {
        &self.tile
    }

    pub fn position(&self) -> crate::geometry::ScreenPoint {
        self.position
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        assert!(zoom > 0.0, "sprite zoom must be positive");
        self.zoom = zoom;
    }

    pub fn zoom_at(&mut self, cursor: crate::geometry::ScreenPoint, zoom: f64) {
        assert!(zoom > 0.0, "sprite zoom must be positive");
        let scale = zoom / self.zoom;
        self.position = crate::geometry::ScreenPoint::new(
            (cursor.x as f64 - (cursor.x - self.position.x) as f64 * scale).round() as i32,
            (cursor.y as f64 - (cursor.y - self.position.y) as f64 * scale).round() as i32,
        );
        self.zoom = zoom;
    }

    pub fn screen_size(&self) -> (u32, u32) {
        (
            ((self.tile.width as f64 * self.zoom).round() as u32).max(1),
            ((self.tile.height as f64 * self.zoom).round() as u32).max(1),
        )
    }

    pub fn set_position(&mut self, position: crate::geometry::ScreenPoint) {
        self.position = position;
    }
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

/// Dispatches tiles to the calculator.
pub struct Orchestrator {
    calculator: crate::Mandelbrot,
}

impl Orchestrator {
    pub fn new(calculator: crate::Mandelbrot) -> Self {
        Self { calculator }
    }

    pub fn render_tile(&self, tile: &Tile) {
        self.render_tile_with_delta(tile, tile.delta);
    }

    fn render_tile_with_delta(&self, tile: &Tile, delta: f64) {
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
                let real = tile.coordinate.x + (x as f64 - center_x) * delta;
                let imaginary = tile.coordinate.y - (y as f64 - center_y) * delta;
                iterations[y as usize * tile.width as usize + x as usize] =
                    self.calculator.escape_iterations(real, imaginary) as u64;
            }
        }
        tile.status
            .store(TileStatus::Completed as u8, Ordering::Release);
    }

    fn render_tile_from_top_left(
        &self,
        tile: &Tile,
        top_left: crate::geometry::ComplexPoint<f64>,
        delta: f64,
    ) {
        tile.status
            .store(TileStatus::Dispatched as u8, Ordering::Release);
        let mut iterations = tile
            .iterations
            .lock()
            .expect("tile iterations mutex poisoned");
        for y in 0..tile.height {
            for x in 0..tile.width {
                let real = top_left.x + x as f64 * delta;
                let imaginary = top_left.y - y as f64 * delta;
                iterations[y as usize * tile.width as usize + x as usize] =
                    self.calculator.escape_iterations(real, imaginary) as u64;
            }
        }
        tile.status
            .store(TileStatus::Completed as u8, Ordering::Release);
    }

    pub fn render_layer(&self, layer: &TileLayer) {
        for (row_index, row) in layer.tiles.iter().enumerate() {
            for (column_index, tile) in row.iter().enumerate() {
                let (x, y) = layer
                    .tile_pixel_origin(row_index, column_index)
                    .expect("tile must belong to its layer");
                self.render_tile_from_top_left(
                    tile,
                    layer.complex_point_for_pixel(x, y),
                    layer.delta,
                );
            }
        }
    }
}
