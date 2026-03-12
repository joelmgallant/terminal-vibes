use anyhow::{Context, Result};
use ringbuf::traits::{Consumer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::audio::{AudioConfig, AudioTap};
use crate::beat::BeatDetector;
use crate::config::Config;
use crate::processing::{FrameData, Processor, ProcessorConfig};

pub struct AudioPipeline {
    frame_rx: Option<mpsc::Receiver<FrameData>>,
    pub running: Arc<AtomicBool>,
    _audio_tap: AudioTap,
    processor_handle: Option<thread::JoinHandle<()>>,
}

impl AudioPipeline {
    /// Take the frame receiver out of the pipeline.
    /// Panics if called more than once.
    pub fn take_frame_rx(&mut self) -> mpsc::Receiver<FrameData> {
        self.frame_rx
            .take()
            .expect("frame_rx already taken from AudioPipeline")
    }
}

impl AudioPipeline {
    pub fn start(config: &Config) -> Result<Self> {
        let rb = HeapRb::<f32>::new(config.audio.buffer_size);
        let (producer, mut consumer) = rb.split();

        let audio_config = AudioConfig {
            sample_rate: 44100.0,
            channels: 2,
        };
        let audio_tap = AudioTap::new(producer, audio_config.clone()).context(
            if cfg!(target_os = "macos") {
                "Failed to start audio capture. \
                 Make sure you're on macOS 15+ and have granted audio permissions."
            } else if cfg!(target_os = "windows") {
                "Failed to start audio capture. \
                 Make sure an audio output device is available."
            } else {
                "Failed to start audio capture. \
                 Make sure PulseAudio or PipeWire is running."
            },
        )?;

        let (frame_tx, frame_rx) = mpsc::sync_channel::<FrameData>(2);
        let running = Arc::new(AtomicBool::new(true));
        let running_processor = running.clone();

        let fft_size = config.audio.fft_size;
        let smoothing = config.audio.smoothing;
        let beat_detection_config = config.beat_detection.clone();

        let processor_handle = thread::spawn(move || {
            let mut processor = Processor::new(ProcessorConfig {
                fft_size,
                smoothing,
                num_bands: 128,
                db_floor: -60.0,
            });
            let mut beat_detector = BeatDetector::new(128, beat_detection_config);

            let mut accum = Vec::with_capacity(fft_size * 2);
            let mut drain_buf = vec![0.0_f32; 4096];
            let interval = Duration::from_millis(1000 / 60);

            while running_processor.load(Ordering::Relaxed) {
                let count = consumer.pop_slice(&mut drain_buf);
                if count > 0 {
                    accum.extend_from_slice(&drain_buf[..count]);
                }

                while accum.len() >= fft_size {
                    let mut frame = processor.process(&accum[..fft_size]);
                    let (beat_data, tempo_data) = beat_detector.analyze(&frame.spectrum);
                    frame.beat = beat_data;
                    frame.tempo = tempo_data;
                    let _ = frame_tx.try_send(frame);

                    let keep = fft_size / 2;
                    let start = accum.len() - keep;
                    accum.drain(..start);
                }

                thread::sleep(interval);
            }
        });

        Ok(Self {
            frame_rx: Some(frame_rx),
            running,
            _audio_tap: audio_tap,
            processor_handle: Some(processor_handle),
        })
    }
}

impl Drop for AudioPipeline {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.processor_handle.take() {
            let _ = handle.join();
        }
    }
}
