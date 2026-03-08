# BPM Estimation & Beat Prediction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add BPM estimation via onset strength autocorrelation, confidence-gated display, phase tracking for tempo-locked visuals, and configurable beat prediction.

**Architecture:** `TempoEstimator` in `beat.rs` accumulates a combined onset strength signal from per-band spectral flux, autocorrelates to find dominant periodicity, tracks phase against the beat grid, and optionally predicts upcoming beats. Owned by `BeatDetector`, runs inline in the processing thread.

**Tech Stack:** Rust, no new dependencies.

---

### Task 1: Add TempoData struct and FrameData field

**Files:**
- Modify: `src/beat.rs:1-44`
- Modify: `src/processing.rs:1-14`

**Step 1: Add `TempoData` struct to `src/beat.rs`**

Add after `BeatData` (after line 44):

```rust
/// Tempo estimation output, attached to each `FrameData`.
#[derive(Debug, Clone)]
pub struct TempoData {
    /// Estimated BPM (0.0 if unknown)
    pub bpm: f32,
    /// Confidence in the estimate (0.0..1.0)
    pub confidence: f32,
    /// Beat phase (0.0..1.0) — 0.0 at beat, rises to 1.0 at next beat
    pub phase: f32,
    /// Predicted beat this frame (phase wrapped past 1.0 with high confidence)
    pub predicted_beat: bool,
}

impl Default for TempoData {
    fn default() -> Self {
        Self {
            bpm: 0.0,
            confidence: 0.0,
            phase: 0.0,
            predicted_beat: false,
        }
    }
}
```

**Step 2: Add `tempo` field to `FrameData` in `src/processing.rs`**

Add `use crate::beat::TempoData;` to the imports (line 1), and add the field to `FrameData` (after line 14):

```rust
use crate::beat::{BeatData, TempoData};

#[derive(Debug, Clone, Default)]
pub struct FrameData {
    pub spectrum: Vec<f32>,
    pub waveform: Vec<f32>,
    pub peak: f32,
    pub rms: f32,
    pub beat: BeatData,
    pub tempo: TempoData,
}
```

**Step 3: Run tests to verify nothing breaks**

Run: `cargo test --lib`
Expected: All existing tests PASS. New types have `Default`, no behavior change.

**Step 4: Commit**

```bash
git add src/beat.rs src/processing.rs
git commit -m "feat(beat): add TempoData struct and wire into FrameData"
```

---

### Task 2: Add tempo config fields

**Files:**
- Modify: `src/beat.rs:46-67`

**Step 1: Add new config fields to `BeatDetectionConfig`**

Replace the struct and Default impl:

```rust
/// Configuration for beat detection.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct BeatDetectionConfig {
    pub envelope_decay: f32,
    pub cooldown_frames: usize,
    pub flux_sensitivity: f32,
    pub flux_history_frames: usize,
    pub energy_floor: f32,
    pub tempo_buffer_frames: usize,
    pub tempo_update_interval: usize,
    pub tempo_min_bpm: f32,
    pub tempo_max_bpm: f32,
    pub tempo_confidence_threshold: f32,
    pub tempo_hysteresis_decay: f32,
    pub prediction_strength: f32,
}

impl Default for BeatDetectionConfig {
    fn default() -> Self {
        Self {
            envelope_decay: 0.95,
            cooldown_frames: 8,
            flux_sensitivity: 3.0,
            flux_history_frames: 30,
            energy_floor: 0.001,
            tempo_buffer_frames: 240,
            tempo_update_interval: 15,
            tempo_min_bpm: 60.0,
            tempo_max_bpm: 200.0,
            tempo_confidence_threshold: 0.3,
            tempo_hysteresis_decay: 0.98,
            prediction_strength: 0.2,
        }
    }
}
```

**Step 2: Run tests to verify nothing breaks**

Run: `cargo test --lib beat`
Expected: All existing tests PASS (new fields have defaults).

**Step 3: Commit**

