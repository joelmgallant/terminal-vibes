use super::AudioConfig;
use anyhow::{anyhow, Result};
use libpulse_binding as pulse;
use libpulse_simple_binding::Simple;
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub struct AudioTap {
    running: Arc<AtomicBool>,
    capture_thread: Option<thread::JoinHandle<()>>,
}

unsafe impl Send for AudioTap {}

impl AudioTap {
    pub fn new(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let channels = config.channels;
        let sample_rate = config.sample_rate as u32;

        // Test connection before spawning thread
        let spec = pulse::sample::Spec {
            format: pulse::sample::Format::F32le,
            channels: channels as u8,
            rate: sample_rate,
        };

        if !spec.is_valid() {
            return Err(anyhow!(
                "Invalid PulseAudio sample spec: {}Hz, {} channels",
                sample_rate,
                channels
            ));
        }

        // Connect to PulseAudio — use default sink's monitor source
        // @DEFAULT_SINK@.monitor works on both PulseAudio and PipeWire
        let simple = Simple::new(
            None,                             // Default server
            "terminal-vibes",                 // Application name
            pulse::stream::Direction::Record, // We're recording
            Some("@DEFAULT_SINK@.monitor"),   // Monitor source of default output
            "audio-capture",                  // Stream description
            &spec,
            None, // Default channel map
            None, // Default buffering attributes
        )
        .map_err(|e| {
            anyhow!(
                "Failed to connect to PulseAudio: {}. \
                 Is PulseAudio or PipeWire running?",
                e
            )
        })?;

        log::info!(
            "PulseAudio monitor capture connected (sample_rate={}, channels={})",
            sample_rate,
            channels
        );

        let capture_thread = thread::spawn(move || {
            Self::capture_loop(simple, producer, channels, &running_clone);
        });

        Ok(Self {
            running,
            capture_thread: Some(capture_thread),
        })
    }

    fn capture_loop(
        simple: Simple,
        mut producer: HeapProd<f32>,
        channels: u32,
        running: &AtomicBool,
    ) {
        // Read buffer: ~10ms of audio at 44100Hz stereo = ~1764 samples = ~3528 f32s
        let buf_frames = 1764;
        let buf_size = buf_frames * channels as usize;
        let mut buf = vec![0.0f32; buf_size];

        // Reinterpret f32 slice as u8 slice for PulseAudio read API
        let byte_len = buf_size * std::mem::size_of::<f32>();

        while running.load(Ordering::Relaxed) {
            let byte_slice =
                unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, byte_len) };

            match simple.read(byte_slice) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("PulseAudio read error: {}", e);
                    break;
                }
            }

            // Stereo-to-mono downmix (same as macOS and Windows)
            if channels >= 2 {
                for chunk in buf.chunks(channels as usize) {
                    let mono = chunk.iter().sum::<f32>() / channels as f32;
                    let _ = producer.try_push(mono);
                }
            } else {
                for &sample in &buf {
                    let _ = producer.try_push(sample);
                }
            }
        }

        log::info!("PulseAudio capture loop ended");
    }
}

impl Drop for AudioTap {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.capture_thread.take() {
            let _ = handle.join();
        }
        log::info!("PulseAudio monitor capture stopped");
    }
}
