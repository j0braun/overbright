//! Overbright texture packs: the project's own replacement-texture format
//! (docs/modding/texture-packs.md, "Overbright packs"). A pack is a folder
//!
//! ```text
//! <data>/overbright/packs/<id>/
//!   pack.toml               name, author, licence, priority (manifest)
//!   world/ car/ any/        <TEXTURE NAME>.png|.dds or <HASH>.png|.dds (subfolders allowed)
//! ```
//!
//! read into an [`index::Index`] (hash → file per domain), compiled once into
//! the asset cache (`<cache>/packs/<id>/{world,car}/<HASH>.dds|.png`: PNG to
//! BC7 with its mip chain, DDS as it is) and handed to the texture loader as
//! ordinary pack folders, first pack first. No pack installed: no folder, the
//! loader's native path. The player's order and on/off are `order.txt` in the
//! packs folder (the launcher's Mods page writes it); packs it does not name
//! follow, by their manifest's priority (higher first), then id.
//!
//! TexMod and TexWizard packages are imported by `texture_mods` (the
//! launcher); this module does not read them.

pub mod dds;
pub mod index;
pub mod manifest;

#[cfg(test)]
mod tests;

use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub use index::{Entry, Index, Kind};
pub use manifest::{Compress, Manifest};

/// Which renderer a texture is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Domain {
    World,
    Car,
}

impl Domain {
    pub const ALL: [Domain; 2] = [Domain::World, Domain::Car];
    pub fn folder(self) -> &'static str {
        match self {
            Domain::World => "world",
            Domain::Car => "car",
        }
    }
}

/// The order file's first line.
pub const ORDER_HEADER: &str = "# Overbright packs v1";
/// The order file's name in the packs folder.
pub const ORDER_FILE: &str = "order.txt";
/// The compiled files' encoder (a change recompiles every pack).
pub const ENCODER: &str = "bc7-fast-1";

/// The installed packs' folder: `<data>/overbright/packs`.
pub fn packs_dir() -> PathBuf {
    crate::platform::dirs::mods_root().join("packs")
}

/// The compiled packs' folder in the asset cache (None: no cache, then
/// packs are compiled into `<packs>/.compiled`).
pub fn compiled_root() -> PathBuf {
    match crate::cache::dir() {
        Some(c) => c.join("packs"),
        None => packs_dir().join(".compiled"),
    }
}

/// A pack id: lower-case letters, digits and `-`, 1..=64 long.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// An id made from a name (`My Roads!` → `my-roads`), not taken in `packs`.
pub fn new_id(packs: &Path, name: &str) -> String {
    let mut base: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(56);
    if base.is_empty() {
        base = "pack".into();
    }
    let mut id = base.clone();
    let mut n = 2;
    while packs.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// A pack read and checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pack {
    pub id: String,
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub index: Index,
}

impl Pack {
    /// The pack in `dir` (its id: the folder's name). A folder without
    /// `pack.toml` is a bare pack (format 0, named by its id). Refused: an
    /// id that is not [`valid_id`], a manifest [`Manifest::parse`] refuses,
    /// an index [`index::scan`] refuses.
    pub fn open(dir: &Path) -> Result<Pack> {
        let id = dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if !valid_id(&id) {
            bail!("{:?} is not a pack id (lower-case letters, digits and -)", id);
        }
        if !dir.is_dir() {
            bail!("{} is not a folder", dir.display());
        }
        let mf = dir.join(manifest::FILE);
        let manifest = match std::fs::metadata(&mf) {
            Ok(m) if m.len() as usize > manifest::MAX_BYTES => bail!("pack.toml is too large"),
            Ok(_) => Manifest::parse(&std::fs::read_to_string(&mf).context("pack.toml")?)?,
            Err(_) => Manifest::bare(&id),
        };
        let index = index::scan(dir)?;
        Ok(Pack { id, dir: dir.to_path_buf(), manifest, index })
    }

    /// The image for texture `hash` in `domain`, if the pack has one.
    pub fn file_for(&self, domain: Domain, hash: u32) -> Option<PathBuf> {
        self.index.domain(domain).get(&hash).map(|e| self.dir.join(&e.file))
    }
}

// ------------------------------------------------------------- the order

/// One line of `order.txt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderEntry {
    pub id: String,
    pub on: bool,
}

