//! Draw clocks (docs/uncapped-fps.md, "Draw clocks"): every time value a shader
//! or a draw-side animation reads comes from the ticks, blended by α: the
//! shaders' clock (`seconds`: the neon's pulse, the car lights, the world's
//! and the cars' animated textures, the garage), the clone's motion blur's
//! drawn time (`blur`) and the world clock as drawn (`world`: the
//! water's surface, the wet roads' ripples, the Next-Gen atmosphere).
//!
//! - [`TickClocks`]: what a tick leaves for the draw (in the tick snapshot,
//!   captured with it: "use, then step").
//! - [`DrawClocks`]: one drawn frame's clocks, `time_at(S_{n−1}, S_n, α)`
//!   for each ([`Game::draw_clocks`]), carried to the draw in
//!   [`FramePoses::clocks`](super::snapshot::FramePoses) and from there in
//!   `FrameInputs::clocks` only: the draw has no other clock (it reads no
//!   wall clock, no `Pacing` clock and no frame counter as time).
//!
//! Exactly S_{n−1}'s at α = 0 and S_n's at α ≥ 1 (bit for bit), monotonic
//! in α and so across a tick boundary, and a function of the two ticks and
//! α only, so the same at every display rate at equal (tick, α). At 60 Hz
//! in the harness (α = 1) every clock is the tick's, as before.
//!
//! Not blended (states, not times; each read from the tick state as it
//! stands): the HUD's and the menus' layout time (`Game::now`: the HUD's
//! needles and blinks stay stepped at 60 Hz), the garage request's
//! time, the flare pools' blink (stepped on the world clock's tick
//! differences), the screen flash (per tick), the time of day
//! and the weather (world state), and the steppers that key on the tick's
//! world clock: the Next-Gen water's simulation (`water::View::clock`, its
//! fixed 1/60 s steps) and the wet car's spray (`wet_car::publish_weather`:
//! it steps once per change of the clock; a blended clock would step it per
//! drawn frame).
use crate::*;

/// The draw's clocks as one tick leaves them (s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TickClocks {
    /// The shaders' clock: [`Game::seconds`] (the package clock over 1000,
    /// or the harness's pinned clock), as f64 (exact).
    pub(crate) seconds: f64,
    /// The world time stepped (`Pacing::world_time`).
    pub(crate) world_time: f64,
    /// The drive's world clock (0x7FB718, `Drive::clock`); None without a
    /// drive.
    pub(crate) world: Option<f64>,
}

/// One drawn frame's clocks: the only time the draw reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DrawClocks {
    /// The shaders' clock (s): the neon's pulse (0x6227DE), the car lights,
    /// the world's and the cars' animated textures, the garage's scene.
    pub(crate) seconds: f32,
    /// The world's time at the drawn poses (s; the motion blur's
    /// shutter and FSR 2's blur).
    pub(crate) blur: f64,
    /// The world clock as drawn (s): the water's surface, the wet roads'
    /// ripples, the Next-Gen atmosphere. None without a drive.
    pub(crate) world: Option<f32>,
}

impl DrawClocks {
    /// A still's clocks (a shot: the shaders' clock at `seconds`, the
    /// blur's time `blur`, the world clock as the drive stands).
    pub(crate) fn still(seconds: f32, blur: f64, world: Option<f32>) -> DrawClocks {
        DrawClocks { seconds, blur, world }
    }

    /// The water's lag behind the tick's world clock `tick` (s): what
    /// `water::View::lag` takes, so that `tick − lag` is exactly the drawn
    /// clock (the clock is ≥ 1 s from a world's start and a tick is under
    /// 1/29 s, so both subtractions are exact: Sterbenz).
    pub(crate) fn water_lag(&self, tick: f32) -> f32 {
        self.world.map_or(0.0, |w| tick - w)
    }
}

/// `alpha` of the way from the tick clock `prev` to `cur`: exactly `prev`
/// at α ≤ 0 and `cur` at α ≥ 1, monotonic in α, never past `cur`. A clock
/// that ran back (a reset in the tick) or a missing `prev` is a cut: `cur`.
pub(crate) fn time_at(prev: Option<f64>, cur: f64, alpha: f32) -> f64 {
    let Some(prev) = prev.filter(|p| *p <= cur) else { return cur };
    if alpha >= 1.0 {
        cur
    } else if alpha <= 0.0 {
        prev
    } else {
        (prev + (cur - prev) * f64::from(alpha)).min(cur)
    }
}

