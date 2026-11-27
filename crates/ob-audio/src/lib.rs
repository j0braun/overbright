//! Overbright's audio layer (ARCHITECTURE.md "Layers"): the game's sound banks and engine recordings decoded as
//! SPEED2.EXE does, the car's, the world's and the front end's voices, the music and the speech, the mix map and
//! the software mixer. On ob-core, ob-formats, ob-map, ob-settings, ob-assets and ob-sim; the library (`nfsu2`)
//! re-exports the module at its old path.

// The lower layers' modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{env_set, game_clock, game_fs, input};
// What the test kit (`ob_testkit::bind!`) and the unit tests name at the crate root.
#[cfg(test)]
pub(crate) use ob_core::{fingerprint, game_root, platform};
pub(crate) use ob_formats::{exe_tables, formats};
pub(crate) use ob_assets::{handling, world};
pub(crate) use ob_assets::frontend;
pub(crate) use ob_sim::opponents;

/// This crate's build id (build_support/build_id.rs): the caches its code makes are named and checked by it.
pub const BUILD_ID: ob_core::cache::BuildId = ob_core::cache::BuildId::from_hex(env!("OB_BUILD_ID"));

#[cfg(test)]
extern crate self as nfsu2;
#[cfg(test)]
mod testkit {
    ob_testkit::bind!();
}

pub(crate) mod qol {
    pub use ob_assets::qol::*;
}
pub mod audio;
