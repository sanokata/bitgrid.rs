use crate::BitBoard;

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 矩形範囲を一括で塗りつぶす (最適化版)
    pub fn set_rect(&mut self, x: i32, y: i32, width: i32, height: i32, value: bool) {
        if width <= 0 || height <= 0 {
            return;
        }

        let min_y = y.max(0);
        let max_y = (y + height - 1).min((H as i32) - 1);
        let min_x = x.max(0);
        let max_x = (x + width - 1).min((W as i32) - 1);

        if min_y > max_y || min_x > max_x {
            return;
        }

        for current_y in min_y..=max_y {
            self.set_row(current_y, min_x, max_x, value);
        }
    }

    /// 指定した行の範囲に一括で値を設定 (内部ワード最適化)
    pub fn set_row(&mut self, y: i32, min_x: i32, max_x: i32, value: bool) {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return;
        }

        let min_x = min_x.max(0) as usize;
        let max_x = max_x.min((W as i32) - 1) as usize;
        let sw = (y as usize) * Self::ROW_U64S + min_x / 64;
        let ew = (y as usize) * Self::ROW_U64S + max_x / 64;

        if sw == ew {
            let mask = Self::make_mask(min_x % 64, max_x % 64);
            self.apply_word_mask(sw, mask, value);
            return;
        }

        // 開始ワード
        let s_mask = Self::make_mask(min_x % 64, 63);
        self.apply_word_mask(sw, s_mask, value);

        // 中間ワード
        if ew > sw + 1 {
            let mid_range = sw + 1..ew;
            if value {
                self.data[mid_range.clone()].fill(!0u64);
                for w in mid_range {
                    self.mark_word_non_empty(w);
                }
            } else {
                self.data[mid_range.clone()].fill(0);
                for w in mid_range {
                    self.l1_mask[w / 64] &= !(1u64 << (w % 64));
                }
            }
        }

        // 終了ワード
        let e_mask = Self::make_mask(0, max_x % 64);
        self.apply_word_mask(ew, e_mask, value);
    }

}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_set_rect_bounds() {
        let mut bb = TestBoard::default();
        
        // 正常ケース
        bb.set_rect(10, 10, 5, 5, true);
        assert!(bb.get(10, 10));
        assert!(bb.get(14, 14));
        assert!(!bb.get(9, 9));
        assert!(!bb.get(15, 15));
        
        // 画面外を含むケース
        bb.set_rect(-5, -5, 10, 10, true);
        assert!(bb.get(0, 0));
        assert!(bb.get(4, 4));
        assert!(!bb.get(5, 5)); // (10,10)のものは残っているが、このfill_rectの影響ではない
        
        // 完全に画面外のケース
        let count_before = bb.count_ones();
        bb.set_rect(300, 300, 10, 10, true);
        bb.set_rect(-20, -20, 10, 10, true);
        assert_eq!(bb.count_ones(), count_before);
    }

    #[test]
    fn test_set_rect_exact_width() {
        // 横幅いっぱいの塗りつぶし
        let mut bb = TestBoard::default();
        bb.set_rect(0, 5, 256, 1, true);
        
        for x in 0..256 {
            assert!(bb.get(x, 5));
        }
        assert_eq!(bb.count_ones(), 256);
    }
}
