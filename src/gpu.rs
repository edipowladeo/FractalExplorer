//! Backend-independent GPU composition contracts.

use crate::geometry::ScreenPoint;
use crate::{Sprite, TileSprite};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureKey {
    pub tile: usize,
    pub content_hash: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureUpload {
    pub key: TextureKey,
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileDrawCommand {
    pub texture: TextureKey,
    pub position: ScreenPoint,
    pub size: (u32, u32),
}

#[derive(Debug, Default)]
pub struct TextureCache {
    entries: HashMap<usize, TextureKey>,
}

impl TextureCache {
    pub fn prepare(
        &mut self,
        tile_sprite: &TileSprite,
        sprite: Arc<Sprite>,
    ) -> Option<TextureUpload> {
        let tile_id = Arc::as_ptr(tile_sprite.tile()) as usize;
        let key = TextureKey {
            tile: tile_id,
            content_hash: hash_pixels(sprite.pixels()),
        };
        if self.entries.get(&tile_id) == Some(&key) {
            return None;
        }
        self.entries.insert(tile_id, key);
        Some(TextureUpload {
            key,
            width: sprite.width() as u32,
            height: sprite.height() as u32,
            rgba8: to_rgba8(sprite.pixels()),
        })
    }
    pub fn retain_only(&mut self, tile_ids: impl IntoIterator<Item = usize>) {
        let retained: std::collections::HashSet<_> = tile_ids.into_iter().collect();
        self.entries.retain(|id, _| retained.contains(id));
    }
}

pub fn build_draw_commands(tile_sprites: &[(&TileSprite, TextureKey)]) -> Vec<TileDrawCommand> {
    tile_sprites
        .iter()
        .map(|(tile, texture)| TileDrawCommand {
            texture: *texture,
            position: tile.position(),
            size: tile.screen_size(),
        })
        .collect()
}

pub const TILE_VERTEX_SHADER: &str = include_str!("../shaders/tile.vert.wgsl");
pub const TILE_FRAGMENT_SHADER: &str = include_str!("../shaders/tile.frag.wgsl");

fn hash_pixels(pixels: &[u32]) -> u64 {
    pixels.iter().fold(0xcbf29ce484222325, |h, p| {
        (h ^ u64::from(*p)).wrapping_mul(0x100000001b3)
    })
}
fn to_rgba8(pixels: &[u32]) -> Vec<u8> {
    pixels
        .iter()
        .flat_map(|p| {
            let [r, g, b, a] = p.to_le_bytes();
            [r, g, b, if a == 0 { 255 } else { a }]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::ComplexPoint;
    use crate::Tile;

    #[test]
    fn uploads_a_new_sprite_only_once_and_emits_rgba_pixels() {
        let tile = Arc::new(Tile::new(ComplexPoint::new(0.0, 0.0), 2, 1, 1.0));
        let tile_sprite = TileSprite::new(tile, ScreenPoint::new(4, 8), 2.0);
        let sprite = Arc::new(Sprite::from_pixels(2, 1, vec![0x00112233, 0x00445566]));
        let mut cache = TextureCache::default();
        let upload = cache.prepare(&tile_sprite, Arc::clone(&sprite)).unwrap();
        assert_eq!(
            upload.rgba8,
            vec![0x33, 0x22, 0x11, 255, 0x66, 0x55, 0x44, 255]
        );
        assert!(cache.prepare(&tile_sprite, sprite).is_none());
    }

    #[test]
    fn draw_commands_preserve_order_and_gpu_scale() {
        let tile = Arc::new(Tile::new(ComplexPoint::new(0.0, 0.0), 4, 3, 1.0));
        let first = TileSprite::new(Arc::clone(&tile), ScreenPoint::new(1, 2), 2.0);
        let second = TileSprite::new(tile, ScreenPoint::new(7, 9), 0.5);
        let commands = build_draw_commands(&[
            (
                &first,
                TextureKey {
                    tile: 1,
                    content_hash: 2,
                },
            ),
            (
                &second,
                TextureKey {
                    tile: 3,
                    content_hash: 4,
                },
            ),
        ]);
        assert_eq!(commands[0].size, (8, 6));
        assert_eq!(commands[1].size, (2, 2));
    }

    #[test]
    fn shaders_define_a_textured_tile_pipeline() {
        assert!(TILE_VERTEX_SHADER.contains("@vertex"));
        assert!(TILE_FRAGMENT_SHADER.contains("textureSample"));
    }
}
