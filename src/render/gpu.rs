use super::device::{CommandList, DeviceError, GraphicsDevice, TextureResourceCache};
use super::{
    FrameOutcome, ImageId, ImageUpdate, RenderCapabilities, RenderError, RenderFrame, RenderTarget,
    SurfaceFailure, Viewport,
};

/// Backend-independent GPU render target.
///
/// It owns resource identity and frame command generation. The concrete
/// graphics API remains behind `GraphicsDevice`.
pub struct GpuRenderTarget<D> {
    device: D,
    viewport: Viewport,
    textures: TextureResourceCache,
}

impl<D> GpuRenderTarget<D> {
    pub fn new(device: D, viewport: Viewport) -> Self {
        Self {
            device,
            viewport,
            textures: TextureResourceCache::new(),
        }
    }

    pub fn device(&self) -> &D {
        &self.device
    }

    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    pub fn textures(&self) -> &TextureResourceCache {
        &self.textures
    }
}

impl<D: GraphicsDevice> RenderTarget for GpuRenderTarget<D> {
    fn capabilities(&self) -> RenderCapabilities {
        RenderCapabilities {
            partial_image_updates: true,
            persistent_resources: true,
        }
    }

    fn resize(&mut self, viewport: Viewport) -> Result<(), RenderError> {
        self.device
            .resize(viewport)
            .map_err(|_| RenderError::BackendUnavailable("GPU resize failed"))?;
        self.viewport = viewport;
        Ok(())
    }

    fn update_images(&mut self, updates: &[ImageUpdate]) -> Result<(), RenderError> {
        self.textures
            .upload_updates(&mut self.device, updates)
            .map(|_| ())
            .map_err(|_| RenderError::BackendUnavailable("GPU image update failed"))
    }

    fn render(&mut self, frame: &RenderFrame) -> Result<FrameOutcome, RenderError> {
        crate::profile_scope!("gpu_render_target");
        if frame.viewport() != self.viewport {
            return Err(RenderError::InvalidFrame(
                "frame viewport differs from target viewport",
            ));
        }

        let mut commands = CommandList::default();
        let images: Vec<_> = frame
            .tiles()
            .iter()
            .map(|tile| tile.image())
            .chain(frame.overlays().iter().map(|overlay| {
                let super::OverlayPrimitive::Image(tile) = overlay;
                tile.image()
            }))
            .collect();
        for tile in frame
            .tiles()
            .iter()
            .chain(frame.overlays().iter().map(|overlay| {
                let super::OverlayPrimitive::Image(tile) = overlay;
                tile
            }))
        {
            let Some(texture) = self.textures.handle(tile.image()) else {
                return Err(RenderError::InvalidFrame(
                    "frame references an unknown image",
                ));
            };
            let destination = tile.destination();
            commands.draw_texture(
                texture,
                destination.x,
                destination.y,
                destination.width,
                destination.height,
                tile.opacity(),
            );
        }
        commands.present();
        self.device
            .submit(commands)
            .map_err(|_| RenderError::BackendUnavailable("GPU command submission failed"))?;
        self.textures.retain_only(&mut self.device, &images);
        Ok(FrameOutcome::submitted())
    }

    fn evict_images(&mut self, images: &[ImageId]) {
        self.textures.evict(&mut self.device, images);
    }

    fn recover(&mut self, _reason: SurfaceFailure) -> Result<(), RenderError> {
        Ok(())
    }
}

#[allow(dead_code)]
fn map_device_error(_error: DeviceError) -> RenderError {
    RenderError::BackendUnavailable("GPU operation failed")
}

#[cfg(test)]
mod tests {
    use super::GpuRenderTarget;
    use crate::render::device::{
        BufferDescriptor, BufferHandle, Command, CommandList, DeviceError, GraphicsDevice,
        TextureDescriptor, TextureHandle,
    };
    use crate::render::{
        ImageId, ImageRevision, ImageUpdate, Rect, RenderFrame, RenderTarget, TileDraw, Viewport,
    };

    #[derive(Default)]
    struct MockDevice {
        next: u64,
        submitted: Vec<CommandList>,
        resized: Vec<Viewport>,
    }

    impl GraphicsDevice for MockDevice {
        fn create_buffer(
            &mut self,
            _descriptor: BufferDescriptor,
        ) -> Result<BufferHandle, DeviceError> {
            self.next += 1;
            Ok(BufferHandle::new(self.next))
        }

        fn create_texture(
            &mut self,
            _descriptor: TextureDescriptor,
        ) -> Result<TextureHandle, DeviceError> {
            self.next += 1;
            Ok(TextureHandle::new(self.next))
        }

        fn submit(&mut self, commands: CommandList) -> Result<(), DeviceError> {
            self.submitted.push(commands);
            Ok(())
        }

        fn resize(&mut self, viewport: Viewport) -> Result<(), DeviceError> {
            self.resized.push(viewport);
            Ok(())
        }

