mod tap;

pub use tap::AudioTap;

use ringbuf::HeapRb;

pub type AudioRingBuffer = HeapRb<f32>;

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: f32,
    pub buffer_size: usize,
    pub channels: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            buffer_size: 4096,
            channels: 2,
        }
    }
}
