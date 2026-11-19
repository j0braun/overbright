//! EAGL4 animation-channel decoder for NFSU2 `__AnimationBank` ELF objects
//! (chunk 0xE34010, loaded by `0x4348F0` -> `0x434710` -> ELF loader `0x6D80B0`).
//!
//! Everything here was traced in SPEED2.EXE (objdump/Ghidra). Items not read
// prov: inferred formats.eagl.module
//! directly off the code are marked **inferred**.
//!
//! # Object model
//! * Type -> class factory: `0x6E2950` (switch on type 0..0x18, jump table
//!   `0x6E2B7C`). Relevant classes:
//!
//!   | type | ctor       | vtable     | init (vt+0x48) | eval (vt+0x18) | meaning |
//!   |------|------------|------------|----------------|----------------|---------|
//!   | 0x0F | `0x6E20E0` | `0x7C1978` | `0x6E6780`     | `0x6E6910`     | compound: list of child channels |
//!   | 0x0A | `0x6E2680` | `0x7C1A40` | `0x6E4730`     | `0x6E5460`     | delta-compressed floats, 1 float per output slot |
//!   | 0x0B | `0x6E26B0` | `0x7C1A98` | `0x6E4730`     | `0x6E5570`     | same delta stream, 4 floats (quaternion) per output slot |
//!   | 0x15 | `0x6E7880` | `0x7C1D28` | `0x6E89A0`     | `0x6E82D0`     | block-keyed (u16 key + u8 deltas) floats + constant floats |
//!   | 0x11 | `0x6ECC20` | `0x7C1F50` | -              | -              | not traced (not needed here) |
//!
//!   `0x6E2C90` maps a data pointer to its runtime object: type 0x0F gets a fresh
//!   object (`0x6E20E0`), types 0x16/0x17/0x19 live in-place before their data.
//!   `0x434A80`/`0x4350C0` bind a bank channel (`0x6E2D40`) into a scene slot;
//!   `0x431400` calls the slot's vt+0x18 = `0x6E6910` with the current time.
//!
//! * Eval signature (all types): `bool eval(float t, float *pose, int blend)`;
//!   `blend == 0` is the plain path decoded here (blend != 0 goes to
//!   `0x6E4A70`/`0x6E4D90`/`0x6E7A80`, not traced). Each sub-channel writes
//!   floats into `pose[slot]`; slots are float indices (observed: scale = 12..14,
// prov: inferred formats.eagl.module-2
//!   quaternion = 16..19, translation = 20..22 - the layout is **inferred** from
//!   the slot lists in the data).
//!
//! # Time
//! `t` is in frames. World-anim controller update `0x434B00` advances
//! `ctrl+0x68 += dt * 30.0 (0x785BD8) * speed(ctrl+0x60)`, so **30 frames/s**
// prov: inferred formats.eagl.module-3
//! (NIS scenes are assumed to use the same rate - **inferred**). The compound
//! object can rescale `t` by a byte at obj+0x15 when obj+0x14 is set (runtime
//! fields, `0x6E6910`; not in the file). `floor(t)` = `0x6E4900`
//! (`fistp(t - 0.4999990)`).
//!
//! # Layouts (offsets in the ELF `.data` section; pointers are .data-relative)
//! See the per-type docs on [`decode_compound`], [`decode_delta`], [`decode_keyblock`].

use std::fmt;

use super::reader::{Limits, Reader};

fn u8_(d: &[u8], o: usize) -> Result<u8, String> {
    Reader::new(d).u8_at(o).map_err(|e| e.to_string())
}
fn u16_(d: &[u8], o: usize) -> Result<u16, String> {
    Reader::new(d).u16_at(o).map_err(|e| e.to_string())
}
fn u32_(d: &[u8], o: usize) -> Result<u32, String> {
    Reader::new(d).u32_at(o).map_err(|e| e.to_string())
}
fn f32_(d: &[u8], o: usize) -> Result<f32, String> {
    Reader::new(d).f32_at(o).map_err(|e| e.to_string())
}

