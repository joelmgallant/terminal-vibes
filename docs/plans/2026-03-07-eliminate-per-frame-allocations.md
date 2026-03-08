# Eliminate Per-Frame Heap Allocations in Visualizations

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove unnecessary `Vec::clone()` and `Vec::collect()` allocations from the hot path (~60Hz update loop) across 5 visualizations.

**Architecture:** Three optimization patterns applied per visualization type:
1. **Simple buffer reuse** (SpectrumBars, RadialSpectrum, Rain): Replace `self.spectrum = frame.spectrum.clone()` with `resize` + `copy_from_slice` on a pre-allocated buffer.
2. **VecDeque recycling** (Spectrogram): When popping old entries from the front, recycle the Vec's allocation for the new entry instead of cloning.
3. **VecDeque recycling with extend** (Lissajous): Same recycle pattern, but clear and re-fill the curve Vec instead of collecting a new one.

**Tech Stack:** Rust, no new dependencies.

---

### Task 1: SpectrumBars — reuse spectrum buffer

**Files:**
- Modify: `src/visualizations/spectrum.rs:144-145`
- Test: `tests/spectrum_test.rs`

**Step 1: Write a failing test that verifies spectrum buffer reuse**

Add to `tests/spectrum_test.rs`:

```rust
#[test]
fn test_spectrum_bars_reuses_buffer_across_updates() {
    let mut viz = SpectrumBars::new();
    let frame1 = FrameData {
        spectrum: vec![0.1, 0.2, 0.3, 0.4],
        waveform: vec![],
        peak: 0.4,
        rms: 0.2,
        beat: Default::default(),
    };
    viz.update(&frame1);

    // Second update with different values but same length
    let frame2 = FrameData {
        spectrum: vec![0.9, 0.8, 0.7, 0.6],
        waveform: vec![],
        peak: 0.9,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame2);

    // Render should work with updated values (no panic, draws content)
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
```

**Step 2: Run test to verify it passes (this tests existing behavior)**

Run: `cargo test --test spectrum_test test_spectrum_bars_reuses_buffer`
Expected: PASS (clone also works, test validates correctness)

**Step 3: Replace clone with buffer reuse in SpectrumBars::update**

In `src/visualizations/spectrum.rs`, change line 145 from:
```rust
self.spectrum = frame.spectrum.clone();
```
to:
```rust
self.spectrum.resize(frame.spectrum.len(), 0.0);
self.spectrum.copy_from_slice(&frame.spectrum);
```

**Step 4: Run all spectrum tests**

Run: `cargo test --test spectrum_test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/visualizations/spectrum.rs tests/spectrum_test.rs
git commit -m "perf: reuse spectrum buffer in SpectrumBars instead of cloning"
```

---

### Task 2: RadialSpectrum — reuse spectrum buffer

**Files:**
- Modify: `src/visualizations/radial.rs:38-39`
- Test: `tests/radial_test.rs`

**Step 1: Write a failing test**

Add to `tests/radial_test.rs`:

```rust
#[test]
fn test_radial_reuses_buffer_across_updates() {
    let mut viz = RadialSpectrum::new();
    let frame1 = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.5,
        rms: 0.3,
        beat: Default::default(),
    };
    viz.update(&frame1);

    let frame2 = FrameData {
        spectrum: vec![0.9; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        beat: Default::default(),
    };
    viz.update(&frame2);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
```

**Step 2: Run test**

Run: `cargo test --test radial_test test_radial_reuses_buffer`
Expected: PASS

**Step 3: Replace clone with buffer reuse in RadialSpectrum::update**

In `src/visualizations/radial.rs`, change line 39 from:
```rust
self.spectrum = frame.spectrum.clone();
```
to:
```rust
self.spectrum.resize(frame.spectrum.len(), 0.0);
self.spectrum.copy_from_slice(&frame.spectrum);
```

**Step 4: Run all radial tests**

Run: `cargo test --test radial_test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/visualizations/radial.rs tests/radial_test.rs
git commit -m "perf: reuse spectrum buffer in RadialSpectrum instead of cloning"
```

---

### Task 3: Rain — reuse spectrum buffer

**Files:**
- Modify: `src/visualizations/rain.rs:76`
- Test: `tests/rain_test.rs`

**Step 1: Write a failing test**

Add to `tests/rain_test.rs`:

```rust
#[test]
fn test_rain_reuses_buffer_across_updates() {
    let mut viz = Rain::new();
    let frame1 = FrameData {
        spectrum: vec![0.5; 64],
        waveform: vec![],
        peak: 0.5,
        rms: 0.3,
        beat: Default::default(),
    };
    viz.update(&frame1);

    let frame2 = FrameData {
        spectrum: vec![0.9; 64],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        beat: Default::default(),
    };
    viz.update(&frame2);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
```

**Step 2: Run test**

Run: `cargo test --test rain_test test_rain_reuses_buffer`
Expected: PASS

**Step 3: Replace clone with buffer reuse in Rain::update**

