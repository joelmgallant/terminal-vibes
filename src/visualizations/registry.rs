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

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

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
}
