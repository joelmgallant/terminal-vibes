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
}
