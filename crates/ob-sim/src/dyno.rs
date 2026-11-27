//! The dyno pull (docs/formats.md, "Dyno chart"): the dyno manager
//! (vehicle +0x1E0, made by 0x56E620 only in the dyno race, race type 8;
//! started by 0x56D1B0 → 0x56D350, updated by 0x56E6F0) drives the car
//! itself, and the chart's progress (0x56D450) is a moving average of its
//! engine rpm.
//!
//! Verified: the start (0x56D350) puts the car in neutral at rest (+0x434
//! = 1, rpm and wheel speeds 0), the automatic gearbox (vt+0x28(−1)), the
//! average reset to 0 (0x5769E0) and asks for first gear (vt+0x28(2), the
//! target +8 = 2). Each frame (0x56E6F0), by the state +4:
//! 1. brake off (0x5EB900(0)); in gear 4 (the third forward gear: 0 R,
//!    1 N) go to 2; in the target gear full gas (0x5EB920(1)), the target
//!    one up once the rpm (0x4018C0: +0x440, or the tweak 0x7FBED8 when it
//!    is not 100) reaches 3000, and a 0 into the average; else shift to the
//!    target with the gas off;
//! 2. brake off, full gas, the rpm into the average; at progress 1 the
//!    automatic gearbox again (vt+0x28(−1)) and 3;
//! 3. gas off, the brake on while in gear 3 or above (0x5EB900), 4 once the
//!    speed (+0x430 × 2.237) is 0;
//! 4. stopped (0x56E690(1)): state 0.
//!
//! The average (0x579F30(20), 0x576980) keeps the last 20 samples: count
//! up to 20, sum += x − the sample it replaces, mean = sum / count. The
//! progress (0x56D450): 0 in state 2 unless in gear 4, else mean / (max
//! rpm − 200), clamped 0..1.
//!
//! The pull's power (verified, 0x5B0530's opening): while the mover is
//! held and the manager is in state 2, every sub-step's force pass sets the
//! engine's power factor (+0x80) to 1.5 (0x7A1A28), so the pull runs at
//! 1.5× torque (`CarState::dyno_boost`); without it third gear took 1.75×
//! the original's time (tests/dyno_pull.rs).
//!
//! The manager is the car's controller in the dyno race: the vehicle
//! update 0x6016B0 runs it instead of the driver's input for race type 8,
//! once per world frame with that frame's time (0x865150). The body is
//! held: the reset 0x56D350 calls 0x599210(1) (at 0x56D415, the car on its
//! full-physics mover), whose position spring 0x5A5C30 keeps the centre
//! of mass where it stood while the velocity runs on (
//! `CarState::set_hold`).
//!
// prov: inferred dyno.module
//! **Inferred**: the frame is 1/60 s (the game's world frame varies; only
//! the 20-sample average's lag depends on it), and the car runs its own
//! physics on a flat floor, held in place by the reset's spring (the
//! rollers, 0x56D1B0, only follow the wheel speeds +0x330..).
use glam::{Mat4, Quat, Vec3};

use crate::car::Car;
use crate::drive::{AiControls, CarField, CarState, Input, Racer, step_cars_posting};
use crate::world::World;

/// The manager's frame, s (inferred: one world frame).
pub const FRAME: f32 = 1.0 / 60.0;
/// Frames the clone's car settles on its springs before a pull (1.5 s).
const SETTLE_FRAMES: usize = 90;
/// The average's length (0x56E620 → 0x579F30(0x14)).
const WINDOW: usize = 20;

/// The moving average 0x576980.
#[derive(Clone, Debug)]
struct Average {
    samples: [f32; WINDOW],
    next: usize,
    count: usize,
    sum: f32,
    mean: f32,
}

impl Average {
    /// 0x5769E0(v): every sample v, the count 0, the sum 20·v.
    fn reset(v: f32) -> Average {
        Average { samples: [v; WINDOW], next: 0, count: 0, sum: v * WINDOW as f32, mean: 0.0 }
    }

    fn push(&mut self, x: f32) {
        self.count = (self.count + 1).min(WINDOW);
        self.sum = self.sum + x - self.samples[self.next];
        self.samples[self.next] = x;
        self.next = (self.next + 1) % WINDOW;
        self.mean = self.sum / self.count as f32;
    }
}

