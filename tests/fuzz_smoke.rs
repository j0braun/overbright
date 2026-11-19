//! Fuzz smoke test (docs/developing/fuzzing.md): every target of `fuzz/targets.rs`
//! (the same functions cargo-fuzz drives) gets mutated inputs from a
//! deterministic engine of its own (seeded xorshift), over seeds taken from
//! the player's game files at test time (nothing is committed). A target
//! must not panic, must return within a time bound and must not allocate
//! more than `Limits::max_alloc` in one request.
//!
//! Environment: `FUZZ_SECS` (seconds per target, default 1.0), `FUZZ_ITERS`
//! (inputs per target, default 3000), `FUZZ_SEED`, `FUZZ_ONLY=name[,name]`,
//! `FUZZ_REPLAY=n` (skip running the first n inputs: reproduce one),
//! `FUZZ_DUMP=dir` (write failing inputs there).

mod common;
#[path = "../fuzz/targets.rs"]
mod targets;

use nfsu2::formats::bundle::{self, Chunk};
use nfsu2::formats::reader::Limits;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use targets::{Seed, TARGETS, Target};

// ---- the allocator: the largest request while tracking; a request that no
// ---- parser can need is refused with the replay line printed first -------

struct Peak;
static TRACK: AtomicBool = AtomicBool::new(false);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CUR_TARGET: AtomicUsize = AtomicUsize::new(usize::MAX);
static CUR_ITER: AtomicUsize = AtomicUsize::new(0);
/// Beyond this one request is refused outright (the process aborts, naming
/// the target and the input number): real memory is not worth a finding.
const REFUSE: usize = 1 << 30;

fn note(n: usize) {
    if TRACK.load(Ordering::Relaxed) {
        PEAK.fetch_max(n, Ordering::Relaxed);
        if n > REFUSE {
            let t = CUR_TARGET.load(Ordering::Relaxed);
            let name = TARGETS.get(t).map_or("?", |t| t.name);
            // No allocation here: a fixed buffer through write(2).
            let mut buf = [0u8; 160];
            let msg = format_noalloc(&mut buf, name, n, CUR_ITER.load(Ordering::Relaxed));
            #[cfg(unix)]
            unsafe {
                libc::write(2, msg.as_ptr().cast(), msg.len())
            };
            #[cfg(not(unix))]
            {
                use std::io::Write;
                // SAFETY: `msg` points into `buf` for `msg.len()` bytes.
                let bytes = unsafe { std::slice::from_raw_parts(msg.as_ptr().cast::<u8>(), msg.len()) };
                let _ = std::io::stderr().write_all(bytes);
            }
        }
    }
}

/// "FUZZ-ALLOC target=<name> bytes=<n> input=<i>\n" into `buf`.
fn format_noalloc<'a>(buf: &'a mut [u8; 160], name: &str, n: usize, i: usize) -> &'a [u8] {
    let mut len = 0;
    let mut put = |s: &[u8]| {
        for &b in s {
            if len < buf.len() {
                buf[len] = b;
                len += 1;
            }
        }
    };
    let num = |mut v: usize, put: &mut dyn FnMut(&[u8])| {
        let mut d = [0u8; 20];
        let mut k = 20;
        loop {
            k -= 1;
            d[k] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        put(&d[k..]);
    };
    put(b"FUZZ-ALLOC target=");
    put(name.as_bytes());
    put(b" bytes=");
    num(n, &mut put);
    put(b" input=");
    num(i, &mut put);
    put(b"\n");
    &buf[..len]
}

