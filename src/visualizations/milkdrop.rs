use crate::processing::FrameData;
use crate::visualizations::feedback::{BlendMode, FeedbackCanvas, WarpGrid};
use crate::visualizations::render::{HalfBlockCanvas, SIN_LUT};
use crate::visualizations::spectrum::ColorPalette;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::f32::consts::PI;

const WARP_GRID_W: usize = 16;
const WARP_GRID_H: usize = 12;

pub struct Milkdrop {
    feedback: FeedbackCanvas,
    canvas: HalfBlockCanvas,
    warp_grid: WarpGrid,
    palette: ColorPalette,

    // Audio state (EMA smoothed)
    bass: f32,
    mid: f32,
    treble: f32,
    rms: f32,
    peak: f32,
    beat_envelope: f32,

    // Transform parameters
    zoom_amount: f32,
    rotation_angle: f32,
    decay_factor: f32,
    time: f32,

    // Layer toggles
    waveform_enabled: bool,
    shapes_enabled: bool,
    particles_enabled: bool,

    // Waveform state
    waveform_hue: f32,

    // Particles state
    particles: Vec<Particle>,

    // Spectrum data (kept for shapes layer)
    spectrum: Vec<f32>,
    waveform_data: Vec<f32>,
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    hue: f32,
}

impl Default for Milkdrop {
    fn default() -> Self {
        Self::new()
    }
}

impl Milkdrop {
    pub fn new() -> Self {
        Self {
            feedback: FeedbackCanvas::new(0, 0),
            canvas: HalfBlockCanvas::new(0, 0),
            warp_grid: WarpGrid::new(WARP_GRID_W, WARP_GRID_H),
            palette: ColorPalette::Synthwave,

            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            rms: 0.0,
            peak: 0.0,
            beat_envelope: 0.0,

            zoom_amount: 1.02,
            rotation_angle: 0.0,
            decay_factor: 0.92,
            time: 0.0,

            waveform_enabled: true,
            shapes_enabled: true,
            particles_enabled: true,

            waveform_hue: 0.0,

            particles: Vec::with_capacity(128),

            spectrum: Vec::new(),
            waveform_data: Vec::new(),
        }
    }

    fn update_audio(&mut self, frame: &FrameData) {
        let smooth = 0.3_f32; // EMA factor

        let band_count = frame.spectrum.len();
        if band_count >= 3 {
            let third = band_count / 3;
            let raw_bass = frame.spectrum[..third].iter().sum::<f32>() / third as f32;
            let raw_mid = frame.spectrum[third..third * 2].iter().sum::<f32>() / third as f32;
            let raw_treble = frame.spectrum[third * 2..].iter().sum::<f32>() / third as f32;
            self.bass = self.bass * (1.0 - smooth) + raw_bass * smooth;
            self.mid = self.mid * (1.0 - smooth) + raw_mid * smooth;
            self.treble = self.treble * (1.0 - smooth) + raw_treble * smooth;
        }

        self.rms = self.rms * (1.0 - smooth) + frame.rms * smooth;
        self.peak = frame.peak; // peak is instant, don't smooth
        self.beat_envelope = frame.beat.envelope;
        self.spectrum.clear();
        self.spectrum.extend_from_slice(&frame.spectrum);
        self.waveform_data.clear();
        self.waveform_data.extend_from_slice(&frame.waveform);
    }

    fn update_transforms(&mut self) {
        let beat_boost = 1.0 + self.beat_envelope * 0.5;

        // Zoom: bass pushes outward
        self.zoom_amount = (1.01 + self.bass * 0.04) * beat_boost;

        // Rotation: mid energy drives spin
        self.rotation_angle += (0.005 + self.mid * 0.03) * beat_boost;

        // Warp grid: treble drives ripple
        let ripple = self.treble * 3.0 * beat_boost;
        for gy in 0..WARP_GRID_H {
            for gx in 0..WARP_GRID_W {
                let nx = gx as f32 / (WARP_GRID_W - 1) as f32 * 2.0 - 1.0;
                let ny = gy as f32 / (WARP_GRID_H - 1) as f32 * 2.0 - 1.0;
                let angle = ny.atan2(nx);
                let dist = (nx * nx + ny * ny).sqrt();
                // Radial outward push + tangential ripple
                let dx =
                    dist * angle.cos() * 0.5 + SIN_LUT.get(self.time * 2.0 + dist * 5.0) * ripple;
                let dy =
                    dist * angle.sin() * 0.5 + SIN_LUT.get(self.time * 2.3 + dist * 5.0) * ripple;
                self.warp_grid.set(gx, gy, (dx, dy));
            }
        }

        // Time advance: scales with audio energy
        self.time += 0.03 + self.rms * 0.05;
    }

