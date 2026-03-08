use crate::processing::FrameData;
use crate::visualizations::render::quantize_color;
use crate::visualizations::spectrum::ColorPalette;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

struct Drop {
    y: f32,      // current row position
    speed: f32,  // rows per update
    length: u16, // tail length
    brightness: f32,
}

struct Column {
    drops: Vec<Drop>,
    spawn_timer: f32,
}

pub struct Rain {
    columns: Vec<Column>,
    rms: f32,
    peak: f32,
    spectrum: Vec<f32>,
    thick: bool,
    palette: ColorPalette,
    frame_counter: u32,
    beat_envelope: f32,
    beat_fired: bool,
}

impl Rain {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rms: 0.0,
            peak: 0.0,
            spectrum: Vec::new(),
            thick: false,
            palette: ColorPalette::Matrix,
            frame_counter: 0,
            beat_envelope: 0.0,
            beat_fired: false,
        }
    }

    fn ensure_columns(&mut self, width: usize) {
        while self.columns.len() < width {
            self.columns.push(Column {
                drops: Vec::new(),
                spawn_timer: 0.0,
            });
        }
        self.columns.truncate(width);
    }

    #[allow(dead_code)]
    fn column_energy(&self, col: usize, total_cols: usize) -> f32 {
        if self.spectrum.is_empty() || total_cols == 0 {
            return 0.3;
        }
        let band_idx = (col * self.spectrum.len()) / total_cols;
        self.spectrum[band_idx.min(self.spectrum.len() - 1)]
    }
}

impl Visualization for Rain {
    fn name(&self) -> &str {
        "rain"
    }

    fn update(&mut self, frame: &FrameData) {
        self.rms = frame.rms;
        self.peak = frame.peak;
        self.spectrum.resize(frame.spectrum.len(), 0.0);
        self.spectrum.copy_from_slice(&frame.spectrum);
        self.beat_envelope = frame.beat.envelope;
        self.beat_fired = frame.beat.beat;
        self.frame_counter = self.frame_counter.wrapping_add(1);

        // Ensure at least some columns exist (render will clip to area width)
        if self.columns.is_empty() {
            self.ensure_columns(200);
        }

        // Beat envelope surges drop speed dramatically
        let global_speed = 0.3 + self.rms * 0.7 + self.beat_envelope * 1.5;
        let num_cols = self.columns.len();

        for (col_idx, column) in self.columns.iter_mut().enumerate() {
            // Move existing drops
            for drop in &mut column.drops {
                drop.y += drop.speed * global_speed;
            }

            // Remove drops that have fallen off screen (generous bound)
            column.drops.retain(|d| d.y < 200.0);

            // Spawn new drops based on column energy
            let energy = if !self.spectrum.is_empty() && num_cols > 0 {
                let band_idx = (col_idx * self.spectrum.len()) / num_cols;
                self.spectrum[band_idx.min(self.spectrum.len() - 1)]
            } else {
                0.3
            };

            column.spawn_timer += energy * 0.5 + 0.05;
            let spawn_threshold = 1.5 - energy * 0.8;

            // Beat storm: force-spawn drops across all columns on beat
            if self.beat_fired && column.drops.len() < 8 {
                let seed = self.frame_counter.wrapping_add(col_idx as u32 * 17);
                let speed_var = ((seed.wrapping_mul(1103515245) >> 16) as f32 / 65536.0) * 0.3;
                column.drops.push(Drop {
                    y: 0.0,
                    speed: 0.6 + speed_var,
                    length: 6 + (energy * 8.0) as u16,
                    brightness: 1.0,
                });
            }

            if column.spawn_timer >= spawn_threshold && column.drops.len() < 8 {
                column.spawn_timer = 0.0;
                // Pseudo-random speed variation
                let seed = self.frame_counter.wrapping_add(col_idx as u32 * 31);
                let speed_var = ((seed.wrapping_mul(1103515245) >> 16) as f32 / 65536.0) * 0.4;

                column.drops.push(Drop {
                    y: 0.0,
                    speed: 0.5 + speed_var,
                    length: 4 + (energy * 12.0) as u16,
                    brightness: 0.7 + self.peak * 0.3,
                });
            }
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Lazy column init on first render
        let width = area.width as usize;
        let height = area.height;

        let head_char = if self.thick { '┃' } else { '│' };
        let tail_char = if self.thick { '║' } else { '│' };

        for (col_idx, column) in self.columns.iter().enumerate() {
            if col_idx >= width {
                break;
            }
            let x = area.x + col_idx as u16;
            let t = col_idx as f32 / width.max(1) as f32;
            let base_color = self.palette.color(t);

            for drop in &column.drops {
                let head_y = drop.y as i32;

                for dy in 0..=drop.length as i32 {
                    let row = head_y - dy;
                    if row < 0 || row >= height as i32 {
                        continue;
                    }
                    let y = area.y + row as u16;

                    // Brightness fades from head to tail
                    let fade = 1.0 - (dy as f32 / drop.length as f32);
                    let fade = fade * drop.brightness;

                    // Beat envelope brightens all drops
                    let beat_brightness = 0.4 + self.beat_envelope * 0.6;
                    let color = quantize_color(if let Color::Rgb(r, g, b) = base_color {
                        Color::Rgb(
                            (r as f32 * fade * beat_brightness) as u8,
                            (g as f32 * fade * beat_brightness) as u8,
                            (b as f32 * fade * beat_brightness) as u8,
                        )
                    } else {
                        base_color
                    });

                    let ch = if dy == 0 { head_char } else { tail_char };
                    buf[(x, y)].set_char(ch).set_fg(color);
                }
            }
        }
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('t') => {
                self.thick = !self.thick;
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
