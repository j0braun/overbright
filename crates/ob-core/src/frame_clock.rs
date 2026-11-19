//! The frame time the world advances by (0x57EAE0, docs/original/uncapped-fps.md).
//!
//! **Verified** (disassembly 0x57EAE0..0x57ECB3, live under Wine
//! 2026-11-04): the main loop 0x5811D0 hands 0x57EAE0 the measured frame
//! time `raw` (s). With the normal path (0x8650CC = 0, 0x7FB700 ≠ 0, both
//! read live) it computes, in x87 precision,
//! `V = min(max(raw + carry, T), 3 × 1/60)` (T = 0x865558, the frame
//! limiter's period), `ticks = trunc(V × 1800 + 0.5)` (0x7A0624, 0x75BED4),
//! `q = trunc(ticks × 2.2222223 + 0.5)` and stores the float
//! `carry = V − q × 0.00025` (0x865144), `0x86513C = ticks` and
//! `0x865194 = ticks × (1/1800)`. The world update 0x5EA370 (0x5813DA)
//! takes `ticks × (1/1800)` (0x576CF0) times its time scale (+0x108, 1) and
//! hands it to the player mover 0x5B72E0, which splits it into
//! ⌊dt / 4.4 ms⌋ + 1 sub-steps. So the world advances in whole 1/1800 s
//! ticks, and because the carry is kept in 1/4000 s units, a steady frame
//! rate gives a repeating tick pattern: 60 fps 30, 29, 31; 120 fps 15, 16,
//! 14; 240 fps 8, 7, 7 (all three seen in the original's mover dt).
//!
//! The clone has no 1/120 limiter (drawing is uncapped, docs/playing/native-deviations.md), so T = 0
//! here, as in the original with its period set to 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameClock {
    /// 0x865144: the part of the measured time not yet given to the world.
    pub carry: f32,
}

/// 3 × 1/60 (0x7FB714 × 0x7875BC), the longest frame the world takes.
pub const MAX_FRAME: f32 = 3.0 * (1.0 / 60.0);

impl FrameClock {
    /// One frame of `raw` seconds: the world's dt (0x865194) and its tick
    /// count (0x86513C).
    pub fn frame(&mut self, raw: f32) -> (f32, u32) {
        self.frame_limited(raw, 0.0)
    }

    /// One frame with the frame limiter's period `limit` (0x865558: 1/120
    /// in free roam, 1/30 while a NIS scene holds the cars, 0x435F40):
    /// `V = min(max(raw + carry, limit), 3/60)`. At a steady `raw` equal to
    /// the limit this gives the original's tick patterns: 1/30 s frames
    /// 60, 60, 60, 60, 61 ticks (every scene clock captured under Wine,
    /// `docs/original/nis-audio-patterns.md`), 1/120 s frames 15, 15, 16, 15, 15.
    pub fn frame_limited(&mut self, raw: f32, limit: f32) -> (f32, u32) {
        // x87 extended precision in the original; f64 holds every value
        // these few operations produce at this magnitude.
        let v = (self.carry as f64 + raw as f64).max(limit as f64).max(0.0).min(MAX_FRAME as f64);
        let ticks = (v * 1800.0 + 0.5).floor();
        let q = (ticks * f32::from_bits(0x400E_38E4) as f64 + 0.5).floor();
        self.carry = (v - q * 0.000_25f32 as f64) as f32;
        (ticks as f32 * (1.0f32 / 1800.0), ticks as u32)
    }
}