/// A dyno pull in progress.
pub struct DynoPull {
    car: Car,
    state: CarState,
    world: World,
    field: CarField,
    /// The contact sounds its steps posted (the caller takes them
    /// into its session's queues after each call, as the process-wide
    /// queues took them before).
    pub sounds: crate::drive::ContactSounds,
    /// The manager's state (+4) and target gear (+8).
    stage: u8,
    target: usize,
    /// The gear asked of the gearbox (vt+0x28: −1 the automatic choice).
    request: i32,
    gas: f32,
    brake: f32,
    average: Average,
    /// Time not yet stepped, s.
    pending: f32,
    /// The settled car before the pull (centre-of-mass height, the body's
    /// turn, each wheel's compression): what [`DynoPull::pose`] measures
    /// the drawn car's movement from.
    rest: (f32, Quat, [f32; 4]),
}

/// The rollers' model (0x56D060: 0x637DC0 with the hash of the name at
/// 0x79DC24), a solid of the dyno region's own stream (L4RG).
pub use crate::world::DYNO_ROLLER_MODEL as ROLLER_MODEL;
/// 0x56D1B0: each wheel's roller sits this far ahead of (wheels 0 and 2)
/// or behind (1 and 3) the wheel's centre along the car, m.
const ROLLER_SPLIT: f32 = 0.26;
/// 0x56D1B0: the rollers' centres this far below the floor under them, m.
const ROLLER_DEPTH: f32 = 0.175;

/// The dyno's four rollers (the manager's objects +0x40, 0xD0 each).
///
/// Verified (0x56D1B0, run when the manager is made, 0x5FC4FF, and on each
/// pull's reset, 0x56E708): roller i belongs to wheel i; its centre is the
/// car's centre of mass (+0x60) plus the car's forward axis (matrix +0x90)
/// times (wheel x ± 0.26 − centre-of-mass x; + for wheels 0 and 2) — no
/// sideways part, so each roller sits on the car's centre line — at the
/// height of the floor under that point (0x5E72F0, asked from the centre
/// of mass's height plus the forward / up parts of (dx, wheel z − 0.175 −
/// centre-of-mass z)) less 0.175; its matrix is the car's (0x4012B0).
/// Each frame of a pull (0x56E6F0, manager state ≠ 0) roller i's angle
/// (+0xC0) loses wheel i's spin (car +0x330 + 4i, rad/s) × the frame's
/// time, and the draw (0x56E900 → 0x56E1E0, in the car pass after the GPS
/// aid, 0x63178D) is RotateY(angle × 65536/2π) × the car's matrix at the
/// centre: the roller turns about the car's sideways axis.
#[derive(Clone, Debug)]
pub struct Rollers {
    pub centres: [Vec3; 4],
    pub turn: Quat,
    pub angles: [f32; 4],
    /// Clone only (rate independence): the spins the last [`Rollers::turn_by`] took
    /// (rad/s), so a frame between two ticks turns each roller the way
    /// it went ([`Rollers::angles_at`]).
    pub spins: [f32; 4],
}

impl Rollers {
    /// 0x56D1B0 for `car` with its centre of mass at `com` (world) and
    /// its body turned by `turn`; `ground` is the floor query 0x5E72F0.
    pub fn place(car: &crate::car::CarType, com: Vec3, turn: Quat, ground: impl Fn(Vec3) -> Option<f32>) -> Rollers {
        let c = Vec3::from(car.handling.centre_of_mass);
        let (fwd, up) = (turn * Vec3::X, turn * Vec3::Z);
        let centres = std::array::from_fn(|i| {
            let w = car.wheels[i].position;
            let split = if i % 2 == 0 { ROLLER_SPLIT } else { -ROLLER_SPLIT };
            let (dx, dz) = (split + w.x - c.x, w.z - ROLLER_DEPTH - c.z);
            let p = com + fwd * dx + up * dz;
            let z = ground(p).unwrap_or(com.z);
            Vec3::new(p.x, p.y, z - ROLLER_DEPTH)
        });
        Rollers { centres, turn, angles: [0.0; 4], spins: [0.0; 4] }
    }

    /// One pull frame (0x56E6F0's end): each roller turns back by its
    /// wheel's spin.
    pub fn turn_by(&mut self, spins: [f32; 4], dt: f32) {
        self.spins = spins;
        for (a, s) in self.angles.iter_mut().zip(spins) {
            *a = (*a - s * dt).rem_euclid(std::f32::consts::TAU);
        }
    }

    /// The four draw matrices (0x56E1E0).
    pub fn transforms(&self) -> [Mat4; 4] {
        self.transforms_with(&self.angles)
    }

    /// [`Rollers::transforms`] with the rollers at `angles`.
    pub fn transforms_with(&self, angles: &[f32; 4]) -> [Mat4; 4] {
        std::array::from_fn(|i| Mat4::from_rotation_translation(self.turn, self.centres[i]) * Mat4::from_rotation_y(angles[i]))
    }

