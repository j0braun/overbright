//! Performance overlay: where the system metrics come from
//! (docs/perf-overlay.md). Linux reads procfs and sysfs: `/proc/stat`
//! (CPU load per core), cpufreq (clocks), hwmon (`k10temp` / `zenpower` /
//! `coretemp` temperatures; the GPU's `amdgpu` hwmon for its clocks,
//! temperatures and power), powercap RAPL (CPU package power, when the
//! file is readable: root-only on most kernels since 2020), the DRM
//! device's `gpu_busy_percent` / `mem_info_*` (amdgpu), `/proc/meminfo`,
//! `/proc/self/{status,io,task}`, `/proc/diskstats`; NVIDIA through NVML
//! (`libnvidia-ml.so.1`, opened at run time, [`super::nvml`]). Every
//! metric is an `Option`: what a machine does not publish is shown as
//! "n/a" and logged empty.
//!
//! Every reader takes a root folder (`/` in the game) so the tests run on
//! the small fixture tree in `tests/fixtures/overlay/`.
//!
//! Windows / macOS: TODO (stubs return nothing). Windows: PDH counters
//! (`\Processor(*)\% Processor Time`, `\GPU Engine(*)\Utilization
//! Percentage`), DXGI `IDXGIAdapter3::QueryVideoMemoryInfo`, ADLX (AMD) /
//! NVAPI or NVML (NVIDIA), `GetProcessMemoryInfo`, `GetProcessIoCounters`.
//! macOS: `host_processor_info`, IOKit `IOAccelerator` performance
//! statistics, `task_info`.

use std::path::{Path, PathBuf};

/// One CPU line of `/proc/stat`: busy and total jiffies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub busy: u64,
    pub total: u64,
}

impl CpuTimes {
    /// The load between `self` (earlier) and `now`, percent.
    pub fn load(self, now: CpuTimes) -> Option<f32> {
        let total = now.total.checked_sub(self.total)?;
        let busy = now.busy.checked_sub(self.busy)?;
        (total > 0).then(|| (busy as f64 / total as f64 * 100.0) as f32)
    }
}

/// `/proc/stat`: the aggregate "cpu" line and each "cpuN" line, in order.
/// Idle is idle + iowait; busy is the rest of the first eight fields
/// (user, nice, system, idle, iowait, irq, softirq, steal; guest time is
/// already in user).
pub fn parse_proc_stat(text: &str) -> (Option<CpuTimes>, Vec<CpuTimes>) {
    let mut all = None;
    let mut cores = Vec::new();
    for l in text.lines() {
        let Some(rest) = l.strip_prefix("cpu") else { continue };
        let (id, fields) = rest.split_once(' ').unwrap_or((rest, ""));
        let v: Vec<u64> = fields.split_whitespace().take(8).filter_map(|x| x.parse().ok()).collect();
        if v.len() < 4 {
            continue;
        }
        let idle = v[3] + v.get(4).copied().unwrap_or(0);
        let total: u64 = v.iter().sum();
        let t = CpuTimes { busy: total - idle, total };
        if id.is_empty() {
            all = Some(t);
        } else if id.parse::<u32>().is_ok() {
            cores.push(t);
        }
    }
    (all, cores)
}

/// `/proc/meminfo`: (total, available, swap total, swap free), bytes.
pub fn parse_meminfo(text: &str) -> (Option<u64>, Option<u64>, Option<u64>, Option<u64>) {
    let (mut total, mut avail, mut swap_total, mut swap_free) = (None, None, None, None);
    for l in text.lines() {
        let Some((k, v)) = l.split_once(':') else { continue };
        let kb = v.split_whitespace().next().and_then(|x| x.parse::<u64>().ok()).map(|x| x * 1024);
        match k {
            "MemTotal" => total = kb,
            "MemAvailable" => avail = kb,
            "SwapTotal" => swap_total = kb,
            "SwapFree" => swap_free = kb,
            _ => {}
        }
    }
    (total, avail, swap_total, swap_free)
}

/// `/proc/self/status`: VmRSS, bytes.
pub fn parse_status_rss(text: &str) -> Option<u64> {
    text.lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kb| kb * 1024)
}

/// `/proc/self/io`: what the process read and wrote — all of it
/// (`rchar` / `wchar`, page cache included) and what reached the storage
/// (`read_bytes` / `write_bytes`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcIo {
    pub rchar: u64,
    pub wchar: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

