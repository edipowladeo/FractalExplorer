use crate::input::InputEvent;
use crate::render::{FrameBuilder, PreparedFrame, RenderError, Viewport};
use crate::{Orchestrator, TiledInfiniteCanvas};

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
    pub exit: bool,
}

pub fn reduce_effects(effects: &[AppEffect]) -> AppActions {
    let mut actions = AppActions::default();
    for effect in effects {
        match effect {
            AppEffect::Reconfigure => actions.reconfigure = true,
            AppEffect::RequestRedraw => actions.request_redraw = true,
            AppEffect::Render => {}
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
                exit: true,
            }
        );
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
}
