//! ob-map's build id (build_support/build_id.rs: its sources and the ids of
//! ob-core and ob-formats, handed up to the crates above it).

#[path = "../../build_support/build_id.rs"]
mod build_id;

fn main() {
    build_id::emit("../../Cargo.lock");
}
