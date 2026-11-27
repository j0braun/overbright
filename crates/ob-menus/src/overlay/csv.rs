//! The performance log, a CSV file of every overlay metric, one row
//! per sample (2–10 Hz), for benchmarks (scripts/perf can read it). The
//! first line names the columns ([`COLUMNS`]); an unknown value is an
//! empty field. Per-core loads and per-thread CPU are one quoted field
//! each, `;`-separated (`12.0;5.5;…`, `main=80.1;render 0=20.4;…`).

use super::sampler::Snapshot;
use super::stats::FrameSummary;
use std::io::Write;

pub const COLUMNS: [&str; 48] = [
    "time_s", "fps", "frame_ms", "frame_p99_ms", "frame_max_ms", "low1_fps", "low01_fps", "update_ms", "draw_ms", "acquire_ms",
    "cpu_pct", "cpu_mhz_avg", "cpu_mhz_max", "cpu_temp_c", "cpu_power_w",
    "gpu_pct", "gpu_core_mhz", "gpu_mem_mhz", "gpu_temp_c", "gpu_hotspot_c", "gpu_power_w", "vram_used_mb", "vram_total_mb", "gtt_used_mb",
    "own_vram_mb", "own_gtt_mb", "own_gpu_pct",
    "rss_mb", "ram_used_mb", "ram_total_mb", "swap_used_mb",
    "io_read_kbs", "io_write_kbs", "io_rchar_kbs", "disk_read_kbs", "disk_write_kbs",
    "process_cpu_pct", "vrr_active", "cores_pct", "threads_pct",
    "trails_ms", "wait_ms", "cap_ms", "gpu_ms", "ticks_per_s", "tick_ms", "speed_pct", "trails_per_s",
];

pub fn header() -> String {
    COLUMNS.join(",") + "\n"
}

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

fn f1(v: Option<f32>) -> String {
    opt(v.map(|v| format!("{v:.1}")))
}

fn mb(v: Option<u64>) -> String {
    opt(v.map(|v| format!("{:.1}", v as f64 / (1024.0 * 1024.0))))
}

fn kbs(v: Option<f64>) -> String {
    opt(v.map(|v| format!("{:.1}", v / 1024.0)))
}

