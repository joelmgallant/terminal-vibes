# Milkdrop-Style Visualizers Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add 7 new Milkdrop-inspired visualizations (Lissajous, Tunnel, Radial Spectrum, Plasma, Aurora, Starfield, Rain) with shared rendering utilities and a stubbed BeatData interface.

**Architecture:** Two shared rendering canvases (BrailleCanvas for dot-based rendering, HalfBlockCanvas for 2x-resolution color fields) live in `visualizations/render.rs`. Each visualizer is a standalone module implementing the `Visualization` trait. `BeatData` is added to `FrameData` as a stub struct defaulting to zero/false, ready for future beat detection.

**Tech Stack:** Rust, ratatui (Buffer/Rect), existing ColorPalette system, existing Visualization trait.

---

### Task 1: Add BeatData stub to FrameData

**Files:**
- Modify: `src/processing.rs:1-10` (add BeatData struct and field)
- Modify: `tests/spectrum_test.rs` (update FrameData construction)
- Modify: `tests/waveform_test.rs` (update FrameData construction)
- Modify: `tests/spectrogram_test.rs` (update FrameData construction)

**Step 1: Add BeatData struct and wire into FrameData**

In `src/processing.rs`, add above `FrameData`:

```rust
#[derive(Debug, Clone, Default)]
pub struct BeatData {
    /// Per-band beat detected this frame
    pub bass_beat: bool,
    pub mid_beat: bool,
    pub treble_beat: bool,

    /// Per-band pulse envelopes (0.0..1.0, fast attack / configurable decay)
    pub bass_pulse: f32,
    pub mid_pulse: f32,
    pub treble_pulse: f32,

    /// Per-band energy levels (0.0..1.0, pre-threshold continuous values)
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,
}
```

Add field to `FrameData`:

```rust
#[derive(Debug, Clone, Default)]
pub struct FrameData {
    pub spectrum: Vec<f32>,
    pub waveform: Vec<f32>,
    pub peak: f32,
    pub rms: f32,
    pub beat: BeatData,
}
```

The `Processor::process()` method already builds `FrameData` with struct literal syntax — since `BeatData` derives `Default`, add `beat: BeatData::default()` to the return.

**Step 2: Update existing tests**

All integration tests that construct `FrameData` by hand need the new `beat` field. Add `beat: Default::default()` to every `FrameData { ... }` literal in:

- `tests/spectrum_test.rs` (2 instances)
- `tests/waveform_test.rs` (1 instance)
- `tests/spectrogram_test.rs` (1 instance)

**Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass, no behavior change.

**Step 4: Commit**

```bash
git add src/processing.rs tests/
git commit -m "feat: add BeatData stub to FrameData for future beat detection"
```

---

### Task 2: BrailleCanvas rendering utility

**Files:**
- Create: `src/visualizations/render.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod render;`)
- Create: `tests/render_test.rs`

**Step 1: Write failing tests**

Create `tests/render_test.rs`:

```rust
use terminal_vibes::visualizations::render::BrailleCanvas;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[test]
fn test_braille_canvas_dimensions() {
    let canvas = BrailleCanvas::new(80, 24);
    // 80 cols * 2 = 160 pixel width, 24 rows * 4 = 96 pixel height
    assert_eq!(canvas.pixel_width(), 160);
    assert_eq!(canvas.pixel_height(), 96);
}

#[test]
fn test_braille_canvas_empty_renders_blanks() {
    let canvas = BrailleCanvas::new(4, 2);
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
    // All cells should be braille blank (U+2800) or space
    for y in 0..2 {
        for x in 0..4 {
            let cell = &buf[(x, y)];
            let ch = cell.symbol().chars().next().unwrap();
            assert!(ch == '\u{2800}' || ch == ' ',
                "Expected braille blank at ({x},{y}), got {:?}", ch);
        }
    }
}

#[test]
fn test_braille_canvas_set_pixel_renders_dot() {
    let mut canvas = BrailleCanvas::new(4, 2);
    canvas.set(0, 0); // top-left dot of cell (0,0)
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
    let ch = buf[(0u16, 0u16)].symbol().chars().next().unwrap();
    assert_ne!(ch, '\u{2800}', "Expected a dot, got blank braille");
    assert_ne!(ch, ' ', "Expected a dot, got space");
}

#[test]
fn test_braille_canvas_out_of_bounds_no_panic() {
    let mut canvas = BrailleCanvas::new(4, 2);
    canvas.set(999, 999); // should silently ignore
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
}

#[test]
fn test_braille_canvas_clear() {
    let mut canvas = BrailleCanvas::new(4, 2);
    canvas.set(0, 0);
    canvas.clear();
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
    let ch = buf[(0u16, 0u16)].symbol().chars().next().unwrap();
    assert!(ch == '\u{2800}' || ch == ' ');
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test render_test`
Expected: FAIL — module `render` doesn't exist.

**Step 3: Implement BrailleCanvas**

Add `pub mod render;` to `src/visualizations/mod.rs`.

Create `src/visualizations/render.rs`:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// A 2D pixel canvas that maps to Unicode braille characters (U+2800 block).
/// Each terminal cell is a 2x4 dot matrix, giving 2x horizontal and 4x vertical
/// sub-cell resolution.
pub struct BrailleCanvas {
    cols: u16,
    rows: u16,
    /// Flat pixel buffer: pixel_width * pixel_height bits stored as bytes
    pixels: Vec<bool>,
}

