use crate::BitBoard;

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 指定サイズのユニットが通行可能な領域（左上座標の集合）を一括計算
    /// 垂直・水平方向に (size - 1) 回のビットシフト AND を行い、全タイルの空きを確認
    pub fn compute_unit_passable(&self, width: u32, height: u32) -> Self {
        let mut result = self.clone();

        // 垂直方向の縮小: 下方向へ AND を繰り返す
        for _ in 1..height {
            for row in 0..H - 1 {
                let s = row * Self::ROW_U64S;
                let next_s = (row + 1) * Self::ROW_U64S;
                for i in 0..Self::ROW_U64S {
                    // 下の行と論理積を取り「自マスとその下が通行可能」な状態を伝播
                    result.data[s + i] &= result.data[next_s + i];
                }
            }
            // 最下行はユニットの起点になり得ないため 0 クリア
            let last_s = (H - 1) * Self::ROW_U64S;
            for i in 0..Self::ROW_U64S {
                result.data[last_s + i] = 0;
            }
        }

        // 水平方向の縮小: 右方向へ AND を繰り返す
        for _ in 1..width {
            for row in 0..H {
                let s = row * Self::ROW_U64S;
                for i in 0..Self::ROW_U64S {
                    // 右隣ワードからのキャリーを取得
                    let carry = if i + 1 < Self::ROW_U64S {
                        result.data[s + i + 1] << 63
                    } else {
                        0
                    };
                    // 右方向 (x+1) と論理積を取る
                    result.data[s + i] &= (result.data[s + i] >> 1) | carry;
                }
            }
        }

        result
    }

    /// BFS ウェーブフロントを 1 ステップ展開
    /// 現在のフロンティアを 4 方向（上下左右）に広げ、マスク処理と既訪問除外を実行
    pub fn expand(&self, passable: &Self, visited: &mut Self) -> Self {
        let mut next = Self::default();
        self.expand_into(passable, visited, &mut next);
        next
    }

    /// `expand` のアロケーションフリー版。
    /// 結果は事前に確保された `out` バッファに書き込まれる。
    /// `out` は呼び出し前にクリアされている必要はない（内部でゼロ初期化される）。
    pub fn expand_into(&self, passable: &Self, visited: &mut Self, out: &mut Self) {
        // 出力バッファをクリア
        for w in out.data.iter_mut() { *w = 0; }

        for row in 0..H {
            let s = row * Self::ROW_U64S;

            // 垂直方向（北・南）の展開
            if row > 0 {
                for i in 0..Self::ROW_U64S {
                    out.data[s + i] |= self.data[s - Self::ROW_U64S + i];
                }
            }
            if row < H - 1 {
                for i in 0..Self::ROW_U64S {
                    out.data[s + i] |= self.data[s + Self::ROW_U64S + i];
                }
            }

            // 水平方向（東・西）の展開。ワード跨ぎのキャリー伝播を含む
            for i in 0..Self::ROW_U64S {
                // East（左シフト）
                let carry_e = if i > 0 { self.data[s + i - 1] >> 63 } else { 0 };
                out.data[s + i] |= (self.data[s + i] << 1) | carry_e;
                // West（右シフト）
                let carry_w = if i + 1 < Self::ROW_U64S {
                    self.data[s + i + 1] << 63
                } else {
                    0
                };
                out.data[s + i] |= (self.data[s + i] >> 1) | carry_w;
            }
        }

        // 通行可能マスク適用、既訪問除外、および訪問済みリストへの追記
        for i in 0..Self::TOTAL_WORDS {
            out.data[i] &= passable.data[i] & !visited.data[i];
            visited.data[i] |= out.data[i];
        }

        // 行末パディングビットのクリーンアップ
        out.clear_padding();
        out.rebuild_l1();
    }

}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    #[test]
    fn expand_basic() {
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
        let next = frontier.expand(&passable, &mut visited);
        assert!(next.get(1, 2)); // West
        assert!(next.get(3, 2)); // East
        assert!(next.get(2, 1)); // North
        assert!(next.get(2, 3)); // South
        assert!(!next.get(2, 2)); // 既訪問
        assert_eq!(next.count_ones(), 4);
    }

    #[test]
    fn expand_respects_walls() {
        type Bb = BitBoard<8, 8>;
        let mut passable = Bb::default();
        // L字型の通路
        passable.set(0, 0, true);
        passable.set(1, 0, true);
        passable.set(1, 1, true);

        let mut frontier = Bb::default();
        frontier.set(0, 0, true);
        let mut visited = frontier.clone();

        let next = frontier.expand(&passable, &mut visited);
        assert!(next.get(1, 0)); // 通路方向に展開
        assert!(!next.get(0, 1)); // 壁なので展開されない
        assert_eq!(next.count_ones(), 1);
    }

    #[test]
    fn expand_no_padding_leak() {
        // W が 64 の倍数でないボードで expand がパディングビットを漏らさないことを確認
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

        let next = frontier.expand(&passable, &mut visited);
        for &(x, _y) in &next.iter_set_bits().collect::<Vec<_>>() {
            assert!(x < 100, "Padding bit leaked in expand: x={x}");
        }
    }

    #[test]
    fn compute_unit_passable_2x2() {
        type Bb = BitBoard<8, 8>;
        let mut passable = Bb::default();
        for y in 0..4 {
            for x in 0..4 {
                passable.set(x, y, true);
            }
        }

        let result = passable.compute_unit_passable(2, 2);

        // 2×2 ユニットの左上が置ける位置
        assert!(result.get(0, 0));
        assert!(result.get(1, 1));
        assert!(result.get(2, 2)); // 右下が (3,3) で通行可能範囲内
        assert!(!result.get(3, 3)); // 右下が (4,4) で通行不可
        assert!(!result.get(4, 0));
        assert!(!result.get(0, 4));
    }
}
