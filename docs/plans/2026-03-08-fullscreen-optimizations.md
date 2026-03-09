# Fullscreen Rendering Optimizations Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve visual smoothness and CPU efficiency when running fullscreen by reducing per-pixel computation cost, smarter color quantization, higher FPS outside tmux, and automatic performance adaptation.

**Architecture:** Five independent optimizations layered on the existing rendering pipeline. SinLut replaces expensive trig in plasma. Color quantization moves earlier in the pipeline (set-time vs render-time) with an adaptive step that scales with terminal size. A user-controlled `color_detail` parameter lets users trade fidelity for performance. Frame time monitoring auto-adjusts when performance degrades.

**Tech Stack:** Rust, ratatui, std::sync::LazyLock

---

### Task 1: Sin Lookup Table

**Files:**
- Modify: `src/visualizations/render.rs` (add SinLut after the `smoothstep` function, ~line 127)
- Modify: `src/visualizations/plasma.rs:100-104,121-123` (swap `.sin()` calls)

**Step 1: Write failing tests for SinLut**

Add to `src/visualizations/render.rs` at the bottom, inside a `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn sin_lut_accuracy() {
        let lut = SinLut::new();
        // Test at known values
        assert!((lut.get(0.0) - 0.0_f32.sin()).abs() < 0.002);
        assert!((lut.get(PI / 2.0) - 1.0).abs() < 0.002);
        assert!((lut.get(PI) - 0.0).abs() < 0.002);
        assert!((lut.get(3.0 * PI / 2.0) - (-1.0)).abs() < 0.002);
    }

    #[test]
    fn sin_lut_wraps_negative() {
        let lut = SinLut::new();
        assert!((lut.get(-PI / 2.0) - (-1.0)).abs() < 0.002);
    }

    #[test]
    fn sin_lut_wraps_large() {
        let lut = SinLut::new();
        let val = 100.0 * PI + PI / 2.0;
        assert!((lut.get(val) - val.sin()).abs() < 0.002);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib render::tests -v`
Expected: FAIL — `SinLut` not defined.

**Step 3: Implement SinLut**

Add to `src/visualizations/render.rs` after the `smoothstep` function (~line 127):

```rust
use std::f32::consts::TAU;
use std::sync::LazyLock;

const SIN_LUT_SIZE: usize = 4096;

pub struct SinLut {
    table: [f32; SIN_LUT_SIZE],
}

impl SinLut {
    fn new() -> Self {
        let mut table = [0.0; SIN_LUT_SIZE];
        for i in 0..SIN_LUT_SIZE {
            table[i] = (TAU * i as f32 / SIN_LUT_SIZE as f32).sin();
        }
        Self { table }
    }

    #[inline]
    pub fn get(&self, radians: f32) -> f32 {
        let normalized = radians.rem_euclid(TAU) / TAU;
        let index = (normalized * SIN_LUT_SIZE as f32) as usize % SIN_LUT_SIZE;
        self.table[index]
    }
}

pub static SIN_LUT: LazyLock<SinLut> = LazyLock::new(SinLut::new);
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib render::tests -v`
Expected: PASS — all 3 tests green.

**Step 5: Swap plasma to use SIN_LUT**

In `src/visualizations/plasma.rs`:

Add import at top:
```rust
use crate::visualizations::render::SIN_LUT;
```

Replace lines 100-104 (the 4 sin calls in `render()`):
```rust
let v1 = SIN_LUT.get(x * self.k1 + self.time);
let v2 = SIN_LUT.get(y * self.k2 + self.time * 1.3);
let v3 = SIN_LUT.get((x + y) * self.k3 + self.time * 0.7);
let dist = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
let v4 = SIN_LUT.get(dist * self.k4 + self.time * 1.1);
```

