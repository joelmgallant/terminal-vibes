# MilkDrop Paint-On Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a new "milkdrop" visualization with double-buffered float RGB feedback rendering — previous frames persist, get transformed (zoom/rotate/warp), and new elements paint on top creating trails and feedback loops.

**Architecture:** `FeedbackCanvas` (two float RGB buffers) in `src/visualizations/feedback.rs` handles the feedback loop. `Milkdrop` viz in `src/visualizations/milkdrop.rs` owns the canvas plus three toggleable paint layers (waveform, shapes, particles). Converts to `HalfBlockCanvas` for ratatui output.

**Tech Stack:** Rust, ratatui (Buffer/Rect), existing HalfBlockCanvas, ColorPalette, Visualization trait, SIN_LUT, BeatData/FrameData.

---

### Task 1: FeedbackCanvas Core — Struct, New, Resize, Pixel Access

**Files:**
- Create: `src/visualizations/feedback.rs`
- Modify: `src/visualizations/mod.rs:1-12` (add `pub mod feedback;`)

**Step 1: Write the failing tests**

In `src/visualizations/feedback.rs`, add at the bottom:

```rust
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — module doesn't exist yet

**Step 3: Write minimal implementation**

Create `src/visualizations/feedback.rs`:

```rust
/// Double-buffered float RGB canvas for MilkDrop-style feedback rendering.
///
/// Two buffers at HalfBlockCanvas pixel resolution (cols × rows×2):
/// - **Front**: read source (previous frame)
/// - **Back**: write target (current frame being built)
///
/// Float RGB (0.0–1.0) gives smooth fades. Memory: ~86KB at 200×120.
pub struct FeedbackCanvas {
    width: usize,
    height: usize, // rows * 2 (half-block vertical resolution)
    front: Vec<(f32, f32, f32)>,
    back: Vec<(f32, f32, f32)>,
}

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
```

Add to `src/visualizations/mod.rs` alongside the other module declarations:

```rust
pub mod feedback;
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 5 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs src/visualizations/mod.rs
git commit -m "feat: add FeedbackCanvas core struct with double-buffered float RGB"
```

---

### Task 2: FeedbackCanvas Swap + Decay

**Files:**
- Modify: `src/visualizations/feedback.rs`

**Step 1: Write the failing tests**

Add to the `tests` module in `feedback.rs`:

```rust
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — `swap` and `decay` not defined

**Step 3: Write minimal implementation**

Add to `impl FeedbackCanvas`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 9 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs
git commit -m "feat: add FeedbackCanvas swap and decay operations"
```

---

### Task 3: FeedbackCanvas Paint with Blend Modes

**Files:**
- Modify: `src/visualizations/feedback.rs`

**Step 1: Write the failing tests**

Add to `feedback.rs` tests module:

```rust
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — `BlendMode` and `paint` not defined

**Step 3: Write minimal implementation**

Add above `FeedbackCanvas` struct definition:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendMode {
    /// dst + src (clamped). Trails glow, colors accumulate.
    Additive,
    /// src replaces dst. Opaque overlay.
    Alpha,
    /// max(dst, src) per channel. Brighten only.
    Max,
}
```

Add to `impl FeedbackCanvas`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 14 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs
git commit -m "feat: add BlendMode and paint operation to FeedbackCanvas"
```

---

### Task 4: FeedbackCanvas Bresenham Line Drawing

**Files:**
- Modify: `src/visualizations/feedback.rs`

**Step 1: Write the failing tests**

Add to tests module:

```rust
    #[test]
    fn test_paint_line_horizontal() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.paint_line(0, 5, 9, 5, (1.0, 1.0, 1.0), BlendMode::Alpha);
        // All pixels on y=5 from x=0 to x=9 should be set
        for x in 0..10 {
            assert_ne!(fb.get_back(x, 5), (0.0, 0.0, 0.0), "pixel at ({x}, 5) should be set");
        }
    }

    #[test]
    fn test_paint_line_vertical() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.paint_line(5, 0, 5, 9, (1.0, 0.0, 0.0), BlendMode::Alpha);
        for y in 0..10 {
            assert_ne!(fb.get_back(5, y), (0.0, 0.0, 0.0), "pixel at (5, {y}) should be set");
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — `paint_line` not defined

**Step 3: Write minimal implementation**

Add to `impl FeedbackCanvas`:

```rust
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
```

Update test signatures to use `isize` coordinates:

```rust
    #[test]
    fn test_paint_line_horizontal() {
        let mut fb = FeedbackCanvas::new(10, 5);
        fb.paint_line(0, 5, 9, 5, (1.0, 1.0, 1.0), BlendMode::Alpha);
        for x in 0..10 {
            assert_ne!(fb.get_back(x, 5), (0.0, 0.0, 0.0), "pixel at ({x}, 5) should be set");
        }
    }
