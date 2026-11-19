//! A monitor's facts read without letting a display driver's odd answer
//! stop the program (docs/notes/graphics/resolutions.md "Display-mode handling").
//!
//! winit's monitor calls can panic: on Windows its mode list asserts that
//! every mode the system lists carries its size, depth and refresh fields
//! (Wine on a virtual X display lists one that does not; a display driver
//! could do the same), and its name and size unwrap GetMonitorInfoW. The
//! game and the launcher read monitors only through here: on Windows the
//! modes come from the system directly ([`crate::platform::win::monitor_modes`]),
//! the modes without a size left out; winit's own mode list (needed for
//! exclusive full screen) is asked for only when every mode is complete.
//! Every winit call is also guarded ([`guarded`]): a panic there is caught
//! (the release profile unwinds), logged as one warning, and the caller
//! falls back (no modes: the desktop size; no exclusive mode: borderless).

use winit::monitor::{MonitorHandle, VideoModeHandle};

thread_local! {
    /// Inside [`guarded`]: the panic hook stays quiet (no message, no
    /// crash report: the caller logs one warning and goes on).
    static GUARDED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The panic hook in force, wrapped once so that a panic inside
/// [`guarded`] prints nothing (a hook installed later, the crash
/// reporter's, calls this one first and is quiet by [`in_guard`] too).
fn quiet_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let next = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !in_guard() {
                next(info);
            }
        }));
    });
}

/// Whether the calling thread is inside [`guarded`] (the panic hooks
/// say nothing then).
pub fn in_guard() -> bool {
    GUARDED.with(|g| g.get() > 0)
}

/// `f`, a call into winit's monitor API; None (with one warning naming
/// `what`) when it panics.
pub fn guarded<T>(what: &str, f: impl FnOnce() -> T) -> Option<T> {
    quiet_hook();
    GUARDED.with(|g| g.set(g.get() + 1));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    GUARDED.with(|g| g.set(g.get() - 1));
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            let why = e
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            log::warn!("monitor: {what} failed in the window system ({why}); using a fallback");
            None
        }
    }
}

/// What the game and the launcher read of a monitor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub name: Option<String>,
    /// The desktop size in physical pixels (0 × 0 when unknown).
    pub size: (u32, u32),
    /// The video modes' sizes (sorted, deduplicated; empty when unknown).
    pub modes: Vec<(u32, u32)>,
    /// winit's mode list can be asked for (exclusive full screen).
    pub exclusive_modes: bool,
}

/// `m`'s facts: the system's own answer on Windows, winit's elsewhere;
/// whatever cannot be read is left unknown, never a panic.
pub fn facts(m: &MonitorHandle) -> Facts {
    let mut f = platform_facts(m);
    f.modes.sort_unstable();
    f.modes.dedup();
    f
}

#[cfg(windows)]
fn platform_facts(m: &MonitorHandle) -> Facts {
    use winit::platform::windows::MonitorHandleExtWindows;
    match crate::platform::win::monitor_modes(m.hmonitor()) {
        Some(w) => {
            // One warning per process (the monitor is read again whenever
            // the window moves or is resized).
            static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if w.incomplete > 0 && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log::warn!(
                    "monitor {}: {} display modes without a size, depth or refresh rate (left out; no exclusive full \
                     screen, borderless instead)",
                    w.name,
                    w.incomplete
                );
            }
            Facts { name: Some(w.name), size: w.desktop, modes: w.sizes, exclusive_modes: w.incomplete == 0 }
        }
        None => {
            log::warn!("monitor: the system does not describe the window's monitor; the window's size is used");
            Facts::default()
        }
    }
}

#[cfg(not(windows))]
fn platform_facts(m: &MonitorHandle) -> Facts {
    let name = guarded("the monitor's name", || m.name()).flatten();
    let size = guarded("the monitor's size", || (m.size().width, m.size().height)).unwrap_or((0, 0));
    let modes = guarded("the monitor's modes", || m.video_modes().map(|v| (v.size().width, v.size().height)).collect());
    Facts { name, size, exclusive_modes: modes.is_some(), modes: modes.unwrap_or_default() }
}

/// `m`'s video mode of `size` at its highest refresh rate (and depth),
/// for exclusive full screen; None when it has none or its modes cannot
/// be listed safely (`exclusive_modes`).
pub fn exclusive_mode(m: &MonitorHandle, facts: &Facts, size: (u32, u32)) -> Option<VideoModeHandle> {
    if !facts.exclusive_modes || !facts.modes.contains(&size) {
        return None;
    }
    guarded("the monitor's modes", || {
        m.video_modes()
            .filter(|v| (v.size().width, v.size().height) == size)
            .max_by_key(|v| (v.refresh_rate_millihertz(), v.bit_depth()))
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A panic inside the guard is caught and answered with None; the
    /// guard is left (the hooks speak again) and the value of a call that
    /// returns comes through.
    #[test]
    fn a_panicking_monitor_call_falls_back() {
        let caught: Option<u32> = guarded("a test call", || panic!("a mode without its fields"));
        assert_eq!(caught, None);
        assert!(!in_guard());
        assert_eq!(guarded("a test call", || 7), Some(7));
        // Nested and on another thread alike.
        let inner = std::thread::spawn(|| guarded("outer", || guarded("inner", || -> u8 { panic!("x") }))).join();
        assert_eq!(inner.unwrap(), Some(None));
    }
}
