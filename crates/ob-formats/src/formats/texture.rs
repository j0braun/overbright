//! Texture packs (TPK).
//!
//! ```text
//! TEXTURE_PACK (0xB3300000)
//! ├─ TPK_HEADER (0xB3310000)
//! │  ├─ TPK_INFO     (0x33310001)  pack name and path
//! │  ├─ TPK_HASHES   (0x33310002)  sorted texture hashes
//! │  ├─ TPK_TEXTURES (0x33310004)  0x7C-byte entry per texture
//! │  │  or TPK_COMPRESSED_BLOCKS (0x33310003), see parse_compressed_pack
//! │  └─ TPK_FORMATS  (0x33310005)  0x20-byte D3D format record per texture
//! └─ TPK_DATA_BLOCK (0xB3320000)
//!    └─ TPK_DATA    (0x33320002)  pixel data, 128-byte aligned; entry
//!                                  offsets are relative to that start
//! ```

use anyhow::{Context, Result, bail};

use super::bundle::{Chunk, id, read_u16, read_u32};
use super::reader::{Limits, Reader};
use super::{huff, jdlz};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// DXT1 / BC1: 8 bytes per 4x4 block, 1-bit alpha.
    Dxt1,
    /// DXT3 / BC2: 16 bytes per block, explicit 4-bit alpha.
    Dxt3,
    /// DXT5 / BC3: 16 bytes per block, interpolated alpha.
    Dxt5,
    /// 8-bit indices into a 256-entry BGRA palette.
    P8,
    /// 32-bit BGRA.
    Bgra8,
    /// 32-bit RGBA, for textures the clone generates itself.
    Rgba8,
    /// BC7: 16 bytes per block. Never in the game's files: only Overbright
    /// texture packs compiled from PNG (`obpack`, docs/modding/texture-packs.md).
    Bc7,
}

impl PixelFormat {
    /// The `ImageCompressionType` byte stored in each texture entry.
    /// 0x22/0x24/0x08 are confirmed by the per-texture D3D format records
    /// (`DXT1`, `DXT3`, D3DFMT_P8 = 41); 0x26 and 0x20 follow the same scheme.
    fn from_compression_byte(b: u8) -> Option<Self> {
        match b {
            0x22 => Some(Self::Dxt1),
            0x24 => Some(Self::Dxt3),
            0x26 => Some(Self::Dxt5),
            // 0x80 / 0x81: the vinyls' colour images (CARS/<car>/VINYLS.BIN;
            // 0x81 the unique and contest ones). Their D3D format records
            // say D3DFMT_P8 (41) with a 0x400-byte palette, as for 0x08:
            // all 25,911 0x08, 24,071 0x80 and 1,840 0x81 images.
            0x08 | 0x80 | 0x81 => Some(Self::P8),
            0x20 => Some(Self::Bgra8),
            _ => None,
        }
    }

    pub fn is_block_compressed(self) -> bool {
        matches!(self, Self::Dxt1 | Self::Dxt3 | Self::Dxt5 | Self::Bc7)
    }

    /// Bytes for one mip level of the given size.
    pub fn level_size(self, width: u32, height: u32) -> usize {
        // In usize: a 16-bit width and height multiply past u32.
        let (w, h) = (width as usize, height as usize);
        let blocks = w.div_ceil(4) * h.div_ceil(4);
        match self {
            Self::Dxt1 => blocks * 8,
            Self::Dxt3 | Self::Dxt5 | Self::Bc7 => blocks * 16,
            Self::P8 => w * h,
            Self::Bgra8 | Self::Rgba8 => w * h * 4,
        }
    }
}

/// Frame-buffer blending for a texture, exactly as SPEED2.EXE sets it when
/// it binds the texture (function 0x5BCC40, switch on entry byte 0x56).
/// Every blended mode also turns depth writes off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Blend {
    /// 0x56 = 0: no blending.
    None,
    /// 0x56 = 1, and any value without a case of its own: SRCALPHA / INVSRCALPHA.
    Alpha,
    /// 0x56 = 2: SRCALPHA / ONE.
    Additive,
    /// 0x56 = 5: DESTALPHA / INVDESTALPHA.
    DestAlpha,
    /// 0x56 = 6: DESTALPHA / ONE.
    DestAlphaAdditive,
}

