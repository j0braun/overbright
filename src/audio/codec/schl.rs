//! EA's streamed audio ("SCHl" files: the speech banks' `.big`, the
//! interactive music's `.mus`), parsed as the audio library's stream
//! reader does and decoded with the codec the header names (0x728DF9:
//! 10 EA-XA, 4 / 0x16 MicroTalk, 0x17 EALayer3).
//!
//! A stream is a run of chunks, each `{fourcc, u32 LE size including the
//! 8-byte head}`: "SCHl" (+8 "PT\0\0" then the tags of [`abk::tags`]:
//! 0x80 version, 0x82 channels, 0x83 codec 1, 0x84 rate, 0x85 samples,
//! 0xA0 codec), "SCCl" (+8 the block count), "SCDl" (+8 the block's
//! samples, then per codec), "SCLl" (loop) and "SCEl" (the end).
//!
//! * EA-XA (0xA0 absent → 10, the PC default): +0xC one u32 per channel,
//!   the channel's data offset after that table; each channel is EA-XA
//!   (see [`super::eaxa::decode_eaxa`]), its history carried across blocks.
//! * MicroTalk: +0xC one u32 per channel (the offset), then the data; a
//!   fresh header each block ([`utk`]).

use super::utk::Utk;
use crate::audio::banks::abk::tags;

#[derive(Clone, Debug, Default)]
pub struct Header {
    pub version: u8,
    pub channels: u8,
    pub codec: u8,
    pub rate: u32,
    pub samples: u32,
    /// Bytes from the stream start to its first block.
    pub header_size: usize,
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// The "SCHl" chunk at the start of `d`.
pub fn header(d: &[u8]) -> Option<Header> {
    if d.get(..4)? != b"SCHl" || d.get(8..10)? != b"PT" {
        return None;
    }
    let size = u32le(d, 4)? as usize;
    let mut h = Header { version: 0, channels: 1, codec: 10, rate: 22050, samples: 0, header_size: size };
    for (t, v) in tags(&d[..size.min(d.len())], 12) {
        let Some(v) = v else { continue };
        match t {
            0x80 => h.version = v as u8,
            0x82 => h.channels = v as u8,
            0x84 => h.rate = v as u32,
            0x85 => h.samples = v as u32,
            0xA0 => h.codec = v as u8,
            _ => {}
        }
    }
    Some(h)
}

/// The data blocks after the header: (block start, its size).
pub fn blocks(d: &[u8], h: &Header) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut o = h.header_size;
    while o + 8 <= d.len() {
        let id = &d[o..o + 4];
        let Some(size) = u32le(d, o + 4).map(|s| s as usize) else { break };
        if size < 8 || o + size > d.len() {
            break;
        }
        if id == b"SCDl" {
            out.push((o, size));
        } else if id == b"SCEl" {
            break;
        }
        o += size;
    }
    out
}

/// The whole length of the stream in bytes, through its "SCEl".
pub fn stream_len(d: &[u8]) -> Option<usize> {
    let h = header(d)?;
    let mut o = h.header_size;
    while o + 8 <= d.len() {
        let size = u32le(d, o + 4)? as usize;
        if size < 8 {
            return None;
        }
        let end = &d[o..o + 4] == b"SCEl";
        o += size;
        if end {
            return Some(o.min(d.len()));
        }
    }
    Some(o.min(d.len()))
}

/// A decoded stream: interleaved samples in −1..1.
#[derive(Clone, Debug, Default)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u8,
    pub samples: Vec<f32>,
}

