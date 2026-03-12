use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::spectrum::SpectrumBars;
use terminal_vibes::visualizations::Visualization;

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
    let mut viz = SpectrumBars::new();
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

#[test]
fn test_spectrum_bars_reuses_buffer_across_updates() {
    let mut viz = SpectrumBars::new();
    let frame1 = FrameData {
        spectrum: vec![0.1, 0.2, 0.3, 0.4],
        waveform: vec![],
        peak: 0.4,
        rms: 0.2,
        beat: Default::default(),
    };
    viz.update(&frame1);

    let frame2 = FrameData {
        spectrum: vec![0.9, 0.8, 0.7, 0.6],
        waveform: vec![],
        peak: 0.9,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame2);

    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
