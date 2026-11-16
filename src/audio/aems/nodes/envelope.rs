use super::super::*;

impl Instance {
    /// 0x72040F: a segment's time and per-tick step.
    pub(super) fn segment(&mut self, b: i32) {
        let i = self.rb(b + 3) as i32;
        let dur = self.rf(b + 0x18 + 8 * i);
        self.wf(b + 4, dur);
        let step = (self.rf(b + 0x1C + 8 * i) - self.rf(b + 0xC)) as f64 / dur as f64;
        self.wf(b + 8, (step * TICK_MS as f64) as f32);
    }

    /// Node 14 (0x720434): a gated multi-segment envelope. Gate (at +u16
    /// [+0]) 1 from 0 restarts from +0x14; 3 jumps to the sustain segment
    /// u16 +0x12; 1 or 3 run the segments (byte +0x10 of {f32 ms, f32
    /// level} from +0x18), 0 after the last; 2 holds; else 0.
    pub(super) fn envelope(&mut self, b: i32) -> i32 {
        let gate = self.rd(b + self.rw(b) as i32);
        let prev = self.rb(b + 2);
        if gate == 1 && prev == 0 {
            self.wb(b + 3, 0);
            self.wr(b + 0xC, self.rd(b + 0x14));
            self.segment(b);
        } else if gate == 3 && prev != 3 && (self.rb(b + 3) as u16) < self.rw(b + 0x12) {
            self.wb(b + 3, self.rb(b + 0x12));
            self.segment(b);
        } else if (gate == 1 || gate == 3) && self.rb(b + 3) < self.rb(b + 0x10) {
            let left = self.rf(b + 4) - TICK_MS;
            self.wf(b + 4, left);
            if left > 0.0 {
                self.wf(b + 0xC, self.rf(b + 8) + self.rf(b + 0xC));
            } else {
                let i = self.rb(b + 3) as i32;
                self.wf(b + 0xC, self.rf(b + 0x1C + 8 * i));
                self.wb(b + 3, (i + 1) as u8);
                if ((i + 1) as u8) < self.rb(b + 0x10) {
                    self.segment(b);
                } else {
                    self.wf(b + 0xC, 0.0);
                }
            }
        } else if gate != 2 {
            self.wf(b + 0xC, 0.0);
        }
        self.wb(b + 2, gate as u8);
        fist(self.rf(b + 0xC) as f64)
    }
}
