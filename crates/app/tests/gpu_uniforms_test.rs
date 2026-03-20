#![cfg(feature = "gui")]

use terminal_vibes::beat::{BeatData, TempoData};
use terminal_vibes::gpu::uniforms::Uniforms;
use terminal_vibes::processing::FrameData;

#[test]
fn test_uniforms_size_is_64_bytes() {
    assert_eq!(std::mem::size_of::<Uniforms>(), 64);
}

#[test]
fn test_uniforms_from_frame() {
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 256],
        peak: 0.8,
        rms: 0.5,
        beat: BeatData {
            bass_energy: 0.3,
            mid_energy: 0.5,
            treble_energy: 0.7,
            bass_beat: true,
            mid_beat: false,
            treble_beat: false,
            bass_envelope: 0.9,
            mid_envelope: 0.4,
            treble_envelope: 0.2,
            beat: true,
            envelope: 0.9,
        },
        tempo: TempoData {
            bpm: 120.0,
            confidence: 0.8,
            phase: 0.5,
            predicted_beat: false,
        },
    };

    let uniforms = Uniforms::from_frame(&frame, 1.5, 0.016, [1280.0, 720.0], 42);

    assert_eq!(uniforms.time, 1.5);
    assert_eq!(uniforms.delta_time, 0.016);
    assert_eq!(uniforms.resolution, [1280.0, 720.0]);
    assert_eq!(uniforms.frame, 42);
    assert_eq!(uniforms.beat_envelope, 0.9);
    assert_eq!(uniforms.bass_energy, 0.3);
    assert_eq!(uniforms.bpm, 120.0);
    assert_eq!(uniforms.bass_beat, 1.0); // true -> 1.0
    assert_eq!(uniforms.mid_beat, 0.0); // false -> 0.0
    assert_eq!(uniforms.feedback_mix, 0.95);
}
