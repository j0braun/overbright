//! The map editor in the game (docs/map-editor.md): unreachable without
//! `--editor`; with it, F8 in free roam opens it over the paused world,
//! its edits reach the world and the mod file in Overbright's data folder,
//! Esc saves and resumes; a custom race runs its own route.
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

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_editor_edits_props_and_saves_a_map_mod() {
    let mut game = free_roam("MAP_EDITOR_TEST");
    // Without --editor, F8 does nothing.
    nfsu2::tools::map::set_editor_enabled(false);
    press(&mut game, KeyCode::F8);
    assert!(game.map_editor.is_none() && matches!(game.mode, Mode::Explore));
    nfsu2::tools::map::set_editor_enabled(true);
    press(&mut game, KeyCode::F8);
    assert!(game.map_editor.is_some(), "F8 opens the editor in free roam");
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))), "the world paused under it");
    // The camera flies with the held keys.
    let before = game.map_editor.as_ref().unwrap().ed.camera.position;
    game.platform.keys.insert(KeyCode::KeyW);
    for _ in 0..30 {
        game.advance(1.0 / 60.0).unwrap();
    }
    game.platform.keys.remove(&KeyCode::KeyW);
    let after = game.map_editor.as_ref().unwrap().ed.camera.position;
    assert!(after.distance(before) > 5.0, "{before} -> {after}");
    // A small prop near the car, aimed at from 10 m away and picked.
    let d = game.sim.drive.as_ref().unwrap();
    let car = d.state.position;
    let (i, centre) = (0..d.world.placements.len())
        .filter(|&i| d.world.placements[i].flags & 0x0110 == 0)
        .filter_map(|i| nfsu2::tools::map::edit::placement_sphere(&d.world, i).map(|s| (i, s)))
        .filter(|(_, (_, r))| *r > 0.3 && *r < 3.0)
        .min_by(|a, b| a.1.0.distance(car).total_cmp(&b.1.0.distance(car)))
        .map(|(i, (c, _))| (i, c))
        .expect("a small prop");
    let ed = &mut game.map_editor.as_mut().unwrap().ed;
    ed.camera = nfsu2::render::Camera::looking_at(centre + Vec3::new(0.0, -10.0, 1.0), centre);
    press(&mut game, KeyCode::Space);
    let picked = game.map_editor.as_ref().unwrap().ed.selected.expect("picked");
    // The ray meets that prop or one in front of it.
    let w = &game.sim.drive.as_ref().unwrap().world;
    let at = w.placements[picked].transform.w_axis.truncate();
    assert!(picked == i || at.distance(centre) < 12.0);
    // Moved one step up: in the world, and as a move edit in the mod.
    press(&mut game, KeyCode::PageUp);
    let w = &game.sim.drive.as_ref().unwrap().world;
    assert!((w.placements[picked].transform.w_axis.z - at.z - 0.25).abs() < 1e-4);
    let ed = &game.map_editor.as_ref().unwrap().ed;
    assert!(matches!(ed.map.props.as_slice(), [nfsu2::tools::map::PropEdit::Move { .. }]));
    // Deleted: the move becomes a delete of the same original.
    press(&mut game, KeyCode::Delete);
    let w = &game.sim.drive.as_ref().unwrap().world;
    assert!(w.placements[picked].flags & nfsu2::world::HIDDEN_FLAG != 0);
    let ed = &game.map_editor.as_ref().unwrap().ed;
    assert!(matches!(ed.map.props.as_slice(), [nfsu2::tools::map::PropEdit::Delete { .. }]));
    // A race on the race page: N at the aim point, two more checkpoints.
    press(&mut game, KeyCode::Tab);
    press(&mut game, KeyCode::KeyN);
    let ed = &game.map_editor.as_ref().unwrap().ed;
    assert_eq!(ed.map.races.len(), 1, "{:?}", ed.status);
    // Esc: saved, enabled, back to driving.
    press(&mut game, KeyCode::Escape);
    assert!(game.map_editor.is_none());
    assert!(matches!(game.mode, Mode::Explore));
    let dir = nfsu2::tools::map::store::maps_dir();
    // The test's own data folder (`test_isolation`), never the player's.
    assert!(dir.starts_with(nfsu2::platform::dirs::app()));
    assert_eq!(nfsu2::tools::map::store::enabled(&dir), vec!["my-map".to_string()]);
    let saved = nfsu2::tools::map::store::read_file(&dir.join("my-map").join("map.obmap")).unwrap();
    assert_eq!(saved.props.len(), 1);
    assert_eq!(saved.races.len(), 1);
    // The world as edited stays so through a kept world's reset.
    let w = &mut game.sim.drive.as_mut().unwrap().world;
    assert!(w.reset_to_loaded(true));
    assert!(w.placements[picked].flags & nfsu2::world::HIDDEN_FLAG != 0);
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