```bash
git add src/beat.rs
git commit -m "feat(beat): add tempo estimation config fields"
```

---

### Task 3: Implement TempoEstimator core with onset strength buffer

**Files:**
- Modify: `src/beat.rs` (add new struct after `BandDetector` impl, before `BeatDetector`)

**Step 1: Write failing tests for TempoEstimator**

Add to the `mod tests` block at the bottom of `src/beat.rs`:

```rust
    #[test]
    fn test_tempo_silence_produces_no_bpm() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // Feed 300 frames of zero onset strength, no beats
        for _ in 0..300 {
            estimator.update(0.0, false, &config);
        }
        let tempo = estimator.tempo_data();
        assert!(
            tempo.confidence < 0.1,
            "Silence should have near-zero confidence, got {}",
            tempo.confidence
        );
        assert!(
            !tempo.predicted_beat,
            "No predicted beats during silence"
        );
    }

    #[test]
    fn test_tempo_onset_buffer_fills() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // Feed some onset strength values
        for i in 0..50 {
            estimator.update(i as f32 * 0.1, false, &config);
        }
        // Should have accumulated 50 values
        assert_eq!(estimator.onset_len(), 50);
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib beat::tests::test_tempo`
Expected: FAIL — `TempoEstimator` doesn't exist yet.

**Step 3: Implement `TempoEstimator` skeleton**

Add after the `BandDetector` impl (before `pub struct BeatDetector`):

```rust
struct TempoEstimator {
    /// Ring buffer of onset strength values
    onset_buf: Vec<f32>,
    onset_pos: usize,
    onset_len: usize,
    /// Current tempo state
    bpm: f32,
    confidence: f32,
    phase: f32,
    predicted_beat: bool,
    /// Frame counter for update interval
    frame_count: usize,
}

impl TempoEstimator {
    fn new(config: &BeatDetectionConfig) -> Self {
        Self {
            onset_buf: vec![0.0; config.tempo_buffer_frames],
            onset_pos: 0,
            onset_len: 0,
            bpm: 0.0,
            confidence: 0.0,
            phase: 0.0,
            predicted_beat: false,
            frame_count: 0,
        }
    }

    fn onset_len(&self) -> usize {
        self.onset_len
    }

    fn update(&mut self, onset_strength: f32, beat_fired: bool, config: &BeatDetectionConfig) {
        // Push onset strength into ring buffer
        let capacity = self.onset_buf.len();
        self.onset_buf[self.onset_pos] = onset_strength;
        self.onset_pos = (self.onset_pos + 1) % capacity;
        if self.onset_len < capacity {
            self.onset_len += 1;
        }

        self.frame_count += 1;

        // Phase tracking
        if self.bpm > 0.0 {
            // Assume 60 FPS processing rate
            self.phase += (self.bpm / 60.0) / 60.0;
        }

        // Reset phase on confirmed beat
        if beat_fired {
            self.phase = 0.0;
        }

        // Predicted beat when phase wraps
        self.predicted_beat = false;
        if self.confidence >= config.tempo_confidence_threshold && self.phase >= 1.0 {
            if config.prediction_strength > 0.0 {
                self.predicted_beat = true;
            }
            self.phase -= 1.0;
        }

        // Periodically recompute BPM
        if self.frame_count % config.tempo_update_interval == 0 && self.onset_len >= 60 {
            self.estimate_tempo(config);
        }
    }

    fn estimate_tempo(&mut self, config: &BeatDetectionConfig) {
        // Placeholder — Task 4 implements this
        let _ = config;
    }

    fn tempo_data(&self) -> TempoData {
        TempoData {
            bpm: self.bpm,
            confidence: self.confidence,
            phase: self.phase,
            predicted_beat: self.predicted_beat,
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib beat::tests::test_tempo`
Expected: PASS.

**Step 5: Run all beat tests**

Run: `cargo test --lib beat`
Expected: All tests PASS.

**Step 6: Commit**