pub fn parse_proc_io(text: &str) -> Option<ProcIo> {
    let mut io = ProcIo::default();
    let mut any = false;
    for l in text.lines() {
        let Some((k, v)) = l.split_once(':') else { continue };
        let Ok(n) = v.trim().parse::<u64>() else { continue };
        any = true;
        match k {
            "rchar" => io.rchar = n,
            "wchar" => io.wchar = n,
            "read_bytes" => io.read_bytes = n,
            "write_bytes" => io.write_bytes = n,
            _ => {}
        }
    }
    any.then_some(io)
}

/// A whole disk's name in `/proc/diskstats` (not a partition, not a
/// RAM or loop device): sdX, vdX, xvdX, hdX, nvmeNnM, mmcblkN.
pub fn is_whole_disk(name: &str) -> bool {
    let tail_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let letters = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase());
    if let Some(r) = name.strip_prefix("nvme") {
        // nvme0n1 (nvme0n1p1 is a partition).
        return r.split_once('n').is_some_and(|(c, n)| tail_digits(c) && tail_digits(n));
    }
    if let Some(r) = name.strip_prefix("mmcblk") {
        return tail_digits(r);
    }
    for p in ["sd", "vd", "xvd", "hd"] {
        if let Some(r) = name.strip_prefix(p) {
            return letters(r);
        }
    }
    false
}

/// `/proc/diskstats`: the whole disks' sectors read and written (512 B
/// each, whatever the device's own sector size), summed.
pub fn parse_diskstats(text: &str) -> Option<(u64, u64)> {
    let mut sum = None;
    for l in text.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() < 10 || !is_whole_disk(f[2]) {
            continue;
        }
        let (Ok(r), Ok(w)) = (f[5].parse::<u64>(), f[9].parse::<u64>()) else { continue };
        let s = sum.get_or_insert((0, 0));
        s.0 += r;
        s.1 += w;
    }
    sum
}

/// `/proc/self/task/<tid>/schedstat`'s first field: the thread's CPU ns.
pub fn parse_schedstat(text: &str) -> Option<u64> {
    text.split_whitespace().next()?.parse().ok()
}

/// A sysfs file's number (trimmed); None if absent or not a number.
pub fn read_num(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// A PCI id file's value ("0x1002").
fn read_hex(path: &Path) -> Option<u32> {
    let t = read_text(path)?;
    u32::from_str_radix(t.trim_start_matches("0x"), 16).ok()
}

/// The hwmon chip folders under `dir` (`/sys/class/hwmon` or a device's
/// `hwmon/`), with their `name`.
fn hwmons(dir: &Path) -> Vec<(PathBuf, String)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<(PathBuf, String)> =
        rd.flatten().map(|e| e.path()).filter_map(|p| Some((p.clone(), read_text(&p.join("name"))?))).collect();
    v.sort();
    v
}

/// The `tempN_input` (m°C) whose `tempN_label` is one of `labels` (in
/// their order of preference), else temp1.
fn hwmon_temp(dir: &Path, labels: &[&str]) -> Option<f32> {
    for want in labels {
        for i in 1..=16 {
            if read_text(&dir.join(format!("temp{i}_label"))).is_some_and(|l| l.eq_ignore_ascii_case(want)) {
                return read_num(&dir.join(format!("temp{i}_input"))).map(|m| m as f32 / 1000.0);
            }
        }
    }
    read_num(&dir.join("temp1_input")).map(|m| m as f32 / 1000.0)
}

/// What the GPU publishes (each where known).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuMetrics {
    /// Busy, percent.
    pub busy: Option<f32>,
    /// Core and memory clocks, MHz.
    pub core_mhz: Option<u32>,
    pub mem_mhz: Option<u32>,
    /// Edge (or the chip's own) temperature and the hot spot, °C.
    pub temp_c: Option<f32>,
    pub hotspot_c: Option<f32>,
    /// Board power, W.
    pub power_w: Option<f32>,
    /// VRAM and GTT (system memory the GPU maps), bytes.
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub gtt_used: Option<u64>,
    pub gtt_total: Option<u64>,
}

