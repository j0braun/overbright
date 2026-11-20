//! docs/modding/pack-hot-reload.md: Reload Packs When Changed. A clone
//! deviation, opt-in on the Enhancements page (Off = native: packs are read
//! at startup or when a world / car loads, as before). Not a feature of
//! SPEED2.EXE; nothing here was traced.
//!
//! A background thread polls the folders in use (the HD UI pack, the world
//! and car texture packs) every [`TICK`] for changed, added or removed
//! `<HASH>.png` / `<HASH>.dds` / `<name>.png` files. A change is published
//! once the file has stayed the same for one more tick (debounce: a file
//! still being written keeps changing) and decodes ([`super::texture_pack::validate`];
//! a file that does not is retried [`RETRIES`] ticks, then kept out with
//! one warning, the texture in use staying as it was). Publishing forgets
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
