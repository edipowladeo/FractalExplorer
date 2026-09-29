use std::collections::HashMap;

use super::{
    FontAtlas, FrameOutcome, GlyphPlacement, ImageId, ImageRevision, ImageUpdate, OverlayPrimitive,
    RenderCapabilities, RenderError, RenderFrame, RenderTarget, SurfaceFailure, TileDraw, Viewport,
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
    font_atlas: FontAtlas,
    preserve_previous_frame: bool,
    clear_color: [u8; 4],
}

impl CpuRenderTarget {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            framebuffer: vec![0; framebuffer_len(viewport)],
            viewport,
            images: HashMap::new(),
            font_atlas: FontAtlas::debug(),
            preserve_previous_frame: false,
            clear_color: [0, 0, 0, 0],
        }
    }

    pub fn set_preserve_previous_frame(&mut self, preserve: bool) {
        self.preserve_previous_frame = preserve;
    }

    pub fn framebuffer_rgba8(&self) -> &[u8] {
        &self.framebuffer
    }

    pub fn set_clear_color(&mut self, color: [u8; 4]) {
        self.clear_color = color;
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
        if !self.preserve_previous_frame {
            for pixel in self.framebuffer.chunks_exact_mut(4) {
                pixel.copy_from_slice(&self.clear_color);
            }
        }
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

    fn draw_text(
        viewport: Viewport,
        framebuffer: &mut [u8],
        atlas: &FontAtlas,
        placement: GlyphPlacement,
    ) {
        let destination = placement.destination();
        if destination.width == 0 || destination.height == 0 {
            return;
        }
        let source = placement.source();
        let (atlas_width, atlas_height) = atlas.dimensions();
        if atlas_width == 0 || atlas_height == 0 {
            return;
        }
        let source_left = (source.left * atlas_width as f32).floor() as u32;
        let source_top = (source.top * atlas_height as f32).floor() as u32;
        let source_right = (source.right * atlas_width as f32).ceil() as u32;
        let source_bottom = (source.bottom * atlas_height as f32).ceil() as u32;
        let source_width = source_right.saturating_sub(source_left).max(1);
        let source_height = source_bottom.saturating_sub(source_top).max(1);
        let color = placement.color();

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

                let source_x =
                    source_left + target_x.saturating_mul(source_width) / destination.width;
                let source_y =
                    source_top + target_y.saturating_mul(source_height) / destination.height;
                let atlas_index = (source_y.min(atlas_height - 1) * atlas_width
                    + source_x.min(atlas_width - 1)) as usize;
                let coverage = atlas.pixels()[atlas_index] as u16;
                let alpha = coverage * color[3] as u16 / 255;
                if alpha == 0 {
                    continue;
                }

                let target = ((screen_y as u32 * viewport.width() + screen_x as u32) * 4) as usize;
                Self::blend_pixel(&mut framebuffer[target..target + 4], color, alpha);
            }
        }
    }

    fn blend_pixel(destination: &mut [u8], color: [u8; 4], alpha: u16) {
        let inverse_alpha = 255 - alpha;
        for channel in 0..3 {
            destination[channel] = ((color[channel] as u16 * alpha
                + destination[channel] as u16 * inverse_alpha)
                / 255) as u8;
        }
        destination[3] = (alpha + destination[3] as u16 * inverse_alpha / 255) as u8;
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
            match overlay {
                OverlayPrimitive::Image(tile) => {
                    let Some(image) = self.images.get(&tile.image()) else {
                        continue;
                    };
                    if image.revision != tile.revision() {
                        continue;
                    }
                    Self::draw_image(self.viewport, &mut self.framebuffer, image, tile);
                }
                OverlayPrimitive::Text(run) => {
                    for placement in self.font_atlas.layout(run) {
                        Self::draw_text(
                            self.viewport,
                            &mut self.framebuffer,
                            &self.font_atlas,
                            placement,
                        );
                    }
                }
            }
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
        ScreenPoint, TextRun, TileDraw, Viewport,
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

    #[test]
    fn cpu_target_composes_text_overlay_from_the_font_atlas() {
        let mut target = CpuRenderTarget::new(Viewport::new(5, 7));
        let text = TextRun::new("C", ScreenPoint::new(0, 0)).with_color([200, 100, 50, 255]);
        let frame =
            RenderFrame::new(0, Viewport::new(5, 7)).with_overlay(OverlayPrimitive::Text(text));

        target.render(&frame).unwrap();

        assert_eq!(target.pixel_rgba8(1, 0), Some([200, 100, 50, 255]));
        assert_eq!(target.pixel_rgba8(0, 0), Some([0, 0, 0, 0]));
    }

    #[test]
    fn cpu_target_scales_and_blends_text_overlay_pixels() {
        let mut target = CpuRenderTarget::new(Viewport::new(10, 14));
        let text = TextRun::new("C", ScreenPoint::new(0, 0))
            .with_scale(2)
            .with_color([200, 100, 50, 128]);
        let frame =
            RenderFrame::new(0, Viewport::new(10, 14)).with_overlay(OverlayPrimitive::Text(text));

        target.render(&frame).unwrap();

        assert_eq!(target.pixel_rgba8(2, 0), Some([100, 50, 25, 128]));
        assert_eq!(target.pixel_rgba8(0, 0), Some([0, 0, 0, 0]));
    }

    #[test]
    fn cpu_target_can_preserve_previous_pixels_between_frames() {
        let mut target = CpuRenderTarget::new(Viewport::new(1, 1));
        target.set_preserve_previous_frame(true);
        let update = ImageUpdate::new(
            ImageId::new(1),
            ImageRevision::new(1),
            1,
            1,
            vec![10, 20, 30, 255],
        )
        .unwrap();
        let frame = RenderFrame::new(0, Viewport::new(1, 1)).with_tile(
            TileDraw::new(ImageId::new(1), ImageRevision::new(1), 0)
                .with_destination(Rect::new(0, 0, 1, 1)),
        );
        target.update_images(&[update]).unwrap();
        target.render(&frame).unwrap();
        target
            .render(&RenderFrame::new(1, Viewport::new(1, 1)))
            .unwrap();

        assert_eq!(target.pixel_rgba8(0, 0), Some([10, 20, 30, 255]));
    }
}
