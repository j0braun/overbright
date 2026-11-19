//! ob-core's build id (build_support/build_id.rs, handed up to the crates
//! above it), the version stamp (`build_info`) and the test switches.

#[path = "../../build_support/build_id.rs"]
mod build_id;
#[path = "../../build_support/stamp.rs"]
mod stamp;

fn main() {
    build_id::emit("../../Cargo.lock");
    let here = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    stamp::test_cfgs(&here.join("../.."));
    stamp::version_stamp();
}