impl Blend {
    pub fn from_entry_byte(b: u8) -> Blend {
        match b {
            0 => Blend::None,
            2 => Blend::Additive,
            5 => Blend::DestAlpha,
            6 => Blend::DestAlphaAdditive,
            _ => Blend::Alpha,
        }
    }
}

/// Texture-coordinate scrolling, as SPEED2.EXE computes it when binding a
/// texture (0x5BCC40 → 0x5B7D50/0x5B7D90 → 0x48B450). With `t` in seconds
/// (EA's 4000 Hz tick counter × 0.00025):
///
/// * kind 0: no offset;
/// * kind 1: `x = t * speed`, offset = `x - trunc(x)`;
/// * kind 2: the same with `t` snapped to whole steps: `x = trunc(t / step) * speed`;
/// * kind 3: a fixed offset, `-fixed / 1024`.
///
/// The offset is added to the UVs by world.efx (`add oT0, v2, TextureOffset`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    pub kind: u8,
    /// Entry +0x5A / +0x5C, divided by 1024.
    pub speed: [f32; 2],
    /// Entry +0x58, divided by 256.
    pub step: f32,
    /// Entry +0x5E / +0x60, scaled by -1/1024 (kind 3).
    pub fixed: [f32; 2],
}

impl Scroll {
    fn from_entry(e: &Reader<'_>) -> Result<Scroll> {
        let s16 = |at: usize| e.i16_at(at).map(|v| v as f32);
        Ok(Scroll {
            kind: e.u8_at(0x52)?,
            speed: [s16(0x5A)? / 1024.0, s16(0x5C)? / 1024.0],
            step: s16(0x58)? / 256.0,
            fixed: [s16(0x5E)? * -1.0 / 1024.0, s16(0x60)? * -1.0 / 1024.0],
        })
    }

    pub fn offset(&self, t: f32) -> [f32; 2] {
        let frac = |x: f32| x - x.trunc();
        match self.kind {
            0 => [0.0, 0.0],
            3 => self.fixed,
            kind => {
                let t = if kind == 2 && self.step != 0.0 { (t / self.step).trunc() } else { t };
                [frac(t * self.speed[0]), frac(t * self.speed[1])]
            }
        }
    }
}

/// Alpha-test reference the game uses for cut-out textures (0x5BCC40 passes
/// 95 when entry byte 0x55 is 1).
pub const ALPHA_TEST_REF: u8 = 95;

#[derive(Clone, Debug)]
pub struct Texture {
    pub name: String,
    pub hash: u32,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// Mip levels actually present in `data` (at least 1).
    pub mip_count: u32,
    /// All mip levels, largest first.
    pub data: crate::cache::Bytes,
    /// 256 BGRA entries for [`PixelFormat::P8`], empty otherwise.
    pub palette: Vec<u8>,
    /// Entry byte 0x55 == 1: discard texels with alpha <= [`ALPHA_TEST_REF`].
    pub alpha_test: bool,
    pub blend: Blend,
    /// Draw-order bucket (entry byte 0x51); glows use 5.
    pub rendering_order: u8,
    /// Entry byte 0x54: non-zero puts the texture's draws in the second
    /// set of buckets (0x8712F0), flushed after the first (0x870D30) by
    /// 0x5CF430 (see `render`'s draw order).
    pub late_buckets: bool,
    pub scroll: Scroll,
}

const ENTRY_SIZE: usize = 0x7C;

pub fn parse_texture_pack(pack: Chunk<'_>) -> Result<Vec<Texture>> {
    parse_texture_pack_where(pack, &|_| true)
}

