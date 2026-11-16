//! The harness on the
//! tick cadence. The world advances only by ticks of the tick clock
//! (`nfsu2::frame_clock::TickClock`, docs/uncapped-fps.md): 1/60 s (the
//! native 30, 29, 31 of 1/1800 s) or, while a scene holds the cars, 1/30 s
//! (60, 60, 60, 60, 61). The harness steps the game the same way:
//!
//! - [`Game::advance_ticks`]: n ticks, each at the rate the state before
//!   it picks, its dt the tick clock's next (`TickClock::begin`/`tick`, the
//!   code the window's `advance` runs; the native carry and so the
//!   3-cycle's phase are the window's own), then the world update.
//! - [`Game::advance_for`]: the ticks a span of real time holds (whole
//!   ticks; the part of a tick left over is carried in the tick clock's
//!   accumulator to the next call, as a drawn frame's is): 1/60 s is one
//!   tick in a race, 1/30 s two in a race and one in a scene, 0.5 s 30 in
//!   a race.
//! - [`Game::advance_for_each`]: the same, a closure after each tick
//!   (what the window's step does besides the world update and a test
//!   wants too, e.g. the particles with the tick's dt).
//! - [`Frames`]: drawn frames on a display schedule through the window's
//!   tick path (`advance`'s catch-up loop one tick at a time), so a flow's
//!   inputs and checks happen AT a tick index, between ticks, never "on
//!   the next drawn frame" (a 30 Hz frame holds two ticks: before, an
//!   input or a race-end check landed one tick late there).
//!
//! `check_dt` (dev-harness and test builds, from `Game::update`): every dt
//! the world update gets is one of the tick clock's dts, so nothing steps
//! off the cadence again.
use super::rate;
use crate::*;
use nfsu2::frame_clock::{FrameTicks, RACE, Rate, SCENE, SNAP_UNITS};

impl Game {
    /// The rate the next tick runs at, from the state before it
    /// (`advance`'s rule): a scene that holds the cars [`SCENE`], else
    /// [`RACE`].
    pub(crate) fn tick_rate(&self) -> Rate {
        if self.scene_frame_cap() { SCENE } else { RACE }
    }

    /// `n` ticks of the world update, each at the rate the state picks,
    /// its dt the tick clock's (the accumulator unchanged: each tick is
    /// handed a whole period of its rate).
    pub(crate) fn advance_ticks(&mut self, n: u32) -> Result<()> {
        for _ in 0..n {
            let rate = self.tick_rate();
            let c = &mut self.pacing.cadence;
            let mut f = c.begin(rate.period);
            let dt = c.tick(&mut f, rate).context("a whole period holds a tick")?;
            self.update(dt)?;
        }
        Ok(())
    }

    /// The ticks `secs` of real time holds (see the module doc).
    pub(crate) fn advance_for(&mut self, secs: f32) -> Result<()> {
        self.span(secs, false, &mut |_, _| Ok(())).map(|_| ())
    }

    /// [`Game::advance_for`] with `each(game, dt)` after every tick; the
    /// ticks taken.
    pub(crate) fn advance_for_each(
        &mut self,
        secs: f32,
        mut each: impl FnMut(&mut Game, f32) -> Result<()>,
    ) -> Result<u32> {
        self.span(secs, false, &mut each)
    }

    /// [`Game::advance_for`] with the window's whole step per tick
    /// (`Game::step`: the world update, then the sounds and particles),
    /// for the tests of what the step adds.
    pub(crate) fn step_for(&mut self, secs: f32) -> Result<()> {
        self.span(secs, true, &mut |_, _| Ok(())).map(|_| ())
    }

    /// The ticks of `secs`: the world update (`full`: `Game::step`), then
    /// `each`.
    fn span(&mut self, secs: f32, full: bool, each: &mut dyn FnMut(&mut Game, f32) -> Result<()>) -> Result<u32> {
        let units = self.pacing.cadence.units_of_secs(secs);
        let mut f = self.pacing.cadence.begin(units);
        let mut n = 0;
        loop {
            // No per-frame cap: a harness span is all ticked.
            let rate = Rate { cap: u32::MAX, ..self.tick_rate() };
            let Some(dt) = self.pacing.cadence.tick(&mut f, rate) else { break };
            if full {
                self.step(dt)?;
            } else {
                self.update(dt)?;
            }
            each(self, dt)?;
            n += 1;
        }
        Ok(n)
    }
}

