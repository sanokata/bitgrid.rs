use crate::BitBoard;

impl<const W: usize, const H: usize> BitBoard<W, H> {
    /// 水平方向に指定距離シフトした新しいボードを返す (内部用)
    pub(crate) fn shifted_h(&self, dist: i32) -> Self {
        let mut res = Self::new();
        if dist == 0 { return self.clone(); }
        let abs_dist = dist.abs() as usize;
        if abs_dist >= W { return res; }
        
        let word_dist = abs_dist / 64;
        let bit_dist = (abs_dist % 64) as u32;
        
        // L1マスクを活用して、64ワード（L1の1ビット分）単位で空の領域をスキップ
        for l1_idx in 0..Self::L1_WORDS {
            if self.l1_mask[l1_idx] == 0 { continue; }
            
            let start_idx = l1_idx * 64;
            let end_idx = (start_idx + 64).min(Self::TOTAL_WORDS);
            
            for i in start_idx..end_idx {
                let row = i / Self::ROW_U64S;
                let _s = row * Self::ROW_U64S;
                let col_word = i % Self::ROW_U64S;
                
                if dist > 0 { // 西から東へ (左シフト)
                    if col_word >= word_dist {
                        let w = self.data[i - word_dist];
                        let carry = if bit_dist > 0 && col_word > word_dist {
                            self.data[i - word_dist - 1] >> (64 - bit_dist)
                        } else { 0 };
                        res.data[i] = (w << bit_dist) | carry;
                    }
                } else { // 東から西へ (右シフト)
                    if col_word + word_dist < Self::ROW_U64S {
                        let w = self.data[i + word_dist];
                        let carry = if bit_dist > 0 && col_word + word_dist + 1 < Self::ROW_U64S {
                            self.data[i + word_dist + 1] << (64 - bit_dist)
                        } else { 0 };
                        res.data[i] = (w >> bit_dist) | carry;
                    }
                }
            }
        }
        res.finalize();
        res
    }

    /// 垂直方向に指定距離シフトした新しいボードを返す (内部用)
    pub(crate) fn shifted_v(&self, dist: i32) -> Self {
        let mut res = Self::new();
        if dist == 0 { return self.clone(); }
        let abs_dist = dist.abs() as usize;
        if abs_dist >= H { return res; }
        
        let word_offset = abs_dist * Self::ROW_U64S;
        
        // 垂直シフトも L1 マスクでスキップ（ソース領域が空ならコピー不要）
        for l1_idx in 0..Self::L1_WORDS {
            if self.l1_mask[l1_idx] == 0 { continue; }
            
            let start_idx = l1_idx * 64;
            let end_idx = (start_idx + 64).min(Self::TOTAL_WORDS);
            
            if dist > 0 { // 下へ (y+)
                for i in start_idx..end_idx {
                    if i + word_offset < Self::TOTAL_WORDS {
                        res.data[i + word_offset] = self.data[i];
                    }
                }
            } else { // 上へ (y-)
                for i in start_idx..end_idx {
                    if i >= word_offset {
                        res.data[i - word_offset] = self.data[i];
                    }
                }
            }
        }
        res.finalize();
        res
    }

    /// セットされたビットを全方向（8方向）に膨張させる (指数的最適化版)
    pub fn dilate(&self, steps: u32) -> Self {
        if steps == 0 { return self.clone(); }
        let mut res = self.clone();

        // 1. 水平方向の膨張 (O(log N))
        let mut current_range = 0;
        while current_range < steps {
            let d = (steps - current_range).min(current_range + 1);
            let shifted_l = res.shifted_h(d as i32);
            let shifted_r = res.shifted_h(-(d as i32));
            res |= &(shifted_l | shifted_r);
            current_range += d;
        }

        // 2. 垂直方向の膨張 (O(log N))
        let mut current_range = 0;
        while current_range < steps {
            let d = (steps - current_range).min(current_range + 1);
            let shifted_u = res.shifted_v(d as i32);
            let shifted_d = res.shifted_v(-(d as i32));
            res |= &(shifted_u | shifted_d);
            current_range += d;
        }
        res.finalize();
        res
    }

    /// セットされたビットを全方向（8方向）に収縮させる (指数的最適化版)
    pub fn erode(&self, steps: u32) -> Self {
        if steps == 0 { return self.clone(); }
        let mut res = self.clone();

        // 1. 水平方向の収縮
        let mut current_range = 0;
        while current_range < steps {
            let d = (steps - current_range).min(current_range + 1);
            let shifted_l = res.shifted_h(d as i32);
            let shifted_r = res.shifted_h(-(d as i32));
            res &= &(shifted_l & shifted_r);
            current_range += d;
        }

        // 2. 垂直方向の収縮
        let mut current_range = 0;
        while current_range < steps {
            let d = (steps - current_range).min(current_range + 1);
            let shifted_u = res.shifted_v(d as i32);
            let shifted_d = res.shifted_v(-(d as i32));
            res &= &(shifted_u & shifted_d);
            current_range += d;
        }
        res.finalize();
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
    fn test_shifted_h_edge_cases() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);
        
        let sh_l = bb.shifted_h(-1); // 東から西へ (x-)
        assert!(!sh_l.get(0, 0)); 
        assert_eq!(sh_l.count_ones(), 0); // 画面外へ
        
        let sh_r = bb.shifted_h(255); // 西から東へ (x+)
        assert!(sh_r.get(255, 0));
        
        let sh_r_out = bb.shifted_h(256); // 画面外へ
        assert_eq!(sh_r_out.count_ones(), 0);
    }
    
    #[test]
    fn test_shifted_v_edge_cases() {
        let mut bb = TestBoard::default();
        bb.set(0, 0, true);
        
        let sh_u = bb.shifted_v(-1); // 上へ (y-)
        assert!(!sh_u.get(0, 0));
        assert_eq!(sh_u.count_ones(), 0); // 画面外へ
        
        let sh_d = bb.shifted_v(255); // 下へ (y+)
        assert!(sh_d.get(0, 255));
        
        let sh_d_out = bb.shifted_v(256); // 画面外へ
        assert_eq!(sh_d_out.count_ones(), 0);
    }
}
