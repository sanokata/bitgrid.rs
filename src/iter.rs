use crate::{BitBoard, BitLayout};

/// ビットが立っている座標を巡回するイテレータ
pub struct BitBoardIter<'a, const W: usize, const H: usize, L: BitLayout<W, H>> {
    bitmap: &'a BitBoard<W, H, L>,
    word_idx: usize,
    current_word: u64,
}

impl<'a, const W: usize, const H: usize, L: BitLayout<W, H>> Iterator
    for BitBoardIter<'a, W, H, L>
{
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        let total = BitBoard::<W, H, L>::total_words();
        loop {
            while self.current_word == 0 {
                self.word_idx += 1;
                if self.word_idx >= total {
                    return None;
                }

                // ブロック層（階層化マスク）を用いた高速スキップ
                let block_word_idx = self.word_idx / 64;
                let bit_in_block = self.word_idx % 64;
                let block_segment = self.bitmap.block_mask[block_word_idx] >> bit_in_block;

                if block_segment == 0 {
                    // 本ブロックワードに含まれる残りの 64 ワードはすべて空。
                    // ループ先頭の `+= 1` で次のブロック先頭に到達するよう、
                    // ブロック末尾のインデックスをセットする。
                    let next_block_start = (block_word_idx + 1) * 64;
                    self.word_idx = next_block_start - 1;
                    continue;
                }

                // 次にビットが立っているワードを特定
                let skip = block_segment.trailing_zeros() as usize;
                self.word_idx += skip;
                self.current_word = self.bitmap.data[self.word_idx];
            }

            let bit = self.current_word.trailing_zeros();
            // 立っているビットを1つ降ろす (n & (n-1))
            self.current_word &= self.current_word - 1;

            let (x, y) = L::word_bit_to_coord(self.word_idx, bit);

            // パディングマスクにより基本的にはパスするが、安全のため境界チェックを行う
            if x >= 0 && x < W as i32 && y >= 0 && y < H as i32 {
                return Some((x, y));
            }
            // パディングビットだった場合は loop により次のビットを探す
        }
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// 立っているビットの座標 (x, y) を列挙するイテレータを取得
    pub fn iter_set_bits(&self) -> BitBoardIter<'_, W, H, L> {
        BitBoardIter {
            bitmap: self,
            word_idx: 0,
            current_word: if Self::total_words() > 0 {
                self.data[0]
            } else {
                0
            },
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

    // ─── エッジケースのテスト ───────────────────────────────────────

    #[test]
    fn test_iter_set_bits_empty_board_returns_none() {
        let bb = TestBoard::default();
        let mut iter = bb.iter_set_bits();
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_iter_set_bits_full_board_visits_all_unique() {
        type SmallBoard = BitBoard<32, 32>;
        let mut bb = SmallBoard::default();
        bb = !bb;
        let coords: Vec<_> = bb.iter_set_bits().collect();
        assert_eq!(coords.len(), 32 * 32);
        // 全ての座標が一意に列挙されること
        use std::collections::HashSet;
        let set: HashSet<_> = coords.iter().copied().collect();
        assert_eq!(set.len(), coords.len());
    }

    #[test]
    fn test_iter_set_bits_word_boundary_continuity() {
        let mut bb = TestBoard::default();
        // word 境界を跨ぐ連続位置を立てる
        bb.set(63, 0, true);
        bb.set(64, 0, true);
        let coords: Vec<_> = bb.iter_set_bits().collect();
        assert!(coords.contains(&(63, 0)));
        assert!(coords.contains(&(64, 0)));
        assert_eq!(coords.len(), 2);
    }

    #[test]
    fn test_iter_set_bits_after_clear_returns_none() {
        let mut bb = TestBoard::default();
        bb.set(10, 10, true);
        bb.set(50, 50, true);
        bb.clear();
        let mut iter = bb.iter_set_bits();
        assert_eq!(iter.next(), None);
    }
}
