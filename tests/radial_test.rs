use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::Visualization;
use terminal_vibes::visualizations::radial::RadialSpectrum;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_radial_name() {
    let viz = RadialSpectrum::new();
    assert_eq!(viz.name(), "radial");
}

#[test]
fn test_radial_render_empty_no_panic() {
    let viz = RadialSpectrum::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_radial_render_zero_area_no_panic() {
    let viz = RadialSpectrum::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_radial_update_and_render() {
    let mut viz = RadialSpectrum::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![],
        peak: 0.7,
        rms: 0.4,
        beat: Default::default(),
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    let has_content = (0..24).any(|y| {
        (0..80).any(|x| {
            let ch = buf[(x as u16, y as u16)].symbol().chars().next().unwrap_or(' ');
            ch != ' '
        })
    });
    assert!(has_content, "RadialSpectrum should render visible content");
}
