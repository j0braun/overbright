use super::*;

/// The cache key of a texture's map: its bytes, shape, format, the
/// strength and [`GENERATOR_VERSION`].
pub fn cache_key(t: &Texture, strength: f32) -> u64 {
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(&t.data[..]);
    h.update(&t.palette);
    h.update(
        &[
            t.width.to_le_bytes(),
            t.height.to_le_bytes(),
            t.mip_count.to_le_bytes(),
            GENERATOR_VERSION.to_le_bytes(),
            strength.to_bits().to_le_bytes(),
        ]
        .concat(),
    );
    h.update(format!("{:?}", t.format).as_bytes());
    h.digest()
}

/// The maps' cache directory (`<cache dir>/surface-maps`).
pub fn cache_dir() -> Option<std::path::PathBuf> {
    crate::cache::dir().map(|d| d.join(crate::cache::SURFACE_MAPS))
}

/// The map for `t` (of class `class`, drawn with worldreflect when
/// `reflect`) from the cache, else judged ([`normal_verdict`]) and made,
/// and stored: `<key>.rg8`, or an empty `<key>.none` for a texture that
/// gets no map. None: no map.
pub fn map_for(t: &Texture, class: Class, reflect: bool) -> Option<NormalMap> {
    map_for_source(t, class, reflect, None)
}

/// A texture pack's replacement of a texture (docs/notes/graphics/surface-maps.md "Texture
/// packs"): its bytes' hash (the cache key: one map per pack's file) and a
/// decoder of its top level as RGBA8 (run on the builder's thread).
pub struct PackSource {
    pub key: u64,
    pub decode: Box<dyn FnOnce() -> Option<(Vec<u8>, u32, u32)> + Send>,
}

/// The largest side a pack's replacement is judged and built at: a box
/// downscale by powers of two (an HD texture's grain at 4–8× the original
/// would alias into the 256² map without it).
pub const PACK_MAX: u32 = 512;

/// `rgba` (`w` × `h`) halved by 2 × 2 boxes until neither side exceeds
/// [`PACK_MAX`].
pub fn fit_pack(mut rgba: Vec<u8>, mut w: u32, mut h: u32) -> (Vec<u8>, u32, u32) {
    // Mismatched pixels and size: left as they are (the verdict rejects them).
    if rgba.len() < w as usize * h as usize * 4 {
        return (rgba, w, h);
    }
    while (w > PACK_MAX || h > PACK_MAX) && w >= 2 && h >= 2 {
        let (nw, nh) = (w / 2, h / 2);
        let mut out = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh as usize {
            for x in 0..nw as usize {
                for c in 0..4 {
                    let at = |xx: usize, yy: usize| rgba[(yy * w as usize + xx) * 4 + c] as u32;
                    out[(y * nw as usize + x) * 4 + c] = ((at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1)
                        + 2)
                        / 4) as u8;
                }
            }
        }
        (rgba, w, h) = (out, nw, nh);
    }
    (rgba, w, h)
}

/// The cache key of a pack replacement's map.
pub fn pack_cache_key(pack: u64, strength: f32) -> u64 {
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(b"pack");
    h.update(
        &[
            pack.to_le_bytes().as_slice(),
            &GENERATOR_VERSION.to_le_bytes(),
            &strength.to_bits().to_le_bytes(),
            &PACK_MAX.to_le_bytes(),
        ]
        .concat(),
    );
    h.digest()
}

