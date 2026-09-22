#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Orchestrator, Tile, TileLayer, TileSprite, TileStatus, TiledInfiniteCanvas};
    use crate::Mandelbrot;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    fn wait_for_completion(tile: &Tile) {
        for _ in 0..1000 {
            if tile.status() == TileStatus::Completed {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("tile worker did not complete the tile");
    }

    #[test]
    fn tile_starts_not_started_and_owns_u64_iteration_storage() {
        let tile = Tile::new(crate::geometry::ComplexPoint::new(1.0, -2.0), 3, 2, 0.25);

        assert_eq!(
            tile.coordinate(),
            &crate::geometry::ComplexPoint::new(1.0, -2.0)
        );
        assert_eq!(tile.status(), TileStatus::NotStarted);
        assert_eq!(tile.width(), 3);
        assert_eq!(tile.height(), 2);
        assert_eq!(tile.delta(), 0.25);
        assert!(tile.iterations().lock().unwrap().is_empty());
    }

    #[test]
    fn destroyed_tile_is_removed_from_pending_work_queue() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            1,
            1,
            1.0,
        ));
        let queue = super::TileWorkQueue::new();

        queue.enqueue(Arc::clone(&tile), 2, 4);

        assert!(queue.remove_tile(&tile));
        assert!(queue.pop_front().is_none());
    }

    #[test]
    fn destroying_a_layer_deactivates_its_work_queue() {
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );
        let queue = Arc::clone(&layer.work_queue);

        assert!(queue.is_active());
        drop(layer);
        assert!(!queue.is_active());
    }

    #[test]
    fn tile_layer_enqueues_its_initial_tile_when_created() {
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );

        assert_eq!(layer.pending_work_positions(), [(0, 0)]);
    }

    #[test]
    fn layer_position_is_redirected_to_the_top_left_tile() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(-2.0, 3.0),
            2,
            2,
            0.5,
        ));
        let layer =
            TileLayer::from_tile(tile.clone(), crate::geometry::ScreenPoint::new(0, 0), 1.0);

        assert!(Arc::ptr_eq(layer.corner_tile().unwrap(), &tile));
        assert_eq!(layer.position(), tile.coordinate());
    }

    #[test]
    fn layer_center_tile_uses_floor_of_half_dimensions() {
        let mut layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );
        layer.ensure_screen_coverage((0, 0, 3, 3));

        assert_eq!(
            layer.center_tile().unwrap().coordinate(),
            &crate::geometry::ComplexPoint::new(2.0, -2.0)
        );
    }

    #[test]
    fn orchestrator_enqueues_and_worker_completes_a_tile() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            3,
            3,
            1.0,
        ));
        Orchestrator::new(Mandelbrot::new(32)).render_tile(&tile);

        wait_for_completion(&tile);
        assert_eq!(tile.status(), TileStatus::Completed);
        assert_eq!(tile.iterations().lock().unwrap().len(), 9);
        assert_eq!(tile.iterations().lock().unwrap()[4], 32u64);
    }

    #[test]
    fn worker_routes_fixed_precision_plan_to_fixed_calculator() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            3,
            3,
            1.0,
        ));
        let plan = crate::PrecisionRenderPlan::direct(crate::PrecisionSpec::fixed(2));
        let orchestrator = Orchestrator::with_worker_count_and_plan(Mandelbrot::new(32), 1, plan);

        orchestrator.render_tile(&tile);
        wait_for_completion(&tile);

        assert_eq!(tile.status(), TileStatus::Completed);
        assert_eq!(tile.iterations().lock().unwrap()[4], 32u64);
    }

    #[test]
    fn workers_use_a_precision_plan_updated_after_startup() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            3,
            3,
            1.0,
        ));
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));
        orchestrator.set_render_plan(crate::PrecisionRenderPlan::direct(
            crate::PrecisionSpec::fixed(2),
        ));

        orchestrator.render_tile(&tile);
        wait_for_completion(&tile);

        assert_eq!(tile.status(), TileStatus::Completed);
        assert_eq!(tile.iterations().lock().unwrap()[4], 32u64);
    }

    #[test]
    fn orchestrator_exposes_the_worker_pool() {
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));

        assert_eq!(orchestrator.worker_statuses().len(), 8);
        assert!(orchestrator
            .worker_statuses()
            .iter()
            .all(|worker| worker.tile.is_none()));
    }

    #[test]
    fn new_tile_is_deferred_before_worker_completes_it() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            3,
            3,
            1.0,
        ));
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));

        orchestrator.render_tile(&tile);

        assert!(matches!(
            tile.status(),
            TileStatus::Deferred | TileStatus::Completed
        ));
        wait_for_completion(&tile);
    }

    #[test]
    fn completed_tile_is_not_recalculated() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            1,
            1,
            1.0,
        ));
        let orchestrator = Orchestrator::new(Mandelbrot::new(32));
        orchestrator.render_tile(&tile);
        wait_for_completion(&tile);
        tile.iterations().lock().unwrap()[0] = 777;

        orchestrator.render_tile(&tile);

        assert_eq!(tile.iterations().lock().unwrap()[0], 777);
        assert_eq!(tile.status(), TileStatus::Completed);
    }

    #[test]
    fn tile_keeps_one_immutable_sprite_reference() {
        let tile = Tile::new(crate::geometry::ComplexPoint::new(0.0, 0.0), 1, 1, 1.0);
        let sprite = Arc::new(crate::Sprite::solid(1, 1, 0xff00ff));
        tile.set_sprite(Arc::clone(&sprite));

        let stored = tile.sprite().expect("tile sprite should exist");

        assert!(Arc::ptr_eq(&stored, &sprite));
    }

    #[test]
    fn tile_sprite_keeps_tile_reference_and_top_left_screen_position() {
        let tile = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            200,
            150,
            1.0,
        ));
        let sprite = TileSprite::new(
            std::sync::Arc::clone(&tile),
            crate::geometry::ScreenPoint::new(10, 20),
            1.5,
        );

        assert!(std::sync::Arc::ptr_eq(sprite.tile(), &tile));
        assert_eq!(sprite.position(), crate::geometry::ScreenPoint::new(10, 20));
        assert_eq!(sprite.zoom(), 1.5);
        assert_eq!(sprite.screen_size(), (300, 225));
    }

    #[test]
    fn tile_sprite_zoom_keeps_cursor_position_invariant() {
        let tile = std::sync::Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            100,
            50,
            1.0,
        ));
        let mut sprite = TileSprite::new(tile, crate::geometry::ScreenPoint::new(100, 80), 1.0);

        sprite.zoom_at(crate::geometry::ScreenPoint::new(150, 100), 2.0);

        assert_eq!(sprite.position(), crate::geometry::ScreenPoint::new(50, 60));
        assert_eq!(sprite.zoom(), 2.0);
    }

    #[test]
    fn tile_layer_expands_grid_until_screen_bounds_are_covered() {
        let mut layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(4, 4),
            1.0,
        );

        layer.ensure_screen_coverage((0, 0, 7, 7));

        assert_eq!((layer.row_count(), layer.column_count()), (4, 4));
        assert_eq!(
            layer.screen_position(),
            crate::geometry::ScreenPoint::new(0, 0)
        );
        assert_eq!((layer.tile_width(), layer.tile_height()), (2, 2));
        assert_eq!(layer.tile(3, 3).unwrap().width(), 2);
        assert_eq!(layer.tile(3, 3).unwrap().delta(), 1.0);

        assert_eq!(
            layer.position(),
            &crate::geometry::ComplexPoint::new(-5.0, 5.0)
        );
    }

    #[test]
    fn tile_layer_removes_tiles_that_are_fully_outside_screen_bounds() {
        let mut layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            2,
            2,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
        );
        layer.ensure_screen_coverage((0, 0, 7, 7));
        layer.set_screen_position(crate::geometry::ScreenPoint::new(-8, -8));

        layer.trim_outside_allocation((0, 0, 7, 7));

        assert_eq!((layer.row_count(), layer.column_count()), (1, 1));
        assert_eq!(
            layer.screen_position(),
            crate::geometry::ScreenPoint::new(-2, -2)
        );
    }

    #[test]
    fn zoom_in_does_not_retract_larger_layer_before_next_layer_is_displayable() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            1,
            1,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
            0.5,
        );
        canvas.expand_one_layer_per_frame();
        canvas.expand_one_layer_per_frame();
        canvas.zoom_at(crate::geometry::ScreenPoint::new(0, 0), 2.0);

        assert!(!canvas.retract_one_layer_per_frame());
        assert_eq!(canvas.layer_count(), 2);
    }

    #[test]
    fn zoom_in_retracts_larger_layer_after_next_layer_is_displayable() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            1,
            1,
            1.0,
            crate::geometry::ScreenPoint::new(0, 0),
            1.0,
            0.5,
        );
        canvas.expand_one_layer_per_frame();
        canvas.expand_one_layer_per_frame();
        canvas.zoom_at(crate::geometry::ScreenPoint::new(0, 0), 2.0);

        let next_layer_tile = canvas.layer(1).unwrap().tile(0, 0).unwrap();
        next_layer_tile.status.store(
            TileStatus::Completed as u8,
            std::sync::atomic::Ordering::Release,
        );
        next_layer_tile.set_sprite(Arc::new(crate::Sprite::solid(1, 1, 0)));

        assert!(canvas.retract_one_layer_per_frame());
        assert_eq!(canvas.layer_count(), 1);
        assert_eq!(canvas.layer(0).unwrap().zoom(), 1.0);
    }

    #[test]
    fn tile_layer_composes_its_initial_tile_without_receiving_one() {
        let layer = TileLayer::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            200,
            150,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            1.0,
        );

        assert_eq!((layer.row_count(), layer.column_count()), (1, 1));
        assert_eq!((layer.tile_width(), layer.tile_height()), (200, 150));
        assert_eq!(layer.tile(0, 0).unwrap().width(), 200);
        assert_eq!(layer.tile(0, 0).unwrap().delta(), 0.01);
    }

    #[test]
    fn tile_layer_can_be_composed_from_one_tile() {
        let tile = Arc::new(Tile::new(
            crate::geometry::ComplexPoint::new(0.0, 0.0),
            200,
            150,
            0.01,
        ));
        let layer = TileLayer::from_tile(
            tile.clone(),
            crate::geometry::ScreenPoint::new(300, 225),
            1.0,
        );

        assert!(Arc::ptr_eq(layer.tile(0, 0).unwrap(), &tile));
        assert_eq!(layer.delta(), 0.01);
        assert_eq!(
            layer.position(),
            &crate::geometry::ComplexPoint::new(0.0, 0.0)
        );
    }

    #[test]
    fn tiled_infinite_canvas_expands_by_one_layer_per_frame() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );

        assert_eq!(canvas.layer_count(), 0);
        assert!(canvas.expand_one_layer_per_frame());
        assert_eq!(canvas.layer_count(), 1);
        assert_eq!(canvas.layer(0).unwrap().zoom(), 8.0);
        assert!(canvas.expand_one_layer_per_frame());
        assert_eq!(canvas.layer_count(), 2);
        assert_eq!(canvas.layer(1).unwrap().zoom(), 4.0);
        assert_eq!(canvas.layer(1).unwrap().delta(), 0.005);
        assert_eq!(canvas.layer(1).unwrap().position().x, -0.9275);
        assert_eq!(canvas.layer(1).unwrap().position().y, 0.9525);
        assert_eq!(
            canvas.layer(1).unwrap().screen_position(),
            canvas.complex_to_screen(canvas.layer(1).unwrap().position().clone())
        );

        canvas.zoom_at(crate::geometry::ScreenPoint::new(400, 300), 16.0);
        assert_eq!(canvas.layer(0).unwrap().zoom(), 16.0);
        assert_eq!(canvas.layer(1).unwrap().zoom(), 8.0);
    }

    #[test]
    fn canvas_can_start_at_a_configured_zoom_within_its_limits() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            8.0,
            0.5,
        );

        canvas.set_initial_zoom(2.5);
        canvas.expand_one_layer_per_frame();

        assert_eq!(canvas.layer(0).unwrap().zoom(), 2.5);
        assert_eq!(canvas.apparent_pixel_size(), 2.5);
    }

    #[test]
    fn records_the_delta_exponent_and_direction_for_created_layers() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            crate::geometry::ScreenPoint::new(385, 290),
            8.0,
            0.5,
        );
        canvas.set_frame_dump_events(vec!["layer_created".to_string()]);

        for _ in 0..3 {
            canvas.begin_frame();
            canvas.ensure_screen_coverage((0, 0, 29, 19));
            canvas.finish_frame();
        }
        assert_eq!(canvas.layer_creation_log().len(), 2);
        canvas.begin_frame();

        let log = canvas.layer_creation_log();
        assert_eq!(log.len(), 3);
        assert!(log[0].starts_with("Camada criada, delta: 7.644, "));
        assert!(log[1].starts_with("Camada menor criada, delta: 8.644, "));
        assert!(log[2].starts_with("Camada menor criada, delta: 9.644, "));
        for line in log {
            assert!(line.contains("tiles="));
            assert!(line.contains("enfileirados="));
            assert!(line.contains("tile_ms="));
            assert!(line.contains("grade_ms="));
            assert!(line.contains("fila_ms="));
            assert!(line.contains("cobertura_ms="));
            assert!(line.contains("frame_ms="));
        }
    }

    #[test]
    fn formats_frame_events_with_elapsed_and_delta_times() {
        let started_at = Instant::now();
        let instrumentation = super::FrameInstrumentation {
            frame_number: 7,
            started_at,
            events: Vec::new(),
        };
        let event = super::FrameEvent {
            kind: super::FrameEventKind::LayerWorkScheduled,
            description: "trabalho das camadas agendado".to_string(),
            timestamp: started_at + Duration::from_micros(113_183),
        };
        let previous_timestamp = started_at + Duration::from_micros(113_174);

        assert_eq!(
            instrumentation.format_event(&event, previous_timestamp),
            "Frame: 113.183 Δ: 0.009, trabalho das camadas agendado."
        );
    }

    #[test]
    fn formats_gpu_surface_timing_events_in_frame_order() {
        let started_at = Instant::now();
        let instrumentation = super::FrameInstrumentation {
            frame_number: 8,
            started_at,
            events: vec![
                super::FrameEvent {
                    kind: super::FrameEventKind::GpuSurfaceAcquireStarted,
                    description: "aquisicao da superficie GPU iniciada".to_string(),
                    timestamp: started_at,
                },
                super::FrameEvent {
                    kind: super::FrameEventKind::GpuSurfaceAcquireFinished,
                    description: "aquisicao da superficie GPU concluida".to_string(),
                    timestamp: started_at + Duration::from_millis(405),
                },
            ],
        };

        let line = instrumentation.format_event(&instrumentation.events[1], started_at);
        assert!(line.contains("aquisicao da superficie GPU concluida"));
        assert!(line.contains("Δ: 405.000"));
    }

    #[test]
    fn slow_frame_is_a_dump_trigger_without_layer_creation() {
        let started_at = Instant::now() - Duration::from_millis(1_001);
        let instrumentation = super::FrameInstrumentation {
            frame_number: 42,
            started_at,
            events: vec![super::FrameEvent {
                kind: super::FrameEventKind::Finalized,
                description: "finalizacao de frame".to_string(),
                timestamp: Instant::now(),
            }],
        };

        let threshold = Duration::from_millis(1_000);
        assert!(instrumentation.is_slow(threshold));
        assert_eq!(
            instrumentation.matching_triggers(&["slow_frame".to_string()], threshold),
            vec!["slow_frame"]
        );
    }

    #[test]
    fn finalization_precedes_slow_trigger_and_preserves_total_frame_duration() {
        let started_at = Instant::now();
        let finished_at = started_at + Duration::from_millis(4_671);
        let next_frame_started_at = finished_at + Duration::from_millis(1);
        let mut instrumentation = super::FrameInstrumentation {
            frame_number: 5,
            started_at,
            events: vec![super::FrameEvent {
                kind: super::FrameEventKind::BufferReadyForPresentation,
                description: "buffer pronto para apresentacao".to_string(),
                timestamp: started_at + Duration::from_millis(109),
            }],
        };

        instrumentation.record_finalization_at(finished_at);
        let total_duration = instrumentation.duration_at(next_frame_started_at);
        instrumentation
            .record_slow_if_needed_at(next_frame_started_at, Duration::from_millis(1_000));

        assert_eq!(
            instrumentation.events[1].description,
            "finalizacao de frame"
        );
        assert_eq!(
            instrumentation.events[2].kind,
            super::FrameEventKind::SlowFrame
        );
        assert_eq!(
            total_duration,
            next_frame_started_at.duration_since(instrumentation.started_at)
        );
        assert!(instrumentation.events[1].timestamp > instrumentation.events[0].timestamp);
        assert_eq!(instrumentation.events[2].timestamp, next_frame_started_at);
        assert!(instrumentation.has_kind(super::FrameEventKind::SlowFrame));
    }

    #[test]
    fn frame_duration_uses_the_same_boundary_as_the_next_frame_start() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );
        canvas.set_frame_dump_events(Vec::new());

        let first_frame_started_at = Instant::now();
        let first_frame_finished_at = first_frame_started_at + Duration::from_millis(10);
        let second_frame_started_at = first_frame_started_at + Duration::from_millis(100);

        canvas.begin_frame_at(first_frame_started_at);
        canvas.finish_frame_at(first_frame_finished_at);
        canvas.begin_frame_at(second_frame_started_at);

        assert_eq!(
            canvas.last_finished_frame_timing(),
            Some((1, Duration::from_millis(100)))
        );
        assert_eq!(
            canvas
                .frame_instrumentation
                .as_ref()
                .expect("second frame should be active")
                .started_at,
            second_frame_started_at
        );
    }

    #[test]
    fn records_buffer_presentation_boundaries_as_frame_events() {
        let mut instrumentation = super::FrameInstrumentation::new(6);

        instrumentation.record_presentation_started();
        instrumentation.record_presentation_finished();

        assert_eq!(
            instrumentation.events[1].description,
            "apresentacao do buffer iniciada"
        );
        assert_eq!(
            instrumentation.events[2].description,
            "apresentacao do buffer concluida"
        );
    }

    #[test]
    fn records_dump_boundaries_as_frame_events() {
        let mut instrumentation = super::FrameInstrumentation::new(7);

        instrumentation.record_dump_started();
        instrumentation.record_dump_finished();

        assert_eq!(instrumentation.events[1].description, "dump iniciado");
        assert_eq!(instrumentation.events[2].description, "dump finalizado");
    }

    #[test]
    fn skips_layer_creation_log_when_disabled() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );
        canvas.set_frame_dump_events(Vec::new());

        canvas.begin_frame();
        canvas.ensure_screen_coverage((0, 0, 799, 599));
        canvas.finish_frame();
        canvas.begin_frame();
        assert!(canvas.layer_creation_log().is_empty());
    }

    #[test]
    fn all_layers_map_the_same_complex_point_to_the_same_screen_point() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            crate::geometry::ScreenPoint::new(300, 225),
            8.0,
            0.8,
        );
        canvas.expand_one_layer_per_frame();
        canvas.expand_one_layer_per_frame();
        canvas.expand_one_layer_per_frame();

        let point = crate::geometry::ComplexPoint::new(-0.95, 0.93);
        let expected = canvas.complex_to_screen(point.clone());

        for layer in canvas.layers() {
            assert_eq!(
                layer.complex_to_screen(point.clone()),
                expected,
                "layer transform is not aligned with the camera"
            );
        }
    }

    #[test]
    fn initial_layer_expansion_keeps_the_cursor_complex_coordinate_aligned() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            crate::geometry::ScreenPoint::new(385, 290),
            8.0,
            0.5,
        );
        for _ in 0..5 {
            canvas.ensure_screen_coverage((0, 0, 799, 599));
        }

        let cursor = crate::geometry::ScreenPoint::new(415, 368);
        let global = canvas.screen_to_complex(cursor);
        assert_eq!(canvas.layer_count(), 5);
        assert_eq!(
            global,
            crate::geometry::ComplexPoint::new(-1.55625, 0.00125)
        );
        for (index, layer) in canvas.layers().iter().enumerate() {
            let point = layer.screen_to_complex(cursor);
            assert!(
                (point.x - global.x).abs() < 1e-12 && (point.y - global.y).abs() < 1e-12,
                "layer {index} is not aligned with the camera at the initial cursor: {point:?} != {global:?}"
            );
        }
    }

    #[test]
    fn each_initial_layer_expansion_keeps_the_cursor_complex_coordinate_aligned() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            crate::geometry::ScreenPoint::new(385, 290),
            8.0,
            0.5,
        );
        let cursor = crate::geometry::ScreenPoint::new(415, 368);

        for expansion in 1..=5 {
            canvas.ensure_screen_coverage((0, 0, 799, 599));
            let global = canvas.screen_to_complex(cursor);
            assert_eq!(canvas.layer_count(), expansion);
            for (index, layer) in canvas.layers().iter().enumerate() {
                let point = layer.screen_to_complex(cursor);
                assert!(
                    (point.x - global.x).abs() < 1e-12 && (point.y - global.y).abs() < 1e-12,
                    "expansion {expansion}, layer {index} is not aligned: {point:?} != {global:?}"
                );
            }
        }
    }

    #[test]
    fn layer_expansion_without_tile_coverage_keeps_the_cursor_coordinate_aligned() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            crate::geometry::ScreenPoint::new(385, 290),
            8.0,
            0.5,
        );
        let cursor = crate::geometry::ScreenPoint::new(415, 368);

        for expansion in 1..=5 {
            assert!(canvas.expand_one_layer_per_frame());
            let global = canvas.screen_to_complex(cursor);
            for (index, layer) in canvas.layers().iter().enumerate() {
                let point = layer.screen_to_complex(cursor);
                assert!(
                    (point.x - global.x).abs() < 1e-12
                        && (point.y - global.y).abs() < 1e-12,
                    "expansion {expansion}, layer {index} is misaligned without tile coverage: {point:?} != {global:?}"
                );
            }
        }
    }

    #[test]
    fn smaller_adjacent_layer_is_aligned_before_position_synchronization() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            crate::geometry::ScreenPoint::new(385, 290),
            8.0,
            0.5,
        );
        for _ in 0..3 {
            assert!(canvas.expand_one_layer_per_frame());
        }
        let cursor = crate::geometry::ScreenPoint::new(415, 368);
        let global = canvas.screen_to_complex(cursor);
        let layer = TiledInfiniteCanvas::adjacent_layer(canvas.layer(2).unwrap(), 0.5);
        let point = layer.screen_to_complex(cursor);

        assert!(
            (point.x - global.x).abs() < 1e-12 && (point.y - global.y).abs() < 1e-12,
            "adjacent layer is misaligned before synchronization: {point:?} != {global:?}"
        );
    }

    #[test]
    fn smaller_adjacent_layer_preserves_its_complex_origin_projection_with_subpixel_precision() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            crate::geometry::ScreenPoint::new(385, 290),
            8.0,
            0.5,
        );
        for _ in 0..3 {
            assert!(canvas.expand_one_layer_per_frame());
        }
        let layer = TiledInfiniteCanvas::adjacent_layer(canvas.layer(2).unwrap(), 0.5);

        let (origin_x, origin_y) = layer.screen_origin();
        let position = layer.position();
        let expected_x = canvas.camera_anchor_screen.x as f64
            + (position.x - canvas.camera_anchor_complex.x) * canvas.camera_scale;
        let expected_y = canvas.camera_anchor_screen.y as f64
            - (position.y - canvas.camera_anchor_complex.y) * canvas.camera_scale;
        assert!(
            (origin_x - expected_x).abs() < 1e-12 && (origin_y - expected_y).abs() < 1e-12,
            "the adjacent layer origin must preserve the camera projection"
        );
    }

    #[test]
    fn reported_navigation_keeps_all_layer_coordinates_under_the_cursor_aligned() {
        let mut canvas = TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.5725, 0.0475),
            30,
            20,
            0.005,
            crate::geometry::ScreenPoint::new(385, 290),
            8.0,
            0.8,
        );
        let zooms = [
            ((316, 352), 10.4),
            ((316, 352), 6.760000000000001),
            ((317, 351), 8.788000000000002),
            ((286, 402), 5.712200000000002),
            ((286, 402), 7.425860000000003),
            ((286, 402), 9.653618000000003),
            ((306, 389), 6.274851700000003),
            ((306, 389), 8.157307210000004),
            ((307, 388), 5.302249686500003),
            ((314, 371), 6.892924592450004),
            ((314, 371), 8.960801970185006),
            ((316, 365), 5.824521280620254),
            ((318, 359), 7.571877664806331),
            ((311, 370), 9.843440964248231),
            ((304, 379), 6.398236626761350),
            ((313, 372), 8.317707614789756),
            ((313, 372), 5.406509949613342),
            ((313, 372), 7.028462934497345),
            ((313, 372), 9.137001814846549),
            ((313, 372), 5.939051179650257),
            ((313, 372), 7.720766533545335),
            ((323, 362), 10.036996493608935),
            ((323, 362), 6.524047720845808),
            ((334, 362), 8.481262037099551),
            ((359, 362), 5.512820324114708),
            ((359, 362), 7.166666421349120),
            ((357, 359), 9.316666347753857),
            ((357, 359), 6.055833126040008),
            ((347, 333), 7.872583063852010),
            ((347, 333), 10.234357983007614),
            ((347, 333), 6.652332688954949),
        ];

        for _ in 0..4 {
            canvas.ensure_screen_coverage((0, 0, 799, 599));
        }
        for ((x, y), zoom) in zooms {
            canvas.zoom_at(crate::geometry::ScreenPoint::new(x, y), zoom);
        }

        let cursor = crate::geometry::ScreenPoint::new(353, 318);
        let global = canvas.screen_to_complex(cursor);
        assert_eq!(canvas.layer_count(), 4);
        for (index, layer) in canvas.layers().iter().enumerate() {
            let layer_point = layer.screen_to_complex(cursor);
            assert!(
                (layer_point.x - global.x).abs() < 1e-12
                    && (layer_point.y - global.y).abs() < 1e-12,
                "layer {index} maps the cursor to {layer_point:?}, instead of {global:?}"
            );
        }
    }
}

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc, Condvar, Mutex, RwLock,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TileStatus {
    NotStarted = 0,
    Deferred = 1,
    Completed = 2,
}