/// An amdgpu DRM device folder (`/sys/class/drm/cardN/device`) and its
/// hwmon chip, read each sample.
pub fn read_amdgpu(device: &Path, hwmon: Option<&Path>) -> GpuMetrics {
    let mut g = GpuMetrics {
        busy: read_num(&device.join("gpu_busy_percent")).map(|v| v as f32),
        vram_used: read_num(&device.join("mem_info_vram_used")),
        vram_total: read_num(&device.join("mem_info_vram_total")),
        gtt_used: read_num(&device.join("mem_info_gtt_used")),
        gtt_total: read_num(&device.join("mem_info_gtt_total")),
        ..GpuMetrics::default()
    };
    if let Some(h) = hwmon {
        // freqN_input in Hz, labelled sclk / mclk.
        for i in 1..=4 {
            let label = read_text(&h.join(format!("freq{i}_label")));
            let mhz = read_num(&h.join(format!("freq{i}_input"))).map(|hz| (hz / 1_000_000) as u32);
            match label.as_deref() {
                Some("sclk") => g.core_mhz = mhz,
                Some("mclk") => g.mem_mhz = mhz,
                _ => {}
            }
        }
        g.temp_c = hwmon_temp(h, &["edge"]);
        g.hotspot_c = (1..=8)
            .find(|i| read_text(&h.join(format!("temp{i}_label"))).is_some_and(|l| l == "junction"))
            .and_then(|i| read_num(&h.join(format!("temp{i}_input"))))
            .map(|m| m as f32 / 1000.0);
        // µW: the average where published (dGPUs), else the input (APUs).
        g.power_w =
            read_num(&h.join("power1_average")).or_else(|| read_num(&h.join("power1_input"))).map(|uw| uw as f32 / 1e6);
    }
    g
}

/// Where each metric is read, found once ([`Sources::discover`]).
#[derive(Debug, Default)]
pub struct Sources {
    pub root: PathBuf,
    /// The CPU's hwmon chip (k10temp / zenpower / coretemp / cpu_thermal).
    pub cpu_hwmon: Option<PathBuf>,
    /// RAPL's package energy counter (µJ) and its wrap.
    pub rapl: Option<(PathBuf, u64)>,
    /// The DRM device folder of the GPU drawn on, its hwmon and driver.
    pub gpu_device: Option<PathBuf>,
    pub gpu_hwmon: Option<PathBuf>,
    pub gpu_driver: Option<String>,
    /// NVML when the GPU drawn on is NVIDIA's and the library loads.
    #[cfg(target_os = "linux")]
    pub nvml: Option<super::nvml::Nvml>,
    /// cpufreq's current-clock files, per core.
    pub cpufreq: Vec<PathBuf>,
}