/// `order.txt`'s text read (unknown lines and repeated ids left out).
pub fn parse_order(text: &str) -> Vec<OrderEntry> {
    let mut out: Vec<OrderEntry> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let Some((state, id)) = l.split_once(char::is_whitespace) else { continue };
        let on = match state {
            "on" => true,
            "off" => false,
            _ => continue,
        };
        let id = id.trim();
        if valid_id(id) && !out.iter().any(|e| e.id == id) {
            out.push(OrderEntry { id: id.to_string(), on });
        }
    }
    out
}

pub fn format_order(list: &[OrderEntry]) -> String {
    let mut s = format!("{ORDER_HEADER}\n# first wins\n");
    for e in list {
        s += &format!("{} {}\n", if e.on { "on" } else { "off" }, e.id);
    }
    s
}

pub fn load_order(packs: &Path) -> Vec<OrderEntry> {
    std::fs::read_to_string(packs.join(ORDER_FILE)).map(|t| parse_order(&t)).unwrap_or_default()
}

/// Keep the order (atomic write).
pub fn save_order(packs: &Path, list: &[OrderEntry]) -> std::io::Result<()> {
    std::fs::create_dir_all(packs)?;
    crate::qol::saves::write_atomic(&packs.join(ORDER_FILE), format_order(list).as_bytes(), false)
}

/// An installed pack as the list shows it: its place, on or off, and the
/// pack or why it is refused.
#[derive(Clone, Debug)]
pub struct Listed {
    pub id: String,
    pub on: bool,
    pub pack: std::result::Result<Pack, String>,
}

/// Every pack folder in `packs`, in the order the loader uses them (the
/// first wins): those `order.txt` names, in its order; then the others,
/// on, by priority (higher first), then id. Refused packs are listed with
/// their reason and never used.
pub fn list_in(packs: &Path) -> Vec<Listed> {
    let order = load_order(packs);
    let mut found: BTreeMap<String, std::result::Result<Pack, String>> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(packs) {
        for e in rd.flatten() {
            let Ok(name) = e.file_name().into_string() else { continue };
            if name.starts_with('.') || !e.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            found.insert(name, Pack::open(&e.path()).map_err(|e| format!("{e:#}")));
        }
    }
    let mut out = Vec::new();
    for o in &order {
        if let Some(p) = found.remove(&o.id) {
            out.push(Listed { id: o.id.clone(), on: o.on, pack: p });
        }
    }
    let mut rest: Vec<Listed> = found.into_iter().map(|(id, pack)| Listed { id, on: true, pack }).collect();
    let prio = |l: &Listed| l.pack.as_ref().map_or(i32::MIN, |p| p.manifest.priority);
    rest.sort_by(|a, b| prio(b).cmp(&prio(a)).then_with(|| a.id.cmp(&b.id)));
    out.extend(rest);
    out
}

/// The packs that are used, first wins.
pub fn enabled_in(packs: &Path) -> Vec<Pack> {
    list_in(packs).into_iter().filter(|l| l.on).filter_map(|l| l.pack.ok()).collect()
}

/// Which pack supplies texture `hash` in `domain` (the first that has it).
pub fn winner<'a>(packs: &'a [Pack], domain: Domain, hash: u32) -> Option<&'a Pack> {
    packs.iter().find(|p| p.index.domain(domain).contains_key(&hash))
}

// ------------------------------------------------------------- compiling

/// What a [`sync`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub compiled: usize,
    pub kept: usize,
    pub removed: usize,
    /// Images left out, with the reason (the pack is used without them).
    pub failed: Vec<(PathBuf, String)>,
}

/// A source file's stamp: its size and modification time (ns).
fn stamp(path: &Path) -> Option<(u64, u128)> {
    let m = std::fs::metadata(path).ok()?;
    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    Some((m.len(), t))
}

const STAMPS: &str = "stamps.tsv";

/// The compiled file of one texture: domain, hash → (source, size, time,
/// encoder, output file name).
type Stamps = BTreeMap<(String, u32), (String, u64, u128, String, String)>;

fn read_stamps(out: &Path) -> Stamps {
    let mut s = Stamps::new();
    let Ok(text) = std::fs::read_to_string(out.join(STAMPS)) else { return s };
    for l in text.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if let [d, h, src, len, t, enc, file] = f[..]
            && let (Ok(h), Ok(len), Ok(t)) = (u32::from_str_radix(h, 16), len.parse(), t.parse())
        {
            s.insert((d.to_string(), h), (src.to_string(), len, t, enc.to_string(), file.to_string()));
        }
    }
    s
}

