//! The game's global allocator (clone choice): mimalloc
//! by default, the system allocator on machines with little memory. Nothing of the original: SPEED2.EXE uses its own pools over the
//! MSVC heap; this only changes how fast the clone's loaders get memory.
//!
//! A global allocator cannot be swapped once memory is handed out, so
//! [`GameAlloc`] picks its mode at the process's very first allocation
//! (before `main`, on one thread), without allocating itself, and keeps it
//! for the process's life: every block is freed by the allocator that gave
//! it.
//!
//! * `fast` (the default): mimalloc with its defaults (v3). Loads about
//!   half of plain glibc's (as fast as glibc with huge pages), at +170 MB
//!   peak in free roam, +450 MB in a race, ~+0.8–1.3 GB over the
//!   transitions bench (README "Performance", 2026-10-31).
//! * `system`: the platform's allocator (glibc's on Linux, then with huge
//!   pages through `main`'s re-exec). What a machine with little memory
//!   gets: on Linux it measured the leanest by far and loads as fast.
//! * `lean`: mimalloc with its memory-saving options (purge freed memory at
//!   once, no transparent huge pages, arenas committed on demand; each left
//!   alone when its own `MIMALLOC_*` variable is set). Only on request:
//!   measured −150…−250 MB against fast but still +300 MB over system in a
//!   race, and the slowest loads.
//!
//! `NFSU2_ALLOC=fast|lean|system` forces one; otherwise (`auto`) the
//! machine decides: below [`LEAN_TOTAL_MB`] / [`LEAN_AVAILABLE_MB`] system,
//! else fast.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering::Relaxed};

/// Below this much physical memory the system allocator is used. 10 GiB
/// covers every "8 GB" laptop (they report 7.2–7.8 GiB, an integrated GPU
/// taking its share from the same memory) and leaves 12/16 GB machines on
/// fast: fast's extra (+0.17 GB free roam, +0.45 GB a race, +0.8–1.3 GB
/// over the transitions bench, README) is 2–16 % of 8 GB, enough with a
/// browser open to push such a machine into swap, which costs far more
/// than mimalloc saves.
pub const LEAN_TOTAL_MB: u64 = 10 * 1024;
/// Or when less than this is available at start (fast's peak over the
/// transitions bench is ~2.5 GB; the GPU driver and the OS want room too).
pub const LEAN_AVAILABLE_MB: u64 = 4 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    System = 1,
    Lean = 2,
    Fast = 3,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::System => "system",
            Mode::Lean => "lean",
            Mode::Fast => "fast",
        }
    }
    fn from_u8(v: u8) -> Mode {
        match v {
            1 => Mode::System,
            2 => Mode::Lean,
            _ => Mode::Fast,
        }
    }
}

static MODE: AtomicU8 = AtomicU8::new(0);
/// What the choice saw (MB; 0 unknown) and whether NFSU2_ALLOC named it.
static TOTAL_MB: AtomicU64 = AtomicU64::new(0);
static AVAILABLE_MB: AtomicU64 = AtomicU64::new(0);
static FORCED: AtomicU8 = AtomicU8::new(0);

/// The allocator mode of this process (decided on first use).
pub fn mode() -> Mode {
    Mode::from_u8(mode_u8())
}

/// "fast (auto: 61 GB RAM, 43 GB available)" for the log and the benches.
pub fn describe() -> String {
    let m = mode();
    let gb = |mb: u64| if mb == 0 { "?".to_string() } else { format!("{:.1}", mb as f64 / 1024.0) };
    let how = if FORCED.load(Relaxed) != 0 { "NFSU2_ALLOC" } else { "auto" };
    format!("{} ({how}: {} GB RAM, {} GB available)", m.name(), gb(TOTAL_MB.load(Relaxed)), gb(AVAILABLE_MB.load(Relaxed)))
}

#[inline]
fn mode_u8() -> u8 {
    let m = MODE.load(Relaxed);
    if m != 0 { m } else { decide() }
}

