//! `wgpu` implementation of the portable graphics-device resource contract.

use crate::config::GpuBackend;
use crate::gpu::{TextureKey, TextureUpload, TileDrawCommand};
use crate::render::device::{
    BufferDescriptor, BufferHandle, BufferUsage, Command, CommandList, DeviceError, GraphicsDevice,
    TextureDescriptor, TextureFormat, TextureHandle,
};
use crate::render::ImageUpdate;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub opacity: f32,
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
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32,
            offset: (2 * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress,
            shader_location: 2,
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
            opacity: 1.0,
        },
        TileVertex {
            position: [1.0, -1.0],
            uv: [1.0, 1.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [1.0, 1.0],
            uv: [1.0, 0.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [-1.0, -1.0],
            uv: [0.0, 1.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [1.0, 1.0],
            uv: [1.0, 0.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [-1.0, 1.0],
            uv: [0.0, 0.0],
            opacity: 1.0,
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
            opacity: 1.0,
        },
        TileVertex {
            position: [x1, y1],
            uv: [1.0, 1.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [x1, y0],
            uv: [1.0, 0.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [x0, y1],
            uv: [0.0, 1.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [x1, y0],
            uv: [1.0, 0.0],
            opacity: 1.0,
        },
        TileVertex {
            position: [x0, y0],
            uv: [0.0, 0.0],
            opacity: 1.0,
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

pub const GPU_VERTEX_BUFFER_RING_SIZE: usize = 3;

fn vertex_buffer_capacity(current: usize, required: usize) -> usize {
    if required <= current {
        return current;
    }
    let mut capacity = current.max(1);
    while capacity < required {
        capacity = capacity.saturating_mul(2);
        if capacity == usize::MAX {
            return required;
        }
    }
    capacity
}

fn vertex_buffer_needs_recreation(current_capacity: usize, required_vertices: usize) -> bool {
    required_vertices > current_capacity
}

fn next_vertex_buffer_slot(current: usize, slot_count: usize) -> usize {
    (current + 1) % slot_count.max(1)
}

pub struct WgpuVertexBufferRing {
    buffers: Vec<Option<wgpu::Buffer>>,
    capacities: Vec<usize>,
    active_slot: usize,
}

impl WgpuVertexBufferRing {
    pub fn new(slot_count: usize) -> Self {
        let slot_count = slot_count.max(1);
        Self {
            buffers: (0..slot_count).map(|_| None).collect(),
            capacities: vec![0; slot_count],
            active_slot: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.active_slot = next_vertex_buffer_slot(self.active_slot, self.buffers.len());
    }

    pub fn reset(&mut self) {
        for buffer in &mut self.buffers {
            *buffer = None;
        }
        self.capacities.fill(0);
        self.active_slot = 0;
    }

    pub fn active_buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffers[self.active_slot].as_ref()
    }

    pub fn active_slot(&self) -> usize {
        self.active_slot
    }

    pub fn slot_count(&self) -> usize {
        self.buffers.len()
    }

    pub fn ensure_buffer(
        &mut self,
        context: &WgpuContext,
        label: &'static str,
        required_vertices: usize,
    ) -> bool {
        if required_vertices == 0 {
            return false;
        }
        let current_capacity = self.capacities[self.active_slot];
        let capacity = vertex_buffer_capacity(current_capacity, required_vertices);
        if self.buffers[self.active_slot].is_none()
            || vertex_buffer_needs_recreation(current_capacity, required_vertices)
        {
            self.buffers[self.active_slot] =
                Some(context.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: (capacity * std::mem::size_of::<TileVertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.capacities[self.active_slot] = capacity;
        }
        true
    }

    pub fn write_active(&self, context: &WgpuContext, bytes: &[u8]) {
        if let Some(buffer) = self.active_buffer() {
            context.queue.write_buffer(buffer, 0, bytes);
        }
    }
}

pub struct WgpuPipeline(pub wgpu::RenderPipeline);

pub struct WgpuTextureLayout(wgpu::BindGroupLayout);

impl WgpuTextureLayout {
    fn as_raw(&self) -> &wgpu::BindGroupLayout {
        &self.0
    }
}

pub struct WgpuCompositionTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    present_vertex_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
}

impl WgpuCompositionTexture {
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn present_vertex_buffer(&self) -> &wgpu::Buffer {
        &self.present_vertex_buffer
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

pub struct WgpuSurface<'window> {
    surface: wgpu::Surface<'window>,
    configuration: wgpu::SurfaceConfiguration,
}

#[derive(Clone, Copy)]
pub struct WgpuSurfaceFormat(wgpu::TextureFormat);

pub enum WgpuSurfaceAcquire {
    Ready(WgpuSurfaceFrame),
    Suboptimal(WgpuSurfaceFrame),
    Timeout,
    Occluded,
    Outdated,
    Lost,
    Validation,
}

pub struct WgpuSurfaceFrame(wgpu::SurfaceTexture);

impl WgpuSurfaceFrame {
    pub fn create_view(&self) -> wgpu::TextureView {
        self.0
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn present(self, context: &WgpuContext) {
        context.queue.present(self.0);
    }
}

impl WgpuSurface<'_> {
    pub fn format(&self) -> WgpuSurfaceFormat {
        WgpuSurfaceFormat(self.configuration.format)
    }

    pub fn present_mode_description(&self) -> String {
        format!("{:?}", self.configuration.present_mode)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.configuration.width, self.configuration.height)
    }

    pub fn configure(&self, context: &WgpuContext) {
        self.surface.configure(&context.device, &self.configuration);
    }

    pub fn resize(&mut self, context: &WgpuContext, width: u32, height: u32) {
        self.configuration.width = width.max(1);
        self.configuration.height = height.max(1);
        self.configure(context);
    }

    pub fn acquire(&self) -> WgpuSurfaceAcquire {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => {
                WgpuSurfaceAcquire::Ready(WgpuSurfaceFrame(frame))
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                WgpuSurfaceAcquire::Suboptimal(WgpuSurfaceFrame(frame))
            }
            wgpu::CurrentSurfaceTexture::Timeout => WgpuSurfaceAcquire::Timeout,
            wgpu::CurrentSurfaceTexture::Occluded => WgpuSurfaceAcquire::Occluded,
            wgpu::CurrentSurfaceTexture::Outdated => WgpuSurfaceAcquire::Outdated,
            wgpu::CurrentSurfaceTexture::Lost => WgpuSurfaceAcquire::Lost,
            wgpu::CurrentSurfaceTexture::Validation => WgpuSurfaceAcquire::Validation,
        }
    }
}

pub fn select_present_mode(modes: &[wgpu::PresentMode]) -> Option<wgpu::PresentMode> {
    [wgpu::PresentMode::AutoNoVsync, wgpu::PresentMode::Immediate]
        .into_iter()
        .find(|preferred| modes.contains(preferred))
        .or_else(|| modes.first().copied())
}

pub fn surface_usage(supported: wgpu::TextureUsages) -> wgpu::TextureUsages {
    wgpu::TextureUsages::RENDER_ATTACHMENT & supported
}

pub fn surface_load_op(
    preserve_previous_frame: bool,
    surface_initialized: bool,
) -> wgpu::LoadOp<wgpu::Color> {
    if preserve_previous_frame && surface_initialized {
        wgpu::LoadOp::Load
    } else {
        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
    }
}

/// Owns the selected adapter, device, queue, and instance for the `wgpu` API.
pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl WgpuContext {
    pub fn adapter_description(&self) -> (wgpu::Backend, String, wgpu::DeviceType) {
        let info = self.adapter.get_info();
        (info.backend, info.name, info.device_type)
    }

    pub fn poll_device(&self) -> Result<String, String> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map(|status| format!("{status:?}"))
            .map_err(|error| error.to_string())
    }

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

    pub fn create_surface(
        &self,
        window: std::sync::Arc<winit::window::Window>,
        width: u32,
        height: u32,
    ) -> Result<WgpuSurface<'static>, String> {
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
        let present_mode = select_present_mode(&capabilities.present_modes)
            .ok_or_else(|| "GPU surface has no supported present modes".to_string())?;
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .ok_or_else(|| "GPU surface has no supported alpha modes".to_string())?;
        let configuration = wgpu::SurfaceConfiguration {
            usage: surface_usage(capabilities.usages),
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: width.max(1),
            height: height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &configuration);
        Ok(WgpuSurface {
            surface,
            configuration,
        })
    }
}

pub fn create_tile_pipeline(
    context: &WgpuContext,
    format: WgpuSurfaceFormat,
) -> (WgpuPipeline, WgpuTextureLayout) {
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
                    format: format.0,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
    (WgpuPipeline(pipeline), WgpuTextureLayout(layout))
}

pub fn create_composition_texture(
    context: &WgpuContext,
    layout: &WgpuTextureLayout,
    format: WgpuSurfaceFormat,
    width: u32,
    height: u32,
) -> WgpuCompositionTexture {
    let texture = context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("persistent-composition"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: format.0,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("composition-sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composition-bind-group"),
            layout: layout.as_raw(),
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
    let present_vertex_buffer =
        context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("composition-present-quad"),
                contents: bytemuck::cast_slice(&tile_quad_vertices()),
                usage: wgpu::BufferUsages::VERTEX,
            });
    WgpuCompositionTexture {
        _texture: texture,
        view,
        bind_group,
        present_vertex_buffer,
        width,
        height,
    }
}

pub struct WgpuEncodedFrame {
    command_buffer: wgpu::CommandBuffer,
    composition_duration: Duration,
    presentation_duration: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WgpuFrameStage {
    CompositionStarted,
    CompositionFinished(Duration),
    PresentationStarted,
    PresentationFinished(Duration),
    CommandEncodingFinished,
}

impl WgpuEncodedFrame {
    pub fn composition_duration(&self) -> Duration {
        self.composition_duration
    }

    pub fn presentation_duration(&self) -> Option<Duration> {
        self.presentation_duration
    }

    pub fn submit(self, context: &WgpuContext) {
        context.queue.submit(Some(self.command_buffer));
    }
}

pub fn encode_frame(
    context: &WgpuContext,
    frame: &WgpuSurfaceFrame,
    pipeline: &WgpuPipeline,
    texture_store: Option<&GpuTextureStore>,
    tile_commands: &[TileDrawCommand],
    tile_vertices: &WgpuVertexBufferRing,
    envelope: Option<(&GpuTileTexture, &WgpuVertexBufferRing)>,
    overlay: Option<(&GpuTileTexture, &WgpuVertexBufferRing)>,
    composition: Option<&WgpuCompositionTexture>,
    preserve_previous_frame: bool,
    surface_initialized: bool,
    mut report_stage: impl FnMut(WgpuFrameStage),
) -> WgpuEncodedFrame {
    let surface_view = frame.create_view();
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu-clear"),
        });
    report_stage(WgpuFrameStage::CompositionStarted);
    let composition_started = Instant::now();
    {
        let target = composition
            .map(WgpuCompositionTexture::view)
            .unwrap_or(&surface_view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gpu-clear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: surface_load_op(preserve_previous_frame, surface_initialized),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline.0);
        if let Some(store) = texture_store {
            if let Some(vertex_buffer) = tile_vertices.active_buffer() {
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                for (index, command) in tile_commands.iter().enumerate() {
                    if let Some(tile_texture) = store.get(&command.texture) {
                        pass.set_bind_group(0, &tile_texture.bind_group, &[]);
                        let start = (index * 6) as u32;
                        pass.draw(start..start + 6, 0..1);
                    }
                }
            }
            if let Some((texture, ring)) = envelope {
                if let Some(vertex_buffer) = ring.active_buffer() {
                    pass.set_bind_group(0, &texture.bind_group, &[]);
                    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    pass.draw(0..6, 0..1);
                }
            }
            if let Some((texture, ring)) = overlay {
                if let Some(vertex_buffer) = ring.active_buffer() {
                    pass.set_bind_group(0, &texture.bind_group, &[]);
                    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    pass.draw(0..6, 0..1);
                }
            }
        }
    }
    let composition_duration = composition_started.elapsed();
    report_stage(WgpuFrameStage::CompositionFinished(composition_duration));

    let present_started = Instant::now();
    if let Some(composition) = composition {
        report_stage(WgpuFrameStage::PresentationStarted);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gpu-present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline.0);
        pass.set_bind_group(0, composition.bind_group(), &[]);
        pass.set_vertex_buffer(0, composition.present_vertex_buffer().slice(..));
        pass.draw(0..6, 0..1);
        drop(pass);
        let presentation_duration = present_started.elapsed();
        report_stage(WgpuFrameStage::PresentationFinished(presentation_duration));
        let command_buffer = encoder.finish();
        report_stage(WgpuFrameStage::CommandEncodingFinished);
        WgpuEncodedFrame {
            command_buffer,
            composition_duration,
            presentation_duration: Some(presentation_duration),
        }
    } else {
        let command_buffer = encoder.finish();
        report_stage(WgpuFrameStage::CommandEncodingFinished);
        WgpuEncodedFrame {
            command_buffer,
            composition_duration,
            presentation_duration: None,
        }
    }
}

struct WgpuBuffer {
    buffer: wgpu::Buffer,
    size: usize,
}

struct WgpuTexture {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// Owns concrete `wgpu` resources behind stable, backend-independent handles.
pub struct WgpuGraphicsDevice {
    context: Arc<WgpuContext>,
    surface: Arc<Mutex<WgpuSurface<'static>>>,
    pipeline: WgpuPipeline,
    texture_layout: WgpuTextureLayout,
    next_handle: u64,
    buffers: HashMap<BufferHandle, WgpuBuffer>,
    textures: HashMap<TextureHandle, WgpuTexture>,
    vertex_ring: WgpuVertexBufferRing,
    composition: Option<WgpuCompositionTexture>,
    preserve_previous_frame: bool,
    surface_initialized: bool,
}

impl WgpuGraphicsDevice {
    pub fn new(
        context: Arc<WgpuContext>,
        surface: Arc<Mutex<WgpuSurface<'static>>>,
    ) -> Result<Self, DeviceError> {
        let format = surface
            .lock()
            .map_err(|_| DeviceError::InvalidResource)?
            .format();
        let (pipeline, texture_layout) = create_tile_pipeline(&context, format);
        Ok(Self {
            context,
            surface,
            pipeline,
            texture_layout,
            next_handle: 0,
            buffers: HashMap::new(),
            textures: HashMap::new(),
            vertex_ring: WgpuVertexBufferRing::new(GPU_VERTEX_BUFFER_RING_SIZE),
            composition: None,
            preserve_previous_frame: false,
            surface_initialized: false,
        })
    }

    pub fn set_preserve_previous_frame(&mut self, preserve: bool) {
        self.preserve_previous_frame = preserve;
        if !preserve {
            self.composition = None;
            self.surface_initialized = false;
        }
    }

    pub fn adapter_description(&self) -> (wgpu::Backend, String, wgpu::DeviceType) {
        self.context.adapter_description()
    }

    pub fn surface_size(&self) -> Result<(u32, u32), DeviceError> {
        self.surface
            .lock()
            .map(|surface| surface.size())
            .map_err(|_| DeviceError::InvalidResource)
    }

    fn draw(&mut self, commands: &CommandList) -> Result<(), DeviceError> {
        let draws = commands
            .commands()
            .iter()
            .filter_map(|command| match command {
                Command::DrawTexture { texture, .. } => Some(*texture),
                _ => None,
            })
            .collect::<Vec<_>>();
        let surface = self
            .surface
            .lock()
            .map_err(|_| DeviceError::InvalidResource)?;
        let (screen_width, screen_height) = surface.size();
        if self.preserve_previous_frame
            && self
                .composition
                .as_ref()
                .is_none_or(|texture| texture.size() != (screen_width, screen_height))
        {
            self.composition = Some(create_composition_texture(
                &self.context,
                &self.texture_layout,
                surface.format(),
                screen_width,
                screen_height,
            ));
            self.surface_initialized = false;
        }
        let vertices = render_vertices_for_commands(commands, screen_width, screen_height);
        self.vertex_ring.begin_frame();
        if !vertices.is_empty()
            && !self.vertex_ring.ensure_buffer(
                &self.context,
                "render-target-draw-vertices",
                vertices.len(),
            )
        {
            return Err(DeviceError::InvalidResource);
        }
        self.vertex_ring
            .write_active(&self.context, bytemuck::cast_slice(&vertices));

        let frame = match surface.acquire() {
            WgpuSurfaceAcquire::Ready(frame) | WgpuSurfaceAcquire::Suboptimal(frame) => frame,
            WgpuSurfaceAcquire::Outdated | WgpuSurfaceAcquire::Lost => {
                surface.configure(&self.context);
                match surface.acquire() {
                    WgpuSurfaceAcquire::Ready(frame) | WgpuSurfaceAcquire::Suboptimal(frame) => {
                        frame
                    }
                    _ => return Err(DeviceError::InvalidResource),
                }
            }
            WgpuSurfaceAcquire::Timeout
            | WgpuSurfaceAcquire::Occluded
            | WgpuSurfaceAcquire::Validation => return Err(DeviceError::InvalidResource),
        };
        let view = frame.create_view();
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render-target-command-encoder"),
                });
        {
            let target = self
                .composition
                .as_ref()
                .map(WgpuCompositionTexture::view)
                .unwrap_or(&view);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-target-composition-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: surface_load_op(
                            self.preserve_previous_frame,
                            self.surface_initialized,
                        ),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline.0);
            if let Some(vertex_buffer) = self.vertex_ring.active_buffer() {
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                for (index, texture) in draws.iter().enumerate() {
                    let texture = self
                        .textures
                        .get(texture)
                        .ok_or(DeviceError::InvalidResource)?;
                    pass.set_bind_group(0, &texture.bind_group, &[]);
                    let start = (index * 6) as u32;
                    pass.draw(start..start + 6, 0..1);
                }
            }
        }
        if let Some(composition) = &self.composition {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-target-present-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline.0);
            pass.set_bind_group(0, composition.bind_group(), &[]);
            pass.set_vertex_buffer(0, composition.present_vertex_buffer().slice(..));
            pass.draw(0..6, 0..1);
        }
        self.context.queue.submit(Some(encoder.finish()));
        frame.present(&self.context);
        self.surface_initialized = true;
        Ok(())
    }

    fn new_handle(&mut self) -> u64 {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        handle
    }
}

