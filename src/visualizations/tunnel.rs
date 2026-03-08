use crate::processing::FrameData;
use crate::visualizations::render::HalfBlockCanvas;
use crate::visualizations::spectrum::ColorPalette;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::f32::consts::PI;

#[derive(Clone, Copy, PartialEq)]
enum TunnelShape {
    Circle,
    Hexagon,
    Square,
}

impl TunnelShape {
    fn next(self) -> Self {
        match self {
            TunnelShape::Circle => TunnelShape::Hexagon,
            TunnelShape::Hexagon => TunnelShape::Square,
            TunnelShape::Square => TunnelShape::Circle,
        }
    }

    #[allow(dead_code)]
    fn name(self) -> &'static str {
        match self {
            TunnelShape::Circle => "circle",
            TunnelShape::Hexagon => "hexagon",
            TunnelShape::Square => "square",
        }
    }
}

struct Ring {
    /// Radius (0.0 = center, 1.0 = edge of screen)
    radius: f32,
    color_t: f32,
}

pub struct Tunnel {
    rings: Vec<Ring>,
    shape: TunnelShape,
    palette: ColorPalette,
    time: f32,
    rms: f32,
    bass: f32,
    treble: f32,
    spawn_timer: f32,
    beat_envelope: f32,
    beat_fired: bool,
    canvas: HalfBlockCanvas,
}

impl Tunnel {
    pub fn new() -> Self {
        Self {
            rings: Vec::new(),
            shape: TunnelShape::Circle,
            palette: ColorPalette::Synthwave,
            time: 0.0,
            rms: 0.0,
            bass: 0.0,
            treble: 0.0,
            spawn_timer: 0.0,
            beat_envelope: 0.0,
            beat_fired: false,
            canvas: HalfBlockCanvas::new(0, 0),
        }
    }

    /// Get vertices for the current shape at a given radius and center
    fn shape_points(&self, cx: f32, cy: f32, radius: f32) -> Vec<(f32, f32)> {
        let n = match self.shape {
            TunnelShape::Circle => 48,
            TunnelShape::Hexagon => 6,
            TunnelShape::Square => 4,
        };
        let angle_offset = match self.shape {
            TunnelShape::Square => PI / 4.0,
            _ => 0.0,
        };
        (0..=n)
            .map(|i| {
                let angle = angle_offset + 2.0 * PI * i as f32 / n as f32;
                (cx + angle.cos() * radius, cy + angle.sin() * radius)
            })
            .collect()
    }
}

impl Visualization for Tunnel {
    fn name(&self) -> &str {
        "tunnel"
    }

    fn update(&mut self, frame: &FrameData) {
        self.rms = frame.rms;
        self.beat_envelope = frame.beat.envelope;
        self.beat_fired = frame.beat.beat;
        let band_count = frame.spectrum.len();
        if band_count > 0 {
            self.bass =
                frame.spectrum[..band_count / 4].iter().sum::<f32>() / (band_count / 4) as f32;
            self.treble =
                frame.spectrum[3 * band_count / 4..].iter().sum::<f32>() / (band_count / 4) as f32;
        }

        // Expand existing rings — envelope dramatically accelerates expansion
        let speed = 0.01 + self.rms * 0.03 + self.beat_envelope * 0.06;
        for ring in &mut self.rings {
            ring.radius += speed;
        }

        // Remove rings that have expanded beyond view
        self.rings.retain(|r| r.radius < 1.5);

        // Spawn new rings at center
        self.spawn_timer += 1.0 + self.bass * 3.0;
        let spawn_interval = 8.0 - self.rms * 4.0; // faster spawning when loud
        while self.spawn_timer >= spawn_interval {
            self.spawn_timer -= spawn_interval;
            self.rings.push(Ring {
                radius: 0.01,
                color_t: self.treble.clamp(0.0, 1.0),
            });
        }

        // Burst of rings on beat — dramatic shockwave
        if self.beat_fired {
            for i in 0..6 {
                self.rings.push(Ring {
                    radius: 0.01 + i as f32 * 0.03,
                    color_t: (self.treble + i as f32 * 0.15).fract(),
                });
            }
        }

        self.time += 0.016;
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
        let max_radius = cx.min(cy);

        // Draw rings from farthest to nearest (painter's algorithm)
        for ring in &self.rings {
            let r = ring.radius * max_radius;
            // Brightness fades with distance — envelope makes everything glow
            let envelope_boost = 0.5 + self.beat_envelope * 0.5;
            let brightness =
                ((1.0 - ring.radius) * envelope_boost + self.beat_envelope * 0.3).clamp(0.0, 1.0);
            let color = self.palette.color(ring.color_t);

            // Dim the color based on distance
            let color = if let ratatui::style::Color::Rgb(cr, cg, cb) = color {
                ratatui::style::Color::Rgb(
                    (cr as f32 * brightness) as u8,
                    (cg as f32 * brightness) as u8,
                    (cb as f32 * brightness) as u8,
                )
            } else {
                color
            };

            // Aspect ratio correction: terminal cells are ~2:1 tall
            let points = self.shape_points(cx, cy, r);
            for window in points.windows(2) {
                let (x0, y0) = window[0];
                let (x1, y1) = window[1];
                // Bresenham-ish line: step along the longer axis
                let dx = (x1 - x0).abs();
                let dy = (y1 - y0).abs();
                let steps = (dx.max(dy) as usize).max(1);
                for s in 0..=steps {
                    let t = s as f32 / steps as f32;
                    let px = (x0 + (x1 - x0) * t) as usize;
                    let py = (y0 + (y1 - y0) * t) as usize;
                    self.canvas.set(px, py, color);
                }
            }
        }

        self.canvas.render(&area, buf);
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('s') => {
                self.shape = self.shape.next();
                true
            }
            crossterm::event::KeyCode::Char('p') => {
                self.palette = ColorPalette::from_name(
                    ColorPalette::ALL
                        .iter()
                        .cycle()
                        .skip_while(|p| p.name() != self.palette.name())
                        .nth(1)
                        .map(|p| p.name())
                        .unwrap_or("neon"),
                )
                .unwrap_or(self.palette);
                true
            }
            crossterm::event::KeyCode::Char('P') => {
                let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
                let idx = names
                    .iter()
                    .position(|n| *n == self.palette.name())
                    .unwrap_or(0);
                let prev_idx = if idx == 0 { names.len() - 1 } else { idx - 1 };
                self.palette = ColorPalette::from_name(names[prev_idx]).unwrap_or(self.palette);
                true
            }
            _ => false,
        }
    }
}