/// A channel's decoded size: `frames` rows of `width` floats, at most
/// [`Limits::max_anim_values`] (the frame count comes from a 16-bit header
/// field and rows past the stored keys repeat, so the data does not bound it).
fn check_values(frames: usize, width: usize) -> Result<(), String> {
    Limits::check("animation values", frames.saturating_mul(width.max(1)), Limits::DEFAULT.max_anim_values).map(|_| ()).map_err(|e| e.to_string())
}

/// A fully decoded channel: `frames[f][k]` is the value written to pose float
/// `slots[k]` at integer frame `f`.
#[derive(Clone, Debug)]
pub struct Channel {
    /// Sub-channel type(s) that produced the data (0x0A, 0x0B, 0x15).
    pub kinds: Vec<u16>,
    /// Pose float indices written (see module docs).
    pub slots: Vec<u16>,
    /// Frame count from the compound header (+0x0A).
    pub frame_count: usize,
    pub frames: Vec<Vec<f32>>,
}

impl Channel {
    /// Value at fractional frame `t`, matching the game's evaluators: clamp to
    /// [0, n-1], then component-wise linear interpolation between the two
    /// neighbouring frames (`0x6EFE40` for 0x0A/0x0B, `0x6E86C0` loop for
    /// 0x15). Quaternions are lerped component-wise and NOT renormalised by the
    // prov: inferred formats.eagl.sample_each
    /// channel evaluator (normalisation, if any, happens later - **inferred**).
    /// Constant slots of 0x15 are identical in every frame so lerp is a no-op.
    /// [`Channel::sample`] without its vector: `f(k, value)` for each
    /// value in turn (the same arithmetic).
    pub fn sample_each(&self, t: f32, mut f: impl FnMut(usize, f32)) {
        let n = self.frames.len();
        if n == 0 {
            return;
        }
        let f_ = t.floor();
        let i = f_ as usize;
        // `i >= n - 1`, not `i + 1 >= n`: a huge `t` saturates `i` to
        // usize::MAX, and `i + 1` wrapped to 0.
        if !(t > 0.0) || i >= n - 1 {
            let row = if !(t > 0.0) { &self.frames[0] } else { &self.frames[n - 1] };
            for (k, &x) in row.iter().enumerate() {
                f(k, x);
            }
            return;
        }
        let a = t - f_;
        for (k, (x, y)) in self.frames[i].iter().zip(&self.frames[i + 1]).enumerate() {
            f(k, x + (y - x) * a);
        }
    }

    pub fn sample(&self, t: f32) -> Vec<f32> {
        let n = self.frames.len();
        if n == 0 {
            return Vec::new();
        }
        if !(t > 0.0) {
            return self.frames[0].clone();
        }
        let f = t.floor();
        let i = f as usize;
        if i >= n - 1 {
            return self.frames[n - 1].clone();
        }
        let a = t - f;
        self.frames[i].iter().zip(&self.frames[i + 1]).map(|(x, y)| x + (y - x) * a).collect()
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "kinds={:x?} slots={:?} frames={}", self.kinds, self.slots, self.frame_count)
    }
}

/// Decode the channel whose header is at `anim_off` (the pointer stored in the
/// bank's anim array). Accepts the 0x0F compound or a bare sub-channel.
pub fn decode_channel(data: &[u8], anim_off: usize) -> Result<Channel, String> {
    match u16_(data, anim_off)? {
        0x0F => decode_compound(data, anim_off),
        t => {
            let (slots, frames) = decode_sub(data, anim_off, None)?;
            let n = frames.len();
            Ok(Channel { kinds: vec![t], slots, frame_count: n, frames })
        }
    }
}

