//! A race raced again (docs/formats.md "Race types: Restart and Next Race keep the
//! field"): Restart races the same field (its line-up, the rivals' looks
//! and performance, the grid slots) on the event's current race, Next Race
//! takes the field on to the event's next race, and the rivals' cars of the
//! race before are taken instead of loaded again, with no difference to the
//! race.
use super::rivals_ahead::{career_at, kinds, race_steps};
use super::*;

/// The field on the grid: each rival's name, car key and grid slot, the
/// player's slot, the round and the track's first grid position.
type FieldSeen = (Vec<(String, crate::loads::preload::RivalKey, usize)>, usize, usize, Vec3);

fn field(game: &Game, what: &str) -> FieldSeen {
    let Mode::Race(r) = &game.mode else { panic!("{what}: not racing ({})", flow_mode(&game.mode)) };
    assert_eq!(r.racers.rivals.len(), r.field.looks.len(), "{what}: a key for every rival");
    let rivals = r
        .racers
        .rivals
        .iter()
        .zip(&r.field.looks)
        .map(|(v, k)| {
            assert_eq!(v.car.car_type.name, k.0, "{what}: the rival's car is its key's");
            (v.name.clone(), k.clone(), v.slot)
        })
        .collect();
    (rivals, r.grid_slot, r.round, r.route.grid.first().map_or(Vec3::ZERO, |g| g.0))
}

/// `n` ticks of the race.
fn steps(game: &mut Game, n: usize) {
    for _ in 0..n {
        game.step_for(1.0 / 60.0).unwrap();
    }
}

/// The race's pause, as its key opens it.
fn pause(game: &mut Game, what: &str) {
    let Mode::Race(r) = std::mem::replace(&mut game.mode, Mode::Explore) else { panic!("{what}: not racing") };
    game.sim.paused_race = Some(r);
    game.mode = game.icon_menu(MenuKind::RacePause);
}

/// `action` on the menu `kind`, its question answered Yes.
fn act(game: &mut Game, action: Action, kind: MenuKind, what: &str) {
    game.fe.confirmed = true;
    game.act(action, kind).unwrap();
    assert!(matches!(game.mode, Mode::Race(_)), "{what}: {action:?} did not race ({})", flow_mode(&game.mode));
}

/// A Quick Race of `mode` in `car` set up from the front end, as its car
/// select leaves it (the drive dropped), from a career at stage 3.
fn quick_race(mode: screens::QrMode, car: &str) -> (Game, usize) {
    let mut game = career_at(3, 2);
    game.drop_drive();
    game.mode = game.icon_menu(MenuKind::Main);
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let e = game.quick_event(mode);
    game.quick.car = Some(car.into());
    (game, e)
}

/// The races checked: every kind of career event of stages 1 and 3 (the
/// URL and tournament events among them) and the Quick Race modes.
fn races() -> Vec<(String, Box<dyn Fn() -> Game>)> {
    let mut out: Vec<(String, Box<dyn Fn() -> Game>)> = Vec::new();
    for stage in [1u8, 3] {
        let probe = career_at(stage, 0);
        for (index, what) in kinds(&probe, stage) {
            out.push((
                what,
                Box::new(move || {
                    let mut g = career_at(stage, 0);
                    g.start_race(index, false, false).unwrap();
                    g
                }),
            ));
        }
    }
    use screens::QrMode;
    for mode in [QrMode::Circuit, QrMode::Sprint, QrMode::Drag, QrMode::Drift, QrMode::StreetX] {
        out.push((
            format!("Quick Race {mode:?}"),
            Box::new(move || {
                let (mut g, e) = quick_race(mode, "PEUGOT");
                g.start_race(e, true, false).unwrap();
                g
            }),
        ));
    }
    out
}

/// Restart from the pause, twice, races the same field again (the
/// original's track switch for message 9 runs no line-up, looks or grid:
/// seen under Wine, the racer records kept byte for byte over three
/// restarts of a Quick Race and of a career race), and takes every rival's
/// car of the race before instead of loading it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn restart_races_the_same_field_again() {
    testkit::game();
    let mut checked = 0;
    for (what, make) in races() {
        let mut game = make();
        let first = field(&game, &what);
        let n = first.0.len();
        for again in 1..=2 {
            steps(&mut game, 90);
            pause(&mut game, &what);
            act(&mut game, Action::Restart, MenuKind::RacePause, &what);
            assert_eq!(field(&game, &what), first, "{what}: restart {again} races the same field");
            assert_eq!(game.loads.rivals_kept, n, "{what}: restart {again} keeps every rival's car");
        }
        eprintln!("{what}: {n} rivals kept");
        checked += 1;
    }
    assert!(checked >= 12, "races checked: {checked}");
}

