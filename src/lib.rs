//! A readable reimplementation of Need for Speed: Underground 2 (2004),
//! running on the original game's data files.
//!
//! * [`formats`] — pure parsers for the game's file formats.
//! * [`world`] — a track assembled from those parsers.
//! * [`render`] — a wgpu renderer for the world.
//! * [`frontend`] — the menus, from the game's FEng packages, and the career.

// The test kit (crates/ob-testkit, docs/testing.md), bound to this crate by
// the file the integration tests and the game binary's tests use too; it
// names this crate `nfsu2` as they do.
#[cfg(test)]
extern crate self as nfsu2;
#[cfg(test)]
#[path = "../tests/common/mod.rs"]
pub(crate) mod testkit;

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

pub mod formats {
    pub mod ai_routes;
    pub mod bundle;
    pub mod camera_tracks;
    pub mod collision;
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
pub mod ai;
pub mod ai_looks;
pub mod carfx_pops;
pub mod track_camera;
pub mod audio;
pub mod cache;
pub mod car;
pub mod character;
pub mod chase;
pub mod collision;
pub mod contact;
pub mod drift;
pub mod style;
pub mod drive;
pub mod event_camera;
pub mod diag;
pub mod difficulty;
pub mod dyno;
pub mod fog;
pub mod frame_clock;
pub mod game_clock;
pub mod latency;
pub mod extras;
pub mod failed_load;
pub mod replay_cams;
pub mod racer_reset;
pub mod interpolate;
#[cfg(feature = "mimalloc")]
pub mod allocator;
pub mod frontend;
pub mod fxhash;
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
pub mod platform;
pub mod prof;
pub mod qol;
pub mod race;
pub mod rear_view;
pub mod region;
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
pub mod draw;
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
// The release build: version stamp, the Windows console and start errors.
pub mod build_info;
pub mod console;
// The opt-in modding tools (texture and map editors; docs/texture-editor.md).
pub mod tools;
// Overbright's own texture-pack format (docs/texture-packs.md).
pub mod obpack;
// Quality tooling (docs/game-fingerprint.md, docs/mark-spot.md).
pub mod fingerprint;
// The engine's tables, in code (docs/exe-tables.md).
pub mod exe_tables;
pub mod game_fs;
/// The launcher (`overbright`, docs/launcher.md).
pub mod launcher;
/// The game↔launcher contract (probe and prepare formats, the error
/// kinds, the shared look's tokens): what the game names of the launcher.
pub mod launch;
pub mod mark_spot;

use std::path::PathBuf;

/// Whether the environment variable `$name` is set, read once per call
/// site (debugging switches tested on frame paths cost a `getenv`, a
/// lock and a scan of the environment, every frame). The variables are
/// set before the game starts.
#[macro_export]
macro_rules! env_set {
    ($name:literal) => {{
        static SET: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *SET.get_or_init(|| std::env::var_os($name).is_some())
    }};
}

/// `$name`'s value (as `std::env::var(..).ok()`), read once per call site
/// (see [`env_set!`]): an `Option<&'static str>`.
#[macro_export]
macro_rules! env_value {
    ($name:literal) => {{
        static VALUE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        VALUE.get_or_init(|| std::env::var($name).ok()).as_deref()
    }};
}

/// `$name`'s value (as `std::env::var_os`), read once per call site (see
/// [`env_set!`]): an `Option<&'static OsStr>`.
#[macro_export]
macro_rules! env_value_os {
    ($name:literal) => {{
        static VALUE: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
        VALUE.get_or_init(|| std::env::var_os($name)).as_deref()
    }};
}

/// `$name` parsed as a `$t`, read once per call site (see [`env_set!`]).
#[macro_export]
macro_rules! env_parsed {
    ($name:literal, $t:ty) => {{
        static VALUE: std::sync::OnceLock<Option<$t>> = std::sync::OnceLock::new();
        *VALUE.get_or_init(|| std::env::var($name).ok().and_then(|v| v.parse::<$t>().ok()))
    }};
}

/// Where the installed game lives: `$NFSU2_ROOT`, else the current directory.
pub fn game_root() -> PathBuf {
    std::env::var_os("NFSU2_ROOT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// A tag for temporary file and folder names that stays unique between
/// processes even when they share a PID (each `bwrap --unshare-pid` run is
/// PID 2): the PID, the start time in nanoseconds and a per-process count.
pub fn unique_tag() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::OnceLock;
    static START: OnceLock<u128> = OnceLock::new();
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let start = *START.get_or_init(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    format!("{}-{start:x}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed))
}

pub mod hd_ui_pack;

// Performance overlay (docs/perf-overlay.md).
pub mod overlay;
