//! EALayer3 (codec 0x17, the audio library's decoder class 6: 0x728DF9
//! → 0x8B4850, set up by 0x72428B): MPEG audio layer III granules in
//! EA's own framing, the music's format (`pfdata/MusicSFx.mus`). The
//! decoder is a libmad-style one (its synthesis window 0x7D1600, the
//! Huffman trees 0x7D0C30, the alias coefficients 0x7CF620..) fed one
//! granule at a time.
//!
//! * **Blocks** ("SCDl", see [`super::schl`]): +8 the block's samples,
//!   +0xC one u32 per channel, then the EA frames.
//! * **EA frame** (version 1): a byte (0 or 0xEE), then bit-packed MSB
//!   first: MPEG version (2: 3 = MPEG-1), rate index (2), channel mode
//!   (2), mode extension (2), the granule index (1), with MPEG-1 and
//!   granule 1 the scfsi (4 a channel), then per channel the granule's
//!   part2_3 length (12) and the rest of its side info (47 bits MPEG-1,
//!   51 MPEG-2), then each channel's main data (no bit reservoir), padded
//!   to a byte. A 0xEE frame then has a big-endian sample offset and
//!   count, and that many interleaved big-endian samples, played after
//!   the granule's; with an empty granule (header 0) the offset follows
//!   the flag directly.
//! * The granule is decoded as layer III does: scalefactors (MPEG-1),
//!   Huffman big values and count1 quads, requantisation, M/S and
//!   intensity stereo, short-block reordering, alias reduction, IMDCT with
//!   overlap, frequency inversion and the polyphase synthesis.

use super::mp3_tables::{HUFF, LINBITS, WINDOW_ROWS};
use super::schl::{Header, Pcm};
use std::f32::consts::PI;

/// The decoder's output as the original makes it (measured under
/// Wine, docs/native-fixes/N-AUD2.tsv): on the music's MMX path the
/// granule's 576 subband floats are turned into 16-bit values (0x72FD95:
/// × 16384.0, `fistp`, low 16 bits — the largest seen was 22389, so that
/// wrap is not reached) and the fixed-point polyphase synthesis 0x730C66
/// (pmaddwd with the window 0x7D11E0, >> 13) packs the samples with
/// `packssdw`: the PCM **saturates** at the 16-bit limits, it never wraps.
/// The clone's float synthesis has the original's scale (native / clone
/// 0.99997 over 16 s of song 0, residual 7 LSB rms from the fixed-point
/// arithmetic, which is not ported), so its sample is rounded to a 16-bit
/// value (to nearest; the original's own rounding is that fixed-point
/// path) and saturated at −32768 / 32767.
pub fn pcm16(v: f32) -> f32 {
    if v.is_nan() {
        return 0.0;
    }
    (v as f64 * 32768.0).round_ties_even().clamp(-32768.0, 32767.0) as f32 / 32768.0
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn bit(&mut self) -> u32 {
        let b = self.d.get(self.pos >> 3).map_or(0, |&b| (b >> (7 - (self.pos & 7))) & 1);
        self.pos += 1;
        b as u32
    }

    fn read(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.bit();
        }
        v
    }
}

#[derive(Clone, Copy, Default)]
struct Side {
    part2_3: u32,
    big_values: u32,
    global_gain: u32,
    scalefac_compress: u32,
    window_switching: bool,
    block_type: u32,
    mixed: bool,
    table: [u32; 3],
    subblock_gain: [u32; 3],
    region0: u32,
    region1: u32,
    preflag: bool,
    scalefac_scale: bool,
    count1table: bool,
}

const PRETAB: [u32; 22] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 3, 2, 0];
const SLEN: [[u32; 16]; 2] =
    [[0, 0, 0, 0, 3, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4], [0, 1, 2, 3, 0, 1, 2, 3, 1, 2, 3, 1, 2, 3, 2, 3]];
