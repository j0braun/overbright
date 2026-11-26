//! DDS files with the DX10 header extension (DXGI formats): what Overbright
//! packs compile PNG images to (BC7) and what authors' tools write for BC1–BC3
//! and BC7 (texconv, GIMP, Photoshop plug-ins). The DX9 FourCC files
//! (DXT1/DXT3/DXT5) stay `upscale::parse_dds`'s.

use crate::formats::texture::PixelFormat;
use crate::upscale::Encoded;

/// DXGI_FORMAT values read (UNORM and SRGB: the same bits; the renderer
/// samples both as UNORM, as the game's own textures are).
const DXGI: [(u32, PixelFormat); 8] = [
    (71, PixelFormat::Dxt1),
    (72, PixelFormat::Dxt1),
    (74, PixelFormat::Dxt3),
    (75, PixelFormat::Dxt3),
    (77, PixelFormat::Dxt5),
    (78, PixelFormat::Dxt5),
    (98, PixelFormat::Bc7),
    (99, PixelFormat::Bc7),
];

/// The largest side read (as the loader's limit).
pub const MAX_SIDE: u32 = 16384;

/// A DX10 DDS's levels, or None when it is not one this reads (a DX9 file, a
/// format other than BC1/BC2/BC3/BC7, an array or cube, a side not a whole
/// number of blocks, data shorter than its levels).
pub fn parse_dx10(bytes: &[u8]) -> Option<Encoded> {
    let u = |i: usize| bytes.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    if bytes.len() < 148 || &bytes[..4] != b"DDS " || u(1)? != 124 || u(20)? & 0x4 == 0 || &bytes[84..88] != b"DX10" {
        return None;
    }
    let (height, width) = (u(3)?, u(4)?);
    let dxgi = u(32)?;
    let (dimension, array) = (u(33)?, u(35)?);
    if dimension != 3 || array > 1 {
        return None;
    }
    let format = DXGI.iter().find(|(d, _)| *d == dxgi)?.1;
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE || width % 4 != 0 || height % 4 != 0 {
        return None;
    }
    let full = 32 - width.max(height).leading_zeros();
    let mips = if u(2)? & 0x20000 != 0 { u(7)?.clamp(1, full) } else { 1 };
    let size: usize = (0..mips).map(|m| format.level_size((width >> m).max(1), (height >> m).max(1))).sum();
    let data = bytes.get(148..148 + size)?.to_vec();
    Some(Encoded { format, width, height, mips, data })
}

/// `e` as a DX10 DDS (BC7: DXGI 98; BC1–BC3: 71 / 74 / 77).
pub fn dx10_bytes(e: &Encoded) -> Vec<u8> {
    let dxgi = match e.format {
        PixelFormat::Dxt1 => 71,
        PixelFormat::Dxt3 => 74,
        PixelFormat::Dxt5 => 77,
        _ => 98,
    };
    let mut h = [0u32; 37];
    h[0] = u32::from_le_bytes(*b"DDS ");
    h[1] = 124;
    // CAPS | HEIGHT | WIDTH | PIXELFORMAT | MIPMAPCOUNT | LINEARSIZE
    h[2] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000 | 0x80000;
    h[3] = e.height;
    h[4] = e.width;
    h[5] = e.format.level_size(e.width, e.height) as u32;
    h[7] = e.mips;
    h[19] = 32;
    h[20] = 0x4;
    h[21] = u32::from_le_bytes(*b"DX10");
    // TEXTURE | MIPMAP | COMPLEX
    h[27] = 0x1000 | 0x400000 | 0x8;
    h[32] = dxgi;
    h[33] = 3;
    h[35] = 1;
    let mut out: Vec<u8> = h.iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extend_from_slice(&e.data);
    out
}

/// BC7 of RGBA8 `rgba` (`w` × `h`, any size): each level of its mip chain
/// ([`crate::upscale::mip_chain`]) padded to whole blocks by repeating its
/// last row and column. Opaque images use the opaque settings.
pub fn encode_bc7(rgba: &[u8], w: u32, h: u32) -> Encoded {
    use block_compression::{BC7Settings, CompressionVariant};
    let opaque = rgba.chunks_exact(4).all(|p| p[3] == 255);
    let settings = if opaque { BC7Settings::opaque_fast() } else { BC7Settings::alpha_fast() };
    let levels = crate::upscale::mip_chain(rgba, w, h);
    let mut data = Vec::new();
    for (lw, lh, l) in &levels {
        let (pw, ph) = (lw.div_ceil(4) * 4, lh.div_ceil(4) * 4);
        let padded: std::borrow::Cow<'_, [u8]> = if (pw, ph) == (*lw, *lh) {
            std::borrow::Cow::Borrowed(l)
        } else {
            let mut p = vec![0u8; (pw * ph * 4) as usize];
            for y in 0..ph {
                for x in 0..pw {
                    let (sx, sy) = (x.min(lw - 1), y.min(lh - 1));
                    p[((y * pw + x) * 4) as usize..][..4].copy_from_slice(&l[((sy * lw + sx) * 4) as usize..][..4]);
                }
            }
            std::borrow::Cow::Owned(p)
        };
        let variant = CompressionVariant::BC7(settings);
        let mut blocks = vec![0u8; variant.blocks_byte_size(pw, ph)];
        block_compression::encode::compress_rgba8(variant, &padded, &mut blocks, pw, ph, pw * 4);
        data.extend_from_slice(&blocks);
    }
    Encoded { format: PixelFormat::Bc7, width: w, height: h, mips: levels.len() as u32, data }
}
