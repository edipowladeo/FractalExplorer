//! Backend-independent GPU composition contracts.

use crate::geometry::ScreenPoint;
use crate::{Sprite, TileSprite};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

pub const TILE_VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<TileVertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            shader_location: 1,
        },
    ],
};

pub fn create_tile_quad(context: &GpuContext) -> wgpu::Buffer {
    let vertices = tile_quad_vertices();
    context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tile-quad"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        })
}

pub fn tile_quad_vertices() -> [TileVertex; 6] {
    [
        TileVertex {
            position: [-1.0, -1.0],
            uv: [0.0, 1.0],
        },
        TileVertex {
            position: [1.0, -1.0],
            uv: [1.0, 1.0],
        },
        TileVertex {
            position: [1.0, 1.0],
            uv: [1.0, 0.0],
        },
        TileVertex {
            position: [-1.0, -1.0],
            uv: [0.0, 1.0],
        },
        TileVertex {
            position: [1.0, 1.0],
            uv: [1.0, 0.0],
        },
        TileVertex {
            position: [-1.0, 1.0],
            uv: [0.0, 0.0],
        },
    ]
}

pub fn tile_vertices_for_screen(
    command: TileDrawCommand,
    screen_width: u32,
    screen_height: u32,
) -> [TileVertex; 6] {
    let x0 = command.position.x as f32 / screen_width as f32 * 2.0 - 1.0;
    let y0 = 1.0 - command.position.y as f32 / screen_height as f32 * 2.0;
    let x1 = (command.position.x as f32 + command.size.0 as f32) / screen_width as f32 * 2.0 - 1.0;
    let y1 = 1.0 - (command.position.y as f32 + command.size.1 as f32) / screen_height as f32 * 2.0;
    [
        TileVertex {
            position: [x0, y1],
            uv: [0.0, 1.0],
        },
        TileVertex {
            position: [x1, y1],
            uv: [1.0, 1.0],
        },
        TileVertex {
            position: [x1, y0],
            uv: [1.0, 0.0],
        },
        TileVertex {
            position: [x0, y1],
            uv: [0.0, 1.0],
        },
        TileVertex {
            position: [x1, y0],
            uv: [1.0, 0.0],
        },
        TileVertex {
            position: [x0, y0],
            uv: [0.0, 0.0],
        },
    ]
}

pub fn tile_vertices_for_commands(
    commands: &[TileDrawCommand],
    screen_width: u32,
    screen_height: u32,
) -> Vec<TileVertex> {
    commands
        .iter()
        .flat_map(|command| tile_vertices_for_screen(*command, screen_width, screen_height))
        .collect()
}

pub fn debug_overlay_upload(text: &str, width: u32, height: u32) -> TextureUpload {
    let mut rgba8 = vec![0; width as usize * height as usize * 4];
    for (character_index, character) in text.chars().enumerate() {
        let Some(glyph) = crate::renderer::glyph(character) else {
            continue;
        };
        let origin_x = character_index * 6 + 2;
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let x = origin_x + column;
                let y = row + 2;
                if x >= width as usize || y >= height as usize {
                    continue;
                }
                let offset = (y * width as usize + x) * 4;
                rgba8[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    TextureUpload {
        key: TextureKey {
            tile: usize::MAX,
            content_hash: hash_pixels(
                &rgba8
                    .chunks_exact(4)
                    .map(|pixel| u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]))
                    .collect::<Vec<_>>(),
            ),
        },
        width,
        height,
        rgba8,
    }
}

/// Owns the device and queue used by the future window-backed renderer.
/// Surface creation stays outside this context because it borrows a window.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    pub async fn initialize() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|error| format!("GPU adapter unavailable: {error}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|error| format!("GPU device unavailable: {error}"))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    pub fn create_surface<'window>(
        &self,
        window: &'window winit::window::Window,
    ) -> Result<GpuSurface<'window>, String> {
        let surface = self
            .instance
            .create_surface(window)
            .map_err(|error| format!("GPU surface unavailable: {error}"))?;
        let capabilities = surface.get_capabilities(&self.adapter);
        let format = capabilities
            .formats
            .first()
            .copied()
            .ok_or_else(|| "GPU surface has no supported formats".to_string())?;
        Ok(GpuSurface {
            surface,
            format,
            present_mode: capabilities
                .present_modes
                .first()
                .copied()
                .unwrap_or(wgpu::PresentMode::Fifo),
        })
    }
}

pub struct GpuSurface<'window> {
    pub surface: wgpu::Surface<'window>,
    pub format: wgpu::TextureFormat,
    pub present_mode: wgpu::PresentMode,
}

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

