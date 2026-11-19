//! Texture animations (flip-books such as water and blinking signs).
//!
//! ```text
//! 0xB0300100 container
//! ├─ 0x30300101  header (u32 count, …)
//! ├─ 0x30300102  0x34-byte records: char name[24], u32 hash, u32 frame
//! │              count, u32 frames per second, runtime fields
//! └─ 0x30300103  frame table, 0x10 bytes per frame: u32 texture hash, pad;
//!                the animations' frames back to back, in record order
//! ```
//!
//! A solid refers to the animation by its base hash, which is also the hash
//! of frame 0. Checked on Bayview: `ANM_WATERA_` has 14 frames at 12 fps,
//! and its frames are `ANM_WATERA_`, `ANM_WATERA_001`, `ANM_WATERA_002`, …

use anyhow::{Context, Result};

use super::bundle::Chunk;
use super::reader::Reader;

pub const TEXTURE_ANIMATIONS: u32 = 0xB030_0100;
const RECORDS: u32 = 0x3030_0102;
const FRAMES: u32 = 0x3030_0103;

#[derive(Clone, Debug)]
pub struct TextureAnimation {
    pub name: String,
    pub hash: u32,
    pub fps: u32,
    pub frames: Vec<u32>,
}

impl TextureAnimation {
    /// The texture shown at `seconds` since the start.
    pub fn frame_at(&self, seconds: f32) -> u32 {
        let n = self.frames.len().max(1);
        let i = (seconds * self.fps as f32) as usize % n;
        // No frames (a damaged record): texture 0, which nothing matches.
        self.frames.get(i).copied().unwrap_or(0)
    }
}

pub fn parse_texture_animations(chunk: Chunk<'_>) -> Result<Vec<TextureAnimation>> {
    const RECORD_SIZE: usize = 0x34;
    const FRAME_SIZE: usize = 0x10;
    let records = Reader::new(chunk.child(RECORDS).context("animation records")?.body);
    let frames = Reader::new(chunk.child(FRAMES).context("animation frames")?.body);
    // Frames whose hash is in the table (`i * FRAME_SIZE + 4 <= len`): a
    // record's count past them reads nothing, so it need not be walked
    // (a count of 4 billion used to loop that often).
    let present = (frames.len() + FRAME_SIZE - 4) / FRAME_SIZE;
    let mut next_frame = 0usize;
    let mut out = Vec::new();
    for r in records.whole_records(RECORD_SIZE) {
        let count = r.u32_at(0x1C)? as usize;
        let end = next_frame.saturating_add(count);
        let frames: Vec<u32> = (next_frame.min(present)..end.min(present)).filter_map(|i| frames.u32_at(i * FRAME_SIZE).ok()).collect();
        next_frame = end;
        out.push(TextureAnimation { name: r.cstr_at(0, 0x18)?, hash: r.u32_at(0x18)?, fps: r.u32_at(0x20)?, frames });
    }
    Ok(out)
}

crate::persist!(TextureAnimation { name, hash, fps, frames });
