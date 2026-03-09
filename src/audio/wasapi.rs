use super::AudioConfig;
use anyhow::{anyhow, Result};
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

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

        // Initialize COM and get audio client on the capture thread
        // (COM objects must be used on the thread where they were created)
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<()>>(1);

        let capture_thread = thread::spawn(move || {
            if let Err(e) = Self::capture_loop(producer, channels, &running_clone, &init_tx) {
                log::error!("WASAPI capture error: {}", e);
                let _ = init_tx.try_send(Err(e));
            }
        });

        // Wait for initialization result from capture thread
        let init_result = init_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow!("Audio capture initialization timed out"))?;
        init_result?;

        log::info!(
            "WASAPI loopback capture started (sample_rate={}, channels={})",
            config.sample_rate,
            config.channels
        );

        Ok(Self {
            running,
            capture_thread: Some(capture_thread),
        })
    }

    fn capture_loop(
        mut producer: HeapProd<f32>,
        channels: u32,
        running: &AtomicBool,
        init_tx: &std::sync::mpsc::SyncSender<Result<()>>,
    ) -> Result<()> {
        unsafe {
            // Initialize COM for this thread
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|e| anyhow!("COM init failed: {}", e))?;

            let result = Self::capture_loop_inner(&mut producer, channels, running, init_tx);

            CoUninitialize();
            result
        }
    }

    unsafe fn capture_loop_inner(
        producer: &mut HeapProd<f32>,
        channels: u32,
        running: &AtomicBool,
        init_tx: &std::sync::mpsc::SyncSender<Result<()>>,
    ) -> Result<()> {
        // Get default audio output device
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| anyhow!("Failed to create device enumerator: {}", e))?;

        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| anyhow!("No audio output device found: {}", e))?;

        // Get audio client
        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| anyhow!("Failed to activate audio client: {}", e))?;

        // Get the mix format (native device format)
        let mix_format_ptr = audio_client
            .GetMixFormat()
            .map_err(|e| anyhow!("Failed to get mix format: {}", e))?;
        let mix_format = &*mix_format_ptr;

        let sample_rate = mix_format.nSamplesPerSec;
        let device_channels = mix_format.nChannels;
        let bits_per_sample = mix_format.wBitsPerSample;

        log::debug!(
            "WASAPI device format: {}Hz, {} channels, {} bits",
            sample_rate,
            device_channels,
            bits_per_sample
        );

        // Initialize in loopback mode
        // AUDCLNT_STREAMFLAGS_LOOPBACK captures what's playing on the device
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                // Buffer duration: 100ms in 100-nanosecond units
                1_000_000,
                0,
                mix_format_ptr,
                None,
            )
            .map_err(|e| anyhow!("Failed to initialize loopback capture: {}", e))?;

        // Get capture client
        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|e| anyhow!("Failed to get capture client: {}", e))?;

        // Start capturing
        audio_client
            .Start()
            .map_err(|e| anyhow!("Failed to start audio capture: {}", e))?;

        // Signal successful initialization
        let _ = init_tx.send(Ok(()));

        log::info!(
            "WASAPI loopback started: {}Hz, {} channels",
            sample_rate,
            device_channels
        );

        // Capture loop — poll every ~10ms
        while running.load(Ordering::Relaxed) {
            let packet_size = match capture_client.GetNextPacketSize() {
                Ok(size) => size,
                Err(e) => {
                    log::warn!("GetNextPacketSize failed: {}", e);
                    break;
                }
            };

            if packet_size == 0 {
                thread::sleep(Duration::from_millis(10));
                continue;
            }

            let mut buffer_ptr = std::ptr::null_mut();
            let mut num_frames = 0u32;
            let mut flags = 0u32;

            if let Err(e) =
                capture_client.GetBuffer(&mut buffer_ptr, &mut num_frames, &mut flags, None, None)
            {
                log::warn!("GetBuffer failed: {}", e);
                break;
            }

            if num_frames > 0 && !buffer_ptr.is_null() {
                let is_silent = (flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)) != 0;

                if is_silent {
                    // Push zeros for silent buffers
                    for _ in 0..num_frames {
                        let _ = producer.try_push(0.0);
                    }
                } else {
                    // Cast to f32 samples
                    let total_samples = (num_frames * device_channels as u32) as usize;
                    let samples =
                        std::slice::from_raw_parts(buffer_ptr as *const f32, total_samples);

                    // Stereo-to-mono downmix (same as macOS callback)
                    if channels >= 2 && device_channels >= 2 {
                        for chunk in samples.chunks(device_channels as usize) {
                            let mono = chunk.iter().sum::<f32>() / device_channels as f32;
                            let _ = producer.try_push(mono);
                        }
                    } else {
                        for &sample in samples {
                            let _ = producer.try_push(sample);
                        }
                    }
                }
            }

            let _ = capture_client.ReleaseBuffer(num_frames);
        }

        // Stop capture
        let _ = audio_client.Stop();

        Ok(())
    }
}

impl Drop for AudioTap {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.capture_thread.take() {
            let _ = handle.join();
        }
        log::info!("WASAPI loopback capture stopped");
    }
}
