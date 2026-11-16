use super::*;

/// TrafficMover's top speed (0x7845A4, 55 mph): its speed is held within
/// 0..this (0x408820).
pub const MOVER_TOP_SPEED: f32 = 24.5865;

/// TrafficMover's acceleration and braking, m/s² per unit pedal, over
/// speed 0..`MOVER_TOP_SPEED` in four points (0x7EA03C, 0x7EA050).
pub(super) const MOVER_ACCEL: [f32; 4] = [3.576_218, 2.682_163_7, 2.011_622_7, 1.341_081_9];

pub(super) const MOVER_BRAKE: [f32; 4] = [6.705_409, 13.410_818, 20.116_228, 26.821_636];

/// 0x5764E0: a table over 0..`MOVER_TOP_SPEED` (step 1 / 0.12201818),
/// linear between points, the first below 0 and the last past the end.
pub(super) fn mover_table(t: &[f32; 4], v: f32) -> f32 {
    let f = v * 0.122_018_18;
    if f < 0.0 {
        return t[0];
    }
    let i = f as usize;
    if i >= t.len() - 1 {
        return t[t.len() - 1];
    }
    let u = f - i as f32;
    u * t[i + 1] + (1.0 - u) * t[i]
}

/// The game's simple mover for traffic (TrafficMover, 0x24 bytes, made by
/// the mode switch 0x5F46B0 for mode 2, the mode 0x5F4410 gives a car
/// whose +0x24 is 4): a kinematic bicycle on the plane, no forces. A car
/// takes it when placed (0x42C950 → 0x5F46B0(2)). A car contact hands it
/// to mode 3, the MeloMvr (traffic controller slot +0x38, 0x408ED0, when
/// car +0x5CC is set); the clone stands the full physics in for that.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrafficMover {
    /// x, y (mover +0x0C, from car +0x60).
    pub position: Vec2,
    /// Radians (+0x14, from the car's u16 heading +0x40C).
    pub heading: f32,
    /// m/s (+0x18, from car +0x430).
    pub speed: f32,
}

impl TrafficMover {
    /// One frame (0x408820). The front-wheel angle `steer` (radians, the
    /// controller's u16 +0x1E0, positive left) turns the car about the
    /// point `wheelbase / steer` to the left of its rear axle (`rear`, the
    /// rear wheels' x: car type +0x1B0) by travel / radius, unless it is
    /// under wheelbase × 0.01 (wheelbase = front − rear wheel x), when the
    /// car goes straight; the travel is the speed before this frame's
    /// pedals. Then the speed gains gas × the acceleration table and loses
    /// brake × the braking table, both at the old speed, held within
    /// 0..`MOVER_TOP_SPEED`.
    pub fn step(&mut self, rear: f32, wheelbase: f32, steer: f32, gas: f32, brake: f32, dt: f32) {
        let d = dt * self.speed;
        let (sin, cos) = self.heading.sin_cos();
        let forward = Vec2::new(cos, sin);
        let left = Vec2::new(-sin, cos);
        let mut steer = steer.rem_euclid(std::f32::consts::TAU);
        if steer > 3.141_59 {
            steer -= 6.283_18;
        }
        if wheelbase * 0.01 <= steer || steer <= -(wheelbase * 0.01) {
            let radius = wheelbase / steer;
            let pivot = self.position + forward * rear + left * radius;
            let turn = d / radius;
            let (s, c) = turn.sin_cos();
            let r = self.position - pivot;
            self.position = pivot + Vec2::new(r.x * c - r.y * s, r.y * c + r.x * s);
            self.heading += turn;
        } else {
            self.position += forward * d;
        }
        let v = self.speed;
        let v = mover_table(&MOVER_ACCEL, v) * gas * dt - mover_table(&MOVER_BRAKE, v) * brake * dt + v;
        self.speed = if v < 0.0 { 0.0 } else { v.min(MOVER_TOP_SPEED) };
    }
}
