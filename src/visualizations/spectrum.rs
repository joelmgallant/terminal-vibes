use crate::processing::FrameData;
use crate::visualizations::render::quantize_color;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq)]
pub enum ColorPalette {
    Neon,
    Fire,
    Ocean,
    Sunset,
    Matrix,
    Ice,
    GruvboxDark,
    GruvboxLight,
    Synthwave,
    Outrun,
    Retrowave,
    Pastel,
}

impl ColorPalette {
    pub const ALL: &[ColorPalette] = &[
        ColorPalette::Neon,
        ColorPalette::Fire,
        ColorPalette::Ocean,
        ColorPalette::Sunset,
        ColorPalette::Matrix,
        ColorPalette::Ice,
        ColorPalette::GruvboxDark,
        ColorPalette::GruvboxLight,
        ColorPalette::Synthwave,
        ColorPalette::Outrun,
        ColorPalette::Retrowave,
        ColorPalette::Pastel,
    ];

    fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn name(self) -> &'static str {
        match self {
            ColorPalette::Neon => "neon",
            ColorPalette::Fire => "fire",
            ColorPalette::Ocean => "ocean",
            ColorPalette::Sunset => "sunset",
            ColorPalette::Matrix => "matrix",
            ColorPalette::Ice => "ice",
            ColorPalette::GruvboxDark => "gruvbox-dark",
            ColorPalette::GruvboxLight => "gruvbox-light",
            ColorPalette::Synthwave => "synthwave",
            ColorPalette::Outrun => "outrun",
            ColorPalette::Retrowave => "retrowave",
            ColorPalette::Pastel => "pastel",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().find(|p| p.name() == name).copied()
    }

    /// Returns a color for position `t` that wraps seamlessly for cycling.
    /// A small fraction of the cycle blends from the last palette color back
    /// to the first, eliminating the hard edge at the wrap boundary.
    pub fn color_cyclic(self, t: f32) -> Color {
        let wrap_frac = 0.125; // 12.5% of cycle for smooth wrap-around
        let t = t.fract().max(0.0);
        let main_end = 1.0 - wrap_frac;

        if t <= main_end {
            self.color(t / main_end)
        } else {
            let s = (t - main_end) / wrap_frac;
            let end_color = self.color(1.0);
            let start_color = self.color(0.0);
            match (end_color, start_color) {
                (Color::Rgb(r0, g0, b0), Color::Rgb(r1, g1, b1)) => {
                    Color::Rgb(lerp_u8(r0, r1, s), lerp_u8(g0, g1, s), lerp_u8(b0, b1, s))
                }
                _ => start_color,
            }
        }
    }

    /// Returns a color for position `t` (0.0 = bass/left, 1.0 = treble/right)
    pub fn color(self, t: f32) -> Color {
        match self {
            ColorPalette::Neon => gradient_neon(t),
            ColorPalette::Fire => gradient_fire(t),
            ColorPalette::Ocean => gradient_ocean(t),
            ColorPalette::Sunset => gradient_sunset(t),
            ColorPalette::Matrix => gradient_matrix(t),
            ColorPalette::Ice => gradient_ice(t),
            ColorPalette::GruvboxDark => gradient_gruvbox_dark(t),
            ColorPalette::GruvboxLight => gradient_gruvbox_light(t),
            ColorPalette::Synthwave => gradient_synthwave(t),
            ColorPalette::Outrun => gradient_outrun(t),
            ColorPalette::Retrowave => gradient_retrowave(t),
            ColorPalette::Pastel => gradient_pastel(t),
        }
    }
}

pub struct SpectrumBars {
    spectrum: Vec<f32>,
    gap: bool,
    chunky: bool,
    palette: ColorPalette,
    cycling: bool,
    phase: f32,
    beat_envelope: f32,
    beat_fired: bool,
}

impl SpectrumBars {
    pub fn new() -> Self {
        Self {
            spectrum: Vec::new(),
            gap: false,
            chunky: true,
            palette: ColorPalette::Neon,
            cycling: false,
            phase: 0.0,
            beat_envelope: 0.0,
            beat_fired: false,
        }
    }
}

impl Visualization for SpectrumBars {
    fn name(&self) -> &str {
        "spectrum"
    }

