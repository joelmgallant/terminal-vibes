use crate::processing::FrameData;
use crate::visualizations::render::HalfBlockCanvas;
use crate::visualizations::spectrum::ColorPalette;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::f32::consts::PI;

pub struct RadialSpectrum {
    spectrum: Vec<f32>,
    rotation: f32,
    rms: f32,
    mirror: bool,
    palette: ColorPalette,
    beat_envelope: f32,
}

impl RadialSpectrum {
    pub fn new() -> Self {
        Self {
            spectrum: Vec::new(),
            rotation: 0.0,
            rms: 0.0,
            mirror: false,
            palette: ColorPalette::Neon,
            beat_envelope: 0.0,
        }
    }
}

impl Visualization for RadialSpectrum {
    fn name(&self) -> &str {
        "radial"
    }

    fn update(&mut self, frame: &FrameData) {
        self.spectrum = frame.spectrum.clone();
        self.rms = frame.rms;
        self.beat_envelope = frame.beat.envelope;
        // Beat envelope spins it hard
        self.rotation += 0.005 + self.rms * 0.02 + self.beat_envelope * 0.05;
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.spectrum.is_empty() {
            return;
        }

        let mut canvas = HalfBlockCanvas::new(area.width, area.height);
        let pw = canvas.pixel_width() as f32;
        let ph = canvas.pixel_height() as f32;
        let cx = pw / 2.0;
        let cy = ph / 2.0;
        let max_len = cx.min(cy) * 0.85;

        let num_rays = self.spectrum.len();

        for (i, &magnitude) in self.spectrum.iter().enumerate() {
            let t = i as f32 / num_rays as f32;
            let angle = self.rotation + t * 2.0 * PI;
            let base_color = self.palette.color(t);
            // Beat envelope drives brightness: dim when quiet, vivid on beat
            let color = if let ratatui::style::Color::Rgb(r, g, b) = base_color {
                let brightness = 0.3 + self.beat_envelope * 0.7;
                ratatui::style::Color::Rgb(
                    (r as f32 * brightness) as u8,
                    (g as f32 * brightness) as u8,
                    (b as f32 * brightness) as u8,
                )
            } else {
                base_color
            };

            // Rays extend further on beat
            let ray_len = magnitude * max_len * (1.0 + self.beat_envelope * 0.4);
            let steps = ray_len as usize;

            for s in 0..=steps {
                let r = s as f32;
                let px = (cx + angle.cos() * r) as usize;
                let py = (cy + angle.sin() * r) as usize;
                canvas.set(px, py, color);

                if self.mirror {
                    // Draw inward mirror
                    let px_in = (cx - angle.cos() * r) as usize;
                    let py_in = (cy - angle.sin() * r) as usize;
                    canvas.set(px_in, py_in, color);
                }
            }
        }

        canvas.render(&area, buf);
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('m') => {
                self.mirror = !self.mirror;
                true
            }
            crossterm::event::KeyCode::Char('p') => {
                let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
                let idx = names
                    .iter()
                    .position(|n| *n == self.palette.name())
                    .unwrap_or(0);
                self.palette =
                    ColorPalette::from_name(names[(idx + 1) % names.len()]).unwrap_or(self.palette);
                true
            }
            crossterm::event::KeyCode::Char('P') => {
                let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
                let idx = names
                    .iter()
                    .position(|n| *n == self.palette.name())
                    .unwrap_or(0);
                let prev = if idx == 0 { names.len() - 1 } else { idx - 1 };
                self.palette = ColorPalette::from_name(names[prev]).unwrap_or(self.palette);
                true
            }
            _ => false,
        }
    }
}