/// Scalefactor band starts (MPEG-1 44.1 / 48 / 32 kHz), long and short.
const SFB_LONG: [[usize; 23]; 3] = [
    [0, 4, 8, 12, 16, 20, 24, 30, 36, 44, 52, 62, 74, 90, 110, 134, 162, 196, 238, 288, 342, 418, 576],
    [0, 4, 8, 12, 16, 20, 24, 30, 36, 42, 50, 60, 72, 88, 106, 128, 156, 190, 230, 276, 330, 384, 576],
    [0, 4, 8, 12, 16, 20, 24, 30, 36, 44, 54, 66, 82, 102, 126, 156, 194, 240, 296, 364, 448, 550, 576],
];
const SFB_SHORT: [[usize; 14]; 3] = [
    [0, 4, 8, 12, 16, 22, 30, 40, 52, 66, 84, 106, 136, 192],
    [0, 4, 8, 12, 16, 22, 28, 38, 50, 64, 80, 100, 126, 192],
    [0, 4, 8, 12, 16, 22, 30, 42, 58, 78, 104, 138, 180, 192],
];

/// The synthesis window D[0..512] from the exe's rows.
fn window() -> Vec<f32> {
    let mut d = vec![0.0f32; 512];
    for i in 0..512 {
        let (j, k) = (i / 32, i % 32);
        if k <= 16 {
            d[i] = -WINDOW_ROWS[k][j] / 32768.0;
        }
    }
    for i in 0..512 {
        if i % 32 > 16 {
            d[i] = -d[512 - i];
        }
    }
    d
}

/// The per-channel state kept between granules.
struct Channel {
    scalefac: [u32; 22],
    overlap: [[f32; 18]; 32],
    v: Vec<f32>,
    v_off: usize,
}

impl Channel {
    fn new() -> Channel {
        Channel { scalefac: [0; 22], overlap: [[0.0; 18]; 32], v: vec![0.0; 1024], v_off: 0 }
    }
}

pub struct Decoder {
    ch: [Channel; 2],
    d: Vec<f32>,
    /// cos((16 + i)(2k + 1)π/64).
    n: Vec<f32>,
    imdct36: Vec<f32>,
    imdct12: Vec<f32>,
    win: [[f32; 36]; 4],
    cs: [f32; 8],
    ca: [f32; 8],
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder::new()
    }
}

impl Decoder {
    pub fn new() -> Decoder {
        let mut n = vec![0.0; 64 * 32];
        for i in 0..64 {
            for k in 0..32 {
                n[i * 32 + k] = ((16 + i) as f32 * (2 * k + 1) as f32 * PI / 64.0).cos();
            }
        }
        let mut imdct36 = vec![0.0; 36 * 18];
        for i in 0..36 {
            for k in 0..18 {
                imdct36[i * 18 + k] = (PI / 72.0 * (2 * i + 1 + 18) as f32 * (2 * k + 1) as f32).cos();
            }
        }
        let mut imdct12 = vec![0.0; 12 * 6];
        for i in 0..12 {
            for k in 0..6 {
                imdct12[i * 6 + k] = (PI / 24.0 * (2 * i + 1 + 6) as f32 * (2 * k + 1) as f32).cos();
            }
        }
        let mut win = [[0.0f32; 36]; 4];
        for i in 0..36 {
            win[0][i] = (PI / 36.0 * (i as f32 + 0.5)).sin();
        }
        for i in 0..36 {
            win[1][i] = match i {
                0..=17 => (PI / 36.0 * (i as f32 + 0.5)).sin(),
                18..=23 => 1.0,
                24..=29 => (PI / 12.0 * (i as f32 - 18.0 + 0.5)).sin(),
                _ => 0.0,
            };
            win[3][i] = match i {
                0..=5 => 0.0,
                6..=11 => (PI / 12.0 * (i as f32 - 6.0 + 0.5)).sin(),
                12..=17 => 1.0,
                _ => (PI / 36.0 * (i as f32 + 0.5)).sin(),
            };
        }
        for i in 0..12 {
            win[2][i] = (PI / 12.0 * (i as f32 + 0.5)).sin();
        }
        let c = [-0.6f32, -0.535, -0.33, -0.185, -0.095, -0.041, -0.0142, -0.0037];
        let mut cs = [0.0; 8];
        let mut ca = [0.0; 8];
        for i in 0..8 {
            let s = (1.0 + c[i] * c[i]).sqrt();
            cs[i] = 1.0 / s;
            ca[i] = c[i] / s;
        }
        Decoder { ch: [Channel::new(), Channel::new()], d: window(), n, imdct36, imdct12, win, cs, ca }
    }

