use crate::{BitBoard, BitLayout};

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    fn apply_morphology_with_buffer<S, Op>(
        &mut self,
        steps: u32,
        buffer: &mut Self,
        shift_into: S,
        mut op: Op,
    ) where
        S: Fn(&Self, i32, &mut Self),
        Op: FnMut(&mut Self, &Self),
    {
        let mut current_range = 0;
        while current_range < steps {
            let d = (steps - current_range).min(current_range + 1);

            // Positive direction
            shift_into(self, d as i32, buffer);
            op(self, buffer);

            // Negative direction
            shift_into(self, -(d as i32), buffer);
            op(self, buffer);

            current_range += d;
        }
    }

    /// Dilates set bits in all 8 directions (allocation-avoidance version).
    pub fn dilate_with_buffer(&mut self, steps: u32, buffer: &mut Self) {
        if steps == 0 {
            return;
        }

        self.apply_morphology_with_buffer(
            steps,
            buffer,
            |b, d, dst| b.shift_horizontal_into(d, dst),
            |r, s| *r |= s,
        );

        self.apply_morphology_with_buffer(
            steps,
            buffer,
            |b, d, dst| b.shift_vertical_into(d, dst),
            |r, s| *r |= s,
        );

        self.finalize();
    }

    /// Dilates set bits in all 8 directions.
    pub fn dilate(&self, steps: u32) -> Self {
        if steps == 0 {
            return self.clone();
        }
        let mut res = self.clone();
        let mut buffer = Self::new();
        res.dilate_with_buffer(steps, &mut buffer);
        res
    }

    /// Erodes set bits in all 8 directions (allocation-avoidance version).
    pub fn erode_with_buffer(&mut self, steps: u32, buffer: &mut Self) {
        if steps == 0 {
            return;
        }

        self.apply_morphology_with_buffer(
            steps,
            buffer,
            |b, d, dst| b.shift_horizontal_into(d, dst),
            |r, s| *r &= s,
        );

        self.apply_morphology_with_buffer(
            steps,
            buffer,
            |b, d, dst| b.shift_vertical_into(d, dst),
            |r, s| *r &= s,
        );

        self.finalize();
    }

    /// Erodes set bits in all 8 directions.
    pub fn erode(&self, steps: u32) -> Self {
        if steps == 0 {
            return self.clone();
        }
        let mut res = self.clone();
        let mut buffer = Self::new();
        res.erode_with_buffer(steps, &mut buffer);
        res
    }
}

#[cfg(test)]
mod tests {
    use crate::BitBoard;

    type TestBoard = BitBoard<256, 256>;

    #[test]
    fn test_morphology_dilate() {
        let mut bb = TestBoard::default();
        bb.set(100, 100, true);

        // 1-step dilation
        let d1 = bb.dilate(1);
        assert_eq!(d1.count_ones(), 9); // 3x3
        assert!(d1.get(99, 99));
        assert!(d1.get(101, 101));
        assert!(!d1.get(98, 100));

        // 2-step dilation
        let d2 = bb.dilate(2);
        assert_eq!(d2.count_ones(), 25); // 5x5
        assert!(d2.get(98, 98));
        assert!(d2.get(102, 102));
    }

    #[test]
    fn test_morphology_erode() {
        let mut bb = TestBoard::default();
        // Create a 3x3 block
        for x in 99..=101 {
            for y in 99..=101 {
                bb.set(x, y, true);
            }
        }
        assert_eq!(bb.count_ones(), 9);

        // 1-step erosion
        let e1 = bb.erode(1);
        assert_eq!(e1.count_ones(), 1); // Only the center remains
        assert!(e1.get(100, 100));
        assert!(!e1.get(99, 99));

        // 2-step erosion
        let e2 = bb.erode(2);
        assert_eq!(e2.count_ones(), 0); // Everything disappears
    }

