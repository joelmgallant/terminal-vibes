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
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct App {
    registry: VisualizationRegistry,
    config: Config,
    running: Arc<AtomicBool>,
    sensitivity: f32,
    beat_intensity: f32,
}

impl App {
    pub fn new(
        mut registry: VisualizationRegistry,
        config: Config,
        running: Arc<AtomicBool>,
    ) -> Self {
        let mut sensitivity = 1.0_f32;
        let mut beat_intensity = 1.0_f32;
        // Restore saved state
        if let Some(state) = Self::load_state() {
            if let Some(viz_name) = state.get("current_visualization").and_then(|v| v.as_str()) {
                registry.select_by_name(viz_name);
            }
            if let Some(s) = state.get("sensitivity").and_then(|v| v.as_float()) {
                sensitivity = s as f32;
            }
            if let Some(b) = state.get("beat_intensity").and_then(|v| v.as_float()) {
                beat_intensity = b as f32;
            }
            if let Some(viz_table) = state.get("visualizations").and_then(|v| v.as_table()) {
                registry.load_all(viz_table);
            }
        }
        Self {
            registry,
            config,
            running,
            sensitivity,
            beat_intensity,
        }
    }

    pub fn run(&mut self, frame_rx: Receiver<FrameData>) -> Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

        let frame_duration = Duration::from_millis(1000 / self.config.display.fps.max(1) as u64);
        let mut last_frame = FrameData::default();
        let mut display_frame = FrameData::default();

        while self.running.load(Ordering::Relaxed) {
            let loop_start = Instant::now();

            // Drain the channel, keep latest frame
            while let Ok(frame) = frame_rx.try_recv() {
                last_frame = frame;
            }

            // Scale spectrum by sensitivity (reuses existing Vec capacity via clone_from)
            display_frame.clone_from(&last_frame);
            for val in &mut display_frame.spectrum {
                *val = (*val * self.sensitivity).clamp(0.0, 1.0);
            }

            // Scale beat data by beat intensity
            let bi = self.beat_intensity;
            display_frame.beat.envelope = (display_frame.beat.envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.bass_envelope = (display_frame.beat.bass_envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.mid_envelope = (display_frame.beat.mid_envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.treble_envelope = (display_frame.beat.treble_envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.bass_energy = (display_frame.beat.bass_energy * bi).clamp(0.0, 1.0);
            display_frame.beat.mid_energy = (display_frame.beat.mid_energy * bi).clamp(0.0, 1.0);
            display_frame.beat.treble_energy = (display_frame.beat.treble_energy * bi).clamp(0.0, 1.0);

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
                    let mode_name = self.registry.current().map(|v| v.name()).unwrap_or("none");
                    let beat_indicator = if display_frame.beat.beat {
                        "BEAT!"
                    } else {
                        "     "
                    };
                    let status = format!(
                        " [{}]  peak: {:.2}  rms: {:.2}  env: {:.2}  {}  sens: {:.1}x  beat: {:.1}x  |  Tab: next  q: quit ",
                        mode_name, display_frame.peak, display_frame.rms,
                        display_frame.beat.envelope, beat_indicator, self.sensitivity, self.beat_intensity,
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

        // Save state before cleanup
        self.save_state();

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
            KeyCode::Char('b') => {
                self.beat_intensity = (self.beat_intensity + 0.1).min(3.0);
                true
            }
            KeyCode::Char('B') => {
                self.beat_intensity = (self.beat_intensity - 0.1).max(0.0);
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

    fn state_path() -> PathBuf {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        config_dir.join("terminal-vibes").join("state.toml")
    }

    fn save_state(&self) {
        let mut root = toml::value::Table::new();

        if let Some(name) = self.registry.current_name() {
            root.insert(
                "current_visualization".to_string(),
                toml::Value::String(name.to_string()),
            );
        }
        root.insert(
            "sensitivity".to_string(),
            toml::Value::Float(self.sensitivity as f64),
        );
        root.insert(
            "beat_intensity".to_string(),
            toml::Value::Float(self.beat_intensity as f64),
        );

        let viz_states = self.registry.save_all();
        if !viz_states.is_empty() {
            root.insert("visualizations".to_string(), toml::Value::Table(viz_states));
        }

        let state_path = Self::state_path();
        if let Some(parent) = state_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(&toml::Value::Table(root)) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&state_path, content) {
                    log::warn!("Failed to save state: {}", e);
                }
            }
            Err(e) => log::warn!("Failed to serialize state: {}", e),
        }
    }

    fn load_state() -> Option<toml::value::Table> {
        let state_path = Self::state_path();
        let content = std::fs::read_to_string(&state_path).ok()?;
        let value: toml::Value = toml::from_str(&content).ok()?;
        value.as_table().cloned()
    }
}
