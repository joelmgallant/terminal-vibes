```
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░▒▓██████████████████████████████████████████████████████████████████████████▓▒░
░▓                                                                            ▓░
░▓   ████████ ████████ ███████  ██     ██ ██ ██    ██  ██████  ██             ▓░
░▓      ██    ██       ██   ██  ███   ███ ██ ███   ██ ██    ██ ██             ▓░
░▓      ██    ██████   ███████  ██ █ █ ██ ██ ██ █  ██ ████████ ██             ▓░
░▓      ██    ██       ██  ██   ██  █  ██ ██ ██  █ ██ ██    ██ ██             ▓░
░▓      ██    ████████ ██   ██  ██     ██ ██ ██   ███ ██    ██ ████████       ▓░
░▓                                                                            ▓░
░▓  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ▓░
░▓  ░▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒░  ▓░
░▓  ░▒   ██    ██ ██ ██████  ████████  ██████                             ▒░  ▓░
░▓  ░▒   ██    ██ ██ ██   ██ ██       ██                                  ▒░  ▓░
░▓  ░▒    ██  ██  ██ ██████  █████     █████                              ▒░  ▓░
░▓  ░▒     ████   ██ ██   ██ ██            ██                             ▒░  ▓░
░▓  ░▒      ██    ██ ██████  ████████ ██████                              ▒░  ▓░
░▓  ░▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒░  ▓░
░▓  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ▓░
░▓                                                                            ▓░
░▓    ·── ♪ ♫  real-time audio visualizer for your terminal  ♫ ♪ ──·          ▓░
░▓                                                                            ▓░
░▓          ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄          ▓░
░▓          █ macOS 15+  ·  Core Audio  ·  ratatui  ·  10 modes    █          ▓░
░▓          █ beat detect ·  60 fps  ·  braille + halfblock art    █          ▓░
░▓          ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀          ▓░
░▓                                                                            ▓░
░▓     sysop: joelmgallant       ░▒▓█▓▒░       est. 2026  ·  rust lang        ▓░
░▓                                                                            ▓░
░▒▓██████████████████████████████████████████████████████████████████████████▓▒░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
```

A real-time terminal music visualizer for macOS. Captures system audio and renders 10 visualization modes directly in your terminal. Built with Rust, Core Audio, and [ratatui](https://github.com/ratatui/ratatui).

https://github.com/user-attachments/assets/f05ef7fd-fa78-4616-abf4-30affbe15c41

## Requirements

- **macOS 15+** (Sequoia) — uses the `AudioProcessTap` API for system audio capture
- Rust toolchain (1.70+)
- Terminal with true color support recommended

## Install

```bash
cargo install terminal-vibes
```

Or build from source:

```bash
git clone https://github.com/joelmgallant/terminal-vibes.git
cd terminal-vibes
cargo build --release
```

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

Switch between modes with **Tab** / **Shift+Tab**. The mode name fades in on switch then disappears after a few seconds.

### Spectrum Bars

Frequency spectrum with logarithmic band distribution and sub-block character resolution. 12 built-in color palettes.

| Key | Action |
|-----|--------|
| `p` / `P` | Next / previous color palette |
| `g` | Toggle bar gaps |
| `c` | Toggle chunky mode |
| `r` | Toggle color cycling |

**Palettes:** Neon, Fire, Ocean, Sunset, Matrix, Ice, GruvboxDark, GruvboxLight, Synthwave, Outrun, Retrowave, Pastel

### Waveform

Oscilloscope-style time-domain display with beat-reactive brightness.

### Spectrogram

Scrolling time-frequency heatmap with a magma colormap. Frequency on the vertical axis, time scrolling left to right.

### Lissajous

Parametric curve spirograph with 8 frequency ratios and braille-resolution rendering.

| Key | Action |
|-----|--------|
| `f` | Freeze / unfreeze ratio drift |
| `r` | Next ratio |

### Tunnel

Concentric rings rushing forward in 3 shape modes.

| Key | Action |
|-----|--------|
| `s` | Cycle shape (circle / hexagon / square) |
| `p` / `P` | Next / previous palette |

### Radial

Circular spectrum starburst — frequency rays radiating from center with beat-driven rotation.

| Key | Action |
|-----|--------|
| `m` | Toggle mirror mode |
| `p` / `P` | Next / previous palette |

### Plasma

Sine wave interference color field. Spectrum bands modulate wave frequencies, beats warp density and shift hue.

### Aurora

Northern lights curtains with flowing animation. Layered sine waves with HSV color cycling and shimmer.

| Key | Action |
|-----|--------|
| `l` | Cycle layer count (2 / 3 / 4) |

### Starfield

3D particle warp drive. Quiet = gentle drift, loud = warp speed, beat = hyperspace burst.

### Rain

Music-reactive falling streams. Per-column energy controls drop density, beats trigger storms.

| Key | Action |
|-----|--------|
| `t` | Toggle thickness (thin / thick) |
| `p` / `P` | Next / previous palette |

## Global Controls

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous visualization |
| `+` / `-` | Increase / decrease sensitivity |
| `b` / `B` | Increase / decrease beat intensity |
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

[beat_detection]
sensitivity = 1.4
variance_scale = 1.0
envelope_decay = 0.95
cooldown_frames = 6
history_frames = 43

[visualizations.spectrum]
palette = "neon"
gap = false
chunky = true
cycling = false

[visualizations.spectrogram]
history_length = 200
```

All fields are optional — defaults are applied for anything not specified.

App state (current visualization, sensitivity, beat intensity, per-viz settings) is persisted across sessions in `~/.config/terminal-vibes/state.toml`.

## How It Works

Terminal-vibes runs a three-thread pipeline:

1. **Audio capture** — A Core Audio `AudioProcessTap` callback writes raw PCM samples into a lock-free ring buffer in real time
2. **DSP processing** — A worker thread runs an FFT pipeline at ~60Hz (windowing, FFT, logarithmic band binning, smoothing) and produces frame data with beat detection
3. **Rendering** — The main thread runs a ratatui event loop at 30 FPS, drawing the active visualization to the terminal

The threads communicate via lock-free data structures (SPSC ring buffer, bounded channel) so the audio callback never blocks.

## License

MIT
