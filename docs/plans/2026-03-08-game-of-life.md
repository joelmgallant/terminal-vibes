# Game of Life Visualization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a "life" visualization that fuses Conway's Game of Life with audio reactivity — audio seeds cells and warps rules, with three switchable render modes (character, halfblock, braille).

**Architecture:** Double-buffered flat `Vec<Cell>` grid with toroidal wrapping. Audio drives both cell spawning (spatial frequency mapping) and rule mutation (bass/treble shift birth/survival thresholds). Three render modes share the simulation logic; toggled with `m` key. Age-based coloring through swappable `ColorPalette`.

**Tech Stack:** Rust, ratatui, crossterm, toml (config persistence). Uses existing `HalfBlockCanvas`, `BrailleCanvas`, `quantize_color`, and `ColorPalette` from the codebase.

---

### Task 1: Create life.rs with struct, constructor, and Visualization trait skeleton

**Files:**
- Create: `src/visualizations/life.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod life;`)

**Step 1: Create the life module with core types and skeleton**

Create `src/visualizations/life.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::render::{quantize_color, BrailleCanvas, HalfBlockCanvas};
use crate::visualizations::spectrum::ColorPalette;
use crate::visualizations::Visualization;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq)]
enum RenderMode {
    Character,
    HalfBlock,
    Braille,
}

impl RenderMode {
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

#[derive(Clone, Copy)]
struct Cell {
    alive: bool,
    age: u16,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            alive: false,
            age: 0,
        }
    }
}

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
```

**Step 2: Add module declaration**

In `src/visualizations/mod.rs`, add `pub mod life;` in alphabetical order (after `pub mod lissajous;`).

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles with warnings about unused fields/imports (that's fine — they'll be used in later tasks).

**Step 4: Commit**

```bash
git add src/visualizations/life.rs src/visualizations/mod.rs
git commit -m "feat(life): add Game of Life visualization skeleton with types and config"
```

---

### Task 2: Register in main.rs, add integration test skeleton

**Files:**
- Modify: `src/main.rs:34,64,94` (add import + list-modes entry + register call)
- Create: `tests/life_test.rs`

**Step 1: Write the integration test**

Create `tests/life_test.rs`:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::life::Life;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_life_name() {
    let viz = Life::new();
    assert_eq!(viz.name(), "life");
}

#[test]
fn test_life_render_zero_area_no_panic() {
    let mut viz = Life::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_life_render_empty_no_panic() {
    let mut viz = Life::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_life_update_stores_audio_state() {
    let mut viz = Life::new();
    let frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        ..Default::default()
    };
    viz.update(&frame);
    // Should not panic, audio state stored internally
}

#[test]
fn test_life_cells_appear_after_updates() {
    let mut viz = Life::new();
    let frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        ..Default::default()
    };
    // Run enough updates for cells to spawn and be visible
    let area = Rect::new(0, 0, 80, 24);
    for _ in 0..30 {
        viz.update(&frame);
        let mut buf = Buffer::empty(area);
        viz.render(area, &mut buf);
    }
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    let has_content = (0..24u16).any(|y| (0..80u16).any(|x| buf[(x, y)].symbol() != " "));
    assert!(has_content, "Life should have visible cells after updates with audio");
}