/// The rivals' cars kept by a restart give the race their loads give: the
/// same rivals, generators and every racer's state in the restarted race's
/// first steps as a restart that loads them again.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_restart_with_the_cars_kept_is_the_restart_that_loads_them() {
    testkit::game();
    let mut checked = 0;
    for (what, make) in races() {
        let mut kept = make();
        let mut loaded = make();
        for g in [&mut kept, &mut loaded] {
            steps(g, 120);
            pause(g, &what);
        }
        // Without the paused race the restart finds no cars to keep (it
        // races the field Restart keeps, `RaceFlow::restart`).
        loaded.sim.paused_race = None;
        act(&mut kept, Action::Restart, MenuKind::RacePause, &what);
        act(&mut loaded, Action::Restart, MenuKind::RacePause, &what);
        let n = field(&kept, &what).0.len();
        assert_eq!(kept.loads.rivals_kept, n, "{what}: every car kept");
        assert_eq!(loaded.loads.rivals_kept, 0, "{what}: every car loaded");
        assert_eq!(field(&kept, &what), field(&loaded, &what), "{what}: the same field");
        assert_eq!(kept.rng.0, loaded.rng.0, "{what}: the generator after the set-up");
        assert_eq!(race_steps(&mut kept), race_steps(&mut loaded), "{what}: the same race");
        checked += 1;
    }
    assert!(checked >= 12, "races checked: {checked}");
}

/// A tournament: Next Race takes the field (line-up, looks, grid slots) on
/// to the event's next race (0x4BEC90 → 0x54CAF0, then the track switch
/// for message 10 without the line-up, 0x5322F0(event, 0)) with the cars
/// of the race before, and Restart races the event's current race again,
/// not its first (the race record 0x85EF48 stays).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_tournament_keeps_its_field_from_race_to_race() {
    testkit::game();
    let mut checked = 0;
    for stage in [1u8, 2, 3] {
        let probe = career_at(stage, 0);
        let events: Vec<usize> = probe
            .game_data
            .career
            .stage_events(&probe.game_data.root, stage)
            .iter()
            .filter(|e| e.races.len() > 1)
            .map(|e| e.index)
            .take(2)
            .collect();
        for index in events {
            let what = format!("stage {stage} event {index}");
            let mut game = career_at(stage, 0);
            game.start_race(index, false, false).unwrap();
            let first = field(&game, &what);
            steps(&mut game, 120);
            // The race's end as `race_end` leaves a tournament's race: the
            // next round with the field, the race kept for its replay.
            let Mode::Race(race) = &game.mode else { unreachable!() };
            let next = (race.event, race.quick, race.from_map, race.round + 1, race.field.clone());
            game.race_flow.next_round = Some(next);
            game.keep_replay(game.icon_menu(MenuKind::PostRace));
            assert!(
                game.sim.replay.is_some() || !game.loads.raced_cars.is_empty(),
                "{what}: the race kept for its replay, or its rivals' cars"
            );
            act(&mut game, Action::NextRace, MenuKind::PostRace, &what);
            let second = field(&game, &what);
            assert_eq!(second.0, first.0, "{what}: the next race's rivals, looks and grid slots");
            assert_eq!(second.1, first.1, "{what}: the player's grid slot");
            assert_eq!(second.2, 1, "{what}: the second race");
            assert_eq!(game.loads.rivals_kept, first.0.len(), "{what}: the rivals' cars of the race before");
            steps(&mut game, 60);
            pause(&mut game, &what);
            act(&mut game, Action::Restart, MenuKind::RacePause, &what);
            assert_eq!(field(&game, &what), second, "{what}: Restart races the second race again");
            checked += 1;
        }
    }
    assert!(checked >= 2, "tournaments checked: {checked}");
}