    fn paint_waveform(&mut self) {
        if !self.waveform_enabled || self.waveform_data.is_empty() {
            return;
        }
        let pw = self.feedback.pixel_width();
        let ph = self.feedback.pixel_height();
        if pw == 0 || ph == 0 {
            return;
        }

        self.waveform_hue += 0.01 + self.beat_envelope * 0.05;
        let hue = self.waveform_hue % 1.0;
        let color = hue_to_rgb(hue);
        let brightness = 0.5 + self.peak * 0.5;
        let color = (
            color.0 * brightness,
            color.1 * brightness,
            color.2 * brightness,
        );

        let cy = ph as f32 / 2.0;
        let samples = &self.waveform_data;
        let step = samples.len() as f32 / pw as f32;

        let mut prev_x: Option<isize> = None;
        let mut prev_y: Option<isize> = None;

        for px in 0..pw {
            let si = (px as f32 * step) as usize;
            let sample = samples.get(si).copied().unwrap_or(0.0);
            let y = (cy + sample * cy * 0.8).round() as isize;
            let x = px as isize;

            if let (Some(px_prev), Some(py_prev)) = (prev_x, prev_y) {
                self.feedback
                    .paint_line(px_prev, py_prev, x, y, color, BlendMode::Additive);
            }
            prev_x = Some(x);
            prev_y = Some(y);
        }
    }

    fn paint_shapes(&mut self) {
        if !self.shapes_enabled || self.spectrum.is_empty() {
            return;
        }
        let pw = self.feedback.pixel_width();
        let ph = self.feedback.pixel_height();
        if pw == 0 || ph == 0 {
            return;
        }

        let cx = pw as f32 / 2.0;
        let cy = ph as f32 / 2.0;
        let base_radius = (pw.min(ph) as f32) * 0.15;
        let radius = base_radius * (1.0 + self.bass * 2.0 + self.beat_envelope * 0.5);

        let num_dots = self.spectrum.len().min(64);
        for i in 0..num_dots {
            let t = i as f32 / num_dots as f32;
            let angle = t * 2.0 * PI + self.time * 0.5;
            let mag = self.spectrum.get(i).copied().unwrap_or(0.0);
            let r = radius * (0.5 + mag * 0.5);
            let x = (cx + r * SIN_LUT.get(angle + PI / 2.0)).round() as usize;
            let y = (cy + r * SIN_LUT.get(angle)).round() as usize;

            let color = self.palette.color(t);
            let (cr, cg, cb) = match color {
                ratatui::style::Color::Rgb(r, g, b) => {
                    (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
                }
                _ => (1.0, 1.0, 1.0),
            };
            let brightness = 0.3 + mag * 0.7;
            self.feedback.paint(
                x,
                y,
                (cr * brightness, cg * brightness, cb * brightness),
                BlendMode::Additive,
            );
        }
    }

    fn update_particles(&mut self) {
        let pw = self.feedback.pixel_width() as f32;
        let ph = self.feedback.pixel_height() as f32;
        if pw == 0.0 || ph == 0.0 {
            return;
        }

        // Spawn on beat
        if self.beat_envelope > 0.5 && self.particles.len() < 128 {
            let burst = ((self.beat_envelope * 8.0) as usize).min(128 - self.particles.len());
            let cx = pw / 2.0;
            let cy = ph / 2.0;
            for _ in 0..burst {
                let angle = self.time * 37.0 + self.particles.len() as f32 * 2.399; // golden angle-ish
                let speed = 1.0 + self.peak * 3.0;
                self.particles.push(Particle {
                    x: cx,
                    y: cy,
                    vx: angle.cos() * speed,
                    vy: angle.sin() * speed,
                    life: 1.0,
                    hue: (self.waveform_hue + self.particles.len() as f32 * 0.1) % 1.0,
                });
            }
        }

        // Update existing
        self.particles.retain_mut(|p| {
            p.x += p.vx;
            p.y += p.vy;
            p.life -= 0.015;
            p.life > 0.0 && p.x >= 0.0 && p.x < pw && p.y >= 0.0 && p.y < ph
        });
    }

    fn paint_particles(&mut self) {
        if !self.particles_enabled {
            return;
        }
        for p in &self.particles {
            let color = hue_to_rgb(p.hue);
            let brightness = p.life;
            self.feedback.paint(
                p.x as usize,
                p.y as usize,
                (
                    color.0 * brightness,
                    color.1 * brightness,
                    color.2 * brightness,
                ),
                BlendMode::Additive,
            );
        }
    }
}

/// Convert hue (0.0–1.0) to float RGB.
fn hue_to_rgb(hue: f32) -> (f32, f32, f32) {
    let h = hue.fract() * 6.0;
    let f = h.fract();
    match h as u8 {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0 - f),
    }
}

