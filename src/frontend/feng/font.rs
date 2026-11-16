//! Bitmap fonts (LANGUAGES/<language>.bin, chunk 0x30201).
//!
//! ```text
//! +0x000  name, file name (NUL-separated)
//! +0x100  texture name ("conduitmditc_tt14i"), "bmp", file name
//! +0x200  "FNTF" header:
//!         +0x0A u16 glyph count   +0x0C u32 flags (0x40000: 16-byte
//!         glyphs, all the shipped fonts)   +0x12 u16 (not used)
//!         +0x14 u32 glyph table (from FNTF)
//!         +0x18 u32 kerning table   +0x24 i32 line height
//!         +0x40 f32 1/texture width   +0x44 f32 1/texture height
//! glyph: u16 code, u8 width, u8 height, u16 x, u16 y,
//!        u8 —, i8 x offset, i8 y offset, u8 kerning count,
//!        u16 first kerning pair, i16 advance
//! kerning: table + 4 + 4 × pair: u16 previous character, i8 adjust, u8 —
//! ```
//!
//! Read as the renderer does: the line height into the font object at
//! 0x51BDC0, glyph offsets and cell at 0x51BFC0, kerning at 0x6F45F0.

use anyhow::{Result, bail};

use crate::formats::bundle::{read_cstr, read_f32, read_u16, read_u32};

#[derive(Clone, Debug)]
pub struct Glyph {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub advance: f32,
    /// (previous character, adjust) pairs.
    pub kerning: Vec<(u16, f32)>,
}

impl Glyph {
    /// The pen adjustment after `previous` (0x6F45F0).
    pub fn kerning(&self, previous: char) -> f32 {
        self.kerning.iter().find(|(c, _)| *c as u32 == previous as u32).map_or(0.0, |(_, k)| *k)
    }
}

#[derive(Clone, Debug)]
pub struct Font {
    pub name: String,
    pub texture: String,
    pub line_height: f32,
    pub texture_size: [f32; 2],
    pub glyphs: crate::fxhash::FxHashMap<u32, Glyph>,
}

impl Font {
    pub fn parse(b: &[u8]) -> Result<Font> {
        let h = 0x200;
        if b.len() < h + 0x50 || &b[h..h + 4] != b"FNTF" {
            bail!("no FNTF header");
        }
        let count = read_u16(b, h + 0x0A) as usize;
        let line_height = read_u32(b, h + 0x24) as i32 as f32;
        let kerning = h + read_u32(b, h + 0x18) as usize + 4;
        let table = h + read_u32(b, h + 0x14) as usize;
        let inv = [read_f32(b, h + 0x40), read_f32(b, h + 0x44)];
        let texture_size =
            [if inv[0] > 0.0 { 1.0 / inv[0] } else { 256.0 }, if inv[1] > 0.0 { 1.0 / inv[1] } else { 256.0 }];
        let mut glyphs = crate::fxhash::FxHashMap::default();
        for i in 0..count {
            let g = table + i * 16;
            if g + 16 > b.len() {
                break;
            }
            let first = read_u16(b, g + 12) as usize;
            let kerning = (0..b[g + 11] as usize)
                .map(|k| kerning + (first + k) * 4)
                .filter(|&e| e + 4 <= b.len())
                .map(|e| (read_u16(b, e), b[e + 2] as i8 as f32))
                .collect();
            glyphs.insert(
                read_u16(b, g) as u32,
                Glyph {
                    width: b[g + 2] as f32,
                    height: b[g + 3] as f32,
                    x: read_u16(b, g + 4) as f32,
                    y: read_u16(b, g + 6) as f32,
                    x_offset: b[g + 9] as i8 as f32,
                    y_offset: b[g + 10] as i8 as f32,
                    advance: read_u16(b, g + 14) as i16 as f32,
                    kerning,
                },
            );
        }
        Ok(Font { name: read_cstr(b, 0, 0x20), texture: read_cstr(b, 0x100, 0x20), line_height, texture_size, glyphs })
    }

    /// Width of `text` in font pixels: the advances plus the kerning
    /// after each previous character (0x50BCE0).
    pub fn measure(&self, text: &str) -> f32 {
        let mut previous = None;
        let mut width = 0.0;
        for c in text.chars() {
            if let Some(g) = self.glyphs.get(&(c as u32)) {
                width += g.advance + previous.map_or(0.0, |p| g.kerning(p));
            }
            previous = Some(c);
        }
        width
    }
}
