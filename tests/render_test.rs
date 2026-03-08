use terminal_vibes::visualizations::render::{BrailleCanvas, HalfBlockCanvas};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[test]
fn test_braille_canvas_dimensions() {
    let canvas = BrailleCanvas::new(80, 24);
    // 80 cols * 2 = 160 pixel width, 24 rows * 4 = 96 pixel height
    assert_eq!(canvas.pixel_width(), 160);
    assert_eq!(canvas.pixel_height(), 96);
}

#[test]
fn test_braille_canvas_empty_renders_blanks() {
    let canvas = BrailleCanvas::new(4, 2);
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
    // All cells should be braille blank (U+2800) or space
    for y in 0..2 {
        for x in 0..4 {
            let cell = &buf[(x, y)];
            let ch = cell.symbol().chars().next().unwrap();
            assert!(ch == '\u{2800}' || ch == ' ',
                "Expected braille blank at ({x},{y}), got {:?}", ch);
        }
    }
}

#[test]
fn test_braille_canvas_set_pixel_renders_dot() {
    let mut canvas = BrailleCanvas::new(4, 2);
    canvas.set(0, 0); // top-left dot of cell (0,0)
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
    let ch = buf[(0u16, 0u16)].symbol().chars().next().unwrap();
    assert_ne!(ch, '\u{2800}', "Expected a dot, got blank braille");
    assert_ne!(ch, ' ', "Expected a dot, got space");
}

#[test]
fn test_braille_canvas_out_of_bounds_no_panic() {
    let mut canvas = BrailleCanvas::new(4, 2);
    canvas.set(999, 999); // should silently ignore
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
}

#[test]
fn test_braille_canvas_clear() {
    let mut canvas = BrailleCanvas::new(4, 2);
    canvas.set(0, 0);
    canvas.clear();
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf, Color::White);
    let ch = buf[(0u16, 0u16)].symbol().chars().next().unwrap();
    assert!(ch == '\u{2800}' || ch == ' ');
}

#[test]
fn test_half_block_canvas_dimensions() {
    let canvas = HalfBlockCanvas::new(40, 12);
    assert_eq!(canvas.pixel_width(), 40);
    assert_eq!(canvas.pixel_height(), 24); // 12 rows * 2
}

#[test]
fn test_half_block_canvas_set_and_render() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 0, Color::Red);   // top pixel of cell (0,0)
    canvas.set(0, 1, Color::Blue);  // bottom pixel of cell (0,0)
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    // Cell (0,0) should have fg=Red (upper), bg=Blue (lower), char=▀
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), "\u{2580}");
}

#[test]
fn test_half_block_canvas_clear() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 0, Color::Red);
    canvas.clear();
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    // After clear, cell should have default (no color set)
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), " ");
}

#[test]
fn test_half_block_canvas_only_top_pixel() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 0, Color::Green); // only top pixel
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), "\u{2580}");
}

#[test]
fn test_half_block_canvas_only_bottom_pixel() {
    let mut canvas = HalfBlockCanvas::new(4, 2);
    canvas.set(0, 1, Color::Green); // only bottom pixel
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    canvas.render(&area, &mut buf);
    let cell = &buf[(0u16, 0u16)];
    assert_eq!(cell.symbol(), "\u{2584}");
}
