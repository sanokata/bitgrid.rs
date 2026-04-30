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

// NOT: 全ビット反転。
// 反転は密になりやすいので block_mask スキップは効かないが、反転と
// block_mask 構築を 1 パスで行うことで rebuild_block_mask の追加走査を排除する。
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
        // src と dst が同じスライスでも安全（インデックスごとに独立な書き換え）
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

/// `src` を反転して `dst_data` に書き込み、同時に `dst_block_mask` を構築する。
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

/// `clear_padding` 後に最終ワードが 0 になった場合のみ block_mask を補正する。
/// レイアウトにパディングがなければ no-op。
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

    // ─── エッジケースのテスト ───────────────────────────────────────

    #[test]
    fn test_not_ref_and_owned_match() {
        let mut a = TestBoard::default();
        a.set(0, 0, true);
        a.set(63, 0, true);
        a.set(64, 0, true);
        a.set(255, 255, true);

        let by_ref = !&a;
        let by_owned = !a.clone();
        assert_eq!(by_ref, by_owned, "&BitBoard と所有版の NOT は一致するべき");
    }

    #[test]
    fn test_not_with_padding_does_not_leak() {
        // W が 64 の倍数でないボードでパディングが反転で漏れないこと
        type Bb = BitBoard<100, 4>;
        let bb = Bb::default();
        let inverted = !bb;
        // 全部 1 のはずだが、padding ビットは 0 のまま
        assert_eq!(inverted.count_ones(), 100 * 4);
        // 範囲外座標へのアクセスは false（パディング由来でないことを確認）
        for (x, _y) in inverted.iter_set_bits() {
            assert!(x < 100, "パディング由来のビットが漏れている: x={x}");
        }
    }

    #[test]
    fn test_and_result_block_mask_consistency() {
        // disjoint な集合での AND は空、block_mask も完全に 0 でなければならない
        let mut a = TestBoard::default();
        let mut b = TestBoard::default();
        a.set(10, 10, true);
        b.set(20, 20, true);
        let r = &a & &b;
        assert_eq!(r.count_ones(), 0);
        assert!(r.is_empty(), "block_mask を含めて空であること");
    }

    #[test]
    fn test_layout_consistency_morton_vs_row_major() {
        use crate::layout::{MortonLayout, RowMajorLayout};

        type Morton = BitBoard<128, 128, MortonLayout>;
        type RowMajor = BitBoard<128, 128, RowMajorLayout>;

        // 同じ座標を立てたとき、論理的には同じ集合になるはず
        let coords = [(0, 0), (63, 0), (64, 0), (100, 100), (127, 127)];

        let mut m = Morton::default();
        let mut r = RowMajor::default();
        for &(x, y) in &coords {
            m.set(x, y, true);
            r.set(x, y, true);
        }

        // count_ones は一致
        assert_eq!(m.count_ones(), r.count_ones());

        // 各座標で get の結果が一致
        for &(x, y) in &coords {
            assert!(m.get(x, y));
            assert!(r.get(x, y));
        }

        // iter_set_bits の集合が一致
        use std::collections::HashSet;
        let m_set: HashSet<_> = m.iter_set_bits().collect();
        let r_set: HashSet<_> = r.iter_set_bits().collect();
        assert_eq!(m_set, r_set);
    }
}
