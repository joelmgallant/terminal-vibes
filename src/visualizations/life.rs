#![allow(dead_code)]

use crate::processing::FrameData;
use crate::visualizations::render::{BrailleCanvas, HalfBlockCanvas};
use crate::visualizations::spectrum::ColorPalette;
use crate::visualizations::Visualization;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum RenderMode {
    Character,
    HalfBlock,
    Braille,
}

impl RenderMode {
    #[allow(dead_code)]
    fn next(self) -> Self {
        match self {
            RenderMode::Character => RenderMode::HalfBlock,
            RenderMode::HalfBlock => RenderMode::Braille,
            RenderMode::Braille => RenderMode::Character,
        }
    }

    fn name(self) -> &'static str {
        match self {
            RenderMode::Character => "character",
            RenderMode::HalfBlock => "halfblock",
            RenderMode::Braille => "braille",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "character" => Some(RenderMode::Character),
            "halfblock" => Some(RenderMode::HalfBlock),
            "braille" => Some(RenderMode::Braille),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Default)]
#[allow(dead_code)]
struct Cell {
    alive: bool,
    age: u16,
}

#[allow(dead_code)]
pub struct Life {
    // Simulation state
    current: Vec<Cell>,
    next: Vec<Cell>,
    grid_width: usize,
    grid_height: usize,
    tick_counter: u8,

    // Audio state
    rms: f32,
    peak: f32,
    spectrum: Vec<f32>,
    beat_envelope: f32,
    beat_fired: bool,
    bass_envelope: f32,
    treble_envelope: f32,
    bass_energy: f32,
    mid_energy: f32,
    treble_energy: f32,

    // Rendering
    render_mode: RenderMode,
    halfblock_canvas: HalfBlockCanvas,
    braille_canvas: BrailleCanvas,
    palette: ColorPalette,
    quant_step: u8,

    // Misc
    frame_counter: u32,
}

impl Default for Life {
    fn default() -> Self {
        Self::new()
    }
}

impl Life {
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            next: Vec::new(),
            grid_width: 0,
            grid_height: 0,
            tick_counter: 0,

            rms: 0.0,
            peak: 0.0,
            spectrum: Vec::new(),
            beat_envelope: 0.0,
            beat_fired: false,
            bass_envelope: 0.0,
            treble_envelope: 0.0,
            bass_energy: 0.0,
            mid_energy: 0.0,
            treble_energy: 0.0,

            render_mode: RenderMode::Character,
            halfblock_canvas: HalfBlockCanvas::new(1, 1),
            braille_canvas: BrailleCanvas::new(1, 1),
            palette: ColorPalette::Neon,
            quant_step: 16,

