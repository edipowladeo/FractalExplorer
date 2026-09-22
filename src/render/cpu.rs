use std::collections::HashMap;

use super::{
    FrameOutcome, ImageId, ImageRevision, ImageUpdate, OverlayPrimitive, RenderCapabilities,
    RenderError, RenderFrame, RenderTarget, SurfaceFailure, TileDraw, Viewport,
};

#[derive(Debug, Clone)]
struct StoredImage {
    revision: ImageRevision,
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

pub struct CpuRenderTarget {
    viewport: Viewport,
    framebuffer: Vec<u8>,
    images: HashMap<ImageId, StoredImage>,
}

impl CpuRenderTarget {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            framebuffer: vec![0; framebuffer_len(viewport)],
            viewport,
            images: HashMap::new(),
        }
    }

    pub fn pixel_rgba8(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.viewport.width() || y >= self.viewport.height() {
            return None;
        }
        let offset = ((y * self.viewport.width() + x) * 4) as usize;
        self.framebuffer
            .get(offset..offset + 4)
            .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
    }

    fn clear(&mut self) {
        self.framebuffer.fill(0);
    }

    fn draw_image(
        viewport: Viewport,
        framebuffer: &mut [u8],
        image: &StoredImage,
        tile: &TileDraw,
    ) {
        let destination = tile.destination();
        if destination.width == 0 || destination.height == 0 {
            return;
        }
        for target_y in 0..destination.height {
            for target_x in 0..destination.width {
                let screen_x = destination.x + target_x as i32;
                let screen_y = destination.y + target_y as i32;
                if screen_x < 0
                    || screen_y < 0
                    || screen_x as u32 >= viewport.width()
                    || screen_y as u32 >= viewport.height()
                {
                    continue;
                }
                let source_x = target_x * image.width / destination.width;
                let source_y = target_y * image.height / destination.height;
                let source = ((source_y * image.width + source_x) * 4) as usize;
                let target = ((screen_y as u32 * viewport.width() + screen_x as u32) * 4) as usize;
                framebuffer[target..target + 4].copy_from_slice(&image.rgba8[source..source + 4]);
            }
        }
    }
}

impl RenderTarget for CpuRenderTarget {
    fn capabilities(&self) -> RenderCapabilities {
        RenderCapabilities {
            partial_image_updates: true,
            persistent_resources: true,
        }
    }

    fn resize(&mut self, viewport: Viewport) -> Result<(), RenderError> {
        self.viewport = viewport;
        self.framebuffer = vec![0; framebuffer_len(viewport)];
        Ok(())
    }

    fn update_images(&mut self, updates: &[ImageUpdate]) -> Result<(), RenderError> {
        for update in updates {
            let (width, height) = update.dimensions();
            self.images.insert(
                update.image(),
                StoredImage {
                    revision: update.revision(),
                    width,
                    height,
                    rgba8: update.rgba8().to_vec(),
                },
            );
        }
        Ok(())
    }

    fn render(&mut self, frame: &RenderFrame) -> Result<FrameOutcome, RenderError> {
        if frame.viewport() != self.viewport {
            return Err(RenderError::InvalidFrame(
                "frame viewport differs from target viewport",
            ));
        }
        self.clear();
        for tile in frame.tiles() {
            let Some(image) = self.images.get(&tile.image()) else {
                continue;
            };
            if image.revision != tile.revision() {
                continue;
            }
            Self::draw_image(self.viewport, &mut self.framebuffer, image, tile);
        }
        for overlay in frame.overlays() {
            let OverlayPrimitive::Image(tile) = overlay;
            let Some(image) = self.images.get(&tile.image()) else {
                continue;
            };
            if image.revision != tile.revision() {
                continue;
            }
            Self::draw_image(self.viewport, &mut self.framebuffer, image, tile);
        }
        Ok(FrameOutcome::submitted())
    }

    fn evict_images(&mut self, images: &[ImageId]) {
        for image in images {
            self.images.remove(image);
        }
    }

    fn recover(&mut self, _reason: SurfaceFailure) -> Result<(), RenderError> {
        self.clear();
        Ok(())
    }
}

fn framebuffer_len(viewport: Viewport) -> usize {
    (viewport.width() as usize)
        .saturating_mul(viewport.height() as usize)
        .saturating_mul(4)
}

#[cfg(test)]
mod tests {
    use crate::render::{
        ImageId, ImageRevision, ImageUpdate, OverlayPrimitive, Rect, RenderFrame, RenderTarget,
        TileDraw, Viewport,
    };

    use super::CpuRenderTarget;

    #[test]
    fn cpu_target_composes_an_updated_image_at_the_frame_destination() {
        let mut target = CpuRenderTarget::new(Viewport::new(2, 1));
        target
            .update_images(&[ImageUpdate::new(
                ImageId::new(1),
                ImageRevision::new(1),
                1,
                1,
                vec![10, 20, 30, 255],
            )
            .unwrap()])
            .unwrap();

        let frame = RenderFrame::new(0, Viewport::new(2, 1)).with_tile(
            TileDraw::new(ImageId::new(1), ImageRevision::new(1), 0)
                .with_destination(Rect::new(1, 0, 1, 1)),
        );
        target.render(&frame).unwrap();

        assert_eq!(target.pixel_rgba8(0, 0), Some([0, 0, 0, 0]));
        assert_eq!(target.pixel_rgba8(1, 0), Some([10, 20, 30, 255]));
    }

    #[test]
    fn cpu_target_composes_overlay_after_tiles() {
        let mut target = CpuRenderTarget::new(Viewport::new(1, 1));
        target
            .update_images(&[
                ImageUpdate::new(
                    ImageId::new(1),
                    ImageRevision::new(1),
                    1,
                    1,
                    vec![10, 20, 30, 255],
                )
                .unwrap(),
                ImageUpdate::new(
                    ImageId::new(2),
                    ImageRevision::new(1),
                    1,
                    1,
                    vec![200, 210, 220, 255],
                )
                .unwrap(),
            ])
            .unwrap();

        let tile = TileDraw::new(ImageId::new(1), ImageRevision::new(1), 0)
            .with_destination(Rect::new(0, 0, 1, 1));
        let overlay = TileDraw::new(ImageId::new(2), ImageRevision::new(1), 0)
            .with_destination(Rect::new(0, 0, 1, 1));
        let frame = RenderFrame::new(0, Viewport::new(1, 1))
            .with_tile(tile)
            .with_overlay(OverlayPrimitive::Image(overlay));

        target.render(&frame).unwrap();

        assert_eq!(target.pixel_rgba8(0, 0), Some([200, 210, 220, 255]));
    }
}
