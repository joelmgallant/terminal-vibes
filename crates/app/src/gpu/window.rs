use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

use crate::config::Config;
use crate::pipeline::AudioPipeline;
use crate::processing::FrameData;

use super::audio_texture::AudioTextureData;
use super::renderer::GpuRenderer;
use super::uniforms::Uniforms;

pub struct GpuApp {
    config: Config,
    _pipeline: Option<AudioPipeline>,
    frame_rx: Option<std::sync::mpsc::Receiver<FrameData>>,
    window: Option<Arc<Window>>,
    renderer: Option<GpuRenderer>,
    start_time: Instant,
    last_frame_time: Instant,
    frame_count: u32,
    sensitivity: f32,
    beat_intensity: f32,
    fullscreen: bool,
    latest_frame: Option<FrameData>,
}

impl GpuApp {
    pub fn new(config: Config) -> Self {
        let now = Instant::now();
        Self {
            config,
            _pipeline: None,
            frame_rx: None,
            window: None,
            renderer: None,
            start_time: now,
            last_frame_time: now,
            frame_count: 0,
            sensitivity: 1.0,
            beat_intensity: 1.0,
            fullscreen: false,
            latest_frame: None,
        }
    }

    fn drain_audio(&mut self) {
        if let Some(rx) = &self.frame_rx {
            // Drain channel, keep only latest frame
            while let Ok(frame) = rx.try_recv() {
                self.latest_frame = Some(frame);
            }
        }
    }

    fn handle_key(&mut self, event: &KeyEvent, event_loop: &ActiveEventLoop) {
        if event.state != ElementState::Pressed {
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Character(c) => match c.as_str() {
                "q" => event_loop.exit(),
                "+" | "=" => {
                    self.sensitivity = (self.sensitivity + 0.1).min(5.0);
                    log::info!("Sensitivity: {:.1}", self.sensitivity);
                }
                "-" => {
                    self.sensitivity = (self.sensitivity - 0.1).max(0.1);
                    log::info!("Sensitivity: {:.1}", self.sensitivity);
                }
                "b" => {
                    self.beat_intensity = (self.beat_intensity + 0.1).min(3.0);
                    log::info!("Beat intensity: {:.1}", self.beat_intensity);
                }
                "B" => {
                    self.beat_intensity = (self.beat_intensity - 0.1).max(0.0);
                    log::info!("Beat intensity: {:.1}", self.beat_intensity);
                }
                "f" => self.toggle_fullscreen(),
                _ => {}
            },
            Key::Named(NamedKey::F11) => self.toggle_fullscreen(),
            Key::Named(NamedKey::Tab) => {
                // TODO: cycle viz (Task 11)
                log::info!("Tab pressed — viz cycling coming in Task 11");
            }
            _ => {}
        }
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(window) = &self.window {
            self.fullscreen = !self.fullscreen;
            if self.fullscreen {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            } else {
                window.set_fullscreen(None);
            }
        }
    }
}

impl ApplicationHandler for GpuApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let width = 1280u32;
        let height = 720u32;

        let attrs = WindowAttributes::default()
            .with_title("terminal-vibes")
            .with_inner_size(winit::dpi::LogicalSize::new(width, height));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        let mut renderer = pollster::block_on(GpuRenderer::new(window.clone()))
            .expect("Failed to create GPU renderer");

        // Initialize shader pipeline with built-in spectrum_rings
        let shader_src = include_str!("shaders/spectrum_rings.wgsl");
        renderer
            .init_pipeline(shader_src)
            .expect("Failed to init shader pipeline");

        self.renderer = Some(renderer);
        self.window = Some(window);

        // Start audio pipeline
        match AudioPipeline::start(&self.config) {
            Ok(mut pipeline) => {
                self.frame_rx = Some(pipeline.take_frame_rx());
                self._pipeline = Some(pipeline);
            }
            Err(e) => log::error!("Failed to start audio: {}", e),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key(&event, event_loop);
            }
            WindowEvent::RedrawRequested => {
                self.drain_audio();

                let now = Instant::now();
                let time = now.duration_since(self.start_time).as_secs_f32();
                let delta = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                if let (Some(renderer), Some(frame)) = (&self.renderer, &self.latest_frame) {
                    let mut scaled_frame = frame.clone();
                    // Apply sensitivity
                    for s in &mut scaled_frame.spectrum {
                        *s = (*s * self.sensitivity).min(1.0);
                    }
                    // Apply beat intensity
                    scaled_frame.beat.envelope *= self.beat_intensity;
                    scaled_frame.beat.bass_envelope *= self.beat_intensity;
                    scaled_frame.beat.mid_envelope *= self.beat_intensity;
                    scaled_frame.beat.treble_envelope *= self.beat_intensity;

                    let tex_data = AudioTextureData::from_frame(&scaled_frame);
                    renderer.update_audio_texture(&tex_data);

                    let resolution = [
                        renderer.surface_config.width as f32,
                        renderer.surface_config.height as f32,
                    ];
                    let uniforms = Uniforms::from_frame(
                        &scaled_frame,
                        time,
                        delta,
                        resolution,
                        self.frame_count,
                    );
                    renderer.update_uniforms(&uniforms);

                    if let Err(e) = renderer.render_shader() {
                        log::error!("Render error: {}", e);
                    }
                } else if let Some(renderer) = &self.renderer {
                    // No audio data yet — clear to black
                    let _ = renderer.render_clear(0.0, 0.0, 0.0);
                }

                self.frame_count += 1;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub fn run_window(config: Config) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = GpuApp::new(config);
    event_loop.run_app(&mut app)?;
    Ok(())
}
