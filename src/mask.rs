use crate::{BitBoard, BitLayout};

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// 指定した矩形範囲のみを 1 にしたマスクを作成
    /// 範囲情報の高速な抽出・制限に使用
    pub fn mask_rect(x: i32, y: i32, width: i32, height: i32) -> Self {
        let mut res = Self::new();
        L::rect_op(&mut res.data, &mut res.block_mask, x, y, width, height, true);
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
        let eps = 1e-6;
        if vy.abs() < eps {
            // 水平レイ: dy の正負とベクトルの向きで全範囲か無効範囲かが決まる
            let ok = if is_start {
                vx * dy >= -eps
            } else {
                vx * dy <= eps
            };
            if ok {
                (f32::NEG_INFINITY, f32::INFINITY)
            } else {
                (f32::INFINITY, f32::NEG_INFINITY)
            }
        } else {
            let x_limit = (vx * dy + if is_start { eps } else { -eps }) / vy;
            if vy > 0.0 {
                if is_start {
                    (f32::NEG_INFINITY, x_limit)
                } else {
                    (x_limit, f32::INFINITY)
                }
            } else {
                if is_start {
                    (x_limit, f32::INFINITY)
                } else {
                    (f32::NEG_INFINITY, x_limit)
                }
            }
        }
    }

    /// 遮蔽物（opaque_board）を考慮した視界マスクを生成
    /// 再帰的シャドウキャスティング（Recursive Shadowcasting）を用いて計算
    pub fn mask_visibility(
        &self,
        cx: i32,
        cy: i32,
        radius: f32,
        opaque_board: &BitBoard<W, H, L>,
    ) -> Self {
        let mut mask = Self::default();
        mask.set(cx, cy, true); // 立っている位置は必ず見える

        // 8つのオクタントに対して走査を行う
        // 方向ベクトル組: (xx, xy, yx, yy)
        let directions = [
            (1, 0, 0, -1),
            (0, 1, -1, 0),
            (0, 1, 1, 0),
            (-1, 0, 0, 1),
            (-1, 0, 0, -1),
            (0, -1, -1, 0),
            (0, -1, 1, 0),
            (1, 0, 0, 1),
        ];

        for (xx, xy, yx, yy) in directions {
            self.scan_octant(
                &mut mask,
                cx,
                cy,
                radius,
                1,
                1.0,
                0.0,
                xx,
                xy,
                yx,
                yy,
                opaque_board,
            );
        }

        mask
    }

    /// 再帰的シャドウキャスティングの走査コアロジック
    #[allow(clippy::too_many_arguments)]
    fn scan_octant(
        &self,
        mask: &mut BitBoard<W, H, L>,
        cx: i32,
        cy: i32,
        radius: f32,
        row: i32,
        mut start_slope: f32,
        end_slope: f32,
        xx: i32,
        xy: i32,
        yx: i32,
        yy: i32,
        opaque_board: &BitBoard<W, H, L>,
    ) {
        if start_slope < end_slope {
            return;
        }

        let radius_sq = radius * radius;

        for distance in row..=(radius.ceil() as i32) {
            let mut last_was_opaque = -1; // -1: initial, 0: trans, 1: opaque

            for i in 0..=distance {
                // 事前に計算されたベクトルによる高速な座標変換
                let dx = distance * xx + i * xy;
                let dy = distance * yx + i * yy;
                let x = cx + dx;
                let y = cy + dy;

                // マップ範囲外チェック（高速化のためここで行う）
                if x < 0 || x >= W as i32 || y < 0 || y >= H as i32 {
                    continue;
                }

                let l_slope = (i as f32 + 0.5) / (distance as f32 - 0.5);
                let r_slope = (i as f32 - 0.5) / (distance as f32 + 0.5);

                if start_slope < r_slope {
                    continue;
                }
                if end_slope > l_slope {
                    break;
                }

                // 距離チェック
                if (dx * dx + dy * dy) as f32 <= radius_sq {
                    mask.set(x, y, true);
                }

                let is_opaque = opaque_board.get(x, y);

                if last_was_opaque == 1 {
                    if !is_opaque {
                        // Transition from opaque to transparent: shrink the wedge
                        last_was_opaque = 0;
                        start_slope = l_slope;
                    }
                } else {
                    if is_opaque {
                        // Transition from transparent to opaque: recurse for the visible segment
                        if distance < radius as i32 && r_slope > end_slope {
                            self.scan_octant(
                                mask,
                                cx,
                                cy,
                                radius,
                                distance + 1,
                                start_slope,
                                r_slope,
                                xx,
                                xy,
                                yx,
                                yy,
                                opaque_board,
                            );
                        }
                        last_was_opaque = 1;
                    }
                }
            }

            // If the row ends with an opaque tile, the wedge is fully blocked for further distances
            if last_was_opaque == 1 {
                break;
            }
        }
    }
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

        let vis = TestBoard::default().mask_visibility(100, 100, 20.0, &opaque);
        
        assert!(vis.get(104, 100)); // 壁の直前は見えている
        assert!(vis.get(105, 100)); // 壁そのものも見えている
        assert!(!vis.get(106, 100), "Tile (106, 100) should be hidden by wall at (105, 100)"); 
        assert!(vis.get(100, 120)); // 反対側は見えている
    }
}
