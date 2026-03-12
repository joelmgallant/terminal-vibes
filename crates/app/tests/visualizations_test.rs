use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::aurora::Aurora;
use terminal_vibes::visualizations::lissajous::Lissajous;
use terminal_vibes::visualizations::plasma::Plasma;
use terminal_vibes::visualizations::radial::RadialSpectrum;
use terminal_vibes::visualizations::rain::Rain;
use terminal_vibes::visualizations::registry::VisualizationRegistry;
use terminal_vibes::visualizations::starfield::Starfield;
use terminal_vibes::visualizations::tunnel::Tunnel;
use terminal_vibes::visualizations::Visualization;
/// A mock visualization for testing the registry.
struct MockViz {
    name: String,
    updated: bool,
}

impl MockViz {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            updated: false,
        }
    }
}

impl Visualization for MockViz {
    fn name(&self) -> &str {
        &self.name
    }
    fn update(&mut self, _frame: &FrameData) {
        self.updated = true;
    }
    fn render(&mut self, _area: Rect, _buf: &mut Buffer) {}
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

#[test]
fn test_registry_with_all_visualizations() {
    let mut registry = VisualizationRegistry::new();
    registry.register(Box::new(MockViz::new("spectrum")));
    registry.register(Box::new(MockViz::new("waveform")));
    registry.register(Box::new(MockViz::new("spectrogram")));
    registry.register(Box::new(Lissajous::new()));
    registry.register(Box::new(Tunnel::new()));
    registry.register(Box::new(RadialSpectrum::new()));
    registry.register(Box::new(Plasma::new()));
    registry.register(Box::new(Aurora::new()));
    registry.register(Box::new(Starfield::new()));
    registry.register(Box::new(Rain::new()));

    assert_eq!(registry.len(), 10);

    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 2048],
        peak: 0.7,
        rms: 0.4,
        beat: Default::default(),
    };

    // Cycle through all 10 and verify update+render doesn't panic
    let area = Rect::new(0, 0, 80, 24);
    for _ in 0..10 {
        registry.update_current(&frame);
        let mut buf = Buffer::empty(area);
        registry.render_current(area, &mut buf);
        registry.next();
    }
}
