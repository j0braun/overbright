//! The frame statistics the overlay shows and logs — FPS and frame
//! times over the last second and what the frames were spent on (the
//! frame loop's [`FrameRecord`]s), the 1 % and 0.1 % lows over the last [`LOWS_SECONDS`], the frame-time
//! graph's last [`GRAPH_LEN`] frames and the frame pacing histogram.
//!
//! The lows follow MangoHud's reading of them (MIT; its `fps_metrics`):
//! the frame rate of the frame time at the 99th / 99.9th percentile of the
//! window, i.e. "1 % of frames were slower than this" (not the mean of the
//! slowest 1 %).

use std::collections::VecDeque;

/// The lows' window, seconds.
pub const LOWS_SECONDS: f32 = 10.0;
/// The most frame times the lows keep (10 s at 3000 FPS).
const LOWS_MAX: usize = 30_000;
/// The graph's frames (MangoHud's default history is 200 too).
pub const GRAPH_LEN: usize = 200;
/// The pacing histogram's upper bin edges, ms of |frame time − median|.
pub const PACING_EDGES: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];
pub const PACING_BINS: usize = PACING_EDGES.len() + 1;

/// One drawn frame as the frame loop measured it, seconds unless said.
/// The parts are the main thread's; they add up, with the time between
/// frames the loop does not see (the event loop, the overlay itself), to
/// the frame's interval: `interval ≈ cap + sim + probe_time + draw + wait`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameRecord {
    /// From the start of the drawn frame before to this one's start:
    /// everything, the frame limiter's sleep and the display's waits too.
    pub interval: f32,
    /// The frame limiter's sleep before this frame (the frame cap, Power
    /// Saving, Low Latency's pacer).
    pub cap: f32,
    /// Stepping the simulation's ticks (without the light-trail renders
    /// between them).
    pub sim: f32,
    /// Ticks stepped by this frame (0 when it fell between two ticks).
    pub ticks: u32,
    /// Real time the tick cap dropped in this frame: the frame was longer
    /// than the most ticks one frame may take, so the game ran slower.
    pub dropped: f32,
    /// Drawing the frame (layout, recording, submission), without the
    /// waits and the light-trail renders.
    pub draw: f32,
    /// Light-trail renders (the streak buffer's offscreen scene renders,
    /// "probes") drawn in this frame, and their time without their waits.
    pub probes: u32,
    pub probe_time: f32,
    /// Waiting for the display: the frame before presented (`settle`) and
    /// a swap-chain image to draw into (`acquire`).
    pub wait: f32,
    /// Of `wait`, the swap-chain image's (the graph's stall note).
    pub acquire: f32,
    /// The GPU's busy time of the frames read back during this frame (the
    /// sum of the passes between timestamps, without the gaps between
    /// submissions, `prof::gpu`; read back a few frames late, the
    /// light-trail renders' too), and how many were read.
    pub gpu: f32,
    pub gpu_frames: u32,
    /// The GPU timestamps were running for this frame.
    pub gpu_timed: bool,
    /// The light-trail buffer's form: None when it is not in use, else
    /// whether this frame was in the exact form (probes allowed).
    pub streak_exact: Option<bool>,
}

