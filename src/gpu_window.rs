use crate::app::{AppEffect, AppEvent, ApplicationController, DefaultApplicationController};
use crate::gpu::{
    create_tile_pipeline, debug_overlay_upload, debug_overlay_upload_with_rectangles,
    texture_keys_for_commands, tile_commands_for_frame, tile_vertices_for_commands,
    upload_tile_texture, write_tile_texture, GpuContext, GpuTextureStore, GpuTileTexture,
    PreparedTileBatch, TextureCache, TileDrawCommand,
};
use crate::input::{InputEvent, ZoomDirection};
use crate::render::{
    FrameBuilder, ImageId, ImageRevision, ImageUpdate, Rect, RenderFrame, TileDraw, Viewport,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

fn needs_batch_rebuild(previous: (u32, u32), next: (u32, u32)) -> bool {
    previous != next
}

fn surface_load_op(
    preserve_previous_frame: bool,
    surface_initialized: bool,
) -> wgpu::LoadOp<wgpu::Color> {
    if preserve_previous_frame && surface_initialized {
        wgpu::LoadOp::Load
    } else {
        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
    }
}

fn surface_usage(supported: wgpu::TextureUsages) -> wgpu::TextureUsages {
    wgpu::TextureUsages::RENDER_ATTACHMENT & supported
}

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

const GPU_VERTEX_BUFFER_RING_SIZE: usize = 3;

fn next_vertex_buffer_slot(current: usize, slot_count: usize) -> usize {
    (current + 1) % slot_count.max(1)
}

struct VertexBufferRing {
    buffers: Vec<Option<wgpu::Buffer>>,
    capacities: Vec<usize>,
    active_slot: usize,
}

impl VertexBufferRing {
    fn new(slot_count: usize) -> Self {
        let slot_count = slot_count.max(1);
        Self {
            buffers: (0..slot_count).map(|_| None).collect(),
            capacities: vec![0; slot_count],
            active_slot: 0,
        }
    }

    fn begin_frame(&mut self) {
        self.active_slot = next_vertex_buffer_slot(self.active_slot, self.buffers.len());
    }

    fn reset(&mut self) {
        for buffer in &mut self.buffers {
            *buffer = None;
        }
        self.capacities.fill(0);
        self.active_slot = 0;
    }

    fn active_buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffers[self.active_slot].as_ref()
    }

    fn active_slot(&self) -> usize {
        self.active_slot
    }

    fn slot_count(&self) -> usize {
        self.buffers.len()
    }

    fn ensure_buffer(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        required_vertices: usize,
    ) -> Option<&wgpu::Buffer> {
        if required_vertices == 0 {
            return None;
        }
        let current_capacity = self.capacities[self.active_slot];
        let capacity = vertex_buffer_capacity(current_capacity, required_vertices);
        if self.buffers[self.active_slot].is_none()
            || vertex_buffer_needs_recreation(current_capacity, required_vertices)
        {
            self.buffers[self.active_slot] = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (capacity * std::mem::size_of::<crate::gpu::TileVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.capacities[self.active_slot] = capacity;
        }
        self.active_buffer()
    }
}

const GPU_FRAME_HISTORY_CAPACITY: usize = 8;

fn format_gpu_frame_history(history: &VecDeque<(u64, Duration)>) -> String {
    history
        .iter()
        .map(|(frame, duration)| format!("#{frame}:{:.3}ms", duration.as_secs_f64() * 1_000.0))
        .collect::<Vec<_>>()
        .join("\n")
}

fn select_present_mode(modes: &[wgpu::PresentMode]) -> Option<wgpu::PresentMode> {
    [wgpu::PresentMode::AutoNoVsync, wgpu::PresentMode::Immediate]
        .into_iter()
        .find(|preferred| modes.contains(preferred))
        .or_else(|| modes.first().copied())
}

fn centered_bounds(width: usize, height: usize, ratio: f64) -> (i32, i32, i32, i32) {
    assert!(ratio > 0.0, "viewport ratio must be positive");
    let width = width.max(1) as f64;
    let height = height.max(1) as f64;
    let left = ((width * (1.0 - ratio)) / 2.0).round() as i32;
    let top = ((height * (1.0 - ratio)) / 2.0).round() as i32;
    let right = (width - 1.0 - left as f64).round() as i32;
    let bottom = (height - 1.0 - top as f64).round() as i32;
    (left, top, right, bottom)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnvelopeCacheKey {
    width: u32,
    height: u32,
    allocation: (i32, i32, i32, i32),
    deallocation: (i32, i32, i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OverlayCacheKey {
    width: u32,
    height: u32,
    content_hash: u64,
}

fn overlay_cache_key(upload: &crate::gpu::TextureUpload) -> OverlayCacheKey {
    OverlayCacheKey {
        width: upload.width,
        height: upload.height,
        content_hash: upload.key.content_hash,
    }
}

fn overlay_needs_refresh(previous: Option<OverlayCacheKey>, next: OverlayCacheKey) -> bool {
    previous != Some(next)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuFrameMetrics {
    visible_tiles: usize,
    uploaded_textures: usize,
    draw_calls: usize,
    prepare_duration: Duration,
}

fn format_gpu_upload_stage(label: &str, elapsed: Duration, detail: &str) -> String {
    format!(
        "{label}: {:.3} ms{}",
        elapsed.as_secs_f64() * 1_000.0,
        if detail.is_empty() {
            String::new()
        } else {
            format!(" ({detail})")
        }
    )
}

impl GpuFrameMetrics {
    pub(crate) fn from_batch(batch: &PreparedTileBatch, prepare_duration: Duration) -> Self {
        Self {
            visible_tiles: batch.commands.len(),
            uploaded_textures: batch.uploads.len(),
            draw_calls: batch.commands.len(),
            prepare_duration,
        }
    }

    fn description(self) -> String {
        format!(
            "tiles rasterizados neste frame: {}, texturas novas neste frame: {}",
            self.visible_tiles, self.uploaded_textures
        )
    }

    pub fn visible_tiles(self) -> usize {
        self.visible_tiles
    }

    pub fn uploaded_textures(self) -> usize {
        self.uploaded_textures
    }

    pub fn draw_calls(self) -> usize {
        self.draw_calls
    }

    pub fn prepare_duration(self) -> Duration {
        self.prepare_duration
    }

    pub fn matches_cpu_visible_tiles(self, cpu_visible_tiles: usize) -> bool {
        self.visible_tiles == cpu_visible_tiles
    }
}

pub struct GpuAppState {
    pub canvas: crate::TiledInfiniteCanvas,
    pub orchestrator: crate::Orchestrator,
    pub config: crate::config::RendererConfig,
    pub prepared_batch: Option<PreparedTileBatch>,
    pub prepared_frame: Option<RenderFrame>,
    pub prepared_image_updates: Vec<ImageUpdate>,
    frame_builder: FrameBuilder,
    pub texture_cache: TextureCache,
    pub last_frame_metrics: Option<GpuFrameMetrics>,
    pub frame_timing_ring: VecDeque<(u64, Duration)>,
    allocation_bounds: (i32, i32, i32, i32),
    deallocation_bounds: (i32, i32, i32, i32),
    cursor: crate::geometry::ScreenPoint,
    left_button_down: bool,
}

impl GpuAppState {
    pub fn new(
        canvas: crate::TiledInfiniteCanvas,
        orchestrator: crate::Orchestrator,
        config: crate::config::RendererConfig,
    ) -> Self {
        let allocation_bounds = centered_bounds(
            config.width,
            config.height,
            config.effective_allocation_ratio(),
        );
        let deallocation_bounds = centered_bounds(
            config.width,
            config.height,
            config.effective_deallocation_ratio(),
        );
        Self {
            canvas,
            orchestrator,
            config,
            prepared_batch: None,
            prepared_frame: None,
            prepared_image_updates: Vec::new(),
            frame_builder: FrameBuilder::new(),
            texture_cache: TextureCache::default(),
            last_frame_metrics: None,
            frame_timing_ring: VecDeque::with_capacity(GPU_FRAME_HISTORY_CAPACITY),
            allocation_bounds,
            deallocation_bounds,
            cursor: crate::geometry::ScreenPoint::new(0, 0),
            left_button_down: false,
        }
    }

    pub fn resize_viewport(&mut self, viewport: Viewport) {
        self.config.width = viewport.width().max(1) as usize;
        self.config.height = viewport.height().max(1) as usize;
        self.prepared_batch = None;
        self.prepared_frame = None;
        self.prepared_image_updates.clear();
    }

    fn input_events_for_window_event(&mut self, event: &WindowEvent) -> Vec<InputEvent> {
        let mut input_events = Vec::new();
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let next = crate::geometry::ScreenPoint::new(
                    position.x.round() as i32,
                    position.y.round() as i32,
                );
                if self.left_button_down {
                    input_events.push(InputEvent::Drag {
                        delta: crate::geometry::ScreenPoint::new(
                            next.x - self.cursor.x,
                            next.y - self.cursor.y,
                        ),
                    });
                }
                self.cursor = next;
            }
            WindowEvent::MouseInput { state, button, .. } if *button == MouseButton::Left => {
                self.left_button_down = *state == ElementState::Pressed;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 40.0,
                };
                if amount != 0.0 {
                    input_events.push(InputEvent::Zoom {
                        direction: if amount > 0.0 {
                            ZoomDirection::In
                        } else {
                            ZoomDirection::Out
                        },
                        cursor: self.cursor,
                    });
                }
            }
            _ => {}
        }
        input_events
    }

    fn apply_input_event(&mut self, event: InputEvent) {
        match event {
            InputEvent::Drag { delta } => self.canvas.drag(delta),
            InputEvent::Zoom { direction, cursor } => {
                let multiplier = self.config.zoom_multiplier;
                let current = self
                    .canvas
                    .layer(0)
                    .map_or(self.config.max_apparent_pixel_size(), |layer| layer.zoom());
                let zoom = match direction {
                    ZoomDirection::In => current * multiplier,
                    ZoomDirection::Out => current / multiplier,
                };
                self.canvas.zoom_at(cursor, zoom);
            }
            InputEvent::MiddleClick(_) => {}
        }
    }

    pub fn prepare_visible_batch(&mut self) {
        let preparation_started = Instant::now();
        self.canvas.begin_frame();
        if let Some(timing) = self.canvas.last_finished_frame_timing() {
            self.frame_timing_ring.push_back(timing);
            while self.frame_timing_ring.len() > GPU_FRAME_HISTORY_CAPACITY {
                self.frame_timing_ring.pop_front();
            }
        }
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::ConfigurationProcessed,
            "configuracao processada",
        );

        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::SurfacePrepared,
            "superficie preparada",
        );
        self.allocation_bounds = centered_bounds(
            self.config.width,
            self.config.height,
            self.config.effective_allocation_ratio(),
        );
        self.deallocation_bounds = centered_bounds(
            self.config.width,
            self.config.height,
            self.config.effective_deallocation_ratio(),
        );
        self.canvas
            .trim_outside_allocation(self.deallocation_bounds);
        self.canvas.ensure_screen_coverage(self.allocation_bounds);
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::ScreenCoverageCompleted,
            "cobertura da tela concluida",
        );

        for layer in self.canvas.layers() {
            self.orchestrator.render_layer(layer);
        }
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::LayerWorkScheduled,
            "trabalho das camadas agendado",
        );
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::TileCompositionStarted,
            "composicao de tiles iniciada",
        );

        let mut tiles = Vec::new();
        for layer in self.canvas.layers() {
            for row in 0..layer.row_count() {
                for column in 0..layer.column_count() {
                    let Some(tile) = layer.tile(row, column) else {
                        continue;
                    };
                    if tile.status() == crate::TileStatus::Completed && tile.sprite().is_none() {
                        let sprite = Arc::new(crate::renderer::sprite_from_tile(
                            tile,
                            self.config.effective_max_iterations() as u64,
                            self.config.palette,
                            self.config.palette_period,
                        ));
                        tile.set_sprite(sprite);
                    }
                    let Some(sprite) = tile.sprite() else {
                        continue;
                    };
                    let tile_sprite = crate::TileSprite::new(
                        Arc::clone(tile),
                        layer.complex_to_screen(tile.coordinate().clone()),
                        layer.zoom(),
                    );
                    tiles.push((tile_sprite, sprite));
                }
            }
        }
        let references: Vec<_> = tiles
            .iter()
            .map(|(tile, sprite)| (tile, Arc::clone(sprite)))
            .collect();
        let batch = crate::gpu::prepare_tile_batch(&mut self.texture_cache, &references);
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::TilesRasterized,
            GpuFrameMetrics::from_batch(&batch, preparation_started.elapsed()).description(),
        );
        self.last_frame_metrics = Some(GpuFrameMetrics::from_batch(
            &batch,
            preparation_started.elapsed(),
        ));
        let frame_tiles = batch
            .commands
            .iter()
            .enumerate()
            .map(|(layer, command)| {
                TileDraw::new(
                    ImageId::new(command.texture.tile as u64),
                    ImageRevision::new(command.texture.content_hash),
                    layer as u32,
                )
                .with_destination(Rect::new(
                    command.position.x,
                    command.position.y,
                    command.size.0,
                    command.size.1,
                ))
            })
            .collect();
        self.prepared_image_updates = batch
            .uploads
            .iter()
            .filter_map(|upload| {
                ImageUpdate::new(
                    ImageId::new(upload.key.tile as u64),
                    ImageRevision::new(upload.key.content_hash),
                    upload.width,
                    upload.height,
                    upload.rgba8.clone(),
                )
                .ok()
            })
            .collect();
        self.prepared_frame = Some(self.frame_builder.build(
            Viewport::new(self.config.width as u32, self.config.height as u32),
            frame_tiles,
            Vec::new(),
        ));
        self.prepared_batch = Some(batch);
    }
}

