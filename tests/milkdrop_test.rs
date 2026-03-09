use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use terminal_vibes::processing::FrameData;
use terminal_vibes::visualizations::milkdrop::Milkdrop;
use terminal_vibes::visualizations::Visualization;

#[test]
fn test_milkdrop_name() {
    let viz = Milkdrop::new();
    assert_eq!(viz.name(), "milkdrop");
}

#[test]
fn test_milkdrop_render_empty_no_panic() {
    let mut viz = Milkdrop::new();
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_milkdrop_render_zero_area_no_panic() {
    let mut viz = Milkdrop::new();
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_milkdrop_with_audio_data() {
    let mut viz = Milkdrop::new();
    let frame = FrameData {
        spectrum: vec![0.5; 128],
        waveform: vec![0.3; 1024],
        peak: 0.7,
        rms: 0.5,
        ..Default::default()
    };
    viz.update(&frame);
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);
}

#[test]
fn test_milkdrop_feedback_persists() {
    let mut viz = Milkdrop::new();
    let frame = FrameData {
        spectrum: vec![0.8; 128],
        waveform: vec![0.5; 1024],
        peak: 0.9,
        rms: 0.7,
        ..Default::default()
    };

    // Render several frames to build up feedback
    let area = Rect::new(0, 0, 40, 20);
    for _ in 0..10 {
        viz.update(&frame);
        let mut buf = Buffer::empty(area);
        viz.render(area, &mut buf);
    }

    // Now render with silence — feedback should still show traces
    let silent = FrameData {
        spectrum: vec![0.0; 128],
        waveform: vec![0.0; 1024],
        peak: 0.0,
        rms: 0.0,
        ..Default::default()
    };
    viz.update(&silent);
    let mut buf = Buffer::empty(area);
    viz.render(area, &mut buf);

    // At least some cells should still have color from decaying feedback
    let has_color = (0..20u16).any(|y| {
        (0..40u16).any(|x| {
            let fg = buf[(x, y)].fg;
            !matches!(
                fg,
                ratatui::style::Color::Reset | ratatui::style::Color::Rgb(0, 0, 0)
            )
        })
    });
    assert!(
        has_color,
        "Feedback should persist after silence — trails should still be visible"
    );
}

#[test]
fn test_milkdrop_heavy_rendering() {
    let viz = Milkdrop::new();
    assert!(
        viz.heavy_rendering(),
        "milkdrop should report heavy rendering"
    );
}
