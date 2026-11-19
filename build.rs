//! The library's build id, which its asset caches key on
//! (build_support/build_id.rs, `nfsu2::BUILD_ID`): a hash of its sources,
//! so any change to a parser makes the caches stale. Also the version
//! stamp (`OB_VERSION`, `OB_COMMIT`) and, for Windows targets, the icon, version resource and manifest of the two player
//! programs in this package (`game.exe`, `overbright.exe`; the map editor's
//! are made by crates/ob-editor/build.rs).

#[path = "build_support/build_id.rs"]
mod build_id;
#[path = "build_support/icon_art.rs"]
mod icon_art;
#[path = "build_support/stamp.rs"]
mod stamp;
#[path = "build_support/windows_res.rs"]
mod windows_res;

fn main() {
    build_id::emit("Cargo.lock");
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    stamp::test_cfgs(&here);
    let (version, commit) = stamp::version_stamp();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        windows_res::build(&out, &version, &commit, &["game", "overbright"]);
    }
}
