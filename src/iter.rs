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
            if self.current_word != 0 {
                let bit = self.current_word.trailing_zeros();
                // 見つけたビットをリセット
                self.current_word &= self.current_word - 1;

                let y = (self.word_idx / BitBoard::<W, H>::ROW_U64S) as i32;
                let x = ((self.word_idx % BitBoard::<W, H>::ROW_U64S) * 64 + bit as usize) as i32;
                return Some((x, y));
            }

            self.word_idx += 1;
            if self.word_idx >= BitBoard::<W, H>::TOTAL_WORDS {
                return None;
            }
            self.current_word = self.bitmap.data[self.word_idx];
        }
    }
}

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 立っているビットの座標 (x, y) を列挙するイテレータを取得
    pub fn iter_set_bits(&self) -> BitBoardIter<'_, W, H> {
        BitBoardIter {
            bitmap: self,
            word_idx: 0,
            current_word: if Self::TOTAL_WORDS > 0 {
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
