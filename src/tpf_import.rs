//! docs/texmod-import.md: import a TexMod package (`.tpf`) into the
//! clone's texture-pack folders. TexMod keys a replacement by its CRC of
//! the texture as SPEED2.EXE uploads it (`formats::tpf`); the clone's packs
//! by the texture's bin hash (`<HASH>.dds` / `.png`, docs/texture-packs.md).
//! [`collect`] reads every texture pack in the game's files, [`texmod_hash`]
//! computes TexMod's key for each, and [`convert`] turns a package image
//! into a pack file. Nothing here runs unless a tool asks (no game path).
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::formats::texture::{PixelFormat, Texture};
use crate::formats::tpf;

/// The loader's size limit per axis (`render::texture_pack`).
pub const MAX_DIMENSION: u32 = 4096;

/// Which pack folder a texture belongs in, by the file it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Domain {
    /// TRACKS/ and the garage / shop scenes (FRONTEND/PLATFORMS), drawn by
    /// the world renderer.
    World,
    /// CARS/: the car renderer.
    Car,
    /// Anything else (UI, GLOBAL): no scoped pack draws it.
    Other,
}

impl Domain {
    pub fn of(rel: &str) -> Domain {
        let up = rel.replace('\\', "/").to_ascii_uppercase();
        if up.starts_with("CARS/") {
            Domain::Car
        } else if up.starts_with("TRACKS/") || up.starts_with("FRONTEND/PLATFORMS/") {
            Domain::World
        } else {
            Domain::Other
        }
    }
    pub fn dir(self) -> &'static str {
        match self {
            Domain::World => "world",
            Domain::Car => "car",
            Domain::Other => "other",
        }
    }

    /// The pack folders a texture of this domain goes to: GLOBAL's shared
    /// textures (license plate, carbon fibre…) may be drawn by either
    /// renderer, so they go to both (a folder's file is only read for a
    /// texture its renderer draws).
    pub fn targets(self) -> &'static [Domain] {
        match self {
            Domain::World => &[Domain::World],
            Domain::Car => &[Domain::Car],
            Domain::Other => &[Domain::World, Domain::Car],
        }
    }
}

/// A texture of the game's files and where it is.
pub struct Source {
    pub file: String,
    pub domain: Domain,
    pub tex: Texture,
}

/// TexMod's key for `t`: the CRC of `width * height * bpp` bytes of the
/// level-0 surface SPEED2.EXE creates. DXT and 32-bit textures upload their
/// level as stored. P8 textures are created as A8R8G8B8, each index
/// expanded through the BGRA palette (observed: every P8 match of the
/// world texture pack tested, 32 of 32, has this key and none has the raw-index
/// one; the conversion code itself is not traced, see the doc).
pub fn texmod_hash(t: &Texture) -> Option<u32> {
    match t.format {
        PixelFormat::P8 => {
            let n = (t.width * t.height) as usize;
            let idx = t.data.get(..n)?;
            let mut argb = Vec::with_capacity(n * 4);
            for &i in idx {
                argb.extend_from_slice(t.palette.get(i as usize * 4..i as usize * 4 + 4)?);
            }
            Some(tpf::texmod_crc(&argb))
        }
        f => tpf::surface_hash(&t.data, t.width, t.height, tpf::bpp(f)),
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| matches!(x.to_ascii_uppercase().to_str(), Some("BUN" | "BIN" | "LZC")))
            // Vinyl images are composited into the skin before upload
            // (docs/formats.md, Vinyls): TexMod never sees them.
            && !p.file_name().is_some_and(|f| f.eq_ignore_ascii_case("VINYLS.BIN"))
        {
            out.push(p);
        }
    }
}

/// Every texture of every texture pack (chunk 0xB3300000) in the game's
/// `.BUN` / `.BIN` / `.LZC` files (JDLZ-compressed files unpacked), in file
/// order. CARS/*/VINYLS.BIN is skipped (composited before upload).
pub fn collect(root: &Path) -> Result<Vec<Source>> {
    use rayon::prelude::*;
    let mut files = Vec::new();
    walk(root, &mut files);
    ensure!(!files.is_empty(), "no game files under {}", root.display());
    let per_file: Vec<Vec<Source>> = files
        .par_iter()
        .map(|path| {
            let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
            let Ok(raw) = std::fs::read(path) else { return Vec::new() };
            let data = if raw.starts_with(b"JDLZ") {
                match crate::formats::jdlz::decompress(&raw) {
                    Ok(d) => d,
                    Err(_) => return Vec::new(),
                }
            } else {
                raw
            };
            let mut out = Vec::new();
            crate::formats::bundle::walk(crate::formats::bundle::chunks(&data, 0), &mut |c| {
                if c.id == 0xB330_0000
                    && let Ok(ts) = crate::formats::texture::parse_texture_pack(c)
                {
                    out.extend(ts.into_iter().map(|tex| Source { file: rel.clone(), domain: Domain::of(&rel), tex }));
                }
            });
            out
        })
        .collect();
    Ok(per_file.into_iter().flatten().collect())
}

// ----------------------------------------------------------------- images

/// A package image: block-compressed levels as stored, or RGBA8.
pub enum Image {
    Bc { format: PixelFormat, width: u32, height: u32, mips: u32, data: Vec<u8> },
    Rgba { width: u32, height: u32, rgba: Vec<u8> },
}

impl Image {
    pub fn size(&self) -> (u32, u32) {
        match self {
            Image::Bc { width, height, .. } | Image::Rgba { width, height, .. } => (*width, *height),
        }
    }
    pub fn format_name(&self) -> String {
        match self {
            Image::Bc { format, mips, .. } => format!("{format:?}/{mips}"),
            Image::Rgba { .. } => "RGBA".into(),
        }
    }
    /// The top level as RGBA8.
    pub fn top_rgba(&self) -> Vec<u8> {
        match self {
            Image::Rgba { rgba, .. } => rgba.clone(),
            Image::Bc { format, width, height, data, .. } => bc_texture(*format, *width, *height, 1, data.clone()).decode_rgba(),
        }
    }
}

fn bc_texture(format: PixelFormat, width: u32, height: u32, mips: u32, data: Vec<u8>) -> Texture {
    Texture {
        name: String::new(),
        hash: 0,
        width,
        height,
        format,
        mip_count: mips,
        data: data.into(),
        palette: Vec::new(),
        alpha_test: false,
        blend: crate::formats::texture::Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Default::default(),
    }
}