    /// Huffman pair from tree `t`.
    fn pair(b: &mut Bits, t: &[i16]) -> (i32, i32) {
        let mut i = 0usize;
        loop {
            let v = t.get(i).copied().unwrap_or(0);
            i += 1;
            if v >= 0 {
                return ((v >> 4) as i32, (v & 15) as i32);
            }
            if b.bit() == 1 {
                i += (-v) as usize;
            }
        }
    }

    /// The count1 quads: table A's tree (ISO table B.7 as the libraries
    /// carry it) or table B (4 bits, inverted).
    fn quad(b: &mut Bits, table_b: bool) -> u32 {
        const A: [i16; 31] = [
            -29, -21, -13, -7, -3, -1, 11, 15, -1, 13, 14, -3, -1, 7, 5, 9, -3, -1, 6, 3, -1, 10, 12, -3, -1, 2, 1, -1,
            4, 8, 0,
        ];
        if table_b {
            return 15 - b.read(4);
        }
        let mut i = 0usize;
        loop {
            let v = A[i];
            i += 1;
            if v >= 0 {
                return v as u32;
            }
            if b.bit() == 1 {
                i += (-v) as usize;
            }
        }
    }

    /// Decode one channel's granule spectrum (scalefactors + Huffman +
    /// requantisation) into `xr`; returns the count of nonzero lines.
    #[allow(clippy::too_many_arguments)]
    fn spectrum(
        &mut self,
        b: &mut Bits,
        ch: usize,
        s: &Side,
        scfsi: u32,
        gr: u32,
        rate: usize,
        xr: &mut [f32; 576],
        sf_short: &mut [[u32; 3]; 13],
    ) -> usize {
        let start = b.pos;
        let (slen1, slen2) = (SLEN[0][s.scalefac_compress as usize & 15], SLEN[1][s.scalefac_compress as usize & 15]);
        let short = s.window_switching && s.block_type == 2;
        let sf = &mut self.ch[ch].scalefac;
        *sf_short = [[0; 3]; 13];
        if short {
            let mut first_short = 0;
            if s.mixed {
                for v in sf.iter_mut().take(8) {
                    *v = b.read(slen1);
                }
                first_short = 3;
            }
            for sfb in first_short..12 {
                let n = if sfb < 6 { slen1 } else { slen2 };
                for w in 0..3 {
                    sf_short[sfb][w] = b.read(n);
                }
            }
        } else {
            let groups = [(0usize, 6usize), (6, 11), (11, 16), (16, 21)];
            for (g, &(a, z)) in groups.iter().enumerate() {
                let keep = gr == 1 && (scfsi >> (3 - g)) & 1 == 1;
                if !keep {
                    let n = if g < 2 { slen1 } else { slen2 };
                    for v in sf.iter_mut().take(z).skip(a) {
                        *v = b.read(n);
                    }
                }
            }
            sf[21] = 0;
        }
        let sf = *sf;
        let end = start + s.part2_3 as usize;
        // Huffman.
        let mut ints = [0i32; 576];
        let big = (s.big_values as usize * 2).min(576);
        let (r1, r2) = if s.window_switching {
            (36usize, 576usize)
        } else {
            let l = &SFB_LONG[rate];
            (l[(s.region0 as usize + 1).min(22)], l[(s.region0 as usize + s.region1 as usize + 2).min(22)])
        };
        let mut i = 0;
        while i < big {
            let t = if i < r1 {
                s.table[0]
            } else if i < r2 {
                s.table[1]
            } else {
                s.table[2]
            } as usize;
            let Some(tree) = HUFF[t].as_deref() else {
                i += 2;
                continue;
            };
            let lin = if t >= 16 { LINBITS[t - 16] as u32 } else { 0 };
            let (mut x, mut y) = Self::pair(b, tree);
            if lin > 0 && x == 15 {
                x += b.read(lin) as i32;
            }
            if x != 0 && b.bit() == 1 {
                x = -x;
            }
            if lin > 0 && y == 15 {
                y += b.read(lin) as i32;
            }
            if y != 0 && b.bit() == 1 {
                y = -y;
            }
            ints[i] = x;
            ints[i + 1] = y;
            i += 2;
        }
        while i + 4 <= 576 && b.pos < end {
            let q = Self::quad(b, s.count1table);
            let mut vals = [((q >> 3) & 1) as i32, ((q >> 2) & 1) as i32, ((q >> 1) & 1) as i32, (q & 1) as i32];
            for v in vals.iter_mut() {
                if *v != 0 && b.bit() == 1 {
                    *v = -*v;
                }
            }
            if b.pos > end {
                break;
            }
            ints[i..i + 4].copy_from_slice(&vals);
            i += 4;
        }
        b.pos = end;
        let nonzero = i;
        // Requantisation.
        let sfm = if s.scalefac_scale { 1.0 } else { 0.5 };
        let gain = 2f32.powf((s.global_gain as f32 - 210.0) / 4.0);
        let pow43 = |v: i32| (v.unsigned_abs() as f32).powf(4.0 / 3.0).copysign(v as f32);
        xr.fill(0.0);
        let long_end = if short { if s.mixed { 36 } else { 0 } } else { 576 };
        let mut sfb = 0;
        for (k, x) in xr.iter_mut().enumerate().take(long_end.min(nonzero.max(0))) {
            while sfb < 21 && k >= SFB_LONG[rate][sfb + 1] {
                sfb += 1;
            }
            let p = if s.preflag { PRETAB[sfb] } else { 0 };
            let e = -sfm * (sf[sfb] + p) as f32;
            *x = pow43(ints[k]) * gain * 2f32.powf(e);
        }
        if short {
            let first = if s.mixed { 3 } else { 0 };
            let ss = &SFB_SHORT[rate];
            for sfb in first..13 {
                let width = ss[sfb + 1] - ss[sfb];
                for w in 0..3 {
                    let g = gain
                        * 2f32.powf(-2.0 * s.subblock_gain[w] as f32)
                        * 2f32.powf(-sfm * sf_short[sfb.min(12)][w] as f32);
                    for j in 0..width {
                        let k = 3 * ss[sfb] + w * width + j;
                        if k < 576 {
                            xr[k] = pow43(ints[k]) * g;
                        }
                    }
                }
            }
        }
        nonzero
    }

