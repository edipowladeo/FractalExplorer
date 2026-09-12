use fractal_explorer::{
    config::AppConfig,
    geometry::{ComplexPoint, ScreenPoint},
    Fixed, Mandelbrot, MandelbrotFixed, Orchestrator,
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
    let initial_tile = layer.tile(0, 0).expect("initial layer tile");
    match config.renderer.rendering_method.as_str() {
        "multiprecision" => render_fixed_tile(initial_tile, &config, false),
        "perturbation" => render_fixed_tile(initial_tile, &config, true),
        _ => orchestrator.render_layer(&layer),
    }
    fractal_explorer::renderer::run(&mut layer, &orchestrator, &config.renderer)?;
    Ok(())
}

fn render_fixed_tile(tile: &fractal_explorer::Tile, config: &AppConfig, perturbation: bool) {
    let fractal = MandelbrotFixed::<2>::new(config.renderer.max_iterations);
    let coordinate = tile.coordinate();
    let center_real = Fixed::from_f64(coordinate.x);
    let center_imaginary = Fixed::from_f64(coordinate.y);
    let delta = Fixed::from_f64(tile.delta());
    let width = tile.width() as usize;
    let height = tile.height() as usize;
    let iterations = if perturbation {
        let center_x = Fixed::from_i64((width.saturating_sub(1) / 2) as i64);
        let center_y = Fixed::from_i64((height.saturating_sub(1) / 2) as i64);
        let min_real = center_real.sub(delta.mul(center_x));
        let max_real = center_real
            .add(delta.mul(Fixed::from_i64(width.saturating_sub(1) as i64).sub(center_x)));
        let min_imaginary = center_imaginary
            .sub(delta.mul(Fixed::from_i64(height.saturating_sub(1) as i64).sub(center_y)));
        let max_imaginary = center_imaginary.add(delta.mul(center_y));
        let reference =
            fractal.select_reference_grid(min_real, max_real, min_imaginary, max_imaginary, 5);
        fractal
            .render_tile_perturbation(
                center_real,
                center_imaginary,
                delta,
                width,
                height,
                &reference,
                config.renderer.perturbation_fallback,
            )
            .into_iter()
            .map(|iteration| iteration.unwrap_or(0))
            .collect()
    } else {
        fractal.render_tile(center_real, center_imaginary, delta, width, height)
    };
    tile.replace_iterations(iterations.into_iter().map(u64::from).collect());
}
