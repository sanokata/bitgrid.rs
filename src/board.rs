/// 行アライメントされたビットマップデータ構造
/// 型パラメータ W と H でボードサイズを型レベルで固定
/// 内部的には Box<[u64]> で保持
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitBoard<const W: usize, const H: usize> {
    pub(crate) data: Box<[u64]>,
    /// 階層化マスク (Level 1): 各ビットは data[i] が 0 でないかを表す
    pub(crate) l1_mask: Box<[u64]>,
}

impl<const W: usize, const H: usize> BitBoard<W, H> {
    // --- Constants & Static Utilities ---

    /// 1行あたりの u64 ワード数
    pub const ROW_U64S: usize = W.div_ceil(64);

    /// data 内部配列の総要素数
    pub const TOTAL_WORDS: usize = Self::ROW_U64S * H;

    /// l1_mask 内部配列の要素数 (1ビットで1ワードをカバー)
    pub const L1_WORDS: usize = Self::TOTAL_WORDS.div_ceil(64);

    /// ボード幅（タイル数）
    pub const WIDTH: usize = W;

    /// ボード高さ（タイル数）
    pub const HEIGHT: usize = H;

    /// 行末パディング用のマスク
    pub(crate) const PADDING_MASK: u64 = if W.is_multiple_of(64) {
        !0u64
    } else {
        (1u64 << (W % 64)) - 1
    };

    /// ワールド座標からタイル座標への公式な変換 (床関数を使用)
    #[inline(always)]
    pub fn pos_to_tile(x: f32, y: f32) -> (i32, i32) {
        (x.floor() as i32, y.floor() as i32)
    }

    /// タイル座標からフラットな空間インデックス (y * W + x) への変換
    #[inline(always)]
    pub fn tile_to_index(x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            None
        } else {
            Some(y as usize * W + x as usize)
        }
    }

    /// フラットインデックスからタイル座標への変換
    #[inline(always)]
    pub fn index_to_tile(idx: usize) -> (i32, i32) {
        ((idx % W) as i32, (idx / W) as i32)
    }

    // --- Creation & Life Cycle ---

    /// 全ビット 0 のボードを生成
    pub fn new() -> Self {
        Self {
            data: vec![0u64; Self::TOTAL_WORDS].into_boxed_slice(),
            l1_mask: vec![0u64; Self::L1_WORDS].into_boxed_slice(),
        }
    }

    /// ボードの状態を整え、整合性を保証する（パディングのクリアとL1再構築）
    pub fn finalize(&mut self) {
        self.clear_padding();
        self.rebuild_l1();
    }

    /// ビットマップ全体を 0 でクリア
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.l1_mask.fill(0);
    }

    // --- Basic Access & Mutation ---

    /// 指定座標のビットを取得
    pub fn get(&self, x: i32, y: i32) -> bool {
        Self::idx(x, y).is_some_and(|(word, bit)| (self.data[word] >> bit) & 1 != 0)
    }

    /// フラットインデックス形式でビットを取得
    pub fn get_by_index(&self, idx: usize) -> bool {
        let (x, y) = Self::index_to_tile(idx);
        self.get(x, y)
    }

    /// 指定座標のビットを設定
    pub fn set(&mut self, x: i32, y: i32, value: bool) {
        if let Some((word, bit)) = Self::idx(x, y) {
            if value {
                self.data[word] |= 1u64 << bit;
                self.l1_mask[word / 64] |= 1u64 << (word % 64);
            } else {
                self.data[word] &= !(1u64 << bit);
                if self.data[word] == 0 {
                    self.l1_mask[word / 64] &= !(1u64 << (word % 64));
                }
            }
        }
    }


    // --- Queries ---

    /// いずれかのビットが立っているか判定 (L1マスクによる高速判定)
    /// いずれかのビットが立っているか判定 (L1マスクによる高速判定)
    pub fn has_any(&self) -> bool {
        self.l1_mask.iter().any(|&w| w != 0)
    }

    /// ビットが一つも立っていないか判定
    pub fn is_empty(&self) -> bool {
        !self.has_any()
    }

    /// 1（オン）状態のビット総数を取得 (L1マスクで空ワードをスキップ)
    pub fn count_ones(&self) -> u32 {
        let mut count = 0;
        for l1_idx in 0..Self::L1_WORDS {
            let mut l1_word = self.l1_mask[l1_idx];
            while l1_word != 0 {
                let bit = l1_word.trailing_zeros();
                l1_word &= l1_word - 1;
                count += self.data[l1_idx * 64 + bit as usize].count_ones();
            }
        }
        count
    }

    /// 指定した行の特定範囲内にビットが立っているか判定
    /// マスク演算による高速一括判定を実行
    pub fn has_any_in_row(&self, y: i32, min_x: i32, max_x: i32) -> bool {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return false;
        }

        let min_x = min_x.max(0) as usize;
        let max_x = max_x.min((W as i32) - 1) as usize;
        let sw = (y as usize) * Self::ROW_U64S + min_x / 64;
        let ew = (y as usize) * Self::ROW_U64S + max_x / 64;

        if sw == ew {
            return (self.data[sw] & Self::make_mask(min_x % 64, max_x % 64)) != 0;
        }

        (self.data[sw] & Self::make_mask(min_x % 64, 63)) != 0
            || self.data[sw + 1..ew].iter().any(|&w| w != 0)
            || (self.data[ew] & Self::make_mask(0, max_x % 64)) != 0
    }



    // --- Internal State Management & Accessors ---

    /// 内部データ (data) への読み取り専用アクセス
    #[allow(dead_code)]
    pub(crate) fn data(&self) -> &[u64] {
        &self.data
    }

    /// L1 層マスクへの読み取り専用アクセス
    #[allow(dead_code)]
    pub(crate) fn l1_mask(&self) -> &[u64] {
        &self.l1_mask
    }

    /// 内部的な初期化用
    #[allow(dead_code)]
    pub(crate) fn new_with_mask(
        data: Box<[u64]>,
        l1_mask: Box<[u64]>,
    ) -> Self {
        Self { data, l1_mask }
    }

    /// 指定インデックスのワードが非空であることを L1 マスクに反映
    #[inline]
    pub(crate) fn mark_word_non_empty(&mut self, word_idx: usize) {
        self.l1_mask[word_idx / 64] |= 1u64 << (word_idx % 64);
    }

    /// 内部データの全走査により L1 マスクを再構築
    pub fn rebuild_l1(&mut self) {
        self.l1_mask.fill(0);
        for i in 0..Self::TOTAL_WORDS {
            if self.data[i] != 0 {
                self.l1_mask[i / 64] |= 1u64 << (i % 64);
            }
        }
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


    /// タイル座標を内部インデックス (word_idx, bit_pos) に変換
    pub(crate) fn idx(x: i32, y: i32) -> Option<(usize, u32)> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return None;
        }
        let word = y as usize * Self::ROW_U64S + x as usize / 64;
        let bit = (x as usize % 64) as u32;
        Some((word, bit))
    }


}

impl<const W: usize, const H: usize> Default for BitBoard<W, H> {
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



}