unsafe impl std::alloc::GlobalAlloc for Peak {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        note(l.size());
        if l.size() > REFUSE && TRACK.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        note(l.size());
        if l.size() > REFUSE && TRACK.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        note(n);
        if n > REFUSE && TRACK.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Peak = Peak;

// ---- the watchdog: a call that spends 30 s of CPU in one call is a hang -----
// Measured on the calling thread's own CPU clock, so a call is not judged hung
// because a busy machine (the gates beside it) parked the thread: under load a
// big archive input once crossed 30 s of wall time. A call that blocks without
// spending CPU (a deadlock) is caught on the wall clock at ten times that.

static CALL_START_MS: AtomicU64 = AtomicU64::new(0);
/// The calling thread's CPU clock (`pthread_getcpuclockid`; -1: none) and
/// its reading at the call's start (ns).
static CALL_CLOCK: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(-1);
static CALL_CPU_START_NS: AtomicU64 = AtomicU64::new(0);
static EPOCH: OnceLock<Instant> = OnceLock::new();
const HANG_CPU_MS: u64 = 30_000;
const HANG_WALL_MS: u64 = 10 * HANG_CPU_MS;

/// The calling thread's CPU clock id, where the platform has one.
fn own_cpu_clock() -> i64 {
    #[cfg(unix)]
    {
        let mut id: libc::clockid_t = 0;
        // SAFETY: `id` is a valid out-pointer for the calling thread's clock.
        if unsafe { libc::pthread_getcpuclockid(libc::pthread_self(), &mut id) } == 0 {
            return i64::from(id);
        }
    }
    -1
}

/// A CPU clock's reading (ns), None when it cannot be read.
fn cpu_clock_ns(id: i64) -> Option<u64> {
    #[cfg(unix)]
    if id >= 0 {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: `ts` is a valid timespec for the call to fill.
        if unsafe { libc::clock_gettime(id as libc::clockid_t, &mut ts) } == 0 {
            return Some(ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64);
        }
    }
    let _ = id;
    None
}

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

fn watchdog() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let s = CALL_START_MS.load(Ordering::Relaxed);
                if s == 0 {
                    continue;
                }
                let wall = now_ms().saturating_sub(s);
                // The call's own CPU time; the wall clock where the
                // thread's clock cannot be read.
                let cpu = cpu_clock_ns(CALL_CLOCK.load(Ordering::Relaxed))
                    .map(|now| now.saturating_sub(CALL_CPU_START_NS.load(Ordering::Relaxed)) / 1_000_000);
                let why = match cpu {
                    Some(c) if c > HANG_CPU_MS => Some(format!("{} s of CPU in one call", c / 1000)),
                    None if wall > HANG_CPU_MS => Some(format!("{} s in one call", wall / 1000)),
                    _ if wall > HANG_WALL_MS => Some(format!("{} s in one call without returning", wall / 1000)),
                    _ => None,
                };
                if let Some(why) = why {
                    let t = CUR_TARGET.load(Ordering::Relaxed);
                    let _ = std::io::Write::write_all(
                        &mut std::io::stderr(),
                        format!("FUZZ-HANG target={} input={} ({why})\n", TARGETS.get(t).map_or("?", |t| t.name), CUR_ITER.load(Ordering::Relaxed)).as_bytes(),
                    );
                    std::process::abort();
                }
            }
        });
    });
}

// ---- the mutation engine ---------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

/// The chunk headers (offset of the size field, body length) of `b` read as
/// a chunk tree from offset `skip`.
fn headers(b: &[u8], skip: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    if b.len() > 1 << 20 || skip > b.len() {
        return v;
    }
    bundle::walk(bundle::chunks(&b[skip..], skip & 0x7F), &mut |c: Chunk| v.push((skip + c.offset - (skip & 0x7F) - 4, c.body.len())));
    v
}

const B32: [u32; 14] = [0, 1, 2, 3, 0x7F, 0x80, 0xFF, 0xFFFF, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFE, 0xFFFF_FFFF, 0x0100_0000, 0x0001_0000];
const B16: [u16; 8] = [0, 1, 0x7F, 0x80, 0xFF, 0x7FFF, 0x8000, 0xFFFF];

