//! EA's Huffman codec ("HUFF" blocks), ported from SPEED2.EXE 0x573850.
//!
//! Container (same shape as JDLZ): `"HUFF" 01 10 00 00`, u32 unpacked
//! size, u32 packed size, then a big-endian, MSB-first bit stream:
//!
//! ```text
//! u16 type        0x30FB plain, 0x32FB delta, 0x34FB double delta;
//!                 +0x8000: sizes are 32-bit instead of 24-bit;
//!                 +0x0100: a packed size follows first (skipped)
//! u24/u32 size    unpacked size
//! u8  clue        escape symbol
//! counts          for bit lengths 1, 2, …: number of codes of that length
//!                 (stops once the canonical code space is full)
//! symbols         one per code, delta-coded against the unused byte values
//! data            canonical Huffman codes; the clue symbol escapes into
//!                 "repeat previous byte n times", "literal byte" or "end"
//! ```
//!
//! Numbers in the header use a gamma-like code (`read_number`).
//! Checked: all 3 HUFF blocks in LOC4DYNTEX.BIN decode to their declared
//! size and end in a well-formed texture entry.

use anyhow::{Result, bail};

use super::reader::{Limits, Reader};

pub const MAGIC: &[u8; 4] = b"HUFF";
const HEADER_SIZE: usize = 16;

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    /// Bits not yet consumed, left-aligned.
    buffer: u64,
    count: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0, buffer: 0, count: 0 }
    }

    fn fill(&mut self, need: u32) {
        while self.count < need {
            // Past the end the game reads garbage; we feed zeros.
            let byte = self.data.get(self.pos).copied().unwrap_or(0) as u64;
            self.pos += 1;
            self.buffer |= byte << (56 - self.count);
            self.count += 8;
        }
    }

    fn peek(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        self.fill(n);
        (self.buffer >> (64 - n)) as u32
    }

    fn take(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.buffer <<= n;
        self.count -= n;
        v
    }

    /// Gamma-like number: `1xx` → 0..3; otherwise `z` zeros, a one, then
    /// `z + 2` bits `v` → `v - 4 + 2^(z+2)`.
    fn read_number(&mut self) -> u32 {
        if self.take(1) == 1 {
            return self.take(2);
        }
        let mut n = 2;
        loop {
            n += 1;
            if self.take(1) == 1 {
                break;
            }
            if n > 32 {
                return 0; // corrupt stream; stop the caller from looping
            }
        }
        // `n` is now the number of zeros read plus two.
        let v = self.take(n) as u64;
        (v + (1u64 << n) - 4) as u32
    }
}

/// Largest output accepted when the caller does not know the real size. The
/// game's biggest HUFF payload is a few MB of texture; the 32-bit size field
/// and unbounded repeat runs would otherwise reserve up to 4 GB.
const DEFAULT_MAX_SIZE: usize = Limits::DEFAULT.max_unpacked;

pub fn decompress(src: &[u8]) -> Result<Vec<u8>> {
    decompress_max(src, DEFAULT_MAX_SIZE)
}