fn write_stamps(out: &Path, s: &Stamps) -> std::io::Result<()> {
    let mut text = format!("# overbright compiled pack {ENCODER}\n");
    for ((d, h), (src, len, t, enc, file)) in s {
        text += &format!("{d}\t{h:08X}\t{src}\t{len}\t{t}\t{enc}\t{file}\n");
    }
    crate::qol::saves::write_atomic(&out.join(STAMPS), text.as_bytes(), false)
}

/// The largest side and decoded size the loader takes (as for PNG packs).
pub const MAX_SIDE: u32 = 4096;

/// A PNG decoded with the loader's limits.
pub fn decode_png(bytes: &[u8]) -> Result<image::RgbaImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    reader.set_format(image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.into_rgba8())
}

/// A DDS the loader reads (DX9 DXT1/3/5 or DX10 BC1/2/3/7), as levels.
pub fn parse_any_dds(bytes: &[u8]) -> Option<crate::upscale::Encoded> {
    crate::upscale::parse_dds(bytes).or_else(|| dds::parse_dx10(bytes))
}

/// One image made ready for the GPU: (output extension, bytes, encoder).
fn build(src: &Path, kind: Kind, compress: Compress) -> Result<(&'static str, Vec<u8>, &'static str)> {
    let bytes = std::fs::read(src)?;
    match kind {
        Kind::Dds => {
            let e = parse_any_dds(&bytes).context("not a DDS of DXT1/DXT3/DXT5 or DX10 BC1/BC2/BC3/BC7")?;
            if e.width > MAX_SIDE || e.height > MAX_SIDE {
                bail!("{}x{} is larger than {MAX_SIDE}", e.width, e.height);
            }
            Ok(("dds", bytes, "dds"))
        }
        Kind::Png => {
            let img = decode_png(&bytes)?;
            let (w, h) = img.dimensions();
            // BC7 needs whole blocks at the top level; other sizes stay PNG.
            if compress == Compress::None || w % 4 != 0 || h % 4 != 0 {
                return Ok(("png", bytes, "png"));
            }
            Ok(("dds", dds::dx10_bytes(&dds::encode_bc7(img.as_raw(), w, h)), ENCODER))
        }
    }
}

/// Bring `pack`'s compiled folder `out` up to date: each texture whose
/// source, size, time or encoder changed is built again (PNG → BC7 DDS,
/// unless the manifest says `compress = "none"` or the size is not whole
/// 4 × 4 blocks; DDS checked and copied), files of textures the pack no
/// longer has are removed. Images that cannot be read are left out and
/// reported. `progress(done, total)` after each texture.
pub fn sync(pack: &Pack, out: &Path, progress: &mut dyn FnMut(usize, usize)) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let old = read_stamps(out);
    let mut new = Stamps::new();
    // (domain, hash, entry) wanted.
    let wanted: Vec<(Domain, &Entry)> =
        Domain::ALL.iter().flat_map(|&d| pack.index.domain(d).values().map(move |e| (d, e))).collect();
    let total = wanted.len();
    // A source built once for both domains (`any/`).
    let mut built: HashMap<PathBuf, std::result::Result<(&'static str, Vec<u8>, &'static str), String>> =
        HashMap::new();
    for (n, (d, e)) in wanted.iter().enumerate() {
        let src = pack.dir.join(&e.file);
        let rel = e.file.to_string_lossy().replace('\\', "/");
        let Some((len, t)) = stamp(&src) else { continue };
        let key = (d.folder().to_string(), e.hash);
        let dir = out.join(d.folder());
        let want_enc = |enc: &str| match (e.kind, pack.manifest.compress) {
            (Kind::Dds, _) => enc == "dds",
            (Kind::Png, Compress::None) => enc == "png",
            // A size BC7 cannot take stays PNG; anything else must be BC7 now.
            (Kind::Png, Compress::Bc7) => enc == ENCODER || enc == "png",
        };
        if let Some(s) = old.get(&key)
            && s.0 == rel
            && s.1 == len
            && s.2 == t
            && want_enc(&s.3)
            && dir.join(&s.4).is_file()
        {
            new.insert(key, s.clone());
            report.kept += 1;
            progress(n + 1, total);
            continue;
        }
        let r = built
            .entry(src.clone())
            .or_insert_with(|| build(&src, e.kind, pack.manifest.compress).map_err(|e| format!("{e:#}")));
        match r {
            Ok((ext, bytes, enc)) => {
                std::fs::create_dir_all(&dir)?;
                let file = format!("{:08X}.{ext}", e.hash);
                // The other kind's file would shadow or stay behind.
                let other = format!("{:08X}.{}", e.hash, if *ext == "png" { "dds" } else { "png" });
                let _ = std::fs::remove_file(dir.join(other));
                crate::qol::saves::write_atomic(&dir.join(&file), bytes, false)?;
                new.insert(key, (rel, len, t, enc.to_string(), file));
                report.compiled += 1;
            }
            Err(why) => {
                if !report.failed.iter().any(|(f, _)| *f == e.file) {
                    report.failed.push((e.file.clone(), why.clone()));
                }
            }
        }
        progress(n + 1, total);
    }
    // Files of textures no longer in the pack.
    for (key, s) in &old {
        if !new.contains_key(key) || new[key].4 != s.4 {
            if std::fs::remove_file(out.join(&key.0).join(&s.4)).is_ok() {
                report.removed += 1;
            }
        }
    }
    std::fs::create_dir_all(out)?;
    write_stamps(out, &new)?;
    Ok(report)
}

