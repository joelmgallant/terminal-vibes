#![allow(dead_code)]

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

/// Configuration for beat detection.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
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

impl Default for BeatDetectionConfig {
    fn default() -> Self {
        Self {
            sensitivity: 1.2,
            variance_scale: 0.4,
            envelope_decay: 0.95,
            cooldown_frames: 4,
            history_frames: 43,
            flux_sensitivity: 2.0,
            flux_history_frames: 30,
            energy_floor: 0.001,
        }
    }
}

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

    fn compute_energy(bands: &[f32]) -> f32 {
        if bands.is_empty() {
            return 0.0;
        }
        let sum: f32 = bands.iter().map(|&v| v * v).sum();
        sum / bands.len() as f32
    }

    fn analyze(&mut self, energy: f32, config: &BeatDetectionConfig) -> (bool, f32) {
        let capacity = self.energy_history.len();
        self.energy_history[self.history_pos] = energy;
        self.history_pos = (self.history_pos + 1) % capacity;
        if self.history_len < capacity {
            self.history_len += 1;
        }

        let history = &self.energy_history[..self.history_len];
        let avg = history.iter().sum::<f32>() / history.len() as f32;
        let variance =
            history.iter().map(|&e| (e - avg).powi(2)).sum::<f32>() / history.len() as f32;

        let threshold = avg * config.sensitivity + variance.sqrt() * config.variance_scale;

        let beat = if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
            false
        } else if energy > threshold && energy > 1e-6 {
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

        (beat, self.envelope)
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
            bass: BandDetector::new(config.history_frames),
            mid: BandDetector::new(config.history_frames),
            treble: BandDetector::new(config.history_frames),
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
}
