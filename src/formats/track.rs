//! A track ("location") is two files:
//!
//! * `TRACKS/L4RA.BUN` — small, always resident: the streaming-section table,
//!   drivable-region polygons, visibility lists, …
//! * `TRACKS/STREAML4RA.BUN` — everything else, cut into *sections* the game
//!   streams in and out as the car moves.
//!
//! Section names are a letter plus a number (`A12`). Letters A–W are regions of
//! the city; `X` sections hold props shared by many regions, `Y` sections
//! shared texture packs, `Z0` a single global scenery group.

use anyhow::{Context, Result};

use super::bundle::{chunks, id};
use super::reader::Reader;

#[derive(Clone, Debug)]
pub struct StreamingSection {
    pub name: String,
    pub number: u32,
    /// Byte range in the STREAM file.
    pub file_offset: u32,
    pub size: u32,
}

const SECTION_RECORD_SIZE: usize = 0x50;

pub fn parse_streaming_sections(track_bun: &[u8]) -> Result<Vec<StreamingSection>> {
    let table = chunks(track_bun, 0)
        .find(|c| c.id == id::TRACK_STREAMING_SECTIONS)
        .context("no streaming-section table")?;
    Reader::with_base(table.body, table.offset)
        .whole_records(SECTION_RECORD_SIZE)
        .map(|r| {
            Ok(StreamingSection {
                name: r.cstr_at(0, 8)?,
                number: r.u32_at(0x08)?,
                file_offset: r.u32_at(0x14)?,
                size: r.u32_at(0x18)?,
            })
        })
        .collect()
}

crate::persist!(StreamingSection { name, number, file_offset, size });
