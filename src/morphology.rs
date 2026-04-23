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
            
            // 正方向
            shift_into(self, d as i32, buffer);
            op(self, buffer);
            
            // 負方向
            shift_into(self, -(d as i32), buffer);
            op(self, buffer);
            
            current_range += d;
        }
    }

    /// セットされたビットを全方向（8方向）に膨張させる (アロケート回避版)
    pub fn dilate_with_buffer(&mut self, steps: u32, buffer: &mut Self) {
        if steps == 0 { return; }

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

    /// セットされたビットを全方向（8方向）に膨張させる
    pub fn dilate(&self, steps: u32) -> Self {
        if steps == 0 { return self.clone(); }
        let mut res = self.clone();
        let mut buffer = Self::new();
        res.dilate_with_buffer(steps, &mut buffer);
        res
    }

    /// セットされたビットを全方向（8方向）に収縮させる (アロケート回避版)
    pub fn erode_with_buffer(&mut self, steps: u32, buffer: &mut Self) {
        if steps == 0 { return; }

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

    /// セットされたビットを全方向（8方向）に収縮させる
    pub fn erode(&self, steps: u32) -> Self {
        if steps == 0 { return self.clone(); }
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
        
        // 1ステップ膨張
        let d1 = bb.dilate(1);
        assert_eq!(d1.count_ones(), 9); // 3x3
        assert!(d1.get(99, 99));
        assert!(d1.get(101, 101));
        assert!(!d1.get(98, 100));

        // 2ステップ膨張
        let d2 = bb.dilate(2);
        assert_eq!(d2.count_ones(), 25); // 5x5
        assert!(d2.get(98, 98));
        assert!(d2.get(102, 102));
    }

    #[test]
    fn test_morphology_erode() {
        let mut bb = TestBoard::default();
        // 3x3 のブロックを作成
        for x in 99..=101 {
            for y in 99..=101 {
                bb.set(x, y, true);
            }
        }
        assert_eq!(bb.count_ones(), 9);

        // 1ステップ収縮
        let e1 = bb.erode(1);
        assert_eq!(e1.count_ones(), 1); // 中心だけ残る
        assert!(e1.get(100, 100));
        assert!(!e1.get(99, 99));

        // 2ステップ収縮
        let e2 = bb.erode(2);
        assert_eq!(e2.count_ones(), 0); // すべて消える
    }

    #[test]
    fn test_shifted_horizontal_edge_cases() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);
        
        let sh_l = bb.shifted_horizontal(-1); // 東から西へ (x-)
        assert!(!sh_l.get(0, 0)); 
        assert_eq!(sh_l.count_ones(), 0); // 画面外へ
        
        let sh_r = bb.shifted_horizontal(255); // 西から東へ (x+)
        assert!(sh_r.get(255, 0));
        
        let sh_r_out = bb.shifted_horizontal(256); // 画面外へ
        assert_eq!(sh_r_out.count_ones(), 0);
    }
    
    #[test]
    fn test_shifted_vertical_edge_cases() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);
        
        let sh_u = bb.shifted_vertical(-1); // 上へ (y-)
        assert!(!sh_u.get(0, 0));
        assert_eq!(sh_u.count_ones(), 0); // 画面外へ
        
        let sh_d = bb.shifted_vertical(255); // 下へ (y+)
        assert!(sh_d.get(0, 255));
        
        let sh_d_out = bb.shifted_vertical(256); // 画面外へ
        assert_eq!(sh_d_out.count_ones(), 0);
    }
}