/// EA-XA with the history carried (0x734AFF per block).
fn eaxa_block(d: &[u8], samples: usize, hist: &mut (f32, f32), out: &mut Vec<f32>) {
    let start = out.len();
    let (mut h1, mut h2) = *hist;
    let mut o = 0;
    let be16 = |o: usize| i16::from_be_bytes([d[o], d[o + 1]]) as f32;
    while out.len() - start < samples && o < d.len() {
        if d[o] == 0xEE {
            if o + 61 > d.len() {
                break;
            }
            h1 = be16(o + 1);
            h2 = be16(o + 3);
            for k in 0..28 {
                out.push(be16(o + 5 + k * 2));
            }
            o += 61;
        } else {
            if o + 15 > d.len() {
                break;
            }
            const C1: [f32; 4] = [0.0, 0.9375, 1.796875, 1.53125];
            const C2: [f32; 4] = [0.0, 0.0, -0.8125, -0.859375];
            let k = (d[o] >> 4) as usize;
            let (c1, c2) = (C1.get(k).copied().unwrap_or(0.0), C2.get(k).copied().unwrap_or(0.0));
            let shift = (d[o] & 0xF) as u32;
            for j in 0..14 {
                let b = d[o + 1 + j];
                for nib in [b >> 4, b & 0xF] {
                    let n = ((nib as i8) << 4 >> 4) as f32;
                    let t = if shift <= 12 { n * (1u32 << (12 - shift)) as f32 } else { 0.0 };
                    let s = c1 * h1 + c2 * h2 + t;
                    h2 = h1;
                    h1 = s;
                    out.push(s);
                }
            }
            o += 15;
        }
    }
    out.truncate(start + samples);
    *hist = (h1, h2);
}

/// The most one decoded stream may take (half of `Limits::max_alloc`: a growing Vec doubles).
const MAX_STREAM_BYTES: usize = crate::formats::reader::Limits::DEFAULT.max_alloc / 2;

/// Decode a whole stream (EA-XA or MicroTalk; EALayer3 in [`super::ealayer3`]).
pub fn decode(d: &[u8]) -> Option<Pcm> {
    let h = header(d)?;
    let ch = h.channels.max(1) as usize;
    let mut per: Vec<Vec<f32>> = vec![Vec::new(); ch];
    match h.codec {
        10 => {
            let mut hist = vec![(0.0f32, 0.0f32); ch];
            for (o, size) in blocks(d, &h) {
                let b = &d[o..o + size];
                let n = u32le(b, 8)? as usize;
                let table = 0xC + 4 * ch;
                for c in 0..ch {
                    let off = table + u32le(b, 0xC + 4 * c)? as usize;
                    eaxa_block(b.get(off..)?, n, &mut hist[c], &mut per[c]);
                }
            }
        }
        4 | 0x16 => {
            // One decoder per channel, kept across blocks; each block's
            // data starts with a byte that says whether a header follows.
            let mut dec: Vec<Option<Utk>> = (0..ch).map(|_| None).collect();
            for (o, size) in blocks(d, &h) {
                let b = &d[o..o + size];
                let n = u32le(b, 8)? as usize;
                let table = 0xC + 4 * ch;
                for c in 0..ch {
                    let off = o + table + u32le(b, 0xC + 4 * c)? as usize;
                    let data = d.get(off + 1..o + size)?;
                    let fresh = d[off] != 0;
                    match &mut dec[c] {
                        Some(u) => u.begin(data, fresh),
                        slot => *slot = Some(Utk::new(data, utk_rev3(&h))),
                    }
                    let u = dec[c].as_mut().unwrap();
                    let mut left = n;
                    while left > 0 {
                        let f = u.frame();
                        let k = left.min(f.len());
                        // A block's sample count is a file field: a decoded
                        // stream stays within the allocation limit (fuzz).
                        if k == 0 || (per[c].len() + k) * ch * 4 > MAX_STREAM_BYTES {
                            return None;
                        }
                        per[c].extend(f[..k].iter().copied());
                        left -= k;
                    }
                }
            }
        }
        0x17 => return super::ealayer3::decode_stream(d, &h),
        _ => return None,
    }
    let len = per.iter().map(|p| p.len()).min().unwrap_or(0);
    let mut samples = Vec::with_capacity(len * ch);
    for i in 0..len {
        for p in &per {
            samples.push(p[i] / 32768.0);
        }
    }
    Some(Pcm { rate: h.rate, channels: ch as u8, samples })
}

/// Whether MicroTalk blocks carry the per-frame marker byte (0x73478B).
pub fn utk_rev3(h: &Header) -> bool {
    h.version >= 2
}