impl TextureUpload {
    pub fn is_well_formed(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.rgba8.len() == self.width as usize * self.height as usize * 4
    }
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

pub fn texture_keys_for_commands(commands: &[TileDrawCommand]) -> Vec<TextureKey> {
    let mut keys = Vec::new();
    for command in commands {
        if !keys.contains(&command.texture) {
            keys.push(command.texture);
        }
    }
    keys
}

#[derive(Debug, Default)]
pub struct PreparedTileBatch {
    pub uploads: Vec<TextureUpload>,
    pub commands: Vec<TileDrawCommand>,
}

pub fn prepare_tile_batch(
    cache: &mut TextureCache,
    tiles: &[(&TileSprite, Arc<Sprite>)],
) -> PreparedTileBatch {
    let mut uploads = Vec::new();
    let mut keyed_tiles = Vec::with_capacity(tiles.len());
    for (tile, sprite) in tiles {
        let tile_id = Arc::as_ptr(tile.tile()) as usize;
        let content_hash = hash_pixels(sprite.pixels());
        let key = TextureKey {
            tile: tile_id,
            content_hash,
        };
        if let Some(upload) = cache.prepare(tile, Arc::clone(sprite)) {
            uploads.push(upload);
        }
        keyed_tiles.push((*tile, key));
    }
    cache.retain_only(keyed_tiles.iter().map(|(_, key)| key.tile));
    PreparedTileBatch {
        uploads,
        commands: build_draw_commands(&keyed_tiles),
    }
}

pub const TILE_VERTEX_SHADER: &str = include_str!("../shaders/tile.vert.wgsl");
pub const TILE_FRAGMENT_SHADER: &str = include_str!("../shaders/tile.frag.wgsl");

pub fn create_tile_pipeline(
    context: &GpuContext,
    format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let layout = context
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tile-texture-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
    let pipeline_layout = context
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tile-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
    let vertex = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tile-vertex-shader"),
            source: wgpu::ShaderSource::Wgsl(TILE_VERTEX_SHADER.into()),
        });
    let fragment = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tile-fragment-shader"),
            source: wgpu::ShaderSource::Wgsl(TILE_FRAGMENT_SHADER.into()),
        });
    let pipeline = context
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tile-render-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(TILE_VERTEX_LAYOUT)],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &fragment,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
    (pipeline, layout)
}

pub struct GpuTileTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
}

pub struct GpuTextureStore {
    textures: HashMap<TextureKey, GpuTileTexture>,
}

impl GpuTextureStore {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    pub fn upload(
        &mut self,
        context: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        upload: TextureUpload,
    ) {
        let texture = upload_tile_texture(context, layout, &upload);
        self.textures.insert(upload.key, texture);
    }

    pub fn get(&self, key: &TextureKey) -> Option<&GpuTileTexture> {
        self.textures.get(key)
    }

    pub fn retain_only(&mut self, keys: impl IntoIterator<Item = TextureKey>) {
        let keys: std::collections::HashSet<_> = keys.into_iter().collect();
        self.textures.retain(|key, _| keys.contains(key));
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }
}

