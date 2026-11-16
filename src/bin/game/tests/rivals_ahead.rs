//! The Engage dialog's rivals loaded ahead (`RivalsAhead`,
//! docs/first-use-builds.md "Car renderers ahead"): the line-up predicted
//! behind the dialog is the race's own, so the race's set-up takes every
//! car loaded ahead, and the race is the same with the loads ahead and
//! without them.
use super::*;

/// A career at `stage` with the first `wins` of each earlier stage's
/// plain events and of its own won (the stage's skill reads the wins),
/// in free roam.
pub(super) fn career_at(stage: u8, wins: usize) -> Game {
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("RIVALS_AHEAD_TEST");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    for s in 1..=stage {
        let events: Vec<_> = game
            .game_data
            .career
            .events
            .iter()
            .filter(|e| e.stage == s && e.category == career::Category::Normal)
            .cloned()
            .collect();
        let n = if s < stage { events.len() } else { wins.min(events.len()) };
        for e in &events[..n] {
            p.win_paid(&game.game_data.career, &game.game_data.root, e, 60.0, None);
        }
    }
    p.stage = stage;
    game.profile = Some(p);
    game.explore(None).unwrap();
    game
}

/// Every kind of event the stage's Engage dialogs offer (circuit, sprint,
/// Street X, drag, drift, the URL and tournament events of more than one
/// race), one each.
pub(super) fn kinds(game: &Game, stage: u8) -> Vec<(usize, String)> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for e in game.game_data.career.stage_events(&game.game_data.root, stage) {
        let what = (e.kind, e.category == career::Category::Url, e.races.len() > 1);
        if !seen.contains(&what) {
            seen.push(what);
            out.push((
                e.index,
                format!("stage {stage} event {} {:?} url {} races {}", e.index, e.kind, what.1, e.races.len()),
            ));
        }
    }
    out
}

/// The race's simulation state after its first steps.
pub(super) fn race_steps(game: &mut Game) -> Vec<[u64; 5]> {
    (0..30)
        .map(|_| {
            game.step_for(1.0 / 60.0).unwrap();
            crate::dev::state_hash::components(game)
        })
        .collect()
}

/// From a fresh career and from careers in their stages (at the stage's
/// start and half way through its events), each event kind's race takes
/// all its rivals' cars loaded ahead of the line-up predicted from the
/// random numbers as they stand, and the race (its rivals, their cars,
/// the generators, every racer's state in its first steps) is the one
/// started with nothing loaded ahead.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_engaged_race_takes_every_rival_loaded_ahead() {
    testkit::game();
    let mut checked = 0;
    for (stage, half) in [(1u8, false), (1, true), (2, false), (3, true), (4, true), (5, true)] {
        let wins = |g: &Game| {
            let n = g
                .game_data
                .career
                .events
                .iter()
                .filter(|e| e.stage == stage && e.category == career::Category::Normal)
                .count();
            if half { n / 2 } else { 0 }
        };
        let probe = career_at(stage, 0);
        let wins = wins(&probe);
        for (index, what) in kinds(&probe, stage) {
            let mut ahead = career_at(stage, wins);
            let mut in_place = career_at(stage, wins);
            assert_eq!(ahead.rng.0, in_place.rng.0, "{what}: the two careers start alike");
            let Some(keys) = ahead.predict_rivals(index) else { panic!("{what}: no line-up predicted") };
            let rng = ahead.rng.0;
            let loads = keys.iter().map(|k| (k.clone(), ahead.spawn_rival_load(k), None)).collect();
            ahead.loads.rivals_ahead = Some(crate::loads::preload::RivalsAhead { event: index, car: None, rng, loads });
            assert_eq!(ahead.rng.0, rng, "{what}: the prediction draws on a copy");
            ahead.start_race(index, false, false).unwrap();
            in_place.start_race(index, false, false).unwrap();
            let (taken, made) = ahead.loads.rivals_taken;
            eprintln!("{what}: {taken} of {made} taken");
            assert_eq!(keys.len(), made, "{what}: the cars predicted are the cars asked for");
            assert_eq!(taken, made, "{what}: every rival's car taken from the loads ahead");
            assert_eq!(in_place.loads.rivals_taken, (0, made), "{what}: nothing ahead in the other");
            let line_up = |g: &Game| match &g.mode {
                Mode::Race(r) => r
                    .racers
                    .rivals
                    .iter()
                    .map(|r| (r.name.clone(), r.car.car_type.name.clone(), r.slot))
                    .collect::<Vec<_>>(),
                _ => panic!("{what}: not racing"),
            };
            assert_eq!(line_up(&ahead), line_up(&in_place), "{what}: the same rivals");
            assert_eq!(ahead.rng.0, in_place.rng.0, "{what}: the generator after the set-up");
            assert_eq!(race_steps(&mut ahead), race_steps(&mut in_place), "{what}: the same race");
            checked += 1;
        }
    }
    assert!(checked >= 20, "event kinds checked: {checked}");
}