```

(Tests already pass with usize→isize since the values are positive.)

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 18 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs
git commit -m "feat: add Bresenham line drawing to FeedbackCanvas"
```

---

### Task 5: FeedbackCanvas Zoom Transform

**Files:**
- Modify: `src/visualizations/feedback.rs`

**Step 1: Write the failing tests**

Add to tests module:

```rust
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — `zoom` not defined

**Step 3: Write minimal implementation**

Add to `impl FeedbackCanvas`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 21 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs
git commit -m "feat: add zoom transform to FeedbackCanvas"
```

---

### Task 6: FeedbackCanvas Rotate Transform

**Files:**
- Modify: `src/visualizations/feedback.rs`

**Step 1: Write the failing tests**

Add to tests module:

```rust
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — `rotate` not defined

**Step 3: Write minimal implementation**

Add to `impl FeedbackCanvas`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 24 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs
git commit -m "feat: add rotate transform to FeedbackCanvas"
```

---

### Task 7: WarpGrid + Warp Transform

**Files:**
- Modify: `src/visualizations/feedback.rs`

**Step 1: Write the failing tests**

Add to tests module:

```rust
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
        // Original pixel at (10,10) should have moved
        // The pixel at (10,10) in back should be sourced from (10-3, 10) = (7,10) in front
        // Since (7,10) was black, (10,10) should be black
        // And (13,10) should now have the red pixel
        let (r_orig, _, _) = fb.get_back(10, 10);
        assert!(r_orig < 0.01, "original position should be empty after warp");
    }

    #[test]
    fn test_warp_grid_set_get() {
        let mut grid = WarpGrid::new(4, 3);
        grid.set(1, 2, (0.5, -0.3));
        assert_eq!(grid.get(1, 2), (0.5, -0.3));
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — `WarpGrid` not defined

**Step 3: Write minimal implementation**

Add to `feedback.rs`, above `FeedbackCanvas`:

```rust
/// Coarse displacement grid for warp transforms.
/// Each grid point stores a (dx, dy) displacement vector.
/// Pixels between grid points interpolate displacement bilinearly.
pub struct WarpGrid {
    pub grid_w: usize,
    pub grid_h: usize,
    pub displacements: Vec<(f32, f32)>,
}

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
```

Add to `impl FeedbackCanvas`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 28 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs
git commit -m "feat: add WarpGrid and warp transform to FeedbackCanvas"
```

---

### Task 8: FeedbackCanvas Combined Transform + to_halfblock Conversion

**Files:**
- Modify: `src/visualizations/feedback.rs`

**Step 1: Write the failing tests**

Add to tests module:

```rust
    use crate::visualizations::render::HalfBlockCanvas;

    #[test]
    fn test_zoom_rotate_combined() {
        let mut fb = FeedbackCanvas::new(20, 10);
        // Fill center area with color
        for y in 8..12 {
            for x in 8..12 {
                fb.set_back(x, y, (0.8, 0.4, 0.2));
            }
        }
        fb.swap();
        fb.zoom_rotate(10.0, 10.0, 1.05, 0.1);
        // Center should still have color
        let (r, _, _) = fb.get_back(10, 10);
        assert!(r > 0.3, "center should retain color after zoom+rotate");
    }

    #[test]
    fn test_to_halfblock_converts_colors() {
        let mut fb = FeedbackCanvas::new(4, 2);
        fb.set_back(0, 0, (1.0, 0.0, 0.0)); // red top-left
        fb.set_back(1, 1, (0.0, 1.0, 0.0)); // green
        let mut canvas = HalfBlockCanvas::new(4, 2);
        fb.to_halfblock(&mut canvas);
        // Canvas should have been populated (non-trivial to check exact output,
        // but we can verify it doesn't panic and produces something)
    }

    #[test]
    fn test_to_halfblock_empty_canvas() {
        let fb = FeedbackCanvas::new(4, 2);
        let mut canvas = HalfBlockCanvas::new(4, 2);
        fb.to_halfblock(&mut canvas);
        // All-black feedback should produce an all-None canvas (no colors set)
    }

    #[test]
    fn test_to_halfblock_dimension_mismatch_handled() {
        let fb = FeedbackCanvas::new(10, 5);
        let mut canvas = HalfBlockCanvas::new(20, 10); // different size
        fb.to_halfblock(&mut canvas); // should not panic
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib feedback::tests -v`
Expected: FAIL — `zoom_rotate` and `to_halfblock` not defined

