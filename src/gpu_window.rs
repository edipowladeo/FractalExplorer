use crate::app::{
    prepare_canvas_common, reduce_effects, AppEvent, ApplicationController,
    DefaultApplicationController,
};
use crate::geometry::ScreenPoint;
use crate::gpu::{
    debug_overlay_upload_with_rectangles, PreparedTileBatch, TextureCache, TextureKey,
    TextureUpload,
};
use crate::input::{InputEvent, ZoomDirection};
use crate::render::gpu::GpuRenderTarget;
use crate::render::graphics::wgpu::{
    WgpuContext as GpuContext, WgpuGraphicsDevice, WgpuSubmissionMetrics, WgpuSurface,
};
use crate::render::{
    ImageId, ImageRevision, ImageUpdate, OverlayPrimitive, PreparedFrame, Rect,
    RenderTargetSession, TextRun, TileDraw, Viewport,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

const GPU_FRAME_HISTORY_CAPACITY: usize = 8;
const GPU_ENVELOPE_IMAGE_ID: ImageId = ImageId::new(u64::MAX - 1);
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

fn format_gpu_preparation_stage(label: &str, elapsed: Duration) -> String {
    format!(
        "{label}: {:.3} ms (preparacao do frame GPU)",
        elapsed.as_secs_f64() * 1_000.0
    )
}

fn format_gpu_preparation_stage_with_detail(
    label: &str,
    elapsed: Duration,
    detail: impl std::fmt::Display,
) -> String {
    format!(
        "{label}: {:.3} ms ({detail}; preparacao do frame GPU)",
        elapsed.as_secs_f64() * 1_000.0
    )
}

fn append_image_overlay(
    prepared: &PreparedFrame,
    image: ImageId,
    upload: TextureUpload,
    destination: Rect,
) -> Result<PreparedFrame, crate::render::ImageUpdateError> {
    let revision = ImageRevision::new(upload.key.content_hash);
    let update = ImageUpdate::new(image, revision, upload.width, upload.height, upload.rgba8)?;
    let frame = append_image_overlay_reference(prepared, image, revision, destination);
    let mut updates = prepared.image_updates().to_vec();
    updates.push(update);
    Ok(PreparedFrame::new(frame, updates))
}

fn append_image_overlay_reference(
    prepared: &PreparedFrame,
    image: ImageId,
    revision: ImageRevision,
    destination: Rect,
) -> crate::render::RenderFrame {
    prepared
        .frame()
        .clone()
        .with_overlay(OverlayPrimitive::Image(
            TileDraw::new(image, revision, 0).with_destination(destination),
        ))
}

fn append_text_overlay_line(
    prepared: &PreparedFrame,
    text: &str,
    x: i32,
    y: i32,
) -> PreparedFrame {
    let frame = prepared
        .frame()
        .clone()
        .with_overlay(OverlayPrimitive::Text(TextRun::new(
            text,
            ScreenPoint::new(x, y),
        )));
    PreparedFrame::new(frame, prepared.image_updates().to_vec())
}

fn submit_prepared_frame<T: crate::render::RenderTarget>(
    target: &mut RenderTargetSession<T>,
    prepared: &PreparedFrame,
) -> Result<crate::render::FrameOutcome, crate::render::RenderError> {
    target.submit(
        prepared.frame().viewport(),
        prepared.image_updates(),
        prepared.frame(),
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
    cached_envelope: Option<CachedEnvelope>,
    cursor: crate::geometry::ScreenPoint,
    left_button_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnvelopeGeometry {
    width: u32,
    height: u32,
    allocation_bounds: (i32, i32, i32, i32),
    deallocation_bounds: (i32, i32, i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedEnvelope {
    geometry: EnvelopeGeometry,
    texture: TextureKey,
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
            cached_envelope: None,
            cursor: crate::geometry::ScreenPoint::new(0, 0),
            left_button_down: false,
        }
    }

    pub fn resize_viewport(&mut self, viewport: Viewport) {
        self.config.width = viewport.width().max(1) as usize;
        self.config.height = viewport.height().max(1) as usize;
        self.prepared_batch = None;
        self.prepared_frame = None;
        self.cached_envelope = None;
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
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::GpuTilesCollected,
            format_gpu_preparation_stage(
                "coleta de tiles concluida",
                preparation_started.elapsed(),
            ),
        );
        let references: Vec<_> = tiles
            .iter()
            .map(|prepared| (&prepared.tile_sprite, Arc::clone(&prepared.sprite)))
            .collect();
        let batch = crate::gpu::prepare_tile_batch(&mut self.texture_cache, &references);
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::GpuBatchBuilt,
            format_gpu_preparation_stage(
                "montagem do batch concluida",
                preparation_started.elapsed(),
            ),
        );
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::TilesRasterized,
            GpuFrameMetrics::from_batch(&batch, preparation_started.elapsed()).description(),
        );
        self.last_frame_metrics = Some(GpuFrameMetrics::from_batch(
            &batch,
            preparation_started.elapsed(),
        ));
        let frame_tiles = tiles.iter().map(|prepared| prepared.draw.clone()).collect();
        let graphic_textures_started = Instant::now();
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
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::GpuGraphicTexturesFinished,
            format_gpu_preparation_stage_with_detail(
                "texturas graficas de tiles concluidas",
                graphic_textures_started.elapsed(),
                format!("uploads={}", batch.uploads.len()),
            ),
        );
        let frame = controller.build_prepared_frame(
            Viewport::new(self.config.width as u32, self.config.height as u32),
            frame_tiles,
            Vec::new(),
            image_updates,
        );
        self.prepared_frame = Some(self.append_debug_overlays(frame));
        self.prepared_batch = Some(batch);
    }

    fn append_debug_overlays(&mut self, mut prepared: PreparedFrame) -> PreparedFrame {
        let debug = &self.config.debug;
        let show_envelope = debug.should_show_allocation_envelope();
        let show_text = debug.overlays_enabled()
            && (debug.text_overlay_frames
                || debug.text_overlay_layers
                || debug.text_overlay_queue
                || debug.text_overlay_workers);
        if !show_envelope && !show_text {
            return prepared;
        }
        let viewport = prepared.frame().viewport();
        let width = viewport.width();
        let height = viewport.height();
        if width == 0 || height == 0 {
            return prepared;
        }

        let overlays_started = Instant::now();
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::GpuOverlaysStarted,
            "construcao dos overlays GPU iniciada",
        );

        if show_envelope {
            let envelope_started = Instant::now();
            let geometry = EnvelopeGeometry {
                width,
                height,
                allocation_bounds: self.allocation_bounds,
                deallocation_bounds: self.deallocation_bounds,
            };
            let rectangles = [
                (self.allocation_bounds, [255, 0, 0, 255]),
                (self.deallocation_bounds, [255, 255, 0, 255]),
            ];
            let destination = Rect::new(0, 0, width, height);
            if let Some(cached) = self
                .cached_envelope
                .filter(|cached| cached.geometry == geometry)
            {
                let frame = append_image_overlay_reference(
                    &prepared,
                    GPU_ENVELOPE_IMAGE_ID,
                    ImageRevision::new(cached.texture.content_hash),
                    destination,
                );
                prepared = PreparedFrame::new(frame, prepared.image_updates().to_vec());
            } else {
                let upload =
                    debug_overlay_upload_with_rectangles("", width, height, &rectangles);
                let texture = upload.key;
                if let Ok(with_envelope) = append_image_overlay(
                    &prepared,
                    GPU_ENVELOPE_IMAGE_ID,
                    upload,
                    destination,
                ) {
                    prepared = with_envelope;
                    self.cached_envelope = Some(CachedEnvelope { geometry, texture });
                }
            }
            self.canvas.record_frame_event(
                crate::orchestrator::FrameEventKind::GpuEnvelopeOverlayFinished,
                format_gpu_preparation_stage(
                    "overlay de envelope GPU concluido",
                    envelope_started.elapsed(),
                ),
            );
        }

        if show_text {
            let text_started = Instant::now();
            let snapshot = crate::app::prepare_overlay_snapshot(
                &self.canvas,
                &self.orchestrator,
                &self.config,
                Some(self.cursor),
            );
            if debug.text_overlay_layers && !snapshot.layer_lines.is_empty() {
                let layer_started = Instant::now();
                let first_y =
                    height.saturating_sub(24 + snapshot.layer_lines.len() as u32 * 8 + 4) as i32;
                for (line, text) in snapshot.layer_lines.iter().enumerate() {
                    prepared =
                        append_text_overlay_line(&prepared, text, 8, first_y + line as i32 * 8);
                }
                self.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuLayerOverlayFinished,
                    format_gpu_preparation_stage_with_detail(
                        "overlay de camadas concluido",
                        layer_started.elapsed(),
                        format!("linhas={}", snapshot.layer_lines.len()),
                    ),
                );
            }
            if debug.text_overlay_queue {
                let queue_started = Instant::now();
                for (line, text) in snapshot.queue_lines.iter().enumerate() {
                    let x = width.saturating_sub(text.chars().count() as u32 * 6 + 8) as i32;
                    prepared = append_text_overlay_line(&prepared, text, x, 8 + line as i32 * 8);
                }
                self.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuQueueOverlayFinished,
                    format_gpu_preparation_stage_with_detail(
                        "overlay de fila concluido",
                        queue_started.elapsed(),
                        format!("linhas={}", snapshot.queue_lines.len()),
                    ),
                );
            }
            if debug.text_overlay_workers {
                let workers_started = Instant::now();
                for (line, text) in snapshot.worker_lines.iter().enumerate() {
                    prepared = append_text_overlay_line(&prepared, text, 8, 8 + line as i32 * 8);
                }
                self.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuWorkerOverlayFinished,
                    format_gpu_preparation_stage_with_detail(
                        "overlay de workers concluido",
                        workers_started.elapsed(),
                        format!("linhas={}", snapshot.worker_lines.len()),
                    ),
                );
            }
            if debug.text_overlay_frames {
                let frames_started = Instant::now();
                let x = width.saturating_sub(240) as i32;
                for (line, (frame, duration)) in self.frame_timing_ring.iter().enumerate() {
                    let text =
                        format!("Frame #{frame}, {:.3} ms", duration.as_secs_f64() * 1_000.0);
                    prepared = append_text_overlay_line(&prepared, &text, x, 8 + line as i32 * 8);
                }
                self.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuFrameOverlayFinished,
                    format_gpu_preparation_stage_with_detail(
                        "overlay de frames concluido",
                        frames_started.elapsed(),
                        format!("linhas={}", self.frame_timing_ring.len()),
                    ),
                );
            }
            self.canvas.record_frame_event(
                crate::orchestrator::FrameEventKind::GpuTextOverlayFinished,
                format_gpu_preparation_stage(
                    "overlays de texto GPU concluidos",
                    text_started.elapsed(),
                ),
            );
        }
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::GpuOverlaysFinished,
            format_gpu_preparation_stage(
                "construcao dos overlays GPU concluida",
                overlays_started.elapsed(),
            ),
        );
        prepared
    }
}

