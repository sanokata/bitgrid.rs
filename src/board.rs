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
    pub fn get_at_index(&self, idx: usize) -> bool {
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

    // --- Bulk Operations ---

    /// 矩形範囲を一括で塗りつぶす (最適化版)
    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, value: bool) {
        if width <= 0 || height <= 0 {
            return;
        }

        let min_y = y.max(0);
        let max_y = (y + height - 1).min((H as i32) - 1);
        let min_x = x.max(0);
        let max_x = (x + width - 1).min((W as i32) - 1);

        if min_y > max_y || min_x > max_x {
            return;
        }

        for current_y in min_y..=max_y {
            self.set_row_range(current_y, min_x, max_x, value);
        }
    }

    /// 指定した行の範囲に一括で値を設定 (内部ワード最適化)
    pub fn set_row_range(&mut self, y: i32, min_x: i32, max_x: i32, value: bool) {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return;
        }

        let min_x = min_x.max(0) as usize;
        let max_x = max_x.min((W as i32) - 1) as usize;
        let sw = (y as usize) * Self::ROW_U64S + min_x / 64;
        let ew = (y as usize) * Self::ROW_U64S + max_x / 64;

        if sw == ew {
            let mask = Self::make_mask(min_x % 64, max_x % 64);
            self.apply_word_mask(sw, mask, value);
            return;
        }

        // 開始ワード
        let s_mask = Self::make_mask(min_x % 64, 63);
        self.apply_word_mask(sw, s_mask, value);

        // 中間ワード
        if ew > sw + 1 {
            let mid_range = sw + 1..ew;
            if value {
                self.data[mid_range.clone()].fill(!0u64);
                for w in mid_range {
                    self.mark_word_non_empty(w);
                }
            } else {
                self.data[mid_range.clone()].fill(0);
                for w in mid_range {
                    self.l1_mask[w / 64] &= !(1u64 << (w % 64));
                }
            }
        }

        // 終了ワード
        let e_mask = Self::make_mask(0, max_x % 64);
        self.apply_word_mask(ew, e_mask, value);
    }

    // --- Queries ---

    /// いずれかのビットが立っているか判定 (L1マスクによる高速判定)
    pub fn any_bits_set(&self) -> bool {
        self.l1_mask.iter().any(|&w| w != 0)
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
    pub fn any_in_row(&self, y: i32, min_x: i32, max_x: i32) -> bool {
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

    // --- Iteration ---

    /// 全てのオン（1）ビットを階層化マスクを利用して高速に走査
    pub fn for_each_set_bit<F>(&self, mut callback: F)
    where
        F: FnMut(i32, i32, usize),
    {
        for l1_idx in 0..Self::L1_WORDS {
            let mut l1_word = self.l1_mask[l1_idx];
            let start_word_idx = l1_idx * 64;
            
            while l1_word != 0 {
                let bit_in_l1 = l1_word.trailing_zeros();
                l1_word &= l1_word - 1;

                let word_idx = start_word_idx + bit_in_l1 as usize;
                if word_idx >= Self::TOTAL_WORDS { break; }

                let mut word_data = self.data[word_idx];
                let y = (word_idx / Self::ROW_U64S) as i32;
                let x_base = (word_idx % Self::ROW_U64S) * 64;
                let y_base_idx = y as usize * W;

                while word_data != 0 {
                    let bit = word_data.trailing_zeros();
                    word_data &= word_data - 1;

                    let x = x_base as i32 + bit as i32;
                    if x < W as i32 {
                        callback(x, y, y_base_idx + x as usize);
                    }
                }
            }
        }
    }

    /// 別のボードとの積集合（AND）が 1 であるビットを指定範囲内のみ高速に走査
    pub fn for_each_intersection<F>(&self, other: &Self, mut callback: F)
    where
        F: FnMut(i32, i32, usize),
    {
        for l1_idx in 0..Self::L1_WORDS {
            let mut combined_l1 = self.l1_mask[l1_idx] & other.l1_mask[l1_idx];
            let start_word_idx = l1_idx * 64;
            
            while combined_l1 != 0 {
                let bit_in_l1 = combined_l1.trailing_zeros();
                combined_l1 &= combined_l1 - 1;

                let word_idx = start_word_idx + bit_in_l1 as usize;
                if word_idx >= Self::TOTAL_WORDS { break; }

                let mut combined_data = self.data[word_idx] & other.data[word_idx];
                let y = (word_idx / Self::ROW_U64S) as i32;
                let x_base = (word_idx % Self::ROW_U64S) * 64;
                let y_base_idx = y as usize * W;

                while combined_data != 0 {
                    let bit = combined_data.trailing_zeros();
                    combined_data &= combined_data - 1;

                    let x = x_base as i32 + bit as i32;
                    if x < W as i32 {
                        callback(x, y, y_base_idx + x as usize);
                    }
                }
            }
        }
    }

    /// 指定したタイル範囲内でのみ、別のボードとの積集合（AND）を高速走査
    pub fn for_each_intersection_in_range<F>(
        &self,
        other: &Self,
        min_tile: (i32, i32),
        max_tile: (i32, i32),
        mut callback: F,
    ) where
        F: FnMut(i32, i32, usize),
    {
        let min_y = min_tile.1.max(0).min(H as i32 - 1) as usize;
        let max_y = max_tile.1.max(0).min(H as i32 - 1) as usize;
        let min_word_x = (min_tile.0.max(0) as usize) / 64;
        let max_word_x = (max_tile.0.min(W as i32 - 1) as usize) / 64;

        for y in min_y..=max_y {
            let row_offset = y * Self::ROW_U64S;
            let y_base_idx = y * W;

            for word_x in min_word_x..=max_word_x {
                let word_idx = row_offset + word_x;

                // L1チェック
                let l1_word_idx = word_idx / 64;
                let bit_in_l1 = word_idx % 64;
                if (self.l1_mask[l1_word_idx] & other.l1_mask[l1_word_idx] & (1u64 << bit_in_l1)) == 0 {
                    continue;
                }

                let mut combined_data = self.data[word_idx] & other.data[word_idx];
                let x_base = word_x * 64;
                while combined_data != 0 {
                    let bit = combined_data.trailing_zeros();
                    combined_data &= combined_data - 1;

                    let x = x_base as i32 + bit as i32;
                    if x < W as i32 {
                        callback(x, y as i32, y_base_idx + x as usize);
                    }
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

    /// 特定ビット範囲のマスクを生成
    #[inline]
    fn make_mask(start_bit: usize, end_bit: usize) -> u64 {
        let len = end_bit - start_bit + 1;
        if len == 64 {
            !0u64
        } else {
            ((1u64 << len) - 1) << start_bit
        }
    }


    /// 特定のワードに対してマスクを適用し、L1 マスクを同期する
    #[inline(always)]
    fn apply_word_mask(&mut self, word_idx: usize, mask: u64, value: bool) {
        if value {
            self.data[word_idx] |= mask;
            self.mark_word_non_empty(word_idx);
        } else {
            self.data[word_idx] &= !mask;
            self.recalc_l1_word(word_idx);
        }
    }

    /// 指定インデックスのワードの状態に基づいて L1 マスクを再計算（低速パス）
    fn recalc_l1_word(&mut self, word_idx: usize) {
        if self.data[word_idx] == 0 {
            self.l1_mask[word_idx / 64] &= !(1u64 << (word_idx % 64));
        } else {
            self.l1_mask[word_idx / 64] |= 1u64 << (word_idx % 64);
        }
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

    #[test]
    fn test_padding_leak() {
        type SmallBoard = BitBoard<10, 2>;
        let mut bb = SmallBoard::default();
        bb = !bb;

        let mut count = 0;
        for (x, y) in bb.iter_set_bits() {
            assert!(x >= 0 && x < 10, "Invalid x: {}", x);
            assert!(y >= 0 && y < 2, "Invalid y: {}", y);
            count += 1;
        }
        assert_eq!(count, 20, "Should only visit 20 bits");

        let mut intersect_count = 0;
        bb.for_each_intersection(&bb, |x, y, _idx| {
            assert!(x >= 0 && x < 10, "Invalid x in intersection: {}", x);
            assert!(y >= 0 && y < 2, "Invalid y in intersection: {}", y);
            intersect_count += 1;
        });
        assert_eq!(intersect_count, 20);
    }

    #[test]
    fn test_for_each_intersection_in_range() {
        let mut bb1 = TestBoard::default();
        let mut bb2 = TestBoard::default();
        bb1.set(100, 100, true);
        bb2.set(100, 100, true);
        bb1.set(200, 100, true);
        bb2.set(200, 100, true);
        bb1.set(100, 200, true);
        bb2.set(100, 200, true);

        let mut hits = Vec::new();
        bb1.for_each_intersection_in_range(&bb2, (90, 90), (110, 110), |x, y, _| {
            hits.push((x, y));
        });

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], (100, 100));
    }
}
