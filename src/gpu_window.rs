use crate::gpu::{
    create_tile_pipeline, texture_keys_for_commands, tile_vertices_for_commands, GpuContext,
    GpuTextureStore, PreparedTileBatch, TextureCache,
};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GpuBatchStats {
    visible_tiles: usize,
    uploaded_textures: usize,
}

impl GpuBatchStats {
    fn from_batch(batch: &PreparedTileBatch) -> Self {
        Self {
            visible_tiles: batch.commands.len(),
            uploaded_textures: batch.uploads.len(),
        }
    }

    fn description(self) -> String {
        format!(
            "tiles rasterizados neste frame: {}, texturas novas neste frame: {}",
            self.visible_tiles, self.uploaded_textures
        )
    }
}

pub struct GpuAppState {
    pub canvas: crate::TiledInfiniteCanvas,
    pub orchestrator: crate::Orchestrator,
    pub config: crate::config::RendererConfig,
    pub prepared_batch: Option<PreparedTileBatch>,
    pub texture_cache: TextureCache,
    cursor: crate::geometry::ScreenPoint,
    left_button_down: bool,
}

impl GpuAppState {
    pub fn new(
        canvas: crate::TiledInfiniteCanvas,
        orchestrator: crate::Orchestrator,
        config: crate::config::RendererConfig,
    ) -> Self {
        Self {
            canvas,
            orchestrator,
            config,
            prepared_batch: None,
            texture_cache: TextureCache::default(),
            cursor: crate::geometry::ScreenPoint::new(0, 0),
            left_button_down: false,
        }
    }

    fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let next = crate::geometry::ScreenPoint::new(
                    position.x.round() as i32,
                    position.y.round() as i32,
                );
                if self.left_button_down {
                    self.canvas.drag(crate::geometry::ScreenPoint::new(
                        next.x - self.cursor.x,
                        next.y - self.cursor.y,
                    ));
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
                    let multiplier = self.config.zoom_multiplier;
                    let current = self
                        .canvas
                        .layer(0)
                        .map_or(self.config.max_apparent_pixel_size(), |layer| layer.zoom());
                    let zoom = if amount > 0.0 {
                        current * multiplier
                    } else {
                        current / multiplier
                    };
                    self.canvas.zoom_at(self.cursor, zoom);
                }
            }
            _ => {}
        }
    }

    pub fn prepare_visible_batch(&mut self) {
        self.canvas.begin_frame();
        self.canvas.record_frame_event(
            crate::orchestrator::FrameEventKind::ConfigurationProcessed,
            "configuracao processada",
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
            GpuBatchStats::from_batch(&batch).description(),
        );
        self.prepared_batch = Some(batch);
    }
}

pub struct GpuWindowApp {
    pub state: Option<GpuAppState>,
    context: Option<GpuContext>,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    pipeline: Option<wgpu::RenderPipeline>,
    tile_bind_group_layout: Option<wgpu::BindGroupLayout>,
    tile_vertex_buffer: Option<wgpu::Buffer>,
    texture_store: Option<GpuTextureStore>,
    tile_commands: Vec<crate::gpu::TileDrawCommand>,
}

impl GpuWindowApp {
    pub fn new() -> Self {
        Self::with_state(None)
    }

    pub fn with_state(state: Option<GpuAppState>) -> Self {
        Self {
            state,
            context: None,
            window: None,
            surface: None,
            surface_config: None,
            pipeline: None,
            tile_bind_group_layout: None,
            tile_vertex_buffer: None,
            texture_store: None,
            tile_commands: Vec::new(),
        }
    }

    pub fn set_state(&mut self, state: GpuAppState) {
        self.state = Some(state);
    }

    fn configure_surface(&mut self, width: u32, height: u32) {
        let (Some(context), Some(surface)) = (&self.context, &self.surface) else {
            return;
        };
        let Some(config) = self.surface_config.as_mut() else {
            return;
        };
        config.width = width.max(1);
        config.height = height.max(1);
        surface.configure(&context.device, config);
    }
}

impl GpuWindowApp {
    pub fn upload_batch(&mut self, batch: PreparedTileBatch) {
        let (Some(context), Some(layout), Some(store)) = (
            &self.context,
            &self.tile_bind_group_layout,
            &mut self.texture_store,
        ) else {
            return;
        };
        for upload in batch.uploads {
            store.upload(context, layout, upload);
        }
        store.retain_only(texture_keys_for_commands(&batch.commands));
        if let Some(surface_config) = &self.surface_config {
            let vertices = tile_vertices_for_commands(
                &batch.commands,
                surface_config.width,
                surface_config.height,
            );
            self.tile_vertex_buffer = Some(context.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("tile-batch-vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }
        self.tile_commands = batch.commands;
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
        let context = match pollster::block_on(GpuContext::initialize()) {
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
        let Some(format) = capabilities.formats.first().copied() else {
            crate::print_local!("GPU não oferece formato de superfície compatível");
            event_loop.exit();
            return;
        };
        let Some(present_mode) = capabilities.present_modes.first().copied() else {
            crate::print_local!("GPU não oferece modo de apresentação compatível");
            event_loop.exit();
            return;
        };
        let Some(alpha_mode) = capabilities.alpha_modes.first().copied() else {
            crate::print_local!("GPU não oferece modo alpha compatível");
            event_loop.exit();
            return;
        };
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
        if let Some(state) = &mut self.state {
            state.handle_window_event(&event);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.configure_surface(size.width, size.height),
            WindowEvent::RedrawRequested => {
                if let (Some(context), Some(surface), Some(pipeline)) =
                    (&self.context, &self.surface, &self.pipeline)
                {
                    if let wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) =
                        surface.get_current_texture()
                    {
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = context.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("gpu-clear"),
                            },
                        );
                        {
                            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("gpu-clear-pass"),
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
                            let mut pass = _pass;
                            pass.set_pipeline(pipeline);
                            if let (Some(store), Some(vertex_buffer)) =
                                (&self.texture_store, &self.tile_vertex_buffer)
                            {
                                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                                for (index, command) in self.tile_commands.iter().enumerate() {
                                    if let Some(tile_texture) = store.get(&command.texture) {
                                        pass.set_bind_group(0, &tile_texture.bind_group, &[]);
                                        let start = (index * 6) as u32;
                                        pass.draw(start..start + 6, 0..1);
                                    }
                                }
                            }
                        }
                        context.queue.submit(Some(encoder.finish()));
                        if let Some(state) = &mut self.state {
                            state.canvas.record_frame_presentation_started();
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
        let batch = self
            .state
            .as_mut()
            .map(|state| {
                state.prepare_visible_batch();
                state.prepared_batch.take()
            })
            .flatten();
        if let Some(batch) = batch {
            self.upload_batch(batch);
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GpuBatchStats, PreparedTileBatch};
    use crate::geometry::ScreenPoint;
    use crate::gpu::{TextureKey, TextureUpload, TileDrawCommand};

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
            GpuBatchStats::from_batch(&batch).description(),
            "tiles rasterizados neste frame: 1, texturas novas neste frame: 1"
        );
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
