# GUI Pixel Rendering — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add GPU-accelerated windowed mode (`--gui` flag) alongside the existing terminal visualizer, powered by wgpu + winit with WGSL shader pipeline and hot-reload.

**Architecture:** Cargo workspace with two crates: `terminal-vibes-core` (audio, FFT, beat detection, config — zero rendering deps) and `terminal-vibes` (binary with `terminal/` and `gpu/` modules). GPU code gated behind `gui` Cargo feature. Both modes consume `FrameData` from the same processing pipeline via a shared `AudioPipeline` orchestrator.

**Tech Stack:** wgpu (GPU), winit (windowing), notify (file watching), bytemuck (GPU data casting), WGSL (shaders)

**Design doc:** `docs/plans/2026-03-11-gui-pixel-rendering-design.md`

---

## Phase A: Workspace Migration (zero regressions)

### Task 1: Scaffold Cargo Workspace

**Files:**
- Modify: `Cargo.toml` (root → workspace definition)
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs` (empty placeholder)
- Create: `crates/app/Cargo.toml`
- Create: `crates/app/src/lib.rs` (empty placeholder)

**Step 1: Create crate directories**

```bash
mkdir -p crates/core/src crates/app/src
```

**Step 2: Create core Cargo.toml**

```toml
# crates/core/Cargo.toml
[package]
name = "terminal-vibes-core"
version = "1.6.5"
edition = "2021"
description = "Core audio processing library for terminal-vibes"
license = "MIT"

[dependencies]
rustfft = "6"
ringbuf = "0.4"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "6"
log = "0.4"
anyhow = "1"

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
core-foundation = "0.10"
uuid = { version = "1", features = ["v4"] }

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Media_Audio",
    "Win32_System_Com",
    "Win32_Devices_Properties",
] }

[target.'cfg(target_os = "linux")'.dependencies]
libpulse-binding = "2.28"
libpulse-simple-binding = "2.28"

[dev-dependencies]
approx = "0.5"
```

**Step 3: Create core lib.rs placeholder**

```rust
// crates/core/src/lib.rs
// Modules will be added in Task 2
```

**Step 4: Create app Cargo.toml**

```toml
# crates/app/Cargo.toml
[package]
name = "terminal-vibes"
version = "1.6.5"
edition = "2021"
description = "Terminal-based music visualizer for system audio"
license = "MIT"
repository = "https://github.com/joelmgallant/terminal-vibes"
readme = "../../README.md"
keywords = ["audio", "visualizer", "terminal", "music", "tui"]
categories = ["command-line-utilities", "multimedia::audio", "visualization"]

[dependencies]
terminal-vibes-core = { path = "../core" }
ratatui = "0.29"
crossterm = "0.28"
ringbuf = "0.4"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "6"
log = "0.4"
env_logger = "0.11"
clap = { version = "4", features = ["derive"] }
anyhow = "1"

[dev-dependencies]
approx = "0.5"

[features]
default = []
```

**Step 5: Create app lib.rs placeholder**

```rust
// crates/app/src/lib.rs
// Modules will be added in Task 3
```

**Step 6: Convert root Cargo.toml to workspace**

Replace the entire root `Cargo.toml` with:

```toml
[workspace]
members = ["crates/core", "crates/app"]
resolver = "2"
```

**Step 7: Verify workspace structure**

Run: `cargo metadata --no-deps --format-version 1 | head -5`
Expected: Shows both workspace members

**Step 8: Commit**

```bash
git add Cargo.toml crates/
git commit -m "chore: scaffold cargo workspace with core and app crates"
```

---

### Task 2: Extract Core Modules

**Files:**
- Move: `src/audio/` → `crates/core/src/audio/`
- Move: `src/beat.rs` → `crates/core/src/beat.rs`
- Move: `src/processing.rs` → `crates/core/src/processing.rs`
- Move: `src/config.rs` → `crates/core/src/config.rs`
- Modify: `crates/core/src/lib.rs`

**Step 1: Move core source files**

```bash
cp -r src/audio crates/core/src/audio
cp src/beat.rs crates/core/src/beat.rs
cp src/processing.rs crates/core/src/processing.rs
cp src/config.rs crates/core/src/config.rs
```

**Step 2: Write core lib.rs**

```rust
// crates/core/src/lib.rs
pub mod audio;
pub mod beat;
pub mod config;
pub mod processing;
```

**Step 3: Fix crate-internal imports in core modules**

The moved files use `crate::` imports that now refer to the core crate. Verify these resolve:

- `config.rs` line 1: `use crate::beat::BeatDetectionConfig;` — ✓ (beat is in core)
- `processing.rs`: `use crate::beat::{BeatData, TempoData};` — ✓ (beat is in core)
- `audio/mod.rs`: no cross-module imports — ✓

**Step 4: Verify core crate compiles**

Run: `cargo build -p terminal-vibes-core`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add crates/core/src/
git commit -m "refactor: extract audio, processing, beat, config into core crate"
```

---

### Task 3: Wire App Crate + Test Compatibility

**Files:**
- Move: `src/main.rs` → `crates/app/src/main.rs`
- Move: `src/ui.rs` → `crates/app/src/ui.rs`
- Move: `src/visualizations/` → `crates/app/src/visualizations/`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/main.rs` (update imports)
- Move: `tests/` → `crates/app/tests/`
- Delete: `src/lib.rs` and emptied `src/` directory

**Step 1: Move app source files**

```bash
cp src/main.rs crates/app/src/main.rs
cp src/ui.rs crates/app/src/ui.rs
cp -r src/visualizations crates/app/src/visualizations
cp -r tests crates/app/tests
```

**Step 2: Write app lib.rs with re-exports**

```rust
// crates/app/src/lib.rs
// Re-export core modules so integration tests and main.rs keep working
// with `terminal_vibes::processing::FrameData` etc.
pub use terminal_vibes_core::audio;
pub use terminal_vibes_core::beat;
pub use terminal_vibes_core::config;
pub use terminal_vibes_core::processing;

pub mod ui;
pub mod visualizations;
```

**Step 3: Update main.rs imports**

Replace the module declarations and imports at the top of `crates/app/src/main.rs`:

```rust
use anyhow::{Context, Result};
use clap::Parser;
use ringbuf::traits::{Consumer, Split};
use ringbuf::HeapRb;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Core types via lib.rs re-exports
use terminal_vibes::audio::{AudioConfig, AudioTap};
use terminal_vibes::beat::BeatDetector;
use terminal_vibes::config::Config;
use terminal_vibes::processing::{FrameData, Processor, ProcessorConfig};

// App types
use terminal_vibes::ui::App;
use terminal_vibes::visualizations::aurora::Aurora;
use terminal_vibes::visualizations::life::Life;
use terminal_vibes::visualizations::lissajous::Lissajous;
use terminal_vibes::visualizations::milkdrop::Milkdrop;
use terminal_vibes::visualizations::plasma::Plasma;
use terminal_vibes::visualizations::radial::RadialSpectrum;
use terminal_vibes::visualizations::rain::Rain;
use terminal_vibes::visualizations::registry::VisualizationRegistry;
use terminal_vibes::visualizations::spectrogram::Spectrogram;
use terminal_vibes::visualizations::spectrum::SpectrumBars;
use terminal_vibes::visualizations::starfield::Starfield;
use terminal_vibes::visualizations::tunnel::Tunnel;
use terminal_vibes::visualizations::waveform::Waveform;
```

Remove the old `mod` declarations (`mod audio;`, `mod beat;`, etc.) — these are no longer local modules.

The rest of `main()` stays identical.

**Step 4: Update ui.rs imports**

In `crates/app/src/ui.rs`, change any `use crate::` imports to work with the new structure. Since `lib.rs` re-exports core modules, `use crate::processing::FrameData` still resolves. No changes needed if all imports use `crate::`.

**Step 5: Update visualization imports**

Each viz file uses `use crate::processing::FrameData;` — this resolves via the re-export in `lib.rs`. No changes needed.

**Step 6: Remove old src/ files (now duplicated)**

```bash
rm -rf src/
```

**Step 7: Verify build**

Run: `cargo build -p terminal-vibes`
Expected: Compiles successfully

**Step 8: Verify tests**

Run: `cargo test -p terminal-vibes --lib`
Expected: All unit tests pass

Run: `cargo test -p terminal-vibes --test beat_test --test config_test --test render_test --test visualizations_test --test spectrum_test --test waveform_test`
Expected: Integration tests pass (skip `processing_test` — pre-existing compile error)

**Step 9: Verify clippy + fmt**

Run: `cargo fmt --all -- --check && cargo clippy --workspace`
Expected: No errors

**Step 10: Commit**

```bash
git add -A
git commit -m "refactor: wire app crate with core re-exports, migrate tests

