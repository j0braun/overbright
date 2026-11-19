//! The replay director's track cameras (docs/formats.md, Replays, "Track
//! cameras"): the route cameras of a race's track, chosen by the car's road
//! record (0x44F9A0) and followed along the record by the car's fraction
//! (0x44FE80), moved by the track camera mover 0x12 (0x453F50).
//!
//! The route groups are ported with the visibility test (0x448280,
//! [`visible`], its car test 0x5F1500 [`eye_in_car`]) and the look-point
//! push of entries with +7 (0x453F50 → 0x444530, the caller's); the native
//! director frame with the car's own road record is
//! [`TrackDirector::direct_on`] (docs/native-fixes/N-CAM.tsv). The
//! scripted scene (0x4324D0), the generic groups, the blend, the shake and
//! the spline lens curve (0x574880 on the mover's +0x38 curve) are open.

mod path;
mod route_director;
mod typed_director;

pub use self::path::*;
pub use self::route_director::*;
pub use self::typed_director::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ai_routes::RouteRecord;
    use crate::formats::camera_tracks::CameraEntry;
    use crate::formats::camera_tracks::CameraTrack;
    use glam::{Mat4, Vec3};

    fn entry(start: f32, eye: [[f32; 3]; 2], mover: u8) -> CameraEntry {
        CameraEntry {
            mover,
            blend: false,
            spline: true,
            eye_space: 1,
            look_space: 1,
            avoid: [false; 2],
            frozen_frame: false,
            shake: false,
            start,
            tangent: [1.0, 1.0],
            eye,
            look: [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            fov: [50.0, 50.0],
            params: [[0.0; 2]; 4],
            tail: [0; 0x18],
        }
    }

    fn track(entries: Vec<CameraEntry>) -> CameraTrack {
        CameraTrack { name: "Track 0".into(), duration: 1.0, entries }
    }

    #[test]
    fn scene_points_follow_the_ground() {
        // 0x4427C0: z += ground under the point − ground under the origin;
        // no step without ground under the point.
        let scene = Mat4::from_translation(Vec3::new(100.0, 0.0, 5.0));
        let ground = |p: Vec3| (p.x < 110.0).then_some(p.x * 0.1);
        let p = to_world(Vec3::new(5.0, 0.0, 1.5), 3, Mat4::IDENTITY, Some(scene), Some(&ground));
        assert!((p.z - (6.5 + 10.5 - 10.0)).abs() < 1e-4);
        let far = to_world(Vec3::new(20.0, 0.0, 1.5), 3, Mat4::IDENTITY, Some(scene), Some(&ground));
        assert!((far.z - 6.5).abs() < 1e-4);
        assert!((to_world(Vec3::new(5.0, 0.0, 1.5), 3, Mat4::IDENTITY, Some(scene), None).z - 6.5).abs() < 1e-4);
    }

    #[test]
    fn the_visibility_test() {
        // An entry 10 m behind and 2 m up, looking at the car and moving
        // forward along with it; the car at the origin facing +x.
        let mut e = entry(0.0, [[-10.0, 0.0, 2.0], [-5.0, 0.0, 2.0]], 0x12);
        e.look = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let car = Mat4::IDENTITY;
        let clear = &mut |_: Vec3, _: Vec3| false;
        // The view from the side (e·d = 0): passes.
        let side = View { eye: Vec3::new(0.0, -8.0, 1.0), forward: Vec3::Y, fov: 1.2, velocity: Vec3::ZERO };
        assert!(visible(&e, car, &side, clear));
        // Something in the way of the car-to-eye segment fails it.
        assert!(!visible(&e, car, &side, &mut |a: Vec3, b: Vec3| a == Vec3::ZERO && b == Vec3::new(-10.0, 0.0, 2.0)));
        // Facing it (e·d ≤ −0.5) fails.
        let front = View { eye: Vec3::new(10.0, 0.0, 2.0), forward: -Vec3::X, fov: 1.2, velocity: Vec3::ZERO };
        assert!(!visible(&e, car, &front, clear));
        // The same line from the same distance and lens: too alike.
        let same = View {
            eye: Vec3::new(-10.0, 0.0, 2.0),
            forward: Vec3::new(10.0, 0.0, -2.0).normalize(),
            fov: lens_angle(50.0),
            velocity: Vec3::X,
        };
        assert!(!visible(&e, car, &same, clear));
        // Three times as far: unlike enough, and the motion test reads the
        // velocity as a point through the inverse frame (+x here).
        let far = View { eye: Vec3::new(-30.0, 0.0, 6.0), ..same };
        assert!(visible(&e, car, &far, clear));
        let back = View { velocity: -Vec3::X, ..far };
        assert!(!visible(&e, car, &back, clear));
        // The other side of the car (e.y · d.y < 0).
        let mut l = e.clone();
        l.look = [[0.0, 1.0, 0.0]; 2];
        let r = View { eye: Vec3::new(0.0, 8.0, 1.0), forward: -Vec3::Y, fov: 1.2, velocity: Vec3::ZERO };
        assert!(!visible(&l, car, &r, clear));
    }

    #[test]
    fn paths_hit_their_end_points() {
        let p = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
        let m = [Vec3::new(10.0, 0.0, 0.0); 2];
        assert!(path(p, m, true, 0.0).distance(Vec3::ZERO) < 1e-5);
        assert!(path(p, m, true, 1.0).distance(Vec3::new(10.0, 0.0, 0.0)) < 1e-5);
        // A straight lerp is the midpoint; a spline with the end-to-end
        // tangents is too, on a straight run.
        assert!(path(p, m, false, 0.5).distance(Vec3::new(5.0, 0.0, 0.0)) < 1e-5);
        assert!(path(p, m, true, 0.5).distance(Vec3::new(5.0, 0.0, 0.0)) < 1e-5);
    }

    #[test]
    fn spaces_follow_the_car_frame() {
        let car =
            Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0)) * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let p = Vec3::new(1.0, 0.0, 0.0);
        assert!(to_world(p, 0, car, None, None).distance(Vec3::new(100.0, 1.0, 0.0)) < 1e-4);
        assert!(to_world(p, 1, car, None, None).distance(p) < 1e-6);
        assert!(to_world(p, 2, car, None, None).distance(Vec3::new(101.0, 0.0, 0.0)) < 1e-6);
    }

    #[test]
    fn entries_and_run_times() {
        let t = track(vec![
            entry(0.0, [[0.0; 3], [1.0, 0.0, 0.0]], 0x12),
            entry(0.5, [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], 0x12),
        ]);
        assert_eq!(entry_index(&t, 0.25), Some(0));
        assert_eq!(entry_index(&t, 0.5), Some(1));
        assert_eq!(entry_index(&t, 1.5), None);
        // The entries chain (the first ends where the second starts): the
        // run goes to the end of the track.
        assert!(chains(&t.entries[0], &t.entries[1]) == false);
        // 1000 m of road, 100 m/s, from 0.5: 0.5 × 1000 / 100 = 5 s.
        assert!((run_time(&t, 1, 0.5, 1000.0, 100.0) - 5.0).abs() < 1e-3);
        assert_eq!(run_time(&t, 1, 0.5, 1000.0, 0.0), 1000.0);
    }

    #[test]
    fn the_record_fraction_wraps_with_the_road() {
        let r = RouteRecord {
            id: 7,
            name: String::new(),
            kind: 0,
            coord_range: (900.0, 100.0),
            bounds: [0.0; 4],
            flags: 1 << 6,
            fade: [false; 2],
            lanes: [false; 10],
            points: Vec::new(),
        };
        // A record across the start line: 900 → 1000 → 100, 200 m on a 1000 m road.
        assert_eq!(section_fraction(&r, 950.0, 1000.0), Some(0.25));
        assert_eq!(section_fraction(&r, 50.0, 1000.0), Some(0.75));
        assert_eq!(section_fraction(&r, 500.0, 1000.0), None);
    }
}

