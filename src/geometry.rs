/// A point in the complex plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexPoint {
    pub x: f64,
    pub y: f64,
}

impl ComplexPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A pixel coordinate on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenPoint {
    pub x: i32,
    pub y: i32,
}

impl ScreenPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Rectangular region of the complex plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Envelope {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

impl Envelope {
    pub const fn new(xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> Self {
        assert!(xmin < xmax, "xmin must be smaller than xmax");
        assert!(ymin < ymax, "ymin must be smaller than ymax");
        Self {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }

    pub fn screen_to_complex(
        self,
        point: ScreenPoint,
        width: usize,
        height: usize,
    ) -> ComplexPoint {
        assert!(
            width > 1 && height > 1,
            "a screen must have at least 2 pixels per axis"
        );
        let x_ratio = point.x as f64 / (width - 1) as f64;
        let y_ratio = point.y as f64 / (height - 1) as f64;
        ComplexPoint::new(
            self.xmin + x_ratio * (self.xmax - self.xmin),
            self.ymax - y_ratio * (self.ymax - self.ymin),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ComplexPoint, Envelope, ScreenPoint};

    #[test]
    fn represents_complex_and_screen_coordinates() {
        let complex = ComplexPoint::new(-2.0, 1.5);
        let screen = ScreenPoint::new(10, 20);
        let envelope = Envelope::new(-2.0, 2.0, -1.5, 1.5);

        assert_eq!(complex, ComplexPoint { x: -2.0, y: 1.5 });
        assert_eq!(screen, ScreenPoint { x: 10, y: 20 });
        assert_eq!(envelope.xmin, -2.0);
        assert_eq!(envelope.xmax, 2.0);
        assert_eq!(envelope.ymin, -1.5);
        assert_eq!(envelope.ymax, 1.5);
    }

    #[test]
    fn converts_screen_corners_to_envelope_corners() {
        let envelope = Envelope::new(-2.0, 2.0, -1.5, 1.5);

        assert_eq!(
            envelope.screen_to_complex(ScreenPoint::new(0, 0), 800, 600),
            ComplexPoint::new(-2.0, 1.5)
        );
        let bottom_right = envelope.screen_to_complex(ScreenPoint::new(799, 599), 800, 600);
        assert!((bottom_right.x - 2.0).abs() < f64::EPSILON);
        assert!((bottom_right.y + 1.5).abs() < f64::EPSILON);
    }
}
