//! The game's in-race camera (the mover the camera factory 0x5F1FE0 builds
//! for type 1: constructor 0x451660, per-frame update 0x453570).
//!
//! Each car carries its own camera positions ([`CameraView`], record
//! +0x720). The game places the eye and the look-at point in a frame that
//! rides on the car, then lets every coordinate ease towards its target with
//! a cubic ([`Ease`]). For the chase views (far, near, high) that frame turns
//! with the car's direction of travel once it is moving faster than 8 m/s,
//! which is what makes the camera swing out in a slide. The eye drops back
//! under acceleration and closes in under braking, by amounts and limits set
//! in per-view tuning tables in the executable.
//!
//! A little shake grows with speed, and a short jolt follows a hard hit
//! ([`impact`]).
//!
//! See docs/formats.md ("Chase camera") for addresses and what is inferred.

mod camera;
pub mod impact;
mod push;
mod showcase;
mod tuning;

pub use self::camera::*;
pub use self::push::*;
pub use self::showcase::*;
pub use self::tuning::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Camera;
    use glam::{Mat3, Vec3};

    /// Looking back with the player's +0x2B set (read 1 under Wine) keeps
    /// the view's own entry and mirrors the eased eye in x and y at once
    /// (0x4539B5): under Wine (2026-11-05, the 206 standing in free roam)
    /// the far chase went from 6.10 m behind the centre of mass, 1.45 m up,
    /// to 5.53 m ahead, 1.36 m up, and the bumper from 0.73 m ahead to
    /// 1.30 m behind, in the press's first frame and unchanged to the
    /// release — the mirror about the car's origin. The clone glided out
    /// to the 0x7EC400 row (17 m ahead, 5.6 m up) before.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn look_back_mirrors_the_views_own_eye_at_once() {
        let root = crate::testkit::game_root();
        let car = crate::car::Car::load(&root, "PEUGOT").unwrap();
        let state = crate::drive::CarState::new(Vec3::new(10.0, 5.0, 0.0), 0.3);
        let rot = Mat3::from_quat(state.orientation);
        for kind in [0u16, 2] {
            let mut chase = ChaseCamera::new(&car, kind);
            for _ in 0..60 {
                chase.update(&state, 1.0 / 60.0, crate::qol::fov_offset());
            }
            let ahead = rot.transpose() * (chase.update(&state, 1.0 / 60.0, crate::qol::fov_offset()).position - state.position);
            chase.set_look_back(true);
            for f in 0..120 {
                let back = rot.transpose() * (chase.update(&state, 1.0 / 60.0, crate::qol::fov_offset()).position - state.position);
                let want = Vec3::new(-ahead.x, -ahead.y, ahead.z);
                assert!(back.distance(want) < 0.02, "kind {kind} frame {f}: {back} want {want}");
            }
            assert_eq!(chase.wall_radius(), 0.7);
            chase.set_look_back(false);
            let again = rot.transpose() * (chase.update(&state, 1.0 / 60.0, crate::qol::fov_offset()).position - state.position);
            assert!(again.distance(ahead) < 0.02, "kind {kind} released: {again} want {ahead}");
        }
    }

    /// The chase direction keeps its vertical part while more than one
    /// wheel is down (0x44DE90 restamps +0x58 each such update, 0x445C20
    /// fades over 2 s of the stamp's age): a car climbing a 6 % grade for
    /// 5 s still has the frame tilted up; with every wheel off the ground
    /// for over 2 s it is level.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_chase_frame_keeps_its_pitch_while_wheels_are_down() {
        let root = crate::testkit::game_root();
        let car = crate::car::Car::load(&root, "PEUGOT").unwrap();
        let mut state = crate::drive::CarState::new(Vec3::ZERO, 0.0);
        state.velocity = Vec3::new(20.0, 0.0, 1.2);
        for w in state.wheels.iter_mut() {
            w.on_ground = true;
        }
        let mut chase = ChaseCamera::new(&car, 1);
        for _ in 0..300 {
            state.position += state.velocity / 60.0;
            chase.update(&state, 1.0 / 60.0, crate::qol::fov_offset());
        }
        assert!(chase.heading.value().z > 0.04, "grounded: {:?}", chase.heading.value());
        for w in state.wheels.iter_mut() {
            w.on_ground = false;
        }
        for _ in 0..240 {
            state.position += state.velocity / 60.0;
            chase.update(&state, 1.0 / 60.0, crate::qol::fov_offset());
        }
        assert!(chase.heading.value().z.abs() < 0.01, "airborne 4 s: {:?}", chase.heading.value());
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_parked_chase_camera_comes_back_snapped() {
        // Camera_Factory's keep flag parks the chase mover through a
        // crash / jump camera; 0x5FFDE0 brings it back with +0x52 set, so
        // its first update snaps every ease (heading included, 0x445C20):
        // under Wine view 1 faced the car's heading within 0.2° on that
        // frame after the car had turned 16–140° during the event.
        let root = crate::testkit::game_root();
        let car = crate::car::Car::load(&root, "PEUGOT").unwrap();
        let mut state = crate::drive::CarState::new(Vec3::ZERO, 0.0);
        state.velocity = Vec3::new(20.0, 0.0, 0.0);
        for w in state.wheels.iter_mut() {
            w.on_ground = true;
        }
        let mut chase = ChaseCamera::new(&car, 1);
        for _ in 0..300 {
            state.position += state.velocity / 60.0;
            chase.update(&state, 1.0 / 60.0, crate::qol::fov_offset());
        }
        // The event: 2.5 s parked, the car spun 120° and nearly stopped.
        let mut after = crate::drive::CarState::new(state.position + Vec3::new(3.0, 1.0, 0.0), 2.1);
        after.velocity = after.orientation * Vec3::new(2.0, 0.0, 0.0);
        for w in after.wheels.iter_mut() {
            w.on_ground = true;
        }
        let mut eased = chase.clone();
        chase.resume(2.5);
        let back = chase.update(&after, 1.0 / 60.0, crate::qol::fov_offset());
        let new = ChaseCamera::new(&car, 1).update(&after, 1.0 / 60.0, crate::qol::fov_offset());
        assert!((back.position - new.position).length() < 0.05, "{} vs {}", back.position, new.position);
        let yaw = |c: &Camera| c.forward().y.atan2(c.forward().x);
        assert!((yaw(&back) - 2.1).abs() < 0.01, "{}", yaw(&back));
        // Without the snap the first frame would still face the old way.
        let lagged = eased.update(&after, 1.0 / 60.0, crate::qol::fov_offset());
        assert!((yaw(&lagged) - 2.1).abs() > 0.5, "{}", yaw(&lagged));
    }

    #[test]
    fn body_box_matches_the_206_read_live() {
        // The clone's 206 hull bounds and record centre of mass.
        let hull = [Vec3::new(-1.9268273, -0.8421037, 0.19421054), Vec3::new(1.9393514, 0.84404975, 1.2464684)];
        let (mid, r) = body_box(&hull, Vec3::new(0.3, 0.0, 0.3)).unwrap();
        assert!((mid - Vec3::new(-0.2937, 0.001, 0.4203)).length() < 1e-3, "{mid}");
        let max = Vec3::new(1.6394, 0.844, 0.9465);
        assert!((r - ((max - Vec3::new(-0.2937, 0.001, 0.4203)).length() + 1.0)).abs() < 1e-3, "{r}");
    }

    use crate::ground::TrackPolygon;
    use glam::Vec2;

    #[test]
    fn a_near_eye_is_lifted_onto_the_sphere() {
        let mut eye = Camera::looking_at(Vec3::new(-2.0, 0.0, 0.5), Vec3::ZERO);
        keep_out_lifted(&mut eye, Vec3::ZERO, &[], 0.7, Some(3.0));
        assert!((eye.position.z - (9.0f32 - 4.0).sqrt()).abs() < 1e-5);
        assert_eq!(eye.position.truncate(), Vec2::new(-2.0, 0.0));
        let mut far = Camera::looking_at(Vec3::new(-5.0, 0.0, 0.5), Vec3::ZERO);
        keep_out_lifted(&mut far, Vec3::ZERO, &[], 0.7, Some(3.0));
        assert_eq!(far.position.z, 0.5);
    }

    #[test]
    fn the_chase_eye_stays_out_of_a_wall() {
        // A wall in the plane x = 0 facing +x, the car at x = 5 and the eye
        // behind the wall at x = −1.
        let corners = [
            Vec3::new(0.0, -5.0, -1.0),
            Vec3::new(0.0, 5.0, -1.0),
            Vec3::new(0.0, 5.0, 6.0),
            Vec3::new(0.0, -5.0, 6.0),
        ];
        let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize();
        let wall = TrackPolygon {
            id: (0, 0),
            corners,
            count: 4,
            normal,
            d: 0.0,
            flags: 2,
            surface: 1,
            turn: false,
            min: Vec2::new(0.0, -5.0),
            max: Vec2::new(0.0, 5.0),
        };
        let centre = Vec3::new(5.0, 0.0, 1.0);
        let mut camera = Camera::looking_at(Vec3::new(-1.0, 0.0, 1.0), centre);
        assert!(keep_out(&mut camera, centre, &[wall]));
        // Pulled in to 0.7 m short of the wall, still looking at the car.
        assert!((camera.position - Vec3::new(0.7, 0.0, 1.0)).length() < 1e-4, "{:?}", camera.position);
        assert!(camera.forward().dot((centre - camera.position).normalize()) > 0.9999);
        // An eye 0.3 m in front of it is pushed out to 0.7.
        let mut near = Camera::looking_at(Vec3::new(0.3, 0.0, 1.0), centre);
        assert!(!keep_out(&mut near, centre, &[wall]));
        let x = near.position.x.abs();
        assert!((x - 0.7).abs() < 1e-4, "{:?}", near.position);
    }
}