/// Read a package image by its extension: DDS (DXT1/2/3/4/5 and the common
/// uncompressed masks), BMP (24/32-bit, uncompressed) or PNG.
pub fn read_image(name: &str, bytes: &[u8]) -> Result<Image> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "dds" => read_dds(bytes),
        "bmp" => read_bmp(bytes),
        "png" => {
            let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.into_rgba8();
            Ok(Image::Rgba { width: img.width(), height: img.height(), rgba: img.into_raw() })
        }
        e => bail!("unsupported image type .{e} (DDS, BMP and PNG are read)"),
    }
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(b.get(o..o + 4).context("truncated header")?.try_into()?))
}

fn read_dds(b: &[u8]) -> Result<Image> {
    ensure!(b.len() >= 128 && &b[..4] == b"DDS " && u32_at(b, 4)? == 124, "not a DDS");
    let flags = u32_at(b, 8)?;
    let (height, width) = (u32_at(b, 12)?, u32_at(b, 16)?);
    ensure!(width > 0 && height > 0 && width <= 16384 && height <= 16384, "DDS size {width}x{height}");
    let full = 32 - width.max(height).leading_zeros();
    let mips = if flags & 0x20000 != 0 { u32_at(b, 28)?.clamp(1, full) } else { 1 };
    let pf_flags = u32_at(b, 80)?;
    let body = &b[128..];
    if pf_flags & 0x4 != 0 {
        let format = match &b[84..88] {
            b"DXT1" => PixelFormat::Dxt1,
            b"DXT2" | b"DXT3" => PixelFormat::Dxt3,
            b"DXT4" | b"DXT5" => PixelFormat::Dxt5,
            f => bail!("unsupported DDS FourCC {:?}", String::from_utf8_lossy(f)),
        };
        // Keep the whole levels that are present.
        let (mut n, mut used) = (0, 0);
        while n < mips {
            let l = format.level_size((width >> n).max(1), (height >> n).max(1));
            if used + l > body.len() {
                break;
            }
            used += l;
            n += 1;
        }
        ensure!(n > 0, "DDS data too short");
        if &b[84..88] == b"DXT2" || &b[84..88] == b"DXT4" {
            // Premultiplied: decode and un-premultiply the top level.
            let mut rgba = bc_texture(format, width, height, 1, body[..format.level_size(width, height)].to_vec()).decode_rgba();
            unpremultiply(&mut rgba);
            return Ok(Image::Rgba { width, height, rgba });
        }
        return Ok(Image::Bc { format, width, height, mips: n, data: body[..used].to_vec() });
    }
    // Uncompressed: by bit count and masks (top level only).
    let bits = u32_at(b, 88)?;
    let masks = [u32_at(b, 92)?, u32_at(b, 96)?, u32_at(b, 100)?, if pf_flags & 0x1 != 0 { u32_at(b, 104)? } else { 0 }];
    ensure!(matches!(bits, 8 | 16 | 24 | 32), "unsupported DDS bit count {bits}");
    let bpp = (bits / 8) as usize;
    let pitch = width as usize * bpp;
    ensure!(body.len() >= pitch * height as usize, "DDS data too short");
    let luminance = pf_flags & 0x20000 != 0;
    let alpha_only = pf_flags & 0x2 != 0 && pf_flags & 0x40 == 0 && !luminance;
    let channel = |px: u32, mask: u32| -> Option<u8> {
        if mask == 0 {
            return None;
        }
        let shift = mask.trailing_zeros();
        let max = mask >> shift;
        Some((((px & mask) >> shift) as u64 * 255 / max as u64) as u8)
    };
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for p in body[..pitch * height as usize].chunks_exact(bpp) {
        let mut v = 0u32;
        for (i, &x) in p.iter().enumerate() {
            v |= (x as u32) << (8 * i);
        }
        if alpha_only {
            rgba.extend_from_slice(&[255, 255, 255, channel(v, masks[3]).unwrap_or(v as u8)]);
        } else if luminance {
            let l = channel(v, masks[0]).unwrap_or(0);
            rgba.extend_from_slice(&[l, l, l, channel(v, masks[3]).unwrap_or(255)]);
        } else {
            rgba.extend_from_slice(&[channel(v, masks[0]).unwrap_or(0), channel(v, masks[1]).unwrap_or(0), channel(v, masks[2]).unwrap_or(0), channel(v, masks[3]).unwrap_or(255)]);
        }
    }
    Ok(Image::Rgba { width, height, rgba })
}

fn unpremultiply(rgba: &mut [u8]) {
    for p in rgba.chunks_exact_mut(4) {
        if p[3] > 0 && p[3] < 255 {
            for c in 0..3 {
                p[c] = ((p[c] as u32 * 255 + p[3] as u32 / 2) / p[3] as u32).min(255) as u8;
            }
        }
    }
}

/// TexMod logs BMPs (bottom-up, 24 or 32 bit); 32-bit ones carry alpha.
fn read_bmp(b: &[u8]) -> Result<Image> {
    ensure!(b.len() >= 54 && &b[..2] == b"BM", "not a BMP");
    let off = u32_at(b, 10)? as usize;
    let w = i32::from_le_bytes(b[18..22].try_into()?);
    let h = i32::from_le_bytes(b[22..26].try_into()?);
    let bits = u16::from_le_bytes(b[28..30].try_into()?);
    let compression = u32_at(b, 30)?;
    ensure!(w > 0 && h != 0 && matches!(bits, 24 | 32) && (compression == 0 || compression == 3), "unsupported BMP ({bits} bit, compression {compression})");
    let (width, height) = (w as u32, h.unsigned_abs());
    let bpp = bits as usize / 8;
    let pitch = (width as usize * bpp).div_ceil(4) * 4;
    ensure!(b.len() >= off + pitch * height as usize, "BMP data too short");
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height as usize {
        let sy = if h > 0 { height as usize - 1 - y } else { y };
        let row = &b[off + sy * pitch..];
        for x in 0..width as usize {
            let p = &row[x * bpp..x * bpp + bpp];
            let a = if bpp == 4 { p[3] } else { 255 };
            rgba[(y * width as usize + x) * 4..][..4].copy_from_slice(&[p[2], p[1], p[0], a]);
        }
    }
    Ok(Image::Rgba { width, height, rgba })
}

// ------------------------------------------------------------- conversion

/// A converted pack file.
pub struct Output {
    /// `dds` or `png`.
    pub ext: &'static str,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: String,
    /// What the conversion did (kept, mips completed, downsized, decoded).
    pub notes: Vec<String>,
}

fn half(rgba: &[u8], w: u32, h: u32) -> (Vec<u8>, u32, u32) {
    let levels = crate::upscale::mip_chain(rgba, w, h);
    let (nw, nh, l) = levels.into_iter().nth(1).expect("a texture larger than 1x1 has a second level");
    (l, nw, nh)
}

