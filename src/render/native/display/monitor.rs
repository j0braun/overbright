//! docs/resolutions.md "Display-mode handling": the window's
//! smallest size and the Resolution row's list per display path, rebuilt
//! when the window's monitor changes.
//!
//! The row offers only sizes the active path can output: windowed, the
//! fixed list and the monitor's modes that fit on the monitor; full
//! screen, the monitor's own video modes (exclusive) and its desktop size
//! (borderless); on Wayland, which has no exclusive mode, the desktop size
//! alone. Before a monitor is known (headless runs, the launcher) the list
//! is [`modes`]'s, as before.

use super::*;

/// The smallest window (and Resolution) the game takes: the original's
/// smallest mode (0x800538 / 0x800550, 640 × 480). A smaller requested
/// size is raised to it ([`clamp_min_size`]); the window cannot be made
/// smaller (the game side's `with_min_inner_size`).
pub const MIN_WINDOW: (u32, u32) = (640, 480);

/// `size` raised to [`MIN_WINDOW`] in each axis, with one log line when
/// that changes it (`what` names where the size came from).
pub fn clamp_min_size(size: (u32, u32), what: &str) -> (u32, u32) {
    let out = (size.0.max(MIN_WINDOW.0), size.1.max(MIN_WINDOW.1));
    if out != size {
        log::info!(
            "{what}: {}x{} is below the smallest size {}x{}; using {}x{}",
            size.0,
            size.1,
            MIN_WINDOW.0,
            MIN_WINDOW.1,
            out.0,
            out.1
        );
    }
    out
}

/// What the Resolution row's list is built from: the window's monitor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonitorInfo {
    pub name: Option<String>,
    /// The desktop size in physical pixels (0 × 0 when unknown).
    pub desktop: (u32, u32),
    /// The monitor's video modes (sizes; any refresh rate).
    pub modes: Vec<(u32, u32)>,
    /// Exclusive full screen exists (false on Wayland).
    pub exclusive: bool,
}

impl MonitorInfo {
    /// The facts of a winit monitor; `wayland` when the window is a
    /// Wayland one (no exclusive full screen). Read through
    /// `platform::monitor` (never a panic: what cannot be read is left
    /// unknown, and the lists fall back to the desktop size).
    pub fn from_winit(m: &winit::monitor::MonitorHandle, wayland: bool) -> MonitorInfo {
        MonitorInfo::from_facts(crate::platform::monitor::facts(m), wayland)
    }

    /// [`from_winit`](Self::from_winit) from facts already read: no
    /// exclusive full screen when the modes cannot be listed safely
    /// (borderless at the desktop size instead).
    pub fn from_facts(f: crate::platform::monitor::Facts, wayland: bool) -> MonitorInfo {
        MonitorInfo { name: f.name, desktop: f.size, modes: f.modes, exclusive: !wayland && f.exclusive_modes }
    }
}

/// The Resolution row's list on `monitor` for the windowed or full-screen
/// path (sorted, deduplicated, every entry at least [`MIN_WINDOW`]).
/// Windowed: [`mode_list`] of the monitor's modes and desktop size, the
/// sizes larger than the desktop left out. Full screen: the monitor's
/// video modes and the desktop size (exclusive), or the desktop size alone
/// (no exclusive mode: borderless). No monitor: [`mode_list`] of none.
pub fn modes_for(monitor: Option<&MonitorInfo>, fullscreen: bool) -> Vec<(u32, u32)> {
    let Some(m) = monitor else { return mode_list(&[]) };
    let known = m.desktop.0 > 0 && m.desktop.1 > 0;
    let fits = |&(w, h): &(u32, u32)| !known || (w <= m.desktop.0 && h <= m.desktop.1);
    let big = |&(w, h): &(u32, u32)| w >= MIN_WINDOW.0 && h >= MIN_WINDOW.1;
    let mut list: Vec<(u32, u32)> = if !fullscreen {
        let mut sizes = m.modes.clone();
        if known {
            sizes.push(m.desktop);
        }
        mode_list(&sizes).into_iter().filter(fits).collect()
    } else if m.exclusive {
        m.modes.iter().copied().chain(known.then_some(m.desktop)).filter(big).filter(fits).collect()
    } else {
        known.then_some(m.desktop).into_iter().collect()
    };
    list.sort_unstable();
    list.dedup();
    if list.is_empty() {
        // A monitor reporting nothing usable (a desktop below 640 × 480):
        // the smallest size, which the window can still take.
        list.push(MIN_WINDOW);
    }
    list
}