// Braille dot positions within a cell:
//   (0,0) (1,0)     bit 0  bit 3
//   (0,1) (1,1)     bit 1  bit 4
//   (0,2) (1,2)     bit 2  bit 5
//   (0,3) (1,3)     bit 6  bit 7
const BRAILLE_DOT_MAP: [[u8; 4]; 2] = [
    [0, 1, 2, 6], // left column (x%2 == 0)
    [3, 4, 5, 7], // right column (x%2 == 1)
];

impl BrailleCanvas {
    pub fn new(cols: u16, rows: u16) -> Self {
        let pw = cols as usize * 2;
        let ph = rows as usize * 4;
        Self {
            cols,
            rows,
            pixels: vec![false; pw * ph],
        }
    }

    pub fn pixel_width(&self) -> usize {
        self.cols as usize * 2
    }

    pub fn pixel_height(&self) -> usize {
        self.rows as usize * 4
    }

    pub fn set(&mut self, x: usize, y: usize) {
        if x < self.pixel_width() && y < self.pixel_height() {
            self.pixels[y * self.pixel_width() + x] = true;
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(false);
    }

    /// Render the pixel buffer into a ratatui Buffer using braille characters.
    pub fn render(&self, area: &Rect, buf: &mut Buffer, color: Color) {
        let render_cols = self.cols.min(area.width);
        let render_rows = self.rows.min(area.height);

        for cy in 0..render_rows {
            for cx in 0..render_cols {
                let mut code: u8 = 0;
                for dx in 0..2usize {
                    for dy in 0..4usize {
                        let px = cx as usize * 2 + dx;
                        let py = cy as usize * 4 + dy;
                        if px < self.pixel_width()
                            && py < self.pixel_height()
                            && self.pixels[py * self.pixel_width() + px]
                        {
                            code |= 1 << BRAILLE_DOT_MAP[dx][dy];
                        }
                    }
                }
                let ch = char::from_u32(0x2800 + code as u32).unwrap_or(' ');
                buf[(area.x + cx, area.y + cy)].set_char(ch).set_fg(color);
            }
        }
    }
}

// --- Common math helpers ---

/// Linear interpolation between a and b.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Hermite smoothstep (smooth 0→1 curve).
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
```

**Step 4: Run tests**

Run: `cargo test --test render_test`
Expected: All 5 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/render.rs src/visualizations/mod.rs tests/render_test.rs
git commit -m "feat: add BrailleCanvas rendering utility with math helpers"
```

---

### Task 3: HalfBlockCanvas rendering utility

**Files:**
- Modify: `src/visualizations/render.rs` (add HalfBlockCanvas)
- Modify: `tests/render_test.rs` (add HalfBlockCanvas tests)

**Step 1: Write failing tests**

Append to `tests/render_test.rs`:

```rust
use terminal_vibes::visualizations::render::HalfBlockCanvas;

#[test]
fn test_half_block_canvas_dimensions() {
    let canvas = HalfBlockCanvas::new(40, 12);
    assert_eq!(canvas.pixel_width(), 40);
    assert_eq!(canvas.pixel_height(), 24); // 12 rows * 2
}

#[test]
fn test_half_block_canvas_set_and_render() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 0, Color::Red);   // top pixel of cell (0,0)
    canvas.set(0, 1, Color::Blue);  // bottom pixel of cell (0,0)
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    // Cell (0,0) should have fg=Red (upper), bg=Blue (lower), char=▀
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), "▀");
}

#[test]
fn test_half_block_canvas_clear() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 0, Color::Red);
    canvas.clear();
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    // After clear, cell should have default (no color set)
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), " ");
}

#[test]
fn test_half_block_canvas_only_top_pixel() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 0, Color::Green); // only top pixel
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), "▀");
}

#[test]
fn test_half_block_canvas_only_bottom_pixel() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 1, Color::Green); // only bottom pixel
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), "▄");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test render_test`
Expected: FAIL — `HalfBlockCanvas` not found.

**Step 3: Implement HalfBlockCanvas**

Append to `src/visualizations/render.rs`:

```rust
/// A 2D color canvas at 2x vertical resolution using half-block characters.
/// Each terminal cell encodes two vertical "pixels" via fg/bg color.
/// Upper pixel uses foreground color with ▀, lower uses background color.
pub struct HalfBlockCanvas {
    cols: u16,
    rows: u16,
    /// Color per pixel: pixel_width * pixel_height, None = unset
    pixels: Vec<Option<Color>>,
}

impl HalfBlockCanvas {
    pub fn new(cols: u16, rows: u16) -> Self {
        let pw = cols as usize;
        let ph = rows as usize * 2;
        Self {
            cols,
            rows,
            pixels: vec![None; pw * ph],
        }
    }

    pub fn pixel_width(&self) -> usize {
        self.cols as usize
    }

    pub fn pixel_height(&self) -> usize {
        self.rows as usize * 2
    }

    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        if x < self.pixel_width() && y < self.pixel_height() {
            self.pixels[y * self.pixel_width() + x] = Some(color);
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(None);
    }

    /// Render the color buffer into a ratatui Buffer using half-block characters.
    pub fn render(&self, area: &Rect, buf: &mut Buffer) {
        let render_cols = self.cols.min(area.width);
        let render_rows = self.rows.min(area.height);

        for cy in 0..render_rows {
            for cx in 0..render_cols {
                let top_idx = (cy as usize * 2) * self.pixel_width() + cx as usize;
                let bot_idx = (cy as usize * 2 + 1) * self.pixel_width() + cx as usize;
                let top = self.pixels[top_idx];
                let bot = self.pixels[bot_idx];

                let cell = &mut buf[(area.x + cx, area.y + cy)];
                match (top, bot) {
                    (Some(tc), Some(bc)) => {
                        cell.set_char('▀').set_fg(tc).set_bg(bc);
                    }
                    (Some(tc), None) => {
                        cell.set_char('▀').set_fg(tc);
                    }
                    (None, Some(bc)) => {
                        cell.set_char('▄').set_fg(bc);
                    }
                    (None, None) => {
                        cell.set_char(' ');
                    }
                }
            }
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test render_test`
Expected: All 10 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/render.rs tests/render_test.rs
git commit -m "feat: add HalfBlockCanvas rendering utility"
```

---

### Task 4: Lissajous visualization

**Files:**
- Create: `src/visualizations/lissajous.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod lissajous;`)
- Create: `tests/lissajous_test.rs`

**Step 1: Write failing tests**

Create `tests/lissajous_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::lissajous::Lissajous;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_lissajous_name() {
    let viz = Lissajous::new();
    assert_eq!(viz.name(), "lissajous");
}

#[test]
fn test_lissajous_render_empty_no_panic() {
    let viz = Lissajous::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_lissajous_render_zero_area_no_panic() {
    let viz = Lissajous::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_lissajous_update_and_render() {
    let mut viz = Lissajous::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 2048],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Should have drawn some braille characters (not all spaces)
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)].symbol().chars().next().unwrap_or(' ');
            ch != ' ' && ch != '\u{2800}'
        })
    });
    assert!(has_content, "Lissajous should render visible content with non-zero input");
}

