use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::render::HalfBlockCanvas;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::f32::consts::PI;

pub struct Plasma {
    time: f32,
    /// Scaling factors modulated by spectrum bands
    k1: f32,
    k2: f32,
    k3: f32,
    k4: f32,
    rms: f32,
    hue_offset: f32,
}

impl Plasma {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            k1: 8.0,
            k2: 12.0,
            k3: 10.0,
            k4: 14.0,
            rms: 0.0,
            hue_offset: 0.0,
        }
    }
}

impl Visualization for Plasma {
    fn name(&self) -> &str {
        "plasma"
    }

    fn update(&mut self, frame: &FrameData) {
        self.rms = frame.rms;

        let band_count = frame.spectrum.len();
        if band_count >= 4 {
            let quarter = band_count / 4;
            let bass = frame.spectrum[..quarter].iter().sum::<f32>() / quarter as f32;
            let low_mid = frame.spectrum[quarter..quarter * 2].iter().sum::<f32>() / quarter as f32;
            let high_mid = frame.spectrum[quarter * 2..quarter * 3].iter().sum::<f32>() / quarter as f32;
            let treble = frame.spectrum[quarter * 3..].iter().sum::<f32>() / quarter as f32;

            // Bass stretches, treble tightens
            self.k1 = 6.0 + bass * 12.0;
            self.k2 = 8.0 + low_mid * 10.0;
            self.k3 = 7.0 + high_mid * 14.0;
            self.k4 = 10.0 + treble * 8.0;
        }

        // Peak boosts color saturation via hue rotation speed
        self.hue_offset += 0.02 + frame.peak * 0.05;
        self.time += 0.03 + self.rms * 0.05;
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut canvas = HalfBlockCanvas::new(area.width, area.height);
        let pw = canvas.pixel_width();
        let ph = canvas.pixel_height();

        for py in 0..ph {
            let y = py as f32 / ph as f32;
            for px in 0..pw {
                let x = px as f32 / pw as f32;

                // Classic plasma: sum of sine functions
                let v1 = (x * self.k1 + self.time).sin();
                let v2 = (y * self.k2 + self.time * 1.3).sin();
                let v3 = ((x + y) * self.k3 + self.time * 0.7).sin();
                let dist = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
                let v4 = (dist * self.k4 + self.time * 1.1).sin();

                let v = (v1 + v2 + v3 + v4) / 4.0; // -1.0..1.0
                let t = (v + 1.0) / 2.0; // normalize to 0.0..1.0

                let color = plasma_color(t, self.hue_offset);
                canvas.set(px, py, color);
            }
        }

        canvas.render(&area, buf);
    }
}

/// Map a value (0..1) and hue offset to an RGB color via HSV-like rotation.
fn plasma_color(t: f32, hue_offset: f32) -> Color {
    let hue = (t + hue_offset) % 1.0;
    let r = ((hue * 2.0 * PI).sin() * 0.5 + 0.5) * 255.0;
    let g = ((hue * 2.0 * PI + 2.094).sin() * 0.5 + 0.5) * 255.0; // +120°
    let b = ((hue * 2.0 * PI + 4.189).sin() * 0.5 + 0.5) * 255.0; // +240°
    Color::Rgb(r as u8, g as u8, b as u8)
}
