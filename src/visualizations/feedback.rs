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

/// Coarse displacement grid for warp transforms.
/// Each grid point stores a (dx, dy) displacement vector.
/// Pixels between grid points interpolate displacement bilinearly.
#[allow(dead_code)]
pub struct WarpGrid {
    pub grid_w: usize,
    pub grid_h: usize,
    pub displacements: Vec<(f32, f32)>,
}

#[allow(dead_code)]
impl WarpGrid {
    pub fn new(grid_w: usize, grid_h: usize) -> Self {
        Self {
            grid_w,
            grid_h,
            displacements: vec![(0.0, 0.0); grid_w * grid_h],
        }
    }

    #[inline]
    pub fn set(&mut self, gx: usize, gy: usize, displacement: (f32, f32)) {
        if gx < self.grid_w && gy < self.grid_h {
            self.displacements[gy * self.grid_w + gx] = displacement;
        }
    }

    #[inline]
    pub fn get(&self, gx: usize, gy: usize) -> (f32, f32) {
        if gx < self.grid_w && gy < self.grid_h {
            self.displacements[gy * self.grid_w + gx]
        } else {
            (0.0, 0.0)
        }
    }

    /// Sample displacement at a fractional grid position using bilinear interpolation.
    pub fn sample(&self, gx: f32, gy: f32) -> (f32, f32) {
        let gx = gx.clamp(0.0, (self.grid_w - 1) as f32);
        let gy = gy.clamp(0.0, (self.grid_h - 1) as f32);

        let x0 = gx.floor() as usize;
        let y0 = gy.floor() as usize;
        let x1 = (x0 + 1).min(self.grid_w - 1);
        let y1 = (y0 + 1).min(self.grid_h - 1);

        let fx = gx - x0 as f32;
        let fy = gy - y0 as f32;

        let d00 = self.get(x0, y0);
        let d10 = self.get(x1, y0);
        let d01 = self.get(x0, y1);
        let d11 = self.get(x1, y1);

        let dx = d00.0 * (1.0 - fx) * (1.0 - fy)
            + d10.0 * fx * (1.0 - fy)
            + d01.0 * (1.0 - fx) * fy
            + d11.0 * fx * fy;
        let dy = d00.1 * (1.0 - fx) * (1.0 - fy)
            + d10.1 * fx * (1.0 - fy)
            + d01.1 * (1.0 - fx) * fy
            + d11.1 * fx * fy;

        (dx, dy)
    }
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

    /// Draw a line using Bresenham's algorithm with the given blend mode.
    pub fn paint_line(
        &mut self,
        x0: isize,
        y0: isize,
        x1: isize,
        y1: isize,
        color: (f32, f32, f32),
        blend: BlendMode,
    ) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx: isize = if x0 < x1 { 1 } else { -1 };
        let sy: isize = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x0;
        let mut y = y0;