Replace lines 121-123 in `plasma_color()`:
```rust
let r = (SIN_LUT.get(hue * 2.0 * PI) * 0.5 + 0.5) * 255.0;
let g = (SIN_LUT.get(hue * 2.0 * PI + 2.094) * 0.5 + 0.5) * 255.0;
let b = (SIN_LUT.get(hue * 2.0 * PI + 4.189) * 0.5 + 0.5) * 255.0;
```

**Step 6: Run full test suite and clippy**

Run: `cargo test --lib && cargo clippy`
Expected: All tests pass, no warnings.

**Step 7: Commit**

```bash
git add src/visualizations/render.rs src/visualizations/plasma.rs
git commit -m "perf: add SinLut and swap plasma trig to table lookups"
```

---

### Task 2: Parameterized quantize_color

**Files:**
- Modify: `src/visualizations/render.rs:11-18` (add step parameter to `quantize_color`)
- Modify: `src/visualizations/render.rs:85,189-190` (canvas callers)
- Modify: `src/visualizations/spectrum.rs:212`
- Modify: `src/visualizations/waveform.rs:59`
- Modify: `src/visualizations/rain.rs:181`
- Modify: `src/visualizations/spectrogram.rs:88`

**Step 1: Write failing test for parameterized quantize_color**

Add to the existing `#[cfg(test)] mod tests` in `src/visualizations/render.rs`:

```rust
#[test]
fn quantize_color_step_16() {
    assert_eq!(
        quantize_color(Color::Rgb(17, 33, 255), 16),
        Color::Rgb(16, 32, 240)
    );
}

#[test]
fn quantize_color_step_32() {
    assert_eq!(
        quantize_color(Color::Rgb(33, 50, 255), 32),
        Color::Rgb(32, 32, 224)
    );
}

#[test]
fn quantize_color_passthrough_non_rgb() {
    assert_eq!(quantize_color(Color::White, 16), Color::White);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib render::tests -v`
Expected: FAIL — `quantize_color` signature mismatch.

**Step 3: Update quantize_color signature**

In `src/visualizations/render.rs`, change `quantize_color`:

```rust
#[inline]
pub fn quantize_color(color: Color, step: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            Color::Rgb((r / step) * step, (g / step) * step, (b / step) * step)
        }
        other => other,
    }
}
```

**Step 4: Fix all callers to pass step=16 (preserve existing behavior)**

- `src/visualizations/render.rs:85` (BrailleCanvas): `quantize_color(color, 16)`
- `src/visualizations/render.rs:189-190` (HalfBlockCanvas): `.map(|c| quantize_color(c, 16))`
- `src/visualizations/spectrum.rs:212`: `quantize_color(if let ... , 16)`
- `src/visualizations/waveform.rs:59`: `quantize_color(if let ... , 16)`
- `src/visualizations/rain.rs:181`: `quantize_color(if let ... , 16)`
- `src/visualizations/spectrogram.rs:88`: `quantize_color(magma_colormap(...), 16)`

**Step 5: Run full test suite and clippy**

Run: `cargo test --lib && cargo clippy`
Expected: All tests pass, no new warnings.

**Step 6: Commit**

```bash
git add src/visualizations/render.rs src/visualizations/spectrum.rs src/visualizations/waveform.rs src/visualizations/rain.rs src/visualizations/spectrogram.rs
git commit -m "refactor: parameterize quantize_color step size"
```

---

### Task 3: Quantize at Set Time + Canvas Step Field

**Files:**
- Modify: `src/visualizations/render.rs` (add `step` field to both canvases, quantize in `set()`, remove from `render()`)

**Step 1: Write failing tests**

Add to `#[cfg(test)] mod tests` in `src/visualizations/render.rs`:

```rust
#[test]
fn halfblock_canvas_quantizes_at_set_time() {
    let mut canvas = HalfBlockCanvas::with_step(2, 2, 16);
    canvas.set(0, 0, Color::Rgb(17, 33, 255));
    // Access internals: the stored pixel should already be quantized
    assert_eq!(canvas.pixels[0], Some(Color::Rgb(16, 32, 240)));
}

#[test]
fn halfblock_canvas_custom_step() {
    let mut canvas = HalfBlockCanvas::with_step(2, 2, 32);
    canvas.set(0, 0, Color::Rgb(33, 50, 255));
    assert_eq!(canvas.pixels[0], Some(Color::Rgb(32, 32, 224)));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib render::tests -v`
