//! EA MicroTalk ("UTK", codecs 4 and 0x16 of a "PT" header: the audio
//! library's decoder class 3 / 4, 0x728DF9 → 0x8B482C / 0x8B4838, set
//! up by 0x72CB4B). The speech streams (`speech/UG2_Speech.big`) use it.
//! Decoded as SPEED2.EXE does (verified against 0x7343F3 and its
//! helpers); the state is 0xD68 bytes (0x7346A0):
//!
//! * +0x0 the read pointer, +4 the bit buffer, +8 its bit count (LSB
//!   first; refilled by a byte whenever under 8, 0x733A33), +0xC the
//!   reduced-bandwidth flag, +0x10 the multi-pulse threshold, +0x14 the
//!   64 fixed gains, +0x114 the 12 reflection coefficients, +0x144 the
//!   synthesis history, +0x174 the adaptive codebook (324 floats) and
//!   +0x684 the frame (432 floats) right after it.
//! * **Header** (0x73432C, when the caller asks for one): 1 bit reduced
//!   bandwidth, 4 bits t (threshold 32 − t), 4 bits g (gain 0 = 8·(g+1)),
//!   6 bits m (gain i = gain i−1 · (1.04 + m·0.001)); the history, the
//!   coefficients and the codebook are cleared.
//! * **Frame** (0x7343F3, 432 samples): coefficient 0 from 6 bits (under
//!   the threshold: multi-pulse excitation), 1–3 from 6 bits, 4–11 from 5
//!   bits + 16, all from the table 0x819208, each stepped in four quarter
//!   steps; four subframes of 108: 8 bits lag (216 + 108·i − lag into the
//!   codebook + frame), 4 bits pitch gain /15, 6 bits fixed gain; the
//!   excitation (0x733A9E: multi-pulse with the codebooks 0x819308 and the
//!   commands 0x819508, else RELP ±2/0), in reduced bandwidth every other
//!   sample after 1 bit alignment and 1 bit "zero" (zero: the rest 0;
//!   else the 6-tap interpolation 0x7D1F00.. and the gain halved); then
//!   the codebook shifted and the 12-tap synthesis (0x733C54 after the
//!   coefficient → LPC conversion 0x733BC8) over 12, 12, 12 and 396
//!   samples.
//! * **Rev-3 blocks** (0x734798, state +0xD64 set by 0x73478B): each
//!   frame is preceded by a marker byte; 0xEE means that, after the frame
//!   is decoded, a big-endian offset, count and `count` big-endian samples
//!   overwrite part of it. The byte left in the bit buffer after a frame
//!   is the next frame's marker; the bit reader restarts after it.

static RC: crate::exe_tables::Table<[f32; 64]> = crate::exe_tables::Table::new(|t| &t.utk_rc);

/// The two codebooks (0x819308): the normal and the large-pulse model's
/// command for the bit buffer's low byte.
static CODEBOOKS: crate::exe_tables::Table<[[u8; 256]; 2]> = crate::exe_tables::Table::new(|t| &t.utk_codebooks);

fn codebook(model: usize, b: usize) -> usize {
    CODEBOOKS[model][b] as usize
}

/// {next model, code size, pulse} (0x819508).
static COMMANDS: crate::exe_tables::Table<[(u8, u8, f32); 29]> = crate::exe_tables::Table::new(|t| &t.utk_commands);

pub struct Utk<'a> {
    d: &'a [u8],
    p: usize,
    bits: u32,
    count: u32,
    reduced_bw: bool,
    threshold: u32,
    gains: [f32; 64],
    rc: [f32; 12],
    synth: [f32; 12],
    /// The adaptive codebook (324) followed by the frame (432).
    buf: [f32; 756],
    rev3: bool,
    pcm_next: bool,
}

impl<'a> Utk<'a> {
    fn byte(&mut self) -> u32 {
        let b = self.d.get(self.p).copied().unwrap_or(0) as u32;
        self.p += 1;
        b
    }

