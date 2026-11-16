//! The game-side halves of the random-number and camera native fixes (
//! docs/native-fixes/GS1.tsv). Each test states the native rule and its
//! address.
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// 0x5B9920, 0x5BA4F4: the front end's one cube keeps the
/// last platform's faces; a platform without faces zeroes only each face's
/// first 1024 bytes (DEFAULTALPHA's size); before any platform there is no
/// cube to keep (the renderer's black start cube stays).
#[test]
fn the_fe_cube_keeps_the_last_platforms_faces() {
    let mut cube = None;
    assert!(fe_cube_copy(&mut cube, None).is_none());
    let faces = (32u32, vec![5u8; 32 * 32 * 4 * 6]);
    assert_eq!(fe_cube_copy(&mut cube, Some(faces.clone())), Some(faces));
    let (size, kept) = fe_cube_copy(&mut cube, None).unwrap();
    assert_eq!(size, 32);
    for face in kept.chunks_exact(32 * 32 * 4) {
        assert!(face[..1024].iter().all(|&b| b == 0) && face[1024..].iter().all(|&b| b == 5));
    }
    // The zeroed rows stay in the cube.
    assert_eq!(cube.as_ref().map(|c| c.1[0]), Some(0));
}

/// 0x620AF0 at 0x6211CB, 0x6162F0 at 0x616776; 0x60C330
/// record 10: LOD D becomes C for a car with no wheel down whose
/// underside its last draw saw; the byte is written from the eye in the
/// car's frame against the two underside planes.
#[test]
fn an_airborne_car_seen_from_below_draws_lod_c() {
    let mut state = CarState::new(Vec3::new(10.0, 20.0, 5.0), 0.3);
    for w in &mut state.wheels {
        w.on_ground = false;
    }
    assert_eq!(airborne_pick_seen(Some(3), true, &state), Some(2));
    assert_eq!(airborne_pick_seen(Some(3), false, &state), Some(3));
    assert_eq!(airborne_pick_seen(Some(1), true, &state), Some(1));
    assert_eq!(airborne_pick_seen(None, true, &state), None);
    state.wheels[2].on_ground = true;
    assert_eq!(airborne_pick_seen(Some(3), true, &state), Some(3));
    let wheels = [
        Vec3::new(1.3, 0.8, 0.3),
        Vec3::new(1.3, -0.8, 0.3),
        Vec3::new(-1.3, 0.8, 0.3),
        Vec3::new(-1.3, -0.8, 0.3),
    ];
    let world = state.transform();
    assert!(underside_from(world.transform_point3(Vec3::new(0.0, 0.0, -4.0)), world, wheels));
    assert!(!underside_from(world.transform_point3(Vec3::new(-6.0, 0.0, 2.0)), world, wheels));
}

/// 0x452ED0 → 0x441340; 0x453570 → 0x44B040:
/// the chase view's shake runs on the world clock's tick (two equal
/// cameras at different ticks differ at speed), and the eye that sits in
/// a drawn traffic car is lifted onto its sphere, the car kept in view.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_chase_view_shakes_on_the_world_clock_and_lifts_over_traffic() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("GS1_CHASE");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    d.state.velocity = d.state.forward() * 30.0;
    let at = |d: &mut Drive, tick: u32| {
        d.chase = ChaseCamera::new(&d.car, 0);
        d.world_clock.tick = tick;
        d.chase_view(1.0 / 60.0);
        d.chase_view(1.0 / 60.0);
        d.camera.clone()
    };
    let a = at(d, 40_000);
    let b = at(d, 40_000);
    let c = at(d, 52_345);
    assert_eq!(a.position, b.position, "the same tick gives the same view");
    assert!(a.position != c.position || a.yaw != c.yaw || a.pitch != c.pitch, "the shake follows the tick");
    // A traffic car right under the eye: the eye goes onto its sphere.
    let eye = a.position;
    let mut car = lift_car(&d.car, &d.state).unwrap();
    car.origin = eye - Vec3::Z * 0.2;
    car.velocity = d.state.velocity;
    d.chase = ChaseCamera::new(&d.car, 0);
    d.world_clock.tick = 40_000;
    d.chase_view(1.0 / 60.0);
    d.chase_view_lifted(1.0 / 60.0, &[car]);
    let lifted = d.camera.clone();
    let r2 = (car.max - car.middle).length() + 1.0;
    let r2 = r2 * r2;
    let (dx, dy) = (lifted.position.x - car.origin.x, lifted.position.y - car.origin.y);
    assert!((lifted.position.z - ((r2 + 0.1 - dx * dx - dy * dy).sqrt() + car.origin.z)).abs() < 1e-3, "{lifted:?}");
    assert!(lifted.position.z > eye.z);
}

/// The reset key (0x602BE0 case 0xA1 → 0x5F3F10 → 0x6009B0): free roam
/// resets as a race does — not above 60 mph, onto the road (zone, node
/// within 150 m or a saved place) instead of a 1 m lift, the placement
/// stamped so that the key does nothing for 2 s.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_free_roam_reset_is_gated_and_puts_the_car_on_the_road() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("GS1_RESET");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    // One free-roam update: the saved places fill.
    let d = game.sim.drive.as_mut().unwrap();
    step_explore(d, None, None, Controls::default(), 1.0 / 60.0, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    assert_ne!(d.roam.place_index, 0, "the first update fills the ring (0x602AEE)");
    let start = d.state.position;
    let mut acts = [false; 18];
    acts[nfsu2::frontend::options::action::RESET] = true;
    // Above 60 mph the key does nothing.
    d.state.velocity = d.state.forward() * (61.0 / 2.237);
    game.dispatch(None, None, acts, &mut || {}).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    assert_eq!(d.state.position, start);
    assert!(d.placed_stamp.is_none());
    // At rest it places and stamps.
    d.state.velocity = Vec3::ZERO;
    game.dispatch(None, None, acts, &mut || {}).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    let placed = d.state.position;
    assert_eq!(d.placed_stamp, Some(d.world_clock.tick));
    assert!(placed != start + Vec3::Z, "not the old 1 m lift");
    // Within the 2 s hold the key does nothing; after it, it acts again.
    d.world_clock.tick += 7_999;
    game.dispatch(None, None, acts, &mut || {}).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    assert_eq!(d.state.position, placed);
    d.world_clock.tick += 1;
    game.dispatch(None, None, acts, &mut || {}).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    assert_eq!(d.placed_stamp, Some(d.world_clock.tick));
    // The car's own start hold (+0x2B4) blocks the key too.
    d.world_clock.tick += 8_000;
    d.state.held = true;
    let before = d.state.position;
    game.dispatch(None, None, acts, &mut || {}).unwrap();
    assert_eq!(game.sim.drive.as_ref().unwrap().state.position, before);
}