    /// Reorder short blocks to window-interleaved order.
    fn reorder(xr: &mut [f32; 576], s: &Side, rate: usize) {
        if !(s.window_switching && s.block_type == 2) {
            return;
        }
        let ss = &SFB_SHORT[rate];
        let first = if s.mixed { 3 } else { 0 };
        let src = *xr;
        for sfb in first..13 {
            let width = ss[sfb + 1] - ss[sfb];
            for w in 0..3 {
                for j in 0..width {
                    let from = 3 * ss[sfb] + w * width + j;
                    let to = 3 * ss[sfb] + 3 * j + w;
                    if from < 576 && to < 576 {
                        xr[to] = src[from];
                    }
                }
            }
        }
    }

    fn antialias(&self, xr: &mut [f32; 576], s: &Side) {
        let short = s.window_switching && s.block_type == 2;
        let limit = if short { if s.mixed { 2 } else { return } } else { 32 };
        for sb in 1..limit {
            for i in 0..8 {
                let (a, b) = (xr[18 * sb - 1 - i], xr[18 * sb + i]);
                xr[18 * sb - 1 - i] = a * self.cs[i] - b * self.ca[i];
                xr[18 * sb + i] = b * self.cs[i] + a * self.ca[i];
            }
        }
    }

    /// IMDCT + overlap + frequency inversion: 18 slots × 32 subbands.
    fn hybrid(&mut self, ch: usize, xr: &[f32; 576], s: &Side, out: &mut [[f32; 32]; 18]) {
        for sb in 0..32 {
            let x = &xr[18 * sb..18 * sb + 18];
            let bt = if s.window_switching && !(s.mixed && sb < 2) { s.block_type } else { 0 };
            let mut y = [0.0f32; 36];
            if bt == 2 {
                for w in 0..3 {
                    for i in 0..12 {
                        let mut v = 0.0;
                        for k in 0..6 {
                            v += x[3 * k + w] * self.imdct12[i * 6 + k];
                        }
                        y[6 + 6 * w + i] += v * self.win[2][i];
                    }
                }
            } else {
                let w = &self.win[bt as usize];
                for i in 0..36 {
                    let mut v = 0.0;
                    for k in 0..18 {
                        v += x[k] * self.imdct36[i * 18 + k];
                    }
                    y[i] = v * w[i];
                }
            }
            let ov = &mut self.ch[ch].overlap[sb];
            for i in 0..18 {
                let mut v = y[i] + ov[i];
                if sb & 1 == 1 && i & 1 == 1 {
                    v = -v;
                }
                out[i][sb] = v;
                ov[i] = y[18 + i];
            }
        }
    }

