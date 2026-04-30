use crate::{BitBoard, BitLayout};

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// 指定サイズのユニットが通行可能な領域（左上座標の集合）を一括計算
    /// 垂直・水平方向に (size - 1) 回のビットシフト AND を指数的に行い、全タイルの空きを確認
    pub fn fit_rect_anchor(&self, width: u32, height: u32) -> Self {
        let mut res = self.clone();
        let mut scratch = Self::new();
        res.fit_rect_anchor_with_buffer(width, height, &mut scratch);
        res
    }

    /// `fit_rect_anchor` の in-place / アロケーションフリー版。
    /// `self` に縮小結果を上書きし、`scratch` をシフト先のバッファとして再利用する。
    /// O(log(max(width, height))) 回呼ばれる shifted_* に伴う allocation を排除する。
    pub fn fit_rect_anchor_with_buffer(
        &mut self,
        width: u32,
        height: u32,
        scratch: &mut Self,
    ) {
        // 垂直方向の縮小
        let mut current_h = 1;
        while current_h < height {
            let d = (height - current_h).min(current_h);
            self.shift_vertical_into(-(d as i32), scratch);
            *self &= &*scratch;
            current_h += d;
        }

        // 水平方向の縮小
        let mut current_w = 1;
        while current_w < width {
            let d = (width - current_w).min(current_w);
            self.shift_horizontal_into(-(d as i32), scratch);
            *self &= &*scratch;
            current_w += d;
        }

        self.finalize();
    }

    /// BFS ウェーブフロントを 1 ステップ展開
    /// 現在のフロンティアを 4 方向（上下左右）に広げ、マスク処理と既訪問除外を実行
    pub fn flood_expand(&self, passable: &Self, visited: &mut Self) -> Self {
        let mut next = Self::default();
        self.flood_expand_into(passable, visited, &mut next);
        next
    }

    /// `flood_expand` のアロケーション抑制版。
    pub fn flood_expand_into(&self, passable: &Self, visited: &mut Self, out: &mut Self) {
        let mut temp = Self::new();
        out.clear();

        // 4方向展開 (上下左右)
        self.shift_vertical_into(-1, &mut temp);
        *out |= &temp;
        self.shift_vertical_into(1, &mut temp);
        *out |= &temp;
        self.shift_horizontal_into(1, &mut temp);
        *out |= &temp;
        self.shift_horizontal_into(-1, &mut temp);
        *out |= &temp;

        // 通行可能マスク適用 + 既訪問除外を実施しつつ、同じ走査内で block_mask を構築。
        // visited を反転させたものをマスクとして使用するが、!visited は密になり得るため
        // block_mask によるスキップは効かない。代わりに rebuild_block_mask の二重走査を排除する。
        out.block_mask.fill(0);
        for i in 0..Self::total_words() {
            out.data[i] &= passable.data[i] & !visited.data[i];
            if out.data[i] != 0 {
                out.block_mask[i / 64] |= 1u64 << (i % 64);
            }
        }

        *visited |= &*out;
    }
}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    #[test]
    fn test_flood_expand_basic() {
        type Bb = BitBoard<16, 16>;
        let mut passable = Bb::default();
        for y in 0..5 {
            for x in 0..5 {
                passable.set(x, y, true);
            }
        }

        let mut frontier = Bb::default();
        frontier.set(2, 2, true);
        let mut visited = frontier.clone();

        // 1 ステップ展開: 4方向の隣接タイル
        let next = frontier.flood_expand(&passable, &mut visited);
        assert!(next.get(1, 2)); // West
        assert!(next.get(3, 2)); // East
        assert!(next.get(2, 1)); // North
        assert!(next.get(2, 3)); // South
        assert!(!next.get(2, 2)); // 既訪問
        assert_eq!(next.count_ones(), 4);
    }

    #[test]
    fn test_flood_expand_respects_walls() {
        type Bb = BitBoard<8, 8>;
        let mut passable = Bb::default();
        // L字型の通路
        passable.set(0, 0, true);
        passable.set(1, 0, true);
        passable.set(1, 1, true);

        let mut frontier = Bb::default();
        frontier.set(0, 0, true);
        let mut visited = frontier.clone();

        let next = frontier.flood_expand(&passable, &mut visited);
        assert!(next.get(1, 0)); // 通路方向に展開
        assert!(!next.get(0, 1)); // 壁なので展開されない
        assert_eq!(next.count_ones(), 1);
    }

    #[test]
    fn test_flood_expand_no_padding_leak() {
        // W が 64 の倍数でないボードで flood_expand がパディングビットを漏らさないことを確認
        type Bb = BitBoard<100, 10>;
        let mut passable = Bb::default();
        for y in 0..10 {
            for x in 0..100 {
                passable.set(x, y, true);
            }
        }

        let mut frontier = Bb::default();
        frontier.set(99, 5, true); // 右端
        let mut visited = frontier.clone();

        let next = frontier.flood_expand(&passable, &mut visited);
        for &(x, _y) in &next.iter_set_bits().collect::<Vec<_>>() {
            assert!(x < 100, "Padding bit leaked in expand: x={x}");
        }
    }

    #[test]
    fn test_fit_rect_anchor_2x2() {
        type Bb = BitBoard<8, 8>;
        let mut passable = Bb::default();
        for y in 0..4 {
            for x in 0..4 {
                passable.set(x, y, true);
            }
        }

        let result = passable.fit_rect_anchor(2, 2);

        // 2×2 ユニットの左上が置ける位置
        assert!(result.get(0, 0));
        assert!(result.get(1, 1));
        assert!(result.get(2, 2)); // 右下が (3,3) で通行可能範囲内
        assert!(!result.get(3, 3)); // 右下が (4,4) で通行不可
        assert!(!result.get(4, 0));
        assert!(!result.get(0, 4));
    }

    #[test]
    fn test_flood_expand_at_edges() {
        type Bb = BitBoard<8, 8>;
        let mut passable = Bb::default();
        for y in 0..8 {
            for x in 0..8 {
                passable.set(x, y, true);
            }
        } // 全て通行可能

        let mut frontier = Bb::default();
        frontier.set(0, 0, true); // 左上隅
        let mut visited = frontier.clone();

        let next = frontier.flood_expand(&passable, &mut visited);
        // 上(-y)と左(-x)には展開されず、右と下のみ展開されること
        assert_eq!(next.count_ones(), 2);
        assert!(next.get(1, 0));
        assert!(next.get(0, 1));
    }

    #[test]
    fn test_fit_rect_anchor_oversize() {
        type Bb = BitBoard<8, 8>;
        let mut passable = Bb::default();
        for y in 0..8 {
            for x in 0..8 {
                passable.set(x, y, true);
            }
        }

        // ボードサイズより大きい要求は全て false になるべき
        let result = passable.fit_rect_anchor(10, 10);
        assert!(result.is_empty());
    }
}
