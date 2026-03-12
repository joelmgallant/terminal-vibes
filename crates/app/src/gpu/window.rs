// crates/app/src/gpu/window.rs
use anyhow::Result;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use super::renderer::GpuRenderer;

pub struct GpuApp {
    window: Option<Arc<Window>>,
    renderer: Option<GpuRenderer>,
    width: u32,
    height: u32,
}

impl GpuApp {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            window: None,
            renderer: None,
            width,
            height,
        }
    }
}

impl ApplicationHandler for GpuApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("terminal-vibes")
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        let renderer = pollster::block_on(GpuRenderer::new(window.clone()))
            .expect("Failed to create GPU renderer");

        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(physical_size.width, physical_size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &self.renderer {
                    // For now, just clear to dark purple (beat will modulate later)
                    let _ = renderer.render_clear(0.05, 0.02, 0.08);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
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

pub fn run_window(width: u32, height: u32) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = GpuApp::new(width, height);
    event_loop.run_app(&mut app)?;
    Ok(())
}
