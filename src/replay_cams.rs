//! Extra replay cameras (docs/modern-extras.md §3; clone deviation,
//! opt-in: Enhancements → Replay Cameras "Extended"). The native replay
//! viewer's six camera indices (0 automatic: the track cameras and the
//! drawn chase views, 1–5 the chase views 0x802130 = bumper, hood, near,
//! far, high; docs/formats.md "Replays") stay as they are and first; the C
//! key then goes on through these before it wraps to the automatic one.
//! Nothing of this is in SPEED2.EXE.
//!
//! The bumper view asked for is native (chase view 0, one of the
//! six); the extras are the trackside cameras placed along the race's
//! road, a helicopter, a wheel camera and an orbit.

use glam::{Mat4, Vec2, Vec3};

use crate::formats::roads::RoadRoute;
use crate::render::camera::Camera;

/// The extra cameras, in the order C reaches them after the native six.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extra {
    /// A camera on a pole beside the road, the car coming toward it and
    /// passing; the next one placed ahead when the car is past.
    Trackside,
    /// High behind and to the side, slowly circling.
    Heli,
    /// On the body beside the front left wheel, looking ahead.
    Wheel,
    /// Circling the car at its height.
    Orbit,
}

pub const EXTRAS: [Extra; 4] = [Extra::Trackside, Extra::Heli, Extra::Wheel, Extra::Orbit];

impl Extra {
    pub fn name(self) -> &'static str {
        match self {
            Extra::Trackside => "Trackside",
            Extra::Heli => "Helicopter",
            Extra::Wheel => "Wheel",
            Extra::Orbit => "Orbit",
        }
    }
}

/// The trackside cameras' spacing along the road (m), their offset from
/// the road's line (m), height over the car (m), and how far past one the
/// car goes before the next is taken (m).
pub const STATION_SPACING: f32 = 60.0;
pub const STATION_SIDE: f32 = 9.0;
pub const STATION_HEIGHT: f32 = 2.2;
pub const STATION_PASSED: f32 = 22.0;

/// What a car's frame gives the cameras.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub transform: Mat4,
    pub velocity: Vec3,
}

impl Target {
    pub fn position(&self) -> Vec3 {
        self.transform.w_axis.truncate()
    }
    /// The car's forward (car space +X, `CarState::forward`).
    pub fn forward(&self) -> Vec3 {
        self.transform.transform_vector3(Vec3::X).normalize_or(Vec3::X)
    }
}

/// The extra cameras' state (the trackside station held).
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The station's distance along the road and its side (±1).
    pub station: Option<(f32, f32)>,
    count: u32,
}

/// A trackside camera's field of view: the car about 9 m across the
/// frame's height wherever it is (zooming in as it goes away), 10°–50°.
pub fn zoom(distance: f32) -> f32 {
    (2.0 * (4.5 / distance.max(1.0)).atan()).clamp(10f32.to_radians(), 50f32.to_radians())
}

impl State {
    /// The camera `extra` shows for `car` at replay time `t` (s; the orbit
    /// and the helicopter turn with it, so a paused replay holds them),
    /// `along` the car's distance along `road`.
    pub fn camera(&mut self, extra: Extra, car: &Target, road: Option<&RoadRoute>, along: f32, t: f32) -> Camera {
        let at = car.position() + Vec3::Z * 0.6;
        match extra {
            Extra::Trackside => {
                let Some(road) = road.filter(|r| r.length > 1.0) else {
                    return self.camera(Extra::Orbit, car, None, along, t);
                };
                // The car's direction along the road.
                let dir2 = road.direction(along);
                let fwd = car.forward().truncate();
                let travel = if car.velocity.truncate().length() > 1.0 { car.velocity.truncate() } else { fwd };
                let sign = if travel.dot(dir2) >= 0.0 { 1.0 } else { -1.0 };
                // Past it by STATION_PASSED, or far from it (a skip, a reset).
                let passed = |s: f32| {
                    let past = road.difference(along, s) * sign;
                    past > STATION_PASSED || past < -STATION_SPACING * 2.5
                };
                if self.station.is_none_or(|(s, _)| passed(s)) {
                    self.count += 1;
                    let side = if self.count % 2 == 0 { 1.0 } else { -1.0 };
                    self.station = Some((along + sign * STATION_SPACING, side));
                }
                let (s, side) = self.station.unwrap();
                let p = road.point_at(s);
                let d = road.direction(s);
                let normal = Vec2::new(-d.y, d.x);
                let base = p + normal * side * STATION_SIDE;
                let eye = Vec3::new(base.x, base.y, car.position().z + STATION_HEIGHT);
                let mut c = Camera::look_at(eye, at, Vec3::Z, zoom(eye.distance(at)));
                c.near = 0.3;
                c
            }
            Extra::Heli => {
                let back = -car.forward();
                let turn = Mat4::from_rotation_z(0.6 + 0.12 * t);
                let off = turn.transform_vector3(Vec3::new(back.x, back.y, 0.0).normalize_or(Vec3::NEG_X)) * 22.0;
                let eye = car.position() + off + Vec3::Z * 16.0;
                Camera::look_at(eye, at, Vec3::Z, 38f32.to_radians())
            }
            Extra::Wheel => {
                // Car space: x forward, y left, z up (`CarState`).
                let eye = car.transform.transform_point3(Vec3::new(1.15, 1.02, 0.32));
                let ahead = car.transform.transform_point3(Vec3::new(12.0, 0.35, 0.2));
                let up = car.transform.transform_vector3(Vec3::Z);
                let mut c = Camera::look_at(eye, ahead, up, 70f32.to_radians());
                c.near = 0.05;
                c
            }
            Extra::Orbit => {
                let a = 0.35 * t;
                let eye = car.position() + Vec3::new(a.cos() * 7.0, a.sin() * 7.0, 1.8);
                let mut c = Camera::look_at(eye, at, Vec3::Z, 50f32.to_radians());
                c.near = 0.2;
                c
            }
        }
    }
}

