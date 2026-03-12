#[cfg(target_os = "macos")]
mod tap;
#[cfg(target_os = "macos")]
pub use tap::AudioTap;

#[cfg(target_os = "windows")]
mod wasapi;
#[cfg(target_os = "windows")]
pub use wasapi::AudioTap;

#[cfg(target_os = "linux")]
mod pulse;
#[cfg(target_os = "linux")]
pub use pulse::AudioTap;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
compile_error!(
    "terminal-vibes requires macOS, Windows, or Linux. \
     No audio capture backend is available for this platform."
);

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: f32,
    pub channels: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            channels: 2,
        }
    }
}
