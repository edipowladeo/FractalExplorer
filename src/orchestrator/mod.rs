#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Orchestrator, Tile, TileLayer, TileSprite, TileStatus, TiledInfiniteCanvas};
    use crate::Mandelbrot;
    use std::{thread, time::Duration};

    fn wait_for_completion(tile: &Tile) {
        for _ in 0..1000 {
            if tile.status() == TileStatus::Completed {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("tile worker did not complete the tile");
    }

    #[test]
    fn tile_starts_not_started_and_owns_u64_iteration_storage() {
        let tile = Tile::new(crate::geometry::ComplexPoint::new(1.0, -2.0), 3, 2, 0.25);

        assert_eq!(
            tile.coordinate(),
            &crate::geometry::ComplexPoint::new(1.0, -2.0)
        );
        assert_eq!(tile.status(), TileStatus::NotStarted);
        assert_eq!(tile.width(), 3);
        assert_eq!(tile.height(), 2);
        assert_eq!(tile.delta(), 0.25);
        assert_eq!(tile.iterations().lock().unwrap().len(), 6);
        assert_eq!(tile.iterations().lock().unwrap()[0], 0u64);
    }

    #[test]
    fn destroyed_tile_is_removed_from_pending_work_queue() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            1,
            1,
            1.0,
        ));
        let queue = super::TileWorkQueue::new();

        queue.enqueue(Arc::clone(&tile), 2, 4);

        assert!(queue.remove_tile(&tile));
        assert!(queue.pop_front().is_none());
    }

    #[test]
    fn destroying_a_layer_deactivates_its_work_queue() {
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );
        let queue = Arc::clone(&layer.work_queue);

        assert!(queue.is_active());
        drop(layer);
        assert!(!queue.is_active());
    }

    #[test]
    fn layer_position_is_redirected_to_the_top_left_tile() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(-2.0, 3.0),
            2,
            2,
            0.5,
        ));
        let layer =
            TileLayer::from_tile(tile.clone(), crate::geometry::ScreenPoint::new(0, 0), 1.0);

        assert!(Arc::ptr_eq(layer.corner_tile().unwrap(), &tile));
        assert_eq!(layer.position(), tile.coordinate());
    }

    #[test]
    fn layer_center_tile_uses_floor_of_half_dimensions() {
        let mut layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );
        layer.ensure_screen_coverage((0, 0, 3, 3));

        assert_eq!(
            layer.center_tile().unwrap().coordinate(),
            &crate::geometry::ComplexPoint::new(2.0, -2.0)
        );
    }

    #[test]
    fn orchestrator_enqueues_and_worker_completes_a_tile() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            3,
            3,
            1.0,
        ));
        Orchestrator::new(Mandelbrot::new(32)).render_tile(&tile);

        wait_for_completion(&tile);
        assert_eq!(tile.status(), TileStatus::Completed);
        assert_eq!(tile.iterations().lock().unwrap()[4], 32u64);
    }

    #[test]
    fn orchestrator_exposes_the_worker_pool() {
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));

        assert_eq!(orchestrator.worker_statuses().len(), 8);
        assert!(orchestrator
            .worker_statuses()
            .iter()
            .all(|worker| worker.tile.is_none()));
    }

    #[test]
    fn new_tile_is_deferred_before_worker_completes_it() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            3,
            3,
            1.0,
        ));
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));

        orchestrator.render_tile(&tile);

        assert!(matches!(
            tile.status(),
            TileStatus::Deferred | TileStatus::Completed
        ));
        wait_for_completion(&tile);
    }

    #[test]
    fn completed_tile_is_not_recalculated() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            1,
            1,
            1.0,
        ));
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));
        orchestrator.render_tile(&tile);
        wait_for_completion(&tile);
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

        assert_eq!(
            layer.position(),
            &crate::geometry::ComplexPoint::new(-5.0, 5.0)
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
            &crate::geometry::ComplexPoint::new(0.0, 0.0)
        );
    }

    #[test]
    fn tiled_infinite_canvas_expands_by_one_layer_per_frame() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );

        assert_eq!(canvas.layer_count(), 0);
        assert!(canvas.expand_one_layer_per_frame());
        assert_eq!(canvas.layer_count(), 1);
        assert_eq!(canvas.layer(0).unwrap().zoom(), 8.0);
        assert!(canvas.expand_one_layer_per_frame());
        assert_eq!(canvas.layer_count(), 2);
        assert_eq!(canvas.layer(1).unwrap().zoom(), 4.0);
        assert_eq!(canvas.layer(1).unwrap().delta(), 0.005);
        assert_eq!(canvas.layer(1).unwrap().position().x, -0.9275);
        assert_eq!(canvas.layer(1).unwrap().position().y, 0.9525);
        assert_eq!(
            canvas.layer(1).unwrap().screen_position(),
            crate::geometry::ScreenPoint::new(360, 265)
        );

        canvas.zoom_at(crate::geometry::ScreenPoint::new(400, 300), 16.0);
        assert_eq!(canvas.layer(0).unwrap().zoom(), 16.0);
        assert_eq!(canvas.layer(1).unwrap().zoom(), 8.0);
    }
}

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc, Condvar, Mutex,
};
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TileStatus {
    NotStarted = 0,
    Deferred = 1,
    Completed = 2,
}