/// `decompress`, refusing blocks that declare more than `max_size` bytes.
pub fn decompress_max(src: &[u8], max_size: usize) -> Result<Vec<u8>> {
    let header = Reader::new(src);
    if src.len() < HEADER_SIZE || &header.array_at::<4>(0)? != MAGIC {
        bail!("not a HUFF block");
    }
    let declared = header.u32_at(8)? as usize;
    if declared > max_size {
        bail!("HUFF: declared size {declared} exceeds the limit {max_size}");
    }
    let mut bits = Bits::new(header.bytes_at(HEADER_SIZE, src.len() - HEADER_SIZE)?);

    let kind = bits.take(16);
    let wide = kind & 0x8000 != 0;
    if kind & 0x100 != 0 {
        bits.take(if wide { 32 } else { 24 });
    }
    let size = bits.take(if wide { 32 } else { 24 }) as usize;
    if size != declared {
        bail!("HUFF: stream size {size} differs from header {declared}");
    }
    let clue = bits.take(8) as u8;

    // Code-length counts, canonical code ranges per length.
    let mut counts = vec![0u32; 1]; // counts[len]
    let mut first_index = vec![0i64; 1]; // code - first_index[len] = symbol index
    let (mut code, mut total) = (0u32, 0u32);
    loop {
        let len = counts.len() as u32;
        if len > 16 {
            bail!("HUFF: code lengths beyond 16 bits");
        }
        first_index.push(code as i64 * 2 - total as i64);
        let n = bits.read_number();
        // A byte has at most 256 distinct values, so more than 256 codes
        // cannot be a valid table (and the symbol loop below would spin).
        total = total.saturating_add(n);
        if total > 256 {
            bail!("HUFF: {total} symbols (at most 256)");
        }
        code = code.wrapping_mul(2).wrapping_add(n);
        counts.push(n);
        // Codes of this length end at `code << (16 - len)`; wrapping to 0
        // means the code space is full.
        if n != 0 && (code << (16 - len)) & 0xFFFF == 0 {
            break; // code space full
        }
    }
    let max_len = counts.len() - 1;

    // Symbols, each coded as "skip this many unused byte values".
    let mut used = [false; 256];
    let mut symbols = Vec::with_capacity(total as usize);
    let mut sym: u8 = 0xFF;
    for _ in 0..total {
        // Skips over the still-unused byte values: at most that many left.
        let unused = 256 - symbols.len() as u32;
        let mut skip = bits.read_number().saturating_add(1);
        if skip > unused {
            bail!("HUFF: symbol skip {skip} beyond the {unused} unused values");
        }
        loop {
            sym = sym.wrapping_add(1);
            if !used[sym as usize] {
                skip -= 1;
                if skip == 0 {
                    break;
                }
            }
        }
        used[sym as usize] = true;
        symbols.push(sym);
    }

    let mut out = Vec::with_capacity(size.min(src.len().saturating_mul(64)));
    // Symbols in code order: lengths ascending, codes ascending within.
    let decode_symbol = |bits: &mut Bits| -> Option<u8> {
        let top = bits.peek(16);
        let mut code_start = 0u32;
        for len in 1..=max_len {
            let n = counts[len];
            let code = top >> (16 - len);
            let end = code_start.wrapping_add(n);
            if code < end {
                bits.take(len as u32);
                let index = code as i64 - first_index[len];
                return symbols.get(index as usize).copied();
            }
            code_start = end.wrapping_mul(2);
        }
        None
    };

    while out.len() < size {
        // Past the end the reader feeds zeros (code 0 = first symbol), which
        // would never error: a real stream never consumes more than it has
        // (the reader runs at most 8 bytes ahead).
        if bits.pos > bits.data.len() + 16 {
            bail!("HUFF: stream ends before {size} bytes");
        }
        let Some(s) = decode_symbol(&mut bits) else { bail!("HUFF: invalid code") };
        if s != clue {
            out.push(s);
            continue;
        }
        let run = bits.read_number();
        if run == 0 {
            if bits.take(1) == 1 {
                break; // end of stream
            }
            out.push(bits.take(8) as u8);
        } else {
            let Some(&previous) = out.last() else { bail!("HUFF: run with no previous byte") };
            let run = (run as usize).min(size - out.len());
            out.extend(std::iter::repeat_n(previous, run));
        }
    }
    out.truncate(size);

    match kind & 0x7EFF {
        0x32FB => {
            let mut acc = 0u8;
            for b in &mut out {
                acc = acc.wrapping_add(*b);
                *b = acc;
            }
        }
        0x34FB => {
            let (mut d, mut acc) = (0u8, 0u8);
            for b in &mut out {
                d = d.wrapping_add(*b);
                acc = acc.wrapping_add(d);
                *b = acc;
            }
        }
        _ => {}
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(declared: u32) -> Vec<u8> {
        let mut b = b"HUFF\x01\x10\0\0".to_vec();
        b.extend_from_slice(&declared.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b
    }

    /// A count of 512 codes of length 1 (which fills the code space, so the
    /// table "ends") used to spin forever in the symbol loop once all 256
    /// byte values were used. Count bits: `0000000 1 000000100` = 512.
    #[test]
    fn too_many_symbols_is_an_error_not_a_hang() {
        let mut b = header(4);
        b.extend_from_slice(&[0x30, 0xFB, 0, 0, 4, 0]);
        b.extend_from_slice(&[0x01, 0x02, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(decompress(&b).is_err());
    }

    /// A 4 GB declared size is refused before anything is reserved, and a
    /// caller-supplied limit is honoured.
    #[test]
    fn declared_size_is_capped() {
        let mut b = header(0xFFFF_FFFF);
        b.extend_from_slice(&[0; 32]);
        assert!(decompress(&b).is_err());
        let mut b = header(1000);
        b.extend_from_slice(&[0; 32]);
        assert!(decompress_max(&b, 999).is_err());
    }

    /// An exhausted stream errors instead of decoding zero bits to the end
    /// of a large declared size.
    #[test]
    fn exhausted_stream_stops() {
        // kind 0x30FB, size 0x0FFFFF, clue 0xFF, counts: `1 01`=1 then ..
        let mut b = header(0x0F_FFFF);
        // 1 symbol of length 1 -> code space needs 2 codes of length 1:
        // counts `110`=2 fills the space; symbols `100`=skip 1 (sym 0),
        // `101`=skip 2 (sym 2).
        b.extend_from_slice(&[0x30, 0xFB, 0x0F, 0xFF, 0xFF, 0xFF, 0b1101_0010, 0b1000_0000]);
        assert!(decompress(&b).is_err());
    }
}
