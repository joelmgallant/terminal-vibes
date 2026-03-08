use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::registry::VisualizationRegistry;
use terminal_vibes::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
/// A mock visualization for testing the registry.
struct MockViz {
    name: String,
    updated: bool,
}

impl MockViz {
    fn new(name: &str) -> Self {
        Self { name: name.to_string(), updated: false }
    }
}

impl Visualization for MockViz {
    fn name(&self) -> &str { &self.name }
    fn update(&mut self, _frame: &FrameData) { self.updated = true; }
    fn render(&self, _area: Rect, _buf: &mut Buffer) {}
}

#[test]
fn test_registry_starts_empty() {
    let registry = VisualizationRegistry::new();
    assert_eq!(registry.len(), 0);
    assert!(registry.current().is_none());
}

#[test]
fn test_registry_register_and_access() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("test1")));
    registry.register(Box::new(MockViz::new("test2")));
    assert_eq!(registry.len(), 2);
    assert_eq!(registry.current().unwrap().name(), "test1");
}

#[test]
fn test_registry_cycle_next() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("a")));
    registry.register(Box::new(MockViz::new("b")));
    registry.register(Box::new(MockViz::new("c")));

    assert_eq!(registry.current().unwrap().name(), "a");
    registry.next();
    assert_eq!(registry.current().unwrap().name(), "b");
    registry.next();
    assert_eq!(registry.current().unwrap().name(), "c");
    registry.next();
    // wraps around
    assert_eq!(registry.current().unwrap().name(), "a");
}

#[test]
fn test_registry_cycle_prev() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("a")));
    registry.register(Box::new(MockViz::new("b")));
    registry.register(Box::new(MockViz::new("c")));

    assert_eq!(registry.current().unwrap().name(), "a");
    registry.prev();
    // wraps around to end
    assert_eq!(registry.current().unwrap().name(), "c");
    registry.prev();
    assert_eq!(registry.current().unwrap().name(), "b");
}

#[test]
fn test_registry_update_current() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("test")));

    let frame = FrameData::default();
    registry.update_current(&frame);

    // We can't easily inspect `updated` through the trait, but the call shouldn't panic.
}