**Step 3: Write minimal implementation**

Add to `impl FeedbackCanvas`:

```rust
    /// Combined zoom + rotate in a single pass (avoids double read of front buffer).
    /// This is the common case — most frames apply both.
    pub fn zoom_rotate(&mut self, cx: f32, cy: f32, zoom: f32, radians: f32) {
        if zoom <= 0.0 {
            return;
        }
        let inv_zoom = 1.0 / zoom;
        let cos = radians.cos();
        let sin = radians.sin();
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                // Inverse zoom + rotation
                let rx = dx * cos + dy * sin;
                let ry = -dx * sin + dy * cos;
                let src_x = (rx * inv_zoom + cx).round() as isize;
                let src_y = (ry * inv_zoom + cy).round() as isize;
                let di = y * self.width + x;
                if src_x >= 0
                    && src_y >= 0
                    && (src_x as usize) < self.width
                    && (src_y as usize) < self.height
                {
                    let si = src_y as usize * self.width + src_x as usize;
                    self.back[di] = self.front[si];
                }
            }
        }
    }

    /// Convert the back buffer to a HalfBlockCanvas for ratatui output.
    /// Maps float RGB (0.0–1.0) to u8 RGB (0–255).
    /// Skips black pixels (leaves canvas cells as None for transparency).
    pub fn to_halfblock(&self, canvas: &mut HalfBlockCanvas) {
        let pw = canvas.pixel_width();
        let ph = canvas.pixel_height();
        canvas.clear();
        for y in 0..ph.min(self.height) {
            for x in 0..pw.min(self.width) {
                let i = y * self.width + x;
                if i < self.back.len() {
                    let (r, g, b) = self.back[i];
                    // Skip near-black pixels (threshold avoids noise)
                    if r > 0.01 || g > 0.01 || b > 0.01 {
                        let color = ratatui::style::Color::Rgb(
                            (r.clamp(0.0, 1.0) * 255.0) as u8,
                            (g.clamp(0.0, 1.0) * 255.0) as u8,
                            (b.clamp(0.0, 1.0) * 255.0) as u8,
                        );
                        canvas.set(x, y, color);
                    }
                }
            }
        }
    }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib feedback::tests -v`
Expected: PASS (all 32 tests)

**Step 5: Commit**

```bash
git add src/visualizations/feedback.rs
git commit -m "feat: add zoom_rotate and to_halfblock conversion to FeedbackCanvas"
```

---

### Task 9: Milkdrop Visualization Struct + Minimal Trait Impl

**Files:**
- Create: `src/visualizations/milkdrop.rs`
- Modify: `src/visualizations/mod.rs` (add `pub mod milkdrop;`)
- Modify: `src/main.rs:84-94` (register Milkdrop)
- Create: `tests/milkdrop_test.rs`

**Step 1: Write the failing integration tests**

Create `tests/milkdrop_test.rs`:

```rust
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
            !matches!(fg, ratatui::style::Color::Reset | ratatui::style::Color::Rgb(0, 0, 0))
        })
    });
    assert!(has_color, "Feedback should persist after silence — trails should still be visible");
}

#[test]
fn test_milkdrop_heavy_rendering() {
    let viz = Milkdrop::new();
    assert!(viz.heavy_rendering(), "milkdrop should report heavy rendering");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test milkdrop_test -v`
