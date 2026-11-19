//! The game's recent events in a small ring (docs/troubleshooting.md):
//! window, focus, display mode and size, surface configuration and
//! swap-chain failures, the adapter chosen, a lost device, screens and
//! loads, long frames, the first image shown. The ring is written to the
//! log ([`dump_to_log`]) on a panic, a hang (the [`watchdog`]), a lost
//! device and on Ctrl+F8; a support file carries it too.
//!
//! Cheap: an event is a short string pushed under a mutex, at most a few
//! a second outside a failure (a repeat of the last event only counts up);
//! each frame costs two atomic stores ([`frame_drawn`], [`loop_turned`]).
//! Nothing here touches the simulation, its random numbers or its clocks.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Events kept.
pub const CAP: usize = 256;

/// What an event is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The process's start and its steps.
    Start,
    /// The window: made, resized, moved, its mode; focus and visibility.
    Window,
    /// The GPU: the adapters, the one chosen, the device lost.
    Gpu,
    /// The swap chain: configured, an image that did not come.
    Surface,
    /// A screen or mode of the game.
    Screen,
    /// A drive's or a screen's load.
    Load,
    /// A frame that took long, no frames for a while.
    Stall,
    /// An error the game went on after, or that ended it.
    Error,
    /// The player's Ctrl+F8.
    Hotkey,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Start => "start",
            Kind::Window => "window",
            Kind::Gpu => "gpu",
            Kind::Surface => "surface",
            Kind::Screen => "screen",
            Kind::Load => "load",
            Kind::Stall => "stall",
            Kind::Error => "error",
            Kind::Hotkey => "hotkey",
        }
    }
}

#[derive(Clone, Debug)]
struct Event {
    /// Milliseconds since the first event of the process.
    at_ms: u64,
    /// Frames shown when it happened.
    frame: u64,
    kind: Kind,
    text: String,
    /// The same event again right after (counted, not stored).
    repeats: u32,
}

static RING: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
static EPOCH: OnceLock<Instant> = OnceLock::new();
/// Swap-chain images shown (acquired for a frame).
static FRAMES: AtomicU64 = AtomicU64::new(0);
/// Window frames drawn (the frame loop's end), for the watchdog.
static DRAWN: AtomicU64 = AtomicU64::new(0);
/// Event-loop turns, for the watchdog.
static TURNS: AtomicU64 = AtomicU64::new(0);
/// Whether the window has the focus and is not hidden (the watchdog's
/// "no frames while shown").
static SHOWN: AtomicBool = AtomicBool::new(true);

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Starts the ring's clock (the process's start); later events are timed
/// from here. Any first [`record`] does the same.
pub fn start() {
    let _ = EPOCH.get_or_init(Instant::now);
}

/// One event. The same kind and text as the last event only counts up.
pub fn record(kind: Kind, text: impl Into<String>) {
    let text = text.into();
    let at_ms = now_ms();
    let frame = FRAMES.load(Ordering::Relaxed);
    // try_lock: a panic while the ring is held (or on the thread holding
    // it) never waits on itself.
    let Ok(mut ring) = RING.try_lock() else { return };
    if let Some(last) = ring.back_mut()
        && last.kind == kind
        && last.text == text
    {
        last.repeats += 1;
        return;
    }
    if ring.len() >= CAP {
        ring.pop_front();
    }
    ring.push_back(Event { at_ms, frame, kind, text, repeats: 0 });
}

/// A swap-chain image was taken for a frame; the first one is an event.
pub fn image_shown() {
    if FRAMES.fetch_add(1, Ordering::Relaxed) == 0 {
        record(Kind::Window, "first image shown");
    }
}

/// Images shown so far.
pub fn images_shown() -> u64 {
    FRAMES.load(Ordering::Relaxed)
}

/// The frame loop ended a frame (the watchdog's heartbeat).
pub fn frame_drawn() {
    DRAWN.fetch_add(1, Ordering::Relaxed);
}

/// The event loop turned (the watchdog's heartbeat for the main thread).
pub fn loop_turned() {
    TURNS.fetch_add(1, Ordering::Relaxed);
}

/// The window's focus and visibility changed.
pub fn set_shown(focused: bool, occluded: bool) {
    SHOWN.store(focused && !occluded, Ordering::Relaxed);
    record(Kind::Window, format!("focused {focused}, hidden {occluded}"));
}

/// The ring as text, oldest first, one event a line.
pub fn text() -> String {
    let events: Vec<Event> = match RING.try_lock() {
        Ok(r) => r.iter().cloned().collect(),
        Err(_) => return "(the event list was busy)\n".into(),
    };
    format_events(&events, FRAMES.load(Ordering::Relaxed), now_ms())
}

fn format_events(events: &[Event], frames: u64, now: u64) -> String {
    let mut s =
        format!("{} events, {frames} images shown, {:.1} s since the start\n", events.len(), now as f64 / 1000.0);
    for e in events {
        s += &format!("{:>9.3} s  frame {:>7}  {:<7} {}", e.at_ms as f64 / 1000.0, e.frame, e.kind.name(), e.text);
        if e.repeats > 0 {
            s += &format!(" (and {} more times)", e.repeats);
        }
        s.push('\n');
    }
    s
}

/// The ring into the log (at Warn, so a plain game run's file keeps it),
/// headed by `why`.
pub fn dump_to_log(why: &str) {
    let text = crate::log_file::scrub(&text());
    log::warn!(target: "events", "the last events ({why}):\n{text}");
}

