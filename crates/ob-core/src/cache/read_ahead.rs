//! The files the next minutes need, read ahead into the system's file
//! cache (docs/developing/read-ahead.md). After a reboot the game's files are on the
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
    give_way_at(epoch().elapsed().as_nanos())
}

/// [`give_way`] at `now` (ns since [`epoch`]).
fn give_way_at(now: u128) -> bool {
    if HOLDS.load(Ordering::Acquire) > 0 {
        return true;
    }
    let started = FRAME.load(Ordering::Acquire);
    started != 0 && now.saturating_sub(u128::from(started)) > BUSY_FRAME.as_nanos()
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
    // A game file the asset layer holds lives in a bundle of the player's own copy: read it there, never the game
    // folder (which may be gone, and which the layer answers for).
    match crate::game_fs::peek(path) {
        crate::game_fs::Lookup::Held(m) => return read_mapped(&m, held),
        crate::game_fs::Lookup::Absent => return (None, 0, held),
        crate::game_fs::Lookup::Folder => {}
    }
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

/// A bundle's mapped bytes brought into memory piece by piece, as [`read_file`] does a file: the pieces
/// already in memory skipped (Linux: `mincore`), each other piece asked for in advance (`madvise`) and then
/// touched, a byte a page, so that the time it took is the disk's.
fn read_mapped(m: &[u8], mut held: Duration) -> (Option<u64>, u64, Duration) {
    let (mut at, mut read, mut resident) = (0usize, 0u64, 0u64);
    while at < m.len() {
        let n = (m.len() - at).min(PIECE);
        let piece = &m[at..at + n];
        if mapped_resident(piece) {
            resident += n as u64;
            at += n;
            continue;
        }
        held += wait_turn();
        let t = Instant::now();
        mapped_advise(piece);
        let mut sum = 0u8;
        for p in piece.iter().step_by(4096) {
            sum = sum.wrapping_add(*p);
        }
        std::hint::black_box(sum);
        read += n as u64;
        at += n;
        let took = t.elapsed();
        if took > SLOW_PIECE {
            let pause = took.min(SLOW_PAUSE);
            std::thread::sleep(pause);
            held += pause;
        }
    }
    (Some(read), resident, held)
}

/// The page range (aligned start, length) of a mapped piece.
#[cfg(target_os = "linux")]
fn page_span(piece: &[u8]) -> Option<(*mut libc::c_void, usize)> {
    // SAFETY: plain sysconf.
    let page = usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok().filter(|&p| p > 0)?;
    let start = piece.as_ptr() as usize;
    let a = start & !(page - 1);
    Some((a as *mut libc::c_void, start + piece.len() - a))
}

/// Whether every page of a mapped piece is in memory.
#[cfg(target_os = "linux")]
fn mapped_resident(piece: &[u8]) -> bool {
    let Some((a, len)) = page_span(piece) else { return false };
    // SAFETY: `a..a+len` lies inside the bundle's mapping (the piece's pages) and the vector holds a byte
    // per page (sized for 4 KiB pages, enough for any larger one); mincore only reports.
    let mut pages = vec![0u8; len.div_ceil(4096) + 1];
    let ok = unsafe { libc::mincore(a, len, pages.as_mut_ptr().cast()) } == 0;
    ok && pages[..len.div_ceil(page_size()).min(pages.len())].iter().all(|&v| v & 1 == 1)
}

#[cfg(target_os = "linux")]
fn page_size() -> usize {
    // SAFETY: plain sysconf.
    usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok().filter(|&p| p > 0).unwrap_or(4096)
}

/// Ask the system to read a mapped piece in advance.
#[cfg(target_os = "linux")]
fn mapped_advise(piece: &[u8]) {
    if let Some((a, len)) = page_span(piece) {
        // SAFETY: the pages of a live mapping; WILLNEED only starts reading them.
        unsafe {
            libc::madvise(a, len, libc::MADV_WILLNEED);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn mapped_resident(_piece: &[u8]) -> bool {
    false
}

#[cfg(not(target_os = "linux"))]
fn mapped_advise(_piece: &[u8]) {}

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

    /// A game file the asset layer holds is read ahead from its bundle (the game folder has no such file), a name
    /// the layer knows is absent is counted missing, and nothing is counted as a game read.
    #[test]
    fn a_layered_file_is_read_ahead_from_its_bundle() {
        use crate::game_fs;
        let _mount = game_fs::TEST_MOUNT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        struct One(std::sync::Arc<memmap2::Mmap>);
        impl game_fs::Layer for One {
            fn get(&self, key: &str) -> Option<game_fs::Mapped> {
                (key == "TRACKS/A.BIN").then(|| game_fs::Mapped::new(self.0.clone(), 100..PIECE * 2 + 5000).unwrap())
            }
            fn absent(&self, key: &str) -> bool {
                key == "TRACKS/GONE.BIN"
            }
            fn name(&self) -> &str {
                "unit-layer"
            }
        }
        let mut m = memmap2::MmapMut::map_anon(PIECE * 3).unwrap();
        m.fill(3);
        let root = temp("layered");
        game_fs::mount(&root, std::sync::Arc::new(One(std::sync::Arc::new(m.make_read_only().unwrap()))));
        let before = game_fs::counts();
        let mut buf = vec![0u8; PIECE];
        let (read, resident, _) = read_file(&root.join("TRACKS").join("A.BIN"), &mut buf);
        assert_eq!(read.unwrap() + resident, (PIECE * 2 + 4900) as u64);
        assert_eq!(read_file(&root.join("TRACKS").join("GONE.BIN"), &mut buf).0, None);
        assert_eq!(game_fs::counts(), before, "read-ahead is not a game read");
        game_fs::unmount();
        let _ = std::fs::remove_dir_all(&root);
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
        // A frame's age is judged against a given time, not the clock
        // (a loaded machine can stall a test thread for longer than a
        // frame between two calls).
        let started = 1_000_000_000u64;
        FRAME.store(started, Ordering::Release);
        let at = |after: Duration| u128::from(started) + after.as_nanos();
        assert!(!give_way_at(at(Duration::ZERO)), "a frame that just started");
        assert!(!give_way_at(at(BUSY_FRAME)), "a frame of just the busy length");
        assert!(give_way_at(at(BUSY_FRAME + Duration::from_millis(1))), "a frame that runs long");
        frame_started();
        assert!(!give_way_at(u128::from(FRAME.load(Ordering::Acquire))), "the next frame started");
        FRAME.store(0, Ordering::Release);
        assert!(!give_way_at(u128::MAX), "no frame yet");
        // A held reader waits for the hold to go: it is still waiting
        // after it has been seen to enter the wait, and ends once the
        // hold is dropped (no frame is running then).
        let h = hold();
        let (entered, seen) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            entered.send(()).unwrap();
            wait_turn()
        });
        seen.recv().unwrap();
        std::thread::sleep(Duration::from_millis(60));
        assert!(!waiter.is_finished(), "the reader waits while the hold lives");
        drop(h);
        waiter.join().unwrap();
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
