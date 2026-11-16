use super::*;

/// The DriftDriver (0x425E40, vtable 0x784F40, 0x250 bytes): the car
/// driver the AI racer controller (0x4260B0) builds instead of the plain
/// one (0x425AB0, "PsychoDriver") when a drift flag is up (0x890114, the
/// drift race 0x89E7D9 or 0x890104) and no drag flag (0x89E7D8, 0x890108).
/// Drift races have no AI opponents, so the only car it drives is the
/// player's past the finish (0x606CC0 → 0x5EBA40(3)). Its frame
/// (0x416120, vtable slot 1) holds a slip angle towards the look-ahead
/// point with the steering, the pedals and the handbrake; see
/// `docs/formats.md` (Drift driver).
#[derive(Clone, Debug)]
pub struct DriftDriver {
    /// +0x224: the look-ahead point (vt+0x80, 0x405BC0).
    pub target: Vec2,
    /// +0x22C: the target speed (vt+0x38, 0x405B60: at most 31.29 m/s).
    target_speed: f32,
    /// +0x230: last frame's slip angle (0x591DA0).
    prev_slip: f32,
    /// +0x238: the slip angle it asks for.
    pub slip_goal: f32,
    /// +0x23C (vt+0x1C, 0x405B10): hold, gas and brake both on.
    pub hold: bool,
    /// +0x1E0: the front-wheel angle, 16-bit; +0x1E4 the gear request
    /// (1 from the base constructor 0x423330).
    steer: u16,
    gear: i32,
    /// The pedals (+0x20C gas, +0x214 brake, +0x210 handbrake).
    gas: f32,
    brake: f32,
    handbrake: f32,
}

impl Default for DriftDriver {
    fn default() -> DriftDriver {
        DriftDriver {
            target: Vec2::ZERO,
            target_speed: 0.0,
            prev_slip: 0.0,
            slip_goal: 0.0,
            hold: false,
            steer: 0,
            gear: 1,
            gas: 0.0,
            brake: 0.0,
            handbrake: 0.0,
        }
    }
}

/// The drift driver's tuning (0x783A5C–0x783AC4, degrees as stored; the
/// code turns them into radians by 0x784244).
pub(super) const DRIFT_SPEED_CAP: f32 = 31.291_908;
pub(super) const DRIFT_FACING: f32 = 120.0;
pub(super) const DRIFT_FACING_REVERSE: f32 = 30.0;
pub(super) const DRIFT_SLOW: f32 = 10.0 * 0.447_027_27;
pub(super) const DEG: f32 = 0.017_453_292;

/// 0x405940: an angle into [−π, π) (floor(x / 2π + ½) turns off, 2π read
/// as 6.28318).
pub(super) fn wrap_pi(x: f32) -> f32 {
    let n = (x * 0.159_155_07 + 0.5).floor();
    x - n * 6.28318
}

/// 0x401090: x held to [lo, hi] (lo first).
pub(super) fn clamp_lo_hi(x: f32, lo: f32, hi: f32) -> f32 {
    let y = if x > lo { x } else { lo };
    if hi < y { hi } else { y }
}

/// 0x591DA0 on the car's body: the heading (+0x40C) less the direction of
/// the velocity (+0x70, +0x74), 16-bit, as radians in (−π, π].
pub fn slip16(car: &CarState) -> f32 {
    let f = car.forward();
    let a = angle16(f.x, f.y).wrapping_sub(angle16(car.velocity.x, car.velocity.y)) as f32 * 9.58738e-05;
    if a > std::f32::consts::PI { a - 2.0 * std::f32::consts::PI } else { a }
}

impl DriftDriver {
    /// 0x405990: turn the front wheels towards `angle` (radians) at most
    /// 360°/s, held to ±45°.
    pub(super) fn steer_to(&mut self, angle: f32, dt: f32) {
        let w = wrap_pi(self.steer as f32 * 9.58738e-05);
        let step = 360.0 * dt * DEG;
        let mut d = angle - w;
        if d <= -step {
            d = -step;
        }
        if step < d {
            d = step;
        }
        let mut n = w + d;
        if n <= -std::f32::consts::FRAC_PI_4 {
            n = -std::f32::consts::FRAC_PI_4;
        } else if std::f32::consts::FRAC_PI_4 < n {
            n = std::f32::consts::FRAC_PI_4;
        }
        self.steer = (n * 10430.378) as i32 as u16;
    }

