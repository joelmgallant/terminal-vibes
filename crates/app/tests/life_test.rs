use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::life::Life;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_life_name() {
    let viz = Life::new();
    assert_eq!(viz.name(), "life");
}

#[test]
fn test_life_render_zero_area_no_panic() {
    let mut viz = Life::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_life_render_empty_no_panic() {
    let mut viz = Life::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_life_update_stores_audio_state() {
    let mut viz = Life::new();
    let frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        ..Default::default()
    };
    viz.update(&frame);
    // Should not panic, audio state stored internally
}

#[test]
fn test_life_cells_appear_after_updates() {
    let mut viz = Life::new();
    let frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![],
        peak: 0.9,
        rms: 0.6,
        ..Default::default()
    };
    // Run enough updates for cells to spawn and be visible
    let area = Rect::new(0, 0, 80, 24);
    for _ in 0..30 {
        viz.update(&frame);
        let mut buf = Buffer::empty(area);
        viz.render(area, &mut buf);
    }
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
    let has_content = (0..24u16).any(|y| (0..80u16).any(|x| buf[(x, y)].symbol() != " "));
    assert!(
        has_content,
        "Life should have visible cells after updates with audio"
    );
}

#[test]
fn test_life_heavy_rendering_depends_on_mode() {
    let mut viz = Life::new();
    // Default is character mode — not heavy
    assert!(!viz.heavy_rendering());

    // Simulate switching to halfblock via on_key
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE);
    viz.on_key(key);
    assert!(viz.heavy_rendering());

    // Switch again to braille — not heavy
    viz.on_key(key);
    assert!(!viz.heavy_rendering());
}
