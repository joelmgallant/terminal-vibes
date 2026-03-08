use crate::processing::FrameData;
use crate::visualizations::render::BrailleCanvas;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::VecDeque;
use std::f32::consts::PI;

/// Frequency ratios that produce interesting Lissajous patterns
const RATIOS: [(f32, f32); 8] = [
    (1.0, 2.0),
    (2.0, 3.0),
    (3.0, 4.0),
    (3.0, 2.0),
    (4.0, 3.0),
    (5.0, 4.0),
    (3.0, 5.0),
    (5.0, 6.0),
];

pub struct Lissajous {
    time: f32,
    ratio_index: usize,
    ratio_drift_timer: f32,
    frozen: bool,
    phase_x: f32,
    phase_y: f32,
    peak: f32,
    rms: f32,
    /// Trail: recent curve snapshots for fading effect
    trail: VecDeque<Vec<(f32, f32)>>,
    color: Color,
    beat_envelope: f32,
    beat_fired: bool,
    canvas: BrailleCanvas,
}

impl Lissajous {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            ratio_index: 0,
            ratio_drift_timer: 0.0,
            frozen: false,
            phase_x: 0.0,
            phase_y: 0.0,
            peak: 0.0,
            rms: 0.0,
            trail: VecDeque::new(),
            color: Color::Rgb(0, 255, 200),
            beat_envelope: 0.0,
            beat_fired: false,
            canvas: BrailleCanvas::new(0, 0),
        }
    }

    fn current_ratio(&self) -> (f32, f32) {
        RATIOS[self.ratio_index % RATIOS.len()]
    }
}

impl Visualization for Lissajous {
    fn name(&self) -> &str {
        "lissajous"
    }

    fn update(&mut self, frame: &FrameData) {
        self.peak = frame.peak;
        self.rms = frame.rms;
        self.beat_envelope = frame.beat.envelope;
        self.beat_fired = frame.beat.beat;

        // Bass energy modulates phase_x, mid energy modulates phase_y
        // Beat envelope adds extra phase velocity
        let band_count = frame.spectrum.len();
        if band_count > 0 {
            let bass =
                frame.spectrum[..band_count / 3].iter().sum::<f32>() / (band_count / 3) as f32;
            let mid = frame.spectrum[band_count / 3..2 * band_count / 3]
                .iter()
                .sum::<f32>()
                / (band_count / 3) as f32;
            let envelope_boost = 1.0 + self.beat_envelope * 1.5;
            self.phase_x += bass * 0.1 * envelope_boost;
            self.phase_y += mid * 0.1 * envelope_boost;
        }

        // Drift to next ratio — beat can trigger immediate jump
        if !self.frozen {
            self.ratio_drift_timer += 1.0;
            let should_advance = self.ratio_drift_timer > 240.0
                || (self.beat_fired && self.ratio_drift_timer > 60.0);
            if should_advance {
                self.ratio_drift_timer = 0.0;
                self.ratio_index = (self.ratio_index + 1) % RATIOS.len();
            }
        }

        // Generate the current curve
        let (a, b) = self.current_ratio();
        // Scale pumps with beat envelope — breathes with the music
        let scale = 0.2 + self.peak * 0.4 + self.beat_envelope * 0.4;
        let num_points = 500;
        let curve: Vec<(f32, f32)> = (0..num_points)
            .map(|i| {
                let t = i as f32 / num_points as f32 * 2.0 * PI;
                let x = (a * t + self.phase_x + self.time).sin() * scale;
                let y = (b * t + self.phase_y).sin() * scale;
                (x, y)
            })
            .collect();

        // Push to trail, keep last N frames
        let max_trail = 5 + (self.rms * 15.0) as usize; // 5-20 frames based on energy
        self.trail.push_back(curve);
        while self.trail.len() > max_trail {
            self.trail.pop_front();
        }

        self.time += 0.02 + self.rms * 0.03;
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.canvas.resize_or_clear(area.width, area.height);
        let pw = self.canvas.pixel_width() as f32;
        let ph = self.canvas.pixel_height() as f32;
        let cx = pw / 2.0;
        let cy = ph / 2.0;
        let scale = cx.min(cy) * 0.9;

        // Draw trail (all frames)
        for curve in &self.trail {
            for &(x, y) in curve {
                let px = (cx + x * scale) as usize;
                let py = (cy + y * scale) as usize;
                self.canvas.set(px, py);
            }
        }

        // Beat envelope drives trail brightness — dim between beats, vivid on beat
        let draw_color = if let Color::Rgb(r, g, b) = self.color {
            let boost = 0.2 + self.beat_envelope * 0.8;
            Color::Rgb(
                (r as f32 * boost) as u8,
                (g as f32 * boost) as u8,
                (b as f32 * boost) as u8,
            )
        } else {
            self.color
        };
        self.canvas.render(&area, buf, draw_color);
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('f') => {
                self.frozen = !self.frozen;
                true
            }
            crossterm::event::KeyCode::Char('r') => {
                self.ratio_index = (self.ratio_index + 1) % RATIOS.len();
                self.ratio_drift_timer = 0.0;
                true
            }
            _ => false,
        }
    }
}
