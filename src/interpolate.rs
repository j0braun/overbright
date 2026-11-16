//! What is drawn between two fixed world updates (`frame_clock::Cadence`,
//! docs/uncapped-fps.md): a car's pose and the camera, blended from the
//! tick snapshot before the last update to the one after it. Drawing
//! only: the game computes the blend into the frame's own copies (docs/uncapped-fps.md "Tick snapshots",
//! `FramePoses`); the world's state is never written for a draw.
use glam::{Quat, Vec3};
use crate::drive::CarState;
use crate::render::camera::Camera;
use std::f32::consts::{PI, TAU};

/// The drawn part of a car's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarPose {
    pub position: Vec3,
    pub orientation: Quat,
    pub steer_degrees: f32,
    pub compression: [f32; 4],
    pub roll: [f32; 4],
    pub spin: [f32; 4],
    pub spinner: [f32; 4],
    pub spinner_rate: [f32; 4],
    pub mount: [f32; 4],
}

/// A blend further than this (m) is a teleport or another car: not blended.
const JUMP: f32 = 30.0;

impl CarPose {
    pub fn of(s: &CarState) -> CarPose {
        CarPose {
            position: s.position,
            orientation: s.orientation,
            steer_degrees: s.steer_degrees,
            compression: std::array::from_fn(|i| s.wheels[i].compression),
            roll: std::array::from_fn(|i| s.wheels[i].roll),
            spin: std::array::from_fn(|i| s.wheels[i].spin),
            spinner: std::array::from_fn(|i| s.wheels[i].spinner),
            spinner_rate: std::array::from_fn(|i| s.wheels[i].spinner_rate),
            mount: s.hydraulics.mount,
        }
    }

    pub fn apply(&self, s: &mut CarState) {
        s.position = self.position;
        s.orientation = self.orientation;
        s.steer_degrees = self.steer_degrees;
        for i in 0..4 {
            s.wheels[i].compression = self.compression[i];
            s.wheels[i].roll = self.roll[i];
            s.wheels[i].spinner = self.spinner[i];
        }
        s.hydraulics.mount = self.mount;
    }

    /// `a` (before the update) towards `b` (after it, `dt` long) by `t`;
    /// None when the car jumped.
    pub fn blend(a: &CarPose, b: &CarPose, t: f32, dt: f32) -> Option<CarPose> {
        if a.position.distance(b.position) > JUMP {
            return None;
        }
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        // A wheel angle wraps at a turn (% TAU in the roll update):
        // take the turn the spin made, not the shortest way.
        let angle = |x: f32, y: f32, rate: f32| {
            let mut d = y - x;
            d += TAU * ((rate * dt - d) / TAU).round();
            x + d * t
        };
        Some(CarPose {
            position: a.position.lerp(b.position, t),
            orientation: a.orientation.slerp(b.orientation, t),
            steer_degrees: lerp(a.steer_degrees, b.steer_degrees),
            compression: std::array::from_fn(|i| lerp(a.compression[i], b.compression[i])),
            roll: std::array::from_fn(|i| angle(a.roll[i], b.roll[i], b.spin[i])),
            spin: b.spin,
            spinner: std::array::from_fn(|i| angle(a.spinner[i], b.spinner[i], b.spinner_rate[i])),
            spinner_rate: b.spinner_rate,
            mount: std::array::from_fn(|i| lerp(a.mount[i], b.mount[i])),
        })
    }
}

/// A car's pose drawn `t` of the way from the tick snapshot
/// before (`a`) to the newest (`b`, `dt` long): exactly `a` at `t ≤ 0`,
/// exactly `b` at `t ≥ 1` (bit for bit: a frame at α = 0 is the older
/// tick's picture, one at α = 1 the newer's), [`CarPose::blend`] between.
/// None when the car jumped (a teleport or another car: drawn at `b`).
pub fn pose_at(a: &CarPose, b: &CarPose, t: f32, dt: f32) -> Option<CarPose> {
    if a.position.distance(b.position) > JUMP {
        return None;
    }
    if t <= 0.0 {
        Some(*a)
    } else if t >= 1.0 {
        Some(*b)
    } else {
        CarPose::blend(a, b, t, dt)
    }
}

