# Spectral Flux Beat Detection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the energy-vs-running-average beat detection with a hybrid spectral flux + energy gate algorithm that reliably detects beats in dance music.

**Architecture:** `BandDetector` computes positive spectral flux per-band (frame-to-frame energy increase), thresholds against a median of recent flux values, and gates on minimum energy. Cooldown and envelope are unchanged.

**Tech Stack:** Rust, no new dependencies (median computed inline).

---

### Task 1: Add new config fields

**Files:**
- Modify: `src/beat.rs:45-66`

**Step 1: Add new fields to `BeatDetectionConfig`**

Add three fields after `history_frames`:

```rust
pub struct BeatDetectionConfig {
    pub sensitivity: f32,
    pub variance_scale: f32,
    pub envelope_decay: f32,
    pub cooldown_frames: usize,
    pub history_frames: usize,
    pub flux_sensitivity: f32,
    pub flux_history_frames: usize,
    pub energy_floor: f32,
}
```

Update the `Default` impl to include:

```rust
flux_sensitivity: 2.0,
flux_history_frames: 30,
energy_floor: 0.001,
```

**Step 2: Run tests to verify nothing breaks**

Run: `cargo test --lib beat`
Expected: All 8 existing tests PASS (new fields have defaults, no behavior change yet).

**Step 3: Commit**

```bash
git add src/beat.rs
git commit -m "feat(beat): add spectral flux config fields"
```

---

### Task 2: Rewrite BandDetector to use spectral flux

**Files:**
- Modify: `src/beat.rs:68-128`

**Step 1: Write failing test for steady periodic beat detection**

Add this test at the bottom of the `mod tests` block in `src/beat.rs`:

```rust
#[test]
fn test_steady_periodic_beats_detected_consistently() {
    let mut detector = BeatDetector::new(128, make_config());
    let quiet = vec![0.05_f32; 128];
    let mut kick = vec![0.05_f32; 128];
    for i in 0..37 {
        kick[i] = 0.8;
    }

    // Warm up
    for _ in 0..60 {
        detector.analyze(&quiet);
    }

    // Simulate 4-on-the-floor at ~128 BPM (28 frames per beat at 60Hz)
    // 8 beats total
    let mut beats_detected = 0;
    for beat_num in 0..8 {
        // Kick frame
        let result = detector.analyze(&kick);
        if result.bass_beat {
            beats_detected += 1;
        }
        // Gap frames (27 quiet frames between kicks)
        for _ in 0..27 {
            detector.analyze(&quiet);
        }
    }

    assert!(
        beats_detected >= 6,
        "Should detect at least 6 of 8 steady beats, got {}",
        beats_detected
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib beat::tests::test_steady_periodic_beats_detected_consistently`
Expected: FAIL (current algorithm misses beats due to rising average).

**Step 3: Write failing test for energy gate**

```rust
#[test]
fn test_energy_gate_prevents_beats_on_near_silence() {
    let mut detector = BeatDetector::new(128, make_config());
    // Very quiet signal with tiny variations
    let silence = vec![0.0001_f32; 128];
    let mut tiny_blip = vec![0.0001_f32; 128];
    // A small blip but still essentially silence
    for i in 0..37 {
        tiny_blip[i] = 0.005;
    }

    // Warm up with silence
    for _ in 0..60 {
        detector.analyze(&silence);
    }

    // Send blips — should NOT trigger beats due to energy floor
    let mut false_beats = 0;
    for _ in 0..20 {
        let result = detector.analyze(&tiny_blip);
        if result.bass_beat {
            false_beats += 1;
        }
        for _ in 0..10 {
            detector.analyze(&silence);
        }
    }

    assert!(
        false_beats == 0,
        "Energy gate should prevent beats on near-silence, got {} false beats",
        false_beats
    );
}
```

**Step 4: Run test to verify it fails (or passes — either way note the result)**

Run: `cargo test --lib beat::tests::test_energy_gate_prevents_beats_on_near_silence`

**Step 5: Rewrite `BandDetector` with spectral flux**

Replace the `BandDetector` struct and impl (lines 68-128) with:

