#![cfg(feature = "gui")]

use terminal_vibes::beat::{BeatData, TempoData};
use terminal_vibes::gpu::audio_texture::AudioTextureData;
use terminal_vibes::processing::FrameData;

fn make_test_frame() -> FrameData {
    FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 256],
        peak: 0.8,
        rms: 0.5,
        beat: BeatData::default(),
        tempo: TempoData::default(),
    }
}

#[test]
fn test_audio_texture_dimensions() {
    let frame = make_test_frame();
    let tex = AudioTextureData::from_frame(&frame);
    // 256 pixels wide, 2 rows, 4 bytes per pixel (RGBA)
    assert_eq!(tex.data.len(), 256 * 2 * 4);
    assert_eq!(tex.width, 256);
    assert_eq!(tex.height, 2);
}

#[test]
fn test_waveform_in_row_0() {
    let mut frame = make_test_frame();
    // Set waveform sample at index 0 to 0.75
    // Mapping: (0.75 * 0.5 + 0.5) = 0.875 -> 0.875 * 255 = ~223
    frame.waveform[0] = 0.75;
    let tex = AudioTextureData::from_frame(&frame);
    // Row 0, pixel 0, red channel
    let r = tex.data[0];
    assert!(r > 220 && r < 230, "Expected ~223, got {}", r);
}

#[test]
fn test_spectrum_in_row_1() {
    let mut frame = make_test_frame();
    frame.spectrum[0] = 1.0;
    let tex = AudioTextureData::from_frame(&frame);
    // Row 1 starts at byte offset 256*4 = 1024
    let r = tex.data[1024];
    assert_eq!(r, 255);
}