/// What [`match_tone`] did to an image.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tone {
    /// The contrast gain on the original's grid (0.5..2).
    pub gain: f32,
    /// The mean colour before and after (0..1), the original's.
    pub mean_before: [f32; 3],
    pub mean_after: [f32; 3],
    /// Alpha: None kept; Some(s) the mean ratio to reach (scaled by s held
    /// to 0.25..4, then offset to the original's mean); Some(inf) opaque.
    pub alpha_scale: Option<f32>,
}

/// Bilinear sample of an RGB f32 grid `gw`×`gh` (wrapping, like the
/// game's samplers) at texel `(x, y)` of a `w`×`h` image.
fn sample_low(low: &[[f32; 3]], gw: usize, gh: usize, x: usize, y: usize, w: usize, h: usize) -> [f32; 3] {
    let fx = (x as f32 + 0.5) * gw as f32 / w as f32 - 0.5;
    let fy = (y as f32 + 0.5) * gh as f32 / h as f32 - 0.5;
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let at = |xx: f32, yy: f32| low[(yy as isize).rem_euclid(gh as isize) as usize * gw + (xx as isize).rem_euclid(gw as isize) as usize];
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1.0, y0), at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
    std::array::from_fn(|i| (a[i] * (1.0 - tx) + b[i] * tx) * (1.0 - ty) + (c[i] * (1.0 - tx) + d[i] * tx) * ty)
}

/// The import's look rule (docs/texmod-import.md): give a replacement (RGBA8
/// `w`×`h`, changed in place) the original's tone. On the original's grid
/// the replacement gets the original's mean colour and luma contrast: the
/// low frequencies (the replacement box-filtered to the original's size,
/// bilinear back up) are scaled about their mean by the contrast ratio
/// (0.5..2) and moved to the original's mean; the finer detail the pack
/// adds rides on top unchanged; the mean is then trimmed for clipping.
/// Means and contrast count the texels the game shows (alpha-tested: alpha
/// > 95; blended: weighted by alpha). Alpha that is data (opaque draws:
/// the roads' reflection amount) keeps the original's mean (an opaque
/// original stays opaque); alpha-tested and blended alpha (cut-out art)
/// stays as authored (`alpha` false: all alpha as authored). Returns None when the image already matches (within
/// 1 % mean, 2 % contrast, 2 % alpha).
pub fn match_tone(orig: &Texture, rgba: &mut [u8], w: u32, h: u32, alpha: bool) -> Option<Tone> {
    let o = orig.decode_rgba();
    let (gw, gh) = (orig.width as usize, orig.height as usize);
    let use_ = crate::upscale::AlphaUse::of(orig);
    let n = resample(rgba, w, h, orig.width, orig.height, use_ != crate::upscale::AlphaUse::Data);
    let weight = |a: u8| match use_ {
        crate::upscale::AlphaUse::Data => 1.0f64,
        crate::upscale::AlphaUse::Test => if a > crate::formats::texture::ALPHA_TEST_REF { 1.0 } else { 0.0 },
        crate::upscale::AlphaUse::Blend => a as f64 / 255.0,
    };
    let luma = |p: [f64; 3]| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
    let stats = |img: &[u8]| {
        let (mut m, mut ws) = ([0f64; 3], 0f64);
        for p in img.chunks_exact(4) {
            let wt = weight(p[3]);
            for c in 0..3 {
                m[c] += wt * p[c] as f64 / 255.0;
            }
            ws += wt;
        }
        let m = m.map(|v| if ws > 0.0 { v / ws } else { 0.0 });
        let mut var = 0f64;
        for p in img.chunks_exact(4) {
            let l = luma([p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0]);
            var += weight(p[3]) * (l - luma(m)).powi(2);
        }
        (m, if ws > 0.0 { (var / ws).sqrt() } else { 0.0 })
    };
    let (mo, so) = stats(&o);
    let (mn_grid, sn) = stats(&n);
    let (mn, _) = stats(rgba);
    let gain = if sn > 1e-3 && so > 1e-3 { (so / sn).clamp(0.5, 2.0) } else { 1.0 } as f32;
    let alpha_scale = match use_ {
        crate::upscale::AlphaUse::Data if alpha => {
            let mean = |img: &[u8]| img.chunks_exact(4).map(|p| p[3] as f64).sum::<f64>() / (img.len() / 4).max(1) as f64;
            if o.chunks_exact(4).all(|p| p[3] == 255) {
                (!rgba.chunks_exact(4).all(|p| p[3] == 255)).then_some(f32::INFINITY)
            } else {
                let (ao, an) = (mean(&o), mean(rgba));
                (an > 0.5 && (ao / an - 1.0).abs() > 0.02).then(|| (ao / an) as f32)
            }
        }
        _ => None,
    };
    if (gain - 1.0).abs() < 0.02 && (0..3).all(|c| (mo[c] - mn[c]).abs() < 0.01) && alpha_scale.is_none() {
        return None;
    }
    // The low frequencies on the original's grid.
    let low: Vec<[f32; 3]> = n.chunks_exact(4).map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]).collect();
    let (wu, hu) = (w as usize, h as usize);
    let src = rgba.to_vec();
    let mut offset = [0f32; 3];
    let render = |offset: [f32; 3], out: &mut [u8]| {
        for y in 0..hu {
            for x in 0..wu {
                let l = sample_low(&low, gw, gh, x, y, wu, hu);
                let i = (y * wu + x) * 4;
                for c in 0..3 {
                    let v = src[i + c] as f32 / 255.0;
                    let lo = (l[c] - mn_grid[c] as f32) * gain + mo[c] as f32 + offset[c];
                    out[i + c] = ((v - l[c] + lo).clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
        }
    };
    // Trim the mean for clipping (a few fixed-point steps).
    for _ in 0..4 {
        render(offset, rgba);
        let (m, _) = stats(rgba);
        let err: Vec<f32> = (0..3).map(|c| (mo[c] - m[c]) as f32).collect();
        if err.iter().all(|e| e.abs() < 0.002) {
            break;
        }
        for c in 0..3 {
            offset[c] += err[c];
        }
    }
    render(offset, rgba);
    if let Some(a) = alpha_scale {
        if a.is_infinite() {
            rgba.chunks_exact_mut(4).for_each(|p| p[3] = 255);
        } else {
            // Scale (held to 0.25..4, so a sparse mask is not just clipped),
            // then an offset trims the mean to the original's.
            let target = o.chunks_exact(4).map(|p| p[3] as f64).sum::<f64>() / (o.len() / 4).max(1) as f64;
            let a = a.clamp(0.25, 4.0);
            let src: Vec<u8> = rgba.chunks_exact(4).map(|p| p[3]).collect();
            let mut off = 0f32;
            for _ in 0..6 {
                let mut sum = 0f64;
                for (p, &s) in rgba.chunks_exact_mut(4).zip(&src) {
                    p[3] = (s as f32 * a + off).round().clamp(0.0, 255.0) as u8;
                    sum += p[3] as f64;
                }
                let err = (target - sum / src.len().max(1) as f64) as f32;
                if err.abs() < 0.5 {
                    break;
                }
                off += err;
            }
        }
    }
    let (after, _) = stats(rgba);
    Some(Tone { gain, mean_before: mn.map(|v| v as f32), mean_after: after.map(|v| v as f32), alpha_scale })
}

/// The image as the loader wants it, at most `max` per axis:
/// * DXT1/DXT3/DXT5 with whole 4×4 blocks → DDS in the same format: the
///   stored levels byte for byte (levels over `max` dropped), missing lower
///   levels completed with the alpha-weighted box chain (`upscale::mip_chain`)
///   and our own encoder; with a `tone` every level is re-encoded from the
///   corrected top level;
/// * anything else (uncompressed, BMP/PNG, a size that is not whole blocks)
///   → PNG RGBA8 (the loader makes its mips), halved while over `max`.
///
/// No resampling up, no colour-space change (the game samples UNORM like
/// D3D9 did).
pub fn convert(img: &Image, max: u32, tone_of: Option<(&Texture, bool)>) -> Result<Output> {
    let mut notes = Vec::new();
    match img {
        Image::Bc { format, width, height, mips, data } if width % 4 == 0 && height % 4 == 0 => {
            let (format, mut w, mut h) = (*format, *width, *height);
            let mut skip = 0usize;
            let mut level = 0u32;
            while w.max(h) > max && level + 1 < *mips && (w / 2) % 4 == 0 && (h / 2) % 4 == 0 {
                skip += format.level_size(w, h);
                w /= 2;
                h /= 2;
                level += 1;
            }
            if level > 0 {
                notes.push(format!("top {level} level(s) dropped to fit {max}"));
            }
            if w.max(h) > max {
                bail!("{width}x{height} has no stored level within {max}");
            }
            let full = 32 - w.max(h).leading_zeros();
            let mut out = data[skip..].to_vec();
            let mut top = bc_texture(format, w, h, 1, out[..format.level_size(w, h)].to_vec()).decode_rgba();
            let tone = tone_of.and_then(|(o, a)| match_tone(o, &mut top, w, h, a));
            let have = if tone.is_some() { 0 } else { *mips - level };
            if have < full {
                // The levels not kept as stored: (toned top,) box chain,
                // our encoder.
                if let Some(t) = tone {
                    out.clear();
                    notes.push(format!("tone matched (gain {:.2}{}), re-encoded", t.gain, alpha_note(t.alpha_scale)));
                } else {
                    out.truncate((0..have).map(|m| format.level_size((w >> m).max(1), (h >> m).max(1))).sum());
                    notes.push(format!("mips {have}->{full} completed"));
                }
                let levels = crate::upscale::mip_chain(&top, w, h);
                let from = have as usize;
                for (lw, lh, l) in levels.iter().skip(from) {
                    out.extend(match format {
                        PixelFormat::Dxt1 => crate::upscale::encode_bc1(l, *lw, *lh),
                        PixelFormat::Dxt3 => crate::upscale::encode_bc2(l, *lw, *lh),
                        _ => crate::upscale::encode_bc3(l, *lw, *lh),
                    });
                }
            }
            let e = crate::upscale::Encoded { format, width: w, height: h, mips: full, data: out };
            Ok(Output { ext: "dds", bytes: crate::upscale::dds_bytes(&e), width: w, height: h, format: format!("{format:?}"), notes })
        }
        _ => {
            let (mut w, mut h) = img.size();
            let mut rgba = img.top_rgba();
            if matches!(img, Image::Bc { .. }) {
                notes.push("blocks not whole 4x4: decoded".into());
            }
            while w.max(h) > max {
                (rgba, w, h) = half(&rgba, w, h);
                notes.push("halved to fit".into());
            }
            if let Some(t) = tone_of.and_then(|(o, a)| match_tone(o, &mut rgba, w, h, a)) {
                notes.push(format!("tone matched (gain {:.2}{})", t.gain, alpha_note(t.alpha_scale)));
            }
            let mut png = Vec::new();
            image::codecs::png::PngEncoder::new(&mut png)
                .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
                .context("PNG encode")?;
            Ok(Output { ext: "png", bytes: png, width: w, height: h, format: "RGBA8".into(), notes })
        }
    }
}

fn alpha_note(a: Option<f32>) -> String {
    match a {
        None => String::new(),
        Some(a) if a.is_infinite() => ", alpha opaque".into(),
        Some(a) => format!(", alpha mean x{a:.2}"),
    }
}

/// Whether a package image is the original texture itself (same size and
/// texels within the DXT rounding): nothing to replace.
pub fn same_as_original(orig: &Texture, img: &Image) -> bool {
    let (w, h) = img.size();
    if (w, h) != (orig.width, orig.height) {
        return false;
    }
    let (a, b) = (orig.decode_rgba(), img.top_rgba());
    a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) <= 2)
}