```bash
git add src/beat.rs
git commit -m "feat(beat): add TempoEstimator skeleton with onset strength buffer"
```

---

### Task 4: Implement autocorrelation and BPM extraction

**Files:**
- Modify: `src/beat.rs` (fill in `estimate_tempo` method, add helper functions)

**Step 1: Write failing test for steady tempo detection**

Add to `mod tests`:

```rust
    #[test]
    fn test_tempo_steady_120_bpm() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // 120 BPM at 60 FPS = beat every 30 frames
        // Simulate 8 seconds (480 frames)
        for frame in 0..480 {
            let is_beat_frame = frame % 30 == 0;
            let onset = if is_beat_frame { 1.0 } else { 0.0 };
            estimator.update(onset, is_beat_frame, &config);
        }
        let tempo = estimator.tempo_data();
        assert!(
            (tempo.bpm - 120.0).abs() < 3.0,
            "Should estimate ~120 BPM, got {}",
            tempo.bpm
        );
        assert!(
            tempo.confidence > 0.5,
            "Should have high confidence, got {}",
            tempo.confidence
        );
    }

    #[test]
    fn test_tempo_steady_140_bpm() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // 140 BPM at 60 FPS = beat every ~25.7 frames
        // Use integer approximation: alternate 25 and 26 frame gaps
        let frames_per_beat = 60.0 / (140.0 / 60.0);
        let mut next_beat = 0.0_f64;
        for frame in 0..600 {
            let is_beat = frame as f64 >= next_beat;
            let onset = if is_beat { 1.0 } else { 0.0 };
            estimator.update(onset, is_beat, &config);
            if is_beat {
                next_beat += frames_per_beat as f64;
            }
        }
        let tempo = estimator.tempo_data();
        assert!(
            (tempo.bpm - 140.0).abs() < 3.0,
            "Should estimate ~140 BPM, got {}",
            tempo.bpm
        );
    }

    #[test]
    fn test_tempo_half_double_resolution() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // 120 BPM (beat every 30 frames) — should NOT report 60 or 240
        for frame in 0..600 {
            let is_beat_frame = frame % 30 == 0;
            let onset = if is_beat_frame { 1.0 } else { 0.0 };
            estimator.update(onset, is_beat_frame, &config);
        }
        let tempo = estimator.tempo_data();
        assert!(
            tempo.bpm > 90.0 && tempo.bpm < 180.0,
            "Should be in 90-180 range (not half/double), got {}",
            tempo.bpm
        );
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib beat::tests::test_tempo_steady`
Expected: FAIL — `estimate_tempo` is a no-op, BPM stays 0.0.

**Step 3: Implement autocorrelation helpers**

Add these as methods on `TempoEstimator`:

```rust
    /// Linearize the ring buffer into a contiguous slice for autocorrelation.
    /// Writes into the provided scratch buffer and returns the filled length.
    fn linearize_onset(&self, scratch: &mut Vec<f32>) -> usize {
        scratch.clear();
        let len = self.onset_len;
        let cap = self.onset_buf.len();
        if len < cap {
            // Buffer hasn't wrapped yet — data is at [0..len]
            scratch.extend_from_slice(&self.onset_buf[..len]);
        } else {
            // Buffer has wrapped — oldest at onset_pos, newest at onset_pos-1
            scratch.extend_from_slice(&self.onset_buf[self.onset_pos..]);
            scratch.extend_from_slice(&self.onset_buf[..self.onset_pos]);
        }
        scratch.len()
    }

    /// Compute normalized autocorrelation at a specific lag.
    fn autocorrelate_at_lag(signal: &[f32], lag: usize) -> f32 {
        let n = signal.len();
        if lag >= n {
            return 0.0;
        }
        let mut sum = 0.0_f64;
        let mut energy = 0.0_f64;
        for i in 0..(n - lag) {
            sum += signal[i] as f64 * signal[i + lag] as f64;
            energy += signal[i] as f64 * signal[i] as f64;
        }
        if energy < 1e-10 {
            return 0.0;
        }
        (sum / energy) as f32
    }

    /// Parabolic interpolation around a peak for sub-sample precision.
    /// Returns (interpolated_lag, interpolated_value).
    fn parabolic_interp(prev: f32, peak: f32, next: f32, peak_lag: usize) -> (f32, f32) {
        let denom = prev - 2.0 * peak + next;
        if denom.abs() < 1e-10 {
            return (peak_lag as f32, peak);
        }
        let offset = 0.5 * (prev - next) / denom;
        let interp_val = peak - 0.25 * (prev - next) * offset;
        (peak_lag as f32 + offset, interp_val)
    }
```

