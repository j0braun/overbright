//! `NFSU2_BENCH_FRAMELOG=file` (measuring aid, docs/perf-budget.md "CPU frame time and hitches"):
//! one tab-separated line per timed frame of a bench, for the tail of a long drive — what each
//! frame cost on which thread and what happened in it, so a slow frame can be given its cause.
//!
//! Columns: frame, update ms, update CPU ms, draw ms, draw CPU ms, throttle ms, ticks run, the
//! main thread's CPU over the whole frame (ms, from the thread's CPU clock; the update and draw
//! CPU columns are the bench's, read from the scheduler's statistics, which a running thread
//! updates only at scheduler events, so a single frame's split between them is approximate), the
//! CPU of the other threads (ms; the render workers apart), pipelines and shader modules built,
//! the camera's section, traffic cars, active encounter cars, the draw's and the tick's parts
//! (`prof::acc` spans) over 0.25 ms (`NFSU2_BENCH_FRAMELOG_MIN_US`) as `label=ms`. Reading every thread's CPU clock costs the
//! main thread about 0.1 ms a frame, outside the update and draw columns.
use crate::*;
use std::io::Write as _;

pub(crate) struct FrameLog {
    out: std::io::BufWriter<std::fs::File>,
    main_tid: u64,
    main_cpu: u64,
    threads: Vec<(u64, String, u64)>,
    builds: (u64, u64),
}

/// One frame's figures, as the bench loop has them.
pub(crate) struct FrameRow<'a> {
    pub(crate) frame: usize,
    pub(crate) update_ms: f64,
    pub(crate) update_cpu_ns: u64,
    pub(crate) draw_ms: f64,
    pub(crate) draw_cpu_ns: u64,
    pub(crate) throttle_ms: f64,
    /// The ticks the frame ran (1 on the bench's fixed step).
    pub(crate) ticks: usize,
    pub(crate) parts: &'a [(&'static str, u64, u64)],
}

impl FrameLog {
    /// The log when `NFSU2_BENCH_FRAMELOG` names a file that could be made.
    pub(crate) fn open() -> Option<FrameLog> {
        let path = nfsu2::env_value_os!("NFSU2_BENCH_FRAMELOG")?;
        let mut out = std::io::BufWriter::new(std::fs::File::create(path).ok()?);
        let _ = writeln!(
            out,
            "frame\tupdate_ms\tupdate_cpu\tdraw_ms\tdraw_cpu\tthrottle_ms\tticks\tmain_cpu\tother_cpu\trender_cpu\tpipelines\tmodules\tsection\ttraffic\tencounters\tparts"
        );
        let main_tid = main_tid();
        Some(FrameLog { out, main_tid, main_cpu: thread_cpu(), threads: nfsu2::prof::threads(), builds: builds() })
    }

    pub(crate) fn line(&mut self, game: &Game, row: FrameRow) {
        let cpu = thread_cpu();
        let main = cpu.saturating_sub(self.main_cpu);
        self.main_cpu = cpu;
        let now = nfsu2::prof::threads();
        let (mut other, mut render) = (0u64, 0u64);
        for (tid, name, ns) in &now {
            if *tid == self.main_tid {
                continue;
            }
            let before = self.threads.iter().find(|t| t.0 == *tid).map_or(0, |t| t.2);
            let d = ns.saturating_sub(before);
            other += d;
            if name.starts_with("render") {
                render += d;
            }
        }
        self.threads = now;
        let b = builds();
        let (pipelines, modules) = (b.0 - self.builds.0, b.1 - self.builds.1);
        self.builds = b;
        let section = game
            .sim
            .drive
            .as_ref()
            .and_then(|d| d.world.visible_sections_at(d.camera.position))
            .map_or(-1, |(s, _)| i64::from(s));
        let race_traffic = match &game.mode {
            Mode::Race(r) => r.racers.traffic.as_ref(),
            _ => None,
        };
        let traffic = game.sim.traffic.as_ref().or(race_traffic).map_or(0, |t| t.cars.len());
        let encounters = game.sim.encounters.as_ref().map_or(0, |e| e.cars.iter().filter(|c| c.active).count());
        // NFSU2_BENCH_FRAMELOG_MIN_US=n: the parts from n µs on (default
        // 250; 0 every part, for a part's distribution over a run).
        let min_ns = nfsu2::env_value!("NFSU2_BENCH_FRAMELOG_MIN_US").and_then(|v| v.parse::<u64>().ok()).unwrap_or(250) * 1000;
        let mut parts: Vec<_> = row.parts.iter().filter(|p| p.1 >= min_ns).collect();
        parts.sort_by_key(|p| std::cmp::Reverse(p.1));
        let parts: Vec<String> =
            parts.iter().map(|(l, ns, _)| format!("{}={:.3}", l.trim(), *ns as f64 / 1e6)).collect();
        let ms = |ns: u64| ns as f64 / 1e6;
        let _ = writeln!(
            self.out,
            "{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}\t{:.3}\t{:.3}\t{:.3}\t{pipelines}\t{modules}\t{section}\t{traffic}\t{encounters}\t{}",
            row.frame,
            row.update_ms,
            ms(row.update_cpu_ns),
            row.draw_ms,
            ms(row.draw_cpu_ns),
            row.throttle_ms,
            row.ticks,
            ms(main),
            ms(other),
            ms(render),
            parts.join(",")
        );
    }
}

fn builds() -> (u64, u64) {
    (nfsu2::render::pipelines_built(), nfsu2::render::core::device_cache::modules_built())
}

/// The calling thread's kernel id (Linux; 0 elsewhere, where `prof::threads` lists none).
fn main_tid() -> u64 {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: gettid has no preconditions.
        unsafe { libc::gettid() as u64 }
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// The calling thread's CPU clock (CLOCK_THREAD_CPUTIME_ID), ns (Linux; 0 elsewhere).
fn thread_cpu() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: clock_gettime writes the timespec it is given.
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
        ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}
