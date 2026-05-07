use crate::{BitBoard, layout::BitLayout};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

// --- Macros for sparse bitwise operations ---

/// Macro to implement binary operations for sparse bitboards.
macro_rules! impl_sparse_binop {
    ($Trait:ident, $method:ident, $block_op:tt, $data_op:tt) => {
        impl<const W: usize, const H: usize, L: BitLayout<W, H>> $Trait for &BitBoard<W, H, L> {
            type Output = BitBoard<W, H, L>;
            fn $method(self, rhs: Self) -> Self::Output {
                let mut result = BitBoard::new();
                for i in 0..BitBoard::<W, H, L>::block_words() {
                    let mut bits = self.block_mask[i] $block_op rhs.block_mask[i];
                    while bits != 0 {
                        let bit = bits.trailing_zeros();
                        let idx = i * 64 + bit as usize;
                        let val = self.data[idx] $data_op rhs.data[idx];
                        if val != 0 {
                            result.data[idx] = val;
                            result.block_mask[i] |= 1u64 << bit;
                        }
                        bits &= bits - 1;
                    }
                }
                result
            }
        }
    };
}

/// Macro to implement assignment operations for sparse bitboards.
macro_rules! impl_sparse_assign_op {
    ($Trait:ident, $method:ident, $block_union:tt, $data_op:tt) => {
        impl<const W: usize, const H: usize, L: BitLayout<W, H>> $Trait<&BitBoard<W, H, L>> for BitBoard<W, H, L> {
            fn $method(&mut self, rhs: &BitBoard<W, H, L>) {
                for i in 0..BitBoard::<W, H, L>::block_words() {
                    let mut bits = self.block_mask[i] $block_union rhs.block_mask[i];
                    self.block_mask[i] = 0;
                    while bits != 0 {
                        let bit = bits.trailing_zeros();
                        let idx = i * 64 + bit as usize;
                        self.data[idx] $data_op rhs.data[idx];
                        if self.data[idx] != 0 {
                            self.block_mask[i] |= 1u64 << bit;
                        }
                        bits &= bits - 1;
                    }
                }
            }
        }
    };
}

// --- Trait Implementations ---

// AND: Skips via block mask as result is likely to be sparse
impl_sparse_binop!(BitAnd, bitand, &, &);
impl_sparse_assign_op!(BitAndAssign, bitand_assign, |, &=);

// XOR: Intermediate nature, but can skip common parts via macro
impl_sparse_binop!(BitXor, bitxor, |, ^);
impl_sparse_assign_op!(BitXorAssign, bitxor_assign, |, ^=);

// OR: Result likely to be dense, but block skip is effective if inputs are sparse
impl_sparse_binop!(BitOr, bitor, |, |);
impl_sparse_assign_op!(BitOrAssign, bitor_assign, |, |=);

