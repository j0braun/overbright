use super::super::*;

impl Instance {
    /// Node 15 (0x7204F3): a curve. +0 the table (a file pointer: byte
    /// +0 the entry size 1 / 2 / 4, u16 +2 the count, +4 / +8 the input
    /// range, f32 +0xC the input scale, entries from +0x10), +0xC the
    /// input; on a change the result +8 is the entry at (clamped input −
    /// min) · scale, linear between entries unless the scale is 1.
    pub(super) fn curve(&mut self, b: i32) -> i32 {
        let x = self.rd(b + 0xC);
        if x != self.rd(b + 4) {
            self.wr(b + 4, x);
            let p = self.program.clone();
            let t = self.rd(b);
            let f = &p.file;
            let at = |o: i32| t as usize + o as usize;
            if at(0x10) > f.len() {
                return self.rd(b + 8);
            }
            let (lo, hi) = (p.file_u32(t + 4), p.file_u32(t + 8));
            let k = x.clamp(lo, hi.max(lo)) - lo;
            let size = f[at(0)];
            let count = read_u16(f, at(2)) as i32;
            let entry = |i: i32| -> i32 {
                let o = at(0x10);
                match size {
                    2 => f
                        .get(o + 2 * i as usize..o + 2 * i as usize + 2)
                        .map_or(0, |e| i16::from_le_bytes([e[0], e[1]]) as i32),
                    1 => f.get(o + i as usize).map_or(0, |&e| e as i8 as i32),
                    _ => p.file_u32(t + 0x10 + 4 * i),
                }
            };
            let scale = f32::from_bits(p.file_u32(t + 0xC) as u32);
            let v = if scale == 1.0 {
                entry(k)
            } else {
                let xs = (k as f32 * scale) as f64;
                let i = fist((xs - 0.5) as f32 as f64);
                let frac = xs - i as f64;
                let j = if i + 1 < count { i + 1 } else { count - 1 };
                let (a, c) = (entry(i) as f64, entry(j) as f64);
                fist(((c - a) * frac + a) as f32 as f64)
            };
            self.wr(b + 8, v);
        }
        self.rd(b + 8)
    }
}
