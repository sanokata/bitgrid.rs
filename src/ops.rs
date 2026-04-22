use crate::BitBoard;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

// ─────────────── マクロ定義 ───────────────────────────────

/// 疎なビットボード向けの二項演算を実装するマクロ
macro_rules! impl_sparse_binop {
    ($Trait:ident, $method:ident, $l1_op:tt, $data_op:tt) => {
        impl<const W: usize, const H: usize> $Trait for &BitBoard<W, H> {
            type Output = BitBoard<W, H>;
            fn $method(self, rhs: Self) -> Self::Output {
                let mut result = BitBoard::new();
                for i in 0..BitBoard::<W, H>::L1_WORDS {
                    let mut bits = self.l1_mask[i] $l1_op rhs.l1_mask[i];
                    while bits != 0 {
                        let bit = bits.trailing_zeros();
                        let idx = i * 64 + bit as usize;
                        let val = self.data[idx] $data_op rhs.data[idx];
                        if val != 0 {
                            result.data[idx] = val;
                            result.l1_mask[i] |= 1u64 << bit;
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
    ($Trait:ident, $method:ident, $l1_union:tt, $data_op:tt) => {
        impl<const W: usize, const H: usize> $Trait<&BitBoard<W, H>> for BitBoard<W, H> {
            fn $method(&mut self, rhs: &BitBoard<W, H>) {
                for i in 0..BitBoard::<W, H>::L1_WORDS {
                    let mut bits = self.l1_mask[i] $l1_union rhs.l1_mask[i];
                    self.l1_mask[i] = 0;
                    while bits != 0 {
                        let bit = bits.trailing_zeros();
                        let idx = i * 64 + bit as usize;
                        self.data[idx] $data_op rhs.data[idx];
                        if self.data[idx] != 0 {
                            self.l1_mask[i] |= 1u64 << bit;
                        }
                        bits &= bits - 1;
                    }
                }
            }
        }
    };
}

// ─────────────── トレイト実装 ───────────────────────────────

// AND: 結果が疎になりやすいため L1 マスクでスキップ
impl_sparse_binop!(BitAnd, bitand, &, &);
impl_sparse_assign_op!(BitAndAssign, bitand_assign, |, &=);

// XOR: 中間的な性質だが、共通項をスキップできるためマクロを使用
impl_sparse_binop!(BitXor, bitxor, |, ^);
impl_sparse_assign_op!(BitXorAssign, bitxor_assign, |, ^=);

// OR: 結果が密になりやすく、単純なループの方が CPU のキャッシュ効率と SIMD 最適化が効くため線形走査
impl<const W: usize, const H: usize> BitOr for &BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn bitor(self, rhs: Self) -> Self::Output {
        let mut result = BitBoard::new();
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            result.data[i] = self.data[i] | rhs.data[i];
        }
        for i in 0..BitBoard::<W, H>::L1_WORDS {
            result.l1_mask[i] = self.l1_mask[i] | rhs.l1_mask[i];
        }
        result
    }
}

impl<const W: usize, const H: usize> BitOrAssign<&BitBoard<W, H>> for BitBoard<W, H> {
    fn bitor_assign(&mut self, rhs: &BitBoard<W, H>) {
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            self.data[i] |= rhs.data[i];
        }
        for i in 0..BitBoard::<W, H>::L1_WORDS {
            self.l1_mask[i] |= rhs.l1_mask[i];
        }
    }
}

// NOT: 全ビット反転
impl<const W: usize, const H: usize> Not for &BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn not(self) -> Self::Output {
        let mut result = BitBoard::new();
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            result.data[i] = !self.data[i];
        }
        result.clear_padding();
        result.rebuild_l1();
        result
    }
}

impl<const W: usize, const H: usize> Not for BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn not(mut self) -> Self::Output {
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            self.data[i] = !self.data[i];
        }
        self.clear_padding();
        self.rebuild_l1();
        self
    }
}

// ─────────────── 所有権ベースの演算子 ───────────────────────────────

impl<const W: usize, const H: usize> BitAnd for BitBoard<W, H> {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output { self &= &rhs; self }
}

impl<const W: usize, const H: usize> BitOr for BitBoard<W, H> {
    type Output = Self;
    fn bitor(mut self, rhs: Self) -> Self::Output { self |= &rhs; self }
}

impl<const W: usize, const H: usize> BitXor for BitBoard<W, H> {
    type Output = Self;
    fn bitxor(mut self, rhs: Self) -> Self::Output { self ^= &rhs; self }
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
    fn not_clears_padding_bits() {
        type Bb = BitBoard<100, 10>;
        let bb = Bb::default();
        let not_bb = !&bb;
        assert!(not_bb.get(0, 0));
        assert!(not_bb.get(99, 9));
        assert_eq!(not_bb.count_ones(), 100 * 10);
    }
}
