use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::aurora::Aurora;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_aurora_name() {
    let viz = Aurora::new();
    assert_eq!(viz.name(), "aurora");
}

#[test]
fn test_aurora_render_empty_no_panic() {
    let viz = Aurora::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_aurora_render_zero_area_no_panic() {
    let viz = Aurora::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_aurora_update_and_render() {
    let mut viz = Aurora::new();
    let frame = FrameData {
        spectrum: vec![0.6; 128],
        waveform: vec![],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 60, 20);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Should have some colored content
    let has_content = (0..20u16).any(|y| (0..60u16).any(|x| buf[(x, y)].symbol() != " "));
    assert!(has_content, "Aurora should render visible curtains");
}
