//! Map mods and the map editor's logic (docs/modding/map-editor.md).
//!
//! A map mod is an overlay in Overbright's own data folder
//! (`<data>/maps/<folder>/map.obmap`, [`store`]): scenery edits (move,
//! delete, add placements) and custom races on a base track's road
//! network. The mods listed in `maps/enabled.txt` are applied to each
//! world as it loads ([`load_world`]) and their races can be raced from
//! the editor. The game's own files are never written. With no mod
//! enabled nothing is read beyond that list and the world is the
//! original's.
//!
//! The editor itself (free camera, picking, the edits) is opt-in: only a
//! start with `--editor` makes it reachable ([`editor_enabled`]).

pub mod assets;
pub mod edit;
pub mod session;
// The map data without a world (crates/ob-map), at its old paths.
pub use ob_map::tools::map::{format, ops, race, sim, store};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub use format::*;

/// What the map mods did to one world (`World::map_overlay`).
#[derive(Clone, Debug, Default)]
pub struct Applied {
    /// The original placements a mod moved or hid, with their names as
    /// the original data has them.
    pub originals: HashMap<usize, PropRef>,
    /// Where the original placements a mod moved stood before (exactly),
    /// so they keep their names and their twins' `nth`.
    pub original_at: HashMap<usize, glam::Vec3>,
    /// The added placements: the mod folder and the add's id.
    pub added: HashMap<usize, (String, String)>,
    /// The first added placement (all after it are added).
    pub first_added: Option<usize>,
    pub moved: usize,
    pub deleted: usize,
    /// Edits naming a placement or solid the world does not have.
    pub missing: usize,
    /// The originals indexed for naming them, and what the editor's live
    /// re-apply keeps (`World::reapply_map_mod`).
    pub index: crate::world::MapIndex,
}

impl Applied {
    /// The placement the add `id` of the mod in `folder` became.
    pub fn placement_of_add(&self, folder: &str, id: &str) -> Option<usize> {
        self.added.iter().find(|(_, (f, a))| f == folder && a == id).map(|(i, _)| *i)
    }
}

static EDITOR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
type Mods = Arc<Vec<(String, MapMod)>>;
/// `--map-mod <folder>` or `--map-file <path>` (a test run started by the
/// desktop editor): this one mod for the whole run instead of
/// `enabled.txt`'s list.
static ONLY: RwLock<Option<Only>> = RwLock::new(None);
/// The enabled mods read, by the maps folder they were read from (a test
/// with its own data folder has its own).
static ACTIVE: RwLock<Option<HashMap<std::path::PathBuf, Mods>>> = RwLock::new(None);

