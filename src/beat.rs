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