/// One to four mutations of `seed`.
fn mutate(seed: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut b = seed.to_vec();
    for _ in 0..1 + rng.below(4) {
        let len = b.len();
        match rng.below(12) {
            // Bit flips.
            0 | 1 if len > 0 => {
                for _ in 0..1 + rng.below(6) {
                    let at = rng.below(len);
                    b[at] ^= 1 << rng.below(8);
                }
            }
            // Bytes set to 0 / 0xFF / 0x80.
            2 if len > 0 => {
                let v = [0u8, 0xFF, 0x80][rng.below(3)];
                for _ in 0..1 + rng.below(4) {
                    let at = rng.below(len);
                    b[at] = v;
                }
            }
            // A 32-bit field to a boundary value (any alignment, or aligned).
            3 | 4 if len >= 4 => {
                let at = if rng.below(2) == 0 { rng.below(len - 3) } else { rng.below(len / 4) * 4 };
                let v = if rng.below(4) == 0 { (len as u32).wrapping_add(rng.below(9) as u32).wrapping_sub(4) } else { B32[rng.below(B32.len())] };
                b[at..at + 4].copy_from_slice(&v.to_le_bytes());
                if rng.below(4) == 0 {
                    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
                }
            }
            // A 16-bit field.
            5 if len >= 2 => {
                let at = rng.below(len - 1);
                let v = B16[rng.below(B16.len())];
                b[at..at + 2].copy_from_slice(&if rng.below(4) == 0 { v.to_be_bytes() } else { v.to_le_bytes() });
            }
            // Truncation.
            6 if len > 0 => b.truncate(rng.below(len)),
            // Block duplication (a block copied over or inserted elsewhere).
            7 if len > 8 => {
                let n = 1 + rng.below(len.min(256));
                let from = rng.below(len - n + 1);
                let blk = b[from..from + n].to_vec();
                let to = rng.below(len);
                if rng.below(2) == 0 {
                    let end = (to + n).min(len);
                    b[to..end].copy_from_slice(&blk[..end - to]);
                } else {
                    b.splice(to..to, blk);
                }
            }
            // Length-field inflation: a small u32 or u16 grows.
            8 | 9 if len >= 4 => {
                for _ in 0..32 {
                    let at = rng.below(len - 3);
                    let v = u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
                    if v != 0 && v as usize <= len {
                        let g = match rng.below(4) {
                            0 => v << (1 + rng.below(20) as u32).min(31 - v.leading_zeros().min(31)),
                            1 => v.wrapping_add(len as u32),
                            2 => v.wrapping_mul(0x10001),
                            _ => 0x4000_0000 | v,
                        };
                        b[at..at + 4].copy_from_slice(&g.to_le_bytes());
                        break;
                    }
                }
            }
            // A chunk header's size: lying (past the parent, huge, zero, odd).
            10 => {
                let h = headers(&b, rng.below(2));
                if let Some(&(at, body)) = h.get(rng.below(h.len())) {
                    let v = match rng.below(3) {
                        0 => B32[rng.below(B32.len())],
                        1 => (body as u32).wrapping_add(rng.below(64) as u32).wrapping_sub(32),
                        _ => (len as u32).wrapping_sub(at as u32).wrapping_add(rng.below(8) as u32),
                    };
                    if at + 4 <= b.len() {
                        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
                    }
                }
            }
            // Grown with zeros or junk.
            _ => {
                for _ in 0..rng.below(48) {
                    b.push(if rng.below(2) == 0 { 0 } else { rng.next() as u8 });
                }
            }
        }
    }
    b
}

// ---- running one input -----------------------------------------------------

thread_local! {
    static QUIET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static WHERE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn quiet_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !QUIET.with(|q| q.get()) {
                default(info);
            } else {
                let mut at = info.location().map_or_else(String::new, |l| format!("{}:{}", l.file(), l.line()));
                // A panic in a shared helper: the first caller in the crate too.
                if at.contains("bundle.rs") || at.contains("reader.rs") || at.contains("library/") {
                    let bt = std::backtrace::Backtrace::force_capture().to_string();
                    if let Some(f) = bt.lines().find(|l| (l.contains("at ./src/") || l.contains("at src/")) && !l.contains("bundle.rs") && !l.contains("reader.rs")) {
                        at += &format!(" via {}", f.trim());
                    }
                }
                WHERE.with(|w| *w.borrow_mut() = at);
            }
        }));
    });
}

