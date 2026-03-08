use super::Visualization;
use crate::processing::FrameData;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub struct VisualizationRegistry {
    plugins: Vec<Box<dyn Visualization>>,
    current_index: usize,
}

impl VisualizationRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            current_index: 0,
        }
    }

    pub fn register(&mut self, viz: Box<dyn Visualization>) {
        self.plugins.push(viz);
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn current(&self) -> Option<&dyn Visualization> {
        if self.current_index < self.plugins.len() {
            Some(&*self.plugins[self.current_index])
        } else {
            None
        }
    }

    pub fn current_mut(&mut self) -> Option<&mut dyn Visualization> {
        if self.current_index < self.plugins.len() {
            Some(&mut *self.plugins[self.current_index])
        } else {
            None
        }
    }

    pub fn next(&mut self) {
        if !self.plugins.is_empty() {
            self.current_index = (self.current_index + 1) % self.plugins.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.plugins.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.plugins.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    pub fn update_current(&mut self, frame: &FrameData) {
        if let Some(viz) = self.current_mut() {
            viz.update(frame);
        }
    }

    pub fn render_current(&self, area: Rect, buf: &mut Buffer) {
        if let Some(viz) = self.current() {
            viz.render(area, buf);
        }
    }

    pub fn on_key_current(&mut self, key: KeyEvent) -> bool {
        if let Some(viz) = self.current_mut() {
            viz.on_key(key)
        } else {
            false
        }
    }

    pub fn current_name(&self) -> Option<&str> {
        self.current().map(|v| v.name())
    }

    pub fn select_by_name(&mut self, name: &str) {
        if let Some(idx) = self.plugins.iter().position(|v| v.name() == name) {
            self.current_index = idx;
        }
    }

    /// Save all visualization states as a TOML table keyed by name.
    pub fn save_all(&self) -> toml::value::Table {
        let mut table = toml::value::Table::new();
        for viz in &self.plugins {
            let config = viz.save_config();
            if let toml::Value::Table(t) = config {
                if !t.is_empty() {
                    table.insert(viz.name().to_string(), toml::Value::Table(t));
                }
            }
        }
        table
    }

    /// Load saved state into all visualizations.
    pub fn load_all(&mut self, table: &toml::value::Table) {
        for viz in &mut self.plugins {
            if let Some(config) = table.get(viz.name()) {
                viz.apply_config(config);
            }
        }
    }
}