**Step 4: Implement `estimate_tempo`**

Replace the placeholder `estimate_tempo` method:

```rust
    fn estimate_tempo(&mut self, config: &BeatDetectionConfig) {
        let mut scratch = Vec::with_capacity(self.onset_buf.len());
        let len = self.linearize_onset(&mut scratch);
        if len < 60 {
            return;
        }

        // Lag range from BPM bounds (at 60 FPS)
        let fps = 60.0_f32;
        let min_lag = (fps * 60.0 / config.tempo_max_bpm) as usize; // high BPM = short lag
        let max_lag = (fps * 60.0 / config.tempo_min_bpm) as usize; // low BPM = long lag
        let max_lag = max_lag.min(len / 2); // Don't exceed half the buffer

        if min_lag >= max_lag {
            return;
        }

        // Compute autocorrelation with harmonic summation
        let mut best_score = 0.0_f32;
        let mut best_lag = min_lag;
        let mut best_raw = 0.0_f32;

        for lag in min_lag..=max_lag {
            let r = Self::autocorrelate_at_lag(&scratch, lag);

            // Harmonic summation: add energy at lag/2, lag/3 (sub-harmonics help resolve octave)
            let mut harmonic_sum = r;
            let sub2 = lag / 2;
            if sub2 >= min_lag {
                harmonic_sum += 0.5 * Self::autocorrelate_at_lag(&scratch, sub2);
            }
            let sub3 = lag / 3;
            if sub3 >= min_lag {
                harmonic_sum += 0.3 * Self::autocorrelate_at_lag(&scratch, sub3);
            }

            if harmonic_sum > best_score {
                best_score = harmonic_sum;
                best_lag = lag;
                best_raw = r;
            }
        }

        // Parabolic interpolation for sub-frame precision
        let (interp_lag, _interp_val) = if best_lag > min_lag && best_lag < max_lag {
            let prev = Self::autocorrelate_at_lag(&scratch, best_lag - 1);
            let next = Self::autocorrelate_at_lag(&scratch, best_lag + 1);
            Self::parabolic_interp(prev, best_raw, next, best_lag)
        } else {
            (best_lag as f32, best_raw)
        };

        // Convert lag to BPM
        let new_bpm = if interp_lag > 0.0 {
            fps * 60.0 / interp_lag
        } else {
            0.0
        };

        // Confidence from peak autocorrelation (clamped to 0..1)
        let new_confidence = best_raw.clamp(0.0, 1.0);

        // Apply hysteresis
        let decayed_confidence = self.confidence * config.tempo_hysteresis_decay;

        if new_confidence >= config.tempo_confidence_threshold {
            // Accept if within 5% of current, or significantly stronger
            let bpm_close = self.bpm <= 0.0 || (new_bpm - self.bpm).abs() / self.bpm < 0.05;
            let much_stronger = new_confidence > self.confidence * 1.3;

            if bpm_close || much_stronger {
                self.bpm = new_bpm;
                self.confidence = new_confidence;
            } else {
                self.confidence = decayed_confidence.max(new_confidence * 0.5);
            }
        } else {
            self.confidence = decayed_confidence;
        }
    }
```

**Step 5: Run the tempo tests**

