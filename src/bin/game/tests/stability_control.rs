//! The Stability Control option in the car physics. The player's option
//! byte (player block +0x16, 1 shows "Off") becomes car +0x464 when the car
//! is placed (0x5F1802 → 0x5F3E20) and, from the pause's Player options, at
//! once (the row's handler 0x4C3200 queues race command kind 2, applied by
//! 0x4A7B30). With it off, the friction circle 0x59D610 and the sliding μ
//! 0x5A8290 scale a forward-spinning, unlocked rear wheel's grip by
//! 0x59D4A0: 0.85 up to 30 mph, 1 from 50 mph. With it on, the frame
//! set-up 0x5A4D90 takes every wheel's grip × 0.85, the handbrake's +0.5,
//! both rear-bonus bases and the record's +0x398 yaw-torque limit.
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Free roam in a fresh career with the option as given.
fn free_roam(off: bool) -> Game {
    let mut game = Game::new().unwrap();
    game.settings.players[0].stability_off = off;
    let mut profile = Profile::new("STABILITY_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    assert!(matches!(game.mode, Mode::Explore));
    game
}

/// The oversteer run of the parity scenario launch_lock: standing, full
/// left lock, full throttle from 1 s, 6 s at the fixed step. Returns the
/// peak |yaw rate| (rad/s) and the peak rear lateral speed (m/s, the
/// car's left axis at the rear axle).
fn oversteer(off: bool) -> (f32, f32) {
    let mut game = free_roam(off);
    let d = game.sim.drive.as_mut().unwrap();
    assert_eq!(d.state.stability_off, off, "the option reaches the car");
    // The scenario's spawn in the original (frame 0 of its run).
    let at = Vec2::new(462.416, -46.1571);
    let z = d.world.road_height_below(at.extend(30.0)).unwrap();
    d.place_reset(at.extend(z), (-0.345541f32).atan2(0.938404));
    let mut rng = nfsu2::opponents::GameRng::default();
    let (mut yaw, mut lateral) = (0.0f32, 0.0f32);
    for frame in 0..360 {
        let throttle = if frame >= 60 { 1.0 } else { 0.0 };
        let controls = Controls { throttle, steer: -1.0, ..Default::default() };
        d.step(controls, 1.0 / 60.0, &mut rng, &mut Default::default());
        yaw = yaw.max(d.state.spin.z.abs());
        let left = d.state.orientation * Vec3::Y;
        let rear = d.state.velocity + d.state.spin.cross(d.state.orientation * Vec3::new(-1.3, 0.0, 0.0));
        lateral = lateral.max(rear.dot(left).abs());
    }
    (yaw, lateral)
}

/// The option set before free roam reaches the car, and a scripted
/// oversteer run breaks the rear away with it off and much less with it
/// on (the original under Wine, the same run from its own spawn: rear
/// lateral speed peaks at 7.8 m/s off, 3.0 m/s on).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn stability_off_loosens_the_rear() {
    testkit::game();
    let (yaw_on, lat_on) = oversteer(false);
    let (yaw_off, lat_off) = oversteer(true);
    eprintln!("yaw rate on {yaw_on} off {yaw_off}; rear lateral speed on {lat_on} off {lat_off}");
    assert!(lat_off > lat_on * 1.5, "rear lateral speed: off {lat_off} vs on {lat_on}");
}

/// Stability Control changed in the pause's Player options reaches the
/// car being driven at once (0x4C3200 → 0x578F00 → 0x4A7B30), not only
/// the next car placed.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn pause_option_reaches_the_driven_car() {
    testkit::game();
    let mut game = free_roam(true);
    let pause = options::Pause { career_roam: true, ..Default::default() };
    let mut o = options::OptionsScreen::new_in(options::Category::Player, &game.settings, false, Some(pause), 0.0);
    let rows = options::rows_in(options::Category::Player, &game.settings, 0, false, Some(pause));
    o.selected = rows.iter().position(|r| r.id == options::RowId::Stability).unwrap();
    game.mode = Mode::Menu(Menu::Options(o));
    game.options_key(Key::Right, 0.0).unwrap();
    assert!(!game.settings.players[0].stability_off);
    assert!(!game.sim.drive.as_ref().unwrap().state.stability_off, "on: the car's flag cleared");
    game.options_key(Key::Right, 0.0).unwrap();
    assert!(game.sim.drive.as_ref().unwrap().state.stability_off, "off again: the car's flag set");
}
