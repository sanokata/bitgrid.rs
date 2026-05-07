use crate::{BitBoard, BitLayout};

/// Epsilon for considering a ray direction vector as "horizontal".
/// If |vy| is less than this value, it branches to either full or invalid range.
const RAY_DIRECTION_EPSILON: f32 = 1e-6;

/// Offset for cell boundary slopes in shadow casting.
/// Boundary inclination when cell center is 0 and edges are ±0.5.
const CELL_SLOPE_OFFSET: f32 = 0.5;

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// Creates a mask where only the specified rectangular range is set to 1.
    /// Used for fast extraction and limitation of range information.
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
                // Concave: Fill the entire circle, then erase the "gap" (the convex sector on the opposite side)
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

    /// Calculates the x-range of a convex region sandwiched between two rays (start/end).
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

    /// Calculates the boundary range of x for a specific ray (direction vector vx, vy).
    fn get_ray_x_limit(dy: f32, vx: f32, vy: f32, is_start: bool) -> (f32, f32) {
        if vy.abs() < RAY_DIRECTION_EPSILON {
            // Horizontal ray: Entire or invalid range determined by the sign of dy and vector direction
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

    /// Generates a visibility mask considering obstructions (opaque_board) (allocates a new board).
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

    /// Generates a visibility mask considering obstructions (opaque_board) in an existing board (allocation-free).
    pub fn mask_visibility_into(
        &mut self,
        cx: i32,
        cy: i32,
        radius: f32,
        opaque_board: &BitBoard<W, H, L>,
    ) {
        self.clear();
        self.set(cx, cy, true); // The tile at the origin is always visible

        // Basis vectors for 8 octants (xx, xy, yx, yy)
        const OCTANTS: [Octant; 8] = [
            Octant {
                xx: 1,
                xy: 0,
                yx: 0,
                yy: -1,
            },
            Octant {
                xx: 0,
                xy: 1,
                yx: -1,
                yy: 0,
            },
            Octant {
                xx: 0,
                xy: 1,
                yx: 1,
                yy: 0,
            },
            Octant {
                xx: -1,
                xy: 0,
                yx: 0,
                yy: 1,
            },
            Octant {
                xx: -1,
                xy: 0,
                yx: 0,
                yy: -1,
            },
            Octant {
                xx: 0,
                xy: -1,
                yx: -1,
                yy: 0,
            },
            Octant {
                xx: 0,
                xy: -1,
                yx: 1,
                yy: 0,
            },
            Octant {
                xx: 1,
                xy: 0,
                yx: 0,
                yy: 1,
            },
        ];

        for octant in OCTANTS {
            self.scan_octant(cx, cy, radius, 1, 1.0, 0.0, octant, opaque_board);
        }

        // Update block mask (since set() was called during scanning)
        self.rebuild_block_mask();
    }

    /// Core scanning logic for recursive shadowcasting.
    /// Abstracts 8 basis vectors via `octant` to consolidate arguments.
    ///
    /// Recursive Shadowcasting algorithm (Berg/Mejaski method):
    /// - Scan rows at `distance` one column at a time; determine visibility by comparing
    ///   left/right cell edge slopes with the current wedge (start_slope / end_slope).
    /// - Narrow the wedge upon hitting an opaque cell and recurse for each visible segment.
    #[allow(clippy::too_many_arguments)]
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
            // Cell state: None=Initial / Some(true)=Opaque / Some(false)=Transparent
            let mut last_was_opaque: Option<bool> = None;

            for i in (0..=distance).rev() {
                let dx = distance * octant.xx + i * octant.xy;
                let dy = distance * octant.yx + i * octant.yy;
                let x = cx + dx;
                let y = cy + dy;

                // Skip scanning if out of map bounds (early check for optimization)
                if x < 0 || x >= W as i32 || y < 0 || y >= H as i32 {
                    continue;
                }

                // Inclination of left/right cell edges (start_slope/end_slope are wedge boundaries).
                // Calculate slopes of left/right edges relative to cell center ± CELL_SLOPE_OFFSET.
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
                        // Opaque -> Transparent: Update left wedge boundary and continue
                        start_slope = l_slope;
                        last_was_opaque = Some(false);
                    }
                    Some(false) if is_opaque && distance < radius as i32 && l_slope > end_slope => {
                        // Transparent -> Opaque: Recurse for the visible segment and mark as opaque
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

            // If the end of the row is opaque, the entire wedge beyond this distance is blocked
            if last_was_opaque == Some(true) {
                break;
            }
        }
    }
}

