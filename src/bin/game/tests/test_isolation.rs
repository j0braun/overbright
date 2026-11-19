//! Test isolation for the game's unit tests (docs/developing/test-determinism.md;
//! test-only, `cfg(test)`).
//!
//! libtest runs every test on a thread of its own, all in one process, so
//! whatever lives process-wide is shared between tests that run at the same
//! time:
//!
//! * the data folder (`XDG_DATA_HOME`: options.txt, the profiles on disk)
//!   and the current profile (`profiles::CURRENT`) — [`Game::new`] read
//!   both, so a race test started in another test's profile, its car and
//!   options (the flaky `wrong_way_street_x` / `rivals_keep_the_race_route`
//!   of 2026-11-04). Each test thread now gets a sandbox of its own
//!   (`profiles::sandbox`): an empty data folder and no current profile.
//! * the Enhancements page's values (`qol`'s settings store, put in force by
//!   `Settings::apply_enhancements`, which every `Game::new` runs) and the
//!   other display globals. A test that changes them takes [`exclusive`]:
//!   it waits for every running game test to end and holds the others off
//!   until it ends; the native defaults are put back then. Every other game
//!   test holds [`shared`] (taken by `Game::new`), so no test ever sees
//!   another's values.
//!
//! * the machine's memory: every game test builds a `Game` (its world,
//!   cars, renderers' data), about 0.4 GB each and up to 1.1 GB at a time
//!   for the largest. libtest runs as many tests at once as the machine has
//!   cores, so a many-core machine with little memory ran out of it. A game
//!   test takes a memory slot ([`slot`]) before anything else; the number
//!   of slots comes from the memory available when the first test starts
//!   (docs/developing/testing.md "Memory").
//!
//! The guards live in a thread-local slot and are released when the test's
//! thread ends (also after a panic).

use std::cell::RefCell;
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

static SETTINGS: RwLock<()> = RwLock::new(());

