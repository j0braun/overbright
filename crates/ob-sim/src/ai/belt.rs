//! The racers' off-screen mover, "BeltMover" (mode 4 of the mode switch
//! 0x5F46B0; ctor 0x5A9040, vtable 0x7A214C, name 0x59E940, update
//! 0x59EC90; verified in the decompiled code unless marked). It runs the
//! car along the driver's path (driver +0x10: count, points from +0x20)
//! with no tyres and no physics: a Catmull-Rom spline through the path
//! resampled to equal spacing, a parameter u that grows at a constant
//! acceleration between rebuilds, and the speed the car shows (+0x42C)
//! being u's rate over the knots per metre. See docs/formats.md, "Racer
//! movers" → "BeltMover".

use glam::{Vec2, Vec3};

/// The share of the driver's target speed the belt aims at (0x5916A0:
/// target × 0.899).
pub const TARGET_SHARE: f32 = 0.899;
/// The time to reach it when the driver's +0x234 is not above 0 (0.5 s).
pub const REACH: f32 = 0.5;
/// The belt's deceleration floor, m/s² (−20 × knots per metre).
pub const MAX_BRAKE: f32 = 20.0;
/// Gravity (0x7FBEF0).
pub const G: f32 = 9.8;
/// The look-ahead of the heading, s (0x7A17F0).
const HEADING_AHEAD: f32 = 0.4;
/// The heading filter's and the lateral offset's blend times (0x5916A0:
/// the +0x28 pair 0.4 s, the +0x18 offset 0.5 s).
const HEADING_BLEND: f32 = 0.4;
const OFFSET_BLEND: f32 = 0.5;
/// The drop test (0x59EA30): over 0.5 s (0x7A17E0) the road falling away
/// more than 1 m (0x7A17E8) under the car's free flight.
pub const DROP_TIME: f32 = 0.5;
pub const DROP_LIMIT: f32 = 1.0;

/// One channel of the game's blend object (0x574E40; +0 value, +4 rate
/// per blend time, +8 target, +0x0C target rate, +0x24 blend time): each
/// frame the value is not at its target (0x574BA0 restarts it) a cubic
/// Hermite from (value, rate) to (target, 0) over the blend time is
/// started and stepped by dt / blend time.
#[derive(Clone, Copy, Debug, Default)]
pub struct Blend {
    pub value: f32,
    pub rate: f32,
}

impl Blend {
    pub fn step(&mut self, target: f32, time: f32, dt: f32) {
        if self.value == target {
            return;
        }
        let d = target - self.value;
        let (c, v) = (self.rate, self.value);
        let a = c - 2.0 * d;
        let b = 3.0 * d - 2.0 * c;
        let tau = dt / time;
        if tau < 1.0 {
            self.value = ((tau * a + b) * tau + c) * tau + v;
            self.rate = (2.0 * b + 3.0 * a * tau) * tau + c;
        } else {
            self.value = target;
            self.rate = 0.0;
        }
    }
}

/// What the belt's build reads of the car (0x5916A0, 0x4038D0).
#[derive(Clone, Copy, Debug)]
pub struct CarNow {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Car +0x42C: the speed along the heading.
    pub speed: f32,
    /// The car's heading, radians (+0x40C).
    pub heading: f32,
}

/// The belt (BeltMover +0x0C … +0x2C).
#[derive(Clone, Debug)]
pub struct Belt {
    /// The resampled path, the spline's knots (+0x14 → +0x24).
    knots: Vec<Vec3>,
    /// Time since the build, s (+0x0C).
    pub t: f32,
    /// Knots per metre (+0x10): (n − 1) / the path's length.
    pub scale: f32,
    /// u at the build (+0x1C), its rate (+0x20, knots/s) and acceleration
    /// (+0x24, knots/s²).
    pub u0: f32,
    pub v0: f32,
    pub a: f32,
    /// The lateral offset off the spline (+0x18, blended to 0 over 0.5 s).
    offset: Blend,
    /// The heading vector's blend (+0x28: x, y over 0.4 s).
    look: [Blend; 2],
    /// Valid (+0x48: the path had two points or more).
    pub valid: bool,
}