/// Type 0x0F compound ("anim list"):
/// ```text
/// +0x00 u16 type = 0x0F   +0x02 u8,u8 (0x14,0x14 in all files; unused by eval)
// prov: inferred formats.eagl.decode_compound
/// +0x04 u32 ptr           (not used by init/eval; **inferred**: back-pointer/id)
/// +0x08 u16 child count   (0x6E6780 / 0x6E67D0 loop bound)
/// +0x0A u16 frame count   (GetLength vt+0x10 = 0x6E2110 returns it as float)
/// +0x0C u32 child[count]  (0x6E67D0: each resolved to an object via alloc vt+0x24)
/// ```
/// Eval `0x6E6910` calls every child's vt+0x18 with the same (t, pose, blend),
/// last child first. Children writing the same slot: the earlier child wins.
pub fn decode_compound(data: &[u8], off: usize) -> Result<Channel, String> {
    let count = u16_(data, off + 8)? as usize;
    let nfr = u16_(data, off + 0xA)? as usize;
    // Every child adds at least one value per frame.
    check_values(nfr, count)?;
    let mut ch = Channel { kinds: vec![], slots: vec![], frame_count: nfr, frames: vec![vec![]; nfr] };
    for c in (0..count).rev() {
        let p = u32_(data, off + 0xC + 4 * c)? as usize;
        let kind = u16_(data, p)?;
        let (slots, frames) = decode_sub(data, p, Some(nfr))?;
        ch.kinds.push(kind);
        for (k, s) in slots.iter().enumerate() {
            let pos = ch.slots.iter().position(|x| x == s);
            for f in 0..nfr {
                let v = frames[f.min(frames.len() - 1)][k];
                match pos {
                    Some(j) => ch.frames[f][j] = v,
                    None => ch.frames[f].push(v),
                }
            }
            if pos.is_none() {
                ch.slots.push(*s);
            }
        }
    }
    ch.kinds.reverse();
    Ok(ch)
}

fn decode_sub(data: &[u8], off: usize, nfr: Option<usize>) -> Result<(Vec<u16>, Vec<Vec<f32>>), String> {
    match u16_(data, off)? {
        0x0A => decode_delta(data, off, 1, nfr),
        0x0B => decode_delta(data, off, 4, nfr),
        0x15 => decode_keyblock(data, off, nfr),
        0x14 => decode_keyblock_vec3(data, off, nfr),
        t => Err(format!("unsupported channel type {:#x} @{:#x}", t, off)),
    }
}

/// Types 0x0A (`stride` 1) and 0x0B (`stride` 4, quaternions).
/// Header (`0x6E5460` / `0x6E5570`):
/// ```text
/// +0x00 u16 type, +0x02 u8,u8
/// +0x04 u32 -> D (delta stream, below)
/// +0x08 u16 frame count   (frame index clamped to [0, count-1])
/// +0x0A u16 slot[N/stride] pose float index of each output (0x0B copies 4 floats from there)
/// ```
/// Delta stream D, decoded by `0x6EFA20` (integer frames) / `0x6EFE40` (fractional):
/// ```text
/// D+0 u16 N        number of float components
/// D+2 u8  bits     4, 8 or 16 (anything else: no deltas applied)
/// D+3 u8           (unused by the decoder)
/// D+4 N x { f32 offset, f32 scale, f32 initial }   12 bytes each
/// then rows r = 0,1,..: N unsigned quanta each; row r takes frame r -> r+1
///   bits 16: u16, row = 2N bytes;  bits 8: u8, row = N bytes;
///   bits 4 : nibbles, row = ceil(N/2) bytes, component 2i = low nibble,
///            2i+1 = high nibble (0x6EFCF0 loop).
/// value(0) = initial;  value(f+1) = value(f) + q[f] * scale + offset
/// ```
pub fn decode_delta(data: &[u8], off: usize, stride: usize, nfr: Option<usize>) -> Result<(Vec<u16>, Vec<Vec<f32>>), String> {
    let d = u32_(data, off + 4)? as usize;
    let count = u16_(data, off + 8)? as usize;
    let n = u16_(data, d)? as usize;
    let bits = u8_(data, d + 2)?;
    if n % stride != 0 {
        return Err(format!("N={} not a multiple of stride {}", n, stride));
    }
    let mut slots = Vec::new();
    for k in 0..n / stride {
        let s = u16_(data, off + 0xA + 2 * k)?;
        for c in 0..stride {
            slots.push(s.wrapping_add(c as u16));
        }
    }
    let rec = |i: usize| -> Result<(f32, f32, f32), String> {
        Ok((f32_(data, d + 4 + 12 * i)?, f32_(data, d + 8 + 12 * i)?, f32_(data, d + 12 + 12 * i)?))
    };
    let rows = d + 4 + 12 * n;
    let row_bytes = match bits {
        16 => 2 * n,
        8 => n,
        4 => (n + 1) / 2,
        _ => 0,
    };
    let q = |r: usize, i: usize| -> Result<u32, String> {
        let b = rows + r * row_bytes;
        Ok(match bits {
            16 => u16_(data, b + 2 * i)? as u32,
            8 => u8_(data, b + i)? as u32,
            4 => {
                let x = u8_(data, b + i / 2)?;
                (if i & 1 == 0 { x & 0xF } else { x >> 4 }) as u32
            }
            _ => 0,
        })
    };
    let mut cur: Vec<f32> = (0..n).map(|i| rec(i).map(|r| r.2)).collect::<Result<_, _>>()?;
    let total = nfr.unwrap_or(count).max(1);
    check_values(total, n)?;
    let mut out = Vec::with_capacity(total);
    for f in 0..total {
        if f > 0 && f < count && row_bytes > 0 {
            for i in 0..n {
                let (o, s, _) = rec(i)?;
                cur[i] = (cur[i] as f64 + q(f - 1, i)? as f64 * s as f64 + o as f64) as f32;
            }
        }
        out.push(cur.clone()); // frames past `count` repeat the last one (clamp)
    }
    Ok((slots, out))
}

