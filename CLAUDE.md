# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Terminal-based music visualizer for macOS. Captures system audio via Core Audio's `AudioProcessTap` API (requires macOS 15+) and renders real-time visualizations using ratatui in the terminal. 10 visualization modes, beat detection, and full TOML configuration.

## Build & Development Commands

```bash
cargo build                  # Build debug
cargo build --release        # Build release
cargo run                    # Run the visualizer
cargo run -- --config path   # Run with custom config
cargo run -- --list-modes    # List available visualization modes

cargo test                   # Run all tests (unit + integration)
cargo test --lib             # Unit tests only
cargo test <test_name>       # Run a single test by name
cargo test --test spectrum_test  # Run a specific integration test file

cargo clippy                 # Lint
cargo fmt                    # Format
```

**Note:** `tests/processing_test.rs` has a pre-existing compile error (`sample_rate` field removed from `ProcessorConfig`). Use `cargo test --lib` or target specific test files to avoid it.

## Architecture

### Three-Thread Pipeline

```
Audio Thread          Processing Thread       Main Thread (UI)
┌──────────┐         ┌──────────────┐        ┌──────────────────┐
│ CoreAudio │──ring──▶│ FFT Pipeline │──mpsc─▶│  Ratatui Loop    │
│ Callback  │ buffer  │   @60 Hz     │channel │  @60 FPS (raw)   │
└──────────┘         └──────────────┘        │  @30 FPS (tmux)  │
                                              └──────────────────┘
```

- **Audio Thread**: Core Audio callback writes f32 PCM samples into a lock-free SPSC ring buffer. Must never block or allocate.
- **Processing Thread**: Polls ring buffer at ~60Hz, runs FFT pipeline (Hann window → FFT → magnitude → log band binning → dB → normalize → EMA smoothing), sends `FrameData` via bounded `mpsc::sync_channel(2)`. Drops frames if channel full.
- **Main Thread**: Ratatui event loop, drains latest `FrameData`, delegates to active visualization plugin for update+render.

### Key Data Types

`FrameData` (in `processing.rs`) flows from processor to UI:
- `spectrum: Vec<f32>` — 128 logarithmic frequency bands, normalized 0..1
- `waveform: Vec<f32>` — raw PCM samples for oscilloscope
- `peak: f32` / `rms: f32` — amplitude metrics
- `beat: BeatData` — per-band (bass/mid/treble) beat detection with envelopes

`BeatData` (in `beat.rs`):
- Per-band energy, beat flags, and envelopes (fast attack, exponential decay)
- Overall envelope = max of all band envelopes
- Configurable sensitivity, cooldown, and history window

### Visualization Plugin System

All visualizations implement the `Visualization` trait (`visualizations/mod.rs`):
- `name(&self) -> &str` — unique identifier
- `update(&mut self, frame: &FrameData)` — process new audio data
- `render(&mut self, area: Rect, buf: &mut Buffer)` — draw to terminal buffer
- `on_key(&mut self, key: KeyEvent) -> bool` — handle mode-specific input
- `set_quantization_step(&mut self, step: u8)` — receive adaptive color quantization step from UI
- `heavy_rendering(&self) -> bool` — flag for escape-sequence-heavy modes (pauses rendering when unfocused in tmux)
- `default_config` / `apply_config` / `save_config` — TOML persistence

Plugins are registered in `VisualizationRegistry` and switched via Tab/Shift+Tab.

**Built-in visualizations** (10 modes):
spectrum, waveform, spectrogram, lissajous, tunnel, radial, plasma, aurora, starfield, rain

### Rendering Canvases

Two canvas types in `visualizations/render.rs`:
- **HalfBlockCanvas** — 2x vertical resolution using `▀`/`▄` with fg+bg color (used by plasma, aurora, tunnel)
- **BrailleCanvas** — 2x4 sub-cell resolution using braille characters (used by lissajous, radial)

Both canvases have a `step: u8` field controlling color quantization granularity. Colors are quantized at **set-time** (not render-time) via `quantize_color(color, step)` so that ratatui's buffer diff sees more unchanged cells between frames, reducing escape sequence volume.

The `SinLut` static in `render.rs` provides a 4096-entry pre-computed sine lookup table (`SIN_LUT.get(radians)`) for O(1) trig — used by plasma to replace ~96K `sin()` calls per frame at fullscreen.

`adaptive_quantization_step(cell_count, color_detail)` computes the quantization step based on terminal size (3 tiers: 16/24/32) scaled by the user's `color_detail` preference (0.5–2.0).

### Module Map

