# Milkdrop-Style Beat Detection Design

## Goal

Shared beat detection service in the processing thread that enriches `FrameData` with a `BeatData` struct, enabling all visualizations to react to musical beats without any per-visualization beat analysis code.

## Data Interface

### BeatData Struct

```rust
pub struct BeatData {
    // Per-band instant energy (0.0..1.0 normalized)
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,

    // Per-band beat detected this frame
    pub bass_beat: bool,
    pub mid_beat: bool,
    pub treble_beat: bool,

    // Per-band envelope (0.0..1.0) — fast attack, smooth decay
    pub bass_envelope: f32,
    pub mid_envelope: f32,
    pub treble_envelope: f32,

    // Overall beat (any band fired)
    pub beat: bool,
    // Overall envelope (max of all bands)
    pub envelope: f32,
}
```

### FrameData Extension

`FrameData` gains a single new field: `beat: BeatData`.

### Frequency Band Splits (mapped to 128 log bands)

- **Bass**: ~30-200 Hz
- **Mids**: ~200-4000 Hz
- **Treble**: ~4000-18000 Hz

## Algorithm

### Per-Band Beat Detection

1. **Instant energy**: Sum squared magnitudes across the band's spectrum bins, normalize to 0.0..1.0
2. **Rolling history**: Ring buffer of last ~43 frames (~0.7s at 60Hz) of energy per band
3. **Adaptive threshold**: Beat fires when `instant_energy > average_energy * sensitivity + variance_scale * energy_variance`. Variance term prevents false positives in loud sections and enables detection in quiet passages.
4. **Cooldown**: Minimum ~6 frames (~100ms) between beats per band to prevent rapid re-triggering

### Envelope Follower (per band)

- On beat: envelope snaps to 1.0 (instant attack)
- Each frame: `envelope *= decay` (default 0.95)
- Provides smooth 0.0..1.0 reactivity curve

### Overall Fields

- `beat`: true if any band fired
- `envelope`: max of all band envelopes

### Configurable Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `history_frames` | 43 | ~0.7s energy lookback window |
| `sensitivity` | 1.4 | Multiplier over average energy |
| `variance_scale` | 1.0 | How much variance affects threshold |
| `envelope_decay` | 0.95 | Per-frame envelope decay rate |
| `cooldown_frames` | 6 | ~100ms minimum between beats |

## Architecture

### New File

`src/beat.rs` — contains `BeatDetector` struct and `BeatData`.

### Processing Pipeline Integration

```
Audio samples -> Hann window -> FFT -> Log binning -> dB normalize -> EMA smooth
                                                                         |
                                                                         v
                                                            BeatDetector.analyze(spectrum)
                                                                         |
                                                                         v
                                                            FrameData { spectrum, waveform, peak, rms, beat }
```

`Processor` owns a `BeatDetector` instance. After computing the smoothed spectrum, it calls `beat_detector.analyze(&spectrum)` which returns `BeatData`. No changes to threading model or channel topology.

### Config

New `[beat_detection]` TOML section in `config.rs`:

```toml
[beat_detection]
sensitivity = 1.4
envelope_decay = 0.95
cooldown_frames = 6
history_frames = 43
```

### Visualization Consumption

No trait changes required. Visualizations read `frame.beat` in their existing `update()` calls.

## Testing

Unit tests in `beat.rs` with synthetic spectrum data:
- Silence: no beats fire
- Steady tone: no beats after initial onset
- Rhythmic pulses: beats detected on each pulse, cooldown respected
- Envelope decay: envelope value decreases correctly per frame
- Adaptive threshold: beats still detected in quiet passages

No audio hardware required for testing.

## Out of Scope

- Making existing visualizations react to beats (follow-up)
- BPM estimation / tempo tracking
- Additional sub-bands beyond 3
