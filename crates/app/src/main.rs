use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use terminal_vibes::config::Config;

#[derive(Parser)]
#[command(name = "terminal-vibes", about = "Terminal-based music visualizer")]
struct Cli {
    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    /// List available visualization modes
    #[arg(long)]
    list_modes: bool,

    /// Launch GPU-accelerated window mode (requires --features gui)
    #[arg(long)]
    gui: bool,
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

    if cli.gui {
        #[cfg(feature = "gui")]
        {
            terminal_vibes::gpu::run(config)?;
        }
        #[cfg(not(feature = "gui"))]
        {
            anyhow::bail!(
                "GUI mode requires the 'gui' feature.\n\
                 Build with: cargo build --features gui\n\
                 Run with:   cargo run --features gui -- --gui"
            );
        }
    } else {
        terminal_vibes::terminal::run(config)?;
    }

    Ok(())
}