    #[test]
    fn test_shifted_horizontal_edge_cases() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);

        let sh_l = bb.shifted_horizontal(-1); // East to West (x-)
        assert!(!sh_l.get(0, 0));
        assert_eq!(sh_l.count_ones(), 0); // Out of screen

        let sh_r = bb.shifted_horizontal(255); // West to East (x+)
        assert!(sh_r.get(255, 0));

        let sh_r_out = bb.shifted_horizontal(256); // Out of screen
        assert_eq!(sh_r_out.count_ones(), 0);
    }

    #[test]
    fn test_morphology_at_boundaries() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);

        let d1 = bb.dilate(1);
        assert_eq!(d1.count_ones(), 4); // (0,0), (1,0), (0,1), (1,1)
        assert!(d1.get(1, 1));
        assert!(!d1.get(2, 2));

        bb.clear();
        bb.set(255, 255, true);
        let d2 = bb.dilate(1);
        assert_eq!(d2.count_ones(), 4);
        assert!(d2.get(254, 254));
    }

    #[test]
    fn test_dilate_erode_cycle() {
        let mut bb = TestBoard::default();
        // 5x5 rect
        for x in 100..105 {
            for y in 100..105 {
                bb.set(x, y, true);
            }
        }

        // 2-step dilation followed by 2-step erosion
        let cycle = bb.dilate(2).erode(2);

        // Should return to the original shape (for simple rectangles)
        assert_eq!(cycle.count_ones(), 25);
        assert!(cycle.get(100, 100));
        assert!(cycle.get(104, 104));
        assert!(!cycle.get(99, 99));
    }

    #[test]
    fn test_dilate_zero() {
        let mut bb = TestBoard::default();
        bb.set(10, 10, true);
        let d0 = bb.dilate(0);
        assert_eq!(d0.count_ones(), 1);
        assert!(d0.get(10, 10));
    }

    #[test]
    fn test_erode_empty() {
        let bb = TestBoard::default();
        let e1 = bb.erode(1);
        assert!(e1.is_empty());
    }

    // --- Edge Case Tests ---

    #[test]
    fn test_shifted_horizontal_zero_is_identity() {
        let mut bb = TestBoard::default();
        bb.set(10, 5, true);
        bb.set(70, 100, true);
        let sh = bb.shifted_horizontal(0);
        assert_eq!(sh, bb, "Shift by 0 is an identity transformation");
    }

    #[test]
    fn test_shifted_vertical_zero_is_identity() {
        let mut bb = TestBoard::default();
        bb.set(10, 5, true);
        bb.set(70, 100, true);
        let sv = bb.shifted_vertical(0);
        assert_eq!(sv, bb, "Shift by 0 is an identity transformation");
    }

    #[test]
    fn test_shifted_extreme_values_do_not_panic() {
        let mut bb = TestBoard::default();
        bb.set(100, 100, true);

        // Huge positive shift -> clear all
        let sh_max = bb.shifted_horizontal(i32::MAX);
        assert!(sh_max.is_empty());
        let sh_min = bb.shifted_horizontal(i32::MIN + 1);
        assert!(sh_min.is_empty());

        // Huge negative shift -> clear all
        let sv_big = bb.shifted_vertical(10_000);
        assert!(sv_big.is_empty());
    }

    #[test]
    fn test_dilate_huge_steps_fills_board() {
        let mut bb = TestBoard::default();
        bb.set(128, 128, true);
        // Board should be completely filled after 256 steps of dilation
        let d = bb.dilate(256);
        assert_eq!(d.count_ones(), 256 * 256);
    }

    #[test]
    fn test_erode_huge_steps_clears_board() {
        let mut bb = TestBoard::default();
        // 5x5 block
        for y in 100..105 {
            for x in 100..105 {
                bb.set(x, y, true);
            }
        }
        // Clear all if erosion steps exceed block size
        let e = bb.erode(10);
        assert!(e.is_empty());
    }
}