use image::ImageEncoder as _;

// ------------------------------------------------------------------ checks

/// How a replacement compares with the original it stands for, on the
/// original's grid (the replacement box-filtered down).
#[derive(Clone, Copy, Debug, Default)]
pub struct Compare {
    /// Mean Rec.601 luma, 0..1, over shown texels (alpha > 95 where the
    /// texture is alpha-tested, else all).
    pub luma_orig: f32,
    pub luma_new: f32,
    /// Pearson correlation of the two luma images (a wrong match is low).
    pub correlation: f32,
    pub alpha_orig: f32,
    pub alpha_new: f32,
    /// Share of texels whose "shown" state (alpha test / mask) differs.
    pub coverage_change: f32,
}

/// Resample RGBA8 `w`×`h` to `ow`×`oh` by area averaging (any ratio);
/// colour weighted by alpha when `weighted` (coverage alpha), plain when
/// alpha is data.
pub fn resample(rgba: &[u8], w: u32, h: u32, ow: u32, oh: u32, weighted: bool) -> Vec<u8> {
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    for oy in 0..oh {
        let (y0, y1) = ((oy * h / oh) as usize, (((oy + 1) * h).div_ceil(oh)).max(oy * h / oh + 1) as usize);
        for ox in 0..ow {
            let (x0, x1) = ((ox * w / ow) as usize, (((ox + 1) * w).div_ceil(ow)).max(ox * w / ow + 1) as usize);
            let mut acc = [0f64; 4];
            let mut n = 0f64;
            let mut acc_w = 0f64;
            for y in y0..y1.min(h as usize) {
                for x in x0..x1.min(w as usize) {
                    let p = &rgba[(y * w as usize + x) * 4..][..4];
                    let a = p[3] as f64;
                    let wt = if weighted { a + 1.0 } else { 1.0 };
                    for c in 0..3 {
                        acc[c] += p[c] as f64 * wt;
                    }
                    acc[3] += a;
                    acc_w += wt;
                    n += 1.0;
                }
            }
            let wsum = acc_w;
            let o = &mut out[((oy * ow + ox) * 4) as usize..][..4];
            for c in 0..3 {
                o[c] = (acc[c] / wsum).round() as u8;
            }
            o[3] = (acc[3] / n).round() as u8;
        }
    }
    out
}

