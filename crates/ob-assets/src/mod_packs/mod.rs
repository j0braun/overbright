//! Mod packs in the game (docs/modding/asset-format/layering.md, layout.md): the packs the player switched on in
//! `<app>/mods/order.txt` are mounted on top of the base set, in that order (the first line strongest), through the
//! layer stack (`ob_asset_format::modpack`) and the merge engine (clashes).
//!
//! A pack made from a Studio map mod holds the map mod's own files (`map.obmap`, `textures/world|car/…`); the game
//! reads them as it reads a map mod in `maps/`, above the mods `maps/enabled.txt` lists. A pack file (`.obpk`) is
//! unpacked once into the cache (keyed by its digest, so a replaced file is unpacked again); a folder pack is read in
//! place. With no `order.txt`, or no pack switched on, nothing is read beyond that file and every result is the
//! native one.
//!
//! A pack that is refused (another Overbright, another base set, a missing dependency) is not mounted at all and the
//! log says why; clashes between packs are logged, never errors.

mod content;
mod credits;
mod convert;
mod inside;
mod layers;
mod pack_export;
mod report;
mod stack;

pub use inside::{Inside, inside};
pub use stack::{Layer, LayerKind, hud_layer, texture_layers, texture_stack_dirs};

pub use layers::{added_key, heading, layers, placement_key, race_key};
pub use credits::Credit;
pub use report::{of_id as report_of_id, of_path as report_of_path, text as report_text};
pub use convert::{Converted, convert_legacy};
pub use pack_export::{ExportOpts, Exported, export_map_mod, export_mod_dir};

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use ob_asset_format::layering::{self, Clash, KindRules};
use ob_asset_format::modpack::{self, BaseId};

use crate::tools::map::MapMod;
use crate::tools::map::store;

static BASE: RwLock<Option<BaseId>> = RwLock::new(None);
/// The mounted packs and where each is read from, weakest first.
static ROOTS: RwLock<Vec<(String, PathBuf)>> = RwLock::new(Vec::new());

/// Tests that set or read the process-wide stack (the mounted base and packs, the cache folder, the map mods'
/// folders read through `mount_all`) take turns: hold this for the whole test.
#[cfg(any(test, feature = "dev-harness"))]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// `<app>/mods`.
pub fn mods_dir() -> PathBuf {
    modpack::mods_dir(&crate::platform::dirs::app())
}

/// This Overbright's version.
pub fn engine() -> semver::Version {
    env!("CARGO_PKG_VERSION").parse().unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}

/// The environment variable of a trial run: the id of a pack that is on for this run only.
pub const TRY_VAR: &str = "NFSU2_TRY_PACK";

/// The pack this run tries (the launcher's "Try it once"), if any.
pub fn tried() -> Option<String> {
    std::env::var(TRY_VAR).ok().filter(|v| !v.is_empty())
}

/// Records the base set the game mounted (None: the game folder is read).
pub fn set_base(b: Option<BaseId>) {
    *BASE.write().unwrap_or_else(|e| e.into_inner()) = b;
}

