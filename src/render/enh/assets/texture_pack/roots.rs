use super::*;

/// A scoped variable's folders: one path, or several separated like PATH
/// (`:` on Unix; docs/texmod-import.md), the first that has a file
/// wins. Missing folders are skipped with a warning.
pub(super) fn explicit_roots(variable: &str, value: Option<std::ffi::OsString>) -> Vec<PathBuf> {
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
/// first; then the Overbright packs (`obpack`: the player's order, each
/// compiled into the cache); then the texture mods the launcher keeps (`texture_mods`: the
/// packages that are on, in the launcher's order); then the World Texture
/// Pack row's folders (read at each load:
/// the row applies when the world or a car loads next), the imported
/// TexMod pack before the generated one (`upscale_pack::selected_all`).
/// Scoped packs never activate HD UI or load map calibration. Without any,
/// retain the existing shared-pack behavior for backwards compatibility.
pub(super) fn roots(domain: Domain) -> Vec<PathBuf> {
    static WORLD: OnceLock<Vec<PathBuf>> = OnceLock::new();
    static CAR: OnceLock<Vec<PathBuf>> = OnceLock::new();
    let mut v = match domain {
        Domain::Ui => Vec::new(),
        Domain::World => WORLD
            .get_or_init(|| {
                explicit_roots("NFSU2_WORLD_TEXTURE_PACK", crate::render::debug_env::nfsu2_world_texture_pack())
            })
            .clone(),
        Domain::Car => CAR
            .get_or_init(|| {
                explicit_roots("NFSU2_CAR_TEXTURE_PACK", crate::render::debug_env::nfsu2_car_texture_pack())
            })
            .clone(),
    };
    if domain != Domain::Ui {
        // Overbright packs (`obpack`, docs/texture-packs.md), compiled, in
        // the player's order (none installed: nothing added, the native path).
        let d = if domain == Domain::Car { crate::obpack::Domain::Car } else { crate::obpack::Domain::World };
        for r in crate::obpack::enabled_roots(d) {
            if !v.contains(&r) {
                v.push(r);
            }
        }
        // The texture mods kept by the launcher, in its order (none kept:
        // nothing added, the native path).
        for d in crate::texture_mods::enabled_roots(domain == Domain::Car) {
            if !v.contains(&d) {
                v.push(d);
            }
        }
        for d in crate::upscale_pack::selected_all(domain == Domain::Car) {
            if !v.contains(&d) {
                v.push(d);
            }
        }
    }
    if v.is_empty() {
        v.extend(crate::hd_ui_pack::active_root().map(Path::to_path_buf));
    }
    v
}

pub(in crate::render) fn enabled(domain: Domain) -> bool {
    !roots(domain).is_empty()
}

/// The folders a domain's replacements come from now (the hot-reload watcher).
pub(in crate::render) fn roots_of(domain: Domain) -> Vec<PathBuf> {
    roots(domain)
}

/// docs/loading-image.md: the UI's second folder, the generated
/// loading-screen textures (`upscale_pack::ui_dir`), searched after the HD
/// UI pack and only while HD UI Art is in force (off: native).
pub(super) fn generated_ui(domain: Domain) -> Option<PathBuf> {
    if domain != Domain::Ui || !crate::hd_ui_pack::active() {
        return None;
    }
    let d = crate::upscale_pack::ui_dir();
    d.is_dir().then_some(d)
}
