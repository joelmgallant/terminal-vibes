# Terminal Vibes — Design Document

A terminal-based music visualizer that hooks into macOS system audio via Core Audio `AudioProcessTap` (macOS 15+) and renders real-time visualizations using ratatui.

## Requirements

- **Language**: Rust
- **Audio capture**: Core Audio `AudioProcessTap` (macOS 15+) — no virtual drivers needed
- **Visualizations**: Multiple modes (spectrum bars, waveform, spectrogram) — toggleable at runtime
- **Layout**: Flexible, adapts to any terminal/pane size
- **Interaction**: TOML config file for defaults + keyboard for live control
- **Extensibility**: Trait-based visualization plugin system for future custom modes

## Architecture

### Threading Model

```
+------------------+     lock-free      +------------------+    channel     +------------------+
|   Audio Thread   |------------------>| Processor Thread |-------------->|   Main Thread    |
|                  |     ring buffer    |                  |   FrameData   |                  |
| CoreAudio Tap    |                    | FFT + binning    |   (bounded)   | ratatui render   |
| callback writes  |                    | smoothing        |               | input polling    |
| raw PCM samples  |                    | feature extract  |               | app state        |
+------------------+                    +------------------+               +------------------+
```

Three threads with clean separation:

1. **Audio thread** — Owned by Core Audio runtime. The `AudioProcessTap` callback writes raw f32 samples into a lock-free SPSC ring buffer. Must never block or allocate.
2. **Processor thread** — Reads from ring buffer at ~60Hz, runs FFT pipeline, produces `FrameData`, sends over a bounded `mpsc::sync_channel`. Drops frames if channel is full (graceful degradation).
3. **Main thread** — Runs the ratatui event loop. Polls keyboard input, receives `FrameData`, calls `update()` + `render()` on the active visualization.

### Lifecycle

1. Parse config, init logging
2. Request audio capture permission
3. Set up `AudioProcessTap` on the system audio process
4. Spawn processor thread
5. Enter main render loop
6. On quit: signal threads via `Arc<AtomicBool>`, join, restore terminal, exit

## Audio Capture Layer

Core Audio `AudioProcessTap` via `objc2` FFI.

- `AudioProcessTap` (macOS 15+) installs a tap on any audio process (or the system mix) to receive PCM audio buffers in real-time.
- `objc2`, `objc2-foundation`, and `block2` crates bridge into the Objective-C runtime.
- Bindings for `AudioProcessTap` will need manual definition since the API is very new.
- The tap callback delivers audio as `AudioBufferList` containing float32 PCM samples.
- Samples are written into a lock-free ring buffer (`ringbuf` crate) from the callback.
- A separate processing thread reads from the ring buffer and downmixes to mono if needed.

### Key Types

```
AudioTap          — sets up the AudioProcessTap, manages lifecycle
RingBuffer<f32>   — lock-free SPSC buffer between audio callback and processor
AudioConfig       — sample rate, buffer size, channel count
```

### Permissions

macOS will prompt the user for audio capture permission on first launch. The app handles denial gracefully with a clear terminal message.

## Audio Processing / DSP Pipeline

FFT and feature extraction using `rustfft`.

### Processing Chain

1. Read N samples from ring buffer
2. Apply Hann window function
3. Run FFT (default size 2048) -> complex output
4. Compute magnitude spectrum (`sqrt(re^2 + im^2)`)
5. Convert to dB scale with configurable floor (e.g., -60dB)
6. Bin magnitudes into frequency bands (logarithmic spacing)

### Output Data

```rust
FrameData {
    spectrum: Vec<f32>,        // binned frequency magnitudes (for spectrum bars)
    waveform: Vec<f32>,        // raw PCM samples (for oscilloscope)
    magnitudes: Vec<Vec<f32>>, // rolling history of spectrums (for spectrogram)
    peak: f32,                 // current peak amplitude
    rms: f32,                  // current RMS level
}
```

- Target ~60 FPS of frame production
- Exponential moving average smoothing on spectrum values (configurable decay)
- Rolling window of ~200 frames for spectrogram history

## Terminal UI Layer

Ratatui-based flexible renderer with plugin architecture.

### Visualization Plugin Trait

```rust
pub trait Visualization: Send {
    /// Unique name shown in status bar and config
    fn name(&self) -> &str;

    /// Process a new frame of audio data (update internal state)
    fn update(&mut self, frame: &FrameData);

    /// Render into the given ratatui buffer area
    fn render(&self, area: Rect, buf: &mut Buffer);

    /// Optional: handle a keypress specific to this visualization
    fn on_key(&mut self, key: KeyEvent) -> bool { false }

    /// Optional: provide config schema for this visualization
    fn default_config(&self) -> toml::Value { toml::Value::Table(Default::default()) }

    /// Optional: apply config values
    fn apply_config(&mut self, config: &toml::Value) {}
}
```

- Every visualization implements `Visualization`. The core app is decoupled from internals.
- `VisualizationRegistry` holds `Vec<Box<dyn Visualization>>`. Built-in modes register at startup.
- Per-plugin config namespaced under `[visualizations.<name>]` in TOML.
- Per-plugin keybindings via `on_key()` — mode-specific keys without polluting the global keymap.

### Built-in Plugins (v1)

| Plugin | Description |
|--------|-------------|
| `SpectrumBars` | Logarithmic frequency bars with color gradient |
| `Waveform` | Oscilloscope-style PCM line |
| `Spectrogram` | Scrolling frequency heatmap |

### Chrome

Minimal status bar at the bottom: active mode, peak level, sample rate, keybinding hints. Toggleable.

## Configuration & Input

### Config File

Location: `~/.config/terminal-vibes/config.toml` (respects `XDG_CONFIG_HOME`). Created with sensible defaults on first launch.

```toml
[audio]
fft_size = 2048
smoothing = 0.7
buffer_size = 4096

[display]
fps = 30
color_mode = "truecolor"  # "truecolor" | "256" | "auto"
show_status_bar = true

[keybindings]
next_mode = "Tab"
prev_mode = "Shift+Tab"
quit = "q"
toggle_status = "s"
increase_sensitivity = "+"
decrease_sensitivity = "-"

[visualizations.spectrum]
bar_width = 2
color_gradient = ["#ff0055", "#ffaa00", "#00ffaa", "#0055ff"]

[visualizations.waveform]
color = "#00ff88"
line_style = "thick"

[visualizations.spectrogram]
color_map = "magma"
history_length = 200
```

### CLI Args

Minimal: `--config <path>` override and `--list-modes`. No flag duplication of config options.

### Runtime Input

`crossterm::event::read()` polled between frames. Global keys handled by `App`, mode-specific keys forwarded to `Visualization::on_key()`.

## Error Handling

- Audio permission denied -> clear message, exit with instructions
- Audio device disconnected -> attempt reconnect, show "no audio" state
- Terminal too small -> show resize message instead of crashing

## Crate Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | Terminal UI framework |
| `crossterm` | Terminal backend + input |
| `objc2`, `block2` | Core Audio FFI bridge |
| `rustfft` | FFT processing |
| `ringbuf` | Lock-free SPSC ring buffer |
| `serde`, `toml` | Config parsing |
| `dirs` | XDG config path resolution |
| `log`, `env_logger` | Debug logging |