pub fn compare(orig: &Texture, new_rgba: &[u8], nw: u32, nh: u32) -> Compare {
    let o = orig.decode_rgba();
    let n = resample(new_rgba, nw, nh, orig.width, orig.height, crate::upscale::AlphaUse::of(orig) != crate::upscale::AlphaUse::Data);
    let shown = |a: u8| !orig.alpha_test || a > crate::formats::texture::ALPHA_TEST_REF;
    let luma = |p: &[u8]| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0;
    let (mut lo, mut ln, mut ao, mut an, mut cnt, mut cov) = (Vec::new(), Vec::new(), 0f32, 0f32, 0f32, 0f32);
    for (po, pn) in o.chunks_exact(4).zip(n.chunks_exact(4)) {
        ao += po[3] as f32 / 255.0;
        an += pn[3] as f32 / 255.0;
        cnt += 1.0;
        if shown(po[3]) != shown(pn[3]) {
            cov += 1.0;
        }
        if shown(po[3]) {
            lo.push(luma(po));
            ln.push(luma(pn));
        }
    }
    let mean = |v: &[f32]| if v.is_empty() { 0.0 } else { v.iter().sum::<f32>() / v.len() as f32 };
    let (mo, mn) = (mean(&lo), mean(&ln));
    let (mut sxy, mut sxx, mut syy) = (0f32, 0f32, 0f32);
    for (a, b) in lo.iter().zip(&ln) {
        sxy += (a - mo) * (b - mn);
        sxx += (a - mo).powi(2);
        syy += (b - mn).powi(2);
    }
    let correlation = if sxx > 1e-6 && syy > 1e-6 { sxy / (sxx * syy).sqrt() } else { 1.0 };
    Compare { luma_orig: mo, luma_new: mn, correlation, alpha_orig: ao / cnt.max(1.0), alpha_new: an / cnt.max(1.0), coverage_change: cov / cnt.max(1.0) }
}

// ------------------------------------------------------------------ import

/// One package texture and what became of it.
pub struct Row {
    pub texmod: u32,
    pub file: String,
    /// The game textures it stands for (bin hash, name, domain, files).
    pub targets: Vec<(u32, String, Domain, Vec<String>)>,
    pub orig: Option<String>,
    pub out: Option<Output>,
    pub compare: Option<Compare>,
    pub flags: Vec<String>,
}

/// The TexMod key → textures index of `sources` (same pixels in several
/// files collapse; a bin hash whose pixels differ between files is kept
/// under each key).
pub fn index(sources: &[Source]) -> HashMap<u32, Vec<&Source>> {
    let mut m: HashMap<u32, Vec<&Source>> = HashMap::new();
    for s in sources {
        if let Some(h) = texmod_hash(&s.tex) {
            m.entry(h).or_default().push(s);
        }
    }
    m
}

/// Bin hashes whose texture differs between files (a replacement keyed by
/// the bin hash would also replace the other variant).
pub fn variants(sources: &[Source]) -> HashMap<u32, usize> {
    let mut m: HashMap<u32, std::collections::HashSet<Option<u32>>> = HashMap::new();
    for s in sources {
        m.entry(s.tex.hash).or_default().insert(texmod_hash(&s.tex));
    }
    m.into_iter().filter(|(_, v)| v.len() > 1).map(|(k, v)| (k, v.len())).collect()
}

/// Package entries (images only) by TexMod key: `texmod.def` names win,
/// then the `_0x<HASH>` in the entry name.
pub fn package_images(p: &tpf::Package) -> Vec<(u32, &tpf::Entry)> {
    let by_name: HashMap<&str, &tpf::Entry> = p.entries.iter().map(|e| (e.name.as_str(), e)).collect();
    let mut out: Vec<(u32, &tpf::Entry)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(def) = p.entries.iter().find(|e| e.name.eq_ignore_ascii_case("texmod.def")) {
        for (h, f) in tpf::parse_def(&String::from_utf8_lossy(&def.data)) {
            let f = f.replace('\\', "/");
            if let Some(e) = by_name.get(f.as_str()).or_else(|| by_name.get(f.rsplit('/').next().unwrap_or(&f))) {
                if seen.insert(h) {
                    out.push((h, e));
                }
            }
        }
    }
    for e in &p.entries {
        if let Some(h) = tpf::name_hash(&e.name)
            && seen.insert(h)
        {
            out.push((h, e));
        }
    }
    out.sort_by_key(|(h, _)| *h);
    out
}

/// How [`import_package`] converts (the import tool's options).
#[derive(Clone, Copy, Debug)]
pub struct ImportOptions {
    /// The largest side kept ([`MAX_DIMENSION`]).
    pub max: u32,
    /// Give each replacement the original's mean colour and contrast
    /// ([`match_tone`]; false: as authored).
    pub tone: bool,
    /// Where alpha is data, match the original's mean alpha too.
    pub alpha: bool,
    /// Convert and report, write nothing.
    pub dry_run: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions { max: MAX_DIMENSION, tone: true, alpha: true, dry_run: false }
    }
}

/// What [`import_package`] did.
#[derive(Clone, Debug, Default)]
pub struct ImportSummary {
    /// Images in the package (by TexMod key).
    pub images: usize,
    /// Images whose key is a texture of the game's files.
    pub matched: usize,
    /// Images whose key is no texture of the game's files (another game,
    /// another version, a composited skin or a render target).
    pub unmatched: usize,
    /// Matched images that could not be read or converted (an unknown
    /// image format, a broken file).
    pub unsupported: usize,
    /// Matched images identical to the original (nothing to replace).
    pub same: usize,
    /// Replacements the package names but does not hold (a TexWizard pair
    /// whose texture is neither in its textures.bin nor in the game).
    pub missing: usize,
    /// Files written, by folder (`world`, `car`).
    pub files: std::collections::BTreeMap<&'static str, usize>,
    /// Bytes written.
    pub bytes: u64,
    /// GPU bytes of the replacements (DDS as stored, PNG as RGBA8 with mips).
    pub vram: u64,
    /// The programs the package's entries are named for (`SPEED2.EXE` for
    /// this game), upper case.
    pub programs: Vec<String>,
    /// The report: one tab-separated line per package texture and target.
    pub lines: Vec<String>,
}