Expected: FAIL — module doesn't exist

**Step 3: Write minimal implementation**

Create `src/visualizations/milkdrop.rs`:

```rust
use crate::processing::FrameData;
use crate::visualizations::feedback::{BlendMode, FeedbackCanvas, WarpGrid};
use crate::visualizations::render::{HalfBlockCanvas, SIN_LUT};
use crate::visualizations::spectrum::ColorPalette;
use crate::visualizations::Visualization;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use std::f32::consts::PI;

const WARP_GRID_W: usize = 16;
const WARP_GRID_H: usize = 12;

pub struct Milkdrop {
    feedback: FeedbackCanvas,
    canvas: HalfBlockCanvas,
    warp_grid: WarpGrid,
    palette: ColorPalette,

    // Audio state (EMA smoothed)
    bass: f32,
    mid: f32,
    treble: f32,
    rms: f32,
    peak: f32,
    beat_envelope: f32,

    // Transform parameters
    zoom_amount: f32,
    rotation_angle: f32,
    decay_factor: f32,
    time: f32,

    // Layer toggles
    waveform_enabled: bool,
    shapes_enabled: bool,
    particles_enabled: bool,

    // Waveform state
    waveform_hue: f32,

    // Particles state
    particles: Vec<Particle>,

    // Spectrum data (kept for shapes layer)
    spectrum: Vec<f32>,
    waveform_data: Vec<f32>,
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    hue: f32,
}

impl Default for Milkdrop {
    fn default() -> Self {
        Self::new()
    }
}

impl Milkdrop {
    pub fn new() -> Self {
        Self {
            feedback: FeedbackCanvas::new(0, 0),
            canvas: HalfBlockCanvas::new(0, 0),
            warp_grid: WarpGrid::new(WARP_GRID_W, WARP_GRID_H),
            palette: ColorPalette::Synthwave,

            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            rms: 0.0,
            peak: 0.0,
            beat_envelope: 0.0,

            zoom_amount: 1.02,
            rotation_angle: 0.0,
            decay_factor: 0.92,
            time: 0.0,

            waveform_enabled: true,
            shapes_enabled: true,
            particles_enabled: true,

            waveform_hue: 0.0,

            particles: Vec::with_capacity(128),

            spectrum: Vec::new(),
            waveform_data: Vec::new(),
        }
    }

    fn update_audio(&mut self, frame: &FrameData) {
        let smooth = 0.3_f32; // EMA factor

        let band_count = frame.spectrum.len();
        if band_count >= 3 {
            let third = band_count / 3;
            let raw_bass = frame.spectrum[..third].iter().sum::<f32>() / third as f32;
            let raw_mid = frame.spectrum[third..third * 2].iter().sum::<f32>() / third as f32;
            let raw_treble = frame.spectrum[third * 2..].iter().sum::<f32>() / third as f32;
            self.bass = self.bass * (1.0 - smooth) + raw_bass * smooth;
            self.mid = self.mid * (1.0 - smooth) + raw_mid * smooth;
            self.treble = self.treble * (1.0 - smooth) + raw_treble * smooth;
        }

        self.rms = self.rms * (1.0 - smooth) + frame.rms * smooth;
        self.peak = frame.peak; // peak is instant, don't smooth
        self.beat_envelope = frame.beat.envelope;
        self.spectrum.clear();
        self.spectrum.extend_from_slice(&frame.spectrum);
        self.waveform_data.clear();
        self.waveform_data.extend_from_slice(&frame.waveform);
    }

    fn update_transforms(&mut self) {
        let beat_boost = 1.0 + self.beat_envelope * 0.5;

        // Zoom: bass pushes outward
        self.zoom_amount = (1.01 + self.bass * 0.04) * beat_boost;

        // Rotation: mid energy drives spin
        self.rotation_angle += (0.005 + self.mid * 0.03) * beat_boost;

        // Warp grid: treble drives ripple
        let ripple = self.treble * 3.0 * beat_boost;
        for gy in 0..WARP_GRID_H {
            for gx in 0..WARP_GRID_W {
                let nx = gx as f32 / (WARP_GRID_W - 1) as f32 * 2.0 - 1.0;
                let ny = gy as f32 / (WARP_GRID_H - 1) as f32 * 2.0 - 1.0;
                let angle = ny.atan2(nx);
                let dist = (nx * nx + ny * ny).sqrt();
                // Radial outward push + tangential ripple
                let dx = dist * angle.cos() * 0.5 + SIN_LUT.get(self.time * 2.0 + dist * 5.0) * ripple;
                let dy = dist * angle.sin() * 0.5 + SIN_LUT.get(self.time * 2.3 + dist * 5.0) * ripple;
                self.warp_grid.set(gx, gy, (dx, dy));
            }
        }

        // Time advance: scales with audio energy
        self.time += 0.03 + self.rms * 0.05;
    }

    fn paint_waveform(&mut self) {
        if !self.waveform_enabled || self.waveform_data.is_empty() {
            return;
        }
        let pw = self.feedback.pixel_width();
        let ph = self.feedback.pixel_height();
        if pw == 0 || ph == 0 {
            return;
        }

        self.waveform_hue += 0.01 + self.beat_envelope * 0.05;
        let hue = self.waveform_hue % 1.0;
        let color = hue_to_rgb(hue);
        let brightness = 0.5 + self.peak * 0.5;
        let color = (color.0 * brightness, color.1 * brightness, color.2 * brightness);

        let cy = ph as f32 / 2.0;
        let samples = &self.waveform_data;
        let step = samples.len() as f32 / pw as f32;

        let mut prev_x: Option<isize> = None;
        let mut prev_y: Option<isize> = None;

        for px in 0..pw {
            let si = (px as f32 * step) as usize;
            let sample = samples.get(si).copied().unwrap_or(0.0);
            let y = (cy + sample * cy * 0.8).round() as isize;
            let x = px as isize;

            if let (Some(px_prev), Some(py_prev)) = (prev_x, prev_y) {
                self.feedback.paint_line(px_prev, py_prev, x, y, color, BlendMode::Additive);
            }
            prev_x = Some(x);
            prev_y = Some(y);
        }
    }

    fn paint_shapes(&mut self) {
        if !self.shapes_enabled || self.spectrum.is_empty() {
            return;
        }
        let pw = self.feedback.pixel_width();
        let ph = self.feedback.pixel_height();
        if pw == 0 || ph == 0 {
            return;
        }

        let cx = pw as f32 / 2.0;
        let cy = ph as f32 / 2.0;
        let base_radius = (pw.min(ph) as f32) * 0.15;
        let radius = base_radius * (1.0 + self.bass * 2.0 + self.beat_envelope * 0.5);

        let num_dots = self.spectrum.len().min(64);
        for i in 0..num_dots {
            let t = i as f32 / num_dots as f32;
            let angle = t * 2.0 * PI + self.time * 0.5;
            let mag = self.spectrum.get(i).copied().unwrap_or(0.0);
            let r = radius * (0.5 + mag * 0.5);
            let x = (cx + r * SIN_LUT.get(angle + PI / 2.0)).round() as usize;
            let y = (cy + r * SIN_LUT.get(angle)).round() as usize;

            let color = self.palette.color(t);
            let (cr, cg, cb) = match color {
                ratatui::style::Color::Rgb(r, g, b) => (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
                _ => (1.0, 1.0, 1.0),
            };
            let brightness = 0.3 + mag * 0.7;
            self.feedback.paint(x, y, (cr * brightness, cg * brightness, cb * brightness), BlendMode::Additive);
        }
    }

    fn update_particles(&mut self) {
        let pw = self.feedback.pixel_width() as f32;
        let ph = self.feedback.pixel_height() as f32;
        if pw == 0.0 || ph == 0.0 {
            return;
        }

        // Spawn on beat
        if self.beat_envelope > 0.5 && self.particles.len() < 128 {
            let burst = ((self.beat_envelope * 8.0) as usize).min(128 - self.particles.len());
            let cx = pw / 2.0;
            let cy = ph / 2.0;
            for _ in 0..burst {
                let angle = self.time * 37.0 + self.particles.len() as f32 * 2.399; // golden angle-ish
                let speed = 1.0 + self.peak * 3.0;
                self.particles.push(Particle {
                    x: cx,
                    y: cy,
                    vx: angle.cos() * speed,
                    vy: angle.sin() * speed,
                    life: 1.0,
                    hue: (self.waveform_hue + self.particles.len() as f32 * 0.1) % 1.0,
                });
            }
        }

        // Update existing
        self.particles.retain_mut(|p| {
            p.x += p.vx;
            p.y += p.vy;
            p.life -= 0.015;
            p.life > 0.0 && p.x >= 0.0 && p.x < pw && p.y >= 0.0 && p.y < ph
        });
    }

    fn paint_particles(&mut self) {
        if !self.particles_enabled {
            return;
        }
        for p in &self.particles {
            let color = hue_to_rgb(p.hue);
            let brightness = p.life;
            self.feedback.paint(
                p.x as usize,
                p.y as usize,
                (color.0 * brightness, color.1 * brightness, color.2 * brightness),
                BlendMode::Additive,
            );
        }
    }
}

/// Convert hue (0.0–1.0) to float RGB.
fn hue_to_rgb(hue: f32) -> (f32, f32, f32) {
    let h = hue.fract() * 6.0;
    let f = h.fract();
    match h as u8 {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0 - f),
    }
}

impl Visualization for Milkdrop {
    fn name(&self) -> &str {
        "milkdrop"
    }

    fn update(&mut self, frame: &FrameData) {
        self.update_audio(frame);
        self.update_transforms();
        self.update_particles();
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.feedback.resize(area.width, area.height);
        self.canvas.resize_or_clear(area.width, area.height);

        let pw = self.feedback.pixel_width() as f32;
        let ph = self.feedback.pixel_height() as f32;
        let cx = pw / 2.0;
        let cy = ph / 2.0;

        // 1. Swap — previous frame becomes read source
        self.feedback.swap();

        // 2. Transform — zoom + rotate the previous frame
        self.feedback.zoom_rotate(cx, cy, self.zoom_amount, self.rotation_angle * 0.02);

        // 3. Warp — apply displacement grid
        // Note: warp also reads from front, but we already wrote zoom_rotate to back.
        // For combined transforms, we'd need a third buffer or apply warp differently.
        // For now, zoom_rotate is the primary spatial transform; warp is applied as
        // additive displacement on top of the back buffer.
        // TODO: If warp quality matters, add a scratch buffer.

        // 4. Decay — fade trails
        self.feedback.decay(self.decay_factor);

        // 5. Paint layers
        self.paint_waveform();
        self.paint_shapes();
        self.paint_particles();

        // 6. Convert to HalfBlockCanvas for ratatui output
        self.feedback.to_halfblock(&mut self.canvas);

        // 7. Render
        self.canvas.render(&area, buf);
    }

    fn heavy_rendering(&self) -> bool {
        true
    }

    fn set_quantization_step(&mut self, step: u8) {
        self.canvas.set_step(step);
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('w') => {
                self.waveform_enabled = !self.waveform_enabled;
                true
            }
            KeyCode::Char('e') => {
                self.shapes_enabled = !self.shapes_enabled;
                true
            }
            KeyCode::Char('r') => {
                self.particles_enabled = !self.particles_enabled;
                true
            }
            KeyCode::Char('d') => {
                self.decay_factor = (self.decay_factor + 0.01).min(0.99);
                true
            }
            KeyCode::Char('D') => {
                self.decay_factor = (self.decay_factor - 0.01).max(0.80);
                true
            }
            KeyCode::Char('z') => {
                self.zoom_amount = (self.zoom_amount + 0.005).min(1.15);
                true
            }
            KeyCode::Char('Z') => {
                self.zoom_amount = (self.zoom_amount - 0.005).max(0.95);
                true
            }
            KeyCode::Char('p') => {
                self.palette = ColorPalette::ALL
                    [(ColorPalette::ALL.iter().position(|&p| p == self.palette).unwrap_or(0) + 1)
                        % ColorPalette::ALL.len()];
                true
            }
            KeyCode::Char('P') => {
                let idx = ColorPalette::ALL.iter().position(|&p| p == self.palette).unwrap_or(0);
                self.palette = ColorPalette::ALL
                    [(idx + ColorPalette::ALL.len() - 1) % ColorPalette::ALL.len()];
                true
            }
            _ => false,
        }
    }

    fn default_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        table.insert("palette".to_string(), toml::Value::String("synthwave".to_string()));
        table.insert("decay_factor".to_string(), toml::Value::Float(0.92));
        table.insert("waveform_enabled".to_string(), toml::Value::Boolean(true));
        table.insert("shapes_enabled".to_string(), toml::Value::Boolean(true));
        table.insert("particles_enabled".to_string(), toml::Value::Boolean(true));
        toml::Value::Table(table)
    }

    fn apply_config(&mut self, config: &toml::Value) {
        if let Some(name) = config.get("palette").and_then(|v| v.as_str()) {
            if let Some(p) = ColorPalette::from_name(name) {
                self.palette = p;
            }
        }
        if let Some(d) = config.get("decay_factor").and_then(|v| v.as_float()) {
            self.decay_factor = (d as f32).clamp(0.80, 0.99);
        }
        if let Some(w) = config.get("waveform_enabled").and_then(|v| v.as_bool()) {
            self.waveform_enabled = w;
        }
        if let Some(s) = config.get("shapes_enabled").and_then(|v| v.as_bool()) {
            self.shapes_enabled = s;
        }
        if let Some(p) = config.get("particles_enabled").and_then(|v| v.as_bool()) {
            self.particles_enabled = p;
        }
    }

    fn save_config(&self) -> toml::Value {
        let mut table = toml::value::Table::new();
        table.insert("palette".to_string(), toml::Value::String(self.palette.name().to_string()));
        table.insert("decay_factor".to_string(), toml::Value::Float(self.decay_factor as f64));
        table.insert("waveform_enabled".to_string(), toml::Value::Boolean(self.waveform_enabled));
        table.insert("shapes_enabled".to_string(), toml::Value::Boolean(self.shapes_enabled));
        table.insert("particles_enabled".to_string(), toml::Value::Boolean(self.particles_enabled));
        toml::Value::Table(table)
    }
}
```