        loop {
            if x >= 0 && y >= 0 {
                self.paint(x as usize, y as usize, color, blend);
            }
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Zoom the front buffer into the back buffer around (cx, cy).
    /// amount > 1.0 = zoom in (enlarge), < 1.0 = zoom out (shrink).
    /// Uses nearest-neighbor sampling from front buffer.
    pub fn zoom(&mut self, cx: f32, cy: f32, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        let inv = 1.0 / amount;
        for y in 0..self.height {
            for x in 0..self.width {
                // Map back to source coordinates in front buffer
                let src_x = ((x as f32 - cx) * inv + cx).round() as isize;
                let src_y = ((y as f32 - cy) * inv + cy).round() as isize;
                if src_x >= 0
                    && src_y >= 0
                    && (src_x as usize) < self.width
                    && (src_y as usize) < self.height
                {
                    let si = src_y as usize * self.width + src_x as usize;
                    let di = y * self.width + x;
                    self.back[di] = self.front[si];
                }
                // out-of-bounds source → back pixel stays at (0,0,0)
            }
        }
    }

    /// Rotate the front buffer into the back buffer around (cx, cy).
    /// Uses nearest-neighbor sampling from front buffer.
    pub fn rotate(&mut self, cx: f32, cy: f32, radians: f32) {
        let cos = radians.cos();
        let sin = radians.sin();
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                // Inverse rotation to find source pixel
                let src_x = (dx * cos + dy * sin + cx).round() as isize;
                let src_y = (-dx * sin + dy * cos + cy).round() as isize;
                if src_x >= 0
                    && src_y >= 0
                    && (src_x as usize) < self.width
                    && (src_y as usize) < self.height
                {
                    let si = src_y as usize * self.width + src_x as usize;
                    let di = y * self.width + x;
                    self.back[di] = self.front[si];
                }
            }
        }
    }

    /// Apply warp grid displacement: read from front, write displaced into back.
    /// Each pixel's source position is offset by the interpolated grid displacement.
    pub fn warp(&mut self, grid: &WarpGrid) {
        if grid.grid_w < 2 || grid.grid_h < 2 {
            // Degenerate grid — just copy front to back
            self.back.copy_from_slice(&self.front);
            return;
        }
        for y in 0..self.height {
            let gy = y as f32 / self.height as f32 * (grid.grid_h - 1) as f32;
            for x in 0..self.width {
                let gx = x as f32 / self.width as f32 * (grid.grid_w - 1) as f32;
                let (dx, dy) = grid.sample(gx, gy);
                let src_x = (x as f32 - dx).round() as isize;
                let src_y = (y as f32 - dy).round() as isize;
                let di = y * self.width + x;
                if src_x >= 0
                    && src_y >= 0
                    && (src_x as usize) < self.width
                    && (src_y as usize) < self.height
                {
                    let si = src_y as usize * self.width + src_x as usize;
                    self.back[di] = self.front[si];
                } else {
                    self.back[di] = (0.0, 0.0, 0.0);
                }
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

    #[test]
    fn test_paint_line_horizontal() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.paint_line(0, 5, 9, 5, (1.0, 1.0, 1.0), BlendMode::Alpha);
        // All pixels on y=5 from x=0 to x=9 should be set
        for x in 0..10 {
            assert_ne!(
                fb.get_back(x, 5),
                (0.0, 0.0, 0.0),
                "pixel at ({x}, 5) should be set"
            );
        }
    }

    #[test]
    fn test_paint_line_vertical() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.paint_line(5, 0, 5, 9, (1.0, 0.0, 0.0), BlendMode::Alpha);
        for y in 0..10 {
            assert_ne!(
                fb.get_back(5, y),
                (0.0, 0.0, 0.0),
                "pixel at (5, {y}) should be set"
            );
        }
    }

    #[test]
    fn test_paint_line_single_point() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.paint_line(3, 3, 3, 3, (0.5, 0.5, 0.5), BlendMode::Alpha);
        assert_eq!(fb.get_back(3, 3), (0.5, 0.5, 0.5));
    }

    #[test]
    fn test_paint_line_uses_blend_mode() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(5, 5, (0.3, 0.3, 0.3));
        fb.paint_line(5, 5, 5, 5, (0.2, 0.2, 0.2), BlendMode::Additive);
        let (r, _, _) = fb.get_back(5, 5);
        assert!((r - 0.5).abs() < 0.001); // additive: 0.3 + 0.2
    }

    #[test]
    fn test_zoom_in_center_pixel_stays() {
        let mut fb = FeedbackCanvas::new(10, 5);
        // Put a pixel at center of front buffer
        fb.set_back(5, 5, (1.0, 0.0, 0.0));
        fb.swap(); // now it's in front
        fb.zoom(5.0, 5.0, 1.1); // zoom in slightly
                                // Center pixel should still be approximately at center in back
        let (r, _, _) = fb.get_back(5, 5);
        assert!(r > 0.5, "center pixel should remain after zoom, got {r}");
    }

    #[test]
    fn test_zoom_out_shrinks() {
        let mut fb = FeedbackCanvas::new(20, 10);
        // Fill front buffer with a white square in center
        for y in 8..12 {
            for x in 8..12 {
                fb.set_back(x, y, (1.0, 1.0, 1.0));
            }
        }
        fb.swap();
        fb.zoom(10.0, 10.0, 0.5); // zoom out = shrink
                                  // Corners of original square should now be closer to center
                                  // Edge pixels should be black (zoomed out beyond original)
        let (r, _, _) = fb.get_back(0, 0);
        assert!(r < 0.01, "corner should be black after zoom out");
    }

    #[test]
    fn test_zoom_identity() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(3, 4, (0.7, 0.3, 0.1));
        fb.swap();
        fb.zoom(5.0, 5.0, 1.0); // no zoom
        let (r, g, b) = fb.get_back(3, 4);
        assert!((r - 0.7).abs() < 0.01);
        assert!((g - 0.3).abs() < 0.01);
        assert!((b - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_rotate_zero_is_identity() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.set_back(7, 3, (0.8, 0.4, 0.2));
        fb.swap();
        fb.rotate(5.0, 5.0, 0.0);
        let (r, g, b) = fb.get_back(7, 3);
        assert!((r - 0.8).abs() < 0.01);
        assert!((g - 0.4).abs() < 0.01);
        assert!((b - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_rotate_moves_pixels() {
        let mut fb = FeedbackCanvas::new(20, 10);
        // Put a pixel to the right of center
        fb.set_back(15, 10, (1.0, 0.0, 0.0));
        fb.swap();
        fb.rotate(10.0, 10.0, std::f32::consts::FRAC_PI_2); // 90 degrees
                                                            // After 90° CCW rotation, (15,10) should move to roughly (10,15)
        let (r, _, _) = fb.get_back(15, 10);
        assert!(r < 0.01, "original position should be empty after rotation");
    }

    #[test]
    fn test_rotate_center_stays() {
        let mut fb = FeedbackCanvas::new(20, 10);
        fb.set_back(10, 10, (1.0, 1.0, 1.0));
        fb.swap();
        fb.rotate(10.0, 10.0, 0.5);
        let (r, _, _) = fb.get_back(10, 10);
        assert!(r > 0.5, "center of rotation should stay");
    }

    #[test]
    fn test_warp_grid_new() {
        let grid = WarpGrid::new(4, 3);
        assert_eq!(grid.grid_w, 4);
        assert_eq!(grid.grid_h, 3);
        assert_eq!(grid.displacements.len(), 12);
    }

    #[test]
    fn test_warp_grid_zero_displacement_is_identity() {
        let mut fb = FeedbackCanvas::new(20, 10);
        fb.set_back(10, 10, (0.8, 0.4, 0.2));
        fb.swap();
        let grid = WarpGrid::new(4, 4); // all zeros
        fb.warp(&grid);
        let (r, g, b) = fb.get_back(10, 10);
        assert!((r - 0.8).abs() < 0.01);
        assert!((g - 0.4).abs() < 0.01);
        assert!((b - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_warp_displaces_pixels() {
        let mut fb = FeedbackCanvas::new(20, 10);
        fb.set_back(10, 10, (1.0, 0.0, 0.0));
        fb.swap();
        let mut grid = WarpGrid::new(2, 2);
        // Set all grid points to displace right by 3 pixels
        for d in &mut grid.displacements {
            *d = (3.0, 0.0);
        }
        fb.warp(&grid);
        // Original pixel at (10,10) in back should be sourced from (10-3, 10) = (7,10) in front
        // Since (7,10) was black, (10,10) should be black
        let (r_orig, _, _) = fb.get_back(10, 10);
        assert!(
            r_orig < 0.01,
            "original position should be empty after warp"
        );
    }

    #[test]
    fn test_warp_grid_set_get() {
        let mut grid = WarpGrid::new(4, 3);
        grid.set(1, 2, (0.5, -0.3));
        assert_eq!(grid.get(1, 2), (0.5, -0.3));
    }
}