Expected: FAIL — `with_step` not defined, `pixels` not public.

**Step 3: Implement canvas step field**

In `src/visualizations/render.rs`, modify `HalfBlockCanvas`:

Add `step: u8` field to the struct. Make `pixels` `pub(crate)` for testing.

```rust
pub struct HalfBlockCanvas {
    cols: u16,
    rows: u16,
    step: u8,
    pub(crate) pixels: Vec<Option<Color>>,
}
```

Add `with_step` constructor and update `new` to delegate:

```rust
pub fn new(cols: u16, rows: u16) -> Self {
    Self::with_step(cols, rows, 16)
}

pub fn with_step(cols: u16, rows: u16, step: u8) -> Self {
    let pw = cols as usize;
    let ph = rows as usize * 2;
    Self {
        cols,
        rows,
        step,
        pixels: vec![None; pw * ph],
    }
}
```

Update `resize_or_clear` to preserve step:

```rust
pub fn resize_or_clear(&mut self, cols: u16, rows: u16) {
    if self.cols != cols || self.rows != rows {
        *self = Self::with_step(cols, rows, self.step);
    } else {
        self.clear();
    }
}
```

Add `set_step` method:

```rust
pub fn set_step(&mut self, step: u8) {
    self.step = step;
}
```

Quantize in `set()`:

```rust
pub fn set(&mut self, x: usize, y: usize, color: Color) {
    let pw = self.pixel_width();
    let ph = self.pixel_height();
    if x < pw && y < ph {
        self.pixels[y * pw + x] = Some(quantize_color(color, self.step));
    }
}
```

Remove quantization from `render()` — change lines 189-190 from:
```rust
let top = self.pixels[top_idx].map(quantize_color);
let bot = self.pixels[bot_idx].map(quantize_color);
```
to:
```rust
let top = self.pixels[top_idx];
let bot = self.pixels[bot_idx];
```

**Step 4: Apply same pattern to BrailleCanvas**

BrailleCanvas takes color as a render() parameter, not per-pixel. Quantize at the render() call site instead — this is already correct since the caller passes the color. Just update the render signature to accept step:

Actually, BrailleCanvas already quantizes in `render()` line 85. Keep this as-is but pass the step through. Add `step` field to BrailleCanvas following the same pattern (field, `with_step`, `set_step`, `resize_or_clear` preserves step). Update `render()` to use `self.step`:

```rust
pub fn render(&self, area: &Rect, buf: &mut Buffer, color: Color) {
    let color = quantize_color(color, self.step);
    // ... rest unchanged
}
```

**Step 5: Run full test suite and clippy**

Run: `cargo test --lib && cargo clippy`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/visualizations/render.rs
git commit -m "perf: quantize colors at canvas set-time for better ratatui diffing"
```

---

### Task 4: Adaptive Quantization Step Function

**Files:**
- Modify: `src/visualizations/render.rs` (add `adaptive_quantization_step` function)

**Step 1: Write failing tests**

Add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn adaptive_step_small_terminal() {
    // 80x24 = 1920 cells
    assert_eq!(adaptive_quantization_step(1920, 1.0), 16);
}

#[test]
fn adaptive_step_medium_terminal() {
    // 150x50 = 7500 cells
    assert_eq!(adaptive_quantization_step(7500, 1.0), 24);
}

#[test]
fn adaptive_step_large_terminal() {
    // 200x60 = 12000 cells
    assert_eq!(adaptive_quantization_step(12000, 1.0), 32);
}

#[test]
fn adaptive_step_detail_doubles_precision() {
    // Large terminal but detail=2.0 → step halved
    assert_eq!(adaptive_quantization_step(12000, 2.0), 16);
}

#[test]
fn adaptive_step_detail_halves_precision() {
    // Small terminal but detail=0.5 → step doubled
    assert_eq!(adaptive_quantization_step(1920, 0.5), 32);
}

#[test]
fn adaptive_step_clamps_minimum() {
    // Never go below 4 (still 64 values per channel)
    assert_eq!(adaptive_quantization_step(100, 2.0), 8);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib render::tests -v`