    /// One frame (0x416120): `path` the controller's path (driver +0x10),
    /// `target_speed` what the controller asks (vt+0x38), `since_green`
    /// the race clock (0x402000) for the path's corner weight.
    pub fn update(
        &mut self,
        car: &CarState,
        path: &[Vec3],
        target_speed: f32,
        since_green: f32,
        dt: f32,
    ) -> AiControls {
        self.target_speed = if target_speed > DRIFT_SPEED_CAP { DRIFT_SPEED_CAP } else { target_speed };
        let pos = car.position;
        let v = car.velocity.length();
        let k = 9.58738e-05;
        // The look-ahead: 15–30 m along the path by the cross-track offset
        // and speed (0x404AF0, 0x415630); a second point at 8–12 m gives
        // the path's direction. Without a path 10 m along the heading
        // (0x403AF0).
        let mut path_yaw = 0.0;
        let look = if path.is_empty() {
            let f = car.forward();
            let h = angle16(f.x, f.y) as f32 * k;
            pos + Vec3::new(h.cos(), h.sin(), 0.0) * 10.0
        } else {
            let d1 = clamp_lo_hi(cross_track(path, pos).abs() * 0.75 + 0.04 * v + 15.0, 15.0, 30.0);
            let p1 = path_point(car, pos, path, d1, since_green);
            let d2 = clamp_lo_hi(0.08 * v + d1, 8.0, 12.0);
            let p2 = path_point(car, pos, path, d2, since_green);
            path_yaw = angle16(p2.x - p1.x, p2.y - p1.y) as f32 * k;
            p1
        };
        self.target = look.truncate();
        let aim = wrap_pi(angle16(self.target.x - pos.x, self.target.y - pos.y) as f32 * k);
        let f = car.forward();
        let heading = angle16(f.x, f.y) as f32 * k;
        // Facing away (more than 120°, 90° while in reverse) or asked to
        // reverse: back up.
        let limit = if self.gear == 0 { (DRIFT_FACING - DRIFT_FACING_REVERSE) * DEG } else { DRIFT_FACING * DEG };
        let away = wrap_pi(heading - aim).abs() > limit || self.target_speed < 0.0;
        if self.hold {
            self.brake = 1.0;
            self.gas = 1.0;
            self.steer_to(0.0, 1.0);
            self.gear = 1;
        } else if !away && DRIFT_SLOW <= v {
            // Drifting: the slip angle asked for turns the travel towards
            // the aim (and the path, whose weight −0.1·v held to [0, 1] is
            // 0 at any forward speed), at most 45° + 0.03·v (≤ 70°).
            let travel = angle16(car.velocity.x, car.velocity.y) as f32 * k;
            let slip = slip16(car);
            let rate = wrap_pi(slip - self.prev_slip) / dt;
            self.prev_slip = slip;
            let short = self.target_speed - v;
            let most = clamp_lo_hi(0.03 * v + 45.0 * DEG, 45.0 * DEG, 70.0 * DEG);
            let w1 = clamp_lo_hi(-0.1 * (v - 0.0), 0.0, 1.0);
            let c1 = clamp_lo_hi(wrap_pi(path_yaw - travel) * w1, -35.0 * DEG, 35.0 * DEG);
            let w2 = clamp_lo_hi(0.05 * v, 0.0, 2.0);
            let c2 = clamp_lo_hi(wrap_pi(aim - travel) * w2, -45.0 * DEG, 45.0 * DEG);
            let goal = clamp_lo_hi(c1 + c2, -most, most);
            self.slip_goal = goal;
            let err = wrap_pi(slip - goal);
            let q = err * 1.5 + rate * -0.02;
            self.steer_to(-q, dt);
            // The handbrake while the slip is more than 20° short of the
            // goal, the steering not past 15° the other way, above 20 mph.
            let e = if goal < 0.0 { -err } else { err };
            let qs = if err > 0.0 { -q } else { q };
            let f1 = clamp_lo_hi((-20.0 * DEG - e) * 2.864_789, 0.0, 1.0);
            let f2 = clamp_lo_hi((15.0 * DEG - qs) * 5.729_578, 0.0, 1.0);
            let f3 = clamp_lo_hi((v - 8.940_545) * 0.11185, 0.0, 1.0);
            self.handbrake = f3 * (f2 * f1);
            if !(self.target_speed >= 2.0) && !self.hold {
                self.brake = 1.0;
                self.gas = 0.0;
            } else {
                let g1 = clamp_lo_hi((short + 2.235_136_3) * 0.11185, 0.0, 1.0);
                let g2 = clamp_lo_hi(v * 0.11185, 0.0, 1.0);
                self.gas = clamp_lo_hi(g1 - g2 * (e - 20.0 * DEG) * 0.9, 0.0, 1.0);
                self.brake = 0.0;
            }
            self.gear = -1;
        } else {
            // Slow or backing up: the pedals by the speed short of the
            // target (±1), straight at the aim (reversed backing up).
            let a = self.target_speed.abs();
            if !(a >= 2.0) {
                self.brake = 1.0;
                self.gas = 0.0;
            } else {
                let x = (a - v).clamp(-1.0, 1.0);
                if x >= 0.0 {
                    self.brake = 0.0;
                    self.gas = x;
                } else {
                    self.brake = -x;
                    self.gas = 0.0;
                }
            }
            self.handbrake = 0.0;
            self.gear = if away { 0 } else { -1 };
            let s = wrap_pi(aim - heading);
            self.steer_to(if away { -s } else { s }, dt);
        }
        AiControls {
            steer_degrees: self.steer as i16 as f32 * (360.0 / 65536.0),
            gas: self.gas,
            brake: self.brake,
            handbrake: self.handbrake,
            gear: self.gear,
            nitrous: false,
            mover_held: false,
        }
    }
}