/// The driver's path resampled to equal spacing (0x405120, run when the
/// path has more than two points): the cumulative lengths, then point i at
/// i · length / (n − 1) linearly between the polyline's points
/// (0x576780 / 0x5767A0 / 0x405020). Returns the points and the length
/// (driver +0x1D4).
pub fn resample(path: &[Vec3]) -> (Vec<Vec3>, f32) {
    let n = path.len();
    let mut sums = Vec::with_capacity(n);
    let mut total = 0.0f32;
    for i in 0..n {
        if i > 0 {
            total += path[i].distance(path[i - 1]);
        }
        sums.push(total);
    }
    if n <= 2 || total <= 0.0 {
        return (path.to_vec(), total);
    }
    let at = |s: f32| -> Vec3 {
        let k = sums.partition_point(|&x| x <= s).clamp(1, n - 1);
        let (s0, s1) = (sums[k - 1], sums[k]);
        let f = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        path[k - 1].lerp(path[k], f.clamp(0.0, 1.0))
    };
    ((0..n).map(|i| at(i as f32 * total / (n - 1) as f32)).collect(), total)
}

impl Belt {
    /// The knot at `i` with the type-2 ends (0x570250: one phantom point
    /// each side, the first segment's and the last's step repeated;
    /// further out the same steps continue, **inferred** for 0x570520).
    fn knot(&self, i: isize) -> Vec3 {
        let n = self.knots.len() as isize;
        if i < 0 {
            let step = self.knots[1] - self.knots[0];
            self.knots[0] + step * i as f32
        } else if i >= n {
            let step = self.knots[(n - 1) as usize] - self.knots[(n - 2) as usize];
            self.knots[(n - 1) as usize] + step * (i - n + 1) as f32
        } else {
            self.knots[i as usize]
        }
    }

    /// The spline at u (0x570650: knots i − 1 … i + 2 for i = trunc(u),
    /// Catmull-Rom (the basis 0x865230 built by 0x56FFB0)); `deriv` its
    /// derivative in knots.
    pub fn eval(&self, u: f32, deriv: bool) -> Vec3 {
        if self.knots.len() < 2 {
            return self.knots.first().copied().unwrap_or(Vec3::ZERO);
        }
        let i = u as isize;
        let f = u - i as f32;
        let [p0, p1, p2, p3] = [self.knot(i - 1), self.knot(i), self.knot(i + 1), self.knot(i + 2)];
        let c1 = p2 - p0;
        let c2 = p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3;
        let c3 = p1 * 3.0 - p0 - p2 * 3.0 + p3;
        if deriv {
            // The second basis (+0x28 + 0x40, flag 1): the derivative.
            (c1 + c2 * (2.0 * f) + c3 * (3.0 * f * f)) * 0.5
        } else {
            p1 + (c1 * f + c2 * (f * f) + c3 * (f * f * f)) * 0.5
        }
    }

    /// u at time t (0x591AE0): u0 + v0 t + a t² / 2.
    pub fn u(&self, t: f32) -> f32 {
        (t * self.a * 0.5 + self.v0) * t + self.u0
    }

    /// The point at time t (0x591AE0; before the first knot no lower than
    /// its height).
    pub fn point(&self, t: f32) -> Vec3 {
        let u = self.u(t);
        let mut p = self.eval(u, false);
        if u < 0.0 && !self.knots.is_empty() {
            p.z = p.z.max(self.knots[0].z);
        }
        p
    }

    /// The velocity at time t (0x591B80): the spline's derivative × u's
    /// rate.
    pub fn velocity(&self, t: f32) -> Vec3 {
        self.eval(self.u(t), true) * (self.v0 + self.a * t)
    }

    /// The speed the car shows, m/s (+0x42C = (a t + v0) / scale).
    pub fn speed(&self) -> f32 {
        // A belt with no path (a fresh BeltMover, e.g. after a reset's
        // placement 0x5F52D0) stands: no spline scale, speed 0.
        if self.scale > 0.0 { (self.a * self.t + self.v0) / self.scale } else { 0.0 }
    }

