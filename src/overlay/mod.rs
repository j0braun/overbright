//! The performance overlay, MangoHud-style (docs/perf-overlay.md). A
//! clone tool, not a game feature: F3 cycles hidden → Minimal → Standard →
//! Full → hidden (Shift+F3 hides it); the Enhancements page picks the
//! layout shown at start, its corner and scale, and a CSV log of every
//! metric ([`csv`]). The three layouts are one panel in one look (font,
//! colours, columns, corner, scale). Minimal is its first rows: FPS, the
//! frame time and its lows, what a frame was spent on (the simulation, the
//! drawing, the light-trail renders, the display's and the frame cap's
//! waits), the GPU's time, a warning when the game runs slower than real
//! time, the deviations on; Standard adds the ticks, the CPU / GPU / memory
//! figures and the frame-time graph; Full adds per-core loads, per-thread
//! CPU, I/O, the last load's time, the pacing histogram and the GPU time
//! per pass (`prof::gpu` timestamps).
//!
//! The frame loop measures each drawn frame ([`stats::FrameRecord`]); every
//! figure is computed from those records by [`stats::FrameStats`], which the
//! tests drive with known schedules, without a window.
//!
//! Cost: the frame figures are kept on the draw thread (a push per
//! frame); the system is read on the "perf sampler" thread
//! ([`sampler`], 4 Hz) only while Standard / Full is shown or the log is
//! on; the quads are rebuilt four times a second and copied in between.
//! Default (Off, nothing shown): nothing runs.
//!
//! Layout and colours follow MangoHud's (MIT, github.com/flightlessmango/
//! MangoHud: its default colours and row order); no code is taken from it.

pub mod csv;
mod panel;
pub mod sampler;
mod settings;
pub mod stats;
mod system;
#[cfg(target_os = "linux")]
pub use system::nvml;
pub use system::sources;

pub use panel::{frame_rows, wrap};
pub use settings::{LAYOUT_NAMES, Layout, POSITION_NAMES, SCALES, Settings};

use crate::frontend::Quad;
use std::sync::atomic::{AtomicU32, Ordering};

/// The last track load's time, ms (0: none yet), from the loading gate.
static LAST_LOAD_MS: AtomicU32 = AtomicU32::new(0);

pub fn record_load(ms: f32) {
    LAST_LOAD_MS.store(ms.to_bits(), Ordering::Relaxed);
}

pub fn last_load_ms() -> Option<f32> {
    Some(f32::from_bits(LAST_LOAD_MS.load(Ordering::Relaxed))).filter(|m| *m > 0.0)
}

/// What the window shows with: the swap chain's present mode and frame
/// latency, Low Latency, the frame cap, the adapter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayInfo {
    pub present: String,
    pub latency: u32,
    pub low_latency: bool,
    pub frame_cap: u32,
    pub size: (u32, u32),
    pub adapter: String,
}

/// The draw thread's side: frame figures, the sampler, the cached quads.
#[derive(Default)]
pub struct Overlay {
    /// The layout shown (None: hidden).
    pub shown: Option<Layout>,
    stats: stats::FrameStats,
    since: f32,
    summary: stats::FrameSummary,
    pacing: [f32; stats::PACING_BINS],
    /// The figures were computed at least once.
    refreshed: bool,
    sampler: Option<sampler::Sampler>,
    snapshot: Option<sampler::Snapshot>,
    /// GPU ms per pass (Full), busiest first; None while timestamps are off.
    passes: Option<Vec<(&'static str, f32)>>,
    /// The timestamps were started by the overlay (not by the bench).
    gpu_timing: Option<bool>,
    cache: Vec<Quad>,
    cache_key: Option<(Option<Layout>, Settings)>,
    dirty: bool,
    /// The log file being written.
    pub csv_path: Option<std::path::PathBuf>,
    /// The GPU drawn on (PCI vendor, device).
    pub pci: Option<(u32, u32)>,
}

/// The refresh period of the text and the quads, s (the F3 overlay's).
const REFRESH: f32 = 0.25;

impl Overlay {
    /// The overlay at start: the layout the Enhancements row picks, or
    /// `$NFSU2_OVERLAY` (off / minimal / standard / full).
    pub fn new(s: &Settings) -> Overlay {
        let env = std::env::var("NFSU2_OVERLAY").ok().map(|v| match v.to_ascii_lowercase().as_str() {
            "1" | "minimal" => 1,
            "2" | "standard" => 2,
            "3" | "full" => 3,
            _ => 0,
        });
        Overlay { shown: Layout::from_index(env.unwrap_or(s.layout)), ..Overlay::default() }
    }