#[cold]
fn decide() -> u8 {
    let m = choose();
    if m == Mode::Lean {
        lean_options();
    }
    // Before main only one thread runs; should two race anyway, the first
    // stored choice wins for both (they read the same machine).
    match MODE.compare_exchange(0, m as u8, Relaxed, Relaxed) {
        Ok(_) => m as u8,
        Err(prev) => prev,
    }
}

/// The mode for these values (`forced` NFSU2_ALLOC's value, if any).
pub fn pick(forced: Option<&[u8]>, total_mb: u64, available_mb: u64) -> Mode {
    match forced {
        Some(b"fast") => return Mode::Fast,
        Some(b"lean") => return Mode::Lean,
        Some(b"system") => return Mode::System,
        _ => {}
    }
    if (total_mb != 0 && total_mb < LEAN_TOTAL_MB) || (available_mb != 0 && available_mb < LEAN_AVAILABLE_MB) { Mode::System } else { Mode::Fast }
}

fn choose() -> Mode {
    let forced = env(c"NFSU2_ALLOC");
    let (total, available) = memory_mb();
    TOTAL_MB.store(total, Relaxed);
    AVAILABLE_MB.store(available, Relaxed);
    let m = pick(forced, total, available);
    if forced.is_some_and(|f| matches!(f, b"fast" | b"lean" | b"system")) {
        FORCED.store(1, Relaxed);
    }
    m
}

/// An environment variable's bytes without allocating (the C runtime's
/// getenv; nothing else runs yet).
fn env(name: &std::ffi::CStr) -> Option<&'static [u8]> {
    unsafe extern "C" {
        fn getenv(name: *const std::ffi::c_char) -> *const std::ffi::c_char;
    }
    // SAFETY: a NUL-terminated name; the result is the environment's own
    // string, which nothing changes before main.
    let p = unsafe { getenv(name.as_ptr()) };
    (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_bytes())
}

// mimalloc v3's option numbers (libmimalloc-sys 0.1.49, c_src/mimalloc/v3
// include/mimalloc.h `mi_option_e`).
const MI_ARENA_EAGER_COMMIT: i32 = 4;
const MI_ALLOW_LARGE_OS_PAGES: i32 = 6;
const MI_PURGE_DELAY: i32 = 15;
const MI_ALLOW_THP: i32 = 43;

unsafe extern "C" {
    fn mi_option_set(option: i32, value: std::ffi::c_long);
}

/// The lean mode's options, set before mimalloc hands out its first block
/// (each read when it is first needed); a `MIMALLOC_*` variable wins.
fn lean_options() {
    for (opt, var, value) in [
        (MI_PURGE_DELAY, c"MIMALLOC_PURGE_DELAY", 0),
        (MI_ALLOW_THP, c"MIMALLOC_ALLOW_THP", 0),
        (MI_ALLOW_LARGE_OS_PAGES, c"MIMALLOC_ALLOW_LARGE_OS_PAGES", 0),
        (MI_ARENA_EAGER_COMMIT, c"MIMALLOC_ARENA_EAGER_COMMIT", 0),
    ] {
        if env(var).is_none() {
            // SAFETY: a plain option store, valid at any time.
            unsafe { mi_option_set(opt, value) };
        }
    }
}

/// Physical memory and what is available now, in MB (0 when unknown).
#[cfg(any(target_os = "linux", target_os = "android"))]
fn memory_mb() -> (u64, u64) {
    // /proc/meminfo read into a stack buffer (MemAvailable counts the page
    // cache the kernel can drop, which sysinfo's freeram does not).
    let mut buf = [0u8; 4096];
    // SAFETY: plain open/read/close into a buffer of ours.
    let n = unsafe {
        let fd = libc::open(c"/proc/meminfo".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC);
        if fd < 0 {
            return (0, 0);
        }
        let n = libc::read(fd, buf.as_mut_ptr().cast(), buf.len());
        libc::close(fd);
        n
    };
    if n <= 0 {
        return (0, 0);
    }
    let text = &buf[..n as usize];
    let field = |key: &[u8]| -> u64 {
        text.split(|&b| b == b'\n')
            .find(|l| l.starts_with(key))
            .map(|l| l[key.len()..].iter().filter(|b| b.is_ascii_digit()).fold(0u64, |a, &b| a * 10 + (b - b'0') as u64))
            .unwrap_or(0)
            / 1024
    };
    (field(b"MemTotal:"), field(b"MemAvailable:"))
}

