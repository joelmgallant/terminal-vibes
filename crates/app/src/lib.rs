pub use terminal_vibes_core::audio;
pub use terminal_vibes_core::beat;
pub use terminal_vibes_core::config;
pub use terminal_vibes_core::processing;

pub mod pipeline;
pub mod terminal;

// Re-export for test backward compatibility
pub use terminal::{ui, visualizations};

#[cfg(feature = "gui")]
pub mod gpu;
