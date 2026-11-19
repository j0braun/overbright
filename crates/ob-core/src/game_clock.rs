//! The game's clocks on ticks (docs/uncapped-fps.md "Game time on ticks";
//! the clock steps below were measured in the original under Wine):
//! every clock the game state reads advances per tick
//! ([`crate::frame_clock::TickClock`]) from that tick's quantised dt
//! (0x865194), never by the wall clock or per drawn frame. So the front
//! end, the HUD packages and the native counters are the same at 30, 60,
//! 120 and 200 Hz, and at a steady 60 Hz they are the original's.
//!
//! **Traced** (SPEED2.EXE, main loop 0x5811D0):
//! - Package clock (cFEng, one for the menus and the in-race HUD packages):
//!   0x5663C0 → 0x5662D0 hands 0x558C10 `ms = _ftol(f32 0x865194 × 1200.0)`
//!   (0x5662F9..0x566306, truncation) once per iteration: 20, 19, 20 per
//!   1/60 s tick (30, 29, 31 ticks of 1/1800 s), 40 per 1/30 s scene tick
//!   (60/61). No cap: out of a running world (`inGame` = 0x8654A4 == 6 and
//!   0x865150 > 0 false: the front end, the pause) 0x558C10 applies the ms
//!   in pieces of at most 20 (0x558DAB); in a running world in one step.
//! - Real-time clock 0x8651A8 (0x57A060): += round(0x865194 × 4000) every
//!   iteration (67, 64, 69 per RACE tick), never reset, runs in the pause.
//! - Sim clock 0x865188 and world counter 0x7FB724 (0x57A0F0, the
//!   iteration's last clock step): when the world dt 0x865150 ≠ 0,
//!   0x865188 += round(0x865150 × 4000) and 0x7FB724 += 1. The front end
//!   sets 0x865150 = 0x865194 (0x581469), so both advance in the menus; the
//!   pause sets it 0 (0x601A70(0)), so both freeze there. World create
//!   0x5FF54D and teardown 0x5FF7CD call 0x576D00: 0x865188 = 4000,
//!   0x7FB724 = 0, 0x865150 = 0 (that iteration's step does nothing);
//!   teardown then sets 0x7FB724 = 100 000 (0x5FF7D2, also its static
//!   value).
//!
//! **Inferred**: the boot value of 0x865188 (not read; 4000, 0x576D00's,
//! is taken).

/// The FEng clock's rate against the front end's milliseconds: package
/// ms = 1.2 × screen ms (`frontend::feng::script::FE_RATE`, where the
/// measurements are).
pub const FE_RATE: f32 = 1.2;

/// 0x865188 after 0x576D00 (world create and teardown).
pub const SIM_CLOCK_RESET: u32 = 4000;
/// 0x7FB724 after a world teardown (0x5FF7D2) and at boot (its static
/// value).
pub const WORLD_COUNTER_IDLE: u32 = 100_000;
/// The largest piece 0x558C10 applies at once out of a running world.
pub const PACKAGE_PIECE_MS: u32 = 20;

/// The game's clocks (owned by the game, advanced once per tick by
/// [`GameClock::tick`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameClock {
    /// Ticks since boot (the world updates; never reset).
    pub tick: u64,
    /// The package clock (ms): the sum of every tick's
    /// [`package_ms`]; what every FEng package's scripts run on.
    pub package_ms: u64,
    /// The last tick's package step (ms) and whether it was applied in
    /// one piece (a running world) or in [`pieces`].
    pub last_step: (u32, bool),
    /// 0x8651A8: real time in 1/4000 s.
    pub real: u64,
    /// 0x8651A8 as the tick in progress (or the last one, and the draw
    /// after it) reads it: `real` at the tick's start. Everything of an
    /// iteration of 0x5811D0 (steps 12–24: the world, the front end's
    /// tick, the HUD, the audio, the render) sees it before that
    /// iteration's own step (0x57A060, step 25), so the C++ menu timers
    /// stamp and test this value (docs/uncapped-fps.md "Menu timers
    /// on the real-time clock").
    pub real_tick: u64,
    /// 0x865194: the tick in progress's quantised dt (set by the game at
    /// the tick's start; the help bar's countdown steps by it).
    pub dt: f32,
    /// 0x865188: the sim clock in 1/4000 s.
    pub sim_clock: u32,
    /// 0x7FB724: the world update counter (the zone lookup's every-8th
    /// phase).
    pub world_counter: u32,
    /// Inside a tick (between [`GameClock::begin`] and [`GameClock::tick`]).
    in_tick: bool,
    /// 0x576D00 ran inside this tick: 0x865150 = 0, so its sim clock and
    /// world counter step is skipped.
    reset_in_tick: bool,
}

