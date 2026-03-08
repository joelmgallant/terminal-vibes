pub mod registry;
pub mod spectrogram;
pub mod spectrum;
pub mod waveform;

use crate::processing::FrameData;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub trait Visualization: Send {
    /// Unique name shown in status bar and config.
    fn name(&self) -> &str;

    /// Process a new frame of audio data (update internal state).
    fn update(&mut self, frame: &FrameData);

    /// Render into the given ratatui buffer area.
    fn render(&self, area: Rect, buf: &mut Buffer);

    /// Handle a keypress specific to this visualization. Returns true if handled.
    fn on_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    /// Provide default config for this visualization.
    fn default_config(&self) -> toml::Value {
        toml::Value::Table(Default::default())
    }

    /// Apply config values.
    fn apply_config(&mut self, _config: &toml::Value) {}
}
