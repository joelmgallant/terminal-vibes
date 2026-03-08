use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

#[derive(Debug, Clone, Default)]
pub struct FrameData {
    pub spectrum: Vec<f32>,
    pub waveform: Vec<f32>,
    pub peak: f32,
    pub rms: f32,
}

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub fft_size: usize,
    pub smoothing: f64,
    pub num_bands: usize,
    pub db_floor: f32,
}

pub struct Processor {
    config: ProcessorConfig,
    planner: FftPlanner<f32>,
    window: Vec<f32>,
    smoothed_spectrum: Vec<f32>,
}

impl Processor {
    pub fn new(config: ProcessorConfig) -> Self {
        let window = hann_window(config.fft_size);
        let smoothed_spectrum = vec![0.0; config.num_bands];
        Self {
            config,
            planner: FftPlanner::new(),
            window,
            smoothed_spectrum,
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> FrameData {
        let n = self.config.fft_size.min(samples.len());

        // Waveform: raw samples
        let waveform = samples[..n].to_vec();

        // Peak and RMS
        let peak = samples[..n]
            .iter()
            .fold(0.0_f32, |acc, &s| acc.max(s.abs()));
        let rms = (samples[..n]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / n as f32)
            .sqrt();

        // Apply window and run FFT
        let mut buffer: Vec<Complex<f32>> = samples[..n]
            .iter()
            .zip(self.window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();

        // Pad to fft_size if needed
        buffer.resize(self.config.fft_size, Complex::new(0.0, 0.0));

        let fft = self.planner.plan_fft_forward(self.config.fft_size);
        fft.process(&mut buffer);

        // Compute magnitudes (only first half — Nyquist)
        let half = self.config.fft_size / 2;
        let magnitudes: Vec<f32> = buffer[..half]
            .iter()
            .map(|c| c.norm() / half as f32)
            .collect();

        // Bin into logarithmic frequency bands
        let spectrum = bin_to_bands(&magnitudes, self.config.num_bands, self.config.db_floor);

        // Apply smoothing
        for (i, val) in spectrum.iter().enumerate() {
            let s = self.config.smoothing as f32;
            self.smoothed_spectrum[i] = self.smoothed_spectrum[i] * s + val * (1.0 - s);
        }

        FrameData {
            spectrum: self.smoothed_spectrum.clone(),
            waveform,
            peak,
            rms,
        }
    }
}

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (size - 1) as f32).cos()))
        .collect()
}

/// Bin linear FFT magnitudes into perceptually-spaced logarithmic bands.
///
/// Uses a true log scale from `f_min` to `f_max` Hz so that bass frequencies
/// get good resolution on the left and treble compresses naturally on the right.
/// Output is normalized to 0.0..1.0 based on dB floor.
fn bin_to_bands(magnitudes: &[f32], num_bands: usize, db_floor: f32) -> Vec<f32> {
    let n = magnitudes.len();
    if n == 0 || num_bands == 0 {
        return vec![0.0; num_bands];
    }

    let mut bands = vec![0.0_f32; num_bands];

    // Perceptual frequency range: 30 Hz to ~18 kHz mapped across bands.
    // We work in bin-index space: bin = freq * fft_size / sample_rate.
    // Since magnitudes.len() == fft_size/2, bin_max = n corresponds to Nyquist.
    // f_min_bin and f_max_bin are the bin indices for our desired range.
    let f_min_bin = 1.0_f64; // ~20-40 Hz depending on sample rate
    let f_max_bin = n as f64; // Nyquist

    for band in 0..num_bands {
        // Log-spaced bin edges: f_min * (f_max/f_min)^(t)
        let t0 = band as f64 / num_bands as f64;
        let t1 = (band + 1) as f64 / num_bands as f64;
        let low = f_min_bin * (f_max_bin / f_min_bin).powf(t0);
        let high = f_min_bin * (f_max_bin / f_min_bin).powf(t1);

        let lo = (low as usize).clamp(0, n - 1);
        let hi = (high as usize).clamp(lo + 1, n);

        // Peak magnitude in this band (peak reads better than average for viz)
        let peak = magnitudes[lo..hi]
            .iter()
            .fold(0.0_f32, |acc, &m| acc.max(m));

        // Convert to dB then normalize to 0.0..1.0
        let db = if peak > 0.0 {
            20.0 * peak.log10()
        } else {
            db_floor
        };

        bands[band] = ((db - db_floor) / -db_floor).clamp(0.0, 1.0);
    }

    bands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window_endpoints_are_zero() {
        let w = hann_window(256);
        assert!(w[0].abs() < 1e-6);
        assert!(w[255].abs() < 1e-6);
    }

    #[test]
    fn test_hann_window_peak_is_one() {
        let w = hann_window(256);
        let mid = w[128];
        approx::assert_abs_diff_eq!(mid, 1.0, epsilon = 0.01);
    }

    #[test]
    fn test_bin_to_bands_silence() {
        let mags = vec![0.0; 512];
        let bands = bin_to_bands(&mags, 16, -60.0);
        for b in &bands {
            assert!(*b <= 0.01);
        }
    }
}
