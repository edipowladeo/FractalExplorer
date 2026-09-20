use fractal_explorer::{
    config::AppConfig,
    geometry::{ComplexPoint, ScreenPoint},
    Mandelbrot, Orchestrator,
};

fn centered_tile_position(
    viewport: (usize, usize),
    tile_size: (u32, u32),
    zoom: f64,
) -> ScreenPoint {
    ScreenPoint::new(
        ((viewport.0 as f64 - tile_size.0 as f64 * zoom) / 2.0).round() as i32,
        ((viewport.1 as f64 - tile_size.1 as f64 * zoom) / 2.0).round() as i32,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load("config.toml").unwrap_or_default();
    println!("Paleta em uso: {:?}", config.renderer.palette);
    let (center, starting_zoom) = config.renderer.starting_view().unwrap_or_else(|error| {
        eprintln!("Aviso: {error}; usando visão inicial padrão");
        (
            ComplexPoint::new(0.0, 0.0),
            config.renderer.max_apparent_pixel_size(),
        )
    });
    let tile_size = (
        config.orchestrator.tile.width,
        config.orchestrator.tile.height,
    );
    let position = centered_tile_position(
        (config.renderer.width, config.renderer.height),
        tile_size,
        starting_zoom,
    );
    let delta = 4.0 / config.renderer.width as f64;
    let layer_position = ComplexPoint::new(
        center.x - (tile_size.0 - 1) as f64 * delta / 2.0,
        center.y + (tile_size.1 - 1) as f64 * delta / 2.0,
    );
    let mut canvas = fractal_explorer::TiledInfiniteCanvas::new(
        layer_position,
        tile_size.0,
        tile_size.1,
        delta,
        position,
        config.renderer.max_apparent_pixel_size().max(starting_zoom),
        config.renderer.min_apparent_pixel_size.min(starting_zoom),
    );
    canvas.set_initial_zoom(starting_zoom);
    let orchestrator = Orchestrator::with_worker_count(
        Mandelbrot::new(config.renderer.effective_max_iterations()),
        config.orchestrator.workers,
    );
    #[cfg(feature = "native-ui")]
    {
        let (renderer_updates, renderer_commands) = std::sync::mpsc::channel();
        let renderer_config = config.renderer.clone();
        let renderer_thread = std::thread::spawn(move || {
            fractal_explorer::renderer::run_with_updates(
                &mut canvas,
                &orchestrator,
                &renderer_config,
                renderer_commands,
            )
        });
        let mut config = config;
        fractal_explorer::config_ui::run_window(&mut config, renderer_updates)?;
        renderer_thread
            .join()
            .map_err(|_| "renderer thread panicked")??;
    }

    #[cfg(not(feature = "native-ui"))]
    fractal_explorer::renderer::run(&mut canvas, &orchestrator, &config.renderer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::centered_tile_position;

    #[test]
    fn centers_the_initial_tile_using_its_zoomed_screen_size() {
        assert_eq!(
            centered_tile_position((1400, 800), (64, 64), 3.0),
            fractal_explorer::geometry::ScreenPoint::new(604, 304)
        );
    }
}
