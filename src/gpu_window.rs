use crate::app::{
    prepare_canvas_common, reduce_effects, AppEvent, ApplicationController,
    DefaultApplicationController,
};
use crate::gpu::{
    debug_overlay_upload, debug_overlay_upload_with_rectangles, texture_keys_for_commands,
    tile_commands_for_frame, PreparedTileBatch, TextureCache, TileDrawCommand,
};
use crate::input::{InputEvent, ZoomDirection};
use crate::render::graphics::wgpu::{
    create_composition_texture, create_tile_pipeline, surface_load_op, tile_vertices_for_commands,
    upload_tile_texture, write_tile_texture, GpuTextureStore, GpuTileTexture, TileVertex,
    WgpuCompositionTexture, WgpuContext as GpuContext, WgpuPipeline, WgpuSurface,
    WgpuSurfaceAcquire, WgpuTextureLayout,
};
use crate::render::{ImageId, ImageRevision, ImageUpdate, PreparedFrame, Viewport};
#[cfg(test)]
use crate::render::{Rect, TileDraw};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

fn needs_batch_rebuild(previous: (u32, u32), next: (u32, u32)) -> bool {
    previous != next
}

fn uses_persistent_composition(preserve_previous_frame: bool) -> bool {
    preserve_previous_frame
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
                size: (capacity * std::mem::size_of::<TileVertex>()) as u64,
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

fn format_gpu_frame_overlay_header(frame_number: u64, visible_tiles: usize) -> String {
    format!("Frame #{frame_number}, tiles:{visible_tiles}")
}

fn format_gpu_overlay_text(state: &GpuAppState, visible_tiles: usize) -> String {
    let debug = &state.config.debug;
    let mut lines = Vec::new();
    let frame_number = state.canvas.current_frame_number();
    let common = crate::app::prepare_overlay_snapshot(
        &state.canvas,
        &state.orchestrator,
        &state.config,
        Some(state.cursor),
    );

    if debug.text_overlay_frames {
        lines.push(format_gpu_frame_overlay_header(frame_number, visible_tiles));
        let history = format_gpu_frame_history(&state.frame_timing_ring);
        if !history.is_empty() {
            lines.extend(history.lines().map(str::to_owned));
        }
    }

    if debug.text_overlay_layers {
        lines.extend(common.layer_lines);
    }

    if debug.text_overlay_queue {
        lines.extend(common.queue_lines);
    }

    if debug.text_overlay_workers {
        lines.extend(common.worker_lines);
    }

    lines.join("\n")
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

fn format_gpu_event_loop_wait(elapsed: Duration) -> String {
    format_gpu_upload_stage("espera do event loop GPU", elapsed, "fora do renderer")
}

fn format_gpu_redraw_latency(elapsed: Duration) -> String {
    format_gpu_upload_stage(
        "latencia entre request_redraw e RedrawRequested",
        elapsed,
        "event loop",
    )
    .replace(" (event loop)", "")
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

#[cfg(test)]
fn frame_tiles_from_batch(batch: &PreparedTileBatch) -> Vec<TileDraw> {
    batch
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
        .collect()
}

pub struct GpuAppState {
    pub canvas: crate::TiledInfiniteCanvas,
    pub orchestrator: crate::Orchestrator,
    pub config: crate::config::RendererConfig,
    pub prepared_batch: Option<PreparedTileBatch>,
    pub prepared_frame: Option<PreparedFrame>,
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
    }

    pub fn apply_config(
        &mut self,
        next_config: crate::config::RendererConfig,
    ) -> Result<(), String> {
        let render_plan = crate::PrecisionDecisionManager::from_config(&next_config)
            .map_err(|_| "invalid renderer precision configuration".to_string())?;
        let viewport_changed =
            (self.config.width, self.config.height) != (next_config.width, next_config.height);
        self.config = next_config;
        self.canvas
            .set_frame_dump_events(self.config.debug.frame_dump_events.clone());
        self.canvas
            .set_slow_frame_threshold_ms(self.config.debug.slow_frame_threshold_ms);
        self.orchestrator.set_render_plan(render_plan);
        self.canvas.set_render_plan(render_plan);
        self.canvas.invalidate_tiles();
        if viewport_changed {
            self.resize_viewport(Viewport::new(
                self.config.width as u32,
                self.config.height as u32,
            ));
        } else {
            self.prepared_batch = None;
            self.prepared_frame = None;
        }
        Ok(())
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
        crate::app::apply_canvas_input(
            &mut self.canvas,
            event,
            self.config.zoom_multiplier,
            self.config.max_apparent_pixel_size(),
        );
    }

    fn refresh_allocation_bounds(&mut self) {
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
    }

    pub fn prepare_visible_batch(&mut self) {
        let mut controller = DefaultApplicationController::new(Viewport::new(
            self.config.width as u32,
            self.config.height as u32,
        ));
        self.prepare_visible_batch_with_controller(&mut controller);
    }

    fn prepare_visible_batch_with_controller(
        &mut self,
        controller: &mut DefaultApplicationController,
    ) {
        self.refresh_allocation_bounds();
        prepare_canvas_common(
            &mut self.canvas,
            &self.orchestrator,
            self.allocation_bounds,
            self.deallocation_bounds,
        );
        self.prepare_visible_batch_after_canvas(controller);
    }

    fn prepare_visible_batch_after_canvas(
        &mut self,
        controller: &mut DefaultApplicationController,
    ) {
        let preparation_started = Instant::now();
        if let Some(timing) = self.canvas.last_finished_frame_timing() {
            self.frame_timing_ring.push_back(timing);
            while self.frame_timing_ring.len() > GPU_FRAME_HISTORY_CAPACITY {
                self.frame_timing_ring.pop_front();
            }
        }
        let tiles = crate::app::collect_completed_canvas_tiles(&self.canvas, &self.config);
        let references: Vec<_> = tiles
            .iter()
            .map(|prepared| (&prepared.tile_sprite, Arc::clone(&prepared.sprite)))
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
        let frame_tiles = tiles.iter().map(|prepared| prepared.draw.clone()).collect();
        let image_updates = batch
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
        let frame = controller.build_prepared_frame(
            Viewport::new(self.config.width as u32, self.config.height as u32),
            frame_tiles,
            Vec::new(),
            image_updates,
        );
        self.prepared_frame = Some(frame);
        self.prepared_batch = Some(batch);
    }
}

pub struct GpuWindowApp {
    pub state: Option<GpuAppState>,
    app_controller: DefaultApplicationController,
    config_updates: Option<std::sync::mpsc::Receiver<crate::config::RendererConfig>>,
    renderer_closed: Option<Arc<std::sync::atomic::AtomicBool>>,
    context: Option<GpuContext>,
    window: Option<Arc<Window>>,
    surface: Option<WgpuSurface<'static>>,
    pipeline: Option<WgpuPipeline>,
    tile_bind_group_layout: Option<WgpuTextureLayout>,
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
    composition_texture: Option<WgpuCompositionTexture>,
    last_frame_finished_at: Option<Instant>,
    last_redraw_requested_at: Option<Instant>,
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
            config_updates: None,
            renderer_closed: None,
            context: None,
            window: None,
            surface: None,
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
            last_frame_finished_at: None,
            last_redraw_requested_at: None,
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
        self.last_frame_finished_at = None;
        self.last_redraw_requested_at = None;
    }

    pub fn with_state_and_config_updates(
        state: Option<GpuAppState>,
        config_updates: std::sync::mpsc::Receiver<crate::config::RendererConfig>,
        renderer_closed: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let mut app = Self::with_state(state);
        app.config_updates = Some(config_updates);
        app.renderer_closed = Some(renderer_closed);
        app
    }

    fn apply_pending_config_updates(&mut self) {
        let Some(receiver) = &self.config_updates else {
            return;
        };
        while let Ok(config) = receiver.try_recv() {
            if let Some(state) = &mut self.state {
                if let Err(error) = state.apply_config(config) {
                    crate::print_local!("Aviso: configuraÃ§Ã£o GPU ignorada: {error}");
                    continue;
                }
                let actions = reduce_effects(
                    &self
                        .app_controller
                        .handle_event(AppEvent::ConfigurationChanged),
                );
                if actions.request_redraw {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
        }
    }

    fn configure_surface(&mut self, width: u32, height: u32) {
        let (Some(context), Some(surface)) = (&self.context, &mut self.surface) else {
            return;
        };
        let previous = surface.size();
        surface.resize(context, width, height);
        if needs_batch_rebuild(previous, surface.size()) {
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
        context: &GpuContext,
        layout: &WgpuTextureLayout,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let needs_recreation = self
            .composition_texture
            .as_ref()
            .is_none_or(|texture| texture.size() != (width, height));
        if needs_recreation {
            self.composition_texture = Some(create_composition_texture(
                context, layout, format, width, height,
            ));
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
            .and_then(|state| state.prepared_frame.as_ref())
            .map(|prepared| prepared.image_updates().to_vec())
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
        if let (Some(context), Some(surface)) = (&self.context, &self.surface) {
            let (width, height) = surface.size();
            let vertices = tile_vertices_for_commands(&frame_commands, width, height);
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
        let show_text_overlay = self.state.as_ref().is_some_and(|state| {
            let debug = &state.config.debug;
            debug.overlays_enabled()
                && (debug.text_overlay_frames
                    || debug.text_overlay_layers
                    || debug.text_overlay_queue
                    || debug.text_overlay_workers)
        });
        let show_envelope = self
            .state
            .as_ref()
            .is_some_and(|state| state.config.debug.should_show_allocation_envelope());
        if show_text_overlay {
            let overlay_started = Instant::now();
            let text = self
                .state
                .as_ref()
                .map(|state| format_gpu_overlay_text(state, self.tile_commands.len()))
                .unwrap_or_default();
            let line_count = text.lines().count().max(1);
            let upload = debug_overlay_upload(&text, 240, line_count as u32 * 16);
            let key = upload.key;
            if let (Some(context), Some(layout), Some(surface)) =
                (&self.context, &self.tile_bind_group_layout, &self.surface)
            {
                let (width, height) = surface.size();
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
                    height.saturating_sub(upload.height + 8) as i32,
                );
                let command = TileDrawCommand {
                    texture: key,
                    position,
                    size: (upload.width, upload.height),
                };
                let vertices = tile_vertices_for_commands(&[command], width, height);
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
            if let (Some(context), Some(layout), Some(surface), Some(state)) = (
                &self.context,
                &self.tile_bind_group_layout,
                &self.surface,
                &self.state,
            ) {
                let (width, height) = surface.size();
                let cache_key = EnvelopeCacheKey {
                    width,
                    height,
                    allocation: state.allocation_bounds,
                    deallocation: state.deallocation_bounds,
                };
                if self.envelope_cache_key != Some(cache_key) {
                    let rectangles = [
                        (state.allocation_bounds, [255, 0, 0, 255]),
                        (state.deallocation_bounds, [255, 255, 0, 255]),
                    ];
                    let upload =
                        debug_overlay_upload_with_rectangles("", width, height, &rectangles);
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
                    let vertices = tile_vertices_for_commands(&[command], width, height);
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
                .with_title(crate::app::window_title())
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
        let size = window.inner_size();
        let surface = match context.create_surface(Arc::clone(&window), size.width, size.height) {
            Ok(surface) => surface,
            Err(error) => {
                crate::print_local!("Falha ao criar superfície GPU: {error}");
                event_loop.exit();
                return;
            }
        };
        let adapter_info = context.adapter.get_info();
        crate::print_local!(
            "GPU adapter: {:?} / {} ({:?}); modo de apresentacao: {:?}",
            adapter_info.backend,
            adapter_info.name,
            adapter_info.device_type,
            surface.present_mode()
        );
        let (pipeline, tile_bind_group_layout) = create_tile_pipeline(&context, surface.format());
        self.context = Some(context);
        self.window = Some(window);
        self.surface = Some(surface);
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
        let mut render_requested = false;
        if matches!(&event, WindowEvent::CloseRequested) {
            signal_renderer_closed(self.renderer_closed.as_ref());
        }
        let app_event = app_event_from_window_event(&event);
        if let Some(app_event) = app_event {
            let actions = reduce_effects(&self.app_controller.handle_event(app_event));
            render_requested = actions.render;
            if actions.exit {
                event_loop.exit();
                return;
            }
            if actions.request_redraw {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
        let input_events = self
            .state
            .as_mut()
            .map(|state| state.input_events_for_window_event(&event))
            .unwrap_or_default();
        for input_event in input_events {
            let actions = reduce_effects(
                &self
                    .app_controller
                    .handle_event(AppEvent::Input(input_event)),
            );
            if actions.request_redraw {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
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
            WindowEvent::RedrawRequested if render_requested => {
                let preserve_previous_frame = self
                    .state
                    .as_ref()
                    .is_some_and(|state| state.config.preserve_previous_frame);
                let composition_parameters = self.surface.as_ref().map(|surface| {
                    let (width, height) = surface.size();
                    (surface.format(), width, height)
                });
                if let (Some(context), Some((format, width, height))) =
                    (self.context.take(), composition_parameters)
                {
                    if uses_persistent_composition(preserve_previous_frame) {
                        let layout = self
                            .tile_bind_group_layout
                            .take()
                            .expect("tile bind group layout must exist");
                        self.ensure_composition_texture(&context, &layout, format, width, height);
                        self.tile_bind_group_layout = Some(layout);
                    }
                    self.context = Some(context);
                }
                if let Some(state) = &mut self.state {
                    state.canvas.record_frame_event(
                        crate::orchestrator::FrameEventKind::GpuRedrawReceived,
                        "evento RedrawRequested recebido",
                    );
                    if let Some(requested_at) = self.last_redraw_requested_at.take() {
                        state.canvas.record_frame_event(
                            crate::orchestrator::FrameEventKind::GpuRedrawLatency,
                            format_gpu_redraw_latency(requested_at.elapsed()),
                        );
                    }
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
                    let frame = match surface.acquire() {
                        WgpuSurfaceAcquire::Ready(frame)
                        | WgpuSurfaceAcquire::Suboptimal(frame) => frame,
                        WgpuSurfaceAcquire::Timeout | WgpuSurfaceAcquire::Occluded => {
                            return;
                        }
                        WgpuSurfaceAcquire::Outdated | WgpuSurfaceAcquire::Lost => {
                            surface.configure(context);
                            self.surface_initialized = false;
                            self.tile_vertex_ring.reset();
                            self.overlay_vertex_ring.reset();
                            self.envelope_vertex_ring.reset();
                            return;
                        }
                        WgpuSurfaceAcquire::Validation => {
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
                        let composition = if uses_persistent_composition(preserve_previous_frame) {
                            Some(
                                self.composition_texture
                                    .as_ref()
                                    .expect("composition texture must exist"),
                            )
                        } else {
                            None
                        };
                        let surface_view = frame.create_view();
                        let mut encoder = context.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("gpu-clear"),
                            },
                        );
                        let composition_started = Instant::now();
                        if let Some(state) = &mut self.state {
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuCompositionPassStarted,
                                "passe de composicao GPU iniciado",
                            );
                        }
                        {
                            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("gpu-clear-pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: composition
                                        .map(WgpuCompositionTexture::view)
                                        .unwrap_or(&surface_view),
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
                            pass.set_pipeline(&pipeline.0);
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
                        if let Some(state) = &mut self.state {
                            state.canvas.record_frame_event(
                                crate::orchestrator::FrameEventKind::GpuCompositionPassFinished,
                                format_gpu_upload_stage(
                                    "passe de composicao GPU concluido",
                                    composition_started.elapsed(),
                                    "codificacao do render pass",
                                ),
                            );
                        }
                        if let Some(composition) = composition {
                            let present_pass_started = Instant::now();
                            if let Some(state) = &mut self.state {
                                state.canvas.record_frame_event(
                                    crate::orchestrator::FrameEventKind::GpuSurfacePresentPassStarted,
                                    "passe de apresentacao da superficie GPU iniciado",
                                );
                            }
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
                            pass.set_pipeline(&pipeline.0);
                            pass.set_bind_group(0, composition.bind_group(), &[]);
                            pass.set_vertex_buffer(
                                0,
                                composition.present_vertex_buffer().slice(..),
                            );
                            pass.draw(0..6, 0..1);
                            if let Some(state) = &mut self.state {
                                state.canvas.record_frame_event(
                                    crate::orchestrator::FrameEventKind::GpuSurfacePresentPassFinished,
                                    format_gpu_upload_stage(
                                        "passe de apresentacao da superficie GPU concluido",
                                        present_pass_started.elapsed(),
                                        "codificacao do render pass",
                                    ),
                                );
                            }
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
                        frame.present(context);
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
                            self.last_frame_finished_at = Some(Instant::now());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.apply_pending_config_updates();
        let actions = reduce_effects(&self.app_controller.handle_event(AppEvent::AboutToWait));
        if actions.exit {
            event_loop.exit();
            return;
        }
        if !actions.prepare_frame {
            return;
        }
        let event_loop_wait = self
            .last_frame_finished_at
            .take()
            .map(|finished_at| finished_at.elapsed());
        if let (Some(state), Some(elapsed)) = (&mut self.state, event_loop_wait) {
            state.canvas.record_frame_event(
                crate::orchestrator::FrameEventKind::GpuEventLoopWait,
                format_gpu_event_loop_wait(elapsed),
            );
        }
        let prepared = {
            let (state, controller) = (&mut self.state, &mut self.app_controller);
            state.as_mut().map(|state| {
                state.refresh_allocation_bounds();
                controller.prepare_canvas(
                    &mut state.canvas,
                    &state.orchestrator,
                    state.allocation_bounds,
                    state.deallocation_bounds,
                );
                controller.begin_tile_composition(&mut state.canvas);
                state.prepare_visible_batch_after_canvas(controller);
                (state.prepared_batch.take(), state.prepared_frame.clone())
            })
        };
        if let Some((Some(batch), Some(frame))) = prepared {
            let frame = self
                .app_controller
                .publish_and_prepare_frame(frame)
                .expect("GPU controller should prepare a published frame");
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
            self.upload_batch(batch, tile_commands_for_frame(frame.frame()));
            if let Some(state) = &mut self.state {
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuBatchUploadFinished,
                    "upload do batch GPU concluido",
                );
            }
        }
        if actions.request_redraw {
            if let Some(window) = &self.window {
                self.last_redraw_requested_at = Some(Instant::now());
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
}

fn app_event_from_window_event(event: &WindowEvent) -> Option<AppEvent> {
    match event {
        WindowEvent::CloseRequested => Some(AppEvent::CloseRequested),
        WindowEvent::Resized(size) => {
            Some(AppEvent::Resized(Viewport::new(size.width, size.height)))
        }
        WindowEvent::RedrawRequested => Some(AppEvent::RedrawRequested),
        _ => None,
    }
}

fn signal_renderer_closed(renderer_closed: Option<&Arc<std::sync::atomic::AtomicBool>>) {
    if let Some(renderer_closed) = renderer_closed {
        renderer_closed.store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        app_event_from_window_event, centered_bounds, format_gpu_event_loop_wait,
        format_gpu_frame_history, format_gpu_frame_overlay_header, format_gpu_redraw_latency,
        format_gpu_upload_stage, frame_tiles_from_batch, needs_batch_rebuild,
        next_vertex_buffer_slot, overlay_cache_key, overlay_needs_refresh, signal_renderer_closed,
        vertex_buffer_capacity, vertex_buffer_needs_recreation, GpuFrameMetrics, PreparedTileBatch,
    };
    use crate::app::ApplicationController;
    use crate::geometry::ScreenPoint;
    use crate::gpu::{TextureKey, TextureUpload, TileDrawCommand};
    use crate::render::{ImageId, ImageRevision, Rect, Viewport};
    use std::sync::Arc;
    use std::time::Duration;
    use winit::dpi::PhysicalSize;
    use winit::event::WindowEvent;

    #[test]
    fn signals_renderer_closed_without_requiring_a_window() {
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));

        signal_renderer_closed(Some(&closed));

        assert!(closed.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn fake_runtime_close_signals_config_ui_and_exits_controller() {
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut controller = crate::app::DefaultApplicationController::new(Viewport::new(320, 200));
        let event = WindowEvent::CloseRequested;

        signal_renderer_closed(Some(&closed));
        let app_event = app_event_from_window_event(&event).unwrap();
        let actions = crate::app::reduce_effects(&controller.handle_event(app_event));

        assert!(closed.load(std::sync::atomic::Ordering::Acquire));
        assert!(actions.exit);
        assert!(controller
            .handle_event(crate::app::AppEvent::AboutToWait)
            .is_empty());
    }

    #[test]
    fn formats_event_loop_wait_and_redraw_latency_separately() {
        assert_eq!(
            format_gpu_event_loop_wait(Duration::from_millis(6019)),
            "espera do event loop GPU: 6019.000 ms (fora do renderer)"
        );
        assert_eq!(
            format_gpu_redraw_latency(Duration::from_micros(570)),
            "latencia entre request_redraw e RedrawRequested: 0.570 ms"
        );
    }

    #[test]
    fn translates_window_lifecycle_events_to_application_events() {
        assert_eq!(
            app_event_from_window_event(&WindowEvent::Resized(PhysicalSize::new(640, 480))),
            Some(crate::app::AppEvent::Resized(Viewport::new(640, 480)))
        );
        assert_eq!(
            app_event_from_window_event(&WindowEvent::RedrawRequested),
            Some(crate::app::AppEvent::RedrawRequested)
        );
        assert_eq!(
            app_event_from_window_event(&WindowEvent::CloseRequested),
            Some(crate::app::AppEvent::CloseRequested)
        );
        assert_eq!(
            app_event_from_window_event(&WindowEvent::Occluded(false)),
            None
        );
    }

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
    fn prepared_batch_golden_data_preserves_image_identity_order_and_destination() {
        let batch = PreparedTileBatch {
            uploads: Vec::new(),
            commands: vec![
                TileDrawCommand {
                    texture: TextureKey {
                        tile: 17,
                        content_hash: 101,
                    },
                    position: ScreenPoint::new(4, 6),
                    size: (20, 10),
                },
                TileDrawCommand {
                    texture: TextureKey {
                        tile: 23,
                        content_hash: 202,
                    },
                    position: ScreenPoint::new(31, 9),
                    size: (8, 12),
                },
            ],
        };

        assert_eq!(
            frame_tiles_from_batch(&batch),
            vec![
                crate::render::TileDraw::new(ImageId::new(17), ImageRevision::new(101), 0)
                    .with_destination(Rect::new(4, 6, 20, 10)),
                crate::render::TileDraw::new(ImageId::new(23), ImageRevision::new(202), 1)
                    .with_destination(Rect::new(31, 9, 8, 12)),
            ]
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
    fn persistent_composition_is_used_only_when_frame_preservation_is_enabled() {
        assert!(super::uses_persistent_composition(true));
        assert!(!super::uses_persistent_composition(false));
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
    }

    #[test]
    fn gpu_state_applies_renderer_configuration_through_one_entry_point() {
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
        let mut next = state.config.clone();
        next.width = 1024;
        next.height = 768;
        next.palette_period = 7.0;

        state.apply_config(next.clone()).unwrap();

        assert_eq!(state.config.width, next.width);
        assert_eq!(state.config.height, next.height);
        assert_eq!(state.config.palette_period, next.palette_period);
        assert!(state.prepared_batch.is_none());
        assert!(state.prepared_frame.is_none());
    }

    #[test]
    fn gpu_runtime_drains_configuration_updates_without_window_access() {
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
        let initial = crate::config::RendererConfig::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = super::GpuWindowApp::with_state_and_config_updates(
            Some(super::GpuAppState::new(
                canvas,
                orchestrator,
                initial.clone(),
            )),
            receiver,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        let mut next = initial;
        next.palette_period = 9.0;
        sender.send(next).unwrap();

        app.apply_pending_config_updates();

        assert_eq!(app.state.as_ref().unwrap().config.palette_period, 9.0);
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
            .is_some_and(|prepared| !prepared.frame().tiles().is_empty()));
        assert!(state
            .prepared_frame
            .as_ref()
            .is_some_and(|prepared| !prepared.image_updates().is_empty()));
        let batch = state.prepared_batch.as_ref().unwrap();
        let frame_tile = &state.prepared_frame.as_ref().unwrap().frame().tiles()[0];
        assert_eq!(
            frame_tile.image().value(),
            batch.commands[0].texture.tile as u64
        );
        assert_eq!(
            frame_tile.revision().value(),
            batch.commands[0].texture.content_hash
        );
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
    fn frame_overlay_header_uses_frame_number_instead_of_tile_count() {
        assert_eq!(
            format_gpu_frame_overlay_header(1530, 84),
            "Frame #1530, tiles:84"
        );
    }

    #[test]
    fn gpu_text_overlay_includes_each_enabled_overlay_group() {
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

        let text = super::format_gpu_overlay_text(&state, 84);

        assert!(text.contains("Frame #1, tiles:84"));
        assert!(text.contains("Camada 0:"));
        assert!(text.contains("Worker 0:"));
    }

    #[test]
    fn reduced_viewport_is_centered_and_scales_both_axes() {
        assert_eq!(centered_bounds(800, 600, 1.0), (0, 0, 799, 599));
        assert_eq!(centered_bounds(800, 600, 0.7), (120, 90, 679, 509));
    }

    #[test]
    fn runtime_channels_connect_config_updates_and_shutdown_signal() {
        let (sender, receiver, renderer_closed) = super::runtime_channels();
        let config = crate::config::RendererConfig::default();
        sender.send(config.clone()).unwrap();

        assert_eq!(receiver.recv().unwrap(), config);
        assert!(!renderer_closed.load(std::sync::atomic::Ordering::Acquire));
        renderer_closed.store(true, std::sync::atomic::Ordering::Release);
        assert!(renderer_closed.load(std::sync::atomic::Ordering::Acquire));
    }
}

pub fn run_window() -> Result<(), winit::error::EventLoopError> {
    run_window_with_state(None)
}

/// Creates the channels shared by the renderer window and the configuration UI.
pub fn runtime_channels() -> (
    std::sync::mpsc::Sender<crate::config::RendererConfig>,
    std::sync::mpsc::Receiver<crate::config::RendererConfig>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let renderer_closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    (sender, receiver, renderer_closed)
}

pub fn run_window_with_state(
    state: Option<GpuAppState>,
) -> Result<(), winit::error::EventLoopError> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut GpuWindowApp::with_state(state))
}

pub fn run_window_with_state_and_updates(
    state: Option<GpuAppState>,
    receiver: std::sync::mpsc::Receiver<crate::config::RendererConfig>,
    renderer_closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), winit::error::EventLoopError> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut GpuWindowApp::with_state_and_config_updates(
        state,
        receiver,
        renderer_closed,
    ))
}