Expected: FAIL — function not defined.

**Step 3: Implement adaptive_quantization_step**

Add to `src/visualizations/render.rs`:

```rust
/// Compute quantization step based on terminal cell count and user color detail preference.
/// Higher cell count → coarser step (more perf). Higher detail → finer step (more fidelity).
pub fn adaptive_quantization_step(cell_count: u32, color_detail: f32) -> u8 {
    let base = if cell_count < 4000 {
        16u8
    } else if cell_count < 10000 {
        24u8
    } else {
        32u8
    };
    let adjusted = (base as f32 / color_detail).round() as u8;
    adjusted.clamp(4, 64)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib render::tests -v`
Expected: All pass.

**Step 5: Commit**

```bash
git add src/visualizations/render.rs
git commit -m "feat: add adaptive quantization step based on terminal size and color detail"
```

---

### Task 5: color_detail User Control

**Files:**
- Modify: `src/ui.rs` (add `color_detail` field, keybindings, state persistence, status bar, wire adaptive step to canvas)

**Step 1: Add color_detail field to App**

In `src/ui.rs`, add to the `App` struct (after `beat_intensity`):

```rust
color_detail: f32,
```

In `App::new()`, initialize it to `1.0` and load from state (follow `beat_intensity` pattern):

```rust
let mut color_detail = 1.0_f32;
```

And in the state loading block:
```rust
if let Some(cd) = state.get("color_detail").and_then(|v| v.as_float()) {
    color_detail = cd as f32;
}
```

Add to the struct construction:
```rust
color_detail,
```

**Step 2: Add d/D keybindings**

In `handle_key()`, add after the `'B'` match arm:

```rust
KeyCode::Char('d') => {
    self.color_detail = (self.color_detail + 0.1).min(2.0);
    true
}
KeyCode::Char('D') => {
    self.color_detail = (self.color_detail - 0.1).max(0.5);
    true
}
```

**Step 3: Wire adaptive step to canvas rendering**

In the `run()` method, before `self.registry.update_current(&display_frame)` (~line 130), compute and set the step:

```rust
use crate::visualizations::render::adaptive_quantization_step;

let cell_count = (terminal.size()?.width as u32) * (terminal.size()?.height as u32);
let quant_step = adaptive_quantization_step(cell_count, self.color_detail);
self.registry.set_quantization_step(quant_step);
```

This requires adding `set_quantization_step` to `VisualizationRegistry` and the `Visualization` trait. Add to the trait in `src/visualizations/mod.rs`:

```rust
fn set_quantization_step(&mut self, _step: u8) {}
```

Add to `VisualizationRegistry` in `src/visualizations/registry.rs`:

```rust
pub fn set_quantization_step(&mut self, step: u8) {
    if let Some(viz) = self.current_mut() {
        viz.set_quantization_step(step);
    }
}
```

Then implement it on visualizations that use canvases. For each viz with a `HalfBlockCanvas` or `BrailleCanvas` (plasma, aurora, tunnel, lissajous, radial):

```rust
fn set_quantization_step(&mut self, step: u8) {
    self.canvas.set_step(step);
}
```

For vizs that call `quantize_color` directly (spectrum, waveform, rain, spectrogram), store the step as a field and use it:

```rust
fn set_quantization_step(&mut self, step: u8) {
    self.quant_step = step;
}
```

Then replace their hardcoded `16` with `self.quant_step`. Initialize `quant_step: u8` to `16` in each struct's `new()`.