/// The watchdog's limits.
pub const HANG_AFTER: Duration = Duration::from_secs(10);

/// Starts a thread that watches the heartbeats: when the event loop does
/// not turn for [`HANG_AFTER`] (the main thread is stuck), or the window is
/// shown but no frame is drawn for that long, the ring goes to the log once
/// per such stall, and its end is an event. Call once, for the window run.
pub fn watchdog() {
    let spawned = std::thread::Builder::new().name("watchdog".into()).spawn(|| {
        let mut last = (TURNS.load(Ordering::Relaxed), DRAWN.load(Ordering::Relaxed));
        let mut since = (Instant::now(), Instant::now());
        let mut told = (false, false);
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let now = (TURNS.load(Ordering::Relaxed), DRAWN.load(Ordering::Relaxed));
            let t = Instant::now();
            let (turn_stuck, frame_stuck) =
                watch_step(&mut last, now, &mut since, t, &mut told, SHOWN.load(Ordering::Relaxed));
            if let Some(secs) = turn_stuck {
                record(Kind::Stall, format!("the main thread has not turned for {secs:.0} s (a hang, or a long load)"));
                dump_to_log("no event-loop turn for 10 s");
                crate::log_file::flush(Duration::from_millis(500));
            }
            if let Some(secs) = frame_stuck {
                record(Kind::Stall, format!("no frame drawn for {secs:.0} s while the window is shown"));
                dump_to_log("no frame for 10 s");
                crate::log_file::flush(Duration::from_millis(500));
            }
        }
    });
    if let Err(e) = spawned {
        log::warn!("watchdog not started: {e}");
    }
}

/// One watchdog look (pure but for the events of a recovery): the counters
/// now against the last, when each last moved, and whether its stall was
/// already told. Returns the stall lengths (s) to tell now.
fn watch_step(
    last: &mut (u64, u64),
    now: (u64, u64),
    since: &mut (Instant, Instant),
    t: Instant,
    told: &mut (bool, bool),
    shown: bool,
) -> (Option<f32>, Option<f32>) {
    let mut out = (None, None);
    if now.0 != last.0 {
        if told.0 {
            record(Kind::Stall, format!("the main thread turns again after {:.1} s", (t - since.0).as_secs_f32()));
        }
        *told = (false, told.1);
        since.0 = t;
    } else if !told.0 && t - since.0 >= HANG_AFTER {
        told.0 = true;
        out.0 = Some((t - since.0).as_secs_f32());
    }
    if now.1 != last.1 || !shown {
        if told.1 && now.1 != last.1 {
            record(Kind::Stall, format!("frames again after {:.1} s", (t - since.1).as_secs_f32()));
        }
        if now.1 != last.1 {
            told.1 = false;
        }
        since.1 = t;
    } else if !told.1 && !told.0 && out.0.is_none() && t - since.1 >= HANG_AFTER {
        told.1 = true;
        out.1 = Some((t - since.1).as_secs_f32());
    }
    *last = now;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_kept_counted_and_formatted() {
        let e = |kind, text: &str, repeats| Event { at_ms: 1500, frame: 3, kind, text: text.into(), repeats };
        let s = format_events(&[e(Kind::Surface, "timeout", 4), e(Kind::Screen, "menu Main", 0)], 9, 2000);
        assert!(s.starts_with("2 events, 9 images shown, 2.0 s"), "{s}");
        assert!(s.contains("surface timeout (and 4 more times)"), "{s}");
        assert!(s.contains("screen  menu Main\n"), "{s}");
    }

    #[test]
    fn the_ring_counts_repeats_and_drops_the_oldest() {
        // The one test that uses the process's ring.
        for i in 0..CAP + 10 {
            record(Kind::Load, format!("ring test {i}"));
        }
        record(Kind::Load, format!("ring test {}", CAP + 9));
        let t = text();
        assert!(!t.contains("ring test 9\n") && t.contains("ring test 10\n"), "{t}");
        assert!(t.contains(&format!("ring test {} (and 1 more times)", CAP + 9)), "{t}");
    }

    #[test]
    fn the_watchdog_tells_a_stall_once_and_its_end() {
        let t0 = Instant::now();
        let mut last = (1, 1);
        let mut since = (t0, t0);
        let mut told = (false, false);
        let at = |s: u64| t0 + Duration::from_secs(s);
        // Turning but no frames, the window shown: a frame stall after 10 s, once.
        assert_eq!(watch_step(&mut last, (2, 1), &mut since, at(5), &mut told, true), (None, None));
        let (a, b) = watch_step(&mut last, (3, 1), &mut since, at(11), &mut told, true);
        assert!(a.is_none() && b.is_some_and(|s| s >= 10.0));
        assert_eq!(watch_step(&mut last, (4, 1), &mut since, at(30), &mut told, true), (None, None));
        // Hidden: no frame stall.
        let mut told2 = (false, false);
        let mut since2 = (t0, t0);
        let mut last2 = (1, 1);
        assert_eq!(watch_step(&mut last2, (2, 1), &mut since2, at(20), &mut told2, false), (None, None));
        // The main thread stuck: told once, not as a frame stall too.
        let (a, b) = watch_step(&mut last2, (2, 1), &mut since2, at(31), &mut told2, true);
        assert!(a.is_some() && b.is_none());
        assert_eq!(watch_step(&mut last2, (2, 1), &mut since2, at(40), &mut told2, true), (None, None));
    }
}