    /// The polyphase synthesis of one slot: 32 samples.
    fn synth(&mut self, ch: usize, s: &[f32; 32], out: &mut [f32]) {
        let c = &mut self.ch[ch];
        c.v_off = (c.v_off + 1024 - 64) % 1024;
        for i in 0..64 {
            let row = &self.n[i * 32..i * 32 + 32];
            let mut v = 0.0;
            for k in 0..32 {
                v += row[k] * s[k];
            }
            c.v[(c.v_off + i) % 1024] = v;
        }
        for j in 0..32 {
            let mut sum = 0.0;
            for i in 0..8 {
                let a = c.v[(c.v_off + i * 128 + j) % 1024];
                let b = c.v[(c.v_off + i * 128 + 96 + j) % 1024];
                sum += a * self.d[i * 64 + j] + b * self.d[i * 64 + 32 + j];
            }
            out[j] = pcm16(sum);
        }
    }

    /// Decode one EA frame at `d`; appends its samples (interleaved, as
    /// many channels as the stream has) and returns the bytes it used.
    pub fn frame(&mut self, d: &[u8], channels: usize, out: &mut Vec<f32>) -> Option<usize> {
        let flag = *d.first()?;
        if flag != 0 && flag != 0xEE {
            return None;
        }
        let mut b = Bits { d: &d[1..], pos: 0 };
        let version = b.read(2);
        let rate_i = b.read(2) as usize;
        let mode = b.read(2);
        let ext = b.read(2);
        if version == 0 && rate_i == 0 && mode == 0 && ext == 0 {
            // No granule: a plain frame ends the block; a 0xEE one is
            // only its PCM part, {offset, count, samples} right after the
            // flag (the first block of a song: 1152 + 47 = its 1199).
            if flag != 0xEE {
                return None;
            }
            let be =
                |o: usize| u16::from_be_bytes([d.get(o).copied().unwrap_or(0), d.get(o + 1).copied().unwrap_or(0)]);
            let n = be(3) as usize;
            for k in 0..n * channels {
                out.push(be(5 + 2 * k) as i16 as f32 / 32768.0);
            }
            return Some(5 + 2 * n * channels);
        }
        let mpeg1 = version == 3;
        if !mpeg1 || rate_i > 2 {
            return None;
        }
        let nch = if mode == 3 { 1 } else { 2 };
        let gr = b.read(1);
        let mut scfsi = [0u32; 2];
        if gr == 1 {
            for s in scfsi.iter_mut().take(nch) {
                *s = b.read(4);
            }
        }
        let mut side = [Side::default(); 2];
        for s in side.iter_mut().take(nch) {
            s.part2_3 = b.read(12);
            s.big_values = b.read(9);
            s.global_gain = b.read(8);
            s.scalefac_compress = b.read(4);
            s.window_switching = b.read(1) == 1;
            if s.window_switching {
                s.block_type = b.read(2);
                s.mixed = b.read(1) == 1;
                s.table[0] = b.read(5);
                s.table[1] = b.read(5);
                for g in s.subblock_gain.iter_mut() {
                    *g = b.read(3);
                }
                s.region0 = if s.block_type == 2 && !s.mixed { 8 } else { 7 };
                s.region1 = 20 - s.region0;
            } else {
                for t in s.table.iter_mut() {
                    *t = b.read(5);
                }
                s.region0 = b.read(4);
                s.region1 = b.read(3);
            }
            s.preflag = b.read(1) == 1;
            s.scalefac_scale = b.read(1) == 1;
            s.count1table = b.read(1) == 1;
        }
        let mut xr = [[0.0f32; 576]; 2];
        let mut nonzero = [0usize; 2];
        let mut sfs = [[[0u32; 3]; 13]; 2];
        for c in 0..nch {
            let mut sf = [[0u32; 3]; 13];
            nonzero[c] = self.spectrum(&mut b, c, &side[c], scfsi[c], gr, rate_i, &mut xr[c], &mut sf);
            sfs[c] = sf;
        }
        // Stereo.
        if nch == 2 && mode == 1 {
            let is_on = ext & 1 == 1;
            let ms_on = ext & 2 == 2;
            let is_bound = if is_on { nonzero[1] } else { 576 };
            if ms_on {
                let r = std::f32::consts::FRAC_1_SQRT_2;
                for i in 0..is_bound.min(576) {
                    let (m, s) = (xr[0][i], xr[1][i]);
                    xr[0][i] = (m + s) * r;
                    xr[1][i] = (m - s) * r;
                }
            }
            if is_on {
                let sf_r = self.ch[1].scalefac;
                let long = !(side[1].window_switching && side[1].block_type == 2);
                for i in is_bound..576 {
                    let pos = if long {
                        let mut sfb = 0;
                        while sfb < 21 && i >= SFB_LONG[rate_i][sfb + 1] {
                            sfb += 1;
                        }
                        if sfb >= 21 { 7 } else { sf_r[sfb] }
                    } else {
                        let ss = &SFB_SHORT[rate_i];
                        let mut sfb = 0;
                        while sfb < 12 && i >= 3 * ss[sfb + 1] {
                            sfb += 1;
                        }
                        let w = ((i - 3 * ss[sfb]) / (ss[sfb + 1] - ss[sfb]).max(1)).min(2);
                        if sfb >= 12 { 7 } else { sfs[1][sfb][w] }
                    };
                    if pos != 7 {
                        let t = (pos as f32 * PI / 12.0).tan();
                        let l = xr[0][i];
                        xr[0][i] = l * t / (1.0 + t);
                        xr[1][i] = l / (1.0 + t);
                    }
                }
            }
        }
        let mut pcm = vec![[0.0f32; 576]; nch];
        for c in 0..nch {
            Self::reorder(&mut xr[c], &side[c], rate_i);
            self.antialias(&mut xr[c], &side[c]);
            let mut slots = [[0.0f32; 32]; 18];
            self.hybrid(c, &xr[c], &side[c], &mut slots);
            for (t, slot) in slots.iter().enumerate() {
                let mut o = [0.0f32; 32];
                self.synth(c, slot, &mut o);
                pcm[c][t * 32..t * 32 + 32].copy_from_slice(&o);
            }
        }
        // The frame's bytes: the header and side info, the main data,
        // padding to a byte.
        let used = b.pos.div_ceil(8);
        let mut size = 1 + used;
        let mut tail = Vec::new();
        if flag == 0xEE {
            let be =
                |o: usize| u16::from_be_bytes([d.get(o).copied().unwrap_or(0), d.get(o + 1).copied().unwrap_or(0)]);
            let n = be(size + 2) as usize;
            size += 4;
            for k in 0..n * nch {
                tail.push(be(size + 2 * k) as i16 as f32 / 32768.0);
            }
            size += 2 * n * nch;
        }
        for i in 0..576 {
            for c in 0..channels {
                out.push(pcm[c.min(nch - 1)][i]);
            }
        }
        // The PCM part follows the granule's samples (**inferred** from
        // the blocks' sample counts; its offset field is not used).
        for k in 0..tail.len() / nch {
            for c in 0..channels {
                out.push(tail[k * nch + c.min(nch - 1)]);
            }
        }
        Some(size)
    }
}