    fn update(&mut self, frame: &FrameData) {
        self.spectrum.resize(frame.spectrum.len(), 0.0);
        self.spectrum.copy_from_slice(&frame.spectrum);
        self.beat_envelope = frame.beat.envelope;
        self.beat_fired = frame.beat.beat;
        // Color cycling accelerates dramatically with beat
        let cycle_speed = if self.cycling {
            0.005 + self.beat_envelope * 0.04
        } else if self.beat_fired {
            // Even without cycling enabled, beat shifts palette momentarily
            0.0
        } else {
            0.0
        };
        if cycle_speed > 0.0 {
            self.phase = (self.phase + cycle_speed) % 1.0;
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.spectrum.is_empty() {
            return;
        }

        let bar_width: u16 = if self.gap { 2 } else { 1 };
        let bar_step = if self.gap { bar_width + 1 } else { bar_width };
        let num_bars = ((area.width + 1) / bar_step) as usize;
        let band_count = self.spectrum.len();

        // Sub-block characters for smooth vertical resolution (eighths)
        let blocks = [
            ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}',
            '\u{2587}', '\u{2588}',
        ];

        for i in 0..num_bars {
            // Map bar index to spectrum band with interpolation
            let t = i as f32 / num_bars.max(1) as f32;
            let src = t * (band_count - 1) as f32;
            let idx = src as usize;
            let frac = src - idx as f32;
            let value = if idx + 1 < band_count {
                self.spectrum[idx] * (1.0 - frac) + self.spectrum[idx + 1] * frac
            } else {
                self.spectrum[idx.min(band_count - 1)]
            };
            // Beat envelope pumps bar height significantly
            let beat_scale = 1.0 + self.beat_envelope * self.beat_envelope * 0.8;
            let value = (value * beat_scale).clamp(0.0, 1.0);

            let x = area.x + (i as u16) * bar_step;
            let color_t = if self.cycling {
                (t + self.phase) % 1.0
            } else {
                t
            };
            let base_color = if self.cycling {
                self.palette.color_cyclic(color_t)
            } else {
                self.palette.color(color_t)
            };
            // Bars stay bright, beat adds a white-hot glow on top
            let color = quantize_color(if let Color::Rgb(r, g, b) = base_color {
                let boost = self.beat_envelope * 0.4;
                Color::Rgb(
                    (r as f32 + (255.0 - r as f32) * boost) as u8,
                    (g as f32 + (255.0 - g as f32) * boost) as u8,
                    (b as f32 + (255.0 - b as f32) * boost) as u8,
                )
            } else {
                base_color
            });

            if self.chunky {
                let full_cells = (value * area.height as f32).round() as u16;
                for dy in 0..full_cells {
                    let y = area.y + area.height - 1 - dy;
                    if y >= area.y {
                        for dx in 0..bar_width {
                            if x + dx < area.x + area.width {
                                buf[(x + dx, y)].set_char('\u{2588}').set_fg(color);
                            }
                        }
                    }
                }
            } else {
                let total_eighths = (value * area.height as f32 * 8.0) as u16;
                let full_cells = total_eighths / 8;
                let remainder = (total_eighths % 8) as usize;

                for dy in 0..full_cells {
                    let y = area.y + area.height - 1 - dy;
                    if y >= area.y {
                        for dx in 0..bar_width {
                            if x + dx < area.x + area.width {
                                buf[(x + dx, y)].set_char('\u{2588}').set_fg(color);
                            }
                        }
                    }
                }

                if remainder > 0 {
                    let y = area.y + area.height - 1 - full_cells;
                    if y >= area.y {
                        for dx in 0..bar_width {
                            if x + dx < area.x + area.width {
                                buf[(x + dx, y)].set_char(blocks[remainder]).set_fg(color);
                            }
                        }
                    }
                }
            }
        }
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('g') => {
                self.gap = !self.gap;
                true
            }
            crossterm::event::KeyCode::Char('c') => {
                self.chunky = !self.chunky;
                true
            }
            crossterm::event::KeyCode::Char('r') => {
                self.cycling = !self.cycling;
                if !self.cycling {
                    self.phase = 0.0;
                }
                true
            }
            crossterm::event::KeyCode::Char('p') => {
                self.palette = self.palette.next();
                true
            }
            crossterm::event::KeyCode::Char('P') => {
                self.palette = self.palette.prev();
                true
            }
            _ => false,
        }
    }

    fn save_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        table.insert(
            "palette".to_string(),
            toml::Value::String(self.palette.name().to_string()),
        );
        table.insert("gap".to_string(), toml::Value::Boolean(self.gap));
        table.insert("chunky".to_string(), toml::Value::Boolean(self.chunky));
        table.insert("cycling".to_string(), toml::Value::Boolean(self.cycling));
        toml::Value::Table(table)
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(name) = config.get("palette").and_then(|v| v.as_str()) {
            if let Some(p) = ColorPalette::from_name(name) {
                self.palette = p;
            }
        }
        if let Some(gap) = config.get("gap").and_then(|v| v.as_bool()) {
            self.gap = gap;
        }
        if let Some(chunky) = config.get("chunky").and_then(|v| v.as_bool()) {
            self.chunky = chunky;
        }
        if let Some(cycling) = config.get("cycling").and_then(|v| v.as_bool()) {
            self.cycling = cycling;
        }
    }
}

