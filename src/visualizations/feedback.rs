#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum BlendMode {
    /// dst + src (clamped). Trails glow, colors accumulate.
    Additive,
    /// src replaces dst. Opaque overlay.
    Alpha,
    /// max(dst, src) per channel. Brighten only.
    Max,
}

/// Double-buffered float RGB canvas for MilkDrop-style feedback rendering.
///
/// Two buffers at HalfBlockCanvas pixel resolution (cols × rows×2):
/// - **Front**: read source (previous frame)
/// - **Back**: write target (current frame being built)
///
/// Float RGB (0.0–1.0) gives smooth fades. Memory: ~86KB at 200×120.
#[allow(dead_code)]
pub struct FeedbackCanvas {
    width: usize,
    height: usize, // rows * 2 (half-block vertical resolution)
    front: Vec<(f32, f32, f32)>,
    back: Vec<(f32, f32, f32)>,
}

#[allow(dead_code)]
impl FeedbackCanvas {
    pub fn new(cols: u16, rows: u16) -> Self {
        let width = cols as usize;
        let height = rows as usize * 2;
        let size = width * height;
        Self {
            width,
            height,
            front: vec![(0.0, 0.0, 0.0); size],
            back: vec![(0.0, 0.0, 0.0); size],
        }
    }

    pub fn pixel_width(&self) -> usize {
        self.width
    }

    pub fn pixel_height(&self) -> usize {
        self.height
    }

