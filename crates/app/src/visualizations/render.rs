use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Quantize an RGB color to reduce unique terminal escape sequences.
///
/// Reduces the galaxy of possible 24-bit colors to a smaller set so that
/// ratatui's frame diff sees more unchanged cells, dramatically cutting
/// escape sequence volume in terminal multiplexers like tmux.
#[inline]
pub fn quantize_color(color: Color, step: u8) -> Color {
    let step = step.max(1);
    match color {
        Color::Rgb(r, g, b) => Color::Rgb((r / step) * step, (g / step) * step, (b / step) * step),
        other => other,
    }
}

/// Compute quantization step based on terminal cell count and user color detail preference.
/// Higher cell count → coarser step (more perf). Higher detail → finer step (more fidelity).
pub fn adaptive_quantization_step(cell_count: u32, color_detail: f32) -> u8 {
    let base = if cell_count < 4000 {
        16u8
    } else if cell_count < 10000 {
        24u8
    } else {
        32u8
    };
    let adjusted = (base as f32 / color_detail).round() as u8;
    adjusted.clamp(4, 64)
}

/// A 2D pixel canvas that maps to Unicode braille characters (U+2800 block).
/// Each terminal cell is a 2x4 dot matrix, giving 2x horizontal and 4x vertical
/// sub-cell resolution.
pub struct BrailleCanvas {
    cols: u16,
    rows: u16,
    step: u8,
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

#[allow(dead_code)]
impl BrailleCanvas {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self::with_step(cols, rows, 16)
    }

    pub fn with_step(cols: u16, rows: u16, step: u8) -> Self {
        let pw = cols as usize * 2;
        let ph = rows as usize * 4;
        Self {
            cols,
            rows,
            step,
            pixels: vec![false; pw * ph],
        }
    }

    /// Reuse this canvas if dimensions match, otherwise reallocate.
    /// Avoids per-frame heap allocations when the terminal size is stable.
    pub fn resize_or_clear(&mut self, cols: u16, rows: u16) {
        if self.cols != cols || self.rows != rows {
            *self = Self::with_step(cols, rows, self.step);
        } else {
            self.clear();
        }
    }

