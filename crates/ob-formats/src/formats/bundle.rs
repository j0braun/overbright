//! The chunk container every NFSU2 `.BUN`/`.BIN` file is built from.
//!
//! A file is a flat sequence of chunks:
//!
//! ```text
//! u32 id      — bit 31 set means "this chunk only contains other chunks"
//! u32 size    — byte length of the body that follows
//! u8  body[size]
//! ```
//!
//! Chunks with id `0` are alignment filler. Inside a body, the game often pads
//! with `0x11` bytes so that the real payload starts on a 16- or 128-byte
//! boundary *in the file* (the engine loads the file at an aligned address and
//! uses the bytes in place). [`Chunk::payload`] strips that filler.

use anyhow::{Result, bail};

use super::reader::Reader;

/// Chunk ids we know. Names follow what the data turned out to contain.
pub mod id {
    // Solid (mesh) lists.
    pub const SOLID_LIST: u32 = 0x8013_4000;
    pub const SOLID_LIST_HEADER: u32 = 0x8013_4001;
    pub const SOLID_LIST_INFO: u32 = 0x0013_4002;
    pub const SOLID_LIST_HASHES: u32 = 0x0013_4003;
    pub const SOLID: u32 = 0x8013_4010;
    pub const SOLID_HEADER: u32 = 0x0013_4011;
    pub const SOLID_TEXTURES: u32 = 0x0013_4012;
    pub const SOLID_MESH: u32 = 0x8013_4100;
    pub const MESH_HEADER: u32 = 0x0013_4900;
    pub const MESH_VERTICES: u32 = 0x0013_4B01;
    pub const MESH_MATERIALS: u32 = 0x0013_4B02;
    pub const MESH_INDICES: u32 = 0x0013_4B03;

    // Scenery (placed instances of solids).
    pub const SCENERY_SECTION: u32 = 0x8003_4100;
    pub const SCENERY_HEADER: u32 = 0x0003_4101;
    pub const SCENERY_INFOS: u32 = 0x0003_4102;
    pub const SCENERY_INSTANCES: u32 = 0x0003_4103;

    // Texture packs (TPK).
    pub const TEXTURE_PACK: u32 = 0xB330_0000;
    pub const TPK_HEADER: u32 = 0xB331_0000;
    pub const TPK_INFO: u32 = 0x3331_0001;
    pub const TPK_HASHES: u32 = 0x3331_0002;
    pub const TPK_TEXTURES: u32 = 0x3331_0004;
    pub const TPK_COMPRESSED_BLOCKS: u32 = 0x3331_0003;
    pub const TPK_FORMATS: u32 = 0x3331_0005;
    pub const TPK_DATA_BLOCK: u32 = 0xB332_0000;
    pub const TPK_DATA: u32 = 0x3332_0002;

    // Track-level tables (in `TRACKS/L4RA.BUN`).
    pub const TRACK_STREAMING_SECTIONS: u32 = 0x0003_4110;
}

/// One chunk, borrowed from the file bytes.
#[derive(Clone, Copy)]
pub struct Chunk<'a> {
    pub id: u32,
    /// The body, without the 8-byte header.
    pub body: &'a [u8],
    /// Absolute file offset of `body[0]` — needed to reproduce the game's
    /// alignment rules.
    pub offset: usize,
}

impl<'a> Chunk<'a> {
    pub fn is_container(&self) -> bool {
        self.id & 0x8000_0000 != 0
    }

