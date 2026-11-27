use super::*;

/// A scoped variable's folders: one path, or several separated like PATH
/// (`:` on Unix; docs/modding/texmod-import.md), the first that has a file
/// wins. Missing folders are skipped with a warning.
pub fn explicit_roots(variable: &str, value: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let Some(value) = value else { return Vec::new() };
    std::env::split_paths(&value)
        .filter(|p| !p.as_os_str().is_empty())
        .filter(|root| {
            let ok = root.is_dir();
            if !ok {
                log::warn!("ignoring {variable}: {} is not a directory", root.display());
            }
            ok
        })
        .collect()
}

/// The folders a domain's replacements come from, first match wins.
/// The scoped variables are read once (restart to change them) and come
/// first; then the enabled map mods' textures (`tools::map::texture_roots`:
/// what a mod painted in Studio Looks, the strongest mod first); then the
/// Overbright packs (`obpack`: the player's order, each
/// compiled into the cache); then the texture mods the launcher keeps (`texture_mods`: the
/// packages that are on, in the launcher's order); then the World Texture
/// Pack row's folders (read at each load:
/// the row applies when the world or a car loads next), the imported
/// TexMod pack before the generated one (`upscale_pack::selected_all`).
/// Scoped packs never activate HD UI or load map calibration. Without any,
/// retain the existing shared-pack behavior for backwards compatibility.
pub fn roots(domain: Domain) -> Vec<PathBuf> {
    static WORLD: OnceLock<Vec<PathBuf>> = OnceLock::new();
    static CAR: OnceLock<Vec<PathBuf>> = OnceLock::new();
    let mut v = match domain {
        Domain::Ui => Vec::new(),
        Domain::World => WORLD
            .get_or_init(|| {
                explicit_roots("NFSU2_WORLD_TEXTURE_PACK", nfsu2_world_texture_pack())
            })
            .clone(),
        Domain::Car => CAR
            .get_or_init(|| {
                explicit_roots("NFSU2_CAR_TEXTURE_PACK", nfsu2_car_texture_pack())
            })
            .clone(),
    };
    if domain != Domain::Ui {
        // Every mod kind's textures, as the layers of the one mod pack stack (`mod_packs::texture_layers`): the
        // packs switched on and the enabled map mods' textures (Studio Looks), the Overbright packs, the texture
        // packages the launcher keeps and the World Texture Pack row's folders, the strongest first (none in
        // use: nothing added, the native path).
        for r in crate::mod_packs::texture_stack_dirs(domain == Domain::Car) {
            if !v.contains(&r) {
                v.push(r);
            }
        }
    }
    if v.is_empty() {
        v.extend(crate::mod_packs::hud_layer().map(|l| l.dir));
    }
    // A Studio test run's live textures (`--looks-live`) before every
    // other pack (none: the folders above, unchanged).
    if let Some(l) = live_root(domain) {
        v.insert(0, l);
    }
    v
}

pub fn enabled(domain: Domain) -> bool {
    !roots(domain).is_empty()
}

/// The folders a domain's replacements come from now (the hot-reload watcher).
pub fn roots_of(domain: Domain) -> Vec<PathBuf> {
    roots(domain)
}

/// docs/original/loading-image.md: the UI's second folder, the generated
/// loading-screen textures (`upscale_pack::ui_dir`), searched after the HD
/// UI pack and only while HD UI Art is in force (off: native).
pub fn generated_ui(domain: Domain) -> Option<PathBuf> {
    if domain != Domain::Ui || !crate::hd_ui_pack::active() {
        return None;
    }
    let d = crate::upscale_pack::ui_dir();
    d.is_dir().then_some(d)
}

// Studio Looks live (docs/modding/studio-looks.md): the test run's live folder, defined here with the pack roots
// that read it (`pack_reload` re-exports it).
pub static LIVE: Mutex<Option<PathBuf>> = Mutex::new(None);
/// Whether [`LIVE`] is set: the native path (no live folder) never takes the lock.
pub static LIVE_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The test run's live folder (None: no live textures, the default).
pub fn set_live(folder: Option<PathBuf>) {
    let changed = {
        let mut l = LIVE.lock().unwrap_or_else(|e| e.into_inner());
        let changed = *l != folder;
        LIVE_ON.store(folder.is_some(), std::sync::atomic::Ordering::Release);
        *l = folder;
        changed
    };
    if changed {
        crate::obpack::forget_roots();
    }
}

/// The live folder, if this run has one.
pub fn live() -> Option<PathBuf> {
    if !LIVE_ON.load(std::sync::atomic::Ordering::Acquire) {
        return None;
    }
    LIVE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The live folder of a domain's textures (`world`, `car`; none for the UI).
pub fn live_root(domain: Domain) -> Option<PathBuf> {
    let sub = match domain {
        Domain::World => "world",
        Domain::Car => "car",
        Domain::Ui => return None,
    };
    live().map(|l| l.join(sub))
}

/// `NFSU2_WORLD_TEXTURE_PACK`: PATH-style list of world pack folders; empty counts as unset.
pub fn nfsu2_world_texture_pack() -> Option<std::ffi::OsString> {
    std::env::var_os("NFSU2_WORLD_TEXTURE_PACK").filter(|p| !p.is_empty())
}
/// `NFSU2_CAR_TEXTURE_PACK`: PATH-style list of car pack folders; empty counts as unset.
pub fn nfsu2_car_texture_pack() -> Option<std::ffi::OsString> {
    std::env::var_os("NFSU2_CAR_TEXTURE_PACK").filter(|p| !p.is_empty())
}
