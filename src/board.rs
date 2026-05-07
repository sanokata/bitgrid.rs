use crate::layout::{BitLayout, RowMajorLayout};
use std::marker::PhantomData;

/// Bitmap data structure
/// Board size is fixed at the type level via W and H parameters.
/// L specifies the memory layout (defaults to Row-Major).
#[derive(Debug, PartialEq, Eq)]
pub struct BitBoard<const W: usize, const H: usize, L: BitLayout<W, H> = RowMajorLayout> {
    pub(crate) data: Box<[u64]>,
    /// Hierarchical mask (Level 1): each bit indicates whether data[i] is non-zero.
    pub(crate) block_mask: Box<[u64]>,
    _layout: PhantomData<L>,
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> Clone for BitBoard<W, H, L> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            block_mask: self.block_mask.clone(),
            _layout: PhantomData,
        }
    }

    /// Copies by reusing the existing buffer. Since size is fixed by const generics,
    /// no re-allocation occurs.
    fn clone_from(&mut self, source: &Self) {
        self.data.copy_from_slice(&source.data);
        self.block_mask.copy_from_slice(&source.block_mask);
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    // --- Constants & Static Utilities --

    /// Board width (number of tiles)
    pub const WIDTH: usize = W;

    /// Board height (number of tiles)
    pub const HEIGHT: usize = H;

    /// Total number of elements in the internal data array
    pub fn total_words() -> usize {
        L::total_words()
    }

    /// Number of elements in the internal block_mask array (1 bit covers 1 word)
    pub fn block_words() -> usize {
        Self::total_words().div_ceil(64)
    }

    /// Official conversion from a continuous position to discrete grid coordinates
    pub fn pos_to_coord(x: f32, y: f32) -> (i32, i32) {
        L::point_to_coord((x, y))
    }

    /// Converts tile coordinates to a flat spatial index
    pub fn tile_to_index(x: i32, y: i32) -> Option<usize> {
        L::coord_to_flat_index(x, y)
    }

    /// Converts a flat index to tile coordinates
    pub fn index_to_tile(idx: usize) -> (i32, i32) {
        L::flat_index_to_coord(idx)
    }

    // --- Creation & Life Cycle ---

    /// Creates a board with all bits set to 0
    pub fn new() -> Self {
        let total = Self::total_words();
        let block_count = Self::block_words();
        Self {
            data: vec![0u64; total].into_boxed_slice(),
            block_mask: vec![0u64; block_count].into_boxed_slice(),
            _layout: PhantomData,
        }
    }

    /// Finalizes the board state to ensure consistency (clears padding and rebuilds block mask)
    pub fn finalize(&mut self) {
        self.clear_padding();
        self.rebuild_block_mask();
    }

    /// Clears the entire bitmap to 0
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.block_mask.fill(0);
    }

    // --- Basic Access & Mutation ---

    /// Gets the bit at the specified coordinates
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> bool {
        Self::idx(x, y).is_some_and(|(word, bit)| (self.data[word] >> bit) & 1 != 0)
    }

    /// Gets the bit in flat index format
    #[inline]
    pub fn get_by_index(&self, idx: usize) -> bool {
        let (x, y) = Self::index_to_tile(idx);
        self.get(x, y)
    }

    /// Sets the bit at the specified coordinates
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, value: bool) {
        if let Some((word, bit)) = Self::idx(x, y) {
            if value {
                self.data[word] |= 1u64 << bit;
                self.block_mask[word / 64] |= 1u64 << (word % 64);
            } else {
                self.data[word] &= !(1u64 << bit);
                if self.data[word] == 0 {
                    self.block_mask[word / 64] &= !(1u64 << (word % 64));
                }
            }
        }
    }

    // --- Internal State Management & Accessors ---

    /// Read-only access to the internal data
    #[allow(dead_code)]
    pub(crate) fn data(&self) -> &[u64] {
        &self.data
    }

    /// Read-only access to the block layer mask
    #[allow(dead_code)]
    pub(crate) fn block_mask(&self) -> &[u64] {
        &self.block_mask
    }

    /// For internal initialization
    #[allow(dead_code)]
    pub(crate) fn new_with_mask(data: Box<[u64]>, block_mask: Box<[u64]>) -> Self {
        Self {
            data,
            block_mask,
            _layout: PhantomData,
        }
    }

    /// Updates the block mask to indicate the word at the specified index is non-empty
    #[allow(dead_code)]
    pub(crate) fn mark_word_non_empty(&mut self, word_idx: usize) {
        self.block_mask[word_idx / 64] |= 1u64 << (word_idx % 64);
    }

    /// Applies a mask to a specific word and synchronizes the block mask
    #[allow(dead_code)]
    pub(crate) fn apply_word_mask(&mut self, word_idx: usize, mask: u64, value: bool) {
        if value {
            self.data[word_idx] |= mask;
            self.mark_word_non_empty(word_idx);
        } else {
            self.data[word_idx] &= !mask;
            self.recalc_block_word(word_idx);
        }
    }

    /// Recalculates the block mask for a specific word index based on its state (slow path)
    #[allow(dead_code)]
    pub(crate) fn recalc_block_word(&mut self, word_idx: usize) {
        if self.data[word_idx] == 0 {
            self.block_mask[word_idx / 64] &= !(1u64 << (word_idx % 64));
        } else {
            self.block_mask[word_idx / 64] |= 1u64 << (word_idx % 64);
        }
    }

    /// Rebuilds the block mask by scanning all internal data
    pub fn rebuild_block_mask(&mut self) {
        self.block_mask.fill(0);
        for i in 0..Self::total_words() {
            if self.data[i] != 0 {
                self.block_mask[i / 64] |= 1u64 << (i % 64);
            }
        }
    }

    /// Clears extra bits in the padding area of each row to 0
    pub(crate) fn clear_padding(&mut self) {
        if !L::has_padding() {
            return;
        }

        let mask = L::padding_mask();
        let row_u64s = W.div_ceil(64);
        for row in 0..H {
            let last = row * row_u64s + row_u64s - 1;
            self.data[last] &= mask;
        }
    }

    /// Converts tile coordinates to internal indices (word_idx, bit_pos)
    #[inline]
    pub(crate) fn idx(x: i32, y: i32) -> Option<(usize, u32)> {
        L::coord_to_word_bit(x, y)
    }

    /// Writes the result of a horizontal shift by a specified distance into another board (avoids allocation)
    pub fn shift_horizontal_into(&self, dist: i32, dst: &mut Self) {
        dst.clear();
        L::shift_horizontal(
            &self.data,
            &self.block_mask,
            &mut dst.data,
            &mut dst.block_mask,
            dist,
        );
        dst.clear_padding();
    }

    /// Writes the result of a vertical shift by a specified distance into another board (avoids allocation)
    pub fn shift_vertical_into(&self, dist: i32, dst: &mut Self) {
        dst.clear();
        L::shift_vertical(
            &self.data,
            &self.block_mask,
            &mut dst.data,
            &mut dst.block_mask,
            dist,
        );
        dst.clear_padding();
    }

    /// Returns a new board shifted horizontally by the specified distance
    pub fn shifted_horizontal(&self, dist: i32) -> Self {
        let mut res = Self::new();
        self.shift_horizontal_into(dist, &mut res);
        res
    }

    /// Returns a new board shifted vertically by the specified distance
    pub fn shifted_vertical(&self, dist: i32) -> Self {
        let mut res = Self::new();
        self.shift_vertical_into(dist, &mut res);
        res
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> Default for BitBoard<W, H, L> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn set_true_and_get() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);
        assert!(bb.get(0, 0));
    }

    #[test]
    fn set_false_clears_bit() {
        let mut bb = TestBoard::default();
        bb.set(5, 3, true);
        bb.set(5, 3, false);
        assert!(!bb.get(5, 3));
    }

    #[test]
    fn get_unset_is_false() {
        let bb = TestBoard::default();
        assert!(!bb.get(10, 10));
    }

    #[test]
    fn out_of_bounds_returns_false() {
        let bb = TestBoard::default();
        assert!(!bb.get(-1, 0));
        assert!(!bb.get(0, -1));
        assert!(!bb.get(TestBoard::WIDTH as i32, 0));
        assert!(!bb.get(0, TestBoard::HEIGHT as i32));
    }

    #[test]
    fn out_of_bounds_set_is_noop() {
        let mut bb = TestBoard::default();
        bb.set(-1, 0, true);
        bb.set(0, -1, true);
        assert!(!bb.get(0, 0));
    }

    #[test]
    fn multiple_bits_independent() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);
        bb.set(1, 0, true);
        bb.set(63, 0, true);
        bb.set(64, 0, true);
        assert!(bb.get(0, 0));
        assert!(bb.get(1, 0));
        assert!(bb.get(63, 0));
        assert!(bb.get(64, 0));
        assert!(!bb.get(2, 0));
    }

    #[test]
    fn clear_resets_all_bits() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);
        bb.set(100, 50, true);
        bb.clear();
        assert!(!bb.get(0, 0));
        assert!(!bb.get(100, 50));
    }

    #[test]
    fn non_64_aligned_width() {
        type SmallBoard = BitBoard<100, 50>;
        let mut bb = SmallBoard::default();
        bb.set(99, 0, true);
        assert!(bb.get(99, 0));
        assert!(!bb.get(100, 0));
        bb.set(0, 49, true);
        assert!(bb.get(0, 49));
        assert!(!bb.get(0, 50));
    }

    #[test]
    fn count_ones_basic() {
        let mut bb = TestBoard::default();
        assert_eq!(bb.count_ones(), 0);
        bb.set(0, 0, true);
        bb.set(100, 100, true);
        bb.set(255, 255, true);
        assert_eq!(bb.count_ones(), 3);
    }

    #[test]
    fn morton_layout_basic() {
        use crate::layout::MortonLayout;
        type MortonBoard = BitBoard<256, 256, MortonLayout>;
        let mut bb = MortonBoard::default();
        bb.set(10, 20, true);
        assert!(bb.get(10, 20));
        assert!(!bb.get(20, 10));
        bb.set(10, 20, false);
        assert!(!bb.get(10, 20));

        // Fill a row
        for x in 0..100 {
            bb.set(x, 50, true);
        }
        assert!(bb.has_any_in_row(50, 0, 99));
        assert!(!bb.has_any_in_row(50, 100, 200));
        assert!(!bb.has_any_in_row(51, 0, 99));
    }

    #[test]
    fn test_coordinate_utilities() {
        // pos_to_coord
        assert_eq!(TestBoard::pos_to_coord(10.5, 20.9), (10, 20));
        assert_eq!(TestBoard::pos_to_coord(-0.1, -1.5), (-1, -2));

        // tile_to_index / index_to_tile
        let idx = TestBoard::tile_to_index(10, 20).unwrap();
        assert_eq!(TestBoard::index_to_tile(idx), (10, 20));
    }

    #[test]
    fn test_block_mask_consistency() {
        let mut bb = TestBoard::default();
        // Set in a sparse state
        bb.set(10, 10, true);
        bb.set(70, 10, true); // Word 1

        let word_idx_10 = TestBoard::idx(10, 10).unwrap().0;
        let word_idx_70 = TestBoard::idx(70, 10).unwrap().0;

        assert!(bb.block_mask[word_idx_10 / 64] & (1 << (word_idx_10 % 64)) != 0);
        assert!(bb.block_mask[word_idx_70 / 64] & (1 << (word_idx_70 % 64)) != 0);

        // Clear
        bb.set(10, 10, false);
        assert!(bb.block_mask[word_idx_10 / 64] & (1 << (word_idx_10 % 64)) == 0);

        // Is it completely cleared?
        bb.clear();
        assert!(bb.block_mask.iter().all(|&w| w == 0));
    }

    #[test]
    fn test_padding_safety() {
        // Board with a width that is not a multiple of 64
        type PaddingBoard = BitBoard<100, 2>;
        let mut bb = PaddingBoard::default();

        // Edge of valid range
        bb.set(99, 0, true);
        assert!(bb.get(99, 0));

        // Are bits in padding area (x=100..127) ignored?
        bb.set(100, 0, true);
        assert!(!bb.get(100, 0));

        // Does rebuild_block_mask ignore padding?
        bb.finalize();
        assert_eq!(bb.count_ones(), 1);
    }

    #[test]
    fn test_large_shifts() {
        let mut bb = TestBoard::default();
        bb.set(100, 100, true);

        // Shift greater than or equal to the board size
        let sh_h = bb.shifted_horizontal(256);
        assert_eq!(sh_h.count_ones(), 0);

        let sh_v = bb.shifted_vertical(-300);
        assert_eq!(sh_v.count_ones(), 0);

        // Shift at the edge of the board
        let sh_edge = bb.shifted_horizontal(155); // 100 + 155 = 255
        assert!(sh_edge.get(255, 100));
        assert_eq!(sh_edge.count_ones(), 1);
    }

    #[test]
    fn test_individual_clear_consistency() {
        let mut bb = TestBoard::default();
        let x = 10;
        let y = 10;
        let (word_idx, _) = TestBoard::idx(x, y).unwrap();

        bb.set(x, y, true);
        assert!(bb.block_mask[word_idx / 64] & (1 << (word_idx % 64)) != 0);

        bb.set(x, y, false);
        assert!(
            bb.block_mask[word_idx / 64] & (1 << (word_idx % 64)) == 0,
            "block_mask should be cleared when last bit in word is unset"
        );
        assert_eq!(bb.count_ones(), 0);
    }

    #[test]
    fn test_equality_and_clear() {
        let mut bb1 = TestBoard::new();
        let bb2 = TestBoard::new();

        bb1.set(50, 50, true);
        bb1.clear();

        assert_eq!(bb1, bb2, "Cleared board should be equal to a new board");
        assert_eq!(bb1.block_mask, bb2.block_mask);
    }

    #[test]
    fn test_extreme_get_set() {
        let mut bb = TestBoard::default();
        // Should not panic
        bb.set(i32::MAX, i32::MAX, true);
        bb.set(i32::MIN, i32::MIN, true);
        assert!(!bb.get(i32::MAX, i32::MAX));
        assert!(!bb.get(i32::MIN, i32::MIN));
    }
}