impl ImportSummary {
    pub fn files_written(&self) -> usize {
        self.files.values().sum()
    }

    /// The package was made for another program than SPEED2.EXE (its
    /// entries name one, and none of its images matched).
    pub fn other_game(&self) -> Option<&str> {
        (self.matched == 0)
            .then(|| self.programs.iter().find(|p| !p.eq_ignore_ascii_case("SPEED2.EXE")).map(String::as_str))
            .flatten()
    }
}

/// The program an entry is named for: `SPEED2.EXE_0x1234ABCD.dds` →
/// `SPEED2.EXE` (None for a bare `0x…` name).
pub fn entry_program(name: &str) -> Option<String> {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let i = file.rfind("_0x").or_else(|| file.rfind("_0X"))?;
    (i > 0).then(|| file[..i].to_ascii_uppercase())
}

/// A folder of loose TexMod files (a `texmod.def` and the images it
/// names, as TexMod's "extract" writes them) read as a package; its
/// comment is the folder's name.
pub fn read_folder(dir: &Path) -> Result<tpf::Package> {
    let mut entries = Vec::new();
    let mut total = 0usize;
    let rd = std::fs::read_dir(dir).with_context(|| dir.display().to_string())?;
    let mut paths: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    paths.sort();
    ensure!(
        paths.iter().any(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("texmod.def"))),
        "{}: no texmod.def (not a folder of TexMod files)",
        dir.display()
    );
    for p in paths {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if !(name.eq_ignore_ascii_case("texmod.def") || matches!(ext.as_str(), "dds" | "bmp" | "png")) {
            continue;
        }
        let data = std::fs::read(&p).with_context(|| p.display().to_string())?;
        total += data.len();
        crate::formats::reader::Limits::check("TexMod folder size", total, tpf::MAX_TOTAL)?;
        entries.push(tpf::Entry { name, data });
    }
    let comment = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(tpf::Package { entries, comment })
}

/// Converts a package's images into texture-pack folders under `out`
/// (`world/`, `car/`, keyed by the game's bin hash) for the game's
/// textures `sources` ([`collect`]); the report lines in the summary.
/// One image per (folder, bin hash): the first key wins. An image that
/// cannot be read or converted is counted (`unsupported`) and left out;
/// the rest goes on. `progress(done, total)` after each image.
pub fn import_package(
    package: &tpf::Package,
    sources: &[Source],
    out: &Path,
    opts: &ImportOptions,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<ImportSummary> {
    let images = package_images(package);
    let items: Vec<(u32, String)> = images.iter().map(|(k, e)| (*k, e.name.clone())).collect();
    let read = |i: usize| read_image(&images[i].1.name, &images[i].1.data);
    let mut sum = import_images(&items, &read, &index(sources), &variants(sources), out, opts, progress, "texmod", "import-tpf.tsv")?;
    let mut programs: Vec<String> = package.entries.iter().filter_map(|e| entry_program(&e.name)).collect();
    programs.sort();
    programs.dedup();
    sum.programs = programs;
    Ok(sum)
}

/// The core of an import: `items` (a key and a label per replacement,
/// `read(i)` its image) matched through `index` (key → the game textures
/// it stands for), converted and written into `out/world`, `out/car`, the
/// report (`key_column` naming the key) written as `report` beside them.
/// One image per (folder, bin hash): the first item wins.
#[allow(clippy::too_many_arguments)]
pub fn import_images(
    items: &[(u32, String)],
    read: &(dyn Fn(usize) -> Result<Image> + Sync),
    index: &HashMap<u32, Vec<&Source>>,
    variants: &HashMap<u32, usize>,
    out: &Path,
    opts: &ImportOptions,
    progress: &(dyn Fn(usize, usize) + Sync),
    key_column: &str,
    report: &str,
) -> Result<ImportSummary> {
    use rayon::prelude::*;
    use std::collections::BTreeMap;

    struct Job<'a> {
        key: u32,
        item: usize,
        folder: Domain,
        tex: &'a Texture,
        files: Vec<String>,
        other: bool,
    }
    let mut jobs: Vec<Job> = Vec::new();
    let mut taken: HashMap<(Domain, u32), u32> = HashMap::new();
    let mut lines = vec![format!("{key_column}\tfile\tfolder\tbin_hash\tname\tfiles\torig\tinput\toutput\tnotes\tluma_orig\tluma_pack\tluma_out\tcorrelation\talpha_orig\talpha_pack\talpha_out\tcoverage_change\tflags")];
    let (mut matched, mut unmatched) = (0, 0);
    for (item, (key, label)) in items.iter().enumerate() {
        let Some(hits) = index.get(key) else {
            unmatched += 1;
            lines.push(format!("{key:08X}\t{label}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\tunmatched: no texture in the game's files has this key"));
            continue;
        };
        matched += 1;
        let mut targets: BTreeMap<(Domain, u32), Job> = BTreeMap::new();
        for s in hits {
            for &folder in s.domain.targets() {
                let j = targets.entry((folder, s.tex.hash)).or_insert(Job { key: *key, item, folder, tex: &s.tex, files: Vec::new(), other: false });
                j.files.push(s.file.clone());
                if s.domain == Domain::Other {
                    j.other = true;
                }
            }
        }
        for ((folder, bin), job) in targets {
            match taken.get(&(folder, bin)) {
                Some(first) => lines.push(format!("{key:08X}\t{label}\t{}\t{bin:08X}\t{}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\tconflict: {first:08X} kept", folder.dir(), job.tex.name)),
                None => {
                    taken.insert((folder, bin), *key);
                    jobs.push(job);
                }
            }
        }
    }
    let done = std::sync::atomic::AtomicUsize::new(0);
    let total = jobs.len();
    progress(0, total);
    type Made = (String, Option<(Domain, String, Vec<u8>, u64)>);
    let results: Vec<Result<Made>> = jobs
        .par_iter()
        .map(|j| -> Result<Made> {
            let r = read(j.item).and_then(|img| convert_job(j.key, &items[j.item].1, &img, j.folder, j.tex, &j.files, j.other, variants.get(&j.tex.hash).copied(), opts));
            progress(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1, total);
            r
        })
        .collect();
    let mut sum = ImportSummary { images: items.len(), matched, unmatched, ..Default::default() };
    let mut unsupported_keys = std::collections::HashSet::new();
    for (j, r) in jobs.iter().zip(results) {
        let (line, file) = match r {
            Ok(x) => x,
            Err(e) => {
                unsupported_keys.insert(j.key);
                lines.push(format!("{:08X}\t{}\t{}\t{:08X}\t{}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\tunsupported: {e:#}", j.key, items[j.item].1, j.folder.dir(), j.tex.hash, j.tex.name));
                continue;
            }
        };
        lines.push(line);
        let Some((folder, name, bytes, v)) = file else {
            sum.same += 1;
            continue;
        };
        sum.vram += v;
        if opts.dry_run {
            continue;
        }
        let d = out.join(folder.dir());
        std::fs::create_dir_all(&d).with_context(|| d.display().to_string())?;
        // One file per hash: a PNG would win over a stale DDS.
        let stem = &name[..8];
        for ext in ["png", "dds"] {
            let _ = std::fs::remove_file(d.join(format!("{stem}.{ext}")));
        }
        std::fs::write(d.join(&name), &bytes).with_context(|| d.join(&name).display().to_string())?;
        sum.bytes += bytes.len() as u64;
        *sum.files.entry(folder.dir()).or_default() += 1;
    }
    sum.unsupported = unsupported_keys.len();
    if !opts.dry_run {
        std::fs::create_dir_all(out)?;
        std::fs::write(out.join(report), lines.join("\n") + "\n")?;
    }
    sum.lines = lines;
    Ok(sum)
}