impl TileStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Deferred,
            2 => Self::Completed,
            _ => Self::NotStarted,
        }
    }
}

/// A fixed-size region of the complex plane and its calculation result.
pub struct Tile {
    coordinate: crate::geometry::ComplexPoint<f64>,
    width: u32,
    height: u32,
    delta: f64,
    status: AtomicU8,
    iterations: Arc<Mutex<Vec<u64>>>,
    sprite: Arc<Mutex<Option<Arc<crate::Sprite>>>>,
}

/// Places one calculated tile in screen space.
pub struct TileSprite {
    tile: Arc<Tile>,
    position: crate::geometry::ScreenPoint,
    zoom: f64,
}

/// A grid of tiles with one shared complex-plane transform.
pub struct TileLayer {
    tile_width: u32,
    tile_height: u32,
    delta: f64,
    tiles: VecDeque<VecDeque<Arc<Tile>>>,
    screen_position: crate::geometry::ScreenPoint,
    screen_origin_x: f64,
    screen_origin_y: f64,
    zoom: f64,
    work_queue: Arc<TileWorkQueue>,
    render_plan: crate::PrecisionRenderPlan,
    creation_diagnostics: LayerCreationDiagnostics,
}

#[derive(Debug, Clone, Copy, Default)]
struct LayerCreationDiagnostics {
    tile_creation: Duration,
    grid_insertion: Duration,
    queue_enqueuing: Duration,
    coverage: Duration,
    frame_interval: Duration,
    tiles_created: usize,
    tiles_enqueued: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameEventKind {
    NewFrame,
    ConfigurationProcessed,
    SurfacePrepared,
    OutsideAllocationTrimmed,
    ScreenCoverageCompleted,
    LayerWorkScheduled,
    InputProcessed,
    TileCompositionStarted,
    TilesRasterized,
    OverlaysDrawn,
    BufferReadyForPresentation,
    Finalized,
    PresentationStarted,
    PresentationFinished,
    GpuSurfaceAcquireStarted,
    GpuSurfaceAcquireFinished,
    GpuBatchPreparationFinished,
    GpuBatchUploadStarted,
    GpuBatchUploadFinished,
    GpuBatchTextureUpload,
    GpuBatchTextureRetention,
    GpuBatchVertexUpload,
    GpuBatchOverlayUpload,
    GpuBatchEnvelopeUpload,
    GpuRedrawRequested,
    GpuRedrawReceived,
    GpuCommandEncodingFinished,
    GpuCompositionPassStarted,
    GpuCompositionPassFinished,
    GpuSurfacePresentPassStarted,
    GpuSurfacePresentPassFinished,
    GpuCommandsSubmitted,
    GpuDevicePollStarted,
    GpuDevicePollFinished,
    GpuPresentationStarted,
    GpuPresentationFinished,
    DumpStarted,
    DumpFinished,
    SlowFrame,
    LayerCreated,
}

#[derive(Debug)]
struct FrameEvent {
    kind: FrameEventKind,
    description: String,
    timestamp: Instant,
}

const BUFFER_PRESENTATION_STARTED: &str = "apresentacao do buffer iniciada";
const BUFFER_PRESENTATION_FINISHED: &str = "apresentacao do buffer concluida";
const FRAME_DUMP_STARTED: &str = "dump iniciado";
const FRAME_DUMP_FINISHED: &str = "dump finalizado";

#[derive(Debug)]
struct FrameInstrumentation {
    frame_number: u64,
    started_at: Instant,
    events: Vec<FrameEvent>,
}

impl FrameInstrumentation {
    fn new(frame_number: u64) -> Self {
        Self::new_at(frame_number, Instant::now())
    }

