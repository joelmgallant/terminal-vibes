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

    // User-tunable transform bases (adjusted via keyboard)
    base_zoom: f32,      // base zoom per frame (1.0 = none)
    rotation_speed: f32, // base rotation radians per frame
    warp_intensity: f32, // warp displacement multiplier
    decay_factor: f32,   // trail fade (0.80–0.99)
    reactivity: f32,     // how much audio drives transforms (0.0–1.0)

    // Computed per-frame (derived from bases + audio * reactivity)
    rotation_angle: f32,
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

            base_zoom: 1.003,
            rotation_speed: 0.002,
            warp_intensity: 0.5,
            decay_factor: 0.92,
            reactivity: 0.3,

            rotation_angle: 0.0,
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
        let smooth = 0.3_f32;

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
        self.peak = frame.peak;
        self.beat_envelope = frame.beat.envelope;
        self.spectrum.clear();
        self.spectrum.extend_from_slice(&frame.spectrum);
        self.waveform_data.clear();
        self.waveform_data.extend_from_slice(&frame.waveform);
    }

    fn update_transforms(&mut self) {
        let r = self.reactivity;
        let beat_boost = 1.0 + self.beat_envelope * 0.3 * r;

        // Rotation: base speed + mid-driven, scaled by reactivity
        self.rotation_angle += (self.rotation_speed + self.mid * 0.01 * r) * beat_boost;

        // Warp grid: treble drives ripple, scaled by intensity and reactivity
        let ripple = self.treble * self.warp_intensity * r * beat_boost;
        let radial_push = self.warp_intensity * 0.3;
        for gy in 0..WARP_GRID_H {
            for gx in 0..WARP_GRID_W {
                let nx = gx as f32 / (WARP_GRID_W - 1) as f32 * 2.0 - 1.0;
                let ny = gy as f32 / (WARP_GRID_H - 1) as f32 * 2.0 - 1.0;
                let angle = ny.atan2(nx);
                let dist = (nx * nx + ny * ny).sqrt();
                let dx = dist * angle.cos() * radial_push
                    + SIN_LUT.get(self.time * 2.0 + dist * 5.0) * ripple;
                let dy = dist * angle.sin() * radial_push
                    + SIN_LUT.get(self.time * 2.3 + dist * 5.0) * ripple;
                self.warp_grid.set(gx, gy, (dx, dy));
            }
        }

        // Time advance
        self.time += 0.015 + self.rms * 0.02 * r;
    }

    /// Compute effective zoom for this frame from base + audio.
    fn effective_zoom(&self) -> f32 {
        let r = self.reactivity;
        let beat_boost = 1.0 + self.beat_envelope * 0.3 * r;
        (self.base_zoom + self.bass * 0.015 * r) * beat_boost
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

        self.waveform_hue += 0.01 + self.beat_envelope * 0.05 * self.reactivity;
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
        let r = self.reactivity;
        let radius = base_radius * (1.0 + self.bass * 1.5 * r + self.beat_envelope * 0.3 * r);

        let num_dots = self.spectrum.len().min(64);
        for i in 0..num_dots {
            let t = i as f32 / num_dots as f32;
            let angle = t * 2.0 * PI + self.time * 0.5;
            let mag = self.spectrum.get(i).copied().unwrap_or(0.0);
            let dot_r = radius * (0.5 + mag * 0.5);
            let x = (cx + dot_r * SIN_LUT.get(angle + PI / 2.0)).round() as usize;
            let y = (cy + dot_r * SIN_LUT.get(angle)).round() as usize;

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
            let burst = ((self.beat_envelope * 8.0 * self.reactivity) as usize)
                .min(128 - self.particles.len());
            let cx = pw / 2.0;
            let cy = ph / 2.0;
            for _ in 0..burst {
                let angle = self.time * 37.0 + self.particles.len() as f32 * 2.399;
                let speed = 0.5 + self.peak * 2.0 * self.reactivity;
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
        let zoom = self.effective_zoom();
        self.feedback.zoom_rotate(cx, cy, zoom, self.rotation_angle);

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
            // Layer toggles
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
            // Decay (trail length)
            KeyCode::Char('d') => {
                self.decay_factor = (self.decay_factor + 0.01).min(0.99);
                true
            }
            KeyCode::Char('D') => {
                self.decay_factor = (self.decay_factor - 0.01).max(0.80);
                true
            }
            // Base zoom
            KeyCode::Char('z') => {
                self.base_zoom = (self.base_zoom + 0.002).min(1.05);
                true
            }
            KeyCode::Char('Z') => {
                self.base_zoom = (self.base_zoom - 0.002).max(0.98);
                true
            }
            // Rotation speed
            KeyCode::Char('x') => {
                self.rotation_speed = (self.rotation_speed + 0.001).min(0.02);
                true
            }
            KeyCode::Char('X') => {
                self.rotation_speed = (self.rotation_speed - 0.001).max(0.0);
                true
            }
            // Warp intensity
            KeyCode::Char('c') => {
                self.warp_intensity = (self.warp_intensity + 0.1).min(3.0);
                true
            }
            KeyCode::Char('C') => {
                self.warp_intensity = (self.warp_intensity - 0.1).max(0.0);
                true
            }
            // Reactivity (audio→transform coupling)
            KeyCode::Char('v') => {
                self.reactivity = (self.reactivity + 0.05).min(1.0);
                true
            }
            KeyCode::Char('V') => {
                self.reactivity = (self.reactivity - 0.05).max(0.0);
                true
            }
            // Palette cycling
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
        table.insert("base_zoom".to_string(), toml::Value::Float(1.003));
        table.insert("rotation_speed".to_string(), toml::Value::Float(0.002));
        table.insert("warp_intensity".to_string(), toml::Value::Float(0.5));
        table.insert("reactivity".to_string(), toml::Value::Float(0.3));
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
        if let Some(v) = config.get("decay_factor").and_then(|v| v.as_float()) {
            self.decay_factor = (v as f32).clamp(0.80, 0.99);
        }
        if let Some(v) = config.get("base_zoom").and_then(|v| v.as_float()) {
            self.base_zoom = (v as f32).clamp(0.98, 1.05);
        }
        if let Some(v) = config.get("rotation_speed").and_then(|v| v.as_float()) {
            self.rotation_speed = (v as f32).clamp(0.0, 0.02);
        }
        if let Some(v) = config.get("warp_intensity").and_then(|v| v.as_float()) {
            self.warp_intensity = (v as f32).clamp(0.0, 3.0);
        }
        if let Some(v) = config.get("reactivity").and_then(|v| v.as_float()) {
            self.reactivity = (v as f32).clamp(0.0, 1.0);
        }
        if let Some(v) = config.get("waveform_enabled").and_then(|v| v.as_bool()) {
            self.waveform_enabled = v;
        }
        if let Some(v) = config.get("shapes_enabled").and_then(|v| v.as_bool()) {
            self.shapes_enabled = v;
        }
        if let Some(v) = config.get("particles_enabled").and_then(|v| v.as_bool()) {
            self.particles_enabled = v;
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
            "base_zoom".to_string(),
            toml::Value::Float(self.base_zoom as f64),
        );
        table.insert(
            "rotation_speed".to_string(),
            toml::Value::Float(self.rotation_speed as f64),
        );
        table.insert(
            "warp_intensity".to_string(),
            toml::Value::Float(self.warp_intensity as f64),
        );
        table.insert(
            "reactivity".to_string(),
            toml::Value::Float(self.reactivity as f64),
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