/// The tick clock (agreed deviation, docs/playing/native-deviations.md "World
/// update rate": the original does not run at those high frame rates, so
/// the world works as at 30 or 60 fps, identical at 30,
/// 60, 120 and 200 fps). Real time is counted in whole [`UNITS`]
/// (1/36 000 s, exact for 30/50/60/72/75/90/100/120/144/200/240 Hz) and the
/// world advances by **ticks**: each tick is one update with the period and
/// the native frame clock's dt of its [`Rate`], which the caller picks from
/// the state before that tick (inside the catch-up loop, [`TickClock::tick`]):
/// [`RACE`] (1/60 s, 30, 29, 31 ticks of 1/1800 s) or [`SCENE`] (1/30 s, 60,
/// 60, 60, 60, 61). The native carry (0x865144) and so the 3-cycle's phase
/// run on from boot through every rate switch; the accumulator is kept in
/// real time across a switch. So the sequence of (rate, dt) is a function
/// of the tick states only, never of how the display sliced the time.
///
/// A drawn frame: [`TickClock::begin`] with its length, then
/// [`TickClock::tick`] until None (a tick while the accumulator holds at
/// least the period less [`SNAP_UNITS`] — the snap only for frames that
/// long themselves —, at most `rate.cap` ticks per frame, the time beyond
/// dropped: below 20 fps the game slows down as the original's 3/60 s cap
/// makes it), then [`TickClock::alpha`]: how far the drawing is from the
/// state before the last tick to the newest one (0 = exactly the state
/// before the last tick: a steady 60 Hz display draws tick states only).
///
/// There is no slow-frame branch: a 30 Hz frame is two RACE ticks, not one
/// tick of 1/30 s, and nothing holds the drawing after a slow frame
/// either.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickClock {
    /// Real time not yet given to the world ([`UNITS`]; down to
    /// −[`SNAP_UNITS`]: a tick taken that much early).
    pub acc: i64,
    /// 0x57EAE0's carry: every tick goes through it, whatever its rate.
    pub clock: FrameClock,
    /// The part of a unit not yet counted from intervals handed in seconds
    /// ([`TickClock::units_of_secs`]; |sub| ≤ 0.5).
    pub sub: f64,
    /// The part of a unit not yet counted from intervals handed in
    /// nanoseconds ([`TickClock::units_of_ns`]): ns × 9 mod 250 000.
    pub ns_rem: u64,
}

/// The old name ([`TickClock`]).
pub type Cadence = TickClock;

/// Real-time units per second (1/36 000 s).
pub const UNITS: i64 = 36_000;

/// One world update of real time at [`RACE`] (s).
pub const UPDATE: f64 = 1.0 / 60.0;
/// The measured time of a 1/60 s frame as the original's main loop hands
/// it over (0x5812EC: ms × 0.001 in float), which gives the 30, 29, 31
/// pattern.
pub const RAW_60: f32 = (1000.0f32 / 60.0) * 0.001;
/// The measured time of a 1/30 s frame (0x5812EC: ms × 0.001 in float).
pub const RAW_30: f32 = (1000.0f32 / 30.0) * 0.001;
/// A tick is taken when the accumulator is within this of a whole period,
/// so a 60 Hz display's jitter does not alternate 0 and 2 ticks. Only for
/// frames of about a period's length (the period less the snap or more):
/// at a high or uncapped rate a tick taken early held the drawn fraction
/// at 0 for two frames, then jumped (docs/notes/parity/first-race-regression.md); at
/// 3/4 of a period (80 Hz, R1) a 75 Hz display took every tick early
/// (docs/notes/front-end/postrace-jitter.md).
pub const SNAP: f64 = 0.002;
/// [`SNAP`] in [`UNITS`].
pub const SNAP_UNITS: i64 = 72;

/// One world cadence: a tick's real period and what it hands the native
/// clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rate {
    /// Real time per tick (s).
    pub update: f64,
    /// Real time per tick ([`UNITS`]).
    pub period: i64,
    /// The measured time each tick hands [`FrameClock`].
    pub raw: f32,
    /// The frame limiter's period then (0x865558).
    pub limit: f32,
    /// The most ticks one drawn frame takes; the time beyond is dropped.
    pub cap: u32,
}

