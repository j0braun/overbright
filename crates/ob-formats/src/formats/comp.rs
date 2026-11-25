//! RAWW and COMP, the two other codecs the original's block decoder knows
//! beside JDLZ and HUFF (dispatcher 0x574680: `RAWW` 0x574500, `HUFF`
//! 0x5744D0, `COMP` 0x574540, `JDLZ` 0x573710). The shipped game uses
//! neither, but tools that write texture packs may (a modded
//! `TRACKS/LOC4DYNTEX.BIN`), and the original reads them.
//!
//! ```text
//! RAWW: "RAWW" u8 1, 3 bytes, u32 size, u32, then `size` stored bytes
//! COMP: "COMP" u16, u16 stored (1 = stored), u32, u32 total size
//!       (including this 16-byte header), then the stream
//! ```
//!
//! COMP's stream (0x57459D…0x574624): a 16-bit flag word (low byte first)
//! read whenever the previous one is used up, consumed low bit first. A 0
//! bit copies one literal byte; a 1 bit reads two bytes `b0 b1`: distance
//! `(b0 & 0xF0) << 4 | b1`, length `(b0 & 0x0F) + 3`, copied byte by byte
//! (source and destination may overlap). The stream ends where the total
//! size says.

use anyhow::{Result, bail};

use super::reader::Reader;

pub const RAWW: &[u8; 4] = b"RAWW";
pub const COMP: &[u8; 4] = b"COMP";
const HEADER_SIZE: usize = 16;

/// A RAWW block's stored bytes (the original checks the magic and that byte 4 is 1).
pub fn raww(src: &[u8], max_size: usize) -> Result<Vec<u8>> {
    let r = Reader::new(src);
    if src.len() < HEADER_SIZE || &r.array_at::<4>(0)? != RAWW || r.u8_at(4)? != 1 {
        bail!("not a RAWW block");
    }
    let size = r.u32_at(8)? as usize;
    if size > max_size {
        bail!("RAWW: declared size {size} is above {max_size}");
    }
    let Some(data) = src.get(HEADER_SIZE..HEADER_SIZE + size) else {
        bail!("RAWW: {size} bytes declared, {} stored", src.len() - HEADER_SIZE);
    };
    Ok(data.to_vec())
}

/// A COMP block decoded (at most `max_size` bytes out).
pub fn comp(src: &[u8], max_size: usize) -> Result<Vec<u8>> {
    let r = Reader::new(src);
    if src.len() < HEADER_SIZE || &r.array_at::<4>(0)? != COMP {
        bail!("not a COMP block");
    }
    let total = r.u32_at(0x0C)? as usize;
    if total < HEADER_SIZE || total > src.len() {
        bail!("COMP: total size {total} for a block of {} bytes", src.len());
    }
    let stream = &src[HEADER_SIZE..total];
    if r.u16_at(6)? == 1 {
        if stream.len() > max_size {
            bail!("COMP: {} stored bytes are above {max_size}", stream.len());
        }
        return Ok(stream.to_vec());
    }
    let mut out = Vec::new();
    let (mut i, mut flags) = (0usize, 1u32);
    let truncated = || anyhow::anyhow!("COMP: truncated input");
    while i < stream.len() {
        if flags == 1 {
            let lo = *stream.get(i).ok_or_else(truncated)? as u32;
            let hi = *stream.get(i + 1).ok_or_else(truncated)? as u32;
            flags = lo | hi << 8 | 0x1_0000;
            i += 2;
            continue;
        }
        if flags & 1 == 1 {
            let &[b0, b1] = stream.get(i..i + 2).ok_or_else(truncated)? else { unreachable!() };
            i += 2;
            let distance = ((b0 as usize) & 0xF0) << 4 | b1 as usize;
            let length = (b0 as usize & 0x0F) + 3;
            if distance == 0 || distance > out.len() {
                bail!("COMP: back-reference before start of output");
            }
            if out.len() + length > max_size {
                bail!("COMP: output above {max_size} bytes");
            }
            for _ in 0..length {
                out.push(out[out.len() - distance]);
            }
        } else {
            if out.len() >= max_size {
                bail!("COMP: output above {max_size} bytes");
            }
            out.push(stream[i]);
            i += 1;
        }
        flags >>= 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(magic: &[u8; 4], b4: u8, w6: u16, w8: u32, w12: u32) -> Vec<u8> {
        let mut h = magic.to_vec();
        h.push(b4);
        h.push(0);
        h.extend_from_slice(&w6.to_le_bytes());
        h.extend_from_slice(&w8.to_le_bytes());
        h.extend_from_slice(&w12.to_le_bytes());
        h
    }

    #[test]
    fn raww_is_stored() {
        let mut b = header(RAWW, 1, 0, 3, 0);
        b.extend_from_slice(b"abcXX");
        assert_eq!(raww(&b, 10).unwrap(), b"abc");
        assert!(raww(&b, 2).is_err());
        b[4] = 2;
        assert!(raww(&b, 10).is_err(), "the original takes version 1 only");
    }

    #[test]
    fn comp_stored_and_compressed() {
        let mut stored = header(COMP, 0, 1, 0, 16 + 4);
        stored.extend_from_slice(b"wxyz");
        assert_eq!(comp(&stored, 100).unwrap(), b"wxyz");
        // Flags 0b0100: two literals, then a reference 2 back of 3 + 2 bytes.
        let stream = [0b100u8, 0, b'a', b'b', 0x02, 0x02];
        let mut b = header(COMP, 0, 0, 0, 16 + stream.len() as u32);
        b.extend_from_slice(&stream);
        assert_eq!(comp(&b, 100).unwrap(), b"abababa");
        assert!(comp(&b, 4).is_err());
    }
}
