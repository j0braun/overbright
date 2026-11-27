//! A readable reimplementation of Need for Speed: Underground 2 (2004),
//! running on the original game's data files.
//!
//! * [`formats`] — pure parsers for the game's file formats.
//! * [`world`] — a track assembled from those parsers.
//! * [`render`] — a wgpu renderer for the world.
//! * [`frontend`] — the menus, from the game's FEng packages, and the career.

// The test kit (crates/ob-testkit, docs/developing/testing.md), bound to this crate by
// the file the integration tests and the game binary's tests use too; it
// names this crate `nfsu2` as they do.
#[cfg(test)]
extern crate self as nfsu2;
#[cfg(test)]
#[path = "../tests/common/mod.rs"]
pub(crate) mod testkit;

// The build id's own test (build_support/build_id.rs).
#[cfg(test)]
#[path = "../build_support/build_id.rs"]
mod build_id;

#[cfg(test)]
mod testkit_tests {
    /// Every lock line parses into a 64-digit hash and a size.
    #[test]
    fn the_fixture_lock_parses() {
        for key in ["burnout-records@1", "handling-peugot@1"] {
            let (sha, size) = super::testkit::pinned(key).unwrap_or_else(|| panic!("{key}"));
            assert!(sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()) && size > 0, "{key}");
        }
        assert!(super::testkit::pinned("burnout-records@2").is_none());
        assert!(super::testkit::fixture_cache().ends_with("overbright-test-fixtures"));
    }
}

pub mod asset_layer;
#[cfg(feature = "mimalloc")]
pub mod allocator;
pub mod frontend;
pub mod qol;
pub mod rear_view;
pub mod sky;
pub mod render;
// The opt-in modding tools (texture and map editors; docs/modding/texture-editor.md).
pub mod tools;
// The one interface Studio's crates use into this library (crates/ob-studio, crates/ob-studio-gui).
pub mod studio_api;
// Overbright's own texture-pack format (docs/modding/texture-packs.md).
// Quality tooling (docs/playing/mark-spot.md).
/// The launcher (`overbright`, docs/launcher.md).
pub mod launcher;
/// The game↔launcher contract (probe and prepare formats, the error
/// kinds, the shared look's tokens): what the game names of the launcher.
pub mod launch;

/// The library's build id (build_support/build_id.rs): the asset caches its
/// code makes are named and checked by it.
pub const BUILD_ID: cache::BuildId = cache::BuildId::from_hex(env!("OB_BUILD_ID"));



// Performance overlay (docs/notes/performance/overlay.md).
pub mod overlay;

// The bottom layer, crates/ob-core (ARCHITECTURE.md "Layers"): its modules
// at their old paths. The parts of `cache`, `diag`, `platform` and `prof`
// that need the library (the cache check, the support file, winit, wgpu)
// stay here, beside the re-exported rest.
pub use ob_core::{
    build_info, console, draw, failed_load, fingerprint, frame_clock, fxhash, game_clock, game_fs, game_rng, log_file,
    region,
};
pub use ob_core::{input, race_mode};
// The asset layer, crates/ob-assets (ARCHITECTURE.md "Layers"): its modules at their old paths. The parts that need
// the menus, the renderer or the simulation (`game_assets`' and `world`'s tests, `tools::map::assets`, the pack
// loader's tests, the effect controller, the surface maps' GPU code) stay here, beside the re-exported rest.
pub mod game_assets;
pub mod world;
pub use ob_assets::{
    car, car_smooth, character, collision, fog, glow, ground, handling, hd_ui_pack, horizon_fog, hud_mod, mod_packs,
    model_fixes, motion_blur, movie, narrow, neon_quality, nitrous, obpack, part_anim, particles, performance, race,
    shadow, surface_maps, texture_mods, texwizard_import, tpf_import, tuning, upscale, upscale_pack,
};
// The settings store and the extras' options, crates/ob-settings (`qol::setup` is the asset crate's).
pub use ob_settings::extras;
// The simulation layer, crates/ob-sim: its modules at their old paths (the audio cues' data it posts, `audio::car::
// pushes`, `audio::car::scripted`, `audio::world::impacts`, and the effect controller are re-exported where they were).
pub use ob_sim::{
    ai, ai_looks, carfx_pops, track_camera, chase, contact, drift, style, drive, event_camera, difficulty, dyno, latency, replay_cams, racer_reset, interpolate, garage, opponents, outrun, pad, photo, smackable, standings, hydraulics, weather, mark_spot, rng_sites, shop_scene, melo, screen_flash, screen_tint,
};
// The audio layer, crates/ob-audio: the `audio` module at its old path.
pub use ob_audio::audio;
pub use ob_core::{env_parsed, env_set, env_value, env_value_os, game_root, persist, persist_enum, unique_tag};

// The file-format parsers and the engine's tables (docs/original/exe-tables.md),
// crates/ob-formats.
pub use ob_formats::{exe_tables, formats};

/// The asset cache (`ob_core::cache`) and `--verify-cache`.
pub mod cache {
    pub use ob_core::cache::*;
    mod verify;
    pub use self::verify::*;
}

/// Diagnostics (`ob_core::diag`) and the support file.
pub mod diag {
    pub use ob_core::diag::*;
    pub mod support;
}

/// Operating-system specifics (`ob_core::platform`) and the winit parts:
/// the window icon and the monitors' modes.
pub mod platform {
    pub use ob_core::platform::*;
    pub mod icon;
    pub mod monitor;
}

/// Timings (`ob_core::prof`) and the GPU's.
pub mod prof {
    pub use ob_core::prof::*;
    /// GPU time per part of the frame, from timestamp queries (the bench
    /// turns them on; a no-op otherwise, and on devices without
    /// [`gpu::FEATURES`]). `gpu::mark(encoder, "x")` writes a timestamp
    /// after the commands recorded so far: the time since the previous mark
    /// goes to "x". Each encoder of a frame starts with a mark of its own
    /// ([`gpu::GAP`]), so the GPU's wait between submissions is not charged
    /// to a part.
    ///
    /// The read-back never stalls the frame: each frame's marks go to one of
    /// [`gpu::RING`] query sets, resolved and copied after the frame's last
    /// submission ([`gpu::end_frame`]) and mapped asynchronously; a frame is
    /// read when its map has completed, a few frames later (`device.poll`
    /// without waiting). Only when the ring comes round to a set still being
    /// read does it wait, for that set alone.
    pub mod gpu;
}
