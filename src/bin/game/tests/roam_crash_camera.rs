//! The crash camera in free roam (docs/native-fixes/crash-camera.tsv):
//! the original runs its camera director's events with race status 4 in
//! free roam, so a hard hit on traffic starts the crash camera there as in
//! a race. Values recorded under Wine (career free roam after the airport,
//! a head-on hit on a traffic car).

use crate::*;

/// The player's car +0x60 and its forward axis when kind 6 started (the
/// frame of the hit), +0x70 at that frame's start (before the hit; after
/// it the car ran backwards at (1.314, 6.171, −1.072)), and the
/// director's place (+0x270) and second place (+0x290) the original chose.
const CAR: [f32; 3] = [1652.168, -667.480, 38.230];
const FWD: [f32; 2] = [-0.2077, -0.9782];
const VEL: [f32; 3] = [-3.667, -17.263, -0.599];
const SPOT: [f32; 3] = [1644.49, -735.48, 37.28];
const SECOND: [f32; 2] = [1628.81, -686.52];

/// A career's free roam with the player's car put where the original's
/// stood when its crash camera started.
fn roam_at_the_hit(jump_camera: bool) -> Game {
    let mut game = Game::new().unwrap();
    game.settings.jump_camera = jump_camera;
    let mut profile = Profile::new("RoamCrash");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    d.state.orientation = glam::Quat::from_rotation_z(FWD[1].atan2(FWD[0]));
    let com = d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
    d.state.position = Vec3::from(CAR) - com;
    d.state.velocity = Vec3::from(VEL);
    d.state.publish_forward_speed();
    game
}

/// Car_Crash's outcome for the player: crash mode and the camera test.
fn crash(game: &mut Game) {
    let d = game.sim.drive.as_mut().unwrap();
    d.state.crash = true;
    d.state.crash_clear = None;
    d.state.crash_hit = true;
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_free_roam_crash_camera_is_placed_as_the_originals() {
    let game = roam_at_the_hit(true);
    let d = game.sim.drive.as_ref().unwrap();
    let routes = game.sim.encounters.as_ref().map(|e| &e.routes);
    let (nodes, far) = crash_route(d, routes).expect("the free-roam route places the camera");
    let centre = Vec3::from(CAR);
    let shot = nfsu2::event_camera::plan_shot(
        &nodes,
        far,
        centre,
        Vec3::from(VEL),
        &|p| d.world.grid_height(p),
        &|a, b| d.sight_blocked(a, b),
    )
    .unwrap();
    eprintln!("roam crash shot: eye {:?} second {:?} side {} visible {}", shot.eye, shot.second, shot.side, shot.visible);
    assert!(shot.eye.truncate().distance(Vec3::from(SPOT).truncate()) < 0.05, "eye {:?}", shot.eye);
    assert!((shot.eye.z - SPOT[2]).abs() < 0.05, "eye {:?}", shot.eye);
    assert!(shot.second.truncate().distance(glam::Vec2::from(SECOND)) < 0.05, "second {:?}", shot.second);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_hard_hit_in_free_roam_starts_the_crash_camera() {
    let mut game = roam_at_the_hit(true);
    crash(&mut game);
    game.advance_for(1.0 / 60.0).unwrap();
    let c = game.sim.roam_camera.as_ref().unwrap();
    // Observed: kind 6, the slow motion 0.4 for 8 s, mover 0x0F, field
    // of view 20°, the HUD gone.
    assert_eq!(c.events.kind, 6);
    assert_eq!(c.events.slow.rate, 0.4);
    assert!(c.events.slow.remaining > 7.9);
    let d = game.sim.drive.as_ref().unwrap();
    if c.shot.as_ref().is_some_and(|s| s.visible) {
        assert_eq!(native_mover_type(&game.mode, Some(d)), 0x0F);
        let fov = d.camera.game_fov.unwrap().to_degrees();
        assert!((fov - 20.0).abs() < 0.05, "{fov}");
    }
    assert!(d.state.crash_camera);
    // A second hit while it runs starts nothing new; once the car is down
    // on four wheels more than 0.5 s of simulation after the hit it ends
    // (observed: 0.5 s of simulation, 1.24 s of real time at 0.4).
    for _ in 0..240 {
        game.advance_for(1.0 / 60.0).unwrap();
        if game.sim.roam_camera.as_ref().unwrap().events.kind == 0 {
            break;
        }
    }
    let c = game.sim.roam_camera.as_ref().unwrap();
    assert_eq!(c.events.kind, 0, "the crash camera ends");
    assert_eq!(c.events.slow.rate, 1.0);
    assert_eq!(native_mover_type(&game.mode, game.sim.drive.as_ref()), 1);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn no_free_roam_crash_camera_with_the_jump_camera_off() {
    let mut game = roam_at_the_hit(false);
    crash(&mut game);
    game.advance_for(1.0 / 60.0).unwrap();
    let c = game.sim.roam_camera.as_ref().unwrap();
    assert_eq!(c.events.kind, 0);
    assert_eq!(c.events.slow.rate, 1.0);
}

/// MARKER_GPS_AID under the crash camera: 0x5F3CC0 draws it only while
/// the player's camera mover is the chase camera (player +0x238 → +0xC =
/// 1); the crash / jump mover (0x44BEA0, type 7) clears its kept point
/// (+0x114 = 0), so the arrow is gone until the chase view is back
/// (observed under Wine: a head-on traffic hit with the GPS set, no arrow
/// in the crash camera, back after it).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_gps_arrow_is_hidden_under_the_free_roam_crash_camera() {
    let mut game = roam_at_the_hit(true);
    let lot = game.game_data.career.shops.iter().find(|s| s.name == "CC_CAR_LOT_1").unwrap().trigger;
    let t = game.sim.route_triggers.find(lot).unwrap();
    game.career_world.gps = Some(dday::Gps::engage(0, &t, game.car_pose()));
    for _ in 0..600 {
        if game.gps_aid_target().is_some() {
            break;
        }
        game.advance_for(1.0 / 60.0).unwrap();
    }
    assert!(game.gps_aid_target().is_some(), "the GPS arrow shows in the chase view");
    crash(&mut game);
    game.advance_for(1.0 / 60.0).unwrap();
    assert_eq!(game.sim.roam_camera.as_ref().unwrap().events.kind, 6);
    assert!(game.sim.roam_camera.as_ref().unwrap().shot.as_ref().is_some_and(|s| s.visible), "the crash camera is placed");
    let mut frames = 0;
    while game.sim.roam_camera.as_ref().unwrap().events.kind != 0 && frames < 240 {
        assert_eq!(game.gps_aid_target(), None, "no GPS arrow under the crash camera (frame {frames})");
        game.advance_for(1.0 / 60.0).unwrap();
        frames += 1;
    }
    assert_eq!(game.sim.roam_camera.as_ref().unwrap().events.kind, 0, "the crash camera ends");
    game.advance_for(1.0 / 60.0).unwrap();
    assert!(game.gps_aid_target().is_some(), "the GPS arrow is back in the chase view");
}
