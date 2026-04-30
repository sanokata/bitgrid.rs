use crate::{BitBoard, BitLayout};

/// レイの方向ベクトルを「水平」とみなすときの判定 ε。
/// vy の絶対値がこの値未満なら「ほぼ水平」として全範囲または無効範囲に分岐する。
const RAY_DIRECTION_EPSILON: f32 = 1e-6;

/// シャドウキャスティングで参照するセル境界スロープのオフセット。
/// セルの中心を 0、左右端を ±0.5 とした場合の境界傾斜。
const CELL_SLOPE_OFFSET: f32 = 0.5;

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// 指定した矩形範囲のみを 1 にしたマスクを作成
    /// 範囲情報の高速な抽出・制限に使用
    pub fn mask_rect(x: i32, y: i32, width: i32, height: i32) -> Self {
        let mut res = Self::new();
        L::rect_op(
            &mut res.data,
            &mut res.block_mask,
            x,
            y,
            width,
            height,
            true,
        );
        res
    }

    pub fn mask_sector(
        cx: i32,
        cy: i32,
        radius: f32,
        start_angle_deg: f32,
        sweep_angle_deg: f32,
    ) -> Self {
        let mut mask = Self::new();
        if radius <= 0.0 {
            return mask;
        }

        let sweep_abs = sweep_angle_deg.abs();
        let is_circle = sweep_abs >= 360.0;
        let is_convex = sweep_abs <= 180.0;

        let start_rad = start_angle_deg.to_radians();
        let sweep_rad = sweep_angle_deg.to_radians();
        let end_rad = start_rad + sweep_rad;

        let (s_vy, s_vx) = start_rad.sin_cos();
        let (e_vy, e_vx) = end_rad.sin_cos();

        let cx_f = cx as f32;
        let cy_f = cy as f32;
        let r_sq = radius * radius;
        let r_i = radius.ceil() as i32;

        let y_min = (cy - r_i).max(0);
        let y_max = (cy + r_i).min(H as i32 - 1);

        for y in y_min..=y_max {
            let dy = y as f32 - cy_f;
            let dx_limit_sq = r_sq - dy * dy;
            if dx_limit_sq < 0.0 {
                continue;
            }
            let dx_limit = dx_limit_sq.sqrt();
            let x_c_min = -dx_limit;
            let x_c_max = dx_limit;

            if is_circle {
                mask.set_row(
                    y,
                    (cx_f + x_c_min).ceil() as i32,
                    (cx_f + x_c_max).floor() as i32,
                    true,
                );
            } else if is_convex {
                let (f_min, f_max) =
                    Self::calc_convex_range(dy, x_c_min, x_c_max, s_vx, s_vy, e_vx, e_vy);
                if f_min <= f_max {
                    mask.set_row(
                        y,
                        (cx_f + f_min).ceil() as i32,
                        (cx_f + f_max).floor() as i32,
                        true,
                    );
                }
            } else {
                // 凹型: 円を塗ってから「隙間（逆側の凸セクター）」を消去
                mask.set_row(
                    y,
                    (cx_f + x_c_min).ceil() as i32,
                    (cx_f + x_c_max).floor() as i32,
                    true,
                );
                let (g_min, g_max) =
                    Self::calc_convex_range(dy, x_c_min, x_c_max, e_vx, e_vy, s_vx, s_vy);
                if g_min <= g_max {
                    mask.set_row(
                        y,
                        (cx_f + g_min).ceil() as i32,
                        (cx_f + g_max).floor() as i32,
                        false,
                    );
                }
            }
        }

        mask
    }

    /// 2つのレイ（開始/終了）に挟まれた凸領域の x 範囲を計算する
    fn calc_convex_range(
        dy: f32,
        x_min: f32,
        x_max: f32,
        s_vx: f32,
        s_vy: f32,
        e_vx: f32,
        e_vy: f32,
    ) -> (f32, f32) {
        let (r1_min, r1_max) = Self::get_ray_x_limit(dy, s_vx, s_vy, true);
        let (r2_min, r2_max) = Self::get_ray_x_limit(dy, e_vx, e_vy, false);
        (x_min.max(r1_min).max(r2_min), x_max.min(r1_max).min(r2_max))
    }

    /// 特定のレイ（方向ベクトル vx, vy）による x の境界範囲を計算
    fn get_ray_x_limit(dy: f32, vx: f32, vy: f32, is_start: bool) -> (f32, f32) {
        if vy.abs() < RAY_DIRECTION_EPSILON {
            // 水平レイ: dy の正負とベクトルの向きで全範囲か無効範囲かが決まる
            let ok = if is_start {
                vx * dy >= -RAY_DIRECTION_EPSILON
            } else {
                vx * dy <= RAY_DIRECTION_EPSILON
            };
            if ok {
                (f32::NEG_INFINITY, f32::INFINITY)
            } else {
                (f32::INFINITY, f32::NEG_INFINITY)
            }
        } else {
            let bias = if is_start {
                RAY_DIRECTION_EPSILON
            } else {
                -RAY_DIRECTION_EPSILON
            };
            let x_limit = (vx * dy + bias) / vy;
            if vy > 0.0 {
                if is_start {
                    (f32::NEG_INFINITY, x_limit)
                } else {
                    (x_limit, f32::INFINITY)
                }
            } else if is_start {
                (x_limit, f32::INFINITY)
            } else {
                (f32::NEG_INFINITY, x_limit)
            }
        }
    }

    /// 遮蔽物（opaque_board）を考慮した視界マスクを生成 (新しいボードを確保)
    pub fn mask_visibility(
        cx: i32,
        cy: i32,
        radius: f32,
        opaque_board: &BitBoard<W, H, L>,
    ) -> Self {
        let mut mask = Self::default();
        mask.mask_visibility_into(cx, cy, radius, opaque_board);
        mask
    }

    /// 遮蔽物（opaque_board）を考慮した視界マスクを既存のボードに生成 (アロケーションフリー)
    pub fn mask_visibility_into(
        &mut self,
        cx: i32,
        cy: i32,
        radius: f32,
        opaque_board: &BitBoard<W, H, L>,
    ) {
        self.clear();
        self.set(cx, cy, true); // 立っている位置は必ず見える

        // 8 オクタントの基底ベクトル (xx, xy, yx, yy)
        const OCTANTS: [Octant; 8] = [
            Octant { xx: 1, xy: 0, yx: 0, yy: -1 },
            Octant { xx: 0, xy: 1, yx: -1, yy: 0 },
            Octant { xx: 0, xy: 1, yx: 1, yy: 0 },
            Octant { xx: -1, xy: 0, yx: 0, yy: 1 },
            Octant { xx: -1, xy: 0, yx: 0, yy: -1 },
            Octant { xx: 0, xy: -1, yx: -1, yy: 0 },
            Octant { xx: 0, xy: -1, yx: 1, yy: 0 },
            Octant { xx: 1, xy: 0, yx: 0, yy: 1 },
        ];

        for octant in OCTANTS {
            self.scan_octant(cx, cy, radius, 1, 1.0, 0.0, octant, opaque_board);
        }

        // 階層化マスクを更新（走査中に set を呼んでいるため）
        self.rebuild_block_mask();
    }

    /// 再帰的シャドウキャスティングの走査コアロジック。
    /// `octant` で 8 方向の基底ベクトルを抽象化し、引数を集約する。
    ///
    /// アルゴリズムは Berg/Mejaski 式の Recursive Shadowcasting:
    /// - 距離 `distance` の行を 1 列ずつ走査し、各セルの左右端傾斜と
    ///   現在の楔（start_slope / end_slope）を比較して可視判定を行う
    /// - 不透明セルにぶつかったら以降の楔を縮め、視認可能な区間ごとに再帰
    fn scan_octant(
        &mut self,
        cx: i32,
        cy: i32,
        radius: f32,
        row: i32,
        mut start_slope: f32,
        end_slope: f32,
        octant: Octant,
        opaque_board: &BitBoard<W, H, L>,
    ) {
        if start_slope < end_slope {
            return;
        }

        let radius_sq = radius * radius;

        for distance in row..=(radius.ceil() as i32) {
            // セルの直前状態: None=初期 / Some(true)=不透明 / Some(false)=透明
            let mut last_was_opaque: Option<bool> = None;

            for i in (0..=distance).rev() {
                let dx = distance * octant.xx + i * octant.xy;
                let dy = distance * octant.yx + i * octant.yy;
                let x = cx + dx;
                let y = cy + dy;

                // マップ範囲外は走査をスキップ（高速化のため早期に判定）
                if x < 0 || x >= W as i32 || y < 0 || y >= H as i32 {
                    continue;
                }

                // 当該セルの左右端の傾斜（start_slope/end_slope は楔の左右境界）。
                // セル中心 ± CELL_SLOPE_OFFSET を取って左右端の傾斜を計算する。
                let l_slope =
                    (i as f32 + CELL_SLOPE_OFFSET) / (distance as f32 - CELL_SLOPE_OFFSET);
                let r_slope =
                    (i as f32 - CELL_SLOPE_OFFSET) / (distance as f32 + CELL_SLOPE_OFFSET);

                if start_slope < r_slope {
                    continue;
                }
                if end_slope > l_slope {
                    break;
                }

                if (dx * dx + dy * dy) as f32 <= radius_sq {
                    self.set(x, y, true);
                }

                let is_opaque = opaque_board.get(x, y);
                match last_was_opaque {
                    Some(true) if !is_opaque => {
                        // 不透明 → 透明: 楔の左境界を更新して継続
                        start_slope = l_slope;
                        last_was_opaque = Some(false);
                    }
                    Some(false) if is_opaque
                        && distance < radius as i32
                        && l_slope > end_slope =>
                    {
                        // 透明 → 不透明: 見えている区間で再帰し、不透明をマーク
                        self.scan_octant(
                            cx,
                            cy,
                            radius,
                            distance + 1,
                            start_slope,
                            l_slope,
                            octant,
                            opaque_board,
                        );
                        last_was_opaque = Some(true);
                    }
                    _ => {
                        last_was_opaque = Some(is_opaque);
                    }
                }
            }

            // 行末が不透明で終わった場合、これ以上遠い距離は楔全体が遮蔽される
            if last_was_opaque == Some(true) {
                break;
            }
        }
    }
}

