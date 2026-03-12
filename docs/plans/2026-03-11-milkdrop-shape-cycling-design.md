# MilkDrop Shape Cycling — Design

Adds smooth cycling between different geometric shape variations to the milkdrop visualization's `paint_shapes` layer. Shapes morph via interpolation on a hybrid timer (auto-cycle + beat-triggered early transitions), with each shape carrying its own visual personality (radius scale, brightness, hue offset).

## Shape Library

5 shapes defined as a static array of preset structs, each with a polar radius function and visual parameters:

| Shape | Polar function `r(t, time)` | Base radius scale | Brightness | Hue offset |
|---|---|---|---|---|
| Circle | `1.0` (constant) | 1.0 | 0.7 | 0.0 |
| Polygon | `cos(π/n) / cos((t*n) mod (2π/n) - π/n)`, random n=3-6 per visit | 0.9 | 0.8 | 0.1 |
| Star | `0.5 + 0.5 * abs(sin(n * angle))`, n=5 | 1.1 | 0.9 | 0.2 |
| Rose | `cos(k * angle)`, k=3 (3-petal flower) | 1.2 | 0.6 | 0.35 |
| Spiral | `t` (radius grows linearly, wrapping multiple revolutions) | 0.8 | 0.75 | 0.5 |

```rust
struct ShapePreset {
    radius_fn: fn(t: f32, time: f32) -> f32,  // polar radius multiplier
    base_radius_scale: f32,
    brightness: f32,
    hue_offset: f32,
    sides: u8,  // for polygon (0 = unused by other shapes)
}
```

Static array `SHAPE_PRESETS: [ShapePreset; 5]`.

## Morph State Machine

New fields on the `Milkdrop` struct:

```rust
shape_index: usize,          // current shape in SHAPE_PRESETS
next_shape_index: usize,     // target shape for morph
morph_t: f32,                // 0.0 = fully current, 1.0 = fully next
morphing: bool,              // whether a transition is active
morph_speed: f32,            // how fast morph_t advances per frame (~0.02 = ~50 frames ≈ 0.8s)
cycle_timer: f32,            // counts up, resets when transition triggers
cycle_interval: f32,         // seconds between auto-transitions (default ~10s)
polygon_sides: u8,           // picked randomly (3-6) each time polygon comes up
```

### Per-frame logic (in `update_transforms`):

1. **Not morphing:** increment `cycle_timer` by frame delta (~0.016s). If `cycle_timer >= cycle_interval` OR `beat_envelope > 0.7` (strong beat), start morph — pick next shape (sequential, wrapping), reset `morph_t = 0.0`, set `morphing = true`, reset timer.
2. **Morphing:** advance `morph_t += morph_speed`. Use `smoothstep(morph_t)` for eased interpolation. When `morph_t >= 1.0`, snap to next shape, set `morphing = false`.
3. **Cooldown:** beat-triggered transitions can't re-trigger within 3 seconds of the last transition.

### Polygon side count

When polygon comes up as the next shape in the cycle, `polygon_sides` is set to a random value 3-6 (using a deterministic hash of `time` to avoid pulling in a RNG crate). Fixed for the duration of that visit.

## Paint Integration

Refactored `paint_shapes` method:

1. Evaluate both `SHAPE_PRESETS[shape_index]` and `SHAPE_PRESETS[next_shape_index]`
2. For each dot `i` (0..num_dots):
   - Compute angle `t = i / num_dots`
   - Get `r_a = preset_a.radius_fn(t, time)` and `r_b = preset_b.radius_fn(t, time)`
   - Lerp radius: `lerp(r_a * scale_a, r_b * scale_b, blend)` where `blend = smoothstep(morph_t)`
   - Lerp brightness: `lerp(bright_a, bright_b, blend)`
   - Lerp hue offset: `lerp(hue_a, hue_b, blend)`
3. Final radius still receives existing bass/beat modulation on top

## Controls

| Key | Action |
|---|---|
| `n` | Force next shape transition |
| `N` | Force previous shape transition |

Added to `help_keys` and `on_key`. Existing `e` toggle still enables/disables the entire shapes layer.

## Config Persistence

No new TOML fields. Shape cycling is ephemeral state — starts fresh each session. `shapes_enabled` toggle is already persisted.

## Scope

Changes are fully contained within `milkdrop.rs`. No modifications to `FeedbackCanvas`, `WarpGrid`, `HalfBlockCanvas`, `render.rs`, `mod.rs`, `main.rs`, or any other visualization.

## Performance

- Zero new allocations — shape presets are static, morph state is just floats
- Per-dot cost adds one extra polar function call + 3 lerps during morphing — negligible vs existing `paint` calls on the feedback buffer
- `smoothstep` is 3 multiplies, no trig
- Polygon's polar function uses `cos` — could use `SIN_LUT` if profiling warrants, but with only 64 dots it won't matter

## Testing

- Unit tests for each polar function (circle returns 1.0, star has expected peaks/valleys, etc.)
- Unit test for morph interpolation (blend=0 → shape A, blend=1 → shape B, blend=0.5 → midpoint)
- Unit test for cycle timer logic (timer triggers transition, beat triggers early transition, cooldown prevents rapid re-triggers)
- Integration test: construct Milkdrop, feed frames, verify no panic across multiple shape transitions

## Edge Cases

- Terminal resize mid-morph — handled by existing `feedback.resize()`, morph state is resolution-independent (normalized 0..1 coordinates)
- Shapes disabled mid-morph — `paint_shapes` early-returns, morph state keeps ticking so it's clean when re-enabled
- All spectrum data empty — existing guard `self.spectrum.is_empty()` handles it
