//! The files the next minutes need, read ahead into the system's file
//! cache (docs/read-ahead.md). After a reboot the game's files are on the
//! disk only, and the loads that read them on demand then wait for the
//! disk, a piece at a time. One thread at the lowest priority reads the
//! queued files from start to end in large pieces, in the order the game
//! needs them, so that the loads find them in memory. It reads nothing a
//! load does not read, and it gives way to the loads the player waits
//! for: it stops between two pieces while a [`Hold`] is held (a drive's
//! load) and while the frame on screen has been drawn for longer than
//! [`BUSY_FRAME`] (a frame that loads), and after a piece that took long
//! (a busy disk, [`SLOW_PIECE`]) it pauses as long. A file whose pages are
//! all in memory already (Linux: `mincore`) is not read again. Nothing it does
//! changes what is loaded: the loads read the same bytes from the same
//! files either way. `NFSU2_READ_AHEAD=0` turns it off.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The size of one read (and the step at which the reader checks whether
/// to give way).
pub const PIECE: usize = 512 << 10;

/// A frame that has run this long is loading something the player waits
/// for: the reader stops until the next frame starts.
pub const BUSY_FRAME: Duration = Duration::from_millis(50);

/// A piece that took longer than this to read means a busy disk: the
/// reader then pauses as long as the piece took (at most [`SLOW_PAUSE`]),
/// so that it never holds more than half of a slow disk's time.
pub const SLOW_PIECE: Duration = Duration::from_millis(20);

/// The longest pause after a slow piece.
pub const SLOW_PAUSE: Duration = Duration::from_millis(500);

/// What the reader did so far (the game logs it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Files read to their end (or found in memory).
    pub files: u32,
    /// Files that could not be opened (not there: skipped).
    pub missing: u32,
    /// Bytes read from the files.
    pub read: u64,
    /// Bytes found in memory already, not read.
    pub resident: u64,
    /// Time spent giving way to loads and long frames.
    pub held_ms: u64,
    /// Time spent reading.
    pub busy_ms: u64,
}

#[derive(Default)]
struct State {
    queue: std::collections::VecDeque<PathBuf>,
    /// Every file ever queued (a file is read once a run).
    seen: std::collections::HashSet<PathBuf>,
    running: bool,
    stats: Stats,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static HOLDS: AtomicUsize = AtomicUsize::new(0);
/// When the frame on screen started (ns since [`epoch`]; 0: no frame yet).
static FRAME: AtomicU64 = AtomicU64::new(0);

fn epoch() -> Instant {
    static E: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *E.get_or_init(Instant::now)
}

fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut s = STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(s.get_or_insert_with(Default::default))
}

/// Whether reading ahead is on (`NFSU2_READ_AHEAD=0`: off).
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NFSU2_READ_AHEAD").is_none_or(|v| v != "0"))
}

/// `files` read ahead in this order, after the ones queued before; a file
/// queued before in this run is left out. The reader thread starts when
/// there is work and ends when the queue is empty.
pub fn queue(files: impl IntoIterator<Item = PathBuf>) {
    if !enabled() {
        return;
    }
    epoch();
    let start = state(|s| {
        for f in files {
            if s.seen.insert(f.clone()) {
                s.queue.push_back(f);
            }
        }
        let start = !s.running && !s.queue.is_empty();
        s.running |= start;
        start
    });
    if start {
        let spawned = std::thread::Builder::new().name("read ahead".into()).spawn(run);
        if spawned.is_err() {
            // No thread: nothing is read ahead (the loads read as before).
            state(|s| {
                s.running = false;
                s.queue.clear();
            });
        }
    }
}

/// What the reader did so far.
pub fn stats() -> Stats {
    state(|s| s.stats)
}

/// Whether the reader has work queued or running.
pub fn busy() -> bool {
    state(|s| s.running)
}

/// A load the player waits for: the reader stops between two pieces until
/// every hold is dropped.
#[must_use = "the reader goes on as soon as the hold is dropped"]
pub struct Hold(());

