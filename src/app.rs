use crate::input::InputEvent;
use crate::render::{FrameBuilder, PreparedFrame, RenderError, Viewport};

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

pub trait ApplicationController {
    fn handle_event(&mut self, event: AppEvent) -> Vec<AppEffect>;
    fn prepare_frame(&mut self) -> Result<PreparedFrame, RenderError>;
    fn take_input_events(&mut self) -> Vec<InputEvent>;
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
        AppEffect, AppEvent, ApplicationController, DefaultApplicationController, PreparedFrame,
    };
    use crate::geometry::ScreenPoint;
    use crate::input::{InputEvent, ZoomDirection};
    use crate::render::Viewport;

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
}
