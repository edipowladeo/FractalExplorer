use fractal_explorer::{
    config::AppConfig, geometry::ComplexPoint, Fixed, Mandelbrot, MandelbrotFixed, Orchestrator,
    Tile,
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
    let tile = Tile::new(
        center.clone(),
        config.orchestrator.tile.width,
        config.orchestrator.tile.height,
        4.0 / config.renderer.width as f64,
    );
    match config.renderer.rendering_method.as_str() {
        "multiprecision" => {
            let fractal = MandelbrotFixed::<2>::new(config.renderer.max_iterations);
            let iterations = fractal.render_tile(
                Fixed::from_f64(center.x),
                Fixed::from_f64(center.y),
                Fixed::from_f64(4.0 / config.renderer.width as f64),
                config.orchestrator.tile.width as usize,
                config.orchestrator.tile.height as usize,
            );
            tile.replace_iterations(iterations.into_iter().map(u64::from).collect());
        }
        "perturbation" => {
            let fractal = MandelbrotFixed::<2>::new(config.renderer.max_iterations);
            let center_real = Fixed::from_f64(center.x);
            let center_imaginary = Fixed::from_f64(center.y);
            let delta = Fixed::from_f64(4.0 / config.renderer.width as f64);
            let reference = fractal.reference_orbit(center_real, center_imaginary);
            let iterations = fractal.render_tile_perturbation(
                center_real,
                center_imaginary,
                delta,
                config.orchestrator.tile.width as usize,
                config.orchestrator.tile.height as usize,
                &reference,
                config.renderer.perturbation_fallback,
            );
            tile.replace_iterations(
                iterations
                    .into_iter()
                    .map(|iteration| iteration.unwrap_or(0) as u64)
                    .collect(),
            );
        }
        _ => Orchestrator::new(Mandelbrot::new(config.renderer.max_iterations)).render_tile(&tile),
    }
    fractal_explorer::renderer::run(&tile, &config.renderer)?;
    Ok(())
}
