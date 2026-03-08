use anyhow::Result;

mod audio;
mod config;
mod processing;
mod ui;
mod visualizations;

fn main() -> Result<()> {
    env_logger::init();
    println!("terminal-vibes");
    Ok(())
}