    fn new_at(frame_number: u64, started_at: Instant) -> Self {
        Self {
            frame_number,
            started_at,
            events: vec![FrameEvent {
                kind: FrameEventKind::NewFrame,
                description: "novo frame".to_string(),
                timestamp: started_at,
            }],
        }
    }

    fn record(&mut self, kind: FrameEventKind, description: impl Into<String>) {
        self.record_at(kind, description, Instant::now());
    }

    fn record_at(
        &mut self,
        kind: FrameEventKind,
        description: impl Into<String>,
        timestamp: Instant,
    ) {
        self.events.push(FrameEvent {
            kind,
            description: description.into(),
            timestamp,
        });
    }

    fn record_presentation_started(&mut self) {
        self.record(
            FrameEventKind::PresentationStarted,
            BUFFER_PRESENTATION_STARTED,
        );
    }

    fn record_presentation_finished(&mut self) {
        self.record(
            FrameEventKind::PresentationFinished,
            BUFFER_PRESENTATION_FINISHED,
        );
    }

    fn record_dump_started(&mut self) {
        self.record(FrameEventKind::DumpStarted, FRAME_DUMP_STARTED);
    }

    fn record_dump_finished(&mut self) {
        self.record(FrameEventKind::DumpFinished, FRAME_DUMP_FINISHED);
    }

