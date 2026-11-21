//! docs/modding/pack-hot-reload.md: Reload Packs When Changed. A clone
//! deviation, opt-in on the Enhancements page (Off = native: packs are read
//! at startup or when a world / car loads, as before). Not a feature of
//! SPEED2.EXE; nothing here was traced.
//!
//! A background thread polls the folders in use (the HD UI pack, the world
//! and car texture packs) every [`TICK`] for changed, added or removed
//! `<HASH>.png` / `<HASH>.dds` / `<name>.png` files. A change is published
//! as soon as the file decodes ([`super::texture_pack::validate`]): at once
//! for a whole save, else once it has stayed the same for one more tick (a
//! file still being written); one that does not is retried [`RETRIES`]
//! ticks, then kept out with one warning, the texture in use staying as it
//! was. Recent files are compared by content too ([`RACY`]), and the
//! watcher looks again soon after a change ([`FAST_TICK`]). Publishing forgets
//! the loader's cached replacement and appends to a log the renderers read
//! from their own generation on ([`since`]): the UI renderer rebinds the
//! changed textures, the world renderer rewrites their array layers (or
//! rebuilds its texture arrays when a size changes), car renderers
//! re-upload theirs. A folder that appears, goes or changes (the HD UI Art
//! or World Texture Pack row, a pack being completed) reloads its domains
//! as a whole; Ctrl+R does that for all ([`request_full`]).
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use super::texture_pack::{self, Domain};

/// The watcher's poll interval, and so the debounce: a change shows once
/// the file has been quiet for one interval.
pub const TICK: Duration = Duration::from_millis(250);
/// Ticks a changed file that does not decode is retried (half-written or
/// still being copied) before it is kept out with a warning.
pub const RETRIES: u32 = 8;
mod journal;
mod live;
mod sync;
mod watch;
pub use journal::*;
pub use live::*;
pub use sync::*;
pub use watch::*;

#[cfg(test)]
mod tests;

/// Tests: a slow disk or a slow reload, injected at the watcher's and the
/// loader's steps (a random wait of up to this many ms; 0: none).
#[cfg(test)]
pub(in crate::render) static SLOW_MS: AtomicU64 = AtomicU64::new(0);

/// Tests: wait as [`SLOW_MS`] says.
#[cfg(test)]
pub(in crate::render) fn slow_step() {
    let ms = SLOW_MS.load(Ordering::Relaxed);
    if ms == 0 {
        return;
    }
    static SEED: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
    let mut x = SEED.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    std::thread::sleep(Duration::from_micros(x % (ms * 1000)));
}
