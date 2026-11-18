use super::*;

/// Whether a pack file decodes as the loader would take it (a PNG or a
/// generated DDS within the size limit).
pub(in crate::render) fn validate(path: &Path) -> Result<(), String> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("dds")) {
        return decode_dds(path, 4096).map(|_| ());
    }
    // The top level as `decode` reads it (its mips are made from it).
    let mut reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
    reader.set_format(image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map(|_| ()).map_err(|e| e.to_string())
}

pub(super) fn load(root: &Path, hash: u32, name: &str, max_dimension: u32) -> Option<Replacement> {
    // A hand-made PNG wins over a generated DDS of the same hash.
    let mut paths = vec![root.join(format!("{hash:08X}.png")), root.join(format!("{hash:08X}.dds"))];
    if !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".." {
        paths.push(root.join(format!("{name}.png")));
    }
    paths.into_iter().find_map(|path| {
        if !path.is_file() {
            return None;
        }
        let decoded = if path.extension().is_some_and(|e| e == "dds") {
            decode_dds(&path, max_dimension.min(4096))
        } else {
            decode(&path, max_dimension.min(4096)).map_err(|e| e.to_string())
        };
        match decoded {
            Ok(r) => {
                log::debug!("replacement texture: {}", path.display());
                Some(r)
            }
            Err(e) => {
                log::debug!("ignoring replacement texture {}: {e}", path.display());
                None
            }
        }
    })
}

fn decode(path: &Path, max_dimension: u32) -> image::ImageResult<Replacement> {
    let mut reader = image::ImageReader::open(path)?;
    reader.set_format(image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_dimension);
    limits.max_image_height = Some(max_dimension);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut level = reader.decode()?.into_rgba8();
    let (width, height) = level.dimensions();
    let mut bytes = level.as_raw().clone();
    let mut mips = 1;
    while level.width() > 1 || level.height() > 1 {
        level = next_mip(&level);
        bytes.extend_from_slice(level.as_raw());
        mips += 1;
    }
    Ok(Replacement { width, height, mips, format: None, bytes })
}

/// A DXT1/DXT3/DXT5 DDS with its mip chain (as `overbright-upscale` and
/// `import_tpf` write it).
fn decode_dds(path: &Path, max_dimension: u32) -> Result<Replacement, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    // DX9 DXT1/3/5, or DX10 BC1/2/3/7 (Overbright packs' compiled PNGs).
    let e = crate::obpack::parse_any_dds(&bytes).ok_or("not a DXT1/DXT3/DXT5 or DX10 BC1/BC2/BC3/BC7 DDS with whole 4x4 blocks")?;
    if e.width > max_dimension || e.height > max_dimension {
        return Err(format!("{}x{} exceeds {max_dimension}", e.width, e.height));
    }
    // Opaque DXT3/DXT5 art as BC1, the same texels in half the memory.
    if let Some(bc1) = crate::upscale::opaque_to_bc1(e.format, &e.data) {
        return Ok(Replacement {
            width: e.width,
            height: e.height,
            mips: e.mips,
            format: Some(crate::formats::texture::PixelFormat::Dxt1),
            bytes: bc1,
        });
    }
    Ok(Replacement { width: e.width, height: e.height, mips: e.mips, format: Some(e.format), bytes: e.data })
}

/// Filter premultiplied color, then return straight alpha for the existing blend modes.
/// The top level stays byte-for-byte intact; no color-space conversion is introduced.
fn next_mip(level: &image::RgbaImage) -> image::RgbaImage {
    let premultiplied = image::Rgba32FImage::from_fn(level.width(), level.height(), |x, y| {
        let p = level.get_pixel(x, y).0;
        let alpha = p[3] as f32 / 255.0;
        image::Rgba([p[0] as f32 / 255.0 * alpha, p[1] as f32 / 255.0 * alpha, p[2] as f32 / 255.0 * alpha, alpha])
    });
    let filtered = image::imageops::resize(
        &premultiplied,
        (level.width() / 2).max(1),
        (level.height() / 2).max(1),
        image::imageops::FilterType::Triangle,
    );
    image::RgbaImage::from_fn(filtered.width(), filtered.height(), |x, y| {
        let p = filtered.get_pixel(x, y).0;
        if p[3] <= 0.0 {
            return image::Rgba([0; 4]);
        }
        image::Rgba([
            (p[0] / p[3] * 255.0).round() as u8,
            (p[1] / p[3] * 255.0).round() as u8,
            (p[2] / p[3] * 255.0).round() as u8,
            (p[3] * 255.0).round() as u8,
        ])
    })
}