Run: `cargo test --lib beat::tests::test_tempo`
Expected: All PASS.

**Step 6: Run all beat tests**

Run: `cargo test --lib beat`
Expected: All tests PASS.

**Step 7: Commit**

```bash
git add src/beat.rs
git commit -m "feat(beat): implement onset autocorrelation BPM estimation"
```

---

### Task 5: Add phase tracking and prediction tests

**Files:**
- Modify: `src/beat.rs` (add tests, may need minor adjustments to phase logic)

**Step 1: Write phase and prediction tests**

Add to `mod tests`:

```rust
    #[test]
    fn test_tempo_phase_resets_on_beat() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // Establish tempo first
        for frame in 0..480 {
            let is_beat = frame % 30 == 0;
            estimator.update(if is_beat { 1.0 } else { 0.0 }, is_beat, &config);
        }
        // Now fire a beat and check phase resets
        estimator.update(1.0, true, &config);
        let tempo = estimator.tempo_data();
        assert!(
            tempo.phase < 0.1,
            "Phase should reset near 0.0 on beat, got {}",
            tempo.phase
        );
    }

    #[test]
    fn test_tempo_phase_accumulates() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // Establish 120 BPM
        for frame in 0..480 {
            let is_beat = frame % 30 == 0;
            estimator.update(if is_beat { 1.0 } else { 0.0 }, is_beat, &config);
        }
        // Reset phase with a beat
        estimator.update(1.0, true, &config);
        // Advance 15 frames (half a beat at 120 BPM / 60 FPS)
        for _ in 0..15 {
            estimator.update(0.0, false, &config);
        }
        let tempo = estimator.tempo_data();
        // At 120 BPM, 60 FPS: phase += (120/60)/60 = 1/30 per frame
        // After 15 frames: phase ~ 0.5
        assert!(
            (tempo.phase - 0.5).abs() < 0.15,
            "Phase should be ~0.5 after half a beat period, got {}",
            tempo.phase
        );
    }

    #[test]
    fn test_tempo_no_prediction_when_confidence_low() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // Don't establish tempo — just feed zeros
        for _ in 0..300 {
            estimator.update(0.0, false, &config);
        }
        let tempo = estimator.tempo_data();
        assert!(
            !tempo.predicted_beat,
            "Should not predict beats with low confidence"
        );
    }

    #[test]
    fn test_tempo_prediction_fires_on_phase_wrap() {
        let mut config = make_config();
        config.prediction_strength = 0.5;
        let mut estimator = TempoEstimator::new(&config);
        // Establish 120 BPM
        for frame in 0..480 {
            let is_beat = frame % 30 == 0;
            estimator.update(if is_beat { 1.0 } else { 0.0 }, is_beat, &config);
        }
        // Fire a beat to reset phase
        estimator.update(1.0, true, &config);
        // Advance close to the next beat (29 frames of 30)
        let mut predicted = false;
        for _ in 0..35 {
            estimator.update(0.0, false, &config);
            if estimator.tempo_data().predicted_beat {
                predicted = true;
            }
        }
        assert!(predicted, "Predicted beat should fire near phase wrap");
    }

    #[test]
    fn test_tempo_hysteresis_holds_through_gap() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // Establish 120 BPM
        for frame in 0..480 {
            let is_beat = frame % 30 == 0;
            estimator.update(if is_beat { 1.0 } else { 0.0 }, is_beat, &config);
        }
        let bpm_before = estimator.tempo_data().bpm;
        // 30 frames of silence (~500ms gap)
        for _ in 0..30 {
            estimator.update(0.0, false, &config);
        }
        let tempo = estimator.tempo_data();
        assert!(
            (tempo.bpm - bpm_before).abs() < 1.0,
            "BPM should hold through brief gap, was {} now {}",
            bpm_before, tempo.bpm
        );
        assert!(
            tempo.confidence > 0.0,
            "Confidence should still be nonzero after gap"
        );
    }
```

**Step 2: Run tests**