fn call_limit() -> Duration {
    Duration::from_millis(std::env::var("FUZZ_CALL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(3000))
}

/// The time this thread has spent on the CPU (unix), so a call is not judged
/// slow because a busy machine parked the thread: under a loaded gate a 64-byte
/// input once "took" 3.07 s of wall time. A real hang is still caught on the
/// wall clock by the watchdog. Elsewhere: the wall clock.
fn thread_time() -> Duration {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: `ts` is a valid timespec for the call to fill.
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) } == 0 {
            return Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32);
        }
    }
    EPOCH.get_or_init(Instant::now).elapsed()
}

/// One input through one target: `Err` on a panic, a slow call, or a
/// request above `Limits::max_alloc`.
fn run_one(t: &Target, input: &[u8]) -> Result<(), String> {
    quiet_hook();
    watchdog();
    PEAK.store(0, Ordering::Relaxed);
    QUIET.with(|q| q.set(true));
    TRACK.store(true, Ordering::Relaxed);
    let clock = own_cpu_clock();
    CALL_CLOCK.store(clock, Ordering::Relaxed);
    CALL_CPU_START_NS.store(cpu_clock_ns(clock).unwrap_or(0), Ordering::Relaxed);
    CALL_START_MS.store(now_ms(), Ordering::Relaxed);
    let start = thread_time();
    let r = catch_unwind(AssertUnwindSafe(|| (t.run)(input)));
    let took = thread_time().saturating_sub(start);
    CALL_START_MS.store(0, Ordering::Relaxed);
    TRACK.store(false, Ordering::Relaxed);
    QUIET.with(|q| q.set(false));
    let peak = PEAK.load(Ordering::Relaxed);
    match r {
        Err(e) => Err(format!(
            "panic at {}: {}",
            WHERE.with(|w| w.borrow().clone()),
            e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_else(|| "(panic)".into())
        )),
        Ok(()) if took > call_limit() => Err(format!("slow: {took:?} for {} bytes", input.len())),
        Ok(()) if peak > Limits::DEFAULT.max_alloc => Err(format!("allocated {peak} bytes in one request ({} byte input)", input.len())),
        Ok(()) => Ok(()),
    }
}

// ---- seeds -----------------------------------------------------------------

fn files() -> &'static Mutex<HashMap<String, std::sync::Arc<Vec<u8>>>> {
    static F: OnceLock<Mutex<HashMap<String, std::sync::Arc<Vec<u8>>>>> = OnceLock::new();
    F.get_or_init(Default::default)
}

/// A game file (JDLZ unpacked), up to 160 MB.
fn read(rel: &str) -> std::sync::Arc<Vec<u8>> {
    if let Some(b) = files().lock().unwrap().get(rel) {
        return b.clone();
    }
    let root = common::game_root();
    let p = nfsu2::formats::viv::find_ci(&root, rel).unwrap_or_else(|| panic!("seed file {rel} is not in the game"));
    let mut b = Vec::new();
    {
        use std::io::Read;
        std::fs::File::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())).take(160 << 20).read_to_end(&mut b).unwrap();
    }
    if b.starts_with(nfsu2::formats::jdlz::MAGIC) {
        b = nfsu2::formats::jdlz::decompress(&b).unwrap_or_else(|e| panic!("{rel}: {e}"));
    }
    let a = std::sync::Arc::new(b);
    files().lock().unwrap().insert(rel.to_string(), a.clone());
    a
}

const MAX_SEED: usize = 512 << 10;

/// Up to `n` chunks spread over the size range, smallest first.
fn pick<T: Clone>(mut all: Vec<(usize, T)>, n: usize) -> Vec<T> {
    all.sort_by_key(|(s, _)| *s);
    all.dedup_by_key(|(s, _)| *s);
    if all.len() <= n {
        return all.into_iter().map(|(_, t)| t).collect();
    }
    (0..n).map(|i| all[i * (all.len() - 1) / (n - 1)].1.clone()).collect()
}