pub struct GpuWindowApp {
    pub state: Option<GpuAppState>,
    app_controller: DefaultApplicationController,
    config_updates: Option<std::sync::mpsc::Receiver<crate::config::RendererConfig>>,
    renderer_closed: Option<Arc<std::sync::atomic::AtomicBool>>,
    context: Option<Arc<GpuContext>>,
    window: Option<Arc<Window>>,
    surface: Option<Arc<std::sync::Mutex<WgpuSurface<'static>>>>,
    render_target: Option<RenderTargetSession<GpuRenderTarget<WgpuGraphicsDevice>>>,
    last_frame_finished_at: Option<Instant>,
    last_redraw_requested_at: Option<Instant>,
}

impl GpuWindowApp {
    pub fn new() -> Self {
        Self::with_state(None)
    }

    pub fn with_state(state: Option<GpuAppState>) -> Self {
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
            render_target: None,
            last_frame_finished_at: None,
            last_redraw_requested_at: None,
        }
    }

    pub fn set_state(&mut self, state: GpuAppState) {
        self.app_controller = DefaultApplicationController::new(Viewport::new(
            state.config.width as u32,
            state.config.height as u32,
        ));
        self.state = Some(state);
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
                let preserve_previous_frame = state.config.preserve_previous_frame;
                if let Some(target) = &mut self.render_target {
                    target
                        .target_mut()
                        .device_mut()
                        .set_preserve_previous_frame(preserve_previous_frame);
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
        let (Some(context), Some(surface)) = (&self.context, &self.surface) else {
            return;
        };
        let Ok(mut surface) = surface.lock() else {
            return;
        };
        surface.resize(context, width, height);
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
        let context = Arc::new(context);
        let surface = Arc::new(std::sync::Mutex::new(surface));
        let Ok(surface_info) = surface.lock() else {
            crate::print_local!("Falha ao acessar superfÃ­cie GPU");
            event_loop.exit();
            return;
        };
        let (adapter_backend, adapter_name, adapter_device_type) = context.adapter_description();
        crate::print_local!(
            "GPU adapter: {:?} / {} ({:?}); modo de apresentacao: {:?}",
            adapter_backend,
            adapter_name,
            adapter_device_type,
            surface_info.present_mode_description()
        );
        let surface_size = surface_info.size();
        drop(surface_info);
        let mut graphics_device =
            match WgpuGraphicsDevice::new(Arc::clone(&context), Arc::clone(&surface)) {
                Ok(device) => device,
                Err(error) => {
                    crate::print_local!("Falha ao criar destino de renderizaÃ§Ã£o GPU: {error:?}");
                    event_loop.exit();
                    return;
                }
            };
        let preserve_previous_frame = self
            .state
            .as_ref()
            .is_some_and(|state| state.config.preserve_previous_frame);
        graphics_device.set_preserve_previous_frame(preserve_previous_frame);
        let viewport = Viewport::new(surface_size.0, surface_size.1);
        self.render_target = Some(RenderTargetSession::new(
            GpuRenderTarget::new(graphics_device, viewport),
            viewport,
        ));
        self.context = Some(context);
        self.window = Some(window);
        self.surface = Some(surface);
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
                if self.render_target.is_some() {
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
                    let Some(prepared) = self
                        .state
                        .as_ref()
                        .and_then(|state| state.prepared_frame.as_ref())
                        .cloned()
                    else {
                        return;
                    };
                    let submit_started = Instant::now();
                    if let Some(state) = &mut self.state {
                        state.canvas.record_frame_presentation_started();
                        state.canvas.record_frame_event(
                            crate::orchestrator::FrameEventKind::GpuPresentationStarted,
                            "submissao do frame pelo destino GPU iniciada",
                        );
                    }
                    let result = submit_prepared_frame(
                        self.render_target
                            .as_mut()
                            .expect("common GPU target checked above"),
                        &prepared,
                    );
                    let metrics = self.render_target.as_mut().and_then(|target| {
                        target.target_mut().device_mut().take_submission_metrics()
                    });
                    match result {
                        Ok(outcome) if outcome.was_submitted() => {
                            if let Some(state) = &mut self.state {
                                if let Some(metrics) = metrics {
                                    record_gpu_submission_metrics(state, metrics);
                                }
                                state.canvas.record_frame_event(
                                    crate::orchestrator::FrameEventKind::GpuCommandsSubmitted,
                                    "comandos do frame submetidos pelo destino GPU",
                                );
                                state.canvas.record_frame_event(
                                    crate::orchestrator::FrameEventKind::GpuDevicePollStarted,
                                    "poll do device GPU iniciado",
                                );
                            }
                            let poll_result = self.context.as_ref().map(|context| {
                                let started = Instant::now();
                                let result = context.poll_device();
                                (started.elapsed(), result)
                            });
                            if let Some(state) = &mut self.state {
                                if let Some((elapsed, result)) = poll_result {
                                    let description = match result {
                                        Ok(status) => format!(
                                            "poll do device GPU concluido: {status} ({:.3} ms)",
                                            elapsed.as_secs_f64() * 1_000.0
                                        ),
                                        Err(error) => format!(
                                            "poll do device GPU falhou: {error} ({:.3} ms)",
                                            elapsed.as_secs_f64() * 1_000.0
                                        ),
                                    };
                                    state.canvas.record_frame_event(
                                        crate::orchestrator::FrameEventKind::GpuDevicePollFinished,
                                        description,
                                    );
                                }
                                state.canvas.record_frame_event(
                                    crate::orchestrator::FrameEventKind::OverlaysDrawn,
                                    "overlays incluidos no frame comum",
                                );
                                state.canvas.record_frame_event(
                                    crate::orchestrator::FrameEventKind::GpuPresentationFinished,
                                    format_gpu_upload_stage(
                                        "submissao/apresentacao pelo destino GPU concluida",
                                        submit_started.elapsed(),
                                        "inclui aquisicao, composicao e apresentacao",
                                    ),
                                );
                                state.canvas.record_frame_presentation_finished();
                                state.canvas.finish_frame();
                            }
                            self.last_frame_finished_at = Some(Instant::now());
                        }
                        Ok(_) => {}
                        Err(error) => {
                            crate::print_local!(
                                "Falha ao submeter frame pelo destino GPU: {error:?}"
                            );
                        }
                    }
                    return;
                }
                /*
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
                        let composition = uses_persistent_composition(preserve_previous_frame)
                            .then(|| {
                                self.composition_texture
                                    .as_ref()
                                    .expect("composition texture must exist")
                            });
                        let envelope = self.envelope_command.and_then(|_| {
                            self.envelope_texture
                                .as_ref()
                                .map(|texture| (texture, &self.envelope_vertex_ring))
                        });
                        let overlay = self.overlay_command.and_then(|_| {
                            self.overlay_texture
                                .as_ref()
                                .map(|texture| (texture, &self.overlay_vertex_ring))
                        });
                        let state = &mut self.state;
                        let encoded = encode_frame(
                            context,
                            &frame,
                            pipeline,
                            self.texture_store.as_ref(),
                            &self.tile_commands,
                            &self.tile_vertex_ring,
                            envelope,
                            overlay,
                            composition,
                            preserve_previous_frame,
                            self.surface_initialized,
                            |stage| {
                                let Some(state) = state.as_mut() else {
                                    return;
                                };
                                match stage {
                                    WgpuFrameStage::CompositionStarted => {
                                        state.canvas.record_frame_event(
                                            crate::orchestrator::FrameEventKind::GpuCompositionPassStarted,
                                            "passe de composicao GPU iniciado",
                                        );
                                    }
                                    WgpuFrameStage::CompositionFinished(duration) => {
                                        state.canvas.record_frame_event(
                                            crate::orchestrator::FrameEventKind::GpuCompositionPassFinished,
                                            format_gpu_upload_stage(
                                                "passe de composicao GPU concluido",
                                                duration,
                                                "codificacao do render pass",
                                            ),
                                        );
                                    }
                                    WgpuFrameStage::PresentationStarted => {
                                        state.canvas.record_frame_event(
                                            crate::orchestrator::FrameEventKind::GpuSurfacePresentPassStarted,
                                            "passe de apresentacao da superficie GPU iniciado",
                                        );
                                    }
                                    WgpuFrameStage::PresentationFinished(duration) => {
                                        state.canvas.record_frame_event(
                                            crate::orchestrator::FrameEventKind::GpuSurfacePresentPassFinished,
                                            format_gpu_upload_stage(
                                                "passe de apresentacao da superficie GPU concluido",
                                                duration,
                                                "codificacao do render pass",
                                            ),
                                        );
                                    }
                                    WgpuFrameStage::CommandEncodingFinished => {
                                        state.canvas.record_frame_event(
                                            crate::orchestrator::FrameEventKind::GpuCommandEncodingFinished,
                                            "encoder GPU finalizado",
                                        );
                                    }
                                }
                            },
                        );
                        encoded.submit(context);
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
                        let poll_result = context.poll_device();
                        if let Some(state) = &mut self.state {
                            let description = match poll_result {
                                Ok(status) => format!("poll do device GPU concluido: {status}"),
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
                */
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
        let batch_preparation_started = Instant::now();
        let prepared = {
            let (state, controller) = (&mut self.state, &mut self.app_controller);
            state.as_mut().map(|state| {
                state.refresh_allocation_bounds();
                let canvas_preparation_started = Instant::now();
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuCanvasPreparationStarted,
                    "preparacao do canvas GPU iniciada",
                );
                controller.prepare_canvas(
                    &mut state.canvas,
                    &state.orchestrator,
                    state.allocation_bounds,
                    state.deallocation_bounds,
                );
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuCanvasPreparationFinished,
                    format_gpu_preparation_stage(
                        "preparacao do canvas GPU concluida",
                        canvas_preparation_started.elapsed(),
                    ),
                );
                controller.begin_tile_composition(&mut state.canvas);
                state.prepare_visible_batch_after_canvas(controller);
                (state.prepared_batch.take(), state.prepared_frame.clone())
            })
        };
        if let Some((Some(_batch), Some(frame))) = prepared {
            let publish_started = Instant::now();
            let frame = self
                .app_controller
                .publish_and_prepare_frame(frame)
                .expect("GPU controller should prepare a published frame");
            if let Some(state) = &mut self.state {
                state.prepared_frame = Some(frame.clone());
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuFramePublished,
                    format_gpu_preparation_stage(
                        "frame GPU publicado no controlador comum",
                        publish_started.elapsed(),
                    ),
                );
            }
            if let Some(state) = &mut self.state {
                state.canvas.record_frame_event(
                    crate::orchestrator::FrameEventKind::GpuBatchPreparationFinished,
                    format_gpu_preparation_stage(
                        "preparacao do batch GPU concluida",
                        batch_preparation_started.elapsed(),
                    ),
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

fn record_gpu_submission_metrics(state: &mut GpuAppState, metrics: WgpuSubmissionMetrics) {
    state.canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::GpuSurfaceAcquireStarted,
        "aquisicao da superficie GPU iniciada",
    );
    state.canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::GpuSurfaceAcquireFinished,
        format_gpu_upload_stage(
            "aquisicao da superficie GPU concluida",
            metrics.surface_acquire,
            "destino GPU comum",
        ),
    );
    state.canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::GpuCompositionPassStarted,
        "passe de composicao GPU iniciado",
    );
    state.canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::GpuCompositionPassFinished,
        format_gpu_upload_stage(
            "passe de composicao GPU concluido",
            metrics.composition_encoding,
            "destino GPU comum",
        ),
    );
    state.canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::GpuCommandEncodingFinished,
        format_gpu_upload_stage(
            "codificacao de comandos GPU concluida",
            metrics.command_encoding + metrics.presentation_encoding,
            "destino GPU comum",
        ),
    );
    state.canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::GpuSurfacePresentPassStarted,
        "passe de apresentacao da superficie GPU iniciado",
    );
    state.canvas.record_frame_event(
        crate::orchestrator::FrameEventKind::GpuSurfacePresentPassFinished,
        format_gpu_upload_stage(
            "passe de apresentacao da superficie GPU concluido",
            metrics.presentation,
            "destino GPU comum",
        ),
    );
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
        format_gpu_preparation_stage, format_gpu_preparation_stage_with_detail,
        format_gpu_redraw_latency, format_gpu_upload_stage, frame_tiles_from_batch,
        signal_renderer_closed, GpuFrameMetrics, PreparedTileBatch,
    };
    use crate::app::ApplicationController;
    use crate::geometry::ScreenPoint;
    use crate::gpu::{TextureKey, TextureUpload, TileDrawCommand};
    use crate::render::{ImageId, ImageRevision, Rect, Viewport};
    use std::sync::Arc;
    use std::time::Duration;
    use winit::dpi::PhysicalSize;
    use winit::event::WindowEvent;

    #[derive(Default)]
    struct RecordingTarget(Vec<&'static str>);

    impl crate::render::RenderTarget for RecordingTarget {
        fn capabilities(&self) -> crate::render::RenderCapabilities {
            crate::render::RenderCapabilities::default()
        }

        fn resize(&mut self, _viewport: Viewport) -> Result<(), crate::render::RenderError> {
            self.0.push("resize");
            Ok(())
        }

        fn update_images(
            &mut self,
            _updates: &[crate::render::ImageUpdate],
        ) -> Result<(), crate::render::RenderError> {
            self.0.push("upload");
            Ok(())
        }

        fn render(
            &mut self,
            _frame: &crate::render::RenderFrame,
        ) -> Result<crate::render::FrameOutcome, crate::render::RenderError> {
            self.0.push("render");
            Ok(crate::render::FrameOutcome::submitted())
        }

        fn evict_images(&mut self, _images: &[ImageId]) {}

        fn recover(
            &mut self,
            _reason: crate::render::SurfaceFailure,
        ) -> Result<(), crate::render::RenderError> {
            Ok(())
        }
    }

    #[test]
    fn gpu_runtime_submits_prepared_frames_through_common_target_order() {
        let initial_viewport = Viewport::new(2, 2);
        let frame_viewport = Viewport::new(1, 1);
        let image = ImageId::new(3);
        let update = crate::render::ImageUpdate::new(
            image,
            ImageRevision::new(1),
            1,
            1,
            vec![10, 20, 30, 255],
        )
        .unwrap();
        let frame = crate::render::RenderFrame::new(1, frame_viewport).with_tile(
            crate::render::TileDraw::new(image, ImageRevision::new(1), 0),
        );
        let prepared = crate::render::PreparedFrame::new(frame, vec![update]);
        let mut session =
            crate::render::RenderTargetSession::new(RecordingTarget::default(), initial_viewport);

        let outcome = super::submit_prepared_frame(&mut session, &prepared).unwrap();

        assert!(outcome.was_submitted());
        assert_eq!(session.target().0, ["resize", "upload", "render"]);
    }

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
        assert_eq!(
            format_gpu_preparation_stage("coleta de tiles concluida", Duration::from_micros(570)),
            "coleta de tiles concluida: 0.570 ms (preparacao do frame GPU)"
        );
        assert_eq!(
            format_gpu_preparation_stage_with_detail(
                "texturas graficas concluidas",
                Duration::from_micros(570),
                "uploads=2",
            ),
            "texturas graficas concluidas: 0.570 ms (uploads=2; preparacao do frame GPU)"
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
    fn prepared_frame_adds_overlay_image_to_shared_resources_and_draw_order() {
        let viewport = Viewport::new(4, 3);
        let base = crate::render::PreparedFrame::new(
            crate::render::RenderFrame::new(7, viewport),
            Vec::new(),
        );
        let upload = TextureUpload {
            key: TextureKey {
                tile: usize::MAX,
                content_hash: 42,
            },
            width: 1,
            height: 1,
            rgba8: vec![255, 255, 255, 255],
        };

        let prepared = super::append_image_overlay(
            &base,
            ImageId::new(usize::MAX as u64),
            upload,
            Rect::new(2, 1, 1, 1),
        )
        .unwrap();

        assert_eq!(prepared.image_updates().len(), 1);
        assert_eq!(
            prepared.image_updates()[0].image(),
            ImageId::new(usize::MAX as u64)
        );
        assert_eq!(prepared.frame().overlays().len(), 1);
        let crate::render::OverlayPrimitive::Image(image) = &prepared.frame().overlays()[0] else {
            panic!("expected image overlay");
        };
        assert_eq!(image.image(), ImageId::new(usize::MAX as u64));
        assert_eq!(image.destination(), Rect::new(2, 1, 1, 1));
    }

    #[test]
    fn reduced_viewport_can_hide_its_envelope_in_the_common_frame() {
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
        let mut config = crate::config::RendererConfig::default();
        config.width = 8;
        config.height = 8;
        config.debug.reduced_viewport = true;
        config.debug.show_allocation_envelope = false;
        config.debug.text_overlay_global = false;
        let mut state = super::GpuAppState::new(canvas, orchestrator, config);
        let base = crate::render::PreparedFrame::new(
            crate::render::RenderFrame::new(8, Viewport::new(8, 8)),
            Vec::new(),
        );

        let prepared = state.append_debug_overlays(base);

        assert!(prepared.frame().overlays().is_empty());
        assert!(prepared.image_updates().is_empty());
    }

    #[test]
    fn allocation_envelope_reuses_its_upload_when_geometry_is_unchanged() {
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
        let mut config = crate::config::RendererConfig::default();
        config.width = 8;
        config.height = 8;
        config.debug.show_allocation_envelope = true;
        config.debug.text_overlay_global = true;
        config.debug.text_overlay_workers = false;
        config.debug.text_overlay_layers = false;
        config.debug.text_overlay_queue = false;
        config.debug.text_overlay_frames = false;
        let mut state = super::GpuAppState::new(canvas, orchestrator, config);
        let first = crate::render::PreparedFrame::new(
            crate::render::RenderFrame::new(8, Viewport::new(8, 8)),
            Vec::new(),
        );
        let second = crate::render::PreparedFrame::new(
            crate::render::RenderFrame::new(9, Viewport::new(8, 8)),
            Vec::new(),
        );

        let first = state.append_debug_overlays(first);
        let second = state.append_debug_overlays(second);

        assert_eq!(first.frame().overlays().len(), 1);
        assert_eq!(first.image_updates().len(), 1);
        assert_eq!(second.frame().overlays().len(), 1);
        assert!(second.image_updates().is_empty());

        state.resize_viewport(Viewport::new(10, 8));
        state.refresh_allocation_bounds();
        let resized = crate::render::PreparedFrame::new(
            crate::render::RenderFrame::new(10, Viewport::new(10, 8)),
            Vec::new(),
        );
        let resized = state.append_debug_overlays(resized);

        assert_eq!(resized.frame().overlays().len(), 1);
        assert_eq!(resized.image_updates().len(), 1);
    }

    #[test]
    fn common_worker_overlay_uses_the_cpu_top_left_anchor() {
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
        let mut config = crate::config::RendererConfig::default();
        config.width = 640;
        config.height = 400;
        config.debug.text_overlay_global = true;
        config.debug.text_overlay_workers = true;
        config.debug.text_overlay_frames = false;
        config.debug.text_overlay_layers = false;
        config.debug.text_overlay_queue = false;
        config.debug.show_allocation_envelope = false;
        let mut state = super::GpuAppState::new(canvas, orchestrator, config);
        let base = crate::render::PreparedFrame::new(
            crate::render::RenderFrame::new(9, Viewport::new(640, 400)),
            Vec::new(),
        );

        let prepared = state.append_debug_overlays(base);

        assert_eq!(prepared.frame().overlays().len(), 1);
        let crate::render::OverlayPrimitive::Text(worker_line) = &prepared.frame().overlays()[0]
        else {
            panic!("worker overlay should be represented as text");
        };
        assert_eq!(worker_line.origin(), ScreenPoint::new(8, 8));
        assert_eq!(worker_line.text(), "Worker 0: ocioso");
        assert!(prepared.image_updates().is_empty());
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
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !state
            .prepared_frame
            .as_ref()
            .is_some_and(|prepared| !prepared.image_updates().is_empty())
        {
            assert!(
                std::time::Instant::now() < deadline,
                "worker did not publish the expected tile before the timeout"
            );
            std::thread::sleep(Duration::from_millis(10));
            state.prepare_visible_batch();
        }

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