/// One refresh's figures (also the CSV log's frame columns). Every
/// figure but the lows is over the last second of frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSummary {
    /// Drawn frames in the last second.
    pub frames: u32,
    /// Drawn frames per second: `frames` / their intervals' sum.
    pub fps: f32,
    /// Mean, 99th percentile and longest frame interval, ms.
    pub avg_ms: f32,
    pub p99_ms: f32,
    pub max_ms: f32,
    /// 1 % and 0.1 % low frame rates over [`LOWS_SECONDS`].
    pub low1_fps: f32,
    pub low01_fps: f32,
    /// Per drawn frame, ms: the simulation's ticks ("update"), the
    /// drawing, the light-trail renders, the display's waits (of them the
    /// swap-chain acquire), the frame limiter's sleep and the rest of the
    /// interval (`avg_ms` less all of these, at least 0).
    pub update_ms: f32,
    pub draw_ms: f32,
    pub probe_ms: f32,
    pub wait_ms: f32,
    pub acquire_ms: f32,
    pub cap_ms: f32,
    pub other_ms: f32,
    /// Ticks per second of real time, and the simulation's time per tick, ms.
    pub ticks_per_s: f32,
    pub tick_ms: f32,
    /// The game's speed: 1 less the share of real time the tick cap
    /// dropped (1: full speed; below: frames too long, the game slowed).
    pub speed: f32,
    /// Light-trail renders per second.
    pub probes_per_s: f32,
    /// The share of frames in the light trails' exact form (None: the
    /// buffer was not in use).
    pub streak_exact: Option<f32>,
    /// The GPU's busy time per drawn frame, ms, and as a share of the
    /// time (0..1). None: not measured (`gpu_state`).
    pub gpu_ms: Option<f32>,
    pub gpu_load: Option<f32>,
    pub gpu_state: GpuState,
}

/// Why the GPU figure is or is not there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuState {
    /// No timestamps read yet (they start with the overlay).
    #[default]
    Waiting,
    /// Measured.
    Timed,
    /// Not timed: the device has no timestamp queries (or the bench
    /// times the GPU itself).
    Untimed,
}

#[derive(Debug, Default)]
pub struct FrameStats {
    /// The last second's frames.
    second: VecDeque<FrameRecord>,
    second_total: f32,
    /// Intervals, s, the last [`LOWS_SECONDS`].
    history: VecDeque<f32>,
    history_total: f32,
    /// Intervals, ms, the last [`GRAPH_LEN`] frames.
    graph: VecDeque<f32>,
    /// The swap-chain acquire of the same frames, ms (what of a
    /// stall was the display holding every image: wgpu-core 30 waits up
    /// to FRAME_TIMEOUT_MS = 1000 ms in `get_current_texture`).
    graph_acquire: VecDeque<f32>,
    scratch: Vec<f32>,
}

pub use crate::prof::percentile;

impl FrameStats {
    pub fn push(&mut self, f: FrameRecord) {
        let interval = f.interval;
        self.second.push_back(f);
        self.second_total += interval;
        while self.second_total > 1.0 && self.second.len() > 1 {
            self.second_total -= self.second.pop_front().map_or(0.0, |f| f.interval);
        }
        self.history.push_back(interval);
        self.history_total += interval;
        while (self.history_total > LOWS_SECONDS && self.history.len() > 1) || self.history.len() > LOWS_MAX {
            self.history_total -= self.history.pop_front().unwrap_or(0.0);
        }
        self.graph.push_back(interval * 1000.0);
        self.graph_acquire.push_back(f.acquire * 1000.0);
        if self.graph.len() > GRAPH_LEN {
            self.graph.pop_front();
            self.graph_acquire.pop_front();
        }
    }

    /// The graph's longest frame and its swap-chain acquire, ms.
    pub fn worst(&self) -> (f32, f32) {
        self.graph.iter().zip(&self.graph_acquire).fold((0.0, 0.0), |w, (&ms, &acq)| if ms > w.0 { (ms, acq) } else { w })
    }

    /// The last second's frames.
    pub fn second(&self) -> impl Iterator<Item = &FrameRecord> {
        self.second.iter()
    }

    /// The graph's frame times, ms, oldest first.
    pub fn graph(&self) -> impl ExactSizeIterator<Item = &f32> {
        self.graph.iter()
    }