#[cfg(test)]
mod showcase_tests {
    use super::*;
    use crate::render::Camera;
    use glam::Vec3;

    #[test]
    fn looking_back_keeps_walls_three_metres_off() {
        use crate::ground::TrackPolygon;
        use glam::Vec2;
        // The wall x = 0 facing +x, the eye 1 m in front of it: 0.7 m
        // leaves it, the look-back radius (0x445A60: 3 m) pushes it to 3.
        let corners = [
            Vec3::new(0.0, -5.0, -1.0),
            Vec3::new(0.0, 5.0, -1.0),
            Vec3::new(0.0, 5.0, 6.0),
            Vec3::new(0.0, -5.0, 6.0),
        ];
        let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize();
        let wall = TrackPolygon {
            id: (0, 0),
            corners,
            count: 4,
            normal,
            d: 0.0,
            flags: 2,
            surface: 1,
            turn: false,
            min: Vec2::new(0.0, -5.0),
            max: Vec2::new(0.0, 5.0),
        };
        let centre = Vec3::new(8.0, 0.0, 1.0);
        let mut near = Camera::looking_at(Vec3::new(1.0, 0.0, 1.0), centre);
        keep_out_with(&mut near, centre, &[wall], WALL_RADIUS[0]);
        assert!((near.position.x - 1.0).abs() < 1e-4);
        let mut back = Camera::looking_at(Vec3::new(1.0, 0.0, 1.0), centre);
        keep_out_with(&mut back, centre, &[wall], WALL_RADIUS[1]);
        assert!((back.position.x - 3.0).abs() < 1e-4, "{:?}", back.position);
        // A floor (no wall flag) keeps the 0.7 m radius.
        let mut floor = wall;
        floor.flags = 0;
        let mut eye = Camera::looking_at(Vec3::new(1.0, 0.0, 1.0), centre);
        keep_out_with(&mut eye, centre, &[floor], WALL_RADIUS[1]);
        assert!((eye.position.x - 1.0).abs() < 1e-4);
    }