            frame_counter: 0,
        }
    }

    /// Ensure grid matches target dimensions, reinitializing if size changed.
    fn ensure_grid(&mut self, width: usize, height: usize) {
        if self.grid_width != width || self.grid_height != height {
            self.grid_width = width;
            self.grid_height = height;
            let size = width * height;
            self.current = vec![Cell::default(); size];
            self.next = vec![Cell::default(); size];
        }
    }

    /// Count live neighbors with toroidal wrapping.
    fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let w = self.grid_width;
        let h = self.grid_height;
        let mut count = 0u8;
        for dy in [h - 1, 0, 1] {
            for dx in [w - 1, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (x + dx) % w;
                let ny = (y + dy) % h;
                if self.current[ny * w + nx].alive {
                    count += 1;
                }
            }
        }
        count
    }

    /// Run one generation of the simulation with audio-warped rules.
    fn tick(&mut self) {
        let w = self.grid_width;
        let h = self.grid_height;
        if w == 0 || h == 0 {
            return;
        }

        // Audio-warped rules:
        // Bass envelope lowers birth threshold (can birth on 2 neighbors)
        // Treble envelope raises survival ceiling (survive on 1-4)
        let birth_min = if self.bass_envelope > 0.5 { 2 } else { 3 };
        let birth_max = 3;
        let survive_min = if self.treble_envelope > 0.5 { 1 } else { 2 };
        let survive_max = if self.treble_envelope > 0.3 { 4 } else { 3 };

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let neighbors = self.count_neighbors(x, y);
                let cell = self.current[idx];

                self.next[idx] = if cell.alive {
                    if neighbors >= survive_min && neighbors <= survive_max {
                        Cell {
                            alive: true,
                            age: cell.age.saturating_add(1).min(1000),
                        }
                    } else {
                        Cell::default()
                    }
                } else if neighbors >= birth_min && neighbors <= birth_max {
                    Cell {
                        alive: true,
                        age: 0,
                    }
                } else {
                    Cell::default()
                };
            }
        }

        std::mem::swap(&mut self.current, &mut self.next);
    }

    /// Seed cells based on audio energy. Frequency bands map spatially.
    fn seed_from_audio(&mut self) {
        let w = self.grid_width;
        let h = self.grid_height;
        if w == 0 || h == 0 || self.spectrum.is_empty() {
            return;
        }

        let spawn_density = self.rms * 0.3 + self.peak * 0.1;
        let third = h / 3;

        for x in 0..w {
            let band_idx = (x * self.spectrum.len()) / w;
            let energy = self.spectrum[band_idx.min(self.spectrum.len() - 1)];

            // Map frequency to vertical zone: bass=bottom, mid=middle, treble=top
            let (y_start, y_end) = if band_idx < self.spectrum.len() / 3 {
                (third * 2, h) // bass = bottom third
            } else if band_idx < self.spectrum.len() * 2 / 3 {
                (third, third * 2) // mid = middle third
            } else {
                (0, third) // treble = top third
            };

            // Probabilistic spawn based on energy and RMS
            let spawn_chance = energy * spawn_density;
            let hash = self
                .frame_counter
                .wrapping_mul(2654435761)
                .wrapping_add(x as u32);
            let rand_val = (hash >> 16) as f32 / 65536.0;

            if rand_val < spawn_chance && y_start < y_end {
                let y = y_start + (hash as usize % (y_end - y_start));
                let idx = y * w + x;
                if !self.current[idx].alive {
                    self.current[idx] = Cell {
                        alive: true,
                        age: 0,
                    };
                }
            }
        }
    }

    /// Spawn a classic pattern at a random position on beat.
    fn spawn_pattern_on_beat(&mut self) {
        if !self.beat_fired {
            return;
        }
        let w = self.grid_width;
        let h = self.grid_height;
        if w < 5 || h < 5 {
            return;
        }

        let hash = self.frame_counter.wrapping_mul(2654435761);
        let cx = (hash as usize) % (w - 4);
        let cy = ((hash >> 8) as usize) % (h - 4);

        // Pick pattern based on energy level
        let pattern: &[(usize, usize)] = if self.beat_envelope > 0.7 {
            // R-pentomino (chaotic, long-lived)
            &[(1, 0), (2, 0), (0, 1), (1, 1), (1, 2)]
        } else if self.beat_envelope > 0.4 {
            // Glider
            &[(2, 0), (0, 1), (2, 1), (1, 2), (2, 2)]
        } else {
            // Blinker
            &[(0, 1), (1, 1), (2, 1)]
        };

        for &(dx, dy) in pattern {
            let x = (cx + dx) % w;
            let y = (cy + dy) % h;
            let idx = y * w + x;
            self.current[idx] = Cell {
                alive: true,
                age: 0,
            };
        }
    }

    /// Compute grid dimensions for the current render mode and terminal area.
    fn grid_dims_for_area(&self, area: Rect) -> (usize, usize) {
        match self.render_mode {
            RenderMode::Character => (area.width as usize, area.height as usize),
            RenderMode::HalfBlock => (area.width as usize, area.height as usize * 2),
            RenderMode::Braille => (area.width as usize * 2, area.height as usize * 4),
        }
    }
}

impl Visualization for Life {
    fn name(&self) -> &str {
        "life"
    }

    fn update(&mut self, frame: &FrameData) {
        // Store audio state
        self.rms = frame.rms;
        self.peak = frame.peak;
        self.spectrum.resize(frame.spectrum.len(), 0.0);
        self.spectrum.copy_from_slice(&frame.spectrum);
        self.beat_envelope = frame.beat.envelope;
        self.beat_fired = frame.beat.beat;
        self.bass_envelope = frame.beat.bass_envelope;
        self.treble_envelope = frame.beat.treble_envelope;
        self.bass_energy = frame.beat.bass_energy;
        self.mid_energy = frame.beat.mid_energy;
        self.treble_energy = frame.beat.treble_energy;
        self.frame_counter = self.frame_counter.wrapping_add(1);

        // Seed cells from audio
        self.seed_from_audio();
        self.spawn_pattern_on_beat();

        // Tick simulation every 2 frames (~30 gen/sec at 60fps)
        // Extra tick on beat for time acceleration
        self.tick_counter = self.tick_counter.wrapping_add(1);
        if self.tick_counter % 2 == 0 || self.beat_fired {
            self.tick();
        }
    }

    fn render(&mut self, _area: Rect, _buf: &mut Buffer) {
        // TODO: Task 4
    }

    fn on_key(&mut self, _key: KeyEvent) -> bool {
        false // TODO: Task 5
    }

    fn heavy_rendering(&self) -> bool {
        self.render_mode == RenderMode::HalfBlock
    }

    fn set_quantization_step(&mut self, step: u8) {
        self.quant_step = step;
        self.halfblock_canvas.set_step(step);
        self.braille_canvas.set_step(step);
    }

    fn save_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        table.insert(
            "render_mode".to_string(),
            toml::Value::String(self.render_mode.name().to_string()),
        );
        table.insert(
            "palette".to_string(),
            toml::Value::String(self.palette.name().to_string()),
        );
        toml::Value::Table(table)
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(name) = config.get("render_mode").and_then(|v| v.as_str()) {
            if let Some(mode) = RenderMode::from_name(name) {
                self.render_mode = mode;
            }
        }
        if let Some(name) = config.get("palette").and_then(|v| v.as_str()) {
            if let Some(p) = ColorPalette::from_name(name) {
                self.palette = p;
            }
        }
    }
}