/// A field quoted when it holds a comma or a quote.
fn field(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

/// One row (with its line end) for a sample and the frame figures then.
pub fn row(s: &Snapshot, f: &FrameSummary) -> String {
    let frame = |v: f32| if f.frames > 0 { format!("{v:.3}") } else { String::new() };
    let cells = [
        format!("{:.3}", s.t),
        if f.frames > 0 { format!("{:.1}", f.fps) } else { String::new() },
        frame(f.avg_ms),
        frame(f.p99_ms),
        frame(f.max_ms),
        if f.frames > 0 { format!("{:.1}", f.low1_fps) } else { String::new() },
        if f.frames > 0 { format!("{:.1}", f.low01_fps) } else { String::new() },
        frame(f.update_ms),
        frame(f.draw_ms),
        frame(f.acquire_ms),
        f1(s.cpu),
        opt(s.cpu_mhz.map(|m| m.0)),
        opt(s.cpu_mhz.map(|m| m.1)),
        f1(s.cpu_temp),
        f1(s.cpu_power),
        f1(s.gpu.busy),
        opt(s.gpu.core_mhz),
        opt(s.gpu.mem_mhz),
        f1(s.gpu.temp_c),
        f1(s.gpu.hotspot_c),
        f1(s.gpu.power_w),
        mb(s.gpu.vram_used),
        mb(s.gpu.vram_total),
        mb(s.gpu.gtt_used),
        mb(s.own_vram),
        mb(s.own_gtt),
        f1(s.own_gpu),
        mb(s.rss),
        mb(s.mem_used),
        mb(s.mem_total),
        mb(s.swap_used),
        kbs(s.io_read),
        kbs(s.io_write),
        kbs(s.io_rchar),
        kbs(s.disk_read),
        kbs(s.disk_write),
        f1(s.process_cpu),
        opt(s.vrr_active.map(u8::from)),
        field(&s.cores.iter().map(|c| format!("{c:.1}")).collect::<Vec<_>>().join(";")),
        field(&s.threads.iter().map(|(n, c)| format!("{n}={c:.1}")).collect::<Vec<_>>().join(";")),
        frame(f.probe_ms),
        frame(f.wait_ms),
        frame(f.cap_ms),
        opt(f.gpu_ms.map(|g| format!("{g:.3}"))),
        if f.frames > 0 { format!("{:.1}", f.ticks_per_s) } else { String::new() },
        frame(f.tick_ms),
        if f.frames > 0 { format!("{:.1}", f.speed * 100.0) } else { String::new() },
        if f.frames > 0 { format!("{:.1}", f.probes_per_s) } else { String::new() },
    ];
    debug_assert_eq!(cells.len(), COLUMNS.len());
    cells.join(",") + "\n"
}

/// An open log file.
pub struct CsvLog {
    path: std::path::PathBuf,
    out: std::io::BufWriter<std::fs::File>,
}

impl CsvLog {
    /// A new file at `path` (its folder made), the header written.
    pub fn create(path: &std::path::Path) -> std::io::Result<CsvLog> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
        out.write_all(header().as_bytes())?;
        out.flush()?;
        Ok(CsvLog { path: path.to_path_buf(), out })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// A row, flushed (a crash keeps every row so far).
    pub fn row(&mut self, s: &Snapshot, f: &FrameSummary) {
        let _ = self.out.write_all(row(s, f).as_bytes());
        let _ = self.out.flush();
    }
}

/// The log's default file: `<data home>/overbright/perf/overbright-
/// <UTC time>.csv`, or `$NFSU2_PERF_CSV`.
pub fn default_path() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("NFSU2_PERF_CSV") {
        return p.into();
    }
    crate::platform::dirs::perf().join(format!("overbright-{}.csv", crate::qol::saves::timestamp(std::time::SystemTime::now())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A CSV line's fields (quotes honoured).
    fn fields(line: &str) -> Vec<String> {
        let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' if quoted && chars.peek() == Some(&'"') => {
                    cur.push('"');
                    chars.next();
                }
                '"' => quoted = !quoted,
                ',' if !quoted => out.push(std::mem::take(&mut cur)),
                c => cur.push(c),
            }
        }
        out.push(cur);
        out
    }

    #[test]
    fn rows_match_the_header_and_leave_unknowns_empty() {
        let h = header();
        assert_eq!(fields(h.trim_end()).len(), COLUMNS.len());
        let empty = row(&Snapshot::default(), &FrameSummary::default());
        let f = fields(empty.trim_end());
        assert_eq!(f.len(), COLUMNS.len());
        assert_eq!(f[0], "0.000");
        assert!(f[1..].iter().all(|v| v.is_empty()), "{f:?}");
        let s = Snapshot {
            t: 1.25,
            cpu: Some(23.04),
            cores: vec![10.0, 36.06],
            gpu: crate::overlay::sources::GpuMetrics { busy: Some(86.0), vram_used: Some(512 * 1024 * 1024), ..Default::default() },
            io_read: Some(2048.0),
            threads: vec![("main".into(), 80.12), ("render, 0".into(), 20.0)],
            ..Default::default()
        };
        let fr = FrameSummary { frames: 60, fps: 60.0, avg_ms: 16.6667, low1_fps: 55.55, ..Default::default() };
        let line = row(&s, &fr);
        assert!(line.ends_with('\n'));
        let f = fields(line.trim_end());
        assert_eq!(f.len(), COLUMNS.len());
        let col = |name: &str| f[COLUMNS.iter().position(|c| *c == name).unwrap()].clone();
        assert_eq!((col("time_s"), col("fps"), col("frame_ms"), col("low1_fps")), ("1.250".into(), "60.0".into(), "16.667".into(), "55.5".into()));
        assert_eq!((col("cpu_pct"), col("gpu_pct"), col("vram_used_mb"), col("io_read_kbs")), ("23.0".into(), "86.0".into(), "512.0".into(), "2.0".into()));
        assert_eq!(col("cores_pct"), "10.0;36.1");
        // A comma in a thread's name is quoted, not a new column.
        assert_eq!(col("threads_pct"), "main=80.1;render, 0=20.0");
        assert_eq!(col("gpu_temp_c"), "");
    }

    #[test]
    fn the_log_file_gets_the_header_and_rows() {
        let dir = std::env::temp_dir().join(format!("overlay-csv-{}", crate::unique_tag()));
        let path = dir.join("sub/log.csv");
        let mut log = CsvLog::create(&path).unwrap();
        log.row(&Snapshot::default(), &FrameSummary::default());
        log.row(&Snapshot { t: 0.5, ..Default::default() }, &FrameSummary::default());
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], COLUMNS.join(","));
        assert!(lines[2].starts_with("0.500,"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