#[cfg(target_os = "macos")]
fn memory_mb() -> (u64, u64) {
    let mut total: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: sysctl into a u64 of ours.
    let rc = unsafe { libc::sysctlbyname(c"hw.memsize".as_ptr(), (&raw mut total).cast(), &mut len, std::ptr::null_mut(), 0) };
    // Available memory is not one number on macOS (compressed memory); the
    // total decides there.
    (if rc == 0 { total / (1024 * 1024) } else { 0 }, 0)
}

#[cfg(windows)]
fn memory_mb() -> (u64, u64) {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }
    // SAFETY: a zeroed struct with its length set, as the API asks.
    let mut s: MemoryStatusEx = unsafe { std::mem::zeroed() };
    s.length = std::mem::size_of::<MemoryStatusEx>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut s) } == 0 {
        return (0, 0);
    }
    (s.total_phys / (1024 * 1024), s.avail_phys / (1024 * 1024))
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
fn memory_mb() -> (u64, u64) {
    (0, 0)
}

/// mimalloc (fast or lean) or the system allocator, as [`mode`] decided.
pub struct GameAlloc;

// SAFETY: every call goes to the one allocator the process's mode names,
// so each block is resized and freed by the allocator that gave it.
unsafe impl GlobalAlloc for GameAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { if mode_u8() == Mode::System as u8 { System.alloc(layout) } else { mimalloc::MiMalloc.alloc(layout) } }
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { if mode_u8() == Mode::System as u8 { System.alloc_zeroed(layout) } else { mimalloc::MiMalloc.alloc_zeroed(layout) } }
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { if mode_u8() == Mode::System as u8 { System.dealloc(ptr, layout) } else { mimalloc::MiMalloc.dealloc(ptr, layout) } }
    }
    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { if mode_u8() == Mode::System as u8 { System.realloc(ptr, layout, new_size) } else { mimalloc::MiMalloc.realloc(ptr, layout, new_size) } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_picks_the_mode() {
        assert_eq!(pick(None, 61 * 1024, 43 * 1024), Mode::Fast);
        assert_eq!(pick(None, 16 * 1024, 9 * 1024), Mode::Fast);
        assert_eq!(pick(None, 7800, 5000), Mode::System, "an 8 GB laptop");
        assert_eq!(pick(None, 32 * 1024, 2000), Mode::System, "little available");
        assert_eq!(pick(None, 16 * 1024, 0), Mode::Fast, "available unknown (macOS)");
        assert_eq!(pick(None, 0, 0), Mode::Fast, "nothing known");
        assert_eq!(pick(Some(b"system"), 61 * 1024, 43 * 1024), Mode::System);
        assert_eq!(pick(Some(b"fast"), 4096, 1000), Mode::Fast);
        assert_eq!(pick(Some(b"lean"), 61 * 1024, 43 * 1024), Mode::Lean);
        assert_eq!(pick(Some(b"bogus"), 4096, 1000), Mode::System);
        assert_eq!(pick(Some(b"auto"), 61 * 1024, 43 * 1024), Mode::Fast);
    }

    #[test]
    fn this_machine_is_read() {
        let (total, available) = memory_mb();
        if cfg!(any(target_os = "linux", windows)) {
            assert!(total > 512 && available > 0 && available <= total, "{total} {available}");
        }
    }
}
