//! Overbright's simulation layer (ARCHITECTURE.md "Layers"): what advances the world each tick: the driving
//! physics, the AI and traffic, contacts and smackable props, the weather, the opponents' grids, the camera and
//! shop scenes, the effect controller and the audio cues' data the simulation posts. On ob-core, ob-formats,
//! ob-map, ob-settings and ob-assets; the library (`nfsu2`) re-exports every module at its old path.

// The lower layers' modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{draw, frame_clock, fxhash, game_fs, game_rng, platform, region};
pub(crate) use ob_core::env_value_os;
// What the test kit (`ob_testkit::bind!`) names at the crate root.
#[cfg(test)]
pub(crate) use ob_core::{cache, failed_load, fingerprint, prof, race_mode};
#[cfg(test)]
pub(crate) use ob_core::{game_root, persist};
pub(crate) use ob_formats::{exe_tables, formats};
pub(crate) use ob_settings::extras;
pub(crate) use ob_assets::{car, glow, ground, handling, narrow, nitrous, particles, performance, race, shadow, world};
pub(crate) use ob_assets::frontend;

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

pub(crate) mod qol {
    pub use ob_assets::qol::*;
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
pub mod ai;
pub mod ai_looks;
pub mod carfx_pops;
pub mod track_camera;
pub mod chase;
pub mod contact;
pub mod drift;
pub mod style;
pub mod drive;
pub mod event_camera;
pub mod difficulty;
pub mod dyno;
pub mod latency;
pub mod replay_cams;
pub mod racer_reset;
pub mod interpolate;
pub mod garage;
pub mod opponents;
pub mod outrun;
pub mod pad;
pub mod photo;
pub mod smackable;
pub mod standings;
pub mod hydraulics;
pub mod weather;
pub mod mark_spot;
pub mod rng_sites;
pub mod shop_scene;
pub mod melo;
pub mod screen_flash;
pub mod screen_tint;
pub mod audio {
    pub mod car {
        pub mod pushes;
        pub mod scripted;
    }
    pub mod world {
        pub mod impacts;
    }
}
pub mod render {
    pub mod native {
        pub mod fx {
            pub use ob_assets::render::native::fx::*;
            pub mod effect_controller;
        }
    }
}
