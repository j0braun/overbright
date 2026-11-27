//! Overbright's asset layer (ARCHITECTURE.md "Layers"): what the game reads from the player's files and the packs
//! and mods laid over them: the world and the cars (with the records a car installs: handling, performance,
//! tuning, part animation, the narrow phase's body points, nitrous, race tunes), the game's data and the front
//! end's parsers, the texture packs and mods, the map tools' world side, the particle and effect tables. On
//! ob-core, ob-formats, ob-map and ob-settings; the library (`nfsu2`) re-exports every module at its old path.

// The lower layers' modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{
    cache, draw, failed_load, fingerprint, fxhash, game_fs, game_rng, platform, prof, race_mode, region,
};
pub(crate) use ob_core::{game_root, persist};
pub(crate) use ob_formats::{exe_tables, formats};

/// This crate's build id (build_support/build_id.rs): the asset caches its code makes are named and checked by it.
pub const BUILD_ID: ob_core::cache::BuildId = ob_core::cache::BuildId::from_hex(env!("OB_BUILD_ID"));

// The test kit (crates/ob-testkit, docs/developing/testing.md) for this crate's unit tests, bound to this crate
// under the name the kit expects.
#[cfg(test)]
extern crate self as nfsu2;
#[cfg(test)]
mod testkit {
    ob_testkit::bind!();
}

pub mod qol {
    pub use ob_settings::qol::*;
    pub mod setup;

    /// Test isolation for this crate's unit tests (the settings store is process-wide and libtest runs the tests
    /// side by side; the library's own `qol::test_globals` also resets the renderer's photo mode): one test that
    /// puts values in force or asserts the defaults at a time, the native defaults back when it ends (also on a
    /// panic).
    #[cfg(test)]
    pub(crate) fn test_globals() -> TestGlobals {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        TestGlobals(LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    #[cfg(test)]
    pub(crate) struct TestGlobals(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

    #[cfg(test)]
    impl Drop for TestGlobals {
        fn drop(&mut self) {
            Enhancements::default().apply();
        }
    }
}
pub mod world;
pub mod car;
pub mod part_anim;
pub mod narrow;
pub mod nitrous;
pub mod performance;
pub mod race;
pub mod tuning;
pub mod handling;
pub mod character;
pub mod collision;
pub mod ground;
pub mod mod_packs;
pub mod movie;
pub mod particles;
pub mod game_assets;
pub mod model_fixes;
pub mod car_smooth;
pub mod neon_quality;
pub mod glow;
pub mod fog;
pub mod shadow;
pub mod horizon_fog;
pub mod motion_blur;
pub mod hd_ui_pack;
pub mod texture_mods;
pub mod tpf_import;
pub mod texwizard_import;
pub mod hud_mod;
pub mod upscale;
pub mod upscale_pack;
pub mod obpack;
pub mod tools {
    pub mod map;
}
pub mod render {
    pub mod enh {
        pub mod geometry {
            pub mod shadow_refine;
        }
        pub mod assets {
            pub mod car_model;
            pub mod neon_quality;
            pub mod hd_ui;
            pub mod pack_select;
            pub mod texture_pack;
            pub mod surface;
        }
    }
    pub mod native {
        pub mod fx;
    }
}
pub mod surface_maps;
pub mod frontend {
    pub mod assets;
    pub mod feng {
        pub mod anim;
        pub mod font;
        pub mod model;
        pub mod parse;
        pub mod state;
        pub mod strings;
    }
    pub mod career {
        // The tracks and their routes are the map data's (crates/ob-map), at their old paths.
        pub use ob_map::frontend::career::*;
        pub mod events;
        pub mod triggers;
    }
}