    /// 0x733A33.
    fn read(&mut self, n: u32) -> u32 {
        let v = self.bits & ((1 << n) - 1);
        self.bits >>= n;
        self.count -= n;
        if self.count < 8 {
            self.bits |= self.byte() << self.count;
            self.count += 8;
        }
        v
    }

    fn start_bits(&mut self) {
        self.bits = self.byte();
        self.count = 8;
    }

    /// A decoder on one block's data (0x7346DB): with `rev3` the first
    /// byte is the marker; `header` parses the header (0x73432C).
    pub fn new(d: &'a [u8], rev3: bool) -> Utk<'a> {
        let mut u = Utk {
            d,
            p: 0,
            bits: 0,
            count: 0,
            reduced_bw: false,
            threshold: 0,
            gains: [0.0; 64],
            rc: [0.0; 12],
            synth: [0.0; 12],
            buf: [0.0; 756],
            rev3,
            pcm_next: false,
        };
        u.begin(d, true);
        u
    }

    /// Continue on a new block (the stream path keeps the state and
    /// parses a fresh header each block: 0x732866 / 0x732980 call
    /// 0x7346DB with the header flag set).
    pub fn begin(&mut self, d: &'a [u8], header: bool) {
        self.d = d;
        self.p = 0;
        if self.rev3 {
            self.pcm_next = self.byte() == 0xEE;
        }
        self.start_bits();
        if header {
            self.reduced_bw = self.read(1) != 0;
            self.threshold = 32 - self.read(4);
            self.gains[0] = 8.0 * (1 + self.read(4)) as f32;
            let m = 1.04 + self.read(6) as f32 * 0.001;
            for i in 1..64 {
                self.gains[i] = self.gains[i - 1] * m;
            }
            self.rc = [0.0; 12];
            self.synth = [0.0; 12];
            self.buf[..324].fill(0.0);
        }
    }

    /// 0x733A9E.
    fn excitation(&mut self, multipulse: bool, out: &mut [f32; 118], start: usize, stride: usize) {
        let mut i = 0;
        if multipulse {
            let mut model = 0usize;
            while i < 108 {
                let cmd = codebook(model, (self.bits & 0xFF) as usize);
                let (next, size, pulse) = COMMANDS[cmd];
                model = next as usize;
                self.read(size as u32);
                if cmd > 3 {
                    out[start + i] = pulse;
                    i += stride;
                } else if cmd > 1 {
                    let mut n = 7 + self.read(6) as usize;
                    if i + n * stride > 108 {
                        n = (108 - i) / stride;
                    }
                    for _ in 0..n {
                        out[start + i] = 0.0;
                        i += stride;
                    }
                } else {
                    let mut x = 7i32;
                    while self.read(1) != 0 {
                        x += 1;
                    }
                    if self.read(1) == 0 {
                        x = -x;
                    }
                    out[start + i] = x as f32;
                    i += stride;
                }
            }
        } else {
            while i < 108 {
                out[start + i] = if self.read(1) == 0 {
                    0.0
                } else if self.read(1) == 0 {
                    -2.0
                } else {
                    2.0
                };
                i += stride;
            }
        }
    }

    /// 0x733BC8.
    fn lpc(&self) -> [f32; 12] {
        let rc = &self.rc;
        let mut t1 = [0.0f32; 12];
        let mut t2 = [0.0f32; 12];
        let mut lpc = [0.0f32; 12];
        for i in (0..=10).rev() {
            t2[1 + i] = rc[i];
        }
        t2[0] = 1.0;
        for i in 0..12 {
            let mut x = -t2[11] * rc[11];
            for j in (0..=10).rev() {
                x -= t2[j] * rc[j];
                t2[j + 1] = x * rc[j] + t2[j];
            }
            t1[i] = x;
            t2[0] = x;
            for j in 0..i {
                x -= t1[i - 1 - j] * lpc[j];
            }
            lpc[i] = x;
        }
        lpc
    }

    /// 0x733C54.
    fn synthesis(&mut self, offset: usize, blocks: usize) {
        let lpc = self.lpc();
        let mut p = 324 + offset;
        for _ in 0..blocks {
            for j in 0..12 {
                let mut x = self.buf[p];
                for k in 0..j {
                    x += lpc[k] * self.synth[k + 12 - j];
                }
                for k in j..12 {
                    x += lpc[k] * self.synth[k - j];
                }
                self.synth[11 - j] = x;
                self.buf[p] = x;
                p += 1;
            }
        }
    }

    /// One frame (0x7343F3), then the rev-3 tail (0x734798): 432 samples
    /// at 16-bit scale.
    pub fn frame(&mut self) -> &[f32] {
        let mut delta = [0.0f32; 12];
        let mut multipulse = false;
        for (i, d) in delta.iter_mut().enumerate() {
            let idx = if i == 0 {
                let v = self.read(6);
                multipulse = v < self.threshold;
                v as usize
            } else if i < 4 {
                self.read(6) as usize
            } else {
                16 + self.read(5) as usize
            };
            *d = (RC[idx] - self.rc[i]) * 0.25;
        }
        for i in 0..4 {
            let lag = self.read(8) as usize;
            let pitch_gain = self.read(4) as f32 * 0.06666667;
            let mut gain = self.gains[self.read(6) as usize];
            let mut exc = [0.0f32; 118];
            if !self.reduced_bw {
                self.excitation(multipulse, &mut exc, 5, 1);
            } else {
                let align = self.read(1) as usize;
                let zero = self.read(1) != 0;
                self.excitation(multipulse, &mut exc, 5 + align, 2);
                let base = 5 + (1 - align);
                if zero {
                    for j in 0..54 {
                        exc[base + 2 * j] = 0.0;
                    }
                } else {
                    for j in (0..108).step_by(2) {
                        let p = base + j;
                        let at = |k: isize| exc.get((p as isize + k) as usize).copied().unwrap_or(0.0);
                        exc[p] = (at(-1) + at(1)) * 0.59738594
                            + ((at(-5) + at(5)) * 0.01803268 - (at(-3) + at(3)) * 0.11459156);
                    }
                    gain *= 0.5;
                }
            }
            for j in 0..108 {
                // With a lag over 216 the first subframe reads below the
                // codebook: the state before it (+0x144 the history, +0x114
                // the coefficients, +0x14 the gains), as the exe does.
                let src = (108 * i + 216 + j) as isize - lag as isize;
                let past = if src >= 0 {
                    self.buf[src as usize]
                } else {
                    let k = (-src) as usize;
                    if k <= 12 {
                        self.synth[12 - k]
                    } else if k <= 24 {
                        self.rc[24 - k]
                    } else {
                        self.gains[88 - k]
                    }
                };
                self.buf[324 + 108 * i + j] = pitch_gain * past + gain * exc[5 + j];
            }
        }
        self.buf.copy_within(432..756, 0);
        for i in 0..4 {
            for j in 0..12 {
                self.rc[j] += delta[j];
            }
            self.synthesis(12 * i, if i < 3 { 1 } else { 33 });
        }
        if self.rev3 {
            // The byte in the bit buffer is the next marker.
            let mut q = self.p - 1;
            if self.pcm_next {
                let be = |o: usize| {
                    i16::from_be_bytes([self.d.get(o).copied().unwrap_or(0), self.d.get(o + 1).copied().unwrap_or(0)])
                };
                let off = be(q) as usize;
                let n = be(q + 2).max(0) as usize;
                for k in 0..n {
                    if let Some(s) = self.buf.get_mut(324 + off + k) {
                        *s = be(q + 4 + 2 * k) as f32;
                    }
                }
                q += 4 + 2 * n;
                self.p = q + 1;
            }
            self.pcm_next = self.d.get(q).copied() == Some(0xEE);
            self.p = q + 1;
            self.start_bits();
        }
        &self.buf[324..]
    }

    /// Bytes consumed so far.
    pub fn position(&self) -> usize {
        self.p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn codebook_rows() {
        assert_eq!(codebook(0, 0x7F), 0);
        assert_eq!(codebook(0, 0xFF), 2);
        assert_eq!(codebook(1, 0x3F), 1);
        assert_eq!(codebook(1, 0x7F), 3);
    }
}
