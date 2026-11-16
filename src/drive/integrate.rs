use super::*;

impl CarState {
    /// 5FBBF0: snapshot once after controller/driver decisions, before the
    /// outer world physics interval. Do not call this per physics substep.
    #[must_use]
    pub fn begin_world_update(&self, dt: f32) -> WorldVelocitySample {
        WorldVelocitySample { velocity: self.velocity, dt }
    }

    /// 604750: publish once after all substeps and contacts. Retained live
    /// world FXSAVE records use PC24/nearest; f32 operations preserve that
    /// significand rounding (extended-exponent extremes remain unverified).
    pub fn finish_world_update(&mut self, sample: WorldVelocitySample) {
        let (before, dt) = (sample.velocity, sample.dt);
        if !(dt > 0.0) {
            return;
        }
        let reciprocal = 1.0f32 / dt;
        self.world_acceleration = (self.velocity - before) * reciprocal;
    }

    /// 5EC3B0 uses the full three-component direction, Z+Y then X, with
    /// each operation rounded to the observed PC24 significand precision.
    pub fn acceleration_along(&self, direction: Vec3) -> f32 {
        let a = self.world_acceleration;
        (a.z * direction.z + a.y * direction.y) + a.x * direction.x
    }

    pub fn forward_acceleration(&self) -> f32 {
        self.acceleration_along(self.forward())
    }

    /// Other-car road entry 60BE40 -> 5EC340: below 0.1 speed return
    /// acceleration magnitude; otherwise project onto full 3D velocity.
    /// Keep the observed PC24 operation order separate from own-forward
    /// acceleration. Extended-exponent extremes remain unverified.
    pub fn velocity_acceleration(&self) -> f32 {
        let length = |v: Vec3| ((v.x * v.x + v.y * v.y) + v.z * v.z).sqrt();
        let a = self.world_acceleration;
        let speed = length(self.velocity);
        if speed < f32::from_bits(0x3dcccccd) {
            length(a)
        } else {
            // 43CF10 recomputes the same PC24 length, then multiplies by
            // its reciprocal. Finite entry speeds here cannot be zero.
            let direction = self.velocity * (1.0 / speed);
            (direction.y * a.y + direction.z * a.z) + direction.x * a.x
        }
    }

    /// The rigid body's inertia choice for the frame (0x5A49B0): ground
    /// outside crash mode with a wheel down; leaving it divides L.z by
    /// record +0x270 (entering keeps L, so the yaw rate drops).
    pub(super) fn update_inertia(&mut self, h: &Handling) {
        let ground = !self.crash && self.wheels_down() > 0;
        if self.ground_inertia && !ground && h.unknown_270 != 0.0 {
            self.angular_momentum.z /= h.unknown_270;
        }
        self.ground_inertia = ground;
    }

    /// The rigid body's rows (glam columns = native rows +0x30/+0x40/+0x50):
    /// the exact rows the last sub-step left while `orientation` still
    /// holds the value they were published as, else the quaternion's.
    pub(crate) fn body_rows(&self) -> Mat3 {
        match self.body_rows {
            Some((q, rows)) if q == self.orientation => rows,
            _ => Mat3::from_quat(self.orientation),
        }
    }

    /// Publish orthonormal rows (0x5BB0E0's result) as the orientation.
    pub(crate) fn set_body_rows(&mut self, rows: Mat3) {
        let q = Quat::from_mat3(&rows).normalize();
        self.orientation = q;
        self.body_rows = Some((q, rows));
    }

    /// Hold the car where it stands (0x599210(1), the dyno pull's reset
    /// 0x56D350 at 0x56D415): the centre of mass is saved
    /// (mover +0x90, the point +0xE0 at it, its velocity +0xF0 zero from
    /// 0x86B390) and mover +0x84 set, which turns the aero factor to 0, the
    /// yaw-torque limit to 1e−14 and the top-speed limit off (0x5A49B0),
    /// takes all grip from the undriven wheels at the gas exactly 1
    /// (0x5A4D90) and runs the position spring each sub-step
    /// ([`CarState::hold_spring`]). Only a fresh mover clears it.
    pub fn set_hold(&mut self, car: &Car) {
        let com = self.position + self.body_rows() * Vec3::from(car.car_type.handling.centre_of_mass);
        self.aero_off = true;
        self.hold = Some(super::state::Hold { target: com, point: com, velocity: Vec3::ZERO, last_force: Vec3::ZERO });
    }

