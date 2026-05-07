use super::BitLayout;

/// Morton Order (Z-order curve) layout
#[derive(Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MortonLayout;

// --- Masks for bit-spreading (interleaving) ---
// `interleave` expands a 16-bit value to 32 bits, inserting a 0 between each bit.
// Bits are spread out in stages, doubling the distance at each step.
// Masks ensure only the target bits are kept at each stage:
// - INTERLEAVE_MASK_8 : 0x00FF_00FF - allows blocks of 8 bits
// - INTERLEAVE_MASK_4 : 0x0F0F_0F0F - allows blocks of 4 bits
// - INTERLEAVE_MASK_2 : 0x3333_3333 - allows blocks of 2 bits
// - INTERLEAVE_MASK_1 : 0x5555_5555 - allows 1 bit every 2 positions (even positions)
const INTERLEAVE_MASK_8: u32 = 0x00FF_00FF;
const INTERLEAVE_MASK_4: u32 = 0x0F0F_0F0F;
const INTERLEAVE_MASK_2: u32 = 0x3333_3333;
const INTERLEAVE_MASK_1: u32 = 0x5555_5555;
/// Mask to keep only the lower 16 bits, used in the final stage of `deinterleave`.
const DEINTERLEAVE_FINAL_MASK: u32 = 0x0000_FFFF;

impl MortonLayout {
    /// Expands a 16-bit value to 32 bits, inserting a 0 bit between each bit.
    /// Classic bit-spreading implementation that doubles the spacing at each step.
    const fn interleave(mut x: u32) -> u32 {
        x = (x | (x << 8)) & INTERLEAVE_MASK_8;
        x = (x | (x << 4)) & INTERLEAVE_MASK_4;
        x = (x | (x << 2)) & INTERLEAVE_MASK_2;
        x = (x | (x << 1)) & INTERLEAVE_MASK_1;
        x
    }

    /// Inverse operation of `interleave`. Removes the inserted zeros and compresses 32 bits back to 16.
    const fn deinterleave(mut x: u32) -> u32 {
        x &= INTERLEAVE_MASK_1;
        x = (x | (x >> 1)) & INTERLEAVE_MASK_2;
        x = (x | (x >> 2)) & INTERLEAVE_MASK_4;
        x = (x | (x >> 4)) & INTERLEAVE_MASK_8;
        x = (x | (x >> 8)) & DEINTERLEAVE_FINAL_MASK;
        x
    }

    /// Converts (x, y) to Morton code: places x bits at even positions and y bits at odd positions.
    pub const fn encode(x: u32, y: u32) -> usize {
        // Shift y by 1 bit to place it in odd positions, then OR with x (even positions)
        (Self::interleave(x) as usize) | ((Self::interleave(y) as usize) << 1)
    }

    pub const fn decode(morton: usize) -> (u32, u32) {
        (
            Self::deinterleave(morton as u32),
            Self::deinterleave((morton >> 1) as u32),
        )
    }

    const RANGE_X_MASK: [[u64; 8]; 8] = Self::compute_range_masks(false);
    const RANGE_Y_MASK: [[u64; 8]; 8] = Self::compute_range_masks(true);

    const fn compute_range_masks(is_y: bool) -> [[u64; 8]; 8] {
        let mut table = [[0u64; 8]; 8];
        let mut min = 0;
        while min < 8 {
            let mut max = min;
            while max < 8 {
                let mut mask = 0u64;
                let mut i = min;
                while i <= max {
                    let mut j = 0;
                    while j < 8 {
                        let (x, y) = if is_y { (j, i) } else { (i, j) };
                        let y_bits = (Self::interleave(y) as u64) << 1;
                        mask |= 1u64 << (y_bits | (Self::interleave(x) as u64));
                        j += 1;
                    }
                    i += 1;
                }
                table[min as usize][max as usize] = mask;
                max += 1;
            }
            min += 1;
        }
        table
    }

    const fn mask_8x8rect(min_x: u32, max_x: u32, min_y: u32, max_y: u32) -> u64 {
        Self::RANGE_X_MASK[min_x as usize][max_x as usize]
            & Self::RANGE_Y_MASK[min_y as usize][max_y as usize]
    }

