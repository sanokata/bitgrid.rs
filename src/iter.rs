use crate::BitBoard;

/// ビットが立っている座標を巡回するイテレータ
pub struct BitBoardIter<'a, const W: usize, const H: usize> {
    bitmap: &'a BitBoard<W, H>,
    word_idx: usize,
    current_word: u64,
}

impl<'a, const W: usize, const H: usize> Iterator for BitBoardIter<'a, W, H> {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while self.current_word == 0 {
                self.word_idx += 1;
                if self.word_idx >= BitBoard::<W, H>::TOTAL_WORDS {
                    return None;
                }

                // L1層（階層化マスク）を用いた高速スキップ
                let l1_word_idx = self.word_idx / 64;
                let bit_in_l1 = self.word_idx % 64;
                let l1_segment = self.bitmap.l1_mask[l1_word_idx] >> bit_in_l1;

                if l1_segment == 0 {
                    // この L1 ワードに含まれる残りの 64 ワードはすべて空。次の L1 境界までジャンプ
                    self.word_idx = (l1_word_idx + 1) * 64 - 1; // loop冒頭で+1されるため-1
                    continue;
                } else {
                    // 次にビットが立っているワードを特定
                    let skip = l1_segment.trailing_zeros() as usize;
                    self.word_idx += skip;

                    self.current_word = self.bitmap.data[self.word_idx];
                }
            }

            let bit = self.current_word.trailing_zeros();
            // 立っているビットを1つ降ろす (n & (n-1))
            self.current_word &= self.current_word - 1;

            let y = (self.word_idx / BitBoard::<W, H>::ROW_U64S) as i32;
            let x = ((self.word_idx % BitBoard::<W, H>::ROW_U64S) * 64 + bit as usize) as i32;

