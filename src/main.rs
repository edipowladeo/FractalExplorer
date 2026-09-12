use fractal_explorer::{
    config::AppConfig, geometry::ComplexPoint, Mandelbrot, Orchestrator, RenderConfig,
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
    let image = Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations)).render(
        RenderConfig::centered_at(config.renderer.width, config.renderer.height, 4.0, center),
    );
    fractal_explorer::renderer::run(image, &config.renderer)?;
    Ok(())
}
