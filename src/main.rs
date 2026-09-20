use fractal_explorer::{
    config::AppConfig,
    geometry::{ComplexPoint, ScreenPoint},
    Mandelbrot, Orchestrator, PrecisionDecisionManager,
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

fn initial_view_parameters(
    viewport_width: usize,
    starting_zoom: f64,
    initial_layer_zoom: f64,
) -> (f64, f64) {
    let base_delta = 4.0 / viewport_width as f64;
    (
        base_delta * initial_layer_zoom / starting_zoom,
        initial_layer_zoom,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::sync::Arc::new(fractal_explorer::output::OutputService::start());
    let _output_scope = output.attach_to_current_thread();
    let config = AppConfig::load("config.toml").unwrap_or_default();
    let render_plan = PrecisionDecisionManager::from_config(&config.renderer)?;
    fractal_explorer::print_local!("Paleta em uso: {:?}", config.renderer.palette);
    let (center, starting_zoom) = config.renderer.starting_view().unwrap_or_else(|error| {
        fractal_explorer::print_local!("Aviso: {error}; usando visão inicial padrão");
        (
            ComplexPoint::new(0.0, 0.0),
            config.renderer.max_apparent_pixel_size(),
        )
    });
    let tile_size = (
        config.orchestrator.tile.width,
        config.orchestrator.tile.height,
    );
    let initial_layer_zoom = config.renderer.max_apparent_pixel_size();
    let (delta, initial_layer_zoom) =
        initial_view_parameters(config.renderer.width, starting_zoom, initial_layer_zoom);
    let position = centered_tile_position(
        (config.renderer.width, config.renderer.height),
        tile_size,
        initial_layer_zoom,
    );
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
        config.renderer.max_apparent_pixel_size(),
        config.renderer.min_apparent_pixel_size,
    );
    fractal_explorer::output::flush_frame();
    canvas.set_initial_zoom(initial_layer_zoom);
    canvas.set_frame_dump_events(config.renderer.debug.frame_dump_events.clone());
    canvas.set_slow_frame_threshold_ms(config.renderer.debug.slow_frame_threshold_ms);
    canvas.set_render_plan(render_plan);
    let orchestrator = Orchestrator::with_worker_count_and_plan(
        Mandelbrot::new(config.renderer.effective_max_iterations()),
        config.orchestrator.workers,
        render_plan,
    );
    #[cfg(feature = "native-ui")]
    {
        let (renderer_updates, renderer_commands) = std::sync::mpsc::channel();
        let renderer_config = config.renderer.clone();
        let renderer_output = std::sync::Arc::clone(&output);
        let renderer_thread = std::thread::spawn(move || {
            fractal_explorer::renderer::run_with_updates(
                &mut canvas,
                &orchestrator,
                &renderer_config,
                renderer_commands,
                &renderer_output,
            )
        });
        let mut config = config;
        fractal_explorer::config_ui::run_window(&mut config, renderer_updates)?;
        renderer_thread
            .join()
            .map_err(|_| "renderer thread panicked")??;
    }

    #[cfg(not(feature = "native-ui"))]
    fractal_explorer::renderer::run(&mut canvas, &orchestrator, &config.renderer, &output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{centered_tile_position, initial_view_parameters};

    #[test]
    fn centers_the_initial_tile_using_its_zoomed_screen_size() {
        assert_eq!(
            centered_tile_position((1400, 800), (64, 64), 3.0),
            fractal_explorer::geometry::ScreenPoint::new(604, 304)
        );
    }

    #[test]
    fn deep_starting_zoom_keeps_seed_layer_screen_size_bounded() {
        let (delta, layer_zoom) = initial_view_parameters(1400, 2.0_f64.powi(48), 8.0);

        assert_eq!(layer_zoom, 8.0);
        assert!(delta.is_finite() && delta > 0.0);
        assert_eq!(
            centered_tile_position((1400, 800), (64, 64), layer_zoom),
            fractal_explorer::geometry::ScreenPoint::new(444, 144)
        );
    }
}
