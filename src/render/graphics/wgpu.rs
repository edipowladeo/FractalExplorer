//! `wgpu` implementation of the portable graphics-device resource contract.

use crate::config::GpuBackend;
use crate::gpu::{TextureKey, TextureUpload, TileDrawCommand};
use crate::render::device::{
    BufferDescriptor, BufferHandle, BufferUsage, Command, CommandList, DeviceError, GraphicsDevice,
    TextureDescriptor, TextureFormat, TextureHandle,
};
use crate::render::ImageUpdate;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

pub const TILE_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

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

pub const TILE_VERTEX_SHADER: &str = include_str!("../../../shaders/tile.vert.wgsl");
pub const TILE_FRAGMENT_SHADER: &str = include_str!("../../../shaders/tile.frag.wgsl");

pub fn create_tile_quad(context: &WgpuContext) -> wgpu::Buffer {
    context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tile-quad"),
            contents: bytemuck::cast_slice(&tile_quad_vertices()),
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

pub struct WgpuPipeline(pub wgpu::RenderPipeline);

pub struct WgpuSurface<'window> {
    pub surface: wgpu::Surface<'window>,
    pub format: wgpu::TextureFormat,
    pub present_mode: wgpu::PresentMode,
}

/// Owns the selected adapter, device, queue, and instance for the `wgpu` API.
pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl WgpuContext {
    pub async fn initialize(gpu_backend: GpuBackend) -> Result<Self, String> {
        let backends = match gpu_backend {
            GpuBackend::Auto => wgpu::Backends::all(),
            GpuBackend::Gl => wgpu::Backends::GL,
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|error| format!("GPU adapter unavailable: {error}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                // Rendering uses vertex/fragment shaders. Keep the adapter's
                // reported limits rather than requiring compute support.
                required_limits: adapter.limits(),
                ..Default::default()
            })
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
    ) -> Result<WgpuSurface<'window>, String> {
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
        Ok(WgpuSurface {
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

pub fn create_tile_pipeline(
    context: &WgpuContext,
    format: wgpu::TextureFormat,
) -> (WgpuPipeline, wgpu::BindGroupLayout) {
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
            source: wgpu::ShaderSource::Wgsl(crate::gpu::TILE_VERTEX_SHADER.into()),
        });
    let fragment = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tile-fragment-shader"),
            source: wgpu::ShaderSource::Wgsl(crate::gpu::TILE_FRAGMENT_SHADER.into()),
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
                buffers: &[Some(crate::gpu::TILE_VERTEX_LAYOUT)],
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
    (WgpuPipeline(pipeline), layout)
}

struct WgpuBuffer(wgpu::Buffer);

struct WgpuTexture(wgpu::Texture);

/// Owns concrete `wgpu` resources behind stable, backend-independent handles.
pub struct WgpuGraphicsDevice<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    next_handle: u64,
    buffers: HashMap<BufferHandle, WgpuBuffer>,
    textures: HashMap<TextureHandle, WgpuTexture>,
}

impl<'a> WgpuGraphicsDevice<'a> {
    pub fn new(device: &'a wgpu::Device, queue: &'a wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            next_handle: 0,
            buffers: HashMap::new(),
            textures: HashMap::new(),
        }
    }

    fn next_handle(&mut self) -> u64 {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        handle
    }
}