All existing tests pass. Zero behavioral changes."
```

---

### Task 4: AudioPipeline Shared Module

Extract audio+processing thread setup into a reusable module so both terminal and GPU modes share it (DRY).

**Files:**
- Create: `crates/app/src/pipeline.rs`
- Modify: `crates/app/src/main.rs` (use AudioPipeline)
- Modify: `crates/app/src/lib.rs` (add pipeline module)

**Step 1: Write the AudioPipeline test**

Add to `crates/app/tests/pipeline_test.rs`:

```rust
use terminal_vibes::pipeline::AudioPipeline;
use terminal_vibes::config::Config;

#[test]
fn test_pipeline_config_defaults() {
    let config = Config::default();
    // Verify the config values that AudioPipeline will use
    assert_eq!(config.audio.fft_size, 2048);
    assert_eq!(config.audio.buffer_size, 4096);
    assert_eq!(config.audio.smoothing, 0.7);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p terminal-vibes --test pipeline_test`
Expected: FAIL — `pipeline` module doesn't exist

**Step 3: Create pipeline.rs**

```rust
// crates/app/src/pipeline.rs
use anyhow::{Context, Result};
use ringbuf::traits::{Consumer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::audio::{AudioConfig, AudioTap};
use crate::beat::BeatDetector;
use crate::config::Config;
use crate::processing::{FrameData, Processor, ProcessorConfig};

pub struct AudioPipeline {
    pub frame_rx: mpsc::Receiver<FrameData>,
    pub running: Arc<AtomicBool>,
    _audio_tap: AudioTap,
    processor_handle: Option<thread::JoinHandle<()>>,
}

impl AudioPipeline {
    pub fn start(config: &Config) -> Result<Self> {
        let rb = HeapRb::<f32>::new(config.audio.buffer_size);
        let (producer, mut consumer) = rb.split();

        let audio_config = AudioConfig {
            sample_rate: 44100.0,
            channels: 2,
        };
        let audio_tap =
            AudioTap::new(producer, audio_config.clone()).context(if cfg!(target_os = "macos") {
                "Failed to start audio capture. \
                 Make sure you're on macOS 15+ and have granted audio permissions."
            } else if cfg!(target_os = "windows") {
                "Failed to start audio capture. \
                 Make sure an audio output device is available."
            } else {
                "Failed to start audio capture. \
                 Make sure PulseAudio or PipeWire is running."
            })?;

        let (frame_tx, frame_rx) = mpsc::sync_channel::<FrameData>(2);
        let running = Arc::new(AtomicBool::new(true));
        let running_processor = running.clone();

        let fft_size = config.audio.fft_size;
        let smoothing = config.audio.smoothing;
        let beat_detection_config = config.beat_detection.clone();

        let processor_handle = thread::spawn(move || {
            let mut processor = Processor::new(ProcessorConfig {
                fft_size,
                smoothing,
                num_bands: 128,
                db_floor: -60.0,
            });
            let mut beat_detector = BeatDetector::new(128, beat_detection_config);

            let mut accum = Vec::with_capacity(fft_size * 2);
            let mut drain_buf = vec![0.0_f32; 4096];
            let interval = Duration::from_millis(1000 / 60);

            while running_processor.load(Ordering::Relaxed) {
                let count = consumer.pop_slice(&mut drain_buf);
                if count > 0 {
                    accum.extend_from_slice(&drain_buf[..count]);
                }

                while accum.len() >= fft_size {
                    let mut frame = processor.process(&accum[..fft_size]);
                    let (beat_data, tempo_data) = beat_detector.analyze(&frame.spectrum);
                    frame.beat = beat_data;
                    frame.tempo = tempo_data;
                    let _ = frame_tx.try_send(frame);

                    let keep = fft_size / 2;
                    let start = accum.len() - keep;
                    accum.drain(..start);
                }

                thread::sleep(interval);
            }
        });

        Ok(Self {
            frame_rx,
            running,
            _audio_tap: audio_tap,
            processor_handle: Some(processor_handle),
        })
    }
}

impl Drop for AudioPipeline {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.processor_handle.take() {
            let _ = handle.join();
        }
    }
}
```

**Step 4: Add pipeline module to lib.rs**

```rust
// crates/app/src/lib.rs
pub use terminal_vibes_core::audio;
pub use terminal_vibes_core::beat;
pub use terminal_vibes_core::config;
pub use terminal_vibes_core::processing;

pub mod pipeline;
pub mod ui;
pub mod visualizations;
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p terminal-vibes --test pipeline_test`
Expected: PASS

**Step 6: Refactor main.rs to use AudioPipeline**

```rust
// crates/app/src/main.rs
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use terminal_vibes::config::Config;
use terminal_vibes::pipeline::AudioPipeline;
use terminal_vibes::ui::App;
use terminal_vibes::visualizations::aurora::Aurora;
use terminal_vibes::visualizations::life::Life;
use terminal_vibes::visualizations::lissajous::Lissajous;
use terminal_vibes::visualizations::milkdrop::Milkdrop;
use terminal_vibes::visualizations::plasma::Plasma;
use terminal_vibes::visualizations::radial::RadialSpectrum;
use terminal_vibes::visualizations::rain::Rain;
use terminal_vibes::visualizations::registry::VisualizationRegistry;
use terminal_vibes::visualizations::spectrogram::Spectrogram;
use terminal_vibes::visualizations::spectrum::SpectrumBars;
use terminal_vibes::visualizations::starfield::Starfield;
use terminal_vibes::visualizations::tunnel::Tunnel;
use terminal_vibes::visualizations::waveform::Waveform;

#[derive(Parser)]
#[command(name = "terminal-vibes", about = "Terminal-based music visualizer")]
struct Cli {
    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    /// List available visualization modes
    #[arg(long)]
    list_modes: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

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
        println!("  life        - Audio-reactive Game of Life");
        return Ok(());
    }

    let config = Config::load(cli.config.as_ref()).context("Failed to load config")?;

    let pipeline = AudioPipeline::start(&config)?;

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
    registry.register(Box::new(Life::new()));
    registry.register(Box::new(Milkdrop::new()));

    let mut app = App::new(registry, config, pipeline.running.clone());
    app.run(pipeline.frame_rx)?;

    // pipeline.drop() handles shutdown
    Ok(())
}
```

**Step 7: Verify build + tests**

Run: `cargo build -p terminal-vibes && cargo test -p terminal-vibes --lib`
Expected: Compiles, all unit tests pass

**Step 8: Commit**

```bash
git add crates/app/src/pipeline.rs crates/app/src/main.rs crates/app/src/lib.rs crates/app/tests/pipeline_test.rs
git commit -m "refactor: extract AudioPipeline for shared audio+processing setup"
```

---

## Phase B: Terminal Module Restructure

### Task 5: Terminal Submodule + --gui CLI Flag

Move terminal rendering into `terminal/` submodule. Add `--gui` flag and `gui` feature gate.

**Files:**
- Create: `crates/app/src/terminal/mod.rs`
- Move: `crates/app/src/ui.rs` → `crates/app/src/terminal/ui.rs`
- Move: `crates/app/src/visualizations/` → `crates/app/src/terminal/visualizations/`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/main.rs` (add --gui flag, route)
- Modify: `crates/app/Cargo.toml` (add gui feature + optional deps)

