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

pub mod ai;
pub mod ai_looks;
pub mod carfx_pops;
pub mod track_camera;
pub mod audio;
pub mod car;
pub mod character;
pub mod chase;
pub mod collision;
pub mod contact;
pub mod drift;
pub mod style;
pub mod drive;
pub mod event_camera;
pub mod difficulty;
pub mod dyno;
pub mod fog;
pub mod latency;
pub mod extras;
pub mod replay_cams;
pub mod racer_reset;
pub mod interpolate;
#[cfg(feature = "mimalloc")]
pub mod allocator;
pub mod frontend;
pub mod garage;
pub mod part_anim;
pub mod glow;
pub mod ground;
pub mod narrow;
pub mod nitrous;
pub mod opponents;
pub mod outrun;
pub mod pad;
pub mod particles;
pub mod performance;
pub mod photo;
pub mod qol;
pub mod race;
pub mod rear_view;
pub mod screen_flash;
pub mod screen_tint;
pub mod sky;
pub mod smackable;
pub mod standings;
pub mod tuning;
pub mod handling;
pub mod hydraulics;
pub mod horizon_fog;
pub mod motion_blur;
pub mod melo;
pub mod model_fixes;
pub mod car_smooth;
pub mod movie;
pub mod game_assets;
pub mod render;
pub mod neon_quality;
pub mod surface_maps;
pub mod rng_sites;
pub mod upscale;
pub mod upscale_pack;
pub mod tpf_import;
pub mod texture_mods;
pub mod texwizard_import;
pub mod hud_mod;
pub mod shadow;
pub mod shop_scene;
pub mod weather;
pub mod world;
// The opt-in modding tools (texture and map editors; docs/modding/texture-editor.md).
pub mod tools;
// The one interface Studio's crates use into this library (crates/ob-studio, crates/ob-studio-gui).
pub mod studio_api;
// Overbright's own texture-pack format (docs/modding/texture-packs.md).
pub mod obpack;
// Quality tooling (docs/playing/mark-spot.md).
/// The launcher (`overbright`, docs/launcher.md).
pub mod launcher;
/// The game↔launcher contract (probe and prepare formats, the error
/// kinds, the shared look's tokens): what the game names of the launcher.
pub mod launch;
pub mod mark_spot;

/// The library's build id (build_support/build_id.rs): the asset caches its
/// code makes are named and checked by it.
pub const BUILD_ID: cache::BuildId = cache::BuildId::from_hex(env!("OB_BUILD_ID"));


pub mod hd_ui_pack;

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
