# Spectral Flux Beat Detection (Hybrid)

## Problem

The current beat detection uses an energy-vs-running-average algorithm that struggles with dance music. When beats are consistent (4-on-the-floor), the running average rises with the beat pattern, inflating the threshold and causing inconsistent detection — catching kicks for a few bars then going quiet.

## Solution: Hybrid Spectral Flux + Energy Gate

### Core Algorithm

**Spectral flux (per-band):** Compute the positive spectral flux — the sum of energy increases across frequency bins compared to the previous frame. Only positive differences count (onsets, not offsets).

```
flux = sum(max(0, current[i] - previous[i]) for i in band_bins)
```

**Median-based threshold:** Keep a short history of flux values. Use the median multiplied by a sensitivity factor as the threshold. Median is more robust than mean — spikes don't inflate it.

```
threshold = median(flux_history) * flux_sensitivity + min_threshold
```

**Energy gate:** A beat only fires if flux exceeds threshold AND band energy exceeds a floor. Prevents false positives during silence.

```
beat = flux > threshold AND energy > energy_floor
```

Cooldown and envelope systems are unchanged.

### Config Changes

New fields on `BeatDetectionConfig`:

| Field | Default | Purpose |
|-------|---------|---------|
| `flux_sensitivity` | 2.0 | Multiplier on median flux for threshold |
| `flux_history_frames` | 30 | Median window (~500ms at 60Hz) |
| `energy_floor` | 0.001 | Minimum energy to allow a beat trigger |

Old `sensitivity` and `variance_scale` fields kept for backward compatibility.

### Data Flow

No pipeline changes. `BandDetector` gains `previous_bins: Vec<f32>` and `flux_history: Vec<f32>`. The `analyze()` method takes raw frequency bin slices (not pre-computed energy) to compute per-bin flux. `BeatDetector::analyze()` already receives the full spectrum and slices per band.

### Testing

- Existing tests pass (silence, envelope decay, cooldown)
- New: steady periodic signal (simulated 4-on-the-floor) detects beats consistently
- New: flux doesn't trigger on silence
- New: energy gate prevents beats when flux spikes but energy is near-zero
