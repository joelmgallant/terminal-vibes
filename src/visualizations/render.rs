use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// A 2D pixel canvas that maps to Unicode braille characters (U+2800 block).
/// Each terminal cell is a 2x4 dot matrix, giving 2x horizontal and 4x vertical
/// sub-cell resolution.
pub struct BrailleCanvas {
    cols: u16,
    rows: u16,
    /// Flat pixel buffer: pixel_width * pixel_height bits stored as bytes
    pixels: Vec<bool>,
}

// Braille dot positions within a cell:
//   (0,0) (1,0)     bit 0  bit 3
//   (0,1) (1,1)     bit 1  bit 4
//   (0,2) (1,2)     bit 2  bit 5
//   (0,3) (1,3)     bit 6  bit 7
const BRAILLE_DOT_MAP: [[u8; 4]; 2] = [
    [0, 1, 2, 6], // left column (x%2 == 0)
    [3, 4, 5, 7], // right column (x%2 == 1)
];

impl BrailleCanvas {
    pub fn new(cols: u16, rows: u16) -> Self {
        let pw = cols as usize * 2;
        let ph = rows as usize * 4;
        Self {
            cols,
            rows,
            pixels: vec![false; pw * ph],
        }
    }

    pub fn pixel_width(&self) -> usize {
        self.cols as usize * 2
    }

    pub fn pixel_height(&self) -> usize {
        self.rows as usize * 4
    }

    pub fn set(&mut self, x: usize, y: usize) {
        let pw = self.pixel_width();
        let ph = self.pixel_height();
        if x < pw && y < ph {
            self.pixels[y * pw + x] = true;
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(false);
    }

    /// Render the pixel buffer into a ratatui Buffer using braille characters.
    pub fn render(&self, area: &Rect, buf: &mut Buffer, color: Color) {
        let render_cols = self.cols.min(area.width);
        let render_rows = self.rows.min(area.height);

        for cy in 0..render_rows {
            for cx in 0..render_cols {
                let mut code: u8 = 0;
                for dx in 0..2usize {
                    for dy in 0..4usize {
                        let px = cx as usize * 2 + dx;
                        let py = cy as usize * 4 + dy;
                        if px < self.pixel_width()
                            && py < self.pixel_height()
                            && self.pixels[py * self.pixel_width() + px]
                        {
                            code |= 1 << BRAILLE_DOT_MAP[dx][dy];
                        }
                    }
                }
                let ch = char::from_u32(0x2800 + code as u32).unwrap_or(' ');
                buf[(area.x + cx, area.y + cy)].set_char(ch).set_fg(color);
            }
        }
    }
}

// --- Common math helpers ---

/// Linear interpolation between a and b.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Hermite smoothstep (smooth 0->1 curve).
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
