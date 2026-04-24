mod board;
mod bulk;
mod iter;
mod layout;
mod query;
mod ops;
mod mask;
mod expand;
mod morphology;

// 公開する主要な型とトレイト
pub use board::{BitBoard, BitBoardInterface};
pub use layout::{BitLayout, RowMajorLayout, MortonLayout};
pub use iter::BitBoardIter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smoke() {
        let mut bb = BitBoard::<256, 256>::default();
        bb.set(10, 10, true);
        assert!(bb.get(10, 10));
        assert_eq!(bb.count_ones(), 1);
    }
}
