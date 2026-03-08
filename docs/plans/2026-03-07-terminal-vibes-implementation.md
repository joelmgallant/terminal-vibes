# Terminal Vibes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a terminal-based music visualizer that captures macOS system audio via Core Audio `AudioProcessTap` (macOS 15+) and renders real-time visualizations in the terminal.

**Architecture:** Three-thread pipeline — audio callback writes PCM into a lock-free ring buffer, processor thread runs FFT and produces `FrameData`, main thread renders via ratatui with a trait-based visualization plugin system.

**Tech Stack:** Rust, ratatui, crossterm, rustfft, ringbuf, objc2/block2 (Core Audio FFI), serde/toml, dirs

**Design doc:** `docs/plans/2026-03-07-terminal-vibes-design.md`

---

### Task 1: Project Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/config.rs` (empty module)
- Create: `src/audio.rs` (empty module)
- Create: `src/processing.rs` (empty module)
- Create: `src/ui.rs` (empty module)
- Create: `src/visualizations/mod.rs` (empty module)

**Step 1: Create Cargo.toml with all dependencies**

```toml
[package]
name = "terminal-vibes"
version = "0.1.0"
edition = "2021"
description = "Terminal-based music visualizer for macOS system audio"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
rustfft = "6"
ringbuf = "0.4"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "6"
log = "0.4"
env_logger = "0.11"
clap = { version = "4", features = ["derive"] }
anyhow = "1"

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = ["all"] }
block2 = "0.6"

[dev-dependencies]
approx = "0.5"
```

**Step 2: Create src/main.rs with minimal entry point**

```rust
use anyhow::Result;

mod config;
mod audio;
mod processing;
mod ui;
mod visualizations;

fn main() -> Result<()> {
    env_logger::init();
    println!("terminal-vibes");
    Ok(())
}
```

**Step 3: Create src/lib.rs re-exporting modules**

```rust
pub mod config;
pub mod audio;
pub mod processing;
pub mod ui;
pub mod visualizations;
```

**Step 4: Create empty module files**

Each of these files should just be an empty file (or contain a single comment):

- `src/config.rs` — empty
- `src/audio.rs` — empty
- `src/processing.rs` — empty
- `src/ui.rs` — empty
- `src/visualizations/mod.rs` — empty

**Step 5: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no errors (warnings about unused modules are fine).

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: scaffold project structure with dependencies"
```

---

### Task 2: Configuration Types and Parsing

**Files:**
- Create: `src/config.rs`
- Create: `tests/config_test.rs`

**Step 1: Write tests for config defaults and TOML parsing**

Create `tests/config_test.rs`:

```rust
use terminal_vibes::config::Config;

#[test]
fn test_default_config_has_sensible_values() {
    let config = Config::default();
    assert_eq!(config.audio.fft_size, 2048);
    assert!((config.audio.smoothing - 0.7).abs() < f64::EPSILON);
    assert_eq!(config.audio.buffer_size, 4096);
    assert_eq!(config.display.fps, 30);
    assert!(config.display.show_status_bar);
    assert_eq!(config.keybindings.quit, "q");
    assert_eq!(config.keybindings.next_mode, "Tab");
}

#[test]
fn test_parse_partial_toml_uses_defaults_for_missing() {
    let toml_str = r#"
[audio]
fft_size = 4096

[display]
fps = 60
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.audio.fft_size, 4096);
    assert_eq!(config.display.fps, 60);
    // defaults for unspecified fields
    assert!((config.audio.smoothing - 0.7).abs() < f64::EPSILON);
    assert!(config.display.show_status_bar);
    assert_eq!(config.keybindings.quit, "q");
}

#[test]
fn test_parse_full_toml_roundtrip() {
    let toml_str = r#"
[audio]
fft_size = 1024
smoothing = 0.5
buffer_size = 8192

[display]
fps = 60
color_mode = "256"
show_status_bar = false

[keybindings]
next_mode = "n"
prev_mode = "p"
quit = "Escape"
toggle_status = "h"
increase_sensitivity = "="
decrease_sensitivity = "_"
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.audio.fft_size, 1024);
    assert!((config.audio.smoothing - 0.5).abs() < f64::EPSILON);
    assert_eq!(config.audio.buffer_size, 8192);
    assert_eq!(config.display.fps, 60);
    assert_eq!(config.display.color_mode, "256");
    assert!(!config.display.show_status_bar);
    assert_eq!(config.keybindings.quit, "Escape");
}