    pub fn set_step(&mut self, step: u8) {
        self.step = step;
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
        let color = quantize_color(color, self.step);
        let render_cols = self.cols.min(area.width);
        let render_rows = self.rows.min(area.height);

        for cy in 0..render_rows {
            for cx in 0..render_cols {
                let mut code: u8 = 0;
                #[allow(clippy::needless_range_loop)]
                for dx in 0..2usize {
                    #[allow(clippy::needless_range_loop)]
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
#[allow(dead_code)]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Hermite smoothstep (smooth 0->1 curve).
#[allow(dead_code)]
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

use std::f32::consts::TAU;
use std::sync::LazyLock;

const SIN_LUT_SIZE: usize = 4096;

/// Pre-computed sine lookup table for fast O(1) trig approximation.
/// 4096 entries cover one full period (0..TAU) with max error < 0.001.
pub struct SinLut {
    table: [f32; SIN_LUT_SIZE],
}

impl SinLut {
    fn new() -> Self {
        let mut table = [0.0; SIN_LUT_SIZE];
        for (i, val) in table.iter_mut().enumerate() {
            *val = (TAU * i as f32 / SIN_LUT_SIZE as f32).sin();
        }
        Self { table }
    }

    #[inline]
    pub fn get(&self, radians: f32) -> f32 {
        let normalized = radians.rem_euclid(TAU) / TAU;
        let index = (normalized * SIN_LUT_SIZE as f32) as usize % SIN_LUT_SIZE;
        self.table[index]
    }
}

pub static SIN_LUT: LazyLock<SinLut> = LazyLock::new(SinLut::new);

/// A 2D color canvas at 2x vertical resolution using half-block characters.
/// Each terminal cell encodes two vertical "pixels" via fg/bg color.
/// Upper pixel uses foreground color with ▀, lower uses background color.
pub struct HalfBlockCanvas {
    cols: u16,
    rows: u16,
    step: u8,
    /// Color per pixel: pixel_width * pixel_height, None = unset
    pub(crate) pixels: Vec<Option<Color>>,
}

#[allow(dead_code)]
impl HalfBlockCanvas {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self::with_step(cols, rows, 16)
    }

    pub fn with_step(cols: u16, rows: u16, step: u8) -> Self {
        let pw = cols as usize;
        let ph = rows as usize * 2;
        Self {
            cols,
            rows,
            step,
            pixels: vec![None; pw * ph],
        }
    }

    /// Reuse this canvas if dimensions match, otherwise reallocate.
    /// Avoids per-frame heap allocations when the terminal size is stable.
    pub fn resize_or_clear(&mut self, cols: u16, rows: u16) {
        if self.cols != cols || self.rows != rows {
            *self = Self::with_step(cols, rows, self.step);
        } else {
            self.clear();
        }
    }

    pub fn set_step(&mut self, step: u8) {
        self.step = step;
    }

    pub fn pixel_width(&self) -> usize {
        self.cols as usize
    }

    pub fn pixel_height(&self) -> usize {
        self.rows as usize * 2
    }

    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        let pw = self.pixel_width();
        let ph = self.pixel_height();
        if x < pw && y < ph {
            self.pixels[y * pw + x] = Some(quantize_color(color, self.step));
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(None);
    }

    /// Render the color buffer into a ratatui Buffer using half-block characters.
    /// Fills the entire area with a black background for consistency.
    pub fn render(&self, area: &Rect, buf: &mut Buffer) {
        let black = Color::Rgb(0, 0, 0);

        // Fill entire area with black background first
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &mut buf[(area.x + x, area.y + y)];
                cell.set_char(' ').set_bg(black);
            }
        }

        let render_cols = self.cols.min(area.width);
        let render_rows = self.rows.min(area.height);

        for cy in 0..render_rows {
            for cx in 0..render_cols {
                let top_idx = (cy as usize * 2) * self.pixel_width() + cx as usize;
                let bot_idx = (cy as usize * 2 + 1) * self.pixel_width() + cx as usize;
                let top = self.pixels[top_idx];
                let bot = self.pixels[bot_idx];

                let cell = &mut buf[(area.x + cx, area.y + cy)];
                match (top, bot) {
                    (Some(tc), Some(bc)) if tc == bc => {
                        cell.set_char('\u{2588}').set_fg(tc).set_bg(black);
                    }
                    (Some(tc), Some(bc)) => {
                        cell.set_char('\u{2580}').set_fg(tc).set_bg(bc);
                    }
                    (Some(tc), None) => {
                        cell.set_char('\u{2580}').set_fg(tc).set_bg(black);
                    }
                    (None, Some(bc)) => {
                        cell.set_char('\u{2584}').set_fg(bc).set_bg(black);
                    }
                    (None, None) => {} // already black from fill
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn quantize_color_step_16() {
        assert_eq!(
            quantize_color(Color::Rgb(17, 33, 255), 16),
            Color::Rgb(16, 32, 240)
        );
    }

    #[test]
    fn quantize_color_step_32() {
        assert_eq!(
            quantize_color(Color::Rgb(33, 50, 255), 32),
            Color::Rgb(32, 32, 224)
        );
    }

    #[test]
    fn quantize_color_passthrough_non_rgb() {
        assert_eq!(quantize_color(Color::White, 16), Color::White);
    }

    #[test]
    fn sin_lut_accuracy() {
        let lut = SinLut::new();
        assert!((lut.get(0.0) - 0.0_f32.sin()).abs() < 0.002);
        assert!((lut.get(PI / 2.0) - 1.0).abs() < 0.002);
        assert!((lut.get(PI) - 0.0).abs() < 0.002);
        assert!((lut.get(3.0 * PI / 2.0) - (-1.0)).abs() < 0.002);
    }

    #[test]
    fn sin_lut_wraps_negative() {
        let lut = SinLut::new();
        assert!((lut.get(-PI / 2.0) - (-1.0)).abs() < 0.002);
    }

    #[test]
    fn sin_lut_wraps_large() {
        let lut = SinLut::new();
        let val = 100.0 * PI + PI / 2.0;
        assert!((lut.get(val) - val.sin()).abs() < 0.002);
    }

    #[test]
    fn halfblock_canvas_quantizes_at_set_time() {
        let mut canvas = HalfBlockCanvas::with_step(2, 2, 16);
        canvas.set(0, 0, Color::Rgb(17, 33, 255));
        // The stored pixel should already be quantized
        assert_eq!(canvas.pixels[0], Some(Color::Rgb(16, 32, 240)));
    }

    #[test]
    fn halfblock_canvas_custom_step() {
        let mut canvas = HalfBlockCanvas::with_step(2, 2, 32);
        canvas.set(0, 0, Color::Rgb(33, 50, 255));
        assert_eq!(canvas.pixels[0], Some(Color::Rgb(32, 32, 224)));
    }

    #[test]
    fn adaptive_step_small_terminal() {
        // 80x24 = 1920 cells
        assert_eq!(adaptive_quantization_step(1920, 1.0), 16);
    }

    #[test]
    fn adaptive_step_medium_terminal() {
        // 150x50 = 7500 cells
        assert_eq!(adaptive_quantization_step(7500, 1.0), 24);
    }

    #[test]
    fn adaptive_step_large_terminal() {
        // 200x60 = 12000 cells
        assert_eq!(adaptive_quantization_step(12000, 1.0), 32);
    }

    #[test]
    fn adaptive_step_detail_doubles_precision() {
        // Large terminal but detail=2.0 → step halved
        assert_eq!(adaptive_quantization_step(12000, 2.0), 16);
    }

    #[test]
    fn adaptive_step_detail_halves_precision() {
        // Small terminal but detail=0.5 → step doubled
        assert_eq!(adaptive_quantization_step(1920, 0.5), 32);
    }

    #[test]
    fn adaptive_step_clamps_minimum() {
        // Never go below 4 (still 64 values per channel)
        assert_eq!(adaptive_quantization_step(100, 2.0), 8);
    }
}
