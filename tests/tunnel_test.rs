use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::tunnel::Tunnel;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_tunnel_name() {
    let viz = Tunnel::new();
    assert_eq!(viz.name(), "tunnel");
}

#[test]
fn test_tunnel_render_empty_no_panic() {
    let viz = Tunnel::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_tunnel_render_zero_area_no_panic() {
    let viz = Tunnel::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_tunnel_update_and_render() {
    let mut viz = Tunnel::new();
    let frame = FrameData {
        spectrum: vec![0.7; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_tunnel_rings_expand_over_time() {
    let mut viz = Tunnel::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.7,
        rms: 0.5,
        beat: Default::default(),
    };

    // Multiple updates to let rings expand
    for _ in 0..30 {
        viz.update(&frame);
    }
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    // Should have drawn content
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)]
                .symbol()
                .chars()
                .next()
                .unwrap_or(' ');
            ch != ' '
        })
    });
    assert!(has_content, "Tunnel should render visible rings");
}
