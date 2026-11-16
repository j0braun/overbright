//! The overlay's background sampler. A thread named "perf sampler"
//! reads [`Sources`] at [`Sampler::hz`] (2–10 Hz, default 4) and keeps
//! the latest [`Snapshot`]; the draw thread only copies it at its own
//! refresh. Deltas (loads, rates) are over the time between two samples.
//! When CSV logging is on, each sample also writes a row ([`super::csv`]).

use super::sources::{CpuTimes, GpuMetrics, ProcIo, Sources};
use super::stats::FrameSummary;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One sample of everything the system publishes (None: not known here).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// Seconds since the sampler started.
    pub t: f64,
    /// CPU load, percent: all cores, and each.
    pub cpu: Option<f32>,
    pub cores: Vec<f32>,
    /// Core clocks, MHz: average and highest.
    pub cpu_mhz: Option<(u32, u32)>,
    pub cpu_temp: Option<f32>,
    /// Package power (RAPL), W.
    pub cpu_power: Option<f32>,
    pub gpu: GpuMetrics,
    /// The game's own GPU memory (VRAM, GTT bytes) and its share of the
    /// gfx engine, percent (DRM fdinfo, `prof::gpu_memory`).
    pub own_vram: Option<u64>,
    pub own_gtt: Option<u64>,
    pub own_gpu: Option<f32>,
    /// The game's resident memory; the system's used / total RAM and swap.
    pub rss: Option<u64>,
    pub mem_used: Option<u64>,
    pub mem_total: Option<u64>,
    pub swap_used: Option<u64>,
    pub swap_total: Option<u64>,
    /// The game's I/O, bytes per second: what reached storage, and all of
    /// it (page cache included).
    pub io_read: Option<f64>,
    pub io_write: Option<f64>,
    pub io_rchar: Option<f64>,
    pub io_wchar: Option<f64>,
    /// The whole disks' throughput, bytes per second.
    pub disk_read: Option<f64>,
    pub disk_write: Option<f64>,
    /// The game's CPU, percent of one core, and per thread (name, percent),
    /// busiest first (those under 0.05 % left out).
    pub process_cpu: Option<f32>,
    pub threads: Vec<(String, f32)>,
    /// Variable refresh (`latency::vrr::probe`, every 2 s): its state in
    /// words, and whether a panel is driven variably now.
    pub vrr: Option<String>,
    pub vrr_active: Option<bool>,
}

/// What a sample needs from the one before.
#[derive(Default)]
pub struct Prev {
    at: Option<Instant>,
    cpu: Option<CpuTimes>,
    cores: Vec<CpuTimes>,
    energy: Option<u64>,
    io: Option<ProcIo>,
    disk: Option<(u64, u64)>,
    gfx_ns: Option<u64>,
    threads: Vec<(u64, u64)>,
    vrr: Option<(crate::latency::vrr::Vrr, Instant)>,
}

/// Thread names as the overlay shows them: the main thread (the process's
/// own name, tid = pid) is "main".
fn thread_label(tid: u64, name: &str, pid: u64) -> String {
    if tid == pid { "main".to_string() } else { name.to_string() }
}