/// A Quick Race's rivals predicted while its transmission question is up
/// (`predict_quick_rivals`: the car picked, the drive dropped, the
/// line-up drawn after the track's load) are the race's own: every mode,
/// with a car and an SUV (its own pool), from the front end of a career
/// part way through, takes every car loaded ahead, and the race is the one
/// started with nothing loaded ahead.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_quick_race_takes_every_rival_loaded_ahead() {
    testkit::game();
    use screens::QrMode;
    let probe = Game::new().unwrap();
    let suv = probe.game_data.car_types.iter().find(|t| t.suv).map(|t| t.name.clone()).expect("an SUV");
    let mut checked = 0;
    for mode in [QrMode::Circuit, QrMode::Sprint, QrMode::Drag, QrMode::Drift, QrMode::StreetX, QrMode::Url] {
        for car in ["PEUGOT", "RX8", suv.as_str()] {
            let what = format!("Quick Race {mode:?} in {car}");
            let setup = || {
                let mut g = career_at(3, 2);
                g.drop_drive();
                g.mode = g.icon_menu(MenuKind::Main);
                g.quick.tracks = g.quick_track_select(mode, 0.0);
                let e = g.quick_event(mode);
                g.quick.car = Some(car.to_string());
                (g, e)
            };
            let (mut ahead, index) = setup();
            let (mut in_place, _) = setup();
            assert_eq!(ahead.rng.0, in_place.rng.0, "{what}: the two games start alike");
            let Some(keys) = ahead.predict_quick_rivals(index) else { panic!("{what}: no line-up predicted") };
            let rng = ahead.rng.0;
            let loads = keys.iter().map(|k| (k.clone(), ahead.spawn_rival_load(k), None)).collect();
            let car_picked = Some((ahead.quick.car.clone(), ahead.quick.parts.clone()));
            ahead.loads.rivals_ahead = Some(crate::loads::preload::RivalsAhead { event: index, car: car_picked, rng, loads });
            assert_eq!(ahead.rng.0, rng, "{what}: the prediction draws on a copy");
            ahead.start_race(index, true, false).unwrap();
            in_place.start_race(index, true, false).unwrap();
            let (taken, made) = ahead.loads.rivals_taken;
            eprintln!("{what}: {taken} of {made} taken");
            assert_eq!(keys.len(), made, "{what}: the cars predicted are the cars asked for");
            assert_eq!(taken, made, "{what}: every rival's car taken from the loads ahead");
            assert_eq!(in_place.loads.rivals_taken, (0, made), "{what}: nothing ahead in the other");
            let line_up = |g: &Game| match &g.mode {
                Mode::Race(r) => {
                    r.racers.rivals.iter().map(|r| (r.name.clone(), r.car.car_type.name.clone(), r.slot)).collect::<Vec<_>>()
                }
                _ => panic!("{what}: not racing"),
            };
            assert_eq!(line_up(&ahead), line_up(&in_place), "{what}: the same rivals");
            assert_eq!(ahead.rng.0, in_place.rng.0, "{what}: the generator after the set-up");
            assert_eq!(race_steps(&mut ahead), race_steps(&mut in_place), "{what}: the same race");
            checked += 1;
        }
    }
    assert!(checked >= 18, "Quick Races checked: {checked}");
}