    /// The rollers' angles `t` of the way from a tick's `before`
    /// to the newest tick's (`self`, `dt` long): exactly `before` at t ≤ 0,
    /// exactly the newest at t ≥ 1; between, each the way its spin turned
    /// it (−spin·dt, not the shortest way: a fast pull turns over half a
    /// turn a tick), as the cars' wheels (`interpolate::CarPose::blend`).
    pub fn angles_at(&self, before: &[f32; 4], t: f32, dt: f32) -> [f32; 4] {
        if !(t > 0.0) {
            return *before;
        }
        if t >= 1.0 {
            return self.angles;
        }
        let tau = std::f32::consts::TAU;
        std::array::from_fn(|i| {
            let (a, b) = (before[i], self.angles[i]);
            let mut d = b - a;
            d += tau * ((-self.spins[i] * dt - d) / tau).round();
            a + d * t
        })
    }
}

/// How the pulled car moves on the rollers, for drawing the dyno's car:
/// each wheel's turn (`roll`) and spin, each wheel's compression change,
/// and the body's tilt and rise about its centre of mass, all measured
/// from the settled car before the pull.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub roll: [f32; 4],
    pub spin: [f32; 4],
    pub compression: [f32; 4],
    /// The body's turn from its settled one, in the body's own axes.
    pub tilt: Quat,
    /// The centre of mass's rise, m.
    pub rise: f32,
}

impl DynoPull {
    /// The player's Stability Control (car +0x464: the dyno race places
    /// the player's own car, 0x5F1802 → 0x5F3E20).
    pub fn with_stability_off(mut self, off: bool) -> DynoPull {
        self.state.stability_off = off;
        self
    }

    /// 0x56D350 for `car`, at rest on a flat floor.
    pub fn new(car: Car) -> DynoPull {
        let w = car.car_type.wheels[0];
        let mut state = CarState::new(Vec3::new(-900.0, 0.0, w.radius - w.position.z), 0.0);
        // The reset's hold (0x56D350 → 0x599210(1)): no aero.
        state.aero_off = true;
        if let Some(d) = state.drivetrain.as_mut() {
            d.gear = crate::handling::NEUTRAL;
        }
        let mut pull = DynoPull {
            car,
            state,
            world: World::flat(1000.0),
            field: CarField::default(),
            sounds: Default::default(),
            stage: 1,
            target: 2,
            request: 2,
            gas: 0.0,
            brake: 0.0,
            average: Average::reset(0.0),
            pending: 0.0,
            rest: (0.0, Quat::IDENTITY, [0.0; 4]),
        };
        // The game's car has stood on the dyno before the pull (its reset
        // 0x56D350 zeroes the rpm and the wheels' spin, not the body): it
        // starts settled on its springs. The clone's starts with the wheels
        // just touching, so it settles first, in neutral with no pedals
        // prov: inferred dyno.new
        // (**inferred** equivalent; it changes nothing the chart reads).
        for _ in 0..SETTLE_FRAMES {
            let input = Input::Ai(AiControls { steer_degrees: 0.0, gas: 0.0, brake: 0.0, handbrake: 0.0, gear: crate::handling::NEUTRAL as i32, nitrous: false, mover_held: false });
            pull.step_car(input);
        }
        if let Some(d) = pull.state.drivetrain.as_mut() {
            d.gear = crate::handling::NEUTRAL;
        }
        pull.state.velocity = Vec3::ZERO;
        pull.state.set_exported_forward_speed(0.0); // native dyno reset 56D3D2
        pull.state.spin = Vec3::ZERO;
        pull.state.angular_momentum = Vec3::ZERO;
        for w in pull.state.wheels.iter_mut() {
            w.spin = 0.0;
        }
        // The reset's hold (0x56D350 → 0x599210(1) at 0x56D415, the car on
        // its full-physics mover): the pose saved where the car stands; the
        // position spring keeps it there.
        pull.state.set_hold(&pull.car);
        pull.rest = (pull.com().z, pull.state.orientation, std::array::from_fn(|i| pull.state.wheels[i].compression));
        pull
    }

    /// The car's centre of mass, world.
    fn com(&self) -> Vec3 {
        self.state.position + self.state.orientation * Vec3::from(self.car.car_type.handling.centre_of_mass)
    }

    /// The pull's car as the dyno draws it (see [`Pose`]).
    pub fn pose(&self) -> Pose {
        let (z, turn, compression) = self.rest;
        Pose {
            roll: std::array::from_fn(|i| self.state.wheels[i].roll),
            spin: std::array::from_fn(|i| self.state.wheels[i].spin),
            compression: std::array::from_fn(|i| self.state.wheels[i].compression - compression[i]),
            tilt: (turn.inverse() * self.state.orientation).normalize(),
            rise: self.com().z - z,
        }
    }