/// One sample from `src`, deltas against `prev` (updated).
pub fn sample(src: &Sources, prev: &mut Prev, started: Instant) -> Snapshot {
    let now = Instant::now();
    let dt = prev.at.map(|a| now.duration_since(a).as_secs_f64()).filter(|d| *d > 1e-4);
    let rate = |a: u64, b: u64| dt.map(|d| b.saturating_sub(a) as f64 / d);
    let mut s = Snapshot { t: now.duration_since(started).as_secs_f64(), ..Snapshot::default() };
    let (all, cores) = src.proc_stat();
    s.cpu = prev.cpu.zip(all).and_then(|(a, b)| a.load(b));
    s.cores = prev.cores.iter().zip(&cores).filter_map(|(a, b)| a.load(*b)).collect();
    prev.cpu = all;
    prev.cores = cores;
    s.cpu_mhz = src.cpu_mhz();
    s.cpu_temp = src.cpu_temp();
    if let Some((e, wrap)) = src.cpu_energy() {
        if let (Some(p), Some(d)) = (prev.energy, dt) {
            let de = if e >= p { e - p } else { wrap.saturating_sub(p) + e };
            s.cpu_power = Some((de as f64 / 1e6 / d) as f32);
        }
        prev.energy = Some(e);
    }
    s.gpu = src.gpu();
    if src.root == std::path::Path::new("/")
        && let Some((vram, gtt, gfx)) = crate::prof::gpu_memory()
    {
        s.own_vram = Some(vram * 1024);
        s.own_gtt = Some(gtt * 1024);
        if let (Some(p), Some(d)) = (prev.gfx_ns, dt) {
            s.own_gpu = Some((gfx.saturating_sub(p) as f64 / (d * 1e9) * 100.0).min(100.0) as f32);
        }
        prev.gfx_ns = Some(gfx);
    }
    let (total, avail, swap_total, swap_free) = src.meminfo();
    s.mem_total = total;
    s.mem_used = total.zip(avail).map(|(t, a)| t.saturating_sub(a));
    s.swap_total = swap_total;
    s.swap_used = swap_total.zip(swap_free).map(|(t, f)| t.saturating_sub(f));
    s.rss = src.rss();
    if let Some(io) = src.proc_io() {
        if let Some(p) = prev.io {
            s.io_read = rate(p.read_bytes, io.read_bytes);
            s.io_write = rate(p.write_bytes, io.write_bytes);
            s.io_rchar = rate(p.rchar, io.rchar);
            s.io_wchar = rate(p.wchar, io.wchar);
        }
        prev.io = Some(io);
    }
    if let Some(d) = src.diskstats() {
        if let Some(p) = prev.disk {
            s.disk_read = rate(p.0 * 512, d.0 * 512);
            s.disk_write = rate(p.1 * 512, d.1 * 512);
        }
        prev.disk = Some(d);
    }
    let threads = src.threads();
    if let Some(d) = dt {
        let pid = u64::from(std::process::id());
        let mut per: Vec<(String, f32)> = threads
            .iter()
            .filter_map(|(tid, name, ns)| {
                let before = prev.threads.iter().find(|p| p.0 == *tid)?.1;
                Some((thread_label(*tid, name, pid), (ns.saturating_sub(before) as f64 / (d * 1e9) * 100.0) as f32))
            })
            .collect();
        per.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        s.process_cpu = Some(per.iter().map(|t| t.1).sum());
        // The idle ones left out (a game has dozens of sleeping threads).
        per.retain(|t| t.1 >= 0.05);
        s.threads = per;
    }
    prev.threads = threads.iter().map(|t| (t.0, t.2)).collect();
    if src.root == std::path::Path::new("/") {
        if prev.vrr.as_ref().is_none_or(|(_, at)| now.duration_since(*at) > Duration::from_secs(2)) {
            prev.vrr = Some((crate::latency::vrr::probe(), now));
        }
        if let Some((v, _)) = &prev.vrr {
            s.vrr = Some(v.describe());
            s.vrr_active = v.known.then(|| v.active());
        }
    }
    prev.at = Some(now);
    s
}

struct Shared {
    latest: Mutex<Option<Snapshot>>,
    frames: Mutex<FrameSummary>,
    csv: Mutex<Option<super::csv::CsvLog>>,
    stop: AtomicBool,
    /// Samples per second.
    hz: AtomicU32,
}