impl Sources {
    /// Look under `root` (`/`) for the readers; `pci` is the GPU drawn on
    /// (vendor, device) from the adapter, to pick its card among several
    /// (a discrete GPU beside the CPU's iGPU). Without a match, the
    /// first card that publishes a busy percentage.
    pub fn discover(root: &Path, pci: Option<(u32, u32)>) -> Sources {
        let mut s = Sources { root: root.to_path_buf(), ..Sources::default() };
        if !cfg!(target_os = "linux") {
            // TODO: Windows PDH/DXGI/ADLX/NVAPI, macOS IOKit.
            return s;
        }
        let hw = hwmons(&root.join("sys/class/hwmon"));
        for want in ["k10temp", "zenpower", "coretemp", "cpu_thermal"] {
            if let Some((p, _)) = hw.iter().find(|(_, n)| n == want) {
                s.cpu_hwmon = Some(p.clone());
                break;
            }
        }
        let rapl = root.join("sys/class/powercap/intel-rapl:0");
        if read_num(&rapl.join("energy_uj")).is_some() {
            s.rapl = Some((rapl.join("energy_uj"), read_num(&rapl.join("max_energy_range_uj")).unwrap_or(u64::MAX)));
        }
        let mut cpus: Vec<(u32, PathBuf)> = std::fs::read_dir(root.join("sys/devices/system/cpu"))
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        Some((
                            e.file_name().to_str()?.strip_prefix("cpu")?.parse::<u32>().ok()?,
                            e.path().join("cpufreq/scaling_cur_freq"),
                        ))
                    })
                    .filter(|(_, p)| p.exists())
                    .collect()
            })
            .unwrap_or_default();
        cpus.sort();
        s.cpufreq = cpus.into_iter().map(|c| c.1).collect();
        // The DRM cards (card0, card1; not the connectors card0-DP-1).
        let mut cards: Vec<PathBuf> = std::fs::read_dir(root.join("sys/class/drm"))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| {
                        e.file_name().to_str().is_some_and(|n| {
                            n.strip_prefix("card")
                                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
                        })
                    })
                    .map(|e| e.path().join("device"))
                    .collect()
            })
            .unwrap_or_default();
        cards.sort();
        let ids = |d: &Path| Some((read_hex(&d.join("vendor"))?, read_hex(&d.join("device"))?));
        let card = pci
            .and_then(|want| cards.iter().find(|d| ids(d) == Some(want)))
            .or_else(|| cards.iter().find(|d| d.join("gpu_busy_percent").exists()));
        if let Some(d) = card {
            s.gpu_hwmon = hwmons(&d.join("hwmon")).into_iter().next().map(|h| h.0);
            s.gpu_driver = std::fs::read_link(d.join("driver"))
                .ok()
                .and_then(|l| l.file_name().map(|n| n.to_string_lossy().into_owned()));
            s.gpu_device = Some(d.clone());
        }
        #[cfg(target_os = "linux")]
        if root == Path::new("/") && (pci.is_some_and(|p| p.0 == 0x10DE) || (pci.is_none() && card.is_none())) {
            s.nvml = super::nvml::Nvml::open();
        }
        s
    }

    pub fn proc_stat(&self) -> (Option<CpuTimes>, Vec<CpuTimes>) {
        std::fs::read_to_string(self.root.join("proc/stat")).map(|t| parse_proc_stat(&t)).unwrap_or_default()
    }

    /// The cores' current clocks, MHz (average, highest).
    pub fn cpu_mhz(&self) -> Option<(u32, u32)> {
        let v: Vec<u64> = self.cpufreq.iter().filter_map(|p| read_num(p)).collect();
        (!v.is_empty()).then(|| {
            (
                (v.iter().sum::<u64>() / v.len() as u64 / 1000) as u32,
                (v.iter().max().copied().unwrap_or(0) / 1000) as u32,
            )
        })
    }

    pub fn cpu_temp(&self) -> Option<f32> {
        self.cpu_hwmon.as_deref().and_then(|h| hwmon_temp(h, &["Tctl", "Tdie", "Package id 0"]))
    }

    /// RAPL's package energy counter, µJ, and its wrap.
    pub fn cpu_energy(&self) -> Option<(u64, u64)> {
        let (p, wrap) = self.rapl.as_ref()?;
        Some((read_num(p)?, *wrap))
    }

    pub fn gpu(&self) -> GpuMetrics {
        #[cfg(target_os = "linux")]
        if let Some(n) = &self.nvml {
            return n.read();
        }
        match &self.gpu_device {
            Some(d) if self.gpu_driver.as_deref().is_none_or(|n| n == "amdgpu") => {
                read_amdgpu(d, self.gpu_hwmon.as_deref())
            }
            // TODO: i915 / xe publish gt_cur_freq_mhz and little else
            // in sysfs; their busy time is in fdinfo (prof::gpu_memory).
            _ => GpuMetrics::default(),
        }
    }

    pub fn meminfo(&self) -> (Option<u64>, Option<u64>, Option<u64>, Option<u64>) {
        std::fs::read_to_string(self.root.join("proc/meminfo")).map(|t| parse_meminfo(&t)).unwrap_or_default()
    }

    pub fn rss(&self) -> Option<u64> {
        parse_status_rss(&std::fs::read_to_string(self.root.join("proc/self/status")).ok()?)
    }

    pub fn proc_io(&self) -> Option<ProcIo> {
        parse_proc_io(&std::fs::read_to_string(self.root.join("proc/self/io")).ok()?)
    }

    pub fn diskstats(&self) -> Option<(u64, u64)> {
        parse_diskstats(&std::fs::read_to_string(self.root.join("proc/diskstats")).ok()?)
    }

    /// The process's threads: (kernel id, name, CPU ns so far).
    pub fn threads(&self) -> Vec<(u64, String, u64)> {
        let Ok(dir) = std::fs::read_dir(self.root.join("proc/self/task")) else { return Vec::new() };
        let mut out: Vec<(u64, String, u64)> = dir
            .flatten()
            .filter_map(|e| {
                let tid = e.file_name().to_str()?.parse::<u64>().ok()?;
                let p = e.path();
                let name = read_text(&p.join("comm")).unwrap_or_default();
                Some((tid, name, std::fs::read_to_string(p.join("schedstat")).ok().and_then(|t| parse_schedstat(&t))?))
            })
            .collect();
        out.sort();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/overlay/root")
    }

    #[test]
    fn proc_stat_parses_the_aggregate_and_each_core() {
        let (all, cores) = parse_proc_stat(&std::fs::read_to_string(root().join("proc/stat")).unwrap());
        // 4000+100+900+20000+500+50+30 = 25580, idle 20000+500.
        assert_eq!(all, Some(CpuTimes { busy: 5080, total: 25580 }));
        assert_eq!(cores.len(), 2);
        let later = CpuTimes { busy: 5080 + 300, total: 25580 + 400 };
        assert_eq!(all.unwrap().load(later), Some(75.0));
        assert_eq!(later.load(all.unwrap()), None);
        assert_eq!(all.unwrap().load(all.unwrap()), None);
    }

    #[test]
    fn memory_io_and_disks_parse() {
        let r = root();
        let (t, a, st, sf) = parse_meminfo(&std::fs::read_to_string(r.join("proc/meminfo")).unwrap());
        assert_eq!(
            (t, a, st, sf),
            (Some(16384000 * 1024), Some(8192000 * 1024), Some(4096000 * 1024), Some(3072000 * 1024))
        );
        assert_eq!(
            parse_status_rss(&std::fs::read_to_string(r.join("proc/self/status")).unwrap()),
            Some(1200000 * 1024)
        );
        let io = parse_proc_io(&std::fs::read_to_string(r.join("proc/self/io")).unwrap()).unwrap();
        assert_eq!(io, ProcIo { rchar: 52428800, wchar: 1048576, read_bytes: 10485760, write_bytes: 524288 });
        assert_eq!(parse_proc_io("nothing here"), None);
        // sda + nvme0n1 only (not their partitions, zram, loop).
        assert_eq!(
            parse_diskstats(&std::fs::read_to_string(r.join("proc/diskstats")).unwrap()),
            Some((20000 + 100000, 8000 + 40000))
        );
        for (n, whole) in [
            ("sda", true),
            ("sda1", false),
            ("nvme0n1", true),
            ("nvme0n1p2", false),
            ("mmcblk0", true),
            ("mmcblk0p1", false),
            ("vdb", true),
            ("zram0", false),
            ("loop3", false),
            ("dm-0", false),
        ] {
            assert_eq!(is_whole_disk(n), whole, "{n}");
        }
        assert_eq!(parse_schedstat("800000000 1000 50\n"), Some(800000000));
    }

    #[test]
    fn discovery_picks_the_adapters_card_and_the_cpu_sensor() {
        let r = root();
        let s = Sources::discover(&r, Some((0x1002, 0x7001)));
        if !cfg!(target_os = "linux") {
            return;
        }
        assert_eq!(s.gpu_device.as_deref(), Some(r.join("sys/class/drm/card0/device").as_path()));
        assert_eq!(s.cpu_hwmon.as_deref(), Some(r.join("sys/class/hwmon/hwmon3").as_path()));
        assert_eq!(s.cpufreq.len(), 2);
        assert!(s.rapl.is_none());
        assert_eq!(s.cpu_mhz(), Some((4100, 4700)));
        assert_eq!(s.cpu_temp(), Some(77.125));
        let g = s.gpu();
        assert_eq!(
            g,
            GpuMetrics {
                busy: Some(86.0),
                core_mhz: Some(2345),
                mem_mhz: Some(1218),
                temp_c: Some(67.0),
                hotspot_c: Some(95.0),
                power_w: Some(144.0),
                vram_used: Some(5185806336),
                vram_total: Some(17163091968),
                gtt_used: Some(149073920),
                gtt_total: Some(33239334912)
            }
        );
        // The iGPU when asked for, its power from power1_input (an APU).
        let i = Sources::discover(&r, Some((0x1002, 0x7002))).gpu();
        assert_eq!(
            (i.busy, i.core_mhz, i.power_w, i.temp_c, i.hotspot_c),
            (Some(3.0), Some(600), Some(12.0), Some(45.0), None)
        );
        // An unknown adapter: the first card with a busy percentage.
        assert_eq!(Sources::discover(&r, Some((0x8086, 0x1234))).gpu_device, s.gpu_device);
        assert_eq!(
            s.threads().iter().map(|t| (t.0, t.1.as_str())).collect::<Vec<_>>(),
            [(4100, "game"), (4101, "render 0"), (4102, "perf sampler")]
        );
    }

    #[test]
    fn a_missing_tree_degrades_to_nothing() {
        let s = Sources::discover(Path::new("/nonexistent-overlay-root"), None);
        assert!(s.gpu_device.is_none() && s.cpu_hwmon.is_none());
        assert_eq!(s.gpu(), GpuMetrics::default());
        assert_eq!((s.cpu_mhz(), s.cpu_temp(), s.rss(), s.proc_io(), s.diskstats()), (None, None, None, None, None));
        assert!(s.threads().is_empty() && s.proc_stat().1.is_empty());
    }
}
