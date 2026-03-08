# Beat Detection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Milkdrop-style beat detection service to the processing pipeline that enriches `FrameData` with per-band beat detection, adaptive thresholds, and attack/decay envelopes.

**Architecture:** New `beat.rs` module defines `BeatData` (output struct) and `BeatDetector` (stateful analyzer). The `Processor` in `processing.rs` gains a `BeatData` field on `FrameData`. The thread loop in `main.rs` creates a `BeatDetector` and calls it after `processor.process()`, attaching the result to `FrameData` before sending down the channel.

**Tech Stack:** Rust, no new dependencies (uses existing `Vec`-based ring buffers for history)

**Design doc:** `docs/plans/2026-03-07-beat-detection-design.md`

---

### Task 1: BeatData Struct and FrameData Integration

**Files:**
- Create: `src/beat.rs`
- Modify: `src/processing.rs:6-12` (add `beat` field to `FrameData`)
- Modify: `src/lib.rs:1-5` (add `pub mod beat`)
- Test: `tests/processing_test.rs` (update `FrameData::default()` test)

**Step 1: Create `src/beat.rs` with `BeatData` struct**

```rust
/// Beat detection output data, attached to each `FrameData`.
#[derive(Debug, Clone)]
pub struct BeatData {
    /// Per-band instant energy (0.0..1.0 normalized)
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,

    /// Per-band beat detected this frame
    pub bass_beat: bool,
    pub mid_beat: bool,
    pub treble_beat: bool,

    /// Per-band envelope (0.0..1.0) — fast attack, smooth decay
    pub bass_envelope: f32,
    pub mid_envelope: f32,
    pub treble_envelope: f32,

    /// Overall beat (any band fired)
    pub beat: bool,
    /// Overall envelope (max of all bands)
    pub envelope: f32,
}

impl Default for BeatData {
    fn default() -> Self {
        Self {
            bass_energy: 0.0,
            mid_energy: 0.0,
            treble_energy: 0.0,
            bass_beat: false,
            mid_beat: false,
            treble_beat: false,
            bass_envelope: 0.0,
            mid_envelope: 0.0,
            treble_envelope: 0.0,
            beat: false,
            envelope: 0.0,
        }
    }
}
```

**Step 2: Add `pub mod beat;` to `src/lib.rs`**

Add `pub mod beat;` to the module declarations in `src/lib.rs`.

**Step 3: Add `beat` field to `FrameData` in `src/processing.rs`**

Add `use crate::beat::BeatData;` at the top, then add `pub beat: BeatData` to the `FrameData` struct. Update the `process()` return to include `beat: BeatData::default()`.

**Step 4: Add `mod beat;` to `src/main.rs`**

Add `mod beat;` to the module declarations in `src/main.rs`.

**Step 5: Update the `FrameData::default()` test in `tests/processing_test.rs`**

The existing `test_frame_data_default_is_empty` test should still pass since `BeatData::default()` is all zeros/false. Verify nothing broke.

**Step 6: Run all tests to verify nothing broke**

Run: `cargo test`
Expected: All existing tests pass (BeatData default is inert).

**Step 7: Commit**

```bash
git add src/beat.rs src/lib.rs src/processing.rs src/main.rs tests/processing_test.rs
git commit -m "feat: add BeatData struct and integrate into FrameData"
```

---

### Task 2: BeatDetector Core — Band Energy Calculation

**Files:**
- Modify: `src/beat.rs` (add `BeatDetector` struct, `BandDetector`, energy computation)
- Test: `src/beat.rs` (inline unit tests)

**Step 1: Write failing tests for band energy calculation**

