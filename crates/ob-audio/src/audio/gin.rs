//! The granular engine sounds (`.gin`, "Gnsu20"): a recording of an
//! engine sweeping its rev range, cut into cycles, replayed cycle by
//! cycle at the position the rpm asks for (the game's own code, 0x466770
//! on; not the audio library).
//!
//! Header (0x466770, checks "Gnsu2"): +0x08 f32 lowest rpm, +0x0C f32
//! highest, +0x10 rpm steps N, +0x14 cycles M, +0x18 samples, +0x1C
//! sample rate; then N + 1 u32 sample positions (rpm table), M + 1 u32
//! cycle starts, then the data in 19-byte blocks of 32 samples.

use crate::audio::codec::eaxa::{coef, scaled};
use crate::formats::bundle::{read_f32, read_u32};

#[derive(Clone, Debug)]
pub struct Gin {
    pub min_rpm: f32,
    pub max_rpm: f32,
    pub rpm_pos: Vec<i32>,
    pub cycles: Vec<i32>,
    pub samples: i32,
    pub rate: u32,
    data: Vec<u8>,
    /// The decoded block (0x466590 keeps one: its index and 32 samples).
    block: Option<(usize, [f32; 32])>,
}

/// Round to nearest, ties to even (the x87's fistp).
fn round(x: f32) -> i32 {
    x.round_ties_even() as i32
}

impl Gin {
    pub fn parse(d: &[u8]) -> Option<Gin> {
        if d.len() < 0x20 || &d[..5] != b"Gnsu2" {
            return None;
        }
        // Fuzz: a rate that is no sound rate would size the granular block
        // (rate x 0.011 samples) from the file.
        if !(1..=384_000).contains(&read_u32(d, 0x1C)) {
            return None;
        }
        let n = read_u32(d, 0x10) as usize;
        let m = read_u32(d, 0x14) as usize;
        let a = 0x20;
        let b = a + (n + 1) * 4;
        let data = b + (m + 1) * 4;
        if data > d.len() {
            return None;
        }
        let rpm_pos: Vec<i32> = (0..=n).map(|i| read_u32(d, a + i * 4) as i32).collect();
        let cycles: Vec<i32> = (0..=m).map(|i| read_u32(d, b + i * 4) as i32).collect();
        let samples = read_u32(d, 0x18) as i32;
        Some(Gin { min_rpm: read_f32(d, 8), max_rpm: read_f32(d, 0xC), rpm_pos, cycles, samples, rate: read_u32(d, 0x1C), data: d[data..].to_vec(), block: None })
    }

    /// Decode block `i` (0x466590): two 12-bit history samples in the
    /// first four bytes (high nibbles of bytes 0 and 2 with bytes 1 and 3
    /// as the high byte), the coefficients from byte 0's low nibble and
    /// the shift from byte 2's, then 15 bytes of nibbles, high first.
    fn decode(&mut self, i: usize) -> [f32; 32] {
        if let Some((at, s)) = self.block
            && at == i
        {
            return s;
        }
        let mut s = [0.0f32; 32];
        let o = i * 0x13;
        if let Some(b) = self.data.get(o..o + 0x13) {
            s[0] = ((b[0] & 0xF0) as i32 + (b[1] as i8 as i32) * 0x100) as f32;
            s[1] = ((b[2] & 0xF0) as i32 + (b[3] as i8 as i32) * 0x100) as f32;
            let (c1, c2) = coef((b[0] & 0xF) as usize);
            let shift = (b[2] & 0xF) as usize;
            for j in 0..15 {
                for (h, nib) in [b[4 + j] >> 4, b[4 + j] & 0xF].into_iter().enumerate() {
                    let n = 2 + j * 2 + h;
                    s[n] = c1 * s[n - 1] + c2 * s[n - 2] + scaled(shift, nib);
                }
            }
        }
        self.block = Some((i, s));
        s
    }

    /// Samples `start..start + n` as i16 (0x466C60; nothing is written
    /// when the range runs outside the recording).
    pub fn read(&mut self, start: i32, n: usize, out: &mut [i16]) {
        if start < 0 || start as i64 + n as i64 - 1 >= self.samples as i64 {
            return;
        }
        for k in 0..n {
            let at = start as usize + k;
            let v = self.decode(at >> 5)[at & 31];
            out[k] = round(v).clamp(-32768, 32767) as i16;
        }
    }

