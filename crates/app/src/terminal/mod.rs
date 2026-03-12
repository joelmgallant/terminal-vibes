pub mod ui;
pub mod visualizations;

use anyhow::Result;

use crate::config::Config;
use crate::pipeline::AudioPipeline;

use self::ui::App;
use self::visualizations::aurora::Aurora;
use self::visualizations::life::Life;
use self::visualizations::lissajous::Lissajous;
use self::visualizations::milkdrop::Milkdrop;
use self::visualizations::plasma::Plasma;
use self::visualizations::radial::RadialSpectrum;
use self::visualizations::rain::Rain;
use self::visualizations::registry::VisualizationRegistry;
use self::visualizations::spectrogram::Spectrogram;
use self::visualizations::spectrum::SpectrumBars;
use self::visualizations::starfield::Starfield;
use self::visualizations::tunnel::Tunnel;
use self::visualizations::waveform::Waveform;

pub fn run(config: Config) -> Result<()> {
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

    Ok(())
}