    fn record_trigger(&mut self, kind: FrameEventKind, description: impl Into<String>) {
        self.record_trigger_at(kind, description, Instant::now());
    }

    fn record_trigger_at(
        &mut self,
        kind: FrameEventKind,
        description: impl Into<String>,
        timestamp: Instant,
    ) {
        self.events.push(FrameEvent {
            kind,
            description: description.into(),
            timestamp,
        });
    }

    fn has_kind(&self, kind: FrameEventKind) -> bool {
        self.events.iter().any(|event| event.kind == kind)
    }

    fn matching_triggers(
        &self,
        configured_events: &[String],
        slow_frame_threshold: Duration,
    ) -> Vec<String> {
        configured_events
            .iter()
            .filter(|configured| match configured.as_str() {
                "slow_frame" => {
                    self.has_kind(FrameEventKind::SlowFrame) || self.is_slow(slow_frame_threshold)
                }
                "layer_created" => self.has_kind(FrameEventKind::LayerCreated),
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn is_slow(&self, threshold: Duration) -> bool {
        self.duration() > threshold
    }

    fn is_slow_at(&self, timestamp: Instant, threshold: Duration) -> bool {
        timestamp.duration_since(self.started_at) > threshold
    }

    fn record_finalization_at(&mut self, finished_at: Instant) {
        self.record_at(
            FrameEventKind::Finalized,
            "finalizacao de frame",
            finished_at,
        );
    }

    fn record_slow_if_needed_at(&mut self, timestamp: Instant, threshold: Duration) {
        if self.is_slow_at(timestamp, threshold) && !self.has_kind(FrameEventKind::SlowFrame) {
            self.record_trigger_at(FrameEventKind::SlowFrame, "frame lento", timestamp);
        }
    }

    fn duration(&self) -> Duration {
        self.events.last().map_or(Duration::ZERO, |event| {
            event.timestamp.duration_since(self.started_at)
        })
    }

    fn duration_at(&self, timestamp: Instant) -> Duration {
        timestamp.duration_since(self.started_at)
    }

    fn dump(&self) {
        for event_index in 0..self.events.len() {
            self.dump_event(event_index);
        }
    }

    fn dump_last_event(&self) {
        if !self.events.is_empty() {
            self.dump_event(self.events.len() - 1);
        }
    }

    fn dump_event(&self, event_index: usize) {
        let event = &self.events[event_index];
        let previous_timestamp = event_index
            .checked_sub(1)
            .map_or(self.started_at, |index| self.events[index].timestamp);
        crate::print_local!("{}", self.format_event(event, previous_timestamp));
    }

    fn format_event(&self, event: &FrameEvent, previous_timestamp: Instant) -> String {
        let since_start = event.timestamp.duration_since(self.started_at);
        let since_previous = event.timestamp.duration_since(previous_timestamp);
        format!(
            "Frame: {:.3} Δ: {:.3}, {}.",
            since_start.as_secs_f64() * 1_000.0,
            since_previous.as_secs_f64() * 1_000.0,
            event.description,
        )
    }
}

impl LayerCreationDiagnostics {
    fn format(self) -> String {
        format!(
            "tiles={} enfileirados={} tile_ms={:.3} grade_ms={:.3} fila_ms={:.3} cobertura_ms={:.3} frame_ms={:.3}",
            self.tiles_created,
            self.tiles_enqueued,
            self.tile_creation.as_secs_f64() * 1_000.0,
            self.grid_insertion.as_secs_f64() * 1_000.0,
            self.queue_enqueuing.as_secs_f64() * 1_000.0,
            self.coverage.as_secs_f64() * 1_000.0,
            self.frame_interval.as_secs_f64() * 1_000.0,
        )
    }
}

impl Drop for TileLayer {
    fn drop(&mut self) {
        self.work_queue.deactivate();
    }
}

/// Ordered collection of fractal layers at progressively smaller apparent pixels.
pub struct TiledInfiniteCanvas {
    layers: VecDeque<TileLayer>,
    position: crate::geometry::ComplexPoint<f64>,
    tile_width: u32,
    tile_height: u32,
    delta: f64,
    screen_position: crate::geometry::ScreenPoint,
    max_apparent_pixel_size: f64,
    min_apparent_pixel_size: f64,
    initial_apparent_pixel_size: f64,
    camera_anchor_complex: crate::geometry::ComplexPoint<f64>,
    camera_anchor_screen: crate::geometry::ScreenPoint,
    camera_scale: f64,
    navigation_history: Vec<CanvasNavigationEvent>,
    layer_creation_log: Vec<String>,
    frame_dump_events: Vec<String>,
    slow_frame_threshold: Duration,
    frame_number: u64,
    pending_layer_creation: Option<PendingLayerCreation>,
    completed_layer_creation: Option<CompletedLayerCreation>,
    frame_instrumentation: Option<FrameInstrumentation>,
    last_finished_frame_timing: Option<(u64, Duration)>,
    render_plan: crate::PrecisionRenderPlan,
}

struct PendingLayerCreation {
    direction: Option<&'static str>,
    layer_index: usize,
}

struct CompletedLayerCreation {
    direction: Option<&'static str>,
    delta: f64,
    diagnostics: LayerCreationDiagnostics,
}

/// A navigation command applied to a canvas, retained for diagnostic replay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasNavigationEvent {
    Drag {
        delta: crate::geometry::ScreenPoint,
    },
    Zoom {
        cursor: crate::geometry::ScreenPoint,
        zoom: f64,
    },
}

/// Immutable parameters used to create a canvas, for deterministic replay.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasInitialState {
    pub position: crate::geometry::ComplexPoint<f64>,
    pub tile_width: u32,
    pub tile_height: u32,
    pub delta: f64,
    pub screen_position: crate::geometry::ScreenPoint,
    pub max_apparent_pixel_size: f64,
    pub min_apparent_pixel_size: f64,
}

impl TiledInfiniteCanvas {
    pub fn new(
        position: crate::geometry::ComplexPoint<f64>,
        tile_width: u32,
        tile_height: u32,
        delta: f64,
        screen_position: crate::geometry::ScreenPoint,
        max_apparent_pixel_size: f64,
        min_apparent_pixel_size: f64,
    ) -> Self {
        assert!(max_apparent_pixel_size > 0.0);
        assert!(min_apparent_pixel_size > 0.0);
        let camera_anchor_complex = crate::geometry::ComplexPoint::new(
            position.x + (tile_width - 1) as f64 * delta / 2.0,
            position.y - (tile_height - 1) as f64 * delta / 2.0,
        );
        let camera_anchor_screen = crate::geometry::ScreenPoint::new(
            screen_position.x
                + ((tile_width as f64 * max_apparent_pixel_size - 1.0) / 2.0).round() as i32,
            screen_position.y
                + ((tile_height as f64 * max_apparent_pixel_size - 1.0) / 2.0).round() as i32,
        );
        Self {
            layers: VecDeque::new(),
            position,
            tile_width,
            tile_height,
            delta,
            screen_position,
            max_apparent_pixel_size,
            min_apparent_pixel_size,
            initial_apparent_pixel_size: max_apparent_pixel_size,
            camera_anchor_complex,
            camera_anchor_screen,
            camera_scale: max_apparent_pixel_size / delta,
            navigation_history: Vec::new(),
            layer_creation_log: Vec::new(),
            frame_dump_events: vec!["slow_frame".to_string()],
            slow_frame_threshold: Duration::from_millis(1_000),
            frame_number: 0,
            pending_layer_creation: None,
            completed_layer_creation: None,
            frame_instrumentation: None,
            last_finished_frame_timing: None,
            render_plan: crate::PrecisionRenderPlan::default(),
        }
    }