/// The base set the game mounted.
pub fn mounted_base() -> Option<BaseId> {
    BASE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The base set in `base_dir` that `spec` names, read from its import record (no bundle is opened).
pub fn base_of(spec: &str, base_dir: &Path) -> Option<BaseId> {
    let set = ob_asset_format::mount::resolve(spec, base_dir).ok()?;
    let rec = ob_asset_format::import::read_record(&set).ok()?;
    Some(BaseId { id: rec.id, identity: format!("sha256:{}", hex(&rec.identity)) })
}

/// The base set a start would mount: the one the game mounted, else the active one on disk (the launcher and the
/// command-line tools use this one).
pub fn base_in_use() -> Option<BaseId> {
    mounted_base().or_else(|| base_of("active", &crate::platform::dirs::app().join("base")))
}

/// What the game mounts from the packs.
#[derive(Default)]
pub struct Mounted {
    /// The map mods of the packs, weakest first, known by their pack ids.
    pub mods: Vec<(String, MapMod)>,
}

/// The folder a mounted pack's files are read from (`map.obmap`, `textures/`).
pub fn root_of(id: &str) -> Option<PathBuf> {
    ROOTS.read().unwrap_or_else(|e| e.into_inner()).iter().find(|r| r.0 == id).map(|r| r.1.clone())
}

/// The texture folders of the mounted packs (the world's, or the cars' with `car`), the strongest pack first. None
/// mounted: empty.
pub fn texture_dirs(car: bool) -> Vec<PathBuf> {
    let roots = ROOTS.read().unwrap_or_else(|e| e.into_inner());
    roots.iter().rev().flat_map(|r| content::texture_dirs(&r.1, car)).collect()
}

/// Reads the packs switched on against `base`: the plan (refusals, notices), the map mods and where each is read from.
/// `taken`: the names the plain map mods already use.
fn load(base: Option<&BaseId>, taken: &[String], extra: Option<&str>) -> (modpack::Plan, Mounted, Vec<(String, PathBuf)>) {
    let plan = modpack::plan_with(&mods_dir(), base, &engine(), extra);
    let mut out = Mounted::default();
    let mut roots = Vec::new();
    for p in &plan.layers {
        if taken.iter().any(|t| *t == p.id) {
            log::warn!("mod pack {} is not used: a map mod folder in maps/ has the same name", p.id);
            continue;
        }
        let root = match materialize(p) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("mod pack {} is not used: {e}", p.id);
                continue;
            }
        };
        roots.push((p.id.clone(), root.clone()));
        match content::read(&root) {
            Ok(Some(m)) => out.mods.push((p.id.clone(), m)),
            Ok(None) => log::info!("mod pack {}: no map data in it", p.id),
            Err(e) => {
                roots.pop();
                log::warn!("mod pack {} is not used: {e}", p.id);
            }
        }
    }
    (plan, out, roots)
}

/// Mounts the packs switched on in `order.txt`. `taken`: the names the plain map mods already use (a pack with such an
/// id is not mounted; the log says so).
pub fn mount(taken: &[String]) -> Mounted {
    let extra = tried();
    if extra.is_none() && !mods_dir().join(modpack::ORDER_FILE).is_file() {
        return Mounted::default();
    }
    let base = mounted_base();
    let (plan, out, roots) = load(base.as_ref(), taken, extra.as_deref());
    if let Some(id) = &extra {
        log::info!("mod pack {id}: tried for this run only; order.txt is not changed");
    }
    for n in &plan.notices {
        log::warn!("mod packs: {n}");
    }
    for (id, why) in &plan.refused {
        log::warn!("mod pack {id} is not used: {why}");
    }
    if !out.mods.is_empty() {
        log::info!(
            "mod packs mounted over {} (weakest first): {}",
            base.map_or("the game folder".to_string(), |b| format!("base set {}", b.id)),
            out.mods.iter().map(|m| m.0.as_str()).collect::<Vec<_>>().join(", ")
        );
        let clashes = clashes(&out.mods);
        for c in clashes.iter().take(20) {
            log::warn!(
                "mod packs clash: {} {}: {} is overruled by {}",
                c.key,
                c.field.as_deref().unwrap_or("(whole item)"),
                c.weaker,
                c.stronger
            );
        }
        if clashes.len() > 20 {
            log::warn!("mod packs clash: {} more not listed", clashes.len() - 20);
        }
    }
    *ROOTS.write().unwrap_or_else(|e| e.into_inner()) = roots;
    out
}

/// Everything the game reads map data from, weakest first: the plain map mods `enabled.txt` lists in `maps`, then the
/// packs switched on in `order.txt` (a pack whose id a map mod folder already has is not mounted). The one load path
/// for both kinds: the game, the launcher's checks and the tools all read the stack here.
pub fn mount_all(maps: &Path) -> Vec<(String, MapMod)> {
    let mut mods = store::load_enabled(maps);
    let taken: Vec<String> = mods.iter().map(|m| m.0.clone()).collect();
    mods.extend(mount(&taken).mods);
    mods
}