/// The editor is reachable (the game was started with `--editor`).
pub fn editor_enabled() -> bool {
    EDITOR.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_editor_enabled(on: bool) {
    EDITOR.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The one mod a run applies ([`set_only`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Only {
    /// The mod in `<maps>/<folder>/map.obmap` (`--map-mod`).
    Folder(String),
    /// A mod file anywhere (`--map-file`, the editor's run copy of unsaved
    /// changes), known in the run as the mod `folder` (its races are found
    /// under that name).
    File { folder: String, path: std::path::PathBuf },
}

impl Only {
    /// The name the run knows the mod by.
    pub fn folder(&self) -> &str {
        match self {
            Only::Folder(f) | Only::File { folder: f, .. } => f,
        }
    }

    /// The file it is read from (`dir`: the maps folder).
    pub fn path(&self, dir: &std::path::Path) -> std::path::PathBuf {
        match self {
            Only::Folder(f) => dir.join(f).join(store::FILE),
            Only::File { path, .. } => path.clone(),
        }
    }
}

/// Apply only this mod for the rest of the run, whatever `enabled.txt`
/// lists (None: the list again). Nothing is written.
pub fn set_only(only: Option<Only>) {
    *ONLY.write().unwrap() = only;
    ACTIVE.write().unwrap().take();
}

/// The one mod [`set_only`] chose.
pub fn only() -> Option<Only> {
    ONLY.read().unwrap().clone()
}

/// The mods in use read from `dir`: the [`set_only`] one, else the
/// enabled ones.
fn read_mods(dir: &std::path::Path, only: Option<Only>) -> Vec<(String, MapMod)> {
    match only {
        Some(o) => match store::read_file(&o.path(dir)) {
            Ok(m) => vec![(o.folder().to_string(), m)],
            Err(e) => {
                log::warn!("map mod {} not loaded: {e:#}", o.folder());
                Vec::new()
            }
        },
        None => store::load_enabled(dir),
    }
}

/// The enabled map mods, read from the data folder on first use.
pub fn active() -> Mods {
    let dir = store::maps_dir();
    if let Some(m) = ACTIVE.read().unwrap().as_ref().and_then(|a| a.get(&dir)) {
        return m.clone();
    }
    let mods: Mods = Arc::new(read_mods(&dir, only()));
    if !mods.is_empty() {
        log::info!("map mods enabled: {}", mods.iter().map(|m| m.0.as_str()).collect::<Vec<_>>().join(", "));
    }
    ACTIVE.write().unwrap().get_or_insert_with(HashMap::new).insert(dir, mods.clone());
    mods
}

/// The enabled mods read again from the data folder (the editor's after
/// a save).
pub fn reload_active() -> Mods {
    let dir = store::maps_dir();
    let mods: Mods = Arc::new(read_mods(&dir, only()));
    ACTIVE.write().unwrap().get_or_insert_with(HashMap::new).insert(dir, mods.clone());
    mods
}

/// A world as the game drives it: [`crate::world::World::load`], then the
/// enabled map mods' edits for the region.
pub fn load_world(root: &std::path::Path, region: &str) -> anyhow::Result<crate::world::World> {
    let mut world = crate::world::World::load(root, region)?;
    let mods = active();
    if !mods.is_empty() {
        world.apply_map_mods(region, &mods);
    }
    Ok(world)
}

/// The custom race `id` of the mod in `folder` among the enabled mods.
pub fn find_race(folder: &str, id: &str) -> Option<CustomRace> {
    active().iter().find(|m| m.0 == folder)?.1.races.iter().find(|r| r.id == id).cloned()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod only_tests {
    use super::*;

    #[test]
    fn one_chosen_mod_replaces_the_enabled_list() {
        let dir = std::env::temp_dir().join(format!("ob-map-only-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let m = |name: &str| MapMod { name: name.into(), ..Default::default() };
        store::save(&dir, "on", &m("On")).unwrap();
        store::save(&dir, "off", &m("Off")).unwrap();
        store::set_enabled(&dir, "on", true).unwrap();
        let before = std::fs::read_to_string(dir.join(store::ENABLED)).unwrap();
        let names = |v: Vec<(String, MapMod)>| v.into_iter().map(|(f, _)| f).collect::<Vec<_>>();
        assert_eq!(names(read_mods(&dir, None)), ["on"]);
        assert_eq!(names(read_mods(&dir, Some(Only::Folder("off".into())))), ["off"], "only the chosen one, on or off");
        assert!(read_mods(&dir, Some(Only::Folder("missing".into()))).is_empty());
        // A file anywhere, known by the folder it was given.
        let copy = dir.join("on").join(store::EDITOR_DIR).join(store::RUN_DIR).join(store::FILE);
        store::write_run_copy(&dir, "on", &m("On, unsaved")).unwrap();
        let file = Only::File { folder: "on".into(), path: copy.clone() };
        let got = read_mods(&dir, Some(file));
        assert_eq!((got.len(), got[0].0.as_str(), got[0].1.name.as_str()), (1, "on", "On, unsaved"));
        assert_eq!(store::read_file(&dir.join("on").join(store::FILE)).unwrap().name, "On", "the saved mod is untouched");
        let gone = Only::File { folder: "on".into(), path: dir.join("nowhere.obmap") };
        assert!(read_mods(&dir, Some(gone)).is_empty());
        assert_eq!(std::fs::read_to_string(dir.join(store::ENABLED)).unwrap(), before, "the list is never written");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