/// One package image converted for one target: the report line and the
/// file (None: the same as the original, skipped).
#[allow(clippy::too_many_arguments)]
fn convert_job(
    key: u32,
    label: &str,
    img: &Image,
    folder: Domain,
    tex: &Texture,
    files: &[String],
    other: bool,
    variants: Option<usize>,
    opts: &ImportOptions,
) -> Result<(String, Option<(Domain, String, Vec<u8>, u64)>)> {
    let (iw, ih) = img.size();
    let top = img.top_rgba();
    let before = compare(tex, &top, iw, ih);
    let mut flags = Vec::new();
    if other {
        flags.push("shared GLOBAL texture (world and car folders)".to_string());
    }
    if let Some(n) = variants {
        flags.push(format!("bin hash has {n} different textures in the files (all replaced)"));
    }
    let head = format!(
        "{key:08X}\t{}\t{}\t{:08X}\t{}\t{}\t{}x{} {:?} {:?}\t{iw}x{ih} {}",
        label,
        folder.dir(),
        tex.hash,
        tex.name,
        files.join(","),
        tex.width,
        tex.height,
        tex.format,
        crate::upscale::AlphaUse::of(tex),
        img.format_name()
    );
    if same_as_original(tex, img) {
        flags.push("same as the original: skipped".into());
        let line = format!(
            "{head}\t-\t-\t{:.3}\t{:.3}\t-\t{:.3}\t{:.3}\t{:.3}\t-\t-\t{}",
            before.luma_orig,
            before.luma_new,
            before.correlation,
            before.alpha_orig,
            before.alpha_new,
            flags.join("; ")
        );
        return Ok((line, None));
    }
    let conv = convert(img, opts.max, opts.tone.then_some((tex, opts.alpha)))?;
    // What the game gets: the written file's top level.
    let written = read_image(&format!("x.{}", conv.ext), &conv.bytes)?;
    let after = compare(tex, &written.top_rgba(), conv.width, conv.height);
    if after.luma_new > after.luma_orig * 1.05 + 0.005 {
        flags.push("brighter than the original".into());
    }
    if after.coverage_change > 0.05 {
        flags.push("alpha-test coverage differs (new cut-out art)".into());
    }
    if (iw as u64 * tex.height as u64) != (ih as u64 * tex.width as u64) {
        flags.push("aspect differs".into());
    }
    let line = format!(
        "{head}\t{}x{} {} {}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}",
        conv.width,
        conv.height,
        conv.format,
        conv.ext,
        if conv.notes.is_empty() { "-".into() } else { conv.notes.join("; ") },
        before.luma_orig,
        before.luma_new,
        after.luma_new,
        before.correlation,
        before.alpha_orig,
        before.alpha_new,
        after.alpha_new,
        after.coverage_change,
        if flags.is_empty() { "-".into() } else { flags.join("; ") },
    );
    let vram = match conv.ext {
        "dds" => conv.bytes.len().saturating_sub(128) as u64,
        _ => (conv.width as u64 * conv.height as u64 * 4) * 4 / 3,
    };
    Ok((line, Some((folder, format!("{:08X}.{}", tex.hash, conv.ext), conv.bytes, vram))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex(format: PixelFormat, w: u32, h: u32, data: Vec<u8>, palette: Vec<u8>) -> Texture {
        Texture { palette, ..bc_texture(format, w, h, 1, data) }
    }

    #[test]
    fn keys_follow_texmods_surface() {
        // DXT1: w*h/2 bytes of level 0 (the lower levels are not hashed).
        let mut d = vec![7u8; 32];
        d.extend([1u8; 8]);
        assert_eq!(texmod_hash(&tex(PixelFormat::Dxt1, 8, 8, d.clone(), vec![])), Some(tpf::texmod_crc(&d[..32])));
        // DXT5: w*h bytes.
        assert_eq!(texmod_hash(&tex(PixelFormat::Dxt5, 4, 4, vec![3; 16], vec![])), Some(tpf::texmod_crc(&[3; 16])));
        // Indices expanded through the BGRA palette (A8R8G8B8 bytes).
        let mut pal = vec![0u8; 1024];
        pal[4..8].copy_from_slice(&[10, 20, 30, 40]);
        let t = tex(PixelFormat::P8, 2, 1, vec![1, 0], pal);
        assert_eq!(texmod_hash(&t), Some(tpf::texmod_crc(&[10, 20, 30, 40, 0, 0, 0, 0])));
    }

    #[test]
    fn domains_by_file() {
        assert_eq!(Domain::of("TRACKS/STREAML4RA.BUN"), Domain::World);
        assert_eq!(Domain::of("FRONTEND/PLATFORMS/Crib01.BIN"), Domain::World);
        assert_eq!(Domain::of("CARS/WHEELS/TEXTURES.BIN"), Domain::Car);
        assert_eq!(Domain::of("GLOBAL/GLOBALB.BUN"), Domain::Other);
    }

    fn dds(format: &[u8; 4], w: u32, h: u32, mips: u32, data: &[u8]) -> Vec<u8> {
        let mut hd = [0u32; 32];
        hd[0] = u32::from_le_bytes(*b"DDS ");
        hd[1] = 124;
        hd[2] = 0x1007 | if mips > 1 { 0x20000 } else { 0 };
        hd[3] = h;
        hd[4] = w;
        hd[7] = mips;
        hd[19] = 32;
        hd[20] = 4;
        hd[21] = u32::from_le_bytes(*format);
        let mut v: Vec<u8> = hd.iter().flat_map(|x| x.to_le_bytes()).collect();
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn dxt3_with_part_of_its_mips_keeps_them_and_completes_the_chain() {
        // 8x8 DXT3, two of four levels stored.
        let l0: Vec<u8> = (0..64).map(|i| i as u8).collect();
        let l1 = vec![0x55u8; 16];
        let img = read_dds(&dds(b"DXT3", 8, 8, 2, &[l0.clone(), l1.clone()].concat())).unwrap();
        let out = convert(&img, 4096, None).unwrap();
        assert_eq!(out.ext, "dds");
        let e = crate::upscale::parse_dds(&out.bytes).unwrap();
        assert_eq!((e.format, e.width, e.height, e.mips), (PixelFormat::Dxt3, 8, 8, 4));
        assert_eq!(&e.data[..80], &[l0, l1].concat()[..], "stored levels kept byte for byte");
        assert_eq!(e.data.len(), 64 + 16 + 16 + 16);
    }

    #[test]
    fn oversized_drops_top_levels_and_odd_sizes_become_png() {
        let w = 8192u32;
        let h = 8u32;
        let sizes: Vec<usize> = (0..14).map(|m| PixelFormat::Dxt1.level_size((w >> m).max(1), (h >> m).max(1))).collect();
        let data = vec![0u8; sizes.iter().sum()];
        let img = read_dds(&dds(b"DXT1", w, h, 14, &data)).unwrap();
        let out = convert(&img, 4096, None).unwrap();
        assert_eq!((out.width, out.height), (4096, 4));
        // 6x5 DXT5 (not whole blocks) -> PNG at its own size.
        let img = read_dds(&dds(b"DXT5", 6, 5, 1, &vec![0u8; 4 * 16])).unwrap();
        let out = convert(&img, 4096, None).unwrap();
        assert_eq!((out.ext, out.width, out.height), ("png", 6, 5));
    }

    #[test]
    fn uncompressed_argb_and_bmp() {
        let mut hd = [0u32; 32];
        hd[0] = u32::from_le_bytes(*b"DDS ");
        hd[1] = 124;
        hd[2] = 0x100F;
        hd[3] = 1;
        hd[4] = 2;
        hd[19] = 32;
        hd[20] = 0x41;
        hd[22] = 32;
        hd[23] = 0x00FF_0000;
        hd[24] = 0x0000_FF00;
        hd[25] = 0x0000_00FF;
        hd[26] = 0xFF00_0000;
        let mut v: Vec<u8> = hd.iter().flat_map(|x| x.to_le_bytes()).collect();
        v.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]); // BGRA
        match read_dds(&v).unwrap() {
            Image::Rgba { rgba, .. } => assert_eq!(rgba, vec![3, 2, 1, 4, 7, 6, 5, 8]),
            _ => panic!(),
        }
        // A 1x2 24-bit bottom-up BMP.
        let mut b = vec![0u8; 54];
        b[..2].copy_from_slice(b"BM");
        b[10..14].copy_from_slice(&54u32.to_le_bytes());
        b[18..22].copy_from_slice(&1i32.to_le_bytes());
        b[22..26].copy_from_slice(&2i32.to_le_bytes());
        b[28..30].copy_from_slice(&24u16.to_le_bytes());
        b.extend_from_slice(&[1, 2, 3, 0, 4, 5, 6, 0]); // bottom row first
        match read_bmp(&b).unwrap() {
            Image::Rgba { rgba, .. } => assert_eq!(rgba, vec![6, 5, 4, 255, 3, 2, 1, 255]),
            _ => panic!(),
        }
    }

    #[test]
    fn tone_match_gives_the_originals_mean_and_contrast() {
        // Original: a dark, contrasty 4x4 ramp; new: a brighter, flatter 8x8
        // with fine detail.
        let o: Vec<u8> = (0..16u32).flat_map(|i| { let v = (i * 12) as u8; [v, v, v, 255] }).collect();
        let orig = Texture { format: PixelFormat::Rgba8, width: 4, height: 4, data: o.into(), ..bc_texture(PixelFormat::Rgba8, 4, 4, 1, vec![]) };
        let mut n: Vec<u8> = (0..64u32).flat_map(|i| { let v = (120 + (i % 8) * 3 + (i / 8) * 6 + (i % 2) * 10) as u8; [v, v, v, 128] }).collect();
        let before = compare(&orig, &n, 8, 8);
        assert!(before.luma_new > before.luma_orig + 0.2);
        let t = match_tone(&orig, &mut n, 8, 8, true).unwrap();
        assert!(t.gain > 1.5, "{t:?}");
        assert_eq!(t.alpha_scale, Some(f32::INFINITY), "an opaque original stays opaque");
        let c = compare(&orig, &n, 8, 8);
        assert!((c.luma_new - c.luma_orig).abs() < 0.01, "{c:?}");
        assert!(n.chunks(4).all(|p| p[3] == 255));
        // An image that already matches is left alone.
        let mut same = orig.decode_rgba();
        assert_eq!(match_tone(&orig, &mut same, 4, 4, true), None);
    }

    #[test]
    fn compare_finds_a_wrong_match() {
        let grad: Vec<u8> = (0..16u32).flat_map(|i| [(i * 16) as u8, (i * 16) as u8, (i * 16) as u8, 255]).collect();
        let o = Texture { format: PixelFormat::Rgba8, width: 4, height: 4, data: grad.clone().into(), ..bc_texture(PixelFormat::Rgba8, 4, 4, 1, vec![]) };
        let up = resample(&grad, 4, 4, 8, 8, true);
        assert!(compare(&o, &up, 8, 8).correlation > 0.95);
        let inv: Vec<u8> = grad.chunks(4).flat_map(|p| [255 - p[0], 255 - p[1], 255 - p[2], 255]).collect();
        assert!(compare(&o, &inv, 4, 4).correlation < 0.0);
    }

    /// The real package, when present (NFSU2_TPF names it): every entry
    /// reads and converts.
    #[test]
    fn real_package_converts() {
        let Some(path) = std::env::var_os("NFSU2_TPF") else { return };
        let p = tpf::read(&std::fs::read(path).unwrap()).unwrap();
        for (h, e) in package_images(&p) {
            let img = read_image(&e.name, &e.data).unwrap_or_else(|err| panic!("{h:08X}: {err}"));
            convert(&img, MAX_DIMENSION, None).unwrap();
        }
    }
}