pub struct GpuWindowApp {
    pub state: Option<GpuAppState>,
    app_controller: DefaultApplicationController,
    context: Option<GpuContext>,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    pipeline: Option<wgpu::RenderPipeline>,
    tile_bind_group_layout: Option<wgpu::BindGroupLayout>,
    tile_vertex_ring: VertexBufferRing,
    overlay_vertex_ring: VertexBufferRing,
    overlay_texture: Option<GpuTileTexture>,
    overlay_cache_key: Option<OverlayCacheKey>,
    overlay_command: Option<TileDrawCommand>,
    envelope_vertex_ring: VertexBufferRing,
    envelope_texture: Option<GpuTileTexture>,
    envelope_command: Option<TileDrawCommand>,
    envelope_cache_key: Option<EnvelopeCacheKey>,
    texture_store: Option<GpuTextureStore>,
    tile_commands: Vec<crate::gpu::TileDrawCommand>,
    surface_initialized: bool,
    composition_texture: Option<CompositionTexture>,
}

struct CompositionTexture {
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    present_vertex_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
}

impl GpuWindowApp {
    pub fn new() -> Self {
        Self::with_state(None)
    }

    pub fn with_state(state: Option<GpuAppState>) -> Self {
        let ring_size = state
            .as_ref()
            .map(|state| state.config.gpu_vertex_buffer_ring_size)
            .unwrap_or(GPU_VERTEX_BUFFER_RING_SIZE);
        let viewport = state
            .as_ref()
            .map(|state| Viewport::new(state.config.width as u32, state.config.height as u32))
            .unwrap_or(Viewport::new(800, 600));
        Self {
            state,
            app_controller: DefaultApplicationController::new(viewport),
            context: None,
            window: None,
            surface: None,
            surface_config: None,
            pipeline: None,
            tile_bind_group_layout: None,
            tile_vertex_ring: VertexBufferRing::new(ring_size),
            overlay_vertex_ring: VertexBufferRing::new(ring_size),
            overlay_texture: None,
            overlay_cache_key: None,
            overlay_command: None,
            envelope_vertex_ring: VertexBufferRing::new(ring_size),
            envelope_texture: None,
            envelope_command: None,
            envelope_cache_key: None,
            texture_store: None,
            tile_commands: Vec::new(),
            surface_initialized: false,
            composition_texture: None,
        }
    }