    /// The direct children of a container chunk.
    pub fn children(&self) -> ChunkIter<'a> {
        ChunkIter { data: self.body, base: self.offset, pos: 0 }
    }

    /// The first direct child with the given id.
    pub fn child(&self, id: u32) -> Option<Chunk<'a>> {
        self.children().find(|c| c.id == id)
    }

    /// The body with leading `0x11` filler removed: the payload starts at a
    /// 16-byte file-aligned offset, possibly after further whole 16-byte
    /// groups of `0x11` (vertex buffers are pushed out to 128 bytes this way).
    ///
    /// The filler ends at the next 128-byte file offset (the
    /// documented rule); a payload that itself starts with 16 bytes of 0x11
    /// keeps them. (Equal to the old unbounded strip on every shipped file:
    /// tests/fuzz_smoke.rs `payload_filler_ends_at_128`.)
    pub fn payload(&self) -> &'a [u8] {
        let mut data = self.aligned(16);
        let mut at = self.offset + (self.body.len() - data.len());
        while at % 128 != 0
            && let Some((group, rest)) = data.split_first_chunk::<16>()
            && group.iter().all(|&b| b == 0x11)
        {
            data = rest;
            at += 16;
        }
        data
    }

    /// The body starting at the first file-absolute offset that is a multiple
    /// of `align`.
    pub fn aligned(&self, align: usize) -> &'a [u8] {
        let start = self.offset.div_ceil(align) * align;
        self.body.get(start - self.offset..).unwrap_or(&[])
    }
}

/// Iterates sibling chunks. Stops (rather than panicking) at a truncated tail.
pub struct ChunkIter<'a> {
    data: &'a [u8],
    base: usize,
    pos: usize,
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Chunk<'a>> {
        let r = Reader::new(self.data);
        loop {
            let id = r.u32_at(self.pos).ok()?;
            let size = r.u32_at(self.pos + 4).ok()? as usize;
            let start = self.pos + 8;
            let body = r.bytes_at(start, size).ok()?;
            self.pos = start + size;
            if id == 0 {
                continue; // alignment filler
            }
            return Some(Chunk { id, body, offset: self.base + start });
        }
    }
}

/// The top-level chunks of a whole file (or of a slice that starts at file
/// offset `base`).
pub fn chunks(data: &[u8], base: usize) -> ChunkIter<'_> {
    ChunkIter { data, base, pos: 0 }
}

/// Depth-first visit of every chunk under (and including) `chunks`.
pub fn walk<'a>(chunks: ChunkIter<'a>, visit: &mut impl FnMut(Chunk<'a>)) {
    // An explicit stack instead of recursion: a container needs only an
    // 8-byte header to hold the next one, so a hostile file nests as deep
    // as it is long / 8 and would overflow the call stack. Same
    // depth-first order as before.
    let mut stack = vec![chunks];
    while let Some(top) = stack.last_mut() {
        let Some(chunk) = top.next() else {
            stack.pop();
            continue;
        };
        visit(chunk);
        if chunk.is_container() {
            stack.push(chunk.children());
        }
    }
}

// --- little-endian readers ------------------------------------------------
//
// These index the slice directly and panic past its end. They stay for the
// callers outside `formats` (a later step switches them); the parsers in
// `formats` read through the checked [`Reader`].

pub fn read_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

pub fn read_i16(b: &[u8], at: usize) -> i16 {
    read_u16(b, at) as i16
}

pub fn read_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

pub fn read_f32(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

pub fn read_vec3(b: &[u8], at: usize) -> [f32; 3] {
    [read_f32(b, at), read_f32(b, at + 4), read_f32(b, at + 8)]
}

/// A NUL-terminated string in a fixed-size field.
pub fn read_cstr(b: &[u8], at: usize, max: usize) -> String {
    let field = &b[at..(at + max).min(b.len())];
    let len = field.iter().position(|&c| c == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..len]).into_owned()
}

pub fn ensure_len(b: &[u8], need: usize, what: &str) -> Result<()> {
    if b.len() < need {
        bail!("{what}: need {need} bytes, have {}", b.len());
    }
    Ok(())
}

#[cfg(test)]
mod walk_tests {
    use super::*;

    /// A file of containers nested 200 000 deep (1.6 MB) used to overflow
    /// the stack; every level is still visited, in order.
    #[test]
    fn walk_survives_deep_nesting() {
        let depth = 200_000usize;
        let mut data = Vec::with_capacity(depth * 8);
        for i in 0..depth {
            data.extend_from_slice(&0x8000_0001u32.to_le_bytes());
            data.extend_from_slice(&(((depth - i - 1) * 8) as u32).to_le_bytes());
        }
        let mut n = 0usize;
        walk(chunks(&data, 0), &mut |_| n += 1);
        assert_eq!(n, depth);
    }
}