impl TileStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Deferred,
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
    tile_width: u32,
    tile_height: u32,
    delta: f64,
    tiles: VecDeque<VecDeque<Arc<Tile>>>,
    screen_position: crate::geometry::ScreenPoint,
    zoom: f64,
    work_queue: Arc<TileWorkQueue>,
}

impl Drop for TileLayer {
    fn drop(&mut self) {
        self.work_queue.deactivate();
    }
}

/// Ordered collection of fractal layers at progressively smaller apparent pixels.
pub struct TiledInfiniteCanvas {
    layers: VecDeque<TileLayer>,
    position: crate::geometry::ComplexPoint<f64>,
    tile_width: u32,
    tile_height: u32,
    delta: f64,
    screen_position: crate::geometry::ScreenPoint,
    max_apparent_pixel_size: f64,
    min_apparent_pixel_size: f64,
}

impl TiledInfiniteCanvas {
    pub fn new(
        position: crate::geometry::ComplexPoint<f64>,
        tile_width: u32,
        tile_height: u32,
        delta: f64,
        screen_position: crate::geometry::ScreenPoint,
        max_apparent_pixel_size: f64,
        min_apparent_pixel_size: f64,
    ) -> Self {
        assert!(max_apparent_pixel_size > 0.0);
        assert!(min_apparent_pixel_size > 0.0);
        Self {
            layers: VecDeque::new(),
            position,
            tile_width,
            tile_height,
            delta,
            screen_position,
            max_apparent_pixel_size,
            min_apparent_pixel_size,
        }
    }

    /// Adds at most one layer per frame, keeping the deque ordered max-to-min zoom.
    pub fn expand_one_layer_per_frame(&mut self) -> bool {
        if self.layers.is_empty() {
            self.layers.push_back(TileLayer::new(
                self.position.clone(),
                self.tile_width,
                self.tile_height,
                self.delta,
                self.screen_position,
                self.max_apparent_pixel_size,
            ));
            return true;
        }

        let front_zoom = self.layers.front().unwrap().zoom();
        let larger_zoom = front_zoom * 2.0;
        if larger_zoom <= self.max_apparent_pixel_size && !self.has_zoom(larger_zoom) {
            let layer = Self::adjacent_layer(self.layers.front().unwrap(), 2.0);
            self.layers.push_front(layer);
            return true;
        }
        let back_zoom = self.layers.back().unwrap().zoom();
        let smaller_zoom = back_zoom / 2.0;
        if smaller_zoom >= self.min_apparent_pixel_size && !self.has_zoom(smaller_zoom) {
            let layer = Self::adjacent_layer(self.layers.back().unwrap(), 0.5);
            self.layers.push_back(layer);
            return true;
        }
        false
    }