/// Driving, free roam, menus over the world: the original at 60 frames a
/// second (30, 29, 31 ticks); at most 3 per drawn frame (0x7FB714 = 3: the
/// original's 3/60 s cap).
pub const RACE: Rate = Rate { update: UPDATE, period: UNITS / 60, raw: RAW_60, limit: 0.0, cap: 3 };

/// A scene that holds the cars (the race start and ending scenes, the
/// career's start, the dyno): the original runs it under its 1/30 s frame
/// limiter (0x435F40 sets 0x865558 = 1/30), so the world ticks once per
/// 1/30 s of real time with the limiter's 60, 60, 60, 60, 61 ticks
/// (`FrameClock::frame_limited`); at most 2 per drawn frame (3/60 s ≥ 1/30 s:
/// one native frame; the second catches up a frame that came late).
pub const SCENE: Rate = Rate { update: 1.0 / 30.0, period: UNITS / 30, raw: RAW_30, limit: 1.0 / 30.0, cap: 2 };

/// One drawn frame's catch-up loop ([`TickClock::begin`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameTicks {
    /// The frame's length ([`UNITS`]).
    pub units: i64,
    /// Ticks taken in it.
    pub taken: u32,
    /// Real time dropped at the cap ([`UNITS`]).
    pub dropped: i64,
}

impl FrameTicks {
    /// The snap for a tick of `rate` in this frame.
    fn snap(&self, rate: Rate) -> i64 {
        if self.units >= rate.period - SNAP_UNITS { SNAP_UNITS } else { 0 }
    }
}

impl TickClock {
    /// An interval in seconds as whole units, the part of a unit left over
    /// carried to the next (rounded, so an f32 a hair under the exact
    /// interval, 1/200 s say, still counts its whole units).
    pub fn units_of_secs(&mut self, seconds: f32) -> i64 {
        let x = self.sub + f64::from(seconds.max(0.0)) * UNITS as f64;
        let u = x.round();
        self.sub = x - u;
        u.max(0.0) as i64
    }

    /// An interval in nanoseconds (the window's measured frame) as whole
    /// units, the remainder carried exactly.
    pub fn units_of_ns(&mut self, ns: u64) -> i64 {
        let t = u128::from(ns) * 9 + u128::from(self.ns_rem);
        self.ns_rem = (t % 250_000) as u64;
        (t / 250_000) as i64
    }

    /// A drawn frame `units` long starts: its time is added.
    pub fn begin(&mut self, units: i64) -> FrameTicks {
        self.acc += units;
        FrameTicks { units, ..Default::default() }
    }

    /// The frame's next tick at `rate` (the rate the state before this
    /// tick runs at): its dt (0x865194), or None when the frame has no
    /// more. At the cap the time beyond one period (less the snap) is
    /// dropped.
    pub fn tick(&mut self, f: &mut FrameTicks, rate: Rate) -> Option<f32> {
        let due = rate.period - f.snap(rate);
        if self.acc < due {
            return None;
        }
        if f.taken >= rate.cap {
            let keep = due - 1;
            f.dropped += self.acc - keep;
            self.acc = keep;
            return None;
        }
        f.taken += 1;
        self.acc -= rate.period;
        Some(self.clock.frame_limited(rate.raw, rate.limit).0)
    }

    /// How far the frame is drawn from the state before the last tick
    /// towards the newest (0..1) at `rate` (the next tick's): the
    /// accumulator over the period; 0 when it is at most the snap on a
    /// frame the snap applies to (a steady 60 Hz display draws exactly the
    /// tick states) or below zero (a tick taken early).
    pub fn alpha(&self, f: &FrameTicks, rate: Rate) -> f32 {
        if self.acc <= 0 || (f.snap(rate) > 0 && self.acc <= SNAP_UNITS) {
            return 0.0;
        }
        (self.acc as f64 / rate.period as f64).min(1.0) as f32
    }

