use super::dsp::*;
use std::path::Path;

/// The mixer's chunk (0x7295A8) and the bus buffer's size (0x400 bytes).
pub const BLOCK: usize = 256;
/// The idle limit of the process hook (0x72656C).
pub const IDLE_LIMIT: u32 = 3000;

/// SFXObj_Reverb's presets (0x7ED608, `sound\fxedit\<name>.fx`).
pub const PRESETS: [&str; 12] = [
    "Car_Show_Sml",
    "Car_Show_Bass",
    "Car_Show_Bass2",
    "Garage",
    "Garage_sml",
    "Simple_Tunnel",
    "Simple_Tunnel_Sml",
    "Hills_Close",
    "Hills",
    "City_Open",
    "City_Dense",
    "Alley",
];

/// One module's kind and state.
#[derive(Clone, Debug)]
enum Kind {
    Input,
    /// {state y₁, feedback g(1 − a), input g·a}
    OnePole {
        y: f32,
        b: f32,
        a: f32,
    },
    /// {history x(n−1)..x(n−8), coefficients c0..c4}
    Fir {
        h: [f32; 8],
        c: [f32; 5],
    },
    /// The resonator: bypassed (out = in) when sr · 2/π ≤ bandwidth, f ≤ 0
    /// or sr / 2 ≤ f.
    Resonator {
        bypass: bool,
        a: f32,
        r: f32,
        c: f32,
        y1: f32,
        y2: f32,
    },
    Gain(f32),
    Allpass {
        buf: Vec<f32>,
        pos: usize,
        g: f32,
    },
    Sum,
    /// {served once, compute on the next read}
    Split {
        served: bool,
        need: bool,
    },
}

#[derive(Clone, Debug)]
struct Module {
    kind: Kind,
    in1: Option<usize>,
    in2: Option<usize>,
}

/// A loaded preset (0x726DB3): its modules, output slots and the
/// process hook's idle count.
#[derive(Clone, Debug)]
pub struct Graph {
    mods: Vec<Module>,
    /// Each module's output for the current chunk (a splitter's is its cache).
    bufs: Vec<Vec<f32>>,
    input: usize,
    /// Output slots 1 and 2 (channels 0 and 1 of the library).
    pub(super) outs: [Option<usize>; 2],
    /// The FX's channel gains (+0x68, 1.0).
    pub gains: [f32; 2],
    idle: u32,
}

fn be16(b: &[u8], at: usize) -> Option<i32> {
    Some(i16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]) as i32)
}