/// The folders the loader searches, world and car, first wins: each enabled
/// pack of `packs`, compiled into `compiled/<id>` first. Nothing installed:
/// empty (the native path).
pub fn roots_in(packs: &Path, compiled: &Path) -> [Vec<PathBuf>; 2] {
    let mut out = [Vec::new(), Vec::new()];
    for p in enabled_in(packs) {
        let dir = compiled.join(&p.id);
        let started = Instant::now();
        match sync(&p, &dir, &mut |_, _| {}) {
            Ok(r) => {
                if r.compiled > 0 || r.removed > 0 {
                    log::info!(
                        "texture packs: {} compiled ({} built, {} removed) in {:.0} ms",
                        p.id,
                        r.compiled,
                        r.removed,
                        started.elapsed().as_secs_f64() * 1000.0
                    );
                }
                for (f, why) in &r.failed {
                    log::warn!("texture packs: {}: {} left out: {why}", p.id, f.display());
                }
            }
            Err(e) => {
                log::warn!("texture packs: {} not used: {e:#}", p.id);
                continue;
            }
        }
        for (i, domain) in Domain::ALL.iter().enumerate() {
            let d = dir.join(domain.folder());
            if d.is_dir() {
                out[i].push(d);
            }
        }
    }
    out
}

struct Seen {
    at: Instant,
    dir: PathBuf,
    world: Vec<PathBuf>,
    car: Vec<PathBuf>,
}
static SEEN: std::sync::Mutex<Option<Seen>> = std::sync::Mutex::new(None);

/// [`roots_in`] for the player's packs, looked at again at most every half
/// second (the loader asks for every texture).
pub fn enabled_roots(domain: Domain) -> Vec<PathBuf> {
    let dir = packs_dir();
    let mut c = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let fresh = c.as_ref().is_some_and(|s| s.dir == dir && s.at.elapsed() < Duration::from_millis(500));
    if !fresh {
        let [world, car] = if dir.is_dir() { roots_in(&dir, &compiled_root()) } else { [Vec::new(), Vec::new()] };
        *c = Some(Seen { at: Instant::now(), dir, world, car });
    }
    let s = c.as_ref().unwrap();
    match domain {
        Domain::World => s.world.clone(),
        Domain::Car => s.car.clone(),
    }
}