#[test]
fn test_life_heavy_rendering_depends_on_mode() {
    let mut viz = Life::new();
    // Default is character mode — not heavy
    assert!(!viz.heavy_rendering());

    // Simulate switching to halfblock via on_key
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE);
    viz.on_key(key);
    assert!(viz.heavy_rendering());

    // Switch again to braille — not heavy
    viz.on_key(key);
    assert!(!viz.heavy_rendering());
}
```

**Step 2: Run tests to see them fail**

Run: `cargo test --test life_test 2>&1 | tail -20`
Expected: Most tests pass (skeleton exists), `test_life_cells_appear_after_updates` fails (no cells rendered yet), `test_life_heavy_rendering_depends_on_mode` fails (on_key not implemented yet).

**Step 3: Register in main.rs**

Add import near line 34:
```rust
use visualizations::life::Life;
```

Add to list-modes output near line 63:
```rust
println!("  life        - Audio-reactive Game of Life");
```

Add registration near line 94:
```rust
registry.register(Box::new(Life::new()));
```

**Step 4: Verify compilation**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles successfully.

**Step 5: Commit**

```bash
git add src/main.rs tests/life_test.rs
git commit -m "feat(life): register visualization and add integration tests"
```

---

### Task 3: Implement simulation logic (update method)

**Files:**
- Modify: `src/visualizations/life.rs`

**Step 1: Add grid helper methods**

Add these methods to the `impl Life` block (after `new()`):

```rust
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
                    Cell { alive: true, age: cell.age.saturating_add(1).min(1000) }
                } else {
                    Cell::default()
                }
            } else if neighbors >= birth_min && neighbors <= birth_max {
                Cell { alive: true, age: 0 }
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
        let hash = self.frame_counter.wrapping_mul(2654435761).wrapping_add(x as u32);
        let rand_val = (hash >> 16) as f32 / 65536.0;

        if rand_val < spawn_chance && y_start < y_end {
            let y = y_start + (hash as usize % (y_end - y_start));
            let idx = y * w + x;
            if !self.current[idx].alive {
                self.current[idx] = Cell { alive: true, age: 0 };
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
        self.current[idx] = Cell { alive: true, age: 0 };
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
```

**Step 2: Implement the update method**

Replace the `update` stub in the `Visualization` impl:

```rust
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
```

**Step 3: Run tests**

Run: `cargo test --test life_test 2>&1 | tail -20`
Expected: `test_life_update_stores_audio_state` passes. `test_life_cells_appear_after_updates` may still fail (render not done yet).

**Step 4: Commit**

```bash
git add src/visualizations/life.rs
git commit -m "feat(life): implement simulation logic with audio seeding and rule warping"
```

---

### Task 4: Implement all three render methods

**Files:**
- Modify: `src/visualizations/life.rs`

**Step 1: Add color helper**

Add to the `impl Life` block:

```rust
/// Get color for a cell based on its age and the active palette.
fn cell_color(&self, age: u16) -> Color {
    let t = (age as f32 / 200.0).min(1.0);
    let base = self.palette.color(t);
    let brightness = 0.3 + self.beat_envelope * 0.7;
    match base {
        Color::Rgb(r, g, b) => quantize_color(
            Color::Rgb(
                (r as f32 * brightness) as u8,
                (g as f32 * brightness) as u8,
                (b as f32 * brightness) as u8,
            ),
            self.quant_step,
        ),
        other => other,
    }
}
```

**Step 2: Implement the render method**

Replace the `render` stub in the `Visualization` impl:

```rust
fn render(&mut self, area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let (gw, gh) = self.grid_dims_for_area(area);
    self.ensure_grid(gw, gh);

    match self.render_mode {
        RenderMode::Character => self.render_character(area, buf),
        RenderMode::HalfBlock => self.render_halfblock(area, buf),
        RenderMode::Braille => self.render_braille(area, buf),
    }
}
```

**Step 3: Add per-mode render methods**

Add to `impl Life`:

```rust
fn render_character(&self, area: Rect, buf: &mut Buffer) {
    let w = self.grid_width;
    let h = self.grid_height;
    for y in 0..h.min(area.height as usize) {
        for x in 0..w.min(area.width as usize) {
            let cell = self.current[y * w + x];
            if cell.alive {
                let ch = match cell.age {
                    0..=5 => '█',
                    6..=20 => '▓',
                    21..=60 => '▒',
                    _ => '░',
                };
                let color = self.cell_color(cell.age);
                buf[(area.x + x as u16, area.y + y as u16)]
                    .set_char(ch)
                    .set_fg(color);
            }
        }
    }
}

fn render_halfblock(&mut self, area: Rect, buf: &mut Buffer) {
    self.halfblock_canvas.resize_or_clear(area.width, area.height);
    let w = self.grid_width;
    let h = self.grid_height;
    let pw = self.halfblock_canvas.pixel_width();
    let ph = self.halfblock_canvas.pixel_height();
    for y in 0..h.min(ph) {
        for x in 0..w.min(pw) {
            let cell = self.current[y * w + x];
            if cell.alive {
                let color = self.cell_color(cell.age);
                self.halfblock_canvas.set(x, y, color);
            }
        }
    }
    self.halfblock_canvas.render(&area, buf);
}

fn render_braille(&mut self, area: Rect, buf: &mut Buffer) {
    self.braille_canvas.resize_or_clear(area.width, area.height);
    let w = self.grid_width;
    let h = self.grid_height;
    let pw = self.braille_canvas.pixel_width();
    let ph = self.braille_canvas.pixel_height();
    // Use the color of the most common age — simplified to palette midpoint + beat
    let color = self.cell_color(50);
    for y in 0..h.min(ph) {
        for x in 0..w.min(pw) {
            let cell = self.current[y * w + x];
            if cell.alive {
                self.braille_canvas.set(x, y);
            }
        }
    }
    self.braille_canvas.render(&area, buf, color);
}
```

**Step 4: Run tests**

Run: `cargo test --test life_test 2>&1 | tail -20`
Expected: `test_life_cells_appear_after_updates` now passes.

**Step 5: Commit**

```bash
git add src/visualizations/life.rs
git commit -m "feat(life): implement character, halfblock, and braille render modes"
```

---

### Task 5: Implement key bindings (m, p/P, r, c)

**Files:**
- Modify: `src/visualizations/life.rs`

**Step 1: Implement on_key**

Replace the `on_key` stub in the `Visualization` impl:

```rust
fn on_key(&mut self, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('m') => {
            self.render_mode = self.render_mode.next();
            // Grid reinitializes on next render (dimensions change)
            self.grid_width = 0;
            self.grid_height = 0;
            true
        }
        KeyCode::Char('r') => {
            // Randomize: ~25% of cells alive
            for cell in &mut self.current {
                let hash = self.frame_counter.wrapping_mul(2654435761);
                self.frame_counter = self.frame_counter.wrapping_add(1);
                cell.alive = (hash % 4) == 0;
                cell.age = 0;
            }
            true
        }
        KeyCode::Char('c') => {
            for cell in &mut self.current {
                cell.alive = false;
                cell.age = 0;
            }
            true
        }
        KeyCode::Char('p') => {
            self.palette = self.palette.next();
            true
        }
        KeyCode::Char('P') => {
            self.palette = self.palette.prev();
            true
        }
        _ => false,
    }
}
```

Note: `ColorPalette::next()` and `prev()` are private methods. We need to use the same pattern as Rain (cycling via `from_name` and `ALL`). Replace the `p`/`P` handlers:

```rust
KeyCode::Char('p') => {
    let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
    let idx = names.iter().position(|n| *n == self.palette.name()).unwrap_or(0);
    self.palette = ColorPalette::from_name(names[(idx + 1) % names.len()])
        .unwrap_or(self.palette);
    true
}
KeyCode::Char('P') => {
    let names: Vec<&str> = ColorPalette::ALL.iter().map(|p| p.name()).collect();
    let idx = names.iter().position(|n| *n == self.palette.name()).unwrap_or(0);
    let prev = if idx == 0 { names.len() - 1 } else { idx - 1 };
    self.palette = ColorPalette::from_name(names[prev]).unwrap_or(self.palette);
    true
}
```

**Step 2: Run tests**

Run: `cargo test --test life_test 2>&1 | tail -20`
Expected: All tests pass, including `test_life_heavy_rendering_depends_on_mode`.

**Step 3: Run clippy**

Run: `cargo clippy 2>&1 | tail -20`
Expected: No errors. Fix any warnings.

**Step 4: Run formatter**

Run: `cargo fmt`

**Step 5: Commit**

```bash
git add src/visualizations/life.rs
git commit -m "feat(life): add key bindings for render mode, palette, randomize, and clear"
```

---

### Task 6: Add unit tests for simulation logic

**Files:**
- Modify: `src/visualizations/life.rs` (add `#[cfg(test)]` module at bottom)

**Step 1: Add unit tests**

Add at the end of `src/visualizations/life.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_life_with_grid(width: usize, height: usize) -> Life {
        let mut life = Life::new();
        life.ensure_grid(width, height);
        life
    }

    #[test]
    fn test_count_neighbors_empty_grid() {
        let life = make_life_with_grid(5, 5);
        assert_eq!(life.count_neighbors(2, 2), 0);
    }

    #[test]
    fn test_count_neighbors_surrounded() {
        let mut life = make_life_with_grid(5, 5);
        // Set all 8 neighbors of (2,2) alive
        for dy in [1, 0, 4] { // 4 wraps to -1 in mod 5
            for dx in [1, 0, 4] {
                if dx == 0 && dy == 0 { continue; }
                let x = (2 + dx) % 5;
                let y = (2 + dy) % 5;
                life.current[y * 5 + x].alive = true;
            }
        }
        assert_eq!(life.count_neighbors(2, 2), 8);
    }

    #[test]
    fn test_toroidal_wrap() {
        let mut life = make_life_with_grid(5, 5);
        // Place cell at (4, 4) — neighbor of (0, 0) via wrapping
        life.current[4 * 5 + 4].alive = true;
        assert_eq!(life.count_neighbors(0, 0), 1);
    }

    #[test]
    fn test_blinker_oscillates() {
        let mut life = make_life_with_grid(5, 5);
        // Horizontal blinker at row 2
        life.current[2 * 5 + 1].alive = true;
        life.current[2 * 5 + 2].alive = true;
        life.current[2 * 5 + 3].alive = true;

        // Zero audio envelopes so rules stay classic B3/S23
        life.bass_envelope = 0.0;
        life.treble_envelope = 0.0;

        life.tick();

        // Should become vertical blinker at col 2
        assert!(!life.current[2 * 5 + 1].alive);
        assert!(life.current[1 * 5 + 2].alive);
        assert!(life.current[2 * 5 + 2].alive);
        assert!(life.current[3 * 5 + 2].alive);
        assert!(!life.current[2 * 5 + 3].alive);
    }

    #[test]
    fn test_block_is_stable() {
        let mut life = make_life_with_grid(6, 6);
        // 2x2 block at (2,2)
        life.current[2 * 6 + 2].alive = true;
        life.current[2 * 6 + 3].alive = true;
        life.current[3 * 6 + 2].alive = true;
        life.current[3 * 6 + 3].alive = true;

        life.bass_envelope = 0.0;
        life.treble_envelope = 0.0;

        life.tick();

        assert!(life.current[2 * 6 + 2].alive);
        assert!(life.current[2 * 6 + 3].alive);
        assert!(life.current[3 * 6 + 2].alive);
        assert!(life.current[3 * 6 + 3].alive);
    }

    #[test]
    fn test_age_increments() {
        let mut life = make_life_with_grid(6, 6);
        // Block (stable pattern) — ages should increment each tick
        life.current[2 * 6 + 2] = Cell { alive: true, age: 0 };
        life.current[2 * 6 + 3] = Cell { alive: true, age: 0 };
        life.current[3 * 6 + 2] = Cell { alive: true, age: 0 };
        life.current[3 * 6 + 3] = Cell { alive: true, age: 0 };

        life.bass_envelope = 0.0;
        life.treble_envelope = 0.0;

        life.tick();
        assert_eq!(life.current[2 * 6 + 2].age, 1);

        life.tick();
        assert_eq!(life.current[2 * 6 + 2].age, 2);
    }

    #[test]
    fn test_bass_envelope_enables_birth_on_two() {
        let mut life = make_life_with_grid(5, 5);
        // Two neighbors of (2,2) — normally not enough to birth
        life.current[1 * 5 + 2].alive = true;
        life.current[3 * 5 + 2].alive = true;

        life.bass_envelope = 0.0;
        life.treble_envelope = 0.0;
        life.tick();
        assert!(!life.current[2 * 5 + 2].alive, "Should not birth with 2 neighbors normally");

        // Reset
        life.current[2 * 5 + 2] = Cell::default();
        life.current[1 * 5 + 2] = Cell { alive: true, age: 0 };
        life.current[3 * 5 + 2] = Cell { alive: true, age: 0 };

        // High bass envelope enables birth on 2
        life.bass_envelope = 0.8;
        life.treble_envelope = 0.0;
        life.tick();
        assert!(life.current[2 * 5 + 2].alive, "Should birth with 2 neighbors when bass is high");
    }

    #[test]
    fn test_render_mode_cycling() {
        let life = Life::new();
        assert_eq!(RenderMode::Character.next(), RenderMode::HalfBlock);
        assert_eq!(RenderMode::HalfBlock.next(), RenderMode::Braille);
        assert_eq!(RenderMode::Braille.next(), RenderMode::Character);
    }

    #[test]
    fn test_grid_dims_for_area() {
        let mut life = Life::new();
        let area = Rect::new(0, 0, 80, 24);

        life.render_mode = RenderMode::Character;
        assert_eq!(life.grid_dims_for_area(area), (80, 24));

        life.render_mode = RenderMode::HalfBlock;
        assert_eq!(life.grid_dims_for_area(area), (80, 48));

        life.render_mode = RenderMode::Braille;
        assert_eq!(life.grid_dims_for_area(area), (160, 96));
    }
}
```

**Step 2: Run unit tests**

Run: `cargo test --lib life 2>&1`
Expected: All unit tests pass.

**Step 3: Run all life tests**

Run: `cargo test life 2>&1`
Expected: All unit + integration tests pass.

**Step 4: Commit**

```bash
git add src/visualizations/life.rs
git commit -m "test(life): add unit tests for simulation, wrapping, rule warping, and rendering"
```

---

### Task 7: Final verification and cleanup

**Files:**
- All files touched in previous tasks

**Step 1: Run full test suite**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: All tests pass.

**Step 2: Run integration tests**

Run: `cargo test --test life_test 2>&1`
Expected: All tests pass.

**Step 3: Run clippy**

Run: `cargo clippy 2>&1 | tail -20`
Expected: No warnings or errors.

**Step 4: Run formatter**

Run: `cargo fmt --check 2>&1`
Expected: No formatting issues.

**Step 5: Verify build**

Run: `cargo build 2>&1 | tail -5`
Expected: Compiles cleanly.

**Step 6: Commit any remaining fixes**

If clippy/fmt changes were needed:
```bash
git add -u
git commit -m "chore(life): fix clippy warnings and formatting"
```
