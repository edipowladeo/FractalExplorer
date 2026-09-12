use fractal_explorer::{
    config::AppConfig,
    geometry::{ComplexPoint, ScreenPoint},
    Mandelbrot, Orchestrator,
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
    let tile_size = (
        config.orchestrator.tile.width,
        config.orchestrator.tile.height,
    );
    let position = ScreenPoint::new(
        (config.renderer.width.saturating_sub(tile_size.0 as usize) / 2) as i32,
        (config.renderer.height.saturating_sub(tile_size.1 as usize) / 2) as i32,
    );
    let delta = 4.0 / config.renderer.width as f64;
    let layer_position = ComplexPoint::new(
        center.x - (tile_size.0 - 1) as f64 * delta / 2.0,
        center.y + (tile_size.1 - 1) as f64 * delta / 2.0,
    );
    let mut layer = fractal_explorer::TileLayer::new(
        layer_position,
        tile_size.0,
        tile_size.1,
        delta,
        position,
        config.renderer.max_apparent_pixel_size(),
    );
    let orchestrator = Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations));
    orchestrator.render_layer(&layer);
    fractal_explorer::renderer::run(&mut layer, &orchestrator, &config.renderer)?;
    Ok(())
}
