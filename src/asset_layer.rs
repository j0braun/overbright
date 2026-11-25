//! The asset layer (docs/modding/asset-format/import.md §13 "Reading through the asset layer"): when the player
//! has imported their copy of the game (`overbright-import`, a finished base set under `<app>/base`), the game's
//! file reads come from that base set instead of the game folder, byte for byte the same files. Without an
//! import, or when the set does not verify, nothing is mounted and every read is today's, from the game folder.
//!
//! `NFSU2_BASE` chooses: unset or `active` = the set `<app>/base/active.txt` names (else the only set there);
//! `off` (or `0`) = the game folder always (the old path); a set id under `<app>/base` or a set's folder = that set.
//! A set that is damaged, incomplete or made by a newer Overbright is not mounted: the log says why and the game
//! reads the game folder.

use std::path::Path;

/// The variable that chooses the base set or turns the layer off.
pub const VAR: &str = "NFSU2_BASE";

/// What `NFSU2_BASE` asks for: None = off.
pub fn spec_from(var: Option<&str>) -> Option<String> {
    match var.map(str::trim).filter(|v| !v.is_empty()) {
        None => Some("active".to_string()),
        Some("off" | "0") => None,
        Some(v) => Some(v.to_string()),
    }
}

/// Mounts the base set `NFSU2_BASE` names (by default the active one) over the game folder `root`; returns its
/// id when mounted.
pub fn mount_from_env(root: &Path) -> Option<String> {
    let var = std::env::var(VAR).ok();
    let Some(spec) = spec_from(var.as_deref()) else {
        log::info!("asset layer: off ({VAR}=off); reading the game folder");
        return None;
    };
    let base = crate::platform::dirs::app().join("base");
    // No import made yet (the default case before the player imports): the game folder, without a warning.
    if var.is_none() && !has_set(&base) {
        return None;
    }
    match mount(&spec, &base, root) {
        Ok(id) => Some(id),
        Err(e) => {
            log::warn!("asset layer: base set {spec:?}: {e}; reading the game folder");
            None
        }
    }
}

/// Whether `base` holds any base set (a folder with `import.toml`).
fn has_set(base: &Path) -> bool {
    std::fs::read_dir(base).into_iter().flatten().flatten().any(|e| e.path().join("import.toml").is_file())
}

/// Resolves, verifies and mounts the base set `spec` over `root`.
pub fn mount(spec: &str, base: &Path, root: &Path) -> Result<String, ob_asset_format::Refusal> {
    let set = ob_asset_format::mount::resolve(spec, base)?;
    let t = std::time::Instant::now();
    let stamps = crate::cache::dir();
    let s = ob_asset_format::mount::BaseSet::open(&set, stamps.as_deref())?;
    let id = s.id().to_string();
    log::info!(
        "asset layer: base set {id} verified in {:.0} ms ({} files, {} MB hashed)",
        t.elapsed().as_secs_f64() * 1e3,
        s.len(),
        s.hashed >> 20
    );
    crate::game_fs::mount(root, std::sync::Arc::new(s));
    Ok(id)
}

/// The exit line with the layer on: what came from the set and what from the folder.
pub fn report() {
    if let Some(id) = crate::game_fs::mounted() {
        let (files, bytes, folder) = crate::game_fs::counts();
        let absent = crate::game_fs::absent_count();
        let line = format!(
            "asset layer: {files} reads ({} MB) from base set {id}, {absent} not in the install, {folder} game files from the folder",
            bytes >> 20
        );
        log::info!("{line}");
        eprintln!("{line}");
        for (k, n) in crate::game_fs::misses() {
            eprintln!("asset layer: not in base set: {k} ({n}x)");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn on_by_default_off_on_request() {
        assert_eq!(super::spec_from(None).as_deref(), Some("active"));
        assert_eq!(super::spec_from(Some("")).as_deref(), Some("active"));
        assert_eq!(super::spec_from(Some("off")), None);
        assert_eq!(super::spec_from(Some("0")), None);
        assert_eq!(super::spec_from(Some("nfsu2-pc12-eu")).as_deref(), Some("nfsu2-pc12-eu"));
    }
}