    /// The sample position for an rpm (0x466840): the rpm table
    /// interpolated, clamped at its ends.
    pub fn rpm_position(&self, rpm: f32) -> i32 {
        let n = self.rpm_pos.len() as i32 - 1;
        if n < 1 {
            return 0;
        }
        if rpm <= self.min_rpm {
            return self.rpm_pos[0];
        }
        if self.max_rpm <= rpm {
            return self.rpm_pos[n as usize];
        }
        let f = (rpm - self.min_rpm) * n as f32 / (self.max_rpm - self.min_rpm);
        let i = (f.floor() as usize).min(n as usize - 1);
        let a = self.rpm_pos[i];
        round((f - i as f32) * (self.rpm_pos[i + 1] - a) as f32 + a as f32)
    }

    fn cycle_count(&self) -> i32 {
        self.cycles.len() as i32 - 1
    }

    /// The fractional cycle at a sample position (0x466AF0): the cycle
    /// holding it plus how far into it. The game finds the cycle by
    /// guessing from the shortest cycle and narrowing; a binary search
    /// finds the same one.
    pub fn cycle_at(&self, pos: i32) -> f32 {
        let m = self.cycle_count();
        if m < 1 || pos <= self.cycles[0] {
            return 0.0;
        }
        if self.cycles[m as usize] <= pos {
            return m as f32;
        }
        // (clamped: a file's cycle table need not be sorted)
        let i = self.cycles.partition_point(|&c| c <= pos).saturating_sub(1).min(m as usize - 1);
        let (c, next) = (self.cycles[i], self.cycles[i + 1]);
        (pos - c) as f32 / (next - c) as f32 + i as f32
    }

    /// The length of the cycle at a fractional cycle (0x4669D0): the
    /// neighbouring cycles' lengths interpolated.
    pub fn cycle_length(&self, c: f32) -> f32 {
        let m = self.cycle_count();
        let t = &self.cycles;
        if m < 1 {
            return 0.0;
        }
        let mut c = c;
        let mut i = c.floor() as i32;
        let (a, b);
        if i < 1 {
            if i < 0 {
                c = 0.0;
                i = 0;
            }
            a = (t[1] - t[0]) as f32;
            b = (t.get(2).copied().unwrap_or(t[1]) - t[0]) as f32 * 0.5;
        } else if i < m - 1 {
            let i = i as usize;
            a = (t[i + 1] - t[i - 1]) as f32 * 0.5;
            b = (t[i + 2] - t[i]) as f32 * 0.5;
        } else {
            if m <= i {
                c = m as f32;
                i = m - 1;
            }
            let mm = m as usize;
            a = (t[mm] - t[mm.saturating_sub(2)]) as f32 * 0.5;
            b = (t[mm] - t[mm - 1]) as f32;
        }
        (b - a) * (c - i as f32) + a
    }

    /// The sample position of a fractional cycle (0x466910).
    pub fn cycle_position(&self, c: f32) -> i32 {
        let m = self.cycle_count();
        if m < 1 {
            return 0;
        }
        if c <= 0.0 {
            return self.cycles[0];
        }
        if m as f32 <= c {
            return self.cycles[m as usize];
        }
        let i = (c.floor() as usize).min(m as usize - 1);
        let a = self.cycles[i];
        round((c - i as f32) * (self.cycles[i + 1] - a) as f32 + a as f32)
    }
}

/// The engine's granular player (0x467160 sets it up, 0x467230 aims it,
/// 0x466D30 fills each block).
#[derive(Clone, Debug)]
pub struct Granular {
    pub gin: Gin,
    /// Block length (rate × 0.011), crossfade (rate × 0.0005), and the
    /// cycle budget after a jump (rate × 0.011): 0x467160.
    block: usize,
    fade: usize,
    budget: i32,
    /// Read position, the position aimed at, the new aim and the blocks
    /// left to reach it, and the samples left before a jump is allowed.
    pos: i32,
    target: i32,
    aim: i32,
    steps: i32,
    left: i32,
}

impl Granular {
    pub fn new(gin: Gin) -> Granular {
        let rate = gin.rate as f32;
        let start = gin.rpm_position(gin.min_rpm);
        Granular {
            block: (rate * 0.011).round_ties_even() as usize,
            fade: (rate * 0.0005).round_ties_even() as usize,
            budget: (rate * 0.011).round_ties_even() as i32,
            pos: start,
            target: start,
            aim: start,
            steps: 1,
            left: 0,
            gin,
        }
    }

    pub fn rate(&self) -> u32 {
        self.gin.rate
    }

    pub fn block(&self) -> usize {
        self.block
    }

    /// The read position and the position the rpm asks for.
    pub fn position(&self) -> (i32, i32) {
        (self.pos, self.aim)
    }