    /// The pull's car (its engine and gearbox for the sound).
    pub fn car_state(&self) -> &CarState {
        &self.state
    }

    /// The car's gear (+0x434: 0 reverse, 1 neutral, 2 first, …).
    pub fn gear(&self) -> usize {
        self.state.drivetrain.as_ref().map_or(crate::handling::NEUTRAL, |d| d.gear)
    }

    /// The car's shown rpm (+0x440, 0x4018C0; `CarState::shown_rpm`).
    pub fn rpm(&self) -> f32 {
        self.state.shown_rpm()
    }

    /// The averaged rpm the chart follows.
    pub fn mean_rpm(&self) -> f32 {
        self.average.mean
    }

    /// The manager's state: 1 shifting up, 2 the pull, 3 slowing, 4 / 0
    /// stopped.
    pub fn stage(&self) -> u8 {
        self.stage
    }

    /// The chart's progress (0x56D450).
    pub fn progress(&self) -> f32 {
        if self.stage == 2 && self.gear() != 4 {
            return 0.0;
        }
        let p = self.average.mean / (self.car.car_type.handling.engine.max_rpm - 200.0);
        if p > 0.0 { p.min(1.0) } else { 0.0 }
    }

    /// Run `dt` seconds of frames.
    pub fn step(&mut self, dt: f32) {
        self.pending += dt;
        while self.pending >= FRAME {
            self.pending -= FRAME;
            self.frame();
        }
    }

    /// One frame: the manager (0x56E6F0), then the car's frame with its
    /// pedals and gear request.
    fn frame(&mut self) {
        match self.stage {
            1 => {
                self.brake = 0.0;
                if self.gear() == 4 {
                    self.stage = 2;
                } else if self.gear() == self.target {
                    self.gas = 1.0;
                    if self.rpm() >= 3000.0 {
                        self.target += 1;
                    }
                    self.average.push(0.0);
                } else {
                    self.request = self.target as i32;
                    self.gas = 0.0;
                }
            }
            2 => {
                self.brake = 0.0;
                self.gas = 1.0;
                self.average.push(self.rpm());
                if self.progress() == 1.0 {
                    self.request = -1;
                    self.stage = 3;
                }
            }
            3 => {
                self.gas = 0.0;
                self.brake = if self.gear() < 3 { 0.0 } else { 1.0 };
                if self.state.speed() * 2.237 == 0.0 {
                    self.stage = 4;
                }
            }
            4 => self.stage = 0,
            _ => {}
        }
        // The force pass reads the manager's state each sub-step (0x5B0549):
        // the pull (state 2) runs the held car's engine at 1.5 power.
        self.state.dyno_boost = self.stage == 2;
        let input = Input::Ai(AiControls { steer_degrees: 0.0, gas: self.gas, brake: self.brake, handbrake: 0.0, gear: self.request, nitrous: false, mover_held: false });
        self.step_car(input);
    }

    /// One frame of the car's physics with `input`.
    fn step_car(&mut self, input: Input) {
        let steps = (FRAME / crate::drive::STEP).ceil().max(1.0) as u32;
        for _ in 0..steps {
            step_cars_posting(&mut [Racer { car: &self.car, state: &mut self.state, input }], &mut self.field, &self.world, FRAME / steps as f32, &mut self.sounds);
        }
    }
}

#[cfg(test)]
mod rollers_between_tests {
    use super::*;

    /// The rollers drawn between two ticks: the older tick's
    /// angles at t = 0, the newest's at t = 1, between the way the spin
    /// turned them, also over half a turn a tick.
    #[test]
    fn rollers_turn_between_ticks_the_spins_way() {
        let mut r = Rollers { centres: [Vec3::ZERO; 4], turn: Quat::IDENTITY, angles: [0.5, 1.0, 6.0, 3.0], spins: [0.0; 4] };
        let before = r.angles;
        let dt = 1.0 / 60.0;
        let spins = [30.0, -30.0, 120.0, 250.0];
        r.turn_by(spins, dt);
        assert_eq!(r.angles_at(&before, 0.0, dt).map(f32::to_bits), before.map(f32::to_bits));
        assert_eq!(r.angles_at(&before, 1.0, dt).map(f32::to_bits), r.angles.map(f32::to_bits));
        let mid = r.angles_at(&before, 0.5, dt);
        for i in 0..4 {
            let want = before[i] - spins[i] * dt * 0.5;
            let d = (mid[i] - want).rem_euclid(std::f32::consts::TAU);
            assert!(d < 1e-4 || d > std::f32::consts::TAU - 1e-4, "roller {i}: {} vs {want}", mid[i]);
        }
    }
}