impl Game {
    /// The draw's clocks as the game stands (captured at each tick's end).
    pub(crate) fn tick_clocks(&self) -> TickClocks {
        TickClocks {
            seconds: f64::from(self.seconds()),
            world_time: self.pacing.world_time,
            world: self.sim.drive.as_ref().map(|d| f64::from(d.clock)),
        }
    }

    /// The frame's clocks `alpha` of the way from S_{n−1} to S_n (a pure
    /// read). Per clock: the tick's value where something set it after the
    /// tick (a load's new world clock, a harness's clock: the live value,
    /// as `frame_poses`' late writes), the harness's pinned clock as it is
    /// (it is the frame's own time already), no S_{n−1} yet (the first
    /// tick) → S_n; under smooth slow motion the tick clock's α, not the
    /// replay's (the clocks run at the ticks' pace).
    pub(crate) fn draw_clocks(&self, alpha: f32) -> DrawClocks {
        let alpha = self.pacing.slow_tick_alpha.unwrap_or(alpha);
        let live = self.tick_clocks();
        let (prev, cur) = (&self.interp.prev, &self.interp.cur);
        let first = prev.tick == 0;
        let at = |p: f64, c: f64, l: f64| -> f64 {
            if first || c.to_bits() != l.to_bits() { l } else { time_at(Some(p), c, alpha) }
        };
        let seconds = if self.pacing.clock.is_some() {
            live.seconds
        } else {
            at(prev.clocks.seconds, cur.clocks.seconds, live.seconds)
        };
        let world = live.world.map(|l| match (prev.clocks.world, cur.clocks.world) {
            (Some(p), Some(c)) => at(p, c, l),
            _ => l,
        });
        DrawClocks {
            seconds: seconds as f32,
            blur: at(prev.clocks.world_time, cur.clocks.world_time, live.world_time),
            world: world.map(|w| w as f32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ends bit for bit, monotonic, never past S_n; a clock that ran
    /// back or has no S_{n−1} is S_n's.
    #[test]
    fn time_at_ends_are_exact_and_it_is_monotonic() {
        // The package clock's seconds over a few ticks (20, 19, 20 ms) and
        // a world clock far from 0 (f32 values, as captured).
        for (p, c) in [(0.02f32, 0.039f32), (1234.567, 1234.586), (1.0, 1.016_666_7), (0.0, 0.016_666_7)] {
            let (p, c) = (f64::from(p), f64::from(c));
            assert_eq!(time_at(Some(p), c, 0.0).to_bits(), p.to_bits());
            assert_eq!(time_at(Some(p), c, 1.0).to_bits(), c.to_bits());
            assert_eq!(time_at(Some(p), c, 1.5).to_bits(), c.to_bits());
            assert_eq!(time_at(Some(p), c, -0.1).to_bits(), p.to_bits());
            // The f32 the shaders get: the same ends.
            assert_eq!((time_at(Some(p), c, 0.0) as f32).to_bits(), (p as f32).to_bits());
            assert_eq!((time_at(Some(p), c, 1.0) as f32).to_bits(), (c as f32).to_bits());
            let mut last = p;
            for k in 0..=1000 {
                let t = time_at(Some(p), c, k as f32 / 1000.0);
                assert!(t >= last && t <= c, "{p}..{c} at {k}: {t} after {last}");
                last = t;
            }
            // Across the tick boundary: the next pair starts where this ended.
            let next = time_at(Some(c), c + 0.02, 0.0);
            assert!(next >= time_at(Some(p), c, 0.999_999));
        }
        assert_eq!(time_at(Some(5.0), 1.0, 0.5), 1.0, "a clock that ran back cuts");
        assert_eq!(time_at(None, 1.0, 0.5), 1.0, "no S_{{n−1}}: S_n");
    }

    /// The water's drawn clock `tick − lag` is exactly the blended clock.
    #[test]
    fn water_lag_gives_the_drawn_clock_exactly() {
        for (p, c) in [(1.0f32, 1.016_666_7f32), (1234.567, 1234.583_7), (99.99, 100.023)] {
            for k in 0..=64 {
                let w = time_at(Some(f64::from(p)), f64::from(c), k as f32 / 64.0) as f32;
                let clocks = DrawClocks::still(0.0, 0.0, Some(w));
                assert_eq!((c - clocks.water_lag(c)).to_bits(), w.to_bits(), "{p}..{c} at {k}");
            }
        }
        assert_eq!(DrawClocks::still(0.0, 0.0, None).water_lag(3.0), 0.0);
    }
}