#[cfg(test)]
mod typed_tests {
    use super::*;
    use crate::opponents::GameRng;
    use glam::{Mat4, Vec3};

    /// The weights as 0x441AF0..0x441F3A compute them (docs/notes/parity/replay-cameras.md).
    #[test]
    fn typed_weights_follow_the_games_functions() {
        let base = TypedInput { forward_speed: 30.0, speed: 30.0, top: 60.0, ..Default::default() };
        // Speed: 0.3 + 0.7 × 30 / 60, nothing at 10 m/s or under, at most 1;
        // no set-up speed reads 60.
        assert!((typed_weight(TYPED_SPEED, &base) - 0.65).abs() < 1e-6);
        assert_eq!(typed_weight(TYPED_SPEED, &TypedInput { forward_speed: 10.0, ..base }), 0.0);
        assert_eq!(typed_weight(TYPED_SPEED, &TypedInput { forward_speed: 90.0, ..base }), 1.0);
        assert!((typed_weight(TYPED_SPEED, &TypedInput { top: 0.0, ..base }) - 0.65).abs() < 1e-6);
        // Corner: 20 × |1 / radius| (a 40 m turn: 0.5), over 5 m/s; flagged
        // (mirrored) for a negative curvature.
        let corner = TypedInput { curvature: Some(-1.0 / 40.0), ..base };
        assert!((typed_weight(TYPED_CORNER, &corner) - 0.5).abs() < 1e-6 && typed_flag(TYPED_CORNER, &corner));
        assert_eq!(typed_weight(TYPED_CORNER, &TypedInput { forward_speed: 5.0, ..corner }), 0.0);
        // PowerSlide: (|slip| − 5°) / 40° + 0.1 over 4 m/s.
        let slide = TypedInput { slip: 25f32.to_radians(), ..base };
        assert!((typed_weight(TYPED_POWERSLIDE, &slide) - 0.6).abs() < 1e-3 && !typed_flag(TYPED_POWERSLIDE, &slide));
        assert_eq!(typed_weight(TYPED_POWERSLIDE, &TypedInput { slip: 4f32.to_radians(), ..base }), 0.0);
        // NOS while a shot runs, Jump upside down.
        assert_eq!(typed_weight(TYPED_NOS, &TypedInput { nitrous: Some(1.0), ..base }), 0.75);
        assert_eq!(typed_weight(TYPED_NOS, &base), 0.0);
        assert_eq!(typed_weight(TYPED_JUMP, &TypedInput { upside_down: true, ..base }), 5.0);
        assert_eq!(typed_weight(TYPED_BURNOUT, &base), 0.0);
        // Burnout: the wheels spinning (sum 15 m/s: 0.5) under 20 m/s; a
        // standing car over 3000 rpm adds 11; over 20 m/s or under −10
        // nothing.
        let burn = TypedInput { forward_speed: 5.0, wheel_slip: 15.0, rpm: 6000.0, ..base };
        assert!((typed_weight(TYPED_BURNOUT, &burn) - 0.5).abs() < 1e-6);
        let stand = TypedInput { forward_speed: 0.5, ..burn };
        assert!((typed_weight(TYPED_BURNOUT, &stand) - 11.5).abs() < 1e-3);
        assert!((typed_weight(TYPED_BURNOUT, &TypedInput { rpm: 3000.0, ..stand }) - 0.5).abs() < 1e-6);
        assert_eq!(typed_weight(TYPED_BURNOUT, &TypedInput { forward_speed: 20.5, ..burn }), 0.0);
        assert_eq!(typed_weight(TYPED_BURNOUT, &TypedInput { forward_speed: -10.5, ..burn }), 0.0);
        assert_eq!(typed_weight(TYPED_BURNOUT, &TypedInput { wheel_slip: 9.0, ..burn }), 0.0);
    }