    pub fn summary(&mut self) -> FrameSummary {
        let n = self.second.len();
        if n == 0 {
            return FrameSummary::default();
        }
        // Recomputed, not the running sum (no drift over hours).
        let total: f32 = self.second.iter().map(|f| f.interval).sum();
        self.second_total = total;
        let nf = n as f32;
        self.scratch.clear();
        self.scratch.extend(self.second.iter().map(|f| f.interval));
        let p99 = percentile(&mut self.scratch, 0.99);
        let max = self.scratch.iter().copied().fold(0.0, f32::max);
        let mut sum = FrameRecord::default();
        let (mut exact, mut streak) = (0u32, 0u32);
        for f in &self.second {
            sum.cap += f.cap;
            sum.sim += f.sim;
            sum.ticks += f.ticks;
            sum.dropped += f.dropped;
            sum.draw += f.draw;
            sum.probes += f.probes;
            sum.probe_time += f.probe_time;
            sum.wait += f.wait;
            sum.acquire += f.acquire;
            sum.gpu += f.gpu;
            sum.gpu_frames += f.gpu_frames;
            sum.gpu_timed |= f.gpu_timed;
            if let Some(e) = f.streak_exact {
                streak += 1;
                exact += u32::from(e);
            }
        }
        self.scratch.clear();
        self.scratch.extend(self.history.iter().copied());
        let low = |s: &mut Vec<f32>, q: f32| {
            let t = percentile(s, q);
            if t > 0.0 { 1.0 / t } else { 0.0 }
        };
        let low1_fps = low(&mut self.scratch, 0.99);
        let low01_fps = low(&mut self.scratch, 0.999);
        let per_frame = |v: f32| v / nf * 1000.0;
        let avg_ms = total / nf * 1000.0;
        let parts = sum.cap + sum.sim + sum.draw + sum.probe_time + sum.wait;
        let total = total.max(1e-6);
        // Every frame's GPU work is read back once, in order, a few frames
        // late: over the second the sum read is the sum done.
        let (gpu_ms, gpu_load, gpu_state) = if !sum.gpu_timed {
            (None, None, GpuState::Untimed)
        } else if sum.gpu_frames == 0 {
            (None, None, GpuState::Waiting)
        } else {
            (Some(sum.gpu / nf * 1000.0), Some((sum.gpu / total).min(1.0)), GpuState::Timed)
        };
        FrameSummary {
            frames: n as u32,
            fps: nf / total,
            avg_ms,
            p99_ms: p99 * 1000.0,
            max_ms: max * 1000.0,
            low1_fps,
            low01_fps,
            update_ms: per_frame(sum.sim),
            draw_ms: per_frame(sum.draw),
            probe_ms: per_frame(sum.probe_time),
            wait_ms: per_frame(sum.wait),
            acquire_ms: per_frame(sum.acquire),
            cap_ms: per_frame(sum.cap),
            other_ms: (avg_ms - per_frame(parts)).max(0.0),
            ticks_per_s: sum.ticks as f32 / total,
            tick_ms: if sum.ticks > 0 { sum.sim / sum.ticks as f32 * 1000.0 } else { 0.0 },
            speed: (1.0 - sum.dropped / total).clamp(0.0, 1.0),
            probes_per_s: sum.probes as f32 / total,
            streak_exact: (streak > 0).then(|| exact as f32 / streak as f32),
            gpu_ms,
            gpu_load,
            gpu_state,
        }
    }

