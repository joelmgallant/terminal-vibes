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
┌──────────┐         ┌──────────────┐        ┌──────────────┐
│ CoreAudio │──ring──▶│ FFT Pipeline │──mpsc─▶│ Ratatui Loop │
│ Callback  │ buffer  │   @60 Hz     │channel │   @30 FPS    │
└──────────┘         └──────────────┘        └──────────────┘
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
- `default_config` / `apply_config` / `save_config` — TOML persistence

Plugins are registered in `VisualizationRegistry` and switched via Tab/Shift+Tab.

**Built-in visualizations** (10 modes):
spectrum, waveform, spectrogram, lissajous, tunnel, radial, plasma, aurora, starfield, rain

### Rendering Canvases

Two canvas types in `visualizations/render.rs`:
- **HalfBlockCanvas** — 2x vertical resolution using `▀`/`▄` with fg+bg color (used by plasma, aurora, spectrogram, tunnel)
- **BrailleCanvas** — 2x4 sub-cell resolution using braille characters (used by lissajous, radial)

Both include `quantize_color()` to reduce unique escape sequences for terminal multiplexer performance.

### Module Map

- `main.rs` — CLI parsing (clap), thread orchestration, shutdown coordination, viz registration
- `config.rs` — TOML config with XDG paths (`~/.config/terminal-vibes/config.toml`)
- `processing.rs` — FFT pipeline and `FrameData` production
- `beat.rs` — Per-band beat detection, envelope tracking, energy analysis
- `audio/tap.rs` — Core Audio FFI, `AudioTap` lifecycle (all unsafe code lives here)
- `audio/mod.rs` — `AudioConfig`, `AudioRingBuffer` types
- `ui.rs` — Ratatui app shell, input handling, status bar, label fade, state persistence
- `visualizations/mod.rs` — `Visualization` trait definition
- `visualizations/registry.rs` — Plugin management, state save/load
- `visualizations/render.rs` — HalfBlockCanvas, BrailleCanvas, math helpers
- `visualizations/*.rs` — Individual visualization implementations

### Synchronization

- Audio→Processor: lock-free SPSC ring buffer (`ringbuf::HeapRb<f32>`)
- Processor→UI: bounded `mpsc::sync_channel(2)` with graceful frame drops
- Shutdown: `Arc<AtomicBool>` shared across all threads

### State Persistence

App state saved to `~/.config/terminal-vibes/state.toml`:
- Current visualization, sensitivity, beat intensity
- Per-visualization config (palette, toggles, etc.)

## Platform Constraints

- **macOS only** — Core Audio FFI bindings in `audio/tap.rs`
- **macOS 15+** — requires `AudioProcessTap` API
- macOS-specific deps are gated with `[target.'cfg(target_os = "macos")'.dependencies]`

## Adding a New Visualization

1. Create `src/visualizations/your_viz.rs` implementing the `Visualization` trait
2. Add `pub mod your_viz;` to `src/visualizations/mod.rs`
3. Register in `main.rs` where the other plugins are registered
4. Add integration test in `tests/your_viz_test.rs`
5. Per-plugin config lives under `[visualizations.<name>]` in TOML

## Performance Considerations

- Audio callback must never block or allocate — ring buffer is lock-free
- Visualizations should reuse buffers (canvas `resize_or_clear`, pre-allocated Vecs)
- Color quantization reduces terminal escape sequence volume (critical for tmux)
- HalfBlockCanvas generates 2 escape sequences per cell (fg+bg) — keep color diversity bounded
- Frame dropping is by design — processor drops if UI can't keep up
