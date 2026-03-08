use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::plasma::Plasma;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_plasma_name() {
    let viz = Plasma::new();
    assert_eq!(viz.name(), "plasma");
}

#[test]
fn test_plasma_render_empty_no_panic() {
    let mut viz = Plasma::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_plasma_render_zero_area_no_panic() {
    let mut viz = Plasma::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_plasma_fills_area_with_color() {
    let mut viz = Plasma::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.7,
        rms: 0.5,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 20, 10);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Plasma should fill every cell (no empty spaces)
    let empty_count = (0..10u16)
        .flat_map(|y| (0..20u16).map(move |x| (x, y)))
        .filter(|&(x, y)| buf[(x, y)].symbol() == " ")
        .count();
    assert_eq!(empty_count, 0, "Plasma should fill every cell");
}

#[test]
fn test_plasma_evolves_over_time() {
    let mut viz = Plasma::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.5,
        rms: 0.3,
        beat: Default::default(),
    };

    viz.update(&frame);
    let area = Rect::new(0, 0, 20, 10);
    let mut buf1 = Buffer::empty(area);
    viz.render(area, &mut buf1);

    for _ in 0..30 {
        viz.update(&frame);
    }
    let mut buf2 = Buffer::empty(area);
    viz.render(area, &mut buf2);

    // Colors should differ between frames
    let differs = (0..10u16).any(|y| (0..20u16).any(|x| buf1[(x, y)].fg != buf2[(x, y)].fg));
    assert!(differs, "Plasma should evolve over time");
}