**Step 1: Move files into terminal/ submodule**

```bash
mkdir -p crates/app/src/terminal
mv crates/app/src/ui.rs crates/app/src/terminal/ui.rs
mv crates/app/src/visualizations crates/app/src/terminal/visualizations
```

**Step 2: Create terminal/mod.rs**

```rust
// crates/app/src/terminal/mod.rs
pub mod ui;
pub mod visualizations;

use anyhow::Result;

use crate::config::Config;
use crate::pipeline::AudioPipeline;

use self::ui::App;
use self::visualizations::aurora::Aurora;
use self::visualizations::life::Life;
use self::visualizations::lissajous::Lissajous;
use self::visualizations::milkdrop::Milkdrop;
use self::visualizations::plasma::Plasma;
use self::visualizations::radial::RadialSpectrum;
use self::visualizations::rain::Rain;
use self::visualizations::registry::VisualizationRegistry;
use self::visualizations::spectrogram::Spectrogram;
use self::visualizations::spectrum::SpectrumBars;
use self::visualizations::starfield::Starfield;
use self::visualizations::tunnel::Tunnel;
use self::visualizations::waveform::Waveform;

pub fn run(config: Config) -> Result<()> {
    let pipeline = AudioPipeline::start(&config)?;

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
    registry.register(Box::new(Life::new()));
    registry.register(Box::new(Milkdrop::new()));

    let mut app = App::new(registry, config, pipeline.running.clone());
    app.run(pipeline.frame_rx)?;

    Ok(())
}
```

**Step 3: Update lib.rs**

```rust
// crates/app/src/lib.rs
pub use terminal_vibes_core::audio;
pub use terminal_vibes_core::beat;
pub use terminal_vibes_core::config;
pub use terminal_vibes_core::processing;

pub mod pipeline;
pub mod terminal;

// Re-export for test backward compatibility
pub use terminal::{ui, visualizations};

#[cfg(feature = "gui")]
pub mod gpu;
```

**Step 4: Simplify main.rs with routing**

```rust
// crates/app/src/main.rs
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use terminal_vibes::config::Config;

#[derive(Parser)]
#[command(name = "terminal-vibes", about = "Terminal-based music visualizer")]
struct Cli {
    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    /// List available visualization modes
    #[arg(long)]
    list_modes: bool,

    /// Launch GPU-accelerated window mode (requires --features gui)
    #[arg(long)]
    gui: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

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
        println!("  life        - Audio-reactive Game of Life");
        return Ok(());
    }

    let config = Config::load(cli.config.as_ref()).context("Failed to load config")?;

    if cli.gui {
        #[cfg(feature = "gui")]
        {
            terminal_vibes::gpu::run(config)?;
        }
        #[cfg(not(feature = "gui"))]
        {
            anyhow::bail!(
                "GUI mode requires the 'gui' feature.\n\
                 Build with: cargo build --features gui\n\
                 Run with:   cargo run --features gui -- --gui"
            );
        }
    } else {
        terminal_vibes::terminal::run(config)?;
    }

    Ok(())
}
```

**Step 5: Add gui feature to app Cargo.toml**

Add to `crates/app/Cargo.toml`:

```toml
[features]
default = []
gui = ["dep:wgpu", "dep:winit", "dep:notify", "dep:bytemuck"]

[dependencies.wgpu]
version = "24"
optional = true

[dependencies.winit]
version = "0.30"
optional = true

[dependencies.notify]
version = "7"
optional = true

[dependencies.bytemuck]
version = "1"
features = ["derive"]
optional = true
```

**Step 6: Create gpu module stub**

```rust
// crates/app/src/gpu/mod.rs
use anyhow::Result;
use crate::config::Config;

pub fn run(_config: Config) -> Result<()> {
    log::info!("GPU mode starting...");
    anyhow::bail!("GPU mode not yet implemented")
}
```

```bash
mkdir -p crates/app/src/gpu
```

**Step 7: Verify build (default features — no GPU deps)**

Run: `cargo build -p terminal-vibes`
Expected: Compiles without GPU dependencies

**Step 8: Verify build (gui feature)**

Run: `cargo build -p terminal-vibes --features gui`
Expected: Compiles with GPU dependencies (downloads wgpu, winit, etc.)

**Step 9: Verify tests**

Run: `cargo test -p terminal-vibes --lib`
Expected: All unit tests pass

Run a few integration tests to verify re-exports work:
Run: `cargo test -p terminal-vibes --test beat_test --test spectrum_test --test visualizations_test`
Expected: Pass

**Step 10: Commit**

```bash
git add -A
git commit -m "refactor: reorganize app into terminal/ submodule, add --gui flag

Terminal mode unchanged. GPU mode stubbed behind 'gui' feature flag.
--gui flag available in CLI, shows helpful error if feature not compiled."
```

---

## Phase C: GPU Foundation

### Task 6: GPU Window + wgpu Device

Create a window with wgpu rendering surface. Render a solid color to prove the pipeline works.

**Files:**
- Modify: `crates/app/src/gpu/mod.rs`
- Create: `crates/app/src/gpu/renderer.rs`
- Create: `crates/app/src/gpu/window.rs`

**Step 1: Write renderer.rs — wgpu device + surface setup**

```rust
// crates/app/src/gpu/renderer.rs
use anyhow::Result;
use std::sync::Arc;
use winit::window::Window;

pub struct GpuRenderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
}

impl GpuRenderer {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("terminal-vibes"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }, None)
            .await?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn render_clear(&self, r: f64, g: f64, b: f64) -> Result<()> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear_encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
```

**Step 2: Write window.rs — winit ApplicationHandler**

```rust
// crates/app/src/gpu/window.rs
use anyhow::Result;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use super::renderer::GpuRenderer;

pub struct GpuApp {
    window: Option<Arc<Window>>,
    renderer: Option<GpuRenderer>,
    width: u32,
    height: u32,
}

impl GpuApp {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            window: None,
            renderer: None,
            width,
            height,
        }
    }
}

impl ApplicationHandler for GpuApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("terminal-vibes")
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height));

        let window = Arc::new(event_loop.create_window(attrs).expect("Failed to create window"));

        let renderer = pollster::block_on(GpuRenderer::new(window.clone()))
            .expect("Failed to create GPU renderer");

        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(physical_size.width, physical_size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &self.renderer {
                    // For now, just clear to dark purple (beat will modulate later)
                    let _ = renderer.render_clear(0.05, 0.02, 0.08);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub fn run_window(width: u32, height: u32) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = GpuApp::new(width, height);
    event_loop.run_app(&mut app)?;
    Ok(())
}
```

**Step 3: Add pollster dependency**

Add to `crates/app/Cargo.toml`:

```toml
[dependencies.pollster]
version = "0.4"
optional = true
```

Update feature:
```toml
gui = ["dep:wgpu", "dep:winit", "dep:notify", "dep:bytemuck", "dep:pollster"]
```

**Step 4: Wire up gpu/mod.rs**

```rust
// crates/app/src/gpu/mod.rs
pub mod renderer;
pub mod window;

use anyhow::Result;
use crate::config::Config;

pub fn run(config: Config) -> Result<()> {
    let width = 1280; // Will come from config in Task 12
    let height = 720;
    log::info!("Starting GPU mode ({}x{})", width, height);
    window::run_window(width, height)
}
```

**Step 5: Verify build with gui feature**

Run: `cargo build -p terminal-vibes --features gui`
Expected: Compiles successfully

**Step 6: Manual test — window opens with dark background**

Run: `cargo run -p terminal-vibes --features gui -- --gui`
Expected: A 1280x720 window opens with dark purple background. Close with window X or Ctrl+C.

**Step 7: Verify default build still works**

Run: `cargo build -p terminal-vibes`
Expected: Compiles without GPU deps

**Step 8: Commit**

```bash
git add crates/app/src/gpu/ crates/app/Cargo.toml
git commit -m "feat(gui): add wgpu window with basic render surface

Opens 1280x720 window, clears to dark background. Foundation for shader pipeline."
```

---