/// [`parse_texture_pack`], keeping only the textures whose hash `keep`
/// takes (a caller that wants a few of a pack's textures, as the
/// car's vinyls of `VINYLS.BIN`, need not decompress every block). A
/// compressed block's record names its texture's hash (+0x00, the same as
/// the entry's +0x24 in every shipped pack; test
/// `vinyl_packs_filtered_parse_matches_full`), so an unwanted block is not
/// decompressed. The same textures, in the same order, as the full parse
/// filtered — unless the pack is damaged in a block the filter skips (the
/// full parse then fails as a whole; this one keeps the wanted textures).
pub fn parse_texture_pack_where(pack: Chunk<'_>, keep: &(dyn Fn(u32) -> bool + Sync)) -> Result<Vec<Texture>> {
    let header = pack.child(id::TPK_HEADER).context("TPK header")?;
    if let Some(blocks) = header.child(id::TPK_COMPRESSED_BLOCKS) {
        return parse_compressed_pack(pack, blocks, keep);
    }
    let entries = header.child(id::TPK_TEXTURES).context("TPK texture table")?;
    let data = pack
        .child(id::TPK_DATA_BLOCK)
        .and_then(|b| b.child(id::TPK_DATA))
        .context("TPK data")?
        .aligned(128);

    let mut out = Vec::new();
    // Whole entries only (a cut-off tail is ignored, as before).
    let n = entries.body.len() / ENTRY_SIZE;
    let mut table = Reader::with_base(entries.body, entries.offset);
    for e in table.records("TPK texture entries", n, ENTRY_SIZE, Limits::DEFAULT.max_textures)? {
        let e = Reader::new(e);
        if !keep(e.u32_at(0x24)?) {
            continue;
        }
        if let Some(t) = parse_entry(&e, data)? {
            out.push(t);
        }
    }
    Ok(out)
}

/// A pack whose textures are compressed one by one (`LOC4DYNTEX.BIN`).
///
/// 0x33310003 holds 0x18-byte records: `u32 hash, u32 offset, u32 packed
/// size, u32 unpacked size, u32 flags, u32 0`. The offset counts from the
/// start of the pack chunk. Unpacked, a block is the pixel data followed by
/// the usual 0x7C-byte entry and the 0x20-byte D3D format record.
fn parse_compressed_pack(pack: Chunk<'_>, blocks: Chunk<'_>, keep: &(dyn Fn(u32) -> bool + Sync)) -> Result<Vec<Texture>> {
    const RECORD_SIZE: usize = 0x18;
    const FORMAT_RECORD_SIZE: usize = 0x20;
    use rayon::prelude::*;
    let pack_start = pack.offset - 8;
    // Whole records only (a cut-off tail is ignored, as before).
    let n = blocks.body.len() / RECORD_SIZE;
    Reader::new(blocks.body).count("TPK compressed blocks", n, RECORD_SIZE, Limits::DEFAULT.max_textures)?;
    let pack_body = Reader::with_base(pack.body, pack.offset);
    // The blocks are independent: decoded in parallel, kept in order.
    let decoded: Vec<Result<Option<Texture>>> = blocks
        .body
        .par_chunks_exact(RECORD_SIZE)
        .map(|r| -> Result<Option<Texture>> {
            let r = Reader::new(r);
            let hash = r.u32_at(0)?;
            if !keep(hash) {
                return Ok(None);
            }
            let offset = (r.u32_at(4)? as usize).checked_sub(pack_start).context("block before pack")?;
            let packed = r.u32_at(8)? as usize;
            // The record's unpacked size bounds what the codecs may produce
            // (and so what they allocate); `offset - 8` and the block end are
            // checked, a block must hold the 4-byte codec magic.
            let unpacked_size = (r.u32_at(0x0C)? as usize).min(Limits::DEFAULT.max_unpacked);
            let block = offset
                .checked_sub(8)
                .and_then(|start| pack_body.bytes_at(start, packed).ok())
                .filter(|block| block.len() >= 4);
            let Some(block) = block else {
                bail!("compressed texture {hash:#010x} outside pack");
            };
            let unpacked = match Reader::new(block).array_at::<4>(0)? {
                m if &m == jdlz::MAGIC => jdlz::decompress_max(block, unpacked_size)?,
                m if &m == huff::MAGIC => huff::decompress_max(block, unpacked_size)?,
                other => {
                    log::warn!("compressed texture {hash:#010x}: unknown codec {:?}", String::from_utf8_lossy(&other));
                    return Ok(None);
                }
            };
            let entry_at = unpacked.len().checked_sub(ENTRY_SIZE + FORMAT_RECORD_SIZE).context("block too small")?;
            let (pixels, entry) = unpacked.split_at(entry_at);
            // The entry keeps the texture's offset in the *uncompressed* pack;
            // inside its own block the pixels start at 0.
            let entry = Reader::new(entry);
            let mut fields = entry.bytes_at(0, ENTRY_SIZE)?.to_vec();
            // A palette follows in the block as it followed the pixels in
            // the pack: its offset moves with theirs.
            let palette_at = entry.u32_at(0x34)?.wrapping_sub(entry.u32_at(0x30)?);
            fields[0x30..0x34].fill(0);
            fields[0x34..0x38].copy_from_slice(&palette_at.to_le_bytes());
            parse_entry(&Reader::new(&fields), pixels)
        })
        .collect();
    let mut out = Vec::new();
    // The first error in block order, as the sequential walk reported it.
    for t in decoded {
        if let Some(t) = t? {
            out.push(t);
        }
    }
    Ok(out)
}