#[test]
fn test_lissajous_evolves_over_updates() {
    let mut viz = Lissajous::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 2048],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };

    // Capture render after 1 update
    viz.update(&frame);
    let area = Rect::new(0, 0, 40, 12);
    let mut buf1 = Buffer::empty(area);
    viz.render(area, &mut buf1);

    // Capture render after 60 more updates (simulating 1 second)
    for _ in 0..60 {
        viz.update(&frame);
    }
    let mut buf2 = Buffer::empty(area);
    viz.render(area, &mut buf2);

    // The two renders should differ (pattern evolves over time)
    let differs = (0..12).any(|y| {
        (0..40).any(|x| {
            buf1[(x as u16, y as u16)].symbol() != buf2[(x as u16, y as u16)].symbol()
        })
    });
    assert!(differs, "Lissajous pattern should evolve over time");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test lissajous_test`
Expected: FAIL — module doesn't exist.

**Step 3: Implement Lissajous**

Add `pub mod lissajous;` to `src/visualizations/mod.rs`.

Create `src/visualizations/lissajous.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::render::BrailleCanvas;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::f32::consts::PI;

/// Frequency ratios that produce interesting Lissajous patterns
const RATIOS: [(f32, f32); 8] = [
    (1.0, 2.0),
    (2.0, 3.0),
    (3.0, 4.0),
    (3.0, 2.0),
    (4.0, 3.0),
    (5.0, 4.0),
    (3.0, 5.0),
    (5.0, 6.0),
];

pub struct Lissajous {
    time: f32,
    ratio_index: usize,
    ratio_drift_timer: f32,
    frozen: bool,
    phase_x: f32,
    phase_y: f32,
    peak: f32,
    rms: f32,
    /// Trail: recent curve snapshots for fading effect
    trail: Vec<Vec<(f32, f32)>>,
    color: Color,
}

impl Lissajous {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            ratio_index: 0,
            ratio_drift_timer: 0.0,
            frozen: false,
            phase_x: 0.0,
            phase_y: 0.0,
            peak: 0.0,
            rms: 0.0,
            trail: Vec::new(),
            color: Color::Rgb(0, 255, 200),
        }
    }

    fn current_ratio(&self) -> (f32, f32) {
        RATIOS[self.ratio_index % RATIOS.len()]
    }
}

impl Visualization for Lissajous {
    fn name(&self) -> &str {
        "lissajous"
    }

