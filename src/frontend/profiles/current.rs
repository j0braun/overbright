use std::sync::Mutex;

use crate::formats::native_save::NativeSave;

/// The current profile: its name and the payload it was loaded from or
/// last saved as (the bytes the clone does not model go back unchanged).
pub(super) static CURRENT: Mutex<Option<(String, NativeSave)>> = Mutex::new(None);

/// [`CURRENT`], or the calling thread's own in a [`sandbox`].
fn with_current<R>(f: impl FnOnce(&mut Option<(String, NativeSave)>) -> R) -> R {
    // `f` runs exactly once: in the sandbox's closure (which then returns
    // Some) or below, so both `take`s find it.
    let mut f = Some(f);
    if let Some(r) = sandbox::with_current(|c| (f.take().expect("taken once"))(c)) {
        return r;
    }
    // A thread that panicked while holding the lock leaves a payload that
    // is still a whole, fixed-size save (`update_current` edits it in
    // place): keep using it rather than panicking on every later access
    // of the profile.
    (f.take().expect("taken once"))(&mut CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

pub fn current_name() -> Option<String> {
    with_current(|c| c.as_ref().map(|c| c.0.clone()))
}

pub fn current_save() -> Option<NativeSave> {
    with_current(|c| c.as_ref().map(|c| c.1.clone()))
}

pub fn set_current(name: &str, save: NativeSave) {
    with_current(|c| *c = Some((name.to_string(), save)));
}

pub fn clear_current() {
    with_current(|c| *c = None);
}

/// Change the current payload in place (the options live in the profile).
pub fn update_current(f: impl FnOnce(&mut NativeSave)) {
    with_current(|c| {
        if let Some(c) = c.as_mut() {
            f(&mut c.1);
        }
    });
}

/// Test isolation (docs/test-determinism.md; not native, never entered by
/// the game): a thread's own data folder and current profile. The unit
/// tests run each test on a thread of its own in one process, where the
/// process-wide `XDG_DATA_HOME` and [`CURRENT`] made one test's saves and
/// committed profile the next test's start (a `Game::new` took another
/// test's profile, car and options: the flaky race tests of 2026-11-04).
/// A test thread that enters a sandbox reads and writes only its own
/// folder and current profile; the folder is removed when the thread ends.
/// Outside a sandbox (the game, every thread it spawns) nothing changes.
#[cfg(any(test, feature = "dev-harness"))]
#[doc(hidden)]
pub mod sandbox {
    use super::NativeSave;
    use std::cell::RefCell;
    use std::path::PathBuf;

    struct Sandbox {
        dir: PathBuf,
        current: Option<(String, NativeSave)>,
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    thread_local! {
        static SANDBOX: RefCell<Option<Sandbox>> = const { RefCell::new(None) };
    }

    /// Give the calling thread a fresh, empty data folder under `parent`
    /// and no current profile, once: later calls on the same thread keep
    /// the sandbox (a test that builds a second game sees its own saves).
    /// Returns the thread's data folder.
    pub fn enter(parent: &std::path::Path) -> PathBuf {
        SANDBOX.with_borrow_mut(|s| {
            if s.is_none() {
                static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let dir = parent.join(format!("thread-{}-{n}", std::process::id()));
                let _ = std::fs::remove_dir_all(&dir);
                let _ = std::fs::create_dir_all(&dir);
                crate::platform::dirs::set_thread_data_home(dir.clone());
                *s = Some(Sandbox { dir, current: None });
            }
            s.as_ref().unwrap().dir.clone()
        })
    }

    /// Whether the calling thread is in a sandbox.
    pub fn active() -> bool {
        SANDBOX.with_borrow(Option::is_some)
    }

    pub(super) fn with_current<R>(f: impl FnOnce(&mut Option<(String, NativeSave)>) -> R) -> Option<R> {
        SANDBOX.with_borrow_mut(|s| s.as_mut().map(|s| f(&mut s.current)))
    }
}

/// Without the developer harness no thread is ever in a sandbox.
#[cfg(not(any(test, feature = "dev-harness")))]
pub(super) mod sandbox {
    use super::NativeSave;

    pub(super) fn with_current<R>(_f: impl FnOnce(&mut Option<(String, NativeSave)>) -> R) -> Option<R> {
        None
    }
}
