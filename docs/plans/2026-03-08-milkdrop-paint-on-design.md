# MilkDrop Paint-On Rendering — Design

Adds a new "milkdrop" visualization mode with a double-buffered float RGB feedback canvas, inspired by MilkDrop's iconic paint-on rendering style. Instead of clearing every frame, the previous frame's pixel buffer persists, gets transformed (zoom, rotate, warp), and new elements paint on top — creating trails, feedback loops, and hypnotic morphing patterns.

## Core Concept: Paint-On Rendering

Traditional visualizations: clear → draw → display.
MilkDrop style: **keep previous frame → transform it → paint new content on top → display**.

The previous frame literally becomes part of the next frame's canvas. Combined with decay (fade), this creates organic trails and feedback loops.

## Architecture

### FeedbackCanvas (Double-Buffered Float RGB)

Two `Vec<(f32, f32, f32)>` buffers at HalfBlockCanvas pixel resolution (cols × rows×2):

- **Front buffer**: read source (previous frame's output)
- **Back buffer**: write target (current frame being built)

Float RGB (0.0–1.0 per channel) instead of u8 gives smooth 60+ step fades (u8 would staircase to black in ~8 steps). Memory cost: ~86KB for a 200×120 pixel terminal — trivial.

The double-buffer eliminates aliasing during warp transforms (reading pixels you've already overwritten). This is exactly what MilkDrop does.

### Per-Frame Pipeline

```
1. swap()           — last frame's output becomes the read source
2. warp/zoom/rotate — read front → write transformed into back
3. decay()          — fade back buffer (trail length control)
4. paint layers     — draw waveform, shapes, particles onto back
5. to_halfblock()   — convert float RGB → HalfBlockCanvas → ratatui
```

### Blend Modes

When painting onto the feedback buffer:
- **Additive** — `dst + src` (trails glow, colors accumulate)
- **Alpha** — `src` replaces `dst` (opaque overlay)
- **Max** — `max(dst, src)` per channel (brighten only)

### WarpGrid

A coarse displacement grid (e.g. 16×12 points) where each point has a (dx, dy) vector driven by audio. Pixels between grid points interpolate displacement via bilinear interpolation of the grid, then sample the front buffer with nearest-neighbor. This creates the flowing/melting distortion effect.

Audio mapping:
- Treble energy → ripple intensity (high-frequency displacement oscillation)
- Beat envelope → spike displacement momentarily
- Base displacement pattern: radial outward + slight rotation

## Paint Layers (Toggleable)

### Layer 1: Waveform (key: `w`)
- Bresenham line drawing the raw PCM waveform across the canvas
- Horizontal center line, amplitude maps to Y displacement
- Color cycles with hue offset (time + beat driven)
- Painted with Additive blend → leaves glowing trails through feedback

### Layer 2: Spectrum Shapes (key: `e`)
- Circle at center whose radius = bass energy
- Dots around circle at positions driven by spectrum band magnitudes
- Color per dot from ColorPalette, intensity from band energy
- Beat envelope scales the whole formation momentarily

### Layer 3: Particles (key: `r`)
- Pool of ~128 pre-allocated particles (no runtime allocation)
- Spawn bursts on beat detection from center with random velocity
- Each particle: position, velocity, life (0..1), color
- Update: position += velocity, life -= decay, recycle when dead
- Painted onto feedback buffer → naturally leave trails through warp/decay

## Transform ↔ Audio Mapping

| Audio Signal | Transform Parameter | Effect |
|---|---|---|
| Bass energy | Zoom amount | Bass hits = breathe outward |
| Mid energy | Rotation speed | Mids spin the image |
| Treble energy | Warp grid ripple | Treble = flowing distortion |
| Beat envelope | Spike all transforms | Beats punch everything |
| RMS | Decay factor | Louder = longer trails |

All parameters EMA-smoothed to prevent jitter.

## Controls

| Key | Action |
|---|---|
| `w` | Toggle waveform layer |
| `e` | Toggle spectrum shapes layer |
| `r` | Toggle particles layer |
| `d`/`D` | Increase/decrease decay (trail length) |
| `z`/`Z` | Increase/decrease base zoom |
| `p`/`P` | Cycle palette |

## File Layout

```
src/visualizations/feedback.rs  — FeedbackCanvas, BlendMode, WarpGrid
src/visualizations/milkdrop.rs  — Milkdrop viz struct, paint layers
src/visualizations/mod.rs       — add pub mod feedback; pub mod milkdrop;
src/main.rs                     — register Milkdrop
tests/milkdrop_test.rs          — integration tests
```
