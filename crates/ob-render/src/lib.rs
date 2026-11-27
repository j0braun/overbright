//! Overbright's render layer (ARCHITECTURE.md "Layers"): the GPU renderer with its native and enhanced passes and
//! WGSL shaders, the sky, the rear-view mirror, the GPU timing scopes and the monitor facts. On ob-core, ob-formats,
//! ob-settings, ob-assets and ob-sim; the library (`nfsu2`) re-exports every module at its old path.

// The lower layers' modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{cache, diag, draw, env_parsed, env_set, env_value, frame_clock, fxhash, game_fs, persist};
pub(crate) use ob_formats::{exe_tables, formats};
pub(crate) use ob_assets::{car, character, fog, glow, horizon_fog, hd_ui_pack, motion_blur, neon_quality, part_anim, particles, shadow, surface_maps, upscale, upscale_pack, world};
pub(crate) use ob_assets::frontend;
#[cfg(test)]
pub(crate) use ob_assets::{mod_packs, texture_mods, tools, tpf_import};
#[cfg(test)]
pub(crate) use ob_sim::ai;
pub(crate) use ob_sim::{photo, racer_reset, screen_tint, weather};
#[cfg(test)]
pub(crate) use ob_core::{fingerprint, game_root};

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

pub mod prof {
    pub use ob_core::prof::*;
    pub mod gpu;
}

pub mod platform {
    pub use ob_core::platform::*;
    pub mod monitor;
}

pub(crate) mod qol {
    pub use ob_assets::qol::*;
    /// Test isolation for this crate's unit tests (the settings store is process-wide and libtest runs the tests
    /// side by side): one test that puts values in force or asserts the defaults at a time, the native defaults
    /// and the renderer's photo mode back when it ends (also on a panic).
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
            crate::render::blur_dof::set_photo_mode(false);
        }
    }
}

pub mod rear_view;
pub mod sky;
pub mod render;
