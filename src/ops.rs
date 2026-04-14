use crate::BitBoard;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

impl<const W: usize, const H: usize> BitAnd for &BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn bitand(self, rhs: Self) -> Self::Output {
        let mut data = Vec::with_capacity(BitBoard::<W, H>::TOTAL_WORDS);
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            data.push(self.data[i] & rhs.data[i]);
        }
        BitBoard::<W, H> { data }
    }
}

impl<const W: usize, const H: usize> BitAndAssign<&BitBoard<W, H>> for BitBoard<W, H> {
    fn bitand_assign(&mut self, rhs: &BitBoard<W, H>) {
        for i in 0..Self::TOTAL_WORDS {
            self.data[i] &= rhs.data[i];
        }
    }
}

impl<const W: usize, const H: usize> BitOr for &BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn bitor(self, rhs: Self) -> Self::Output {
        let mut data = Vec::with_capacity(BitBoard::<W, H>::TOTAL_WORDS);
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            data.push(self.data[i] | rhs.data[i]);
        }
        BitBoard::<W, H> { data }
    }
}

impl<const W: usize, const H: usize> BitOrAssign<&BitBoard<W, H>> for BitBoard<W, H> {
    fn bitor_assign(&mut self, rhs: &BitBoard<W, H>) {
        for i in 0..Self::TOTAL_WORDS {
            self.data[i] |= rhs.data[i];
        }
    }
}

impl<const W: usize, const H: usize> BitXor for &BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn bitxor(self, rhs: Self) -> Self::Output {
        let mut data = Vec::with_capacity(BitBoard::<W, H>::TOTAL_WORDS);
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            data.push(self.data[i] ^ rhs.data[i]);
        }
        BitBoard::<W, H> { data }
    }
}

impl<const W: usize, const H: usize> BitXorAssign<&BitBoard<W, H>> for BitBoard<W, H> {
    fn bitxor_assign(&mut self, rhs: &BitBoard<W, H>) {
        for i in 0..Self::TOTAL_WORDS {
            self.data[i] ^= rhs.data[i];
        }
    }
}

impl<const W: usize, const H: usize> Not for &BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn not(self) -> Self::Output {
        let mut data = Vec::with_capacity(BitBoard::<W, H>::TOTAL_WORDS);
        for i in 0..BitBoard::<W, H>::TOTAL_WORDS {
            data.push(!self.data[i]);
        }
        let mut result = BitBoard::<W, H> { data };
        result.clear_padding();
        result
    }
}

impl<const W: usize, const H: usize> Not for BitBoard<W, H> {
    type Output = BitBoard<W, H>;
    fn not(mut self) -> Self::Output {
        for i in 0..Self::TOTAL_WORDS {
            self.data[i] = !self.data[i];
        }
        self.clear_padding();
        self
    }
}

// ─────────────── owned value operators ───────────────────────────────

impl<const W: usize, const H: usize> BitAnd for BitBoard<W, H> {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output {
        self &= &rhs;
        self
    }
}

impl<const W: usize, const H: usize> BitOr for BitBoard<W, H> {
    type Output = Self;
    fn bitor(mut self, rhs: Self) -> Self::Output {
        self |= &rhs;
        self
    }
}

impl<const W: usize, const H: usize> BitXor for BitBoard<W, H> {
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

        // AND: 共通部分のみ
        let and_res: TestBoard = &a & &b;
        assert!(!and_res.get(10, 10));
        assert!(and_res.get(20, 20));
        assert!(!and_res.get(30, 30));

        // OR: 両方の和
        let or_res: TestBoard = &a | &b;
        assert!(or_res.get(10, 10));
        assert!(or_res.get(20, 20));
        assert!(or_res.get(30, 30));

        // NOT: 反転
        let not_a: TestBoard = !&a;
        assert!(!not_a.get(10, 10));
        assert!(not_a.get(0, 0));
    }

    #[test]
    fn not_clears_padding_bits() {
        // W=100 は 64 の倍数でないため、パディングビットが存在する
        type Bb = BitBoard<100, 10>;
        let bb = Bb::default();
        let not_bb = !&bb;

        // 有効範囲内のビットは全て 1
        assert!(not_bb.get(0, 0));
        assert!(not_bb.get(99, 9));

        // パディングビットが漏れていないことを確認
        let coords: Vec<_> = not_bb.iter_set_bits().collect();
        for &(x, _y) in &coords {
            assert!(x < 100, "Padding bit leaked: x={x}");
        }
        assert_eq!(not_bb.count_ones(), 100 * 10);
    }

    #[test]
    fn assign_operators() {
        let mut a = TestBoard::default();
        let b = {
            let mut b = TestBoard::default();
            b.set(5, 5, true);
            b
        };
        a.set(5, 5, true);
        a.set(10, 10, true);

        // XorAssign: 共通ビットが消える
        let mut c = a.clone();
        c ^= &b;
        assert!(!c.get(5, 5));
        assert!(c.get(10, 10));

        // OrAssign
        let mut d = TestBoard::default();
        d |= &b;
        assert!(d.get(5, 5));

        // AndAssign
        let mut e = a.clone();
        e &= &b;
        assert!(e.get(5, 5));
        assert!(!e.get(10, 10));
    }

    #[test]
    fn owned_value_operators() {
        let mut a = TestBoard::default();
        let mut b = TestBoard::default();
        a.set(10, 10, true);
        a.set(20, 20, true);
        b.set(10, 10, true);

        // owned AND
        let result = a.clone() & b.clone();
        assert!(result.get(10, 10));
        assert!(!result.get(20, 20));

        // owned OR
        let result = a.clone() | b.clone();
        assert!(result.get(10, 10));
        assert!(result.get(20, 20));

        // owned XOR
        let result = a.clone() ^ b.clone();
        assert!(!result.get(10, 10));
        assert!(result.get(20, 20));
    }
}