In `src/visualizations/rain.rs`, change line 76 from:
```rust
self.spectrum = frame.spectrum.clone();
```
to:
```rust
self.spectrum.resize(frame.spectrum.len(), 0.0);
self.spectrum.copy_from_slice(&frame.spectrum);
```

**Step 4: Run all rain tests**

Run: `cargo test --test rain_test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/visualizations/rain.rs tests/rain_test.rs
git commit -m "perf: reuse spectrum buffer in Rain instead of cloning"
```

---

### Task 4: Spectrogram — recycle VecDeque entries

**Files:**
- Modify: `src/visualizations/spectrogram.rs:31-39`
- Test: `tests/spectrogram_test.rs`

**Step 1: Write a test for buffer recycling**

Add to `tests/spectrogram_test.rs`:

```rust
#[test]
fn test_spectrogram_recycling_across_many_updates() {
    let mut viz = Spectrogram::new(10); // small history for test
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.5,
        rms: 0.3,
        beat: Default::default(),
    };

    // Push more frames than history size to trigger recycling
    for i in 0..30 {
        let mut f = frame.clone();
        f.spectrum[0] = i as f32 / 30.0;
        viz.update(&f);
    }

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
```

**Step 2: Run test**

Run: `cargo test --test spectrogram_test test_spectrogram_recycling`
Expected: PASS

**Step 3: Replace clone with recycle pattern in Spectrogram::update**

In `src/visualizations/spectrogram.rs`, replace the entire `update` method (lines 31-39):

```rust
fn update(&mut self, frame: &FrameData) {
    // Recycle a buffer from the front if at capacity, avoiding allocation
    let mut recycled = if self.history.len() >= self.max_history {
        self.beat_markers.pop_front();
        self.history.pop_front().unwrap()
    } else {
        Vec::new()
    };
    recycled.resize(frame.spectrum.len(), 0.0);
    recycled.copy_from_slice(&frame.spectrum);
    self.history.push_back(recycled);
    self.beat_markers.push_back(frame.beat.beat);
}
```

**Step 4: Run all spectrogram tests**

Run: `cargo test --test spectrogram_test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/visualizations/spectrogram.rs tests/spectrogram_test.rs
git commit -m "perf: recycle VecDeque buffers in Spectrogram instead of cloning"
```

---

### Task 5: Lissajous — recycle trail curve buffers

**Files:**
- Modify: `src/visualizations/lissajous.rs:100-119`
- Test: `tests/lissajous_test.rs`

**Step 1: Write a test for trail recycling**

Add to `tests/lissajous_test.rs`:

```rust
#[test]
fn test_lissajous_recycling_across_many_updates() {
    let mut viz = Lissajous::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 2048],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };

    // Push many frames to trigger trail recycling
    for _ in 0..50 {
        viz.update(&frame);
    }

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);

    // Should still render content after recycling
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)]
                .symbol()
                .chars()
                .next()
                .unwrap_or(' ');
            ch != ' ' && ch != '\u{2800}'
        })
    });
    assert!(has_content, "Lissajous should render content after trail recycling");
}
```

**Step 2: Run test**

Run: `cargo test --test lissajous_test test_lissajous_recycling`
Expected: PASS

**Step 3: Replace collect with recycle pattern in Lissajous::update**

In `src/visualizations/lissajous.rs`, replace lines 100-119 (the curve generation and trail management):

```rust
// Generate the current curve
let (a, b) = self.current_ratio();
// Scale pumps with beat envelope — breathes with the music
let scale = 0.2 + self.peak * 0.4 + self.beat_envelope * 0.4;
let num_points = 500;

let max_trail = 5 + (self.rms * 15.0) as usize; // 5-20 frames based on energy

// Recycle a buffer from the front if at capacity, avoiding allocation
let mut curve = if self.trail.len() >= max_trail {
    let mut c = self.trail.pop_front().unwrap();
    c.clear();
    c
} else {
    Vec::with_capacity(num_points)
};

curve.extend((0..num_points).map(|i| {
    let t = i as f32 / num_points as f32 * 2.0 * PI;
    let x = (a * t + self.phase_x + self.time).sin() * scale;
    let y = (b * t + self.phase_y + self.time * 0.7).sin() * scale;
    (x, y)
}));

self.trail.push_back(curve);
// max_trail can shrink when RMS drops — trim excess
while self.trail.len() > max_trail {
    self.trail.pop_front();
}

self.time += 0.05 + self.rms * 0.05;
```

**Step 4: Run all lissajous tests**

Run: `cargo test --test lissajous_test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/visualizations/lissajous.rs tests/lissajous_test.rs
git commit -m "perf: recycle trail curve buffers in Lissajous instead of allocating"
```

---

### Task 6: Final verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cargo clippy`
Expected: No warnings

**Step 3: Run fmt**

Run: `cargo fmt --check`
Expected: No formatting issues

**Step 4: Squash commit (if needed)**

If all individual commits passed, no squash needed. Clean history.