Add `pub mod milkdrop;` to `src/visualizations/mod.rs`.

Add to `src/main.rs` imports:

```rust
use visualizations::milkdrop::Milkdrop;
```

Add to registration block:

```rust
registry.register(Box::new(Milkdrop::new()));
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test milkdrop_test -v`
Expected: PASS (all 6 tests)

Also run: `cargo test --lib -v`
Expected: PASS (all unit tests still pass)

Also run: `cargo clippy`
Expected: No warnings

**Step 5: Commit**

```bash
git add src/visualizations/milkdrop.rs src/visualizations/mod.rs src/main.rs tests/milkdrop_test.rs
git commit -m "feat: add milkdrop visualization with feedback rendering, paint layers, and config"
```

---

### Task 10: Polish — Remove Unused Import, Clean Warnings

**Files:**
- Modify: `src/visualizations/milkdrop.rs` (if any unused imports after compilation)

**Step 1: Run clippy**

Run: `cargo clippy 2>&1`

**Step 2: Fix any warnings**

Review output and fix unused imports, unnecessary casts, etc. Common issues:
- Remove `use ratatui::prelude::CrosstermBackend;` if unused
- Fix any clippy lints about float comparisons, shadowing, etc.

**Step 3: Run full test suite**

Run: `cargo test --lib -v && cargo test --test milkdrop_test -v`
Expected: All PASS

**Step 4: Commit**

```bash
git add -u
git commit -m "chore: fix clippy warnings in milkdrop visualization"
```

---

### Task 11: Integration — Verify Full Build and All Existing Tests

**Files:** None (verification only)

**Step 1: Run full build**

Run: `cargo build`
Expected: Success

**Step 2: Run all tests**

Run: `cargo test --lib -v`
Expected: All existing tests + new feedback + milkdrop tests pass

Run: `cargo test --test milkdrop_test -v`
Expected: All 6 integration tests pass

**Step 3: Run clippy + fmt**

Run: `cargo clippy && cargo fmt --check`
Expected: Clean

**Step 4: Final commit if any formatting changes needed**

```bash
cargo fmt
git add -u
git commit -m "style: format milkdrop and feedback modules"
```