    /// The held mover's position spring (0x5A5C30, the last call of the
    /// force pass 0x5B0530 while mover +0x84 is set, so before the next
    /// integration): the point takes the last integrated force when it
    /// pushes along the velocity (+0x130 · v > 0, else 0x86B390's zero),
    /// plus 150 × (saved − point) and −15 × its velocity, at 1/m × h; it
    /// moves by its velocity × h, keeps the body's height, and the body's
    /// centre of mass is put at it in x and y. The undriven wheels (by the
    /// rear share: under 0.2 the rear, over 0.8 the front) then spin with
    /// it: forward · its velocity / radius, 0 under 0.03 m/s. Returns the
    /// new centre of mass.
    pub(super) fn hold_spring(&mut self, h: &Handling, table: &[crate::car::Wheel; 4], com: Vec3, rows: Mat3, sub: f32) -> Vec3 {
        let Some(hold) = self.hold.as_mut() else { return com };
        let push = if hold.last_force.dot(self.velocity) > 0.0 { hold.last_force } else { Vec3::ZERO };
        let f = push + (hold.target - hold.point) * 150.0 + hold.velocity * -15.0;
        hold.velocity += f * ((1.0 / h.mass) * sub);
        hold.point += hold.velocity * sub;
        hold.point.z = com.z;
        let moved = Vec3::new(hold.point.x, hold.point.y, com.z);
        self.position += moved - com;
        let split = h.gearbox.rear_split;
        let along = rows.x_axis.dot(hold.velocity);
        let along = if along.abs() < 0.03 { 0.0 } else { along };
        for (i, w) in self.wheels.iter_mut().enumerate() {
            let undriven = if split < 0.2 { i >= 2 } else { split > 0.8 && i < 2 };
            if undriven {
                w.spin = along / table[i].radius;
            }
        }
        moved
    }

    /// Set a scripted angular velocity and its matching world-space momentum.
    /// Pose placement must not leave the previous physics momentum behind.
    pub fn set_scripted_spin(&mut self, h: &Handling, spin: Vec3) {
        let rot = Mat3::from_quat(self.orientation);
        self.spin = spin;
        self.angular_momentum = rot * ((rot.transpose() * spin) * inertia_of(h, self.ground_inertia));
    }

    /// The tidy-up's ω (0x59C600) from the rows as they stand.
    pub(super) fn spin_from_momentum(&mut self, h: &Handling) {
        let rows = self.body_rows();
        self.spin = spin_of(h, self.ground_inertia, rows, self.angular_momentum);
    }

    /// What a knock-over prop's contact needs from the car at a world
    /// `point`: the point's velocity, the arm from the centre of mass, and
    /// the car's share of the impulse denominator along `normal`
    /// (1/m′ + n·((0.4·I⁻¹(r × n)) × r), as in 0x5AF0F0).
    pub fn prop_contact_terms(&self, car: &Car, point: Vec3, normal: Vec3) -> (Vec3, Vec3, f32) {
        let h = &car.car_type.handling;
        let rot = Mat3::from_quat(self.orientation);
        let arm = point - (self.position + rot * Vec3::from(h.centre_of_mass));
        let inertia = inertia_of(h, self.ground_inertia);
        // Body-space, applied to world vectors (0x593A50 / 0x5AF0F0).
        let inv = |v: Vec3| (v / inertia) * if self.crash { 1.0 } else { 0.4 };
        let k = (1.0 / h.mass).min(1.0 / 1.85) + normal.dot(inv(arm.cross(normal)).cross(arm));
        (self.velocity + self.spin.cross(arm), arm, k)
    }

    /// Take a prop's push at `arm` from the centre of mass (0x5A0B10).
    pub fn take_push(&mut self, car: &Car, impulse: Vec3, arm: Vec3) {
        self.take(&car.car_type.handling, impulse, arm);
    }

    /// Take an impulse at `arm` from the centre of mass (0x5A0B10).
    pub(super) fn take(&mut self, h: &Handling, impulse: Vec3, arm: Vec3) {
        let (linear, angular) = contact::apply(h.mass, self.crash, impulse, arm);
        self.velocity += linear / h.mass;
        self.angular_momentum += angular;
        self.spin_from_momentum(h);
    }
}

/// 0x59C600's ω = R·I⁻¹·Rᵀ·L for the inertia the body uses (+0x1B0 on the
/// ground, +0x170 otherwise, by +0x158), in the original's operation order
/// ([`super::rigid::spin_from_rows`]), held to 250 rad/s.
pub(super) fn spin_of(h: &Handling, ground: bool, rows: Mat3, l: Vec3) -> Vec3 {
    let inertia = inertia_of(h, ground);
    let inv = Vec3::new(1.0 / inertia.x, 1.0 / inertia.y, 1.0 / inertia.z);
    super::rigid::spin_from_rows(rows, inv, l)
}
