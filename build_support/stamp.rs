//! The build scripts' shared stamps (build.rs, crates/ob-editor/build.rs):
//! the version stamp and the test kit's switches.

/// The version stamp (`nfsu2::build_info`): `OB_VERSION` and `OB_COMMIT`
/// as the release build sets them (scripts/release/container/build.sh: the
/// `git describe` and the commit of the tree it builds; a `git archive`
/// tree has no .git to ask). Unset or empty: the package version and
/// "unknown". Only these two variables are watched, so an ordinary build
/// does not rerun for them.
pub fn version_stamp() -> (String, String) {
    println!("cargo:rerun-if-env-changed=OB_VERSION");
    println!("cargo:rerun-if-env-changed=OB_COMMIT");
    let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string());
    let version = get("OB_VERSION").unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
    let commit = get("OB_COMMIT").unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=OB_VERSION={version}");
    println!("cargo:rustc-env=OB_COMMIT={commit}");
    // Where the source code is published (scripts/release/package.sh's
    // SOURCE_URL, set by the release maker; docs/developing/building-releases.md); empty in an
    // ordinary build. For the about screen (`env!("OB_SOURCE_URL")`).
    println!("cargo:rerun-if-env-changed=OB_SOURCE_URL");
    println!("cargo:rustc-env=OB_SOURCE_URL={}", get("OB_SOURCE_URL").unwrap_or_default());
    (version, commit)
}

/// The test kit's switches (crates/ob-testkit, docs/developing/testing.md): tests
/// that need the player's game are marked
/// `#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT ...")]` and those
/// that need the private set `cfg_attr(not(ob_private), ...)`, so a run
/// without them lists them as IGNORED with the reason instead of passing.
/// Only `#[test]` items depend on these: the shipped binaries are the same
/// either way. `repo` is the repository's top folder (where the private
/// set is when `OB_PRIVATE` is unset).
pub fn test_cfgs(repo: &std::path::Path) {
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
    let private = private.unwrap_or_else(|| repo.to_path_buf());
    if private.is_dir() && (std::env::var_os("OB_PRIVATE").is_some_and(|d| !d.is_empty()) || private.join("tests/fixtures/native_saves").is_dir()) {
        println!("cargo:rustc-cfg=ob_private");
    }
}
