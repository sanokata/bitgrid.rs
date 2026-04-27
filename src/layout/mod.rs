use std::fmt::Debug;

pub mod morton;
pub mod row_major;

pub use morton::MortonLayout;
pub use row_major::RowMajorLayout;

/// BitBoard のメモリレイアウトを定義するトレイト
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
    /// 指定されたサイズに必要なワード数 (u64) を計算
    fn total_words() -> usize;

    /// タイル座標 (x, y) からビット位置 (word_idx, bit_pos) への変換
    fn coord_to_word_bit(x: i32, y: i32) -> Option<(usize, u32)>;

    /// ビット位置 (word_idx, bit_pos) からタイル座標 (x, y) への変換
    fn word_bit_to_coord(word: usize, bit: u32) -> (i32, i32);

    /// フラットインデックスからタイル座標への変換 (外部API用)
    fn flat_index_to_coord(idx: usize) -> (i32, i32);

    /// タイル座標からフラットインデックスへの変換 (外部API用)
    fn coord_to_flat_index(x: i32, y: i32) -> Option<usize>;

    /// 行末パディングが必要か判定
    fn has_padding() -> bool;

    /// 行末パディング用のマスクを取得
    fn padding_mask() -> u64;

    /// 水平シフト処理
    fn shift_horizontal(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    );

    /// 垂直シフト処理
    fn shift_vertical(
        src: &[u64],
        block: &[u64],
        dst: &mut [u64],
        dst_block: &mut [u64],
        dist: i32,
    );

    /// 矩形範囲の一括操作
    fn rect_op(
        data: &mut [u64],
        block: &mut [u64],
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        value: bool,
    );

    /// 行範囲の塗りつぶし
    fn set_row(data: &mut [u64], block: &mut [u64], y: i32, min_x: i32, max_x: i32, value: bool);

    /// 指定行にビットが立っているか判定
    fn has_any_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool;

    /// 指定行の範囲内がすべてセットされているか判定
    fn is_all_in_row(data: &[u64], y: i32, min_x: i32, max_x: i32) -> bool;

    /// ワールド座標 (f32) をタイル座標 (i32) に変換
    fn world_to_tile(pos: (f32, f32)) -> (i32, i32) {
        (pos.0.round() as i32, pos.1.round() as i32)
    }

    /// タイル座標 (i32) をワールド座標 (f32) に変換（中心座標）
    fn tile_to_world(x: i32, y: i32) -> (f32, f32) {
        (x as f32, y as f32)
    }
}
