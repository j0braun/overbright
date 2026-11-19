//! Timings on stderr when `NFSU2_TIMING` is set: `let _t = prof::span("x");`
//! prints how long the scope took when it ends (wall time, and the
//! calling thread's CPU time). `NFSU2_TRACE=file.json` writes the same
//! spans (and the bench's frame phases) as a Chrome trace (Perfetto /
//! chrome://tracing): begin, length, thread, thread CPU — the loads'
//! critical path across threads (docs/perf-gates.md). Both are off unless
//! set: a span is then one cached bool load.

use std::sync::OnceLock;
use std::time::Instant;

/// `NFSU2_TIMING` is set (the spans print).
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NFSU2_TIMING").is_some())
}

/// Spans are measured at all: `NFSU2_TIMING` or `NFSU2_TRACE`.
fn spans_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| enabled() || trace::on())
}

/// The calling thread's CPU time so far, in ns (Linux's schedstat; 0
/// elsewhere).
pub fn thread_cpu_ns() -> u64 {
    std::fs::read_to_string("/proc/thread-self/schedstat").ok().and_then(|s| s.split_whitespace().next()?.parse().ok()).unwrap_or(0)
}

pub struct Span(&'static str, Instant, u64);

impl Drop for Span {
    fn drop(&mut self) {
        let cpu = thread_cpu_ns().saturating_sub(self.2);
        if enabled() {
            eprintln!("[time] {}: {:.1} ms (thread cpu {:.1} ms)", self.0, self.1.elapsed().as_secs_f64() * 1000.0, cpu as f64 / 1e6);
        }
        trace::complete(self.0, self.1, self.1.elapsed(), Some(cpu));
    }
}

pub fn span(label: &'static str) -> Option<Span> {
    spans_on().then(|| Span(label, Instant::now(), thread_cpu_ns()))
}

/// The Chrome trace (`NFSU2_TRACE=file.json`): one "complete" event per
/// span, written as it ends (JSON array format; the closing bracket is
/// optional in that format, so a run that stops early still loads), and
/// each thread's name once.
pub mod trace {
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    struct Sink {
        out: std::io::BufWriter<std::fs::File>,
        named: Vec<u64>,
    }

    fn sink() -> &'static Option<(Instant, Mutex<Sink>)> {
        static SINK: OnceLock<Option<(Instant, Mutex<Sink>)>> = OnceLock::new();
        SINK.get_or_init(|| {
            let path = std::env::var_os("NFSU2_TRACE")?;
            let mut out = std::io::BufWriter::new(std::fs::File::create(path).ok()?);
            out.write_all(b"[\n").ok()?;
            Some((Instant::now(), Mutex::new(Sink { out, named: Vec::new() })))
        })
    }

    /// `NFSU2_TRACE` is set and its file could be made.
    pub fn on() -> bool {
        sink().is_some()
    }

    /// The calling thread's kernel id (Linux), else a per-thread number.
    fn tid() -> u64 {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: the gettid system call has no preconditions. Called through
            // syscall: glibc's own gettid() wrapper needs glibc 2.30, the release
            // build runs on 2.28 (scripts/release/check_linux.sh).
            unsafe { libc::syscall(libc::SYS_gettid) as u64 }
        }
        #[cfg(not(target_os = "linux"))]
        {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            std::thread::current().id().hash(&mut h);
            h.finish() & 0xFFFF_FFFF
        }
    }

    /// A JSON string's inside.
    pub fn escape(s: &str) -> String {
        let mut o = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '"' => o.push_str("\\\""),
                '\\' => o.push_str("\\\\"),
                c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                c => o.push(c),
            }
        }
        o
    }

    /// One trace event line: `name` from `begin` (µs since the trace's
    /// start) for `dur` µs on thread `tid`, with the thread CPU if known.
    pub fn line(name: &str, tid: u64, begin_us: f64, dur_us: f64, cpu_ns: Option<u64>) -> String {
        let args = cpu_ns.map(|c| format!(",\"args\":{{\"cpu_ms\":{:.3}}}", c as f64 / 1e6)).unwrap_or_default();
        format!("{{\"name\":\"{}\",\"ph\":\"X\",\"pid\":1,\"tid\":{tid},\"ts\":{begin_us:.1},\"dur\":{dur_us:.1}{args}}},\n", escape(name))
    }

    /// A finished span: `name` from `begin` for `dur` (no-op when off).
    pub fn complete(name: &str, begin: Instant, dur: Duration, cpu_ns: Option<u64>) {
        let Some((start, m)) = sink() else { return };
        let tid = tid();
        let begin_us = begin.saturating_duration_since(*start).as_secs_f64() * 1e6;
        let mut s = m.lock().unwrap();
        if !s.named.contains(&tid) {
            s.named.push(tid);
            let name = std::thread::current().name().map(str::to_string).unwrap_or_else(|| format!("thread {tid}"));
            let _ = write!(s.out, "{{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{tid},\"args\":{{\"name\":\"{}\"}}}},\n", escape(&name));
        }
        let _ = s.out.write_all(line(name, tid, begin_us, dur.as_secs_f64() * 1e6, cpu_ns).as_bytes());
        let _ = s.out.flush();
    }
}

