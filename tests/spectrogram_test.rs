use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::spectrogram::Spectrogram;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_spectrogram_name() {
    let viz = Spectrogram::new(200);
    assert_eq!(viz.name(), "spectrogram");
}

#[test]
fn test_spectrogram_accumulates_history() {
    let mut viz = Spectrogram::new(5);

    for i in 0..7 {
        let frame = FrameData {
            spectrum: vec![i as f32 * 0.1; 8],
            waveform: vec![],
            peak: 0.5,
            rms: 0.3,
        };
        viz.update(&frame);
    }

    // Render should not panic even with history wrapping
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrogram_render_empty_no_panic() {
    let viz = Spectrogram::new(200);
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_spectrogram_render_zero_area_no_panic() {
    let viz = Spectrogram::new(200);
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}
