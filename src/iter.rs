use crate::{BitBoard, BitLayout};

/// Iterator that visits coordinates where bits are set
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

                // Fast skip using the block layer (hierarchical mask)
                let block_word_idx = self.word_idx / 64;
                let bit_in_block = self.word_idx % 64;
                let block_segment = self.bitmap.block_mask[block_word_idx] >> bit_in_block;

                if block_segment == 0 {
                    // All remaining 64 words in this block word are empty.
                    // Set word_idx to the end of the block so that the next `+= 1`
                    // at the start of the loop reaches the beginning of the next block.
                    let next_block_start = (block_word_idx + 1) * 64;
                    self.word_idx = next_block_start - 1;
                    continue;
                }

                // Identify the next word with set bits
                let skip = block_segment.trailing_zeros() as usize;
                self.word_idx += skip;
                self.current_word = self.bitmap.data[self.word_idx];
            }

            let bit = self.current_word.trailing_zeros();
            // Clear the lowest set bit (n & (n-1))
            self.current_word &= self.current_word - 1;

            let (x, y) = L::word_bit_to_coord(self.word_idx, bit);

            // Normally passes due to the padding mask, but check boundaries for safety
            if x >= 0 && x < W as i32 && y >= 0 && y < H as i32 {
                return Some((x, y));
            }
            // If it was a padding bit, loop to find the next bit
        }
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// Gets an iterator that yields the coordinates (x, y) of all set bits
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
        bb.set(70, 10, true); // Word boundary (64)
        bb.set(100, 100, true);

        let mut coords: Vec<(i32, i32)> = bb.iter_set_bits().collect();
        coords.sort_by_key(|&(x, y)| (y, x));

        assert_eq!(coords.len(), 3);
        assert_eq!(coords[0], (10, 5));
        assert_eq!(coords[1], (70, 10));
        assert_eq!(coords[2], (100, 100));

        // has_any test
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
            assert!((0..10).contains(&x), "Invalid x: {}", x);
            assert!((0..2).contains(&y), "Invalid y: {}", y);
            count += 1;
        }
        assert_eq!(count, 20, "Should only visit 20 bits");

        let mut intersect_count = 0;
        bb.for_each_overlap(&bb, |x, y, _idx| {
            assert!((0..10).contains(&x), "Invalid x in intersection: {}", x);
            assert!((0..2).contains(&y), "Invalid y in intersection: {}", y);
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

    // --- Edge Case Tests ---

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
        // Verify all coordinates are listed uniquely
        use std::collections::HashSet;
        let set: HashSet<_> = coords.iter().copied().collect();
        assert_eq!(set.len(), coords.len());
    }

    #[test]
    fn test_iter_set_bits_word_boundary_continuity() {
        let mut bb = TestBoard::default();
        // Set continuous positions across a word boundary
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
