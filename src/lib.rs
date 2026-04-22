//! # lexaos_bitboard
//!
//! 依存関係ゼロのビットボード演算ライブラリ
//!
//! BitBoard<const W, const H> でサイズを型レベルで固定し、
//! 異なるサイズのボード間演算をコンパイル時に防止する。
//!
//! ## 使用例
//! ```
//! use lexaos_bitboard::BitBoard;
//! type MapBoard = BitBoard<256, 256>;
//!
//! let mut board = MapBoard::default();
//! board.set(10, 20, true);
//! assert!(board.get(10, 20));
//! ```

mod board;
mod bulk;
mod expand;
mod iter;
mod mask;
mod morphology;
mod ops;

pub use board::BitBoard;
pub use iter::BitBoardIter;
