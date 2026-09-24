pub mod cpu;
pub mod device;
pub mod gpu;
pub mod graphics;

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
        FrameBuilder, FrameOutcome, ImageId, ImageRevision, ImageUpdate, PreparedFrame, Rect,
        RenderCapabilities, RenderError, RenderFrame, RenderTarget, RenderTargetFactory,
        RenderTargetPipeline, RenderTargetSession, SurfaceFailure, TileDraw, Viewport,
    };

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
}
