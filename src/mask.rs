use crate::BitBoard;

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 指定した矩形範囲のみを 1 にしたマスクを作成
    /// 範囲情報の高速な抽出・制限に使用
    pub fn rectangle_mask(x: i32, y: i32, width: i32, height: i32) -> Self {
        let mut mask = Self::default();

        let x1 = x.max(0) as usize;
        let y1 = y.max(0) as usize;
        let x2 = (x + width).max(0).min(W as i32) as usize;
        let y2 = (y + height).max(0).min(H as i32) as usize;

        if x1 >= x2 || y1 >= y2 {
            return mask;
        }

        let start_word = x1 / 64;
        let end_word = (x2 - 1) / 64;
        let start_mask = !0u64 << (x1 % 64);
        let end_mask = !0u64 >> (63 - ((x2 - 1) % 64));

        for row in y1..y2 {
            let row_offset = row * Self::ROW_U64S;
            if start_word == end_word {
                mask.data[row_offset + start_word] = start_mask & end_mask;
            } else {
                mask.data[row_offset + start_word] = start_mask;
                for w in (start_word + 1)..end_word {
                    mask.data[row_offset + w] = !0u64;
                }
                mask.data[row_offset + end_word] = end_mask;
            }
        }
        mask
    }

    /// 扇形（扇状）のビットマスクを生成
    /// 中心座標、半径、開始角度、掃引角度を指定。360度以上で円形。
    pub fn sector_mask(
        cx: i32,
        cy: i32,
        radius: f32,
        start_angle_deg: f32,
        sweep_angle_deg: f32,
    ) -> Self {
        let mut mask = Self::default();
        if radius <= 0.0 {
            return mask;
        }

        let r_i = radius.ceil() as i32;
        let y_min = (cy - r_i).max(0);
        let y_max = (cy + r_i).min(H as i32 - 1);

        let is_circle = sweep_angle_deg >= 360.0;
        let start_rad = start_angle_deg.to_radians();
        let sweep_rad = sweep_angle_deg.to_radians();

        for y in y_min..=y_max {
            let dy = y as f32 - cy as f32;
            let dx_limit_sq = radius * radius - dy * dy;
            if dx_limit_sq < 0.0 {
                continue;
            }
            let dx_limit = dx_limit_sq.sqrt();

            let x_start = (cx as f32 - dx_limit).ceil() as i32;
            let x_end = (cx as f32 + dx_limit).floor() as i32;
            let x_min = x_start.max(0);
            let x_max = x_end.min(W as i32 - 1);

            if x_min > x_max {
                continue;
            }

            if is_circle {
                // 円形の場合は行範囲を一括設定
                mask.set_row_range(y as usize, x_min as usize, (x_max + 1) as usize);
            } else {
                let start_vec_x = start_rad.cos();
                let start_vec_y = start_rad.sin();
                let end_rad = start_rad + sweep_rad;
                let end_vec_x = end_rad.cos();
                let end_vec_y = end_rad.sin();
                let is_convex = sweep_rad <= std::f32::consts::PI;

                for x in x_min..=x_max {
                    let dx = x as f32 - cx as f32;

                    // 外積を用いた角度範囲の判定
                    let cross_start = start_vec_x * dy - start_vec_y * dx;
                    let cross_end = end_vec_x * dy - end_vec_y * dx;

                    let in_sector = if is_convex {
                        cross_start >= -1e-6 && cross_end <= 1e-6
                    } else {
                        cross_start >= -1e-6 || cross_end <= 1e-6
                    };

                    if in_sector {
                        mask.set(x, y, true);
                    }
                }
            }
        }
        mask
    }

    /// 行内の指定範囲 [x1, x2) のビットを立てる (内部用)
    pub(crate) fn set_row_range(&mut self, row: usize, x1: usize, x2: usize) {
        if x1 >= x2 || x1 >= W || row >= H {
            return;
        }
        let x2 = x2.min(W);

        let start_word = x1 / 64;
        let end_word = (x2 - 1) / 64;
        let start_mask = !0u64 << (x1 % 64);
        let end_mask = !0u64 >> (63 - ((x2 - 1) % 64));

        let row_offset = row * Self::ROW_U64S;
        if start_word == end_word {
            self.data[row_offset + start_word] |= start_mask & end_mask;
        } else {
            self.data[row_offset + start_word] |= start_mask;
            for w in (start_word + 1)..end_word {
                self.data[row_offset + w] = !0u64;
            }
            self.data[row_offset + end_word] |= end_mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_rectangle_mask() {
        // 64タイルを跨ぐ矩形 (x=60, w=10)
        // word 0 の bits 60-63 と word 1 の bits 0-5 が 1 になるはず
        let mask = TestBoard::rectangle_mask(60, 0, 10, 1);

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
    fn test_sector_mask() {
        let cx = 100;
        let cy = 100;
        let radius = 10.0;

        // 全円
        let circle = TestBoard::sector_mask(cx, cy, radius, 0.0, 360.0);
        assert!(circle.get(cx, cy));
        assert!(circle.get(cx + 10, cy));
        assert!(!circle.get(cx + 11, cy));

        // 右下 90 度の扇形
        let sector = TestBoard::sector_mask(cx, cy, radius, 0.0, 90.0);
        assert!(sector.get(cx + 5, cy + 5)); // 右下
        assert!(!sector.get(cx - 5, cy + 5)); // 左下は範囲外
    }
}
