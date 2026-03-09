use crate::processing::FrameData;
use crate::visualizations::render::BrailleCanvas;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

struct Particle {
    x: f32,
    y: f32,
    z: f32,
}

/// Integer hash for pseudo-random particle placement.
/// The previous LCG approach (seed * multiplier + offset) didn't wrap for small
/// seeds, causing all y-values to cluster near -1.0 (stars flew "overhead").
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    x
}

fn hash_f32(seed: u32, channel: u32) -> f32 {
    hash_u32(seed.wrapping_add(channel.wrapping_mul(0x9E3779B9))) as f32 / u32::MAX as f32
}

impl Particle {
    fn new_random(seed: u32) -> Self {
        Self {
            x: (hash_f32(seed, 0) - 0.5) * 2.0, // -1..1
            y: (hash_f32(seed, 1) - 0.5) * 2.0, // -1..1
            z: hash_f32(seed, 2) * 0.8 + 0.2,   // 0.2..1.0 (avoid z=0 division)
        }
    }

    fn reset_to_center(&mut self, seed: u32) {
        self.x = (hash_f32(seed, 0) - 0.5) * 0.2; // small spread near center
        self.y = (hash_f32(seed, 1) - 0.5) * 0.2;
        self.z = 1.0; // start far away
    }
}

pub struct Starfield {
    particles: Vec<Particle>,
    rms: f32,
    peak: f32,
    prev_peak: f32,
    frame_counter: u32,
    density: usize,
    #[allow(dead_code)]
    color: Color,
    beat_envelope: f32,
    beat_fired: bool,
    canvas: BrailleCanvas,
}

impl Default for Starfield {
    fn default() -> Self {
        Self::new()
    }
}

impl Starfield {
    pub fn new() -> Self {
        Self::with_density(500)
    }

    fn with_density(n: usize) -> Self {
        let particles = (0..n as u32).map(Particle::new_random).collect();
        Self {
            particles,
            rms: 0.0,
            peak: 0.0,
            prev_peak: 0.0,
            frame_counter: 0,
            density: n,
            color: Color::White,
            beat_envelope: 0.0,
            beat_fired: false,
            canvas: BrailleCanvas::new(0, 0),
        }
    }
}

impl Visualization for Starfield {
    fn name(&self) -> &str {
        "starfield"
    }

    fn update(&mut self, frame: &FrameData) {
        self.prev_peak = self.peak;
        self.rms = frame.rms;
        self.peak = frame.peak;
        self.beat_envelope = frame.beat.envelope;
        self.beat_fired = frame.beat.beat;
        self.frame_counter = self.frame_counter.wrapping_add(1);

        // Speed: quiet = gentle drift, loud = warp speed, beat = HYPERSPACE
        let speed = 0.005 + self.rms * 0.03 + self.beat_envelope * 0.06;

        // Beat or peak spike: spawn burst of particles at center
        let peak_spike = self.peak > self.prev_peak + 0.1 || self.beat_fired;

        for (i, particle) in self.particles.iter_mut().enumerate() {
            // Move toward viewer (decrease z)
            particle.z -= speed;

            // Recycle particles that pass the viewer
            if particle.z <= 0.01 {
                particle.reset_to_center(self.frame_counter.wrapping_add(i as u32));
            }

            // Peak/beat burst: reset particles to center for warp effect
            if peak_spike && i % 4 == 0 {
                particle.reset_to_center(self.frame_counter.wrapping_add(i as u32 * 7));
            }
        }
    }

    fn set_quantization_step(&mut self, step: u8) {
        self.canvas.set_step(step);
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

        // Use uniform focal length so stars radiate symmetrically from center.
        // Different focal lengths (cx vs cy) would distort the radial motion.
        let focal = cx.min(cy);

        for particle in &self.particles {
            // Perspective projection
            let screen_x = cx + (particle.x / particle.z) * focal;
            let screen_y = cy + (particle.y / particle.z) * focal;

            let px = screen_x as usize;
            let py = screen_y as usize;

            if px < self.canvas.pixel_width() && py < self.canvas.pixel_height() {
                self.canvas.set(px, py);

                // Closer particles get bigger (plot adjacent dots)
                if particle.z < 0.4 {
                    if px + 1 < self.canvas.pixel_width() {
                        self.canvas.set(px + 1, py);
                    }
                    if py + 1 < self.canvas.pixel_height() {
                        self.canvas.set(px, py + 1);
                    }
                }
            }
        }

        // Brightness pulses with beat — dim drift, bright burst
        let brightness = (80.0 + self.rms * 50.0 + self.beat_envelope * 125.0) as u8;
        let color = Color::Rgb(brightness, brightness, brightness);
        self.canvas.render(&area, buf, color);
    }

    fn help_keys(&self) -> &[(&str, &str)] {
        &[("d", "cycle density")]
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('d') => {
                self.density = match self.density {
                    250 => 500,
                    500 => 1000,
                    _ => 250,
                };
                self.particles = (0..self.density as u32).map(Particle::new_random).collect();
                true
            }
            _ => false,
        }
    }
}