            // パディングマスクにより基本的にはパスするが、安全のため境界チェックを行う
            if x < W as i32 {
                return Some((x, y));
            }
            // パディングビットだった場合は loop により次のビットを探す
        }
    }
}

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 立っているビットの座標 (x, y) を列挙するイテレータを取得
    pub fn iter_set_bits(&self) -> BitBoardIter<'_, W, H> {
        BitBoardIter {
            bitmap: self,
            word_idx: 0,
            current_word: if BitBoard::<W, H>::TOTAL_WORDS > 0 {
                self.data[0]
            } else {
                0
            },
        }
    }

    /// 全てのオン（1）ビットを階層化マスクを利用して高速に走査
    pub fn for_each_bit<F>(&self, mut callback: F)
    where
        F: FnMut(i32, i32, usize),
    {
        for l1_idx in 0..Self::L1_WORDS {
            let mut l1_word = self.l1_mask[l1_idx];
            let start_word_idx = l1_idx * 64;
            
            while l1_word != 0 {
                let bit_in_l1 = l1_word.trailing_zeros();
                l1_word &= l1_word - 1;

                let word_idx = start_word_idx + bit_in_l1 as usize;
                if word_idx >= Self::TOTAL_WORDS { break; }

                let mut word_data = self.data[word_idx];
                let y = (word_idx / Self::ROW_U64S) as i32;
                let x_base = (word_idx % Self::ROW_U64S) * 64;
                let y_base_idx = y as usize * W;

                while word_data != 0 {
                    let bit = word_data.trailing_zeros();
                    word_data &= word_data - 1;

                    let x = x_base as i32 + bit as i32;
                    if x < W as i32 {
                        callback(x, y, y_base_idx + x as usize);
                    }
                }
            }
        }
    }

    /// 別のボードとの積集合（AND）が 1 であるビットを指定範囲内のみ高速に走査
    pub fn for_each_overlap<F>(&self, other: &Self, mut callback: F)
    where
        F: FnMut(i32, i32, usize),
    {
        for l1_idx in 0..Self::L1_WORDS {
            let mut combined_l1 = self.l1_mask[l1_idx] & other.l1_mask[l1_idx];
            let start_word_idx = l1_idx * 64;
            
            while combined_l1 != 0 {
                let bit_in_l1 = combined_l1.trailing_zeros();
                combined_l1 &= combined_l1 - 1;

                let word_idx = start_word_idx + bit_in_l1 as usize;
                if word_idx >= Self::TOTAL_WORDS { break; }

                let mut combined_data = self.data[word_idx] & other.data[word_idx];
                let y = (word_idx / Self::ROW_U64S) as i32;
                let x_base = (word_idx % Self::ROW_U64S) * 64;
                let y_base_idx = y as usize * W;

                while combined_data != 0 {
                    let bit = combined_data.trailing_zeros();
                    combined_data &= combined_data - 1;

                    let x = x_base as i32 + bit as i32;
                    if x < W as i32 {
                        callback(x, y, y_base_idx + x as usize);
                    }
                }
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
        let min_y = min_tile.1.max(0).min(H as i32 - 1) as usize;
        let max_y = max_tile.1.max(0).min(H as i32 - 1) as usize;
        let min_word_x = (min_tile.0.max(0) as usize) / 64;
        let max_word_x = (max_tile.0.min(W as i32 - 1) as usize) / 64;

        for y in min_y..=max_y {
            let row_offset = y * Self::ROW_U64S;
            let y_base_idx = y * W;

            for word_x in min_word_x..=max_word_x {
                let word_idx = row_offset + word_x;

                // L1チェック
                let l1_word_idx = word_idx / 64;
                let bit_in_l1 = word_idx % 64;
                if (self.l1_mask[l1_word_idx] & other.l1_mask[l1_word_idx] & (1u64 << bit_in_l1)) == 0 {
                    continue;
                }

                let mut combined_data = self.data[word_idx] & other.data[word_idx];

                // X範囲のマスク適用（境界ワードの端を削る）
                if word_x == min_word_x {
                    let start_bit = (min_tile.0 % 64).max(0) as u32;
                    combined_data &= !0u64 << start_bit;
                }
                if word_x == max_word_x {
                    let end_bit = (max_tile.0 % 64).max(0) as u32;
                    if end_bit < 63 {
                        combined_data &= (1u64 << (end_bit + 1)) - 1;
                    }
                }

                let x_base = word_x * 64;
                while combined_data != 0 {
                    let bit = combined_data.trailing_zeros();
                    combined_data &= combined_data - 1;

                    let x = x_base as i32 + bit as i32;
                    if x < W as i32 {
                        callback(x, y as i32, y_base_idx + x as usize);
                    }
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_iter_set_bits() {
        let mut bb = TestBoard::default();
        bb.set(10, 5, true);
        bb.set(70, 10, true); // Word 境界 (64) を超えた位置
        bb.set(100, 100, true);

        let mut coords: Vec<(i32, i32)> = bb.iter_set_bits().collect();
        coords.sort_by_key(|&(x, y)| (y, x));

        assert_eq!(coords.len(), 3);
        assert_eq!(coords[0], (10, 5));
        assert_eq!(coords[1], (70, 10));
        assert_eq!(coords[2], (100, 100));

        // has_any のテスト
        assert!(bb.has_any());
        bb.clear();
        assert!(!bb.has_any());
    }

    #[test]
    fn test_padding_leak() {
        type SmallBoard = BitBoard<10, 2>;
        let mut bb = SmallBoard::default();
        bb = !bb;

        let mut count = 0;
        for (x, y) in bb.iter_set_bits() {
            assert!(x >= 0 && x < 10, "Invalid x: {}", x);
            assert!(y >= 0 && y < 2, "Invalid y: {}", y);
            count += 1;
        }
        assert_eq!(count, 20, "Should only visit 20 bits");

        let mut intersect_count = 0;
        bb.for_each_overlap(&bb, |x, y, _idx| {
            assert!(x >= 0 && x < 10, "Invalid x in intersection: {}", x);
            assert!(y >= 0 && y < 2, "Invalid y in intersection: {}", y);
            intersect_count += 1;
        });
        assert_eq!(intersect_count, 20);
    }

    #[test]
    fn test_for_each_intersection_in_range() {
        let mut bb1 = TestBoard::default();
        let mut bb2 = TestBoard::default();
        bb1.set(100, 100, true);
        bb2.set(100, 100, true);
        bb1.set(200, 100, true);
        bb2.set(200, 100, true);
        bb1.set(100, 200, true);
        bb2.set(100, 200, true);

        let mut hits = Vec::new();
        bb1.for_each_overlap_in(&bb2, (90, 90), (110, 110), |x, y, _| {
            hits.push((x, y));
        });

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], (100, 100));
    }
}