pub fn upload_tile_texture(
    context: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    upload: &TextureUpload,
) -> GpuTileTexture {
    let texture = context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tile-texture"),
        size: wgpu::Extent3d {
            width: upload.width,
            height: upload.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    context.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &upload.rgba8,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * upload.width),
            rows_per_image: Some(upload.height),
        },
        wgpu::Extent3d {
            width: upload.width,
            height: upload.height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("tile-sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tile-bind-group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
    GpuTileTexture {
        texture,
        view,
        sampler,
        bind_group,
    }
}

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

    #[test]
    fn tile_quad_covers_normalized_clip_space_with_consistent_uvs() {
        let vertices = tile_quad_vertices();
        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0].position, [-1.0, -1.0]);
        assert_eq!(vertices[2].position, [1.0, 1.0]);
        assert_eq!(vertices[0].uv, [0.0, 1.0]);
        assert_eq!(vertices[2].uv, [1.0, 0.0]);
    }

    #[test]
    fn tile_vertex_layout_matches_shader_locations_and_stride() {
        assert_eq!(TILE_VERTEX_LAYOUT.array_stride, 16);
        assert_eq!(TILE_VERTEX_LAYOUT.attributes.len(), 2);
        assert_eq!(TILE_VERTEX_LAYOUT.attributes[0].shader_location, 0);
        assert_eq!(TILE_VERTEX_LAYOUT.attributes[1].shader_location, 1);
        assert_eq!(
            TILE_VERTEX_LAYOUT.attributes[0].format,
            wgpu::VertexFormat::Float32x2
        );
        assert_eq!(
            TILE_VERTEX_LAYOUT.attributes[1].format,
            wgpu::VertexFormat::Float32x2
        );
    }

    #[test]
    fn texture_upload_preserves_dimensions_and_rgba_payload_size() {
        let upload = TextureUpload {
            key: TextureKey {
                tile: 7,
                content_hash: 11,
            },
            width: 3,
            height: 2,
            rgba8: vec![0; 3 * 2 * 4],
        };
        assert_eq!(
            upload.rgba8.len(),
            upload.width as usize * upload.height as usize * 4
        );
    }

    #[test]
    fn visible_batch_keys_are_unique_and_follow_draw_order() {
        let first = TextureKey {
            tile: 1,
            content_hash: 10,
        };
        let second = TextureKey {
            tile: 2,
            content_hash: 20,
        };
        let commands = vec![
            TileDrawCommand {
                texture: first,
                position: ScreenPoint::new(0, 0),
                size: (1, 1),
            },
            TileDrawCommand {
                texture: first,
                position: ScreenPoint::new(1, 0),
                size: (1, 1),
            },
            TileDrawCommand {
                texture: second,
                position: ScreenPoint::new(2, 0),
                size: (1, 1),
            },
        ];

        assert_eq!(texture_keys_for_commands(&commands), vec![first, second]);
    }

    #[test]
    fn tile_vertices_for_commands_concatenate_six_vertices_per_tile() {
        let command = TileDrawCommand {
            texture: TextureKey {
                tile: 1,
                content_hash: 2,
            },
            position: ScreenPoint::new(0, 0),
            size: (10, 10),
        };
        assert_eq!(
            tile_vertices_for_commands(&[command, command], 100, 100).len(),
            12
        );
    }

    #[test]
    fn debug_overlay_upload_contains_transparent_background_and_text_pixels() {
        let upload = debug_overlay_upload("C1", 64, 16);
        assert_eq!((upload.width, upload.height), (64, 16));
        assert_eq!(upload.rgba8.len(), 64 * 16 * 4);
        assert!(upload.rgba8.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(upload.rgba8.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }

    #[test]
    fn compares_gpu_visible_tiles_with_cpu_reference_count() {
        let metrics = crate::gpu_window::GpuFrameMetrics::from_batch(
            &PreparedTileBatch {
                uploads: Vec::new(),
                commands: vec![TileDrawCommand {
                    texture: TextureKey {
                        tile: 1,
                        content_hash: 2,
                    },
                    position: ScreenPoint::new(0, 0),
                    size: (1, 1),
                }],
            },
            std::time::Duration::ZERO,
        );
        assert!(metrics.matches_cpu_visible_tiles(1));
        assert!(!metrics.matches_cpu_visible_tiles(2));
    }

    #[test]
    fn tile_batch_uploads_new_content_once_and_keeps_draw_order() {
        let first_tile = Arc::new(Tile::new(ComplexPoint::new(0.0, 0.0), 2, 2, 1.0));
        let second_tile = Arc::new(Tile::new(ComplexPoint::new(1.0, 0.0), 2, 2, 1.0));
        let first = TileSprite::new(first_tile, ScreenPoint::new(3, 4), 2.0);
        let second = TileSprite::new(second_tile, ScreenPoint::new(9, 4), 1.0);
        let first_sprite = Arc::new(Sprite::solid(2, 2, 0x00ff00));
        let second_sprite = Arc::new(Sprite::solid(2, 2, 0xff0000));
        let mut cache = TextureCache::default();

        let first_batch = prepare_tile_batch(
            &mut cache,
            &[
                (&first, Arc::clone(&first_sprite)),
                (&second, Arc::clone(&second_sprite)),
            ],
        );
        let second_batch = prepare_tile_batch(
            &mut cache,
            &[(&first, first_sprite), (&second, second_sprite)],
        );

        assert_eq!(first_batch.uploads.len(), 2);
        assert_eq!(first_batch.commands.len(), 2);
        assert_eq!(first_batch.commands[0].position, ScreenPoint::new(3, 4));
        assert_eq!(first_batch.commands[1].position, ScreenPoint::new(9, 4));
        assert!(second_batch.uploads.is_empty());
        assert_eq!(second_batch.commands.len(), 2);
    }

    #[test]
    fn tile_batch_uploads_are_well_formed_and_match_commands() {
        let tile = Arc::new(Tile::new(ComplexPoint::new(0.0, 0.0), 1, 1, 1.0));
        let tile_sprite = TileSprite::new(tile, ScreenPoint::new(0, 0), 1.0);
        let sprite = Arc::new(Sprite::solid(1, 1, 0x123456));
        let mut cache = TextureCache::default();
        let batch = prepare_tile_batch(&mut cache, &[(&tile_sprite, sprite)]);

        assert!(batch.uploads.iter().all(TextureUpload::is_well_formed));
        assert_eq!(batch.uploads[0].key, batch.commands[0].texture);
    }
}
