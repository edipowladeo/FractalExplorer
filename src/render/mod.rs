pub mod cpu;
pub mod device;
pub mod gpu;
pub mod graphics;

use crate::geometry::ScreenPoint;
use std::collections::BTreeMap;

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

/// Stable identifier for a glyph in a font atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphId(u16);

impl GlyphId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Backend-independent metrics describing one glyph in an atlas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphMetrics {
    source: UvRect,
    size: (u32, u32),
    bearing: (i32, i32),
    advance: i32,
}

impl GlyphMetrics {
    pub const fn new(source: UvRect, size: (u32, u32), bearing: (i32, i32), advance: i32) -> Self {
        Self {
            source,
            size,
            bearing,
            advance,
        }
    }

    pub const fn source(self) -> UvRect {
        self.source
    }

    pub const fn size(self) -> (u32, u32) {
        self.size
    }

    pub const fn bearing(self) -> (i32, i32) {
        self.bearing
    }

    pub const fn advance(self) -> i32 {
        self.advance
    }
}

/// A backend-independent bitmap font atlas.
///
/// The atlas stores one alpha byte per pixel. Backends decide how to upload
/// and sample those pixels, while layout code only consumes glyph IDs and
/// metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct FontAtlas {
    width: u32,
    height: u32,
    line_height: u32,
    missing_advance: i32,
    pixels: Vec<u8>,
    glyphs: BTreeMap<char, (GlyphId, GlyphMetrics)>,
}

const DEBUG_FONT_CHARACTERS: &[char] = &[
    'C', '#', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'x', 'y', 'p', 'z', 'o', 'a', 'm',
    'd', 'e', 't', 'i', 'l', 's', '*', ':', '=', '.', '-', ' ',
];

impl FontAtlas {
    /// Builds a tightly packed, fixed-cell atlas from 5x7 bitmap glyphs.
    pub fn from_bitmap_glyphs(glyphs: &[(char, [u8; 7])]) -> Self {
        let cell_width = 6usize;
        let cell_height = 7usize;
        let atlas_width = glyphs.len().max(1) * cell_width;
        let height = cell_height;
        let mut pixels = vec![0; atlas_width * height];
        let mut entries = BTreeMap::new();

        for (index, &(character, bitmap)) in glyphs.iter().enumerate() {
            let left = index * cell_width;
            for (row, bits) in bitmap.iter().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        pixels[row * atlas_width + left + column] = 255;
                    }
                }
            }

            let atlas_width = atlas_width as f32;
            let source = UvRect {
                left: left as f32 / atlas_width,
                top: 0.0,
                right: (left + 5) as f32 / atlas_width,
                bottom: 1.0,
            };
            entries.insert(
                character,
                (
                    GlyphId::new(index as u16),
                    GlyphMetrics::new(source, (5, 7), (0, 0), cell_width as i32),
                ),
            );
        }

        Self {
            width: atlas_width as u32,
            height: height as u32,
            line_height: cell_height as u32,
            missing_advance: cell_width as i32,
            pixels,
            glyphs: entries,
        }
    }

    /// Builds the bitmap font currently used by diagnostic overlays.
    pub fn debug() -> Self {
        let glyphs = DEBUG_FONT_CHARACTERS
            .iter()
            .filter_map(|&character| {
                crate::renderer::glyph(character).map(|glyph| (character, glyph))
            })
            .collect::<Vec<_>>();
        Self::from_bitmap_glyphs(&glyphs)
    }

    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn glyph(&self, character: char) -> Option<(GlyphId, GlyphMetrics)> {
        self.glyphs.get(&character).copied()
    }

    /// Lays out a text run without rasterizing or touching backend resources.
    pub fn layout(&self, run: &TextRun) -> Vec<GlyphPlacement> {
        let scale = run.scale().min(i32::MAX as u32) as i32;
        let line_height = self.line_height.saturating_mul(run.scale());
        let line_height = line_height.min(i32::MAX as u32) as i32;
        let missing_advance = self.missing_advance.saturating_mul(scale);
        let mut pen_x = run.origin().x;
        let mut pen_y = run.origin().y;
        let mut placements = Vec::with_capacity(run.text().chars().count());

        for character in run.text().chars() {
            if character == '\n' {
                pen_x = run.origin().x;
                pen_y = pen_y.saturating_add(line_height);
                continue;
            }

            let Some((glyph, metrics)) = self.glyph(character) else {
                pen_x = pen_x.saturating_add(missing_advance);
                continue;
            };
            let (width, height) = metrics.size();
            let (bearing_x, bearing_y) = metrics.bearing();
            let destination = Rect::new(
                pen_x.saturating_add(bearing_x.saturating_mul(scale)),
                pen_y.saturating_add(bearing_y.saturating_mul(scale)),
                width.saturating_mul(run.scale()),
                height.saturating_mul(run.scale()),
            );
            placements.push(GlyphPlacement::new(
                glyph,
                destination,
                metrics.source(),
                run.color(),
                run.layer(),
            ));
            pen_x = pen_x.saturating_add(metrics.advance().saturating_mul(scale));
        }

        placements
    }
}

