# MilkDrop Shape Cycling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add smooth cycling between 5 geometric shape variations to the milkdrop visualization's `paint_shapes` layer, with hybrid auto/beat-triggered transitions and full crossfade interpolation.

**Architecture:** Polar function table approach — shapes defined as a static array of preset structs with function pointers and visual params. A morph state machine lerps between current and next shape using smoothstep easing. All changes contained within `milkdrop.rs`.

**Tech Stack:** Rust, ratatui, existing `lerp`/`smoothstep` from `render.rs`, `SIN_LUT` for trig.

---

### Task 1: Add ShapePreset struct and polar functions

**Files:**
- Modify: `src/visualizations/milkdrop.rs:1-11` (add after existing constants)

**Step 1: Write unit tests for polar functions**

Add a `#[cfg(test)]` module at the bottom of `milkdrop.rs` with tests for each shape function:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_shape_circle_constant() {
        // Circle should always return 1.0 regardless of input
        assert!((shape_circle(0.0, 0.0) - 1.0).abs() < f32::EPSILON);
        assert!((shape_circle(0.5, 10.0) - 1.0).abs() < f32::EPSILON);
        assert!((shape_circle(1.0, 99.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_shape_star_has_peaks_and_valleys() {
        // Star with 5 points: at peak angles should be ~1.0, at valley angles should be ~0.5
        let peak = shape_star(0.0, 0.0); // t=0 is a peak
        let valley = shape_star(0.1, 0.0); // between peaks
        assert!(peak > valley, "star peaks should exceed valleys");
        // Output should be in 0.5..=1.0 range
        for i in 0..100 {
            let v = shape_star(i as f32 / 100.0, 0.0);
            assert!(v >= 0.49 && v <= 1.01, "star value {v} out of range");
        }
    }

    #[test]
    fn test_shape_rose_symmetric() {
        // Rose with k=3 should have 3 petals, values in 0.0..=1.0
        for i in 0..100 {
            let v = shape_rose(i as f32 / 100.0, 0.0);
            assert!(v >= -0.01 && v <= 1.01, "rose value {v} out of range");
        }
    }

    #[test]
    fn test_shape_spiral_grows_with_t() {
        // Spiral radius increases with t
        let r1 = shape_spiral(0.0, 0.0);
        let r2 = shape_spiral(0.5, 0.0);
        let r3 = shape_spiral(1.0, 0.0);
        assert!(r3 > r2, "spiral should grow: r3={r3} > r2={r2}");
        assert!(r2 > r1, "spiral should grow: r2={r2} > r1={r1}");
    }

    #[test]
    fn test_shape_polygon_nonzero() {
        // Polygon should produce positive values for any side count 3-6
        for sides in 3..=6 {
            for i in 0..64 {
                let v = shape_polygon(i as f32 / 64.0, 0.0, sides);
                assert!(v > 0.0, "polygon sides={sides} t={} gave {v}", i as f32 / 64.0);
            }
        }
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib milkdrop::tests -v`
Expected: FAIL — `shape_circle`, `shape_star`, etc. not defined yet.

**Step 3: Add ShapePreset struct and the 5 polar functions**

Add after `const WARP_GRID_H` (line 11), before `pub struct Milkdrop`:

```rust
/// Polar radius function: takes normalized parameter t (0..1 around the circle)
/// and animation time, returns a radius multiplier.
type PolarFn = fn(t: f32, time: f32) -> f32;

struct ShapePreset {
    name: &'static str,
    radius_fn: PolarFn,
    base_radius_scale: f32,
    brightness: f32,
    hue_offset: f32,
}

const SHAPE_PRESETS: [ShapePreset; 5] = [
    ShapePreset { name: "circle",  radius_fn: shape_circle,  base_radius_scale: 1.0, brightness: 0.7, hue_offset: 0.0 },
    ShapePreset { name: "polygon", radius_fn: shape_polygon_default, base_radius_scale: 0.9, brightness: 0.8, hue_offset: 0.1 },
    ShapePreset { name: "star",    radius_fn: shape_star,    base_radius_scale: 1.1, brightness: 0.9, hue_offset: 0.2 },
    ShapePreset { name: "rose",    radius_fn: shape_rose,    base_radius_scale: 1.2, brightness: 0.6, hue_offset: 0.35 },
    ShapePreset { name: "spiral",  radius_fn: shape_spiral,  base_radius_scale: 0.8, brightness: 0.75, hue_offset: 0.5 },
];

fn shape_circle(_t: f32, _time: f32) -> f32 {
    1.0
}

/// Polygon with configurable sides. Standalone version used by tests.
fn shape_polygon(t: f32, _time: f32, sides: u8) -> f32 {
    let n = sides as f32;
    let angle = t * 2.0 * PI;
    let sector = PI / n;
    // Distance from center to polygon edge at this angle
    sector.cos() / ((angle % (2.0 * sector)) - sector).cos().abs().max(0.001)
}

/// Default polygon (hexagon) for use in the SHAPE_PRESETS const array.
fn shape_polygon_default(t: f32, time: f32) -> f32 {
    shape_polygon(t, time, 6)
}

fn shape_star(t: f32, _time: f32) -> f32 {
    let angle = t * 2.0 * PI;
    0.5 + 0.5 * (5.0 * angle).sin().abs()
}

fn shape_rose(t: f32, _time: f32) -> f32 {
    let angle = t * 2.0 * PI;
    (3.0 * angle).cos().abs()
}

fn shape_spiral(t: f32, _time: f32) -> f32 {
    // t goes 0..1, spiral wraps 3 revolutions so dots spread outward
    0.3 + 0.7 * t
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib milkdrop::tests -v`
Expected: All 5 tests PASS.

**Step 5: Run full test suite and lint**

Run: `cargo clippy && cargo test --lib`
Expected: No warnings, all tests pass.

**Step 6: Commit**

```bash
git add src/visualizations/milkdrop.rs
git commit -m "feat(milkdrop): add shape preset structs and polar functions

Five polar functions (circle, polygon, star, rose, spiral) with
ShapePreset config and unit tests for each."
```

---

### Task 2: Add morph state fields and cycling logic

**Files:**
- Modify: `src/visualizations/milkdrop.rs` — struct fields (lines 13-52), `new()` (lines 70-104), `update_transforms()` (lines 129-159)

**Step 1: Write unit tests for morph state machine**

Add to the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_morph_timer_triggers_transition() {
    let mut m = Milkdrop::new();
    assert!(!m.morphing);
    assert_eq!(m.shape_index, 0);

    // Simulate enough time passing to trigger auto-cycle
    // cycle_interval defaults to 10.0, each update_transforms adds ~0.015
    m.cycle_timer = 10.0; // force timer past threshold
    m.update_shape_cycle();
    assert!(m.morphing, "should start morphing after timer expires");
    assert_eq!(m.next_shape_index, 1);
}

#[test]
fn test_morph_beat_triggers_early_transition() {
    let mut m = Milkdrop::new();
    m.cycle_timer = 5.0; // not at interval yet
    m.beat_envelope = 0.8; // strong beat
    m.update_shape_cycle();
    assert!(m.morphing, "strong beat should trigger early transition");
}

#[test]
fn test_morph_cooldown_prevents_rapid_retrigger() {
    let mut m = Milkdrop::new();
    // Trigger a transition
    m.cycle_timer = 10.0;
    m.update_shape_cycle();
    assert!(m.morphing);

    // Complete the morph
    m.morph_t = 1.0;
    m.update_shape_cycle();
    assert!(!m.morphing);
    assert_eq!(m.shape_index, 1);

    // Immediately try beat trigger — should be blocked by cooldown
    m.beat_envelope = 0.9;
    m.cycle_timer = 0.5; // well within cooldown
    m.update_shape_cycle();
    assert!(!m.morphing, "cooldown should prevent re-trigger");
}

#[test]
fn test_morph_completes_and_snaps() {
    let mut m = Milkdrop::new();
    m.cycle_timer = 10.0;
    m.update_shape_cycle(); // start morph
    assert!(m.morphing);
    assert_eq!(m.next_shape_index, 1);

    // Push morph_t past 1.0
    m.morph_t = 1.05;
    m.update_shape_cycle();
    assert!(!m.morphing, "morph should complete");
    assert_eq!(m.shape_index, 1, "should snap to next shape");
}

#[test]
fn test_morph_wraps_around_shape_list() {
    let mut m = Milkdrop::new();
    m.shape_index = SHAPE_PRESETS.len() - 1; // last shape
    m.cycle_timer = 10.0;
    m.update_shape_cycle();
    assert_eq!(m.next_shape_index, 0, "should wrap to first shape");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib milkdrop::tests -v`
Expected: FAIL — fields and `update_shape_cycle` don't exist yet.

**Step 3: Add morph state fields to struct and new()**

Add these fields to `pub struct Milkdrop` after the `// Spectrum data` section (after line 51):

```rust
    // Shape cycling state
    shape_index: usize,
    next_shape_index: usize,
    morph_t: f32,
    morphing: bool,
    morph_speed: f32,
    cycle_timer: f32,
    cycle_interval: f32,
    morph_cooldown: f32,
    polygon_sides: u8,
```

Add corresponding defaults in `new()` after `waveform_data: Vec::new(),`:

```rust
            shape_index: 0,
            next_shape_index: 0,
            morph_t: 0.0,
            morphing: false,
            morph_speed: 0.02,
            cycle_timer: 0.0,
            cycle_interval: 10.0,
            morph_cooldown: 0.0,
            polygon_sides: 6,
```

**Step 4: Add `update_shape_cycle` method**

Add as a new method on `impl Milkdrop`, before `paint_waveform`:

```rust
    fn update_shape_cycle(&mut self) {
        use crate::visualizations::render::smoothstep;
        let dt = 0.016_f32; // ~60fps frame time

        if self.morphing {
            self.morph_t += self.morph_speed;
            if self.morph_t >= 1.0 {
                // Morph complete — snap to target
                self.shape_index = self.next_shape_index;
                self.morph_t = 0.0;
                self.morphing = false;
                self.morph_cooldown = 3.0; // 3 second cooldown
            }
        } else {
            self.cycle_timer += dt;
            self.morph_cooldown = (self.morph_cooldown - dt).max(0.0);

            let should_trigger = self.cycle_timer >= self.cycle_interval
                || (self.beat_envelope > 0.7 && self.morph_cooldown <= 0.0);

            if should_trigger {
                self.next_shape_index = (self.shape_index + 1) % SHAPE_PRESETS.len();
                self.morph_t = 0.0;
                self.morphing = true;
                self.cycle_timer = 0.0;

                // Pick random polygon sides when polygon is the target
                if SHAPE_PRESETS[self.next_shape_index].name == "polygon" {
                    // Deterministic pseudo-random from time: 3 + (time_bits % 4) -> 3..=6
                    self.polygon_sides = 3 + ((self.time * 1000.0) as u8 % 4);
                }
            }
        }
    }
```

**Step 5: Wire into update_transforms**

At the end of `update_transforms` (after the time advance on line 158), add:

```rust
        self.update_shape_cycle();
```

**Step 6: Run tests to verify they pass**

Run: `cargo test --lib milkdrop::tests -v`
Expected: All tests PASS (original 5 + new 5).

**Step 7: Run full test suite and lint**

Run: `cargo clippy && cargo test --lib`
Expected: Clean.

**Step 8: Commit**

```bash
git add src/visualizations/milkdrop.rs
git commit -m "feat(milkdrop): add shape morph state machine with hybrid cycling

Timer-based auto-cycling (10s) with beat-triggered early transitions
and 3-second cooldown to prevent rapid re-triggers."
```

---

### Task 3: Refactor paint_shapes to use morph interpolation

**Files:**
- Modify: `src/visualizations/milkdrop.rs:210-250` — replace `paint_shapes` body

**Step 1: Write unit test for interpolated rendering**

Add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_paint_shapes_mid_morph_no_panic() {
    let mut m = Milkdrop::new();
    m.morphing = true;
    m.shape_index = 0;
    m.next_shape_index = 2; // circle -> star
    m.morph_t = 0.5;
    m.spectrum = vec![0.5; 128];

    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.3; 1024],
        peak: 0.7,
        rms: 0.5,
        ..Default::default()
    };
    m.update(&frame);

    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    m.render(area, &mut buf);
    // Should not panic — rendering with interpolated shapes works
}

#[test]
fn test_shapes_survive_full_cycle() {
    let mut m = Milkdrop::new();
    let frame = FrameData {
        spectrum: vec![0.6; 128],
        waveform: vec![0.3; 1024],
        peak: 0.7,
        rms: 0.5,
        ..Default::default()
    };
    let area = Rect::new(0, 0, 40, 20);

    // Run enough frames to cycle through all shapes
    for _ in 0..5000 {
        m.update(&frame);
        let mut buf = Buffer::empty(area);
        m.render(area, &mut buf);
    }
    // Should survive without panic through multiple full cycles
}
```

**Step 2: Run tests to verify the new tests pass (existing code won't panic, but morph won't be visible)**

Run: `cargo test --lib milkdrop::tests -v`
Expected: PASS (current code doesn't panic, just doesn't morph visually yet).

**Step 3: Refactor paint_shapes**

Replace the entire `paint_shapes` method body (lines 210-250) with:

```rust
    fn paint_shapes(&mut self) {
        if !self.shapes_enabled || self.spectrum.is_empty() {
            return;
        }
        let pw = self.feedback.pixel_width();
        let ph = self.feedback.pixel_height();
        if pw == 0 || ph == 0 {
            return;
        }

        use crate::visualizations::render::{lerp, smoothstep};

        let cx = pw as f32 / 2.0;
        let cy = ph as f32 / 2.0;
        let base_radius = (pw.min(ph) as f32) * 0.15;
        let r = self.reactivity;
        let audio_scale = 1.0 + self.bass * 2.0 * r + self.beat_envelope * 0.5 * r;

        let preset_a = &SHAPE_PRESETS[self.shape_index];
        let preset_b = &SHAPE_PRESETS[self.next_shape_index];
        let blend = if self.morphing { smoothstep(self.morph_t) } else { 0.0 };

        let eff_radius_scale = lerp(preset_a.base_radius_scale, preset_b.base_radius_scale, blend);
        let eff_brightness_base = lerp(preset_a.brightness, preset_b.brightness, blend);
        let eff_hue_offset = lerp(preset_a.hue_offset, preset_b.hue_offset, blend);

        let num_dots = self.spectrum.len().min(64);
        for i in 0..num_dots {
            let t = i as f32 / num_dots as f32;
            let angle = t * 2.0 * PI + self.time * 0.5;

            // Evaluate both shapes' polar functions
            let r_a = self.eval_shape(self.shape_index, t);
            let r_b = self.eval_shape(self.next_shape_index, t);
            let shape_r = lerp(r_a, r_b, blend);

            let mag = self.spectrum.get(i).copied().unwrap_or(0.0);
            let dot_r = base_radius * eff_radius_scale * audio_scale * shape_r * (0.5 + mag * 0.5);
            let x = (cx + dot_r * SIN_LUT.get(angle + PI / 2.0)).round() as usize;
            let y = (cy + dot_r * SIN_LUT.get(angle)).round() as usize;

            let color = self.palette.color((t + eff_hue_offset).fract());
            let (cr, cg, cb) = match color {
                ratatui::style::Color::Rgb(r, g, b) => {
                    (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
                }
                _ => (1.0, 1.0, 1.0),
            };
            let brightness = eff_brightness_base * (0.4 + mag * 0.6);
            self.feedback.paint(
                x,
                y,
                (cr * brightness, cg * brightness, cb * brightness),
                BlendMode::Additive,
            );
        }
    }

    /// Evaluate a shape's polar function, handling polygon's variable side count.
    fn eval_shape(&self, index: usize, t: f32) -> f32 {
        let preset = &SHAPE_PRESETS[index];
        if preset.name == "polygon" {
            shape_polygon(t, self.time, self.polygon_sides)
        } else {
            (preset.radius_fn)(t, self.time)
        }
    }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib milkdrop::tests -v && cargo test --test milkdrop_test -v`
Expected: All unit + integration tests PASS.

**Step 5: Lint**

Run: `cargo clippy`
Expected: Clean.

**Step 6: Commit**

```bash
git add src/visualizations/milkdrop.rs
git commit -m "feat(milkdrop): refactor paint_shapes for morph interpolation

Shapes now smoothly interpolate position, radius, brightness, and
hue offset between current and next preset using smoothstep easing."
```

---

### Task 4: Add keyboard controls for manual shape cycling

**Files:**
- Modify: `src/visualizations/milkdrop.rs` — `help_keys()` (line 378), `on_key()` (line 392)

**Step 1: Add `n`/`N` to help_keys**

In `help_keys`, add after the `("p/P", "palette")` entry (line 389):

```rust
            ("n/N", "next/prev shape"),
```

**Step 2: Add key handlers in on_key**

Add before the `_ => false` arm (line 472), after the `KeyCode::Char('P')` block:

```rust
            // Manual shape cycling
            KeyCode::Char('n') => {
                if !self.morphing {
                    self.next_shape_index = (self.shape_index + 1) % SHAPE_PRESETS.len();
                    self.morph_t = 0.0;
                    self.morphing = true;
                    self.cycle_timer = 0.0;
                    if SHAPE_PRESETS[self.next_shape_index].name == "polygon" {
                        self.polygon_sides = 3 + ((self.time * 1000.0) as u8 % 4);
                    }
                }
                true
            }
            KeyCode::Char('N') => {
                if !self.morphing {
                    self.next_shape_index = if self.shape_index == 0 {
                        SHAPE_PRESETS.len() - 1
                    } else {
                        self.shape_index - 1
                    };
                    self.morph_t = 0.0;
                    self.morphing = true;
                    self.cycle_timer = 0.0;
                    if SHAPE_PRESETS[self.next_shape_index].name == "polygon" {
                        self.polygon_sides = 3 + ((self.time * 1000.0) as u8 % 4);
                    }
                }
                true
            }
```

**Step 3: Run all tests and lint**

Run: `cargo clippy && cargo test --lib && cargo test --test milkdrop_test`
Expected: All pass, no warnings.

**Step 4: Commit**

```bash
git add src/visualizations/milkdrop.rs
git commit -m "feat(milkdrop): add n/N keybinds for manual shape cycling

Press n for next shape, N for previous. Ignored during an active morph."
```

---

### Task 5: Integration test for shape cycling through multiple transitions

**Files:**
- Modify: `tests/milkdrop_test.rs`

**Step 1: Add integration test**

Append to `tests/milkdrop_test.rs`:

```rust
#[test]
fn test_milkdrop_shape_cycling_no_panic() {
    let mut viz = Milkdrop::new();
    let area = Rect::new(0, 0, 60, 30);

    let loud_frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![0.5; 1024],
        peak: 0.9,
        rms: 0.7,
        ..Default::default()
    };

    // Run many frames — enough to cycle through all 5 shapes multiple times
    // Each shape cycles at ~10s (~600 frames at 60fps), so 5000 frames ≈ 8+ full cycles
    for _ in 0..5000 {
        viz.update(&loud_frame);
        let mut buf = Buffer::empty(area);
        viz.render(area, &mut buf);
    }
}

#[test]
fn test_milkdrop_shape_cycling_with_beats() {
    use terminal_vibes::processing::BeatData;

    let mut viz = Milkdrop::new();
    let area = Rect::new(0, 0, 40, 20);

    // Alternate between silence and loud beats to trigger beat-driven transitions
    for i in 0..2000 {
        let beat_envelope = if i % 100 < 10 { 0.9 } else { 0.1 };
        let frame = FrameData {
            spectrum: vec![0.5; 128],
            waveform: vec![0.3; 1024],
            peak: 0.7,
            rms: 0.5,
            beat: BeatData {
                envelope: beat_envelope,
                ..Default::default()
            },
            ..Default::default()
        };
        viz.update(&frame);
        let mut buf = Buffer::empty(area);
        viz.render(area, &mut buf);
    }
}
```

**Step 2: Run integration tests**

Run: `cargo test --test milkdrop_test -v`
Expected: All tests PASS (original 5 + new 2).

**Step 3: Run full test suite**

Run: `cargo test --lib && cargo test --test milkdrop_test`
Expected: Everything green.

**Step 4: Commit**

```bash
git add tests/milkdrop_test.rs
git commit -m "test(milkdrop): add integration tests for shape cycling

Verifies no panics through multiple full shape cycles, including
beat-triggered transitions."
```

---

### Task 6: Final lint, format, and full test pass

**Files:** None — verification only.

**Step 1: Format**

Run: `cargo fmt`

**Step 2: Lint**

Run: `cargo clippy`
Expected: No warnings.

**Step 3: Full test suite**

Run: `cargo test`
Expected: All tests pass. (Note: skip `tests/processing_test.rs` if it has the pre-existing compile error — use `cargo test --lib && cargo test --test milkdrop_test` as fallback.)

**Step 4: Commit if fmt changed anything**

```bash
git add -A && git diff --cached --quiet || git commit -m "style: format milkdrop shape cycling code"
```
