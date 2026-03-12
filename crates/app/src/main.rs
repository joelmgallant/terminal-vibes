use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use terminal_vibes::config::Config;
use terminal_vibes::pipeline::AudioPipeline;
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

    let mut pipeline = AudioPipeline::start(&config)?;

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

    let frame_rx = pipeline.take_frame_rx();
    let mut app = App::new(registry, config, pipeline.running.clone());
    app.run(frame_rx)?;

    // pipeline.drop() handles shutdown
    Ok(())
}