    /// A whole drawn frame of `units` with the rate picked per tick by
    /// `rate_of(ticks taken so far in this frame)`: the dts and alpha.
    pub fn frame_units(&mut self, units: i64, mut rate_of: impl FnMut(&[f32]) -> Rate) -> (Vec<f32>, f32) {
        let mut f = self.begin(units);
        let mut dts = Vec::new();
        while let Some(dt) = self.tick(&mut f, rate_of(&dts)) {
            dts.push(dt);
        }
        let alpha = self.alpha(&f, rate_of(&dts));
        (dts, alpha)
    }

    /// A drawn frame of `interval` seconds at [`RACE`]: the dts and alpha.
    pub fn frame(&mut self, interval: f32) -> (Vec<f32>, f32) {
        self.frame_at(interval, RACE)
    }

    /// A drawn frame of `interval` seconds, every tick at `rate`.
    pub fn frame_at(&mut self, interval: f32, rate: Rate) -> (Vec<f32>, f32) {
        let units = self.units_of_secs(interval);
        self.frame_units(units, |_| rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's mover dt in ticks (0x86513C) at a fixed 60 / 120 /
    /// 240 frames a game second (the maintainer's private captures): the clock
    /// reaches the same repeating pattern.
    #[test]
    fn steady_rates_give_the_native_tick_patterns() {
        assert_eq!(f32::from_bits(0x400E_38E4), 2.222_222_3);
        for (fps, cycle) in [(60.0f32, [30u32, 29, 31]), (120.0, [15, 16, 14]), (240.0, [8, 7, 7])] {
            let mut c = FrameClock::default();
            let ticks: Vec<u32> = (0..60).map(|_| c.frame((1000.0f32 / fps) * 0.001).1).collect();
            let tail = &ticks[30..];
            let start = tail.iter().position(|&t| t == cycle[0]).unwrap();
            for (i, &t) in tail[start..].iter().enumerate() {
                assert_eq!(t, cycle[i % 3], "{fps} fps: {ticks:?}");
            }
        }
    }

    /// Over many frames the 1/4000 s carry keeps the 4000 Hz clock on real
    /// time, while the world gets whole 1/1800 s ticks: exact at 60 and
    /// 120 fps, 22 ticks per 3 frames (2.2 % short) at 240 fps.
    #[test]
    fn world_time_per_rate() {
        for (fps, ratio) in [(60.0f32, 1.0f32), (120.0, 1.0), (240.0, 22.0 / 22.5)] {
            let mut c = FrameClock::default();
            let n = 3 * fps as usize;
            let world: f32 = (0..n).map(|_| c.frame(1.0 / fps).0).sum();
            assert!((world / 3.0 - ratio).abs() < 0.002, "{fps}: {}", world / 3.0);
        }
    }

    fn q(dt: f32) -> u32 {
        (dt * 1800.0).round() as u32
    }

    /// RACE ticks: 30, 29, 31 from boot, phase continuous; SCENE ticks:
    /// 60, 60, 60, 60, 61 (never 59, which the NIS mover skips).
    #[test]
    fn race_and_scene_tick_patterns() {
        let mut c = TickClock::default();
        let race: Vec<u32> = (0..30).flat_map(|_| c.frame(1.0 / 60.0).0).map(q).collect();
        assert_eq!(race.len(), 30);
        for (i, &t) in race[3..].iter().enumerate() {
            assert_eq!(t, [30, 29, 31][i % 3], "{race:?}");
        }
        let mut c = TickClock::default();
        let scene: Vec<u32> = (0..60).flat_map(|_| c.frame_at(1.0 / 30.0, SCENE).0).map(q).collect();
        let tail = &scene[10..];
        let start = tail.iter().position(|&t| t == 61).unwrap();
        for (i, &t) in tail[start..].iter().enumerate() {
            assert_eq!(t, if i % 5 == 0 { 61 } else { 60 }, "{scene:?}");
        }
        // The old path (each 1/30 s frame stepped by its own measured
        // time, no limiter) gave 59s.
        let mut old = FrameClock::default();
        assert!((0..30).map(|_| old.frame(RAW_30).1).any(|t| t == 59));
    }

    /// Exact display rates over 10 s: every one takes the same 600 RACE
    /// ticks (300 SCENE) with the same dts as 60 Hz, 30 Hz two per frame
    /// (no slow-frame branch), 200 Hz not one short.
    #[test]
    fn exact_tick_counts_at_every_common_rate() {
        let run = |hz: i64, rate: Rate| {
            let mut c = TickClock::default();
            let mut dts = Vec::new();
            for _ in 0..10 * hz {
                dts.extend(c.frame_units(UNITS / hz, |_| rate).0);
            }
            (dts, c.acc)
        };
        for rate in [RACE, SCENE] {
            let (sixty, _) = run(60, rate);
            assert_eq!(sixty.len() as i64, 10 * UNITS / rate.period);
            for hz in [30, 50, 60, 72, 75, 90, 100, 120, 144, 200, 240] {
                assert_eq!(UNITS % hz, 0);
                let (dts, acc) = run(hz, rate);
                assert_eq!(dts, sixty, "{hz} Hz at {rate:?}");
                assert!(acc.abs() <= SNAP_UNITS, "{hz} Hz: {acc} left");
            }
        }
        // The same through seconds: an f32 1/200 s is a hair under 5 ms,
        // still 600 ticks (an f64 accumulator would lose one).
        for hz in [30.0f32, 60.0, 120.0, 144.0, 200.0, 240.0] {
            let mut c = TickClock::default();
            let n: usize = (0..(10.0 * hz) as usize).map(|_| c.frame(1.0 / hz).0.len()).sum();
            assert_eq!(n, 600, "{hz} Hz through f32 seconds");
        }
        // And through nanoseconds (the window).
        let mut c = TickClock::default();
        let n: usize = (0..2000).map(|_| {
            let u = c.units_of_ns(5_000_000);
            c.frame_units(u, |_| RACE).0.len()
        }).sum();
        assert_eq!((n, c.ns_rem), (600, 0));
        let mut c = TickClock::default();
        let total: i64 = (0..3000).map(|_| c.units_of_ns(16_666_667)).sum();
        assert_eq!(total, (3000 * 16_666_667u64 * 9 / 250_000) as i64);
    }

    /// A rate switch keeps the accumulator in real time and the native
    /// carry's phase: the tick after a SCENE stretch continues the 4000 Hz
    /// carry exactly as a clock that never left, and the world's real time
    /// is spent exactly.
    #[test]
    fn phase_continues_across_a_rate_switch() {
        // Ticks 0..20 RACE, 20..35 SCENE, then RACE, at 144 Hz.
        let mut c = TickClock::default();
        let mut seq = Vec::new();
        let mut real = 0i64;
        for _ in 0..144 * 3 {
            real += UNITS / 144;
            let (dts, _) = c.frame_units(UNITS / 144, |d| {
                let n = seq.len() + d.len();
                if (20..35).contains(&n) { SCENE } else { RACE }
            });
            seq.extend(dts);
        }
        let spent: i64 = (0..seq.len()).map(|n| if (20..35).contains(&n) { SCENE.period } else { RACE.period }).sum();
        assert_eq!(real - spent, c.acc);
        assert!(c.acc >= 0 && c.acc < RACE.period);
        // The native clock fed the same raw times in the same order.
        let mut native = FrameClock::default();
        let want: Vec<f32> = (0..seq.len())
            .map(|n| if (20..35).contains(&n) { native.frame_limited(RAW_30, 1.0 / 30.0).0 } else { native.frame(RAW_60).0 })
            .collect();
        assert_eq!(seq, want);
        assert_eq!(c.clock, native);
        assert_eq!(seq[20..35].iter().map(|&d| q(d)).filter(|&t| t < 60).count(), 0);
    }

    /// At most 3 RACE / 2 SCENE ticks per drawn frame; the time beyond is
    /// dropped (below 20 fps the game slows down, as the original's 3/60 s
    /// cap makes it), and the next frame starts under one period.
    #[test]
    fn cap_and_drop() {
        let mut c = TickClock::default();
        let mut f = c.begin(UNITS / 10);
        let mut n = 0;
        while c.tick(&mut f, RACE).is_some() {
            n += 1;
        }
        assert_eq!(n, 3);
        assert_eq!(c.acc, RACE.period - SNAP_UNITS - 1);
        assert_eq!(f.dropped, UNITS / 10 - 3 * RACE.period - c.acc);
        // 10 fps for 10 s: 3 ticks a frame, 300 in all (half speed).
        let mut c = TickClock::default();
        let n: usize = (0..100).map(|_| c.frame_units(UNITS / 10, |_| RACE).0.len()).sum();
        assert_eq!(n, 300);
        let n: Vec<usize> = (0..10).map(|_| c.frame_units(UNITS / 5, |_| SCENE).0.len()).collect();
        assert!(n.iter().all(|&k| k == 2), "{n:?}");
        // 20 fps exactly: 3 ticks a frame, nothing dropped.
        let mut c = TickClock::default();
        for _ in 0..40 {
            let mut f = c.begin(UNITS / 20);
            while c.tick(&mut f, RACE).is_some() {}
            assert_eq!((f.taken, f.dropped), (3, 0));
        }
        // A stall drops time, never ticks: the dts are the next ones of
        // the same sequence.
        let mut a = TickClock::default();
        let mut b = TickClock::default();
        let mut sa = Vec::new();
        let mut sb = Vec::new();
        for k in 0..200 {
            sa.extend(a.frame_units(UNITS / 60, |_| RACE).0);
            sb.extend(b.frame_units(if k == 50 { UNITS * 120 / 1000 } else { UNITS / 60 }, |_| RACE).0);
        }
        let n = sa.len().min(sb.len());
        assert_eq!(sa[..n], sb[..n]);
        assert_eq!(sb.len(), 202);
    }

    /// α: the accumulator over the period; exactly 0 at a steady 60 Hz
    /// (and within the snap of it on a jittery 60 Hz display), rising with
    /// real time at a high rate.
    #[test]
    fn alpha_and_its_snap() {
        let mut c = TickClock::default();
        for _ in 0..600 {
            let (dts, a) = c.frame(1.0 / 60.0);
            assert_eq!((dts.len(), a), (1, 0.0));
        }
        // A jittery 60 Hz: 16.67 ms ± 1 ms; one tick a frame, α 0 while
        // within the snap.
        let mut c = TickClock::default();
        for k in 0..600 {
            let u = RACE.period + [36, -36, 20, -20][k % 4];
            let (dts, a) = c.frame_units(u, |_| RACE);
            assert_eq!(dts.len(), 1, "frame {k}");
            assert_eq!(a, 0.0, "frame {k}: acc {}", c.acc);
        }
        // 240 Hz: 0.25, 0.5, 0.75, then 0 with the tick.
        let mut c = TickClock::default();
        let a: Vec<(usize, f32)> = (0..8).map(|_| { let (d, a) = c.frame_units(UNITS / 240, |_| RACE); (d.len(), a) }).collect();
        assert_eq!(a, [(0, 0.25), (0, 0.5), (0, 0.75), (1, 0.0), (0, 0.25), (0, 0.5), (0, 0.75), (1, 0.0)]);
        // A short frame never snaps: 144 Hz draws 250 / 600 after its first.
        let mut c = TickClock::default();
        assert_eq!(c.frame_units(UNITS / 144, |_| RACE).1, 250.0 / 600.0);
    }

    /// At a high or uncapped rate the drawn time moves on with real time
    /// every frame: no frame drawn standing still and none jumping.
    #[test]
    fn fast_frames_draw_steadily() {
        for hz in [75.0f64, 100.0, 144.0, 165.0, 240.0, 360.0, 1000.0] {
            let mut c = TickClock::default();
            let (mut updates, mut last) = (0usize, None::<f64>);
            for _ in 0..(5.0 * hz) as usize {
                let (dts, alpha) = c.frame((1.0 / hz) as f32);
                updates += dts.len();
                let drawn = (updates as f64 - 1.0) + f64::from(alpha);
                if let Some(prev) = last {
                    let step = (drawn - prev) * UPDATE * hz;
                    assert!((step - 1.0).abs() < 0.01, "{hz} Hz: a frame drew {step:.3} frames of motion");
                }
                last = Some(drawn);
            }
        }
        // Uncapped: 1–4 ms frames (through nanoseconds, as the window).
        let mut c = TickClock::default();
        let (mut updates, mut last) = (0usize, None::<f64>);
        for k in 0..4000u64 {
            let ns = 1_000_000 + ((k.wrapping_mul(2_654_435_761) >> 7) % 3000) * 1000;
            let u = c.units_of_ns(ns);
            let (dts, alpha) = c.frame_units(u, |_| RACE);
            updates += dts.len();
            let drawn = (updates as f64 - 1.0) + f64::from(alpha);
            if let Some(prev) = last {
                let step = (drawn - prev) * UPDATE / (ns as f64 * 1e-9);
                assert!((step - 1.0).abs() < 0.05, "uncapped: a {ns} ns frame drew {step:.3} frames of motion");
            }
            last = Some(drawn);
        }
    }

    /// A scene drawn at the display's rate between its 1/30 s ticks moves
    /// on with real time every frame.
    #[test]
    fn scene_frames_draw_steadily() {
        for hz in [75.0f64, 144.0, 240.0] {
            let mut c = TickClock::default();
            let (mut world, mut last_dt, mut last) = (0.0f64, 0.0f64, None::<f64>);
            for _ in 0..(5.0 * hz) as usize {
                let (dts, alpha) = c.frame_at((1.0 / hz) as f32, SCENE);
                for dt in &dts {
                    world += f64::from(*dt);
                    last_dt = f64::from(*dt);
                }
                let drawn = world - (1.0 - f64::from(alpha)) * last_dt;
                if let Some(prev) = last.filter(|_| world > 0.5) {
                    let step = (drawn - prev) * hz;
                    assert!((step - 1.0).abs() < 0.05, "{hz} Hz: a frame drew {step:.3} frames of motion");
                }
                last = Some(drawn);
            }
        }
    }

    proptest::proptest! {
        /// The tick sequence (rate, dt) is a function of the tick states
        /// only: any slicing of real time into frames (1–150 ms, stalls
        /// included) gives a prefix of the same sequence, the rate picked
        /// per tick from the state (here a function of the tick index).
        #[test]
        fn tick_sequence_ignores_the_slicing(frames in proptest::collection::vec(36i64..5400, 1..400), scene_from in 0usize..200, scene_len in 0usize..120) {
            let rate_at = |n: usize| if (scene_from..scene_from + scene_len).contains(&n) { SCENE } else { RACE };
            let run = |slices: &mut dyn Iterator<Item = i64>, ticks: usize| {
                let mut c = TickClock::default();
                let mut seq: Vec<(i64, u32)> = Vec::new();
                for u in slices {
                    if seq.len() >= ticks {
                        break;
                    }
                    let (dts, _) = c.frame_units(u, |d| rate_at(seq.len() + d.len()));
                    for dt in dts {
                        let r = rate_at(seq.len());
                        seq.push((r.period, dt.to_bits()));
                    }
                }
                seq
            };
            let sliced = run(&mut frames.clone().into_iter(), usize::MAX);
            let reference = run(&mut std::iter::repeat(UNITS / 60), sliced.len());
            proptest::prop_assert_eq!(&sliced[..], &reference[..sliced.len()]);
        }
    }
}