    fn apply_shift<const W: usize, const H: usize, F>(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        shift_op: F,
    ) where
        F: Fn(i32, i32) -> Option<(u32, u32)>,
    {
        dst.fill(0);
        dst_block.fill(0);
        for (block_idx, &block_val) in block.iter().enumerate() {
            let mut block_word = block_val;
            while block_word != 0 {
                let bit = block_word.trailing_zeros();
                block_word &= block_word - 1;
                let word_idx = block_idx * 64 + bit as usize;
                let mut data_word = src[word_idx];
                while data_word != 0 {
                    let dbit = data_word.trailing_zeros();
                    data_word &= data_word - 1;
                    let (x, y) = <Self as BitLayout<W, H>>::word_bit_to_coord(word_idx, dbit);
                    if let Some((nx, ny)) = shift_op(x, y) {
                        let morton = Self::encode(nx, ny);
                        dst[morton / 64] |= 1u64 << (morton % 64);
                        dst_block[morton / 64 / 64] |= 1u64 << ((morton / 64) % 64);
                    }
                }
            }
        }
    }
}

impl<const W: usize, const H: usize> BitLayout<W, H> for MortonLayout {
    fn total_words() -> usize {
        let max_dim = W.max(H).next_power_of_two();
        (max_dim * max_dim).div_ceil(64)
    }

