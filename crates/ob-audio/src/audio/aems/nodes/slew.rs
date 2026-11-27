use super::super::*;

impl Instance {
    /// Node 29 (0x720F32): glide f32 +0 to the target +0x18 over +0x10
    /// ms (rate × int +0x14 / 4096), without overshoot.
    pub(super) fn slew(&mut self, b: i32) -> i32 {
        let target = self.rd(b + 0x18);
        let cur = self.rf(b);
        if target as f32 == cur {
            return target;
        }
        if !(target == self.rd(b + 8) && self.rd(b + 0x10) == self.rd(b + 0xC)) {
            let ms = self.rd(b + 0x10);
            self.wr(b + 8, target);
            self.wr(b + 0xC, ms);
            if ms <= 0 {
                self.wf(b, target as f32);
                return target;
            }
            let step = (target as f64 - cur as f64) / ms as f64 * TICK_MS as f64 * (1.0 / 4096.0);
            self.wf(b + 4, step as f32);
        }
        let step = self.rf(b + 4);
        let mut v = (self.rd(b + 0x14) as f64 * step as f64 + cur as f64) as f32;
        let t = target as f32;
        if (step >= 0.0 && v > t) || (step < 0.0 && v < t) {
            v = t;
        }
        self.wf(b, v);
        fist(v as f64)
    }
}
