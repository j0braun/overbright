//! Quality-of-life options: **clone deviations**, every one opt-in, every
//! default native (docs/formats.md, "QoL (clone deviation)"). The settings store
//! and its option table are `ob_settings::qol`, and `setup` (finding and
//! checking the game folder on a first run) is `ob_assets::qol::setup`
//! (re-exported here at the old path); the library's own tests stay here.

pub use ob_assets::qol::*;

#[cfg(test)]
mod tests;

/// Test isolation for the library's unit tests (docs/developing/test-determinism.md;
/// test-only): the settings store is process-wide and libtest runs the
/// tests side by side, so a test that puts values in force
/// (`Enhancements::apply`, photo mode) or asserts the defaults holds this
/// guard: one such test at a time, the native defaults back when it ends
/// (also on a panic).
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