/// One 0x7C-byte texture entry; offsets in it are relative to `data`.
fn parse_entry(e: &Reader<'_>, data: &[u8]) -> Result<Option<Texture>> {
    let name = e.cstr_at(0x0C, 0x18)?;
    let compression = e.u8_at(0x4A)?;
    let Some(format) = PixelFormat::from_compression_byte(compression) else {
        log::warn!("texture {name}: unknown compression {compression:#x}, skipped");
        return Ok(None);
    };
    let offset = e.u32_at(0x30)? as usize;
    let palette_offset = e.u32_at(0x34)? as usize;
    let size = e.u32_at(0x38)? as usize;
    let palette_size = e.u32_at(0x3C)? as usize;
    let width = e.u16_at(0x44)? as u32;
    let height = e.u16_at(0x46)? as u32;
    let stored_mips = e.u8_at(0x4E)?.max(1) as u32;
    let max_dim = Limits::DEFAULT.max_texture_dim;
    Limits::check("texture size", width.max(height) as usize, max_dim as usize)?;

    let data = Reader::new(data);
    let Ok(pixels) = data.bytes_at(offset, size) else {
        bail!("texture {name}: data {offset:#x}+{size:#x} outside pack");
    };
    let palette = if format == PixelFormat::P8 {
        data.bytes_at(palette_offset, palette_size)
            .ok()
            .with_context(|| format!("texture {name}: palette outside pack"))?
            .to_vec()
    } else {
        Vec::new()
    };

    // Keep only the mip levels whose bytes are really there.
    let mut mip_count = 0;
    let mut used = 0;
    let (mut w, mut h) = (width, height);
    while mip_count < stored_mips {
        let level = format.level_size(w, h);
        if used + level > pixels.len() {
            break;
        }
        used += level;
        mip_count += 1;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    if mip_count == 0 {
        log::warn!("texture {name}: {size} bytes is too small for {width}x{height} {format:?}");
        return Ok(None);
    }

    Ok(Some(Texture {
        name,
        hash: e.u32_at(0x24)?,
        width,
        height,
        format,
        mip_count,
        data: pixels[..used].into(),
        palette,
        alpha_test: e.u8_at(0x55)? == 1,
        blend: Blend::from_entry_byte(e.u8_at(0x56)?),
        rendering_order: e.u8_at(0x51)?,
        late_buckets: e.u8_at(0x54)? != 0,
        scroll: Scroll::from_entry(e)?,
    }))
}

impl Texture {
    /// Decode the top mip level to RGBA8 (row-major, `width * height * 4`).
    pub fn decode_rgba(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut rgba = vec![0u8; w * h * 4];
        match self.format {
            PixelFormat::Bgra8 => {
                for (dst, src) in rgba.chunks_exact_mut(4).zip(self.data.chunks_exact(4)) {
                    dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
                }
            }
            PixelFormat::Rgba8 => rgba.copy_from_slice(&self.data[..w * h * 4]),
            PixelFormat::P8 => {
                for (dst, &i) in rgba.chunks_exact_mut(4).zip(&self.data) {
                    // A palette shorter than 256 entries (damaged file):
                    // the missing entries are transparent black.
                    let p = self.palette.get(i as usize * 4..i as usize * 4 + 4).unwrap_or(&[0; 4]);
                    dst.copy_from_slice(&[p[2], p[1], p[0], p[3]]);
                }
            }
            PixelFormat::Bc7 => {
                let blocks_x = w.div_ceil(4);
                let mut texels = [0u8; 64];
                for (n, block) in self.data.chunks_exact(16).take(blocks_x * h.div_ceil(4)).enumerate() {
                    block_compression::decode::decode_block_bc7(block, &mut texels, 16);
                    let (bx, by) = ((n % blocks_x) * 4, (n / blocks_x) * 4);
                    for (i, texel) in texels.chunks_exact(4).enumerate() {
                        let (x, y) = (bx + i % 4, by + i / 4);
                        if x < w && y < h {
                            rgba[(y * w + x) * 4..][..4].copy_from_slice(texel);
                        }
                    }
                }
            }
            PixelFormat::Dxt1 | PixelFormat::Dxt3 | PixelFormat::Dxt5 => {
                let block_bytes = if self.format == PixelFormat::Dxt1 { 8 } else { 16 };
                let blocks_x = w.div_ceil(4);
                for (n, block) in self.data.chunks_exact(block_bytes).take(blocks_x * h.div_ceil(4)).enumerate() {
                    let texels = decode_block(self.format, block);
                    let (bx, by) = ((n % blocks_x) * 4, (n / blocks_x) * 4);
                    for (i, texel) in texels.iter().enumerate() {
                        let (x, y) = (bx + i % 4, by + i / 4);
                        if x < w && y < h {
                            rgba[(y * w + x) * 4..][..4].copy_from_slice(texel);
                        }
                    }
                }
            }
        }
        rgba
    }
}

fn rgb565(c: u16) -> [u8; 3] {
    let r = (c >> 11) & 31;
    let g = (c >> 5) & 63;
    let b = c & 31;
    [(r * 255 / 31) as u8, (g * 255 / 63) as u8, (b * 255 / 31) as u8]
}

/// Decode one 4x4 block into 16 RGBA texels.
fn decode_block(format: PixelFormat, block: &[u8]) -> [[u8; 4]; 16] {
    let (alpha, color) = match format {
        PixelFormat::Dxt1 => (None, block),
        _ => (Some(&block[..8]), &block[8..]),
    };
    let c0 = read_u16(color, 0);
    let c1 = read_u16(color, 2);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |wa: u32, wb: u32, d: u32| -> [u8; 4] {
        std::array::from_fn(|i| if i == 3 { 255 } else { ((a[i] as u32 * wa + b[i] as u32 * wb) / d) as u8 })
    };
    // DXT1 with c0 <= c1 is the 3-colour + transparent mode; DXT3/5 always
    // use four colours.
    let palette = if c0 > c1 || format != PixelFormat::Dxt1 {
        [mix(1, 0, 1), mix(0, 1, 1), mix(2, 1, 3), mix(1, 2, 3)]
    } else {
        [mix(1, 0, 1), mix(0, 1, 1), mix(1, 1, 2), [0, 0, 0, 0]]
    };
    let bits = read_u32(color, 4);
    let mut out: [[u8; 4]; 16] = std::array::from_fn(|i| palette[((bits >> (i * 2)) & 3) as usize]);

    match (format, alpha) {
        (PixelFormat::Dxt3, Some(a)) => {
            for (i, texel) in out.iter_mut().enumerate() {
                let nibble = (a[i / 2] >> ((i % 2) * 4)) & 0xF;
                texel[3] = nibble * 17;
            }
        }
        (PixelFormat::Dxt5, Some(a)) => {
            let (a0, a1) = (a[0] as u32, a[1] as u32);
            let table: [u8; 8] = std::array::from_fn(|i| match i {
                0 => a0 as u8,
                1 => a1 as u8,
                _ if a0 > a1 => (((8 - i as u32) * a0 + (i as u32 - 1) * a1) / 7) as u8,
                6 => 0,
                7 => 255,
                _ => (((6 - i as u32) * a0 + (i as u32 - 1) * a1) / 5) as u8,
            });
            let bits = a[2..8].iter().rev().fold(0u64, |acc, &b| (acc << 8) | b as u64);
            for (i, texel) in out.iter_mut().enumerate() {
                texel[3] = table[((bits >> (i * 3)) & 7) as usize];
            }
        }
        _ => {}
    }
    out
}

crate::persist!(Texture { name, hash, width, height, format, mip_count, data, palette: pod, alpha_test, blend, rendering_order, late_buckets, scroll });
crate::persist!(Scroll { kind, speed, step, fixed });
crate::persist_enum!(PixelFormat { Dxt1, Dxt3, Dxt5, P8, Bgra8, Rgba8 });
crate::persist_enum!(Blend { None, Alpha, Additive, DestAlpha, DestAlphaAdditive });

#[cfg(test)]
mod tests {
    use super::*;

    /// [`parse_texture_pack_where`] gives exactly the full parse's
    /// textures that pass the filter, in the same order, for every pack of
    /// every car's `VINYLS.BIN` (JDLZ-compressed blocks: the record's hash
    /// is the entry's) and of the shared `CARS/TEXTURES.BIN`; and no full
    /// parse fails (so the filter's skipped blocks hide no error).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn vinyl_packs_filtered_parse_matches_full() {
        let root = crate::testkit::game_root();
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("CARS")).unwrap().flatten().map(|e| e.path().join("VINYLS.BIN")).filter(|p| p.exists()).collect();
        files.sort();
        files.push(root.join("CARS/TEXTURES.BIN"));
        assert!(files.len() > 10, "{files:?}");
        let (mut packs, mut kept) = (0, 0);
        for file in &files {
            let data = std::fs::read(file).unwrap();
            crate::formats::bundle::walk(crate::formats::bundle::chunks(&data, 0), &mut |c| {
                if c.id != id::TEXTURE_PACK {
                    return;
                }
                let full = parse_texture_pack(c).unwrap_or_else(|e| panic!("{}: {e:#}", file.display()));
                for k in [0u32, 1, 2] {
                    let keep = |h: u32| h % 3 == k;
                    let got = parse_texture_pack_where(c, &keep).unwrap();
                    let want: Vec<&Texture> = full.iter().filter(|t| keep(t.hash)).collect();
                    assert_eq!(got.len(), want.len(), "{}", file.display());
                    for (a, b) in got.iter().zip(want) {
                        assert!(a.hash == b.hash && a.name == b.name && a.width == b.width && a.height == b.height && a.mip_count == b.mip_count && a.data == b.data && a.palette == b.palette, "{}: {}", file.display(), b.name);
                    }
                    kept += got.len();
                }
                assert!(parse_texture_pack_where(c, &|_| false).unwrap().is_empty());
                packs += 1;
            });
        }
        assert!(packs > 10 && kept > 0, "{packs} packs, {kept} kept");
    }

    /// A record whose packed size is under 4 bytes used to slice `block[..4]`
    /// and panic; one whose offset is inside the chunk header used to
    /// overflow `offset - 8 + packed`. Both are errors now.
    #[test]
    fn compressed_pack_records_with_bad_sizes_are_errors() {
        use crate::formats::bundle::chunks;
        fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
            let mut v = id.to_le_bytes().to_vec();
            v.extend_from_slice(&(body.len() as u32).to_le_bytes());
            v.extend_from_slice(body);
            v
        }
        let record = |offset: u32, packed: u32| {
            let mut r = Vec::new();
            for v in [1u32, offset, packed, 16, 0, 0] {
                r.extend_from_slice(&v.to_le_bytes());
            }
            r
        };
        // The pack chunk starts at file offset 0 (body at 8); its blocks
        // region is 64 bytes of zeros.
        for (offset, packed) in [(16u32, 2u32), (3, 0xFFFF_FFFF), (8, 0), (16, 0xFFFF_FFFF)] {
            let pack_body = chunk(0x3331_0001, &[0u8; 64]);
            let blocks = chunk(0x3331_0003, &record(offset, packed));
            let mut file = chunk(id::TEXTURE_PACK, &[pack_body, blocks].concat());
            // Container chunk: ids with bit 31 hold children.
            file[3] |= 0x80;
            let pack = chunks(&file, 0).next().unwrap();
            let blocks = pack.child(0x3331_0003).unwrap();
            let r = parse_compressed_pack(pack, blocks, &|_| true);
            assert!(r.is_err(), "offset {offset} packed {packed}");
        }
    }
}
