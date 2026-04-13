//! # bitboard
//!
//! 依存関係ゼロのビットボード演算ライブラリ。
//!
//! `BitBoard<const W: usize, const H: usize>` でサイズを型レベルで固定し、
//! 異なるサイズのビットボード同士の演算をコンパイル時に防止する。
//!
//! ## 使用例
//!
//! ```
//! use lexaos_bitboard::BitBoard;
//!
//! // 利用側でマップサイズに応じた型エイリアスを定義
//! type MapBoard = BitBoard<256, 256>;
//!
//! let mut board = MapBoard::default();
//! board.set(10, 20, true);
//! assert!(board.get(10, 20));
//! ```

mod board;
mod expand;
mod iter;
mod mask;
mod ops;

pub use board::BitBoard;
pub use iter::BitBoardIter;