/// Type 0x15 block-keyed channel. Header (`0x6E82D0`, table build `0x6E78F0`):
/// ```text
/// +0x00 u16 type, +0x02 u8,u8
/// +0x04 u32 -> u16 slot[N]  pose float index per animated component (0x6E86DD)
/// +0x08 u32 -> u16 keytime[] or 0 (0 = one key per frame). If present,
///            keytime[k-1] is the frame of key k (key 0 at frame 0) (0x6E834D, 0x6E8612)
/// +0x0C u16 K  key count
/// +0x0E u16 N  animated components
/// +0x10 u8  s  log2 keys per block (B = 1<<s)
/// +0x11 u8  M  constant components
/// +0x14 N x { f32 min, f32 range, u16 dmin, u16 dspan }   12 bytes each
/// then ceil(K/B) blocks, block size = ((B+1)*N + 1) & !1 bytes:
///   N x u16 absolute value of the block's first key
///   (B-1) rows of N x u8 deltas for the following keys
/// after the last (possibly short: ((r+1)*N+1)&!1 bytes) block:
///   u16 cslot[M], then (aligned to 4) f32 cval[M]   pose[cslot[j]] = cval[j]  (0x6E8826)
/// ```
/// Per component the game precomputes (f32, `0x6E78F0`, c = 1.5259022e-5 = 1/65535):
/// `A = range*c`, `Doff = 2*dmin*range*c - range`, `Dscl = (2*dspan*range*c) * (1/255)`.
/// key value: `v = u16*A + min`, then for each later key in the block
/// `v += u8*Dscl + Doff`.
pub fn decode_keyblock(data: &[u8], off: usize, nfr: Option<usize>) -> Result<(Vec<u16>, Vec<Vec<f32>>), String> {
    decode_keyblock_wide(data, off, nfr, 1)
}

/// Type 0x14: the 0x15 layout with vec3 items (`0x6E8970`, vtable
/// `0x7C1DB0`, eval `0x6E9780`, table build `0x6E89C0`, constant section
/// `0x6E88F0`; init `0x6E89A0` is 0x15's). The character banks
/// of the NIS scenes use it for bone translations and scales.
/// ```text
/// +0x04 u32 -> u16 slot[N]  first pose float of each vec3 (x, y, z at slot..slot+2)
/// +0x08 u32 -> u16 keytime[] or 0, +0x0C u16 K, +0x0E u16 N items,
/// +0x10 u8 s, +0x11 u8 M constant vec3s
/// +0x14 N x { f32 min[3], f32 range[3], u16 dmin[3], u16 dspan[3] }  0x24 bytes each
/// blocks of ((B+1)*3N + 1) & !1 bytes: 3N u16 first-key values (x, y, z
/// per item), then (B-1) rows of 3N u8 deltas; constants: u16 slot[M], then
/// (aligned to 4) f32 xyz[M].
/// ```
/// Per component the table and the key / delta sums are 0x15's.
pub fn decode_keyblock_vec3(data: &[u8], off: usize, nfr: Option<usize>) -> Result<(Vec<u16>, Vec<Vec<f32>>), String> {
    decode_keyblock_wide(data, off, nfr, 3)
}