// --- Color Palettes ---

/// Neon: magenta → blue → cyan → green
fn gradient_neon(t: f32) -> Color {
    if t < 0.33 {
        let s = t / 0.33;
        Color::Rgb(
            lerp_u8(255, 50, s),
            lerp_u8(0, 100, s),
            lerp_u8(150, 255, s),
        )
    } else if t < 0.66 {
        let s = (t - 0.33) / 0.33;
        Color::Rgb(
            lerp_u8(50, 0, s),
            lerp_u8(100, 230, s),
            lerp_u8(255, 255, s),
        )
    } else {
        let s = (t - 0.66) / 0.34;
        Color::Rgb(
            lerp_u8(0, 50, s),
            lerp_u8(230, 255, s),
            lerp_u8(255, 100, s),
        )
    }
}

/// Fire: deep red → red → orange → yellow
fn gradient_fire(t: f32) -> Color {
    if t < 0.33 {
        let s = t / 0.33;
        Color::Rgb(lerp_u8(120, 255, s), lerp_u8(0, 30, s), lerp_u8(0, 0, s))
    } else if t < 0.66 {
        let s = (t - 0.33) / 0.33;
        Color::Rgb(lerp_u8(255, 255, s), lerp_u8(30, 150, s), lerp_u8(0, 0, s))
    } else {
        let s = (t - 0.66) / 0.34;
        Color::Rgb(
            lerp_u8(255, 255, s),
            lerp_u8(150, 240, s),
            lerp_u8(0, 50, s),
        )
    }
}

/// Ocean: deep navy → teal → bright aqua
fn gradient_ocean(t: f32) -> Color {
    if t < 0.33 {
        let s = t / 0.33;
        Color::Rgb(lerp_u8(10, 0, s), lerp_u8(20, 100, s), lerp_u8(80, 180, s))
    } else if t < 0.66 {
        let s = (t - 0.33) / 0.33;
        Color::Rgb(lerp_u8(0, 0, s), lerp_u8(100, 200, s), lerp_u8(180, 220, s))
    } else {
        let s = (t - 0.66) / 0.34;
        Color::Rgb(
            lerp_u8(0, 100, s),
            lerp_u8(200, 255, s),
            lerp_u8(220, 255, s),
        )
    }
}

/// Sunset: deep purple → red → orange → gold
fn gradient_sunset(t: f32) -> Color {
    if t < 0.25 {
        let s = t / 0.25;
        Color::Rgb(lerp_u8(80, 180, s), lerp_u8(0, 20, s), lerp_u8(120, 100, s))
    } else if t < 0.50 {
        let s = (t - 0.25) / 0.25;
        Color::Rgb(
            lerp_u8(180, 240, s),
            lerp_u8(20, 50, s),
            lerp_u8(100, 30, s),
        )
    } else if t < 0.75 {
        let s = (t - 0.50) / 0.25;
        Color::Rgb(lerp_u8(240, 255, s), lerp_u8(50, 140, s), lerp_u8(30, 0, s))
    } else {
        let s = (t - 0.75) / 0.25;
        Color::Rgb(
            lerp_u8(255, 255, s),
            lerp_u8(140, 220, s),
            lerp_u8(0, 50, s),
        )
    }
}

/// Matrix: dark green → bright green (monochrome hacker vibes)
fn gradient_matrix(t: f32) -> Color {
    Color::Rgb(lerp_u8(0, 30, t), lerp_u8(80, 255, t), lerp_u8(0, 20, t))
}

/// Ice: white → light blue → deep blue
fn gradient_ice(t: f32) -> Color {
    if t < 0.5 {
        let s = t / 0.5;
        Color::Rgb(
            lerp_u8(240, 150, s),
            lerp_u8(250, 200, s),
            lerp_u8(255, 255, s),
        )
    } else {
        let s = (t - 0.5) / 0.5;
        Color::Rgb(
            lerp_u8(150, 40, s),
            lerp_u8(200, 80, s),
            lerp_u8(255, 220, s),
        )
    }
}

