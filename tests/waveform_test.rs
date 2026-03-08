use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::waveform::Waveform;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_waveform_name() {
    let viz = Waveform::new();
    assert_eq!(viz.name(), "waveform");
}

#[test]
fn test_waveform_update_stores_samples() {
    let mut viz = Waveform::new();
    let frame = FrameData {
        spectrum: vec![],
        waveform: vec![0.0, 0.5, 1.0, 0.5, 0.0, -0.5, -1.0, -0.5],
        peak: 1.0,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_waveform_render_empty_no_panic() {
    let viz = Waveform::new();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_waveform_render_zero_area_no_panic() {
    let viz = Waveform::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