#[test]
fn test_config_file_path_respects_default() {
    // When no XDG override, should use ~/.config/terminal-vibes/config.toml
    let path = Config::default_path();
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("terminal-vibes"));
    assert!(path_str.ends_with("config.toml"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test config_test`
Expected: FAIL — `Config` type doesn't exist yet.

**Step 3: Implement config types**

Write `src/config.rs`:

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub audio: AudioConfig,
    pub display: DisplayConfig,
    pub keybindings: KeybindingsConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub fft_size: usize,
    pub smoothing: f64,
    pub buffer_size: usize,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub fps: u32,
    pub color_mode: String,
    pub show_status_bar: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub next_mode: String,
    pub prev_mode: String,
    pub quit: String,
    pub toggle_status: String,
    pub increase_sensitivity: String,
    pub decrease_sensitivity: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig::default(),
            display: DisplayConfig::default(),
            keybindings: KeybindingsConfig::default(),
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            smoothing: 0.7,
            buffer_size: 4096,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            color_mode: "truecolor".to_string(),
            show_status_bar: true,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next_mode: "Tab".to_string(),
            prev_mode: "Shift+Tab".to_string(),
            quit: "q".to_string(),
            toggle_status: "s".to_string(),
            increase_sensitivity: "+".to_string(),
            decrease_sensitivity: "-".to_string(),
        }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        config_dir.join("terminal-vibes").join("config.toml")
    }

    pub fn load(path: Option<&PathBuf>) -> anyhow::Result<Self> {
        let config_path = path
            .cloned()
            .unwrap_or_else(Self::default_path);

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            log::info!("No config file found at {:?}, using defaults", config_path);
            Ok(Config::default())
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test config_test`
Expected: All 4 tests PASS.

**Step 5: Commit**

```bash
git add src/config.rs tests/config_test.rs
git commit -m "feat: add configuration types with TOML parsing and defaults"
```

---

### Task 3: FrameData and DSP Processing Pipeline

**Files:**
- Create: `src/processing.rs`
- Create: `tests/processing_test.rs`

**Step 1: Write tests for DSP pipeline with synthetic audio**

Create `tests/processing_test.rs`:

```rust
use terminal_vibes::processing::{FrameData, Processor, ProcessorConfig};
use std::f32::consts::PI;

/// Generate a sine wave at a given frequency and sample rate.
fn sine_wave(freq_hz: f32, sample_rate: f32, num_samples: usize) -> Vec<f32> {
    (0..num_samples)
        .map(|i| (2.0 * PI * freq_hz * i as f32 / sample_rate).sin())
        .collect()
}

#[test]
fn test_frame_data_default_is_empty() {
    let frame = FrameData::default();
    assert!(frame.spectrum.is_empty());
    assert!(frame.waveform.is_empty());
    assert_eq!(frame.peak, 0.0);
    assert_eq!(frame.rms, 0.0);
}

#[test]
fn test_processor_produces_spectrum_from_sine_wave() {
    let sample_rate = 44100.0;
    let fft_size = 2048;
    let config = ProcessorConfig {
        fft_size,
        sample_rate,
        smoothing: 0.0, // no smoothing for test clarity
        num_bands: 32,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    // 440Hz sine wave — should produce a peak in the spectrum
    let samples = sine_wave(440.0, sample_rate, fft_size);
    let frame = processor.process(&samples);

    assert_eq!(frame.spectrum.len(), 32);
    assert_eq!(frame.waveform.len(), fft_size);

    // The spectrum should have at least one non-zero bin
    let max_val = frame.spectrum.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(max_val > 0.0, "Spectrum should have non-zero values for a sine input");
}

#[test]
fn test_processor_peak_and_rms_for_known_signal() {
    let config = ProcessorConfig {
        fft_size: 1024,
        sample_rate: 44100.0,
        smoothing: 0.0,
        num_bands: 16,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    // Constant signal of 0.5 amplitude
    let samples = vec![0.5_f32; 1024];
    let frame = processor.process(&samples);

    approx::assert_abs_diff_eq!(frame.peak, 0.5, epsilon = 0.01);
    approx::assert_abs_diff_eq!(frame.rms, 0.5, epsilon = 0.01);
}

#[test]
fn test_processor_silence_produces_low_spectrum() {
    let config = ProcessorConfig {
        fft_size: 1024,
        sample_rate: 44100.0,
        smoothing: 0.0,
        num_bands: 16,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    let samples = vec![0.0_f32; 1024];
    let frame = processor.process(&samples);

    // All spectrum values should be at or near zero (clamped from dB floor)
    for val in &frame.spectrum {
        assert!(*val <= 0.01, "Silence should produce near-zero spectrum, got {}", val);
    }
    approx::assert_abs_diff_eq!(frame.peak, 0.0, epsilon = 0.001);
}

#[test]
fn test_smoothing_reduces_jitter() {
    let config = ProcessorConfig {
        fft_size: 1024,
        sample_rate: 44100.0,
        smoothing: 0.8, // heavy smoothing
        num_bands: 16,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    // First frame: loud sine
    let loud = sine_wave(440.0, 44100.0, 1024);
    let frame1 = processor.process(&loud);

    // Second frame: silence — with smoothing, values should decay, not drop to zero
    let silence = vec![0.0_f32; 1024];
    let frame2 = processor.process(&silence);

    let max_after_silence = frame2.spectrum.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let max_loud = frame1.spectrum.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    // Smoothed silence should still retain some energy from previous frame
    assert!(max_after_silence > 0.0, "Smoothing should retain some energy");
    assert!(max_after_silence < max_loud, "But less than the loud frame");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test processing_test`
Expected: FAIL — `FrameData`, `Processor`, `ProcessorConfig` don't exist.

**Step 3: Implement the DSP pipeline**

Write `src/processing.rs`:

```rust
use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

#[derive(Debug, Clone, Default)]
pub struct FrameData {
    pub spectrum: Vec<f32>,
    pub waveform: Vec<f32>,
    pub peak: f32,
    pub rms: f32,
}

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub fft_size: usize,
    pub sample_rate: f32,
    pub smoothing: f64,
    pub num_bands: usize,
    pub db_floor: f32,
}

pub struct Processor {
    config: ProcessorConfig,
    planner: FftPlanner<f32>,
    window: Vec<f32>,
    smoothed_spectrum: Vec<f32>,
}

impl Processor {
    pub fn new(config: ProcessorConfig) -> Self {
        let window = hann_window(config.fft_size);
        let smoothed_spectrum = vec![0.0; config.num_bands];
        Self {
            config,
            planner: FftPlanner::new(),
            window,
            smoothed_spectrum,
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> FrameData {
        let n = self.config.fft_size.min(samples.len());

        // Waveform: raw samples
        let waveform = samples[..n].to_vec();

        // Peak and RMS
        let peak = samples[..n]
            .iter()
            .fold(0.0_f32, |acc, &s| acc.max(s.abs()));
        let rms = (samples[..n]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / n as f32)
            .sqrt();

        // Apply window and run FFT
        let mut buffer: Vec<Complex<f32>> = samples[..n]
            .iter()
            .zip(self.window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();

        // Pad to fft_size if needed
        buffer.resize(self.config.fft_size, Complex::new(0.0, 0.0));

        let fft = self.planner.plan_fft_forward(self.config.fft_size);
        fft.process(&mut buffer);

        // Compute magnitudes (only first half — Nyquist)
        let half = self.config.fft_size / 2;
        let magnitudes: Vec<f32> = buffer[..half]
            .iter()
            .map(|c| c.norm() / half as f32)
            .collect();

        // Bin into logarithmic frequency bands
        let spectrum = bin_to_bands(&magnitudes, self.config.num_bands, self.config.db_floor);

        // Apply smoothing
        for (i, val) in spectrum.iter().enumerate() {
            let s = self.config.smoothing as f32;
            self.smoothed_spectrum[i] = self.smoothed_spectrum[i] * s + val * (1.0 - s);
        }

        FrameData {
            spectrum: self.smoothed_spectrum.clone(),
            waveform,
            peak,
            rms,
        }
    }
}

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (size - 1) as f32).cos()))
        .collect()
}

/// Bin linear frequency magnitudes into logarithmically-spaced bands.
/// Output is normalized to 0.0..1.0 range based on dB floor.
fn bin_to_bands(magnitudes: &[f32], num_bands: usize, db_floor: f32) -> Vec<f32> {
    let n = magnitudes.len();
    if n == 0 || num_bands == 0 {
        return vec![0.0; num_bands];
    }

    let mut bands = vec![0.0_f32; num_bands];

    for band in 0..num_bands {
        // Logarithmic bin edges
        let low = ((band as f64 / num_bands as f64).exp2() - 1.0)
            / (2.0_f64.powi(1) - 1.0)
            * n as f64;
        let high = (((band + 1) as f64 / num_bands as f64).exp2() - 1.0)
            / (2.0_f64.powi(1) - 1.0)
            * n as f64;

        let lo = (low as usize).max(0).min(n - 1);
        let hi = (high as usize).max(lo + 1).min(n);

        // Average magnitude in this band
        let avg = if hi > lo {
            magnitudes[lo..hi].iter().sum::<f32>() / (hi - lo) as f32
        } else {
            magnitudes[lo]
        };

        // Convert to dB then normalize to 0.0..1.0
        let db = if avg > 0.0 {
            20.0 * avg.log10()
        } else {
            db_floor
        };

        bands[band] = ((db - db_floor) / -db_floor).clamp(0.0, 1.0);
    }

    bands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window_endpoints_are_zero() {
        let w = hann_window(256);
        assert!(w[0].abs() < 1e-6);
        assert!(w[255].abs() < 1e-6);
    }

    #[test]
    fn test_hann_window_peak_is_one() {
        let w = hann_window(256);
        let mid = w[128];
        approx::assert_abs_diff_eq!(mid, 1.0, epsilon = 0.01);
    }

    #[test]
    fn test_bin_to_bands_silence() {
        let mags = vec![0.0; 512];
        let bands = bin_to_bands(&mags, 16, -60.0);
        for b in &bands {
            assert!(*b <= 0.01);
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test processing_test && cargo test processing::tests`
Expected: All tests PASS.

**Step 5: Commit**

```bash
git add src/processing.rs tests/processing_test.rs
git commit -m "feat: add DSP processing pipeline with FFT, windowing, and band binning"
```

---

### Task 4: Visualization Trait and Registry

**Files:**
- Create: `src/visualizations/mod.rs`
- Create: `src/visualizations/registry.rs`
- Create: `tests/visualizations_test.rs`

**Step 1: Write tests for the trait and registry**

Create `tests/visualizations_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::registry::VisualizationRegistry;
use terminal_vibes::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crossterm::event::KeyEvent;

/// A mock visualization for testing the registry.
struct MockViz {
    name: String,
    updated: bool,
}

impl MockViz {
    fn new(name: &str) -> Self {
        Self { name: name.to_string(), updated: false }
    }
}

impl Visualization for MockViz {
    fn name(&self) -> &str { &self.name }
    fn update(&mut self, _frame: &FrameData) { self.updated = true; }
    fn render(&self, _area: Rect, _buf: &mut Buffer) {}
}

#[test]
fn test_registry_starts_empty() {
    let registry = VisualizationRegistry::new();
    assert_eq!(registry.len(), 0);
    assert!(registry.current().is_none());
}

#[test]
fn test_registry_register_and_access() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("test1")));
    registry.register(Box::new(MockViz::new("test2")));
    assert_eq!(registry.len(), 2);
    assert_eq!(registry.current().unwrap().name(), "test1");
}

#[test]
fn test_registry_cycle_next() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("a")));
    registry.register(Box::new(MockViz::new("b")));
    registry.register(Box::new(MockViz::new("c")));

    assert_eq!(registry.current().unwrap().name(), "a");
    registry.next();
    assert_eq!(registry.current().unwrap().name(), "b");
    registry.next();
    assert_eq!(registry.current().unwrap().name(), "c");
    registry.next();
    // wraps around
    assert_eq!(registry.current().unwrap().name(), "a");
}

#[test]
fn test_registry_cycle_prev() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("a")));
    registry.register(Box::new(MockViz::new("b")));
    registry.register(Box::new(MockViz::new("c")));

    assert_eq!(registry.current().unwrap().name(), "a");
    registry.prev();
    // wraps around to end
    assert_eq!(registry.current().unwrap().name(), "c");
    registry.prev();
    assert_eq!(registry.current().unwrap().name(), "b");
}

#[test]
fn test_registry_update_current() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("test")));

    let frame = FrameData::default();
    registry.update_current(&frame);

    // We can't easily inspect `updated` through the trait, but the call shouldn't panic.
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test visualizations_test`
Expected: FAIL — `Visualization` trait and `VisualizationRegistry` don't exist.

**Step 3: Implement the trait and registry**

Write `src/visualizations/mod.rs`:

```rust
pub mod registry;

use crate::processing::FrameData;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub trait Visualization: Send {
    /// Unique name shown in status bar and config.
    fn name(&self) -> &str;

    /// Process a new frame of audio data (update internal state).
    fn update(&mut self, frame: &FrameData);

    /// Render into the given ratatui buffer area.
    fn render(&self, area: Rect, buf: &mut Buffer);

    /// Handle a keypress specific to this visualization. Returns true if handled.
    fn on_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    /// Provide default config for this visualization.
    fn default_config(&self) -> toml::Value {
        toml::Value::Table(Default::default())
    }

    /// Apply config values.
    fn apply_config(&mut self, _config: &toml::Value) {}
}
```

Write `src/visualizations/registry.rs`:

```rust
use super::Visualization;
use crate::processing::FrameData;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub struct VisualizationRegistry {
    plugins: Vec<Box<dyn Visualization>>,
    current_index: usize,
}

impl VisualizationRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            current_index: 0,
        }
    }

    pub fn register(&mut self, viz: Box<dyn Visualization>) {
        self.plugins.push(viz);
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn current(&self) -> Option<&dyn Visualization> {
        self.plugins.get(self.current_index).map(|v| v.as_ref())
    }

    pub fn current_mut(&mut self) -> Option<&mut dyn Visualization> {
        self.plugins.get_mut(self.current_index).map(|v| v.as_mut())
    }

    pub fn next(&mut self) {
        if !self.plugins.is_empty() {
            self.current_index = (self.current_index + 1) % self.plugins.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.plugins.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.plugins.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    pub fn update_current(&mut self, frame: &FrameData) {
        if let Some(viz) = self.current_mut() {
            viz.update(frame);
        }
    }

    pub fn render_current(&self, area: Rect, buf: &mut Buffer) {
        if let Some(viz) = self.current() {
            viz.render(area, buf);
        }
    }

    pub fn on_key_current(&mut self, key: KeyEvent) -> bool {
        if let Some(viz) = self.current_mut() {
            viz.on_key(key)
        } else {
            false
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test visualizations_test`
Expected: All 5 tests PASS.

**Step 5: Commit**

```bash
git add src/visualizations/ tests/visualizations_test.rs
git commit -m "feat: add Visualization trait and VisualizationRegistry"
```

---

### Task 5: Spectrum Bars Visualization

**Files:**
- Create: `src/visualizations/spectrum.rs`
- Modify: `src/visualizations/mod.rs` — add `pub mod spectrum;`
- Create: `tests/spectrum_test.rs`

**Step 1: Write tests for spectrum bars**

Create `tests/spectrum_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::spectrum::SpectrumBars;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_spectrum_bars_name() {
    let viz = SpectrumBars::new();
    assert_eq!(viz.name(), "spectrum");
}

#[test]
fn test_spectrum_bars_update_stores_spectrum() {
    let mut viz = SpectrumBars::new();
    let frame = FrameData {
        spectrum: vec![0.5, 0.8, 0.3, 0.9],
        waveform: vec![],
        peak: 0.9,
        rms: 0.5,
    };
    viz.update(&frame);
    // After update, render should not panic
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrum_bars_render_empty_no_panic() {
    let viz = SpectrumBars::new();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrum_bars_render_zero_area_no_panic() {
    let mut viz = SpectrumBars::new();
    let frame = FrameData {
        spectrum: vec![0.5; 16],
        waveform: vec![],
        peak: 0.5,
        rms: 0.3,
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test spectrum_test`
Expected: FAIL — `SpectrumBars` doesn't exist.

**Step 3: Implement SpectrumBars**

Write `src/visualizations/spectrum.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub struct SpectrumBars {
    spectrum: Vec<f32>,
    bar_width: u16,
    colors: Vec<Color>,
}

impl SpectrumBars {
    pub fn new() -> Self {
        Self {
            spectrum: Vec::new(),
            bar_width: 2,
            colors: default_gradient(),
        }
    }
}

impl Visualization for SpectrumBars {
    fn name(&self) -> &str {
        "spectrum"
    }

    fn update(&mut self, frame: &FrameData) {
        self.spectrum = frame.spectrum.clone();
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.spectrum.is_empty() {
            return;
        }

        let num_bars = (area.width / self.bar_width.max(1)) as usize;
        let band_count = self.spectrum.len();

        for i in 0..num_bars.min(band_count) {
            let value = self.spectrum[i * band_count / num_bars.max(1)].clamp(0.0, 1.0);
            let bar_height = (value * area.height as f32) as u16;
            let x = area.x + (i as u16) * self.bar_width;

            let color_idx = (i * self.colors.len()) / num_bars.max(1);
            let color = self.colors[color_idx.min(self.colors.len() - 1)];

            for dy in 0..bar_height {
                let y = area.y + area.height - 1 - dy;
                for dx in 0..self.bar_width.min(area.width - (x - area.x)) {
                    if x + dx < area.x + area.width && y >= area.y {
                        buf[(x + dx, y)]
                            .set_char('\u{2588}')
                            .set_fg(color);
                    }
                }
            }
        }
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(bw) = config.get("bar_width").and_then(|v| v.as_integer()) {
            self.bar_width = (bw as u16).max(1);
        }
    }
}

fn default_gradient() -> Vec<Color> {
    vec![
        Color::from_u32(0x00ff0055), // red-pink (bass)
        Color::from_u32(0x00ffaa00), // orange
        Color::from_u32(0x0000ffaa), // green-cyan
        Color::from_u32(0x000055ff), // blue (treble)
    ]
}
```

Update `src/visualizations/mod.rs` — add `pub mod spectrum;` to the top.

**Step 4: Run tests to verify they pass**

Run: `cargo test --test spectrum_test`
Expected: All 4 tests PASS.

**Step 5: Commit**

```bash
git add src/visualizations/ tests/spectrum_test.rs
git commit -m "feat: add SpectrumBars visualization plugin"
```

---

### Task 6: Waveform Visualization

**Files:**
- Create: `src/visualizations/waveform.rs`
- Modify: `src/visualizations/mod.rs` — add `pub mod waveform;`
- Create: `tests/waveform_test.rs`

**Step 1: Write tests for waveform**

Create `tests/waveform_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::waveform::Waveform;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_waveform_name() {
    let viz = Waveform::new();
    assert_eq!(viz.name(), "waveform");
}

#[test]
fn test_waveform_update_stores_samples() {
    let mut viz = Waveform::new();
    let frame = FrameData {
        spectrum: vec![],
        waveform: vec![0.0, 0.5, 1.0, 0.5, 0.0, -0.5, -1.0, -0.5],
        peak: 1.0,
        rms: 0.5,
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_waveform_render_empty_no_panic() {
    let viz = Waveform::new();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_waveform_render_zero_area_no_panic() {
    let viz = Waveform::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test waveform_test`
Expected: FAIL — `Waveform` doesn't exist.

**Step 3: Implement Waveform**

Write `src/visualizations/waveform.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub struct Waveform {
    samples: Vec<f32>,
    color: Color,
}

impl Waveform {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            color: Color::from_u32(0x0000ff88),
        }
    }
}

impl Visualization for Waveform {
    fn name(&self) -> &str {
        "waveform"
    }

    fn update(&mut self, frame: &FrameData) {
        self.samples = frame.waveform.clone();
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.samples.is_empty() {
            return;
        }

        let mid_y = area.y + area.height / 2;

        for x in 0..area.width {
            // Map terminal column to sample index
            let sample_idx = (x as usize * self.samples.len()) / area.width as usize;
            let sample = self.samples[sample_idx.min(self.samples.len() - 1)];

            // Map sample (-1.0..1.0) to y position
            let half_h = area.height as f32 / 2.0;
            let y_offset = (-sample * half_h) as i16;
            let y = (mid_y as i16 + y_offset).clamp(area.y as i16, (area.y + area.height - 1) as i16) as u16;

            buf[(area.x + x, y)]
                .set_char('\u{2022}') // bullet dot
                .set_fg(self.color);

            // Draw a vertical line from mid to point for thickness
            let (y_start, y_end) = if y < mid_y { (y, mid_y) } else { (mid_y, y) };
            for fill_y in y_start..=y_end {
                if fill_y >= area.y && fill_y < area.y + area.height {
                    buf[(area.x + x, fill_y)]
                        .set_char('\u{2502}') // thin vertical line
                        .set_fg(self.color);
                }
            }
            // Overwrite the point itself with a solid dot
            buf[(area.x + x, y)]
                .set_char('\u{2022}')
                .set_fg(self.color);
        }
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(color_str) = config.get("color").and_then(|v| v.as_str()) {
            if let Some(c) = parse_hex_color(color_str) {
                self.color = c;
            }
        }
    }
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
```

Update `src/visualizations/mod.rs` — add `pub mod waveform;`.

**Step 4: Run tests to verify they pass**

Run: `cargo test --test waveform_test`
Expected: All 4 tests PASS.

**Step 5: Commit**

```bash
git add src/visualizations/ tests/waveform_test.rs
git commit -m "feat: add Waveform visualization plugin"
```

---

### Task 7: Spectrogram Visualization

**Files:**
- Create: `src/visualizations/spectrogram.rs`
- Modify: `src/visualizations/mod.rs` — add `pub mod spectrogram;`
- Create: `tests/spectrogram_test.rs`

**Step 1: Write tests for spectrogram**

Create `tests/spectrogram_test.rs`:

```rust
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::spectrogram::Spectrogram;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_spectrogram_name() {
    let viz = Spectrogram::new(200);
    assert_eq!(viz.name(), "spectrogram");
}

#[test]
fn test_spectrogram_accumulates_history() {
    let mut viz = Spectrogram::new(5);

    for i in 0..7 {
        let frame = FrameData {
            spectrum: vec![i as f32 * 0.1; 8],
            waveform: vec![],
            peak: 0.5,
            rms: 0.3,
        };
        viz.update(&frame);
    }

    // Render should not panic even with history wrapping
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrogram_render_empty_no_panic() {
    let viz = Spectrogram::new(200);
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrogram_render_zero_area_no_panic() {
    let viz = Spectrogram::new(200);
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test spectrogram_test`
Expected: FAIL — `Spectrogram` doesn't exist.

**Step 3: Implement Spectrogram**

Write `src/visualizations/spectrogram.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::VecDeque;

pub struct Spectrogram {
    history: VecDeque<Vec<f32>>,
    max_history: usize,
}

impl Spectrogram {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }
}

impl Visualization for Spectrogram {
    fn name(&self) -> &str {
        "spectrogram"
    }

    fn update(&mut self, frame: &FrameData) {
        self.history.push_back(frame.spectrum.clone());
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.history.is_empty() {
            return;
        }

        let cols = area.width as usize;
        let rows = area.height as usize;

        // Show the most recent `cols` frames (time flows left to right)
        let visible_history: Vec<&Vec<f32>> = self.history
            .iter()
            .rev()
            .take(cols)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let x_offset = if visible_history.len() < cols {
            cols - visible_history.len()
        } else {
            0
        };

        for (col_idx, spectrum) in visible_history.iter().enumerate() {
            let x = area.x + (x_offset + col_idx) as u16;
            if x >= area.x + area.width {
                continue;
            }

            for row in 0..rows {
                // Map row to frequency band (bottom = low freq, top = high freq)
                let band_idx = ((rows - 1 - row) * spectrum.len()) / rows.max(1);
                let band_idx = band_idx.min(spectrum.len().saturating_sub(1));
                let intensity = if spectrum.is_empty() {
                    0.0
                } else {
                    spectrum[band_idx].clamp(0.0, 1.0)
                };

                let y = area.y + row as u16;
                let color = magma_colormap(intensity);

                buf[(x, y)]
                    .set_char(intensity_char(intensity))
                    .set_fg(color);
            }
        }
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(len) = config.get("history_length").and_then(|v| v.as_integer()) {
            self.max_history = len as usize;
        }
    }
}

/// Map intensity (0.0..1.0) to a block character of varying density.
fn intensity_char(intensity: f32) -> char {
    match (intensity * 4.0) as u8 {
        0 => ' ',
        1 => '\u{2591}', // light shade
        2 => '\u{2592}', // medium shade
        3 => '\u{2593}', // dark shade
        _ => '\u{2588}', // full block
    }
}

/// Simple magma-ish colormap: black -> purple -> orange -> yellow.
fn magma_colormap(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.33 {
        let s = t / 0.33;
        (
            (s * 120.0) as u8,
            0u8,
            (s * 150.0) as u8,
        )
    } else if t < 0.66 {
        let s = (t - 0.33) / 0.33;
        (
            120 + (s * 135.0) as u8,
            (s * 80.0) as u8,
            150 - (s * 150.0) as u8,
        )
    } else {
        let s = (t - 0.66) / 0.34;
        (
            255,
            80 + (s * 175.0) as u8,
            (s * 80.0) as u8,
        )
    };
    Color::Rgb(r, g, b)
}
```

Update `src/visualizations/mod.rs` — add `pub mod spectrogram;`.

**Step 4: Run tests to verify they pass**

Run: `cargo test --test spectrogram_test`
Expected: All 4 tests PASS.

**Step 5: Commit**

```bash
git add src/visualizations/ tests/spectrogram_test.rs
git commit -m "feat: add Spectrogram visualization plugin"
```

---

### Task 8: TUI App Shell (Render Loop + Input Handling)

**Files:**
- Create: `src/ui.rs`
- Modify: `src/main.rs` — wire up the app

**Step 1: Implement the App struct and render loop**

Write `src/ui.rs`:

```rust
use crate::config::Config;
use crate::processing::FrameData;
use crate::visualizations::registry::VisualizationRegistry;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct App {
    registry: VisualizationRegistry,
    config: Config,
    running: Arc<AtomicBool>,
    sensitivity: f32,
}

impl App {
    pub fn new(registry: VisualizationRegistry, config: Config, running: Arc<AtomicBool>) -> Self {
        Self {
            registry,
            config,
            running,
            sensitivity: 1.0,
        }
    }

    pub fn run(&mut self, frame_rx: Receiver<FrameData>) -> Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

        let frame_duration = Duration::from_millis(1000 / self.config.display.fps.max(1) as u64);
        let mut last_frame = FrameData::default();

        while self.running.load(Ordering::Relaxed) {
            let loop_start = Instant::now();

            // Drain the channel, keep latest frame
            while let Ok(frame) = frame_rx.try_recv() {
                last_frame = frame;
            }

            // Scale spectrum by sensitivity
            let mut display_frame = last_frame.clone();
            for val in &mut display_frame.spectrum {
                *val = (*val * self.sensitivity).clamp(0.0, 1.0);
            }

            self.registry.update_current(&display_frame);

            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(if self.config.display.show_status_bar {
                        vec![Constraint::Min(1), Constraint::Length(1)]
                    } else {
                        vec![Constraint::Min(1)]
                    })
                    .split(f.area());

                // Main visualization area
                let viz_area = chunks[0];
                self.registry.render_current(viz_area, f.buffer_mut());

                // Status bar
                if self.config.display.show_status_bar && chunks.len() > 1 {
                    let mode_name = self.registry
                        .current()
                        .map(|v| v.name())
                        .unwrap_or("none");
                    let status = format!(
                        " [{}]  peak: {:.2}  rms: {:.2}  sens: {:.1}x  |  Tab: next  q: quit ",
                        mode_name,
                        display_frame.peak,
                        display_frame.rms,
                        self.sensitivity,
                    );
                    let status_bar = Paragraph::new(status)
                        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
                    f.render_widget(status_bar, chunks[1]);
                }
            })?;

            // Handle input
            let poll_timeout = frame_duration.saturating_sub(loop_start.elapsed());
            if event::poll(poll_timeout)? {
                if let Event::Key(key) = event::read()? {
                    if !self.handle_key(key) {
                        // Forward to current visualization
                        self.registry.on_key_current(key);
                    }
                }
            }
        }

        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => {
                self.running.store(false, Ordering::Relaxed);
                true
            }
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.registry.prev();
                } else {
                    self.registry.next();
                }
                true
            }
            KeyCode::BackTab => {
                self.registry.prev();
                true
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.sensitivity = (self.sensitivity + 0.1).min(5.0);
                true
            }
            KeyCode::Char('-') => {
                self.sensitivity = (self.sensitivity - 0.1).max(0.1);
                true
            }
            KeyCode::Char('s') => {
                self.config.display.show_status_bar = !self.config.display.show_status_bar;
                true
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running.store(false, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add src/ui.rs
git commit -m "feat: add TUI app shell with render loop, input handling, and status bar"
```

---

### Task 9: Core Audio AudioProcessTap FFI

**Files:**
- Create: `src/audio.rs`
- Create: `src/audio/tap.rs`

This is the boss fight. `AudioProcessTap` is a macOS 15+ C API from CoreAudio. We need to:
1. Define FFI bindings for `CATapDescription` and `AudioHardwareCreateProcessTap`
2. Create the tap and get an `AudioDeviceID` for the aggregate device
3. Set up an `AudioUnit` (AUHAL) with a render callback to receive PCM samples
4. Write samples into the ring buffer from the callback

**Step 1: Create the audio module with FFI bindings**

Restructure `src/audio.rs` into a module directory. Create `src/audio/mod.rs`:

```rust
mod tap;

pub use tap::AudioTap;

use ringbuf::HeapRb;
use std::sync::Arc;

pub type AudioRingBuffer = HeapRb<f32>;

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: f32,
    pub buffer_size: usize,
    pub channels: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            buffer_size: 4096,
            channels: 2,
        }
    }
}
```

**Step 2: Implement the AudioProcessTap wrapper**

Create `src/audio/tap.rs`:

```rust
use super::AudioConfig;
use anyhow::{anyhow, Result};
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Core Audio FFI types and constants
#[allow(non_camel_case_types)]
mod ffi {
    use std::os::raw::c_void;

    pub type OSStatus = i32;
    pub type AudioObjectID = u32;
    pub type AudioDeviceID = AudioObjectID;
    pub type UInt32 = u32;
    pub type Float64 = f64;

    pub const kAudioObjectSystemObject: AudioObjectID = 1;

    // AudioObjectPropertyAddress
    #[repr(C)]
    pub struct AudioObjectPropertyAddress {
        pub selector: u32,
        pub scope: u32,
        pub element: u32,
    }

    // Property selectors
    pub const kAudioHardwarePropertyProcessTapList: u32 = u32::from_be_bytes(*b"tps#");
    pub const kAudioHardwarePropertyTapList: u32 = u32::from_be_bytes(*b"tps#");

    // Scopes
    pub const kAudioObjectPropertyScopeGlobal: u32 = u32::from_be_bytes(*b"glob");
    pub const kAudioObjectPropertyScopeInput: u32 = u32::from_be_bytes(*b"inpt");
    pub const kAudioObjectPropertyScopeOutput: u32 = u32::from_be_bytes(*b"outp");
    pub const kAudioObjectPropertyElementMain: u32 = 0;

    // Device properties
    pub const kAudioDevicePropertyStreamConfiguration: u32 = u32::from_be_bytes(*b"slay");
    pub const kAudioDevicePropertyNominalSampleRate: u32 = u32::from_be_bytes(*b"nsrt");

    // AudioUnit types
    pub type AudioUnit = *mut c_void;
    pub type AudioComponentInstance = AudioUnit;
    pub type AudioComponent = *mut c_void;

    #[repr(C)]
    pub struct AudioComponentDescription {
        pub component_type: u32,
        pub component_sub_type: u32,
        pub component_manufacturer: u32,
        pub component_flags: u32,
        pub component_flags_mask: u32,
    }

    pub const kAudioUnitType_Output: u32 = u32::from_be_bytes(*b"auou");
    pub const kAudioUnitSubType_HALOutput: u32 = u32::from_be_bytes(*b"ahal");
    pub const kAudioUnitManufacturer_Apple: u32 = u32::from_be_bytes(*b"appl");

    // AudioUnit properties
    pub const kAudioOutputUnitProperty_EnableIO: u32 = 2003;
    pub const kAudioOutputUnitProperty_CurrentDevice: u32 = 2000;
    pub const kAudioUnitProperty_StreamFormat: u32 = 8;
    pub const kAudioUnitProperty_SetRenderCallback: u32 = 23;

    pub const kAudioUnitScope_Input: u32 = 1;
    pub const kAudioUnitScope_Output: u32 = 0;
    pub const kAudioUnitScope_Global: u32 = 0;

    #[repr(C)]
    pub struct AudioStreamBasicDescription {
        pub sample_rate: Float64,
        pub format_id: u32,
        pub format_flags: u32,
        pub bytes_per_packet: u32,
        pub frames_per_packet: u32,
        pub bytes_per_frame: u32,
        pub channels_per_frame: u32,
        pub bits_per_channel: u32,
        pub reserved: u32,
    }

    pub const kAudioFormatLinearPCM: u32 = u32::from_be_bytes(*b"lpcm");
    pub const kAudioFormatFlagIsFloat: u32 = 1 << 0;
    pub const kAudioFormatFlagIsPacked: u32 = 1 << 3;
    pub const kAudioFormatFlagIsNonInterleaved: u32 = 1 << 5;

    #[repr(C)]
    pub struct AudioBufferList {
        pub number_buffers: u32,
        pub buffers: [AudioBuffer; 1], // variable-length array
    }

    #[repr(C)]
    pub struct AudioBuffer {
        pub number_channels: u32,
        pub data_byte_size: u32,
        pub data: *mut c_void,
    }

    #[repr(C)]
    pub struct AURenderCallbackStruct {
        pub input_proc: unsafe extern "C" fn(
            in_ref_con: *mut c_void,
            io_action_flags: *mut u32,
            in_time_stamp: *const AudioTimeStamp,
            in_bus_number: u32,
            in_number_frames: u32,
            io_data: *mut AudioBufferList,
        ) -> OSStatus,
        pub input_proc_ref_con: *mut c_void,
    }

    #[repr(C)]
    pub struct AudioTimeStamp {
        pub sample_time: f64,
        pub host_time: u64,
        pub rate_scalar: f64,
        pub word_clock_time: u64,
        pub smpte_time: [u8; 24], // SMPTETime struct, opaque here
        pub flags: u32,
        pub reserved: u32,
    }

    // CATapDescription for macOS 15+
    // This is an Objective-C class. We interact via objc runtime.

    extern "C" {
        pub fn AudioObjectGetPropertyDataSize(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            out_data_size: *mut u32,
        ) -> OSStatus;

        pub fn AudioObjectGetPropertyData(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            io_data_size: *mut u32,
            out_data: *mut c_void,
        ) -> OSStatus;

        pub fn AudioObjectSetPropertyData(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            data_size: u32,
            data: *const c_void,
        ) -> OSStatus;

        pub fn AudioComponentFindNext(
            component: AudioComponent,
            description: *const AudioComponentDescription,
        ) -> AudioComponent;

        pub fn AudioComponentInstanceNew(
            component: AudioComponent,
            out_instance: *mut AudioComponentInstance,
        ) -> OSStatus;

        pub fn AudioComponentInstanceDispose(instance: AudioComponentInstance) -> OSStatus;

        pub fn AudioUnitSetProperty(
            unit: AudioUnit,
            property_id: u32,
            scope: u32,
            element: u32,
            data: *const c_void,
            data_size: u32,
        ) -> OSStatus;

        pub fn AudioUnitGetProperty(
            unit: AudioUnit,
            property_id: u32,
            scope: u32,
            element: u32,
            data: *mut c_void,
            data_size: *mut u32,
        ) -> OSStatus;

        pub fn AudioUnitInitialize(unit: AudioUnit) -> OSStatus;
        pub fn AudioUnitUninitialize(unit: AudioUnit) -> OSStatus;
        pub fn AudioOutputUnitStart(unit: AudioUnit) -> OSStatus;
        pub fn AudioOutputUnitStop(unit: AudioUnit) -> OSStatus;

        pub fn AudioUnitRender(
            unit: AudioUnit,
            io_action_flags: *mut u32,
            in_time_stamp: *const AudioTimeStamp,
            in_output_bus_number: u32,
            in_number_frames: u32,
            io_data: *mut AudioBufferList,
        ) -> OSStatus;

        // macOS 15+ AudioHardwareCreateProcessTap
        pub fn AudioHardwareCreateProcessTap(
            tap_description: *const c_void, // CATapDescription*
            out_tap_id: *mut AudioObjectID,
        ) -> OSStatus;

        pub fn AudioHardwareDestroyProcessTap(tap_id: AudioObjectID) -> OSStatus;
    }
}

/// Context passed to the audio render callback.
struct CallbackContext {
    producer: HeapProd<f32>,
    channels: u32,
}

/// The audio render callback. Called on the real-time audio thread.
/// MUST NOT allocate, lock, or block.
unsafe extern "C" fn render_callback(
    in_ref_con: *mut std::os::raw::c_void,
    _io_action_flags: *mut u32,
    _in_time_stamp: *const ffi::AudioTimeStamp,
    _in_bus_number: u32,
    in_number_frames: u32,
    io_data: *mut ffi::AudioBufferList,
) -> ffi::OSStatus {
    let ctx = &mut *(in_ref_con as *mut CallbackContext);

    if io_data.is_null() {
        return 0;
    }

    let buffer_list = &*io_data;
    if buffer_list.number_buffers == 0 {
        return 0;
    }

    // Read from first buffer (interleaved or mono)
    let buffer = &buffer_list.buffers[0];
    let num_samples = buffer.data_byte_size as usize / std::mem::size_of::<f32>();

    if !buffer.data.is_null() && num_samples > 0 {
        let samples = std::slice::from_raw_parts(buffer.data as *const f32, num_samples);

        // Downmix to mono if stereo
        if ctx.channels >= 2 {
            for chunk in samples.chunks(ctx.channels as usize) {
                let mono = chunk.iter().sum::<f32>() / ctx.channels as f32;
                let _ = ctx.producer.try_push(mono);
            }
        } else {
            for &sample in samples {
                let _ = ctx.producer.try_push(sample);
            }
        }
    }

    0
}

pub struct AudioTap {
    audio_unit: ffi::AudioUnit,
    tap_id: Option<ffi::AudioObjectID>,
    _callback_context: Box<CallbackContext>,
    config: AudioConfig,
}

// Safety: AudioUnit is accessed only from this struct's methods and the callback.
// The callback context is pinned in a Box and referenced by pointer.
unsafe impl Send for AudioTap {}

impl AudioTap {
    /// Create and start an audio tap on system audio output.
    pub fn new(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        unsafe { Self::create_tap(producer, config) }
    }

    unsafe fn create_tap(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        // 1. Create CATapDescription via Objective-C runtime
        use objc2::runtime::{AnyClass, AnyObject, Sel, Bool};
        use objc2::msg_send;

        let tap_desc_class = AnyClass::get(c"CATapDescription")
            .ok_or_else(|| anyhow!(
                "CATapDescription class not found. Requires macOS 15+."
            ))?;

        // +[CATapDescription alloc] then -[CATapDescription initStereoGlobalTapButExcludeProcesses:]
        let tap_desc: *mut AnyObject = msg_send![tap_desc_class, alloc];
        // Create an empty NSArray for the exclusion list
        let nsarray_class = AnyClass::get(c"NSArray")
            .ok_or_else(|| anyhow!("NSArray class not found"))?;
        let empty_array: *mut AnyObject = msg_send![nsarray_class, array];
        let tap_desc: *mut AnyObject = msg_send![tap_desc, initStereoGlobalTapButExcludeProcesses: empty_array];

        if tap_desc.is_null() {
            return Err(anyhow!("Failed to create CATapDescription"));
        }

        // 2. Create the process tap
        let mut tap_id: ffi::AudioObjectID = 0;
        let status = ffi::AudioHardwareCreateProcessTap(
            tap_desc as *const _,
            &mut tap_id,
        );
        if status != 0 {
            return Err(anyhow!(
                "AudioHardwareCreateProcessTap failed with status {}. \
                 Make sure you've granted audio capture permissions.",
                status
            ));
        }

        // 3. Create AUHAL AudioUnit
        let desc = ffi::AudioComponentDescription {
            component_type: ffi::kAudioUnitType_Output,
            component_sub_type: ffi::kAudioUnitSubType_HALOutput,
            component_manufacturer: ffi::kAudioUnitManufacturer_Apple,
            component_flags: 0,
            component_flags_mask: 0,
        };

        let component = ffi::AudioComponentFindNext(std::ptr::null_mut(), &desc);
        if component.is_null() {
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("Could not find HAL output AudioComponent"));
        }

        let mut audio_unit: ffi::AudioUnit = std::ptr::null_mut();
        let status = ffi::AudioComponentInstanceNew(component, &mut audio_unit);
        if status != 0 {
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("AudioComponentInstanceNew failed: {}", status));
        }

        // 4. Enable input on the AUHAL (bus 1) and disable output (bus 0)
        let enable: u32 = 1;
        let disable: u32 = 0;
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioOutputUnitProperty_EnableIO,
            ffi::kAudioUnitScope_Input,
            1, // input bus
            &enable as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioOutputUnitProperty_EnableIO,
            ffi::kAudioUnitScope_Output,
            0, // output bus
            &disable as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // 5. Set the tap's aggregate device as the input device
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioOutputUnitProperty_CurrentDevice,
            ffi::kAudioUnitScope_Global,
            0,
            &tap_id as *const _ as *const _,
            std::mem::size_of::<ffi::AudioDeviceID>() as u32,
        );

        // 6. Set stream format to float32, interleaved
        let format = ffi::AudioStreamBasicDescription {
            sample_rate: config.sample_rate as f64,
            format_id: ffi::kAudioFormatLinearPCM,
            format_flags: ffi::kAudioFormatFlagIsFloat | ffi::kAudioFormatFlagIsPacked,
            bytes_per_packet: 4 * config.channels,
            frames_per_packet: 1,
            bytes_per_frame: 4 * config.channels,
            channels_per_frame: config.channels,
            bits_per_channel: 32,
            reserved: 0,
        };
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioUnitProperty_StreamFormat,
            ffi::kAudioUnitScope_Output, // output scope of input bus
            1,
            &format as *const _ as *const _,
            std::mem::size_of::<ffi::AudioStreamBasicDescription>() as u32,
        );

        // 7. Set render callback
        let callback_context = Box::new(CallbackContext {
            producer,
            channels: config.channels,
        });
        let callback_struct = ffi::AURenderCallbackStruct {
            input_proc: render_callback,
            input_proc_ref_con: &*callback_context as *const _ as *mut _,
        };
        let status = ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioUnitProperty_SetRenderCallback,
            ffi::kAudioUnitScope_Output,
            1, // input bus
            &callback_struct as *const _ as *const _,
            std::mem::size_of::<ffi::AURenderCallbackStruct>() as u32,
        );
        if status != 0 {
            ffi::AudioComponentInstanceDispose(audio_unit);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("Failed to set render callback: {}", status));
        }

        // 8. Initialize and start
        let status = ffi::AudioUnitInitialize(audio_unit);
        if status != 0 {
            ffi::AudioComponentInstanceDispose(audio_unit);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("AudioUnitInitialize failed: {}", status));
        }

        let status = ffi::AudioOutputUnitStart(audio_unit);
        if status != 0 {
            ffi::AudioUnitUninitialize(audio_unit);
            ffi::AudioComponentInstanceDispose(audio_unit);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("AudioOutputUnitStart failed: {}", status));
        }

        log::info!("Audio tap started (tap_id={}, sample_rate={})", tap_id, config.sample_rate);

        Ok(Self {
            audio_unit,
            tap_id: Some(tap_id),
            _callback_context: callback_context,
            config,
        })
    }

    pub fn config(&self) -> &AudioConfig {
        &self.config
    }
}

impl Drop for AudioTap {
    fn drop(&mut self) {
        unsafe {
            ffi::AudioOutputUnitStop(self.audio_unit);
            ffi::AudioUnitUninitialize(self.audio_unit);
            ffi::AudioComponentInstanceDispose(self.audio_unit);
            if let Some(tap_id) = self.tap_id {
                ffi::AudioHardwareDestroyProcessTap(tap_id);
                log::info!("Audio tap destroyed (tap_id={})", tap_id);
            }
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles (may have warnings). Note: this cannot be meaningfully unit tested without actual audio hardware — integration testing happens in Task 10.

**Step 3: Commit**

```bash
git add src/audio/ src/audio.rs
git commit -m "feat: add Core Audio AudioProcessTap FFI bindings and AudioTap wrapper"
```

---

### Task 10: Wire Everything Together in main.rs

**Files:**
- Modify: `src/main.rs`

**Step 1: Implement the full main function**

Write `src/main.rs`:

```rust
use anyhow::{Context, Result};
use clap::Parser;
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Split};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod audio;
mod config;
mod processing;
mod ui;
mod visualizations;

use audio::{AudioConfig, AudioTap};
use config::Config;
use processing::{FrameData, Processor, ProcessorConfig};
use ui::App;
use visualizations::registry::VisualizationRegistry;
use visualizations::spectrogram::Spectrogram;
use visualizations::spectrum::SpectrumBars;
use visualizations::waveform::Waveform;

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
        println!("  spectrum   - Frequency spectrum bars");
        println!("  waveform   - Oscilloscope waveform");
        println!("  spectrogram - Scrolling frequency heatmap");
        return Ok(());
    }

    let config = Config::load(cli.config.as_ref())
        .context("Failed to load config")?;

    // Set up ring buffer
    let rb = HeapRb::<f32>::new(config.audio.buffer_size);
    let (producer, mut consumer) = rb.split();

    // Set up audio tap
    let audio_config = AudioConfig {
        sample_rate: 44100.0,
        buffer_size: config.audio.buffer_size,
        channels: 2,
    };
    let _audio_tap = AudioTap::new(producer, audio_config.clone())
        .context("Failed to start audio capture. Make sure you're on macOS 15+ and have granted audio permissions.")?;

    // Set up visualization registry
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(SpectrumBars::new()));
    registry.register(Box::new(Waveform::new()));
    registry.register(Box::new(Spectrogram::new(200)));

    // Set up processing -> UI channel
    let (frame_tx, frame_rx) = mpsc::sync_channel::<FrameData>(2);

    let running = Arc::new(AtomicBool::new(true));
    let running_processor = running.clone();

    // Spawn processor thread
    let fft_size = config.audio.fft_size;
    let smoothing = config.audio.smoothing;
    let processor_handle = thread::spawn(move || {
        let mut processor = Processor::new(ProcessorConfig {
            fft_size,
            sample_rate: audio_config.sample_rate,
            smoothing,
            num_bands: 64,
            db_floor: -60.0,
        });

        let mut sample_buf = vec![0.0_f32; fft_size];
        let interval = Duration::from_millis(1000 / 60); // ~60 Hz

        while running_processor.load(Ordering::Relaxed) {
            // Read available samples from ring buffer
            let count = consumer.pop_slice(&mut sample_buf);
            if count > 0 {
                let frame = processor.process(&sample_buf[..count.max(fft_size).min(sample_buf.len())]);
                let _ = frame_tx.try_send(frame); // drop if UI is behind
            }

            thread::sleep(interval);
        }
    });

    // Run UI on main thread
    let mut app = App::new(registry, config, running.clone());
    app.run(frame_rx)?;

    // Clean shutdown
    running.store(false, Ordering::Relaxed);
    let _ = processor_handle.join();

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully.

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up audio capture, processing, and UI in main"
```

---

### Task 11: Manual Integration Test

**Files:** None new — this is a manual verification step.

**Step 1: Build release binary**

Run: `cargo build --release`
Expected: Clean compile.

**Step 2: Run the app**

Run: `./target/release/terminal-vibes`
Expected:
- macOS prompts for audio capture permission (accept it)
- Terminal enters alternate screen with spectrum bars visualization
- Play music — bars should react to audio
- Press Tab to cycle through waveform and spectrogram modes
- Press +/- to adjust sensitivity
- Press s to toggle status bar
- Press q to quit cleanly
- Terminal restores properly on exit

**Step 3: Test --list-modes**

Run: `./target/release/terminal-vibes --list-modes`
Expected: Prints three modes and exits.

**Step 4: Test with a config file**

Create `~/.config/terminal-vibes/config.toml` with custom values and verify they take effect.

**Step 5: Commit any fixes found during integration testing**

```bash
git add -A
git commit -m "fix: integration test fixes"
```

---

### Task 12: README and Polish

**Files:**
- Create: `README.md`

**Step 1: Write README**

```markdown
# terminal-vibes