    fn has_zoom(&self, candidate: f64) -> bool {
        self.layers.iter().any(|layer| {
            let scale = candidate.abs().max(layer.zoom().abs()).max(1.0);
            (layer.zoom() - candidate).abs() <= scale * f64::EPSILON * 8.0
        })
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn layer(&self, index: usize) -> Option<&TileLayer> {
        self.layers.get(index)
    }

    pub fn layer_mut(&mut self, index: usize) -> Option<&mut TileLayer> {
        self.layers.get_mut(index)
    }

    pub fn layers(&self) -> &VecDeque<TileLayer> {
        &self.layers
    }

    pub fn layers_mut(&mut self) -> &mut VecDeque<TileLayer> {
        &mut self.layers
    }

    pub fn ensure_screen_coverage(&mut self, bounds: (i32, i32, i32, i32)) {
        if self.retract_one_layer_per_frame() && self.layers.is_empty() {
            self.expand_one_layer_per_frame();
        } else {
            self.expand_one_layer_per_frame();
        }
        for layer in &mut self.layers {
            layer.ensure_screen_coverage(bounds);
        }
    }

    fn retract_one_layer_per_frame(&mut self) -> bool {
        if self
            .layers
            .front()
            .is_some_and(|layer| layer.zoom() > self.max_apparent_pixel_size)
        {
            self.layers.pop_front();
            return true;
        }
        if self
            .layers
            .back()
            .is_some_and(|layer| layer.zoom() < self.min_apparent_pixel_size)
        {
            self.layers.pop_back();
            return true;
        }
        false
    }

    fn adjacent_layer(tip: &TileLayer, zoom_factor: f64) -> TileLayer {
        let zoom = tip.zoom() * zoom_factor;
        let delta = tip.delta() * zoom_factor;
        let center = crate::geometry::ComplexPoint::new(
            tip.position().x + (tip.tile_width() - 1) as f64 * tip.delta() / 2.0,
            tip.position().y - (tip.tile_height() - 1) as f64 * tip.delta() / 2.0,
        );
        let position = crate::geometry::ComplexPoint::new(
            center.x - (tip.tile_width() - 1) as f64 * delta / 2.0,
            center.y + (tip.tile_height() - 1) as f64 * delta / 2.0,
        );
        let screen_center = crate::geometry::ScreenPoint::new(
            tip.screen_position().x
                + ((tip.tile_width() as f64 * tip.zoom() - 1.0) / 2.0).round() as i32,
            tip.screen_position().y
                + ((tip.tile_height() as f64 * tip.zoom() - 1.0) / 2.0).round() as i32,
        );
        let screen_position = crate::geometry::ScreenPoint::new(
            screen_center.x - ((tip.tile_width() as f64 * zoom - 1.0) / 2.0).round() as i32,
            screen_center.y - ((tip.tile_height() as f64 * zoom - 1.0) / 2.0).round() as i32,
        );
        TileLayer::new(
            position,
            tip.tile_width(),
            tip.tile_height(),
            delta,
            screen_position,
            zoom,
        )
    }

    pub fn trim_outside_allocation(&mut self, bounds: (i32, i32, i32, i32)) {
        for layer in &mut self.layers {
            layer.trim_outside_allocation(bounds);
        }
    }

    pub fn drag(&mut self, delta: crate::geometry::ScreenPoint) {
        for layer in &mut self.layers {
            layer.set_screen_position(crate::geometry::ScreenPoint::new(
                layer.screen_position().x + delta.x,
                layer.screen_position().y + delta.y,
            ));
        }
    }

    pub fn zoom_at(&mut self, cursor: crate::geometry::ScreenPoint, zoom: f64) {
        let current_zoom = self
            .layers
            .front()
            .map_or(self.max_apparent_pixel_size, TileLayer::zoom);
        let scale = zoom / current_zoom;
        for layer in &mut self.layers {
            layer.zoom_at(cursor, layer.zoom() * scale);
        }
    }
}

impl TileLayer {
    pub fn from_tile(
        tile: Arc<Tile>,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        let delta = tile.delta();
        let mut row = VecDeque::new();
        row.push_back(tile.clone());
        let mut tiles = VecDeque::new();
        tiles.push_back(row);
        Self::from_grid(
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
        let tile = Arc::new(Tile::new(position.clone(), tile_width, tile_height, delta));
        let mut row = VecDeque::new();
        row.push_back(tile);
        let mut tiles = VecDeque::new();
        tiles.push_back(row);
        Self::from_grid(tile_width, tile_height, delta, tiles, screen_position, zoom)
    }

    fn from_grid(
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
            tile_width,
            tile_height,
            delta,
            tiles,
            screen_position,
            zoom,
            work_queue: Arc::new(TileWorkQueue::new()),
        }
    }