/// Every thread of the process: (kernel id, name, CPU time so far in ns),
/// from /proc/self/task (Linux; empty elsewhere). For the bench's
/// per-thread CPU (a snapshot before and after the timed frames).
pub fn threads() -> Vec<(u64, String, u64)> {
    let Ok(dir) = std::fs::read_dir("/proc/self/task") else { return Vec::new() };
    let mut out = Vec::new();
    for e in dir.flatten() {
        let Some(tid) = e.file_name().to_str().and_then(|t| t.parse::<u64>().ok()) else { continue };
        let p = e.path();
        let name = std::fs::read_to_string(p.join("comm")).map(|s| s.trim().to_string()).unwrap_or_default();
        let ns = std::fs::read_to_string(p.join("schedstat")).ok().and_then(|s| s.split_whitespace().next()?.parse().ok()).unwrap_or(0);
        out.push((tid, name, ns));
    }
    out.sort();
    out
}

/// One DRM client's usage from an fdinfo file's text (the kernel's
/// drm-usage-stats keys): its client id, VRAM and GTT resident KiB, and
/// the gfx engine's busy ns. None for a file that is not a DRM client.
pub fn parse_drm_fdinfo(text: &str) -> Option<(u64, u64, u64, u64)> {
    let mut client = None;
    let (mut vram, mut gtt, mut gfx) = (0, 0, 0);
    for l in text.lines() {
        let Some((k, v)) = l.split_once(':') else { continue };
        let n = || v.split_whitespace().next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
        match k.trim() {
            "drm-client-id" => client = Some(n()),
            "drm-memory-vram" | "drm-resident-vram" => vram = vram.max(n()),
            "drm-memory-gtt" | "drm-resident-gtt" => gtt = gtt.max(n()),
            "drm-engine-gfx" => gfx = n(),
            _ => {}
        }
    }
    client.map(|c| (c, vram, gtt, gfx))
}

/// The process's own GPU memory: VRAM and GTT KiB and the gfx engine's
/// busy ns, summed over its DRM clients (amdgpu and others that publish
/// drm-usage-stats; /proc/self/fdinfo, Linux). None without a DRM client.
pub fn gpu_memory() -> Option<(u64, u64, u64)> {
    let dir = std::fs::read_dir("/proc/self/fdinfo").ok()?;
    let mut seen: Vec<(u64, u64, u64, u64)> = Vec::new();
    for e in dir.flatten() {
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        if let Some(c) = parse_drm_fdinfo(&text)
            && !seen.iter().any(|s| s.0 == c.0)
        {
            seen.push(c);
        }
    }
    (!seen.is_empty()).then(|| seen.iter().fold((0, 0, 0), |a, c| (a.0 + c.1, a.1 + c.2, a.2 + c.3)))
}

/// Per-label CPU-time totals the bench prints (`--bench` turns them on):
/// `let _t = prof::acc("x");` adds the scope's thread CPU time.
pub mod acc {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    static ON: AtomicBool = AtomicBool::new(false);
    static TOTALS: Mutex<Vec<(&'static str, u64, u64)>> = Mutex::new(Vec::new());

    pub fn enable(on: bool) {
        ON.store(on, Ordering::Relaxed);
    }

    pub struct Acc(&'static str, u64);

    impl Drop for Acc {
        fn drop(&mut self) {
            let ns = now() - self.1;
            let mut t = TOTALS.lock().unwrap();
            match t.iter_mut().find(|e| e.0 == self.0) {
                Some(e) => {
                    e.1 += ns;
                    e.2 += 1;
                }
                None => t.push((self.0, ns, 1)),
            }
        }
    }

    pub fn span(label: &'static str) -> Option<Acc> {
        ON.load(Ordering::Relaxed).then(|| Acc(label, now()))
    }

    /// The thread's CPU clock (CLOCK_THREAD_CPUTIME_ID), cheaper than
    /// schedstat for many small spans.
    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    fn now() -> u64 {
        unsafe extern "C" {
            fn clock_gettime(clock: i32, ts: *mut [i64; 2]) -> i32;
        }
        let mut ts = [0i64; 2];
        // SAFETY: clock_gettime writes a struct timespec (two longs on 64-bit Linux).
        unsafe { clock_gettime(3, &mut ts) };
        ts[0] as u64 * 1_000_000_000 + ts[1] as u64
    }
    #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
    fn now() -> u64 {
        super::thread_cpu_ns()
    }

    /// The totals so far (label, ns, calls), cleared.
    pub fn take() -> Vec<(&'static str, u64, u64)> {
        std::mem::take(&mut *TOTALS.lock().unwrap())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drm_fdinfo_parses_amdgpu_keys() {
        let t = "pos:\t0\nflags:\t02100002\ndrm-driver:\tamdgpu\ndrm-client-id:\t1234\ndrm-engine-gfx:\t5000 ns\ndrm-memory-vram:\t966000 KiB\ndrm-memory-gtt:\t1200 KiB\n";
        assert_eq!(parse_drm_fdinfo(t), Some((1234, 966000, 1200, 5000)));
        assert_eq!(parse_drm_fdinfo("pos:\t0\nflags:\t0\n"), None);
    }

    #[test]
    fn trace_line_is_json() {
        let l = trace::line("load: \"a\\b\"", 7, 1.5, 2.0, Some(3_000_000));
        assert_eq!(l, "{\"name\":\"load: \\\"a\\\\b\\\"\",\"ph\":\"X\",\"pid\":1,\"tid\":7,\"ts\":1.5,\"dur\":2.0,\"args\":{\"cpu_ms\":3.000}},\n");
    }

    #[test]
    fn threads_lists_this_one() {
        if cfg!(target_os = "linux") {
            assert!(!threads().is_empty());
        }
    }
}