    pub fn set_state(&mut self, state: GpuAppState) {
        let ring_size = state.config.gpu_vertex_buffer_ring_size;
        self.app_controller = DefaultApplicationController::new(Viewport::new(
            state.config.width as u32,
            state.config.height as u32,
        ));
        self.state = Some(state);
        self.tile_vertex_ring = VertexBufferRing::new(ring_size);
        self.overlay_vertex_ring = VertexBufferRing::new(ring_size);
        self.envelope_vertex_ring = VertexBufferRing::new(ring_size);
        self.surface_initialized = false;
        self.composition_texture = None;
    }

    fn configure_surface(&mut self, width: u32, height: u32) {
        let (Some(context), Some(surface)) = (&self.context, &self.surface) else {
            return;
        };
        let Some(config) = self.surface_config.as_mut() else {
            return;
        };
        let previous = (config.width, config.height);
        config.width = width.max(1);
        config.height = height.max(1);
        surface.configure(&context.device, config);
        if needs_batch_rebuild(previous, (config.width, config.height)) {
            self.surface_initialized = false;
            self.composition_texture = None;
            self.tile_vertex_ring.reset();
            self.overlay_vertex_ring.reset();
            self.envelope_vertex_ring.reset();
            self.envelope_texture = None;
            self.envelope_command = None;
            self.envelope_cache_key = None;
        }
    }