impl GraphicsDevice for WgpuGraphicsDevice {
    fn resize(&mut self, viewport: crate::render::Viewport) -> Result<(), DeviceError> {
        self.surface
            .lock()
            .map_err(|_| DeviceError::InvalidResource)?
            .resize(&self.context, viewport.width(), viewport.height());
        self.vertex_ring.reset();
        self.composition = None;
        self.surface_initialized = false;
        Ok(())
    }

    fn create_buffer(&mut self, descriptor: BufferDescriptor) -> Result<BufferHandle, DeviceError> {
        let usage = match descriptor.usage {
            BufferUsage::Vertex => wgpu::BufferUsages::VERTEX,
            BufferUsage::Index => wgpu::BufferUsages::INDEX,
            BufferUsage::Uniform => wgpu::BufferUsages::UNIFORM,
        } | wgpu::BufferUsages::COPY_DST;
        let handle = BufferHandle::new(self.new_handle());
        let buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("abstract-render-buffer"),
            size: descriptor.size.max(1) as u64,
            usage,
            mapped_at_creation: false,
        });
        self.buffers.insert(
            handle,
            WgpuBuffer {
                buffer,
                size: descriptor.size.max(1),
            },
        );
        Ok(handle)
    }

    fn create_texture(
        &mut self,
        descriptor: TextureDescriptor,
    ) -> Result<TextureHandle, DeviceError> {
        if descriptor.width == 0 || descriptor.height == 0 {
            return Err(DeviceError::InvalidResource);
        }
        let format = match descriptor.format {
            TextureFormat::Rgba8 => TILE_TEXTURE_FORMAT,
        };
        let texture = self
            .context
            .device
            .create_texture(&wgpu::TextureDescriptor {
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self
            .context
            .device
            .create_sampler(&wgpu::SamplerDescriptor {
                label: Some("abstract-render-sampler"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("abstract-render-bind-group"),
                layout: self.texture_layout.as_raw(),
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
        let handle = TextureHandle::new(self.new_handle());
        self.textures.insert(
            handle,
            WgpuTexture {
                _texture: texture,
                bind_group,
                width: descriptor.width,
                height: descriptor.height,
            },
        );
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
                    if offset
                        .checked_add(bytes.len())
                        .is_none_or(|end| end > buffer.size)
                    {
                        return Err(DeviceError::InvalidResource);
                    }
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
                    let expected_bytes = (*width as usize)
                        .checked_mul(*height as usize)
                        .and_then(|pixels| pixels.checked_mul(4));
                    if (*width, *height) != (texture.width, texture.height)
                        || expected_bytes != Some(bytes.len())
                    {
                        return Err(DeviceError::InvalidResource);
                    }
                }
                Command::DrawTexture {
                    texture,
                    width,
                    height,
                    opacity_bits,
                    ..
                } => {
                    if !self.textures.contains_key(texture)
                        || *width == 0
                        || *height == 0
                        || !(0.0..=1.0).contains(&f32::from_bits(*opacity_bits))
                    {
                        return Err(DeviceError::InvalidResource);
                    }
                }
                Command::Present => {}
            }
        }
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
                    self.context
                        .queue
                        .write_buffer(&buffer.buffer, *offset as u64, bytes);
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
                    self.context.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture._texture,
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
                Command::DrawTexture { .. } => {}
                Command::Present => {}
            }
        }
        if commands
            .commands()
            .iter()
            .any(|command| matches!(command, Command::Present))
        {
            self.draw(&commands)?;
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
        layout: &WgpuTextureLayout,
        upload: TextureUpload,
    ) {
        let texture = upload_tile_texture(context, layout, &upload);
        self.textures.insert(upload.key, texture);
    }

    pub fn upload_image_update(
        &mut self,
        context: &WgpuContext,
        layout: &WgpuTextureLayout,
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
    layout: &WgpuTextureLayout,
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
            layout: layout.as_raw(),
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
    let mut drawing_started = false;
    let mut presented = false;
    for command in commands.commands() {
        if presented {
            return Err(DeviceError::UnsupportedCommand);
        }
        match command {
            Command::WriteBuffer { .. } | Command::WriteTexture { .. } if drawing_started => {
                return Err(DeviceError::UnsupportedCommand);
            }
            Command::DrawTexture { .. } => drawing_started = true,
            Command::Present => presented = true,
            Command::WriteBuffer { .. } | Command::WriteTexture { .. } => {}
        }
    }
    if drawing_started && !presented {
        return Err(DeviceError::UnsupportedCommand);
    }
    Ok(())
}

fn render_vertices_for_commands(
    commands: &CommandList,
    screen_width: u32,
    screen_height: u32,
) -> Vec<TileVertex> {
    let draws = commands
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::DrawTexture {
                texture,
                x,
                y,
                width,
                height,
                opacity_bits,
            } => Some((
                TileDrawCommand {
                    texture: TextureKey {
                        tile: texture.value() as usize,
                        content_hash: 0,
                    },
                    position: crate::geometry::ScreenPoint::new(*x, *y),
                    size: (*width, *height),
                },
                f32::from_bits(*opacity_bits),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let commands = draws
        .iter()
        .map(|(command, _)| *command)
        .collect::<Vec<_>>();
    let mut vertices = tile_vertices_for_commands(&commands, screen_width, screen_height);
    for (quad, (_, opacity)) in vertices.chunks_exact_mut(6).zip(draws) {
        for vertex in quad {
            vertex.opacity = opacity;
        }
    }
    vertices
}

#[cfg(test)]
mod tests {
    use super::{
        next_vertex_buffer_slot, render_vertices_for_commands, select_present_mode,
        surface_load_op, surface_usage, validate_commands, vertex_buffer_capacity,
        vertex_buffer_needs_recreation,
    };
    use crate::render::device::{CommandList, DeviceError, TextureHandle};

    #[test]
    fn surface_policy_prefers_non_vsync_modes_and_falls_back_to_supported_modes() {
        assert_eq!(
            select_present_mode(&[
                wgpu::PresentMode::Fifo,
                wgpu::PresentMode::Immediate,
                wgpu::PresentMode::AutoNoVsync,
            ]),
            Some(wgpu::PresentMode::AutoNoVsync)
        );
        assert_eq!(
            select_present_mode(&[wgpu::PresentMode::Fifo]),
            Some(wgpu::PresentMode::Fifo)
        );
        assert_eq!(select_present_mode(&[]), None);
    }

    #[test]
    fn surface_policy_requests_only_render_attachment_usage() {
        let supported = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        assert_eq!(
            surface_usage(supported),
            wgpu::TextureUsages::RENDER_ATTACHMENT
        );
    }

    #[test]
    fn surface_load_policy_preserves_only_an_initialized_frame() {
        assert!(matches!(
            surface_load_op(false, false),
            wgpu::LoadOp::Clear(_)
        ));
        assert!(matches!(
            surface_load_op(true, false),
            wgpu::LoadOp::Clear(_)
        ));
        assert!(matches!(surface_load_op(true, true), wgpu::LoadOp::Load));
    }

    #[test]
    fn vertex_buffer_capacity_grows_only_when_required_vertices_do_not_fit() {
        assert_eq!(vertex_buffer_capacity(96, 48), 96);
        assert_eq!(vertex_buffer_capacity(96, 97), 192);
        assert_eq!(vertex_buffer_capacity(0, 1), 1);
    }

    #[test]
    fn vertex_buffer_recreation_is_needed_only_after_capacity_is_exceeded() {
        assert!(!vertex_buffer_needs_recreation(96, 96));
        assert!(!vertex_buffer_needs_recreation(96, 48));
        assert!(vertex_buffer_needs_recreation(96, 97));
    }

    #[test]
    fn vertex_buffer_ring_rotates_without_reusing_the_current_slot() {
        assert_eq!(next_vertex_buffer_slot(0, 3), 1);
        assert_eq!(next_vertex_buffer_slot(1, 3), 2);
        assert_eq!(next_vertex_buffer_slot(2, 3), 0);
        assert_eq!(next_vertex_buffer_slot(0, 0), 0);
    }

    #[test]
    fn validates_upload_draw_present_order_for_gpu_render_targets() {
        let mut uploads = CommandList::default();
        uploads.write_texture(TextureHandle::new(1), 1, 1, vec![0, 0, 0, 255]);
        assert_eq!(validate_commands(&uploads), Ok(()));

        let mut commands = CommandList::default();
        commands.write_texture(TextureHandle::new(1), 1, 1, vec![0, 0, 0, 255]);
        commands.draw_texture(TextureHandle::new(1), 0, 0, 1, 1, 1.0);
        commands.present();
        assert_eq!(validate_commands(&commands), Ok(()));

        let mut writes_after_draw = CommandList::default();
        writes_after_draw.draw_texture(TextureHandle::new(1), 0, 0, 1, 1, 1.0);
        writes_after_draw.write_texture(TextureHandle::new(1), 1, 1, vec![0, 0, 0, 255]);
        writes_after_draw.present();
        assert_eq!(
            validate_commands(&writes_after_draw),
            Err(DeviceError::UnsupportedCommand)
        );

        let mut draws_after_present = CommandList::default();
        draws_after_present.present();
        draws_after_present.draw_texture(TextureHandle::new(1), 0, 0, 1, 1, 1.0);
        assert_eq!(
            validate_commands(&draws_after_present),
            Err(DeviceError::UnsupportedCommand)
        );
    }

    #[test]
    fn draw_vertices_preserve_command_opacity() {
        let mut commands = CommandList::default();
        commands.draw_texture(TextureHandle::new(7), 2, 3, 4, 5, 0.375);
        commands.present();

        let vertices = render_vertices_for_commands(&commands, 16, 16);

        assert_eq!(vertices.len(), 6);
        assert!(vertices.iter().all(|vertex| vertex.opacity == 0.375));
    }
}