    /// The nearest u to a point (0x570740 from knot 0: walk on while the
    /// next knot is nearer, then back; 0x5774D0: Newton steps of at most a
    /// knot, halved on each sign change, until within 0.005 m (0x3BA3D70A),
    /// at most 998).
    fn nearest(&self, p: Vec3) -> f32 {
        let n = self.knots.len();
        let mut best = 0usize;
        let mut d = p.distance(self.knots[0]);
        for (i, k) in self.knots.iter().enumerate().skip(1) {
            let e = p.distance(*k);
            if d <= e {
                break;
            }
            d = e;
            best = i;
        }
        let mut u = best as f32;
        let (mut step_max, mut last_step, mut last_err) = (1.0f32, 0.0f32, 0.0f32);
        for _ in 0..998 {
            let at = self.eval(u, false);
            let tangent = self.eval(u, true);
            let len = tangent.length();
            if len == 0.0 {
                return u;
            }
            let err = (p - at).dot(tangent) / len;
            if err.abs() < 0.005 {
                return u;
            }
            if last_step != 0.0 && -((err - last_err) / last_step) == 0.0 {
                return u;
            }
            let step = (err / len).clamp(-step_max, step_max);
            u += step;
            if step * last_step < 0.0 {
                step_max *= 0.5;
            }
            last_step = step;
            last_err = err;
        }
        let _ = n;
        u
    }

    /// The build (0x5918B0 → 0x405120, 0x5916A0): the driver's path as the
    /// spline, t = 0, the scale (n − 1) / length, u0 the nearest point to
    /// the car, v0 the car's speed (+0x42C) × scale, and the acceleration
    /// that reaches 0.899 × the driver's target speed (+0x230, slot +0x74)
    /// in its +0x234 time (slot +0x78; 0.5 s when not above 0), held under
    /// `max_accel` (0x4038D0, m/s², given the slope term) × scale and above
    /// −20 × scale. The lateral offset starts at the car's offset off the
    /// spline (its rate half the car's lateral speed) and the heading
    /// blend, when `fresh` (the ctor 0x5A9040), at the car's velocity
    /// along its heading. A path of under two points leaves the belt
    /// invalid (+0x48 = 0).
    pub fn build(prev: Option<&Belt>, path: &[Vec3], car: CarNow, target: f32, reach: f32, max_accel: impl Fn(f32, f32) -> f32) -> Belt {
        let (knots, length) = if path.len() > 1 { resample(path) } else { (path.to_vec(), 0.0) };
        let look = prev.map_or_else(
            || {
                let f = Vec2::from_angle(car.heading) * car.velocity.length();
                [Blend { value: f.x, rate: 0.0 }, Blend { value: f.y, rate: 0.0 }]
            },
            |b| b.look,
        );
        let mut belt = Belt { knots, t: 0.0, scale: 0.0, u0: 0.0, v0: 0.0, a: 0.0, offset: Blend::default(), look, valid: false };
        if belt.knots.len() < 2 || length <= 0.0 {
            return belt;
        }
        belt.valid = true;
        belt.scale = (belt.knots.len() - 1) as f32 / length;
        belt.u0 = belt.nearest(car.position);
        let at = belt.eval(belt.u0, false);
        let tangent = belt.eval(belt.u0, true).normalize_or(Vec3::X);
        belt.v0 = car.speed * belt.scale;
        let reach = if reach <= 0.0 { REACH } else { reach };
        let mut a = (target * TARGET_SHARE * belt.scale - belt.v0) / reach;
        // 0x591630: −g × the spline's unit tangent z at u0.
        let cap = max_accel(belt.v0 / belt.scale, -G * tangent.z) * belt.scale;
        if a > cap {
            a = cap;
        } else if a < -MAX_BRAKE * belt.scale {
            a = -MAX_BRAKE * belt.scale;
        }
        belt.a = a;
        let side = tangent.cross(Vec3::Z);
        belt.offset = Blend { value: (car.position - at).dot(side), rate: car.velocity.dot(side) * 0.5 };
        belt
    }