/// A backend-independent request to lay out one piece of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRun {
    text: String,
    origin: ScreenPoint,
    color: [u8; 4],
    scale: u32,
    layer: u32,
}

impl TextRun {
    pub fn new(text: impl Into<String>, origin: ScreenPoint) -> Self {
        Self {
            text: text.into(),
            origin,
            color: [255, 255, 255, 255],
            scale: 1,
            layer: 0,
        }
    }

    pub fn with_color(mut self, color: [u8; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_scale(mut self, scale: u32) -> Self {
        self.scale = scale.max(1);
        self
    }

    pub fn with_layer(mut self, layer: u32) -> Self {
        self.layer = layer;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn origin(&self) -> ScreenPoint {
        self.origin
    }

    pub const fn color(&self) -> [u8; 4] {
        self.color
    }

    pub const fn scale(&self) -> u32 {
        self.scale
    }

    pub const fn layer(&self) -> u32 {
        self.layer
    }
}

/// One laid-out glyph ready for a backend to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphPlacement {
    glyph: GlyphId,
    destination: Rect,
    source: UvRect,
    color: [u8; 4],
    layer: u32,
}

impl GlyphPlacement {
    pub const fn new(
        glyph: GlyphId,
        destination: Rect,
        source: UvRect,
        color: [u8; 4],
        layer: u32,
    ) -> Self {
        Self {
            glyph,
            destination,
            source,
            color,
            layer,
        }
    }

    pub const fn glyph(self) -> GlyphId {
        self.glyph
    }

    pub const fn destination(self) -> Rect {
        self.destination
    }

    pub const fn source(self) -> UvRect {
        self.source
    }

    pub const fn color(self) -> [u8; 4] {
        self.color
    }

    pub const fn layer(self) -> u32 {
        self.layer
    }
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

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedFrame {
    frame: RenderFrame,
    image_updates: Vec<ImageUpdate>,
}

impl PreparedFrame {
    pub fn new(frame: RenderFrame, image_updates: Vec<ImageUpdate>) -> Self {
        Self {
            frame,
            image_updates,
        }
    }

    pub fn frame(&self) -> &RenderFrame {
        &self.frame
    }

    pub fn image_updates(&self) -> &[ImageUpdate] {
        &self.image_updates
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderCapabilities {
    pub partial_image_updates: bool,
    pub persistent_resources: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFailure {
    Lost,
    Outdated,
    DeviceRemoved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    InvalidFrame(&'static str),
    BackendUnavailable(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameOutcome {
    submitted: bool,
}

impl FrameOutcome {
    pub const fn submitted() -> Self {
        Self { submitted: true }
    }

    pub const fn was_submitted(self) -> bool {
        self.submitted
    }
}

pub trait RenderTarget: Send {
    fn capabilities(&self) -> RenderCapabilities;
    fn resize(&mut self, viewport: Viewport) -> Result<(), RenderError>;
    fn update_images(&mut self, updates: &[ImageUpdate]) -> Result<(), RenderError>;
    fn render(&mut self, frame: &RenderFrame) -> Result<FrameOutcome, RenderError>;
    fn evict_images(&mut self, images: &[ImageId]);
    fn recover(&mut self, reason: SurfaceFailure) -> Result<(), RenderError>;
}

pub trait RenderTargetFactory: Send + Sync {
    fn create(&self, viewport: Viewport) -> Result<Box<dyn RenderTarget>, RenderError>;
}

/// Coordinates the lifecycle shared by every render target.
///
/// Platform adapters should feed frames through this type instead of deciding
/// independently when to resize, upload images, or render. The target remains
/// responsible only for backend-specific work.
pub struct RenderTargetSession<T> {
    target: T,
    viewport: Viewport,
}

impl<T: RenderTarget> RenderTargetSession<T> {
    pub fn new(target: T, viewport: Viewport) -> Self {
        Self { target, viewport }
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    pub fn target_mut(&mut self) -> &mut T {
        &mut self.target
    }

    pub fn submit(
        &mut self,
        viewport: Viewport,
        updates: &[ImageUpdate],
        frame: &RenderFrame,
    ) -> Result<FrameOutcome, RenderError> {
        if self.viewport != viewport {
            self.target.resize(viewport)?;
            self.viewport = viewport;
        }
        self.target.update_images(updates)?;
        self.target.render(frame)
    }

    pub fn recover(&mut self, reason: SurfaceFailure) -> Result<(), RenderError> {
        self.target.recover(reason)
    }
}

/// Owns the backend-independent submission sequence for one render target.
pub struct RenderTargetPipeline {
    target: Box<dyn RenderTarget>,
    viewport: Viewport,
}

impl RenderTargetPipeline {
    pub fn create(
        factory: &dyn RenderTargetFactory,
        viewport: Viewport,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            target: factory.create(viewport)?,
            viewport,
        })
    }

    pub fn submit(&mut self, prepared: &PreparedFrame) -> Result<FrameOutcome, RenderError> {
        if self.viewport != prepared.frame().viewport() {
            self.target.resize(prepared.frame().viewport())?;
            self.viewport = prepared.frame().viewport();
        }
        self.target.update_images(prepared.image_updates())?;
        self.target.render(prepared.frame())
    }

    pub fn evict_images(&mut self, images: &[ImageId]) {
        self.target.evict_images(images);
    }

    pub fn recover(&mut self, reason: SurfaceFailure) -> Result<(), RenderError> {
        self.target.recover(reason)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CpuRenderTargetFactory;

impl CpuRenderTargetFactory {
    pub fn create_cpu_target(&self, viewport: Viewport) -> cpu::CpuRenderTarget {
        cpu::CpuRenderTarget::new(viewport)
    }
}

impl RenderTargetFactory for CpuRenderTargetFactory {
    fn create(&self, viewport: Viewport) -> Result<Box<dyn RenderTarget>, RenderError> {
        Ok(Box::new(self.create_cpu_target(viewport)))
    }
}

#[derive(Debug, Default)]
pub struct FrameBuilder {
    next_frame_id: u64,
}

impl FrameBuilder {
    pub const fn new() -> Self {
        Self { next_frame_id: 0 }
    }

    pub fn build(
        &mut self,
        viewport: Viewport,
        tiles: Vec<TileDraw>,
        overlays: Vec<OverlayPrimitive>,
    ) -> RenderFrame {
        let frame = RenderFrame {
            frame_id: self.next_frame_id,
            viewport,
            tiles,
            overlays,
        };
        self.next_frame_id = self.next_frame_id.wrapping_add(1);
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FontAtlas, FrameBuilder, FrameOutcome, GlyphId, GlyphMetrics, GlyphPlacement, ImageId,
        ImageRevision, ImageUpdate, PreparedFrame, Rect, RenderCapabilities, RenderError,
        RenderFrame, RenderTarget, RenderTargetFactory, RenderTargetPipeline, RenderTargetSession,
        SurfaceFailure, TextRun, TileDraw, UvRect, Viewport,
    };
    use crate::geometry::ScreenPoint;

    #[derive(Default)]
    struct RecordingTarget {
        resized: Vec<Viewport>,
        updated_images: usize,
        rendered_frames: Vec<u64>,
        evicted_images: usize,
        recovered: Vec<SurfaceFailure>,
    }

    impl RenderTarget for RecordingTarget {
        fn capabilities(&self) -> RenderCapabilities {
            RenderCapabilities::default()
        }

        fn resize(&mut self, viewport: Viewport) -> Result<(), RenderError> {
            self.resized.push(viewport);
            Ok(())
        }

        fn update_images(&mut self, updates: &[ImageUpdate]) -> Result<(), RenderError> {
            self.updated_images += updates.len();
            Ok(())
        }

        fn render(&mut self, frame: &RenderFrame) -> Result<FrameOutcome, RenderError> {
            self.rendered_frames.push(frame.frame_id());
            Ok(FrameOutcome::submitted())
        }

        fn evict_images(&mut self, images: &[ImageId]) {
            self.evicted_images += images.len();
        }

        fn recover(&mut self, reason: SurfaceFailure) -> Result<(), RenderError> {
            self.recovered.push(reason);
            Ok(())
        }
    }

    struct RecordingFactory {
        target: std::sync::Arc<std::sync::Mutex<RecordingTarget>>,
    }

    impl RenderTargetFactory for RecordingFactory {
        fn create(&self, _viewport: Viewport) -> Result<Box<dyn RenderTarget>, RenderError> {
            Ok(Box::new(SharedRecordingTarget {
                target: std::sync::Arc::clone(&self.target),
            }))
        }
    }

    struct SharedRecordingTarget {
        target: std::sync::Arc<std::sync::Mutex<RecordingTarget>>,
    }

    impl RenderTarget for SharedRecordingTarget {
        fn capabilities(&self) -> RenderCapabilities {
            RenderCapabilities::default()
        }

        fn resize(&mut self, viewport: Viewport) -> Result<(), RenderError> {
            self.target.lock().unwrap().resized.push(viewport);
            Ok(())
        }

        fn update_images(&mut self, updates: &[ImageUpdate]) -> Result<(), RenderError> {
            self.target.lock().unwrap().updated_images += updates.len();
            Ok(())
        }

        fn render(&mut self, frame: &RenderFrame) -> Result<FrameOutcome, RenderError> {
            self.target
                .lock()
                .unwrap()
                .rendered_frames
                .push(frame.frame_id());
            Ok(FrameOutcome::submitted())
        }

        fn evict_images(&mut self, images: &[ImageId]) {
            self.target.lock().unwrap().evicted_images += images.len();
        }

        fn recover(&mut self, reason: SurfaceFailure) -> Result<(), RenderError> {
            self.target.lock().unwrap().recovered.push(reason);
            Ok(())
        }
    }

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

    #[test]
    fn prepared_frame_keeps_logical_frame_separate_from_image_updates() {
        let frame = RenderFrame::new(4, Viewport::new(2, 1)).with_tile(
            TileDraw::new(ImageId::new(7), ImageRevision::new(11), 0)
                .with_destination(Rect::new(1, 0, 1, 1)),
        );
        let update = ImageUpdate::new(
            ImageId::new(7),
            ImageRevision::new(11),
            1,
            1,
            vec![10, 20, 30, 255],
        )
        .unwrap();

        let prepared = PreparedFrame::new(frame.clone(), vec![update.clone()]);

        assert_eq!(prepared.frame(), &frame);
        assert_eq!(prepared.image_updates(), &[update]);
    }

    #[test]
    fn render_target_contract_keeps_lifecycle_independent_of_backend() {
        let mut target = RecordingTarget::default();
        target.resize(Viewport::new(640, 480)).unwrap();
        target.update_images(&[]).unwrap();
        target
            .render(&RenderFrame::new(11, Viewport::new(640, 480)))
            .unwrap();
        target.evict_images(&[ImageId::new(3)]);
        target.recover(SurfaceFailure::Lost).unwrap();

        assert_eq!(target.resized, vec![Viewport::new(640, 480)]);
        assert_eq!(target.updated_images, 0);
        assert_eq!(target.rendered_frames, vec![11]);
        assert_eq!(target.evicted_images, 1);
        assert_eq!(target.recovered, vec![SurfaceFailure::Lost]);
    }

    #[test]
    fn render_target_session_resizes_only_when_viewport_changes() {
        let target = RecordingTarget::default();
        let mut session = RenderTargetSession::new(target, Viewport::new(320, 200));
        let frame = RenderFrame::new(3, Viewport::new(640, 400));

        session
            .submit(Viewport::new(320, 200), &[], &frame)
            .expect("first frame should render");
        session
            .submit(Viewport::new(640, 400), &[], &frame)
            .expect("resized frame should render");

        assert_eq!(session.target().resized, vec![Viewport::new(640, 400)]);
        assert_eq!(session.target().rendered_frames, vec![3, 3]);
    }

    #[test]
    fn render_target_pipeline_submits_prepared_frame_lifecycle_in_order() {
        let target = std::sync::Arc::new(std::sync::Mutex::new(RecordingTarget::default()));
        let factory = RecordingFactory {
            target: std::sync::Arc::clone(&target),
        };
        let mut pipeline = RenderTargetPipeline::create(&factory, Viewport::new(320, 200)).unwrap();
        let update = ImageUpdate::new(
            ImageId::new(9),
            ImageRevision::new(1),
            1,
            1,
            vec![1, 2, 3, 255],
        )
        .unwrap();
        let prepared =
            PreparedFrame::new(RenderFrame::new(12, Viewport::new(640, 400)), vec![update]);

        assert!(pipeline.submit(&prepared).unwrap().was_submitted());
        pipeline.evict_images(&[ImageId::new(9)]);
        pipeline.recover(SurfaceFailure::Lost).unwrap();

        let target = target.lock().unwrap();
        assert_eq!(target.resized, vec![Viewport::new(640, 400)]);
        assert_eq!(target.updated_images, 1);
        assert_eq!(target.rendered_frames, vec![12]);
        assert_eq!(target.evicted_images, 1);
        assert_eq!(target.recovered, vec![SurfaceFailure::Lost]);
    }

    #[test]
    fn cpu_factory_creates_the_common_render_target_contract() {
        let factory = super::CpuRenderTargetFactory;
        let mut target = factory
            .create(Viewport::new(320, 200))
            .expect("CPU target should be available");

        assert_eq!(target.capabilities().persistent_resources, true);
        target
            .render(&RenderFrame::new(0, Viewport::new(320, 200)))
            .expect("empty frame should render");
    }

    #[test]
    fn frame_builder_assigns_monotonic_ids_to_immutable_snapshots() {
        let mut builder = FrameBuilder::new();
        let first = builder.build(Viewport::new(320, 200), Vec::new(), Vec::new());
        let second = builder.build(
            Viewport::new(640, 400),
            vec![TileDraw::new(ImageId::new(1), ImageRevision::new(2), 0)],
            Vec::new(),
        );

        assert_eq!(first.frame_id(), 0);
        assert_eq!(second.frame_id(), 1);
        assert_eq!(first.viewport(), Viewport::new(320, 200));
        assert!(first.tiles().is_empty());
        assert_eq!(second.tiles().len(), 1);
    }

    #[test]
    fn text_run_keeps_backend_independent_layout_configuration() {
        let run = TextRun::new("FPS: 60", ScreenPoint::new(4, 8))
            .with_color([10, 20, 30, 255])
            .with_scale(2)
            .with_layer(7);

        assert_eq!(run.text(), "FPS: 60");
        assert_eq!(run.origin(), ScreenPoint::new(4, 8));
        assert_eq!(run.color(), [10, 20, 30, 255]);
        assert_eq!(run.scale(), 2);
        assert_eq!(run.layer(), 7);
    }

    #[test]
    fn glyph_contract_keeps_metrics_and_placement_separate() {
        let source = UvRect {
            left: 0.1,
            top: 0.2,
            right: 0.3,
            bottom: 0.4,
        };
        let metrics = GlyphMetrics::new(source, (5, 7), (1, -2), 6);
        let placement = GlyphPlacement::new(
            GlyphId::new(12),
            Rect::new(4, 8, 5, 7),
            source,
            [255, 255, 255, 255],
            7,
        );

        assert_eq!(GlyphId::new(12).value(), 12);
        assert_eq!(metrics.source(), source);
        assert_eq!(metrics.size(), (5, 7));
        assert_eq!(metrics.bearing(), (1, -2));
        assert_eq!(metrics.advance(), 6);
        assert_eq!(placement.glyph(), GlyphId::new(12));
        assert_eq!(placement.destination(), Rect::new(4, 8, 5, 7));
        assert_eq!(placement.source(), source);
        assert_eq!(placement.color(), [255, 255, 255, 255]);
        assert_eq!(placement.layer(), 7);
    }

    #[test]
    fn font_atlas_lays_out_bitmap_glyphs_and_exposes_metrics() {
        let atlas = FontAtlas::from_bitmap_glyphs(&[
            ('A', [0b01110, 0b10001, 0b11111, 0b10001, 0b10001, 0, 0]),
            ('B', [0b11110, 0b10001, 0b11110, 0b10001, 0b11110, 0, 0]),
        ]);

        assert_eq!(atlas.dimensions(), (12, 7));
        assert_eq!(atlas.pixels().len(), 12 * 7);

        let (a_id, a_metrics) = atlas.glyph('A').expect("A must be in the atlas");
        let (b_id, b_metrics) = atlas.glyph('B').expect("B must be in the atlas");
        assert_eq!(a_id, GlyphId::new(0));
        assert_eq!(b_id, GlyphId::new(1));
        assert_eq!(a_metrics.size(), (5, 7));
        assert_eq!(a_metrics.advance(), 6);
        assert_eq!(a_metrics.source().left, 0.0);
        assert_eq!(a_metrics.source().right, 5.0 / 12.0);
        assert_eq!(b_metrics.source().left, 6.0 / 12.0);
        assert_eq!(atlas.pixels()[1], 255);
        assert_eq!(atlas.pixels()[5], 0);
        assert!(atlas.glyph('?').is_none());
    }

    #[test]
    fn debug_font_atlas_contains_overlay_characters() {
        let atlas = FontAtlas::debug();

        assert!(atlas.glyph('0').is_some());
        assert!(atlas.glyph(':').is_some());
        assert!(atlas.glyph(' ').is_some());
        assert!(atlas.glyph('A').is_none());
    }

    #[test]
    fn text_layout_emits_scaled_placements_and_skips_unknown_pixels() {
        let atlas = FontAtlas::from_bitmap_glyphs(&[
            ('A', [0b01110, 0b10001, 0b11111, 0b10001, 0b10001, 0, 0]),
            ('B', [0b11110, 0b10001, 0b11110, 0b10001, 0b11110, 0, 0]),
        ]);
        let run = TextRun::new("A?B\nA", ScreenPoint::new(10, 20))
            .with_color([10, 20, 30, 255])
            .with_scale(2)
            .with_layer(9);

        let placements = atlas.layout(&run);

        assert_eq!(placements.len(), 3);
        assert_eq!(placements[0].glyph(), GlyphId::new(0));
        assert_eq!(placements[0].destination(), Rect::new(10, 20, 10, 14));
        assert_eq!(placements[1].glyph(), GlyphId::new(1));
        assert_eq!(placements[1].destination(), Rect::new(34, 20, 10, 14));
        assert_eq!(placements[2].glyph(), GlyphId::new(0));
        assert_eq!(placements[2].destination(), Rect::new(10, 34, 10, 14));
        assert!(
            placements
                .iter()
                .all(|placement| placement.color() == [10, 20, 30, 255] && placement.layer() == 9)
        );
    }
}
