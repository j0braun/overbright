//! Map mods and the map editor's logic (docs/map-editor.md).
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

pub mod edit;
pub mod format;
pub mod race;
pub mod store;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub use format::*;

/// What the map mods did to one world (`World::map_overlay`).
#[derive(Clone, Debug, Default)]
pub struct Applied {
    /// The original placements a mod moved or hid, with their names as
    /// the original data has them.
    pub originals: HashMap<usize, PropRef>,
    /// The added placements: the mod folder and the index of its edit.
    pub added: HashMap<usize, (String, usize)>,
    /// The first added placement (all after it are added).
    pub first_added: Option<usize>,
    pub moved: usize,
    pub deleted: usize,
    /// Edits naming a placement or solid the world does not have.
    pub missing: usize,
}

static EDITOR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
type Mods = Arc<Vec<(String, MapMod)>>;
/// `--map-mod <folder>` (a test run started by the desktop editor): this
/// one mod for the whole run instead of `enabled.txt`'s list.
static ONLY: RwLock<Option<String>> = RwLock::new(None);
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

/// Apply only the mod in `folder` for the rest of the run, whatever
/// `enabled.txt` lists (None: the list again). Nothing is written.
pub fn set_only(folder: Option<&str>) {
    *ONLY.write().unwrap() = folder.map(str::to_string);
    ACTIVE.write().unwrap().take();
}

/// The one mod [`set_only`] chose.
pub fn only() -> Option<String> {
    ONLY.read().unwrap().clone()
}

/// The mods in use read from `dir`: the [`set_only`] one, else the
/// enabled ones.
fn read_mods(dir: &std::path::Path, only: Option<String>) -> Vec<(String, MapMod)> {
    match only {
        Some(f) => match store::read_file(&dir.join(&f).join(store::FILE)) {
            Ok(m) => vec![(f, m)],
            Err(e) => {
                log::warn!("map mod {f} not loaded: {e:#}");
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
        assert_eq!(names(read_mods(&dir, Some("off".into()))), ["off"], "only the chosen one, on or off");
        assert!(read_mods(&dir, Some("missing".into())).is_empty());
        assert_eq!(std::fs::read_to_string(dir.join(store::ENABLED)).unwrap(), before, "the list is never written");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