    fn update(&mut self, frame: &FrameData) {
        self.peak = frame.peak;
        self.rms = frame.rms;

        // Bass energy modulates phase_x, mid energy modulates phase_y
        let band_count = frame.spectrum.len();
        if band_count > 0 {
            let bass = frame.spectrum[..band_count / 3]
                .iter()
                .sum::<f32>()
                / (band_count / 3) as f32;
            let mid = frame.spectrum[band_count / 3..2 * band_count / 3]
                .iter()
                .sum::<f32>()
                / (band_count / 3) as f32;
            self.phase_x += bass * 0.1;
            self.phase_y += mid * 0.1;
        }

        // Drift to next ratio every ~4 seconds (240 frames at 60Hz)
        if !self.frozen {
            self.ratio_drift_timer += 1.0;
            if self.ratio_drift_timer > 240.0 {
                self.ratio_drift_timer = 0.0;
                self.ratio_index = (self.ratio_index + 1) % RATIOS.len();
            }
        }

        // Generate the current curve
        let (a, b) = self.current_ratio();
        let scale = 0.3 + self.peak * 0.6; // 30-90% of canvas
        let num_points = 500;
        let curve: Vec<(f32, f32)> = (0..num_points)
            .map(|i| {
                let t = i as f32 / num_points as f32 * 2.0 * PI;
                let x = (a * t + self.phase_x + self.time).sin() * scale;
                let y = (b * t + self.phase_y).sin() * scale;
                (x, y)
            })
            .collect();

        // Push to trail, keep last N frames
        let max_trail = 5 + (self.rms * 15.0) as usize; // 5-20 frames based on energy
        self.trail.push(curve);
        while self.trail.len() > max_trail {
            self.trail.remove(0);
        }

        self.time += 0.02 + self.rms * 0.03;
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
        let scale = cx.min(cy) * 0.9;

        // Draw trail (all frames)
        for curve in &self.trail {
            for &(x, y) in curve {
                let px = (cx + x * scale) as usize;
                let py = (cy + y * scale) as usize;
                canvas.set(px, py);
            }
        }

        canvas.render(&area, buf, self.color);
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('f') => {
                self.frozen = !self.frozen;
                true
            }
            crossterm::event::KeyCode::Char('r') => {
                self.ratio_index = (self.ratio_index + 1) % RATIOS.len();
                self.ratio_drift_timer = 0.0;
                true
            }
            _ => false,
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test lissajous_test`
Expected: All 5 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/lissajous.rs src/visualizations/mod.rs tests/lissajous_test.rs
git commit -m "feat: add Lissajous visualization plugin"
```

---

### Task 5: Tunnel visualization

**Files:**
- Create: `src/visualizations/tunnel.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod tunnel;`)
- Create: `tests/tunnel_test.rs`

**Step 1: Write failing tests**

Create `tests/tunnel_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::tunnel::Tunnel;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_tunnel_name() {
    let viz = Tunnel::new();
    assert_eq!(viz.name(), "tunnel");
}

#[test]
fn test_tunnel_render_empty_no_panic() {
    let viz = Tunnel::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_tunnel_render_zero_area_no_panic() {
    let viz = Tunnel::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_tunnel_update_and_render() {
    let mut viz = Tunnel::new();
    let frame = FrameData {
        spectrum: vec![0.7; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_tunnel_rings_expand_over_time() {
    let mut viz = Tunnel::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.7,
        rms: 0.5,
        beat: Default::default(),
    };

    // Multiple updates to let rings expand
    for _ in 0..30 {
        viz.update(&frame);
    }
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Should have drawn content
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)].symbol().chars().next().unwrap_or(' ');
            ch != ' '
        })
    });
    assert!(has_content, "Tunnel should render visible rings");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test tunnel_test`
Expected: FAIL — module doesn't exist.

**Step 3: Implement Tunnel**

Add `pub mod tunnel;` to `src/visualizations/mod.rs`.

Create `src/visualizations/tunnel.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::render::HalfBlockCanvas;
use crate::visualizations::spectrum::ColorPalette;
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
        let band_count = frame.spectrum.len();
        if band_count > 0 {
            self.bass = frame.spectrum[..band_count / 4]
                .iter()
                .sum::<f32>()
                / (band_count / 4) as f32;
            self.treble = frame.spectrum[3 * band_count / 4..]
                .iter()
                .sum::<f32>()
                / (band_count / 4) as f32;
        }

        // Expand existing rings
        let speed = 0.01 + self.rms * 0.03;
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

        self.time += 0.016;
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut canvas = HalfBlockCanvas::new(area.width, area.height);
        let pw = canvas.pixel_width() as f32;
        let ph = canvas.pixel_height() as f32;
        let cx = pw / 2.0;
        let cy = ph / 2.0;
        let max_radius = cx.min(cy);

        // Draw rings from farthest to nearest (painter's algorithm)
        for ring in &self.rings {
            let r = ring.radius * max_radius;
            // Brightness fades with distance
            let brightness = (1.0 - ring.radius).clamp(0.0, 1.0);
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
                    canvas.set(px, py, color);
                }
            }
        }

        canvas.render(&area, buf);
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
                let idx = names.iter().position(|n| *n == self.palette.name()).unwrap_or(0);
                let prev_idx = if idx == 0 { names.len() - 1 } else { idx - 1 };
                self.palette = ColorPalette::from_name(names[prev_idx]).unwrap_or(self.palette);
                true
            }
            _ => false,
        }
    }
}
```

Note: `ColorPalette::ALL` is currently a private const. We need to make it `pub` in `src/visualizations/spectrum.rs` — change `const ALL:` to `pub const ALL:`.

**Step 4: Run tests**

Run: `cargo test --test tunnel_test`
Expected: All 5 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/tunnel.rs src/visualizations/mod.rs src/visualizations/spectrum.rs tests/tunnel_test.rs
git commit -m "feat: add Tunnel visualization plugin"
```

---

### Task 6: Radial Spectrum visualization

**Files:**
- Create: `src/visualizations/radial.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod radial;`)
- Create: `tests/radial_test.rs`

**Step 1: Write failing tests**

Create `tests/radial_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::radial::RadialSpectrum;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_radial_name() {
    let viz = RadialSpectrum::new();
    assert_eq!(viz.name(), "radial");
}

#[test]
fn test_radial_render_empty_no_panic() {
    let viz = RadialSpectrum::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_radial_render_zero_area_no_panic() {
    let viz = RadialSpectrum::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_radial_update_and_render() {
    let mut viz = RadialSpectrum::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.7,
        rms: 0.4,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)].symbol().chars().next().unwrap_or(' ');
            ch != ' '
        })
    });
    assert!(has_content, "RadialSpectrum should render visible content");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test radial_test`
Expected: FAIL — module doesn't exist.

**Step 3: Implement RadialSpectrum**

Add `pub mod radial;` to `src/visualizations/mod.rs`.

Create `src/visualizations/radial.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::render::HalfBlockCanvas;
use crate::visualizations::spectrum::ColorPalette;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::f32::consts::PI;