fn seeds_of(t: &Target) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = vec![Vec::new(), vec![0u8; 64], (0..255u8).collect()];
    // A chunk-shaped input for the chunk targets that finds no real seed.
    let mut c = vec![0u8];
    c.extend(0x8003_4100u32.to_le_bytes());
    c.extend(40u32.to_le_bytes());
    c.extend((0..40u8).collect::<Vec<_>>());
    out.push(c);
    for s in t.seeds {
        out.extend(match *s {
            Seed::Chunks(file, ids) => {
                let d = read(file);
                let mut all = Vec::new();
                bundle::walk(bundle::chunks(&d, 0), &mut |c: Chunk| {
                    if ids.contains(&c.id) && c.body.len() >= 16 && c.body.len() + 9 <= MAX_SEED {
                        let at = c.offset - 8;
                        let mut v = vec![(at & 0x7F) as u8];
                        v.extend_from_slice(&d[at..c.offset + c.body.len()]);
                        all.push((c.body.len(), v));
                    }
                });
                pick(all, 5)
            }
            Seed::Concat(file, ids) => {
                let d = read(file);
                let mut out = Vec::new();
                for &want in ids {
                    let mut got = None;
                    bundle::walk(bundle::chunks(&d, 0), &mut |c: Chunk| {
                        if c.id == want && got.is_none() {
                            got = Some(d[c.offset - 8..c.offset + c.body.len()].to_vec());
                        }
                    });
                    out.extend(got.unwrap_or_else(|| panic!("no chunk {want:#x} in {file}")));
                    while out.len() % 16 != 0 {
                        out.push(0);
                    }
                }
                vec![out]
            }
            Seed::File(file) => {
                let d = read(file);
                let mut v = vec![d[..d.len().min(MAX_SEED * 4)].to_vec()];
                if d.len() > MAX_SEED * 4 || file.ends_with("sdat.viv") {
                    v = vec![d[..d.len().min(0x10000)].to_vec(), d[..d.len().min(MAX_SEED)].to_vec()];
                }
                v
            }
            Seed::Viv(file, magic) => {
                let root = common::game_root();
                let p = nfsu2::formats::viv::find_ci(&root, file).unwrap_or_else(|| panic!("{file} is not in the game"));
                let viv = nfsu2::formats::viv::Viv::open(&p).unwrap_or_else(|| panic!("{file} does not open"));
                let mut all = Vec::new();
                for e in &viv.entries {
                    if let Some(h) = viv.read_range(e, 0, 4096)
                        && h.starts_with(magic)
                        && let Some(d) = viv.read_range(e, 0, (e.size as usize).min(MAX_SEED / 2))
                    {
                        all.push((d.len(), d));
                    }
                }
                pick(all, 6)
            }
            Seed::Block(file, magic) => {
                let d = read(file);
                let mut all = Vec::new();
                bundle::walk(bundle::chunks(&d, 0), &mut |c: Chunk| {
                    if c.id != bundle::id::TEXTURE_PACK {
                        return;
                    }
                    let Some(blocks) = c.child(bundle::id::TPK_HEADER).and_then(|h| h.child(bundle::id::TPK_COMPRESSED_BLOCKS)) else { return };
                    for r in blocks.body.chunks_exact(0x18) {
                        let u = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap()) as usize;
                        let Some(start) = u(4).checked_sub(c.offset).and_then(|s| s.checked_sub(0)) else { continue };
                        if let Some(block) = c.body.get(start.saturating_sub(0)..(start + u(8)).min(c.body.len()))
                            && block.starts_with(magic)
                            && block.len() < MAX_SEED
                        {
                            all.push((block.len(), block.to_vec()));
                        }
                    }
                });
                pick(all, 4)
            }
            Seed::Top(file) => {
                let d = read(file);
                let all: Vec<_> = bundle::chunks(&d, 0).take(2000).filter(|c| c.body.len() + 8 <= MAX_SEED).map(|c| (c.body.len(), d[c.offset - 8..c.offset + c.body.len()].to_vec())).collect();
                pick(all, 6)
            }
            Seed::Bodies(file, ids) => {
                let d = read(file);
                let mut all = Vec::new();
                bundle::walk(bundle::chunks(&d, 0), &mut |c: Chunk| {
                    if ids.contains(&c.id) && c.body.len() < MAX_SEED {
                        all.push((c.body.len(), c.body.to_vec()));
                        if let Some(at) = c.body.windows(4).take(32).position(|w| w == b"JDLZ" || w == b"HUFF") {
                            let p = &c.body[at..];
                            let u = if p.starts_with(b"JDLZ") { nfsu2::formats::jdlz::decompress(p) } else { nfsu2::formats::huff::decompress(p) };
                            if let Ok(u) = u
                                && u.len() < MAX_SEED
                            {
                                all.push((u.len(), u));
                            }
                        }
                    }
                });
                pick(all, 6)
            }
            Seed::Text(kind) => text_seeds(kind),
        });
    }
    out
}

