use crate::input::{InputEvent, ZoomDirection};
use crate::render::{
    FrameBuilder, ImageUpdate, OverlayPrimitive, PreparedFrame, RenderError, TileDraw, Viewport,
};
use crate::{Orchestrator, Sprite, TileSprite, TiledInfiniteCanvas};
use std::sync::Arc;

use std::sync::OnceLock;

static WINDOW_TITLE: OnceLock<String> = OnceLock::new();

pub fn title_with_suffix(suffix: Option<&str>) -> String {
    suffix.filter(|value| !value.trim().is_empty()).map_or_else(
        || "FractalExplorer".to_owned(),
        |value| format!("FractalExplorer - {}", value.trim()),
    )
}

pub fn initialize_window_title(suffix: Option<&str>) {
    let _ = WINDOW_TITLE.set(title_with_suffix(suffix));
}

pub fn window_title() -> &'static str {
    WINDOW_TITLE
        .get_or_init(|| title_with_suffix(None))
        .as_str()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    Resized(Viewport),
    Input(InputEvent),
    ConfigurationChanged,
    RedrawRequested,
    CloseRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEffect {
    Reconfigure,
    RequestRedraw,
    Render,
    Exit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppActions {
    pub reconfigure: bool,
    pub request_redraw: bool,
    pub render: bool,
    pub exit: bool,
}

pub fn reduce_effects(effects: &[AppEffect]) -> AppActions {
    let mut actions = AppActions::default();
    for effect in effects {
        match effect {
            AppEffect::Reconfigure => actions.reconfigure = true,
            AppEffect::RequestRedraw => actions.request_redraw = true,
            AppEffect::Render => actions.render = true,
            AppEffect::Exit => actions.exit = true,
        }
    }
    actions
}

pub trait ApplicationController {
    fn handle_event(&mut self, event: AppEvent) -> Vec<AppEffect>;
    fn prepare_frame(&mut self) -> Result<PreparedFrame, RenderError>;
    fn take_input_events(&mut self) -> Vec<InputEvent>;
}

pub fn prepare_canvas_common(
    canvas: &mut TiledInfiniteCanvas,
    orchestrator: &Orchestrator,
    allocation_bounds: (i32, i32, i32, i32),
    deallocation_bounds: (i32, i32, i32, i32),
) {
    canvas.begin_frame();
    canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::ConfigurationProcessed,
        "configuracao processada",
    );
    canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::SurfacePrepared,
        "superficie preparada",
    );
    canvas.trim_outside_allocation(deallocation_bounds);
    canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::OutsideAllocationTrimmed,
        "tiles fora da alocacao removidos",
    );
    canvas.ensure_screen_coverage(allocation_bounds);
    canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::ScreenCoverageCompleted,
        "cobertura da tela concluida",
    );
    for layer in canvas.layers() {
        orchestrator.render_layer(layer);
    }
    canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::LayerWorkScheduled,
        "trabalho das camadas agendado",
    );
}

pub struct PreparedCanvasTile {
    pub tile_sprite: TileSprite,
    pub sprite: Arc<Sprite>,
    pub draw: TileDraw,
}

pub fn collect_completed_canvas_tiles(
    canvas: &TiledInfiniteCanvas,
    config: &crate::config::RendererConfig,
) -> Vec<PreparedCanvasTile> {
    let mut prepared = Vec::new();
    for (layer_index, layer) in canvas.layers().iter().enumerate() {
        let (tile_width, tile_height) = layer.tile_screen_size();
        for row in 0..layer.row_count() {
            for column in 0..layer.column_count() {
                let Some(tile) = layer.tile(row, column) else {
                    continue;
                };
                if tile.status() == crate::TileStatus::Completed && tile.sprite().is_none() {
                    let sprite = Arc::new(crate::renderer::sprite_from_tile(
                        tile,
                        config.effective_max_iterations() as u64,
                        config.palette,
                        config.palette_period,
                    ));
                    tile.set_sprite(sprite);
                }
                let Some(sprite) = tile.sprite() else {
                    continue;
                };
                let screen_position = layer.complex_to_screen(tile.coordinate().clone());
                let image_id = crate::render::ImageId::new(Arc::as_ptr(tile) as usize as u64);
                let revision = crate::render::ImageRevision::new(crate::gpu::sprite_content_hash(
                    sprite.pixels(),
                ));
                let draw = TileDraw::new(image_id, revision, layer_index as u32).with_destination(
                    crate::render::Rect::new(
                        screen_position.x,
                        screen_position.y,
                        tile_width,
                        tile_height,
                    ),
                );
                prepared.push(PreparedCanvasTile {
                    tile_sprite: TileSprite::new(Arc::clone(tile), screen_position, layer.zoom()),
                    sprite,
                    draw,
                });
            }
        }
    }
    prepared
}