    /// The pacing histogram over the last second: the share (0..1) of
    /// frames whose interval is within each [`PACING_EDGES`] bin of the
    /// median interval (the last bin: beyond 4 ms). Even pacing puts
    /// everything in the first bin.
    pub fn pacing(&mut self) -> [f32; PACING_BINS] {
        let mut bins = [0.0; PACING_BINS];
        if self.second.is_empty() {
            return bins;
        }
        self.scratch.clear();
        self.scratch.extend(self.second.iter().map(|f| f.interval));
        let median = percentile(&mut self.scratch, 0.5);
        let n = self.second.len() as f32;
        for f in &self.second {
            let dev = (f.interval - median).abs() * 1000.0;
            let i = PACING_EDGES.iter().position(|&e| dev <= e).unwrap_or(PACING_BINS - 1);
            bins[i] += 1.0 / n;
        }
        bins
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_clock::{RACE, SCENE, TickClock, UNITS};

    fn rec(interval: f32) -> FrameRecord {
        FrameRecord { interval, ..Default::default() }
    }

    /// A display schedule through the game's own tick clock: `frames`
    /// frames of `interval` s, each tick costing `tick` s of simulation,
    /// `draw` s of drawing, the rest of the interval waited for the
    /// display; one light-trail render per tick beyond a frame's first
    /// when `probes` (the exact form).
    fn schedule(s: &mut FrameStats, interval: f32, frames: usize, tick: f32, draw: f32, probes: bool) {
        let mut clock = TickClock::default();
        for _ in 0..frames {
            let units = clock.units_of_secs(interval);
            let mut f = clock.begin(units);
            let mut ticks = 0u32;
            while clock.tick(&mut f, RACE).is_some() {
                ticks += 1;
            }
            let p = if probes { ticks.saturating_sub(1) } else { 0 };
            let sim = tick * ticks as f32;
            let probe_time = 0.001 * p as f32;
            s.push(FrameRecord {
                interval,
                sim,
                ticks,
                dropped: f.dropped as f32 / UNITS as f32,
                draw,
                probes: p,
                probe_time,
                wait: (interval - sim - draw - probe_time).max(0.0),
                streak_exact: Some(probes),
                ..Default::default()
            });
        }
    }

    /// FPS counts drawn frames, not ticks or light-trail renders; the
    /// ticks are 60 a second at every display rate; the update time is per
    /// drawn frame (a frame without a tick adds 0, one with two ticks
    /// both) and per tick; the parts add up to the frame time.
    #[test]
    fn every_display_rate_counts_drawn_frames_and_sixty_ticks() {
        for hz in [30.0f32, 60.0, 75.0, 144.0, 200.0] {
            let mut s = FrameStats::default();
            schedule(&mut s, 1.0 / hz, (hz * 3.0) as usize, 0.002, 0.003, true);
            let m = s.summary();
            assert!((m.fps - hz).abs() < 0.01 * hz, "{hz}: {m:?}");
            assert!((m.avg_ms - 1000.0 / hz).abs() < 0.01, "{hz}: {m:?}");
            assert!((m.ticks_per_s - 60.0).abs() <= 1.5, "{hz}: {m:?}");
            assert!((m.tick_ms - 2.0).abs() < 1e-3, "{hz}: {m:?}");
            assert!((m.update_ms - 2.0 * m.ticks_per_s / m.fps).abs() < 0.05, "{hz}: {m:?}");
            assert!((m.draw_ms - 3.0).abs() < 1e-3, "{hz}: {m:?}");
            // A probe per tick beyond a frame's first: only at 30 Hz.
            let probes = if hz == 30.0 { 30.0 } else { 0.0 };
            assert!((m.probes_per_s - probes).abs() <= 1.0, "{hz}: {m:?}");
            assert_eq!(m.speed, 1.0, "{hz}: nothing dropped");
            let parts = m.update_ms + m.draw_ms + m.probe_ms + m.wait_ms + m.cap_ms + m.other_ms;
            assert!((parts - m.avg_ms).abs() < 0.05 || m.wait_ms == 0.0, "{hz}: {m:?}");
        }
    }

    /// The frame cap's sleep is part of the frame time and shown as such.
    #[test]
    fn the_caps_sleep_is_in_the_frame_time_and_its_own_part() {
        let mut s = FrameStats::default();
        for _ in 0..200 {
            s.push(FrameRecord { interval: 1.0 / 60.0, cap: 0.010, sim: 0.001, ticks: 1, draw: 0.004, wait: 0.0016667, ..Default::default() });
        }
        let m = s.summary();
        assert!((m.fps - 60.0).abs() < 0.1 && (m.cap_ms - 10.0).abs() < 1e-3, "{m:?}");
        assert!(m.other_ms < 0.01, "{m:?}");
    }

    /// Below 20 fps the tick cap (3 ticks a frame) drops time: the game
    /// runs slower than real time, and the speed says by how much.
    #[test]
    fn frames_longer_than_three_ticks_slow_the_game_and_show_it() {
        let mut s = FrameStats::default();
        schedule(&mut s, 1.0 / 15.0, 30, 0.002, 0.03, false);
        let m = s.summary();
        assert!((m.fps - 15.0).abs() < 0.1, "{m:?}");
        assert!((m.ticks_per_s - 45.0).abs() < 1.0, "{m:?}");
        assert!((m.speed - 0.75).abs() < 0.01, "{m:?}");
        // At 20 fps nothing is dropped yet.
        let mut s = FrameStats::default();
        schedule(&mut s, 1.0 / 20.0, 40, 0.002, 0.03, false);
        assert_eq!(s.summary().speed, 1.0);
    }

    /// A stall (a load, a long frame) is one long frame: one spike in the
    /// graph, the longest frame, the lows for their window; the time past
    /// three ticks is dropped (the game holds still, it does not race to
    /// catch up), and the second after it says so.
    #[test]
    fn a_stall_is_one_long_frame() {
        let mut s = FrameStats::default();
        schedule(&mut s, 1.0 / 60.0, 120, 0.001, 0.002, false);
        let mut clock = TickClock::default();
        let units = clock.units_of_secs(0.5);
        let mut f = clock.begin(units);
        let mut ticks = 0;
        while clock.tick(&mut f, RACE).is_some() {
            ticks += 1;
        }
        assert_eq!(ticks, 3);
        s.push(FrameRecord { interval: 0.5, ticks, dropped: f.dropped as f32 / UNITS as f32, ..Default::default() });
        let m = s.summary();
        assert!((m.max_ms - 500.0).abs() < 1e-3, "{m:?}");
        assert_eq!(s.graph().filter(|&&ms| ms > 100.0).count(), 1);
        assert!(m.speed < 0.6, "{m:?}");
        assert!(m.low01_fps <= 2.0 + 1e-3, "{m:?}");
        // A second later the speed is whole again; the lows still hold it.
        schedule(&mut s, 1.0 / 60.0, 60, 0.001, 0.002, false);
        let m = s.summary();
        assert_eq!(m.speed, 1.0);
        assert!((m.max_ms - 1000.0 / 60.0).abs() < 0.01, "{m:?}");
        assert!(m.low01_fps <= 2.0 + 1e-3, "{m:?}");
    }

    /// A scene's 1/30 s ticks: 30 a second, at most two a frame.
    #[test]
    fn scene_ticks_are_thirty_a_second() {
        let mut s = FrameStats::default();
        let mut clock = TickClock::default();
        for _ in 0..144 {
            let units = clock.units_of_secs(1.0 / 144.0);
            let mut f = clock.begin(units);
            let mut ticks = 0;
            while clock.tick(&mut f, SCENE).is_some() {
                ticks += 1;
            }
            s.push(FrameRecord { interval: 1.0 / 144.0, ticks, sim: 0.001 * ticks as f32, ..Default::default() });
        }
        let m = s.summary();
        assert!((m.ticks_per_s - 30.0).abs() <= 1.0, "{m:?}");
    }

    /// The GPU time is per drawn frame: the busy time read back over the
    /// second (in bursts, a few frames late; the light-trail renders'
    /// too) over the frames drawn; its share of the time is the load.
    /// Untimed (no timestamps) and not yet read say so.
    #[test]
    fn gpu_time_is_the_busy_time_read_back_per_drawn_frame() {
        let mut s = FrameStats::default();
        for i in 0..120 {
            // 6 ms of GPU work a drawn frame (a frame and a probe of 3 ms),
            // read back three frames at a time.
            let (gpu, n) = if i % 3 == 0 { (0.018, 6) } else { (0.0, 0) };
            s.push(FrameRecord { interval: 1.0 / 120.0, probes: 1, gpu, gpu_frames: n, gpu_timed: true, ..Default::default() });
        }
        let m = s.summary();
        assert_eq!(m.gpu_state, GpuState::Timed);
        assert!((m.gpu_ms.unwrap() - 6.0).abs() < 1e-3, "{m:?}");
        assert!((m.gpu_load.unwrap() - 0.72).abs() < 1e-3, "{m:?}");
        let mut s = FrameStats::default();
        for _ in 0..60 {
            s.push(rec(1.0 / 60.0));
        }
        assert_eq!((s.summary().gpu_ms, s.summary().gpu_state), (None, GpuState::Untimed));
        let mut s = FrameStats::default();
        s.push(FrameRecord { interval: 1.0 / 60.0, gpu_timed: true, ..Default::default() });
        assert_eq!((s.summary().gpu_ms, s.summary().gpu_state), (None, GpuState::Waiting));
    }

    /// The light trails' form: the share of frames in the exact form;
    /// none when the buffer is not in use.
    #[test]
    fn the_light_trails_form_is_a_share_of_frames() {
        let mut s = FrameStats::default();
        for i in 0..100 {
            s.push(FrameRecord { interval: 0.005, streak_exact: Some(i % 4 != 0), ..Default::default() });
        }
        assert_eq!(s.summary().streak_exact, Some(0.75));
        let mut s = FrameStats::default();
        s.push(rec(0.005));
        assert_eq!(s.summary().streak_exact, None);
    }

    /// A frame that waited a second for a swap-chain image (the
    /// compositor held them all) is told apart from one the game spent.
    #[test]
    fn the_worst_frame_keeps_its_acquire_wait() {
        let mut s = FrameStats::default();
        for i in 0..300 {
            let (interval, acquire) = if i == 250 { (1.0075, 1.0) } else { (0.02, 0.015) };
            s.push(FrameRecord { interval, acquire, wait: acquire, ..Default::default() });
        }
        let (ms, acq) = s.worst();
        assert!((ms - 1007.5).abs() < 0.01 && (acq - 1000.0).abs() < 0.01, "{ms} {acq}");
        for _ in 0..GRAPH_LEN {
            s.push(rec(0.02));
        }
        assert!((s.worst().0 - 20.0).abs() < 1e-3 && s.worst().1 == 0.0);
    }

    #[test]
    fn even_frames_have_no_lows_below_the_rate() {
        let mut s = FrameStats::default();
        for _ in 0..600 {
            s.push(FrameRecord { interval: 1.0 / 120.0, sim: 0.001, draw: 0.002, ..Default::default() });
        }
        let m = s.summary();
        assert_eq!(m.frames, 120);
        assert!((m.fps - 120.0).abs() < 0.5, "{m:?}");
        assert!((m.low1_fps - 120.0).abs() < 0.5 && (m.low01_fps - 120.0).abs() < 0.5, "{m:?}");
        assert!((m.update_ms - 1.0).abs() < 1e-3 && (m.draw_ms - 2.0).abs() < 1e-3);
        assert!((s.pacing()[0] - 1.0).abs() < 1e-4);
        assert_eq!(s.graph().len(), GRAPH_LEN);
    }

    #[test]
    fn a_stutter_shows_in_the_lows_and_the_histogram() {
        let mut s = FrameStats::default();
        // 100 FPS with one 50 ms frame every 50 frames (2 %), 5.4 s.
        for i in 0..500 {
            s.push(rec(if i % 50 == 49 { 0.05 } else { 0.01 }));
        }
        let m = s.summary();
        // 2 % of the frames are the 50 ms ones: both percentiles are one.
        assert!((m.low1_fps - 20.0).abs() < 0.1, "{m:?}");
        assert!((m.low01_fps - 20.0).abs() < 0.1, "{m:?}");
        assert!((m.max_ms - 50.0).abs() < 1e-3);
        let p = s.pacing();
        assert!(p[PACING_BINS - 1] > 0.0 && p[0] > 0.9, "{p:?}");
    }

    #[test]
    fn percentile_is_the_overlay_index() {
        let mut v: Vec<f32> = (1..=100).map(|i| i as f32).rev().collect();
        assert_eq!(percentile(&mut v, 0.99), 100.0);
        assert_eq!(percentile(&mut v, 0.5), 51.0);
        assert_eq!(percentile(&mut [], 0.5), 0.0);
    }
}