/// The 0x15 (`w` 1) and 0x14 (`w` 3) block-keyed decoder.
fn decode_keyblock_wide(data: &[u8], off: usize, nfr: Option<usize>, w: usize) -> Result<(Vec<u16>, Vec<Vec<f32>>), String> {
    let slot_p = u32_(data, off + 4)? as usize;
    let time_p = u32_(data, off + 8)? as usize;
    let k = u16_(data, off + 0xC)? as usize;
    let items = u16_(data, off + 0xE)? as usize;
    let n = items * w;
    let s = u8_(data, off + 0x10)? as u32;
    let m = u8_(data, off + 0x11)? as usize;
    // Fuzzing: a block shift past the 16-bit key count is corrupt data,
    // not an overflow.
    if s > 16 {
        return Err(format!("keyblock shift {s}"));
    }
    let b = 1usize << s;
    let c: f32 = f32::from_bits(0x37800080); // 0x7C1D18
    let c255: f32 = f32::from_bits(0x3B808081); // 0x7C1D10
    let mut tab = Vec::with_capacity(n);
    for i in 0..n {
        let (item, comp) = (i / w, i % w);
        let r = off + 0x14 + 12 * w * item;
        let min = f32_(data, r + 4 * comp)?;
        let range = f32_(data, r + 4 * (w + comp))?;
        let dmin = u16_(data, r + 8 * w + 2 * comp)? as f64;
        let dspan = u16_(data, r + 10 * w + 2 * comp)? as f64;
        let a = (range as f64 * c as f64) as f32;
        let doff = (dmin * range as f64 * c as f64 * 2.0 - range as f64) as f32;
        let dscl = ((dspan * range as f64 * c as f64 * 2.0) as f32 as f64 * c255 as f64) as f32;
        tab.push((min, a, doff, dscl));
    }
    let blk0 = off + 0x14 + 12 * n;
    let blk_bytes = ((b + 1) * n + 1) & !1;
    let key_val = |key: usize| -> Result<Vec<f32>, String> {
        let base = blk0 + (key >> s) * blk_bytes;
        let j = key & (b - 1);
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let (min, a, doff, dscl) = tab[i];
            let mut x = (u16_(data, base + 2 * i)? as f64 * a as f64 + min as f64) as f32;
            for row in 1..=j {
                let q = u8_(data, base + (row + 1) * n + i)? as f64;
                x = (q * dscl as f64 + doff as f64 + x as f64) as f32;
            }
            v.push(x);
        }
        Ok(v)
    };
    // constant section
    let mut end = blk0 + (k >> s) * blk_bytes;
    let rem = k & (b - 1);
    if rem > 0 {
        end = ((rem + 1) * n + end + 1) & !1;
    }
    let cval_p = (end + 2 * m + 3) & !3; // **inferred**: .data offset alignment == memory alignment
    let mut slots: Vec<u16> = (0..n).map(|i| u16_(data, slot_p + 2 * (i / w)).map(|s| s.wrapping_add((i % w) as u16))).collect::<Result<_, _>>()?;
    let mut consts = Vec::with_capacity(m * w);
    for j in 0..m {
        let s = u16_(data, end + 2 * j)?;
        for c in 0..w {
            slots.push(s.wrapping_add(c as u16));
            consts.push(f32_(data, cval_p + 4 * (w * j + c))?);
        }
    }
    let times: Option<Vec<u16>> = if time_p == 0 {
        None
    } else {
        Some((0..k.saturating_sub(1)).map(|i| u16_(data, time_p + 2 * i)).collect::<Result<_, _>>()?)
    };
    if k == 0 {
        return Err("keyblock without keys".into());
    }
    let total = nfr.unwrap_or(k).max(1);
    check_values(total, n + m * w)?;
    check_values(k, n)?;
    let keys: Vec<Vec<f32>> = (0..k).map(key_val).collect::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(total);
    for f in 0..total {
        let mut v = match &times {
            None => keys[f.min(k - 1)].clone(),
            Some(ts) => {
                // key 0 at frame 0, key i at ts[i-1]; lerp between neighbours
                let kt = |i: usize| if i == 0 { 0.0 } else { ts[i - 1] as f32 };
                let mut i = 0;
                while i + 1 < k && kt(i + 1) <= f as f32 {
                    i += 1;
                }
                if i + 1 >= k {
                    keys[k - 1].clone()
                } else {
                    let a = (f as f32 - kt(i)) / (kt(i + 1) - kt(i));
                    keys[i].iter().zip(&keys[i + 1]).map(|(x, y)| x + (y - x) * a).collect()
                }
            }
        };
        v.extend_from_slice(&consts);
        out.push(v);
    }
    Ok((slots, out))
}

