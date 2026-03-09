use crate::processing::FrameData;
use crate::visualizations::render::HalfBlockCanvas;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::f32::consts::PI;

struct Curtain {
    /// Base vertical position (0.0 = top, 1.0 = bottom)
    base_y: f32,
    /// Wave frequency
    freq: f32,
    /// Wave speed
    speed: f32,
    /// Height multiplier
    height: f32,
    /// Color (R, G, B)
    color: (u8, u8, u8),
}

pub struct Aurora {
    curtains: Vec<Curtain>,
    time: f32,
    rms: f32,
    peak: f32,
    band_energies: Vec<f32>,
    num_layers: usize,
    beat_envelope: f32,
    canvas: HalfBlockCanvas,
    column_colors: Vec<(f32, f32, f32)>,
}

impl Default for Aurora {
    fn default() -> Self {
        Self::new()
    }
}

impl Aurora {
    pub fn new() -> Self {
        Self {
            curtains: Self::make_curtains(3),
            time: 0.0,
            rms: 0.0,
            peak: 0.0,
            band_energies: Vec::new(),
            num_layers: 3,
            beat_envelope: 0.0,
            canvas: HalfBlockCanvas::new(0, 0),
            column_colors: Vec::new(),
        }
    }

    fn make_curtains(n: usize) -> Vec<Curtain> {
        #[allow(clippy::type_complexity)]
        let configs: Vec<(f32, f32, f32, f32, (u8, u8, u8))> = vec![
            (0.7, 2.0, 0.3, 0.4, (0, 200, 100)),  // green, low
            (0.5, 3.0, 0.5, 0.3, (0, 150, 255)),  // blue, mid
            (0.3, 4.5, 0.7, 0.2, (180, 0, 255)),  // purple, high
            (0.4, 3.5, 0.4, 0.25, (0, 255, 200)), // cyan, mid-high
        ];
        configs[..n.min(configs.len())]
            .iter()
            .map(|&(base_y, freq, speed, height, color)| Curtain {
                base_y,
                freq,
                speed,
                height,
                color,
            })
            .collect()
    }
}

impl Visualization for Aurora {
    fn name(&self) -> &str {
        "aurora"
    }

    fn update(&mut self, frame: &FrameData) {
        self.rms = frame.rms;
        self.peak = frame.peak;
        self.beat_envelope = frame.beat.envelope;

        // Split spectrum into per-curtain band energies
        let n = self.curtains.len();
        let band_count = frame.spectrum.len();
        self.band_energies.clear();
        if band_count > 0 && n > 0 {
            let chunk = band_count / n;
            for i in 0..n {
                let start = i * chunk;
                let end = if i == n - 1 {
                    band_count
                } else {
                    (i + 1) * chunk
                };
                let energy = frame.spectrum[start..end].iter().sum::<f32>() / (end - start) as f32;
                self.band_energies.push(energy);
            }
        }

        self.time += 0.02 + self.rms * 0.04;
    }

    fn set_quantization_step(&mut self, step: u8) {
        self.canvas.set_step(step);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.canvas.resize_or_clear(area.width, area.height);
        let pw = self.canvas.pixel_width();
        let ph = self.canvas.pixel_height();

        // For each column, compute each curtain's contribution
        for px in 0..pw {
            let x = px as f32 / pw as f32;

            // Reuse column color buffer instead of allocating per column
            self.column_colors.resize(ph, (0.0, 0.0, 0.0));
            self.column_colors.fill((0.0, 0.0, 0.0));

            for (i, curtain) in self.curtains.iter().enumerate() {
                let energy = self.band_energies.get(i).copied().unwrap_or(0.3);

                // Curtain center oscillates with sine wave
                let wave = (x * curtain.freq * PI + self.time * curtain.speed).sin();
                let center = curtain.base_y + wave * 0.1;
                // Beat envelope dramatically expands curtains
                let height = curtain.height * (0.3 + energy + self.beat_envelope * 0.8);

                // Brightness pulses hard with beat — dim between, vivid on beat
                let brightness = 0.3 + energy * 0.2 + self.beat_envelope * 0.6;

                for py in 0..ph {
                    let y = py as f32 / ph as f32;
                    let dist = (y - center).abs();
                    if dist < height {
                        // Smooth falloff from center
                        let falloff = 1.0 - (dist / height);
                        let falloff = falloff * falloff; // quadratic
                        let intensity = falloff * brightness;

                        self.column_colors[py].0 += curtain.color.0 as f32 * intensity;
                        self.column_colors[py].1 += curtain.color.1 as f32 * intensity;
                        self.column_colors[py].2 += curtain.color.2 as f32 * intensity;
                    }
                }
            }

            // Write blended colors to canvas
            for (py, &(r, g, b)) in self.column_colors.iter().enumerate() {
                // Skip pixels that would quantize to invisible black (matches STEP=16)
                if r.max(g).max(b) > 16.0 {
                    let color = Color::Rgb(r as u8, g as u8, b as u8);
                    self.canvas.set(px, py, color);
                }
            }
        }

        self.canvas.render(&area, buf);
    }

    fn heavy_rendering(&self) -> bool {
        true
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('l') => {
                self.num_layers = match self.num_layers {
                    2 => 3,
                    3 => 4,
                    _ => 2,
                };
                self.curtains = Self::make_curtains(self.num_layers);
                true
            }
            _ => false,
        }
    }
}
