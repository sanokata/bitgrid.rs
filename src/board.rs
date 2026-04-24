use std::marker::PhantomData;
use serde::{Serialize, Deserialize, Serializer, Deserializer};
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

/// ビットボードの操作を抽象化するインターフェース
pub trait BitBoardInterface: Send + Sync {
    fn set(&mut self, x: i32, y: i32, value: bool);
    fn get(&self, x: i32, y: i32) -> bool;
    fn count_ones(&self) -> usize;
    fn clear(&mut self);
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoardInterface for BitBoard<W, H, L> {
    fn set(&mut self, x: i32, y: i32, value: bool) {
        self.set(x, y, value);
    }
    fn get(&self, x: i32, y: i32) -> bool {
        self.get(x, y)
    }
    fn count_ones(&self) -> usize {
        self.count_ones()
    }
    fn clear(&mut self) {
        self.clear();
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> Default for BitBoard<W, H, L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const W: usize, const H: usize, L: BitLayout<W, H>> Serialize for BitBoard<W, H, L> {
    /// BitBoard をシリアライズします。
    ///
    /// `block_mask` は `data` から再計算可能な冗長データであるため、
    /// `data` フィールドのみをシリアライズしてデータサイズを削減します。
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.data.serialize(serializer)
    }
}

impl<'de, const W: usize, const H: usize, L: BitLayout<W, H>> Deserialize<'de> for BitBoard<W, H, L> {
    /// BitBoard をデシリアライズします。
    ///
    /// `data` フィールドを復元した後、`rebuild_block_mask` を呼び出して
    /// `block_mask` を再構築します。
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data_vec: Vec<u64> = Vec::deserialize(deserializer)?;
        let expected_len = Self::total_words();

        if data_vec.len() != expected_len {
            return Err(serde::de::Error::custom(format!(
                "invalid data length for BitBoard<{}, {}>: got {}, expected {}",
                W, H, data_vec.len(), expected_len
            )));
        }

        let mut board = Self::new_with_mask(
            data_vec.into_boxed_slice(),
            vec![0u64; Self::block_words()].into_boxed_slice(),
        );
        board.rebuild_block_mask();
        Ok(board)
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
    fn test_block_mask_consistency() {
        let mut bb = TestBoard::default();
        // 疎な状態でセット
        bb.set(10, 10, true);
        bb.set(70, 10, true); // Word 1
        
        let word_idx_10 = TestBoard::idx(10, 10).unwrap().0;
        let word_idx_70 = TestBoard::idx(70, 10).unwrap().0;
        
        assert!(bb.block_mask[word_idx_10 / 64] & (1 << (word_idx_10 % 64)) != 0);
        assert!(bb.block_mask[word_idx_70 / 64] & (1 << (word_idx_70 % 64)) != 0);
        
        // クリア
        bb.set(10, 10, false);
        assert!(bb.block_mask[word_idx_10 / 64] & (1 << (word_idx_10 % 64)) == 0);
        
        // 完全にクリアされたか
        bb.clear();
        assert!(bb.block_mask.iter().all(|&w| w == 0));
    }

    #[test]
    fn test_padding_safety() {
        // 幅が64の倍数でないボード
        type PaddingBoard = BitBoard<100, 2>;
        let mut bb = PaddingBoard::default();
        
        // 有効範囲ギリギリ
        bb.set(99, 0, true);
        assert!(bb.get(99, 0));
        
        // パディング領域 (x=100..127) は無視されるか
        bb.set(100, 0, true);
        assert!(!bb.get(100, 0));
        
        // rebuild_block_mask がパディングを無視するか
        bb.finalize();
        assert_eq!(bb.count_ones(), 1);
    }

    #[test]
    fn test_large_shifts() {
        let mut bb = TestBoard::default();
        bb.set(100, 100, true);
        
        // 盤面サイズ以上のシフト
        let sh_h = bb.shifted_horizontal(256);
        assert_eq!(sh_h.count_ones(), 0);
        
        let sh_v = bb.shifted_vertical(-300);
        assert_eq!(sh_v.count_ones(), 0);
        
        // ギリギリのシフト
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
        assert!(bb.block_mask[word_idx / 64] & (1 << (word_idx % 64)) == 0, "block_mask should be cleared when last bit in word is unset");
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