/// Basis transformation representing the 8 octants for shadowcasting.
/// Expresses coordinate transformation per octant in the form:
/// (dx, dy) = (distance * xx + i * xy, distance * yx + i * yy)
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
        // Rect spanning 64 tiles (x=60, w=10)
        // word 0 bits 60-63 and word 1 bits 0-5 should be 1
        let mask = TestBoard::mask_rect(60, 0, 10, 1);

        assert!(mask.get(60, 0));
        assert!(mask.get(63, 0));
        assert!(mask.get(64, 0));
        assert!(mask.get(69, 0));
        assert!(!mask.get(59, 0));
        assert!(!mask.get(70, 0));
        assert!(!mask.get(65, 1)); // Another row

        // Mask operation simulation
        let mut data = TestBoard::default();
        data.set(65, 0, true);
        data.set(75, 0, true);
        let result: BitBoard<256, 256> = &data & &mask;
        assert!(result.get(65, 0)); // Maintained as it is within the mask
        assert!(!result.get(75, 0)); // Erased as it is outside the mask
    }

    #[test]
    fn test_mask_sector() {
        let cx = 100;
        let cy = 100;
        let radius = 10.0;

        // Full circle
        let circle = TestBoard::mask_sector(cx, cy, radius, 0.0, 360.0);
        assert!(circle.get(cx, cy));
        assert!(circle.get(cx + 10, cy));
        assert!(!circle.get(cx + 11, cy));

        // 90-degree sector (lower-right)
        let sector = TestBoard::mask_sector(cx, cy, radius, 0.0, 90.0);
        assert!(sector.get(cx + 5, cy + 5)); // Lower-right
        assert!(!sector.get(cx - 5, cy + 5)); // Lower-left is out of range
    }

    #[test]
    fn test_mask_visibility() {
        let mut opaque = TestBoard::default();
        // Build a wall (x=105, y=95..105)
        for y in 95..=105 {
            opaque.set(105, y, true);
        }

        let vis = TestBoard::mask_visibility(100, 100, 20.0, &opaque);

        assert!(vis.get(104, 100)); // Visible just before the wall
        assert!(vis.get(105, 100)); // Wall itself is also visible
        assert!(
            !vis.get(106, 100),
            "Tile (106, 100) should be hidden by wall at (105, 100)"
        );
        assert!(vis.get(100, 120)); // Opposite side is visible
    }

    #[test]
    fn test_mask_visibility_diagonal_pillar() {
        let mut opaque = TestBoard::default();
        // Build a 2x2 pillar lower-right relative to (100,100)
        opaque.set(105, 105, true);
        opaque.set(106, 105, true);
        opaque.set(105, 106, true);
        opaque.set(106, 106, true);

        let vis = TestBoard::mask_visibility(100, 100, 20.0, &opaque);

        assert!(vis.get(104, 104), "Pillar front should be visible");
        assert!(vis.get(105, 105), "Pillar itself should be visible");
        // Tiles directly behind the pillar (107, 107) and its extension should be in shadow
        assert!(
            !vis.get(108, 108),
            "Tile behind the 2x2 pillar should be hidden"
        );
        // Tiles adjacent to the shadow (108, 105) should be visible
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

        // Concave sector (270 degrees centered to the right = top, right, bottom. Left side is missing)
        // This logic internally passes "subtract a 90-degree convex sector pointing left from the full circle"
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
        // Scan visibility at the top-left corner (0, 0) of the map. Ensure it doesn't panic when accessing negative coordinates.
        let vis_tl = TestBoard::mask_visibility(0, 0, 10.0, &opaque);
        assert!(vis_tl.get(0, 0));
        assert!(vis_tl.get(5, 5));
        assert!(!vis_tl.get(-1, -1)); // Out-of-bounds should be false

        // Bottom-right corner (255, 255)
        let vis_br = TestBoard::mask_visibility(255, 255, 10.0, &opaque);
        assert!(vis_br.get(255, 255));
        assert!(vis_br.get(250, 250));
    }

    #[test]
    fn test_mask_rect_out_of_bounds() {
        // Partially outside
        let mask = TestBoard::mask_rect(-5, -5, 10, 10);
        assert!(mask.get(0, 0));
        assert!(mask.get(4, 4));
        assert!(!mask.get(5, 5));
        assert_eq!(mask.count_ones(), 25); // 5x5 visible part

        // Completely outside
        let mask_out = TestBoard::mask_rect(300, 300, 10, 10);
        assert_eq!(mask_out.count_ones(), 0);
    }

    #[test]
    fn test_mask_sector_angle_normalization() {
        let cx = 100;
        let cy = 100;
        let radius = 10.0;

        // Sweep exceeding 360 degrees should result in a full circle
        let full = TestBoard::mask_sector(cx, cy, radius, 0.0, 400.0);
        assert_eq!(
            full.count_ones(),
            TestBoard::mask_sector(cx, cy, radius, 0.0, 360.0).count_ones()
        );

        // Case including a negative starting angle
        let neg_start = TestBoard::mask_sector(cx, cy, radius, -20.0, 40.0);
        assert!(neg_start.get(cx + 5, cy)); // Right direction (0 degrees)
        assert!(neg_start.get(cx + 5, cy - 1)); // Approx -11.3 degrees (within range)
        assert!(neg_start.get(cx + 5, cy + 1)); // Approx +11.3 degrees (within range)
    }

    #[test]
    fn test_mask_visibility_thin_walls() {
        let mut opaque = TestBoard::default();
        // Thin horizontal wall
        for x in 90..=110 {
            opaque.set(x, 105, true);
        }

        let vis = TestBoard::mask_visibility(100, 100, 20.0, &opaque);
        assert!(vis.get(100, 104));
        assert!(vis.get(100, 105)); // Wall itself
        assert!(!vis.get(100, 106)); // Beyond the wall
    }

    // --- Edge Case Tests ---

    #[test]
    fn test_mask_visibility_zero_radius() {
        let opaque = TestBoard::default();
        let vis = TestBoard::mask_visibility(100, 100, 0.0, &opaque);
        // Even with radius=0, its own tile is visible
        assert!(vis.get(100, 100));
        assert!(!vis.get(101, 100));
        assert!(!vis.get(100, 101));
    }

    #[test]
    fn test_mask_visibility_completely_enclosed() {
        let mut opaque = TestBoard::default();
        // Enclose all 8 surrounding directions with walls (center is completely sealed)
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx != 0 || dy != 0 {
                    opaque.set(100 + dx, 100 + dy, true);
                }
            }
        }
        let vis = TestBoard::mask_visibility(100, 100, 20.0, &opaque);
        // Self and adjacent 8 tiles (including walls) are visible
        assert!(vis.get(100, 100));
        assert!(vis.get(101, 101));
        // Tiles 2 units away are blocked by walls and invisible
        assert!(!vis.get(102, 102));
        assert!(!vis.get(102, 100));
        assert!(!vis.get(100, 102));
    }

    #[test]
    fn test_mask_sector_zero_sweep_does_not_panic() {
        // sweep=0 is a degenerate form. Exact semantics are implementation-defined,
        // but guarantee it doesn't panic and returns a finite number of bits.
        let m = TestBoard::mask_sector(100, 100, 10.0, 0.0, 0.0);
        // Should not exceed upper bound for circle (21*21 = 441)
        assert!(m.count_ones() <= 21 * 21);
    }

    #[test]
    fn test_mask_sector_full_circle_via_720() {
        let r = 10.0;
        let circle_360 = TestBoard::mask_sector(100, 100, r, 0.0, 360.0);
        let circle_720 = TestBoard::mask_sector(100, 100, r, 0.0, 720.0);
        assert_eq!(
            circle_360.count_ones(),
            circle_720.count_ones(),
            "720-degree sweep should result in the same full circle as 360 degrees"
        );
    }

    #[test]
    fn test_mask_sector_zero_radius_is_empty() {
        let m = TestBoard::mask_sector(100, 100, 0.0, 0.0, 360.0);
        assert_eq!(m.count_ones(), 0, "radius=0 results in an empty mask");
    }

    #[test]
    fn test_mask_visibility_center_in_wall() {
        let mut opaque = TestBoard::default();
        opaque.set(100, 100, true); // Center is a wall
        let vis = TestBoard::mask_visibility(100, 100, 5.0, &opaque);
        // Center (player position) is always visible
        assert!(vis.get(100, 100));
        // Surroundings are visible as usual
        assert!(vis.get(102, 100));
    }

    #[test]
    fn test_mask_rect_zero_dimensions() {
        // Rect with width 0 or height 0 should be an empty mask
        let m_zero_w = TestBoard::mask_rect(10, 10, 0, 5);
        assert_eq!(m_zero_w.count_ones(), 0);

        let m_zero_h = TestBoard::mask_rect(10, 10, 5, 0);
        assert_eq!(m_zero_h.count_ones(), 0);
    }
}
