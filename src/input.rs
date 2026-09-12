#[cfg(test)]
mod tests {
    use super::{InputEvent, InputState};
    use crate::geometry::ScreenPoint;

    #[test]
    fn emits_drag_delta_while_left_button_is_held() {
        let mut input = InputState::new();
        input.update(Some(ScreenPoint::new(20, 30)), true, false, 0.0);

        assert_eq!(
            input.update(Some(ScreenPoint::new(35, 24)), true, false, 0.0),
            vec![InputEvent::Drag {
                delta: ScreenPoint::new(15, -6)
            }]
        );
    }

    #[test]
    fn emits_middle_click_only_when_button_becomes_pressed() {
        let mut input = InputState::new();
        let cursor = ScreenPoint::new(100, 200);

        assert_eq!(
            input.update(Some(cursor), false, true, 0.0),
            vec![InputEvent::MiddleClick(cursor)]
        );
        assert!(input.update(Some(cursor), false, true, 0.0).is_empty());
    }

    #[test]
    fn emits_zoom_direction_from_vertical_wheel_delta() {
        let mut input = InputState::new();

        assert_eq!(
            input.update(Some(ScreenPoint::new(40, 50)), false, false, 1.0),
            vec![InputEvent::Zoom {
                direction: super::ZoomDirection::In,
                cursor: ScreenPoint::new(40, 50),
            }]
        );
        assert_eq!(
            input.update(Some(ScreenPoint::new(40, 50)), false, false, -1.0),
            vec![InputEvent::Zoom {
                direction: super::ZoomDirection::Out,
                cursor: ScreenPoint::new(40, 50),
            }]
        );
    }
}
use crate::geometry::ScreenPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Drag {
        delta: ScreenPoint,
    },
    MiddleClick(ScreenPoint),
    Zoom {
        direction: ZoomDirection,
        cursor: ScreenPoint,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomDirection {
    In,
    Out,
}

#[derive(Debug, Default)]
pub struct InputState {
    previous_mouse_position: Option<ScreenPoint>,
    left_button_was_down: bool,
    middle_button_was_down: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(
        &mut self,
        mouse_position: Option<ScreenPoint>,
        left_button_is_down: bool,
        middle_button_is_down: bool,
        scroll_y: f32,
    ) -> Vec<InputEvent> {
        let mut events = Vec::new();

        if let Some(current_mouse_position) = mouse_position {
            if left_button_is_down && self.left_button_was_down {
                if let Some(previous_mouse_position) = self.previous_mouse_position {
                    events.push(InputEvent::Drag {
                        delta: ScreenPoint::new(
                            current_mouse_position.x - previous_mouse_position.x,
                            current_mouse_position.y - previous_mouse_position.y,
                        ),
                    });
                }
            }
            self.previous_mouse_position = left_button_is_down.then_some(current_mouse_position);

            if middle_button_is_down && !self.middle_button_was_down {
                events.push(InputEvent::MiddleClick(current_mouse_position));
            }
        } else {
            self.previous_mouse_position = None;
        }

        if let Some(cursor) = mouse_position {
            if scroll_y > 0.0 {
                events.push(InputEvent::Zoom {
                    direction: ZoomDirection::In,
                    cursor,
                });
            } else if scroll_y < 0.0 {
                events.push(InputEvent::Zoom {
                    direction: ZoomDirection::Out,
                    cursor,
                });
            }
        }

        self.left_button_was_down = left_button_is_down;
        self.middle_button_was_down = middle_button_is_down;
        events
    }
}