    /// A car facing +x on flat ground: yaw 0 puts the eye ahead of it,
    /// d away, raised by d·sin 3°.
    #[test]
    fn showcase_eye_in_the_ground_frame() {
        let p = [0.0, -9.0, 4.0, 1.0, 0.0, 0.5, 90.0, 0.0, 0.0];
        let c = showcase_view(&p, Vec3::new(10.0, 20.0, 1.0), glam::Quat::IDENTITY, Vec3::Z);
        let want = Vec3::new(11.0 + 4.0, 20.0, 1.5 + 4.0 * 3f32.to_radians().sin());
        assert!((c.position - want).length() < 1e-3, "{:?}", c.position);
        assert!((c.game_fov.unwrap() - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        // Turned a quarter: yaw 90 is to the car's left (+y in its frame).
        let q = glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let p = [90.0, 3.0, 4.0, 0.0, 0.0, 0.0, 45.0, 0.0, 0.0];
        let c = showcase_view(&p, Vec3::ZERO, q, Vec3::Z);
        assert!((c.position.truncate() - glam::Vec2::new(-4.0, 0.0)).length() < 1e-3, "{:?}", c.position);
    }
}

#[cfg(test)]
mod native_fix_tests {
    use super::*;
    use crate::render::Camera;
    use glam::{Mat3, Vec3};

    fn peak(f: impl Fn(f32) -> [f32; 4]) -> [f32; 4] {
        let mut m = [0.0f32; 4];
        for i in 0..4000 {
            let s = f(i as f32 * 0.01);
            for k in 0..4 {
                m[k] = m[k].max(s[k].abs());
            }
        }
        m
    }

    /// 0x452ED0's graph row is |v| · 4 / S: at 25 m/s an S of 50 gives row
    /// 2 (sway amplitude 0.3), 100 row 1 (0.1); S = 0 the last row.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_shake_row_runs_on_the_trap_speed() {
        let asphalt = [1u8; 4];
        let trap = peak(|t| camera::shake(25.0, 50.0, true, false, asphalt, t));
        let hundred = peak(|t| camera::shake(25.0, 100.0, true, false, asphalt, t));
        // The pitch channel: 0.7° × amplitude × the noise's peak.
        let ratio = trap[2] / hundred[2];
        assert!((ratio - 3.0).abs() < 0.6, "{trap:?} vs {hundred:?}");
        // Without a reached trap speed the row is the last one.
        let last = peak(|t| camera::shake(1.0, 0.0, true, false, asphalt, t));
        let top = peak(|t| camera::shake(1000.0, 50.0, true, false, asphalt, t));
        assert_eq!(last, top);
    }

    /// The rough-surface channel (0x452ED0): nothing on asphalt (row +0x3C
    /// 0), on dirt graph[1] · 4 · 0.05 · 2.5 with no ×0.2 outside a drag
    /// race; the sway part is ×0.2 there.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn rough_surfaces_add_their_own_shake() {
        let road = peak(|t| camera::shake(30.0, 60.0, false, false, [1; 4], t));
        let dirt = peak(|t| camera::shake(30.0, 60.0, false, false, [8; 4], t));
        // Row 2: sway amplitude 0.3 · 0.2 = 0.06 → pitch ≤ 0.7 · 0.06 · 2;
        // rough amplitude 0.8 · 0.2 · 2.5 = 0.4 → pitch up to 0.6 · 0.4 · 2.
        assert!(road[2] < 0.7 * 0.06 * 2.0 + 1e-6, "{road:?}");
        assert!(dirt[2] > 3.0 * road[2], "{dirt:?} vs {road:?}");
        assert!(dirt[3] > 0.8 * 0.4 * 0.5, "{dirt:?}");
    }

