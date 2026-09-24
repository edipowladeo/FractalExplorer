//! `wgpu` implementation of the portable graphics-device resource contract.

use crate::render::device::{
    BufferDescriptor, BufferHandle, BufferUsage, Command, CommandList, DeviceError, GraphicsDevice,
    TextureDescriptor, TextureFormat, TextureHandle,
};
use std::collections::HashMap;

pub struct WgpuPipeline(pub wgpu::RenderPipeline);

pub struct WgpuSurface<'window> {
    pub surface: wgpu::Surface<'window>,
    pub format: wgpu::TextureFormat,
    pub present_mode: wgpu::PresentMode,
}

pub fn create_tile_pipeline(
    context: &crate::gpu::GpuContext,
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
