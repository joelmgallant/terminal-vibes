pub mod renderer;
pub mod window;

use crate::config::Config;
use anyhow::Result;

pub fn run(_config: Config) -> Result<()> {
    let width = 1280; // Will come from config in Task 12
    let height = 720;
    log::info!("Starting GPU mode ({}x{})", width, height);
    window::run_window(width, height)
}