pub fn apply_canvas_input(
    canvas: &mut TiledInfiniteCanvas,
    event: InputEvent,
    zoom_multiplier: f64,
    default_zoom: f64,
) -> Option<crate::geometry::ComplexPoint<f64>> {
    match event {
        InputEvent::Drag { delta } => canvas.drag(delta),
        InputEvent::MiddleClick(cursor) => return Some(canvas.screen_to_complex(cursor)),
        InputEvent::Zoom { direction, cursor } => {
            assert!(
                zoom_multiplier > 1.0,
                "zoom_multiplier must be greater than 1"
            );
            let current_zoom = canvas.layer(0).map_or(default_zoom, |layer| layer.zoom());
            let zoom = match direction {
                ZoomDirection::In => current_zoom * zoom_multiplier,
                ZoomDirection::Out => current_zoom / zoom_multiplier,
            };
            canvas.zoom_at(cursor, zoom);
        }
    }
    None
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OverlaySnapshot {
    pub layer_lines: Vec<String>,
    pub queue_lines: Vec<String>,
    pub worker_lines: Vec<String>,
}

pub fn prepare_overlay_snapshot(
    canvas: &TiledInfiniteCanvas,
    orchestrator: &Orchestrator,
    config: &crate::config::RendererConfig,
    cursor: Option<crate::geometry::ScreenPoint>,
) -> OverlaySnapshot {
    let debug = &config.debug;
    if !debug.overlays_enabled() {
        return OverlaySnapshot::default();
    }
    let mouse = cursor.map(|point| canvas.screen_to_complex(point));
    let layer_lines = if debug.text_overlay_layers {
        canvas
            .layers()
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                crate::renderer::format_layer_overlay(
                    index,
                    layer.zoom(),
                    -layer.delta().log2(),
                    layer.column_count(),
                    layer.row_count(),
                    mouse.clone(),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let queue_lines = if debug.text_overlay_queue {
        canvas
            .layers()
            .iter()
            .enumerate()
            .flat_map(|(layer_index, layer)| {
                layer
                    .pending_work_positions()
                    .into_iter()
                    .map(move |(row, column)| {
                        crate::renderer::format_worker_queue_line(
                            layer_index,
                            row,
                            column,
                            -layer.delta().log2(),
                        )
                    })
            })
            .collect()
    } else {
        Vec::new()
    };
    let worker_lines = if debug.text_overlay_workers {
        orchestrator
            .worker_statuses()
            .into_iter()
            .map(|worker| {
                crate::renderer::format_worker_status_line(worker.id, worker.tile.as_ref())
            })
            .collect()
    } else {
        Vec::new()
    };
    OverlaySnapshot {
        layer_lines,
        queue_lines,
        worker_lines,
    }
}

pub struct DefaultApplicationController {
    viewport: Viewport,
    closed: bool,
    frame_builder: FrameBuilder,
    pending_input: Vec<InputEvent>,
    published_frame: Option<PreparedFrame>,
}

impl DefaultApplicationController {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            viewport,
            closed: false,
            frame_builder: FrameBuilder::new(),
            pending_input: Vec::new(),
            published_frame: None,
        }
    }

    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn publish_frame(&mut self, frame: PreparedFrame) {
        self.viewport = frame.frame().viewport();
        self.published_frame = Some(frame);
    }

    pub fn publish_and_prepare_frame(
        &mut self,
        frame: PreparedFrame,
    ) -> Result<PreparedFrame, RenderError> {
        self.publish_frame(frame);
        self.prepare_frame()
    }

    pub fn build_prepared_frame(
        &mut self,
        viewport: Viewport,
        tiles: Vec<TileDraw>,
        overlays: Vec<OverlayPrimitive>,
        image_updates: Vec<ImageUpdate>,
    ) -> PreparedFrame {
        let frame = self.frame_builder.build(viewport, tiles, overlays);
        PreparedFrame::new(frame, image_updates)
    }

    pub fn prepare_canvas(
        &mut self,
        canvas: &mut TiledInfiniteCanvas,
        orchestrator: &Orchestrator,
        allocation_bounds: (i32, i32, i32, i32),
        deallocation_bounds: (i32, i32, i32, i32),
    ) {
        prepare_canvas_common(canvas, orchestrator, allocation_bounds, deallocation_bounds);
    }

    pub fn begin_tile_composition(&mut self, canvas: &mut TiledInfiniteCanvas) {
        canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::TileCompositionStarted,
            "composicao de tiles iniciada",
        );
    }
}

