use fractal_explorer::{config::AppConfig, Mandelbrot, Orchestrator, RenderConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load("config.toml").unwrap_or_default();
    let (width, height) = config.renderer.effective_viewport();
    let image = Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations))
        .render(RenderConfig::centered(width, height, 4.0));
    fractal_explorer::renderer::run(image)?;
    Ok(())
}
