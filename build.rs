//! The build id the asset cache keys on (`src/cache/`): a hash of every
//! library source file, so any change to a parser makes the caches stale.
//! Also the version stamp (`OB_VERSION`, `OB_COMMIT`) and, for Windows
//! targets, the icon, version resource and manifest of the player's two
//! programs (`game.exe`, `overbright.exe`).

use std::hash::{Hash, Hasher};

#[path = "build_support/icon_art.rs"]
mod icon_art;
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
    test_cfgs();
    let (version, commit) = version_stamp();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        windows_res::build(&out, &version, &commit);
    }
}

/// The version stamp (`nfsu2::build_info`): `OB_VERSION` and `OB_COMMIT`
/// as the release build sets them (scripts/release/container/build.sh: the
/// `git describe` and the commit of the tree it builds; a `git archive`
/// tree has no .git to ask). Unset or empty: the package version and
/// "unknown". Only these two variables are watched, so an ordinary build
/// does not rerun for them.
fn version_stamp() -> (String, String) {
    println!("cargo:rerun-if-env-changed=OB_VERSION");
    println!("cargo:rerun-if-env-changed=OB_COMMIT");
    let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string());
    let version = get("OB_VERSION").unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
    let commit = get("OB_COMMIT").unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=OB_VERSION={version}");
    println!("cargo:rustc-env=OB_COMMIT={commit}");
    // Where the source code is published (scripts/release/package.sh's
    // SOURCE_URL, set by the release maker; docs/building-releases.md); empty in an
    // ordinary build. For the about screen (`env!("OB_SOURCE_URL")`).
    println!("cargo:rerun-if-env-changed=OB_SOURCE_URL");
    println!("cargo:rustc-env=OB_SOURCE_URL={}", get("OB_SOURCE_URL").unwrap_or_default());
    (version, commit)
}

/// The test kit's switches (tests/common/mod.rs, docs/testing.md): tests
/// that need the player's game are marked
/// `#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT ...")]` and those
/// that need the private set `cfg_attr(not(ob_private), ...)`, so a run
/// without them lists them as IGNORED with the reason instead of passing.
/// Only `#[test]` items depend on these: the shipped binaries are the same
/// either way.
fn test_cfgs() {
    println!("cargo::rustc-check-cfg=cfg(ob_game)");
    println!("cargo::rustc-check-cfg=cfg(ob_private)");
    println!("cargo:rerun-if-env-changed=NFSU2_ROOT");
    println!("cargo:rerun-if-env-changed=OB_PRIVATE");
    if std::env::var_os("NFSU2_ROOT").is_some_and(|r| !r.is_empty()) {
        println!("cargo:rustc-cfg=ob_game");
    }
    // The private set: $OB_PRIVATE, else (until it moves out) this
    // repository while it still holds the captures and saves (the public
    // repository has none: its private tests are ignored with the reason).
    let private = std::env::var_os("OB_PRIVATE").filter(|d| !d.is_empty()).map(std::path::PathBuf::from);
    let private = private.unwrap_or_else(|| std::env::var_os("CARGO_MANIFEST_DIR").map(Into::into).unwrap_or_default());
    if private.is_dir() && (std::env::var_os("OB_PRIVATE").is_some_and(|d| !d.is_empty()) || private.join("tests/fixtures/native_saves").is_dir()) {
        println!("cargo:rustc-cfg=ob_private");
    }
}
