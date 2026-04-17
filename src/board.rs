/// 行アライメントされたビットマップデータ構造
/// 型パラメータ W と H でボードサイズを型レベルで固定
/// 内部的には u64 配列で保持し、高速なビット演算をサポート
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitBoard<const W: usize, const H: usize> {
    pub(crate) data: Vec<u64>,
}

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 1行あたりの u64 ワード数
    pub const ROW_U64S: usize = (W + 63) / 64;

    /// data 内部配列の総要素数
    pub const TOTAL_WORDS: usize = Self::ROW_U64S * H;

    /// ボード幅（タイル数）
    pub const WIDTH: usize = W;

    /// ボード高さ（タイル数）
    pub const HEIGHT: usize = H;

    /// 行末パディング用のマスク
    pub(crate) const PADDING_MASK: u64 = if W % 64 == 0 {
        !0u64
    } else {
        (1u64 << (W % 64)) - 1
    };

    /// 全ビット 0 のボードを生成
    pub fn new() -> Self {
        Self {
            data: vec![0u64; Self::TOTAL_WORDS],
        }
    }

    /// タイル座標を内部インデックス (word_idx, bit_pos) に変換
    pub(crate) fn idx(x: i32, y: i32) -> Option<(usize, u32)> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return None;
        }
        let word = y as usize * Self::ROW_U64S + x as usize / 64;
        let bit = (x as usize % 64) as u32;
        Some((word, bit))
    }

    /// 指定座標のビットを設定
    pub fn set(&mut self, x: i32, y: i32, value: bool) {
        if let Some((word, bit)) = Self::idx(x, y) {
            if value {
                self.data[word] |= 1u64 << bit;
            } else {
                self.data[word] &= !(1u64 << bit);
            }
        }
    }

    /// 指定座標のビットを取得
    pub fn get(&self, x: i32, y: i32) -> bool {
        Self::idx(x, y).map_or(false, |(word, bit)| (self.data[word] >> bit) & 1 != 0)
    }

    /// ビットマップ全体を 0 でクリア
    pub fn clear(&mut self) {
        self.data.iter_mut().for_each(|v| *v = 0);
    }

    /// いずれかのビットが立っているか判定
    pub fn any_bits_set(&self) -> bool {
        self.data.iter().any(|&w| w != 0)
    }

    /// 指定した行の特定範囲内にビットが立っているか判定
    /// マスク演算による高速一括判定を実行
    pub fn any_in_row(&self, y: i32, min_x: i32, max_x: i32) -> bool {
        if y < 0 || y >= H as i32 || min_x >= W as i32 || max_x < 0 || min_x > max_x {
            return false;
        }
        
        let min_x = min_x.max(0) as usize;
        let max_x = max_x.min((W as i32) - 1) as usize;

        let start_word = (y as usize) * Self::ROW_U64S + min_x / 64;
        let end_word = (y as usize) * Self::ROW_U64S + max_x / 64;

        if start_word == end_word {
            let len = max_x - min_x + 1;
            let mask = if len == 64 { !0u64 } else { ((1u64 << len) - 1) << (min_x % 64) };
            (self.data[start_word] & mask) != 0
        } else {
            let mask_start = !0u64 << (min_x % 64);
            if (self.data[start_word] & mask_start) != 0 { return true; }
            
            for w in (start_word + 1)..end_word {
                if self.data[w] != 0 { return true; }
            }

            let len_end = (max_x % 64) + 1;
            let mask_end = if len_end == 64 { !0u64 } else { (1u64 << len_end) - 1 };
            (self.data[end_word] & mask_end) != 0
        }
    }

    /// 1（オン）状態のビット総数を取得
    pub fn count_ones(&self) -> u32 {
        self.data.iter().map(|w| w.count_ones()).sum()
    }

    /// 行ごとの余剰ビット（パディング領域）を 0 クリア
    pub(crate) fn clear_padding(&mut self) {
        if Self::PADDING_MASK != !0u64 {
            for row in 0..H {
                let last = row * Self::ROW_U64S + Self::ROW_U64S - 1;
                self.data[last] &= Self::PADDING_MASK;
            }
        }
    }
}

impl<const W: usize, const H: usize> Default for BitBoard<W, H> {
    fn default() -> Self {
        Self::new()
    }
}

// Traits removed as they exist in ops.rs

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
        // パニックせず、境界内の値も変わらない
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
        bb.set(64, 0, true); // 次の u64 ワード
        assert!(bb.get(0, 0));
        assert!(bb.get(1, 0));
        assert!(bb.get(63, 0));
        assert!(bb.get(64, 0));
        assert!(!bb.get(2, 0)); // 設定していない箇所
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
        assert!(!bb.get(100, 0)); // 範囲外
        bb.set(0, 49, true);
        assert!(bb.get(0, 49));
        assert!(!bb.get(0, 50)); // 範囲外
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
}