    /// Adds at most one layer per frame, keeping the deque ordered max-to-min zoom.
    pub fn expand_one_layer_per_frame(&mut self) -> bool {
        if self.layers.is_empty() {
            let mut layer = TileLayer::new(
                self.position.clone(),
                self.tile_width,
                self.tile_height,
                self.delta,
                self.screen_position,
                self.initial_apparent_pixel_size,
            );
            layer.set_render_plan(self.render_plan);
            self.layers.push_back(layer);
            self.synchronize_layer_positions();
            self.pending_layer_creation = Some(PendingLayerCreation {
                direction: None,
                layer_index: self.layers.len() - 1,
            });
            return true;
        }

        let front_zoom = self.layers.front().unwrap().zoom();
        let larger_zoom = front_zoom * 2.0;
        if larger_zoom <= self.max_apparent_pixel_size && !self.has_zoom(larger_zoom) {
            let mut layer = Self::adjacent_layer(self.layers.front().unwrap(), 2.0);
            layer.set_render_plan(self.render_plan);
            self.layers.push_front(layer);
            self.synchronize_layer_positions();
            self.pending_layer_creation = Some(PendingLayerCreation {
                direction: Some("maior"),
                layer_index: 0,
            });
            return true;
        }
        let back_zoom = self.layers.back().unwrap().zoom();
        let smaller_zoom = back_zoom / 2.0;
        if smaller_zoom >= self.min_apparent_pixel_size && !self.has_zoom(smaller_zoom) {
            let mut layer = Self::adjacent_layer(self.layers.back().unwrap(), 0.5);
            layer.set_render_plan(self.render_plan);
            self.layers.push_back(layer);
            self.synchronize_layer_positions();
            self.pending_layer_creation = Some(PendingLayerCreation {
                direction: Some("menor"),
                layer_index: self.layers.len() - 1,
            });
            return true;
        }
        false
    }

    fn has_zoom(&self, candidate: f64) -> bool {
        self.layers.iter().any(|layer| {
            let scale = candidate.abs().max(layer.zoom().abs()).max(1.0);
            (layer.zoom() - candidate).abs() <= scale * f64::EPSILON * 8.0
        })
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Selects the apparent pixel size used when the first layer is created.
    pub fn set_initial_zoom(&mut self, zoom: f64) {
        assert!(
            self.layers.is_empty(),
            "initial zoom must be set before layers are created"
        );
        assert!(
            zoom >= self.min_apparent_pixel_size && zoom <= self.max_apparent_pixel_size,
            "initial zoom must be within the configured limits"
        );
        self.initial_apparent_pixel_size = zoom;
        self.camera_anchor_screen = crate::geometry::ScreenPoint::new(
            self.screen_position.x + ((self.tile_width as f64 * zoom - 1.0) / 2.0).round() as i32,
            self.screen_position.y + ((self.tile_height as f64 * zoom - 1.0) / 2.0).round() as i32,
        );
        self.camera_scale = zoom / self.delta;
    }

    /// Current apparent size of one complex-plane sample in screen pixels.
    pub fn apparent_pixel_size(&self) -> f64 {
        self.camera_scale * self.delta
    }

    pub fn set_render_plan(&mut self, plan: crate::PrecisionRenderPlan) {
        self.render_plan = plan;
        for layer in &mut self.layers {
            layer.set_render_plan(plan);
        }
    }

    pub fn layer(&self, index: usize) -> Option<&TileLayer> {
        self.layers.get(index)
    }

    pub fn layer_mut(&mut self, index: usize) -> Option<&mut TileLayer> {
        self.layers.get_mut(index)
    }

    pub fn layers(&self) -> &VecDeque<TileLayer> {
        &self.layers
    }

    pub fn layers_mut(&mut self) -> &mut VecDeque<TileLayer> {
        &mut self.layers
    }

    pub fn invalidate_tiles(&self) {
        for layer in &self.layers {
            layer.invalidate_tiles();
        }
    }

    /// Commands applied since the canvas was created, in replay order.
    pub fn navigation_history(&self) -> &[CanvasNavigationEvent] {
        &self.navigation_history
    }

    /// Lines emitted when the canvas creates a new layer.
    pub fn layer_creation_log(&self) -> &[String] {
        &self.layer_creation_log
    }

    pub fn set_frame_dump_events(&mut self, events: Vec<String>) {
        self.frame_dump_events = events;
    }

    pub fn set_slow_frame_threshold_ms(&mut self, threshold_ms: u64) {
        self.slow_frame_threshold = Duration::from_millis(threshold_ms);
    }

    pub fn begin_frame(&mut self) {
        self.begin_frame_at(Instant::now());
    }

    fn begin_frame_at(&mut self, frame_started_at: Instant) {
        let previous_frame = self.frame_instrumentation.take();
        let completed_layer_creation = self.completed_layer_creation.take();
        let timing = if let Some(mut frame) = previous_frame {
            let frame_duration = frame.duration_at(frame_started_at);
            frame.record_slow_if_needed_at(frame_started_at, self.slow_frame_threshold);
            let triggering_events =
                frame.matching_triggers(&self.frame_dump_events, self.slow_frame_threshold);
            if !triggering_events.is_empty() {
                frame.record_dump_started();
                crate::print_local!(
                    "Frame #{} dump disparado por: {}",
                    frame.frame_number,
                    triggering_events.join(", ")
                );
                if let Some(mut creation) = completed_layer_creation {
                    creation.diagnostics.frame_interval = frame_duration;
                    self.record_layer_creation(
                        creation.direction,
                        creation.delta,
                        creation.diagnostics,
                    );
                }
                frame.dump();
                frame.record_dump_finished();
                frame.dump_last_event();
            }
            Some((frame.frame_number, frame_duration))
        } else {
            None
        };
        self.last_finished_frame_timing = timing;
        self.frame_number = self.frame_number.saturating_add(1);
        self.frame_instrumentation = Some(FrameInstrumentation::new_at(
            self.frame_number,
            frame_started_at,
        ));
    }

    pub(crate) fn record_frame_event(
        &mut self,
        kind: FrameEventKind,
        description: impl Into<String>,
    ) {
        if let Some(frame) = self.frame_instrumentation.as_mut() {
            frame.record(kind, description);
        }
    }

    pub fn record_frame_presentation_started(&mut self) {
        if let Some(frame) = self.frame_instrumentation.as_mut() {
            frame.record_presentation_started();
        }
    }

    pub fn record_frame_presentation_finished(&mut self) {
        if let Some(frame) = self.frame_instrumentation.as_mut() {
            frame.record_presentation_finished();
        }
    }

    pub fn finish_frame(&mut self) {
        self.finish_frame_at(Instant::now());
    }

    fn finish_frame_at(&mut self, finished_at: Instant) {
        if let Some(frame) = self.frame_instrumentation.as_mut() {
            frame.record_finalization_at(finished_at);
        }
    }

    pub fn last_finished_frame_timing(&self) -> Option<(u64, Duration)> {
        self.last_finished_frame_timing
    }

    pub fn current_frame_number(&self) -> u64 {
        self.frame_number
    }

    /// Parameters supplied at canvas creation, before any navigation command.
    pub fn initial_state(&self) -> CanvasInitialState {
        CanvasInitialState {
            position: self.position.clone(),
            tile_width: self.tile_width,
            tile_height: self.tile_height,
            delta: self.delta,
            screen_position: self.screen_position,
            max_apparent_pixel_size: self.max_apparent_pixel_size,
            min_apparent_pixel_size: self.min_apparent_pixel_size,
        }
    }

    fn record_layer_creation(
        &mut self,
        direction: Option<&str>,
        delta: f64,
        diagnostics: LayerCreationDiagnostics,
    ) {
        let delta_exponent = -delta.log2();
        let line = match direction {
            Some(direction) => format!(
                "Camada {direction} criada, delta: {delta_exponent:.3}, {}",
                diagnostics.format()
            ),
            None => format!(
                "Camada criada, delta: {delta_exponent:.3}, {}",
                diagnostics.format()
            ),
        };
        crate::print_local!("{line}");
        self.layer_creation_log.push(line);
    }

    pub fn screen_to_complex(
        &self,
        point: crate::geometry::ScreenPoint,
    ) -> crate::geometry::ComplexPoint<f64> {
        crate::geometry::ComplexPoint::new(
            self.camera_anchor_complex.x
                + (point.x - self.camera_anchor_screen.x) as f64 / self.camera_scale,
            self.camera_anchor_complex.y
                - (point.y - self.camera_anchor_screen.y) as f64 / self.camera_scale,
        )
    }

    pub fn complex_to_screen(
        &self,
        point: crate::geometry::ComplexPoint<f64>,
    ) -> crate::geometry::ScreenPoint {
        crate::geometry::ScreenPoint::new(
            self.camera_anchor_screen.x
                + ((point.x - self.camera_anchor_complex.x) * self.camera_scale).round() as i32,
            self.camera_anchor_screen.y
                - ((point.y - self.camera_anchor_complex.y) * self.camera_scale).round() as i32,
        )
    }

    fn synchronize_layer_positions(&mut self) {
        let anchor_complex = self.camera_anchor_complex.clone();
        let anchor_screen = self.camera_anchor_screen;
        let scale = self.camera_scale;
        for layer in &mut self.layers {
            let position = layer.position();
            layer.set_screen_origin(
                anchor_screen.x as f64 + (position.x - anchor_complex.x) * scale,
                anchor_screen.y as f64 - (position.y - anchor_complex.y) * scale,
            );
        }
    }

    pub fn ensure_screen_coverage(&mut self, bounds: (i32, i32, i32, i32)) {
        if self.frame_instrumentation.is_none() {
            self.frame_number = self.frame_number.saturating_add(1);
            self.frame_instrumentation = Some(FrameInstrumentation::new(self.frame_number));
        }
        if self.retract_one_layer_per_frame() && self.layers.is_empty() {
            self.expand_one_layer_per_frame();
        } else {
            self.expand_one_layer_per_frame();
        }
        for layer in &mut self.layers {
            layer.ensure_screen_coverage(bounds);
        }
        if let Some(pending) = self.pending_layer_creation.take() {
            let diagnostics = self.layers[pending.layer_index].take_creation_diagnostics();
            let delta = self.layers[pending.layer_index].delta();
            let description = match pending.direction {
                Some(direction) => format!("camada {direction} criada"),
                None => "camada criada".to_string(),
            };
            self.frame_instrumentation
                .as_mut()
                .expect("frame instrumentation must be started before coverage")
                .record_trigger(FrameEventKind::LayerCreated, description);
            self.completed_layer_creation = Some(CompletedLayerCreation {
                direction: pending.direction,
                delta,
                diagnostics,
            });
        }
    }

    fn retract_one_layer_per_frame(&mut self) -> bool {
        if self
            .layers
            .front()
            .is_some_and(|layer| layer.zoom() > self.max_apparent_pixel_size)
        {
            if self
                .layers
                .get(1)
                .is_some_and(TileLayer::is_ready_for_display)
            {
                self.layers.pop_front();
                return true;
            }
        }
        if self
            .layers
            .back()
            .is_some_and(|layer| layer.zoom() < self.min_apparent_pixel_size)
        {
            self.layers.pop_back();
            return true;
        }
        false
    }

    fn adjacent_layer(tip: &TileLayer, zoom_factor: f64) -> TileLayer {
        let zoom = tip.zoom() * zoom_factor;
        let delta = tip.delta() * zoom_factor;
        let center = crate::geometry::ComplexPoint::new(
            tip.position().x + (tip.tile_width() - 1) as f64 * tip.delta() / 2.0,
            tip.position().y - (tip.tile_height() - 1) as f64 * tip.delta() / 2.0,
        );
        let position = crate::geometry::ComplexPoint::new(
            center.x - (tip.tile_width() - 1) as f64 * delta / 2.0,
            center.y + (tip.tile_height() - 1) as f64 * delta / 2.0,
        );
        let (tip_origin_x, tip_origin_y) = tip.screen_origin();
        let screen_origin_x = tip_origin_x + (tip.tile_width() - 1) as f64 * tip.zoom() / 2.0
            - (tip.tile_width() - 1) as f64 * zoom / 2.0;
        let screen_origin_y = tip_origin_y + (tip.tile_height() - 1) as f64 * tip.zoom() / 2.0
            - (tip.tile_height() - 1) as f64 * zoom / 2.0;
        let mut layer = TileLayer::new(
            position,
            tip.tile_width(),
            tip.tile_height(),
            delta,
            crate::geometry::ScreenPoint::new(
                screen_origin_x.round() as i32,
                screen_origin_y.round() as i32,
            ),
            zoom,
        );
        layer.set_screen_origin(screen_origin_x, screen_origin_y);
        layer
    }

    pub fn trim_outside_allocation(&mut self, bounds: (i32, i32, i32, i32)) {
        for layer in &mut self.layers {
            layer.trim_outside_allocation(bounds);
        }
    }

    pub fn drag(&mut self, delta: crate::geometry::ScreenPoint) {
        self.navigation_history
            .push(CanvasNavigationEvent::Drag { delta });
        self.camera_anchor_screen = crate::geometry::ScreenPoint::new(
            self.camera_anchor_screen.x + delta.x,
            self.camera_anchor_screen.y + delta.y,
        );
        self.synchronize_layer_positions();
    }

    pub fn zoom_at(&mut self, cursor: crate::geometry::ScreenPoint, zoom: f64) {
        self.navigation_history
            .push(CanvasNavigationEvent::Zoom { cursor, zoom });
        let current_zoom = self
            .layers
            .front()
            .map_or(self.initial_apparent_pixel_size, TileLayer::zoom);
        let scale = zoom / current_zoom;
        let cursor_complex = self.screen_to_complex(cursor);
        self.camera_anchor_complex = cursor_complex;
        self.camera_anchor_screen = cursor;
        self.camera_scale *= scale;
        for layer in &mut self.layers {
            layer.zoom_at(cursor, layer.zoom() * scale);
        }
        self.synchronize_layer_positions();
    }
}

impl TileLayer {
    fn is_ready_for_display(&self) -> bool {
        self.tiles
            .iter()
            .flatten()
            .all(|tile| tile.status() == TileStatus::Completed && tile.sprite().is_some())
    }

