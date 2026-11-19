//! The build id the asset cache keys on (`src/cache/`): a hash of every
//! library source file, so any change to a parser makes the caches stale.
//! Also the version stamp (`OB_VERSION`, `OB_COMMIT`) and, for Windows
//! targets, the icon, version resource and manifest of the two player
//! programs in this package (`game.exe`, `overbright.exe`; the map editor's
//! are made by crates/ob-editor/build.rs).

use std::hash::{Hash, Hasher};

#[path = "build_support/icon_art.rs"]
mod icon_art;
#[path = "build_support/stamp.rs"]
mod stamp;
#[path = "build_support/windows_res.rs"]
mod windows_res;

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let mut files = Vec::new();
    walk(std::path::Path::new("src"), &mut files);
    files.retain(|p| !p.starts_with("src/bin"));
    files.push("Cargo.lock".into());
    files.sort();
    // SipHash with fixed keys: the same sources give the same id.
    let mut h = std::hash::DefaultHasher::new();
    for f in &files {
        f.to_string_lossy().hash(&mut h);
        std::fs::read(f).unwrap_or_default().hash(&mut h);
    }
    // The enabled features too: a feature can change what the library
    // makes, and the build id is the cache's only version (in every cache
    // file's name and header; docs/asset-cache.md).
    let mut features: Vec<String> =
        std::env::vars().map(|(k, _)| k).filter(|k| k.starts_with("CARGO_FEATURE_")).collect();
    features.sort();
    features.hash(&mut h);
    println!("cargo:rustc-env=NFSU2_BUILD_ID={:016x}", h.finish());
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    stamp::test_cfgs(&here);
    let (version, commit) = stamp::version_stamp();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        windows_res::build(&out, &version, &commit, &["game", "overbright"]);
    }
}
