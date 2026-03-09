pub mod aurora;
pub mod feedback;
pub mod life;
pub mod lissajous;
pub mod plasma;
pub mod radial;
pub mod rain;
pub mod registry;
pub mod render;
pub mod spectrogram;
pub mod spectrum;
pub mod starfield;
pub mod tunnel;
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
    fn render(&mut self, area: Rect, buf: &mut Buffer);

    /// Handle a keypress specific to this visualization. Returns true if handled.
    fn on_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    /// Whether this visualization generates heavy escape sequence volume
    /// (e.g. full-screen dual-RGB HalfBlockCanvas). When true, rendering
    /// pauses automatically when the terminal pane loses focus.
    fn heavy_rendering(&self) -> bool {
        false
    }

    /// Provide default config for this visualization.
    fn default_config(&self) -> toml::Value {
        toml::Value::Table(Default::default())
    }

    /// Apply config values.
    fn apply_config(&mut self, _config: &toml::Value) {}

    /// Set the quantization step for color reduction (adaptive detail control).
    fn set_quantization_step(&mut self, _step: u8) {}

    /// Return current runtime state for persistence.
    fn save_config(&self) -> toml::Value {
        self.default_config()
    }
}