Add to the bottom of `src/beat.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> BeatDetectionConfig {
        BeatDetectionConfig::default()
    }

    #[test]
    fn test_silence_produces_zero_energy() {
        let mut detector = BeatDetector::new(128, make_config());
        let spectrum = vec![0.0_f32; 128];
        let beat = detector.analyze(&spectrum);
        assert_eq!(beat.bass_energy, 0.0);
        assert_eq!(beat.mid_energy, 0.0);
        assert_eq!(beat.treble_energy, 0.0);
    }

    #[test]
    fn test_bass_energy_from_low_bands() {
        let mut detector = BeatDetector::new(128, make_config());
        let mut spectrum = vec![0.0_f32; 128];
        // Set bass region to loud
        for i in 0..41 {
            spectrum[i] = 0.8;
        }
        let beat = detector.analyze(&spectrum);
        assert!(beat.bass_energy > 0.5, "Bass energy should be high, got {}", beat.bass_energy);
        assert!(beat.mid_energy < 0.01, "Mid energy should be ~zero, got {}", beat.mid_energy);
        assert!(beat.treble_energy < 0.01, "Treble energy should be ~zero, got {}", beat.treble_energy);
    }

    #[test]
    fn test_treble_energy_from_high_bands() {
        let mut detector = BeatDetector::new(128, make_config());
        let mut spectrum = vec![0.0_f32; 128];
        // Set treble region to loud
        for i in 97..128 {
            spectrum[i] = 0.8;
        }
        let beat = detector.analyze(&spectrum);
        assert!(beat.treble_energy > 0.3, "Treble energy should be high, got {}", beat.treble_energy);
        assert!(beat.bass_energy < 0.01, "Bass energy should be ~zero, got {}", beat.bass_energy);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib beat`
Expected: FAIL — `BeatDetector` and `BeatDetectionConfig` do not exist yet.

**Step 3: Implement `BeatDetectionConfig`, `BandDetector`, and `BeatDetector` skeleton**

Add to `src/beat.rs` above the tests:

```rust
/// Configuration for beat detection.
#[derive(Debug, Clone)]
pub struct BeatDetectionConfig {
    /// Multiplier over average energy to trigger a beat (default 1.4)
    pub sensitivity: f32,
    /// How much energy variance raises the threshold (default 1.0)
    pub variance_scale: f32,
    /// Per-frame envelope decay rate (default 0.95)
    pub envelope_decay: f32,
    /// Minimum frames between beats per band (default 6, ~100ms at 60Hz)
    pub cooldown_frames: usize,
    /// Number of history frames for adaptive threshold (default 43, ~0.7s at 60Hz)
    pub history_frames: usize,
}

impl Default for BeatDetectionConfig {
    fn default() -> Self {
        Self {
            sensitivity: 1.4,
            variance_scale: 1.0,
            envelope_decay: 0.95,
            cooldown_frames: 6,
            history_frames: 43,
        }
    }
}

/// Per-band beat detection state.
struct BandDetector {
    energy_history: Vec<f32>,
    history_pos: usize,
    history_len: usize,
    cooldown_remaining: usize,
    envelope: f32,
}

impl BandDetector {
    fn new(history_frames: usize) -> Self {
        Self {
            energy_history: vec![0.0; history_frames],
            history_pos: 0,
            history_len: 0,
            cooldown_remaining: 0,
            envelope: 0.0,
        }
    }

    /// Compute instant energy for a slice of spectrum bands.
    /// Returns normalized energy (mean of squared values).
    fn compute_energy(bands: &[f32]) -> f32 {
        if bands.is_empty() {
            return 0.0;
        }
        let sum: f32 = bands.iter().map(|&v| v * v).sum();
        sum / bands.len() as f32
    }
}

/// Milkdrop-style beat detector with 3 frequency bands.
pub struct BeatDetector {
    bass: BandDetector,
    mid: BandDetector,
    treble: BandDetector,
    config: BeatDetectionConfig,
    /// Band index where bass ends and mids begin
    bass_end: usize,
    /// Band index where mids end and treble begins
    mid_end: usize,
}

impl BeatDetector {
    /// Create a new beat detector for a given number of spectrum bands.
    ///
    /// Band splits are computed via log-frequency mapping:
    /// - Bass: ~30–200 Hz
    /// - Mids: ~200–4000 Hz
    /// - Treble: ~4000–18000 Hz
    pub fn new(num_bands: usize, config: BeatDetectionConfig) -> Self {
        // Log-frequency band index calculation:
        // band_index(f) = num_bands * ln(f/f_min) / ln(f_max/f_min)
        // f_min ~= 30 Hz, f_max ~= 18000 Hz
        let f_min: f64 = 30.0;
        let f_max: f64 = 18000.0;
        let log_ratio = (f_max / f_min).ln();

        let bass_end = ((num_bands as f64) * (200.0_f64 / f_min).ln() / log_ratio) as usize;
        let mid_end = ((num_bands as f64) * (4000.0_f64 / f_min).ln() / log_ratio) as usize;

        let bass_end = bass_end.clamp(1, num_bands - 2);
        let mid_end = mid_end.clamp(bass_end + 1, num_bands - 1);

        Self {
            bass: BandDetector::new(config.history_frames),
            mid: BandDetector::new(config.history_frames),
            treble: BandDetector::new(config.history_frames),
            bass_end,
            mid_end,
            config,
        }
    }

    /// Analyze a spectrum frame and return beat detection results.
    pub fn analyze(&mut self, spectrum: &[f32]) -> BeatData {
        let num_bands = spectrum.len();
        if num_bands == 0 {
            return BeatData::default();
        }

        let bass_end = self.bass_end.min(num_bands);
        let mid_end = self.mid_end.min(num_bands);

        let bass_energy = BandDetector::compute_energy(&spectrum[..bass_end]);
        let mid_energy = BandDetector::compute_energy(&spectrum[bass_end..mid_end]);
        let treble_energy = BandDetector::compute_energy(&spectrum[mid_end..]);

        BeatData {
            bass_energy,
            mid_energy,
            treble_energy,
            // Beat detection and envelopes will be added in Task 3
            bass_beat: false,
            mid_beat: false,
            treble_beat: false,
            bass_envelope: 0.0,
            mid_envelope: 0.0,
            treble_envelope: 0.0,
            beat: false,
            envelope: 0.0,
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib beat`
Expected: All 3 tests PASS.

