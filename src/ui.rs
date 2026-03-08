use crate::config::Config;
use crate::processing::FrameData;
use crate::visualizations::registry::VisualizationRegistry;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct App {
    registry: VisualizationRegistry,
    config: Config,
    running: Arc<AtomicBool>,
    sensitivity: f32,
}

impl App {
    pub fn new(registry: VisualizationRegistry, config: Config, running: Arc<AtomicBool>) -> Self {
        Self {
            registry,
            config,
            running,
            sensitivity: 1.0,
        }
    }

    pub fn run(&mut self, frame_rx: Receiver<FrameData>) -> Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

        let frame_duration = Duration::from_millis(1000 / self.config.display.fps.max(1) as u64);
        let mut last_frame = FrameData::default();

        while self.running.load(Ordering::Relaxed) {
            let loop_start = Instant::now();

            // Drain the channel, keep latest frame
            while let Ok(frame) = frame_rx.try_recv() {
                last_frame = frame;
            }

            // Scale spectrum by sensitivity
            let mut display_frame = last_frame.clone();
            for val in &mut display_frame.spectrum {
                *val = (*val * self.sensitivity).clamp(0.0, 1.0);
            }

            self.registry.update_current(&display_frame);

            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(if self.config.display.show_status_bar {
                        vec![Constraint::Min(1), Constraint::Length(1)]
                    } else {
                        vec![Constraint::Min(1)]
                    })
                    .split(f.area());

                // Main visualization area
                let viz_area = chunks[0];
                self.registry.render_current(viz_area, f.buffer_mut());

                // Status bar
                if self.config.display.show_status_bar && chunks.len() > 1 {
                    let mode_name = self.registry
                        .current()
                        .map(|v| v.name())
                        .unwrap_or("none");
                    let status = format!(
                        " [{}]  peak: {:.2}  rms: {:.2}  sens: {:.1}x  |  Tab: next  q: quit ",
                        mode_name,
                        display_frame.peak,
                        display_frame.rms,
                        self.sensitivity,
                    );
                    let status_bar = Paragraph::new(status)
                        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
                    f.render_widget(status_bar, chunks[1]);
                }
            })?;

            // Handle input
            let poll_timeout = frame_duration.saturating_sub(loop_start.elapsed());
            if event::poll(poll_timeout)? {
                if let Event::Key(key) = event::read()? {
                    if !self.handle_key(key) {
                        // Forward to current visualization
                        self.registry.on_key_current(key);
                    }
                }
            }
        }

        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => {
                self.running.store(false, Ordering::Relaxed);
                true
            }
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.registry.prev();
                } else {
                    self.registry.next();
                }
                true
            }
            KeyCode::BackTab => {
                self.registry.prev();
                true
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.sensitivity = (self.sensitivity + 0.1).min(5.0);
                true
            }
            KeyCode::Char('-') => {
                self.sensitivity = (self.sensitivity - 0.1).max(0.1);
                true
            }
            KeyCode::Char('s') => {
                self.config.display.show_status_bar = !self.config.display.show_status_bar;
                true
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running.store(false, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }
}
