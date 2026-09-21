use crate::render::{FrameBuilder, RenderError, RenderFrame, Viewport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    Resized(Viewport),
    RedrawRequested,
    CloseRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEffect {
    RequestRedraw,
    Render,
    Exit,
}

pub trait ApplicationController {
    fn handle_event(&mut self, event: AppEvent) -> Vec<AppEffect>;
    fn prepare_frame(&mut self) -> Result<RenderFrame, RenderError>;
}

pub struct DefaultApplicationController {
    viewport: Viewport,
    closed: bool,
    frame_builder: FrameBuilder,
}

impl DefaultApplicationController {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            viewport,
            closed: false,
            frame_builder: FrameBuilder::new(),
        }
    }

    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
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
                vec![AppEffect::RequestRedraw]
            }
            AppEvent::RedrawRequested => vec![AppEffect::Render],
            AppEvent::CloseRequested => {
                self.closed = true;
                vec![AppEffect::Exit]
            }
        }
    }

    fn prepare_frame(&mut self) -> Result<RenderFrame, RenderError> {
        if self.closed {
            return Err(RenderError::InvalidFrame("application is closed"));
        }
        Ok(self
            .frame_builder
            .build(self.viewport, Vec::new(), Vec::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::{AppEffect, AppEvent, ApplicationController, DefaultApplicationController};
    use crate::render::Viewport;

    #[test]
    fn controller_normalizes_resize_redraw_and_close_events() {
        let mut controller = DefaultApplicationController::new(Viewport::new(800, 600));

        assert_eq!(
            controller.handle_event(AppEvent::Resized(Viewport::new(1024, 768))),
            vec![AppEffect::RequestRedraw]
        );
        assert_eq!(controller.viewport(), Viewport::new(1024, 768));
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
}