    /// A burning nitrous shot adds 0.2 to the sway amplitude (after the
    /// 0.2 factor) and takes its rate × 1.4 (0x452ED0, 0x59FA40 / 0x5ECDC0).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_nitrous_flame_shakes_harder() {
        let off = peak(|t| camera::shake(30.0, 60.0, false, false, [1; 4], t));
        let on = peak(|t| camera::shake(30.0, 60.0, false, true, [1; 4], t));
        // 0.06 → 0.26.
        assert!(on[2] > 3.5 * off[2], "{on:?} vs {off:?}");
    }

    fn traffic(origin: Vec3, velocity: Vec3) -> LiftCar {
        LiftCar {
            origin,
            velocity,
            rotation: Mat3::IDENTITY,
            min: Vec3::new(-2.0, -0.9, -0.3),
            max: Vec3::new(2.0, 0.9, 1.0),
            middle: Vec3::new(0.0, 0.0, 0.35),
        }
    }

    /// 0x44B040 / 0x44AC10: an eye inside a traffic car's sphere and z-band
    /// is lifted onto the sphere's top over that car's origin; the view
    /// turns to keep the camera's car on screen.
    #[test]
    fn the_chase_eye_is_lifted_over_traffic() {
        let own = Vec3::new(6.0, 0.0, 0.0);
        let car = traffic(Vec3::new(0.5, 0.0, 0.0), Vec3::ZERO);
        let r = (car.max - car.middle).length() + 1.0;
        let mut cam = Camera::looking_at(Vec3::new(0.0, 0.0, 1.0), own);
        assert!(lift_over_traffic(&mut cam, own, Vec3::ZERO, &[car], false));
        let want = (r * r + 0.1 - 0.25).sqrt();
        assert!((cam.position.z - want).abs() < 1e-4, "{} want {want}", cam.position.z);
        assert_eq!(cam.position.truncate(), glam::Vec2::new(0.0, 0.0));
        assert!(cam.forward().dot((own - cam.position).normalize()) > 0.999);
        // Above the band (z max + 0.5) or 12 m away: nothing.
        let mut high = Camera::looking_at(Vec3::new(0.0, 0.0, 1.6), own);
        assert!(!lift_over_traffic(&mut high, own, Vec3::ZERO, &[car], false));
        let far = traffic(Vec3::new(13.0, 0.0, 0.0), Vec3::ZERO);
        assert!(car_holding_eye(Vec3::new(0.0, 0.0, 1.0), own, Vec3::ZERO, &[far], false).is_none());
    }

    /// The motion skip: a car moving over 15 m/s relative to the camera's
    /// car, neither ahead of the eye along that motion nor ahead of the
    /// car, is passed over (unless in flow state 6).
    #[test]
    fn fast_passing_traffic_is_not_lifted_over() {
        let own = Vec3::new(6.0, 0.0, 0.0);
        let eye = Vec3::new(0.0, 0.0, 1.0);
        // Own car at 20 m/s along +y, the traffic car still beside the eye.
        let car = traffic(Vec3::new(0.5, 0.0, 0.0), Vec3::ZERO);
        let v = Vec3::new(0.0, 20.0, 0.0);
        assert!(car_holding_eye(eye, own, v, &[car], false).is_none());
        assert!(car_holding_eye(eye, own, v, &[car], true).is_some());
        // The camera's car closing on it (own-to-car along the motion): kept.
        assert!(car_holding_eye(eye, own, Vec3::new(-20.0, 0.0, 0.0), &[car], false).is_some());
    }
}