    pub fn from_tile(
        tile: Arc<Tile>,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        let delta = tile.delta();
        let mut row = VecDeque::new();
        row.push_back(tile.clone());
        let mut tiles = VecDeque::new();
        tiles.push_back(row);
        Self::from_grid(
            tile.width(),
            tile.height(),
            delta,
            tiles,
            screen_position,
            zoom,
        )
    }

    pub fn new(
        position: crate::geometry::ComplexPoint<f64>,
        tile_width: u32,
        tile_height: u32,
        delta: f64,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        let tile_creation_started = Instant::now();
        let tile = Arc::new(Tile::new(position.clone(), tile_width, tile_height, delta));
        let tile_creation = tile_creation_started.elapsed();
        let grid_insertion_started = Instant::now();
        let mut row = VecDeque::new();
        row.push_back(tile);
        let mut tiles = VecDeque::new();
        tiles.push_back(row);
        let mut layer =
            Self::from_grid(tile_width, tile_height, delta, tiles, screen_position, zoom);
        layer.creation_diagnostics.tile_creation += tile_creation;
        layer.creation_diagnostics.grid_insertion += grid_insertion_started.elapsed();
        layer.creation_diagnostics.tiles_created += 1;
        layer
    }

    fn from_grid(
        tile_width: u32,
        tile_height: u32,
        delta: f64,
        tiles: VecDeque<VecDeque<Arc<Tile>>>,
        screen_position: crate::geometry::ScreenPoint,
        zoom: f64,
    ) -> Self {
        assert!(delta > 0.0, "layer delta must be positive");
        assert!(
            tile_width > 0 && tile_height > 0,
            "layer tile size must be positive"
        );
        assert!(zoom > 0.0, "layer zoom must be positive");
        assert!(!tiles.is_empty(), "a layer must have at least one row");
        let columns = tiles.front().map_or(0, VecDeque::len);
        assert!(columns > 0, "a layer must have at least one column");
        assert!(
            tiles.iter().all(|row| row.len() == columns),
            "layer rows must have equal lengths"
        );
        assert!(
            tiles
                .iter()
                .flatten()
                .all(|tile| tile.width() == tile_width && tile.height() == tile_height),
            "layer tile sizes must match"
        );
        let mut layer = Self {
            tile_width,
            tile_height,
            delta,
            tiles,
            screen_position,
            screen_origin_x: screen_position.x as f64,
            screen_origin_y: screen_position.y as f64,
            zoom,
            work_queue: Arc::new(TileWorkQueue::new()),
            render_plan: crate::PrecisionRenderPlan::default(),
            creation_diagnostics: LayerCreationDiagnostics::default(),
        };
        let enqueue_started = Instant::now();
        for (row_index, row) in layer.tiles.iter().enumerate() {
            for (column_index, tile) in row.iter().enumerate() {
                layer
                    .work_queue
                    .enqueue(Arc::clone(tile), row_index, column_index);
            }
        }
        layer.creation_diagnostics.queue_enqueuing += enqueue_started.elapsed();
        layer.creation_diagnostics.tiles_enqueued +=
            layer.tiles.iter().map(VecDeque::len).sum::<usize>();
        layer
    }

    pub fn position(&self) -> &crate::geometry::ComplexPoint<f64> {
        self.corner_tile()
            .expect("layer must contain a tile")
            .coordinate()
    }
    pub fn delta(&self) -> f64 {
        self.delta
    }
    pub fn tile_width(&self) -> u32 {
        self.tile_width
    }
    pub fn tile_height(&self) -> u32 {
        self.tile_height
    }
    pub fn row_count(&self) -> usize {
        self.tiles.len()
    }
    pub fn column_count(&self) -> usize {
        self.tiles.front().map_or(0, VecDeque::len)
    }
    pub fn tile(&self, row: usize, column: usize) -> Option<&Arc<Tile>> {
        self.tiles.get(row)?.get(column)
    }
    pub fn corner_tile(&self) -> Option<&Arc<Tile>> {
        self.tile(0, 0)
    }
    pub fn center_tile(&self) -> Option<&Arc<Tile>> {
        self.tile(self.row_count() / 2, self.column_count() / 2)
    }
    pub fn screen_position(&self) -> crate::geometry::ScreenPoint {
        self.screen_position
    }
    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn render_plan(&self) -> crate::PrecisionRenderPlan {
        self.render_plan
    }

    pub fn set_render_plan(&mut self, plan: crate::PrecisionRenderPlan) {
        self.render_plan = plan;
    }
    pub fn screen_to_complex(
        &self,
        point: crate::geometry::ScreenPoint,
    ) -> crate::geometry::ComplexPoint<f64> {
        let origin = self.position();
        crate::geometry::ComplexPoint::new(
            origin.x + (point.x as f64 - self.screen_origin_x) * self.delta / self.zoom,
            origin.y - (point.y as f64 - self.screen_origin_y) * self.delta / self.zoom,
        )
    }
    pub fn complex_to_screen(
        &self,
        point: crate::geometry::ComplexPoint<f64>,
    ) -> crate::geometry::ScreenPoint {
        let origin = self.position();
        crate::geometry::ScreenPoint::new(
            (self.screen_origin_x + (point.x - origin.x) / self.delta * self.zoom).round() as i32,
            (self.screen_origin_y + (origin.y - point.y) / self.delta * self.zoom).round() as i32,
        )
    }
    pub fn pending_work_positions(&self) -> Vec<(usize, usize)> {
        self.work_queue.pending_positions()
    }

