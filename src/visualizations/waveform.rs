use crate::processing::FrameData;
use crate::visualizations::render::quantize_color;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub struct Waveform {
    samples: Vec<f32>,
    color: Color,
    beat_envelope: f32,
}

impl Waveform {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            color: Color::from_u32(0x0000ff88),
            beat_envelope: 0.0,
        }
    }
}

impl Visualization for Waveform {
    fn name(&self) -> &str {
        "waveform"
    }

    fn update(&mut self, frame: &FrameData) {
        self.samples = frame.waveform.clone();
        self.beat_envelope = frame.beat.envelope;
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.samples.is_empty() {
            return;
        }

        let mid_y = area.y + area.height / 2;

        // Beat envelope drives brightness: dim between beats, vivid on beats
        let draw_color = quantize_color(if let Color::Rgb(r, g, b) = self.color {
            let brightness = 0.3 + self.beat_envelope * 0.7;
            Color::Rgb(
                (r as f32 * brightness) as u8,
                (g as f32 * brightness) as u8,
                (b as f32 * brightness) as u8,
            )
        } else {
            self.color
        });

        for x in 0..area.width {
            // Map terminal column to sample index
            let sample_idx = (x as usize * self.samples.len()) / area.width as usize;
            let sample = self.samples[sample_idx.min(self.samples.len() - 1)];

            // Map sample (-1.0..1.0) to y position
            let half_h = area.height as f32 / 2.0;
            let y_offset = (-sample * half_h) as i16;
            let y = (mid_y as i16 + y_offset)
                .clamp(area.y as i16, (area.y + area.height - 1) as i16) as u16;

            buf[(area.x + x, y)]
                .set_char('\u{2022}') // bullet dot
                .set_fg(draw_color);

            // Draw a vertical line from mid to point for thickness
            let (y_start, y_end) = if y < mid_y { (y, mid_y) } else { (mid_y, y) };
            for fill_y in y_start..=y_end {
                if fill_y >= area.y && fill_y < area.y + area.height {
                    buf[(area.x + x, fill_y)]
                        .set_char('\u{2502}') // thin vertical line
                        .set_fg(draw_color);
                }
            }
            // Overwrite the point itself with a solid dot
            buf[(area.x + x, y)].set_char('\u{2022}').set_fg(draw_color);
        }
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(color_str) = config.get("color").and_then(|v| v.as_str()) {
            if let Some(c) = parse_hex_color(color_str) {
                self.color = c;
            }
        }
    }

    fn save_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        if let Color::Rgb(r, g, b) = self.color {
            table.insert(
                "color".to_string(),
                toml::Value::String(format!("#{:02x}{:02x}{:02x}", r, g, b)),
            );
        }
        toml::Value::Table(table)
    }
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