// NOT
// Performs inversion and block_mask construction in a single pass to eliminate an additional pass by rebuild_block_mask.
impl<const W: usize, const H: usize, L: BitLayout<W, H>> Not for &BitBoard<W, H, L> {
    type Output = BitBoard<W, H, L>;
    fn not(self) -> Self::Output {
        let mut result = BitBoard::new();
        not_into(&self.data, &mut result.data, &mut result.block_mask);
        result.clear_padding();
        fix_padding_block_mask::<W, H, L>(&result.data, &mut result.block_mask);
        result
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> Not for BitBoard<W, H, L> {
    type Output = BitBoard<W, H, L>;
    fn not(mut self) -> Self::Output {
        // src and dst can be the same slice (independent rewrites per index)
        let total = BitBoard::<W, H, L>::total_words();
        self.block_mask.fill(0);
        for i in 0..total {
            self.data[i] = !self.data[i];
            if self.data[i] != 0 {
                self.block_mask[i / 64] |= 1u64 << (i % 64);
            }
        }
        self.clear_padding();
        fix_padding_block_mask::<W, H, L>(&self.data, &mut self.block_mask);
        self
    }
}

/// Inverts `src` and writes to `dst_data`, while simultaneously constructing `dst_block_mask`.
#[inline]
fn not_into(src: &[u64], dst_data: &mut [u64], dst_block_mask: &mut [u64]) {
    debug_assert_eq!(src.len(), dst_data.len());
    dst_block_mask.fill(0);
    for (i, &w) in src.iter().enumerate() {
        let inverted = !w;
        dst_data[i] = inverted;
        if inverted != 0 {
            dst_block_mask[i / 64] |= 1u64 << (i % 64);
        }
    }
}

/// Corrects `block_mask` if the final word becomes 0 after `clear_padding`.
/// No-op if the layout has no padding.
#[inline]
fn fix_padding_block_mask<const W: usize, const H: usize, L: BitLayout<W, H>>(
    data: &[u64],
    block_mask: &mut [u64],
) {
    if !L::has_padding() {
        return;
    }
    let row_u64s = W.div_ceil(64);
    for row in 0..H {
        let idx = row * row_u64s + row_u64s - 1;
        if data[idx] == 0 {
            block_mask[idx / 64] &= !(1u64 << (idx % 64));
        }
    }
}

// --- Ownership-based Operators ---

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitAnd for BitBoard<W, H, L> {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output {
        self &= &rhs;
        self
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitOr for BitBoard<W, H, L> {
    type Output = Self;
    fn bitor(mut self, rhs: Self) -> Self::Output {
        self |= &rhs;
        self
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitXor for BitBoard<W, H, L> {
    type Output = Self;
    fn bitxor(mut self, rhs: Self) -> Self::Output {
        self ^= &rhs;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_bitwise_ops() {
        let mut a = TestBoard::default();
        let mut b = TestBoard::default();
        a.set(10, 10, true);
        a.set(20, 20, true);
        b.set(20, 20, true);
        b.set(30, 30, true);

        let and_res = &a & &b;
        assert!(!and_res.get(10, 10));
        assert!(and_res.get(20, 20));
        assert!(!and_res.get(30, 30));

        let or_res = &a | &b;
        assert!(or_res.get(10, 10));
        assert!(or_res.get(20, 20));
        assert!(or_res.get(30, 30));

        let xor_res = &a ^ &b;
        assert!(xor_res.get(10, 10));
        assert!(!xor_res.get(20, 20));
        assert!(xor_res.get(30, 30));
    }

    #[test]
    fn test_ops_identity_cases() {
        let mut a = TestBoard::default();
        a.set(10, 10, true);

        // OR with empty
        let or_res = &a | &TestBoard::default();
        assert_eq!(or_res.count_ones(), 1);

        // AND with self
        let and_res = &a & &a;
        assert_eq!(and_res.count_ones(), 1);

        // XOR with self
        let xor_res = &a ^ &a;
        assert_eq!(xor_res.count_ones(), 0);
    }

    #[test]
    fn test_ops_with_padding() {
        type Bb = BitBoard<100, 1>;
        let mut a = Bb::default();
        let mut b = Bb::default();
        a.set(99, 0, true);
        b.set(99, 0, true);

        let and_res = &a & &b;
        assert_eq!(and_res.count_ones(), 1);

        let not_a = !&a;
        assert_eq!(not_a.count_ones(), 99); // 100 - 1
        assert!(!not_a.get(99, 0));
    }

    #[test]
    fn test_not_empty_is_full() {
        let empty = TestBoard::default();
        let full = !empty;
        assert_eq!(full.count_ones(), 256 * 256);
        assert!(full.get(0, 0));
        assert!(full.get(255, 255));
    }

    #[test]
    fn test_ops_on_full_boards() {
        let mut a = TestBoard::default();
        let mut b = TestBoard::default();
        a = !a; // Full
        b.set(10, 10, true);

        let and_res = &a & &b;
        assert_eq!(and_res.count_ones(), 1);
        assert!(and_res.get(10, 10));

        let xor_res = &a ^ &b;
        assert_eq!(xor_res.count_ones(), (256 * 256) - 1);
        assert!(!xor_res.get(10, 10));
    }

    // --- Edge Case Tests ---

    #[test]
    fn test_not_ref_and_owned_match() {
        let mut a = TestBoard::default();
        a.set(0, 0, true);
        a.set(63, 0, true);
        a.set(64, 0, true);
        a.set(255, 255, true);

        let by_ref = !&a;
        let by_owned = !a.clone();
        assert_eq!(
            by_ref, by_owned,
            "In NOT, ref and owned versions should match"
        );
    }

    #[test]
    fn test_not_with_padding_does_not_leak() {
        // Ensure padding does not leak in NOT for boards where W is not a multiple of 64
        type Bb = BitBoard<100, 4>;
        let bb = Bb::default();
        let inverted = !bb;
        // Should be all 1s logically, but padding bits remain 0
        assert_eq!(inverted.count_ones(), 100 * 4);
        // Access to out-of-bounds coordinates is false (verify it's not due to padding)
        for (x, _y) in inverted.iter_set_bits() {
            assert!(x < 100, "Padding bit leaked: x={x}");
        }
    }

    #[test]
    fn test_and_result_block_mask_consistency() {
        // AND on disjoint sets should be empty, and block_mask must be completely 0
        let mut a = TestBoard::default();
        let mut b = TestBoard::default();
        a.set(10, 10, true);
        b.set(20, 20, true);
        let r = &a & &b;
        assert_eq!(r.count_ones(), 0);
        assert!(r.is_empty(), "Should be empty including block_mask");
    }

    #[test]
    fn test_layout_consistency_morton_vs_row_major() {
        use crate::layout::{MortonLayout, RowMajorLayout};

        type Morton = BitBoard<128, 128, MortonLayout>;
        type RowMajor = BitBoard<128, 128, RowMajorLayout>;

        // When the same coordinates are set, they should be logically identical sets
        let coords = [(0, 0), (63, 0), (64, 0), (100, 100), (127, 127)];

        let mut m = Morton::default();
        let mut r = RowMajor::default();
        for &(x, y) in &coords {
            m.set(x, y, true);
            r.set(x, y, true);
        }

        // count_ones matches
        assert_eq!(m.count_ones(), r.count_ones());

        // get results match for each coordinate
        for &(x, y) in &coords {
            assert!(m.get(x, y));
            assert!(r.get(x, y));
        }

        // iter_set_bits sets match
        use std::collections::HashSet;
        let m_set: HashSet<_> = m.iter_set_bits().collect();
        let r_set: HashSet<_> = r.iter_set_bits().collect();
        assert_eq!(m_set, r_set);
    }
}
