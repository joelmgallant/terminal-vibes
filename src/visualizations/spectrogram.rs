use crate::processing::FrameData;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::VecDeque;

pub struct Spectrogram {
    history: VecDeque<Vec<f32>>,
    /// Track which frames had a beat for marker rendering
    beat_markers: VecDeque<bool>,
    max_history: usize,
}

impl Spectrogram {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            beat_markers: VecDeque::with_capacity(max_history),
            max_history,
        }
    }
}

impl Visualization for Spectrogram {
    fn name(&self) -> &str {
        "spectrogram"
    }

    fn update(&mut self, frame: &FrameData) {
        self.history.push_back(frame.spectrum.clone());
        self.beat_markers.push_back(frame.beat.beat);
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }
        while self.beat_markers.len() > self.max_history {
            self.beat_markers.pop_front();
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.history.is_empty() {
            return;
        }

        let cols = area.width as usize;
        let rows = area.height as usize;

        // Show the most recent `cols` frames (time flows left to right)
        let visible_entries: Vec<(&Vec<f32>, bool)> = self
            .history
            .iter()
            .zip(self.beat_markers.iter())
            .rev()
            .take(cols)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|(spec, &beat)| (spec, beat))
            .collect();
        let visible_history: Vec<&Vec<f32>> = visible_entries.iter().map(|(s, _)| *s).collect();
        let visible_beats: Vec<bool> = visible_entries.iter().map(|(_, b)| *b).collect();

        let x_offset = if visible_history.len() < cols {
            cols - visible_history.len()
        } else {
            0
        };

        for (col_idx, spectrum) in visible_history.iter().enumerate() {
            let x = area.x + (x_offset + col_idx) as u16;
            if x >= area.x + area.width {
                continue;
            }

            for row in 0..rows {
                // Map row to frequency band (bottom = low freq, top = high freq)
                let band_idx = ((rows - 1 - row) * spectrum.len()) / rows.max(1);
                let band_idx = band_idx.min(spectrum.len().saturating_sub(1));
                let intensity = if spectrum.is_empty() {
                    0.0
                } else {
                    spectrum[band_idx].clamp(0.0, 1.0)
                };

                let y = area.y + row as u16;
                // Boost intensity on beat frames for a bright column marker
                let is_beat = visible_beats.get(col_idx).copied().unwrap_or(false);
                let display_intensity = if is_beat {
                    (intensity + 0.3).clamp(0.0, 1.0)
                } else {
                    intensity
                };
                let color = magma_colormap(display_intensity);

                buf[(x, y)]
                    .set_char(intensity_char(display_intensity))
                    .set_fg(color);
            }
        }
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(len) = config.get("history_length").and_then(|v| v.as_integer()) {
            self.max_history = len as usize;
        }
    }

    fn save_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        table.insert(
            "history_length".to_string(),
            toml::Value::Integer(self.max_history as i64),
        );
        toml::Value::Table(table)
    }
}

/// Map intensity (0.0..1.0) to a block character of varying density.
fn intensity_char(intensity: f32) -> char {
    match (intensity * 4.0) as u8 {
        0 => ' ',
        1 => '\u{2591}', // light shade
        2 => '\u{2592}', // medium shade
        3 => '\u{2593}', // dark shade
        _ => '\u{2588}', // full block
    }
}

/// Simple magma-ish colormap: black -> purple -> orange -> yellow.
fn magma_colormap(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.33 {
        let s = t / 0.33;
        ((s * 120.0) as u8, 0u8, (s * 150.0) as u8)
    } else if t < 0.66 {
        let s = (t - 0.33) / 0.33;
        (
            120 + (s * 135.0) as u8,
            (s * 80.0) as u8,
            150 - (s * 150.0) as u8,
        )
    } else {
        let s = (t - 0.66) / 0.34;
        (255, 80 + (s * 175.0) as u8, (s * 80.0) as u8)
    };
    Color::Rgb(r, g, b)
}
