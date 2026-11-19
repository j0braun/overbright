//! JDLZ, the LZ77 variant EA used for compressed texture blocks.
//!
//! ```text
//! "JDLZ" 02 10 00 00   magic and version
//! u32 uncompressed size
//! u32 compressed size (including this 16-byte header)
//! stream
//! ```
//!
//! The stream interleaves two flag bytes, each consumed one bit at a time
//! (low bit first). A 0 bit in the first flags means "copy one literal byte".
//! A 1 bit means a back-reference of two bytes, whose form depends on the
//! next bit of the second flags:
//!
//! * 1 — near:  distance `(b0 & 0x0F) + 1`, length `(b1 | (b0 & 0xF0) << 4) + 3`
//! * 0 — far:   distance `(b1 | (b0 & 0xE0) << 3) + 17`, length `(b0 & 0x1F) + 3`
//!
//! Checked on all 55 JDLZ blocks in `LOC4DYNTEX.BIN`: each decodes to exactly
//! its declared size and ends with a well-formed texture entry.

use anyhow::{Result, bail};

use super::reader::{Limits, Reader};

pub const MAGIC: &[u8; 4] = b"JDLZ";
const HEADER_SIZE: usize = 16;

/// Longest output one stream byte pair can produce: a near reference copies
/// `0xFFF + 3` bytes from 2 stream bytes, so no valid block expands by more
/// than 2049:1 (a literal is 1:1). Bounds the up-front allocation.
const MAX_RATIO: usize = 2049;

/// Decompress a block of at most [`Limits::max_unpacked`] bytes (8x the
/// largest shipped one).
pub fn decompress(src: &[u8]) -> Result<Vec<u8>> {
    decompress_max(src, Limits::DEFAULT.max_unpacked)
}

/// `decompress`, refusing blocks that declare more than `max_size` bytes
/// (callers that know the real unpacked size pass it).
pub fn decompress_max(src: &[u8], max_size: usize) -> Result<Vec<u8>> {
    let header = Reader::new(src);
    if src.len() < HEADER_SIZE || &header.array_at::<4>(0)? != MAGIC {
        bail!("not a JDLZ block");
    }
    let size = header.u32_at(8)? as usize;
    if size > max_size || size > src.len().saturating_mul(MAX_RATIO) {
        bail!("JDLZ: declared size {size} is impossible for {} packed bytes", src.len());
    }
    let mut out = vec![0u8; size];
    let (mut i, mut o) = (HEADER_SIZE, 0);
    let (mut flags1, mut flags2) = (1u32, 1u32);

    // Any out-of-range read below means corrupt input; report it rather
    // than panicking.
    let truncated = || anyhow::anyhow!("JDLZ: truncated input");

    while o < size {
        if flags1 == 1 {
            flags1 = *src.get(i).ok_or_else(truncated)? as u32 | 0x100;
            i += 1;
        }
        if flags2 == 1 {
            flags2 = *src.get(i).ok_or_else(truncated)? as u32 | 0x100;
            i += 1;
        }
        if flags1 & 1 == 1 {
            let &[b0, b1] = src.get(i..i + 2).ok_or_else(truncated)? else { unreachable!() };
            let (b0, b1) = (b0 as usize, b1 as usize);
            i += 2;
            let (distance, length) = if flags2 & 1 == 1 {
                ((b0 & 0x0F) + 1, (b1 | (b0 & 0xF0) << 4) + 3)
            } else {
                ((b1 | (b0 & 0xE0) << 3) + 17, (b0 & 0x1F) + 3)
            };
            flags2 >>= 1;
            if distance > o {
                bail!("JDLZ: back-reference before start of output");
            }
            let n = length.min(size - o);
            if distance >= n {
                // Source and destination apart: one copy.
                out.copy_within(o - distance..o - distance + n, o);
            } else if distance == 1 {
                // A run of the last byte.
                let b = out[o - 1];
                out[o..o + n].fill(b);
            } else {
                // Byte by byte: source and destination overlap.
                for k in o..o + n {
                    out[k] = out[k - distance];
                }
            }
            o += n;
        } else {
            out[o] = *src.get(i).ok_or_else(truncated)?;
            o += 1;
            i += 1;
        }
        flags1 >>= 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::bundle::read_u32;

    #[test]
    fn rejects_impossible_declared_size() {
        let mut b = b"JDLZ\x02\x10\0\0".to_vec();
        b.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        b.extend_from_slice(&[16, 0, 0, 0]);
        assert!(decompress(&b).is_err());
        let mut ok = b[..8].to_vec();
        ok.extend_from_slice(&5u32.to_le_bytes());
        ok.extend_from_slice(&[16, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5]);
        assert!(decompress_max(&ok, 4).is_err());
    }

    /// The plain byte-by-byte decoder the fast one must match.
    fn reference(src: &[u8]) -> Vec<u8> {
        let size = read_u32(src, 8) as usize;
        let mut out = vec![0u8; size];
        let (mut i, mut o) = (HEADER_SIZE, 0);
        let (mut flags1, mut flags2) = (1u32, 1u32);
        while o < size {
            if flags1 == 1 {
                flags1 = src[i] as u32 | 0x100;
                i += 1;
            }
            if flags2 == 1 {
                flags2 = src[i] as u32 | 0x100;
                i += 1;
            }
            if flags1 & 1 == 1 {
                let (b0, b1) = (src[i] as usize, src[i + 1] as usize);
                i += 2;
                let (distance, length) = if flags2 & 1 == 1 { ((b0 & 0x0F) + 1, (b1 | (b0 & 0xF0) << 4) + 3) } else { ((b1 | (b0 & 0xE0) << 3) + 17, (b0 & 0x1F) + 3) };
                flags2 >>= 1;
                for _ in 0..length.min(size - o) {
                    out[o] = out[o - distance];
                    o += 1;
                }
            } else {
                out[o] = src[i];
                o += 1;
                i += 1;
            }
            flags1 >>= 1;
        }
        out
    }

    /// Every JDLZ block in a few of the game's files decodes as the plain
    /// decoder decodes it.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn matches_reference_on_game_data() {
        let root = crate::testkit::game_root();
        let mut blocks = 0;
        for f in ["CARS/TEXTURES.BIN", "CARS/WHEELS/TEXTURES.BIN", "TRACKS/LOC4DYNTEX.BIN", "GLOBAL/GlobalB.lzc", "FRONTEND/FrontB.lzc"] {
            let Ok(data) = std::fs::read(root.join(f)) else { continue };
            let mut at = 0;
            while let Some(p) = data[at..].windows(4).position(|w| w == MAGIC) {
                let start = at + p;
                at = start + 4;
                let Ok(out) = decompress(&data[start..]) else { continue };
                assert_eq!(out, reference(&data[start..]), "{f} at {start:#x}");
                blocks += 1;
            }
        }
        assert!(blocks > 100);
    }
}