/// A one-entry stored ZIP (no game sample exists for `.tpf`, a mod format).
fn zip() -> Vec<u8> {
    let body = b"texture bytes".to_vec();
    let crc = nfsu2::formats::tpf::zip_crc(&body);
    let name = b"SPEED2.EXE_0x12345678.dds";
    let mut z = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    z.extend_from_slice(b"PK\x03\x04");
    for x in [20u16, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, crc);
    le32(&mut z, body.len() as u32);
    le32(&mut z, body.len() as u32);
    le16(&mut z, name.len() as u16);
    le16(&mut z, 0);
    z.extend_from_slice(name);
    z.extend_from_slice(&body);
    let cd = z.len();
    z.extend_from_slice(b"PK\x01\x02");
    for x in [20u16, 20, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, crc);
    le32(&mut z, body.len() as u32);
    le32(&mut z, body.len() as u32);
    for x in [name.len() as u16, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, 0);
    le32(&mut z, 0);
    z.extend_from_slice(name);
    let cd_len = z.len() - cd;
    z.extend_from_slice(b"PK\x05\x06");
    for x in [0u16, 0, 1, 1] {
        le16(&mut z, x);
    }
    le32(&mut z, cd_len as u32);
    le32(&mut z, cd as u32);
    le16(&mut z, 0);
    z
}

