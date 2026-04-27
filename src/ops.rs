use crate::{BitBoard, BitLayout};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

// ─────────────── マクロ定義 ───────────────────────────────

/// 疎なビットボード向けの二項演算を実装するマクロ
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

/// 疎なビットボード向けの代入演算を実装するマクロ
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

// ─────────────── トレイト実装 ───────────────────────────────

// AND: 結果が疎になりやすいため ブロックマスク でスキップ
impl_sparse_binop!(BitAnd, bitand, &, &);
impl_sparse_assign_op!(BitAndAssign, bitand_assign, |, &=);

// XOR: 中間的な性質だが、共通項をスキップできるためマクロを使用
impl_sparse_binop!(BitXor, bitxor, |, ^);
impl_sparse_assign_op!(BitXorAssign, bitxor_assign, |, ^=);

// OR: 結果が密になりやすいが、入力が疎な場合にはブロックスキップが有効
impl_sparse_binop!(BitOr, bitor, |, |);
impl_sparse_assign_op!(BitOrAssign, bitor_assign, |, |=);

// NOT: 全ビット反転
impl<const W: usize, const H: usize, L: BitLayout<W, H>> Not for &BitBoard<W, H, L> {
    type Output = BitBoard<W, H, L>;
    fn not(self) -> Self::Output {
        let mut result = BitBoard::new();
        for i in 0..BitBoard::<W, H, L>::total_words() {
            result.data[i] = !self.data[i];
        }
        result.clear_padding();
        result.rebuild_block_mask();
        result
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> Not for BitBoard<W, H, L> {
    type Output = BitBoard<W, H, L>;
    fn not(mut self) -> Self::Output {
        for i in 0..BitBoard::<W, H, L>::total_words() {
            self.data[i] = !self.data[i];
        }
        self.clear_padding();
        self.rebuild_block_mask();
        self
    }
}

// ─────────────── 所有権ベースの演算子 ───────────────────────────────

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
}