/// [`map_for`], from the pack's replacement when `pack` is given (its
/// pixels judged and built; same UVs, so the map lines up), else from the
/// game's texture.
pub fn map_for_source(t: &Texture, class: Class, reflect: bool, pack: Option<PackSource>) -> Option<NormalMap> {
    let strength = class.params().normal;
    if strength <= 0.0 {
        return None;
    }
    let key = match &pack {
        Some(p) => pack_cache_key(p.key, strength),
        None => cache_key(t, strength),
    };
    let dir = cache_dir();
    let path = |ext: &str| dir.as_ref().map(|d| d.join(format!("{key:016x}.{ext}")));
    if let Some(p) = path("none")
        && p.exists()
    {
        return None;
    }
    if let Some(p) = path("rg8")
        && let Ok(b) = std::fs::read(&p)
        && let Some(m) = NormalMap::from_bytes(&b)
    {
        // Used: the cache's size limit evicts the maps used longest ago.
        crate::cache::touch_file(&p);
        return Some(m);
    }
    let (rgba, w, h) = match pack.and_then(|p| (p.decode)()) {
        Some((rgba, w, h)) => fit_pack(rgba, w, h),
        None => (t.decode_rgba(), t.width, t.height),
    };
    let info = TextureInfo { width: w, height: h, ..TextureInfo::of(t, reflect) };
    if normal_verdict(&info, class, &rgba, w, h) != NormalVerdict::Map {
        store(&path, "none", &[]);
        return None;
    }
    let m = build_normals(&rgba, w, h, strength);
    store(&path, "rg8", &m.bytes());
    Some(m)
}

/// A temporary name for writing `p`, unique per call: parallel jobs with the
/// same cache key must not truncate each other's file.
pub(super) fn temp_path(p: &std::path::Path) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    p.with_extension(format!("tmp{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)))
}

/// Writes a cache file atomically (a temporary file renamed).
fn store(path: &dyn Fn(&str) -> Option<std::path::PathBuf>, ext: &str, bytes: &[u8]) {
    if let Some(p) = path(ext) {
        // `p` is the cache folder joined with a file name: it has a parent.
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let tmp = temp_path(&p);
        if std::fs::write(&tmp, bytes).is_ok() {
            if std::fs::rename(&tmp, &p).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

/// The cache key of a car map: the source's key (the texture's bytes, or
/// the pack file's), the regions and [`GENERATOR_VERSION`].
pub fn car_cache_key(source: u64, regions: &[CarRegion]) -> u64 {
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(b"car");
    h.update(&source.to_le_bytes());
    h.update(&GENERATOR_VERSION.to_le_bytes());
    h.update(&PACK_MAX.to_le_bytes());
    for r in regions {
        h.update(format!("{:?}", r.class).as_bytes());
        h.update(&r.class.params().normal.to_bits().to_le_bytes());
        for t in &r.tris {
            for uv in t {
                h.update(&uv[0].to_bits().to_le_bytes());
                h.update(&uv[1].to_bits().to_le_bytes());
            }
        }
    }
    h.digest()
}

/// A texture's own key (its bytes, palette, shape and format).
pub fn texture_key(t: &Texture) -> u64 {
    cache_key(t, 0.0)
}

/// A car texture's map from the cache, else judged and made, and stored
/// (`car-<key>.rg8`, or an empty `.none`). `pack`: the texture pack's
/// replacement (built from its pixels, same UVs).
pub fn car_map_for(t: &Texture, regions: &[CarRegion], pack: Option<PackSource>) -> Option<CarMap> {
    if regions.is_empty() {
        return None;
    }
    let key = car_cache_key(pack.as_ref().map_or_else(|| texture_key(t), |p| p.key ^ 0x7061_636B), regions);
    let dir = cache_dir();
    let path = |ext: &str| dir.as_ref().map(|d| d.join(format!("car-{key:016x}.{ext}")));
    if let Some(p) = path("none")
        && p.exists()
    {
        return None;
    }
    if let Some(p) = path("rg8")
        && let Ok(b) = std::fs::read(&p)
        && let Some(m) = CarMap::from_bytes(&b)
    {
        crate::cache::touch_file(&p);
        return Some(m);
    }
    let (rgba, w, h) = match pack.and_then(|p| (p.decode)()) {
        Some((rgba, w, h)) => fit_pack(rgba, w, h),
        None => fit_pack(t.decode_rgba(), t.width, t.height),
    };
    match build_car_normals(&rgba, w, h, regions) {
        Some(m) => {
            store(&path, "rg8", &m.bytes());
            Some(m)
        }
        None => {
            store(&path, "none", &[]);
            None
        }
    }
}