- `main.rs` — CLI parsing (clap), thread orchestration, shutdown coordination, viz registration
- `config.rs` — TOML config with XDG paths (`~/.config/terminal-vibes/config.toml`)
- `processing.rs` — FFT pipeline and `FrameData` production
- `beat.rs` — Per-band beat detection, envelope tracking, energy analysis
- `audio/tap.rs` — Core Audio FFI, `AudioTap` lifecycle (all unsafe code lives here)
- `audio/mod.rs` — `AudioConfig`, `AudioRingBuffer` types
- `ui.rs` — Ratatui app shell, input handling, status bar, label fade, state persistence, frame budget monitoring
- `visualizations/mod.rs` — `Visualization` trait definition
- `visualizations/registry.rs` — Plugin management, state save/load
- `visualizations/render.rs` — HalfBlockCanvas, BrailleCanvas, SinLut, quantize_color, adaptive_quantization_step, math helpers
- `visualizations/*.rs` — Individual visualization implementations

### Synchronization

- Audio→Processor: lock-free SPSC ring buffer (`ringbuf::HeapRb<f32>`)
- Processor→UI: bounded `mpsc::sync_channel(2)` with graceful frame drops
- Shutdown: `Arc<AtomicBool>` shared across all threads

### State Persistence

App state saved to `~/.config/terminal-vibes/state.toml`:
- Current visualization, sensitivity, beat intensity, color detail
- Per-visualization config (palette, toggles, etc.)

## Platform Constraints

- **macOS only** — Core Audio FFI bindings in `audio/tap.rs`
- **macOS 15+** — requires `AudioProcessTap` API
- macOS-specific deps are gated with `[target.'cfg(target_os = "macos")'.dependencies]`

## Adding a New Visualization

1. Create `src/visualizations/your_viz.rs` implementing the `Visualization` trait
2. Add `pub mod your_viz;` to `src/visualizations/mod.rs`
3. Register in `main.rs` where the other plugins are registered
4. Implement `set_quantization_step` — canvas-based vizs delegate to `self.canvas.set_step(step)`, direct-buffer vizs store `quant_step: u8` field and pass to `quantize_color`
5. If the viz fills every cell with unique colors, return `true` from `heavy_rendering()`
6. Add integration test in `tests/your_viz_test.rs`
7. Per-plugin config lives under `[visualizations.<name>]` in TOML

## Release Process

Fully automated via `.github/workflows/release.yml` using [release-plz](https://release-plz.ieni.dev/).

**To release**: Push to `trunk` with conventional commit messages. CI handles everything else.

### What CI Does

1. Runs `release-plz update` — analyzes commits, bumps `Cargo.toml` version, updates `CHANGELOG.md`
2. Commits as `chore(release): bump to vX.Y.Z and update changelog`
3. Runs `release-plz release` — creates a GitHub Release with generated notes

### Versioning

Based on conventional commit prefixes:
- `fix:` / `perf:` → patch bump
- `feat:` → minor bump
- `BREAKING CHANGE` → major bump

### Local/Remote Version Drift

After CI runs, `origin/trunk` will have a `chore(release)` commit that bumps `Cargo.toml` and changelog. Always `git fetch` and rebase before pushing new work to avoid conflicts.

### Notes

- Runner is `macos-latest` (needed for platform-specific dependencies)
- Loop prevention: CI skips commits starting with `chore(release):`
- `Cargo.toml` version may appear stale locally — the CI commit updates it on the remote

## Performance Considerations

- Audio callback must never block or allocate — ring buffer is lock-free
- Visualizations should reuse buffers (canvas `resize_or_clear`, pre-allocated Vecs)
- Use `SIN_LUT.get()` instead of `.sin()` for per-pixel trig in hot render loops
- Color quantization at canvas set-time reduces ratatui diff volume (fewer escape sequences)
- Adaptive quantization step scales with terminal size — coarser at fullscreen for better perf
- Frame budget monitoring auto-adjusts `effective_color_detail` every ~30 frames to prevent dropped frames
- FPS defaults to 60 outside tmux, capped at 30 inside tmux
- HalfBlockCanvas generates 2 escape sequences per cell (fg+bg) — keep color diversity bounded
- Frame dropping is by design — processor drops if UI can't keep up

### Adding Performance-Sensitive Visualizations

When implementing a new visualization that does per-pixel work:
- Use `SIN_LUT.get()` from `render.rs` instead of `f32::sin()` for trig
- Implement `set_quantization_step` to receive the adaptive step (canvas-based: `self.canvas.set_step(step)`, direct-buffer: store as `self.quant_step` and pass to `quantize_color`)
- Set `heavy_rendering() -> true` if the viz fills every cell with unique colors (enables auto-pause when unfocused in tmux)

### User Controls

| Key | Parameter | Range | Effect |
|-----|-----------|-------|--------|
| `+`/`-` | sensitivity | 0.1–5.0 | Scales spectrum amplitude |
| `b`/`B` | beat_intensity | 0.0–3.0 | Scales beat envelope |
| `]`/`[` | color_detail | 0.5–2.0 | Finer/coarser color quantization |
| `s` | show_status_bar | toggle | Show/hide status bar |
| Tab/Shift+Tab | visualization | cycle | Switch visualization mode |
