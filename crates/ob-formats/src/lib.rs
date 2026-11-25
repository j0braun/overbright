//! Overbright's file-format parsers and the engine's tables
//! (ARCHITECTURE.md "Layers"): pure readers of the original's files (bundle
//! chunks, hashes, solids, scenery, textures, tracks, compressed packs, VIV
//! archives, VP6 video, routes, saves, texture-mod packages) and the numbers,
//! curves and names the original keeps in its executable
//! (docs/original/exe-tables.md). On ob-core only; the library (`nfsu2`) re-exports
//! both modules at their old paths.

// The bottom layer's modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{cache, draw, game_fs, game_rng, persist, persist_enum, prof};
#[cfg(test)]
use ob_core::{fingerprint, game_root};

// The test kit (crates/ob-testkit, docs/developing/testing.md) for this crate's unit
// tests, bound to this crate under the name the kit expects.
#[cfg(test)]
extern crate self as nfsu2;
#[cfg(test)]
mod testkit {
    ob_testkit::bind!();
}
// What the kit reads through `nfsu2::` besides this crate's own modules.
#[cfg(test)]
use ob_core::platform;

/// This crate's build id (build_support/build_id.rs).
pub const BUILD_ID: ob_core::cache::BuildId = ob_core::cache::BuildId::from_hex(env!("OB_BUILD_ID"));

pub mod formats {
    pub mod ai_routes;
    pub mod bundle;
    pub mod camera_tracks;
    pub mod collision;
    pub mod comp;
    pub mod eagl;
    pub mod hash;
    pub mod huff;
    pub mod jdlz;
    pub mod lights;
    pub mod magazines;
    pub mod native_save;
    pub mod parameter_map;
    pub mod parts;
    pub mod reader;
    pub mod roads;
    pub mod scenery;
    pub mod smackable;
    pub mod solid;
    pub mod texture;
    pub mod texwizard;
    pub mod tpf;
    pub mod texture_anim;
    pub mod track;
    pub mod visibility;
    pub mod viv;
    pub mod vp6;
    pub mod world_anim;
    pub mod zam;
    pub mod zip;
}

// The engine's tables, in code (docs/original/exe-tables.md).
pub mod exe_tables;