/// The camera drawn `t` of the way from the tick snapshot before
/// (`a`) to the newest (`b`): exactly `a` at `t ≤ 0`, exactly `b` at
/// `t ≥ 1`, [`blend_camera`] between. None after a cut (as `blend_camera`).
pub fn camera_at(a: &Camera, b: &Camera, t: f32) -> Option<Camera> {
    if a.position.distance(b.position) > JUMP || a.game_fov.is_some() != b.game_fov.is_some() {
        return None;
    }
    if t <= 0.0 {
        Some(*a)
    } else if t >= 1.0 {
        Some(*b)
    } else {
        blend_camera(a, b, t)
    }
}

/// The camera between two updates; None after a cut (a jump of more than
/// [`JUMP`] or another kind of camera).
pub fn blend_camera(a: &Camera, b: &Camera, t: f32) -> Option<Camera> {
    if a.position.distance(b.position) > JUMP || a.game_fov.is_some() != b.game_fov.is_some() {
        return None;
    }
    let wrap = |d: f32| (d + PI).rem_euclid(TAU) - PI;
    let mut c = b.clone();
    c.position = a.position.lerp(b.position, t);
    c.yaw = a.yaw + wrap(b.yaw - a.yaw) * t;
    c.pitch = a.pitch + (b.pitch - a.pitch) * t;
    c.fov_y = a.fov_y + (b.fov_y - a.fov_y) * t;
    c.game_fov = a.game_fov.zip(b.game_fov).map(|(x, y)| x + (y - x) * t);
    c.up = a.up.lerp(b.up, t).normalize_or(b.up);
    Some(c)
}

/// A placement's transform (a world animation's piece, a
/// knocked-over prop) drawn `t` of the way from the tick before (`a`) to
/// the newest (`b`): exactly `a` at `t ≤ 0`, exactly `b` at `t ≥ 1`;
/// between, scale and translation blended and the rotation slerped
/// (decomposed: an element-wise blend would shrink a turning prop). None
/// after a jump of more than [`JUMP`] (a prop put back, a loop's restart):
/// drawn at `b`, as the cars.
pub fn transform_at(a: &glam::Mat4, b: &glam::Mat4, t: f32) -> Option<glam::Mat4> {
    if a.w_axis.truncate().distance(b.w_axis.truncate()) > JUMP {
        return None;
    }
    if !(t > 0.0) {
        return Some(*a);
    }
    if t >= 1.0 || a == b {
        return Some(*b);
    }
    let (sa, ra, ta) = a.to_scale_rotation_translation();
    let (sb, rb, tb) = b.to_scale_rotation_translation();
    Some(glam::Mat4::from_scale_rotation_translation(sa.lerp(sb, t), ra.slerp(rb, t), ta.lerp(tb, t)))
}

#[cfg(test)]
mod transform_at_tests {
    use super::*;