Run: `cargo test --lib beat::tests::test_tempo`
Expected: All PASS. If any fail, adjust thresholds or phase logic.

**Step 3: Commit**

```bash
git add src/beat.rs
git commit -m "test(beat): add phase tracking and prediction tests"
```

---

### Task 6: Implement tempo change detection test

**Files:**
- Modify: `src/beat.rs` (add test)

**Step 1: Write tempo change test**

Add to `mod tests`:

```rust
    #[test]
    fn test_tempo_change_converges() {
        let config = make_config();
        let mut estimator = TempoEstimator::new(&config);
        // Establish 120 BPM (beat every 30 frames)
        for frame in 0..480 {
            let is_beat = frame % 30 == 0;
            estimator.update(if is_beat { 1.0 } else { 0.0 }, is_beat, &config);
        }
        let bpm1 = estimator.tempo_data().bpm;
        assert!(
            (bpm1 - 120.0).abs() < 3.0,
            "Should start at ~120, got {}", bpm1
        );

        // Switch to ~90 BPM (beat every 40 frames)
        // Need enough time for buffer to fill with new tempo
        for frame in 0..600 {
            let is_beat = frame % 40 == 0;
            estimator.update(if is_beat { 1.0 } else { 0.0 }, is_beat, &config);
        }
        let bpm2 = estimator.tempo_data().bpm;
        assert!(
            (bpm2 - 90.0).abs() < 5.0,
            "Should converge to ~90, got {}", bpm2
        );
    }
```

**Step 2: Run test**

Run: `cargo test --lib beat::tests::test_tempo_change`
Expected: PASS.

**Step 3: Commit**

```bash
git add src/beat.rs
git commit -m "test(beat): add tempo change convergence test"
```

---

### Task 7: Wire TempoEstimator into BeatDetector

**Files:**
- Modify: `src/beat.rs:122,160-227` (BandDetector return type, BeatDetector struct and analyze)
- Modify: `src/main.rs:130` (call site)

**Step 1: Change `BandDetector::analyze` to also return flux**

In `src/beat.rs`, change the return type of `BandDetector::analyze` from `(bool, f32, f32)` to `(bool, f32, f32, f32)`:

```rust
    fn analyze(&mut self, bins: &[f32], config: &BeatDetectionConfig) -> (bool, f32, f32, f32) {
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

        (beat, self.envelope, energy, flux)
    }
```

**Step 2: Add `TempoEstimator` to `BeatDetector` and update `analyze`**

Update the `BeatDetector` struct to own a `TempoEstimator`:

```rust
pub struct BeatDetector {
    bass: BandDetector,
    mid: BandDetector,
    treble: BandDetector,
    config: BeatDetectionConfig,
    bass_end: usize,
    mid_end: usize,
    tempo: TempoEstimator,
}
```

Update `BeatDetector::new` to create the `TempoEstimator`:

```rust
    pub fn new(num_bands: usize, config: BeatDetectionConfig) -> Self {
        let f_min: f64 = 30.0;
        let f_max: f64 = 18000.0;
        let log_ratio = (f_max / f_min).ln();

        let bass_end = ((num_bands as f64) * (200.0_f64 / f_min).ln() / log_ratio) as usize;
        let mid_end = ((num_bands as f64) * (4000.0_f64 / f_min).ln() / log_ratio) as usize;

        let bass_end = bass_end.clamp(1, num_bands - 2);
        let mid_end = mid_end.clamp(bass_end + 1, num_bands - 1);

        let tempo = TempoEstimator::new(&config);

        Self {
            bass: BandDetector::new(config.flux_history_frames),
            mid: BandDetector::new(config.flux_history_frames),
            treble: BandDetector::new(config.flux_history_frames),
            bass_end,
            mid_end,
            config,
            tempo,
        }
    }
```

Change `BeatDetector::analyze` to return `(BeatData, TempoData)`:

