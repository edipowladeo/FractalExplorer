use std::sync::Arc;

use fractal_explorer::{
    config::AppConfig,
    geometry::{ComplexPoint, ScreenPoint},
    Mandelbrot, Orchestrator, Tile, TileSprite,
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
        center,
        config.orchestrator.tile.width,
        config.orchestrator.tile.height,
        4.0 / config.renderer.width as f64,
    ));
    Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations)).render_tile(&tile);
    let tile_size = (tile.width(), tile.height());
    let position = ScreenPoint::new(
        (config.renderer.width.saturating_sub(tile_size.0 as usize) / 2) as i32,
        (config.renderer.height.saturating_sub(tile_size.1 as usize) / 2) as i32,
    );
    let mut tile_sprite = TileSprite::new(tile, position, 1.0);
    fractal_explorer::renderer::run(&mut tile_sprite, &config.renderer)?;
    Ok(())
}
