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
#[derive(Debug, Clone)]
pub struct BeatDetectionConfig {
    pub sensitivity: f32,
    pub variance_scale: f32,
    pub envelope_decay: f32,
    pub cooldown_frames: usize,
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

        BeatData {
            bass_energy,
            mid_energy,
            treble_energy,
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
}