impl Default for GameClock {
    fn default() -> GameClock {
        GameClock {
            tick: 0,
            package_ms: 0,
            last_step: (0, false),
            real: 0,
            real_tick: 0,
            dt: 0.0,
            sim_clock: SIM_CLOCK_RESET,
            world_counter: WORLD_COUNTER_IDLE,
            in_tick: false,
            reset_in_tick: false,
        }
    }
}

/// A tick's package step (0x5662D0): `_ftol(f32 dt × 1200.0)`, the x87
/// product of two floats (exact in extended precision), truncated.
pub fn package_ms(dt: f32) -> u32 {
    (f64::from(dt) * f64::from(1200.0f32)).max(0.0) as u32
}

/// 0x57A060 / 0x57A0F0's step of a 4000 Hz clock: round(dt × 4000) (the
/// x87's round to nearest even).
pub fn clock_4000(dt: f32) -> u32 {
    (f64::from(dt) * 4000.0).max(0.0).round_ties_even() as u32
}

/// x87 `fild d; fmul f32 0.00025` (0x784268): `d` ticks of the real-time
/// clock 0x8651A8 in seconds, as the C++ timers compare them (exact in an
/// f64 below 2^29 ticks, 37 hours: a 24-bit float times an integer).
pub fn real_secs(d: u64) -> f64 {
    d as f64 * f64::from(0.00025f32)
}

/// A C++ timer stamped with 0x8651A8 at `since` has run more than `secs`
/// at `now` (`(now − since) × 0.00025 > secs`, the x87 compare against
/// the f32 constant: `fcomp; test ah, 0x41; jne`).
pub fn real_over(now: u64, since: u64, secs: f32) -> bool {
    real_secs(now.saturating_sub(since)) > f64::from(secs)
}

/// The same timer at `secs` or more (`test ah, 1; jne`: not below).
pub fn real_at_least(now: u64, since: u64, secs: f32) -> bool {
    real_secs(now.saturating_sub(since)) >= f64::from(secs)
}

/// x87 `fld f32 dt; fmul f32 4000.0; _ftol` (0x577320, 0x784264): a
/// countdown's step in 1/4000 s for the tick's quantised dt, truncated
/// (66, 64, 68 for the RACE ticks' 30, 29, 31 / 1800 s; the real-time
/// clock rounds the same dt to 67, 64, 69).
pub fn countdown_step(dt: f32) -> i32 {
    (f64::from(dt) * f64::from(4000.0f32)).max(0.0) as i32
}

/// How 0x558C10 applies `ms`: in one step in a running world (`in_game`),
/// else in pieces of at most 20 ms (`ms -= step` until 0).
pub fn pieces(ms: u32, in_game: bool) -> Vec<u32> {
    if in_game || ms <= PACKAGE_PIECE_MS {
        return if ms == 0 { Vec::new() } else { vec![ms] };
    }
    let mut v = Vec::new();
    let mut left = ms;
    while left > 0 {
        let step = left.min(PACKAGE_PIECE_MS);
        v.push(step);
        left -= step;
    }
    v
}

impl GameClock {
    /// A tick starts (the resets inside it skip its counter step).
    pub fn begin(&mut self) {
        self.in_tick = true;
        self.reset_in_tick = false;
        self.real_tick = self.real;
    }

    /// The end of a tick of quantised dt `dt` (0x865194): the package
    /// clock and the real-time clock step; the sim clock and the world
    /// counter step when the world ran this tick (`world_ran`: the front
    /// end, a race, free roam or a scene; not the pause, 0x865150 = 0) and
    /// no world was created or torn down in it. `world_ran` with a world
    /// loaded is also the package's `inGame` (one step, not pieces).
    pub fn tick(&mut self, dt: f32, world_ran: bool, world_loaded: bool) {
        let step = package_ms(dt);
        self.package_ms += u64::from(step);
        self.last_step = (step, world_ran && world_loaded);
        self.real += u64::from(clock_4000(dt));
        if world_ran && !self.reset_in_tick {
            self.sim_clock = self.sim_clock.wrapping_add(clock_4000(dt));
            self.world_counter = self.world_counter.wrapping_add(1);
        }
        self.tick += 1;
        self.in_tick = false;
        self.reset_in_tick = false;
    }

    /// World create (0x5FF54D → 0x576D00).
    pub fn world_create(&mut self) {
        self.sim_clock = SIM_CLOCK_RESET;
        self.world_counter = 0;
        self.reset_in_tick |= self.in_tick;
    }

    /// World teardown (0x5FF7CD → 0x576D00, then 0x5FF7D2).
    pub fn world_teardown(&mut self) {
        self.sim_clock = SIM_CLOCK_RESET;
        self.world_counter = WORLD_COUNTER_IDLE;
        self.reset_in_tick |= self.in_tick;
    }