**Step 5: Commit**

```bash
git add src/beat.rs
git commit -m "feat: add BeatDetector with band energy calculation"
```

---

### Task 3: Adaptive Beat Detection and Envelope Follower

**Files:**
- Modify: `src/beat.rs` (complete `BandDetector` with threshold + envelope logic, wire into `analyze()`)

**Step 1: Write failing tests for beat detection behavior**

Add to the `tests` module in `src/beat.rs`:

```rust
#[test]
fn test_no_beats_on_silence() {
    let mut detector = BeatDetector::new(128, make_config());
    let spectrum = vec![0.0_f32; 128];
    // Feed many silent frames to fill history
    for _ in 0..50 {
        let beat = detector.analyze(&spectrum);
        assert!(!beat.bass_beat, "No beats on silence");
        assert!(!beat.beat, "No overall beat on silence");
    }
}

#[test]
fn test_beat_detected_on_sudden_energy_spike() {
    let mut detector = BeatDetector::new(128, make_config());
    let quiet = vec![0.05_f32; 128];
    let mut loud = vec![0.05_f32; 128];
    // Make bass region very loud
    for i in 0..41 {
        loud[i] = 0.9;
    }

    // Feed quiet frames to establish baseline
    for _ in 0..50 {
        detector.analyze(&quiet);
    }

    // Spike should trigger a bass beat
    let beat = detector.analyze(&loud);
    assert!(beat.bass_beat, "Bass beat should fire on energy spike");
    assert!(beat.beat, "Overall beat should fire");
}

#[test]
fn test_cooldown_prevents_rapid_beats() {
    let config = BeatDetectionConfig {
        cooldown_frames: 6,
        ..BeatDetectionConfig::default()
    };
    let mut detector = BeatDetector::new(128, config);
    let quiet = vec![0.05_f32; 128];
    let mut loud = vec![0.05_f32; 128];
    for i in 0..41 {
        loud[i] = 0.9;
    }

    // Establish baseline
    for _ in 0..50 {
        detector.analyze(&quiet);
    }

    // First spike: should beat
    let beat1 = detector.analyze(&loud);
    assert!(beat1.bass_beat, "First spike should beat");

    // Immediate second spike: cooldown should block it
    // (feed one quiet frame then spike again — still within cooldown)
    detector.analyze(&quiet);
    let beat2 = detector.analyze(&loud);
    assert!(!beat2.bass_beat, "Cooldown should prevent rapid re-trigger");
}

#[test]
fn test_envelope_decays_after_beat() {
    let mut detector = BeatDetector::new(128, make_config());
    let quiet = vec![0.05_f32; 128];
    let mut loud = vec![0.05_f32; 128];
    for i in 0..41 {
        loud[i] = 0.9;
    }

    // Establish baseline
    for _ in 0..50 {
        detector.analyze(&quiet);
    }

    // Trigger beat
    let beat = detector.analyze(&loud);
    assert!(beat.bass_envelope > 0.9, "Envelope should snap to ~1.0 on beat");

    // Decay over several quiet frames
    let mut prev_env = beat.bass_envelope;
    for _ in 0..10 {
        let b = detector.analyze(&quiet);
        assert!(b.bass_envelope < prev_env, "Envelope should decay each frame");
        prev_env = b.bass_envelope;
    }
    assert!(prev_env < 0.7, "Envelope should have decayed significantly after 10 frames");
}

#[test]
fn test_overall_envelope_is_max_of_bands() {
    let mut detector = BeatDetector::new(128, make_config());
    let quiet = vec![0.05_f32; 128];
    let mut loud_treble = vec![0.05_f32; 128];
    for i in 97..128 {
        loud_treble[i] = 0.9;
    }

    for _ in 0..50 {
        detector.analyze(&quiet);
    }

    let beat = detector.analyze(&loud_treble);
    assert!(
        (beat.envelope - beat.treble_envelope).abs() < 0.01
            || beat.envelope >= beat.treble_envelope,
        "Overall envelope should be >= treble envelope"
    );
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib beat`
Expected: FAIL — beat detection always returns `false` and envelopes are always `0.0`.