### Task 7: Audio Texture + Uniform Buffer

Pure data conversion functions: FrameData → GPU-ready buffers. Fully testable with TDD.

**Files:**
- Create: `crates/app/src/gpu/audio_texture.rs`
- Create: `crates/app/src/gpu/uniforms.rs`
- Create: `crates/app/tests/gpu_audio_texture_test.rs`
- Create: `crates/app/tests/gpu_uniforms_test.rs`

**Step 1: Write audio texture test**

```rust
// crates/app/tests/gpu_audio_texture_test.rs
#![cfg(feature = "gui")]

use terminal_vibes::gpu::audio_texture::AudioTextureData;
use terminal_vibes::processing::FrameData;
use terminal_vibes::beat::{BeatData, TempoData};

fn make_test_frame() -> FrameData {
    FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 256],
        peak: 0.8,
        rms: 0.5,
        beat: BeatData::default(),
        tempo: TempoData::default(),
    }
}

#[test]
fn test_audio_texture_dimensions() {
    let frame = make_test_frame();
    let tex = AudioTextureData::from_frame(&frame);
    // 256 pixels wide, 2 rows, 4 bytes per pixel (RGBA)
    assert_eq!(tex.data.len(), 256 * 2 * 4);
    assert_eq!(tex.width, 256);
    assert_eq!(tex.height, 2);
}

#[test]
fn test_waveform_in_row_0() {
    let mut frame = make_test_frame();
    // Set waveform sample at index 0 to 0.75 (maps to ~191 in u8)
    frame.waveform[0] = 0.75;
    let tex = AudioTextureData::from_frame(&frame);
    // Row 0, pixel 0, red channel
    let r = tex.data[0];
    assert!(r > 180 && r < 200, "Expected ~191, got {}", r);
}

#[test]
fn test_spectrum_in_row_1() {
    let mut frame = make_test_frame();
    frame.spectrum[0] = 1.0;
    let tex = AudioTextureData::from_frame(&frame);
    // Row 1 starts at byte offset 256*4 = 1024
    let r = tex.data[1024];
    assert_eq!(r, 255);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p terminal-vibes --features gui --test gpu_audio_texture_test`
Expected: FAIL — module doesn't exist

**Step 3: Implement audio_texture.rs**

```rust
// crates/app/src/gpu/audio_texture.rs
use terminal_vibes_core::processing::FrameData;

/// 256x2 RGBA texture data following Shadertoy conventions:
/// - Row 0 (y=0.25): waveform samples, normalized 0..1
/// - Row 1 (y=0.75): FFT spectrum, normalized 0..1
pub struct AudioTextureData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl AudioTextureData {
    pub fn from_frame(frame: &FrameData) -> Self {
        let width = 256u32;
        let height = 2u32;
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Row 0: waveform (256 samples)
        for i in 0..256 {
            let sample = if i < frame.waveform.len() {
                // Waveform is -1..1, map to 0..1
                (frame.waveform[i] * 0.5 + 0.5).clamp(0.0, 1.0)
            } else {
                0.5
            };
            let byte = (sample * 255.0) as u8;
            let offset = i * 4;
            data[offset] = byte;     // R
            data[offset + 1] = byte; // G
            data[offset + 2] = byte; // B
            data[offset + 3] = 255;  // A
        }

        // Row 1: spectrum (128 bands → stretched to 256 pixels)
        let row1_offset = (width * 4) as usize;
        for i in 0..256 {
            let band_idx = i * frame.spectrum.len() / 256;
            let value = if band_idx < frame.spectrum.len() {
                frame.spectrum[band_idx].clamp(0.0, 1.0)
            } else {
                0.0
            };
            let byte = (value * 255.0) as u8;
            let offset = row1_offset + i * 4;
            data[offset] = byte;     // R
            data[offset + 1] = byte; // G
            data[offset + 2] = byte; // B
            data[offset + 3] = 255;  // A
        }

        Self {
            data,
            width,
            height,
        }
    }
}
```

**Step 4: Run audio texture test**

Run: `cargo test -p terminal-vibes --features gui --test gpu_audio_texture_test`
Expected: PASS

**Step 5: Write uniform buffer test**

```rust
// crates/app/tests/gpu_uniforms_test.rs
#![cfg(feature = "gui")]

use terminal_vibes::gpu::uniforms::Uniforms;
use terminal_vibes::processing::FrameData;
use terminal_vibes::beat::{BeatData, TempoData};

#[test]
fn test_uniforms_size_is_64_bytes() {
    assert_eq!(std::mem::size_of::<Uniforms>(), 64);
}

#[test]
fn test_uniforms_from_frame() {
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 256],
        peak: 0.8,
        rms: 0.5,
        beat: BeatData {
            bass_energy: 0.3,
            mid_energy: 0.5,
            treble_energy: 0.7,
            bass_beat: true,
            mid_beat: false,
            treble_beat: false,
            bass_envelope: 0.9,
            mid_envelope: 0.4,
            treble_envelope: 0.2,
            beat: true,
            envelope: 0.9,
        },
        tempo: TempoData {
            bpm: 120.0,
            confidence: 0.8,
            phase: 0.5,
            predicted_beat: false,
        },
    };

    let uniforms = Uniforms::from_frame(&frame, 1.5, 0.016, [1280.0, 720.0], 42);

    assert_eq!(uniforms.time, 1.5);
    assert_eq!(uniforms.delta_time, 0.016);
    assert_eq!(uniforms.resolution, [1280.0, 720.0]);
    assert_eq!(uniforms.frame, 42);
    assert_eq!(uniforms.beat_envelope, 0.9);
    assert_eq!(uniforms.bass_energy, 0.3);
    assert_eq!(uniforms.bpm, 120.0);
    assert_eq!(uniforms.bass_beat, 1.0); // true → 1.0
    assert_eq!(uniforms.mid_beat, 0.0);  // false → 0.0
}
```

**Step 6: Run uniform test to verify it fails**

Run: `cargo test -p terminal-vibes --features gui --test gpu_uniforms_test`
Expected: FAIL

**Step 7: Implement uniforms.rs**

```rust
// crates/app/src/gpu/uniforms.rs
use bytemuck::{Pod, Zeroable};
use terminal_vibes_core::processing::FrameData;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub time: f32,
    pub delta_time: f32,
    pub resolution: [f32; 2],
    pub frame: u32,
    pub beat_envelope: f32,
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,
    pub bass_beat: f32,
    pub mid_beat: f32,
    pub treble_beat: f32,
    pub bpm: f32,
    pub beat_phase: f32,
    pub beat_confidence: f32,
    pub _pad: f32,
}

impl Uniforms {
    pub fn from_frame(
        frame: &FrameData,
        time: f32,
        delta_time: f32,
        resolution: [f32; 2],
        frame_count: u32,
    ) -> Self {
        Self {
            time,
            delta_time,
            resolution,
            frame: frame_count,
            beat_envelope: frame.beat.envelope,
            bass_energy: frame.beat.bass_energy,
            mid_energy: frame.beat.mid_energy,
            treble_energy: frame.beat.treble_energy,
            bass_beat: if frame.beat.bass_beat { 1.0 } else { 0.0 },
            mid_beat: if frame.beat.mid_beat { 1.0 } else { 0.0 },
            treble_beat: if frame.beat.treble_beat { 1.0 } else { 0.0 },
            bpm: frame.tempo.bpm,
            beat_phase: frame.tempo.phase,
            beat_confidence: frame.tempo.confidence,
            _pad: 0.0,
        }
    }
}
```

**Step 8: Add modules to gpu/mod.rs**

```rust
// crates/app/src/gpu/mod.rs
pub mod audio_texture;
pub mod renderer;
pub mod uniforms;
pub mod window;

use anyhow::Result;
use crate::config::Config;

pub fn run(config: Config) -> Result<()> {
    let width = 1280;
    let height = 720;
    log::info!("Starting GPU mode ({}x{})", width, height);
    window::run_window(width, height)
}
```

**Step 9: Run all tests**

