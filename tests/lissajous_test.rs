use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::lissajous::Lissajous;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_lissajous_name() {
    let viz = Lissajous::new();
    assert_eq!(viz.name(), "lissajous");
}

#[test]
fn test_lissajous_render_empty_no_panic() {
    let mut viz = Lissajous::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_lissajous_render_zero_area_no_panic() {
    let mut viz = Lissajous::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_lissajous_update_and_render() {
    let mut viz = Lissajous::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 2048],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Should have drawn some braille characters (not all spaces)
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)]
                .symbol()
                .chars()
                .next()
                .unwrap_or(' ');
            ch != ' ' && ch != '\u{2800}'
        })
    });
    assert!(
        has_content,
        "Lissajous should render visible content with non-zero input"
    );
}

#[test]
fn test_lissajous_evolves_over_updates() {
    let mut viz = Lissajous::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.0; 2048],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };

    // Capture render after 1 update
    viz.update(&frame);
    let area = Rect::new(0, 0, 40, 12);
    let mut buf1 = Buffer::empty(area);
    viz.render(area, &mut buf1);

    // Capture render after 60 more updates (simulating 1 second)
    for _ in 0..60 {
        viz.update(&frame);
    }
    let mut buf2 = Buffer::empty(area);
    viz.render(area, &mut buf2);

    // The two renders should differ (pattern evolves over time)
    let differs = (0..12).any(|y| {
        (0..40).any(|x| buf1[(x as u16, y as u16)].symbol() != buf2[(x as u16, y as u16)].symbol())
    });
    assert!(differs, "Lissajous pattern should evolve over time");
}