    fn coord_to_word_bit(x: i32, y: i32) -> Option<(usize, u32)> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return None;
        }
        let morton = Self::encode(x as u32, y as u32);
        Some((morton / 64, (morton % 64) as u32))
    }

    fn word_bit_to_coord(word: usize, bit: u32) -> (i32, i32) {
        let morton = word * 64 + bit as usize;
        let (x, y) = Self::decode(morton);
        (x as i32, y as i32)
    }

    fn flat_index_to_coord(idx: usize) -> (i32, i32) {
        let (x, y) = Self::decode(idx);
        (x as i32, y as i32)
    }

    fn coord_to_flat_index(x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            None
        } else {
            Some(Self::encode(x as u32, y as u32))
        }
    }

    fn has_padding() -> bool {
        false
    }

    fn padding_mask() -> u64 {
        !0u64
    }

    fn shift_horizontal(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    ) {
        Self::apply_shift::<W, H, _>(src, block, dst, dst_block, |x, y| {
            let nx = x + dist;
            if nx >= 0 && nx < W as i32 {
                Some((nx as u32, y as u32))
            } else {
                None
            }
        });
    }

    fn shift_vertical(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    ) {
        Self::apply_shift::<W, H, _>(src, block, dst, dst_block, |x, y| {
            let ny = y + dist;
            if ny >= 0 && ny < H as i32 {
                Some((x as u32, ny as u32))
            } else {
                None
            }
        });
    }

    fn rect_op(
        data: &mut [u64],
        block: &mut [u64],
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        value: bool,
    ) {
        let x1 = x.max(0) as u32;
        let y1 = y.max(0) as u32;
        let x2 = (x + width).max(0).min(W as i32) as u32;
        let y2 = (y + height).max(0).min(H as i32) as u32;

        if x1 >= x2 || y1 >= y2 {
            return;
        }

        let min_blk_x = x1 / 8;
        let max_blk_x = (x2 - 1) / 8;
        let min_blk_y = y1 / 8;
        let max_blk_y = (y2 - 1) / 8;

        for by in min_blk_y..=max_blk_y {
            let local_min_y = if by == min_blk_y { y1 % 8 } else { 0 };
            let local_max_y = if by == max_blk_y { (y2 - 1) % 8 } else { 7 };
            let y_bits = Self::encode(0, by * 8); // Calculate bit contribution of y

            for bx in min_blk_x..=max_blk_x {
                let local_min_x = if bx == min_blk_x { x1 % 8 } else { 0 };
                let local_max_x = if bx == max_blk_x { (x2 - 1) % 8 } else { 7 };

                let word_idx = (Self::encode(bx * 8, 0) | y_bits) / 64;
                let mask = Self::mask_8x8rect(local_min_x, local_max_x, local_min_y, local_max_y);

                if value {
                    data[word_idx] |= mask;
                    block[word_idx / 64] |= 1u64 << (word_idx % 64);
                } else {
                    data[word_idx] &= !mask;
                    if data[word_idx] == 0 {
                        block[word_idx / 64] &= !(1u64 << (word_idx % 64));
                    }
                }
            }
        }
    }

    fn set_row(data: &mut [u64], block: &mut [u64], y: i32, min_x: i32, max_x: i32, value: bool) {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return;
        }
        let min_x = min_x.max(0) as u32;
        let max_x = max_x.min((W as i32) - 1) as u32;
        let uy = y as u32;

        let min_blk_x = min_x / 8;
        let max_blk_x = max_x / 8;
        let by = uy / 8;
        let local_y = uy % 8;

        for bx in min_blk_x..=max_blk_x {
            let local_min_x = if bx == min_blk_x { min_x % 8 } else { 0 };
            let local_max_x = if bx == max_blk_x { max_x % 8 } else { 7 };

            let word_idx = Self::encode(bx * 8, by * 8) / 64;
            let mask = Self::mask_8x8rect(local_min_x, local_max_x, local_y, local_y);

            if value {
                data[word_idx] |= mask;
                block[word_idx / 64] |= 1u64 << (word_idx % 64);
            } else {
                data[word_idx] &= !mask;
                if data[word_idx] == 0 {
                    block[word_idx / 64] &= !(1u64 << (word_idx % 64));
                }
            }
        }
    }

    fn has_any_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return false;
        }
        let min_x = min_x.max(0) as u32;
        let max_x = max_x.min((W as i32) - 1) as u32;
        let uy = y as u32;

        let min_blk_x = min_x / 8;
        let max_blk_x = max_x / 8;
        let by = uy / 8;
        let local_y = uy % 8;

        for bx in min_blk_x..=max_blk_x {
            let local_min_x = if bx == min_blk_x { min_x % 8 } else { 0 };
            let local_max_x = if bx == max_blk_x { max_x % 8 } else { 7 };

            let word_idx = Self::encode(bx * 8, by * 8) / 64;
            let mask = Self::mask_8x8rect(local_min_x, local_max_x, local_y, local_y);

            if (data[word_idx] & mask) != 0 {
                return true;
            }
        }
        false
    }

    fn is_all_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return false;
        }
        let min_x = min_x.max(0) as u32;
        let max_x = max_x.min((W as i32) - 1) as u32;
        let uy = y as u32;

        let min_blk_x = min_x / 8;
        let max_blk_x = max_x / 8;
        let by = uy / 8;
        let local_y = uy % 8;

        for bx in min_blk_x..=max_blk_x {
            let local_min_x = if bx == min_blk_x { min_x % 8 } else { 0 };
            let local_max_x = if bx == max_blk_x { max_x % 8 } else { 7 };

            let word_idx = Self::encode(bx * 8, by * 8) / 64;
            let mask = Self::mask_8x8rect(local_min_x, local_max_x, local_y, local_y);

            if (data[word_idx] & mask) != mask {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morton_encoding_boundaries() {
        assert_eq!(MortonLayout::encode(0, 0), 0);
        assert_eq!(MortonLayout::encode(1, 0), 1);
        assert_eq!(MortonLayout::encode(0, 1), 2);
        assert_eq!(MortonLayout::encode(1, 1), 3);
        assert_eq!(MortonLayout::encode(255, 255), 65535);

        let (x, y) = MortonLayout::decode(65535);
        assert_eq!(x, 255);
        assert_eq!(y, 255);
    }

    #[test]
    fn test_morton_coord_conversions() {
        assert_eq!(
            <MortonLayout as BitLayout<256, 256>>::coord_to_word_bit(0, 0),
            Some((0, 0))
        );
        assert_eq!(
            <MortonLayout as BitLayout<256, 256>>::coord_to_word_bit(255, 255),
            Some((1023, 63))
        );
        assert_eq!(
            <MortonLayout as BitLayout<256, 256>>::coord_to_word_bit(-1, 0),
            None
        );
        assert_eq!(
            <MortonLayout as BitLayout<256, 256>>::coord_to_word_bit(0, -1),
            None
        );
        assert_eq!(
            <MortonLayout as BitLayout<256, 256>>::coord_to_word_bit(256, 0),
            None
        );
        assert_eq!(
            <MortonLayout as BitLayout<256, 256>>::coord_to_word_bit(0, 256),
            None
        );
    }
}
