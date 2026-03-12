# GUI Pixel Rendering Mode — Design

## Summary

Add a GPU-accelerated windowed mode alongside the existing terminal visualizer. Single binary, `--gui` flag, powered by wgpu + winit. Full WGSL shader pipeline with hot-reloadable user presets and Shadertoy-compatible audio data. Terminal mode stays exactly as-is with zero regressions.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Product model | Side-by-side (terminal + GUI) | Ship both, don't replace terminal |
| Visual fidelity | Full GPU shader pipeline | WGSL fragment shaders, maximum flexibility |
| GPU API | wgpu | Rust-native, cross-platform, WebGPU future |
| Windowing | winit | Minimal, standard wgpu pairing |
| Viz relationship | Hybrid | Shared vizs via common math, GPU-exclusive shader vizs |
| Launch mode | Single binary, `--gui` flag | Feature-gated, no GPU deps in default build |
| Shader customization | Built-in + hot-reload from disk | Ship curated presets, users add `.wgsl` files |
| Audio data format | Shadertoy-compat + extras | 256x2 audio texture + beat/tempo uniforms |
| Milkdrop compat | Phased — inspiration first | Design for future `.milk` support, don't build it yet |

## Crate Architecture

```
terminal-vibes/                    (workspace root)
├── Cargo.toml                     (workspace definition)
├── crates/
│   ├── core/                      (terminal-vibes-core lib crate)
│   │   ├── audio/                 (tap.rs, wasapi.rs, pulse.rs, mod.rs)
│   │   ├── processing.rs          (FFT pipeline, FrameData)
│   │   ├── beat.rs                (beat detection, tempo, BeatData)
│   │   ├── config.rs              (TOML config, XDG paths)
│   │   └── lib.rs
│   └── app/                       (terminal-vibes bin crate)
│       ├── terminal/              (all existing terminal rendering)
│       │   ├── ui.rs              (ratatui event loop, status bar, frame budget)
│       │   ├── visualizations/    (ALL existing viz code, untouched)
│       │   │   ├── mod.rs         (Visualization trait)
│       │   │   ├── registry.rs
│       │   │   ├── render.rs      (HalfBlockCanvas, BrailleCanvas, SinLut, quantize_color)
│       │   │   ├── spectrum.rs, plasma.rs, aurora.rs, ...
│       │   └── mod.rs
│       ├── gpu/                   (new GPU rendering, behind feature flag)
│       │   ├── renderer.rs        (wgpu device, surface, pipeline setup)
│       │   ├── audio_texture.rs   (FrameData -> GPU textures)
│       │   ├── shader_loader.rs   (built-in + hot-reload from disk)
│       │   ├── uniforms.rs        (time, resolution, beat data)
│       │   ├── window.rs          (winit event loop, input handling)
│       │   ├── visualizations/    (GPU viz trait + implementations)
│       │   │   ├── mod.rs         (GpuVisualization trait)
│       │   │   ├── registry.rs
│       │   │   └── ...
│       │   └── mod.rs
│       └── main.rs                (CLI parsing, --gui flag routing)
```

**Principles:**
- `core` has zero rendering dependencies (no ratatui, no wgpu, no winit)
- `terminal/` is the existing code relocated with no behavioral changes
- `gpu/` is entirely behind `#[cfg(feature = "gui")]`
- `main.rs` routes to `terminal::run()` or `gpu::run()` based on `--gui` flag

## GPU Rendering Pipeline

**Per frame:**
1. Drain latest `FrameData` from bounded mpsc channel (same as terminal mode)
2. Upload 256x2 RGBA audio texture (row 0 = waveform, row 1 = FFT) — Shadertoy convention
3. Update uniform buffer (time, resolution, beat/tempo data, frame count)
4. Bind active shader's render pipeline
5. Draw fullscreen triangle
6. Present frame

**Shader interface:**