    /// InGameCommon.bun's typed groups are the six the game held in memory
    /// under Wine (the camera manager +0x14: key, track count), and a typed
    /// shot runs its track's 3 s, then needs a new draw.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn typed_groups_load_as_the_game_holds_them() {
        let root = crate::testkit::game_root();
        let data = std::fs::read(root.join("GLOBAL/InGameCommon.bun")).unwrap();
        let groups = crate::formats::bundle::chunks(&data, 0)
            .find(|c| c.id == crate::formats::camera_tracks::TYPED_CAMERAS)
            .and_then(|c| crate::formats::camera_tracks::parse_camera_tracks(c).ok())
            .unwrap();
        let got: Vec<(u32, usize)> = groups.iter().map(|g| (g.key, g.tracks.len())).collect();
        assert_eq!(
            got,
            [
                (TYPED_JUMP, 4),
                (TYPED_NOS, 1),
                (TYPED_SPEED, 36),
                (TYPED_CORNER, 7),
                (TYPED_BURNOUT, 6),
                (TYPED_POWERSLIDE, 6)
            ]
        );
        let speed = groups.iter().position(|g| g.key == TYPED_SPEED).unwrap();
        assert!(groups[speed].tracks.iter().all(|t| t.duration == 3.0));
        // A Speed shot drawn at 30 m/s, nothing in the way.
        let car = Mat4::IDENTITY;
        let view =
            View { eye: Vec3::new(-6.0, 0.0, 1.4), forward: Vec3::X, fov: 1.2, velocity: Vec3::new(30.0, 0.0, 0.0) };
        let input = TypedInput { forward_speed: 30.0, speed: 30.0, top: 60.0, ..Default::default() };
        let mut rng = GameRng::default();
        let mut drawn = 0;
        for _ in 0..50 {
            if let Some(shot) = draw_typed(&groups, &input, &[], car, &view, &mut rng, &mut |_, _| false, 10.0) {
                assert_eq!(shot.group, speed);
                assert!(follow_typed(&groups, &shot, 12.9, car).is_some());
                assert!(follow_typed(&groups, &shot, 13.1, car).is_none());
                drawn += 1;
            }
        }
        assert!(drawn > 0);
        // The last three drawn are left out.
        let all: Vec<(usize, usize)> = (0..groups[speed].tracks.len()).map(|t| (speed, t)).collect();
        assert!(draw_typed(&groups, &input, &all, car, &view, &mut rng, &mut |_, _| false, 10.0).is_none());
    }
}