**Step 3: Implement adaptive threshold and envelope in `BandDetector`**

Add methods to `BandDetector`:

```rust
impl BandDetector {
    /// Push energy into history ring buffer and check for beat.
    /// Returns (beat_detected, envelope_value).
    fn analyze(&mut self, energy: f32, config: &BeatDetectionConfig) -> (bool, f32) {
        // Update history ring buffer
        let capacity = self.energy_history.len();
        self.energy_history[self.history_pos] = energy;
        self.history_pos = (self.history_pos + 1) % capacity;
        if self.history_len < capacity {
            self.history_len += 1;
        }

        // Compute average and variance from history
        let history = &self.energy_history[..self.history_len];
        let avg = history.iter().sum::<f32>() / history.len() as f32;
        let variance = history.iter().map(|&e| (e - avg).powi(2)).sum::<f32>() / history.len() as f32;

        // Adaptive threshold
        let threshold = avg * config.sensitivity + variance.sqrt() * config.variance_scale;

        // Beat detection with cooldown
        let beat = if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
            false
        } else if energy > threshold && energy > 1e-6 {
            self.cooldown_remaining = config.cooldown_frames;
            true
        } else {
            false
        };

        // Envelope: snap to 1.0 on beat, decay otherwise
        if beat {
            self.envelope = 1.0;
        } else {
            self.envelope *= config.envelope_decay;
        }

        (beat, self.envelope)
    }
}
```

Then update `BeatDetector::analyze()` to call `BandDetector::analyze()`:

