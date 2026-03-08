use std::f32::consts::PI;
use terminal_vibes::processing::{FrameData, Processor, ProcessorConfig};

/// Generate a sine wave at a given frequency and sample rate.
fn sine_wave(freq_hz: f32, sample_rate: f32, num_samples: usize) -> Vec<f32> {
    (0..num_samples)
        .map(|i| (2.0 * PI * freq_hz * i as f32 / sample_rate).sin())
        .collect()
}

#[test]
fn test_frame_data_default_is_empty() {
    let frame = FrameData::default();
    assert!(frame.spectrum.is_empty());
    assert!(frame.waveform.is_empty());
    assert_eq!(frame.peak, 0.0);
    assert_eq!(frame.rms, 0.0);
}

#[test]
fn test_processor_produces_spectrum_from_sine_wave() {
    let sample_rate = 44100.0;
    let fft_size = 2048;
    let config = ProcessorConfig {
        fft_size,
        sample_rate,
        smoothing: 0.0, // no smoothing for test clarity
        num_bands: 32,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    // 440Hz sine wave — should produce a peak in the spectrum
    let samples = sine_wave(440.0, sample_rate, fft_size);
    let frame = processor.process(&samples);

    assert_eq!(frame.spectrum.len(), 32);
    assert_eq!(frame.waveform.len(), fft_size);

    // The spectrum should have at least one non-zero bin
    let max_val = frame
        .spectrum
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max_val > 0.0,
        "Spectrum should have non-zero values for a sine input"
    );
}

#[test]
fn test_processor_peak_and_rms_for_known_signal() {
    let config = ProcessorConfig {
        fft_size: 1024,
        sample_rate: 44100.0,
        smoothing: 0.0,
        num_bands: 16,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    // Constant signal of 0.5 amplitude
    let samples = vec![0.5_f32; 1024];
    let frame = processor.process(&samples);

    approx::assert_abs_diff_eq!(frame.peak, 0.5, epsilon = 0.01);
    approx::assert_abs_diff_eq!(frame.rms, 0.5, epsilon = 0.01);
}

#[test]
fn test_processor_silence_produces_low_spectrum() {
    let config = ProcessorConfig {
        fft_size: 1024,
        sample_rate: 44100.0,
        smoothing: 0.0,
        num_bands: 16,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    let samples = vec![0.0_f32; 1024];
    let frame = processor.process(&samples);

    // All spectrum values should be at or near zero (clamped from dB floor)
    for val in &frame.spectrum {
        assert!(
            *val <= 0.01,
            "Silence should produce near-zero spectrum, got {}",
            val
        );
    }
    approx::assert_abs_diff_eq!(frame.peak, 0.0, epsilon = 0.001);
}

#[test]
fn test_smoothing_reduces_jitter() {
    let config = ProcessorConfig {
        fft_size: 1024,
        sample_rate: 44100.0,
        smoothing: 0.8, // heavy smoothing
        num_bands: 16,
        db_floor: -60.0,
    };
    let mut processor = Processor::new(config);

    // First frame: loud sine
    let loud = sine_wave(440.0, 44100.0, 1024);
    let frame1 = processor.process(&loud);

    // Second frame: silence — with smoothing, values should decay, not drop to zero
    let silence = vec![0.0_f32; 1024];
    let frame2 = processor.process(&silence);

    let max_after_silence = frame2
        .spectrum
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let max_loud = frame1
        .spectrum
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);

    // Smoothed silence should still retain some energy from previous frame
    assert!(
        max_after_silence > 0.0,
        "Smoothing should retain some energy"
    );
    assert!(max_after_silence < max_loud, "But less than the loud frame");
}
