use super::super::*;

impl Instance {
    /// Node 16 (0x7205F3): a delay line of u16 +2 dwords (from +0xC); the
    /// input and its delay in ms at +u16 [+0] / +4.
    pub(super) fn delay(&mut self, b: i32) -> i32 {
        let src = b + self.rw(b) as i32;
        let len = self.rw(b + 2) as i32;
        let ms = self.rd(src + 4);
        if ms != self.rd(b + 8) {
            self.wr(b + 8, ms);
            if ms < 0 {
                self.wr(src + 4, 0);
            }
            let n = fist((self.rd(src + 4) as f32 / TICK_MS) as f64);
            let n = if n >= len { len - 1 } else { n };
            self.ww(b + 4, (self.rw(b + 6) as i32 + n) as u16);
        }
        let len16 = len as u16;
        if self.rw(b + 4) >= len16 {
            self.ww(b + 4, self.rw(b + 4).wrapping_sub(len16));
        }
        if self.rw(b + 6) >= len16 {
            self.ww(b + 6, 0);
        }
        let w = self.rw(b + 4) as i32;
        self.wr(b + 0xC + 4 * w, self.rd(src));
        let r = self.rw(b + 6) as i32;
        let out = self.rd(b + 0xC + 4 * r);
        self.ww(b + 4, (w + 1) as u16);
        self.ww(b + 6, (r + 1) as u16);
        out
    }
}
