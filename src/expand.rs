use crate::{BitBoard, BitLayout};

impl<const W: usize, const H: usize, L: BitLayout<W, H>> BitBoard<W, H, L> {
    /// Efficiently calculates areas (set of top-left coordinates) where a unit of a given size can fit.
    /// Performs (size - 1) bitwise AND operations with vertical and horizontal shifts exponentially.
    pub fn fit_rect_anchor(&self, width: u32, height: u32) -> Self {
        let mut res = self.clone();
        let mut scratch = Self::new();
        res.fit_rect_anchor_with_buffer(width, height, &mut scratch);
        res
    }

    /// In-place / allocation-free version of `fit_rect_anchor`.
    /// Overwrites `self` with the result and reuses `scratch` as a shift buffer.
    /// Eliminates allocations from `shifted_*` called O(log(max(width, height))) times.
    pub fn fit_rect_anchor_with_buffer(&mut self, width: u32, height: u32, scratch: &mut Self) {
        // Vertical reduction
        let mut current_h = 1;
        while current_h < height {
            let d = (height - current_h).min(current_h);
            self.shift_vertical_into(-(d as i32), scratch);
            *self &= &*scratch;
            current_h += d;
        }

        // Horizontal reduction
        let mut current_w = 1;
        while current_w < width {
            let d = (width - current_w).min(current_w);
            self.shift_horizontal_into(-(d as i32), scratch);
            *self &= &*scratch;
            current_w += d;
        }

        self.finalize();
    }

    /// Expands the BFS wavefront by one step.
    /// Expands the current frontier in 4 directions (up, down, left, right), applying mask and excluding visited tiles.
    pub fn flood_expand(&self, passable: &Self, visited: &mut Self) -> Self {
        let mut next = Self::default();
        self.flood_expand_into(passable, visited, &mut next);
        next
    }

    /// Allocation-reduced version of `flood_expand`.
    pub fn flood_expand_into(&self, passable: &Self, visited: &mut Self, out: &mut Self) {
        let mut temp = Self::new();
        out.clear();

        // Expand in 4 directions (up, down, left, right)
        self.shift_vertical_into(-1, &mut temp);
        *out |= &temp;
        self.shift_vertical_into(1, &mut temp);
        *out |= &temp;
        self.shift_horizontal_into(1, &mut temp);
        *out |= &temp;
        self.shift_horizontal_into(-1, &mut temp);
        *out |= &temp;

        // Apply passable mask and exclude visited tiles, while building block_mask in the same pass.
        // !visited is used as a mask, but since it can be dense, block_mask skip is ineffective.
        // Instead, we eliminate the double pass of rebuild_block_mask.
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

        // 1-step expansion: 4 adjacent tiles
        let next = frontier.flood_expand(&passable, &mut visited);
        assert!(next.get(1, 2)); // West
        assert!(next.get(3, 2)); // East
        assert!(next.get(2, 1)); // North
        assert!(next.get(2, 3)); // South
        assert!(!next.get(2, 2)); // Already visited
        assert_eq!(next.count_ones(), 4);
    }

    #[test]
    fn test_flood_expand_respects_walls() {
        type Bb = BitBoard<8, 8>;
        let mut passable = Bb::default();
        // L-shaped passage
        passable.set(0, 0, true);
        passable.set(1, 0, true);
        passable.set(1, 1, true);

        let mut frontier = Bb::default();
        frontier.set(0, 0, true);
        let mut visited = frontier.clone();

        let next = frontier.flood_expand(&passable, &mut visited);
        assert!(next.get(1, 0)); // Expanded towards passage
        assert!(!next.get(0, 1)); // Wall, so not expanded
        assert_eq!(next.count_ones(), 1);
    }

    #[test]
    fn test_flood_expand_no_padding_leak() {
        // Ensure flood_expand does not leak padding bits on boards where W is not a multiple of 64
        type Bb = BitBoard<100, 10>;
        let mut passable = Bb::default();
        for y in 0..10 {
            for x in 0..100 {
                passable.set(x, y, true);
            }
        }

        let mut frontier = Bb::default();
        frontier.set(99, 5, true); // Right edge
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

        // Positions where top-left of a 2x2 unit can be placed
        assert!(result.get(0, 0));
        assert!(result.get(1, 1));
        assert!(result.get(2, 2)); // Bottom-right is (3,3), within passable range
        assert!(!result.get(3, 3)); // Bottom-right is (4,4), not passable
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
        } // All tiles passable

        let mut frontier = Bb::default();
        frontier.set(0, 0, true); // Top-left corner
        let mut visited = frontier.clone();

        let next = frontier.flood_expand(&passable, &mut visited);
        // Not expanded up (-y) or left (-x); only expanded right and down
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

        // Requirements larger than the board size should return an empty mask
        let result = passable.fit_rect_anchor(10, 10);
        assert!(result.is_empty());
    }
}
