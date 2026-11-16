//! The turbo's flame pops: car effect kind 10 (`CARFX_NITRO`, the nitrous
//! flame's emitters at the exhaust markers) fired without a nitrous shot
//! (docs/sparks-fire.md "Native triggers").
//!
//! **Verified** in the disassembly (REA decompile and `speed2.dis`):
//! * **At a shift** — 0x5EBE10, called by each car's frame Car_Frame
//!   0x6012C0 (0x6012EB) with the gear (car +0x434). When the gear differs
//!   from the last seen (+0x4D0) it is stored, and the pop is armed for a
//!   car on the physics mover (car +0x28 = 1) or when more than 0.5 s
//!   (0x7A5620) of the game clock (0x865188 × 0.00025, 0x784268) passed
//!   since the last change (+0x9D8, restamped on every change). On the
//!   physics mover in a gear above 2 (above first: 0 reverse, 1 neutral,
//!   2 first) the pop is instead the clutch locking: car +0x448 (the
//!   transmission's state 3, Locked) set now and clear at the last call
//!   (+0x4D4, which follows +0x448 on every return). The intensity is
//!   L = 0x5995F0(4) (the turbo's rounded upgrade level, ride +0x2F0),
//!   + 0.5 (0x784260) when ride +0x328 is set; below 2.0 (0x7844C0)
//!   nothing fires, else 0x61FB10(10, L × 0.2857143 (0x7A5D3C), gear, 0).
//!   The sound context's slot +0x48 gets event 1 on a gear change and 9 on
//!   a pop (0x459550: both ignored, only 7 and 8 do anything).
//! * **Revving in neutral** — 0x5F5F5F in each car's update 0x5F5740,
//!   after the shot's flame or purge: in neutral (car +0x434 = 1) with
//!   L ≥ 2 and the rpm r (car +0x440; the override 0x7FBED8 is off at 100)
//!   against the range R = engine +0x1F8 − +0x1F0 (the tuning block's max
//!   and idle rpm, record +0x308 / +0x300): armed (car +0x924) when
//!   r ≤ 0.5 R (0x7A5638), and an armed car past 0.75 R (0x7A5634) pops
//!   (the same call) and disarms.
//!
//! **Observed** under Wine (2026-11-05, D-Day free roam, Rachel's 350Z:
//! upgrade float[4] = 1.98 → L = 2, ride +0x328 = 0): driving off on the
//! keys, the CARFX_NITRO emitter's random state (+0x24) and velocity
//! (+0x80) change on the update the clutch locks in second gear (gear 3,
//! +0x448 0 → 1), and at no other time; standing in first the car reads
//! gear 2, armed (+0x924 = 1). (`docs/evidence/sparks-fire/`.)
//!
//! **Inferred / open**: what ride +0x328 is (0 on every car read; only the
//! intensity depends on it, never whether a pop fires); the last gear's
//! value before a car's first frame (taken as its first gear: no pop on
//! load); Car_Frame's place in the update relative to the particle update
//! (the clone fires the pops with the other car effects, after it).

use crate::car::Car;
use crate::drive::CarState;
use crate::handling::{Shift, NEUTRAL};

/// 0x7A5D3C: the pop's intensity per turbo level.
pub const POP_SCALE: f32 = 0.285_714_3;

/// What the pops read of a car each update.
#[derive(Clone, Copy, Debug)]
pub struct CarIn {
    /// Car +0x28 = 1: the full-physics mover (not the MeloMvr).
    pub physics: bool,
    /// Car +0x434.
    pub gear: usize,
    /// Car +0x448: the clutch locked (transmission state 3).
    pub locked: bool,
    /// Car +0x440.
    pub rpm: f32,
    /// Engine +0x1F0 / +0x1F8 (record +0x300 / +0x308).
    pub idle: f32,
    pub max: f32,
    /// L: 0x5995F0(4) (+ 0.5 with ride +0x328, taken as clear).
    // prov: open carfx_pops.ride_328
    pub level: f32,
}

impl CarIn {
    pub fn of(car: &Car, state: &CarState) -> CarIn {
        let e = &car.car_type.handling.engine;
        let (physics, gear, locked) = match (&state.melo, &state.drivetrain) {
            (Some(m), _) => (false, m.gear, false),
            (None, Some(t)) => (true, t.gear, t.shift == Shift::Locked),
            (None, None) => (false, NEUTRAL, false),
        };
        CarIn { physics, gear, locked, rpm: state.shown_rpm(), idle: e.idle_rpm, max: e.max_rpm, level: f32::from(car.car_type.turbo_level) }
    }
}

/// One car's pop memory.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pops {
    /// Car +0x4D0 (None before its first frame).
    gear: Option<usize>,
    /// Car +0x4D4: +0x448 at the last call.
    locked: bool,
    /// Car +0x9D8: the clock at the last gear change, s.
    changed: f32,
    /// Car +0x924: the neutral rev pop armed.
    armed: bool,
}