    fn enqueue_created_tile(&mut self, tile: Arc<Tile>, row: usize, column: usize) {
        let enqueue_started = Instant::now();
        self.work_queue.enqueue(tile, row, column);
        self.creation_diagnostics.queue_enqueuing += enqueue_started.elapsed();
        self.creation_diagnostics.tiles_enqueued += 1;
    }

    fn take_creation_diagnostics(&mut self) -> LayerCreationDiagnostics {
        std::mem::take(&mut self.creation_diagnostics)
    }

    fn invalidate_tiles(&self) {
        for (row_index, row) in self.tiles.iter().enumerate() {
            for (column_index, tile) in row.iter().enumerate() {
                self.work_queue.remove_tile(tile);
                tile.invalidate();
                self.work_queue
                    .enqueue(Arc::clone(tile), row_index, column_index);
            }
        }
    }
    pub fn screen_size(&self) -> (u32, u32) {
        (
            ((self.tile_width as f64 * self.zoom).round() as u32).max(1)
                * self.column_count() as u32,
            ((self.tile_height as f64 * self.zoom).round() as u32).max(1) * self.row_count() as u32,
        )
    }
    pub fn tile_screen_size(&self) -> (u32, u32) {
        (
            ((self.tile_width as f64 * self.zoom).round() as u32).max(1),
            ((self.tile_height as f64 * self.zoom).round() as u32).max(1),
        )
    }
    pub fn set_screen_position(&mut self, position: crate::geometry::ScreenPoint) {
        self.set_screen_origin(position.x as f64, position.y as f64);
    }
    fn screen_origin(&self) -> (f64, f64) {
        (self.screen_origin_x, self.screen_origin_y)
    }
    fn set_screen_origin(&mut self, x: f64, y: f64) {
        self.screen_origin_x = x;
        self.screen_origin_y = y;
        self.screen_position =
            crate::geometry::ScreenPoint::new(x.round() as i32, y.round() as i32);
    }
    pub fn zoom_at(&mut self, cursor: crate::geometry::ScreenPoint, zoom: f64) {
        assert!(zoom > 0.0, "layer zoom must be positive");
        let scale = zoom / self.zoom;
        self.set_screen_origin(
            cursor.x as f64 - (cursor.x as f64 - self.screen_origin_x) * scale,
            cursor.y as f64 - (cursor.y as f64 - self.screen_origin_y) * scale,
        );
        self.zoom = zoom;
    }

    pub fn ensure_screen_coverage(&mut self, bounds: (i32, i32, i32, i32)) {
        let coverage_started = Instant::now();
        let (left, top, right, bottom) = bounds;
        let (tile_width, tile_height) = self.tile_screen_size();
        let complex_width = self.tile_width as f64 * self.delta;
        let complex_height = self.tile_height as f64 * self.delta;

        while self.screen_position.x > left {
            let origin = self.position().clone();
            self.screen_position.x -= tile_width as i32;
            self.screen_origin_x -= tile_width as f64;
            for row_index in 0..self.row_count() {
                let tile = self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x - complex_width,
                    origin.y - row_index as f64 * complex_height,
                ));
                let grid_started = Instant::now();
                self.tiles[row_index].push_front(Arc::clone(&tile));
                self.creation_diagnostics.grid_insertion += grid_started.elapsed();
                self.enqueue_created_tile(tile, row_index, 0);
            }
        }
        while self.screen_position.y > top {
            let origin = self.position().clone();
            self.screen_position.y -= tile_height as i32;
            self.screen_origin_y -= tile_height as f64;
            let mut row = VecDeque::new();
            for column_index in 0..self.column_count() {
                let tile = self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x + column_index as f64 * complex_width,
                    origin.y + complex_height,
                ));
                let grid_started = Instant::now();
                row.push_back(Arc::clone(&tile));
                self.creation_diagnostics.grid_insertion += grid_started.elapsed();
                self.enqueue_created_tile(tile, 0, column_index);
            }
            self.tiles.push_front(row);
        }
        while self.screen_position.x + self.screen_size().0 as i32 - 1 < right {
            let column_index = self.column_count();
            let origin = self.position().clone();
            for row_index in 0..self.row_count() {
                let tile = self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x + column_index as f64 * complex_width,
                    origin.y - row_index as f64 * complex_height,
                ));
                let grid_started = Instant::now();
                self.tiles[row_index].push_back(Arc::clone(&tile));
                self.creation_diagnostics.grid_insertion += grid_started.elapsed();
                self.enqueue_created_tile(tile, row_index, column_index);
            }
        }
        while self.screen_position.y + self.screen_size().1 as i32 - 1 < bottom {
            let row_index = self.row_count();
            let origin = self.position().clone();
            let mut row = VecDeque::new();
            for column_index in 0..self.column_count() {
                let tile = self.make_tile_at(crate::geometry::ComplexPoint::new(
                    origin.x + column_index as f64 * complex_width,
                    origin.y - row_index as f64 * complex_height,
                ));
                let grid_started = Instant::now();
                row.push_back(Arc::clone(&tile));
                self.creation_diagnostics.grid_insertion += grid_started.elapsed();
                self.enqueue_created_tile(tile, row_index, column_index);
            }
            self.tiles.push_back(row);
        }
        self.creation_diagnostics.coverage += coverage_started.elapsed();
    }

    pub fn trim_outside_allocation(&mut self, bounds: (i32, i32, i32, i32)) {
        let (left, top, right, bottom) = bounds;
        let (tile_width, tile_height) = self.tile_screen_size();

        let queue = Arc::clone(&self.work_queue);
        while self.column_count() > 1 && self.screen_position.x + tile_width as i32 - 1 < left {
            for row in &mut self.tiles {
                if let Some(tile) = row.front() {
                    queue.remove_tile(tile);
                }
                row.pop_front();
            }
            self.screen_position.x += tile_width as i32;
            self.screen_origin_x += tile_width as f64;
        }
        while self.column_count() > 1
            && self.screen_position.x + (self.column_count() as i32 - 1) * tile_width as i32 > right
        {
            for row in &mut self.tiles {
                if let Some(tile) = row.back() {
                    queue.remove_tile(tile);
                }
                row.pop_back();
            }
        }
        while self.row_count() > 1 && self.screen_position.y + tile_height as i32 - 1 < top {
            if let Some(row) = self.tiles.front() {
                for tile in row {
                    queue.remove_tile(tile);
                }
            }
            self.tiles.pop_front();
            self.screen_position.y += tile_height as i32;
            self.screen_origin_y += tile_height as f64;
        }
        while self.row_count() > 1
            && self.screen_position.y + (self.row_count() as i32 - 1) * tile_height as i32 > bottom
        {
            if let Some(row) = self.tiles.back() {
                for tile in row {
                    queue.remove_tile(tile);
                }
            }
            self.tiles.pop_back();
        }
    }

    fn make_tile_at(&mut self, coordinate: crate::geometry::ComplexPoint<f64>) -> Arc<Tile> {
        let tile_creation_started = Instant::now();
        let tile = Arc::new(Tile::new(
            coordinate,
            self.tile_width,
            self.tile_height,
            self.delta,
        ));
        self.creation_diagnostics.tile_creation += tile_creation_started.elapsed();
        self.creation_diagnostics.tiles_created += 1;
        tile
    }
}

impl TileSprite {
    pub fn new(tile: Arc<Tile>, position: crate::geometry::ScreenPoint, zoom: f64) -> Self {
        assert!(zoom > 0.0, "sprite zoom must be positive");
        Self {
            tile,
            position,
            zoom,
        }
    }

    pub fn tile(&self) -> &Arc<Tile> {
        &self.tile
    }

    pub fn position(&self) -> crate::geometry::ScreenPoint {
        self.position
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        assert!(zoom > 0.0, "sprite zoom must be positive");
        self.zoom = zoom;
    }

    pub fn zoom_at(&mut self, cursor: crate::geometry::ScreenPoint, zoom: f64) {
        assert!(zoom > 0.0, "sprite zoom must be positive");
        let scale = zoom / self.zoom;
        self.position = crate::geometry::ScreenPoint::new(
            (cursor.x as f64 - (cursor.x - self.position.x) as f64 * scale).round() as i32,
            (cursor.y as f64 - (cursor.y - self.position.y) as f64 * scale).round() as i32,
        );
        self.zoom = zoom;
    }

    pub fn screen_size(&self) -> (u32, u32) {
        (
            ((self.tile.width as f64 * self.zoom).round() as u32).max(1),
            ((self.tile.height as f64 * self.zoom).round() as u32).max(1),
        )
    }

    pub fn set_position(&mut self, position: crate::geometry::ScreenPoint) {
        self.position = position;
    }
}

impl Tile {
    pub fn new(
        coordinate: crate::geometry::ComplexPoint<f64>,
        width: u32,
        height: u32,
        delta: f64,
    ) -> Self {
        assert!(width > 0 && height > 0, "a tile must have a size");
        assert!(delta > 0.0, "tile delta must be positive");
        Self {
            coordinate,
            width,
            height,
            delta,
            status: AtomicU8::new(TileStatus::NotStarted as u8),
            iterations: Arc::new(Mutex::new(Vec::new())),
            sprite: Arc::new(Mutex::new(None)),
        }
    }

    pub fn coordinate(&self) -> &crate::geometry::ComplexPoint<f64> {
        &self.coordinate
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn delta(&self) -> f64 {
        self.delta
    }

    pub fn status(&self) -> TileStatus {
        TileStatus::from_u8(self.status.load(Ordering::Acquire))
    }

    pub fn iterations(&self) -> Arc<Mutex<Vec<u64>>> {
        Arc::clone(&self.iterations)
    }

    fn prepare_iterations(&self) -> std::sync::MutexGuard<'_, Vec<u64>> {
        let mut iterations = self
            .iterations
            .lock()
            .expect("tile iterations mutex poisoned");
        iterations.resize(self.width as usize * self.height as usize, 0);
        iterations
    }