A terminal-based music visualizer that captures macOS system audio in real-time.

## Requirements

- macOS 15 (Sequoia) or later
- Rust 1.75+

## Install

\`\`\`bash
cargo install --path .
\`\`\`

## Usage

\`\`\`bash
terminal-vibes
\`\`\`

On first launch, macOS will prompt for audio capture permission.

### Keybindings

| Key | Action |
|-----|--------|
| Tab / Shift+Tab | Next / previous visualization |
| + / - | Increase / decrease sensitivity |
| s | Toggle status bar |
| q | Quit |

### Visualization Modes

- **spectrum** — Frequency spectrum bars with color gradient
- **waveform** — Oscilloscope-style waveform display
- **spectrogram** — Scrolling frequency heatmap

### Configuration

Config file: `~/.config/terminal-vibes/config.toml`

See [design doc](docs/plans/2026-03-07-terminal-vibes-design.md) for full config reference.

## Architecture

Three-thread pipeline:
1. **Audio thread** — Core Audio `AudioProcessTap` callback writes PCM into lock-free ring buffer
2. **Processor thread** — FFT + frequency binning produces `FrameData` at ~60Hz
3. **Main thread** — ratatui render loop with keyboard input

Visualizations implement a `Visualization` trait for extensibility.
\`\`\`

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with usage, keybindings, and architecture overview"
```