impl Game {
    /// The index of the next tick (`GameClock::tick`: the update an input
    /// queued for it goes before).
    pub(crate) fn next_tick(&self) -> u64 {
        self.pacing.clocks.tick
    }

    /// A flow's key tap through the input queue (`Game::input_at`,
    /// the window's path): pressed at the start of the next tick (the key's
    /// action and held), let go at the start of the one after.
    pub(crate) fn tap_next(&mut self, code: KeyCode) {
        let t = self.next_tick();
        self.input_at(t, InputEvent::press(code));
        self.input_at(t + 1, InputEvent::KeyUp(code));
    }

    /// A flow's held key through the input queue: pressed (`on`) or let go
    /// at the start of the next tick; nothing queued when it already is
    /// (the window sends a press or a release only on a change).
    pub(crate) fn hold_next(&mut self, code: KeyCode, on: bool) {
        if self.platform.keys.contains(&code) == on {
            return;
        }
        let t = self.next_tick();
        self.input_at(t, if on { InputEvent::press(code) } else { InputEvent::KeyUp(code) });
    }
}

/// The tick clock's dts in 1/1800 s ticks: [`RACE`] 29, 30, 31; [`SCENE`]
/// 60, 61 (`tick_dts_are_the_clock_s` checks the set under any switching).
const TICK_QS: [u32; 5] = [29, 30, 31, 60, 61];

/// Whether `dt` is one of the tick clock's dts (bit for bit as
/// `FrameClock` makes them).
pub(crate) fn is_tick_dt(dt: f32) -> bool {
    TICK_QS.iter().any(|&q| (q as f32 * (1.0f32 / 1800.0)).to_bits() == dt.to_bits())
}

/// `Game::update`'s check (dev-harness and test builds): its dt is a
/// tick's. The front end without a world ticks at RACE too
/// (`advance`: one tick clock from boot through menu → load → world), so
/// the only exception left is [`off_cadence`].
pub(crate) fn check_dt(_game: &Game, dt: f32) {
    if is_tick_dt(dt) || OFF_CADENCE.with(|c| c.get()) {
        return;
    }
    panic!(
        "an update off the tick cadence: dt {dt} ({} of 1/1800 s); step with advance_ticks/advance_for",
        dt * 1800.0
    );
}

thread_local! {
    /// Inside [`off_cadence`].
    static OFF_CADENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `f` with [`check_dt`] off: only for a replay of the original's own
/// measured frame times (the parity flow's capture, an INTRO_FRAMES
/// replay's log), which are not the clone's ticks by design.
pub(crate) fn off_cadence<T>(f: impl FnOnce() -> T) -> T {
    let was = OFF_CADENCE.with(|c| c.replace(true));
    let r = f();
    OFF_CADENCE.with(|c| c.set(was));
    r
}

/// A drawn frame [`Frames::tick`] finished: how far it was drawn between
/// the last two ticks and its interval (s).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Drawn {
    pub(crate) alpha: f32,
    pub(crate) interval: f32,
}

/// A drawn frame being ticked.
struct Open {
    f: FrameTicks,
    interval: f32,
    updates: usize,
    world: f32,
}

/// Drawn frames on a display schedule (a rate run's `NFSU2_RATE`, else
/// frames of 1/`hz` s; `hz` 0: uncapped, `rate::interval`), the world
/// ticked through the window's tick path one tick per [`Frames::tick`]:
/// a flow acts between ticks. A frame's draw-side state updates
/// (`rate::drawn`) run when it ends, as the window's do. At a steady
/// 60 Hz each call is one whole frame, in the old flows' order.
pub(crate) struct Frames {
    hz: f32,
    frame: u64,
    open: Option<Open>,
}

impl Frames {
    pub(crate) fn new(hz: f32) -> Frames {
        Frames { hz, frame: 0, open: None }
    }

    /// Drawn frames begun so far.
    pub(crate) fn frames(&self) -> u64 {
        self.frame
    }

    /// Exactly one tick (frames without a tick drawn through first), then
    /// its frame finished when it has no more ticks due. The frames
    /// finished meanwhile, in order.
    pub(crate) fn tick(&mut self, game: &mut Game) -> Result<Vec<Drawn>> {
        let mut drawn = Vec::new();
        self.tick_with(game, &mut |_, d| {
            drawn.push(d);
            Ok(())
        })?;
        Ok(drawn)
    }