fn text_seeds(kind: &str) -> Vec<Vec<u8>> {
    match kind {
        "tpf" => vec![zip(), b"0x12345678|SPEED2.EXE_0x12345678.dds\n0xABCDEF01|x.dds\n".to_vec()],
        "texwizard" => vec![
            br#"{ "textures": [ [ "ROAD", "_ROAD" ], [ "0x1234ABCD", "0xBEEF" ], ], // a comment
                 "links": [ { "path": "TRACKS\\L4RA.BUN", "textures": [ "ROAD" ] } ] }"#
                .to_vec(),
        ],
        "save" => {
            let mut v = vec![nfsu2::frontend::career::fresh_save("FUZZ").to_bytes(), nfsu2::formats::native_save::picture(&[128; 16 * 16 * 4], 16, 16)];
            // The original's save of a finished career, with the private set (OB_PRIVATE).
            if cfg!(ob_private) {
                v.push(common::private("tests/fixtures/native_saves/career_complete.sav").to_vec());
            }
            v
        }
        "options" => {
            let mut s = nfsu2::frontend::options::Settings::default();
            s.enh.parse("enh.hud_scale", "120");
            vec![s.options_text().into_bytes()]
        }
        "profile" => vec![b"name FUZZ\ncash 1200\ncar COROLLA\nstage 2\nreputation 40\nwon 5\nsponsor 1 2\nsms 1 0 0 0 0\nsms 5 1 1 0 0 7 8 9\n".to_vec()],
        "obpack" => {
            let mut dds = vec![0u8; 148 + 16];
            dds[..4].copy_from_slice(b"DDS ");
            for (i, v) in [(1usize, 124u32), (2, 0x2_1007), (3, 4), (4, 4), (7, 1), (20, 4), (32, 98), (33, 3), (35, 1)] {
                dds[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            dds[84..88].copy_from_slice(b"DX10");
            vec![
                b"format = 1\nname = \"Roads\"\nauthor = \"A\"\nlicence = \"CC0-1.0\"\npriority = 5\ncompress = \"none\"\n".to_vec(),
                b"# Overbright packs v1\non roads\noff signs\n".to_vec(),
                dds,
            ]
        }
        "dds" => {
            let mut d = vec![0u8; 128 + 8 * 4];
            d[..4].copy_from_slice(b"DDS ");
            d[4..8].copy_from_slice(&124u32.to_le_bytes());
            d[8..12].copy_from_slice(&0x1007u32.to_le_bytes());
            d[12..16].copy_from_slice(&8u32.to_le_bytes());
            d[16..20].copy_from_slice(&8u32.to_le_bytes());
            d[80..84].copy_from_slice(&4u32.to_le_bytes());
            d[84..88].copy_from_slice(b"DXT1");
            vec![d]
        }
        "vp6" => {
            if cfg!(ob_game) {
                let m = nfsu2::movie::Movie::parse(read("MOVIES/blank.vp6").to_vec()).expect("blank.vp6");
                m.frames.iter().take(4).map(|(_, c)| m.payload(c).to_vec()).collect()
            } else {
                Vec::new()
            }
        }
        other => panic!("unknown seed text {other}"),
    }
}

// ---- the driver ------------------------------------------------------------

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn name_hash(s: &str) -> u64 {
    s.bytes().fold(0xCBF2_9CE4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01B3))
}

