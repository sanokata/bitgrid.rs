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

        // any_bits_set のテスト
        assert!(bb.any_bits_set());
        bb.clear();
        assert!(!bb.any_bits_set());
    }
}
