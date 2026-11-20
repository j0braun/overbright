//! ob-studio's build script: the test kit's switches (`ob_game`, `ob_private`, as the root package's build.rs sets
//! them) for its tests on the player's game files and, for Windows targets, `overbright-studio-cli.exe`'s icon,
//! version resource and manifest (build_support/windows_res.rs, shared with the root package and the editor).

#[path = "../../build_support/icon_art.rs"]
mod icon_art;
#[path = "../../build_support/stamp.rs"]
mod stamp;
#[path = "../../build_support/windows_res.rs"]
mod windows_res;

fn main() {
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let repo = here.ancestors().nth(2).unwrap_or(&here).to_path_buf();
    stamp::test_cfgs(&repo);
    let (version, commit) = stamp::version_stamp();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        windows_res::build(&out, &version, &commit, &["overbright-studio-cli"]);
    }
}
