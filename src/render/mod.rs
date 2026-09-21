#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(u64);

impl ImageId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageRevision(u64);

impl ImageRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    width: u32,
    height: u32,
}

impl Viewport {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl UvRect {
    pub const FULL: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageUpdateError {
    InvalidRgba8Length { expected: usize, actual: usize },
    DimensionsOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageUpdate {
    image: ImageId,
    revision: ImageRevision,
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

impl ImageUpdate {
    pub fn new(
        image: ImageId,
        revision: ImageRevision,
        width: u32,
        height: u32,
        rgba8: Vec<u8>,
    ) -> Result<Self, ImageUpdateError> {
        let Some(expected) = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
        else {
            return Err(ImageUpdateError::DimensionsOverflow);
        };
        if rgba8.len() != expected {
            return Err(ImageUpdateError::InvalidRgba8Length {
                expected,
                actual: rgba8.len(),
            });
        }
        Ok(Self {
            image,
            revision,
            width,
            height,
            rgba8,
        })
    }

    pub const fn image(&self) -> ImageId {
        self.image
    }

    pub const fn revision(&self) -> ImageRevision {
        self.revision
    }

    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn rgba8(&self) -> &[u8] {
        &self.rgba8
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileDraw {
    image: ImageId,
    revision: ImageRevision,
    destination: Rect,
    source: UvRect,
    layer: u32,
    opacity: f32,
}

impl TileDraw {
    pub fn new(image: ImageId, revision: ImageRevision, layer: u32) -> Self {
        Self {
            image,
            revision,
            destination: Rect::new(0, 0, 0, 0),
            source: UvRect::FULL,
            layer,
            opacity: 1.0,
        }
    }

    pub const fn image(&self) -> ImageId {
        self.image
    }

    pub const fn revision(&self) -> ImageRevision {
        self.revision
    }

    pub const fn layer(&self) -> u32 {
        self.layer
    }

    pub const fn destination(&self) -> Rect {
        self.destination
    }

    pub const fn source(&self) -> UvRect {
        self.source
    }

    pub const fn opacity(&self) -> f32 {
        self.opacity
    }

    pub fn with_destination(mut self, destination: Rect) -> Self {
        self.destination = destination;
        self
    }

    pub fn with_source(mut self, source: UvRect) -> Self {
        self.source = source;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&opacity),
            "opacity must be between 0 and 1"
        );
        self.opacity = opacity;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayPrimitive {
    Image(TileDraw),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderFrame {
    frame_id: u64,
    viewport: Viewport,
    tiles: Vec<TileDraw>,
    overlays: Vec<OverlayPrimitive>,
}

impl RenderFrame {
    pub fn new(frame_id: u64, viewport: Viewport) -> Self {
        Self {
            frame_id,
            viewport,
            tiles: Vec::new(),
            overlays: Vec::new(),
        }
    }

    pub fn with_tile(mut self, tile: TileDraw) -> Self {
        self.tiles.push(tile);
        self
    }

    pub fn with_overlay(mut self, overlay: OverlayPrimitive) -> Self {
        self.overlays.push(overlay);
        self
    }

    pub const fn frame_id(&self) -> u64 {
        self.frame_id
    }

    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn tiles(&self) -> &[TileDraw] {
        &self.tiles
    }

    pub fn overlays(&self) -> &[OverlayPrimitive] {
        &self.overlays
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageId, ImageRevision, ImageUpdate, RenderFrame, TileDraw, Viewport};

    #[test]
    fn frame_references_stable_images_and_preserves_draw_order() {
        let frame = RenderFrame::new(7, Viewport::new(800, 600))
            .with_tile(TileDraw::new(ImageId::new(2), ImageRevision::new(3), 10))
            .with_tile(TileDraw::new(ImageId::new(1), ImageRevision::new(9), 20));

        assert_eq!(frame.frame_id(), 7);
        assert_eq!(frame.viewport(), Viewport::new(800, 600));
        assert_eq!(frame.tiles()[0].image(), ImageId::new(2));
        assert_eq!(frame.tiles()[0].revision(), ImageRevision::new(3));
        assert_eq!(frame.tiles()[0].layer(), 10);
        assert_eq!(frame.tiles()[1].image(), ImageId::new(1));
        assert_eq!(frame.tiles()[1].layer(), 20);
    }

    #[test]
    fn image_update_keeps_revision_and_validates_rgba_dimensions() {
        let update = ImageUpdate::new(
            ImageId::new(4),
            ImageRevision::new(8),
            2,
            1,
            vec![10, 20, 30, 255, 40, 50, 60, 255],
        )
        .expect("valid RGBA image");

        assert_eq!(update.image(), ImageId::new(4));
        assert_eq!(update.revision(), ImageRevision::new(8));
        assert_eq!(update.dimensions(), (2, 1));
        assert_eq!(update.rgba8(), &[10, 20, 30, 255, 40, 50, 60, 255]);
        assert!(
            ImageUpdate::new(ImageId::new(4), ImageRevision::new(9), 2, 1, vec![0; 4]).is_err()
        );
    }
}