/// Run every selected target for its budget; failures are listed at the end.
fn fuzz(real: bool, default_secs: f64) {
    // One run at a time: the allocator's peak, the current target and the
    // watchdog are process-wide, and the first call of some targets builds
    // shared tables (`OnceLock`) that another thread would wait for.
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let secs: f64 = env("FUZZ_SECS", default_secs);
    let iters: usize = env("FUZZ_ITERS", 200_000);
    let seed: u64 = env("FUZZ_SEED", 0x5EED_F022);
    let replay: usize = env("FUZZ_REPLAY", 0);
    let only = std::env::var("FUZZ_ONLY").ok();
    let dump = std::env::var("FUZZ_DUMP").ok();
    let mut failures = Vec::new();
    let mut total = 0usize;
    let mut report = Vec::new();
    for (ti, t) in TARGETS.iter().enumerate() {
        if only.as_deref().is_some_and(|o| !o.split(',').any(|n| n == t.name)) {
            continue;
        }
        CUR_TARGET.store(ti, Ordering::Relaxed);
        let seeds: Vec<Vec<u8>> = if real {
            seeds_of(t)
        } else {
            // Without the game: the built-in seeds of the target only.
            let mut s = seeds_of(&Target { name: t.name, seeds: &[], run: t.run });
            for seed in t.seeds {
                if let Seed::Text(k) = seed {
                    s.extend(text_seeds(k));
                }
            }
            s
        };
        let mut n = 0usize;
        let mut fails = 0usize;
        // The first call may build shared tables (not timed).
        let _ = catch_unwind(AssertUnwindSafe(|| (t.run)(&[])));
        for (k, s) in seeds.iter().enumerate() {
            CUR_ITER.store(usize::MAX, Ordering::Relaxed);
            if let Err(e) = run_one(t, s) {
                fails += 1;
                failures.push(format!("{}: seed {k} ({} bytes) as is: {e}", t.name, s.len()));
            }
        }
        let mut rng = Rng(seed ^ name_hash(t.name) | 1);
        let start = Instant::now();
        for i in 0..iters {
            if i >= replay && start.elapsed().as_secs_f64() > secs && replay == 0 {
                break;
            }
            let k = rng.below(seeds.len());
            let input = mutate(&seeds[k], &mut rng);
            if i < replay {
                continue;
            }
            CUR_ITER.store(i, Ordering::Relaxed);
            n += 1;
            if let Err(e) = run_one(t, &input) {
                fails += 1;
                if failures.len() < 100 {
                    failures.push(format!("{}: input {i} (seed {k}, {} bytes): {e}", t.name, input.len()));
                }
                if let Some(d) = &dump {
                    let _ = std::fs::create_dir_all(d);
                    let _ = std::fs::write(format!("{d}/{}-{i}.bin", t.name), &input);
                }
                if fails >= 5 {
                    break;
                }
            }
        }
        total += n;
        report.push(format!("{} {n}", t.name));
    }
    CUR_TARGET.store(usize::MAX, Ordering::Relaxed);
    eprintln!("fuzz ({}): {total} mutated inputs over {} targets, {} failures\n  {}", if real { "game seeds" } else { "built-in seeds" }, report.len(), failures.len(), report.join(", "));
    assert!(failures.is_empty(), "{} fuzz failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn fuzz_smoke_builtin_seeds() {
    fuzz(false, 0.25);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn fuzz_smoke_game_seeds() {
    // Seeds need the game; the context some targets read as well.
    let _ = common::game_root();
    fuzz(true, 1.0);
}

/// Every target is listed once, named like its function, with a seed.
#[test]
fn every_target_is_registered_once() {
    let mut names: Vec<_> = TARGETS.iter().map(|t| t.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), TARGETS.len());
    assert!(TARGETS.iter().all(|t| !t.seeds.is_empty()));
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/fuzz_targets");
    for t in TARGETS {
        let f = format!("{dir}/{}.rs", t.name);
        let src = std::fs::read_to_string(&f).unwrap_or_else(|_| panic!("{f} is missing"));
        assert!(src.contains(&format!("targets::{}(", t.name)), "{f} does not call targets::{}", t.name);
    }
}

/// `Chunk::payload` strips 0x11 filler only up to the next
/// 128-byte file offset. Shipped bundles give the same bytes as the old
/// unbounded strip.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn payload_filler_ends_at_128() {
    let (mut chunks, mut stripped) = (0usize, 0usize);
    for f in [BUN_STREAM_FILE, "TRACKS/L4RA.BUN", "TRACKS/STREAML4RC.BUN", "GLOBAL/GLOBALB.BUN", "CARS/COROLLA/GEOMETRY.BIN", "CARS/MUSTANGGT/GEOMETRY.BIN", "FRONTEND/FrontB.lzc"] {
        let d = read(f);
        bundle::walk(bundle::chunks(&d, 0), &mut |c: Chunk| {
            let mut old = c.aligned(16);
            while let Some((g, rest)) = old.split_first_chunk::<16>()
                && g.iter().all(|&b| b == 0x11)
            {
                old = rest;
            }
            chunks += 1;
            stripped += (c.aligned(16).len() != old.len()) as usize;
            assert_eq!(c.payload(), old, "{f}: chunk {:#x} at {}", c.id, c.offset);
        });
    }
    eprintln!("payload: {chunks} chunks, {stripped} with filler, all equal");
}

const BUN_STREAM_FILE: &str = "TRACKS/STREAML4RA.BUN";

/// `FUZZ_INPUT=file FUZZ_ONLY=target`: one saved input (a `FUZZ_DUMP` file)
/// through one target, with its time (docs/developing/fuzzing.md).
#[test]
fn replay_one_input() {
    let (Ok(f), Ok(name)) = (std::env::var("FUZZ_INPUT"), std::env::var("FUZZ_ONLY")) else { return };
    let t = TARGETS.iter().find(|t| t.name == name).expect("FUZZ_ONLY names a target");
    let input = std::fs::read(&f).unwrap();
    let start = Instant::now();
    let r = run_one(t, &input);
    eprintln!("{name} {f}: {:?} in {:?}", r, start.elapsed());
    r.unwrap();
}
