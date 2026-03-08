mod tap;

pub use tap::AudioTap;

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