        fn destroy_buffer(&mut self, _buffer: BufferHandle) {}
        fn destroy_texture(&mut self, _texture: TextureHandle) {}
    }

    #[test]
    fn gpu_target_uploads_each_revision_once_and_submits_draw_commands() {
        let viewport = Viewport::new(2, 1);
        let mut target = GpuRenderTarget::new(MockDevice::default(), viewport);
        let update = ImageUpdate::new(
            ImageId::new(1),
            ImageRevision::new(1),
            1,
            1,
            vec![10, 20, 30, 255],
        )
        .unwrap();
        target.update_images(&[update.clone()]).unwrap();
        target.update_images(&[update]).unwrap();

        let frame = RenderFrame::new(0, viewport).with_tile(
            TileDraw::new(ImageId::new(1), ImageRevision::new(1), 0)
                .with_destination(Rect::new(1, 0, 1, 1)),
        );
        target.render(&frame).unwrap();

        assert_eq!(target.device().submitted.len(), 2);
        assert!(matches!(
            target.device().submitted[1].commands()[0],
            Command::DrawTexture { x: 1, y: 0, .. }
        ));
        assert!(matches!(
            target.device().submitted[1].commands()[1],
            Command::Present
        ));
    }

    #[test]
    fn gpu_target_rejects_frames_with_unknown_images() {
        let viewport = Viewport::new(1, 1);
        let mut target = GpuRenderTarget::new(MockDevice::default(), viewport);
        let frame = RenderFrame::new(0, viewport).with_tile(TileDraw::new(
            ImageId::new(99),
            ImageRevision::new(1),
            0,
        ));

        assert!(target.render(&frame).is_err());
        assert!(target.device().submitted.is_empty());
    }

    #[test]
    fn gpu_target_evicts_images_after_they_leave_the_frame() {
        let viewport = Viewport::new(2, 1);
        let mut target = GpuRenderTarget::new(MockDevice::default(), viewport);
        let visible = ImageId::new(21);
        let stale = ImageId::new(22);
        let updates = [
            ImageUpdate::new(visible, ImageRevision::new(1), 1, 1, vec![1, 2, 3, 255]).unwrap(),
            ImageUpdate::new(stale, ImageRevision::new(1), 1, 1, vec![4, 5, 6, 255]).unwrap(),
        ];
        target.update_images(&updates).unwrap();
        let frame = RenderFrame::new(1, viewport).with_tile(TileDraw::new(
            visible,
            ImageRevision::new(1),
            0,
        ));

        target.render(&frame).unwrap();

        assert!(target.textures().handle(visible).is_some());
        assert!(target.textures().handle(stale).is_none());
    }

    #[test]
    fn gpu_target_forwards_resize_to_the_graphics_device() {
        let mut target = GpuRenderTarget::new(MockDevice::default(), Viewport::new(2, 1));
        let viewport = Viewport::new(640, 480);

        target.resize(viewport).unwrap();

        assert_eq!(target.device().resized, vec![viewport]);
    }

    #[test]
    fn moving_an_image_reuses_its_texture_and_only_changes_draw_coordinates() {
        let viewport = Viewport::new(8, 4);
        let mut target = GpuRenderTarget::new(MockDevice::default(), viewport);
        let update = ImageUpdate::new(
            ImageId::new(4),
            ImageRevision::new(1),
            2,
            1,
            vec![10, 20, 30, 255, 40, 50, 60, 255],
        )
        .unwrap();
        target.update_images(&[update.clone()]).unwrap();
        let first = RenderFrame::new(1, viewport).with_tile(
            TileDraw::new(ImageId::new(4), ImageRevision::new(1), 0)
                .with_destination(Rect::new(1, 1, 2, 1)),
        );
        target.render(&first).unwrap();

        target.update_images(&[update]).unwrap();
        let moved = RenderFrame::new(2, viewport).with_tile(
            TileDraw::new(ImageId::new(4), ImageRevision::new(1), 0)
                .with_destination(Rect::new(5, 2, 2, 1)),
        );
        target.render(&moved).unwrap();

        let submissions = &target.device().submitted;
        assert_eq!(submissions.len(), 3);
        let Command::WriteTexture { texture, .. } = submissions[0].commands()[0] else {
            panic!("first submission must upload the image")
        };
        assert!(submissions[1]
            .commands()
            .iter()
            .all(|command| !matches!(command, Command::WriteTexture { .. })));
        let Command::DrawTexture {
            texture: first_texture,
            x: 1,
            y: 1,
            ..
        } = submissions[1].commands()[0]
        else {
            panic!("first frame must draw at its initial destination")
        };
        let Command::DrawTexture {
            texture: moved_texture,
            x: 5,
            y: 2,
            ..
        } = submissions[2].commands()[0]
        else {
            panic!("moved frame must update only the draw destination")
        };
        assert_eq!(texture, first_texture);
        assert_eq!(texture, moved_texture);
    }
}