    /// Resize buffers if dimensions changed. Clears both buffers on resize.
    /// If dimensions match, does nothing (feedback persists).
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let width = cols as usize;
        let height = rows as usize * 2;
        if width != self.width || height != self.height {
            let size = width * height;
            self.width = width;
            self.height = height;
            self.front = vec![(0.0, 0.0, 0.0); size];
            self.back = vec![(0.0, 0.0, 0.0); size];
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(y * self.width + x)
        } else {
            None
        }
    }

    #[inline]
    pub fn set_back(&mut self, x: usize, y: usize, color: (f32, f32, f32)) {
        if let Some(i) = self.idx(x, y) {
            self.back[i] = color;
        }
    }

    #[inline]
    pub fn get_back(&self, x: usize, y: usize) -> (f32, f32, f32) {
        self.idx(x, y)
            .map(|i| self.back[i])
            .unwrap_or((0.0, 0.0, 0.0))
    }

    #[inline]
    pub fn get_front(&self, x: usize, y: usize) -> (f32, f32, f32) {
        self.idx(x, y)
            .map(|i| self.front[i])
            .unwrap_or((0.0, 0.0, 0.0))
    }

    /// Swap front and back buffers. Previous frame's output becomes the
    /// read source. Back buffer is zeroed for the new frame.
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
        for pixel in &mut self.back {
            *pixel = (0.0, 0.0, 0.0);
        }
    }

    /// Multiply all back-buffer pixels by factor (0.0–1.0).
    /// Controls trail length: 0.95 = long trails, 0.8 = short trails.
    pub fn decay(&mut self, factor: f32) {
        for pixel in &mut self.back {
            pixel.0 *= factor;
            pixel.1 *= factor;
            pixel.2 *= factor;
        }
    }

    /// Paint a pixel onto the back buffer with the given blend mode.
    #[inline]
    pub fn paint(&mut self, x: usize, y: usize, color: (f32, f32, f32), blend: BlendMode) {
        let Some(i) = self.idx(x, y) else { return };
        let dst = &mut self.back[i];
        match blend {
            BlendMode::Additive => {
                dst.0 = (dst.0 + color.0).min(1.0);
                dst.1 = (dst.1 + color.1).min(1.0);
                dst.2 = (dst.2 + color.2).min(1.0);
            }
            BlendMode::Alpha => {
                *dst = color;
            }
            BlendMode::Max => {
                dst.0 = dst.0.max(color.0);
                dst.1 = dst.1.max(color.1);
                dst.2 = dst.2.max(color.2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_zeroed_buffers() {
        let fb = FeedbackCanvas::new(10, 5);
        assert_eq!(fb.pixel_width(), 10);
        assert_eq!(fb.pixel_height(), 10); // 5 rows * 2 (half-block resolution)
        assert_eq!(fb.get_back(0, 0), (0.0, 0.0, 0.0));
        assert_eq!(fb.get_back(9, 9), (0.0, 0.0, 0.0));
    }

    #[test]
    fn test_resize_changes_dimensions() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.resize(20, 10);
        assert_eq!(fb.pixel_width(), 20);
        assert_eq!(fb.pixel_height(), 20);
        assert_eq!(fb.get_back(19, 19), (0.0, 0.0, 0.0));
    }

    #[test]
    fn test_resize_same_dims_clears() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(5, 5, (1.0, 0.0, 0.0));
        fb.resize(10, 5);
        // resize with same dims should NOT clear — feedback persists
        // only clears when dimensions actually change
        assert_eq!(fb.get_back(5, 5), (1.0, 0.0, 0.0));
    }

    #[test]
    fn test_set_and_get_back() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(3, 4, (0.5, 0.7, 0.3));
        assert_eq!(fb.get_back(3, 4), (0.5, 0.7, 0.3));
    }

    #[test]
    fn test_out_of_bounds_ignored() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(100, 100, (1.0, 1.0, 1.0)); // should not panic
        assert_eq!(fb.get_back(100, 100), (0.0, 0.0, 0.0)); // out of bounds returns black
    }

    #[test]
    fn test_swap_moves_back_to_front() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(3, 3, (1.0, 0.5, 0.0));
        assert_eq!(fb.get_front(3, 3), (0.0, 0.0, 0.0)); // front is still empty
        fb.swap();
        assert_eq!(fb.get_front(3, 3), (1.0, 0.5, 0.0)); // now it's in front
    }

    #[test]
    fn test_swap_clears_back() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(3, 3, (1.0, 0.5, 0.0));
        fb.swap();
        assert_eq!(fb.get_back(3, 3), (0.0, 0.0, 0.0)); // back was cleared
    }

    #[test]
    fn test_decay_fades_back_buffer() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(0, 0, (1.0, 1.0, 1.0));
        fb.decay(0.9);
        let (r, g, b) = fb.get_back(0, 0);
        assert!((r - 0.9).abs() < 0.001);
        assert!((g - 0.9).abs() < 0.001);
        assert!((b - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_decay_repeated_converges_to_zero() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(0, 0, (1.0, 1.0, 1.0));
        for _ in 0..100 {
            fb.decay(0.9);
        }
        let (r, g, b) = fb.get_back(0, 0);
        assert!(r < 0.001, "should fade to near-zero, got {r}");
        assert!(g < 0.001);
        assert!(b < 0.001);
    }

    #[test]
    fn test_paint_additive() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(0, 0, (0.3, 0.2, 0.1));
        fb.paint(0, 0, (0.2, 0.3, 0.4), BlendMode::Additive);
        let (r, g, b) = fb.get_back(0, 0);
        assert!((r - 0.5).abs() < 0.001);
        assert!((g - 0.5).abs() < 0.001);
        assert!((b - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_paint_additive_clamps() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(0, 0, (0.8, 0.9, 1.0));
        fb.paint(0, 0, (0.5, 0.5, 0.5), BlendMode::Additive);
        let (r, g, b) = fb.get_back(0, 0);
        assert!(r <= 1.0);
        assert!(g <= 1.0);
        assert!(b <= 1.0);
    }

    #[test]
    fn test_paint_alpha_replaces() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(0, 0, (1.0, 1.0, 1.0));
        fb.paint(0, 0, (0.2, 0.3, 0.4), BlendMode::Alpha);
        let (r, g, b) = fb.get_back(0, 0);
        assert!((r - 0.2).abs() < 0.001);
        assert!((g - 0.3).abs() < 0.001);
        assert!((b - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_paint_max_brightens_only() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(0, 0, (0.5, 0.8, 0.3));
        fb.paint(0, 0, (0.7, 0.2, 0.9), BlendMode::Max);
        let (r, g, b) = fb.get_back(0, 0);
        assert!((r - 0.7).abs() < 0.001); // 0.7 > 0.5
        assert!((g - 0.8).abs() < 0.001); // 0.8 > 0.2
        assert!((b - 0.9).abs() < 0.001); // 0.9 > 0.3
    }

    #[test]
    fn test_paint_out_of_bounds_no_panic() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.paint(100, 100, (1.0, 1.0, 1.0), BlendMode::Additive);
    }
}