/// The clashes among the packs that are on, as the launcher shows them (nothing is logged or mounted): each pair of
/// packs and how many values they both give, weaker first.
pub fn clash_pairs(base: Option<&BaseId>) -> Vec<(String, String, usize)> {
    clash_pairs_with(base, None)
}

/// [`clash_pairs`] with the pack `extra` counted as on.
pub fn clash_pairs_with(base: Option<&BaseId>, extra: Option<&str>) -> Vec<(String, String, usize)> {
    let (_, out, _) = load(base, &[], extra);
    let mut pairs: Vec<(String, String, usize)> = Vec::new();
    for c in clashes(&out.mods) {
        match pairs.iter_mut().find(|p| p.0 == c.weaker && p.1 == c.stronger) {
            Some(p) => p.2 += 1,
            None => pairs.push((c.weaker, c.stronger, 1)),
        }
    }
    pairs
}

/// What the packs `mods` (weakest first) both change: placements and races as the layering tables, the stronger
/// layer winning (layering.md §4).
pub fn clashes(mods: &[(String, MapMod)]) -> Vec<Clash> {
    let (mut base, mut places, mut races) = (Vec::new(), Vec::new(), Vec::new());
    for (id, m) in mods {
        let (b, p, r) = layers(m, id);
        for (k, t) in b {
            if !base.iter().any(|(x, _): &(String, toml::Table)| *x == k) {
                base.push((k, t));
            }
        }
        places.push(p);
        races.push(r);
    }
    let mut v = layering::merge(base, &places, &KindRules::ALL).clashes;
    v.extend(layering::merge(Vec::new(), &races, &KindRules::ALL).clashes);
    v
}

/// A pack's files on disk: its folder, or the folder its pack file was unpacked into.
pub fn materialize(p: &modpack::Installed) -> Result<PathBuf, String> {
    use ob_asset_format::bundle::{self, Bundle};
    let file = match &p.source {
        modpack::Source::Folder(d) => return Ok(d.clone()),
        modpack::Source::File(f) => f,
    };
    let b = Bundle::open(file, bundle::Kind::PackFile).map_err(|e| e.to_string())?;
    let key = b.digest().to_string().replace(':', "-");
    let cache = crate::cache::dir().unwrap_or_else(|| std::env::temp_dir().join("overbright-cache"));
    let root = cache.join("mod-packs").join(format!("{}-{}", p.id, &key[..key.len().min(24)]));
    if root.join(".complete").is_file() {
        return Ok(root);
    }
    b.verify().map_err(|e| e.to_string())?;
    let tmp = cache.join("mod-packs").join(format!(".tmp-{}-{}", p.id, ob_core::unique_tag()));
    let _ = std::fs::remove_dir_all(&tmp);
    let wanted = |path: &str| {
        path == store::FILE || ["textures/", "world/", "races/", "cars/", "global/"].iter().any(|d| path.starts_with(d))
    };
    let mut total = 0u64;
    let r = (|| -> Result<(), String> {
        for e in b.entries().iter().filter(|e| wanted(&e.path)) {
            total += e.size;
            if total > crate::tools::map::share::MAX_BYTES
                || (e.path == store::FILE && e.size > crate::tools::map::format::MAX_FILE_BYTES)
            {
                return Err("too large for a mod".into());
            }
            let data = b.read(e).map_err(|x| x.to_string())?;
            let out = tmp.join(&e.path);
            if let Some(d) = out.parent() {
                std::fs::create_dir_all(d).map_err(|x| format!("{}: {x}", d.display()))?;
            }
            std::fs::write(&out, &*data).map_err(|x| format!("{}: {x}", out.display()))?;
        }
        std::fs::create_dir_all(&tmp).map_err(|x| x.to_string())?;
        std::fs::write(tmp.join(".complete"), b"").map_err(|x| x.to_string())
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&root);
    if let Err(e) = std::fs::rename(&tmp, &root) {
        // Another start unpacked the same file meanwhile.
        let _ = std::fs::remove_dir_all(&tmp);
        if !root.join(".complete").is_file() {
            return Err(format!("{}: {e}", root.display()));
        }
    }
    Ok(root)
}

#[cfg(test)]
mod tests;