impl ApplicationController for DefaultApplicationController {
    fn handle_event(&mut self, event: AppEvent) -> Vec<AppEffect> {
        if self.closed {
            return Vec::new();
        }
        match event {
            AppEvent::Resized(viewport) => {
                self.viewport = viewport;
                self.published_frame = None;
                vec![AppEffect::RequestRedraw]
            }
            AppEvent::Input(input) => {
                self.pending_input.push(input);
                vec![AppEffect::RequestRedraw]
            }
            AppEvent::ConfigurationChanged => {
                vec![AppEffect::Reconfigure, AppEffect::RequestRedraw]
            }
            AppEvent::RedrawRequested => vec![AppEffect::Render],
            AppEvent::CloseRequested => {
                self.closed = true;
                vec![AppEffect::Exit]
            }
        }
    }

    fn prepare_frame(&mut self) -> Result<PreparedFrame, RenderError> {
        if self.closed {
            return Err(RenderError::InvalidFrame("application is closed"));
        }
        Ok(self.published_frame.clone().unwrap_or_else(|| {
            PreparedFrame::new(
                self.frame_builder
                    .build(self.viewport, Vec::new(), Vec::new()),
                Vec::new(),
            )
        }))
    }

    fn take_input_events(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.pending_input)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        reduce_effects, AppEffect, AppEvent, ApplicationController, DefaultApplicationController,
        PreparedFrame,
    };
    use crate::geometry::ScreenPoint;
    use crate::input::{InputEvent, ZoomDirection};
    use crate::render::Viewport;

    #[test]
    fn reduces_controller_effects_to_runtime_actions() {
        assert_eq!(
            reduce_effects(&[
                AppEffect::Reconfigure,
                AppEffect::RequestRedraw,
                AppEffect::Render,
                AppEffect::Exit,
            ]),
            super::AppActions {
                reconfigure: true,
                request_redraw: true,
                render: true,
                exit: true,
            }
        );
    }

    #[test]
    fn render_effect_is_preserved_for_the_window_runtime() {
        assert!(reduce_effects(&[AppEffect::Render]).render);
    }

    #[test]
    fn controller_normalizes_resize_redraw_and_close_events() {
        let mut controller = DefaultApplicationController::new(Viewport::new(800, 600));

        assert_eq!(
            controller.handle_event(AppEvent::Resized(Viewport::new(1024, 768))),
            vec![AppEffect::RequestRedraw]
        );
        assert_eq!(controller.viewport(), Viewport::new(1024, 768));
        let input = InputEvent::Zoom {
            direction: ZoomDirection::In,
            cursor: ScreenPoint::new(12, 20),
        };
        assert_eq!(
            controller.handle_event(AppEvent::Input(input)),
            vec![AppEffect::RequestRedraw]
        );
        assert_eq!(controller.take_input_events(), vec![input]);
        assert!(controller.take_input_events().is_empty());
        assert_eq!(
            controller.handle_event(AppEvent::ConfigurationChanged),
            vec![AppEffect::Reconfigure, AppEffect::RequestRedraw]
        );
        assert_eq!(
            controller.handle_event(AppEvent::RedrawRequested),
            vec![AppEffect::Render]
        );
        assert_eq!(
            controller.handle_event(AppEvent::CloseRequested),
            vec![AppEffect::Exit]
        );
        assert!(controller.is_closed());
    }

    #[test]
    fn controller_prepares_a_shared_frame_snapshot() {
        let mut controller = DefaultApplicationController::new(Viewport::new(320, 200));
        let published = PreparedFrame::new(
            crate::render::RenderFrame::new(9, Viewport::new(320, 200)),
            Vec::new(),
        );

        controller.publish_frame(published.clone());

        let prepared = controller.prepare_frame().unwrap();

        assert_eq!(prepared, published);
    }

    #[test]
    fn controller_publishes_and_prepares_a_frame_as_one_runtime_boundary() {
        let mut controller = DefaultApplicationController::new(Viewport::new(320, 200));
        let frame = PreparedFrame::new(
            crate::render::RenderFrame::new(12, Viewport::new(640, 400)),
            Vec::new(),
        );

        let prepared = controller.publish_and_prepare_frame(frame.clone()).unwrap();

        assert_eq!(prepared, frame);
        assert_eq!(controller.viewport(), Viewport::new(640, 400));
    }

    #[test]
    fn controller_builds_sequential_shared_frames_with_tiles_and_overlays() {
        let viewport = Viewport::new(320, 200);
        let mut controller = DefaultApplicationController::new(viewport);
        let tile = crate::render::TileDraw::new(
            crate::render::ImageId::new(5),
            crate::render::ImageRevision::new(2),
            3,
        );
        let overlay = crate::render::OverlayPrimitive::Image(crate::render::TileDraw::new(
            crate::render::ImageId::new(8),
            crate::render::ImageRevision::new(1),
            0,
        ));

        let first = controller.build_prepared_frame(
            viewport,
            vec![tile.clone()],
            vec![overlay.clone()],
            Vec::new(),
        );
        let second = controller.build_prepared_frame(viewport, Vec::new(), Vec::new(), Vec::new());

        assert_eq!(first.frame().frame_id(), 0);
        assert_eq!(first.frame().tiles(), &[tile]);
        assert_eq!(first.frame().overlays(), &[overlay]);
        assert_eq!(second.frame().frame_id(), 1);
    }

    #[test]
    fn shared_canvas_input_applies_zoom_and_reports_middle_click_coordinates() {
        let mut canvas = crate::TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            ScreenPoint::new(0, 0),
            8.0,
            0.8,
        );
        canvas.expand_one_layer_per_frame();
        let cursor = ScreenPoint::new(12, 20);
        let before = canvas.screen_to_complex(cursor);

        assert_eq!(
            super::apply_canvas_input(&mut canvas, InputEvent::MiddleClick(cursor), 1.2, 8.0,),
            Some(before.clone())
        );
        super::apply_canvas_input(
            &mut canvas,
            InputEvent::Zoom {
                direction: ZoomDirection::In,
                cursor,
            },
            2.0,
            8.0,
        );

        assert_eq!(canvas.layer(0).unwrap().zoom(), 16.0);
        assert_eq!(canvas.screen_to_complex(cursor), before);
    }

    #[test]
    fn overlay_snapshot_obeys_the_global_overlay_switch() {
        let canvas = crate::TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-1.0, 1.0),
            30,
            20,
            0.01,
            ScreenPoint::new(0, 0),
            8.0,
            0.8,
        );
        let orchestrator = crate::Orchestrator::new(crate::Mandelbrot::new(16));
        let mut config = crate::config::RendererConfig::default();
        config.debug.text_overlay_global = false;

        let snapshot = super::prepare_overlay_snapshot(&canvas, &orchestrator, &config, None);

        assert_eq!(snapshot, super::OverlaySnapshot::default());
    }
}