Run: `cargo test -p terminal-vibes --features gui --test gpu_audio_texture_test --test gpu_uniforms_test`
Expected: All PASS

**Step 10: Commit**

```bash
git add crates/app/src/gpu/audio_texture.rs crates/app/src/gpu/uniforms.rs crates/app/src/gpu/mod.rs crates/app/tests/gpu_audio_texture_test.rs crates/app/tests/gpu_uniforms_test.rs
git commit -m "feat(gui): add audio texture and uniform buffer conversion

Shadertoy-compatible 256x2 RGBA audio texture.
64-byte uniform struct with time, resolution, beat/tempo data.
Full test coverage for both data paths."
```

---

### Task 8: Fullscreen Shader Pipeline + First Shader

Build the render pipeline that draws a fullscreen triangle with a fragment shader, consuming the audio texture and uniforms.

**Files:**
- Modify: `crates/app/src/gpu/renderer.rs` (add shader pipeline)
- Create: `crates/app/src/gpu/shaders/fullscreen_vert.wgsl`
- Create: `crates/app/src/gpu/shaders/spectrum_rings.wgsl`

**Step 1: Create vertex shader**

```wgsl
// crates/app/src/gpu/shaders/fullscreen_vert.wgsl
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Fullscreen triangle: vertices at (-1,-1), (3,-1), (-1,3)
    let x = f32(i32(vertex_index & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}
```

**Step 2: Create first fragment shader — spectrum_rings**

```wgsl
// crates/app/src/gpu/shaders/spectrum_rings.wgsl

// --- Binding declarations (same in every viz shader) ---
struct Uniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    frame: u32,
    beat_envelope: f32,
    bass_energy: f32,
    mid_energy: f32,
    treble_energy: f32,
    bass_beat: f32,
    mid_beat: f32,
    treble_beat: f32,
    bpm: f32,
    beat_phase: f32,
    beat_confidence: f32,
    _pad: f32,
};

@group(0) @binding(0) var audio_data: texture_2d<f32>;
@group(0) @binding(1) var audio_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

// --- Visualization ---
@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;
    let center = uv - vec2<f32>(0.5, 0.5);
    let aspect = u.resolution.x / u.resolution.y;
    let corrected = vec2<f32>(center.x * aspect, center.y);
    let dist = length(corrected);
    let angle = atan2(corrected.y, corrected.x);

    // Sample spectrum at distance-mapped frequency index
    let freq_uv = clamp(dist * 1.5, 0.0, 1.0);
    let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(freq_uv, 0.75)).r;

    // Concentric rings modulated by spectrum amplitude
    let ring_freq = 20.0 + u.bass_energy * 10.0;
    let ring = sin(dist * ring_freq - u.time * 3.0) * 0.5 + 0.5;
    let brightness = ring * spectrum * (0.8 + u.beat_envelope * 0.8);

    // Color rotation based on angle and time
    let hue_shift = u.time * 0.3;
    let r = brightness * (0.5 + 0.5 * sin(angle * 2.0 + hue_shift));
    let g = brightness * (0.5 + 0.5 * sin(angle * 2.0 + hue_shift + 2.094));
    let b = brightness * (0.5 + 0.5 * sin(angle * 2.0 + hue_shift + 4.189));

    // Vignette
    let vignette = 1.0 - smoothstep(0.3, 0.9, dist);

    return vec4<f32>(r * vignette, g * vignette, b * vignette, 1.0);
}
```

**Step 3: Add shader pipeline to renderer.rs**

Add these methods and structs to `GpuRenderer`:

```rust
// Add to crates/app/src/gpu/renderer.rs

use super::audio_texture::AudioTextureData;
use super::uniforms::Uniforms;

// Add to GpuRenderer struct fields:
//   pipeline: Option<wgpu::RenderPipeline>,
//   bind_group: Option<wgpu::BindGroup>,
//   bind_group_layout: wgpu::BindGroupLayout,
//   audio_texture: wgpu::Texture,
//   uniform_buffer: wgpu::Buffer,
//   vertex_shader: wgpu::ShaderModule,

impl GpuRenderer {
    // Add after new():

    pub fn init_pipeline(&mut self, fragment_wgsl: &str) -> anyhow::Result<()> {
        let vertex_shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fullscreen_vert"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/fullscreen_vert.wgsl").into(),
            ),
        });

        let fragment_shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fragment"),
            source: wgpu::ShaderSource::Wgsl(fragment_wgsl.into()),
        });

        // Create 256x2 audio texture
        let audio_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("audio_data"),
            size: wgpu::Extent3d { width: 256, height: 2, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let audio_texture_view = audio_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let audio_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("audio_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Uniform buffer
        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viz_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&audio_texture_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&audio_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: uniform_buffer.as_entire_binding() },
            ],
        });

        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viz_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viz_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.pipeline = Some(pipeline);
        self.bind_group = Some(bind_group);
        self.audio_texture = Some(audio_texture);
        self.uniform_buffer = Some(uniform_buffer);
        self.bind_group_layout = Some(bind_group_layout);

        Ok(())
    }

    pub fn update_audio_texture(&self, tex_data: &AudioTextureData) {
        if let Some(texture) = &self.audio_texture {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &tex_data.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(tex_data.width * 4),
                    rows_per_image: Some(tex_data.height),
                },
                wgpu::Extent3d {
                    width: tex_data.width,
                    height: tex_data.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub fn update_uniforms(&self, uniforms: &Uniforms) {
        if let Some(buffer) = &self.uniform_buffer {
            self.queue.write_buffer(buffer, 0, bytemuck::bytes_of(uniforms));
        }
    }

    pub fn render_shader(&self) -> anyhow::Result<()> {
        let pipeline = self.pipeline.as_ref().ok_or_else(|| anyhow::anyhow!("No pipeline"))?;
        let bind_group = self.bind_group.as_ref().ok_or_else(|| anyhow::anyhow!("No bind group"))?;

        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shader_encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shader_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
```

Note: The `GpuRenderer` struct needs to be updated to include the new optional fields (`pipeline`, `bind_group`, `audio_texture`, `uniform_buffer`, `bind_group_layout`). Initialize them as `None` in `new()`.

**Step 4: Verify build**

Run: `cargo build -p terminal-vibes --features gui`
Expected: Compiles

**Step 5: Commit**

```bash
git add crates/app/src/gpu/
git commit -m "feat(gui): fullscreen shader pipeline with spectrum_rings shader

Fullscreen triangle vertex shader + fragment shader pipeline.
Audio texture upload + uniform buffer update per frame.
First built-in viz: spectrum_rings (concentric audio-reactive rings)."
```

---

### Task 9: GPU Event Loop + Input + Audio Integration

Wire the audio pipeline into the GPU window loop. Add keyboard input handling.

**Files:**
- Modify: `crates/app/src/gpu/window.rs`
- Modify: `crates/app/src/gpu/mod.rs`

**Step 1: Update GpuApp to hold audio pipeline and state**

Refactor `crates/app/src/gpu/window.rs` to integrate `AudioPipeline`:

