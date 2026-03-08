use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::starfield::Starfield;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_starfield_name() {
    let viz = Starfield::new();
    assert_eq!(viz.name(), "starfield");
}

#[test]
fn test_starfield_render_empty_no_panic() {
    let viz = Starfield::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_starfield_render_zero_area_no_panic() {
    let viz = Starfield::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_starfield_particles_appear_after_updates() {
    let mut viz = Starfield::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.8,
        rms: 0.5,
        beat: Default::default(),
    };
    // Run several updates so particles have time to spread
    for _ in 0..60 {
        viz.update(&frame);
    }
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
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
        "Starfield should have visible particles after updates"
    );
}