    /// [`Frames::tick`] with `on_frame(game, frame)` as each drawn frame
    /// ends (after its draw-side updates, the state as it drew it).
    pub(crate) fn tick_with(
        &mut self,
        game: &mut Game,
        on_frame: &mut dyn FnMut(&mut Game, Drawn) -> Result<()>,
    ) -> Result<()> {
        for _ in 0..100_000 {
            if self.open.is_none() {
                self.begin(game);
            }
            let o = self.open.as_mut().expect("a frame begun");
            let rate = game.tick_rate();
            if let Some(dt) = game.pacing.cadence.tick(&mut o.f, rate) {
                o.updates += 1;
                o.world += dt;
                game.harness_tick(dt)?;
                // Its frame finished unless another tick is due in it
                // (frames without a tick come at the next call, before its
                // tick: a flow that stops leaves no frame begun behind).
                if self.open.as_ref().is_some_and(|o| !Self::due(game, &o.f)) {
                    self.finish(game, on_frame)?;
                } else {
                    // Another tick follows in this frame: the
                    // streak probe of this one now, before the flow acts
                    // (where a 60 Hz frame is drawn), as `advance` makes it.
                    game.probe_between_ticks()?;
                }
                return Ok(());
            }
            self.finish(game, on_frame)?;
        }
        anyhow::bail!("100000 drawn frames without a tick")
    }

    /// The next drawn frame begun. With or without a world (the front end
    /// ticks at RACE too, `advance`'s one path).
    fn begin(&mut self, game: &mut Game) {
        game.streak_harness_form();
        let interval = rate::interval(game, self.hz, self.frame);
        self.frame += 1;
        let units = game.pacing.cadence.units_of_secs(interval);
        let f = game.pacing.cadence.begin(units);
        self.open = Some(Open { f, interval, updates: 0, world: 0.0 });
    }

    /// Whether the open frame takes another tick (`TickClock::tick`'s
    /// test at the state's rate, the cap included).
    fn due(game: &Game, f: &FrameTicks) -> bool {
        let rate = game.tick_rate();
        let snap = if f.units >= rate.period - SNAP_UNITS { SNAP_UNITS } else { 0 };
        game.pacing.cadence.acc >= rate.period - snap && f.taken < rate.cap
    }

    /// The open frame ended as `advance` ends it: its last test of the
    /// clock (the time beyond the cap dropped), the counters, alpha, the
    /// draw-side updates.
    fn finish(&mut self, game: &mut Game, on_frame: &mut dyn FnMut(&mut Game, Drawn) -> Result<()>) -> Result<()> {
        let Some(mut o) = self.open.take() else { return Ok(()) };
        loop {
            let rate = game.tick_rate();
            let Some(dt) = game.pacing.cadence.tick(&mut o.f, rate) else { break };
            if o.updates > 0 {
                game.probe_between_ticks()?;
            }
            o.updates += 1;
            o.world += dt;
            game.harness_tick(dt)?;
        }
        game.pacing.frame_updates = o.updates;
        game.pacing.frame_world = o.world;
        let alpha = game.smooth_slow_alpha(game.pacing.cadence.alpha(&o.f, game.tick_rate()));
        rate::drawn(game);
        on_frame(game, Drawn { alpha, interval: o.interval })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfsu2::frame_clock::TickClock;

    /// Every dt the tick clock hands out, whatever the rates and frames
    /// (switches between RACE and SCENE at any tick, frames of any
    /// length), is in `TICK_QS`; and those are bit for bit
    /// `FrameClock`'s.
    #[test]
    fn tick_dts_are_the_clock_s() {
        let mut c = TickClock::default();
        let mut seen = std::collections::BTreeSet::new();
        let mut x = 0x9E37_79B9u64;
        for _ in 0..200_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let rate = if x % 7 < 2 { SCENE } else { RACE };
            let units = (x >> 8) % 3000;
            let mut f = c.begin(units as i64);
            while let Some(dt) = c.tick(&mut f, rate) {
                assert!(is_tick_dt(dt), "dt {dt} ({} ticks)", dt * 1800.0);
                seen.insert((dt * 1800.0).round() as u32);
            }
        }
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), TICK_QS.to_vec());
        assert!(!is_tick_dt(1.0 / 120.0) && !is_tick_dt(0.5) && !is_tick_dt(1.0 / 20.0));
        // A constant 1/60 s or 1/30 s step is a tick's dt bit for bit (the
        // default bench's fixed step passes the check).
        assert!(is_tick_dt(1.0 / 60.0) && is_tick_dt(1.0 / 30.0));
    }
}