#[cfg(test)]
mod native_fix_tests {
    use super::*;
    use crate::formats::ai_routes::RouteRecord;
    use crate::formats::camera_tracks::{CameraEntry, CameraGroup, CameraKind, CameraTrack};
    use crate::opponents::GameRng;
    use glam::{Mat3, Mat4, Vec3};

    fn entry(start: f32, eye: [[f32; 3]; 2], mover: u8) -> CameraEntry {
        CameraEntry {
            mover,
            blend: false,
            spline: true,
            eye_space: 1,
            look_space: 1,
            avoid: [false; 2],
            frozen_frame: false,
            shake: false,
            start,
            tangent: [1.0, 1.0],
            eye,
            look: [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            fov: [50.0, 50.0],
            params: [[0.0; 2]; 4],
            tail: [0; 0x18],
        }
    }

    fn record(lo: f32, hi: f32) -> RouteRecord {
        RouteRecord {
            id: 7,
            name: String::new(),
            kind: 0,
            coord_range: (lo, hi),
            bounds: [0.0; 4],
            flags: 1 << 6,
            fade: [false; 2],
            lanes: [false; 10],
            points: Vec::new(),
        }
    }

    /// 0x5D4550: inside the record the same as before, outside a number
    /// (not none), and 0 for an empty span.
    #[test]
    fn the_record_fraction_is_a_number_outside_the_record() {
        let r = record(900.0, 100.0);
        assert!((record_fraction(&r, 950.0, 1000.0) - 0.25).abs() < 1e-5);
        assert!((record_fraction(&r, 50.0, 1000.0) - 0.75).abs() < 1e-5);
        // 500 is 400 m past the record's start, re-centred on the half lap:
        // (500 + 500 − 1000·0) … = 2.0 of the 200 m span after the start.
        let f = record_fraction(&r, 500.0, 1000.0);
        assert!((f - 3.0).abs() < 1e-4 || (f + 2.0).abs() < 1e-4, "{f}");
        assert_eq!(record_fraction(&record(10.0, 10.0005), 10.0, 1000.0), 0.0);
    }

    /// 0x44F9A0 takes the car's own record and its direction bit only.
    #[test]
    fn the_route_group_is_the_cars_own_record() {
        let g = |key: u32| CameraGroup { kind: CameraKind::Route, key, tracks: Vec::new() };
        let groups = [g(7 | 1 << 16), g(7)];
        let mut r = record(0.0, 100.0);
        assert_eq!(route_group_of(&groups, Some(&r), true, 1000.0, 50.0), Some((1, 0.5)));
        r.flags |= 2;
        assert_eq!(route_group_of(&groups, Some(&r), true, 1000.0, 50.0), Some((0, 0.5)));
        // No race coordinates, no record: none.
        assert_eq!(route_group_of(&groups, Some(&r), false, 1000.0, 50.0), None);
        assert_eq!(route_group_of(&groups, None, true, 1000.0, 50.0), None);
        // Outside the record the fraction is still given.
        assert!(route_group_of(&groups, Some(&r), true, 1000.0, 150.0).is_some());
    }

    /// 0x447910 in the candidate loop: entry 0 before the first start, the
    /// last past 1.
    #[test]
    fn the_candidate_entry_clamps_to_the_track() {
        let t = CameraTrack {
            name: String::new(),
            duration: 1.0,
            entries: vec![entry(0.1, [[0.0; 3]; 2], 0x12), entry(0.5, [[0.0; 3]; 2], 0x12)],
        };
        assert_eq!(entry_at(&t, -0.5), Some(0));
        assert_eq!(entry_at(&t, 0.05), Some(0));
        assert_eq!(entry_at(&t, 0.7), Some(1));
        assert_eq!(entry_at(&t, 1.5), Some(1));
        assert_eq!(entry_index(&t, 1.5), None);
    }

    /// 0x44A160 / 0x4434F0: the type, spaces, tangent scales and lenses
    /// must match exactly; the points within 1e-5 inclusive.
    #[test]
    fn the_chain_test_is_the_games() {
        let a = entry(0.0, [[0.0; 3], [1.0, 0.0, 0.0]], 0x12);
        let mut b = entry(0.5, [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], 0x12);
        b.look[0] = a.look[1];
        assert!(chains(&a, &b));
        let mut c = b.clone();
        c.mover = 0x13;
        assert!(!chains(&a, &c));
        let mut c = b.clone();
        c.tangent[0] = 0.9;
        assert!(!chains(&a, &c));
        let mut c = b.clone();
        c.eye_space = 0;
        assert!(!chains(&a, &c));
        let mut c = b.clone();
        c.fov[0] = 50.000_004;
        assert!(!chains(&a, &c));
        let mut c = b.clone();
        c.mover = 0;
        assert!(!chains(&a, &c));
    }

    /// 0x450040: at a chained joint both entries take the blended tangent.
    #[test]
    fn chained_entries_share_a_blended_tangent() {
        let a = entry(0.0, [[0.0; 3], [10.0, 0.0, 0.0]], 0x12);
        let mut b = entry(0.25, [[10.0, 0.0, 0.0], [10.0, 30.0, 0.0]], 0x12);
        b.look[0] = a.look[1];
        let t = CameraTrack { name: String::new(), duration: 1.0, entries: vec![a, b] };
        let (ea, _, _) = end_tangents(&t, 0);
        let (eb, _, _) = end_tangents(&t, 1);
        // a's end: dt = 0.25, dn = 0.75: w = 0.25, B = 0.75, A = 0.25 · 0.25 / 0.75.
        let want = Vec3::new(0.0, 30.0, 0.0) * (0.25 * 0.25 / 0.75) + Vec3::new(10.0, 0.0, 0.0) * 0.75;
        assert!(ea[1].distance(want) < 1e-4, "{} want {want}", ea[1]);
        // a's start has no neighbour: (p1 − p0) · s.
        assert!(ea[0].distance(Vec3::new(10.0, 0.0, 0.0)) < 1e-5);
        // b's start: dt = 0.75, dn = 0.25: w = 0.75, B = 0.25, A = 0.75 · 0.75 / 0.25.
        let want = Vec3::new(10.0, 0.0, 0.0) * (0.75 * 0.75 / 0.25) + Vec3::new(0.0, 30.0, 0.0) * 0.25;
        assert!(eb[0].distance(want) < 1e-4, "{} want {want}", eb[0]);
    }

    /// 0x5F1500: a point within √5.5 m of another car and inside its box
    /// grown by 0.5 fails the visibility test.
    #[test]
    fn a_camera_inside_another_car_is_not_visible() {
        let c = crate::chase::LiftCar {
            origin: Vec3::new(5.0, 0.0, 0.0),
            velocity: Vec3::ZERO,
            rotation: Mat3::IDENTITY,
            min: Vec3::new(-2.0, -0.9, -0.3),
            max: Vec3::new(2.0, 0.9, 1.0),
            middle: Vec3::ZERO,
        };
        assert!(eye_in_car(Vec3::new(5.0, 1.3, 0.5), &[c]));
        assert!(!eye_in_car(Vec3::new(5.0, 1.5, 0.5), &[c]));
        // Inside the box but past √5.5 m of the origin.
        assert!(!eye_in_car(Vec3::new(7.4, 0.0, 0.5), &[c]));
    }

    /// 0x5FB060: a held shot whose entry turns type 1 is dropped with no
    /// draw (the generator untouched); a draw with +0x224 clear first steps
    /// the generator once (0x5FB2EA), with +0x224 set it does not.
    #[test]
    fn the_director_drops_a_type_one_entry_and_steps_the_generator() {
        let mut e0 = entry(0.0, [[-10.0, 0.0, 2.0], [-10.0, 5.0, 2.0]], 0x12);
        e0.look = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let mut e1 = entry(0.5, [[-10.0, 5.0, 2.0], [-10.0, 9.0, 2.0]], 1);
        e1.look = [[0.0, 5.0, 0.0], [0.0, 5.0, 0.0]];
        let track = CameraTrack { name: String::new(), duration: 1.0, entries: vec![e0, e1] };
        let groups = [CameraGroup { kind: CameraKind::Route, key: 7, tracks: vec![track] }];
        let r = record(0.0, 1000.0);
        let view = View { eye: Vec3::new(0.0, -8.0, 1.0), forward: Vec3::Y, fov: 1.2, velocity: Vec3::ZERO };
        let car = Mat4::IDENTITY;
        let mut d = TrackDirector::default();
        let mut rng = GameRng::seeded(1);
        let clear = &mut |_: Vec3, _: Vec3| false;
        let cam = d.direct_on(&groups, Some(&r), true, 1000.0, 100.0, car, 50.0, &mut rng, &view, true, false, clear);
        assert!(cam.is_some() && d.shot.is_some());
        // One step (1.0) and one pick (count 1).
        let mut want = GameRng::seeded(1);
        want.unit(1.0);
        want.below(1);
        assert_eq!(rng.0, want.0);
        // Held, still type 0x12: no draw.
        let before = rng.0;
        assert!(d.direct_on(&groups, Some(&r), true, 1000.0, 200.0, car, 50.0, &mut rng, &view, true, false, clear).is_some());
        assert_eq!(rng.0, before);
        // The entry turns type 1: dropped, nothing drawn, generator untouched.
        assert!(d.direct_on(&groups, Some(&r), true, 1000.0, 600.0, car, 50.0, &mut rng, &view, true, false, clear).is_none());
        assert!(d.shot.is_none());
        assert_eq!(rng.0, before);
        // A draw with +0x224 set: no step; the only candidate's entry is
        // type 1, so it is not kept.
        let mut fresh = GameRng::seeded(1);
        assert!(d.direct_on(&groups, Some(&r), true, 1000.0, 600.0, car, 50.0, &mut fresh, &view, true, true, clear).is_none());
        let mut want = GameRng::seeded(1);
        want.below(1);
        assert_eq!(fresh.0, want.0);
        assert!(d.shot.is_none());
    }
}
