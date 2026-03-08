# terminal-vibes

A real-time terminal music visualizer for macOS. Captures system audio and renders frequency spectrum, waveform, and spectrogram visualizations directly in your terminal.

Built with Rust, Core Audio, and [ratatui](https://github.com/ratatui/ratatui).

## Requirements

- **macOS 15+** (Sequoia) — uses the `AudioProcessTap` API for system audio capture
- Rust toolchain (1.70+)
- Terminal with true color support recommended

## Install

```bash
git clone https://github.com/joelmgallant/terminal-vibes.git
cd terminal-vibes
cargo build --release
```

The binary will be at `target/release/terminal-vibes`.

## Usage

```bash
cargo run --release
```

Or after building:

```bash
./target/release/terminal-vibes
```

### Options

```
terminal-vibes [OPTIONS]

Options:
  --config <PATH>    Path to config file
  --list-modes       List available visualization modes
  -h, --help         Print help
```

## Visualizations

Switch between modes with **Tab** / **Shift+Tab**.

### Spectrum Bars

Frequency spectrum with logarithmic band distribution and sub-block character resolution. 12 built-in color palettes including Neon, Fire, Ocean, Synthwave, Gruvbox, and more.

| Key | Action |
|-----|--------|
| `p` / `P` | Next / previous color palette |
| `g` | Toggle bar gaps |
| `c` | Toggle chunky mode |

### Waveform

Oscilloscope-style time-domain display of the audio signal.

### Spectrogram

Scrolling time-frequency heatmap with a magma colormap. Frequency on the vertical axis, time scrolling left to right.

### Global Keys

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous visualization |
| `+` / `-` | Increase / decrease sensitivity |
| `s` | Toggle status bar |
| `q` / `Ctrl+C` | Quit |

## Configuration

Configuration is loaded from `~/.config/terminal-vibes/config.toml` (respects `XDG_CONFIG_HOME`).

```toml
[audio]
fft_size = 2048
smoothing = 0.7
buffer_size = 4096

[display]
fps = 30
show_status_bar = true

[visualizations.spectrum]
# Per-visualization config

[visualizations.spectrogram]
history_length = 200
```

All fields are optional — defaults are applied for anything not specified.

## How It Works

Terminal-vibes runs a three-thread pipeline:

1. **Audio capture** — A Core Audio `AudioProcessTap` callback writes raw PCM samples into a lock-free ring buffer in real time
2. **DSP processing** — A worker thread runs an FFT pipeline at ~60Hz (windowing, FFT, logarithmic band binning, smoothing) and produces frame data
3. **Rendering** — The main thread runs a ratatui event loop at 30 FPS, drawing the active visualization to the terminal

The threads communicate via lock-free data structures (SPSC ring buffer, bounded channel) so the audio callback never blocks.

## License

MIT