    /// F3: the next layout; `hide` (Shift+F3): hidden.
    pub fn cycle(&mut self, hide: bool) {
        // Shown again after a while hidden: the frames from before are not
        // the last second's (frames are counted only while active).
        if self.shown.is_none() {
            self.stats = stats::FrameStats::default();
        }
        self.shown = if hide { None } else { Layout::cycle(self.shown) };
        self.dirty = true;
        self.since = REFRESH;
    }

    /// Frames are counted: shown, or logging.
    pub fn active(&self, s: &Settings) -> bool {
        self.shown.is_some() || s.csv || std::env::var_os("NFSU2_PERF_CSV").is_some()
    }

    /// The sampler is needed: Standard / Full shown, or logging.
    fn wants_sampler(&self, s: &Settings) -> bool {
        self.shown.is_some_and(|l| l >= Layout::Standard) || s.csv || std::env::var_os("NFSU2_PERF_CSV").is_some()
    }

    /// One drawn frame's measurements. Four times a second the figures
    /// are refreshed (and the sampler started or stopped).
    pub fn push(&mut self, mut frame: stats::FrameRecord, s: &Settings) {
        // The GPU frames read back by now (the timestamps the overlay
        // started; the bench reads its own).
        if self.gpu_timing == Some(true) && crate::prof::gpu::active() {
            let read = crate::prof::gpu::take_frames();
            frame.gpu = (read.iter().sum::<f64>() / 1000.0) as f32;
            frame.gpu_frames = read.len() as u32;
            frame.gpu_timed = true;
        }
        self.stats.push(frame);
        self.since += frame.interval;
        if self.since < REFRESH && self.refreshed {
            return;
        }
        self.since = 0.0;
        self.refresh(s);
    }

