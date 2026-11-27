//! The test switches (`ob_game`, `ob_private`: build_support/stamp.rs). The settings layer makes no asset cache,
//! so it has no build id.

// The test switches only (the version stamp is the library's and ob-core's).
#[allow(dead_code)]
#[path = "../../build_support/stamp.rs"]
mod stamp;

fn main() {
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    stamp::test_cfgs(&here.join("../.."));
}