    /// Aim at an rpm over `ms` milliseconds (0x467230): the new position
    /// from the rpm table, reached over ms / 11 + 1 blocks.
    pub fn set_rpm(&mut self, rpm: f32, ms: f32) {
        self.aim = self.gin.rpm_position(rpm);
        self.steps = (ms / (0.011 * 1000.0)).floor() as i32 + 1;
    }

    /// One block (0x466D30): keep reading on while the current cycle
    /// lasts; when it runs out inside the block, jump by whole cycles
    /// towards the aimed cycle — crossfading `fade` samples from the old
    /// place into the new — when the drift calls for it.
    pub fn fill(&mut self, out: &mut [i16]) {
        let block = self.block;
        let g = &mut self.gin;
        let want = g.cycle_at(self.target);
        let step = (self.aim - self.target) as f32 / self.steps as f32;
        let mut jump = 0i32;
        let mut part = 1.0f32;
        if self.left < block as i32 {
            let at = g.cycle_at(self.pos);
            let drift = want - at;
            let rate = step / g.cycle_length(want) - block as f32 / g.cycle_length(at);
            let t0 = self.left as f32 / block as f32;
            let a = t0 * rate + drift;
            let b = rate + drift;
            let mut take = false;
            if a.floor() == b.floor() {
                let r = (rate * 1.0).abs();
                if a.abs() > r.ceil() {
                    jump = if a >= 0.0 { a.floor() as i32 } else { a.ceil() as i32 };
                    part = t0;
                    take = jump != 0;
                }
            } else {
                jump = if b <= a { a.floor() as i32 } else { a.ceil() as i32 };
                part = (jump as f32 - drift) / rate;
                take = jump != 0;
            }
            if !take {
                jump = 0;
                part = 1.0;
            }
        }
        let mut n = ((block as f32) * part).round_ties_even() as usize;
        n = n.min(block);
        g.read(self.pos, n, &mut out[..n]);
        self.pos += n as i32;
        self.left = (self.left - n as i32).max(0);
        if jump != 0 {
            let fade = self.fade.min(block - n);
            g.read(self.pos, fade, &mut out[n..n + fade]);
            let to = g.cycle_position(jump as f32 + g.cycle_at(self.pos));
            self.pos = to;
            // The crossfade's new samples on the stack (rate × 0.0005: 24
            // at 48 kHz); no allocation in the audio callback.
            // A longer one (above 128 kHz) on the heap.
            let mut stack = [0i16; 64];
            let mut heap = Vec::new();
            let other: &mut [i16] = if fade <= stack.len() {
                &mut stack[..fade]
            } else {
                heap.resize(fade, 0);
                &mut heap
            };
            g.read(self.pos, fade, other);
            let mut w = 0.0f32;
            for k in 0..fade {
                let a = out[n + k] as f32;
                out[n + k] = round((other[k] as f32 - a) * w + a).clamp(-32768, 32767) as i16;
                w += 1.0 / fade as f32;
            }
            self.pos += fade as i32;
            n += fade;
            self.left = self.budget - fade as i32;
            let rest = block as i32 - n as i32;
            if rest > 0 {
                g.read(self.pos, rest as usize, &mut out[n..block]);
                self.pos += rest;
                self.left -= rest;
            }
        }
        self.target = (self.target as f32 + step).round_ties_even() as i32;
        if self.steps > 1 {
            self.steps -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Gin {
        Gin {
            min_rpm: 1000.0,
            max_rpm: 3000.0,
            rpm_pos: vec![0, 100, 200],
            cycles: vec![0, 50, 100, 150, 200],
            samples: 256,
            rate: 36000,
            data: vec![0; 0x13 * 8],
            block: None,
        }
    }

    #[test]
    fn rpm_and_cycle_maps_interpolate() {
        let g = tiny();
        assert_eq!(g.rpm_position(500.0), 0);
        assert_eq!(g.rpm_position(2000.0), 100);
        assert_eq!(g.rpm_position(2500.0), 150);
        assert_eq!(g.cycle_at(75), 1.5);
        assert_eq!(g.cycle_position(2.5), 125);
        assert_eq!(g.cycle_length(1.0), 50.0);
    }

    #[test]
    fn blocks_decode_their_history() {
        let mut g = tiny();
        g.data[0] = 0x10; // coefficient set 1, history 0x10
        g.data[1] = 0x01; // high byte 1 → 0x110
        g.data[2] = 0x0C; // shift 12
        g.data[4] = 0x10; // nibbles 1, 0
        let s = g.decode(0);
        assert_eq!(s[0], 0x110 as f32);
        assert_eq!(s[2], 0.9375 * s[1] + 1.0);
    }
}
