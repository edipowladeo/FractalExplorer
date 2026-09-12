#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
    fn completed_tile_is_not_recalculated() {
        let tile = Tile::new(crate::geometry::ComplexPoint::new(0.0, 0.0), 1, 1, 1.0);
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));
        orchestrator.render_tile(&tile);
        tile.iterations().lock().unwrap()[0] = 777;

        orchestrator.render_tile(&tile);

        assert_eq!(tile.iterations().lock().unwrap()[0], 777);
        assert_eq!(tile.status(), TileStatus::Completed);
    }

    #[test]
    fn tile_keeps_one_immutable_sprite_reference() {
        let tile = Tile::new(crate::geometry::ComplexPoint::new(0.0, 0.0), 1, 1, 1.0);
        let sprite = Arc::new(crate::Sprite::solid(1, 1, 0xff00ff));
        tile.set_sprite(Arc::clone(&sprite));

        let stored = tile.sprite().expect("tile sprite should exist");

        assert!(Arc::ptr_eq(&stored, &sprite));
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
    fn tile_layer_expands_grid_until_screen_bounds_are_covered() {
        let mut layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(4, 4),
            1.0,
        );

        layer.ensure_screen_coverage((0, 0, 7, 7));

        assert_eq!((layer.row_count(), layer.column_count()), (4, 4));
        assert_eq!(
            layer.screen_position(),
            crate::geometry::ScreenPoint::new(0, 0)
        );
        assert_eq!((layer.tile_width(), layer.tile_height()), (2, 2));
        assert_eq!(layer.tile(3, 3).unwrap().width(), 2);
        assert_eq!(layer.tile(3, 3).unwrap().delta(), 1.0);

        layer.set_position(crate::geometry::ComplexPoint::new(-2.0, 2.0));
        assert_eq!(
            layer.position(),
            &crate::geometry::ComplexPoint::new(-2.0, 2.0)
        );
    }

    #[test]
    fn tile_layer_removes_tiles_that_are_fully_outside_screen_bounds() {
        let mut layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );
        layer.ensure_screen_coverage((0, 0, 7, 7));
        layer.set_screen_position(crate::geometry::ScreenPoint::new(-8, -8));

        layer.trim_outside_allocation((0, 0, 7, 7));

        assert_eq!((layer.row_count(), layer.column_count()), (1, 1));
        assert_eq!(
            layer.screen_position(),
            crate::geometry::ScreenPoint::new(-2, -2)
        );
    }

    #[test]
    fn tile_layer_composes_its_initial_tile_without_receiving_one() {
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            200,
            150,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            1.0,
        );

        assert_eq!((layer.row_count(), layer.column_count()), (1, 1));
        assert_eq!((layer.tile_width(), layer.tile_height()), (200, 150));
        assert_eq!(layer.tile(0, 0).unwrap().width(), 200);
        assert_eq!(layer.tile(0, 0).unwrap().delta(), 0.01);
    }

    #[test]
    fn tile_layer_can_be_composed_from_one_tile() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            200,
            150,
            0.01,
        ));
        let layer = TileLayer::from_tile(
            tile.clone(),
            crate::geometry::ScreenPoint::new(300, 225),
            1.0,
        );

        assert!(Arc::ptr_eq(layer.tile(0, 0).unwrap(), &tile));
        assert_eq!(layer.delta(), 0.01);
        assert_eq!(
            layer.position(),
            &crate::geometry::ComplexPoint::new(-0.995, 0.745)
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
    sprite: Arc<Mutex<Option<Arc<crate::Sprite>>>>,
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
    tile_width: u32,
    tile_height: u32,
    delta: f64,
    tiles: VecDeque<VecDeque<Arc<Tile>>>,
    screen_position: crate::geometry::ScreenPoint,
    zoom: f64,
}

