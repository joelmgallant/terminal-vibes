use crate::config::Config;
use crate::processing::FrameData;
use crate::visualizations::registry::VisualizationRegistry;
use anyhow::Result;
use crossterm::{
    event::{self, DisableFocusChange, EnableFocusChange, Event, KeyCode, KeyEvent, KeyModifiers},
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

/// Duration the visualization label stays fully visible after switching.
const LABEL_VISIBLE_SECS: f32 = 2.0;
/// Duration the label spends fading out after the visible period.
const LABEL_FADE_SECS: f32 = 1.0;

pub struct App {
    registry: VisualizationRegistry,
    config: Config,
    running: Arc<AtomicBool>,
    sensitivity: f32,
    beat_intensity: f32,
    color_detail: f32,
    /// EMA of frame render time in seconds
    render_time_ema: f32,
    /// Auto-adjusted color detail (may be lower than user's setting)
    effective_color_detail: f32,
    /// When the visualization was last switched (drives label fade).
    label_shown_at: Instant,
}

impl App {
    pub fn new(
        mut registry: VisualizationRegistry,
        config: Config,
        running: Arc<AtomicBool>,
    ) -> Self {
        let mut sensitivity = 1.0_f32;
        let mut beat_intensity = 1.0_f32;
        let mut color_detail = 1.0_f32;
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
            if let Some(cd) = state.get("color_detail").and_then(|v| v.as_float()) {
                color_detail = cd as f32;
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
            color_detail,
            render_time_ema: 0.0,
            effective_color_detail: color_detail,
            label_shown_at: Instant::now(),
        }
    }

    pub fn run(&mut self, frame_rx: Receiver<FrameData>) -> Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(EnableFocusChange)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

        // Auto-detect tmux and cap FPS to avoid overwhelming the terminal
        // multiplexer with escape sequences from color-intensive visualizations.
        let in_tmux = std::env::var("TMUX").is_ok();
        let effective_fps = if in_tmux {
            self.config.display.fps.min(30)
        } else {
            self.config.display.fps.max(60)
        };
        let frame_duration = Duration::from_millis(1000 / effective_fps.max(1) as u64);
        let mut last_frame = FrameData::default();
        let mut display_frame = FrameData::default();
        let mut focused = true;

        while self.running.load(Ordering::Relaxed) {
            let loop_start = Instant::now();

            // Drain the channel, keep latest frame (always drain to prevent backpressure)
            while let Ok(frame) = frame_rx.try_recv() {
                last_frame = frame;
            }

            // When unfocused and the current visualization is heavy (full-screen
            // dual-RGB HalfBlockCanvas), skip rendering to avoid flooding tmux with
            // escape sequences. Lightweight visualizations keep rendering normally.
            if in_tmux && !focused && self.registry.current_heavy_rendering() {
                if event::poll(frame_duration)? {
                    match event::read()? {
                        Event::FocusGained => focused = true,
                        Event::Key(key) => {
                            self.handle_key(key);
                        }
                        _ => {}
                    }
                }
                continue;
            }

            // Scale spectrum by sensitivity (reuses existing Vec capacity via clone_from)
            display_frame.clone_from(&last_frame);
            for val in &mut display_frame.spectrum {
                *val = (*val * self.sensitivity).clamp(0.0, 1.0);
            }

            // Scale beat data by beat intensity
            let bi = self.beat_intensity;
            display_frame.beat.envelope = (display_frame.beat.envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.bass_envelope =
                (display_frame.beat.bass_envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.mid_envelope =
                (display_frame.beat.mid_envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.treble_envelope =
                (display_frame.beat.treble_envelope * bi).clamp(0.0, 1.0);
            display_frame.beat.bass_energy = (display_frame.beat.bass_energy * bi).clamp(0.0, 1.0);
            display_frame.beat.mid_energy = (display_frame.beat.mid_energy * bi).clamp(0.0, 1.0);
            display_frame.beat.treble_energy =
                (display_frame.beat.treble_energy * bi).clamp(0.0, 1.0);

            let term_size = terminal.size()?;
            let cell_count = term_size.width as u32 * term_size.height as u32;
            let quant_step = crate::visualizations::render::adaptive_quantization_step(
                cell_count,
                self.effective_color_detail,
            );
            self.registry.set_quantization_step(quant_step);

            self.registry.update_current(&display_frame);

            let draw_start = Instant::now();
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

                // Visualization name label — fades out after switching
                if viz_area.height > 2 {
                    let elapsed = self.label_shown_at.elapsed().as_secs_f32();
                    let total = LABEL_VISIBLE_SECS + LABEL_FADE_SECS;
                    if elapsed < total {
                        let opacity = if elapsed < LABEL_VISIBLE_SECS {
                            1.0
                        } else {
                            1.0 - (elapsed - LABEL_VISIBLE_SECS) / LABEL_FADE_SECS
                        };
                        let gray = (180.0 * opacity) as u8;
                        let bg_gray = (30.0 * opacity) as u8;

                        let name =
                            self.registry.current().map(|v| v.name()).unwrap_or("none");
                        let label = format!(" {} ", name);
                        let label_w = label.len() as u16;
                        let label_x =
                            viz_area.x + viz_area.width.saturating_sub(label_w) / 2;
                        let label_y = viz_area.y + viz_area.height - 1;
                        let label_area =
                            Rect::new(label_x, label_y, label_w.min(viz_area.width), 1);
                        let label_widget = Paragraph::new(label).style(
                            Style::default()
                                .fg(Color::Rgb(gray, gray, gray))
                                .bg(Color::Rgb(bg_gray, bg_gray, bg_gray)),
                        );
                        f.render_widget(label_widget, label_area);
                    }
                }

                // Status bar
                if self.config.display.show_status_bar && chunks.len() > 1 {
                    let mode_name = self.registry.current().map(|v| v.name()).unwrap_or("none");
                    let beat_indicator = if display_frame.beat.beat || display_frame.tempo.predicted_beat {
                        "BEAT!"
                    } else {
                        "     "
                    };
                    let bpm_display = if display_frame.tempo.confidence >= 0.6 {
                        format!("{}BPM", display_frame.tempo.bpm.round() as u32)
                    } else if display_frame.tempo.confidence >= 0.3 {
                        format!("~{}BPM", display_frame.tempo.bpm.round() as u32)
                    } else {
                        String::new()
                    };
                    let bpm_section = if bpm_display.is_empty() {
                        String::new()
                    } else {
                        format!("  {}  ", bpm_display)
                    };
                    let status = format!(
                        " [{}]  peak: {:.2}  rms: {:.2}  env: {:.2}  {}{}  sens: {:.1}x  beat: {:.1}x  detail: {:.1}x  |  Tab: next  q: quit ",
                        mode_name, display_frame.peak, display_frame.rms,
                        display_frame.beat.envelope, beat_indicator, bpm_section, self.sensitivity, self.beat_intensity, self.color_detail,
                    );
                    let status_bar = Paragraph::new(status)
                        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
                    f.render_widget(status_bar, chunks[1]);
                }
            })?;
            let draw_elapsed = draw_start.elapsed().as_secs_f32();

            // EMA with alpha ~1/30 (smooths over ~30 frames)
            self.render_time_ema = self.render_time_ema * 0.97 + draw_elapsed * 0.03;

            let frame_budget = frame_duration.as_secs_f32();
            if self.render_time_ema > frame_budget * 0.8 {
                // Struggling — reduce effective detail
                self.effective_color_detail = (self.effective_color_detail - 0.1).max(0.5);
            } else if self.render_time_ema < frame_budget * 0.5 {
                // Headroom — recover toward user's chosen detail
                self.effective_color_detail =
                    (self.effective_color_detail + 0.1).min(self.color_detail);
            }

            // Handle input
            let poll_timeout = frame_duration.saturating_sub(loop_start.elapsed());
            if event::poll(poll_timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        if !self.handle_key(key) {
                            // Forward to current visualization
                            self.registry.on_key_current(key);
                        }
                    }
                    Event::FocusLost => focused = false,
                    Event::FocusGained => focused = true,
                    _ => {}
                }
            }
        }

        // Save state before cleanup
        self.save_state();

        stdout().execute(DisableFocusChange)?;
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
                self.label_shown_at = Instant::now();
                true
            }
            KeyCode::BackTab => {
                self.registry.prev();
                self.label_shown_at = Instant::now();
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
            KeyCode::Char(']') => {
                self.color_detail = (self.color_detail + 0.1).min(2.0);
                self.effective_color_detail = self.color_detail;
                true
            }
            KeyCode::Char('[') => {
                self.color_detail = (self.color_detail - 0.1).max(0.5);
                self.effective_color_detail = self.color_detail;
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
        root.insert(
            "color_detail".to_string(),
            toml::Value::Float(self.color_detail as f64),
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
