# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Terminal-based music visualizer for macOS. Captures system audio via Core Audio's `AudioProcessTap` API (requires macOS 15+) and renders real-time visualizations using ratatui in the terminal.

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

### Key Data Type

`FrameData` (in `processing.rs`) flows from processor to UI:
- `spectrum: Vec<f32>` — 128 logarithmic frequency bands, normalized 0..1
- `waveform: Vec<f32>` — raw PCM samples for oscilloscope
- `peak: f32` / `rms: f32` — amplitude metrics

### Visualization Plugin System

All visualizations implement the `Visualization` trait (`visualizations/mod.rs`):
- `update(&mut self, frame: &FrameData)` — receive new data
- `render(&self, area: Rect, buf: &mut Buffer)` — draw to terminal
- `on_key(&mut self, key: KeyEvent) -> bool` — handle mode-specific input

Plugins are registered in `VisualizationRegistry` and switched via Tab/Shift+Tab.

**Built-in plugins**: SpectrumBars (12 color palettes), Waveform (oscilloscope), Spectrogram (scrolling heatmap)

### Module Map

- `main.rs` — CLI parsing, thread orchestration, shutdown coordination
- `config.rs` — TOML config with XDG paths (`~/.config/terminal-vibes/config.toml`)
- `processing.rs` — FFT pipeline and `FrameData` production
- `audio/tap.rs` — Core Audio FFI, `AudioTap` lifecycle (all unsafe code lives here)
- `audio/mod.rs` — `AudioConfig`, `AudioRingBuffer` types
- `ui.rs` — Ratatui app shell, input handling, status bar
- `visualizations/` — Trait definition, registry, and plugin implementations

### Synchronization

- Audio→Processor: lock-free SPSC ring buffer (`ringbuf::HeapRb<f32>`)
- Processor→UI: bounded `mpsc::sync_channel(2)` with graceful frame drops
- Shutdown: `Arc<AtomicBool>` shared across all threads

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