    fn ensure_composition_texture(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let needs_recreation = self
            .composition_texture
            .as_ref()
            .is_none_or(|texture| texture.width != width || texture.height != height);
        if needs_recreation {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("persistent-composition"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("composition-sampler"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("composition-bind-group"),
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
            let present_vertex_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("composition-present-quad"),
                    contents: bytemuck::cast_slice(&crate::gpu::tile_quad_vertices()),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            self.composition_texture = Some(CompositionTexture {
                view,
                bind_group,
                present_vertex_buffer,
                width,
                height,
            });
            self.surface_initialized = false;
        }
    }
}

impl GpuWindowApp {
    fn record_gpu_upload_stage(
        &mut self,
        kind: crate::orchestrator::FrameEventKind,
        label: &str,
        started_at: Instant,
        detail: impl AsRef<str>,
    ) {
        if let Some(state) = &mut self.state {
            state.canvas.record_frame_event(
                kind,
                format_gpu_upload_stage(label, started_at.elapsed(), detail.as_ref()),
            );
        }
    }

    pub fn upload_batch(&mut self, batch: PreparedTileBatch, frame_commands: Vec<TileDrawCommand>) {
        if self.context.is_none()
            || self.tile_bind_group_layout.is_none()
            || self.texture_store.is_none()
        {
            return;
        }
        let image_updates = self
            .state
            .as_ref()
            .map(|state| state.prepared_image_updates.clone())
            .unwrap_or_default();
        let texture_upload_count = if image_updates.is_empty() {
            batch.uploads.len()
        } else {
            image_updates.len()
        };
        let texture_upload_started = Instant::now();
        {
            let (Some(context), Some(layout), Some(store)) = (
                &self.context,
                &self.tile_bind_group_layout,
                &mut self.texture_store,
            ) else {
                return;
            };
            if image_updates.is_empty() {
                for upload in batch.uploads {
                    store.upload(context, layout, upload);
                }
            } else {
                for update in &image_updates {
                    store.upload_image_update(context, layout, update);
                }
            }
        }
        self.record_gpu_upload_stage(
            crate::orchestrator::FrameEventKind::GpuBatchTextureUpload,
            "upload de texturas do batch GPU",
            texture_upload_started,
            format!("{texture_upload_count} atualizacoes"),
        );

        let retention_started = Instant::now();
        {
            let Some(store) = &mut self.texture_store else {
                return;
            };
            store.retain_only(texture_keys_for_commands(&frame_commands));
        }
        self.record_gpu_upload_stage(
            crate::orchestrator::FrameEventKind::GpuBatchTextureRetention,
            "retencao de texturas do batch GPU",
            retention_started,
            format!("{} comandos", frame_commands.len()),
        );

        self.tile_vertex_ring.begin_frame();
        self.overlay_vertex_ring.begin_frame();
        self.envelope_vertex_ring.begin_frame();
        let vertex_upload_started = Instant::now();
        if let (Some(context), Some(surface_config)) = (&self.context, &self.surface_config) {
            let vertices = tile_vertices_for_commands(
                &frame_commands,
                surface_config.width,
                surface_config.height,
            );
            if !vertices.is_empty() {
                if let Some(buffer) = self.tile_vertex_ring.ensure_buffer(
                    &context.device,
                    "tile-batch-vertices",
                    vertices.len(),
                ) {
                    context
                        .queue
                        .write_buffer(buffer, 0, bytemuck::cast_slice(&vertices));
                }
            }
        }
        self.record_gpu_upload_stage(
            crate::orchestrator::FrameEventKind::GpuBatchVertexUpload,
            "upload de vertices dos tiles",
            vertex_upload_started,
            format!(
                "{} comandos, slot {}/{}",
                frame_commands.len(),
                self.tile_vertex_ring.active_slot(),
                self.tile_vertex_ring.slot_count()
            ),
        );
        self.tile_commands = frame_commands;

        self.overlay_command = None;
        let show_frame_overlay = self.state.as_ref().is_some_and(|state| {
            state.config.debug.overlays_enabled() && state.config.debug.text_overlay_frames
        });
        let show_envelope = self.state.as_ref().is_some_and(|state| {
            state.config.debug.overlays_enabled() && state.config.debug.show_allocation_envelope
        });
        if show_frame_overlay {
            let overlay_started = Instant::now();
            let history = self
                .state
                .as_ref()
                .map(|state| format_gpu_frame_history(&state.frame_timing_ring))
                .unwrap_or_default();
            let text = if !show_frame_overlay {
                String::new()
            } else if history.is_empty() {
                format!(
                    "#{} tiles:{}",
                    self.tile_commands.len(),
                    self.tile_commands.len()
                )
            } else {
                format!(
                    "#{} tiles:{}\n{}",
                    self.tile_commands.len(),
                    self.tile_commands.len(),
                    history
                )
            };
            let line_count = text.lines().count().max(1);
            let upload = debug_overlay_upload(&text, 240, line_count as u32 * 16);
            let key = upload.key;
            if let (Some(context), Some(layout), Some(surface_config)) = (
                &self.context,
                &self.tile_bind_group_layout,
                &self.surface_config,
            ) {
                let cache_key = overlay_cache_key(&upload);
                if overlay_needs_refresh(self.overlay_cache_key, cache_key) {
                    if let Some(texture) = self.overlay_texture.as_ref().filter(|texture| {
                        texture.width == upload.width && texture.height == upload.height
                    }) {
                        write_tile_texture(context, &texture.texture, &upload);
                    } else {
                        self.overlay_texture = Some(upload_tile_texture(context, layout, &upload));
                    }
                    self.overlay_cache_key = Some(cache_key);
                }
                let position = crate::geometry::ScreenPoint::new(
                    8,
                    surface_config.height.saturating_sub(upload.height + 8) as i32,
                );
                let command = TileDrawCommand {
                    texture: key,
                    position,
                    size: (upload.width, upload.height),
                };
                let vertices = tile_vertices_for_commands(
                    &[command],
                    surface_config.width,
                    surface_config.height,
                );
                if let Some(buffer) = self.overlay_vertex_ring.ensure_buffer(
                    &context.device,
                    "gpu-debug-overlay-vertices",
                    vertices.len(),
                ) {
                    context
                        .queue
                        .write_buffer(buffer, 0, bytemuck::cast_slice(&vertices));
                }
                self.overlay_command = Some(command);
            }
            self.record_gpu_upload_stage(
                crate::orchestrator::FrameEventKind::GpuBatchOverlayUpload,
                "upload do overlay GPU",
                overlay_started,
                format!(
                    "overlay habilitado, slot {}/{}",
                    self.overlay_vertex_ring.active_slot(),
                    self.overlay_vertex_ring.slot_count()
                ),
            );
        } else {
            self.overlay_texture = None;
            self.overlay_cache_key = None;
        }
        if show_envelope {
            let envelope_started = Instant::now();
            if let (Some(context), Some(layout), Some(surface_config), Some(state)) = (
                &self.context,
                &self.tile_bind_group_layout,
                &self.surface_config,
                &self.state,
            ) {
                let cache_key = EnvelopeCacheKey {
                    width: surface_config.width,
                    height: surface_config.height,
                    allocation: state.allocation_bounds,
                    deallocation: state.deallocation_bounds,
                };
                if self.envelope_cache_key != Some(cache_key) {
                    let rectangles = [
                        (state.allocation_bounds, [255, 0, 0, 255]),
                        (state.deallocation_bounds, [255, 255, 0, 255]),
                    ];
                    let upload = debug_overlay_upload_with_rectangles(
                        "",
                        surface_config.width,
                        surface_config.height,
                        &rectangles,
                    );
                    let key = upload.key;
                    self.envelope_texture = Some(upload_tile_texture(context, layout, &upload));
                    self.envelope_cache_key = Some(cache_key);
                    self.envelope_command = Some(TileDrawCommand {
                        texture: key,
                        position: crate::geometry::ScreenPoint::new(0, 0),
                        size: (upload.width, upload.height),
                    });
                }
                if let Some(command) = self.envelope_command {
                    let vertices = tile_vertices_for_commands(
                        &[command],
                        surface_config.width,
                        surface_config.height,
                    );
                    if let Some(buffer) = self.envelope_vertex_ring.ensure_buffer(
                        &context.device,
                        "gpu-allocation-envelope-vertices",
                        vertices.len(),
                    ) {
                        context
                            .queue
                            .write_buffer(buffer, 0, bytemuck::cast_slice(&vertices));
                    }
                }
            }
            self.record_gpu_upload_stage(
                crate::orchestrator::FrameEventKind::GpuBatchEnvelopeUpload,
                "upload do envelope GPU",
                envelope_started,
                format!(
                    "envelope habilitado, slot {}/{}",
                    self.envelope_vertex_ring.active_slot(),
                    self.envelope_vertex_ring.slot_count()
                ),
            );
        } else {
            self.envelope_texture = None;
            self.envelope_command = None;
            self.envelope_cache_key = None;
        }
    }
}

impl ApplicationHandler for GpuWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (window_width, window_height) = self
            .state
            .as_ref()
            .map(|state| (state.config.width as u32, state.config.height as u32))
            .unwrap_or((800, 600));
        let window = match event_loop.create_window(
            WindowAttributes::default()
                .with_title("FractalExplorer - GPU")
                .with_inner_size(LogicalSize::new(window_width, window_height)),
        ) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                crate::print_local!("Falha ao criar janela GPU: {error}");
                event_loop.exit();
                return;
            }
        };
        let gpu_backend = match self
            .state
            .as_ref()
            .map(|state| state.config.gpu_backend_kind())
        {
            Some(Ok(backend)) => backend,
            Some(Err(error)) => {
                crate::print_local!("Configuração de backend GPU inválida: {error}");
                event_loop.exit();
                return;
            }
            None => crate::config::GpuBackend::Auto,
        };
        let context = match pollster::block_on(GpuContext::initialize(gpu_backend)) {
            Ok(context) => context,
            Err(error) => {
                crate::print_local!("Falha ao inicializar GPU: {error}");
                event_loop.exit();
                return;
            }
        };
        let surface = match context.instance.create_surface(Arc::clone(&window)) {
            Ok(surface) => surface,
            Err(error) => {
                crate::print_local!("Falha ao criar superfície GPU: {error}");
                event_loop.exit();
                return;
            }
        };
        let capabilities = surface.get_capabilities(&context.adapter);
        let adapter_info = context.adapter.get_info();
        crate::print_local!(
            "GPU adapter: {:?} / {} ({:?}); modos de apresentacao: {:?}",
            adapter_info.backend,
            adapter_info.name,
            adapter_info.device_type,
            capabilities.present_modes
        );
        let Some(format) = capabilities.formats.first().copied() else {
            crate::print_local!("GPU não oferece formato de superfície compatível");
            event_loop.exit();
            return;
        };
        let Some(present_mode) = select_present_mode(&capabilities.present_modes) else {
            crate::print_local!("GPU não oferece modo de apresentação compatível");
            event_loop.exit();
            return;
        };
        crate::print_local!("Modo de apresentacao GPU selecionado: {:?}", present_mode);
        let Some(alpha_mode) = capabilities.alpha_modes.first().copied() else {
            crate::print_local!("GPU não oferece modo alpha compatível");
            event_loop.exit();
            return;
        };
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: surface_usage(capabilities.usages),
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&context.device, &config);
        let (pipeline, tile_bind_group_layout) = create_tile_pipeline(&context, format);
        self.context = Some(context);
        self.window = Some(window);
        self.surface = Some(surface);
        self.surface_config = Some(config);
        self.pipeline = Some(pipeline);
        self.tile_bind_group_layout = Some(tile_bind_group_layout);
        self.texture_store = Some(GpuTextureStore::new());
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let app_event = match &event {
            WindowEvent::CloseRequested => Some(AppEvent::CloseRequested),
            WindowEvent::Resized(size) => {
                Some(AppEvent::Resized(Viewport::new(size.width, size.height)))
            }
            WindowEvent::RedrawRequested => Some(AppEvent::RedrawRequested),
            _ => None,
        };
        if let Some(app_event) = app_event {
            let effects = self.app_controller.handle_event(app_event);
            if effects.contains(&AppEffect::Exit) {
                event_loop.exit();
                return;
            }
        }
        let input_events = self
            .state
            .as_mut()
            .map(|state| state.input_events_for_window_event(&event))
            .unwrap_or_default();
        for input_event in input_events {
            self.app_controller
                .handle_event(AppEvent::Input(input_event));
        }
        let pending_input = self.app_controller.take_input_events();
        if let Some(state) = &mut self.state {
            for input_event in pending_input {
                state.apply_input_event(input_event);
            }
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.resize_viewport(Viewport::new(size.width, size.height));
                }
                self.configure_surface(size.width, size.height)
            }
            WindowEvent::RedrawRequested => {
                let composition_parameters = self
                    .surface_config
                    .as_ref()
                    .map(|config| (config.format, config.width, config.height));
                if let (Some(context), Some((format, width, height))) =
                    (self.context.take(), composition_parameters)
                {
                    let layout = self
                        .tile_bind_group_layout
                        .take()
                        .expect("tile bind group layout must exist");
                    self.ensure_composition_texture(
                        &context.device,
                        &layout,
                        format,
                        width,
                        height,
                    );
                    self.tile_bind_group_layout = Some(layout);
                    self.context = Some(context);
                }
                if let Some(state) = &mut self.state {
                    state.canvas.record_frame_event(
                        crate::orchestrator::FrameEventKind::GpuRedrawReceived,
                        "evento RedrawRequested recebido",
                    );
                }
                if let (Some(context), Some(surface), Some(pipeline)) =
                    (&self.context, &self.surface, &self.pipeline)
                {
                    if let Some(state) = &mut self.state {
                        state.canvas.record_frame_event(
                            crate::orchestrator::FrameEventKind::GpuSurfaceAcquireStarted,
                            "aquisicao da superficie GPU iniciada",
                        );
                    }
                    let frame = match surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(frame)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                        wgpu::CurrentSurfaceTexture::Timeout
                        | wgpu::CurrentSurfaceTexture::Occluded => {
                            return;
                        }
                        wgpu::CurrentSurfaceTexture::Outdated
                        | wgpu::CurrentSurfaceTexture::Lost => {
                            if let Some(config) = &self.surface_config {
                                surface.configure(&context.device, config);
                            }
                            self.surface_initialized = false;
                            self.tile_vertex_ring.reset();
                            self.overlay_vertex_ring.reset();
                            self.envelope_vertex_ring.reset();
                            return;
                        }
                        wgpu::CurrentSurfaceTexture::Validation => {
                            crate::print_local!("Falha de validação ao obter frame GPU");
                            return;
                        }
                    };
                    if let Some(state) = &mut self.state {
                        state.canvas.record_frame_event(
                            crate::orchestrator::FrameEventKind::GpuSurfaceAcquireFinished,
                            "aquisicao da superficie GPU concluida",
                        );
                    }
                    {
                        let composition = self
                            .composition_texture
                            .as_ref()
                            .expect("composition texture must exist");
                        let mut encoder = context.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("gpu-clear"),
                            },
                        );
                        let preserve_previous_frame = self
                            .state
                            .as_ref()
                            .is_some_and(|state| state.config.preserve_previous_frame);
                        {
                            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("gpu-clear-pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &composition.view,
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: surface_load_op(
                                            preserve_previous_frame,
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
                            let mut pass = _pass;
                            pass.set_pipeline(pipeline);
                            if let Some(store) = &self.texture_store {
                                if let Some(vertex_buffer) = self.tile_vertex_ring.active_buffer() {
                                    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                                    for (index, command) in self.tile_commands.iter().enumerate() {
                                        if let Some(tile_texture) = store.get(&command.texture) {
                                            pass.set_bind_group(0, &tile_texture.bind_group, &[]);
                                            let start = (index * 6) as u32;
                                            pass.draw(start..start + 6, 0..1);
                                        }
                                    }
                                }
                                if let (Some(envelope), Some(command), Some(envelope_vertices)) = (
                                    &self.envelope_texture,
                                    self.envelope_command,
                                    self.envelope_vertex_ring.active_buffer(),
                                ) {
                                    pass.set_bind_group(0, &envelope.bind_group, &[]);
                                    pass.set_vertex_buffer(0, envelope_vertices.slice(..));
                                    pass.draw(0..6, 0..1);
                                    let _ = command;
                                }
                                if let (Some(overlay), Some(command), Some(overlay_vertices)) = (
                                    &self.overlay_texture,
                                    self.overlay_command,
                                    self.overlay_vertex_ring.active_buffer(),
                                ) {
                                    pass.set_bind_group(0, &overlay.bind_group, &[]);
                                    pass.set_vertex_buffer(0, overlay_vertices.slice(..));
                                    pass.draw(0..6, 0..1);
                                    let _ = command;
                                }
                            }
                        }
                        let surface_view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        {
                            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                            let mut pass = _pass;
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, &composition.bind_group, &[]);
                            pass.set_vertex_buffer(0, composition.present_vertex_buffer.slice(..));
                            pass.draw(0..6, 0..1);
                        }
                        let command_buffer = encoder.finish();
                        if let Some(state) = &mut self.state {
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuCommandEncodingFinished,
                                "encoder GPU finalizado",
                            );
                        }
                        context.queue.submit(Some(command_buffer));
                        if let Some(state) = &mut self.state {
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuCommandsSubmitted,
                                "comandos GPU submetidos",
                            );
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuDevicePollStarted,
                                "poll do device GPU iniciado",
                            );
                        }
                        let poll_result = context.device.poll(wgpu::PollType::Poll);
                        if let Some(state) = &mut self.state {
                            let description = match poll_result {
                                Ok(status) => format!("poll do device GPU concluido: {status:?}"),
                                Err(error) => format!("poll do device GPU falhou: {error}"),
                            };
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuDevicePollFinished,
                                &description,
                            );
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuPresentationStarted,
                                "apresentacao GPU iniciada",
                            );
                            state.canvas.record_frame_presentation_started();
                        }
                        context.queue.present(frame);
                        self.surface_initialized = true;
                        if let Some(state) = &mut self.state {
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuPresentationFinished,
                                "apresentacao GPU concluida",
                            );
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::BufferReadyForPresentation,
                                "buffer pronto para apresentacao",
                            );
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::OverlaysDrawn,
                                "overlays desenhados",
                            );
                            state.canvas.record_frame_presentation_finished();
                            state.canvas.finish_frame();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let prepared = self.state.as_mut().map(|state| {
            state.prepare_visible_batch();
            (state.prepared_batch.take(), state.prepared_frame.clone())
        });
        if let Some((Some(batch), Some(frame))) = prepared {
            if let Some(state) = &mut self.state {
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuBatchPreparationFinished,
                    "preparacao do batch GPU concluida",
                );
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuBatchUploadStarted,
                    "upload do batch GPU iniciado",
                );
            }
            self.upload_batch(batch, tile_commands_for_frame(&frame));
            if let Some(state) = &mut self.state {
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuBatchUploadFinished,
                    "upload do batch GPU concluido",
                );
            }
        }
        if let Some(window) = &self.window {
            if let Some(state) = &mut self.state {
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuRedrawRequested,
                    "request_redraw GPU disparado",
                );
            }
            window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        centered_bounds, format_gpu_frame_history, format_gpu_upload_stage, needs_batch_rebuild,
        next_vertex_buffer_slot, overlay_cache_key, overlay_needs_refresh, select_present_mode,
        vertex_buffer_capacity, vertex_buffer_needs_recreation, GpuFrameMetrics, PreparedTileBatch,
    };
    use crate::geometry::ScreenPoint;
    use crate::gpu::{TextureKey, TextureUpload, TileDrawCommand};
    use crate::render::Viewport;
    use std::time::Duration;

    #[test]
    fn frame_stats_describe_visible_tiles_and_new_uploads() {
        let batch = PreparedTileBatch {
            uploads: vec![TextureUpload {
                key: TextureKey {
                    tile: 1,
                    content_hash: 2,
                },
                width: 1,
                height: 1,
                rgba8: vec![1, 2, 3, 4],
            }],
            commands: vec![TileDrawCommand {
                texture: TextureKey {
                    tile: 1,
                    content_hash: 2,
                },
                position: ScreenPoint::new(0, 0),
                size: (1, 1),
            }],
        };

        assert_eq!(
            GpuFrameMetrics::from_batch(&batch, Duration::from_millis(3)).description(),
            "tiles rasterizados neste frame: 1, texturas novas neste frame: 1"
        );
        assert_eq!(
            GpuFrameMetrics::from_batch(&batch, Duration::ZERO).draw_calls(),
            1
        );
    }

    #[test]
    fn gpu_upload_stage_description_includes_duration_and_detail() {
        assert_eq!(
            format_gpu_upload_stage(
                "upload de texturas",
                Duration::from_micros(1_250),
                "3 atualizacoes",
            ),
            "upload de texturas: 1.250 ms (3 atualizacoes)"
        );
    }

    #[test]
    fn gpu_upload_stage_events_are_distinct() {
        let kinds = [
            crate::orchestrator::FrameEventKind::GpuBatchTextureUpload,
            crate::orchestrator::FrameEventKind::GpuBatchTextureRetention,
            crate::orchestrator::FrameEventKind::GpuBatchVertexUpload,
            crate::orchestrator::FrameEventKind::GpuBatchOverlayUpload,
            crate::orchestrator::FrameEventKind::GpuBatchEnvelopeUpload,
        ];
        assert_eq!(kinds.len(), 5);
    }

    #[test]
    fn overlay_cache_reuses_an_unchanged_image_and_refreshes_changed_content() {
        let first = TextureUpload {
            key: TextureKey {
                tile: usize::MAX,
                content_hash: 10,
            },
            width: 240,
            height: 16,
            rgba8: vec![0; 240 * 16 * 4],
        };
        let same = TextureUpload {
            key: TextureKey {
                tile: usize::MAX,
                content_hash: 10,
            },
            width: 240,
            height: 16,
            rgba8: vec![255; 240 * 16 * 4],
        };
        let changed = TextureUpload {
            key: TextureKey {
                tile: usize::MAX,
                content_hash: 11,
            },
            width: 240,
            height: 32,
            rgba8: vec![0; 240 * 32 * 4],
        };
        let first_key = overlay_cache_key(&first);

        assert!(overlay_needs_refresh(None, first_key));
        assert!(!overlay_needs_refresh(
            Some(first_key),
            overlay_cache_key(&same)
        ));
        assert!(overlay_needs_refresh(
            Some(first_key),
            overlay_cache_key(&changed)
        ));
    }

    #[test]
    fn resize_invalidates_vertices_only_when_surface_dimensions_change() {
        assert!(!needs_batch_rebuild((800, 600), (800, 600)));
        assert!(needs_batch_rebuild((800, 600), (1024, 768)));
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
    fn vertex_buffer_ring_rotates_slots_without_reusing_the_current_slot() {
        assert_eq!(next_vertex_buffer_slot(0, 3), 1);
        assert_eq!(next_vertex_buffer_slot(1, 3), 2);
        assert_eq!(next_vertex_buffer_slot(2, 3), 0);
        assert_eq!(next_vertex_buffer_slot(0, 0), 0);
    }

    #[test]
    fn surface_load_op_preserves_previous_pixels_only_after_initialization() {
        assert!(matches!(
            super::surface_load_op(false, false),
            wgpu::LoadOp::Clear(_)
        ));
        assert!(matches!(
            super::surface_load_op(true, false),
            wgpu::LoadOp::Clear(_)
        ));
        assert!(matches!(
            super::surface_load_op(true, true),
            wgpu::LoadOp::Load
        ));
    }

    #[test]
    fn surface_usage_does_not_request_unsupported_copy_destination() {
        let supported = wgpu::TextureUsages::RENDER_ATTACHMENT;

        assert_eq!(super::surface_usage(supported), supported);
    }

    #[test]
    fn gpu_state_resize_updates_the_logical_viewport_and_discards_prepared_frame() {
        let canvas = crate::TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-2.0, 1.0),
            8,
            8,
            0.01,
            ScreenPoint::new(0, 0),
            8.0,
            0.5,
        );
        let orchestrator = crate::Orchestrator::with_worker_count(crate::Mandelbrot::new(32), 1);
        let mut state = super::GpuAppState::new(
            canvas,
            orchestrator,
            crate::config::RendererConfig::default(),
        );
        state.prepare_visible_batch();

        state.resize_viewport(Viewport::new(1024, 768));

        assert_eq!(state.config.width, 1024);
        assert_eq!(state.config.height, 768);
        assert!(state.prepared_batch.is_none());
        assert!(state.prepared_frame.is_none());
        assert!(state.prepared_image_updates.is_empty());
    }

    #[test]
    fn gpu_state_publishes_a_completed_tile_after_workers_run() {
        let mut canvas = crate::TiledInfiniteCanvas::new(
            crate::geometry::ComplexPoint::new(-2.0, 1.0),
            8,
            8,
            0.01,
            ScreenPoint::new(0, 0),
            8.0,
            0.5,
        );
        canvas.set_initial_zoom(1.0);
        let orchestrator = crate::Orchestrator::with_worker_count(crate::Mandelbrot::new(32), 2);
        let mut state = super::GpuAppState::new(
            canvas,
            orchestrator,
            crate::config::RendererConfig::default(),
        );

        state.prepare_visible_batch();
        std::thread::sleep(Duration::from_millis(50));
        state.prepare_visible_batch();

        assert!(state
            .prepared_batch
            .as_ref()
            .is_some_and(|batch| !batch.commands.is_empty()));
        assert!(state
            .prepared_frame
            .as_ref()
            .is_some_and(|frame| !frame.tiles().is_empty()));
        assert!(!state.prepared_image_updates.is_empty());
    }

    #[test]
    fn frame_history_overlay_formats_one_line_per_ring_entry() {
        let history = std::collections::VecDeque::from([
            (7, Duration::from_micros(1_250)),
            (8, Duration::from_micros(2_500)),
        ]);
        assert_eq!(format_gpu_frame_history(&history), "#7:1.250ms\n#8:2.500ms");
    }

    #[test]
    fn selects_a_non_vsync_present_mode_when_the_adapter_supports_one() {
        assert_eq!(
            select_present_mode(&[wgpu::PresentMode::Fifo, wgpu::PresentMode::AutoNoVsync,]),
            Some(wgpu::PresentMode::AutoNoVsync)
        );
        assert_eq!(
            select_present_mode(&[wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate]),
            Some(wgpu::PresentMode::Immediate)
        );
        assert_eq!(
            select_present_mode(&[wgpu::PresentMode::Fifo]),
            Some(wgpu::PresentMode::Fifo)
        );
    }

    #[test]
    fn reduced_viewport_is_centered_and_scales_both_axes() {
        assert_eq!(centered_bounds(800, 600, 1.0), (0, 0, 799, 599));
        assert_eq!(centered_bounds(800, 600, 0.7), (120, 90, 679, 509));
    }
}

pub fn run_window() -> Result<(), winit::error::EventLoopError> {
    run_window_with_state(None)
}

pub fn run_window_with_state(
    state: Option<GpuAppState>,
) -> Result<(), winit::error::EventLoopError> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut GpuWindowApp::with_state(state))
}
