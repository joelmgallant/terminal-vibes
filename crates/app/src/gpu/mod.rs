pub mod audio_texture;
pub mod renderer;
pub mod shader_loader;
pub mod uniforms;
pub mod window;

use crate::config::Config;
use anyhow::Result;

pub fn run(config: Config) -> Result<()> {
    log::info!("Starting GPU mode");
    window::run_window(config)
}
