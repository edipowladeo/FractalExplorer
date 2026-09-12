use fractal_explorer::{config::AppConfig, geometry::ComplexPoint, Mandelbrot, Orchestrator, Tile};

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
    let tile = Tile::new(
        center,
        config.orchestrator.tile.width,
        config.orchestrator.tile.height,
        4.0 / config.renderer.width as f64,
    );
    Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations)).render_tile(&tile);
    fractal_explorer::renderer::run(&tile, &config.renderer)?;
    Ok(())
}