```rust
struct BandDetector {
    previous_bins: Vec<f32>,
    flux_history: Vec<f32>,
    flux_pos: usize,
    flux_len: usize,
    cooldown_remaining: usize,
    envelope: f32,
}

impl BandDetector {
    fn new(flux_history_frames: usize) -> Self {
        Self {
            previous_bins: Vec::new(),
            flux_history: vec![0.0; flux_history_frames],
            flux_pos: 0,
            flux_len: 0,
            cooldown_remaining: 0,
            envelope: 0.0,
        }
    }

    fn compute_energy(bins: &[f32]) -> f32 {
        if bins.is_empty() {
            return 0.0;
        }
        let sum: f32 = bins.iter().map(|&v| v * v).sum();
        sum / bins.len() as f32
    }

    fn compute_flux(current: &[f32], previous: &[f32]) -> f32 {
        if previous.is_empty() {
            return 0.0;
        }
        current
            .iter()
            .zip(previous.iter())
            .map(|(&c, &p)| (c - p).max(0.0))
            .sum()
    }

    fn median(values: &[f32], len: usize) -> f32 {
        if len == 0 {
            return 0.0;
        }
        let mut sorted: Vec<f32> = values[..len].to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if len % 2 == 0 {
            (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
        } else {
            sorted[len / 2]
        }
    }

    fn analyze(&mut self, bins: &[f32], config: &BeatDetectionConfig) -> (bool, f32, f32) {
        let energy = Self::compute_energy(bins);
        let flux = Self::compute_flux(bins, &self.previous_bins);

        // Store current bins for next frame
        self.previous_bins.clear();
        self.previous_bins.extend_from_slice(bins);

        // Update flux history ring buffer
        let capacity = self.flux_history.len();
        self.flux_history[self.flux_pos] = flux;
        self.flux_pos = (self.flux_pos + 1) % capacity;
        if self.flux_len < capacity {
            self.flux_len += 1;
        }

        // Threshold: median of flux history * sensitivity
        let median_flux = Self::median(&self.flux_history, self.flux_len);
        let threshold = median_flux * config.flux_sensitivity;

        // Beat: flux exceeds threshold AND energy exceeds floor
        let beat = if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
            false
        } else if flux > threshold && flux > 1e-6 && energy > config.energy_floor {
            self.cooldown_remaining = config.cooldown_frames;
            true
        } else {
            false
        };

        if beat {
            self.envelope = 1.0;
        } else {
            self.envelope *= config.envelope_decay;
        }

        (beat, self.envelope, energy)
    }
}
```

**Step 6: Update `BeatDetector` to use new `BandDetector` signature**

Update `BeatDetector::new` to use `flux_history_frames`:

```rust
Self {
    bass: BandDetector::new(config.flux_history_frames),
    mid: BandDetector::new(config.flux_history_frames),
    treble: BandDetector::new(config.flux_history_frames),
    bass_end,
    mid_end,
    config,
}
```

Update `BeatDetector::analyze` to pass bin slices and receive energy back:

```rust
pub fn analyze(&mut self, spectrum: &[f32]) -> BeatData {
    let num_bands = spectrum.len();
    if num_bands == 0 {
        return BeatData::default();
    }

    let bass_end = self.bass_end.min(num_bands);
    let mid_end = self.mid_end.min(num_bands);

    let (bass_beat, bass_envelope, bass_energy) =
        self.bass.analyze(&spectrum[..bass_end], &self.config);
    let (mid_beat, mid_envelope, mid_energy) =
        self.mid.analyze(&spectrum[bass_end..mid_end], &self.config);
    let (treble_beat, treble_envelope, treble_energy) =
        self.treble.analyze(&spectrum[mid_end..], &self.config);

    let beat = bass_beat || mid_beat || treble_beat;
    let envelope = bass_envelope.max(mid_envelope).max(treble_envelope);

    BeatData {
        bass_energy,
        mid_energy,
        treble_energy,
        bass_beat,
        mid_beat,
        treble_beat,
        bass_envelope,
        mid_envelope,
        treble_envelope,
        beat,
        envelope,
    }
}
```

**Step 7: Run all beat tests**

Run: `cargo test --lib beat`
Expected: All tests PASS including the two new ones.

**Step 8: Commit**

```bash
git add src/beat.rs
git commit -m "feat(beat): replace energy-vs-average with spectral flux + energy gate"
```

---

### Task 3: Remove dead code

**Files:**
- Modify: `src/beat.rs`

**Step 1: Remove unused config fields**

Remove `sensitivity`, `variance_scale`, and `history_frames` from `BeatDetectionConfig` and its `Default` impl since they are no longer used by the algorithm.

**Step 2: Remove `#![allow(dead_code)]` from top of file**

Line 1 — delete it.

**Step 3: Run full test suite and clippy**

Run: `cargo test --lib beat && cargo clippy`
Expected: All tests PASS, no clippy warnings.

**Step 4: Commit**

```bash
git add src/beat.rs
git commit -m "chore(beat): remove dead config fields and dead_code allow"
```

---

### Task 4: Final verification

**Step 1: Run full project tests**

Run: `cargo test --lib`
Expected: All tests PASS.

**Step 2: Run clippy and fmt**

Run: `cargo clippy && cargo fmt --check`
Expected: Clean.

**Step 3: Build release**

Run: `cargo build --release`
Expected: Compiles successfully.