impl Channel {
    /// The frame count.
    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    /// Slots 20–22 (translation) or 12–14 (scale) as x, y, z at frame `t`,
    /// by the slots the channel writes (a 0x15 channel lists them out of
    /// order: 21, 22, 20 for the door slats).
    pub fn vec3(&self, t: f32) -> glam::Vec3 {
        let v = self.sample(t);
        let base = self.slots.iter().copied().min().unwrap_or(0);
        let mut out = [0.0; 3];
        for (k, &s) in self.slots.iter().enumerate() {
            if let (Some(o), Some(&x)) = (out.get_mut((s - base) as usize), v.get(k)) {
                *o = x;
            }
        }
        glam::Vec3::from(out)
    }

    /// Slots 16–19 as a quaternion x, y, z, w at frame `t`, normalised.
    pub fn quat(&self, t: f32) -> glam::Quat {
        let v = self.sample(t);
        if v.len() < 4 {
            return glam::Quat::IDENTITY;
        }
        glam::Quat::from_xyzw(v[0], v[1], v[2], v[3]).normalize()
    }
}

/// Every channel of a bank (chunk 0xE34010's aligned body, an ELF object
/// whose `__AnimationBank:::bank` symbol gives +0x04 the count, +0x10 the
/// animation pointers and +0x14 their name pointers), by name.
pub fn parse_bank(elf: &[u8]) -> Result<Vec<(String, Channel)>, String> {
    let start = elf.windows(4).position(|w| w == b"\x7fELF").ok_or("not an ELF object")?;
    let elf = &elf[start..];
    let shoff = u32_(elf, 0x20)? as usize;
    let shnum = u16_(elf, 0x30)? as usize;
    let sec = |i: usize, f: usize| -> Result<usize, String> { Ok(u32_(elf, shoff + 40 * i + 4 * f)? as usize) };
    let (data_off, data_len) = (sec(1, 4)?, sec(1, 5)?);
    let data = elf.get(data_off..data_off + data_len).ok_or(".data bounds")?;
    let sym = (0..shnum).find(|&i| sec(i, 1).is_ok_and(|t| t == 2)).ok_or("no .symtab")?;
    let strs = sec(sec(sym, 6)?, 4)?;
    let cstr = |b: &[u8], o: usize| -> String { b.get(o..).map(|s| String::from_utf8_lossy(s.split(|&c| c == 0).next().unwrap_or_default()).into_owned()).unwrap_or_default() };
    let mut bank = None;
    for i in 0..sec(sym, 5)? / 16 {
        let so = sec(sym, 4)? + 16 * i;
        if cstr(elf, strs + u32_(elf, so)? as usize).starts_with("__AnimationBank") {
            bank = Some(u32_(elf, so + 4)? as usize);
        }
    }
    let bank = bank.ok_or("no bank symbol")?;
    let count = u32_(data, bank + 4)? as usize;
    let (anims, names) = (u32_(data, bank + 0x10)? as usize, u32_(data, bank + 0x14)? as usize);
    // The whole bank's values together (the channels may share data, so
    // each one within the limit is not enough).
    let mut values = 0usize;
    (0..count)
        .map(|i| {
            let name = cstr(data, u32_(data, names + 4 * i)? as usize);
            let channel = decode_channel(data, u32_(data, anims + 4 * i)? as usize)?;
            values = values.saturating_add(channel.frames.iter().map(Vec::len).sum());
            check_values(values, 1)?;
            Ok((name, channel))
        })
        .collect()
}

crate::persist!(Channel { kinds, slots, frame_count, frames });
