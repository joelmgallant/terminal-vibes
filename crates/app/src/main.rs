use anyhow::{Context, Result};
use clap::Parser;
use ringbuf::traits::{Consumer, Split};
use ringbuf::HeapRb;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Core types via lib.rs re-exports
use terminal_vibes::audio::{AudioConfig, AudioTap};
use terminal_vibes::beat::BeatDetector;
use terminal_vibes::config::Config;
use terminal_vibes::processing::{FrameData, Processor, ProcessorConfig};

// App types
use terminal_vibes::ui::App;
use terminal_vibes::visualizations::aurora::Aurora;
use terminal_vibes::visualizations::life::Life;
use terminal_vibes::visualizations::lissajous::Lissajous;
use terminal_vibes::visualizations::milkdrop::Milkdrop;
use terminal_vibes::visualizations::plasma::Plasma;
use terminal_vibes::visualizations::radial::RadialSpectrum;
use terminal_vibes::visualizations::rain::Rain;
use terminal_vibes::visualizations::registry::VisualizationRegistry;
use terminal_vibes::visualizations::spectrogram::Spectrogram;
use terminal_vibes::visualizations::spectrum::SpectrumBars;
use terminal_vibes::visualizations::starfield::Starfield;
use terminal_vibes::visualizations::tunnel::Tunnel;
use terminal_vibes::visualizations::waveform::Waveform;

#[derive(Parser)]
#[command(name = "terminal-vibes", about = "Terminal-based music visualizer")]
struct Cli {
    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    /// List available visualization modes
    #[arg(long)]
    list_modes: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    if cli.list_modes {
        println!("Available visualization modes:");
        println!("  spectrum    - Frequency spectrum bars");
        println!("  waveform    - Oscilloscope waveform");
        println!("  spectrogram - Scrolling frequency heatmap");
        println!("  lissajous   - Parametric curve spirograph");
        println!("  tunnel      - Concentric rings rushing forward");
        println!("  radial      - Circular spectrum starburst");
        println!("  plasma      - Sine interference color field");
        println!("  aurora      - Northern lights curtains");
        println!("  starfield   - 3D particle warp drive");
        println!("  rain        - Music-reactive falling streams");
        println!("  life        - Audio-reactive Game of Life");
        return Ok(());
    }

    let config = Config::load(cli.config.as_ref()).context("Failed to load config")?;

    // Set up ring buffer
    let rb = HeapRb::<f32>::new(config.audio.buffer_size);
    let (producer, mut consumer) = rb.split();

    // Set up audio tap
    let audio_config = AudioConfig {
        sample_rate: 44100.0,
        channels: 2,
    };
    let _audio_tap =
        AudioTap::new(producer, audio_config.clone()).context(if cfg!(target_os = "macos") {
            "Failed to start audio capture. \
             Make sure you're on macOS 15+ and have granted audio permissions."
        } else if cfg!(target_os = "windows") {
            "Failed to start audio capture. \
             Make sure an audio output device is available."
        } else {
            "Failed to start audio capture. \
             Make sure PulseAudio or PipeWire is running."
        })?;

    // Set up visualization registry
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(SpectrumBars::new()));
    registry.register(Box::new(Waveform::new()));
    registry.register(Box::new(Spectrogram::new(200)));
    registry.register(Box::new(Lissajous::new()));
    registry.register(Box::new(Tunnel::new()));
    registry.register(Box::new(RadialSpectrum::new()));
    registry.register(Box::new(Plasma::new()));
    registry.register(Box::new(Aurora::new()));
    registry.register(Box::new(Starfield::new()));
    registry.register(Box::new(Rain::new()));
    registry.register(Box::new(Life::new()));
    registry.register(Box::new(Milkdrop::new()));

    // Set up processing -> UI channel
    let (frame_tx, frame_rx) = mpsc::sync_channel::<FrameData>(2);

    let running = Arc::new(AtomicBool::new(true));
    let running_processor = running.clone();

    // Spawn processor thread
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

        // Accumulation buffer — we collect samples across multiple polls
        let mut accum = Vec::with_capacity(fft_size * 2);
        let mut drain_buf = vec![0.0_f32; 4096];
        let interval = Duration::from_millis(1000 / 60); // ~60 Hz

        while running_processor.load(Ordering::Relaxed) {
            // Drain available samples from ring buffer into accumulator
            let count = consumer.pop_slice(&mut drain_buf);
            if count > 0 {
                accum.extend_from_slice(&drain_buf[..count]);
            }

            // Process all available windows (prevents accum growth under load)
            while accum.len() >= fft_size {
                let mut frame = processor.process(&accum[..fft_size]);
                let (beat_data, tempo_data) = beat_detector.analyze(&frame.spectrum);
                frame.beat = beat_data;
                frame.tempo = tempo_data;
                let _ = frame_tx.try_send(frame);

                // Slide: keep the last half for overlap
                let keep = fft_size / 2;
                let start = accum.len() - keep;
                accum.drain(..start);
            }

            thread::sleep(interval);
        }
    });

    // Run UI on main thread
    let mut app = App::new(registry, config, running.clone());
    app.run(frame_rx)?;

    // Clean shutdown
    running.store(false, Ordering::Relaxed);
    let _ = processor_handle.join();

    Ok(())
}