impl TileLayer {
    pub fn from_tile(
        tile: Arc<Tile>,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        let delta = tile.delta();
        let position = crate::geometry::ComplexPoint::new(
            tile.coordinate().x - (tile.width() - 1) as f64 * delta / 2.0,
            tile.coordinate().y + (tile.height() - 1) as f64 * delta / 2.0,
        );
        let mut row = VecDeque::new();
        row.push_back(tile.clone());
        let mut tiles = VecDeque::new();
        tiles.push_back(row);
        Self::from_grid(
            position,
            tile.width(),
            tile.height(),
            delta,
            tiles,
            screen_position,
            zoom,
        )
    }

    pub fn new(
        position: crate::geometry::ComplexPoint<f64>,
        tile_width: u32,
        tile_height: u32,
        delta: f64,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        let center = crate::geometry::ComplexPoint::new(
            position.x + (tile_width - 1) as f64 * delta / 2.0,
            position.y - (tile_height - 1) as f64 * delta / 2.0,
        );
        let tile = Arc::new(Tile::new(center, tile_width, tile_height, delta));
        let mut row = VecDeque::new();
        row.push_back(tile);
        let mut tiles = VecDeque::new();
        tiles.push_back(row);
        Self::from_grid(
            position,
            tile_width,
            tile_height,
            delta,
            tiles,
            screen_position,
            zoom,
        )
    }

    fn from_grid(
        position: crate::geometry::ComplexPoint<f64>,
        tile_width: u32,
        tile_height: u32,
        delta: f64,
        tiles: VecDeque<VecDeque<Arc<Tile>>>,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        assert!(delta > 0.0, "layer delta must be positive");
        assert!(
            tile_width > 0 && tile_height > 0,
            "layer tile size must be positive"
        );
        assert!(zoom > 0.0, "layer zoom must be positive");
        assert!(!tiles.is_empty(), "a layer must have at least one row");
        let columns = tiles.front().map_or(0, VecDeque::len);
        assert!(columns > 0, "a layer must have at least one column");
        assert!(
            tiles.iter().all(|row| row.len() == columns),
            "layer rows must have equal lengths"
        );
        assert!(
            tiles
                .iter()
                .flatten()
                .all(|tile| tile.width() == tile_width && tile.height() == tile_height),
            "layer tile sizes must match"
        );
        Self {
            position,
            tile_width,
            tile_height,
            delta,
            tiles,
            screen_position,
            zoom,
        }
    }

