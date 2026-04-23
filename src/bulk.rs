use crate::{BitBoard, BitLayout};

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// 矩形範囲を一括で塗りつぶす (最適化版)
    pub fn set_rect(&mut self, x: i32, y: i32, width: i32, height: i32, value: bool) {
        L::rect_op(&mut self.data, &mut self.block_mask, x, y, width, height, value);
    }

    /// 指定した座標を中心とした十字形を塗りつぶす
    pub fn set_plus(&mut self, x: i32, y: i32, range: i32, value: bool) {
        if range < 0 { return; }
        // 垂直
        self.set_rect(x, y - range, 1, range * 2 + 1, value);
        // 水平
        self.set_rect(x - range, y, range * 2 + 1, 1, value);
    }

    /// 指定した座標を中心とした菱形（マンハッタン距離内）を塗りつぶす
    pub fn set_diamond(&mut self, x: i32, y: i32, range: i32, value: bool) {
        if range < 0 { return; }
        for dy in -range..=range {
            let h_width = range - dy.abs();
            self.set_row(y + dy, x - h_width, x + h_width, value);
        }
    }

    /// 指定した行の範囲に一括で値を設定 (内部ワード最適化)
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
        assert!(!bb.get(99, 99)); // 斜めは含まない
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
        assert!(!bb.get(99, 99)); // range 1 では角は含まない
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
}