    /// The ends are the ticks' transforms bit for bit; halfway a prop
    /// turned 40° is at 20°, at the middle place, at its size; a jump is
    /// a cut.
    #[test]
    fn placements_blend_between_ticks() {
        let a = glam::Mat4::from_scale_rotation_translation(Vec3::splat(1.5), Quat::from_rotation_z(0.1), Vec3::new(1.0, 2.0, 3.0));
        let b = glam::Mat4::from_scale_rotation_translation(Vec3::splat(1.5), Quat::from_rotation_z(0.1 + 40f32.to_radians()), Vec3::new(3.0, 2.0, 3.0));
        assert_eq!(transform_at(&a, &b, 0.0).unwrap().to_cols_array().map(f32::to_bits), a.to_cols_array().map(f32::to_bits));
        assert_eq!(transform_at(&a, &b, 1.0).unwrap().to_cols_array().map(f32::to_bits), b.to_cols_array().map(f32::to_bits));
        let m = transform_at(&a, &b, 0.5).unwrap();
        let (s, r, tr) = m.to_scale_rotation_translation();
        assert!((s - Vec3::splat(1.5)).abs().max_element() < 1e-5);
        assert!((tr - Vec3::new(2.0, 2.0, 3.0)).abs().max_element() < 1e-5);
        assert!(r.angle_between(Quat::from_rotation_z(0.1 + 20f32.to_radians())) < 1e-4);
        let far = glam::Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0));
        assert!(transform_at(&a, &far, 0.5).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_are_the_states_and_wheels_turn_the_spins_way() {
        let mut s = CarState::new(Vec3::new(1.0, 2.0, 3.0), 0.3);
        let a = CarPose::of(&s);
        s.position += Vec3::X;
        s.wheels[0].spin = 60.0; // 1 rad per 1/60 s
        s.wheels[0].roll = (a.roll[0] + 1.0 + TAU) % TAU - TAU; // wrapped
        let b = CarPose::of(&s);
        let dt = 1.0 / 60.0;
        let at0 = CarPose::blend(&a, &b, 0.0, dt).unwrap();
        let at1 = CarPose::blend(&a, &b, 1.0, dt).unwrap();
        assert_eq!(at0.position, a.position);
        assert_eq!(at1.position, b.position);
        let half = CarPose::blend(&a, &b, 0.5, dt).unwrap();
        assert!((half.roll[0] - (a.roll[0] + 0.5)).abs() < 1e-4, "{} vs {}", half.roll[0], a.roll[0]);
        s.position += Vec3::X * 100.0;
        assert!(CarPose::blend(&a, &CarPose::of(&s), 0.5, dt).is_none());
    }

    /// The frame's lerp returns the snapshots bit for bit at its
    /// ends (α = 0: the older tick, α = 1: the newer), stays between them
    /// in between, and a jump is a cut at every α.
    #[test]
    fn pose_and_camera_ends_are_the_snapshots_exactly() {
        let mut s = CarState::new(Vec3::new(0.1, 0.2, 0.3), 0.7);
        s.wheels[1].compression = 0.03;
        let a = CarPose::of(&s);
        s.position += Vec3::new(0.33, -0.17, 0.011);
        s.orientation = (s.orientation * Quat::from_rotation_z(0.05)).normalize();
        s.steer_degrees = 7.3;
        s.wheels[1].compression = 0.071;
        let b = CarPose::of(&s);
        let dt = 1.0 / 60.0;
        assert_eq!(pose_at(&a, &b, 0.0, dt), Some(a));
        assert_eq!(pose_at(&a, &b, 1.0, dt), Some(b));
        assert_eq!(pose_at(&a, &b, 1.5, dt), Some(b));
        for t in [0.1f32, 0.25, 0.5, 0.75, 0.9] {
            let m = pose_at(&a, &b, t, dt).unwrap();
            for k in 0..3 {
                let (lo, hi) = (a.position[k].min(b.position[k]), a.position[k].max(b.position[k]));
                assert!(m.position[k] >= lo && m.position[k] <= hi, "t {t} axis {k}");
            }
            assert!(m.steer_degrees >= a.steer_degrees && m.steer_degrees <= b.steer_degrees);
        }
        let far = CarPose { position: a.position + Vec3::X * 40.0, ..b };
        for t in [0.0, 0.5, 1.0] {
            assert_eq!(pose_at(&a, &far, t, dt), None, "a jump is a cut at {t}");
        }
        let mut ca = Camera::new(Vec3::new(1.0, 2.0, 3.0), 0.4, 0.1);
        ca.up = Vec3::new(0.01, 0.0, 1.0).normalize();
        let mut cb = Camera::new(Vec3::new(1.3, 2.2, 3.05), 0.45, 0.12);
        cb.near = 0.7;
        let bits = |c: &Camera| {
            [c.position.x, c.position.y, c.position.z, c.yaw, c.pitch, c.fov_y, c.up.x, c.up.y, c.up.z, c.near, c.far]
                .map(f32::to_bits)
        };
        assert_eq!(bits(&camera_at(&ca, &cb, 0.0).unwrap()), bits(&ca));
        assert_eq!(bits(&camera_at(&ca, &cb, 1.0).unwrap()), bits(&cb));
        let mid = camera_at(&ca, &cb, 0.5).unwrap();
        assert!(mid.yaw > ca.yaw && mid.yaw < cb.yaw);
        let jumped = Camera::new(Vec3::new(100.0, 2.0, 3.0), 0.4, 0.1);
        assert!(camera_at(&ca, &jumped, 0.0).is_none() && camera_at(&ca, &jumped, 1.0).is_none());
    }

    #[test]
    fn camera_turns_the_short_way() {
        let a = Camera::new(Vec3::ZERO, PI - 0.1, 0.0);
        let b = Camera::new(Vec3::X, -PI + 0.1, 0.0);
        let c = blend_camera(&a, &b, 0.5).unwrap();
        assert!((c.yaw.abs() - PI).abs() < 1e-4, "{}", c.yaw);
        assert_eq!(c.position, Vec3::new(0.5, 0.0, 0.0));
    }
}
