use std::fmt::Debug;

pub mod morton;
pub mod row_major;

pub use morton::MortonLayout;
pub use row_major::RowMajorLayout;

/// Trait defining the memory layout for BitBoard
pub trait BitLayout<const W: usize, const H: usize>:
    Default
    + Clone
    + Debug
    + serde::Serialize
    + for<'de> serde::Deserialize<'de>
    + PartialEq
    + Eq
    + Send
    + Sync
    + 'static
{
    /// Calculates the number of words (u64) required for the specified size
    fn total_words() -> usize;

    /// Converts tile coordinates (x, y) to bit position (word_idx, bit_pos)
    fn coord_to_word_bit(x: i32, y: i32) -> Option<(usize, u32)>;

    /// Converts bit position (word_idx, bit_pos) to tile coordinates (x, y)
    fn word_bit_to_coord(word: usize, bit: u32) -> (i32, i32);

    /// Converts flat index to tile coordinates (for external API)
    fn flat_index_to_coord(idx: usize) -> (i32, i32);

    /// Converts tile coordinates to flat index (for external API)
    fn coord_to_flat_index(x: i32, y: i32) -> Option<usize>;

    /// Checks if end-of-row padding is required
    fn has_padding() -> bool;

    /// Gets the mask for end-of-row padding
    fn padding_mask() -> u64;

    /// Processes horizontal shift
    fn shift_horizontal(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    );

    /// Processes vertical shift
    fn shift_vertical(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    );

    /// Performs batch operation on a rectangular range
    fn rect_op(
        data: &mut [u64],
        block: &mut [u64],
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        value: bool,
    );

    /// Fills a row range
    fn set_row(data: &mut [u64], block: &mut [u64], y: i32, min_x: i32, max_x: i32, value: bool);

    /// Checks if any bit is set in the specified row range
    fn has_any_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool;

    /// Checks if all bits in the specified row range are set
    fn is_all_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool;

    /// Converts a continuous position (Point) to discrete grid coordinates (Coord)
    fn point_to_coord(point: (f32, f32)) -> (i32, i32) {
        (point.0.floor() as i32, point.1.floor() as i32)
    }

    /// Converts discrete grid coordinates (Coord) to a continuous position (Point) (center coordinates)
    fn coord_to_point(x: i32, y: i32) -> (f32, f32) {
        (x as f32, y as f32)
    }
}