```rust
    pub fn analyze(&mut self, spectrum: &[f32]) -> (BeatData, TempoData) {
        let num_bands = spectrum.len();
        if num_bands == 0 {
            return (BeatData::default(), TempoData::default());
        }

        let bass_end = self.bass_end.min(num_bands);
        let mid_end = self.mid_end.min(num_bands);

        let (bass_beat, bass_envelope, bass_energy, bass_flux) =
            self.bass.analyze(&spectrum[..bass_end], &self.config);
        let (mid_beat, mid_envelope, mid_energy, mid_flux) =
            self.mid.analyze(&spectrum[bass_end..mid_end], &self.config);
        let (treble_beat, treble_envelope, treble_energy, treble_flux) =
            self.treble.analyze(&spectrum[mid_end..], &self.config);

        let beat = bass_beat || mid_beat || treble_beat;
        let envelope = bass_envelope.max(mid_envelope).max(treble_envelope);

        // Combined onset strength: bass-weighted sum of per-band flux
        let onset_strength = bass_flux * 0.6 + mid_flux * 0.25 + treble_flux * 0.15;

        self.tempo.update(onset_strength, beat, &self.config);

        let beat_data = BeatData {
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
        };

        (beat_data, self.tempo.tempo_data())
    }
```

**Step 3: Update call site in `src/main.rs`**

Change line 130 from:

```rust
frame.beat = beat_detector.analyze(&frame.spectrum);
```

to:

```rust
let (beat_data, tempo_data) = beat_detector.analyze(&frame.spectrum);
frame.beat = beat_data;
frame.tempo = tempo_data;
```

**Step 4: Run all tests**

Run: `cargo test --lib`
Expected: All tests PASS.

**Step 5: Run clippy**

Run: `cargo clippy`
Expected: No warnings (unused flux variables are now used).

**Step 6: Commit**

```bash
git add src/beat.rs src/main.rs
git commit -m "feat(beat): wire TempoEstimator into BeatDetector pipeline"
```

---

### Task 8: Add BPM to status bar

**Files:**
- Modify: `src/ui.rs:187-191`

**Step 1: Update status bar format string**

In `src/ui.rs`, find the status bar formatting (around line 187). Change the format string to include BPM:

```rust
                    let bpm_display = if display_frame.tempo.confidence >= 0.6 {
                        format!("{}BPM", display_frame.tempo.bpm.round() as u32)
                    } else if display_frame.tempo.confidence >= 0.3 {
                        format!("~{}BPM", display_frame.tempo.bpm.round() as u32)
                    } else {
                        String::new()
                    };
                    let bpm_section = if bpm_display.is_empty() {
                        String::new()
                    } else {
                        format!("  {}  ", bpm_display)
                    };
                    let status = format!(
                        " [{}]  peak: {:.2}  rms: {:.2}  env: {:.2}  {}{}  sens: {:.1}x  beat: {:.1}x  |  Tab: next  q: quit ",
                        mode_name, display_frame.peak, display_frame.rms,
                        display_frame.beat.envelope, beat_indicator, bpm_section, self.sensitivity, self.beat_intensity,
                    );
```

**Step 2: Add `use crate::beat::TempoData;` if needed**

Check if `TempoData` is accessed through `FrameData` (it is via `display_frame.tempo`), so no new import is needed since `FrameData` already carries it.

**Step 3: Run all tests**

Run: `cargo test --lib`
Expected: All PASS.

**Step 4: Run clippy**

Run: `cargo clippy`
Expected: Clean.

**Step 5: Commit**

```bash
git add src/ui.rs
git commit -m "feat(ui): show BPM in status bar with confidence indicator"
```

---

### Task 9: Final verification

**Step 1: Run full test suite**

Run: `cargo test --lib`
Expected: All tests PASS.

**Step 2: Run clippy and fmt**

Run: `cargo clippy && cargo fmt --check`
Expected: Clean.

**Step 3: Build release**

Run: `cargo build --release`
Expected: Compiles successfully.