enum Held {
    Shared(#[allow(dead_code)] RwLockReadGuard<'static, ()>),
    Exclusive(#[allow(dead_code)] RwLockWriteGuard<'static, ()>),
}

struct Slot(Option<Held>, Option<MemorySlot>);

impl Drop for Slot {
    fn drop(&mut self) {
        if matches!(self.0, Some(Held::Exclusive(_))) {
            reset_globals();
        }
    }
}

thread_local! {
    static HELD: RefCell<Slot> = const { RefCell::new(Slot(None, None)) };
}

/// The native defaults of the globals a test may change, put back as an
/// exclusive test ends.
fn reset_globals() {
    nfsu2::frontend::options::Settings::default().apply_enhancements();
    nfsu2::render::blur_dof::set_photo_mode(false);
    nfsu2::render::probes::set_burst_serial(false);
    nfsu2::qol::set_output_size((0, 0));
    nfsu2::frontend::career::UNLOCK_ALL.store(false, std::sync::atomic::Ordering::Relaxed);
    // A test's rain scene (`--shot explore_rain`'s clock): also put back
    // when the test failed before it did so itself.
    crate::dev::hooks::SHOT_RAIN_CLOCK.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// The calling test's sandbox (its own data folder and current profile)
/// and its shared hold on the process-wide settings; done by `Game::new`,
/// once per test thread.
pub(crate) fn enter() {
    // NFSU2_TEST_SHARED_STATE=1: the old shared state (no sandbox, no
    // hold), to reproduce the flakes this fixes.
    if std::env::var_os("NFSU2_TEST_SHARED_STATE").is_some() {
        return;
    }
    let parent = std::env::temp_dir().join(format!("nfsu2-unittest-{}", std::process::id()));
    nfsu2::frontend::profiles::sandbox::enter(&parent);
    shared();
}

/// The memory one game test may take at once (its `Game`, with room to
/// spare: the measured largest is 1.1 GB, the usual 0.3-0.5 GB).
const SLOT_MB: u64 = 1024;
/// Left free for the rest of the machine (and the test binary's own data).
const RESERVE_MB: u64 = 2048;

/// The number of game tests that may run at once: `OB_TEST_GAME_SLOTS`, else
/// the memory available now ([`RESERVE_MB`] kept free) in [`SLOT_MB`] steps,
/// at least 1; unlimited where the memory cannot be read.
fn slots_for(avail_mb: Option<u64>, env: Option<&str>) -> usize {
    if let Some(n) = env.and_then(|v| v.trim().parse::<usize>().ok()) {
        return n.max(1);
    }
    match avail_mb {
        Some(mb) => (mb.saturating_sub(RESERVE_MB) / SLOT_MB).max(1) as usize,
        None => usize::MAX,
    }
}

/// MemAvailable (Linux), in MB.
fn available_mb() -> Option<u64> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = info.lines().find(|l| l.starts_with("MemAvailable:"))?;
    Some(line.split_whitespace().nth(1)?.parse::<u64>().ok()? / 1024)
}

/// The free memory slots ([`slot`]), sized at the first use.
static SLOTS: std::sync::Mutex<Option<usize>> = std::sync::Mutex::new(None);
static SLOT_FREED: std::sync::Condvar = std::sync::Condvar::new();

/// A held memory slot, given back when dropped (the test's thread ends).
struct MemorySlot;

impl Drop for MemorySlot {
    fn drop(&mut self) {
        let mut free = SLOTS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(n) = free.as_mut() {
            *n = n.saturating_add(1);
        }
        SLOT_FREED.notify_one();
    }
}

/// This test's memory slot for the rest of it: waits while as many game
/// tests run as the memory allows (a no-op when the thread holds one).
/// Always taken before the settings' hold, so a test waiting for a slot
/// holds nothing another test waits for.
pub(crate) fn slot() {
    HELD.with_borrow_mut(|h| {
        if h.1.is_some() {
            return;
        }
        let mut free = SLOTS.lock().unwrap_or_else(PoisonError::into_inner);
        if free.is_none() {
            let n = slots_for(available_mb(), std::env::var("OB_TEST_GAME_SLOTS").ok().as_deref());
            if n != usize::MAX {
                eprintln!("game tests: at most {n} at once (memory available; OB_TEST_GAME_SLOTS overrides)");
            }
            *free = Some(n);
        }
        while free.is_some_and(|n| n == 0) {
            free = SLOT_FREED.wait(free).unwrap_or_else(PoisonError::into_inner);
        }
        if let Some(n) = free.as_mut() {
            *n = n.saturating_sub(1);
        }
        h.1 = Some(MemorySlot);
    });
}

/// A shared hold on the process-wide settings for the rest of the test (a
/// no-op when the thread holds one already, shared or exclusive).
pub(crate) fn shared() {
    slot();
    HELD.with_borrow_mut(|h| {
        if h.0.is_none() {
            h.0 = Some(Held::Shared(SETTINGS.read().unwrap_or_else(PoisonError::into_inner)));
        }
    });
}

/// The process-wide settings for this test alone, for the rest of it: call
/// before changing the Enhancements (`apply_enhancements`), photo mode or
/// other display globals. Waits until no other game test runs; the
/// defaults are put back when the test ends.
pub(crate) fn exclusive() {
    slot();
    HELD.with_borrow_mut(|h| {
        if matches!(h.0, Some(Held::Exclusive(_))) {
            return;
        }
        // Give up this thread's own shared hold first (it would wait on
        // itself), then wait for the others.
        h.0 = None;
        h.0 = Some(Held::Exclusive(SETTINGS.write().unwrap_or_else(PoisonError::into_inner)));
    });
}

/// Whether the calling thread holds the settings exclusively.
#[allow(dead_code)]
pub(crate) fn is_exclusive() -> bool {
    HELD.with_borrow(|h| matches!(h.0, Some(Held::Exclusive(_))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slots: the override first, else the available memory less the
    /// reserve in 1 GB steps, never fewer than one.
    #[test]
    fn memory_slots_follow_the_available_memory() {
        assert_eq!(slots_for(Some(34_000), None), 31);
        assert_eq!(slots_for(Some(12_000), None), 9);
        assert_eq!(slots_for(Some(1_500), None), 1);
        assert_eq!(slots_for(Some(34_000), Some("3")), 3);
        assert_eq!(slots_for(Some(34_000), Some("0")), 1);
        assert_eq!(slots_for(None, None), usize::MAX);
        assert!(available_mb().is_some_and(|mb| mb > 0));
    }
}