/// シャドウキャスティングの 8 オクタントを示す基底変換。
/// (dx, dy) = (distance * xx + i * xy, distance * yx + i * yy) の形で
/// オクタントごとの座標変換を表現する。
#[derive(Debug, Clone, Copy)]
struct Octant {
    xx: i32,
    xy: i32,
    yx: i32,
    yy: i32,
}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_mask_rect() {
        // 64タイルを跨ぐ矩形 (x=60, w=10)
        // word 0 の bits 60-63 と word 1 の bits 0-5 が 1 になるはず
        let mask = TestBoard::mask_rect(60, 0, 10, 1);

        assert!(mask.get(60, 0));
        assert!(mask.get(63, 0));
        assert!(mask.get(64, 0));
        assert!(mask.get(69, 0));
        assert!(!mask.get(59, 0));
        assert!(!mask.get(70, 0));
        assert!(!mask.get(65, 1)); // 別の行

        // マスク操作のシミュレーション
        let mut data = TestBoard::default();
        data.set(65, 0, true);
        data.set(75, 0, true);
        let result: BitBoard<256, 256> = &data & &mask;
        assert!(result.get(65, 0)); // マスク内なので維持
        assert!(!result.get(75, 0)); // マスク外なので消える
    }

    #[test]
    fn test_mask_sector() {
        let cx = 100;
        let cy = 100;
        let radius = 10.0;

        // 全円
        let circle = TestBoard::mask_sector(cx, cy, radius, 0.0, 360.0);
        assert!(circle.get(cx, cy));
        assert!(circle.get(cx + 10, cy));
        assert!(!circle.get(cx + 11, cy));

        // 右下 90 度の扇形
        let sector = TestBoard::mask_sector(cx, cy, radius, 0.0, 90.0);
        assert!(sector.get(cx + 5, cy + 5)); // 右下
        assert!(!sector.get(cx - 5, cy + 5)); // 左下は範囲外
    }

    #[test]
    fn test_mask_visibility() {
        let mut opaque = TestBoard::default();
        // 壁を建てる (x=105, y=95..105)
        for y in 95..=105 {
            opaque.set(105, y, true);
        }

        let vis = TestBoard::mask_visibility(100, 100, 20.0, &opaque);

        assert!(vis.get(104, 100)); // 壁の直前は見えている
        assert!(vis.get(105, 100)); // 壁そのものも見えている
        assert!(
            !vis.get(106, 100),
            "Tile (106, 100) should be hidden by wall at (105, 100)"
        );
        assert!(vis.get(100, 120)); // 反対側は見えている
    }

    #[test]
    fn test_mask_visibility_diagonal_pillar() {
        let mut opaque = TestBoard::default();
        // (100,100) から見て右下方向に 2x2 の柱を立てる
        opaque.set(105, 105, true);
        opaque.set(106, 105, true);
        opaque.set(105, 106, true);
        opaque.set(106, 106, true);

        let vis = TestBoard::mask_visibility(100, 100, 20.0, &opaque);

        assert!(vis.get(104, 104), "Pillar front should be visible");
        assert!(vis.get(105, 105), "Pillar itself should be visible");
        // 柱の真後ろ (107, 107) やその延長線上のタイルは影になるべき
        assert!(
            !vis.get(108, 108),
            "Tile behind the 2x2 pillar should be hidden"
        );
        // 柱の横 (108, 105) は視界が通るべき
        assert!(
            vis.get(108, 105),
            "Tile adjacent to the shadow should be visible"
        );
    }

    #[test]
    fn test_mask_sector_concave() {
        let cx = 100;
        let cy = 100;
        let radius = 10.0;

        // 凹型セクター (右方向を中心に 270度 = 上、右、下の範囲。左側が欠ける扇形)
        // このロジックは内部で「全円から左方向の90度凸型セクターを引き算する」パスを通る
        let sector = TestBoard::mask_sector(cx, cy, radius, -135.0, 270.0);

        assert!(sector.get(cx + 5, cy), "Right should be included");
        assert!(sector.get(cx, cy - 5), "Top should be included");
        assert!(sector.get(cx, cy + 5), "Bottom should be included");
        assert!(
            !sector.get(cx - 5, cy),
            "Left should be EXCLUDED in this concave sector"
        );
    }

    #[test]
    fn test_mask_visibility_out_of_bounds() {
        let opaque = TestBoard::default();
        // マップの左上隅 (0, 0) で視界計算。負の座標にアクセスしようとしてもパニックしないか。
        let vis_tl = TestBoard::mask_visibility(0, 0, 10.0, &opaque);
        assert!(vis_tl.get(0, 0));
        assert!(vis_tl.get(5, 5));
        assert!(!vis_tl.get(-1, -1)); // 範囲外は false になること

        // マップの右下隅 (255, 255)
        let vis_br = TestBoard::mask_visibility(255, 255, 10.0, &opaque);
        assert!(vis_br.get(255, 255));
        assert!(vis_br.get(250, 250));
    }

    #[test]
    fn test_mask_rect_out_of_bounds() {
        // 部分的に外側
        let mask = TestBoard::mask_rect(-5, -5, 10, 10);
        assert!(mask.get(0, 0));
        assert!(mask.get(4, 4));
        assert!(!mask.get(5, 5));
        assert_eq!(mask.count_ones(), 25); // 5x5 visible part

        // 完全に外側
        let mask_out = TestBoard::mask_rect(300, 300, 10, 10);
        assert_eq!(mask_out.count_ones(), 0);
    }

    #[test]
    fn test_mask_sector_angle_normalization() {
        let cx = 100;
        let cy = 100;
        let radius = 10.0;

        // 360度を超えるスイープは全円になるか
        let full = TestBoard::mask_sector(cx, cy, radius, 0.0, 400.0);
        assert_eq!(
            full.count_ones(),
            TestBoard::mask_sector(cx, cy, radius, 0.0, 360.0).count_ones()
        );

        // 負の開始角度を含むケース
        let neg_start = TestBoard::mask_sector(cx, cy, radius, -20.0, 40.0);
        assert!(neg_start.get(cx + 5, cy)); // 右方向 (0度)
        assert!(neg_start.get(cx + 5, cy - 1)); // 約 -11.3 度 (範囲内)
        assert!(neg_start.get(cx + 5, cy + 1)); // 約 +11.3 度 (範囲内)
    }

    #[test]
    fn test_mask_visibility_thin_walls() {
        let mut opaque = TestBoard::default();
        // 薄い水平の壁
        for x in 90..=110 {
            opaque.set(x, 105, true);
        }

        let vis = TestBoard::mask_visibility(100, 100, 20.0, &opaque);
        assert!(vis.get(100, 104));
        assert!(vis.get(100, 105)); // 壁自体
        assert!(!vis.get(100, 106)); // 壁の向こう
    }
}
