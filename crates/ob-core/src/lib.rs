//! Overbright's bottom layer (ARCHITECTURE.md "Layers"): the environment
//! switches, the game folder and its fingerprint, the asset cache, the
//! platform folders, the clocks, the logs and the diagnostics' event ring,
//! the game's random numbers and the camera's values. It depends on no other
//! Overbright crate and on no window, GPU or audio crate; the library
//! (`nfsu2`) re-exports every module at its old path.

pub mod build_info;
pub mod cache;
pub mod console;
pub mod diag;
pub mod draw;
pub mod failed_load;
pub mod fingerprint;
pub mod frame_clock;
pub mod fxhash;
pub mod game_clock;
pub mod game_fs;
pub mod game_rng;
pub mod input;
pub mod log_file;
pub mod platform;
pub mod race_mode;
pub mod prof;
pub mod region;

use std::path::PathBuf;

/// Whether the environment variable `$name` is set, read once per call
/// site (debugging switches tested on frame paths cost a `getenv`, a
/// lock and a scan of the environment, every frame). The variables are
/// set before the game starts.
#[macro_export]
macro_rules! env_set {
    ($name:literal) => {{
        static SET: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *SET.get_or_init(|| std::env::var_os($name).is_some())
    }};
}

/// `$name`'s value (as `std::env::var(..).ok()`), read once per call site
/// (see [`env_set!`]): an `Option<&'static str>`.
#[macro_export]
macro_rules! env_value {
    ($name:literal) => {{
        static VALUE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        VALUE.get_or_init(|| std::env::var($name).ok()).as_deref()
    }};
}

/// `$name`'s value (as `std::env::var_os`), read once per call site (see
/// [`env_set!`]): an `Option<&'static OsStr>`.
#[macro_export]
macro_rules! env_value_os {
    ($name:literal) => {{
        static VALUE: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
        VALUE.get_or_init(|| std::env::var_os($name)).as_deref()
    }};
}

/// `$name` parsed as a `$t`, read once per call site (see [`env_set!`]).
#[macro_export]
macro_rules! env_parsed {
    ($name:literal, $t:ty) => {{
        static VALUE: std::sync::OnceLock<Option<$t>> = std::sync::OnceLock::new();
        *VALUE.get_or_init(|| std::env::var($name).ok().and_then(|v| v.parse::<$t>().ok()))
    }};
}

/// This crate's build id (build_support/build_id.rs): the asset caches its
/// code makes are named and checked by it.
pub const BUILD_ID: cache::BuildId = cache::BuildId::from_hex(env!("OB_BUILD_ID"));

/// Where the installed game lives: `$NFSU2_ROOT`, else the current directory.
pub fn game_root() -> PathBuf {
    std::env::var_os("NFSU2_ROOT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// A tag for temporary file and folder names that stays unique between
/// processes even when they share a PID (each `bwrap --unshare-pid` run is
/// PID 2): the PID, the start time in nanoseconds and a per-process count.
pub fn unique_tag() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::OnceLock;
    static START: OnceLock<u128> = OnceLock::new();
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let start = *START.get_or_init(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    format!("{}-{start:x}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed))
}