/// Make [`enabled_roots`] look again at its next call (a pack was written).
pub fn forget_roots() {
    *SEEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

// ------------------------------------------------------------- writing

/// Make a new pack folder `packs/<id>` with `manifest` (refused when it exists).
pub fn create(packs: &Path, id: &str, manifest: &Manifest) -> Result<Pack> {
    if !valid_id(id) {
        bail!("{id:?} is not a pack id");
    }
    let dir = packs.join(id);
    if dir.exists() {
        bail!("{} exists", dir.display());
    }
    std::fs::create_dir_all(&dir)?;
    let mut m = manifest.clone();
    m.format = manifest::FORMAT;
    crate::qol::saves::write_atomic(&dir.join(manifest::FILE), m.to_toml().as_bytes(), false)?;
    Pack::open(&dir)
}

/// Rewrite a pack's manifest.
pub fn write_manifest(pack: &Pack, manifest: &Manifest) -> Result<()> {
    let mut m = manifest.clone();
    m.format = manifest::FORMAT;
    Manifest::parse(&m.to_toml())?;
    crate::qol::saves::write_atomic(&pack.dir.join(manifest::FILE), m.to_toml().as_bytes(), false)?;
    Ok(())
}

/// The file stem a texture is saved under: its name when it hashes to it and
/// is a safe file name, else `0x` and its hash.
pub fn stem_for(hash: u32, name: &str) -> String {
    let safe = !name.is_empty()
        && name.len() <= 120
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        && !name.starts_with('.');
    match index::key_of_stem(name) {
        Some((h, Some(_))) if safe && h == hash => name.to_string(),
        _ => format!("0x{hash:08X}"),
    }
}

/// Put an image (PNG bytes) into `pack` as texture `hash` (named `name`) for
/// `domain`: written to `<domain>/<stem>.png`, replacing what the pack had
/// for it in that folder. Returns the file written.
pub fn put_png(pack: &Pack, domain: Domain, hash: u32, name: &str, png: &[u8]) -> Result<PathBuf> {
    decode_png(png).context("the image")?;
    let dir = pack.dir.join(domain.folder());
    std::fs::create_dir_all(&dir)?;
    // The texture's earlier file in this folder (another name, or a DDS).
    if let Some(e) = pack.index.domain(domain).get(&hash)
        && e.file.starts_with(domain.folder())
    {
        let _ = std::fs::remove_file(pack.dir.join(&e.file));
    }
    let path = dir.join(format!("{}.png", stem_for(hash, name)));
    crate::qol::saves::write_atomic(&path, png, false)?;
    Ok(path)
}

/// Copy a pack folder (from a download or another place) into `packs` as an
/// installed pack: checked first ([`Pack::open`] on the source, renamed into
/// a valid id when its folder name is not one), copied into `.install-<id>`,
/// then renamed into place; an existing pack of that id is replaced. Only the
/// manifest, the images and `preview.png` / `README.txt` / `LICENSE.txt` are
/// copied.
pub fn install_folder(src: &Path, packs: &Path) -> Result<String> {
    // Read through a valid-id view: the source folder's name may be anything.
    let mf = src.join(manifest::FILE);
    let manifest = match std::fs::read_to_string(&mf) {
        Ok(t) => Manifest::parse(&t)?,
        Err(_) => Manifest::bare(&src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
    };
    let index = index::scan(src)?;
    if index.world.is_empty() && index.car.is_empty() {
        bail!("{} has no world/, car/ or any/ images", src.display());
    }
    let folder = src.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let id = if valid_id(folder) { folder.to_string() } else { new_id(packs, &manifest.name) };
    std::fs::create_dir_all(packs)?;
    let stage = packs.join(format!(".install-{id}"));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    let mut files: Vec<PathBuf> = index.world.values().chain(index.car.values()).map(|e| e.file.clone()).collect();
    files.sort();
    files.dedup();
    for extra in [manifest::FILE, "preview.png", "README.txt", "LICENSE.txt"] {
        if src.join(extra).is_file() {
            files.push(PathBuf::from(extra));
        }
    }
    for f in &files {
        let to = stage.join(f);
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(src.join(f), &to).with_context(|| f.display().to_string())?;
    }
    let dest = packs.join(&id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::rename(&stage, &dest)?;
    Pack::open(&dest)?;
    Ok(id)
}

/// Remove an installed pack (its folder and its compiled files) and its line
/// in the order.
pub fn remove(packs: &Path, compiled: &Path, id: &str) -> Result<()> {
    if !valid_id(id) {
        bail!("{id:?} is not a pack id");
    }
    let dir = packs.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let c = compiled.join(id);
    if c.exists() {
        std::fs::remove_dir_all(&c)?;
    }
    let mut order = load_order(packs);
    if order.iter().any(|e| e.id == id) {
        order.retain(|e| e.id != id);
        save_order(packs, &order)?;
    }
    Ok(())
}
