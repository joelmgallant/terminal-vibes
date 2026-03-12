use crate::config::Config;
use anyhow::Result;

pub fn run(_config: Config) -> Result<()> {
    log::info!("GPU mode starting...");
    anyhow::bail!("GPU mode not yet implemented")
}
