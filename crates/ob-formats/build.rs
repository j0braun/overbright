//! ob-formats' build id (build_support/build_id.rs: its sources and
//! ob-core's id, handed up to the crates above it) and the test switches.

#[path = "../../build_support/build_id.rs"]
mod build_id;
// The test switches only (the version stamp is the library's and ob-core's).
#[allow(dead_code)]
#[path = "../../build_support/stamp.rs"]
mod stamp;

fn main() {
    build_id::emit("../../Cargo.lock");
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    stamp::test_cfgs(&here.join("../.."));
}
