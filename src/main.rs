use std::collections::VecDeque;
use std::sync::Arc;

use fractal_explorer::{
    config::AppConfig,
    geometry::{ComplexPoint, ScreenPoint},
    Mandelbrot, Orchestrator, Tile,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load("config.toml").unwrap_or_default();
    println!("Paleta em uso: {:?}", config.renderer.palette);
    let center = config
        .renderer
        .starting_point_coordinates()
        .unwrap_or_else(|error| {
            eprintln!("Aviso: {error}; usando centro inicial (0, 0)");
            ComplexPoint::new(0.0, 0.0)
        });
    let tile = Arc::new(Tile::new(
        center.clone(),
        config.orchestrator.tile.width,
        config.orchestrator.tile.height,
        4.0 / config.renderer.width as f64,
    ));
    let mut row = VecDeque::new();
    row.push_back(Arc::clone(&tile));
    let mut tiles = VecDeque::new();
    tiles.push_back(row);
    let tile_size = (tile.width(), tile.height());
    let position = ScreenPoint::new(
        (config.renderer.width.saturating_sub(tile_size.0 as usize) / 2) as i32,
        (config.renderer.height.saturating_sub(tile_size.1 as usize) / 2) as i32,
    );
    let mut layer = fractal_explorer::TileLayer::new(
        center,
        4.0 / config.renderer.width as f64,
        tiles,
        position,
        1.0,
    );
    Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations)).render_layer(&layer);
    fractal_explorer::renderer::run(&mut layer, &config.renderer)?;
    Ok(())
}
