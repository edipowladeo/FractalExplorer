/// A point in the complex plane.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexPoint<T> {
    pub x: T,
    pub y: T,
}

impl<T> ComplexPoint<T> {
    pub const fn new(x: T, y: T) -> Self {
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

/// Dimensions of a screen or framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    pub width: usize,
    pub height: usize,
}

impl ScreenSize {
    pub const fn new(width: usize, height: usize) -> Self {
        assert!(width > 0 && height > 0, "a screen must have a size");
        Self { width, height }
    }
}

/// Rectangular region of the complex plane.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexEnvelope<T> {
    pub xmin: T,
    pub xmax: T,
    pub ymin: T,
    pub ymax: T,
}

impl<T: PartialOrd> ComplexEnvelope<T> {
    pub fn new(xmin: T, xmax: T, ymin: T, ymax: T) -> Self {
        assert!(xmin < xmax, "xmin must be smaller than xmax");
        assert!(ymin < ymax, "ymin must be smaller than ymax");
        Self {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }
}

impl ComplexEnvelope<f64> {
    pub fn screen_to_complex(&self, point: ScreenPoint, size: ScreenSize) -> ComplexPoint<f64> {
        assert!(
            size.width > 1 && size.height > 1,
            "a screen must have at least 2 pixels per axis"
        );
        let x_ratio = point.x as f64 / (size.width - 1) as f64;
        let y_ratio = point.y as f64 / (size.height - 1) as f64;
        ComplexPoint::new(
            self.xmin + x_ratio * (self.xmax - self.xmin),
            self.ymax - y_ratio * (self.ymax - self.ymin),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ComplexEnvelope, ComplexPoint, ScreenPoint, ScreenSize};

    #[test]
    fn represents_complex_and_screen_coordinates() {
        let complex = ComplexPoint::new(-2.0, 1.5);
        let screen = ScreenPoint::new(10, 20);
        let envelope = ComplexEnvelope::new(-2.0, 2.0, -1.5, 1.5);

        assert_eq!(complex, ComplexPoint { x: -2.0, y: 1.5 });
        assert_eq!(screen, ScreenPoint { x: 10, y: 20 });
        assert_eq!(envelope.xmin, -2.0);
        assert_eq!(envelope.xmax, 2.0);
        assert_eq!(envelope.ymin, -1.5);
        assert_eq!(envelope.ymax, 1.5);
    }

    #[test]
    fn converts_screen_corners_to_envelope_corners() {
        let envelope = ComplexEnvelope::new(-2.0, 2.0, -1.5, 1.5);
        let size = ScreenSize::new(800, 600);

        assert_eq!(
            envelope.screen_to_complex(ScreenPoint::new(0, 0), size),
            ComplexPoint::new(-2.0, 1.5)
        );
        let bottom_right = envelope.screen_to_complex(ScreenPoint::new(799, 599), size);
        assert!((bottom_right.x - 2.0).abs() < f64::EPSILON);
        assert!((bottom_right.y + 1.5).abs() < f64::EPSILON);
    }
}
