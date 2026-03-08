use crate::processing::FrameData;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub struct SpectrumBars {
    spectrum: Vec<f32>,
    bar_width: u16,
    colors: Vec<Color>,
}

impl SpectrumBars {
    pub fn new() -> Self {
        Self {
            spectrum: Vec::new(),
            bar_width: 2,
            colors: default_gradient(),
        }
    }
}

impl Visualization for SpectrumBars {
    fn name(&self) -> &str {
        "spectrum"
    }

    fn update(&mut self, frame: &FrameData) {
        self.spectrum = frame.spectrum.clone();
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.spectrum.is_empty() {
            return;
        }

        let num_bars = (area.width / self.bar_width.max(1)) as usize;
        let band_count = self.spectrum.len();

        for i in 0..num_bars.min(band_count) {
            let value = self.spectrum[i * band_count / num_bars.max(1)].clamp(0.0, 1.0);
            let bar_height = (value * area.height as f32) as u16;
            let x = area.x + (i as u16) * self.bar_width;

            let color_idx = (i * self.colors.len()) / num_bars.max(1);
            let color = self.colors[color_idx.min(self.colors.len() - 1)];

            for dy in 0..bar_height {
                let y = area.y + area.height - 1 - dy;
                for dx in 0..self.bar_width.min(area.width - (x - area.x)) {
                    if x + dx < area.x + area.width && y >= area.y {
                        buf[(x + dx, y)]
                            .set_char('\u{2588}')
                            .set_fg(color);
                    }
                }
            }
        }
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(bw) = config.get("bar_width").and_then(|v| v.as_integer()) {
            self.bar_width = (bw as u16).max(1);
        }
    }
}

fn default_gradient() -> Vec<Color> {
    vec![
        Color::from_u32(0x00ff0055), // red-pink (bass)
        Color::from_u32(0x00ffaa00), // orange
        Color::from_u32(0x0000ffaa), // green-cyan
        Color::from_u32(0x000055ff), // blue (treble)
    ]
}