/// Decode a whole EALayer3 SCHl stream.
pub fn decode_stream(d: &[u8], h: &Header) -> Option<Pcm> {
    let ch = h.channels.max(1) as usize;
    let mut dec = Decoder::new();
    let mut out = Vec::new();
    for (o, size) in super::schl::blocks(d, h) {
        let mut p = o + 0xC + 4 * ch;
        let end = o + size;
        while p < end {
            match dec.frame(&d[p..end], ch, &mut out) {
                Some(n) if n > 0 => {
                    p += n;
                }
                _ => break,
            }
        }
    }
    out.truncate(h.samples as usize * ch);
    Some(Pcm { rate: h.rate, channels: ch as u8, samples: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One spectral line through alias reduction, IMDCT, overlap and the
    /// synthesis comes out as a unit tone near line · fs / 1152 (a constant
    /// line repeats each granule, so it lands on a harmonic of fs / 576).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn line_becomes_a_tone() {
        for line in [10usize, 100, 300] {
            let mut dec = Decoder::new();
            let side = Side::default();
            let mut out = Vec::new();
            for _ in 0..20 {
                let mut xr = [0.0f32; 576];
                xr[line] = 1.0;
                dec.antialias(&mut xr, &side);
                let mut slots = [[0.0f32; 32]; 18];
                dec.hybrid(0, &xr, &side, &mut slots);
                for slot in slots.iter() {
                    let mut o = [0.0f32; 32];
                    dec.synth(0, slot, &mut o);
                    out.extend_from_slice(&o);
                }
            }
            let x = &out[576 * 4..];
            let f = (line as f32 + 0.5) / 1152.0;
            let (mut re, mut im, mut e) = (0.0f64, 0.0f64, 0.0f64);
            for (n, &v) in x.iter().enumerate() {
                let p = 2.0 * std::f64::consts::PI * f as f64 * n as f64;
                re += v as f64 * p.cos();
                im += v as f64 * p.sin();
                e += (v * v) as f64;
            }
            let tone = 2.0 * (re * re + im * im) / x.len() as f64;
            let mut best = (0.0f64, 0usize);
            for b in 0..576 {
                let f = b as f64 / 1152.0;
                let (mut r2, mut i2) = (0.0f64, 0.0f64);
                for (n, &v) in x.iter().enumerate() {
                    let p = 2.0 * std::f64::consts::PI * f * n as f64;
                    r2 += v as f64 * p.cos();
                    i2 += v as f64 * p.sin();
                }
                let m = r2 * r2 + i2 * i2;
                if m > best.0 {
                    best = (m, b);
                }
            }
            eprintln!("line {line}: energy {:.4} tone {:.4} peak bin {}", e, tone, best.1);
            let _ = tone;
            assert!((best.1 as isize - line as isize).abs() <= 1, "line {line}: the tone is at bin {}", best.1);
            assert!((e / x.len() as f64 - 0.5).abs() < 0.05, "line {line}: power {}", e / x.len() as f64);
        }
    }

    #[test]
    fn the_output_saturates_as_the_original_s_packssdw() {
        // 0x730C66 packssdw: 16-bit values, saturated (measured under
        // Wine: 1738 of 522432 samples of song 0 at the limit, none
        // wrapped); the clone rounds its float to nearest.
        assert_eq!(pcm16(0.5 / 32768.0), 0.0);
        assert_eq!(pcm16(1.5 / 32768.0), 2.0 / 32768.0);
        assert_eq!(pcm16(-2.5 / 32768.0), -2.0 / 32768.0);
        assert_eq!(pcm16(32767.0 / 32768.0), 32767.0 / 32768.0);
        assert_eq!(pcm16(1.0), 32767.0 / 32768.0);
        assert_eq!(pcm16(1.5), 32767.0 / 32768.0);
        assert_eq!(pcm16(-1.2), -1.0);
        assert_eq!(pcm16(f32::NAN), 0.0);
        assert_eq!(pcm16(1.0e6), 32767.0 / 32768.0);
    }
}