    /// The drop test (0x59EA30(0.5)): g' = 9.8 + the downforce's pull
    /// (v0 / scale × (front + rear downforce) × 0.5 / mass); stepping 0.05 s
    /// to 0.45 s ahead, the free-flight height from the last point the path
    /// held the car (its height and vertical speed) against the path's; the
    /// most the path lies below it.
    pub fn drop(&self, downforce: f32, mass: f32) -> f32 {
        let g = self.v0 / self.scale * downforce * 0.5 / mass + G;
        let mut z0 = self.point(self.t).z;
        let mut vz = self.velocity(self.t).z;
        let (mut since, mut worst) = (0.0f32, 0.0f32);
        for k in 1..10 {
            let dt = k as f32 * DROP_TIME * 0.1;
            let t = dt + self.t;
            let path_z = self.point(t).z;
            let s = dt - since;
            let gap = ((s * -g + vz) * s + z0) - path_z;
            if gap <= 0.0 {
                vz = self.velocity(t).z;
                z0 = path_z;
                since = dt;
            } else if worst <= gap {
                worst = gap;
            }
        }
        worst
    }

    /// One frame (0x59EC90). None when the belt cannot run (fewer than two
    /// knots, both rates under 0.01 m/s, or u0 past the last knot): the
    /// game then rebuilds it and stops the car (+0x70 = 0, +0x42C = +0x430
    /// = 0). Else the new place (the point at t + dt, moved off the spline
    /// by the blended offset), heading (the blended velocity 0.4 s ahead)
    /// and speed.
    pub fn step(&mut self, dt: f32) -> Option<(Vec3, f32, f32)> {
        let stopped = self.scale > 0.0 && (self.a / self.scale).abs() < 0.01 && (self.v0 / self.scale).abs() < 0.01;
        if self.knots.len() < 2 || !self.valid || stopped || self.knots.len() as f32 <= self.u0 {
            return None;
        }
        self.t += dt;
        let speed = self.speed();
        let ahead = self.velocity(self.t + HEADING_AHEAD);
        self.look[0].step(ahead.x, HEADING_BLEND, dt);
        self.look[1].step(ahead.y, HEADING_BLEND, dt);
        let heading = self.look[1].value.atan2(self.look[0].value);
        let tangent = self.velocity(self.t).normalize_or(Vec3::X);
        let side = tangent.cross(Vec3::Z);
        self.offset.step(0.0, OFFSET_BLEND, dt);
        let at = self.point(self.t) + side * self.offset.value;
        Some((at, heading, speed))
    }
}

#[cfg(test)]
mod tests {

    #[test]
    /// A fresh BeltMover with no path (the reset's placement 0x5F52D0
    /// switches the mover off and on again) stands: speed 0, no step.
    fn a_belt_with_no_path_stands() {
        let car = CarNow { position: Vec3::ZERO, velocity: Vec3::new(80.0, 0.0, -80.0), speed: 80.0, heading: 0.0 };
        let mut b = Belt::build(None, &[], car, 30.0, 0.0, |_, _| 5.0);
        assert_eq!(b.speed(), 0.0);
        assert!(b.step(1.0 / 60.0).is_none());
    }

    use super::*;

    fn line(n: usize, step: f32) -> Vec<Vec3> {
        (0..n).map(|i| Vec3::new(i as f32 * step, 0.0, 0.0)).collect()
    }

    #[test]
    fn belt_reaches_its_share_of_the_target() {
        let path = line(20, 5.0);
        let car = CarNow { position: Vec3::new(0.0, 1.0, 0.0), velocity: Vec3::X * 20.0, speed: 20.0, heading: 0.0 };
        let mut b = Belt::build(None, &path, car, 30.0, 0.0, |_, _| 50.0);
        assert!((b.scale - 0.2).abs() < 1e-5);
        assert!(b.u0.abs() < 1e-2);
        let mut last = None;
        for _ in 0..30 {
            last = b.step(1.0 / 60.0);
        }
        let (at, heading, speed) = last.unwrap();
        assert!((speed - 26.97).abs() < 0.01, "{speed}");
        assert!(heading.abs() < 1e-3);
        assert!(at.y.abs() < 0.5 && at.x > 10.0, "{at}");
    }

    #[test]
    fn belt_is_held_by_the_car_s_acceleration() {
        let path = line(20, 5.0);
        let car = CarNow { position: Vec3::ZERO, velocity: Vec3::X * 20.0, speed: 20.0, heading: 0.0 };
        let mut b = Belt::build(None, &path, car, 40.0, 0.0, |_, slope| 2.0 + slope);
        for _ in 0..60 {
            b.step(1.0 / 60.0);
        }
        assert!((b.speed() - 22.0).abs() < 0.01);
    }
}
