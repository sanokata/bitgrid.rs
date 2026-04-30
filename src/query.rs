use crate::{BitBoard, BitLayout};

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// ボード上に一つでもオン（1）のビットがあるか確認
    #[inline]
    pub fn has_any(&self) -> bool {
        self.block_mask.iter().any(|&w| w != 0)
    }

    /// ボードが完全に空であるか確認
    #[inline]
    pub fn is_empty(&self) -> bool {
        !self.has_any()
    }

    /// セットされているビットの総数を階層化マスクを利用して高速にカウント
    pub fn count_ones(&self) -> usize {
        let mut count = 0;
        for i in 0..Self::block_words() {
            let mut bits = self.block_mask[i];
            while bits != 0 {
                let bit = bits.trailing_zeros();
                count += self.data[i * 64 + bit as usize].count_ones() as usize;
                bits &= bits - 1;
            }
        }
        count
    }

    /// 指定した行の指定範囲内にオン（1）のビットがあるか確認
    pub fn has_any_in_row(&self, y: i32, min_x: i32, max_x: i32) -> bool {
        L::has_any_in_row(&self.data, y, min_x, max_x)
    }

    /// 別のボードとの積集合（AND）が 1 であるビットを指定範囲内のみ高速に走査
    pub fn for_each_overlap<F>(&self, other: &Self, mut callback: F)
    where
        F: FnMut(i32, i32, usize),
    {
        for block_idx in 0..Self::block_words() {
            let mut combined_block = self.block_mask[block_idx] & other.block_mask[block_idx];
            let start_word_idx = block_idx * 64;

            while combined_block != 0 {
                let bit_in_block = combined_block.trailing_zeros();
                combined_block &= combined_block - 1;

                let word_idx = start_word_idx + bit_in_block as usize;
                if word_idx >= Self::total_words() {
                    break;
                }

                let mut combined_data = self.data[word_idx] & other.data[word_idx];
                while combined_data != 0 {
                    let bit = combined_data.trailing_zeros();
                    combined_data &= combined_data - 1;

                    let (x, y) = L::word_bit_to_coord(word_idx, bit);
                    if x >= 0 && x < W as i32 && y >= 0 && y < H as i32 {
                        let idx = L::coord_to_flat_index(x, y).unwrap_or(0);
                        callback(x, y, idx);
                    }
                }
            }
        }
    }

    /// 全てのオン（1）ビットを階層化マスクを利用して高速に走査
    pub fn for_each_bit<F>(&self, mut callback: F)
    where
        F: FnMut(i32, i32, usize),
    {
        for i in 0..Self::block_words() {
            let mut bits = self.block_mask[i];
            while bits != 0 {
                let bit = bits.trailing_zeros();
                let idx = i * 64 + bit as usize;
                let mut val = self.data[idx];
                while val != 0 {
                    let b = val.trailing_zeros();
                    let (x, y) = L::word_bit_to_coord(idx, b);
                    callback(x, y, idx * 64 + b as usize);
                    val &= val - 1;
                }
                bits &= bits - 1;
            }
        }
    }

    /// 指定したタイル範囲内でのみ、別のボードとの積集合（AND）を高速走査
    pub fn for_each_overlap_in<F>(
        &self,
        other: &Self,
        min_tile: (i32, i32),
        max_tile: (i32, i32),
        mut callback: F,
    ) where
        F: FnMut(i32, i32, usize),
    {
        self.for_each_overlap(other, |x, y, idx| {
            if x >= min_tile.0 && x <= max_tile.0 && y >= min_tile.1 && y <= max_tile.1 {
                callback(x, y, idx);
            }
        });
    }

    /// 指定した矩形範囲（中心 x, y、半径 radius）内がすべてセット（1）されているか判定
    pub fn is_area_all_set(&self, x: i32, y: i32, radius: i32) -> bool {
        let x1 = x - radius;
        let x2 = x + radius;
        let y1 = y - radius;
        let y2 = y + radius;

        for cur_y in y1..=y2 {
            if !L::is_all_in_row(&self.data, cur_y, x1, x2) {
                return false;
            }
        }
        true
    }

    /// 指定した矩形範囲（中心 x, y、半径 radius）内に一つでもセット（1）されたビットがあるか判定
    pub fn is_area_any_set(&self, x: i32, y: i32, radius: i32) -> bool {
        let x1 = x - radius;
        let x2 = x + radius;
        let y1 = y - radius;
        let y2 = y + radius;

        for cur_y in y1..=y2 {
            if L::has_any_in_row(&self.data, cur_y, x1, x2) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::BitBoard;
    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_basic_queries() {
        let mut bb = TestBoard::default();
        assert!(bb.is_empty());
        assert!(!bb.has_any());
        assert_eq!(bb.count_ones(), 0);

        bb.set(10, 10, true);
        assert!(!bb.is_empty());
        assert!(bb.has_any());
        assert_eq!(bb.count_ones(), 1);
    }

    #[test]
    fn test_for_each_bit() {
        let mut bb = TestBoard::default();
        bb.set(10, 10, true);
        bb.set(20, 20, true);

        let mut found = Vec::new();
        bb.for_each_bit(|x, y, _| {
            found.push((x, y));
        });
        found.sort_by_key(|&(x, y)| (y, x));

        assert_eq!(found.len(), 2);
        assert_eq!(found[0], (10, 10));
        assert_eq!(found[1], (20, 20));
    }

    #[test]
    fn test_overlap_queries() {
        let mut a = TestBoard::default();
        let mut b = TestBoard::default();
        a.set(10, 10, true);
        a.set(20, 20, true);
        b.set(20, 20, true);
        b.set(30, 30, true);

        let mut count = 0;
        a.for_each_overlap(&b, |x, y, _| {
            assert_eq!(x, 20);
            assert_eq!(y, 20);
            count += 1;
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn test_area_queries() {
        let mut bb = TestBoard::default();
        // (10, 10) を中心に 3x3 (radius=1) を塗りつぶす
        for y in 9..=11 {
            for x in 9..=11 {
                bb.set(x, y, true);
            }
        }

        // 正常系: すべてセットされている
        assert!(bb.is_area_all_set(10, 10, 1)); // 3x3
        assert!(bb.is_area_all_set(10, 10, 0)); // 1x1 中心
        assert!(bb.is_area_any_set(10, 10, 2)); // より広い範囲

        // 一部欠けている場合
        bb.set(9, 9, false);
        assert!(!bb.is_area_all_set(10, 10, 1));
        assert!(bb.is_area_any_set(10, 10, 1));

        // 完全に空の範囲
        assert!(!bb.is_area_any_set(100, 100, 10));

        // 境界条件: マップ端
        let mut edge_bb = TestBoard::default();
        edge_bb.set(0, 0, true);
        assert!(edge_bb.is_area_all_set(0, 0, 0));
        // radius=1 だと (-1, -1) 等を含むが、これは範囲外なので通常 false 扱い（または実装に依存）
        // 現状の実装では範囲外は 0 (false) として扱うため、all_set は false になる
        assert!(!edge_bb.is_area_all_set(0, 0, 1));
        assert!(edge_bb.is_area_any_set(0, 0, 1));
    }
}
