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
}

impl Visualization for Life {
    fn name(&self) -> &str {
        "life"
    }

    fn update(&mut self, _frame: &FrameData) {
        // TODO: Task 3
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
