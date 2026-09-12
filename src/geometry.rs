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
    xmin: T,
    xmax: T,
    ymin: T,
    ymax: T,
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

    pub fn xmin(&self) -> &T {
        &self.xmin
    }

    pub fn xmax(&self) -> &T {
        &self.xmax
    }

    pub fn ymin(&self) -> &T {
        &self.ymin
    }

    pub fn ymax(&self) -> &T {
        &self.ymax
    }

    pub fn with_xmin(self, xmin: T) -> Self {
        Self { xmin, ..self }
    }

    pub fn with_xmax(self, xmax: T) -> Self {
        Self { xmax, ..self }
    }

    pub fn with_ymin(self, ymin: T) -> Self {
        Self { ymin, ..self }
    }

    pub fn with_ymax(self, ymax: T) -> Self {
        Self { ymax, ..self }
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
            *self.xmin() + x_ratio * (*self.xmax() - *self.xmin()),
            *self.ymax() - y_ratio * (*self.ymax() - *self.ymin()),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraEnvelope {
    complex: ComplexEnvelope<f64>,
}

impl CameraEnvelope {
    pub fn new(center: ComplexPoint<f64>, zoom: f64, size: ScreenSize) -> Self {
        assert!(zoom > 0.0, "camera zoom must be positive");
        let half_width = 2.0 / zoom;
        let half_height = half_width * size.height as f64 / size.width as f64;
        Self {
            complex: ComplexEnvelope::new(
                center.x - half_width,
                center.x + half_width,
                center.y - half_height,
                center.y + half_height,
            ),
        }
    }

    pub fn complex(&self) -> &ComplexEnvelope<f64> {
        &self.complex
    }

    pub fn screen_to_complex(&self, point: ScreenPoint, size: ScreenSize) -> ComplexPoint<f64> {
        self.complex.screen_to_complex(point, size)
    }

    pub fn complex_to_screen(&self, point: ComplexPoint<f64>, size: ScreenSize) -> ScreenPoint {
        let x = ((point.x - *self.complex.xmin()) / (*self.complex.xmax() - *self.complex.xmin())
            * (size.width - 1) as f64)
            .round() as i32;
        let y = ((*self.complex.ymax() - point.y) / (*self.complex.ymax() - *self.complex.ymin())
            * (size.height - 1) as f64)
            .round() as i32;
        ScreenPoint::new(x, y)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    center: ComplexPoint<f64>,
    zoom: f64,
    size: ScreenSize,
}

impl Camera {
    pub fn new(center: ComplexPoint<f64>, zoom: f64, size: ScreenSize) -> Self {
        assert!(zoom > 0.0, "camera zoom must be positive");
        Self { center, zoom, size }
    }

    pub fn center(&self) -> &ComplexPoint<f64> {
        &self.center
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn envelope(&self) -> CameraEnvelope {
        CameraEnvelope::new(self.center.clone(), self.zoom, self.size)
    }
}

#[cfg(test)]
mod tests {
    use super::{Camera, ComplexEnvelope, ComplexPoint, ScreenPoint, ScreenSize};

    #[test]
    fn camera_envelope_round_trips_screen_and_complex_points() {
        let camera = Camera::new(ComplexPoint::new(0.0, 0.0), 2.0, ScreenSize::new(800, 600));
        let envelope = camera.envelope();
        let complex =
            envelope.screen_to_complex(ScreenPoint::new(400, 300), ScreenSize::new(800, 600));
        let screen = envelope.complex_to_screen(complex, ScreenSize::new(800, 600));

        assert_eq!(screen, ScreenPoint::new(400, 300));
        assert_eq!(*envelope.complex().xmin(), -1.0);
        assert_eq!(*envelope.complex().xmax(), 1.0);
    }

    #[test]
    fn represents_complex_and_screen_coordinates() {
        let complex = ComplexPoint::new(-2.0, 1.5);
        let screen = ScreenPoint::new(10, 20);
        let envelope = ComplexEnvelope::new(-2.0, 2.0, -1.5, 1.5);

        assert_eq!(complex, ComplexPoint { x: -2.0, y: 1.5 });
        assert_eq!(screen, ScreenPoint { x: 10, y: 20 });
        assert_eq!(*envelope.xmin(), -2.0);
        assert_eq!(*envelope.xmax(), 2.0);
        assert_eq!(*envelope.ymin(), -1.5);
        assert_eq!(*envelope.ymax(), 1.5);
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

    #[test]
    fn changes_a_bound_by_returning_a_new_envelope() {
        let original = ComplexEnvelope::new(-2.0, 2.0, -1.5, 1.5);
        let changed = original.clone().with_xmax(3.0);

        assert_eq!(*original.xmax(), 2.0);
        assert_eq!(*changed.xmax(), 3.0);
    }
}