```rust
pub fn analyze(&mut self, spectrum: &[f32]) -> BeatData {
    let num_bands = spectrum.len();
    if num_bands == 0 {
        return BeatData::default();
    }

    let bass_end = self.bass_end.min(num_bands);
    let mid_end = self.mid_end.min(num_bands);

    let bass_energy = BandDetector::compute_energy(&spectrum[..bass_end]);
    let mid_energy = BandDetector::compute_energy(&spectrum[bass_end..mid_end]);
    let treble_energy = BandDetector::compute_energy(&spectrum[mid_end..]);

    let (bass_beat, bass_envelope) = self.bass.analyze(bass_energy, &self.config);
    let (mid_beat, mid_envelope) = self.mid.analyze(mid_energy, &self.config);
    let (treble_beat, treble_envelope) = self.treble.analyze(treble_energy, &self.config);

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

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib beat`
Expected: All beat detection tests PASS.

**Step 5: Commit**

```bash
git add src/beat.rs
git commit -m "feat: implement adaptive beat detection and envelope follower"
```

---

### Task 4: Beat Detection Config in TOML

**Files:**
- Modify: `src/config.rs:1-10` (add `BeatDetectionConfig` deserialization, add field to `Config`)
- Modify: `src/beat.rs` (add `serde::Deserialize` derive to `BeatDetectionConfig`)
- Test: `tests/config_test.rs` (verify config parses)

**Step 1: Read `tests/config_test.rs` to understand existing config test patterns**

Check the file to follow the same conventions.

**Step 2: Write a failing test for beat detection config parsing**

Add to `tests/config_test.rs`:

```rust
#[test]
fn test_beat_detection_config_defaults() {
    let config = Config::default();
    assert_eq!(config.beat_detection.sensitivity, 1.4);
    assert_eq!(config.beat_detection.envelope_decay, 0.95);
    assert_eq!(config.beat_detection.cooldown_frames, 6);
    assert_eq!(config.beat_detection.history_frames, 43);
}

#[test]
fn test_beat_detection_config_from_toml() {
    let toml_str = r#"
[beat_detection]
sensitivity = 1.8
envelope_decay = 0.9
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.beat_detection.sensitivity, 1.8);
    assert_eq!(config.beat_detection.envelope_decay, 0.9);
    // Non-specified fields use defaults
    assert_eq!(config.beat_detection.cooldown_frames, 6);
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test --test config_test`
Expected: FAIL — `Config` has no `beat_detection` field.

**Step 4: Add `Deserialize` to `BeatDetectionConfig` and integrate into `Config`**

In `src/beat.rs`, add `use serde::Deserialize;` and `#[derive(Deserialize)]` plus `#[serde(default)]` to `BeatDetectionConfig`.

In `src/config.rs`:
- Add `use crate::beat::BeatDetectionConfig;`
- Add `pub beat_detection: BeatDetectionConfig` to the `Config` struct
- Add `beat_detection: BeatDetectionConfig::default()` to `Config::default()`

**Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests PASS.

**Step 6: Commit**

```bash
git add src/beat.rs src/config.rs tests/config_test.rs
git commit -m "feat: add beat_detection config section with TOML support"
```

---

### Task 5: Thread Integration

**Files:**
- Modify: `src/main.rs:82-117` (create `BeatDetector`, call in processor loop)

**Step 1: Integrate `BeatDetector` into the processor thread loop**

In `src/main.rs`, add the import:

```rust
use beat::{BeatDetector, BeatDetectionConfig};
```

In the processor thread spawn block (around line 84), create the `BeatDetector` alongside the `Processor`:

```rust
let beat_config = BeatDetectionConfig {
    sensitivity: config.beat_detection.sensitivity,
    envelope_decay: config.beat_detection.envelope_decay,
    cooldown_frames: config.beat_detection.cooldown_frames,
    history_frames: config.beat_detection.history_frames,
    ..BeatDetectionConfig::default()
};
let mut beat_detector = BeatDetector::new(128, beat_config);
```