**Step 4: Add to status bar**

In the status bar format string (~line 198), add `detail: {:.1}x` using `self.color_detail`.

**Step 5: Add to state persistence**

In `save_state()`, add after beat_intensity:
```rust
root.insert(
    "color_detail".to_string(),
    toml::Value::Float(self.color_detail as f64),
);
```

**Step 6: Run full test suite and clippy**

Run: `cargo test --lib && cargo clippy`
Expected: All tests pass, no warnings.

**Step 7: Commit**

```bash
git add src/ui.rs src/visualizations/mod.rs src/visualizations/registry.rs src/visualizations/plasma.rs src/visualizations/aurora.rs src/visualizations/tunnel.rs src/visualizations/lissajous.rs src/visualizations/radial.rs src/visualizations/spectrum.rs src/visualizations/waveform.rs src/visualizations/rain.rs src/visualizations/spectrogram.rs
git commit -m "feat: add color_detail control with adaptive quantization"
```

---

### Task 6: Higher FPS Outside tmux

**Files:**
- Modify: `src/ui.rs:76-80`

**Step 1: Update effective_fps logic**

Change lines 76-80 in `src/ui.rs` from:

```rust
let effective_fps = if in_tmux {
    self.config.display.fps.min(30)
} else {
    self.config.display.fps
};
```

To:

```rust
let effective_fps = if in_tmux {
    self.config.display.fps.min(30)
} else {
    self.config.display.fps.max(60)
};
```

**Step 2: Run tests and clippy**

Run: `cargo test --lib && cargo clippy`
Expected: All pass.

**Step 3: Commit**

```bash
git add src/ui.rs
git commit -m "perf: default to 60fps when not running in tmux"
```

---

### Task 7: Frame Time Budget Monitoring

**Files:**
- Modify: `src/ui.rs` (add frame time EMA tracking and auto-adjustment logic)

**Step 1: Add frame budget state to App**

Add fields to `App` struct:

```rust
/// EMA of frame render time in seconds
render_time_ema: f32,
/// Auto-adjusted color detail (may be lower than user's setting)
effective_color_detail: f32,
```

Initialize both in `App::new()`:
```rust
render_time_ema: 0.0,
effective_color_detail: color_detail,
```

**Step 2: Wrap terminal.draw() with timing**

In the `run()` loop, wrap the `terminal.draw()` call:

```rust
let draw_start = Instant::now();
terminal.draw(|f| {
    // ... existing draw code ...
})?;
let draw_elapsed = draw_start.elapsed().as_secs_f32();

// EMA with alpha ~1/30 (smooths over ~30 frames)
self.render_time_ema = self.render_time_ema * 0.97 + draw_elapsed * 0.03;
```

**Step 3: Add auto-adjustment after drawing**

After the EMA update:

```rust
let frame_budget = frame_duration.as_secs_f32();
if self.render_time_ema > frame_budget * 0.8 {
    // Struggling — reduce effective detail
    self.effective_color_detail = (self.effective_color_detail - 0.1).max(0.5);
} else if self.render_time_ema < frame_budget * 0.5 {
    // Headroom — recover toward user's chosen detail
    self.effective_color_detail =
        (self.effective_color_detail + 0.1).min(self.color_detail);
}
```

**Step 4: Use effective_color_detail for quantization**

Change the `adaptive_quantization_step` call from Task 5 to use `self.effective_color_detail` instead of `self.color_detail`:

```rust
let quant_step = adaptive_quantization_step(cell_count, self.effective_color_detail);
```

Also update `effective_color_detail` when user changes `color_detail` via `d`/`D` keys — in those key handlers, also reset:

```rust
self.effective_color_detail = self.color_detail;
```

**Step 5: Run full test suite and clippy**

Run: `cargo test --lib && cargo clippy`
Expected: All pass.

**Step 6: Commit**

```bash
git add src/ui.rs
git commit -m "perf: add frame time budget monitoring with auto color detail adjustment"
```
