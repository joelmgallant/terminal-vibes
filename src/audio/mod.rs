#[cfg(target_os = "macos")]
mod tap;
#[cfg(target_os = "macos")]
pub use tap::AudioTap;

#[cfg(target_os = "windows")]
mod wasapi;
#[cfg(target_os = "windows")]
pub use wasapi::AudioTap;

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