/// Gruvbox Dark: red → orange → yellow → green → aqua → blue → purple
/// Uses gruvbox bright colors on dark background
fn gradient_gruvbox_dark(t: f32) -> Color {
    // Bright: red #fb4934, orange #fe8019, yellow #fabd2f,
    //         green #b8bb26, aqua #8ec07c, blue #83a598, purple #d3869b
    const STOPS: [(u8, u8, u8); 7] = [
        (0xfb, 0x49, 0x34), // red
        (0xfe, 0x80, 0x19), // orange
        (0xfa, 0xbd, 0x2f), // yellow
        (0xb8, 0xbb, 0x26), // green
        (0x8e, 0xc0, 0x7c), // aqua
        (0x83, 0xa5, 0x98), // blue
        (0xd3, 0x86, 0x9b), // purple
    ];
    multi_stop_gradient(&STOPS, t)
}

/// Gruvbox Light: red → orange → yellow → green → aqua → blue → purple
/// Uses gruvbox faded (dark) colors suited for light backgrounds
fn gradient_gruvbox_light(t: f32) -> Color {
    // Faded: red #9d0006, orange #af3a03, yellow #b57614,
    //        green #79740e, aqua #427b58, blue #076678, purple #8f3f71
    const STOPS: [(u8, u8, u8); 7] = [
        (0x9d, 0x00, 0x06), // red
        (0xaf, 0x3a, 0x03), // orange
        (0xb5, 0x76, 0x14), // yellow
        (0x79, 0x74, 0x0e), // green
        (0x42, 0x7b, 0x58), // aqua
        (0x07, 0x66, 0x78), // blue
        (0x8f, 0x3f, 0x71), // purple
    ];
    multi_stop_gradient(&STOPS, t)
}

/// Synthwave: deep violet → hot magenta → electric blue → cyan
/// Classic 80s synth album cover energy
fn gradient_synthwave(t: f32) -> Color {
    const STOPS: [(u8, u8, u8); 5] = [
        (0x2b, 0x00, 0x4a), // deep violet
        (0x8b, 0x00, 0xff), // electric purple
        (0xff, 0x00, 0x80), // hot magenta
        (0x00, 0x80, 0xff), // electric blue
        (0x00, 0xff, 0xf0), // cyan
    ];
    multi_stop_gradient(&STOPS, t)
}

/// Outrun: dark purple → hot pink → orange → yellow
/// That sunset-over-a-grid aesthetic
fn gradient_outrun(t: f32) -> Color {
    const STOPS: [(u8, u8, u8); 5] = [
        (0x1a, 0x00, 0x33), // near-black purple
        (0x7b, 0x2d, 0x8e), // purple
        (0xff, 0x2e, 0x63), // hot pink
        (0xff, 0x6b, 0x35), // orange
        (0xff, 0xd3, 0x19), // golden yellow
    ];
    multi_stop_gradient(&STOPS, t)
}

/// Retrowave: neon pink → neon purple → neon blue
/// Maximum neon sign vibes
fn gradient_retrowave(t: f32) -> Color {
    const STOPS: [(u8, u8, u8); 5] = [
        (0xff, 0x71, 0xce), // neon pink
        (0xff, 0x00, 0xdc), // magenta
        (0xb9, 0x67, 0xff), // neon purple
        (0x01, 0xcd, 0xfe), // neon blue
        (0x05, 0xff, 0xc1), // neon mint
    ];
    multi_stop_gradient(&STOPS, t)
}

/// Pastel: soft pink → lavender → baby blue → mint → soft yellow
fn gradient_pastel(t: f32) -> Color {
    const STOPS: [(u8, u8, u8); 6] = [
        (0xff, 0xb3, 0xba), // soft pink
        (0xd5, 0xaa, 0xff), // lavender
        (0xa8, 0xd8, 0xea), // baby blue
        (0xb5, 0xea, 0xd7), // mint
        (0xff, 0xf1, 0xb0), // soft yellow
        (0xff, 0xcc, 0xb6), // peach
    ];
    multi_stop_gradient(&STOPS, t)
}

/// Interpolate through an array of color stops evenly spaced across t=0..1
fn multi_stop_gradient(stops: &[(u8, u8, u8)], t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let n = stops.len();
    if n == 0 {
        return Color::White;
    }
    if n == 1 {
        let (r, g, b) = stops[0];
        return Color::Rgb(r, g, b);
    }
    let seg = t * (n - 1) as f32;
    let idx = (seg as usize).min(n - 2);
    let frac = seg - idx as f32;
    let (r0, g0, b0) = stops[idx];
    let (r1, g1, b1) = stops[idx + 1];
    Color::Rgb(
        lerp_u8(r0, r1, frac),
        lerp_u8(g0, g1, frac),
        lerp_u8(b0, b1, frac),
    )
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}