/// `at` when `list` has it, else the largest listed size that fits in it
/// (by area), else the list's smallest; `at` for an empty list.
pub fn fit_mode(list: &[(u32, u32)], at: (u32, u32)) -> (u32, u32) {
    if list.is_empty() || list.contains(&at) {
        return at;
    }
    list.iter()
        .copied()
        .filter(|&(w, h)| w <= at.0 && h <= at.1)
        .max_by_key(|&(w, h)| (u64::from(w) * u64::from(h), w))
        .unwrap_or(list[0])
}

static MONITOR: std::sync::RwLock<Option<MonitorInfo>> = std::sync::RwLock::new(None);

/// Records the window's monitor (at window creation, and whenever the
/// window may be on another one: moved, a new scale factor, resized);
/// true when it differs from the one recorded (the lists change), with a
/// log line.
pub fn set_monitor(info: MonitorInfo) -> bool {
    let mut m = MONITOR.write().unwrap();
    if m.as_ref() == Some(&info) {
        return false;
    }
    log::info!(
        "monitor {:?}: desktop {}x{}, {} modes{}; Resolution list rebuilt",
        info.name,
        info.desktop.0,
        info.desktop.1,
        info.modes.len(),
        if info.exclusive { "" } else { ", no exclusive full screen" }
    );
    *m = Some(info);
    true
}

/// The recorded monitor ([`set_monitor`]), if any.
pub fn monitor() -> Option<MonitorInfo> {
    MONITOR.read().unwrap().clone()
}

/// The Resolution row's list for the active path: [`modes_for`] the
/// recorded monitor, or [`modes`] before one is recorded (headless runs,
/// a game side that still calls [`set_modes`]).
pub fn modes_on(fullscreen: bool) -> Vec<(u32, u32)> {
    match MONITOR.read().unwrap().as_ref() {
        Some(m) => modes_for(Some(m), fullscreen),
        None => modes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(desktop: (u32, u32), modes: &[(u32, u32)], exclusive: bool) -> MonitorInfo {
        MonitorInfo { name: None, desktop, modes: modes.to_vec(), exclusive }
    }

    #[test]
    fn smallest_size() {
        assert_eq!(clamp_min_size((320, 240), "test"), (640, 480));
        assert_eq!(clamp_min_size((1080, 400), "test"), (1080, 480));
        assert_eq!(clamp_min_size((640, 480), "test"), (640, 480));
        assert_eq!(clamp_min_size((1080, 1920), "test"), (1080, 1920));
    }

    #[test]
    fn lists_per_path() {
        let fhd = mon((1920, 1080), &[(640, 480), (800, 600), (1280, 720), (1920, 1080), (720, 400)], true);
        // Windowed: the fixed sizes and the monitor's that fit on it.
        let w = modes_for(Some(&fhd), false);
        assert!(w.contains(&(640, 480)) && w.contains(&(1280, 960)) && w.contains(&(1280, 720)));
        assert!(w.contains(&(1920, 1080)) && !w.contains(&(1600, 1200)) && !w.contains(&(2560, 1440)));
        assert!(!w.contains(&(720, 400)) && w.windows(2).all(|p| p[0] < p[1]));
        // Exclusive full screen: the monitor's modes and its desktop only.
        assert_eq!(modes_for(Some(&fhd), true), vec![(640, 480), (800, 600), (1280, 720), (1920, 1080)]);
        // No exclusive mode (Wayland): borderless at the desktop size.
        let way = mon((2560, 1440), &[(2560, 1440)], false);
        assert_eq!(modes_for(Some(&way), true), vec![(2560, 1440)]);
        assert!(modes_for(Some(&way), false).contains(&(2560, 1440)));
        // A portrait desktop: no landscape size taller than it is wide.
        let tall = mon((1080, 1920), &[], false);
        let t = modes_for(Some(&tall), false);
        assert!(t.contains(&(1080, 1920)) && t.contains(&(1024, 768)) && !t.contains(&(1280, 960)));
        // Unknown monitor: the old list.
        assert_eq!(modes_for(None, true), mode_list(&[]));
        // A desktop below the minimum: the minimum.
        assert_eq!(modes_for(Some(&mon((320, 240), &[], true)), true), vec![MIN_WINDOW]);
    }

    #[test]
    fn fitting() {
        let list = [(640, 480), (1280, 720), (1920, 1080)];
        assert_eq!(fit_mode(&list, (1280, 720)), (1280, 720));
        assert_eq!(fit_mode(&list, (1600, 1200)), (1280, 720));
        assert_eq!(fit_mode(&list, (2560, 1440)), (1920, 1080));
        assert_eq!(fit_mode(&list, (320, 240)), (640, 480));
        assert_eq!(fit_mode(&[], (800, 600)), (800, 600));
    }
}