```wgsl
// Shadertoy-compatible audio data
@group(0) @binding(0) var audio_data: texture_2d<f32>;    // 256x2
@group(0) @binding(1) var audio_sampler: sampler;

// terminal-vibes uniforms
struct Uniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    frame: u32,
    beat_envelope: f32,
    bass_energy: f32,
    mid_energy: f32,
    treble_energy: f32,
    bass_beat: f32,         // 1.0 on beat, 0.0 otherwise
    mid_beat: f32,
    treble_beat: f32,
    bpm: f32,
    beat_phase: f32,        // 0.0..1.0
    beat_confidence: f32,
};
@group(0) @binding(2) var<uniform> u: Uniforms;
```

## GpuVisualization Trait

```rust
pub trait GpuVisualization: Send {
    fn name(&self) -> &str;
    fn update(&mut self, frame: &FrameData);
    fn shader_source(&self) -> &str;
    fn custom_uniforms(&self) -> Vec<u8> { vec![] }
    fn additional_textures(&self) -> Vec<GpuTexture> { vec![] }
    fn on_key(&mut self, key: KeyEvent) -> bool { false }
    fn default_config(&self) -> toml::Value;
    fn apply_config(&mut self, config: &toml::Value);
    fn save_config(&self) -> toml::Value;
}
```

**Two kinds:**
1. **Simple shader presets** — A WGSL string (embedded or loaded from disk). All logic in the shader
2. **Complex vizs** — Multi-pass, feedback textures, custom state. Real `update()` logic, additional textures

## Shader Hot-Reload

- Watch `~/.config/terminal-vibes/shaders/*.wgsl` via `notify` crate
- On change: recompile shader, rebuild pipeline, hot-swap
- On new file: register as new viz, available via Tab cycling
- On delete: remove from registry, fall back to previous viz
- On compile error: log warning, keep last working shader. No crash
- Built-in shaders embedded via `include_str!()`
- Ship a documented example shader as a starting template

## Feature Gating & Dependencies

```toml
[features]
default = []
gui = ["dep:wgpu", "dep:winit", "dep:notify"]
```

| Command | Result |
|---------|--------|
| `cargo build` | Terminal-only, no GPU deps |
| `cargo build --features gui` | Both modes compiled |
| `cargo run` | Terminal mode |
| `cargo run --features gui -- --gui` | GPU window mode |

## Input & UX (GUI Mode)

| Key | Action |
|-----|--------|
| Tab / Shift+Tab | Cycle GPU visualizations |
| `+` / `-` | Sensitivity |
| `b` / `B` | Beat intensity |
| `s` | Toggle status overlay |
| `f` / `F11` | Toggle fullscreen |
| `q` / `Esc` | Quit |

- Default window size: 1280x720, resizable
- Status overlay: viz name, BPM, sensitivity, fps. Fades after interaction
- No mouse interaction in v1
- Window title: `terminal-vibes — <viz_name>`

## Config Additions

```toml
[gui]
width = 1280
height = 720
vsync = true
fullscreen = false

[gui.shader_dirs]
extra = ["~/my-shaders"]
```

## Migration Strategy (Zero Regression Guarantee)

1. **Extract core crate** — Move audio, processing, beat, config. Wire back up. All existing tests pass. Terminal behavior identical
2. **Relocate terminal code** — Pure file moves into `crates/app/terminal/`. No logic changes. All tests pass
3. **Build GPU module** — From scratch behind feature flag. Terminal code never touched during GPU development

**Preserved without modification:**
- HalfBlockCanvas, BrailleCanvas, SinLut, quantize_color, adaptive_quantization_step
- Frame budget monitoring and auto-adjustment
- Tmux detection and FPS capping
- Heavy rendering pause when unfocused
- All 10 existing visualizations
- Ring buffer, bounded channel, frame dropping
- State persistence and config format (additive changes only)

## Explicit Non-Goals (v1)

- No mouse interaction
- No GUI widgets / settings panels
- No multi-window support
- No recording / export to video
- No Milkdrop `.milk` file parsing
- No HLSL/GLSL transpilation
- No WebAssembly target
- No feedback textures / multi-pass (trait supports it, not implemented)

## Future Roadmap (Enabled by This Architecture)

- Milkdrop preset loading via equation parser + HLSL->WGSL transpiler
- Feedback textures (previous frame as input) for trails/echo effects
- Multi-pass rendering for blur, bloom, composite
- WebAssembly + WebGPU browser target
- projectM integration as optional backend
- Mouse interaction and GUI settings panel