pub struct RadialSpectrum {
    spectrum: Vec<f32>,
    rotation: f32,
    rms: f32,
    mirror: bool,
    palette: ColorPalette,
}

impl RadialSpectrum {
    pub fn new() -> Self {
        Self {
            spectrum: Vec::new(),
            rotation: 0.0,
            rms: 0.0,
            mirror: false,
            palette: ColorPalette::Neon,
        }
    }
}

impl Visualization for RadialSpectrum {
    fn name(&self) -> &str {
        "radial"
    }

    fn update(&mut self, frame: &FrameData) {
        self.spectrum = frame.spectrum.clone();
        self.rms = frame.rms;
        self.rotation += 0.005 + self.rms * 0.02;
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.spectrum.is_empty() {
            return;
        }

        let mut canvas = HalfBlockCanvas::new(area.width, area.height);
        let pw = canvas.pixel_width() as f32;
        let ph = canvas.pixel_height() as f32;
        let cx = pw / 2.0;
        let cy = ph / 2.0;
        let max_len = cx.min(cy) * 0.85;

        let num_rays = self.spectrum.len();

        for (i, &magnitude) in self.spectrum.iter().enumerate() {
            let t = i as f32 / num_rays as f32;
            let angle = self.rotation + t * 2.0 * PI;
            let color = self.palette.color(t);

            let ray_len = magnitude * max_len;
            let steps = ray_len as usize;

            for s in 0..=steps {
                let r = s as f32;
                let px = (cx + angle.cos() * r) as usize;
                let py = (cy + angle.sin() * r) as usize;
                canvas.set(px, py, color);

                if self.mirror {
                    // Draw inward mirror
                    let px_in = (cx - angle.cos() * r) as usize;
                    let py_in = (cy - angle.sin() * r) as usize;
                    canvas.set(px_in, py_in, color);
                }
            }
        }

        canvas.render(&area, buf);
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Char('m') => {
                self.mirror = !self.mirror;
                true
            }
            crossterm::event::KeyCode::Char('p') => {
                let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
                let idx = names.iter().position(|n| *n == self.palette.name()).unwrap_or(0);
                self.palette = ColorPalette::from_name(names[(idx + 1) % names.len()])
                    .unwrap_or(self.palette);
                true
            }
            crossterm::event::KeyCode::Char('P') => {
                let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
                let idx = names.iter().position(|n| *n == self.palette.name()).unwrap_or(0);
                let prev = if idx == 0 { names.len() - 1 } else { idx - 1 };
                self.palette = ColorPalette::from_name(names[prev]).unwrap_or(self.palette);
                true
            }
            _ => false,
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test radial_test`
Expected: All 4 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/radial.rs src/visualizations/mod.rs tests/radial_test.rs
git commit -m "feat: add RadialSpectrum visualization plugin"
```

---

### Task 7: Plasma visualization

**Files:**
- Create: `src/visualizations/plasma.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod plasma;`)
- Create: `tests/plasma_test.rs`

**Step 1: Write failing tests**

Create `tests/plasma_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::plasma::Plasma;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_plasma_name() {
    let viz = Plasma::new();
    assert_eq!(viz.name(), "plasma");
}

#[test]
fn test_plasma_render_empty_no_panic() {
    let viz = Plasma::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_plasma_render_zero_area_no_panic() {
    let viz = Plasma::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_plasma_fills_area_with_color() {
    let mut viz = Plasma::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.7,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 20, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Plasma should fill every cell (no empty spaces)
    let empty_count = (0..10u16).flat_map(|y| {
        (0..20u16).filter(move |&x| buf[(x, y)].symbol() == " ")
    }).count();
    assert_eq!(empty_count, 0, "Plasma should fill every cell");
}

#[test]
fn test_plasma_evolves_over_time() {
    let mut viz = Plasma::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.5,
        rms: 0.3,
        beat: Default::default(),
    };

    viz.update(&frame);
    let area = Rect::new(0, 0, 20, 10);
    let mut buf1 = Buffer::empty(area);
    viz.render(area, &mut buf1);

    for _ in 0..30 {
        viz.update(&frame);
    }
    let mut buf2 = Buffer::empty(area);
    viz.render(area, &mut buf2);

    // Colors should differ between frames
    let differs = (0..10u16).any(|y| {
        (0..20u16).any(|x| buf1[(x, y)].fg != buf2[(x, y)].fg)
    });
    assert!(differs, "Plasma should evolve over time");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test plasma_test`
Expected: FAIL — module doesn't exist.

**Step 3: Implement Plasma**

Add `pub mod plasma;` to `src/visualizations/mod.rs`.

Create `src/visualizations/plasma.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::render::HalfBlockCanvas;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::f32::consts::PI;

pub struct Plasma {
    time: f32,
    /// Scaling factors modulated by spectrum bands
    k1: f32,
    k2: f32,
    k3: f32,
    k4: f32,
    rms: f32,
    hue_offset: f32,
}

impl Plasma {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            k1: 8.0,
            k2: 12.0,
            k3: 10.0,
            k4: 14.0,
            rms: 0.0,
            hue_offset: 0.0,
        }
    }
}

impl Visualization for Plasma {
    fn name(&self) -> &str {
        "plasma"
    }

    fn update(&mut self, frame: &FrameData) {
        self.rms = frame.rms;

        let band_count = frame.spectrum.len();
        if band_count >= 4 {
            let quarter = band_count / 4;
            let bass = frame.spectrum[..quarter].iter().sum::<f32>() / quarter as f32;
            let low_mid = frame.spectrum[quarter..quarter * 2].iter().sum::<f32>() / quarter as f32;
            let high_mid = frame.spectrum[quarter * 2..quarter * 3].iter().sum::<f32>() / quarter as f32;
            let treble = frame.spectrum[quarter * 3..].iter().sum::<f32>() / quarter as f32;

            // Bass stretches, treble tightens
            self.k1 = 6.0 + bass * 12.0;
            self.k2 = 8.0 + low_mid * 10.0;
            self.k3 = 7.0 + high_mid * 14.0;
            self.k4 = 10.0 + treble * 8.0;
        }

        // Peak boosts color saturation via hue rotation speed
        self.hue_offset += 0.02 + frame.peak * 0.05;
        self.time += 0.03 + self.rms * 0.05;
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut canvas = HalfBlockCanvas::new(area.width, area.height);
        let pw = canvas.pixel_width();
        let ph = canvas.pixel_height();

        for py in 0..ph {
            let y = py as f32 / ph as f32;
            for px in 0..pw {
                let x = px as f32 / pw as f32;

                // Classic plasma: sum of sine functions
                let v1 = (x * self.k1 + self.time).sin();
                let v2 = (y * self.k2 + self.time * 1.3).sin();
                let v3 = ((x + y) * self.k3 + self.time * 0.7).sin();
                let dist = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
                let v4 = (dist * self.k4 + self.time * 1.1).sin();

                let v = (v1 + v2 + v3 + v4) / 4.0; // -1.0..1.0
                let t = (v + 1.0) / 2.0; // normalize to 0.0..1.0

                let color = plasma_color(t, self.hue_offset);
                canvas.set(px, py, color);
            }
        }

        canvas.render(&area, buf);
    }
}

/// Map a value (0..1) and hue offset to an RGB color via HSV-like rotation.
fn plasma_color(t: f32, hue_offset: f32) -> Color {
    let hue = (t + hue_offset) % 1.0;
    let r = ((hue * 2.0 * PI).sin() * 0.5 + 0.5) * 255.0;
    let g = ((hue * 2.0 * PI + 2.094).sin() * 0.5 + 0.5) * 255.0; // +120°
    let b = ((hue * 2.0 * PI + 4.189).sin() * 0.5 + 0.5) * 255.0; // +240°
    Color::Rgb(r as u8, g as u8, b as u8)
}
```

**Step 4: Run tests**

Run: `cargo test --test plasma_test`
Expected: All 5 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/plasma.rs src/visualizations/mod.rs tests/plasma_test.rs
git commit -m "feat: add Plasma visualization plugin"
```

---

### Task 8: Aurora visualization

**Files:**
- Create: `src/visualizations/aurora.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod aurora;`)
- Create: `tests/aurora_test.rs`

**Step 1: Write failing tests**

Create `tests/aurora_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::aurora::Aurora;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_aurora_name() {
    let viz = Aurora::new();
    assert_eq!(viz.name(), "aurora");
}

#[test]
fn test_aurora_render_empty_no_panic() {
    let viz = Aurora::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_aurora_render_zero_area_no_panic() {
    let viz = Aurora::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_aurora_update_and_render() {
    let mut viz = Aurora::new();
    let frame = FrameData {
        spectrum: vec![0.6; 128],
        waveform: vec![],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 60, 20);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Should have some colored content
    let has_content = (0..20u16).any(|y| {
        (0..60u16).any(|x| {
            buf[(x, y)].symbol() != " "
        })
    });
    assert!(has_content, "Aurora should render visible curtains");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test aurora_test`
Expected: FAIL — module doesn't exist.

**Step 3: Implement Aurora**

Add `pub mod aurora;` to `src/visualizations/mod.rs`.

Create `src/visualizations/aurora.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::render::HalfBlockCanvas;
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
        }
    }

    fn make_curtains(n: usize) -> Vec<Curtain> {
        let configs: Vec<(f32, f32, f32, f32, (u8, u8, u8))> = vec![
            (0.7, 2.0, 0.3, 0.4, (0, 200, 100)),   // green, low
            (0.5, 3.0, 0.5, 0.3, (0, 150, 255)),    // blue, mid
            (0.3, 4.5, 0.7, 0.2, (180, 0, 255)),    // purple, high
            (0.4, 3.5, 0.4, 0.25, (0, 255, 200)),   // cyan, mid-high
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

        // Split spectrum into per-curtain band energies
        let n = self.curtains.len();
        let band_count = frame.spectrum.len();
        self.band_energies.clear();
        if band_count > 0 && n > 0 {
            let chunk = band_count / n;
            for i in 0..n {
                let start = i * chunk;
                let end = if i == n - 1 { band_count } else { (i + 1) * chunk };
                let energy = frame.spectrum[start..end].iter().sum::<f32>()
                    / (end - start) as f32;
                self.band_energies.push(energy);
            }
        }

        self.time += 0.02 + self.rms * 0.04;
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut canvas = HalfBlockCanvas::new(area.width, area.height);
        let pw = canvas.pixel_width();
        let ph = canvas.pixel_height();

        // For each column, compute each curtain's contribution
        for px in 0..pw {
            let x = px as f32 / pw as f32;

            // Accumulate color per pixel row (additive blending)
            let mut column_colors: Vec<(f32, f32, f32)> = vec![(0.0, 0.0, 0.0); ph];

            for (i, curtain) in self.curtains.iter().enumerate() {
                let energy = self.band_energies.get(i).copied().unwrap_or(0.3);

                // Curtain center oscillates with sine wave
                let wave = (x * curtain.freq * PI + self.time * curtain.speed).sin();
                let center = curtain.base_y + wave * 0.1;
                let height = curtain.height * (0.5 + energy);

                // Brightness flash on peak
                let brightness = 0.6 + energy * 0.3 + self.peak * 0.1;

                for py in 0..ph {
                    let y = py as f32 / ph as f32;
                    let dist = (y - center).abs();
                    if dist < height {
                        // Smooth falloff from center
                        let falloff = 1.0 - (dist / height);
                        let falloff = falloff * falloff; // quadratic
                        let intensity = falloff * brightness;

                        column_colors[py].0 += curtain.color.0 as f32 * intensity;
                        column_colors[py].1 += curtain.color.1 as f32 * intensity;
                        column_colors[py].2 += curtain.color.2 as f32 * intensity;
                    }
                }
            }

            // Write blended colors to canvas
            for (py, &(r, g, b)) in column_colors.iter().enumerate() {
                if r > 1.0 || g > 1.0 || b > 1.0 {
                    let color = Color::Rgb(
                        (r as u8).min(255),
                        (g as u8).min(255),
                        (b as u8).min(255),
                    );
                    canvas.set(px, py, color);
                }
            }
        }

        canvas.render(&area, buf);
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
```

**Step 4: Run tests**

Run: `cargo test --test aurora_test`
Expected: All 4 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/aurora.rs src/visualizations/mod.rs tests/aurora_test.rs
git commit -m "feat: add Aurora visualization plugin"
```

---

### Task 9: Starfield visualization

**Files:**
- Create: `src/visualizations/starfield.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod starfield;`)
- Create: `tests/starfield_test.rs`

**Step 1: Write failing tests**

Create `tests/starfield_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::starfield::Starfield;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_starfield_name() {
    let viz = Starfield::new();
    assert_eq!(viz.name(), "starfield");
}

#[test]
fn test_starfield_render_empty_no_panic() {
    let viz = Starfield::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_starfield_render_zero_area_no_panic() {
    let viz = Starfield::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_starfield_particles_appear_after_updates() {
    let mut viz = Starfield::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };
    // Run several updates so particles have time to spread
    for _ in 0..60 {
        viz.update(&frame);
    }
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)].symbol().chars().next().unwrap_or(' ');
            ch != ' ' && ch != '\u{2800}'
        })
    });
    assert!(has_content, "Starfield should have visible particles after updates");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test starfield_test`
Expected: FAIL — module doesn't exist.

**Step 3: Implement Starfield**

Add `pub mod starfield;` to `src/visualizations/mod.rs`.

Create `src/visualizations/starfield.rs`:

```rust
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
```

**Step 4: Run tests**

Run: `cargo test --test starfield_test`
Expected: All 4 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/starfield.rs src/visualizations/mod.rs tests/starfield_test.rs
git commit -m "feat: add Starfield visualization plugin"
```

---

### Task 10: Rain visualization

**Files:**
- Create: `src/visualizations/rain.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod rain;`)
- Create: `tests/rain_test.rs`

**Step 1: Write failing tests**

Create `tests/rain_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::rain::Rain;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_rain_name() {
    let viz = Rain::new();
    assert_eq!(viz.name(), "rain");
}

#[test]
fn test_rain_render_empty_no_panic() {
    let viz = Rain::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_rain_render_zero_area_no_panic() {
    let viz = Rain::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_rain_streams_appear_after_updates() {
    let mut viz = Rain::new();
    let frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        beat: Default::default(),
    };
    // Run enough updates for drops to appear and fall
    for _ in 0..30 {
        viz.update(&frame);
    }
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    let has_content = (0..24u16).any(|y| {
        (0..80u16).any(|x| {
            buf[(x, y)].symbol() != " "
        })
    });
    assert!(has_content, "Rain should have visible streams after updates");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test rain_test`
Expected: FAIL — module doesn't exist.

**Step 3: Implement Rain**

Add `pub mod rain;` to `src/visualizations/mod.rs`.

Create `src/visualizations/rain.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use crate::visualizations::spectrum::ColorPalette;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

struct Drop {
    y: f32,        // current row position
    speed: f32,    // rows per update
    length: u16,   // tail length
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
        self.spectrum = frame.spectrum.clone();
        self.frame_counter = self.frame_counter.wrapping_add(1);

        let global_speed = 0.3 + self.rms * 0.7;
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

    fn render(&self, area: Rect, buf: &mut Buffer) {
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

                    let color = if let Color::Rgb(r, g, b) = base_color {
                        Color::Rgb(
                            (r as f32 * fade) as u8,
                            (g as f32 * fade) as u8,
                            (b as f32 * fade) as u8,
                        )
                    } else {
                        base_color
                    };

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
                let idx = names.iter().position(|n| *n == self.palette.name()).unwrap_or(0);
                self.palette = ColorPalette::from_name(names[(idx + 1) % names.len()])
                    .unwrap_or(self.palette);
                true
            }
            crossterm::event::KeyCode::Char('P') => {
                let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
                let idx = names.iter().position(|n| *n == self.palette.name()).unwrap_or(0);
                let prev = if idx == 0 { names.len() - 1 } else { idx - 1 };
                self.palette = ColorPalette::from_name(names[prev]).unwrap_or(self.palette);
                true
            }
            _ => false,
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test rain_test`
Expected: All 4 tests pass.

**Step 5: Commit**

```bash
git add src/visualizations/rain.rs src/visualizations/mod.rs tests/rain_test.rs
git commit -m "feat: add Rain visualization plugin"
```

---

### Task 11: Register all new visualizations and update CLI

**Files:**
- Modify: `src/main.rs:43-48` (update `--list-modes` output)
- Modify: `src/main.rs:70-73` (register new plugins)

**Step 1: Update --list-modes output**

In `src/main.rs`, replace the `list_modes` block:

```rust
    if cli.list_modes {
        println!("Available visualization modes:");
        println!("  spectrum    - Frequency spectrum bars");
        println!("  waveform    - Oscilloscope waveform");
        println!("  spectrogram - Scrolling frequency heatmap");
        println!("  lissajous   - Parametric curve spirograph");
        println!("  tunnel      - Concentric rings rushing forward");
        println!("  radial      - Circular spectrum starburst");
        println!("  plasma      - Sine interference color field");
        println!("  aurora      - Northern lights curtains");
        println!("  starfield   - 3D particle warp drive");
        println!("  rain        - Music-reactive falling streams");
        return Ok(());
    }
```

**Step 2: Add imports and register new visualizations**

Add to imports in `src/main.rs`:

```rust
use visualizations::lissajous::Lissajous;
use visualizations::tunnel::Tunnel;
use visualizations::radial::RadialSpectrum;
use visualizations::plasma::Plasma;
use visualizations::aurora::Aurora;
use visualizations::starfield::Starfield;
use visualizations::rain::Rain;
```

Register after existing plugins:

```rust
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(SpectrumBars::new()));
    registry.register(Box::new(Waveform::new()));
    registry.register(Box::new(Spectrogram::new(200)));
    registry.register(Box::new(Lissajous::new()));
    registry.register(Box::new(Tunnel::new()));
    registry.register(Box::new(RadialSpectrum::new()));
    registry.register(Box::new(Plasma::new()));
    registry.register(Box::new(Aurora::new()));
    registry.register(Box::new(Starfield::new()));
    registry.register(Box::new(Rain::new()));
```

**Step 3: Ensure Rain initializes columns on first render**

The `Rain` visualization needs to call `ensure_columns(width)` at the start of `render()` and `update()`. Update `rain.rs` — in `update()`, if columns are empty and spectrum is available, don't process column-level logic (it will lazy-init on first render). In `render()`, add at the top after early returns:

```rust
    // Lazy-init: rain needs to know terminal width for column setup
    // We do a mutable borrow workaround by checking if we need init
```

Actually, since `render` takes `&self`, the column initialization should happen in `update()`. Change the `update` method to call `self.ensure_columns(80)` with a reasonable default, then re-init in `render` via interior mutability — OR simply make `render` adapt to whatever columns exist. The simplest approach: call `ensure_columns` in `update` using the current column count or a reasonable max (200). The actual rendering adapts to `area.width`.

Revised approach — in `update()`, add at the start:

```rust
    // Ensure at least some columns exist (render will clip to area width)
    if self.columns.is_empty() {
        self.ensure_columns(200);
    }
```

**Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass (unit + all integration tests).

**Step 5: Commit**

```bash
git add src/main.rs src/visualizations/rain.rs
git commit -m "feat: register all 7 new visualizations and update CLI listing"
```

---

### Task 12: Final integration test — all visualizers through registry

**Files:**
- Modify: `tests/visualizations_test.rs` (add comprehensive test)

**Step 1: Write integration test**

Append to `tests/visualizations_test.rs`:

```rust
use terminal_vibes::visualizations::lissajous::Lissajous;
use terminal_vibes::visualizations::tunnel::Tunnel;
use terminal_vibes::visualizations::radial::RadialSpectrum;
use terminal_vibes::visualizations::plasma::Plasma;
use terminal_vibes::visualizations::aurora::Aurora;
use terminal_vibes::visualizations::starfield::Starfield;
use terminal_vibes::visualizations::rain::Rain;

#[test]
fn test_registry_with_all_visualizations() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("spectrum")));
    registry.register(Box::new(MockViz::new("waveform")));
    registry.register(Box::new(MockViz::new("spectrogram")));
    registry.register(Box::new(Lissajous::new()));
    registry.register(Box::new(Tunnel::new()));
    registry.register(Box::new(RadialSpectrum::new()));
    registry.register(Box::new(Plasma::new()));
    registry.register(Box::new(Aurora::new()));
    registry.register(Box::new(Starfield::new()));
    registry.register(Box::new(Rain::new()));

    assert_eq!(registry.len(), 10);

    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 2048],
        peak: 0.7,
        rms: 0.4,
        beat: Default::default(),
    };

    // Cycle through all and verify update+render doesn't panic
    let area = Rect::new(0, 0, 80, 24);
    for _ in 0..10 {
        registry.update_current(&frame);
        let mut buf = Buffer::empty(area);
        registry.render_current(area, &mut buf);
        registry.next();
    }
}
```

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add tests/visualizations_test.rs
git commit -m "test: add integration test for all 10 visualizations through registry"
```
