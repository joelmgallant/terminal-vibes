use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::rain::Rain;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_rain_name() {
    let viz = Rain::new();
    assert_eq!(viz.name(), "rain");
}

#[test]
fn test_rain_render_empty_no_panic() {
    let viz = Rain::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_rain_render_zero_area_no_panic() {
    let viz = Rain::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_rain_streams_appear_after_updates() {
    let mut viz = Rain::new();
    let frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        beat: Default::default(),
    };
    // Run enough updates for drops to appear and fall
    for _ in 0..30 {
        viz.update(&frame);
    }
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    let has_content = (0..24u16).any(|y| {
        (0..80u16).any(|x| {
            buf[(x, y)].symbol() != " "
        })
    });
    assert!(has_content, "Rain should have visible streams after updates");
}
