// Re-export core modules so integration tests and main.rs keep working
// with `terminal_vibes::processing::FrameData` etc.
pub use terminal_vibes_core::audio;
pub use terminal_vibes_core::beat;
pub use terminal_vibes_core::config;
pub use terminal_vibes_core::processing;

pub mod ui;
pub mod visualizations;