    /// Recompute the figures now.
    pub fn refresh(&mut self, s: &Settings) {
        self.summary = self.stats.summary();
        self.pacing = self.stats.pacing();
        self.refreshed = true;
        if self.wants_sampler(s) {
            let sampler = self.sampler.get_or_insert_with(|| sampler::Sampler::start(self.pci, u32::from(s.hz)));
            sampler.set_hz(u32::from(s.hz));
            let logging = s.csv || std::env::var_os("NFSU2_PERF_CSV").is_some();
            if logging && !sampler.logging() {
                self.csv_path = sampler.set_csv(Some(&csv::default_path()));
                if let Some(p) = &self.csv_path {
                    log::info!("performance log: {}", p.display());
                }
            } else if !logging && sampler.logging() {
                sampler.set_csv(None);
                self.csv_path = None;
            }
            sampler.publish_frames(self.summary);
            if let Some(snap) = sampler.latest() {
                self.snapshot = Some(snap);
            }
        } else if self.sampler.take().is_some() {
            self.snapshot = None;
            self.csv_path = None;
        }
        if self.gpu_timing == Some(true) {
            // Taken every refresh (the totals do not pile up), shown in Full.
            let parts = crate::prof::gpu::take();
            if self.shown == Some(Layout::Full) {
                let mut v: Vec<(&'static str, f32)> = parts.into_iter().filter(|p| p.0 != crate::prof::gpu::GAP && p.2 > 0).map(|p| (p.0, (p.1 / p.2 as f64) as f32)).collect();
                v.sort_by(|a, b| b.1.total_cmp(&a.1));
                if !v.is_empty() || self.passes.is_none() {
                    self.passes = Some(v);
                }
            }
        }
        self.dirty = true;
    }

    /// Start or pause the GPU timestamps (after the frame's submissions;
    /// `prof::gpu`): on while the overlay is shown or logging. The bench's
    /// own timing is left alone.
    pub fn sync_gpu_timing(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, s: &Settings) {
        let want = self.active(s);
        match self.gpu_timing {
            None if want => {
                if crate::prof::gpu::ready() {
                    self.gpu_timing = Some(false);
                } else {
                    let ok = crate::prof::gpu::init(device, queue);
                    self.gpu_timing = Some(ok);
                }
            }
            Some(true) => {
                if crate::prof::gpu::active() != want {
                    crate::prof::gpu::set_active(want);
                }
            }
            _ => {}
        }
    }

    /// Fill in figures for a still (`--shot` with `NFSU2_SHOT_FPS`): 60 FPS
    /// with a slower frame now and then, and (Standard, Full) a sample of
    /// this machine taken now.
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn fake_frames_for_shot(&mut self, s: &Settings) {
        // Every layout: a 60 FPS run with a slower frame now and then (the
        // graph and the lows show something).
        for i in 0..600 {
            let slow = i % 97 == 0;
            let interval = if slow { 1.0 / 30.0 } else { 1.0 / 60.0 };
            let ticks = if slow { 2 } else { 1 };
            self.stats.push(stats::FrameRecord {
                interval,
                sim: 0.0008 * ticks as f32,
                ticks,
                draw: 0.004,
                wait: interval - 0.0008 * ticks as f32 - 0.004,
                acquire: 0.0002,
                gpu: 0.0031,
                gpu_frames: 1,
                gpu_timed: true,
                streak_exact: Some(true),
                ..Default::default()
            });
        }
        self.refresh(s);
        if self.shown.is_some_and(|l| l >= Layout::Standard) {
            let src = sources::Sources::discover(std::path::Path::new("/"), self.pci);
            let started = std::time::Instant::now();
            let mut prev = sampler::Prev::default();
            sampler::sample(&src, &mut prev, started);
            std::thread::sleep(std::time::Duration::from_millis(250));
            self.snapshot = Some(sampler::sample(&src, &mut prev, started));
        }
        self.sampler = None;
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_writes_nothing() {
        let mut out = String::new();
        Settings::default().write(&mut out);
        assert!(out.is_empty());
        let s = Settings { layout: 3, position: 2, scale: 150, csv: true, hz: 8 };
        s.write(&mut out);
        let mut t = Settings::default();
        for l in out.lines() {
            let (k, v) = l.split_once(' ').unwrap();
            assert!(t.parse(k, v), "{l}");
        }
        assert_eq!(t, s);
        assert!(!t.parse("enh.fov", "3"));
        // Out of range values are held to the steps.
        assert!(t.parse("overlay.scale", "140") && t.scale == 150);
        assert!(t.parse("overlay.hz", "60") && t.hz == 10);
        assert!(t.parse("overlay.layout", "9") && t.layout == 3);
    }

    #[test]
    fn f3_cycles_every_layout_then_hides() {
        let mut shown = None;
        let mut seen = Vec::new();
        for _ in 0..4 {
            shown = Layout::cycle(shown);
            seen.push(shown);
        }
        assert_eq!(seen, [Some(Layout::Minimal), Some(Layout::Standard), Some(Layout::Full), None]);
        let mut s = Settings::default();
        s.step_scale(true);
        assert_eq!(s.scale, 125);
        s.step_scale(false);
        s.step_scale(false);
        assert_eq!(s.scale, 75);
        s.step_scale(false);
        assert_eq!(s.scale, 200);
    }

    /// The figure space as a plain space, for reading the text.
    fn plain(s: &str) -> String {
        s.replace('\u{2007}', " ")
    }

    /// The rows as "label: value", figure spaces read as spaces.
    fn lines(rows: &[([f32; 4], String, String)]) -> Vec<String> {
        rows.iter().map(|r| format!("{}: {}", r.1, plain(&r.2))).collect()
    }

    #[test]
    fn frame_rows_show_the_frame_and_its_parts() {
        let f = stats::FrameSummary {
            frames: 60,
            fps: 60.0,
            avg_ms: 16.67,
            p99_ms: 16.7,
            max_ms: 17.0,
            low1_fps: 55.0,
            low01_fps: 40.0,
            update_ms: 0.8,
            draw_ms: 4.0,
            wait_ms: 11.6,
            acquire_ms: 0.2,
            gpu_ms: Some(3.1),
            gpu_load: Some(0.186),
            gpu_state: stats::GpuState::Timed,
            speed: 1.0,
            ..Default::default()
        };
        assert_eq!(
            lines(&frame_rows(&f)),
            [
                "FPS:  60    16.67 ms   1% 55   0.1% 40",
                "Frame: 99%  16.7   max  17.0 ms",
                "CPU time: update  0.80   draw  4.00 ms",
                "Waits: display 11.60 ms",
                "GPU time:  3.10 ms   busy 19%",
            ]
        );
        assert_eq!(panel::slowed_text(&f), None);
        // Light-trail renders and the cap's sleep get their parts; a slowed
        // game its warning; no timestamps or no scene say so.
        let g = stats::FrameSummary {
            probes_per_s: 30.0,
            probe_ms: 1.25,
            cap_ms: 2.5,
            speed: 0.75,
            gpu_ms: None,
            gpu_load: None,
            gpu_state: stats::GpuState::Untimed,
            ..f
        };
        let t = lines(&frame_rows(&g));
        assert_eq!(t[2], "CPU time: update  0.80   draw  4.00   trails  1.25 ms");
        assert_eq!(t[3], "Waits: display 11.60   cap  2.50 ms");
        assert_eq!(t[4], "GPU time: n/a (no GPU timestamps)");
        assert_eq!(panel::slowed_text(&g).as_deref(), Some("game slowed to 75% speed (frames too long)"));
        let h = stats::FrameSummary { gpu_state: stats::GpuState::Waiting, ..g };
        assert_eq!(lines(&frame_rows(&h))[4], "GPU time: ...");
        // No frames yet.
        assert_eq!(lines(&frame_rows(&stats::FrameSummary::default()))[0], "FPS: n/a");
    }

    /// Numbers keep their width as they change: digits are drawn in cells
    /// of one width and padded with figure spaces.
    #[test]
    fn numbers_do_not_change_width() {
        use crate::frontend::font::{Font, Glyph};
        let glyph = |advance: f32| Glyph { width: advance, height: 10.0, x: 0.0, y: 0.0, x_offset: 0.0, y_offset: 0.0, advance, kerning: Vec::new() };
        let mut glyphs = crate::fxhash::FxHashMap::default();
        for (i, c) in ('0'..='9').enumerate() {
            glyphs.insert(u32::from(c), glyph(if c == '1' { 4.0 } else { 6.0 + i as f32 * 0.1 }));
        }
        for c in [' ', '.', 'm', 's', 'F', 'P', 'S'] {
            glyphs.insert(u32::from(c), glyph(5.0));
        }
        let font = Font { name: "t".into(), texture: "t".into(), line_height: 10.0, texture_size: [64.0, 64.0], glyphs };
        let w = |v: f32| panel::measure_tabular(&font, &format!("{} FPS {} ms", panel::num(v, 3, 0), panel::num(v / 7.0, 6, 2)));
        let base = w(144.0);
        for v in [1.0, 11.0, 60.0, 99.0, 111.0, 888.0] {
            assert_eq!(w(v), base, "{v}");
        }
    }

    #[test]
    fn deviations_wrap_at_commas() {
        assert_eq!(wrap("none", 10), ["none"]);
        assert_eq!(wrap("aaaa, bbbb, cccc", 11), ["aaaa, bbbb,", "cccc"]);
    }
}
