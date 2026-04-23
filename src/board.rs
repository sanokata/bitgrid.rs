use std::marker::PhantomData;
use crate::layout::{BitLayout, RowMajorLayout};

/// ビットマップデータ構造
/// 型パラメータ W と H でボードサイズを型レベルで固定
/// L でメモリレイアウトを指定 (デフォルトは行アライメント)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitBoard<const W: usize, const H: usize, L: BitLayout<W, H> = RowMajorLayout> {
    pub(crate) data: Box<[u64]>,
    /// 階層化マスク (Level 1): 各ビットは data[i] が 0 でないかを表す
    pub(crate) block_mask: Box<[u64]>,
    _layout: PhantomData<L>,
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    // --- Constants & Static Utilities --

    /// ボード幅（タイル数）
    pub const WIDTH: usize = W;

    /// ボード高さ（タイル数）
    pub const HEIGHT: usize = H;

    /// data 内部配列の総要素数
    pub fn total_words() -> usize {
        L::total_words()
    }

    /// block_mask 内部配列の要素数 (1ビットで1ワードをカバー)
    pub fn block_words() -> usize {
        Self::total_words().div_ceil(64)
    }

    /// 行末パディング用のマスク
    #[allow(dead_code)]
    pub(crate) fn padding_mask() -> u64 {
        L::padding_mask()
    }

    /// ワールド座標からタイル座標への公式な変換 (床関数を使用)
    pub fn pos_to_tile(x: f32, y: f32) -> (i32, i32) {
        (x.floor() as i32, y.floor() as i32)
    }

    /// タイル座標からフラットな空間インデックスへの変換
    pub fn tile_to_index(x: i32, y: i32) -> Option<usize> {
        L::coord_to_flat_index(x, y)
    }

    /// フラットインデックスからタイル座標への変換
    pub fn index_to_tile(idx: usize) -> (i32, i32) {
        L::flat_index_to_coord(idx)
    }

    // --- Creation & Life Cycle ---

    /// 全ビット 0 のボードを生成
    pub fn new() -> Self {
        let total = Self::total_words();
        let block_count = Self::block_words();
        Self {
            data: vec![0u64; total].into_boxed_slice(),
            block_mask: vec![0u64; block_count].into_boxed_slice(),
            _layout: PhantomData,
        }
    }

    /// ボードの状態を整え、整合性を保証する（パディングのクリアとブロック再構築）
    pub fn finalize(&mut self) {
        self.clear_padding();
        self.rebuild_block_mask();
    }

    /// ビットマップ全体を 0 でクリア
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.block_mask.fill(0);
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

    /// 内部データ (data) への読み取り専用アクセス
    #[allow(dead_code)]
    pub(crate) fn data(&self) -> &[u64] {
        &self.data
    }

    /// ブロック 層マスクへの読み取り専用アクセス
    #[allow(dead_code)]
    pub(crate) fn block_mask(&self) -> &[u64] {
        &self.block_mask
    }

    /// 内部的な初期化用
    #[allow(dead_code)]
    pub(crate) fn new_with_mask(
        data: Box<[u64]>,
        block_mask: Box<[u64]>,
    ) -> Self {
        Self { data, block_mask, _layout: PhantomData }
    }

    /// 指定インデックスのワードが非空であることを ブロック マスクに反映
    #[allow(dead_code)]
    pub(crate) fn mark_word_non_empty(&mut self, word_idx: usize) {
        self.block_mask[word_idx / 64] |= 1u64 << (word_idx % 64);
    }

    /// 特定のワードに対してマスクを適用し、ブロック マスクを同期する
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

    /// 指定インデックスのワードの状態に基づいて ブロック マスクを再計算（低速パス）
    #[allow(dead_code)]
    pub(crate) fn recalc_block_word(&mut self, word_idx: usize) {
        if self.data[word_idx] == 0 {
            self.block_mask[word_idx / 64] &= !(1u64 << (word_idx % 64));
        } else {
            self.block_mask[word_idx / 64] |= 1u64 << (word_idx % 64);
        }
    }

    /// 内部データの全走査により ブロック マスクを再構築
    pub fn rebuild_block_mask(&mut self) {
        self.block_mask.fill(0);
        for i in 0..Self::total_words() {
            if self.data[i] != 0 {
                self.block_mask[i / 64] |= 1u64 << (i % 64);
            }
        }
    }

    /// 行ごとの余剰ビット（パディング領域）を 0 クリア
    pub(crate) fn clear_padding(&mut self) {
        if !L::has_padding() { return; }
        
        let mask = L::padding_mask();
        let row_u64s = W.div_ceil(64);
        for row in 0..H {
            let last = row * row_u64s + row_u64s - 1;
            self.data[last] &= mask;
        }
    }


    /// タイル座標を内部インデックス (word_idx, bit_pos) に変換
    pub(crate) fn idx(x: i32, y: i32) -> Option<(usize, u32)> {
        L::coord_to_word_bit(x, y)
    }

    /// 水平方向に指定距離シフトした結果を別のボードに書き込む (アロケーション回避用)
    pub fn shift_horizontal_into(&self, dist: i32, dst: &mut Self) {
        dst.clear();
        L::shift_horizontal(&self.data, &self.block_mask, &mut dst.data, &mut dst.block_mask, dist);
        dst.clear_padding();
    }

    /// 垂直方向に指定距離シフトした結果を別のボードに書き込む (アロケーション回避用)
    pub fn shift_vertical_into(&self, dist: i32, dst: &mut Self) {
        dst.clear();
        L::shift_vertical(&self.data, &self.block_mask, &mut dst.data, &mut dst.block_mask, dist);
        dst.clear_padding();
    }

    /// 水平方向に指定距離シフトした新しいボードを返す
    pub fn shifted_horizontal(&self, dist: i32) -> Self {
        let mut res = Self::new();
        self.shift_horizontal_into(dist, &mut res);
        res
    }

    /// 垂直方向に指定距離シフトした新しいボードを返す
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
        // pos_to_tile
        assert_eq!(TestBoard::pos_to_tile(10.5, 20.9), (10, 20));
        assert_eq!(TestBoard::pos_to_tile(-0.1, -1.5), (-1, -2));

        // tile_to_index / index_to_tile
        let idx = TestBoard::tile_to_index(10, 20).unwrap();
        assert_eq!(TestBoard::index_to_tile(idx), (10, 20));
    }

    #[test]
    fn test_shift_into_allocation_free() {
        let mut bb = TestBoard::default();
        bb.set(100, 100, true);
        
        let mut dst = TestBoard::default();
        bb.shift_horizontal_into(10, &mut dst);
        assert!(dst.get(110, 100));
        assert!(!dst.get(100, 100));
        
        bb.shift_vertical_into(-20, &mut dst);
        assert!(dst.get(100, 80));
    }
}
