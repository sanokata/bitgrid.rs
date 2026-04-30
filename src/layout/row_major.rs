use super::BitLayout;

/// 標準的な行アライメントレイアウト (Row-Major)
#[derive(Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RowMajorLayout;

impl RowMajorLayout {
    /// 特定ビット範囲のマスクを生成
    fn make_mask(start_bit: usize, end_bit: usize) -> u64 {
        let len = end_bit - start_bit + 1;
        if len == 64 {
            !0u64
        } else {
            ((1u64 << len) - 1) << start_bit
        }
    }
}

impl<const W: usize, const H: usize> BitLayout<W, H> for RowMajorLayout {
    fn total_words() -> usize {
        W.div_ceil(64) * H
    }

    fn coord_to_word_bit(x: i32, y: i32) -> Option<(usize, u32)> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return None;
        }
        let row_u64s = W.div_ceil(64);
        let word = y as usize * row_u64s + x as usize / 64;
        let bit = (x as usize % 64) as u32;
        Some((word, bit))
    }

    fn word_bit_to_coord(word: usize, bit: u32) -> (i32, i32) {
        let row_u64s = W.div_ceil(64);
        let y = word / row_u64s;
        let x = (word % row_u64s) * 64 + bit as usize;
        (x as i32, y as i32)
    }

    fn flat_index_to_coord(idx: usize) -> (i32, i32) {
        ((idx % W) as i32, (idx / W) as i32)
    }

    fn coord_to_flat_index(x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            None
        } else {
            Some(y as usize * W + x as usize)
        }
    }

    fn has_padding() -> bool {
        !W.is_multiple_of(64)
    }

    fn padding_mask() -> u64 {
        (1u64 << (W % 64)) - 1
    }

    /// 水平方向（X 軸）にビットボードをシフトする。
    ///
    /// 各行は `row_u64s` 個の u64 ワードに分かれており、行をまたいだ波及は無い。
    /// 行内では 2 種類の経路に分かれる:
    /// - `bit_offset == 0`: 64 ビット境界そのものを跨ぐシフトなので、単純な
    ///   ワード `copy_from_slice` で完了する。
    /// - `bit_offset != 0`: ワード境界をまたぐビット位置にずれるため、
    ///   隣接ワードへ「あふれた」ビットをキャリーとして次のワードに OR する。
    ///   - 正方向シフト (dist > 0): 低位 → 高位の順に走査し、`val << bit_offset`
    ///     の上位 `inv_bit_offset` ビットを次ワードへキャリー
    ///   - 負方向シフト (dist < 0): 高位 → 低位の順に走査し、`val >> bit_offset`
    ///     の下位 `inv_bit_offset` ビットを前ワードへキャリー
    fn shift_horizontal(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    ) {
        if dist == 0 {
            dst.copy_from_slice(src);
            dst_block.copy_from_slice(block);
            return;
        }

        let abs_dist = dist.unsigned_abs() as usize;
        if abs_dist >= W {
            // ボード幅以上のシフトは全消去
            dst.fill(0);
            dst_block.fill(0);
            return;
        }

        let row_u64s = W.div_ceil(64);
        let word_offset = abs_dist / 64;
        let bit_offset = (abs_dist % 64) as u32;

        // ─ ワード境界に揃ったシフト: 各行内の連続コピーのみで完了 ─
        if bit_offset == 0 {
            for y in 0..H {
                let row_base = y * row_u64s;
                let src_row = &src[row_base..row_base + row_u64s];
                let dst_row = &mut dst[row_base..row_base + row_u64s];
                if dist > 0 {
                    if word_offset < row_u64s {
                        dst_row[word_offset..]
                            .copy_from_slice(&src_row[..row_u64s - word_offset]);
                    }
                } else if word_offset < row_u64s {
                    dst_row[..row_u64s - word_offset]
                        .copy_from_slice(&src_row[word_offset..]);
                }
            }
        } else {
            // ─ ワード境界をまたぐシフト: 隣接ワードへキャリーを伝播 ─
            let inv_bit_offset = 64 - bit_offset;
            for y in 0..H {
                let row_base = y * row_u64s;
                if dist > 0 {
                    // 正方向: 低位ワードから走査し、上位 inv_bit_offset ビットを次へ持ち越す
                    let mut carry = 0;
                    for i in 0..row_u64s {
                        let idx = row_base + i;
                        let shifted_idx = idx + word_offset;
                        if shifted_idx < row_base + row_u64s {
                            let val = src[idx];
                            dst[shifted_idx] = (val << bit_offset) | carry;
                            carry = val >> inv_bit_offset;
                        }
                    }
                } else {
                    // 負方向: 高位ワードから逆走査し、下位 inv_bit_offset ビットを前へ持ち越す
                    let mut carry = 0;
                    for i in (0..row_u64s).rev() {
                        let idx = row_base + i;
                        if idx >= word_offset {
                            let shifted_idx = idx - word_offset;
                            if shifted_idx >= row_base {
                                let val = src[idx];
                                dst[shifted_idx] = (val >> bit_offset) | carry;
                                carry = val << inv_bit_offset;
                            }
                        }
                    }
                }
            }
        }

        // dst の block_mask を再構築（呼び出し側で clear_padding を行うため、
        // ここではパディング由来の偽陽性を許容する）
        for i in 0..dst.len() {
            if dst[i] != 0 {
                dst_block[i / 64] |= 1 << (i % 64);
            }
        }
    }

    fn shift_vertical(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    ) {
        if dist == 0 {
            dst.copy_from_slice(src);
            dst_block.copy_from_slice(block);
            return;
        }

        let row_u64s = W.div_ceil(64);
        let abs_dist = dist.unsigned_abs() as usize;
        if abs_dist >= H {
            dst.fill(0);
            dst_block.fill(0);
            return;
        }
        let word_offset = abs_dist * row_u64s;

        if dist > 0 {
            if word_offset < src.len() {
                dst[word_offset..].copy_from_slice(&src[..src.len() - word_offset]);
            }
        } else {
            if word_offset < src.len() {
                dst[..src.len() - word_offset].copy_from_slice(&src[word_offset..]);
            }
        }

        let block_word_offset = word_offset / 64;
        let block_bit_offset = (word_offset % 64) as u32;

        if block_bit_offset == 0 {
            if dist > 0 {
                if block_word_offset < dst_block.len() {
                    dst_block[block_word_offset..]
                        .copy_from_slice(&block[..block.len() - block_word_offset]);
                }
            } else {
                if block_word_offset < dst_block.len() {
                    dst_block[..block.len() - block_word_offset]
                        .copy_from_slice(&block[block_word_offset..]);
                }
            }
        } else {
            for i in 0..dst.len() {
                if dst[i] != 0 {
                    dst_block[i / 64] |= 1 << (i % 64);
                }
            }
        }
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
        if y + height <= 0
            || y >= H as i32
            || x + width <= 0
            || x >= W as i32
            || width <= 0
            || height <= 0
        {
            return;
        }
        let y1 = y.max(0) as usize;
        let y2 = (y + height).min(H as i32) as usize;
        let x1 = x.max(0) as usize;
        let x2 = (x + width).min(W as i32) as usize;

        let row_u64s = W.div_ceil(64);
        let sw_rel = x1 / 64;
        let ew_rel = (x2 - 1) / 64;

        let mask_sw = if sw_rel == ew_rel {
            Self::make_mask(x1 % 64, (x2 - 1) % 64)
        } else {
            Self::make_mask(x1 % 64, 63)
        };
        let mask_ew = Self::make_mask(0, (x2 - 1) % 64);

        for row in y1..y2 {
            let row_base = row * row_u64s;
            let sw = row_base + sw_rel;
            let ew = row_base + ew_rel;

            if sw == ew {
                if value {
                    data[sw] |= mask_sw;
                    block[sw / 64] |= 1 << (sw % 64);
                } else {
                    data[sw] &= !mask_sw;
                    if data[sw] == 0 {
                        block[sw / 64] &= !(1 << (sw % 64));
                    }
                }
            } else {
                if value {
                    data[sw] |= mask_sw;
                    block[sw / 64] |= 1 << (sw % 64);
                    if ew > sw + 1 {
                        data[sw + 1..ew].fill(!0u64);
                        for w in sw + 1..ew {
                            block[w / 64] |= 1 << (w % 64);
                        }
                    }
                    data[ew] |= mask_ew;
                    block[ew / 64] |= 1 << (ew % 64);
                } else {
                    data[sw] &= !mask_sw;
                    if data[sw] == 0 {
                        block[sw / 64] &= !(1 << (sw % 64));
                    }
                    if ew > sw + 1 {
                        data[sw + 1..ew].fill(0);
                        for w in sw + 1..ew {
                            block[w / 64] &= !(1 << (w % 64));
                        }
                    }
                    data[ew] &= !mask_ew;
                    if data[ew] == 0 {
                        block[ew / 64] &= !(1 << (ew % 64));
                    }
                }
            }
        }
    }

    fn set_row(data: &mut [u64], block: &mut [u64], y: i32, min_x: i32, max_x: i32, value: bool) {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x <= -1 {
            return;
        }
        let min_x = min_x.max(0) as usize;
        let max_x = max_x.min((W as i32) - 1) as usize;
        let row_u64s = W.div_ceil(64);
        let sw = (y as usize) * row_u64s + min_x / 64;
        let ew = (y as usize) * row_u64s + max_x / 64;

        if sw == ew {
            let mask = Self::make_mask(min_x % 64, max_x % 64);
            if value {
                data[sw] |= mask;
                block[sw / 64] |= 1u64 << (sw % 64);
            } else {
                data[sw] &= !mask;
                if data[sw] == 0 {
                    block[sw / 64] &= !(1u64 << (sw % 64));
                }
            }
            return;
        }

        let s_mask = Self::make_mask(min_x % 64, 63);
        let e_mask = Self::make_mask(0, max_x % 64);

        if value {
            data[sw] |= s_mask;
            block[sw / 64] |= 1u64 << (sw % 64);
            if ew > sw + 1 {
                data[sw + 1..ew].fill(!0u64);
                for w in sw + 1..ew {
                    block[w / 64] |= 1u64 << (w % 64);
                }
            }
            data[ew] |= e_mask;
            block[ew / 64] |= 1u64 << (ew % 64);
        } else {
            data[sw] &= !s_mask;
            if data[sw] == 0 {
                block[sw / 64] &= !(1u64 << (sw % 64));
            }
            if ew > sw + 1 {
                data[sw + 1..ew].fill(0);
                for w in sw + 1..ew {
                    block[w / 64] &= !(1u64 << (w % 64));
                }
            }
            data[ew] &= !e_mask;
            if data[ew] == 0 {
                block[ew / 64] &= !(1u64 << (ew % 64));
            }
        }
    }

    fn has_any_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return false;
        }
        let min_x = min_x.max(0) as usize;
        let max_x = max_x.min((W as i32) - 1) as usize;
        let row_u64s = W.div_ceil(64);
        let sw = (y as usize) * row_u64s + min_x / 64;
        let ew = (y as usize) * row_u64s + max_x / 64;

        if sw == ew {
            return (data[sw] & Self::make_mask(min_x % 64, max_x % 64)) != 0;
        }

        (data[sw] & Self::make_mask(min_x % 64, 63)) != 0
            || data[sw + 1..ew].iter().any(|&w| w != 0)
            || (data[ew] & Self::make_mask(0, max_x % 64)) != 0
    }

    fn is_all_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool {
        if y < 0 || y >= H as i32 || min_x > max_x || min_x >= W as i32 || max_x < 0 {
            return false;
        }
        let min_x = min_x.max(0) as usize;
        let max_x = max_x.min((W as i32) - 1) as usize;
        let row_u64s = W.div_ceil(64);
        let sw = (y as usize) * row_u64s + min_x / 64;
        let ew = (y as usize) * row_u64s + max_x / 64;

        if sw == ew {
            let mask = Self::make_mask(min_x % 64, max_x % 64);
            return (data[sw] & mask) == mask;
        }

        let s_mask = Self::make_mask(min_x % 64, 63);
        let e_mask = Self::make_mask(0, max_x % 64);

        (data[sw] & s_mask) == s_mask
            && data[sw + 1..ew].iter().all(|&w| w == !0u64)
            && (data[ew] & e_mask) == e_mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_major_coord_boundaries() {
        assert_eq!(
            <RowMajorLayout as BitLayout<256, 256>>::coord_to_word_bit(0, 0),
            Some((0, 0))
        );
        assert_eq!(
            <RowMajorLayout as BitLayout<256, 256>>::coord_to_word_bit(255, 255),
            Some((1023, 63))
        );
        assert_eq!(
            <RowMajorLayout as BitLayout<256, 256>>::coord_to_word_bit(-1, 0),
            None
        );
        assert_eq!(
            <RowMajorLayout as BitLayout<256, 256>>::coord_to_word_bit(0, -1),
            None
        );
        assert_eq!(
            <RowMajorLayout as BitLayout<256, 256>>::coord_to_word_bit(256, 0),
            None
        );
        assert_eq!(
            <RowMajorLayout as BitLayout<256, 256>>::coord_to_word_bit(0, 256),
            None
        );
    }
}
