use crate::{BitBoard, BitLayout};

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// Fills a rectangular range in batch (optimized version).
    pub fn set_rect(&mut self, x: i32, y: i32, width: i32, height: i32, value: bool) {
        L::rect_op(
            &mut self.data,
            &mut self.block_mask,
            x,
            y,
            width,
            height,
            value,
        );
    }

    /// Fills a plus-shaped area centered at the specified coordinates.
    pub fn set_plus(&mut self, x: i32, y: i32, range: i32, value: bool) {
        if range < 0 {
            return;
        }
        // Vertical
        self.set_rect(x, y - range, 1, range * 2 + 1, value);
        // Horizontal
        self.set_rect(x - range, y, range * 2 + 1, 1, value);
    }

    /// Fills a diamond-shaped area (within Manhattan distance) centered at the specified coordinates.
    pub fn set_diamond(&mut self, x: i32, y: i32, range: i32, value: bool) {
        if range < 0 {
            return;
        }
        for dy in -range..=range {
            let h_width = range - dy.abs();
            self.set_row(y + dy, x - h_width, x + h_width, value);
        }
    }

    /// Sets values for a range in a specified row in batch (internal word optimization).
    pub fn set_row(&mut self, y: i32, min_x: i32, max_x: i32, value: bool) {
        L::set_row(&mut self.data, &mut self.block_mask, y, min_x, max_x, value);
    }
}

#[cfg(test)]
mod tests {
    use crate::BitBoard;
    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_set_rect_bounds() {
        let mut bb = TestBoard::default();
        bb.set_rect(10, 10, 5, 5, true);
        assert!(bb.get(10, 10));
        assert!(bb.get(14, 14));
        assert!(!bb.get(9, 9));
        assert!(!bb.get(15, 15));
    }

    #[test]
    fn test_set_plus() {
        let mut bb = TestBoard::default();
        bb.set_plus(100, 100, 2, true);
        assert!(bb.get(100, 100));
        assert!(bb.get(100, 98));
        assert!(bb.get(100, 102));
        assert!(bb.get(98, 100));
        assert!(bb.get(102, 100));
        assert!(!bb.get(99, 99)); // Diagonal not included
    }

    #[test]
    fn test_set_diamond() {
        let mut bb = TestBoard::default();
        bb.set_diamond(100, 100, 1, true);
        assert!(bb.get(100, 100));
        assert!(bb.get(100, 99));
        assert!(bb.get(100, 101));
        assert!(bb.get(99, 100));
        assert!(bb.get(101, 100));
        assert!(!bb.get(99, 99)); // Corners not included for range 1
    }

    #[test]
    fn test_set_row_logic() {
        let mut bb = TestBoard::default();
        bb.set_row(50, 10, 20, true);
        assert!(bb.get(10, 50));
        assert!(bb.get(15, 50));
        assert!(bb.get(20, 50));
        assert!(!bb.get(9, 50));
        assert!(!bb.get(21, 50));
    }

    #[test]
    fn test_set_rect_out_of_bounds() {
        let mut bb = TestBoard::default();
        // Completely off-screen (negative)
        bb.set_rect(-20, -20, 10, 10, true);
        assert!(bb.is_empty());

        // Completely off-screen (positive)
        bb.set_rect(300, 300, 10, 10, true);
        assert!(bb.is_empty());

        // Partially off-screen
        bb.set_rect(-5, -5, 10, 10, true); // x: -5..5, y: -5..5 -> (0,0) to (4,4) should be filled
        assert!(bb.get(0, 0));
        assert!(bb.get(4, 4));
        assert!(!bb.get(5, 5));
        assert_eq!(bb.count_ones(), 25);
    }

    #[test]
    fn test_set_rect_invalid_dimensions() {
        let mut bb = TestBoard::default();
        // Zero width or height
        bb.set_rect(10, 10, 0, 5, true);
        bb.set_rect(10, 10, 5, 0, true);
        assert!(bb.is_empty());

        // Negative width or height
        bb.set_rect(10, 10, -5, 5, true);
        bb.set_rect(10, 10, 5, -5, true);
        assert!(bb.is_empty());
    }

    #[test]
    fn test_set_row_out_of_bounds() {
        let mut bb = TestBoard::default();
        // y out of bounds
        bb.set_row(-1, 0, 10, true);
        bb.set_row(256, 0, 10, true);
        assert!(bb.is_empty());

        // x range out of bounds
        bb.set_row(10, -10, -1, true);
        bb.set_row(10, 256, 300, true);
        assert!(bb.is_empty());

        // x range partially out of bounds
        bb.set_row(10, -5, 5, true); // 0..=5 should be set
        assert!(bb.get(0, 10));
        assert!(bb.get(5, 10));
        assert!(!bb.get(6, 10));
        assert_eq!(bb.count_ones(), 6);
    }
}
