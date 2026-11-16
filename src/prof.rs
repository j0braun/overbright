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
            // SAFETY: gettid has no preconditions.
            unsafe { libc::gettid() as u64 }
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

/// docs/perf-gpu.md "Render-target memory": the device's allocator
/// report (every live texture and buffer by its label, as wgpu-hal's
/// gpu-allocator holds them) grouped by label, largest first: count, MiB,
/// then the totals. `NFSU2_VRAM_REPORT=file` writes it at a bench's end; a
/// measuring aid, read once, nothing when unset. None where the backend
/// keeps no report.
pub fn allocator_report(device: &wgpu::Device) -> Option<String> {
    let r = device.generate_allocator_report()?;
    let mut by: Vec<(String, u64, u64)> = Vec::new();
    for a in &r.allocations {
        match by.iter_mut().find(|e| e.0 == a.name) {
            Some(e) => {
                e.1 += 1;
                e.2 += a.size;
            }
            None => by.push((a.name.clone(), 1, a.size)),
        }
    }
    by.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
    let now = gpu_memory().map_or(String::new(), |m| format!(", process VRAM now {} MiB, GTT {} MiB", m.0 / 1024, m.1 / 1024));
    let mut out = format!("# allocated {:.1} MiB in {} allocations, reserved {:.1} MiB in {} blocks{now}\n", mib(r.total_allocated_bytes), r.allocations.len(), mib(r.total_reserved_bytes), r.blocks.len());
    for (name, n, bytes) in by {
        out.push_str(&format!("{:10.2}\t{n}\t{name}\n", mib(bytes)));
    }
    Some(out)
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

/// GPU time per part of the frame, from timestamp queries (the bench
/// turns them on; a no-op otherwise, and on devices without
/// [`gpu::FEATURES`]). `gpu::mark(encoder, "x")` writes a timestamp
/// after the commands recorded so far: the time since the previous mark
/// goes to "x". Each encoder of a frame starts with a mark of its own
/// ([`gpu::GAP`]), so the GPU's wait between submissions is not charged
/// to a part.
///
/// The read-back never stalls the frame: each frame's marks go to one of
/// [`gpu::RING`] query sets, resolved and copied after the frame's last
/// submission ([`gpu::end_frame`]) and mapped asynchronously; a frame is
/// read when its map has completed, a few frames later (`device.poll`
/// without waiting). Only when the ring comes round to a set still being
/// read does it wait, for that set alone.
pub mod gpu {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// What the timestamps need: queries, and writes between passes.
    pub const FEATURES: wgpu::Features = wgpu::Features::TIMESTAMP_QUERY.union(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
    /// The label of an encoder's first mark.
    pub const GAP: &str = "(between submissions)";
    const MAX: u32 = 256;
    /// Frames whose timestamps can be in flight at once.
    pub const RING: usize = 4;

    static ON: AtomicBool = AtomicBool::new(false);
    /// Ask for [`FEATURES`] when the device is made (`render::Gpu::new`).
    static WANTED: AtomicBool = AtomicBool::new(false);

    struct Slot {
        set: wgpu::QuerySet,
        resolve: wgpu::Buffer,
        read: wgpu::Buffer,
        /// The marks written into `set`, in the order made.
        labels: Vec<&'static str>,
        /// Being read back: the frame's number, its submission, and set
        /// once the buffer is mapped.
        pending: Option<(u64, wgpu::SubmissionIndex, Arc<AtomicBool>)>,
    }

    struct State {
        slots: Vec<Slot>,
        /// The slot this frame's marks go to.
        current: usize,
        /// Frames ended so far.
        frame: u64,
        /// ns per tick.
        period: f64,
        /// Per label: total ms, frames.
        totals: Vec<(&'static str, f64, u64)>,
        /// Per frame read back: its number, the GPU's busy time (the
        /// parts, without the gaps between submissions), ms.
        frames: Vec<(u64, f64)>,
        /// The parts of the frame read last.
        last: Vec<&'static str>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    pub fn want(on: bool) {
        WANTED.store(on, Ordering::Relaxed);
    }

    pub fn wanted() -> bool {
        WANTED.load(Ordering::Relaxed)
    }

    /// Start timing on `device` (made with [`FEATURES`]); false if it lacks them.
    pub fn init(device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        if !device.features().contains(FEATURES) {
            return false;
        }
        let size = u64::from(MAX) * 8;
        let slots = (0..RING)
            .map(|_| Slot {
                set: device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("gpu timing"), ty: wgpu::QueryType::Timestamp, count: MAX }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor { label: Some("gpu timing"), size, usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false }),
                read: device.create_buffer(&wgpu::BufferDescriptor { label: Some("gpu timing"), size, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false }),
                labels: Vec::new(),
                pending: None,
            })
            .collect();
        *STATE.lock().unwrap() = Some(State { slots, current: 0, frame: 0, period: f64::from(queue.get_timestamp_period()), totals: Vec::new(), frames: Vec::new(), last: Vec::new() });
        ON.store(true, Ordering::Relaxed);
        true
    }

    /// A timestamp after what `encoder` holds so far, closing part `label`.
    pub fn mark(encoder: &mut wgpu::CommandEncoder, label: &'static str) {
        if !ON.load(Ordering::Relaxed) {
            return;
        }
        let mut state = STATE.lock().unwrap();
        let Some(s) = state.as_mut() else { return };
        let slot = &mut s.slots[s.current];
        let i = slot.labels.len() as u32;
        if i < MAX {
            encoder.write_timestamp(&slot.set, i);
            slot.labels.push(label);
        }
    }

    /// After the frame's last submission: resolve its timestamps and start
    /// reading them back, without waiting; add the frames read back by
    /// now to the totals. Returns the parts of the latest frame read.
    pub fn end_frame(device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<&'static str> {
        if !ON.load(Ordering::Relaxed) {
            return Vec::new();
        }
        let mut state = STATE.lock().unwrap();
        let Some(s) = state.as_mut() else { return Vec::new() };
        let frame = s.frame;
        s.frame += 1;
        let cur = s.current;
        let n = s.slots[cur].labels.len() as u32;
        if n > 0 {
            let slot = &mut s.slots[cur];
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.resolve_query_set(&slot.set, 0..n, &slot.resolve, 0);
            encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.read, 0, u64::from(n) * 8);
            let index = queue.submit([encoder.finish()]);
            let done = Arc::new(AtomicBool::new(false));
            let flag = done.clone();
            slot.read.slice(..u64::from(n) * 8).map_async(wgpu::MapMode::Read, move |r| {
                if r.is_ok() {
                    flag.store(true, Ordering::Release);
                }
            });
            slot.pending = Some((frame, index, done));
        }
        // The next frame's set: free once its read-back is in (wait for it
        // only if not yet).
        s.current = (cur + 1) % RING;
        let _ = device.poll(wgpu::PollType::Poll);
        if let Some((_, index, done)) = &s.slots[s.current].pending
            && !done.load(Ordering::Acquire)
        {
            let _ = device.poll(wgpu::PollType::Wait { submission_index: Some(index.clone()), timeout: None });
        }
        harvest(s);
        s.slots[s.current].labels.clear();
        s.last.clone()
    }

    /// Wait for every frame's read-back still under way and add them in.
    pub fn finish(device: &wgpu::Device) {
        if !ON.load(Ordering::Relaxed) {
            return;
        }
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let mut state = STATE.lock().unwrap();
        if let Some(s) = state.as_mut() {
            harvest(s);
        }
    }

    /// Reads every slot whose map has completed, oldest frame first.
    fn harvest(s: &mut State) {
        let mut ready: Vec<usize> = (0..s.slots.len()).filter(|&i| s.slots[i].pending.as_ref().is_some_and(|p| p.2.load(Ordering::Acquire))).collect();
        ready.sort_by_key(|&i| s.slots[i].pending.as_ref().unwrap().0);
        for i in ready {
            let slot = &mut s.slots[i];
            let (frame, _, _) = slot.pending.take().unwrap();
            let n = slot.labels.len();
            let ticks: Vec<u64> = slot.read.slice(..n as u64 * 8).get_mapped_range().expect("mapped timestamps").chunks_exact(8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).collect();
            slot.read.unmap();
            // In the order the GPU wrote them (marks are made on several
            // threads, into command buffers submitted in order).
            let mut marks: Vec<(u64, &'static str)> = ticks.iter().copied().zip(std::mem::take(&mut slot.labels)).collect();
            marks.sort_by_key(|m| m.0);
            let mut parts: Vec<&'static str> = marks.iter().map(|m| m.1).filter(|&l| l != GAP).collect();
            parts.dedup();
            // Per label, this frame's sum (a label may close several parts).
            let mut sums: Vec<(&'static str, f64)> = Vec::new();
            for i in 1..marks.len() {
                let ms = marks[i].0.saturating_sub(marks[i - 1].0) as f64 * s.period / 1e6;
                match sums.iter_mut().find(|e| e.0 == marks[i].1) {
                    Some(e) => e.1 += ms,
                    None => sums.push((marks[i].1, ms)),
                }
            }
            let busy = sums.iter().filter(|e| e.0 != GAP).map(|e| e.1).sum();
            s.frames.push((frame, busy));
            for (label, ms) in sums {
                match s.totals.iter_mut().find(|e| e.0 == label) {
                    Some(e) => {
                        e.1 += ms;
                        e.2 += 1;
                    }
                    None => s.totals.push((label, ms, 1)),
                }
            }
            s.last = parts;
        }
    }

    /// Per part: its total ms and the frames it ran in, cleared.
    pub fn take() -> Vec<(&'static str, f64, u64)> {
        STATE.lock().unwrap().as_mut().map(|s| std::mem::take(&mut s.totals)).unwrap_or_default()
    }

    /// Per frame read back since the last call: the GPU's busy time, ms,
    /// in frame order.
    pub fn take_frames() -> Vec<f64> {
        STATE.lock().unwrap().as_mut().map(|s| {
            let mut f = std::mem::take(&mut s.frames);
            f.sort_by_key(|e| e.0);
            f.into_iter().map(|e| e.1).collect()
        }).unwrap_or_default()
    }

    /// Drop the current frame's marks unread (e.g. frames not timed).
    pub fn discard() {
        if let Some(s) = STATE.lock().unwrap().as_mut() {
            let cur = s.current;
            s.slots[cur].labels.clear();
        }
    }

    /// the performance overlay's Full layout: timing was started
    /// ([`init`]) by someone.
    pub fn ready() -> bool {
        STATE.lock().unwrap().is_some()
    }

    /// The marks are written ([`init`] made, not paused).
    pub fn active() -> bool {
        ON.load(Ordering::Relaxed)
    }

    /// Pause or resume timing after [`init`] (the overlay's Full layout
    /// hidden / shown): paused, `mark` and `end_frame` do nothing. On
    /// resuming, the marks left from the frame paused in are dropped.
    pub fn set_active(on: bool) {
        if on && ready() {
            discard();
            ON.store(true, Ordering::Relaxed);
        } else if !on {
            ON.store(false, Ordering::Relaxed);
        }
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
