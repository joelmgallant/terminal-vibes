use terminal_vibes::config::Config;

// AudioPipeline::start() requires a real audio device, so we only test
// that the module is importable and config defaults are valid.
#[allow(unused_imports)]
use terminal_vibes::pipeline::AudioPipeline;

#[test]
fn test_pipeline_config_defaults() {
    let config = Config::default();
    // Verify the config values that AudioPipeline will use
    assert_eq!(config.audio.fft_size, 2048);
    assert_eq!(config.audio.buffer_size, 4096);
    assert_eq!(config.audio.smoothing, 0.7);
}