/// Smooth slow motion's speeds (Extended): the comma key steps 1 → ½ → ¼
/// → 1. (The native slow motion holds three render frames of four,
/// 0x581060 with 0x865148 & 3: ¼ speed at 15 updates a second.)
pub const SLOW_STEPS: [f32; 3] = [1.0, 0.5, 0.25];

pub fn next_speed(speed: f32) -> f32 {
    let i = SLOW_STEPS.iter().position(|&s| (s - speed).abs() < 1e-3).unwrap_or(0);
    SLOW_STEPS[(i + 1) % SLOW_STEPS.len()]
}

/// The drawn fraction between the last two recorded frames under smooth
/// slow motion: the replay's time since the last frame (`carry`, plus the
/// cadence's `alpha` of an update `update` s long at `speed`) over the
/// next frame's `dt`.
pub fn slow_alpha(carry: f32, alpha: f32, update: f32, speed: f32, dt: f32) -> f32 {
    ((carry + alpha * update * speed) / dt.max(1e-6)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(at: Vec3, heading: f32, speed: f32) -> Target {
        let q = glam::Quat::from_rotation_z(heading);
        Target { transform: Mat4::from_rotation_translation(q, at), velocity: q * Vec3::X * speed }
    }

    fn looks_at(c: &Camera, p: Vec3) -> f32 {
        c.forward().dot((p - c.position).normalize())
    }

    #[test]
    fn every_extra_looks_at_the_car() {
        let road = RoadRoute::new((0..50).map(|i| Vec2::new(i as f32 * 20.0, 0.0)).collect(), false);
        let mut s = State::default();
        for (i, e) in EXTRAS.into_iter().enumerate() {
            let t = car(Vec3::new(300.0, 2.0, 10.0), 0.0, 30.0);
            let c = s.camera(e, &t, Some(&road), 300.0, i as f32);
            if e == Extra::Wheel {
                // Beside the wheel, ahead along the car.
                assert!(c.forward().x > 0.95, "{c:?}");
                assert!((c.position - t.position()).length() < 2.0);
            } else {
                assert!(looks_at(&c, t.position() + Vec3::Z * 0.6) > 0.999, "{e:?}");
            }
        }
    }

    #[test]
    fn trackside_stations_hold_then_move_ahead() {
        let road = RoadRoute::new((0..100).map(|i| Vec2::new(i as f32 * 20.0, 0.0)).collect(), false);
        let mut s = State::default();
        let mut stations = Vec::new();
        let mut along = 100.0;
        while along < 600.0 {
            let t = car(Vec3::new(along, 0.0, 5.0), 0.0, 40.0);
            let c = s.camera(Extra::Trackside, &t, Some(&road), along, 0.0);
            // On a pole beside the road, not on it.
            assert!((c.position.y.abs() - STATION_SIDE).abs() < 1e-3);
            let st = s.station.unwrap().0;
            if stations.last() != Some(&st) {
                stations.push(st);
            }
            along += 40.0 / 60.0;
        }
        // A station every ~(spacing + passed) m, each ahead when taken.
        assert!(stations.len() >= 5 && stations.len() <= 8, "{stations:?}");
        assert!(stations.windows(2).all(|w| w[1] > w[0]));
        // Driven the other way, the stations come from the other side.
        let mut s = State::default();
        let t = car(Vec3::new(500.0, 0.0, 5.0), std::f32::consts::PI, 40.0);
        s.camera(Extra::Trackside, &t, Some(&road), 500.0, 0.0);
        assert!(s.station.unwrap().0 < 500.0);
        assert!(zoom(100.0) < zoom(10.0));
    }

    #[test]
    fn slow_motion_steps_and_fraction() {
        assert_eq!(next_speed(1.0), 0.5);
        assert_eq!(next_speed(0.5), 0.25);
        assert_eq!(next_speed(0.25), 1.0);
        // Half speed: half an update after a frame at carry 0, a quarter of
        // the next frame.
        assert!((slow_alpha(0.0, 0.5, 1.0 / 60.0, 0.5, 1.0 / 60.0) - 0.25).abs() < 1e-6);
        assert_eq!(slow_alpha(0.02, 1.0, 1.0 / 60.0, 1.0, 1.0 / 60.0), 1.0);
    }
}
