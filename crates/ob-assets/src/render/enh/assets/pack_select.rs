use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::upscale::Category;
use crate::upscale_pack::*;

// ------------------------------------------------------------ the choice

pub use crate::qol::pack_choice::{choice, set_choice};
/// The adapter's dedicated memory in bytes (0 unknown) and whether it is an
/// integrated / software adapter.
static VRAM: AtomicU64 = AtomicU64::new(0);
static SMALL: AtomicBool = AtomicBool::new(false);

/// The adapter (from `render::Gpu::new`): its memory, read from the DRM
/// sysfs node with the same PCI vendor/device (amdgpu, i915/xe export
/// `mem_info_vram_total`; elsewhere unknown). **Inferred**: a GPU whose
/// driver does not export it counts as large unless it is integrated.
pub fn set_adapter(info: &wgpu::AdapterInfo) {
    SMALL.store(matches!(info.device_type, wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::Cpu), Ordering::Relaxed);
    VRAM.store(sysfs_vram(info.vendor, info.device).unwrap_or(0), Ordering::Relaxed);
}

fn sysfs_vram(vendor: u32, device: u32) -> Option<u64> {
    let hex = |p: &Path| {
        std::fs::read_to_string(p).ok().and_then(|s| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
    };
    for e in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
        let d = e.path().join("device");
        if hex(&d.join("vendor")) == Some(vendor) && hex(&d.join("device")) == Some(device) {
            if let Some(v) =
                std::fs::read_to_string(d.join("mem_info_vram_total")).ok().and_then(|s| s.trim().parse().ok())
            {
                return Some(v);
            }
        }
    }
    None
}

/// The adapter's video memory in bytes (0: unknown; for the GPU class).
pub fn vram_bytes() -> u64 {
    VRAM.load(Ordering::Relaxed)
}

/// Under 8 GB of video memory (a 8 GB card reports a little less than
/// 8 GiB; the line is 7.5 GiB), or an integrated adapter of unknown size.
pub fn low_vram() -> bool {
    match VRAM.load(Ordering::Relaxed) {
        0 => SMALL.load(Ordering::Relaxed),
        v => v < (15u64 << 30) / 2,
    }
}

/// The scale the loader holds the bulk categories ([`CAPPED`]) to: Lean
/// always, On on a GPU under 8 GB; None otherwise.
pub fn cap() -> Option<u32> {
    match choice() {
        2 => Some(2),
        1 if low_vram() => Some(2),
        _ => None,
    }
}

/// What the cap applies to: buildings and the other bulk world categories,
/// and car parts. Roads, signs, rims and decals keep their scale.
pub const CAPPED: &[Category] = &[Category::Building, Category::Cutout, Category::Decal, Category::CarPart];

/// The default pack folder: `platform::dirs::packs()` (`<data home>/overbright/texture-pack`).
pub fn pack_dir() -> PathBuf {
    crate::platform::dirs::packs()
}

pub fn world_dir() -> PathBuf {
    pack_dir().join("world")
}

pub fn car_dir() -> PathBuf {
    pack_dir().join("car")
}

/// Whether a pack has been generated (its world manifest exists). Cached;
/// the generator refreshes it.
pub fn generated() -> bool {
    static STATE: AtomicU8 = AtomicU8::new(0);
    static DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
    let dir = world_dir();
    let mut d = DIR.lock().unwrap_or_else(|e| e.into_inner());
    if d.as_ref() != Some(&dir) || REFRESH.swap(false, Ordering::Relaxed) {
        *d = Some(dir.clone());
        STATE.store(if dir.join(MANIFEST).is_file() { 2 } else { 1 }, Ordering::Relaxed);
    }
    STATE.load(Ordering::Relaxed) == 2
}
pub static REFRESH: AtomicBool = AtomicBool::new(false);

/// The generated folder the row selects for a domain (car or world), if
/// it is on and a pack exists.
pub fn selected(car: bool) -> Option<PathBuf> {
    if choice() == 0 || !generated() {
        return None;
    }
    let d = if car { car_dir() } else { world_dir() };
    d.is_dir().then_some(d)
}

/// docs/modding/texmod-import.md: the imported pack's folder for a domain,
/// `<pack dir>/imported/{world,car}` (what `import_tpf --install` writes,
/// or a link to a pack made elsewhere).
pub fn imported_dir(car: bool) -> PathBuf {
    pack_dir().join("imported").join(if car { "car" } else { "world" })
}

/// Whether an imported pack exists (either domain's folder).
pub fn imported() -> bool {
    imported_dir(false).is_dir() || imported_dir(true).is_dir()
}

/// Every folder the World Texture Pack row selects for a domain, first
/// match wins: the imported TexMod pack (hand-made art) before the
/// generated upscale, which fills in what the import does not cover.
pub fn selected_all(car: bool) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if choice() != 0 {
        let d = imported_dir(car);
        if d.is_dir() {
            v.push(d);
        }
    }
    v.extend(selected(car));
    v
}

/// The categories of a pack folder by hash, from its manifest (cached per
/// folder; the loader's cap reads it).
pub fn manifest_categories(dir: &Path) -> Arc<HashMap<u32, Category>> {
    type Stamp = Option<(std::time::SystemTime, u64)>;
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, (Stamp, Arc<HashMap<u32, Category>>)>>> = OnceLock::new();
    let mut c = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    // Keyed on the manifest's mtime and length: a pack generated (or
    // regenerated) after the first call is picked up.
    let stamp: Stamp = std::fs::metadata(dir.join(MANIFEST)).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
    if let Some((st, m)) = c.get(dir)
        && *st == stamp
    {
        return m.clone();
    }
    let text = std::fs::read_to_string(dir.join(MANIFEST)).unwrap_or_default();
    let m: HashMap<u32, Category> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("hash"))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Some((u32::from_str_radix(f.first()?, 16).ok()?, category_named(f.get(3)?)?))
        })
        .collect();
    let m = Arc::new(m);
    c.insert(dir.to_path_buf(), (stamp, m.clone()));
    m
}

pub fn category_named(s: &str) -> Option<Category> {
    use Category::*;
    [
        Road, Sign, Building, Natural, Cutout, Decal, Skip, CarSkin, CarPart, CarLamp, CarRim, CarMask, CarDecal,
        CarGlass, UiPicture,
    ]
    .into_iter()
    .find(|c| format!("{c:?}") == s)
}

/// Mip levels to drop from a `width`-wide pack texture of category `cat`
/// for a source `orig_width` wide under the cap `cap` (0: none).
pub fn capped_levels(cat: Option<Category>, width: u32, orig_width: u32, cap: Option<u32>) -> u32 {
    let (Some(cat), Some(cap)) = (cat, cap) else { return 0 };
    if !CAPPED.contains(&cat) {
        return 0;
    }
    let mut n = 0;
    while (width >> n) > orig_width.max(1) * cap && (width >> n) > 4 {
        n += 1;
    }
    n
}
