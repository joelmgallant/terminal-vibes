use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::render::BrailleCanvas;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

struct Particle {
    x: f32,
    y: f32,
    z: f32,
}

impl Particle {
    fn new_random(seed: u32) -> Self {
        // Simple deterministic pseudo-random using seed
        let s1 = ((seed.wrapping_mul(1103515245).wrapping_add(12345)) as f32) / u32::MAX as f32;
        let s2 = ((seed.wrapping_mul(214013).wrapping_add(2531011)) as f32) / u32::MAX as f32;
        let s3 = ((seed.wrapping_mul(1664525).wrapping_add(1013904223)) as f32) / u32::MAX as f32;
        Self {
            x: (s1 - 0.5) * 2.0, // -1..1
            y: (s2 - 0.5) * 2.0, // -1..1
            z: s3 * 0.8 + 0.2,   // 0.2..1.0 (avoid z=0 division)
        }
    }

    fn reset_to_center(&mut self, seed: u32) {
        let s1 = ((seed.wrapping_mul(1103515245).wrapping_add(12345)) as f32) / u32::MAX as f32;
        let s2 = ((seed.wrapping_mul(214013).wrapping_add(2531011)) as f32) / u32::MAX as f32;
        self.x = (s1 - 0.5) * 0.2; // small spread near center
        self.y = (s2 - 0.5) * 0.2;
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
    color: Color,
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
        self.frame_counter = self.frame_counter.wrapping_add(1);

        // Speed: quiet = gentle drift, loud = warp speed
        let speed = 0.005 + self.rms * 0.03;

        // Peak spike: spawn burst of particles at center
        let peak_spike = self.peak > self.prev_peak + 0.1;

        for (i, particle) in self.particles.iter_mut().enumerate() {
            // Move toward viewer (decrease z)
            particle.z -= speed;

            // Recycle particles that pass the viewer
            if particle.z <= 0.01 {
                particle.reset_to_center(self.frame_counter.wrapping_add(i as u32));
            }

            // Peak burst: reset some particles to center
            if peak_spike && i % 8 == 0 {
                particle.reset_to_center(self.frame_counter.wrapping_add(i as u32 * 7));
            }
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut canvas = BrailleCanvas::new(area.width, area.height);
        let pw = canvas.pixel_width() as f32;
        let ph = canvas.pixel_height() as f32;
        let cx = pw / 2.0;
        let cy = ph / 2.0;

        for particle in &self.particles {
            // Perspective projection
            let screen_x = cx + (particle.x / particle.z) * cx;
            let screen_y = cy + (particle.y / particle.z) * cy;

            let px = screen_x as usize;
            let py = screen_y as usize;

            if px < canvas.pixel_width() && py < canvas.pixel_height() {
                canvas.set(px, py);

                // Closer particles get bigger (plot adjacent dots)
                if particle.z < 0.4 {
                    if px + 1 < canvas.pixel_width() {
                        canvas.set(px + 1, py);
                    }
                    if py + 1 < canvas.pixel_height() {
                        canvas.set(px, py + 1);
                    }
                }
            }
        }

        // Brightness based on RMS
        let brightness = (128.0 + self.rms * 127.0) as u8;
        let color = Color::Rgb(brightness, brightness, brightness);
        canvas.render(&area, buf, color);
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