    /// The front end's milliseconds (`Game::now`): the package clock over
    /// [`FE_RATE`], so a screen's script time `FE_RATE × (now − start)`
    /// is package ms (the screens keep their times in these units).
    pub fn fe_ms(&self) -> f32 {
        (self.package_ms as f64 / f64::from(FE_RATE)) as f32
    }

    /// The last tick's package step as 0x558C10 applied it.
    pub fn last_pieces(&self) -> Vec<u32> {
        pieces(self.last_step.0, self.last_step.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_clock::{RACE, SCENE, TickClock, UNITS};

    fn ticks_at(hz: i64, seconds: i64, rate: crate::frame_clock::Rate) -> Vec<f32> {
        let mut c = TickClock::default();
        (0..seconds * hz).flat_map(|_| c.frame_units(UNITS / hz, |_| rate).0).collect()
    }

    /// Measured: the package clock at 60 Hz steps 20, 19, 20 (30, 29, 31 ticks
    /// of 1/1800 s): 59 ms per 3 ticks, 1180 per second, not 1200.
    #[test]
    fn package_clock_at_60_hz() {
        let dts = ticks_at(60, 10, RACE);
        let mut g = GameClock::default();
        let mut steps = Vec::new();
        for &dt in &dts {
            g.begin();
            g.tick(dt, true, false);
            steps.push(g.last_step.0);
        }
        // The frame clock's first ticks settle its carry; then the cycle.
        let q: Vec<u32> = dts.iter().map(|d| (d * 1800.0).round() as u32).collect();
        for (k, (&s, &t)) in steps.iter().zip(&q).enumerate().skip(3) {
            assert_eq!(s, match t { 30 => 20, 29 => 19, 31 => 20, _ => panic!("{t}") }, "tick {k}");
        }
        assert_eq!(&steps[3..9], &[20, 19, 20, 20, 19, 20][..]);
        let per_s: u64 = steps[60..120].iter().map(|&s| u64::from(s)).sum();
        assert_eq!(per_s, 1180);
        // The real-time clock: 67, 64, 69 (200 per 3 ticks = 1/20 s).
        let mut g = GameClock::default();
        for &dt in &dts[3..6] {
            g.begin();
            g.tick(dt, true, false);
        }
        assert_eq!(g.real, 200);
    }

    /// Measured: a 1/30 s scene tick (60 or 61 ticks of 1/1800 s) steps 40, in
    /// one step in a running world, in two pieces of 20 out of one.
    #[test]
    fn package_clock_in_a_scene_and_its_pieces() {
        for &dt in &ticks_at(30, 4, SCENE)[5..] {
            assert_eq!(package_ms(dt), 40);
        }
        assert_eq!(pieces(40, true), [40]);
        assert_eq!(pieces(40, false), [20, 20]);
        assert_eq!(pieces(60, false), [20, 20, 20]);
        assert_eq!(pieces(22, false), [20, 2]);
        assert_eq!(pieces(19, false), [19]);
        assert_eq!(pieces(0, false), Vec::<u32>::new());
    }

    /// Measured: in the pause (the world not run, a world loaded) the package
    /// and real-time clocks step, in pieces of at most 20 ms; the sim clock
    /// and world counter do not.
    #[test]
    fn pause_freezes_the_world_clocks() {
        let mut g = GameClock::default();
        g.world_create();
        let dts = ticks_at(60, 1, RACE);
        for &dt in &dts[..10] {
            g.begin();
            g.tick(dt, true, true);
        }
        assert_eq!(g.world_counter, 10);
        let (sim, counter, pkg, real) = (g.sim_clock, g.world_counter, g.package_ms, g.real);
        for &dt in &dts[10..40] {
            g.begin();
            g.tick(dt, false, true);
            assert!(g.last_pieces().iter().all(|&p| p <= PACKAGE_PIECE_MS));
            assert_eq!(g.last_pieces().iter().sum::<u32>(), package_ms(dt));
        }
        assert_eq!((g.sim_clock, g.world_counter), (sim, counter));
        assert!(g.package_ms > pkg + 500 && g.real > real + 1900);
    }

    /// Measured: world create resets 0x865188 / 0x7FB724 to 4000 / 0 and
    /// teardown to 4000 / 100 000; the tick holding the reset does not
    /// step them; the front end steps them (+1 per tick, 67/64/69).
    #[test]
    fn world_counter_resets() {
        let dts = ticks_at(60, 1, RACE);
        let mut g = GameClock::default();
        assert_eq!((g.sim_clock, g.world_counter), (4000, 100_000));
        for &dt in &dts[..5] {
            g.begin();
            g.tick(dt, true, false);
        }
        assert_eq!(g.world_counter, 100_005);
        // The load's tick creates the world.
        g.begin();
        g.world_create();
        g.tick(dts[5], true, true);
        assert_eq!((g.sim_clock, g.world_counter), (4000, 0));
        g.begin();
        g.tick(dts[6], true, true);
        assert_eq!((g.sim_clock, g.world_counter), (4000 + clock_4000(dts[6]), 1));
        // A reset between ticks: the next tick steps.
        g.world_teardown();
        g.begin();
        g.tick(dts[7], true, false);
        assert_eq!(g.world_counter, 100_001);
        // In a tick: held.
        g.begin();
        g.world_teardown();
        g.tick(dts[8], true, false);
        assert_eq!((g.sim_clock, g.world_counter), (4000, 100_000));
        assert_eq!(g.tick, 9);
    }

    /// The package clock across menu → world → menu at any display rate:
    /// one carry (the tick clock's), so the sequence of steps is the same
    /// as a clock that never left the menus, at 30 / 60 / 120 / 200 Hz.
    #[test]
    fn package_clock_continuous_and_rate_free() {
        let run = |hz: i64| {
            let mut c = TickClock::default();
            let mut g = GameClock::default();
            let mut seq = Vec::new();
            for f in 0..(12 * hz) {
                let n = seq.len();
                // menu, a load creates the world at tick 200, a scene from
                // 260 to 320, the world to 500, then the menu.
                let rate_of = |k: usize| if (260..320).contains(&k) { SCENE } else { RACE };
                let (dts, _) = c.frame_units(UNITS / hz, |d| rate_of(n + d.len()));
                for dt in dts {
                    let k = seq.len();
                    g.begin();
                    if k == 200 {
                        g.world_create();
                    }
                    if k == 500 {
                        g.world_teardown();
                    }
                    g.tick(dt, true, (200..500).contains(&k));
                    seq.push((g.package_ms, g.real, g.sim_clock, g.world_counter));
                }
                let _ = f;
            }
            seq
        };
        let sixty = run(60);
        assert!(sixty.len() > 600);
        for hz in [30, 120, 200] {
            let s = run(hz);
            let n = s.len().min(sixty.len());
            assert!(n > 600);
            assert_eq!(s[..n], sixty[..n], "{hz} Hz");
        }
        // Continuous: the steps around the transitions are the RACE cycle.
        let steps: Vec<u64> = sixty.windows(2).map(|w| w[1].0 - w[0].0).collect();
        assert!(steps[190..250].iter().all(|&s| s == 19 || s == 20));
        assert!(steps[262..318].iter().all(|&s| s == 40));
        assert_eq!(sixty[200].3, 0);
        assert_eq!(sixty[500].3, 100_000);
        assert_eq!(sixty[510].3, 100_010);
    }

    /// The C++ timers' arithmetic on 0x8651A8: the RACE ticks'
    /// countdown steps truncate (66, 64, 68), the real-time clock rounds
    /// (67, 64, 69); the compares against the f32 constants with 0.00025
    /// as the f32 0.000250000012: the cursor's 2 s is over at 8000 ticks
    /// (2.0000001 > 2), not at 7999; the lots' 0.3 s (0x78758C =
    /// 0.300000012) holds at 1200 (`>=`), not at 1199; the chyron's 7.2 s
    /// (7.19999981) at 28 800.
    #[test]
    fn real_clock_timer_arithmetic() {
        let dts = ticks_at(60, 1, RACE);
        let q: Vec<(u32, i32, u32)> =
            dts[3..6].iter().map(|&d| ((d * 1800.0).round() as u32, countdown_step(d), clock_4000(d))).collect();
        for (t, step, real) in q {
            let want = match t {
                30 => (66, 67),
                29 => (64, 64),
                31 => (68, 69),
                _ => panic!("{t}"),
            };
            assert_eq!((step, real), want, "{t} ticks");
        }
        assert!(real_at_least(1_200, 0, 0.3) && !real_at_least(1_199, 0, 0.3));
        assert!(real_over(8_000, 0, 2.0) && !real_over(7_999, 0, 2.0));
        assert!(real_over(28_800, 0, 7.2) && !real_over(28_799, 0, 7.2));
        assert!(real_over(120_000, 0, 30.0) && !real_over(119_999, 0, 30.0));
        // The tick's `real_tick` is `real` before the tick's own step.
        let mut g = GameClock::default();
        g.begin();
        g.tick(dts[3], true, false);
        g.begin();
        assert_eq!(g.real_tick, g.real);
        let r = g.real;
        g.tick(dts[4], true, false);
        assert_eq!(g.real_tick, r);
        assert!(g.real > r);
    }

    /// `fe_ms` × 1.2 gives the package ms back (a screen's script time).
    #[test]
    fn fe_ms_is_package_time() {
        let mut g = GameClock::default();
        for ms in [0u64, 19, 20, 59, 1180, 70_800, 3_600_000] {
            g.package_ms = ms;
            assert!((g.fe_ms() * FE_RATE - ms as f32).abs() <= ms as f32 * 1e-6, "{ms}");
        }
    }
}