    pub fn sprite(&self) -> Option<Arc<crate::Sprite>> {
        self.sprite
            .lock()
            .expect("tile sprite mutex poisoned")
            .clone()
    }

    pub fn set_sprite(&self, sprite: Arc<crate::Sprite>) {
        let mut stored = self.sprite.lock().expect("tile sprite mutex poisoned");
        if stored.is_none() {
            *stored = Some(sprite);
        }
    }

    fn invalidate(&self) {
        self.status
            .store(TileStatus::NotStarted as u8, Ordering::Release);
        self.sprite
            .lock()
            .expect("tile sprite mutex poisoned")
            .take();
    }
}

/// Dispatches tiles to the calculator.
pub struct Orchestrator {
    layer_queues: Arc<Mutex<VecDeque<RegisteredQueue>>>,
    available: Arc<Condvar>,
    stop_worker: Arc<AtomicBool>,
    worker_statuses: Arc<Mutex<Vec<WorkerStatus>>>,
    workers: Vec<JoinHandle<()>>,
    render_plan: Arc<RwLock<crate::PrecisionRenderPlan>>,
}

struct RegisteredQueue {
    queue: Arc<TileWorkQueue>,
    zoom: f64,
}

pub const DEFAULT_WORKER_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerTile {
    pub coordinate: crate::geometry::ComplexPoint<f64>,
    pub delta: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerStatus {
    pub id: usize,
    pub tile: Option<WorkerTile>,
}

impl Orchestrator {
    pub fn new(calculator: crate::Mandelbrot) -> Self {
        Self::with_worker_count(calculator, DEFAULT_WORKER_COUNT)
    }

    pub fn with_worker_count(calculator: crate::Mandelbrot, worker_count: usize) -> Self {
        Self::with_worker_count_and_plan(
            calculator,
            worker_count,
            crate::PrecisionRenderPlan::default(),
        )
    }

    pub fn with_worker_count_and_plan(
        calculator: crate::Mandelbrot,
        worker_count: usize,
        render_plan: crate::PrecisionRenderPlan,
    ) -> Self {
        assert!(worker_count > 0, "worker count must be positive");
        let layer_queues = Arc::new(Mutex::new(VecDeque::new()));
        let calculator = Arc::new(calculator);
        let render_plan = Arc::new(RwLock::new(render_plan));
        let available = Arc::new(Condvar::new());
        let stop_worker = Arc::new(AtomicBool::new(false));
        let worker_statuses = Arc::new(Mutex::new(
            (0..worker_count)
                .map(|id| WorkerStatus { id, tile: None })
                .collect::<Vec<_>>(),
        ));
        let mut workers = Vec::with_capacity(worker_count);
        for worker_id in 0..worker_count {
            let worker_queues = Arc::clone(&layer_queues);
            let worker_available = Arc::clone(&available);
            let worker_stop = Arc::clone(&stop_worker);
            let worker_states = Arc::clone(&worker_statuses);
            let worker_calculator = Arc::clone(&calculator);
            let worker_render_plan = Arc::clone(&render_plan);
            workers.push(thread::spawn(move || {
                while let Some(queued) = next_tile(&worker_queues, &worker_available, &worker_stop)
                {
                    worker_states.lock().expect("worker status mutex poisoned")[worker_id].tile =
                        Some(WorkerTile {
                            coordinate: queued.tile.coordinate().clone(),
                            delta: queued.tile.delta(),
                        });
                    let plan = *worker_render_plan
                        .read()
                        .expect("render plan lock poisoned");
                    calculate_tile_with_plan(&worker_calculator, &queued.tile, plan);
                    worker_states.lock().expect("worker status mutex poisoned")[worker_id].tile =
                        None;
                }
            }));
        }

        Self {
            layer_queues,
            available,
            stop_worker,
            worker_statuses,
            workers,
            render_plan,
        }
    }

    pub fn render_tile(&self, tile: &Arc<Tile>) {
        let queue = Arc::new(TileWorkQueue::new());
        self.register_queue(Arc::clone(&queue), 0.0);
        queue.enqueue(Arc::clone(tile), 0, 0);
        self.available.notify_one();
    }

    pub fn render_layer(&self, layer: &TileLayer) {
        self.register_queue(Arc::clone(&layer.work_queue), layer.zoom());
        self.available.notify_one();
    }

    pub fn worker_statuses(&self) -> Vec<WorkerStatus> {
        self.worker_statuses
            .lock()
            .expect("worker status mutex poisoned")
            .clone()
    }

    pub fn set_render_plan(&self, plan: crate::PrecisionRenderPlan) {
        *self.render_plan.write().expect("render plan lock poisoned") = plan;
        self.available.notify_all();
    }

    fn register_queue(&self, queue: Arc<TileWorkQueue>, zoom: f64) {
        let mut queues = self
            .layer_queues
            .lock()
            .expect("layer queue mutex poisoned");
        if let Some(registered) = queues
            .iter_mut()
            .find(|registered| Arc::ptr_eq(&registered.queue, &queue))
        {
            registered.zoom = zoom;
        } else {
            queues.push_back(RegisteredQueue { queue, zoom });
        }
        queues.make_contiguous().sort_by(|left, right| {
            right
                .zoom
                .partial_cmp(&left.zoom)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.available.notify_one();
    }
}

impl Drop for Orchestrator {
    fn drop(&mut self) {
        self.stop_worker.store(true, Ordering::Release);
        self.available.notify_all();
        for worker in self.workers.drain(..) {
            worker.join().expect("tile worker panicked");
        }
    }
}

struct QueuedTile {
    tile: Arc<Tile>,
    row: usize,
    column: usize,
}

struct TileWorkQueue {
    pending: Mutex<VecDeque<QueuedTile>>,
    active: AtomicBool,
}

impl TileWorkQueue {
    fn new() -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            active: AtomicBool::new(true),
        }
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
        self.pending
            .lock()
            .expect("tile queue mutex poisoned")
            .clear();
    }

    fn enqueue(&self, tile: Arc<Tile>, row: usize, column: usize) {
        if !self.is_active() {
            return;
        }
        if tile
            .status
            .compare_exchange(
                TileStatus::NotStarted as u8,
                TileStatus::Deferred as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.pending
                .lock()
                .expect("tile queue mutex poisoned")
                .push_back(QueuedTile { tile, row, column });
        }
    }

    fn pop_front(&self) -> Option<QueuedTile> {
        self.pending
            .lock()
            .expect("tile queue mutex poisoned")
            .pop_front()
    }

    fn remove_tile(&self, target: &Arc<Tile>) -> bool {
        let mut pending = self.pending.lock().expect("tile queue mutex poisoned");
        let original_len = pending.len();
        pending.retain(|queued| !Arc::ptr_eq(&queued.tile, target));
        pending.len() != original_len
    }

    fn pending_positions(&self) -> Vec<(usize, usize)> {
        self.pending
            .lock()
            .expect("tile queue mutex poisoned")
            .iter()
            .map(|queued| (queued.row, queued.column))
            .collect()
    }
}

fn next_tile(
    queues: &Mutex<VecDeque<RegisteredQueue>>,
    available: &Condvar,
    stop: &AtomicBool,
) -> Option<QueuedTile> {
    let mut queues_guard = queues.lock().expect("layer queue mutex poisoned");
    loop {
        for registered in queues_guard.iter() {
            if !registered.queue.is_active() {
                continue;
            }
            if let Some(tile) = registered.queue.pop_front() {
                if registered.queue.is_active() {
                    return Some(tile);
                }
            }
        }
        if stop.load(Ordering::Acquire) {
            return None;
        }
        queues_guard = available
            .wait(queues_guard)
            .expect("layer queue mutex poisoned");
    }
}

fn calculate_tile_with_plan(
    calculator: &crate::Mandelbrot,
    tile: &Tile,
    plan: crate::PrecisionRenderPlan,
) {
    let precision = match plan.method() {
        crate::RenderMethod::Direct { precision }
        | crate::RenderMethod::Perturbation { seed: precision } => precision,
    };
    match precision.technique {
        crate::PrecisionTechnique::Float => calculate_tile(calculator, tile),
        crate::PrecisionTechnique::Fixed => match precision.level {
            1 => calculate_fixed_tile::<1>(calculator.max_iterations(), tile),
            _ => calculate_fixed_tile::<2>(calculator.max_iterations(), tile),
        },
    }
}

fn calculate_fixed_tile<const N: usize>(max_iterations: u32, tile: &Tile) {
    let fractal = crate::MandelbrotFixed::<N>::new(max_iterations);
    let center_real = crate::Fixed::from_f64(tile.coordinate.x);
    let center_imaginary = crate::Fixed::from_f64(tile.coordinate.y);
    let delta = crate::Fixed::from_f64(tile.delta);
    let mut iterations = tile.prepare_iterations();
    for y in 0..tile.height {
        for x in 0..tile.width {
            let real = center_real.add(delta.mul(crate::Fixed::from_i64(x as i64)));
            let imaginary = center_imaginary.sub(delta.mul(crate::Fixed::from_i64(y as i64)));
            iterations[y as usize * tile.width as usize + x as usize] =
                fractal.escape_iterations(real, imaginary) as u64;
        }
    }
    tile.status
        .store(TileStatus::Completed as u8, Ordering::Release);
}

fn calculate_tile(calculator: &crate::Mandelbrot, tile: &Tile) {
    let mut iterations = tile.prepare_iterations();
    for y in 0..tile.height {
        for x in 0..tile.width {
            let real = tile.coordinate.x + x as f64 * tile.delta;
            let imaginary = tile.coordinate.y - y as f64 * tile.delta;
            iterations[y as usize * tile.width as usize + x as usize] =
                calculator.escape_iterations(real, imaginary) as u64;
        }
    }
    tile.status
        .store(TileStatus::Completed as u8, Ordering::Release);
}
