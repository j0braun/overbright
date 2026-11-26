//! A wall drawn in the map editor stops a car at top speed (issue: added scenery gets the original's
//! collision): the same run with and without the wall, at 30 / 60 / 120 / 200 Hz. Without it the car
//! drives through the spot; with it the car never gets past the wall's plane.

use crate::*;
use nfsu2::tools::map::draw::DrawnFace;
use nfsu2::world::DrawnCollision;

/// Where the car stands (free roam, a road in Bayview), its heading and the speed it hits at (m/s, a
/// stock car's top speed is about 70).
const CAR: [f32; 3] = [1652.168, -667.480, 38.230];
const FWD: [f32; 2] = [-0.2077, -0.9782];
const SPEED: f32 = 70.0;
/// How far ahead the wall stands (m) and how big it is.
const AHEAD: f32 = 40.0;
const HALF_WIDTH: f32 = 30.0;
const HEIGHT: f32 = 8.0;

/// The furthest the car's centre got along its heading, relative to its start, over `seconds` at `hz`.
fn furthest(hz: u32, wall: bool, seconds: u32) -> f32 {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("WallTopSpeed");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let fwd = Vec3::new(FWD[0], FWD[1], 0.0).normalize();
    {
        let d = game.sim.drive.as_mut().unwrap();
        d.state.orientation = glam::Quat::from_rotation_z(FWD[1].atan2(FWD[0]));
        let com = d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
        d.state.position = Vec3::from(CAR) - com;
        d.state.velocity = fwd * SPEED;
        d.state.publish_forward_speed();
        if wall {
            // One wall face across the road, its outward normal towards the car (as the editor draws a long side).
            let side = Vec3::new(-fwd.y, fwd.x, 0.0) * HALF_WIDTH;
            let mid = Vec3::from(CAR) + fwd * AHEAD;
            let (a, b) = (mid - side - Vec3::Z * 5.0, mid + side - Vec3::Z * 5.0);
            let up = Vec3::Z * (HEIGHT + 5.0);
            let face = DrawnFace { corners: [b, a, a + up, b + up], quad: true, wall: true, surface: 0 };
            let done = d.world.apply_map_collision(&[], &DrawnCollision { remove: Vec::new(), faces: vec![face] });
            assert!(done.added > 0, "the wall was written into a pack: {done:?}");
        }
    }
    let start = Vec3::from(CAR);
    let mut best = f32::MIN;
    for _ in 0..seconds * hz {
        game.advance(1.0 / hz as f32).unwrap();
        let d = game.sim.drive.as_ref().unwrap();
        best = best.max((d.state.position - start).dot(fwd));
    }
    best
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_car_at_top_speed_stays_out_of_an_added_wall_at_every_frame_rate() {
    for hz in [30u32, 60, 120, 200] {
        let free = furthest(hz, false, 3);
        assert!(free > AHEAD + 20.0, "{hz} Hz: without the wall the car passes the spot ({free:.1} m)");
        let walled = furthest(hz, true, 3);
        assert!(walled < AHEAD, "{hz} Hz: the car reached {walled:.1} m, the wall stands at {AHEAD} m");
    }
}
