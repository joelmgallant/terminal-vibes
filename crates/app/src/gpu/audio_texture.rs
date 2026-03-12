use terminal_vibes_core::processing::FrameData;

/// 256x2 RGBA texture data following Shadertoy conventions:
/// - Row 0 (y=0.25): waveform samples, normalized 0..1
/// - Row 1 (y=0.75): FFT spectrum, normalized 0..1
pub struct AudioTextureData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl AudioTextureData {
    pub fn from_frame(frame: &FrameData) -> Self {
        let width = 256u32;
        let height = 2u32;
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Row 0: waveform (256 samples)
        for i in 0..256 {
            let sample = if i < frame.waveform.len() {
                // Waveform is -1..1, map to 0..1
                (frame.waveform[i] * 0.5 + 0.5).clamp(0.0, 1.0)
            } else {
                0.5
            };
            let byte = (sample * 255.0) as u8;
            let offset = i * 4;
            data[offset] = byte; // R
            data[offset + 1] = byte; // G
            data[offset + 2] = byte; // B
            data[offset + 3] = 255; // A
        }

        // Row 1: spectrum (128 bands -> stretched to 256 pixels)
        let row1_offset = (width * 4) as usize;
        for i in 0..256 {
            let band_idx = i * frame.spectrum.len() / 256;
            let value = if band_idx < frame.spectrum.len() {
                frame.spectrum[band_idx].clamp(0.0, 1.0)
            } else {
                0.0
            };
            let byte = (value * 255.0) as u8;
            let offset = row1_offset + i * 4;
            data[offset] = byte; // R
            data[offset + 1] = byte; // G
            data[offset + 2] = byte; // B
            data[offset + 3] = 255; // A
        }

        Self {
            data,
            width,
            height,
        }
    }
}
