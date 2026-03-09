# Fullscreen Rendering Optimizations

## Goal

Improve both visual smoothness (higher FPS) and CPU efficiency (lower thermal load) when running fullscreen, particularly for heavy visualizations like plasma and aurora.

## Problem

Heavy visualizations (plasma, aurora, tunnel) perform expensive per-pixel computation every frame. At fullscreen terminal sizes (~200x60 = 24K+ half-block pixels), this means:

- Plasma: ~96K `sin()` calls per frame
- Aurora: ~72K distance calculations per frame (3 curtains)
- Color quantization happens at render time, not set time, so ratatui's buffer diff misses opportunities to skip unchanged cells
- FPS capped at 30 even outside tmux where there's no escape sequence bottleneck
- No adaptive quality scaling — same work whether terminal is 80x24 or 240x70

## Design

### 1. Sin Lookup Table

Add a `SinLut` struct in `render.rs`:

- 4096-entry pre-computed table covering 0..2pi
- Single `fn get(radians: f32) -> f32` method, wraps input to table index
- Stored as `LazyLock<SinLut>` static — computed once, zero per-frame cost
- Plasma swaps `(expr).sin()` calls to `SIN_LUT.get(expr)`
- Max error ~0.0008 — imperceptible in a terminal visualizer
- Roughly halves plasma render time since trig is the dominant per-pixel cost

### 2. Quantize Colors at Set Time

Move `quantize_color()` from `HalfBlockCanvas::render()` into `HalfBlockCanvas::set()`:

- Colors get bucketed to the quantization step immediately when written to the canvas
- `render()` drops its `quantize_color()` calls (already done at set time)
- Same change for `BrailleCanvas::render()` — quantize the color param upfront
- Net effect: ratatui's buffer diff sees more truly-unchanged cells between frames, emitting fewer terminal escape sequences
- Also simplifies the render() hot loop (one fewer function call per cell)

### 3. Adaptive Quantization with User Control

Base quantization step auto-scales with terminal size:

- Small (< 4000 cells): STEP=16 (current quality)
- Medium (4000-10000 cells): STEP=24
- Large (> 10000 cells): STEP=32

User-controlled `color_detail` parameter:

- Range: 0.5 to 2.0, default 1.0
- Effective step = `base_step / color_detail`
- Keybindings: `d` to increase detail (+0.1), `D` to decrease detail (-0.1)
- Persisted in `state.toml` alongside sensitivity and beat_intensity
- Displayed in the status bar

### 4. Higher FPS Outside tmux

- When NOT in tmux, default to 60fps (currently defaults to 30)
- In tmux, behavior unchanged (capped at 30)
- User's explicit `display.fps` config still overrides everything
- Implementation: `if in_tmux { config.fps.min(30) } else { config.fps.max(60) }`

### 5. Frame Time Budget Monitoring

Automatic performance cruise control:

- Measure wall clock time around `terminal.draw()` each frame
- Track rolling EMA over ~30 frames (single f32, no allocations)
- If avg render time > 80% of frame budget: nudge effective `color_detail` down by 0.1 (min 0.5)
- If avg render time < 50% of frame budget: nudge back toward user's chosen `color_detail` by 0.1
- User's `d`/`D` setting is the target — auto-adapt never exceeds it, only temporarily reduces
- No UI indicator — silent adaptation

## Files Affected

- `src/visualizations/render.rs` — SinLut, quantize_color changes, quantization step on canvas structs
- `src/visualizations/plasma.rs` — swap sin() to SIN_LUT.get()
- `src/ui.rs` — color_detail param, d/D keybindings, frame budget monitoring, higher FPS logic, status bar update
- `src/config.rs` — no changes needed (fps config already exists)

## Non-Goals

- No new dependencies (no rayon for parallelism)
- No spatial subsampling or resolution reduction
- No changes to the processing thread or audio pipeline