/// The sampler thread; stopped when dropped.
pub struct Sampler {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Sampler {
    /// Start sampling the system (`pci`: the GPU drawn on).
    pub fn start(pci: Option<(u32, u32)>, hz: u32) -> Sampler {
        let shared = Arc::new(Shared { latest: Mutex::new(None), frames: Mutex::new(FrameSummary::default()), csv: Mutex::new(None), stop: AtomicBool::new(false), hz: AtomicU32::new(hz.clamp(2, 10)) });
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("perf sampler".into())
            .spawn(move || {
                let src = Sources::discover(std::path::Path::new("/"), pci);
                let started = Instant::now();
                let mut prev = Prev::default();
                // A first sample for the deltas, then one each period.
                sample(&src, &mut prev, started);
                while !s.stop.load(Ordering::Relaxed) {
                    std::thread::park_timeout(Duration::from_secs_f64(1.0 / f64::from(s.hz.load(Ordering::Relaxed).max(1))));
                    if s.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let snap = sample(&src, &mut prev, started);
                    if let Some(log) = s.csv.lock().unwrap().as_mut() {
                        let f = *s.frames.lock().unwrap();
                        log.row(&snap, &f);
                    }
                    *s.latest.lock().unwrap() = Some(snap);
                }
            })
            .ok();
        Sampler { shared, thread }
    }

    pub fn set_hz(&self, hz: u32) {
        self.shared.hz.store(hz.clamp(2, 10), Ordering::Relaxed);
    }

    /// The latest sample (None until the second one is in).
    pub fn latest(&self) -> Option<Snapshot> {
        self.shared.latest.lock().unwrap().clone()
    }

    /// The frame figures the CSV rows carry.
    pub fn publish_frames(&self, f: FrameSummary) {
        *self.shared.frames.lock().unwrap() = f;
    }

    /// Log to `path` (a new file), or stop logging (None). Returns the
    /// file's path when it was opened.
    pub fn set_csv(&self, path: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
        let mut csv = self.shared.csv.lock().unwrap();
        match path {
            None => {
                *csv = None;
                None
            }
            Some(p) if csv.as_ref().is_some_and(|c| c.path() == p) => Some(p.to_path_buf()),
            Some(p) => match super::csv::CsvLog::create(p) {
                Ok(c) => {
                    *csv = Some(c);
                    Some(p.to_path_buf())
                }
                Err(e) => {
                    log::warn!("performance log {} not opened: {e}", p.display());
                    None
                }
            },
        }
    }

    pub fn logging(&self) -> bool {
        self.shared.csv.lock().unwrap().is_some()
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sample_of_the_fixture_tree() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/overlay/root");
        let src = Sources::discover(&root, Some((0x1002, 0x7001)));
        let started = Instant::now();
        let mut prev = Prev::default();
        let first = sample(&src, &mut prev, started);
        // No deltas yet.
        assert_eq!((first.cpu, first.io_read, first.disk_read, first.process_cpu), (None, None, None, None));
        assert_eq!(first.mem_total, Some(16384000 * 1024));
        assert_eq!(first.mem_used, Some((16384000 - 8192000) * 1024));
        assert_eq!(first.swap_used, Some(1024000 * 1024));
        assert_eq!(first.rss, Some(1200000 * 1024));
        // Not the live system's DRM clients for a fixture root.
        assert_eq!(first.own_vram, None);
        std::thread::sleep(Duration::from_millis(5));
        let second = sample(&src, &mut prev, started);
        // The same files again: no time on any CPU, no I/O.
        assert_eq!(second.cpu, None);
        assert_eq!(second.io_read, Some(0.0));
        assert_eq!(second.disk_write, Some(0.0));
        assert_eq!(second.process_cpu, Some(0.0));
        // All idle: none listed.
        assert!(second.threads.is_empty());
        assert!(second.t >= first.t);
        if cfg!(target_os = "linux") {
            assert_eq!(second.gpu.busy, Some(86.0));
        }
    }

    #[test]
    fn the_sampler_thread_samples_and_stops() {
        let s = Sampler::start(None, 10);
        let t = Instant::now();
        while s.latest().is_none() && t.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let snap = s.latest().expect("a sample within 3 s");
        if cfg!(target_os = "linux") {
            assert!(snap.mem_total.is_some() && snap.rss.is_some() && !snap.threads.is_empty());
        }
        let stop = Instant::now();
        drop(s);
        assert!(stop.elapsed() < Duration::from_secs(1));
    }
}
