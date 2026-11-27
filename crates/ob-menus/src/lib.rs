//! Overbright's menus layer (ARCHITECTURE.md "Layers"): the front end built from the game's FEng packages (screens,
//! widgets, the HUD, the career, the options and their tables) and the performance overlay. On every layer below it
//! (ob-core, ob-formats, ob-map, ob-settings, ob-assets, ob-sim, ob-audio, ob-render); the library (`nfsu2`)
//! re-exports both modules at their old paths.

// The lower layers' modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{draw, fxhash, game_clock, game_fs, game_root, input, region};
#[cfg(test)]
pub(crate) use ob_core::{fingerprint, frame_clock, unique_tag};
pub(crate) use ob_core::platform;

/// Timings (`ob_core::prof`) and the GPU's (crates/ob-render).
pub(crate) mod prof {
    pub use ob_core::prof::*;
    pub use ob_render::prof::gpu;
}
pub(crate) use ob_formats::{exe_tables, formats};
pub(crate) use ob_settings::extras;
pub(crate) use ob_assets::{car, game_assets, handling, hd_ui_pack, mod_packs, performance, qol, race, tuning, upscale_pack};
pub(crate) use ob_sim::{ai, drift, dyno, latency, mark_spot, opponents, outrun, pad, standings};
pub(crate) use ob_audio::audio;
pub(crate) use ob_render::render;

/// This crate's build id (build_support/build_id.rs): the caches its code makes are named and checked by it.
pub const BUILD_ID: ob_core::cache::BuildId = ob_core::cache::BuildId::from_hex(env!("OB_BUILD_ID"));

// The test kit (crates/ob-testkit, docs/developing/testing.md) for this crate's unit tests, bound to this crate
// under the name the kit expects.
#[cfg(test)]
extern crate self as nfsu2;
#[cfg(test)]
mod testkit {
    ob_testkit::bind!();
}

pub mod frontend;
pub mod overlay;