```rust
// crates/app/src/gpu/window.rs
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

use crate::config::Config;
use crate::pipeline::AudioPipeline;
use crate::processing::FrameData;

use super::audio_texture::AudioTextureData;
use super::renderer::GpuRenderer;
use super::uniforms::Uniforms;

pub struct GpuApp {
    config: Config,
    pipeline: Option<AudioPipeline>,
    window: Option<Arc<Window>>,
    renderer: Option<GpuRenderer>,
    start_time: Instant,
    last_frame_time: Instant,
    frame_count: u32,
    sensitivity: f32,
    beat_intensity: f32,
    fullscreen: bool,
    latest_frame: Option<FrameData>,
}

impl GpuApp {
    pub fn new(config: Config) -> Self {
        let now = Instant::now();
        Self {
            config,
            pipeline: None,
            window: None,
            renderer: None,
            start_time: now,
            last_frame_time: now,
            frame_count: 0,
            sensitivity: 1.0,
            beat_intensity: 1.0,
            fullscreen: false,
            latest_frame: None,
        }
    }

    fn drain_audio(&mut self) {
        if let Some(pipeline) = &self.pipeline {
            // Drain channel, keep only latest frame
            while let Ok(frame) = pipeline.frame_rx.try_recv() {
                self.latest_frame = Some(frame);
            }
        }
    }

    fn handle_key(&mut self, event: &KeyEvent, event_loop: &ActiveEventLoop) {
        if event.state != ElementState::Pressed {
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Character(c) => match c.as_str() {
                "q" => event_loop.exit(),
                "+" | "=" => {
                    self.sensitivity = (self.sensitivity + 0.1).min(5.0);
                    log::info!("Sensitivity: {:.1}", self.sensitivity);
                }
                "-" => {
                    self.sensitivity = (self.sensitivity - 0.1).max(0.1);
                    log::info!("Sensitivity: {:.1}", self.sensitivity);
                }
                "b" => {
                    self.beat_intensity = (self.beat_intensity + 0.1).min(3.0);
                    log::info!("Beat intensity: {:.1}", self.beat_intensity);
                }
                "B" => {
                    self.beat_intensity = (self.beat_intensity - 0.1).max(0.0);
                    log::info!("Beat intensity: {:.1}", self.beat_intensity);
                }
                "f" => self.toggle_fullscreen(),
                _ => {}
            },
            Key::Named(NamedKey::F11) => self.toggle_fullscreen(),
            Key::Named(NamedKey::Tab) => {
                // TODO: cycle viz (Task 11)
                log::info!("Tab pressed — viz cycling coming in Task 11");
            }
            _ => {}
        }
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(window) = &self.window {
            self.fullscreen = !self.fullscreen;
            if self.fullscreen {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            } else {
                window.set_fullscreen(None);
            }
        }
    }
}

impl ApplicationHandler for GpuApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let width = 1280u32;
        let height = 720u32;

        let attrs = WindowAttributes::default()
            .with_title("terminal-vibes")
            .with_inner_size(winit::dpi::LogicalSize::new(width, height));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        let mut renderer = pollster::block_on(GpuRenderer::new(window.clone()))
            .expect("Failed to create GPU renderer");

        // Initialize shader pipeline with built-in spectrum_rings
        let shader_src = include_str!("shaders/spectrum_rings.wgsl");
        renderer
            .init_pipeline(shader_src)
            .expect("Failed to init shader pipeline");

        self.renderer = Some(renderer);
        self.window = Some(window);

        // Start audio pipeline
        match AudioPipeline::start(&self.config) {
            Ok(pipeline) => self.pipeline = Some(pipeline),
            Err(e) => log::error!("Failed to start audio: {}", e),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key(&event, event_loop);
            }
            WindowEvent::RedrawRequested => {
                self.drain_audio();

                let now = Instant::now();
                let time = now.duration_since(self.start_time).as_secs_f32();
                let delta = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                if let (Some(renderer), Some(frame)) = (&self.renderer, &self.latest_frame) {
                    let mut scaled_frame = frame.clone();
                    // Apply sensitivity
                    for s in &mut scaled_frame.spectrum {
                        *s = (*s * self.sensitivity).min(1.0);
                    }
                    // Apply beat intensity
                    scaled_frame.beat.envelope *= self.beat_intensity;
                    scaled_frame.beat.bass_envelope *= self.beat_intensity;
                    scaled_frame.beat.mid_envelope *= self.beat_intensity;
                    scaled_frame.beat.treble_envelope *= self.beat_intensity;

                    let tex_data = AudioTextureData::from_frame(&scaled_frame);
                    renderer.update_audio_texture(&tex_data);

                    let resolution = [
                        renderer.surface_config.width as f32,
                        renderer.surface_config.height as f32,
                    ];
                    let uniforms =
                        Uniforms::from_frame(&scaled_frame, time, delta, resolution, self.frame_count);
                    renderer.update_uniforms(&uniforms);

                    if let Err(e) = renderer.render_shader() {
                        log::error!("Render error: {}", e);
                    }
                } else if let Some(renderer) = &self.renderer {
                    // No audio data yet — clear to black
                    let _ = renderer.render_clear(0.0, 0.0, 0.0);
                }

                self.frame_count += 1;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub fn run_window(config: Config) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = GpuApp::new(config);
    event_loop.run_app(&mut app)?;
    Ok(())
}
```

**Step 2: Update gpu/mod.rs to pass config**

```rust
// crates/app/src/gpu/mod.rs
pub mod audio_texture;
pub mod renderer;
pub mod uniforms;
pub mod window;

use anyhow::Result;
use crate::config::Config;

pub fn run(config: Config) -> Result<()> {
    log::info!("Starting GPU mode");
    window::run_window(config)
}
```

**Step 3: Ensure FrameData is Clone**

Check if `FrameData`, `BeatData`, and `TempoData` derive `Clone`. If not, add `#[derive(Clone)]` to each in `crates/core/src/processing.rs` and `crates/core/src/beat.rs`.

**Step 4: Verify build**

Run: `cargo build -p terminal-vibes --features gui`
Expected: Compiles

**Step 5: Manual test — audio-reactive visualization**

Run: `cargo run -p terminal-vibes --features gui -- --gui`
Expected: Window opens with spectrum rings reacting to system audio. +/- adjusts sensitivity, f toggles fullscreen, q/Esc quits.

**Step 6: Commit**

```bash
git add crates/app/src/gpu/
git commit -m "feat(gui): integrate audio pipeline into GPU event loop

Audio-reactive rendering with spectrum_rings shader.
Keyboard input: +/- sensitivity, b/B beat intensity, f fullscreen, q quit.
Window title, resize handling, continuous redraw."
```

---

### Task 10: Shader Hot-Reload

Watch shader directory for changes, recompile on the fly.

**Files:**
- Create: `crates/app/src/gpu/shader_loader.rs`
- Modify: `crates/app/src/gpu/mod.rs`
- Modify: `crates/app/src/gpu/window.rs`

**Step 1: Implement shader_loader.rs**

