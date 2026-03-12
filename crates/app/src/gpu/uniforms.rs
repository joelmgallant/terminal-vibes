use bytemuck::{Pod, Zeroable};
use terminal_vibes_core::processing::FrameData;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub time: f32,
    pub delta_time: f32,
    pub resolution: [f32; 2],
    pub frame: u32,
    pub beat_envelope: f32,
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,
    pub bass_beat: f32,
    pub mid_beat: f32,
    pub treble_beat: f32,
    pub bpm: f32,
    pub beat_phase: f32,
    pub beat_confidence: f32,
    pub _pad: f32,
}

impl Uniforms {
    pub fn from_frame(
        frame: &FrameData,
        time: f32,
        delta_time: f32,
        resolution: [f32; 2],
        frame_count: u32,
    ) -> Self {
        Self {
            time,
            delta_time,
            resolution,
            frame: frame_count,
            beat_envelope: frame.beat.envelope,
            bass_energy: frame.beat.bass_energy,
            mid_energy: frame.beat.mid_energy,
            treble_energy: frame.beat.treble_energy,
            bass_beat: if frame.beat.bass_beat { 1.0 } else { 0.0 },
            mid_beat: if frame.beat.mid_beat { 1.0 } else { 0.0 },
            treble_beat: if frame.beat.treble_beat { 1.0 } else { 0.0 },
            bpm: frame.tempo.bpm,
            beat_phase: frame.tempo.phase,
            beat_confidence: frame.tempo.confidence,
            _pad: 0.0,
        }
    }
}