impl Visualization for Milkdrop {
    fn name(&self) -> &str {
        "milkdrop"
    }

    fn update(&mut self, frame: &FrameData) {
        self.update_audio(frame);
        self.update_transforms();
        self.update_particles();
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.feedback.resize(area.width, area.height);
        self.canvas.resize_or_clear(area.width, area.height);

        let pw = self.feedback.pixel_width() as f32;
        let ph = self.feedback.pixel_height() as f32;
        let cx = pw / 2.0;
        let cy = ph / 2.0;

        // 1. Swap — previous frame becomes read source
        self.feedback.swap();

        // 2. Transform — zoom + rotate the previous frame
        self.feedback
            .zoom_rotate(cx, cy, self.zoom_amount, self.rotation_angle * 0.02);

        // 3. Decay — fade trails
        self.feedback.decay(self.decay_factor);

        // 4. Paint layers
        self.paint_waveform();
        self.paint_shapes();
        self.paint_particles();

        // 5. Convert to HalfBlockCanvas for ratatui output
        self.feedback.to_halfblock(&mut self.canvas);

        // 6. Render
        self.canvas.render(&area, buf);
    }

    fn heavy_rendering(&self) -> bool {
        true
    }

    fn set_quantization_step(&mut self, step: u8) {
        self.canvas.set_step(step);
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('w') => {
                self.waveform_enabled = !self.waveform_enabled;
                true
            }
            KeyCode::Char('e') => {
                self.shapes_enabled = !self.shapes_enabled;
                true
            }
            KeyCode::Char('r') => {
                self.particles_enabled = !self.particles_enabled;
                true
            }
            KeyCode::Char('d') => {
                self.decay_factor = (self.decay_factor + 0.01).min(0.99);
                true
            }
            KeyCode::Char('D') => {
                self.decay_factor = (self.decay_factor - 0.01).max(0.80);
                true
            }
            KeyCode::Char('z') => {
                self.zoom_amount = (self.zoom_amount + 0.005).min(1.15);
                true
            }
            KeyCode::Char('Z') => {
                self.zoom_amount = (self.zoom_amount - 0.005).max(0.95);
                true
            }
            KeyCode::Char('p') => {
                self.palette = ColorPalette::ALL[(ColorPalette::ALL
                    .iter()
                    .position(|&p| p == self.palette)
                    .unwrap_or(0)
                    + 1)
                    % ColorPalette::ALL.len()];
                true
            }
            KeyCode::Char('P') => {
                let idx = ColorPalette::ALL
                    .iter()
                    .position(|&p| p == self.palette)
                    .unwrap_or(0);
                self.palette = ColorPalette::ALL
                    [(idx + ColorPalette::ALL.len() - 1) % ColorPalette::ALL.len()];
                true
            }
            _ => false,
        }
    }

    fn default_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        table.insert(
            "palette".to_string(),
            toml::Value::String("synthwave".to_string()),
        );
        table.insert("decay_factor".to_string(), toml::Value::Float(0.92));
        table.insert("waveform_enabled".to_string(), toml::Value::Boolean(true));
        table.insert("shapes_enabled".to_string(), toml::Value::Boolean(true));
        table.insert("particles_enabled".to_string(), toml::Value::Boolean(true));
        toml::Value::Table(table)
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(name) = config.get("palette").and_then(|v| v.as_str()) {
            if let Some(p) = ColorPalette::from_name(name) {
                self.palette = p;
            }
        }
        if let Some(d) = config.get("decay_factor").and_then(|v| v.as_float()) {
            self.decay_factor = (d as f32).clamp(0.80, 0.99);
        }
        if let Some(w) = config.get("waveform_enabled").and_then(|v| v.as_bool()) {
            self.waveform_enabled = w;
        }
        if let Some(s) = config.get("shapes_enabled").and_then(|v| v.as_bool()) {
            self.shapes_enabled = s;
        }
        if let Some(p) = config.get("particles_enabled").and_then(|v| v.as_bool()) {
            self.particles_enabled = p;
        }
    }

    fn save_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        table.insert(
            "palette".to_string(),
            toml::Value::String(self.palette.name().to_string()),
        );
        table.insert(
            "decay_factor".to_string(),
            toml::Value::Float(self.decay_factor as f64),
        );
        table.insert(
            "waveform_enabled".to_string(),
            toml::Value::Boolean(self.waveform_enabled),
        );
        table.insert(
            "shapes_enabled".to_string(),
            toml::Value::Boolean(self.shapes_enabled),
        );
        table.insert(
            "particles_enabled".to_string(),
            toml::Value::Boolean(self.particles_enabled),
        );
        toml::Value::Table(table)
    }
}