```rust
// crates/app/src/gpu/shader_loader.rs
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

pub struct ShaderEntry {
    pub name: String,
    pub source: String,
    pub path: Option<PathBuf>, // None for built-in shaders
}

pub enum ShaderEvent {
    Modified(PathBuf),
    Created(PathBuf),
    Removed(PathBuf),
}

pub struct ShaderLoader {
    shaders: Vec<ShaderEntry>,
    current_index: usize,
    event_rx: mpsc::Receiver<ShaderEvent>,
    _watcher: Option<RecommendedWatcher>,
}

impl ShaderLoader {
    pub fn new(extra_dirs: &[PathBuf]) -> Self {
        let (event_tx, event_rx) = mpsc::channel();

        // Built-in shaders
        let mut shaders = vec![
            ShaderEntry {
                name: "spectrum_rings".to_string(),
                source: include_str!("shaders/spectrum_rings.wgsl").to_string(),
                path: None,
            },
            ShaderEntry {
                name: "audio_wave".to_string(),
                source: include_str!("shaders/audio_wave.wgsl").to_string(),
                path: None,
            },
            ShaderEntry {
                name: "plasma".to_string(),
                source: include_str!("shaders/plasma.wgsl").to_string(),
                path: None,
            },
        ];

        // Watch shader directories
        let shader_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("terminal-vibes")
            .join("shaders");

        let mut dirs_to_watch = vec![shader_dir];
        dirs_to_watch.extend(extra_dirs.iter().cloned());

        // Load existing .wgsl files from watched dirs
        for dir in &dirs_to_watch {
            if dir.exists() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == "wgsl") {
                            if let Ok(source) = std::fs::read_to_string(&path) {
                                let name = path
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                shaders.push(ShaderEntry {
                                    name,
                                    source,
                                    path: Some(path),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Set up file watcher
        let tx = event_tx.clone();
        let watcher_result = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in &event.paths {
                        if path.extension().map_or(false, |e| e == "wgsl") {
                            let shader_event = match event.kind {
                                EventKind::Create(_) => Some(ShaderEvent::Created(path.clone())),
                                EventKind::Modify(_) => Some(ShaderEvent::Modified(path.clone())),
                                EventKind::Remove(_) => Some(ShaderEvent::Removed(path.clone())),
                                _ => None,
                            };
                            if let Some(e) = shader_event {
                                let _ = tx.send(e);
                            }
                        }
                    }
                }
            },
            Config::default(),
        );

        let watcher = match watcher_result {
            Ok(mut w) => {
                for dir in &dirs_to_watch {
                    if dir.exists() {
                        let _ = w.watch(dir, RecursiveMode::NonRecursive);
                    }
                }
                Some(w)
            }
            Err(e) => {
                log::warn!("Failed to set up shader watcher: {}", e);
                None
            }
        };

        Self {
            shaders,
            current_index: 0,
            event_rx,
            _watcher: watcher,
        }
    }

    pub fn current(&self) -> Option<&ShaderEntry> {
        self.shaders.get(self.current_index)
    }

    pub fn current_name(&self) -> &str {
        self.current()
            .map(|s| s.name.as_str())
            .unwrap_or("none")
    }

    pub fn current_source(&self) -> &str {
        self.current()
            .map(|s| s.source.as_str())
            .unwrap_or("")
    }

    pub fn next(&mut self) {
        if !self.shaders.is_empty() {
            self.current_index = (self.current_index + 1) % self.shaders.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.shaders.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.shaders.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    /// Poll for file changes. Returns true if the current shader was modified
    /// (caller should rebuild pipeline).
    pub fn poll_changes(&mut self) -> bool {
        let mut current_changed = false;

        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ShaderEvent::Modified(path) => {
                    if let Some(entry) = self.shaders.iter_mut().find(|s| s.path.as_ref() == Some(&path)) {
                        match std::fs::read_to_string(&path) {
                            Ok(source) => {
                                entry.source = source;
                                if self.shaders.get(self.current_index).and_then(|s| s.path.as_ref()) == Some(&path) {
                                    current_changed = true;
                                }
                                log::info!("Shader reloaded: {}", entry.name);
                            }
                            Err(e) => log::warn!("Failed to read shader {}: {}", path.display(), e),
                        }
                    }
                }
                ShaderEvent::Created(path) => {
                    match std::fs::read_to_string(&path) {
                        Ok(source) => {
                            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                            log::info!("New shader discovered: {}", name);
                            self.shaders.push(ShaderEntry { name, source, path: Some(path) });
                        }
                        Err(e) => log::warn!("Failed to read new shader {}: {}", path.display(), e),
                    }
                }
                ShaderEvent::Removed(path) => {
                    if let Some(idx) = self.shaders.iter().position(|s| s.path.as_ref() == Some(&path)) {
                        let name = self.shaders[idx].name.clone();
                        log::info!("Shader removed: {}", name);
                        self.shaders.remove(idx);
                        if self.current_index >= self.shaders.len() && !self.shaders.is_empty() {
                            self.current_index = self.shaders.len() - 1;
                            current_changed = true;
                        } else if idx == self.current_index {
                            current_changed = true;
                        } else if idx < self.current_index {
                            self.current_index -= 1;
                        }
                    }
                }
            }
        }

        current_changed
    }

    pub fn shader_count(&self) -> usize {
        self.shaders.len()
    }
}
```

**Step 2: Create additional built-in shaders**

`crates/app/src/gpu/shaders/audio_wave.wgsl`:

```wgsl
struct Uniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    frame: u32,
    beat_envelope: f32,
    bass_energy: f32,
    mid_energy: f32,
    treble_energy: f32,
    bass_beat: f32,
    mid_beat: f32,
    treble_beat: f32,
    bpm: f32,
    beat_phase: f32,
    beat_confidence: f32,
    _pad: f32,
};

@group(0) @binding(0) var audio_data: texture_2d<f32>;
@group(0) @binding(1) var audio_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3<f32>(h, h, h) + k) * 6.0 - vec3<f32>(3.0, 3.0, 3.0));
    return v * mix(vec3<f32>(1.0, 1.0, 1.0), clamp(p - vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.0), vec3<f32>(1.0)), s);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;

    // Top half: spectrum bars
    if uv.y > 0.5 {
        let bar_y = (uv.y - 0.5) * 2.0;
        let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(uv.x, 0.75)).r;
        if bar_y < spectrum {
            let color = hsv2rgb(uv.x * 0.8, 0.8, 1.0 - bar_y * 0.3);
            return vec4<f32>(color * (0.8 + u.beat_envelope * 0.4), 1.0);
        }
        return vec4<f32>(0.02, 0.02, 0.05, 1.0);
    }

    // Bottom half: waveform with glow
    let waveform = textureSample(audio_data, audio_sampler, vec2<f32>(uv.x, 0.25)).r;
    let wave_y = uv.y * 2.0;
    let wave_pos = waveform * 0.4 + 0.5;
    let dist = abs(wave_y - wave_pos);
    let glow = 0.004 / (dist * dist + 0.004);
    let color = vec3<f32>(0.2, 0.8, 0.4) * glow * (0.6 + u.beat_envelope * 0.5);

    return vec4<f32>(color, 1.0);
}
```

`crates/app/src/gpu/shaders/plasma.wgsl`:

```wgsl
struct Uniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    frame: u32,
    beat_envelope: f32,
    bass_energy: f32,
    mid_energy: f32,
    treble_energy: f32,
    bass_beat: f32,
    mid_beat: f32,
    treble_beat: f32,
    bpm: f32,
    beat_phase: f32,
    beat_confidence: f32,
    _pad: f32,
};

@group(0) @binding(0) var audio_data: texture_2d<f32>;
@group(0) @binding(1) var audio_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;
    let t = u.time;
    let aspect = u.resolution.x / u.resolution.y;
    let p = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);

    let bass = u.bass_energy;
    let mid = u.mid_energy;
    let treble = u.treble_energy;

    var v = 0.0;
    v += sin(p.x * 10.0 + t * 1.2 + bass * 6.0);
    v += sin(p.y * 12.0 + t * 0.8 + mid * 6.0);
    v += sin((p.x + p.y) * 8.0 + t * 0.6);
    v += sin(length(p) * 14.0 - t * 2.0 + treble * 6.0);
    v += sin(length(p - vec2<f32>(0.3 * sin(t * 0.5), 0.2 * cos(t * 0.7))) * 12.0);
    v *= 0.2;

    let r = sin(v * 3.14159 + t * 0.3) * 0.5 + 0.5;
    let g = sin(v * 3.14159 + t * 0.3 + 2.094) * 0.5 + 0.5;
    let b = sin(v * 3.14159 + t * 0.3 + 4.189) * 0.5 + 0.5;

    let brightness = 0.6 + u.beat_envelope * 0.4;

    return vec4<f32>(r * brightness, g * brightness, b * brightness, 1.0);
}
```

**Step 3: Integrate shader loader into GpuApp**

Update `window.rs` to use `ShaderLoader` instead of hardcoded shader. In `GpuApp::new()`, create a `ShaderLoader`. In `resumed()`, init pipeline from `shader_loader.current_source()`. In `RedrawRequested`, call `shader_loader.poll_changes()` — if true, rebuild pipeline. Tab/Shift-Tab call `shader_loader.next()`/`shader_loader.prev()` and rebuild pipeline.

Add a `rebuild_pipeline` helper:

```rust
fn rebuild_pipeline(&mut self) {
    if let Some(renderer) = &mut self.renderer {
        let source = self.shader_loader.current_source().to_string();
        match renderer.init_pipeline(&source) {
            Ok(()) => {
                log::info!("Shader loaded: {}", self.shader_loader.current_name());
                if let Some(window) = &self.window {
                    window.set_title(&format!("terminal-vibes — {}", self.shader_loader.current_name()));
                }
            }
            Err(e) => {
                log::warn!("Shader compile error: {}. Keeping previous shader.", e);
            }
        }
    }
}
```

**Step 4: Update Tab/Shift-Tab handling**

