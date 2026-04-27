mod board;
mod bulk;
mod expand;
mod iter;
mod layout;
mod mask;
mod morphology;
mod ops;
mod query;

// 公開する主要な型とトレイト
pub use board::BitBoard;
pub use iter::BitBoardIter;
pub use layout::{BitLayout, MortonLayout, RowMajorLayout};

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