Note: `config.beat_detection` fields need to be cloned/copied before the `move` closure. Extract them before the thread spawn, or clone the beat_detection config. Since `BeatDetectionConfig` fields are all `Copy` types, extract them into a local variable before the closure:

```rust
let beat_detection_config = config.beat_detection.clone();
```

Then inside the closure:

```rust
let mut beat_detector = BeatDetector::new(128, beat_detection_config);
```

Update the frame processing (around line 106):

```rust
let mut frame = processor.process(&accum[..fft_size]);
frame.beat = beat_detector.analyze(&frame.spectrum);
let _ = frame_tx.try_send(frame);
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation.

**Step 3: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

**Step 4: Run clippy**

Run: `cargo clippy`
Expected: No warnings.

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: integrate beat detection into processing thread"
```

---

### Task 6: Integration Test

**Files:**
- Create: `tests/beat_test.rs`

**Step 1: Write integration test**

```rust
use terminal_vibes::beat::{BeatDetector, BeatDetectionConfig};

/// Generate a rhythmic pattern: alternating loud and quiet bass frames.
fn rhythmic_bass_pattern(num_bands: usize, loud: bool) -> Vec<f32> {
    let mut spectrum = vec![0.05; num_bands];
    if loud {
        // ~30-200 Hz region (first ~41 bands of 128)
        for i in 0..41 {
            spectrum[i] = 0.9;
        }
    }
    spectrum
}

#[test]
fn test_rhythmic_pattern_detects_beats() {
    let config = BeatDetectionConfig::default();
    let mut detector = BeatDetector::new(128, config);

    // Warm up with quiet frames
    let quiet = rhythmic_bass_pattern(128, false);
    for _ in 0..50 {
        detector.analyze(&quiet);
    }

    // Simulate 4 beats: loud frame, then 10 quiet frames (well past cooldown)
    let loud = rhythmic_bass_pattern(128, true);
    let mut beats_detected = 0;

    for beat_num in 0..4 {
        let result = detector.analyze(&loud);
        if result.bass_beat {
            beats_detected += 1;
        }

        // Quiet gap between beats
        for _ in 0..10 {
            detector.analyze(&quiet);
        }
    }

    assert!(
        beats_detected >= 3,
        "Should detect at least 3 of 4 rhythmic beats, got {}",
        beats_detected
    );
}

#[test]
fn test_steady_signal_does_not_beat_continuously() {
    let config = BeatDetectionConfig::default();
    let mut detector = BeatDetector::new(128, config);

    // Feed constant loud signal
    let loud = vec![0.8_f32; 128];
    let mut beat_count = 0;

    for _ in 0..100 {
        let result = detector.analyze(&loud);
        if result.beat {
            beat_count += 1;
        }
    }

    // Should only beat a few times at the start, not continuously
    assert!(
        beat_count < 10,
        "Steady signal should not produce continuous beats, got {}",
        beat_count
    );
}
```

**Step 2: Run integration tests**

Run: `cargo test --test beat_test`
Expected: All integration tests PASS.

**Step 3: Run full test suite and clippy**

Run: `cargo test && cargo clippy`
Expected: All tests PASS, no clippy warnings.

**Step 4: Commit**

```bash
git add tests/beat_test.rs
git commit -m "test: add integration tests for beat detection"
```

---

### Task 7: Final Verification

**Step 1: Run `cargo fmt`**

Run: `cargo fmt`
Expected: No formatting changes (or apply them).

**Step 2: Run full test suite**

Run: `cargo test`
Expected: All tests PASS.

**Step 3: Run clippy**

Run: `cargo clippy`
Expected: No warnings.

**Step 4: Manual smoke test**

Run: `cargo run`
Expected: App launches and runs without crashes. Beat detection is active in the background — no visible changes yet since visualizations don't consume `BeatData` yet (that's a follow-up).

**Step 5: Final commit (if fmt made changes)**

```bash
git add -A
git commit -m "style: apply formatting"
```
