/// Beat detection output data, attached to each `FrameData`.
#[derive(Debug, Clone)]
pub struct BeatData {
    /// Per-band instant energy (0.0..1.0 normalized)
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,

    /// Per-band beat detected this frame (used by visualizations)
    #[allow(dead_code)]
    pub bass_beat: bool,
    #[allow(dead_code)]
    pub mid_beat: bool,
    #[allow(dead_code)]
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

        // Compute autocorrelation with octave disambiguation
        let mut best_score = 0.0_f32;
        let mut best_lag = min_lag;
        let mut best_raw = 0.0_f32;

        for lag in min_lag..=max_lag {
            let r = Self::autocorrelate_at_lag(&scratch, lag);
            let mut score = r;

            // Penalize lags whose sub-harmonic (half period) is equally strong —
            // indicates this lag is a multiple of the true period
            let sub2 = lag / 2;
            if sub2 >= min_lag {
                let r_sub = Self::autocorrelate_at_lag(&scratch, sub2);
                if r_sub > r * 0.8 {
                    score *= 0.5;
                }
            }

            // Boost if super-harmonic (double period) is also strong — confirms fundamental
            let super2 = lag * 2;
            if super2 < len {
                score += 0.3 * Self::autocorrelate_at_lag(&scratch, super2);
            }

            if score > best_score {
                best_score = score;
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

    fn tempo_data(&self) -> TempoData {
        TempoData {
            bpm: self.bpm,
            confidence: self.confidence,
            phase: self.phase,
            predicted_beat: self.predicted_beat,
        }
    }
}

pub struct BeatDetector {
    bass: BandDetector,
    mid: BandDetector,
    treble: BandDetector,
    config: BeatDetectionConfig,
    bass_end: usize,
    mid_end: usize,
}

impl BeatDetector {
    pub fn new(num_bands: usize, config: BeatDetectionConfig) -> Self {
        let f_min: f64 = 30.0;
        let f_max: f64 = 18000.0;
        let log_ratio = (f_max / f_min).ln();

        let bass_end = ((num_bands as f64) * (200.0_f64 / f_min).ln() / log_ratio) as usize;
        let mid_end = ((num_bands as f64) * (4000.0_f64 / f_min).ln() / log_ratio) as usize;

        let bass_end = bass_end.clamp(1, num_bands - 2);
        let mid_end = mid_end.clamp(bass_end + 1, num_bands - 1);

        Self {
            bass: BandDetector::new(config.flux_history_frames),
            mid: BandDetector::new(config.flux_history_frames),
            treble: BandDetector::new(config.flux_history_frames),
            bass_end,
            mid_end,
            config,
        }
    }

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
}

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
        // Fill only bass bins (bass_end=37 for 128 bands)
        for i in 0..37 {
            spectrum[i] = 0.8;
        }
        let beat = detector.analyze(&spectrum);
        assert!(
            beat.bass_energy > 0.5,
            "Bass energy should be high, got {}",
            beat.bass_energy
        );
        assert!(
            beat.mid_energy < 0.01,
            "Mid energy should be ~zero, got {}",
            beat.mid_energy
        );
        assert!(
            beat.treble_energy < 0.01,
            "Treble energy should be ~zero, got {}",
            beat.treble_energy
        );
    }

    #[test]
    fn test_treble_energy_from_high_bands() {
        let mut detector = BeatDetector::new(128, make_config());
        let mut spectrum = vec![0.0_f32; 128];
        for i in 97..128 {
            spectrum[i] = 0.8;
        }
        let beat = detector.analyze(&spectrum);
        assert!(
            beat.treble_energy > 0.3,
            "Treble energy should be high, got {}",
            beat.treble_energy
        );
        assert!(
            beat.bass_energy < 0.01,
            "Bass energy should be ~zero, got {}",
            beat.bass_energy
        );
    }

    #[test]
    fn test_no_beats_on_silence() {
        let mut detector = BeatDetector::new(128, make_config());
        let spectrum = vec![0.0_f32; 128];
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
        // Fill bass bins only (bass_end=37 for 128 bands)
        for i in 0..37 {
            loud[i] = 0.9;
        }

        for _ in 0..50 {
            detector.analyze(&quiet);
        }

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
        // Fill bass bins only (bass_end=37 for 128 bands)
        for i in 0..37 {
            loud[i] = 0.9;
        }

        for _ in 0..50 {
            detector.analyze(&quiet);
        }

        let beat1 = detector.analyze(&loud);
        assert!(beat1.bass_beat, "First spike should beat");

        detector.analyze(&quiet);
        let beat2 = detector.analyze(&loud);
        assert!(!beat2.bass_beat, "Cooldown should prevent rapid re-trigger");
    }

    #[test]
    fn test_envelope_decays_after_beat() {
        let mut detector = BeatDetector::new(128, make_config());
        let quiet = vec![0.05_f32; 128];
        let mut loud = vec![0.05_f32; 128];
        // Fill bass bins only (bass_end=37 for 128 bands)
        for i in 0..37 {
            loud[i] = 0.9;
        }

        for _ in 0..50 {
            detector.analyze(&quiet);
        }

        let beat = detector.analyze(&loud);
        assert!(
            beat.bass_envelope > 0.9,
            "Envelope should snap to ~1.0 on beat"
        );

        let mut prev_env = beat.bass_envelope;
        for _ in 0..10 {
            let b = detector.analyze(&quiet);
            assert!(
                b.bass_envelope < prev_env,
                "Envelope should decay each frame"
            );
            prev_env = b.bass_envelope;
        }
        assert!(
            prev_env < 0.7,
            "Envelope should have decayed significantly after 10 frames"
        );
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
        for _beat_num in 0..8 {
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
        assert!(!tempo.predicted_beat, "No predicted beats during silence");
    }

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
}
