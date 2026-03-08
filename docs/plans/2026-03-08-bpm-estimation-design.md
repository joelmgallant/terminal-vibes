# BPM Estimation & Beat Prediction Design

## Goal

Add BPM estimation, tempo-locked visual sync, and beat prediction to the existing spectral flux beat detection system. Serves three purposes: display estimated BPM in the status bar, provide a continuous phase signal for tempo-locked animations, and predict upcoming beats for tighter visual sync.

## Target Use Case

General music (rock, pop, hip-hop, electronic). Tempos 60-200 BPM, mostly steady with occasional changes. Does not target jazz, odd time signatures, or heavy rubato.

## Data Interface

### TempoData Struct

```rust
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
```

### FrameData Extension

`FrameData` gains one new field: `tempo: TempoData`.

### Visualization Consumption

- **Display**: `tempo.bpm` + `tempo.confidence` for status bar (`~128 BPM` vs `128 BPM`)
- **Tempo sync**: `tempo.phase` is a 0.0..1.0 sawtooth locked to the beat grid. Visualizations use it for smooth tempo-locked animations (e.g., pulse = `1.0 - phase`).
- **Beat prediction**: `tempo.predicted_beat` fires 1-2 frames early when confidence is high. Visualizations can check this instead of or alongside `beat.beat`.

## Algorithm: Onset Strength Autocorrelation

### Onset Strength Signal

Each frame, combine per-band spectral flux into a single scalar onset strength: weighted sum of bass/mid/treble flux (bass weighted heavier since kicks are the primary tempo marker). Store in a ring buffer of ~240 frames (~4 seconds at 60Hz).

### Autocorrelation

Every N frames (default 15, ~250ms), autocorrelate the onset strength buffer. Search for peaks in the lag range corresponding to the configured BPM range:
- 200 BPM = 18 frames/beat at 60Hz (min lag)
- 60 BPM = 60 frames/beat at 60Hz (max lag)

Parabolic interpolation around the best peak for sub-frame BPM precision.

### Harmonic Summation (Half/Double Resolution)

For each candidate lag L, sum autocorrelation values at L, L/2, L/3 (and 2L, 3L if in range). Pick the lag with the highest harmonic-summed score. This resolves "is it 60 or 120 BPM" ambiguity.

### Confidence

- Peak autocorrelation value (normalized 0.0..1.0) = raw confidence.
- Hysteresis: `conf = max(new_conf, old_conf * decay)` so brief uncertain sections don't cause flicker.
- BPM only updates when: new confidence exceeds threshold (0.3) AND new BPM is within ~5% of current, OR new confidence significantly exceeds current (regime change / new song).

### Phase Tracking

- Phase accumulator: `phase += (bpm / 60.0) / fps` each frame.
- On confirmed beat event (from spectral flux detector), reset phase to 0.0 — keeps phase locked to actual audio.
- When confidence is high and no beat detected but phase wraps past 1.0, fire `predicted_beat` and reset phase.

### Beat Prediction

- `prediction_strength` config (0.0..1.0, default 0.2).
- `predicted_beat` fires when `phase > 1.0 - (prediction_strength * lookahead_fraction)`.
- At 0.0: prediction off. At 1.0: fires up to ~50ms early.

## Architecture

### New Struct

`TempoEstimator` in `beat.rs`, owned by `BeatDetector`. No new files.

### Data Flow

```
BeatDetector::analyze(spectrum)
    |
    v
BandDetector::analyze() x3  ->  BeatData (unchanged)
    |
    +-- per-band flux values --> TempoEstimator::update(flux, beat_fired)
                                        |
                                        v
                                 TempoData { bpm, confidence, phase, predicted_beat }
```

No changes to threading model, channel topology, or processing pipeline structure. `BeatDetector::analyze()` returns both `BeatData` and `TempoData` (or `FrameData` gains the `tempo` field downstream).

### Config

New fields on `BeatDetectionConfig`:

| Field | Default | Purpose |
|-------|---------|---------|
| `tempo_buffer_frames` | 240 | Autocorrelation window (~4s at 60Hz) |
| `tempo_update_interval` | 15 | Recompute BPM every N frames (~250ms) |
| `tempo_min_bpm` | 60.0 | Lower BPM search bound |
| `tempo_max_bpm` | 200.0 | Upper BPM search bound |
| `tempo_confidence_threshold` | 0.3 | Min confidence to accept a BPM |
| `tempo_hysteresis_decay` | 0.98 | Confidence decay rate between updates |
| `prediction_strength` | 0.2 | 0.0=off, 1.0=aggressive (~50ms lookahead) |

All optional with serde defaults. Existing configs unaffected.

### UI Changes

Status bar in `ui.rs`:

- confidence >= 0.6: `128 BPM`
- confidence 0.3..0.6: `~128 BPM`
- confidence < 0.3: no BPM shown

BPM rounded to nearest integer. No new keybindings.

## Testing

Unit tests with synthetic data (no audio hardware):

1. **Steady tempo detection** — Pulses at 120 BPM, assert BPM within +/-2 and confidence > 0.6
2. **Half/double resolution** — Verify harmonic summation picks correct octave
3. **Tempo change** — 120 BPM -> 140 BPM, assert convergence within ~4 seconds
4. **Silence produces no BPM** — Confidence stays near 0.0, no predicted beats
5. **Phase resets on beat** — Phase near 0.0 after beat event
6. **Phase accumulates between beats** — Phase proportional to elapsed frames
7. **Predicted beat fires near phase wrap** — With high confidence + prediction_strength > 0
8. **Prediction off when confidence low** — No predicted beats at low confidence
9. **Hysteresis holds through brief gap** — BPM persists through short silence
10. **BPM display formatting** — Correct status bar text at each confidence tier

## Out of Scope

- PLL-based tempo tracker (future enhancement on top of this)
- Odd time signature detection (3/4, 5/4, 7/8)
- Beat subdivision (8ths, 16ths) — phase signal implicitly provides this
- Per-visualization tempo config — all share the same TempoData
