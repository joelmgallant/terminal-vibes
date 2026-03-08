# Milkdrop-Style Visualizers Design

Adds 7 new visualizations inspired by Milkdrop's aesthetic categories: geometric/mathematical, fluid/organic, and particle systems. Brings the total from 3 to 10 visualization modes.

## Rendering Utilities (`visualizations/render.rs`)

Shared primitives so visualizers focus on logic, not terminal plumbing.

### Braille Canvas

A 2D boolean pixel grid mapped to braille characters (U+2800 block). Each terminal cell encodes a 2x4 dot matrix, giving 2x horizontal and 4x vertical sub-cell resolution. API: plot points at pixel coordinates, flush to ratatui buffer with a color. Used by Lissajous, Starfield, and anything needing smooth curves or dots.

### Half-Block Canvas

A 2D color grid at 2x vertical resolution using `▀` (upper half), `▄` (lower half), and `█` (full block). Each terminal cell encodes two vertical "pixels" via fg/bg color. API: set pixel color at coordinates, flush to ratatui buffer. Used by Plasma, Aurora, Tunnel, Radial Spectrum — anything wanting smooth color fields.

### Common Math Helpers

Pure functions shared across visualizers: `lerp`, `smoothstep`, polar-to-cartesian, trig helpers. Keep minimal — add as needed.

## Visualizers

### 1. Lissajous (Geometric — Braille)

Parametric curves: `x = sin(a*t + phase_x)`, `y = sin(b*t + phase_y)`.

- Frequency ratio `a:b` drifts slowly over time (1:2, 2:3, 3:4, etc.) creating evolving spirograph shapes
- Bass spectrum energy modulates `phase_x`, mids modulate `phase_y`
- Peak amplitude scales curve size, RMS controls trail length (plot last N frames with fading brightness)
- Keybinds: `f` freeze drift, `r` randomize ratio

### 2. Tunnel (Geometric — Half-Block)

Concentric rings expanding outward from center, simulating forward motion through a tube.

- Rings rendered back-to-front as polygons (circle, hexagon, or square — user toggleable)
- Rings spawn at center, expand over time — speed driven by RMS
- Bass energy pulses ring spacing, treble modulates ring color
- Depth-based gradient (near = bright, far = dim) using existing `ColorPalette` system
- Keybinds: `s` cycle shape, `p`/`P` palette

### 3. Radial Spectrum (Geometric — Half-Block)

Spectrum bands as lines radiating from center in a starburst/sun pattern.

- Each line's length = band magnitude, angle = evenly spaced around 360°
- Slow rotation, speed scales with RMS
- Mirror mode draws both inward and outward from center
- Uses existing `ColorPalette` for gradient along each ray
- Keybinds: `m` mirror toggle, `p`/`P` palette

### 4. Plasma (Fluid — Half-Block)

Classic sine interference pattern filling every pixel with color.

- Formula: `v = sin(x/k + t) + sin(y/k + t) + sin((x+y)/k + t) + sin(sqrt(x²+y²)/k + t)`
- Value `v` maps through a rotating color palette (hue shifts over time)
- Spectrum bands modulate the `k` scaling factors — bass stretches the pattern, treble tightens it
- Peak boosts saturation, RMS controls animation speed
- Continuously morphing, never repeating
- Keybinds: `p`/`P` palette

### 5. Aurora (Fluid — Half-Block)

Layered horizontal color curtains that wave and shimmer vertically, like northern lights.

- Rendered column by column, each column has overlapping sine-wave curtains (3-4 layers)
- Each curtain has independent wave speed, amplitude, and color
- Spectrum bands map to curtain heights — bass controls lowest/widest, treble controls highest/thinnest
- RMS modulates shimmer speed, peak adds brightness flares
- Colors blend additively where curtains overlap
- Keybinds: `l` cycle layer count (2/3/4), `p`/`P` palette

### 6. Starfield (Particles — Braille)

3D particle system projected to 2D, flying outward from center.

- Particles stored as `(x, y, z)`, projected with perspective divide (`screen_x = x/z`)
- Base drift speed tied to RMS — quiet = gentle float, loud = warp speed
- Peak spikes spawn bursts of new particles at center
- Brightness based on z-depth (closer = brighter, uses braille dot density)
- Particle pool of ~500, recycled on screen exit — no runtime allocations
- Keybinds: `d` toggle density (250/500/1000)

### 7. Rain (Particles — Block Characters)

Vertical streams of characters falling per-column, Matrix-style but music-reactive.

- Uses `│`, `┃`, `╽`, `║` and partial blocks for droplet heads
- Each column has independent streams at different speeds
- Stream density per column driven by corresponding spectrum band — bass on left rains hard, treble on right sparkles
- Head-to-tail brightness fade (bright head, dim trail)
- RMS controls global fall speed, peak flashes newest droplet heads
- Random character variation in tails for texture
- Default palette: Matrix green. Fire rain also goes hard.
- Keybinds: `t` toggle thin/thick streams, `p`/`P` palette

## Beat Detection Interface (Stub)

Nested struct added to `FrameData`. All fields default to zero/false — no behavior change until detection pipeline is implemented separately.

```rust
pub struct BeatData {
    // Per-band beat detected this frame
    pub bass_beat: bool,
    pub mid_beat: bool,
    pub treble_beat: bool,

    // Per-band pulse envelopes (0.0..1.0)
    // Fast attack, configurable decay
    pub bass_pulse: f32,
    pub mid_pulse: f32,
    pub treble_pulse: f32,

    // Per-band energy levels (0.0..1.0, pre-threshold)
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,
}
```

Added to `FrameData` as `pub beat: BeatData`. Processor fills with `BeatData::default()` until detection lands.

### Future Beat Wiring (Not Implemented Now)

Visualizers designed with these hooks in mind:

- **Tunnel** — `bass_beat` triggers ring spawn burst, `bass_pulse` modulates ring spacing
- **Starfield** — `bass_beat` spawns particle bursts, `treble_pulse` modulates brightness
- **Plasma** — `mid_pulse` modulates wave scaling, `bass_pulse` controls animation speed
- **Rain** — `bass_beat` triggers flash on droplet heads, `treble_energy` drives sparkle density

## Palette System

All new visualizers that render in color reuse the existing `ColorPalette` enum and `p`/`P` keybinds. No new palette infrastructure needed — the 12 palettes already cover the aesthetic range.
