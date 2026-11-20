//! ob-studio's build script: the test kit's switches (`ob_game`, `ob_private`, as the root package's build.rs sets
//! them) for its tests on the player's game files.

#[path = "../../build_support/stamp.rs"]
#[allow(dead_code)]
mod stamp;

fn main() {
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let repo = here.ancestors().nth(2).unwrap_or(&here).to_path_buf();
    stamp::test_cfgs(&repo);
}
