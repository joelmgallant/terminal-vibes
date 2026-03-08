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
    pub sample_rate: f32,
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

/// Bin linear frequency magnitudes into logarithmically-spaced bands.
/// Output is normalized to 0.0..1.0 range based on dB floor.
fn bin_to_bands(magnitudes: &[f32], num_bands: usize, db_floor: f32) -> Vec<f32> {
    let n = magnitudes.len();
    if n == 0 || num_bands == 0 {
        return vec![0.0; num_bands];
    }

    let mut bands = vec![0.0_f32; num_bands];

    for band in 0..num_bands {
        // Logarithmic bin edges
        let low = ((band as f64 / num_bands as f64).exp2() - 1.0)
            / (2.0_f64.powi(1) - 1.0)
            * n as f64;
        let high = (((band + 1) as f64 / num_bands as f64).exp2() - 1.0)
            / (2.0_f64.powi(1) - 1.0)
            * n as f64;

        let lo = (low as usize).max(0).min(n - 1);
        let hi = (high as usize).max(lo + 1).min(n);

        // Average magnitude in this band
        let avg = if hi > lo {
            magnitudes[lo..hi].iter().sum::<f32>() / (hi - lo) as f32
        } else {
            magnitudes[lo]
        };

        // Convert to dB then normalize to 0.0..1.0
        let db = if avg > 0.0 {
            20.0 * avg.log10()
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
