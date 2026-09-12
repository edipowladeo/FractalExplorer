use fractal_explorer::{config::AppConfig, Mandelbrot, Orchestrator, RenderConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load("config.toml").unwrap_or_default();
    println!("Paleta em uso: {:?}", config.renderer.palette);
    let image = Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations)).render(
        RenderConfig::centered(config.renderer.width, config.renderer.height, 4.0),
    );
    fractal_explorer::renderer::run(image, &config.renderer)?;
    Ok(())
}