impl Pops {
    /// 0x5EBE10 for one frame: the pop's intensity, if it fires.
    pub fn shift(&mut self, c: &CarIn, clock: f32) -> Option<f32> {
        let mut fire = false;
        match self.gear {
            // prov: inferred carfx_pops.first_gear
            None => self.gear = Some(c.gear),
            Some(g) if g != c.gear => {
                self.gear = Some(c.gear);
                fire = c.physics || clock - self.changed > 0.5;
                self.changed = clock;
            }
            _ => {}
        }
        if c.physics && c.gear > 2 {
            fire = !self.locked && c.locked;
        }
        self.locked = c.locked;
        (fire && c.level >= 2.0).then_some(c.level * POP_SCALE)
    }

    /// 0x5F5F5F for one update: the pop's intensity, if it fires.
    pub fn rev(&mut self, c: &CarIn) -> Option<f32> {
        if c.gear != NEUTRAL || c.level < 2.0 {
            return None;
        }
        let range = c.max - c.idle;
        if self.armed {
            if c.rpm < range * 0.75 {
                return None;
            }
            self.armed = false;
            Some(c.level * POP_SCALE)
        } else {
            if c.rpm <= range * 0.5 {
                self.armed = true;
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(gear: usize, locked: bool, rpm: f32, level: f32) -> CarIn {
        CarIn { physics: true, gear, locked, rpm, idle: 800.0, max: 8000.0, level }
    }

    #[test]
    fn shift_pop_fires_when_the_clutch_locks_above_first() {
        let mut p = Pops::default();
        // First frame: no pop (inferred), first gear standing, unlocked.
        assert_eq!(p.shift(&car(2, false, 800.0, 2.0), 0.0), None);
        // Pulling away in first: the clutch locks — gear ≤ 2, no pop.
        assert_eq!(p.shift(&car(2, true, 900.0, 2.0), 0.1), None);
        // Up-shift to second: the gear changes with the clutch open — the
        // gear-above-2 rule replaces the change test (no pop yet) …
        assert_eq!(p.shift(&car(3, false, 6900.0, 2.0), 2.5), None);
        assert_eq!(p.shift(&car(3, false, 6000.0, 2.0), 2.55), None);
        // … and the lock fires L · 2/7 once (the Wine log: 2nd gear).
        let a = p.shift(&car(3, true, 5800.0, 2.0), 2.6).unwrap();
        assert!((a - 0.571_428_6).abs() < 1e-5);
        assert_eq!(p.shift(&car(3, true, 6000.0, 2.0), 2.65), None);
    }

    #[test]
    fn shift_pop_needs_turbo_level_two() {
        let mut p = Pops::default();
        p.shift(&car(3, false, 0.0, 1.0), 0.0);
        assert_eq!(p.shift(&car(3, true, 0.0, 1.0), 0.1), None);
        let mut p = Pops::default();
        p.shift(&car(3, false, 0.0, 3.0), 0.0);
        assert!((p.shift(&car(3, true, 0.0, 3.0), 0.1).unwrap() - 0.857_142_9).abs() < 1e-5);
    }

    #[test]
    fn shift_pop_into_first_and_reverse_on_the_change() {
        let mut p = Pops::default();
        p.shift(&car(1, true, 800.0, 2.0), 0.0);
        assert!(p.shift(&car(2, true, 800.0, 2.0), 0.1).is_some());
        assert!(p.shift(&car(0, false, 800.0, 2.0), 0.2).is_some());
    }

    #[test]
    fn other_movers_pop_on_a_change_half_a_second_apart() {
        let mut c = car(2, false, 0.0, 2.0);
        c.physics = false;
        let mut p = Pops::default();
        p.shift(&c, 0.0);
        c.gear = 3;
        assert!(p.shift(&c, 1.0).is_some());
        c.gear = 4;
        assert_eq!(p.shift(&c, 1.3), None);
        c.gear = 5;
        assert!(p.shift(&c, 1.9).is_some());
    }

    #[test]
    fn rev_pop_in_neutral_arms_low_and_fires_high() {
        let mut p = Pops::default();
        // R = 7200: arm at ≤ 3600, fire at ≥ 5400.
        assert_eq!(p.rev(&car(NEUTRAL, false, 3000.0, 2.0)), None);
        assert_eq!(p.rev(&car(NEUTRAL, false, 5000.0, 2.0)), None);
        assert!(p.rev(&car(NEUTRAL, false, 5500.0, 2.0)).is_some());
        assert_eq!(p.rev(&car(NEUTRAL, false, 7000.0, 2.0)), None);
        // In gear nothing (and the arm is untouched).
        assert_eq!(p.rev(&car(2, false, 1000.0, 2.0)), None);
        assert_eq!(p.rev(&car(2, false, 6000.0, 2.0)), None);
    }
}
