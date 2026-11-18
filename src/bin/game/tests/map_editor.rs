//! The free camera in the game (docs/map-editor.md): unreachable without
//! `--editor`; with it, F8 in free roam opens it over the paused world, the
//! held keys fly it, the wheel and Z / X set its speed and field of view, G
//! puts the car at its aim point; a custom race runs its own route.
use super::*;

fn free_roam(name: &str) -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new(name);
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.advance(1.0 / 60.0).unwrap();
    game
}

fn press(game: &mut Game, code: KeyCode) {
    game.key(code, &mut || {}).unwrap();
}

fn hold(game: &mut Game, code: KeyCode, ticks: usize) {
    game.platform.keys.insert(code);
    for _ in 0..ticks {
        game.advance(1.0 / 60.0).unwrap();
    }
    game.platform.keys.remove(&code);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_free_camera_flies_and_puts_the_car_down() {
    use crate::enh::free_camera::{FOV_RANGE, SPEED_START};
    let mut game = free_roam("FREE_CAMERA_TEST");
    // Without --editor, F8 does nothing.
    nfsu2::tools::map::set_editor_enabled(false);
    press(&mut game, KeyCode::F8);
    assert!(game.free_camera.is_none() && matches!(game.mode, Mode::Explore));
    nfsu2::tools::map::set_editor_enabled(true);
    // Ctrl+F8 stays the support file's key.
    game.platform.keys.insert(KeyCode::ControlLeft);
    press(&mut game, KeyCode::F8);
    game.platform.keys.remove(&KeyCode::ControlLeft);
    assert!(game.free_camera.is_none(), "Ctrl+F8 is not the free camera's");
    press(&mut game, KeyCode::F8);
    assert!(game.free_camera.is_some(), "F8 opens the free camera in free roam");
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))), "the world paused under it");
    // The HUD hidden, the strip shown; H hides the strip too.
    assert!(!game.free_camera_quads().unwrap().is_empty());
    press(&mut game, KeyCode::KeyH);
    assert!(game.free_camera_quads().unwrap().is_empty());
    press(&mut game, KeyCode::KeyH);
    // The held keys fly the camera half a second at the start speed; the
    // car stays where it was.
    let car = game.sim.drive.as_ref().unwrap().state.position;
    let before = game.free_camera.as_ref().unwrap().camera.position;
    hold(&mut game, KeyCode::KeyW, 30);
    let after = game.free_camera.as_ref().unwrap().camera.position;
    let flown = after.distance(before);
    assert!((flown - SPEED_START * 0.5).abs() < 1.0, "{before} -> {after}");
    assert!(game.sim.drive.as_ref().unwrap().state.position.distance(car) < 0.05, "the world paused");
    // Up with R; the wheel speeds it up.
    let z = game.free_camera.as_ref().unwrap().camera.position.z;
    game.platform.mouse.wheel = 3.0;
    hold(&mut game, KeyCode::KeyR, 30);
    let fc = game.free_camera.as_ref().unwrap();
    assert!(fc.speed > SPEED_START * 1.9, "{}", fc.speed);
    assert!(fc.camera.position.z - z > 15.0, "{z} -> {}", fc.camera.position.z);
    // X widens the field of view up to its limit; Z narrows it.
    hold(&mut game, KeyCode::KeyX, 600);
    assert!((game.free_camera.as_ref().unwrap().fov_degrees() - FOV_RANGE.1).abs() < 0.1);
    hold(&mut game, KeyCode::KeyZ, 60);
    assert!(game.free_camera.as_ref().unwrap().fov_degrees() < FOV_RANGE.1 - 20.0);
    // Other keys of the old editor do nothing (no pages, no saving).
    for code in [KeyCode::Tab, KeyCode::F5, KeyCode::KeyT, KeyCode::KeyN, KeyCode::Space, KeyCode::Delete] {
        press(&mut game, code);
    }
    assert!(game.free_camera.is_some());
    assert!(!nfsu2::tools::map::store::maps_dir().join("my-map").exists(), "nothing saved");
    // Esc: back to driving.
    press(&mut game, KeyCode::Escape);
    assert!(game.free_camera.is_none() && matches!(game.mode, Mode::Explore));
    // G: the car to the ground point the camera looks at, then driving.
    press(&mut game, KeyCode::F8);
    let fc = game.free_camera.as_mut().unwrap();
    let aim_at = car + Vec3::new(30.0, 0.0, 0.0);
    fc.camera = nfsu2::render::Camera::looking_at(aim_at + Vec3::new(-6.0, 0.0, 6.0), aim_at);
    let expect = {
        let d = game.sim.drive.as_ref().unwrap();
        crate::enh::free_camera::aim(&game.free_camera.as_ref().unwrap().camera, &d.world)
    };
    press(&mut game, KeyCode::KeyG);
    assert!(game.free_camera.is_none() && matches!(game.mode, Mode::Explore));
    let now = game.sim.drive.as_ref().unwrap().state.position;
    assert!(now.truncate().distance(expect.truncate()) < 1.5, "{now} vs {expect}");
    assert!(now.truncate().distance(car.truncate()) > 20.0, "{car} -> {now}");
    nfsu2::tools::map::set_editor_enabled(false);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_custom_race_runs_its_own_route() {
    let mut game = free_roam("MAP_RACE_TEST");
    // A sprint over the first three checkpoints of a career sprint's track.
    let event = game
        .game_data
        .career
        .events
        .iter()
        .find(|e| e.kind == career::EventKind::Sprint && e.races.first().is_none_or(|r| r.1 & 1 == 0))
        .expect("a sprint")
        .clone();
    let number = event.races.first().map_or(event.track, |r| r.0);
    let track = game.game_data.career.tracks[&number].clone();
    let native = career::load_route(&game.game_data.root, &track, false).unwrap();
    let mut race = nfsu2::tools::map::CustomRace::new("short", number, nfsu2::tools::map::RaceKind::Sprint);
    race.checkpoints = native.checkpoints[..3].to_vec();
    game.sim.map_race = Some(race.clone());
    game.start_race(event.index, true, false).unwrap();
    let Mode::Race(r) = &game.mode else { panic!("racing") };
    assert_eq!(r.route.checkpoints, race.checkpoints);
    assert!(r.route.road.length < native.road.length, "{} < {}", r.route.road.length, native.road.length);
    assert_eq!(r.laps, 1);
    for _ in 0..120 {
        game.advance(1.0 / 60.0).unwrap();
    }
    // Back in the city the custom race is over.
    game.explore(None).unwrap();
    assert!(game.sim.map_race.is_none());
}