    pub fn position(&self) -> &crate::geometry::ComplexPoint<f64> {
        self.corner_tile()
            .expect("layer must contain a tile")
            .coordinate()
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
    pub fn corner_tile(&self) -> Option<&Arc<Tile>> {
        self.tile(0, 0)
    }
    pub fn center_tile(&self) -> Option<&Arc<Tile>> {
        self.tile(self.row_count() / 2, self.column_count() / 2)
    }
    pub fn screen_position(&self) -> crate::geometry::ScreenPoint {
        self.screen_position
    }
    pub fn zoom(&self) -> f64 {
        self.zoom
    }
    pub fn pending_work_positions(&self) -> Vec<(usize, usize)> {
        self.work_queue.pending_positions()
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
        let complex_width = self.tile_width as f64 * self.delta;
        let complex_height = self.tile_height as f64 * self.delta;

        while self.screen_position.x > left {
            let origin = self.position().clone();
            self.screen_position.x -= tile_width as i32;
            for row_index in 0..self.row_count() {
                let tile = self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x - complex_width,
                    origin.y - row_index as f64 * complex_height,
                ));
                self.tiles[row_index].push_front(tile);
            }
        }
        while self.screen_position.y > top {
            let origin = self.position().clone();
            self.screen_position.y -= tile_height as i32;
            let mut row = VecDeque::new();
            for column_index in 0..self.column_count() {
                row.push_back(self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x + column_index as f64 * complex_width,
                    origin.y + complex_height,
                )));
            }
            self.tiles.push_front(row);
        }
        while self.screen_position.x + self.screen_size().0 as i32 - 1 < right {
            let column_index = self.column_count();
            let origin = self.position().clone();
            for row_index in 0..self.row_count() {
                let tile = self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x + column_index as f64 * complex_width,
                    origin.y - row_index as f64 * complex_height,
                ));
                self.tiles[row_index].push_back(tile);
            }
        }
        while self.screen_position.y + self.screen_size().1 as i32 - 1 < bottom {
            let row_index = self.row_count();
            let origin = self.position().clone();
            let mut row = VecDeque::new();
            for column_index in 0..self.column_count() {
                row.push_back(self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x + column_index as f64 * complex_width,
                    origin.y - row_index as f64 * complex_height,
                )));
            }
            self.tiles.push_back(row);
        }
    }

    pub fn trim_outside_allocation(&mut self, bounds: (i32, i32, i32, i32)) {
        let (left, top, right, bottom) = bounds;
        let (tile_width, tile_height) = self.tile_screen_size();

        let queue = Arc::clone(&self.work_queue);
        while self.column_count() > 1 && self.screen_position.x + tile_width as i32 - 1 < left {
            for row in &mut self.tiles {
                if let Some(tile) = row.front() {
                    queue.remove_tile(tile);
                }
                row.pop_front();
            }
            self.screen_position.x += tile_width as i32;
        }
        while self.column_count() > 1
            && self.screen_position.x + (self.column_count() as i32 - 1) * tile_width as i32 > right
        {
            for row in &mut self.tiles {
                if let Some(tile) = row.back() {
                    queue.remove_tile(tile);
                }
                row.pop_back();
            }
        }
        while self.row_count() > 1 && self.screen_position.y + tile_height as i32 - 1 < top {
            if let Some(row) = self.tiles.front() {
                for tile in row {
                    queue.remove_tile(tile);
                }
            }
            self.tiles.pop_front();
            self.screen_position.y += tile_height as i32;
        }
        while self.row_count() > 1
            && self.screen_position.y + (self.row_count() as i32 - 1) * tile_height as i32 > bottom
        {
            if let Some(row) = self.tiles.back() {
                for tile in row {
                    queue.remove_tile(tile);
                }
            }
            self.tiles.pop_back();
        }
    }

    fn make_tile_at(&self, coordinate: crate::geometry::ComplexPoint<f64>) -> Arc<Tile> {
        Arc::new(Tile::new(
            coordinate,
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
    layer_queues: Arc<Mutex<VecDeque<RegisteredQueue>>>,
    available: Arc<Condvar>,
    stop_worker: Arc<AtomicBool>,
    worker_statuses: Arc<Mutex<Vec<WorkerStatus>>>,
    workers: Vec<JoinHandle<()>>,
}

struct RegisteredQueue {
    queue: Arc<TileWorkQueue>,
    zoom: f64,
}

pub const DEFAULT_WORKER_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerTile {
    pub coordinate: crate::geometry::ComplexPoint<f64>,
    pub delta: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerStatus {
    pub id: usize,
    pub tile: Option<WorkerTile>,
}

impl Orchestrator {
    pub fn new(calculator: crate::Mandelbrot) -> Self {
        Self::with_worker_count(calculator, DEFAULT_WORKER_COUNT)
    }

    pub fn with_worker_count(calculator: crate::Mandelbrot, worker_count: usize) -> Self {
        assert!(worker_count > 0, "worker count must be positive");
        let layer_queues = Arc::new(Mutex::new(VecDeque::new()));
        let calculator = Arc::new(calculator);
        let available = Arc::new(Condvar::new());
        let stop_worker = Arc::new(AtomicBool::new(false));
        let worker_statuses = Arc::new(Mutex::new(
            (0..worker_count)
                .map(|id| WorkerStatus { id, tile: None })
                .collect::<Vec<_>>(),
        ));
        let mut workers = Vec::with_capacity(worker_count);
        for worker_id in 0..worker_count {
            let worker_queues = Arc::clone(&layer_queues);
            let worker_available = Arc::clone(&available);
            let worker_stop = Arc::clone(&stop_worker);
            let worker_states = Arc::clone(&worker_statuses);
            let worker_calculator = Arc::clone(&calculator);
            workers.push(thread::spawn(move || {
                while let Some(queued) = next_tile(&worker_queues, &worker_available, &worker_stop)
                {
                    worker_states.lock().expect("worker status mutex poisoned")[worker_id].tile =
                        Some(WorkerTile {
                            coordinate: queued.tile.coordinate().clone(),
                            delta: queued.tile.delta(),
                        });
                    calculate_tile(&worker_calculator, &queued.tile);
                    worker_states.lock().expect("worker status mutex poisoned")[worker_id].tile =
                        None;
                }
            }));
        }

        Self {
            layer_queues,
            available,
            stop_worker,
            worker_statuses,
            workers,
        }
    }

    pub fn render_tile(&self, tile: &Arc<Tile>) {
        let queue = Arc::new(TileWorkQueue::new());
        self.register_queue(Arc::clone(&queue), 0.0);
        queue.enqueue(Arc::clone(tile), 0, 0);
        self.available.notify_one();
    }

    pub fn render_layer(&self, layer: &TileLayer) {
        self.register_queue(Arc::clone(&layer.work_queue), layer.zoom());
        for (row_index, row) in layer.tiles.iter().enumerate() {
            for (column_index, tile) in row.iter().enumerate() {
                layer
                    .work_queue
                    .enqueue(Arc::clone(tile), row_index, column_index);
            }
        }
        self.available.notify_one();
    }

    pub fn worker_statuses(&self) -> Vec<WorkerStatus> {
        self.worker_statuses
            .lock()
            .expect("worker status mutex poisoned")
            .clone()
    }

    fn register_queue(&self, queue: Arc<TileWorkQueue>, zoom: f64) {
        let mut queues = self
            .layer_queues
            .lock()
            .expect("layer queue mutex poisoned");
        if let Some(registered) = queues
            .iter_mut()
            .find(|registered| Arc::ptr_eq(&registered.queue, &queue))
        {
            registered.zoom = zoom;
        } else {
            queues.push_back(RegisteredQueue { queue, zoom });
        }
        queues.make_contiguous().sort_by(|left, right| {
            right
                .zoom
                .partial_cmp(&left.zoom)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.available.notify_one();
    }
}

impl Drop for Orchestrator {
    fn drop(&mut self) {
        self.stop_worker.store(true, Ordering::Release);
        self.available.notify_all();
        for worker in self.workers.drain(..) {
            worker.join().expect("tile worker panicked");
        }
    }
}

struct QueuedTile {
    tile: Arc<Tile>,
    row: usize,
    column: usize,
}

struct TileWorkQueue {
    pending: Mutex<VecDeque<QueuedTile>>,
    active: AtomicBool,
}

impl TileWorkQueue {
    fn new() -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            active: AtomicBool::new(true),
        }
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
        self.pending
            .lock()
            .expect("tile queue mutex poisoned")
            .clear();
    }

    fn enqueue(&self, tile: Arc<Tile>, row: usize, column: usize) {
        if !self.is_active() {
            return;
        }
        if tile
            .status
            .compare_exchange(
                TileStatus::NotStarted as u8,
                TileStatus::Deferred as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.pending
                .lock()
                .expect("tile queue mutex poisoned")
                .push_back(QueuedTile { tile, row, column });
        }
    }

    fn pop_front(&self) -> Option<QueuedTile> {
        self.pending
            .lock()
            .expect("tile queue mutex poisoned")
            .pop_front()
    }

    fn remove_tile(&self, target: &Arc<Tile>) -> bool {
        let mut pending = self.pending.lock().expect("tile queue mutex poisoned");
        let original_len = pending.len();
        pending.retain(|queued| !Arc::ptr_eq(&queued.tile, target));
        pending.len() != original_len
    }

    fn pending_positions(&self) -> Vec<(usize, usize)> {
        self.pending
            .lock()
            .expect("tile queue mutex poisoned")
            .iter()
            .map(|queued| (queued.row, queued.column))
            .collect()
    }
}

fn next_tile(
    queues: &Mutex<VecDeque<RegisteredQueue>>,
    available: &Condvar,
    stop: &AtomicBool,
) -> Option<QueuedTile> {
    let mut queues_guard = queues.lock().expect("layer queue mutex poisoned");
    loop {
        for registered in queues_guard.iter() {
            if !registered.queue.is_active() {
                continue;
            }
            if let Some(tile) = registered.queue.pop_front() {
                if registered.queue.is_active() {
                    return Some(tile);
                }
            }
        }
        if stop.load(Ordering::Acquire) {
            return None;
        }
        queues_guard = available
            .wait(queues_guard)
            .expect("layer queue mutex poisoned");
    }
}

fn calculate_tile(calculator: &crate::Mandelbrot, tile: &Tile) {
    let mut iterations = tile
        .iterations
        .lock()
        .expect("tile iterations mutex poisoned");
    for y in 0..tile.height {
        for x in 0..tile.width {
            let real = tile.coordinate.x + x as f64 * tile.delta;
            let imaginary = tile.coordinate.y - y as f64 * tile.delta;
            iterations[y as usize * tile.width as usize + x as usize] =
                calculator.escape_iterations(real, imaginary) as u64;
        }
    }
    tile.status
        .store(TileStatus::Completed as u8, Ordering::Release);
}
