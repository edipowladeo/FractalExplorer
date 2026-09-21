//! Primitives shared by the first Windows sprite renderer.

pub mod app;
pub mod calculator;
pub mod config;
pub mod config_ui;
pub mod fixed;
pub mod geometry;
pub mod gpu;
pub mod gpu_window;
pub mod input;
pub mod orchestrator;
pub mod output;
// TODO(profiling): reativar quando src/profiling.rs e a feature correspondente
// forem adicionados pelo agente responsável.
// pub mod profiling;
pub mod precision;
pub mod render;
pub mod renderer;

pub use geometry::{Camera, CameraEnvelope};

pub use calculator::{Mandelbrot, MandelbrotFixed};
pub use fixed::Fixed;
pub use input::{InputEvent, InputState};
pub use orchestrator::{
    Orchestrator, Tile, TileLayer, TileSprite, TileStatus, TiledInfiniteCanvas,
};
pub use precision::{
    PrecisionDecisionManager, PrecisionRenderPlan, PrecisionSpec, PrecisionTechnique, RenderMethod,
};

/// A small, packed RGBA sprite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

// TODO(profiling): reativar junto com o módulo de profiling.
// #[cfg(test)]
// mod profiling_tests {
//     #[test]
//     fn profiling_reports_whether_the_feature_is_enabled() {
//         assert_eq!(crate::profiling::enabled(), cfg!(feature = "profiling"));
//     }
// }

impl Sprite {
    /// Creates a sprite from a row-major pixel buffer.
    pub fn from_pixels(width: usize, height: usize, pixels: Vec<u32>) -> Self {
        assert!(width > 0 && height > 0, "a sprite must have a size");
        assert_eq!(
            pixels.len(),
            width * height,
            "pixel buffer has the wrong size"
        );
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Creates a solid-color sprite.
    pub fn solid(width: usize, height: usize, color: u32) -> Self {
        Self::from_pixels(width, height, vec![color; width * height])
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Copies the sprite into a framebuffer, clipping pixels outside its bounds.
    pub fn draw_into(&self, framebuffer: &mut [u32], framebuffer_width: usize, x: isize, y: isize) {
        let framebuffer_height = framebuffer.len() / framebuffer_width;
        for sprite_y in 0..self.height {
            for sprite_x in 0..self.width {
                let target_x = x + sprite_x as isize;
                let target_y = y + sprite_y as isize;
                if target_x >= 0
                    && target_y >= 0
                    && (target_x as usize) < framebuffer_width
                    && (target_y as usize) < framebuffer_height
                {
                    let source = sprite_y * self.width + sprite_x;
                    let target = target_y as usize * framebuffer_width + target_x as usize;
                    framebuffer[target] = self.pixels[source];
                }
            }
        }
    }

    pub fn draw_into_scaled(
        &self,
        framebuffer: &mut [u32],
        framebuffer_width: usize,
        x: isize,
        y: isize,
        width: usize,
        height: usize,
    ) {
        let framebuffer_height = framebuffer.len() / framebuffer_width;
        for target_y in 0..height {
            for target_x in 0..width {
                let destination_x = x + target_x as isize;
                let destination_y = y + target_y as isize;
                if destination_x >= 0
                    && destination_y >= 0
                    && (destination_x as usize) < framebuffer_width
                    && (destination_y as usize) < framebuffer_height
                {
                    let source_x = target_x * self.width / width;
                    let source_y = target_y * self.height / height;
                    let source = source_y * self.width + source_x;
                    let destination =
                        destination_y as usize * framebuffer_width + destination_x as usize;
                    framebuffer[destination] = self.pixels[source];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sprite;

    #[test]
    fn creates_a_solid_sprite_with_expected_size_and_pixels() {
        let sprite = Sprite::solid(2, 3, 0x00ff00);

        assert_eq!(sprite.width(), 2);
        assert_eq!(sprite.height(), 3);
        assert_eq!(sprite.pixels(), &[0x00ff00; 6]);
    }

    #[test]
    fn draws_sprite_at_position_and_clips_to_framebuffer() {
        let color = 0xabcdef;
        let sprite = Sprite::solid(2, 2, color);
        let mut framebuffer = vec![0; 3 * 3];

        sprite.draw_into(&mut framebuffer, 3, -1, 1);

        assert_eq!(framebuffer, vec![0, 0, 0, color, 0, 0, color, 0, 0]);
    }
}