impl GraphicsDevice for WgpuGraphicsDevice<'_> {
    fn create_buffer(&mut self, descriptor: BufferDescriptor) -> Result<BufferHandle, DeviceError> {
        let usage = match descriptor.usage {
            BufferUsage::Vertex => wgpu::BufferUsages::VERTEX,
            BufferUsage::Index => wgpu::BufferUsages::INDEX,
            BufferUsage::Uniform => wgpu::BufferUsages::UNIFORM,
        } | wgpu::BufferUsages::COPY_DST;
        let handle = BufferHandle::new(self.next_handle());
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("abstract-render-buffer"),
            size: descriptor.size.max(1) as u64,
            usage,
            mapped_at_creation: false,
        });
        self.buffers.insert(handle, WgpuBuffer(buffer));
        Ok(handle)
    }

    fn create_texture(
        &mut self,
        descriptor: TextureDescriptor,
    ) -> Result<TextureHandle, DeviceError> {
        let format = match descriptor.format {
            TextureFormat::Rgba8 => crate::gpu::TILE_TEXTURE_FORMAT,
        };
        if descriptor.width == 0 || descriptor.height == 0 {
            return Err(DeviceError::InvalidResource);
        }
        let handle = TextureHandle::new(self.next_handle());
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("abstract-render-texture"),
            size: wgpu::Extent3d {
                width: descriptor.width,
                height: descriptor.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.textures.insert(handle, WgpuTexture(texture));
        Ok(handle)
    }

    fn submit(&mut self, commands: CommandList) -> Result<(), DeviceError> {
        validate_commands(&commands)?;
        for command in commands.commands() {
            match command {
                Command::WriteBuffer {
                    buffer,
                    offset,
                    bytes,
                } => {
                    let buffer = self
                        .buffers
                        .get(buffer)
                        .ok_or(DeviceError::InvalidResource)?;
                    self.queue.write_buffer(&buffer.0, *offset as u64, bytes);
                }
                Command::WriteTexture {
                    texture,
                    width,
                    height,
                    bytes,
                } => {
                    let texture = self
                        .textures
                        .get(texture)
                        .ok_or(DeviceError::InvalidResource)?;
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture.0,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytes,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * width),
                            rows_per_image: Some(*height),
                        },
                        wgpu::Extent3d {
                            width: *width,
                            height: *height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                Command::DrawTexture { .. } | Command::Present => {
                    return Err(DeviceError::UnsupportedCommand);
                }
            }
        }
        Ok(())
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) {
        self.buffers.remove(&buffer);
    }

    fn destroy_texture(&mut self, texture: TextureHandle) {
        self.textures.remove(&texture);
    }
}

pub struct GpuTileTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
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
        context: &WgpuContext,
        layout: &wgpu::BindGroupLayout,
        upload: TextureUpload,
    ) {
        let texture = upload_tile_texture(context, layout, &upload);
        self.textures.insert(upload.key, texture);
    }

    pub fn upload_image_update(
        &mut self,
        context: &WgpuContext,
        layout: &wgpu::BindGroupLayout,
        update: &ImageUpdate,
    ) {
        let (width, height) = update.dimensions();
        self.upload(
            context,
            layout,
            TextureUpload {
                key: TextureKey {
                    tile: update.image().value() as usize,
                    content_hash: update.revision().value(),
                },
                width,
                height,
                rgba8: update.rgba8().to_vec(),
            },
        );
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
    context: &WgpuContext,
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
        format: TILE_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_tile_texture(context, &texture, upload);
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
        width: upload.width,
        height: upload.height,
    }
}

pub fn write_tile_texture(context: &WgpuContext, texture: &wgpu::Texture, upload: &TextureUpload) {
    context.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
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
}

fn validate_commands(commands: &CommandList) -> Result<(), DeviceError> {
    if commands
        .commands()
        .iter()
        .any(|command| matches!(command, Command::DrawTexture { .. } | Command::Present))
    {
        return Err(DeviceError::UnsupportedCommand);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_commands;
    use crate::render::device::{CommandList, DeviceError, TextureHandle};

    #[test]
    fn rejects_draw_and_present_until_the_render_target_owns_presentation() {
        let mut uploads = CommandList::default();
        uploads.write_texture(TextureHandle::new(1), 1, 1, vec![0, 0, 0, 255]);
        assert_eq!(validate_commands(&uploads), Ok(()));

        let mut commands = CommandList::default();
        commands.draw_texture(TextureHandle::new(1), 0, 0, 1, 1, 1.0);
        commands.present();

        assert_eq!(
            validate_commands(&commands),
            Err(DeviceError::UnsupportedCommand)
        );
    }
}