```rust
Key::Named(NamedKey::Tab) => {
    self.shader_loader.next();
    self.rebuild_pipeline();
}
// For Shift+Tab, check event.modifiers or handle BackTab
```

**Step 5: Verify build**

Run: `cargo build -p terminal-vibes --features gui`
Expected: Compiles

**Step 6: Manual test — shader cycling and hot-reload**

Run: `cargo run -p terminal-vibes --features gui -- --gui`

1. Press Tab — should cycle through spectrum_rings, audio_wave, plasma
2. Create `~/.config/terminal-vibes/shaders/test.wgsl` with a copy of plasma.wgsl — should appear as new viz
3. Edit the file — shader should hot-reload
4. Delete the file — should fall back to previous viz

**Step 7: Commit**

```bash
git add crates/app/src/gpu/
git commit -m "feat(gui): shader hot-reload with built-in presets

ShaderLoader watches ~/.config/terminal-vibes/shaders/ for .wgsl files.
Hot-reload on save, auto-discover new files, graceful error handling.
Built-in presets: spectrum_rings, audio_wave, plasma.
Tab/Shift-Tab cycles through all available shaders."
```

---

### Task 11: Config Additions + Status Overlay

Add `[gui]` config section and window title status display.

**Files:**
- Modify: `crates/core/src/config.rs`
- Modify: `crates/app/src/gpu/window.rs`
- Create: `crates/app/tests/gui_config_test.rs`

**Step 1: Write config test**

```rust
// crates/app/tests/gui_config_test.rs
use terminal_vibes::config::Config;

#[test]
fn test_gui_config_defaults() {
    let config = Config::default();
    assert_eq!(config.gui.width, 1280);
    assert_eq!(config.gui.height, 720);
    assert!(config.gui.vsync);
    assert!(!config.gui.fullscreen);
}

#[test]
fn test_gui_config_from_toml() {
    let toml_str = r#"
    [gui]
    width = 1920
    height = 1080
    vsync = false
    fullscreen = true
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.gui.width, 1920);
    assert_eq!(config.gui.height, 1080);
    assert!(!config.gui.vsync);
    assert!(config.gui.fullscreen);
}
```

**Step 2: Run test — fails**

Run: `cargo test -p terminal-vibes --test gui_config_test`
Expected: FAIL — `gui` field doesn't exist on Config

**Step 3: Add GuiConfig to config.rs**

```rust
// Add to crates/core/src/config.rs

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GuiConfig {
    pub width: u32,
    pub height: u32,
    pub vsync: bool,
    pub fullscreen: bool,
    pub extra_shader_dirs: Vec<String>,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            vsync: true,
            fullscreen: false,
            extra_shader_dirs: vec![],
        }
    }
}
```

Add `pub gui: GuiConfig` field to `Config` struct.

**Step 4: Run test — passes**

Run: `cargo test -p terminal-vibes --test gui_config_test`
Expected: PASS

**Step 5: Wire config into GPU window**

Update `GpuApp::resumed()` to use `self.config.gui.width`, `self.config.gui.height`, `self.config.gui.vsync`, `self.config.gui.fullscreen`.

Update `ShaderLoader::new()` to accept `&config.gui.extra_shader_dirs`.

Set present mode based on vsync:
```rust
let present_mode = if self.config.gui.vsync {
    wgpu::PresentMode::AutoVsync
} else {
    wgpu::PresentMode::AutoNoVsync
};
```

**Step 6: Add window title status**

Update the window title on each frame to show current viz name, BPM, and FPS:

```rust
// In RedrawRequested handler, after rendering:
if self.frame_count % 30 == 0 {
    if let Some(window) = &self.window {
        let fps = 1.0 / delta.max(0.001);
        let bpm = self.latest_frame.as_ref().map_or(0.0, |f| f.tempo.bpm);
        let viz_name = self.shader_loader.current_name();
        window.set_title(&format!(
            "terminal-vibes — {} | {:.0} BPM | {:.0} FPS",
            viz_name, bpm, fps
        ));
    }
}
```

**Step 7: Verify build + tests**

Run: `cargo test -p terminal-vibes --test gui_config_test && cargo build -p terminal-vibes --features gui`
Expected: All pass

**Step 8: Create example shader template**

Create `crates/app/src/gpu/shaders/example_template.wgsl` with detailed comments explaining the uniform interface. This file gets copied to the user's shader dir on first run (or shipped in docs).

```wgsl
// example_template.wgsl — Template for custom terminal-vibes shaders
//
// Save .wgsl files to ~/.config/terminal-vibes/shaders/
// They will be automatically discovered and hot-reloaded.
//
// Available inputs:
//   audio_data (texture): 256x2 RGBA texture
//     - Row 0 (sample at y=0.25): waveform (PCM samples, 0..1)
//     - Row 1 (sample at y=0.75): FFT spectrum (frequency bands, 0..1)
//   u.time: seconds since start
//   u.delta_time: seconds since last frame
//   u.resolution: window size in pixels
//   u.beat_envelope: overall beat envelope (0..1, fast attack, slow decay)
//   u.bass_energy, u.mid_energy, u.treble_energy: frequency band energy
//   u.bass_beat, u.mid_beat, u.treble_beat: 1.0 on beat, 0.0 otherwise
//   u.bpm: estimated beats per minute
//   u.beat_phase: 0..1 phase within beat cycle

struct Uniforms {
    time: f32, delta_time: f32, resolution: vec2<f32>, frame: u32,
    beat_envelope: f32, bass_energy: f32, mid_energy: f32, treble_energy: f32,
    bass_beat: f32, mid_beat: f32, treble_beat: f32,
    bpm: f32, beat_phase: f32, beat_confidence: f32, _pad: f32,
};

@group(0) @binding(0) var audio_data: texture_2d<f32>;
@group(0) @binding(1) var audio_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;

    // Your visualization here!
    let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(uv.x, 0.75)).r;
    let brightness = spectrum * (0.5 + u.beat_envelope * 0.5);

    return vec4<f32>(uv.x * brightness, uv.y * brightness, brightness, 1.0);
}
```

**Step 9: Commit**

```bash
git add crates/core/src/config.rs crates/app/src/gpu/ crates/app/tests/gui_config_test.rs
git commit -m "feat(gui): add [gui] config section, status in window title

Config: width, height, vsync, fullscreen, extra_shader_dirs.
Window title shows viz name, BPM, FPS.
Ships example_template.wgsl for user-authored shaders."
```

---

### Task 12: Final Integration + Verification

End-to-end verification, documentation update, cleanup.

**Files:**
- Modify: `CLAUDE.md` (add GUI mode docs)
- Verify all tests pass in both modes

**Step 1: Run full test suite (default features)**

Run: `cargo test -p terminal-vibes-core && cargo test -p terminal-vibes --lib`
Expected: All pass — zero regressions to terminal mode

**Step 2: Run GUI-specific tests**

Run: `cargo test -p terminal-vibes --features gui --test gpu_audio_texture_test --test gpu_uniforms_test --test gui_config_test`
Expected: All pass

**Step 3: Run clippy on workspace**

Run: `cargo clippy --workspace && cargo clippy --workspace --features gui`
Expected: No warnings

**Step 4: Run fmt check**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues

**Step 5: Manual smoke test — terminal mode unchanged**

Run: `cargo run -p terminal-vibes`
Expected: Terminal visualizer works exactly as before

**Step 6: Manual smoke test — GUI mode**

Run: `cargo run -p terminal-vibes --features gui -- --gui`
Expected:
- Window opens at configured size
- Audio-reactive shader visualization
- Tab cycles through spectrum_rings → audio_wave → plasma
- +/- adjusts sensitivity
- f toggles fullscreen
- Window title shows viz name, BPM, FPS
- q/Esc quits cleanly

**Step 7: Update CLAUDE.md with GUI build commands**

Add to the Build & Development Commands section:

```markdown
cargo build --features gui        # Build with GPU mode
cargo run --features gui -- --gui  # Run GPU window mode
```

**Step 8: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add GUI mode build commands to CLAUDE.md"
```