impl Drop for Hold {
    fn drop(&mut self) {
        HOLDS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// A [`Hold`] for as long as it lives.
pub fn hold() -> Hold {
    HOLDS.fetch_add(1, Ordering::AcqRel);
    Hold(())
}

/// The frame loop: a frame starts now ([`BUSY_FRAME`]).
pub fn frame_started() {
    let ns = epoch().elapsed().as_nanos().max(1);
    FRAME.store(u64::try_from(ns).unwrap_or(u64::MAX), Ordering::Release);
}

/// Whether the reader is to give way now: a hold, or a frame that has run
/// for longer than [`BUSY_FRAME`].
fn give_way() -> bool {
    if HOLDS.load(Ordering::Acquire) > 0 {
        return true;
    }
    let started = FRAME.load(Ordering::Acquire);
    started != 0 && epoch().elapsed().as_nanos().saturating_sub(u128::from(started)) > BUSY_FRAME.as_nanos()
}

/// The reader thread: the queue's files, one after another.
fn run() {
    lower_priority();
    let mut buf = vec![0u8; PIECE];
    loop {
        let Some(path) = state(|s| {
            let next = s.queue.pop_front();
            s.running = next.is_some();
            next
        }) else {
            let s = stats();
            log::info!(
                "read ahead: queue done {} ms after the first: {} files, {:.1} MB read, {:.1} MB in memory already, {} not there; gave way {} ms, read {} ms",
                epoch().elapsed().as_millis(),
                s.files,
                s.read as f64 / 1e6,
                s.resident as f64 / 1e6,
                s.missing,
                s.held_ms,
                s.busy_ms
            );
            return;
        };
        let t = Instant::now();
        let (read, resident, held) = read_file(&path, &mut buf);
        let busy = t.elapsed().saturating_sub(held);
        state(|s| {
            match read {
                Some(n) => {
                    s.stats.files += 1;
                    s.stats.read += n;
                    s.stats.resident += resident;
                }
                None => s.stats.missing += 1,
            }
            s.stats.held_ms += held.as_millis() as u64;
            s.stats.busy_ms += busy.as_millis() as u64;
        });
    }
}

/// Wait while [`give_way`]; the time waited.
fn wait_turn() -> Duration {
    let t = Instant::now();
    while give_way() {
        std::thread::sleep(Duration::from_millis(2));
    }
    t.elapsed()
}

/// `path` read from start to end, the pieces already in memory skipped:
/// the bytes read (None: not opened), the bytes skipped, the time waited.
fn read_file(path: &Path, buf: &mut [u8]) -> (Option<u64>, u64, Duration) {
    let mut held = wait_turn();
    let Ok(file) = std::fs::File::open(path) else { return (None, 0, held) };
    let Ok(len) = file.metadata().map(|m| m.len()) else { return (None, 0, held) };
    let in_memory = Residency::of(&file, len);
    let (mut at, mut read, mut resident) = (0u64, 0u64, 0u64);
    while at < len {
        let n = (len - at).min(buf.len() as u64);
        if in_memory.as_ref().is_some_and(|r| r.all(at, n)) {
            resident += n;
            at += n;
            continue;
        }
        held += wait_turn();
        let t = Instant::now();
        match read_at(&file, &mut buf[..n as usize], at) {
            Ok(0) | Err(_) => break,
            Ok(got) => {
                read += got as u64;
                at += got as u64;
            }
        }
        let took = t.elapsed();
        if took > SLOW_PIECE {
            let pause = took.min(SLOW_PAUSE);
            std::thread::sleep(pause);
            held += pause;
        }
    }
    (Some(read), resident, held)
}

#[cfg(unix)]
fn read_at(file: &std::fs::File, buf: &mut [u8], at: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buf, at)
}

#[cfg(windows)]
fn read_at(file: &std::fs::File, buf: &mut [u8], at: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, buf, at)
}

#[cfg(not(any(unix, windows)))]
fn read_at(file: &std::fs::File, buf: &mut [u8], at: u64) -> std::io::Result<usize> {
    use std::io::{Read, Seek};
    let mut f = file;
    f.seek(std::io::SeekFrom::Start(at))?;
    f.read(buf)
}

/// Which pages of a file are in memory (Linux: `mincore` over a mapping);
/// None elsewhere (every piece is read).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct Residency {
    pages: Vec<u8>,
    page: u64,
}

impl Residency {
    #[cfg(target_os = "linux")]
    fn of(file: &std::fs::File, len: u64) -> Option<Residency> {
        if len == 0 {
            return None;
        }
        // SAFETY: a read-only private mapping, only passed to mincore (its
        // bytes are never read here); dropped at the end of this call.
        let map = unsafe { memmap2::MmapOptions::new().map(file) }.ok()?;
        // SAFETY: plain sysconf.
        let page = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok().filter(|&p| p > 0)?;
        let mut pages = vec![0u8; len.div_ceil(page) as usize];
        // SAFETY: the mapping's start is page-aligned and `pages` holds one
        // byte per page of its length.
        let ok = unsafe { libc::mincore(map.as_ptr() as *mut libc::c_void, map.len(), pages.as_mut_ptr().cast()) } == 0;
        ok.then_some(Residency { pages, page })
    }

    #[cfg(not(target_os = "linux"))]
    fn of(_file: &std::fs::File, _len: u64) -> Option<Residency> {
        None
    }

