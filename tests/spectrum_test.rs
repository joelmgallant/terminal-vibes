use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::spectrum::SpectrumBars;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_spectrum_bars_name() {
    let viz = SpectrumBars::new();
    assert_eq!(viz.name(), "spectrum");
}

#[test]
fn test_spectrum_bars_update_stores_spectrum() {
    let mut viz = SpectrumBars::new();
    let frame = FrameData {
        spectrum: vec![0.5, 0.8, 0.3, 0.9],
        waveform: vec![],
        peak: 0.9,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    // After update, render should not panic
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrum_bars_render_empty_no_panic() {
    let viz = SpectrumBars::new();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrum_bars_render_zero_area_no_panic() {
    let mut viz = SpectrumBars::new();
    let frame = FrameData {
        spectrum: vec![0.5; 16],
        waveform: vec![],
        peak: 0.5,
        rms: 0.3,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
