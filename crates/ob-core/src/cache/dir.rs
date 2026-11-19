//! Where the cache lives (and the test override).

/// A cache folder set by [`override_dir`] (tests).
pub(super) static DIR_OVERRIDE: std::sync::RwLock<Option<std::path::PathBuf>> = std::sync::RwLock::new(None);

/// Tests: the cache lives in `dir` for this process until the guard is
/// dropped, as `NFSU2_CACHE_DIR` would set it, but without `set_var` (which
/// races every other thread's environment reads). Process-wide: a test that
/// uses it belongs in a test binary of its own (tests/world_cache.rs).
#[doc(hidden)]
pub fn override_dir(dir: std::path::PathBuf) -> DirOverride {
    let mut slot = DIR_OVERRIDE.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    DirOverride(slot.replace(dir))
}

/// Puts the previous cache folder back when dropped ([`override_dir`]).
#[doc(hidden)]
pub struct DirOverride(Option<std::path::PathBuf>);

impl Drop for DirOverride {
    fn drop(&mut self) {
        *DIR_OVERRIDE.write().unwrap_or_else(std::sync::PoisonError::into_inner) = self.0.take();
    }
}

/// Where the cache lives, or None when it is off.
pub fn dir() -> Option<std::path::PathBuf> {
    if std::env::var_os("NFSU2_NO_CACHE").is_some_and(|v| v != "0") {
        return None;
    }
    if let Some(d) = DIR_OVERRIDE.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone() {
        return Some(d);
    }
    if let Some(d) = std::env::var_os("NFSU2_CACHE_DIR") {
        return Some(d.into());
    }
    crate::platform::dirs::cache()
}