fn be32(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// 0x818A08: a module record's size by kind.
const RECORD: [usize; 10] = [4, 8, 4, 4, 8, 8, 4, 8, 4, 4];

impl Graph {
    /// Parse a preset for two output channels at `rate` Hz (0x726DB3 /
    /// 0x7268BF). `None` for a file it cannot read.
    pub fn parse(file: &[u8], rate: u32) -> Option<Graph> {
        if file.get(0..2)? != b"FX" {
            return None;
        }
        let sr = rate as i32;
        let h = |i: usize| be32(file, i * 4);
        // The largest config c ≤ 2 with modules.
        let cfg = (1..=2).rev().find(|&c| h(1 + c).is_some_and(|n| n > 0))?;
        let mut p = 0x38;
        for c in 1..cfg {
            for _ in 0..h(1 + c)? {
                p += *RECORD.get(*file.get(p)? as usize)?;
            }
            p += 12 * h(7 + c)?;
        }
        let count = h(1 + cfg)?;
        let links = h(7 + cfg)?;
        let mut mods = Vec::with_capacity(count);
        let mut outs = [None; 2];
        let mut input = None;
        let slot = |ch: usize| -> Option<usize> {
            // 0x8188DD for (c, N = 2): c 2 → channel k at slot k; c 1 → slot 1.
            match (cfg, ch) {
                (2, 1 | 2) | (1, 1) => Some(ch - 1),
                _ => None,
            }
        };
        for i in 0..count {
            let kind_byte = *file.get(p)?;
            let rec = file.get(p..p + *RECORD.get(kind_byte as usize)?)?;
            let (p2, p4, p6) = (be16(rec, 2), be16(rec, 4), be16(rec, 6));
            let kind = match kind_byte {
                0 => {
                    input.get_or_insert(i);
                    Kind::Input
                }
                1 => {
                    let (f, g) = (p4?, p6?);
                    let a = (2 * (f << 8)) as f64 / (sr << 8) as f64;
                    let a32 = a as f32;
                    let one = (1.0 - a) as f32;
                    Kind::OnePole {
                        y: 0.0,
                        a: (g as f64 * a32 as f64 / 256.0) as f32,
                        b: (g as f64 * one as f64 / 256.0) as f32,
                    }
                }
                2 => Kind::Fir { h: [0.0; 8], c: fir_design(2, 0.0, (2.0 * p2? as f64 / sr as f64) as f32) },
                3 => Kind::Fir { h: [0.0; 8], c: fir_design(3, (2.0 * p2? as f64 / sr as f64) as f32, 0.0) },
                4 => Kind::Fir {
                    h: [0.0; 8],
                    c: fir_design(4, (2.0 * p4? as f64 / sr as f64) as f32, (2.0 * p6? as f64 / sr as f64) as f32),
                },
                5 => resonator(sr, p2?, p4?, p6?),
                6 => Kind::Gain((p2? as f64 / 256.0) as f32),
                7 => {
                    let g = ((p2? >> 1) as f64 * (1.0f32 / 127.0) as f64) as f32;
                    Kind::Allpass { buf: vec![0.0; allpass_len(sr, p4?)], pos: 0, g }
                }
                8 => Kind::Sum,
                9 => Kind::Split { served: false, need: true },
                _ => return None,
            };
            let ch = rec[1] as usize;
            let chans = if kind_byte == 9 { [ch & 0xF, ch >> 4] } else { [ch, 0] };
            for c in chans.into_iter().filter(|&c| c != 0) {
                if let Some(s) = slot(c) {
                    outs[s] = Some(i);
                }
            }
            mods.push(Module { kind, in1: None, in2: None });
            p += rec.len();
        }
        for _ in 0..links {
            let l = file.get(p..p + 12)?;
            let (from, to) = (be32(l, 0)?, be32(l, 4)?);
            if from < count && to < count {
                // 0x72A26D: an input already taken refuses the link.
                let m = &mut mods[to];
                match l[9] {
                    1 if m.in1.is_none() => m.in1 = Some(from),
                    2 if m.in2.is_none() => m.in2 = Some(from),
                    _ => {}
                }
            }
            p += 12;
        }
        Some(Graph {
            bufs: vec![vec![0.0; BLOCK]; mods.len()],
            mods,
            input: input?,
            outs,
            gains: [1.0; 2],
            idle: IDLE_LIMIT,
        })
    }

    /// The preset `index` from `SOUND/FXEDIT` (names compared
    /// case-insensitively, as Windows opens them).
    pub fn load(root: &Path, index: usize, rate: u32) -> Option<Graph> {
        let want = format!("{}.fx", PRESETS.get(index)?).to_ascii_lowercase();
        let dir = crate::game_fs::path(root, "SOUND/FXEDIT");
        let path = std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.to_ascii_lowercase() == want))?;
        Graph::parse(&crate::game_fs::read_file(&path).ok()?, rate)
    }

    /// Whether the hook would run now (the idle count is under 3000).
    pub fn running(&self) -> bool {
        self.idle < IDLE_LIMIT
    }

    /// One chunk (n ≤ [`BLOCK`]) of the process hook 0x72655A: `sent` is
    /// whether a voice mixed a nonzero send into `bus` this chunk; the wet
    /// output is added into `out[0]` / `out[1]` (the library's channels 0
    /// and 1). Returns whether the graph ran.
    pub fn process(&mut self, bus: &[f32], sent: bool, out: [&mut [f32]; 2]) -> bool {
        if sent {
            self.idle = 0;
        }
        self.idle = self.idle.saturating_add(1);
        if self.idle >= IDLE_LIMIT {
            return false;
        }
        let n = bus.len().min(BLOCK);
        let [o0, o1] = out;
        for (s, o) in [(0, o0), (1, o1)] {
            let Some(m) = self.outs[s] else { continue };
            pull(&mut self.mods, &mut self.bufs, m, n, &bus[..n], self.input);
            let g = self.gains[s];
            for (d, v) in o.iter_mut().zip(&self.bufs[m][..n]) {
                *d += v * g;
            }
        }
        true
    }
}