    /// Whether every page of `[at, at + n)` is in memory.
    fn all(&self, at: u64, n: u64) -> bool {
        let (a, b) = ((at / self.page) as usize, (at + n).div_ceil(self.page) as usize);
        self.pages.get(a..b).is_some_and(|p| p.iter().all(|&v| v & 1 == 1))
    }
}

/// The reader thread at the lowest CPU and disk priority (Linux: nice 19
/// and the idle I/O class, which BFQ honours; Windows: background mode,
/// which lowers its I/O priority too).
fn lower_priority() {
    #[cfg(target_os = "linux")]
    // SAFETY: plain calls on this thread (Linux: a thread is a task for
    // setpriority and ioprio_set; 0 = the calling thread). IOPRIO_WHO_PROCESS
    // = 1, IOPRIO_CLASS_IDLE = 3 in the top bits (13 up).
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 19);
        libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13);
    }
    #[cfg(windows)]
    crate::platform::win::background_thread();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nfsu2-read-ahead-{tag}-{}", crate::unique_tag()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A file is read from start to end, whatever its size against the
    /// piece; a missing one is counted and skipped.
    #[test]
    fn a_file_is_read_whole_and_a_missing_one_skipped() {
        let d = temp("whole");
        let p = d.join("a.bin");
        let len = PIECE * 2 + 1234;
        std::fs::write(&p, vec![7u8; len]).unwrap();
        let mut buf = vec![0u8; PIECE];
        let (read, resident, _) = read_file(&p, &mut buf);
        assert_eq!(read.unwrap() + resident, len as u64);
        assert_eq!(read_file(&d.join("missing.bin"), &mut buf).0, None);
        let empty = d.join("empty.bin");
        std::fs::write(&empty, b"").unwrap();
        let (read, resident, _) = read_file(&empty, &mut buf);
        assert_eq!((read, resident), (Some(0), 0));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A file just written is in memory (Linux): nothing of it is read
    /// again.
    #[cfg(target_os = "linux")]
    #[test]
    fn pages_in_memory_are_not_read_again() {
        let d = temp("resident");
        let p = d.join("b.bin");
        let len = PIECE * 3;
        std::fs::write(&p, vec![1u8; len]).unwrap();
        // Read once through the page cache: every page is resident now.
        let _ = std::fs::read(&p).unwrap();
        let f = std::fs::File::open(&p).unwrap();
        let r = Residency::of(&f, len as u64).expect("mincore");
        assert!(r.all(0, len as u64));
        let mut buf = vec![0u8; PIECE];
        let (read, resident, _) = read_file(&p, &mut buf);
        assert_eq!((read, resident), (Some(0), len as u64));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The reader gives way while a hold lives and while a frame runs
    /// longer than [`BUSY_FRAME`], and goes on after.
    #[test]
    fn the_reader_gives_way_to_holds_and_long_frames() {
        // The process-wide state: this is the only test that holds or
        // starts frames, and it leaves no frame behind (the other tests'
        // reads only wait while it runs).
        let h = hold();
        assert!(give_way());
        let h2 = hold();
        drop(h);
        assert!(give_way(), "one hold is still held");
        drop(h2);
        frame_started();
        assert!(!give_way(), "a frame that just started");
        std::thread::sleep(BUSY_FRAME + Duration::from_millis(20));
        assert!(give_way(), "a frame that runs long");
        frame_started();
        assert!(!give_way());
        // A held reader waits for the hold to go.
        let h = hold();
        let waited = std::thread::spawn(wait_turn);
        std::thread::sleep(Duration::from_millis(60));
        frame_started();
        drop(h);
        let waited = waited.join().unwrap();
        FRAME.store(0, Ordering::Release);
        assert!(waited >= Duration::from_millis(50), "{waited:?}");
    }

    /// Queued files are read in the order queued, each once a run, by one
    /// thread that ends with the queue.
    #[test]
    fn queued_files_are_read_once_in_order() {
        let d = temp("queue");
        let files: Vec<PathBuf> = (0..4).map(|i| d.join(format!("{i}.bin"))).collect();
        for (i, f) in files.iter().enumerate() {
            std::fs::write(f, vec![i as u8; 1000 + i]).unwrap();
        }
        let before = stats();
        queue(files.clone());
        queue(files.clone());
        queue([d.join("gone.bin")]);
        let t = Instant::now();
        while busy() && t.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let after = stats();
        assert!(!busy());
        if enabled() {
            assert_eq!(after.files - before.files, 4, "each file once");
            assert_eq!(after.missing - before.missing, 1);
            let total: u64 = (0..4).map(|i| 1000 + i as u64).sum();
            assert_eq!((after.read + after.resident) - (before.read + before.resident), total);
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
