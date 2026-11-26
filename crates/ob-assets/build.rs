//! ob-assets' build id (build_support/build_id.rs: its sources and the ids of the crates below it, handed up to
//! the crates above it) and the test switches (`ob_game`, `ob_private`: build_support/stamp.rs).

#[path = "../../build_support/build_id.rs"]
mod build_id;
#[allow(dead_code)]
#[path = "../../build_support/stamp.rs"]
mod stamp;

fn main() {
    build_id::emit("../../Cargo.lock");
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    stamp::test_cfgs(&here.join("../.."));
}