/// 0x72A817: the resonator's coefficients.
fn resonator(sr: i32, f: i32, bw: i32, g: i32) -> Kind {
    let gain = (g as f64 / 256.0) as f32;
    let (ff, bwf) = (f as f32, bw as f32);
    let bypass = sr as f64 * 0.636_619_75f32 as f64 <= bwf as f64 || ff <= 0.0 || ((sr >> 1) as f32) <= ff;
    let r_ext = 1.0 - (bwf as f64 * PI as f64) / sr as f64;
    let r = r_ext as f32;
    let r2 = (r_ext * r_ext) as f32;
    let theta = (ff as f64 * TWO_PI as f64 / sr as f64) as f32;
    let c_ext = (2.0 * r as f64) / (r2 as f64 + 1.0) * cos_approx(theta);
    let s = sqrt1p_approx((-(c_ext * c_ext)) as f32);
    let a = ((1.0 - r2 as f64) * s * gain as f64) as f32;
    Kind::Resonator { bypass, a, r, c: c_ext as f32, y1: 0.0, y2: 0.0 }
}

/// Pull module `i`'s chunk into `bufs[i]` (its vtable +0).
fn pull(mods: &mut [Module], bufs: &mut [Vec<f32>], i: usize, n: usize, bus: &[f32], input: usize) {
    // A missing input reads zeros (the original reads its scratch buffer;
    // no preset leaves one unconnected).
    let (in1, in2) = (mods[i].in1, mods[i].in2);
    let fetch = |mods: &mut [Module], bufs: &mut [Vec<f32>], j: Option<usize>| {
        if let Some(j) = j {
            pull(mods, bufs, j, n, bus, input);
        }
    };
    if !matches!(mods[i].kind, Kind::Input | Kind::Split { .. } | Kind::Sum) {
        fetch(mods, bufs, in1);
    }
    let mut out = std::mem::take(&mut bufs[i]);
    let src = |bufs: &[Vec<f32>], j: Option<usize>, k: usize| j.map_or(0.0, |j| bufs[j][k]);
    match &mut mods[i].kind {
        Kind::Input => {
            let _ = input;
            out[..n].copy_from_slice(&bus[..n]);
        }
        Kind::OnePole { y, b, a } => {
            for k in 0..n {
                *y = *y * *b + *a * src(bufs, in1, k);
                out[k] = *y;
            }
        }
        Kind::Fir { h, c } => {
            for k in 0..n {
                let x = src(bufs, in1, k);
                out[k] = (h[7] + x) * c[0]
                    + (h[2] + h[4]) * c[3]
                    + (h[5] + h[1]) * c[2]
                    + (h[0] + h[6]) * c[1]
                    + h[3] * c[4];
                h.copy_within(0..7, 1);
                h[0] = x;
            }
        }
        Kind::Resonator { bypass, a, r, c, y1, y2 } => {
            for k in 0..n {
                let x = src(bufs, in1, k);
                out[k] = if *bypass {
                    x
                } else {
                    let y = (x + GUARD) * *a + *r * *c * 2.0 * *y1 - *r * *r * *y2;
                    *y2 = *y1;
                    *y1 = y;
                    y
                };
            }
        }
        Kind::Gain(g) => {
            for k in 0..n {
                out[k] = src(bufs, in1, k) * *g;
            }
        }
        Kind::Allpass { buf, pos, g } => {
            for k in 0..n {
                if *pos >= buf.len() {
                    *pos = 0;
                }
                let d = buf[*pos];
                let v = *g * d + src(bufs, in1, k) + GUARD;
                out[k] = -*g * v + d;
                buf[*pos] = v;
                *pos += 1;
            }
        }
        Kind::Sum | Kind::Split { .. } => {}
    }
    bufs[i] = out;
    match mods[i].kind {
        Kind::Sum => {
            fetch(mods, bufs, in1);
            fetch(mods, bufs, in2);
            let mut out = std::mem::take(&mut bufs[i]);
            for k in 0..n {
                out[k] = src(bufs, in1, k) + src(bufs, in2, k);
            }
            bufs[i] = out;
        }
        Kind::Split { served, need } => {
            // 0x72A2F3: the first read computes and caches, the second
            // reads the cache.
            let (mut served, mut need) = (served, need);
            if served {
                served = false;
                need = true;
            }
            if need {
                fetch(mods, bufs, in1);
                let mut out = std::mem::take(&mut bufs[i]);
                for k in 0..n {
                    out[k] = src(bufs, in1, k);
                }
                bufs[i] = out;
                need = false;
            } else {
                served = true;
            }
            mods[i].kind = Kind::Split { served, need };
        }
        _ => {}
    }
}
