/// 行アライメントされたビットマップデータ構造
///
/// 型パラメータ `W` と `H` でボードのタイル幅・高さをコンパイル時に固定する。
/// 各行は [`ROW_U64S`](Self::ROW_U64S) 個の u64 で構成される。`W` が 64 の倍数のとき
/// 垂直方向のビット演算が単純な配列インデックス操作になる。
///
/// 座標計算: `(x, y)` → `(data[y * ROW_U64S + x / 64], x % 64)`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitBoard<const W: usize, const H: usize> {
    pub(crate) data: Vec<u64>,
}

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 1行あたりの u64 ワード数（自動計算）
    pub const ROW_U64S: usize = (W + 63) / 64;

    /// data 配列の総要素数（u64 の個数）
    pub const TOTAL_WORDS: usize = Self::ROW_U64S * H;

    /// ボードの幅（タイル数）
    pub const WIDTH: usize = W;

    /// ボードの高さ（タイル数）
    pub const HEIGHT: usize = H;

    /// 各行の最後のワードに適用するパディングマスク。
    /// `W` が 64 の倍数の場合は全ビット有効 (`!0`)、
    /// そうでない場合は下位 `W % 64` ビットのみ有効。
    pub(crate) const PADDING_MASK: u64 = if W % 64 == 0 {
        !0u64
    } else {
        (1u64 << (W % 64)) - 1
    };

    /// 全ビット 0 で初期化された新しいビットボードを生成する
    pub fn new() -> Self {
        Self {
            data: vec![0u64; Self::TOTAL_WORDS],
        }
    }

    /// タイル座標 `(x, y)` を `(word, bit)` に変換する
    ///
    /// - `word` : `data` のインデックス（`y * ROW_U64S + x / 64`）
    /// - `bit`  : その word 内のビット位置（`x % 64`）
    ///
    /// マップ境界外の場合は `None` を返す。
    pub(crate) fn idx(x: i32, y: i32) -> Option<(usize, u32)> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return None;
        }
        let word = y as usize * Self::ROW_U64S + x as usize / 64;
        let bit = (x as usize % 64) as u32;
        Some((word, bit))
    }

    /// タイル座標 `(x, y)` のビットを設定する
    ///
    /// `value = true` でビットを立て、`false` で落とす。
    /// マップ境界外の座標は無視される。
    pub fn set(&mut self, x: i32, y: i32, value: bool) {
        if let Some((word, bit)) = Self::idx(x, y) {
            if value {
                self.data[word] |= 1u64 << bit;
            } else {
                self.data[word] &= !(1u64 << bit);
            }
        }
    }

    /// タイル座標 `(x, y)` のビットを返す
    ///
    /// マップ境界外の座標は `false` を返す。
    pub fn get(&self, x: i32, y: i32) -> bool {
        Self::idx(x, y).map_or(false, |(word, bit)| (self.data[word] >> bit) & 1 != 0)
    }

    /// ビットマップ全体をクリアする
    pub fn clear(&mut self) {
        self.data.iter_mut().for_each(|v| *v = 0);
    }

    /// ビットが 1 つでも立っているかを確認する（空判定の高速化）
    pub fn any_bits_set(&self) -> bool {
        self.data.iter().any(|&w| w != 0)
    }

    /// 立っているビットの総数を返す
    pub fn count_ones(&self) -> u32 {
        self.data.iter().map(|w| w.count_ones()).sum()
    }

    /// 各行末尾のパディングビットをクリアする。
    ///
    /// `W` が 64 の倍数でない場合、各行の最後の u64 ワードには
    /// ボード範囲外の余剰ビットが存在する。`Not` や `expand` など
    /// パディング領域にゴミビットが発生し得る演算の後に呼び出す。
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