    pub fn position(&self) -> &crate::geometry::ComplexPoint<f64> {
        &self.position
    }
    pub fn delta(&self) -> f64 {
        self.delta
    }
    pub fn tile_width(&self) -> u32 {
        self.tile_width
    }
    pub fn tile_height(&self) -> u32 {
        self.tile_height
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
    pub fn screen_position(&self) -> crate::geometry::ScreenPoint {
        self.screen_position
    }
    pub fn zoom(&self) -> f64 {
        self.zoom
    }
    pub fn screen_size(&self) -> (u32, u32) {
        (
            ((self.tile_width as f64 * self.zoom).round() as u32).max(1)
                * self.column_count() as u32,
            ((self.tile_height as f64 * self.zoom).round() as u32).max(1) * self.row_count() as u32,
        )
    }
    pub fn tile_screen_size(&self) -> (u32, u32) {
        (
            ((self.tile_width as f64 * self.zoom).round() as u32).max(1),
            ((self.tile_height as f64 * self.zoom).round() as u32).max(1),
        )
    }
    pub fn set_screen_position(&mut self, position: crate::geometry::ScreenPoint) {
        self.screen_position = position;
    }
    pub fn set_position(&mut self, position: crate::geometry::ComplexPoint<f64>) {
        self.position = position;
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

    pub fn ensure_screen_coverage(&mut self, bounds: (i32, i32, i32, i32)) {
        let (left, top, right, bottom) = bounds;
        let (tile_width, tile_height) = self.tile_screen_size();
        let (complex_width, complex_height) = {
            let tile = self.tile(0, 0).expect("layer must contain a tile");
            (
                tile.width() as f64 * self.delta,
                tile.height() as f64 * self.delta,
            )
        };

        while self.screen_position.x > left {
            self.position.x -= complex_width;
            self.screen_position.x -= tile_width as i32;
            for row_index in 0..self.row_count() {
                let tile = self.make_tile(row_index, 0);
                self.tiles[row_index].push_front(tile);
            }
        }
        while self.screen_position.y > top {
            self.position.y += complex_height;
            self.screen_position.y -= tile_height as i32;
            let mut row = VecDeque::new();
            for column_index in 0..self.column_count() {
                row.push_back(self.make_tile(0, column_index));
            }
            self.tiles.push_front(row);
        }
        while self.screen_position.x + self.screen_size().0 as i32 - 1 < right {
            let column_index = self.column_count();
            for row_index in 0..self.row_count() {
                let tile = self.make_tile(row_index, column_index);
                self.tiles[row_index].push_back(tile);
            }
        }
        while self.screen_position.y + self.screen_size().1 as i32 - 1 < bottom {
            let row_index = self.row_count();
            let mut row = VecDeque::new();
            for column_index in 0..self.column_count() {
                row.push_back(self.make_tile(row_index, column_index));
            }
            self.tiles.push_back(row);
        }
    }

    pub fn trim_outside_allocation(&mut self, bounds: (i32, i32, i32, i32)) {
        let (left, top, right, bottom) = bounds;
        let (tile_width, tile_height) = self.tile_screen_size();
        let complex_width = self.tile_width as f64 * self.delta;
        let complex_height = self.tile_height as f64 * self.delta;

        while self.column_count() > 1 && self.screen_position.x + tile_width as i32 - 1 < left {
            for row in &mut self.tiles {
                row.pop_front();
            }
            self.position.x += complex_width;
            self.screen_position.x += tile_width as i32;
        }
        while self.column_count() > 1
            && self.screen_position.x + (self.column_count() as i32 - 1) * tile_width as i32 > right
        {
            for row in &mut self.tiles {
                row.pop_back();
            }
        }
        while self.row_count() > 1 && self.screen_position.y + tile_height as i32 - 1 < top {
            self.tiles.pop_front();
            self.position.y -= complex_height;
            self.screen_position.y += tile_height as i32;
        }
        while self.row_count() > 1
            && self.screen_position.y + (self.row_count() as i32 - 1) * tile_height as i32 > bottom
        {
            self.tiles.pop_back();
        }
    }

    fn make_tile(&self, row: usize, column: usize) -> Arc<Tile> {
        let x = self.position.x
            + (column as f64 * self.tile_width as f64 + (self.tile_width - 1) as f64 / 2.0)
                * self.delta;
        let y = self.position.y
            - (row as f64 * self.tile_height as f64 + (self.tile_height - 1) as f64 / 2.0)
                * self.delta;
        Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(x, y),
            self.tile_width,
            self.tile_height,
            self.delta,
        ))
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
            sprite: Arc::new(Mutex::new(None)),
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

    pub fn replace_iterations(&self, values: Vec<u64>) {
        assert_eq!(
            values.len(),
            self.width as usize * self.height as usize,
            "fixed render result has the wrong size"
        );
        *self
            .iterations
            .lock()
            .expect("tile iterations mutex poisoned") = values;
        self.status
            .store(TileStatus::Completed as u8, Ordering::Release);
    }

    pub fn sprite(&self) -> Option<Arc<crate::Sprite>> {
        self.sprite
            .lock()
            .expect("tile sprite mutex poisoned")
            .clone()
    }

    pub fn set_sprite(&self, sprite: Arc<crate::Sprite>) {
        let mut stored = self.sprite.lock().expect("tile sprite mutex poisoned");
        if stored.is_none() {
            *stored = Some(sprite);
        }
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
        if tile.status() == TileStatus::Completed {
            return;
        }
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

    pub fn render_layer(&self, layer: &TileLayer) {
        for row in &layer.tiles {
            for tile in row {
                self.render_tile_with_delta(tile, layer.delta);
            }
        }
    }
}
