//! The per-crate build id (build.rs of each library crate; docs/asset-cache.md
//! "Build id"): a hash of the crate's own sources (its `src/` without the
//! programs in `src/bin/`), the workspace's Cargo.lock, the crate's enabled
//! features and the build ids of the workspace crates it depends on. A cache
//! file is named and checked by the build id of the crate whose code made it,
//! so a change in that code or below it makes the file stale, and a change in
//! a crate above it does not.
//!
//! The id goes to the crate as `OB_BUILD_ID` (`env!`, hex) and, for a crate
//! with a `links` key, to the crates that depend on it as
//! `DEP_<LINKS>_BUILD_ID`, which [`emit`] mixes in.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
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

/// The build id of the sources under `src` (`src/bin` left out), the lock
/// file `lock`, the enabled `features` and the dependencies' `deps` ids
/// (`(name, id)`; any order).
pub fn compute(src: &Path, lock: &Path, features: &[String], deps: &[(String, String)]) -> u64 {
    let mut files = Vec::new();
    walk(src, &mut files);
    files.retain(|p| !p.starts_with(src.join("bin")));
    files.sort();
    // SipHash with fixed keys: the same sources give the same id.
    let mut h = std::hash::DefaultHasher::new();
    for f in &files {
        f.strip_prefix(src).unwrap_or(f).to_string_lossy().hash(&mut h);
        std::fs::read(f).unwrap_or_default().hash(&mut h);
    }
    std::fs::read(lock).unwrap_or_default().hash(&mut h);
    let mut features = features.to_vec();
    features.sort();
    features.hash(&mut h);
    let mut deps = deps.to_vec();
    deps.sort();
    deps.hash(&mut h);
    h.finish()
}

/// In a build script: the crate's id from its `src/`, the workspace's
/// Cargo.lock at `lock` (relative to the crate), its `CARGO_FEATURE_*`
/// switches and every `DEP_OB_*_BUILD_ID` it was given, printed as
/// `OB_BUILD_ID` for the crate and as `build_id` metadata for the crates
/// above it (a `links` key passes it on). Returns the id.
#[allow(dead_code)]
pub fn emit(lock: &str) -> u64 {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed={lock}");
    let features: Vec<String> = std::env::vars().map(|(k, _)| k).filter(|k| k.starts_with("CARGO_FEATURE_")).collect();
    let deps: Vec<(String, String)> =
        std::env::vars().filter(|(k, _)| k.starts_with("DEP_OB_") && k.ends_with("_BUILD_ID")).collect();
    let id = compute(Path::new("src"), Path::new(lock), &features, &deps);
    println!("cargo:rustc-env=OB_BUILD_ID={id:016x}");
    if std::env::var_os("CARGO_MANIFEST_LINKS").is_some() {
        println!("cargo:build_id={id:016x}");
    }
    id
}

#[cfg(test)]
mod tests {
    use super::compute;

    /// Every input on a cache's path moves the id: a source file's content,
    /// a new source file, the lock file, a feature and a dependency's id;
    /// a program's source (`src/bin`) does not.
    #[test]
    fn every_input_changes_the_build_id() {
        let d = std::env::temp_dir().join(format!("ob-build-id-{}", crate::unique_tag()));
        let src = d.join("src");
        std::fs::create_dir_all(src.join("bin")).unwrap();
        std::fs::create_dir_all(src.join("formats")).unwrap();
        std::fs::write(src.join("lib.rs"), "pub mod formats;").unwrap();
        std::fs::write(src.join("formats/mod.rs"), "pub fn a() {}").unwrap();
        std::fs::write(src.join("bin/tool.rs"), "fn main() {}").unwrap();
        let lock = d.join("Cargo.lock");
        std::fs::write(&lock, "version = 4").unwrap();
        let deps = vec![("DEP_OB_CORE_BUILD_ID".to_string(), "0000000000000001".to_string())];
        let id = |features: &[String], deps: &[(String, String)]| compute(&src, &lock, features, deps);
        let base = id(&[], &deps);
        assert_eq!(base, id(&[], &deps), "the same inputs, the same id");

        std::fs::write(src.join("formats/mod.rs"), "pub fn b() {}").unwrap();
        let edited = id(&[], &deps);
        assert_ne!(edited, base, "a source file's content");
        std::fs::write(src.join("formats/new.rs"), "").unwrap();
        let added = id(&[], &deps);
        assert_ne!(added, edited, "a new source file");
        std::fs::write(&lock, "version = 4\n# changed").unwrap();
        let locked = id(&[], &deps);
        assert_ne!(locked, added, "the lock file");
        assert_ne!(id(&["CARGO_FEATURE_DEV_HARNESS".into()], &deps), locked, "a feature");
        let other = vec![("DEP_OB_CORE_BUILD_ID".to_string(), "0000000000000002".to_string())];
        assert_ne!(id(&[], &other), locked, "a dependency's id");
        std::fs::write(src.join("bin/tool.rs"), "fn main() { println!(); }").unwrap();
        assert_eq!(id(&[], &deps), locked, "a program's source is not on the cache's path");
        let _ = std::fs::remove_dir_all(&d);
    }
}
