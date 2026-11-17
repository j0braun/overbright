use crate::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

#[test]
fn wrong_way_follows_the_games_thresholds() {
    let road = Vec2::X;
    let back = std::f32::consts::PI;
    // Along the road: never.
    assert!(!wrong_way(false, 0.0, road, Vec2::new(20.0, 0.0), 4));
    // Turned round at speed: only with all wheels down when new.
    assert!(wrong_way(false, back, road, Vec2::new(-20.0, 0.0), 4));
    assert!(!wrong_way(false, back, road, Vec2::new(-20.0, 0.0), 3));
    // At walking pace the last answer stands.
    assert!(!wrong_way(false, back, road, Vec2::new(-2.0, 0.0), 4));
    assert!(wrong_way(true, back, road, Vec2::new(-2.0, 0.0), 4));
    // 105° off: wrong only if it already was (110° less 10°).
    let h = 105f32.to_radians();
    let v = Vec2::new(h.cos(), h.sin()) * 20.0;
    assert!(!wrong_way(false, h, road, v, 4));
    assert!(wrong_way(true, h, road, v, 4));
    let h = 95f32.to_radians();
    assert!(!wrong_way(true, h, road, Vec2::new(h.cos(), h.sin()) * 20.0, 4));
}

/// One wrong-way run (see [`wrong_way_run`]).
#[derive(Debug, Default)]
struct WrongWayRun {
    /// WRONG WAY while the car went on along the race road (its road
    /// distance up 2 m or more over the last 0.5 s, within 12 m of the
    /// road, its travel within 90° of the route's way there) and the
    /// locator's way was the route's: time, road distance, position.
    false_frames: Vec<(f32, f32, Vec2)>,
    /// WRONG WAY while the car really went back or off the road, or
    /// against the route's way (the controller overshooting a hairpin,
    /// Sprint 4161 v0 at ~72.7 s: travel 109° off the road), or where
    /// the native locator's way is against the route's (a race spline's
    /// own loop: Circuit 4005 v0's 10 m segment 12 between 50–80 m
    /// neighbours, the original gives the same way under Wine,
    /// `race_locator_matches_native_in_a_spline_loop`).
    real_frames: usize,
    /// The race's progress in laps at the end, and whether it finished.
    progress: f32,
    finished: bool,
    /// The U-turn showed WRONG WAY (None: no U-turn asked).
    u_turn: Option<bool>,
}

/// One race on `track` / `variant` over `laps` for the wrong-way
/// checks: an event of `kind` pointed there, no rivals or traffic, the
/// player's car on the AI racer controller (`Autopilot`) from the
/// green light. With `u_turn` the car is turned round where it is
/// from that race time on, once it runs along the road, and driven
/// straight on at full throttle for 4 s.
fn wrong_way_run(
    game: &mut Game,
    kind: career::EventKind,
    track: u16,
    variant: u8,
    laps: u8,
    u_turn: Option<f32>,
) -> WrongWayRun {
    let event = game.game_data.career.events.iter().position(|e| e.kind == kind).unwrap();
    {
        let e = &mut game.game_data.career.events[event];
        (e.track, e.variant, e.laps, e.traffic) = (track, variant, laps, 0);
        e.races.clear();
    }
    game.start_race(event, true, false).unwrap();
    while game.scenes.shop.is_some() && matches!(game.mode, Mode::Race(_)) {
        game.advance_for(1.0 / 30.0).unwrap();
    }
    for _ in 0..(COUNTDOWN * 30.0) as u32 + 2 {
        game.advance_for(1.0 / 30.0).unwrap();
    }
    {
        let (Mode::Race(race), Some(d)) = (&mut game.mode, game.sim.drive.as_ref()) else { panic!("racing") };
        race.racers.rivals.clear();
        race.order = vec![0];
        race.racers.traffic = None;
        race.autopilot =
            Some(Autopilot::new(d, race.racers.ai_routes.as_ref().unwrap(), d.car.car_type.handling.race.drift()));
    }
    let mut run = WrongWayRun { u_turn: u_turn.map(|_| false), ..Default::default() };
    let mut history = std::collections::VecDeque::new();
    let mut turned = None;
    // 480 s: Circuit 4083's two laps take a rival ~390 s, more when it
    // backs out of the hairpin at (−1510, 740) (its GetUnstuck), and
    // the start scene's roll-in shifts where the racers meet it.
    for _ in 0..(480 * 30) {
        let Mode::Race(race) = &mut game.mode else { break };
        // The race's replay recording is kept off.
        race.replay.recording = None;
        if race.racers.me.finished.is_some() {
            break;
        }
        let clock = race.timing.clock;
        // The U-turn once the car runs on along the road (heading
        // within 30° of the road's way, over 10 m/s).
        let aligned = {
            let st = &game.sim.drive.as_ref().unwrap().state;
            let h = st.heading();
            Vec2::new(h.cos(), h.sin()).dot(race.racers.me.locator.direction) > 0.866 && st.velocity.length() > 10.0
        };
        if u_turn.is_some_and(|at| turned.is_none() && clock >= at && aligned) {
            race.autopilot = None;
            let d = game.sim.drive.as_mut().unwrap();
            let (p, h) = (d.state.position, d.state.heading() + std::f32::consts::PI);
            d.place(p + Vec3::Z, h);
            turned = Some(clock);
            game.platform.keys.clear();
            game.platform.keys.insert(KeyCode::ArrowUp);
        }
        let at = game.sim.drive.as_ref().unwrap().state.position.truncate();
        history.push_back(race.racers.me.along);
        if history.len() > 16 {
            history.pop_front();
        }
        if let Some(t) = turned {
            if clock > t + 4.0 {
                break;
            }
            if race.wrong_way {
                run.u_turn = Some(true);
            }
        } else if race.wrong_way {
            let way = race.route.road.direction(race.racers.me.along);
            let travel = game.sim.drive.as_ref().unwrap().state.velocity.truncate();
            let on = history.len() > 15
                && race.route.road.difference(race.racers.me.along, history[0]) >= 2.0
                && at.distance(race.racers.me.locator.point) < 12.0
                && travel.dot(way) > 0.0
                && race.racers.me.locator.direction.dot(way) > 0.0;
            if on {
                run.false_frames.push((clock, race.racers.me.along, at));
            } else {
                run.real_frames += 1;
            }
        }
        game.advance_for(1.0 / 30.0).unwrap();
    }
    game.platform.keys.clear();
    if let Mode::Race(race) = &game.mode {
        (run.progress, run.finished) = (race.racers.me.progress(), race.racers.me.finished.is_some());
    }
    run
}

/// Wrong way over every track of `kinds` (each kind's laps), both
/// directions: a clean run first, then one with a U-turn at 20 s.
/// Returns the failures (a false WRONG WAY, or a U-turn without one)
/// and the runs the AI did not finish (lost or stuck).
fn wrong_way_tracks(kinds: &[(career::EventKind, u8)]) -> (Vec<String>, Vec<String>) {
    let mut game = Game::new().unwrap();
    println!("profile {:?}", game.profile.as_ref().map(|p| (&p.name, &p.car, p.stage, p.perf.len())));
    let (mut failures, mut unfinished) = (Vec::new(), Vec::new());
    for &(kind, laps) in kinds {
        let mut tracks: Vec<u16> = game
            .game_data.career
            .events
            .iter()
            .filter(|e| e.kind == kind)
            .flat_map(|e| std::iter::once(e.track).chain(e.races.iter().map(|r| r.0)))
            .collect();
        tracks.sort();
        tracks.dedup();
        if let Ok(only) = std::env::var("WRONG_WAY_TRACK") {
            tracks.retain(|t| t.to_string() == only);
        }
        for &track in &tracks {
            for variant in [0u8, 1] {
                let clean = wrong_way_run(&mut game, kind, track, variant, laps, None);
                let turn = wrong_way_run(&mut game, kind, track, variant, laps, Some(20.0));
                let name = format!("{kind:?} {track} variant {variant}");
                println!(
                    "{name}: {:.2} laps, false wrong way {} frames, real {}, U-turn shown {:?}",
                    clean.progress,
                    clean.false_frames.len(),
                    clean.real_frames,
                    turn.u_turn
                );
                for w in clean.false_frames.iter().take(3) {
                    println!("    false wrong way at {:.1} s, road {:.0} m, at {:.0} {:.0}", w.0, w.1, w.2.x, w.2.y);
                }
                if !clean.false_frames.is_empty() {
                    failures.push(format!("{name}: false wrong way"));
                }
                if turn.u_turn != Some(true) {
                    failures.push(format!("{name}: U-turn not shown"));
                }
                if !clean.finished {
                    unfinished.push(name);
                }
            }
        }
    }
    (failures, unfinished)
}

/// Every Street X track, both directions, two laps on the AI racer
/// controller: no WRONG WAY while the car goes on along the road
/// (the clone's road polyline once had a backward
/// closing piece past the loop's end, 4607), and a U-turn shows it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn wrong_way_street_x() {
    testkit::game();
    let (failures, unfinished) = wrong_way_tracks(&[(career::EventKind::StreetX, 2)]);
    assert!(failures.is_empty() && unfinished.is_empty(), "{failures:?} unfinished {unfinished:?}");
}

/// The player's own path through a Street X race (a repeated player
/// report): the career's first Street X event
/// (STAGE_1_SHORT_1) moved onto every Street X track and direction,
/// started as the race dialog does (the start scene and its posed
/// cars, the rivals), then driven on the keys at 60 fps along the
/// road with a weave, through the first corner and the lap wrap. No
/// WRONG WAY while the car runs on along the road (the road polyline's
/// way near the car, apart from the road locator), and the HUD never
/// draws a "Wrong Way" string: HUD_Short_Track's WrongWayText, which
/// no element claims, stays hidden as the HUD's constructor leaves it
/// (0x52CFD0, see `hud::Hud::quads`). It used to show from the start.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn wrong_way_street_x_on_keys() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let e = game.game_data.career.events.iter().position(|e| e.name == "STAGE_1_SHORT_1").unwrap();
    let mut tracks: Vec<u16> = game
        .game_data.career
        .events
        .iter()
        .filter(|e| e.kind == career::EventKind::StreetX)
        .flat_map(|e| std::iter::once(e.track).chain(e.races.iter().map(|r| r.0)))
        .collect();
    tracks.sort();
    tracks.dedup();
    let mut failures = Vec::new();
    const DT: f32 = 1.0 / 60.0;
    for &track in &tracks {
        for variant in [0u8, 1] {
            {
                let ev = &mut game.game_data.career.events[e];
                (ev.track, ev.variant, ev.laps) = (track, variant, 2);
                ev.races.clear();
            }
            game.start_race(e, false, false).unwrap();
            let name = format!("Street X {track} variant {variant}");
            let (mut false_frames, mut text_frames, mut laps_done) = (0, 0, false);
            let mut t = 0.0f32;
            let (mut stalled, mut reverse, mut recoveries) = (0.0f32, 0.0f32, 0);
            let mut backward_at: Option<usize> = None;
            for frame in 0..(300.0 / DT) as usize {
                let Mode::Race(race) = &game.mode else { break };
                if race.racers.me.finished.is_some() {
                    laps_done = true;
                    break;
                }
                let st = &game.sim.drive.as_ref().unwrap().state;
                let pos = st.position.truncate();
                let live = race.timing.clock >= 0.0 && game.scenes.shop.is_none();
                let truth = race.route.road.direction(race.route.road.project(pos, Some(race.racers.me.along), 40.0));
                let v = st.velocity.truncate();
                // A U-turn after the car really ran backwards (a
                // rival's knock in 4607 variant 1) keeps the flag until
                // the travel is within 100° of the locator's road
                // (`wrong_way`'s 10° slack), past this test's 72.5°: the
                // second after backward travel is not counted.
                if live && v.length() > 1.0 && v.normalize().dot(truth) < -0.3 {
                    backward_at = Some(frame);
                }
                let turning = backward_at.is_some_and(|b| frame - b < 60);
                if live && race.wrong_way && v.length() > 4.0 && v.normalize().dot(truth) > 0.3 && !turning {
                    false_frames += 1;
                }
                let wrong = race.wrong_way;
                if frame % 30 == 0 {
                    let now = game.now();
                    let texts = nfsu2::frontend::drawn_texts(|| drop(game.hud(now)));
                    if !wrong && texts.iter().any(|(_, s)| s.eq_ignore_ascii_case("wrong way")) {
                        text_frames += 1;
                    }
                }
                let Mode::Race(race) = &game.mode else { break };
                let st = &game.sim.drive.as_ref().unwrap().state;
                // Pure pursuit 10 m ahead on the road, weaving ±5 m.
                t += DT;
                let ahead = race.racers.me.along + 10.0;
                let dir = race.route.road.direction(ahead);
                let target = race.route.road.point_at(ahead) + Vec2::new(-dir.y, dir.x) * 5.0 * (t * 0.7).sin();
                let fwd = st.forward().truncate();
                let to = target - pos;
                let a = to.y.atan2(to.x) - fwd.y.atan2(fwd.x);
                let steer = a.sin().atan2(a.cos());
                let speed = st.velocity.length();

                // Rival contact can turn this simple pursuit bot into a
                // wall. Back out through real player controls instead of
                // spending the rest of the unchanged lap budget pushing.
                let wall = st.contacts.iter().any(|c| c.wall && c.age < 0.2);
                stalled = if live && speed < 1.0 && wall { stalled + DT } else { 0.0 };
                if stalled >= 1.0 && reverse <= 0.0 {
                    reverse = 2.0;
                    stalled = 0.0;
                    recoveries += 1;
                    println!("{name}: reverse recovery {recoveries} at clock {}", race.timing.clock);
                }
                game.platform.keys.clear();
                if live && reverse > 0.0 {
                    reverse -= DT;
                    game.platform.keys.insert(KeyCode::ArrowDown);
                    if steer.abs() > 0.05 {
                        game.platform.keys.insert(if steer > 0.0 { KeyCode::ArrowRight } else { KeyCode::ArrowLeft });
                    }
                } else if live {
                    let limit = if steer.abs() > 0.6 { 12.0 } else { 30.0 };
                    if speed < limit {
                        game.platform.keys.insert(KeyCode::ArrowUp);
                    } else if speed > limit + 3.0 {
                        game.platform.keys.insert(KeyCode::ArrowDown);
                    }
                    if steer.abs() > 0.05 {
                        game.platform.keys.insert(if steer > 0.0 { KeyCode::ArrowLeft } else { KeyCode::ArrowRight });
                    }
                }
                game.advance_for(DT).unwrap();
            }
            game.platform.keys.clear();
            println!(
                "{name}: finished {laps_done}, false wrong way {false_frames} frames, \"Wrong Way\" text {text_frames} times"
            );
            if false_frames > 0 || text_frames > 0 || !laps_done {
                failures.push(name);
            }
            game.mode = Mode::Explore;
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// The race locator (0x5D9E40 → 0x5D5A10 / 0x5774D0) against the
/// original's (car +0x8D8, read under Wine with the car teleported,
/// the maintainer's private capture) on
/// Circuit 4005 v0 (Garibaldi Run) where segment 12 (three nodes 10 m
/// apart) gets its phantom points from its 50–80 m neighbours
/// (0x5E48C0 → 0x5D50A0) and its spline loops back on itself: the
/// same node, parameter, point, driving way and road distance from a
/// fresh locator, the way up to 129° off the route's.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_locator_matches_native_in_a_spline_loop() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let event = game.game_data.career.events.iter().position(|e| e.kind == career::EventKind::Circuit).unwrap();
    {
        let e = &mut game.game_data.career.events[event];
        (e.track, e.variant, e.laps, e.traffic) = (4005, 0, 1, 0);
        e.races.clear();
    }
    game.start_race(event, true, false).unwrap();
    let Mode::Race(race) = &game.mode else { panic!("racing") };
    let road = &race.route.road;
    // Position; native node's place, parameter +0x1C, point +0x10,
    // way +0x28, road distance +0x24.
    let native: [((f32, f32), (f32, f32), f32, (f32, f32), (f32, f32), f32); 8] = [
        ((68.97, 308.86), (70.12, 317.71), 1.856, (69.24, 317.47), (1.000, -0.030), 1876.46),
        ((69.0, 310.0), (70.12, 317.71), 1.856, (69.23, 317.47), (1.000, -0.031), 1876.46),
        ((69.5, 312.0), (70.12, 317.71), 1.867, (69.27, 317.47), (0.999, 0.043), 1876.50),
        ((68.5, 314.0), (70.12, 317.71), 1.838, (69.19, 317.47), (0.982, -0.188), 1876.41),
        ((67.4, 308.3), (60.09, 312.45), 12.131, (64.50, 314.54), (0.907, 0.420), 1870.31),
        ((66.0, 311.0), (60.09, 312.45), 12.126, (64.37, 314.49), (0.907, 0.421), 1870.17),
        ((72.0, 312.0), (70.12, 317.71), 1.987, (69.99, 317.66), (0.943, 0.334), 1877.31),
        ((67.5, 318.0), (70.12, 317.71), 1.582, (69.19, 317.87), (-0.056, -0.998), 1876.36),
    ];
    for (at, node, t, point, way, along) in native {
        let mut loc = Locator::default();
        road.locate(&mut loc, Vec2::new(at.0, at.1));
        let n = road.nodes[loc.node.unwrap()].at;
        assert!(n.distance(Vec2::new(node.0, node.1)) < 0.01, "{at:?}: node at {n:?}");
        assert!((loc.t - t).abs() < 0.002, "{at:?}: t {}", loc.t);
        assert!(loc.point.distance(Vec2::new(point.0, point.1)) < 0.01, "{at:?}: point {:?}", loc.point);
        assert!(loc.direction.distance(Vec2::new(way.0, way.1)) < 0.005, "{at:?}: way {:?}", loc.direction);
        assert!((loc.along - along).abs() < 0.01, "{at:?}: along {}", loc.along);
    }
    // The loop: the way at (67.5, 318) is 96° off the route's.
    let mut loc = Locator::default();
    road.locate(&mut loc, Vec2::new(67.5, 318.0));
    assert!(loc.direction.dot(road.direction(loc.along)) < 0.0);
}

/// The same over the circuits and sprints, one lap each (run by hand,
/// about 8 minutes: `cargo test --release --bin game
/// wrong_way_circuits_and_sprints -- --ignored --nocapture`); the
/// AI racer controller must finish every run.
#[test]
#[ignore = "about 8 minutes: every circuit and sprint with the AI controller"]
fn wrong_way_circuits_and_sprints() {
    testkit::game();
    let (failures, unfinished) = wrong_way_tracks(&[(career::EventKind::Circuit, 1), (career::EventKind::Sprint, 1)]);
    assert!(failures.is_empty() && unfinished.is_empty(), "{failures:?} unfinished {unfinished:?}");
}

/// Runs the AI racer controller once lost its way on (its road node's
/// search after leaving the road took the car's way, not the
/// record's, 0x40F750, and the race clock never reached the junction
/// choice): one lap each with the player's car on the controller,
/// finished and without a false WRONG WAY.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn ai_keeps_the_race_route() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut lost = Vec::new();
    for (kind, track, variant) in [
        (career::EventKind::Circuit, 4002, 0),
        (career::EventKind::Circuit, 4081, 1),
        (career::EventKind::Sprint, 4107, 0),
        (career::EventKind::Sprint, 4103, 1),
    ] {
        let run = wrong_way_run(&mut game, kind, track, variant, 1, None);
        if !run.finished || !run.false_frames.is_empty() {
            lost.push(format!(
                "{kind:?} {track} variant {variant}: {:.2} laps, false wrong way {}",
                run.progress,
                run.false_frames.len()
            ));
        }
    }
    assert!(lost.is_empty(), "{lost:?}");
}

/// A full race with its opponents and the player's car on the AI racer
/// controller: each rival's lowest progress below its best (laps) and
/// whether it finished within 400 s.
fn rivals_run(game: &mut Game, kind: career::EventKind, track: u16, variant: u8, laps: u8) -> Vec<(String, f32, bool)> {
    let event = game.game_data.career.events.iter().position(|e| e.kind == kind).unwrap();
    {
        let e = &mut game.game_data.career.events[event];
        (e.track, e.variant, e.laps, e.traffic) = (track, variant, laps, 0);
        e.races.clear();
    }
    game.start_race(event, true, false).unwrap();
    while game.scenes.shop.is_some() && matches!(game.mode, Mode::Race(_)) {
        game.advance_for(1.0 / 30.0).unwrap();
    }
    for _ in 0..(COUNTDOWN * 30.0) as u32 + 2 {
        game.advance_for(1.0 / 30.0).unwrap();
    }
    {
        let (Mode::Race(race), Some(d)) = (&mut game.mode, game.sim.drive.as_ref()) else { panic!("racing") };
        race.racers.traffic = None;
        race.autopilot =
            Some(Autopilot::new(d, race.racers.ai_routes.as_ref().unwrap(), d.car.car_type.handling.race.drift()));
    }
    let mut worst: Vec<(f32, f32)> = Vec::new();
    // 480 s: Circuit 4083's two laps take a rival ~390 s, more when it
    // backs out of the hairpin at (−1510, 740) (its GetUnstuck), and
    // the start scene's roll-in shifts where the racers meet it.
    for _ in 0..(480 * 30) {
        let Mode::Race(race) = &mut game.mode else { break };
        race.replay.recording = None;
        // The race ends 3 s after the player's finish (the AI racers
        // still running then are ended where they are); hold it open
        // to watch the rivals drive their whole race.
        race.over = -1.0e9;
        worst.resize(race.racers.rivals.len(), (f32::MIN, 0.0));
        for (w, r) in worst.iter_mut().zip(&race.racers.rivals) {
            let p = r.progress.progress();
            w.0 = w.0.max(p);
            w.1 = w.1.max(w.0 - p);
        }
        if race.racers.rivals.iter().all(|r| r.progress.finished.is_some()) {
            break;
        }

        game.advance_for(1.0 / 30.0).unwrap();
    }
    let Mode::Race(race) = &game.mode else { return Vec::new() };
    race.racers.rivals.iter().zip(&worst).map(|(r, w)| (r.name.clone(), w.1, r.progress.finished.is_some())).collect()
}

/// Test isolation (docs/test-determinism.md): a game test starts from
/// nothing another test left. Another test's thread commits and saves a
/// profile in another car and writes options.txt; a game built here
/// after it still has no profile, the default car and options, and the
/// other test sees its own. (`Game::new` read the process-wide current
/// profile and data folder: a race test drove another test's car — the
/// flaky `rivals_keep_the_race_route` / `wrong_way_street_x`.)
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn game_tests_start_from_their_own_sandbox() {
    testkit::game();
    let other = std::thread::spawn(|| {
        let mut game = Game::new().unwrap();
        let mut p = Profile::new("FOREIGN");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        p.save().unwrap();
        game.settings.difficulty = 2;
        game.settings.save().unwrap();
        let again = Game::new().unwrap();
        (again.profile.map(|p| p.car), again.settings.difficulty, profiles::data_home())
    });
    let (car, difficulty, home) = other.join().unwrap();
    assert_eq!((car.as_deref(), difficulty), (Some("PEUGOT"), 2), "a test keeps its own saves");
    let game = Game::new().unwrap();
    assert!(
        game.profile.is_none() && profiles::current_name().is_none(),
        "no other test's profile: {:?}",
        game.profile.as_ref().map(|p| p.name.clone())
    );
    assert_eq!(
        (game.car_key(0).0.as_str(), game.settings.difficulty),
        ("240SX", options::Settings::default().difficulty)
    );
    assert_ne!(profiles::data_home(), home, "a data folder per test");
    assert!(!home.exists(), "the other test's folder is gone with its thread");
}

/// Probe (docs/test-determinism.md): the race-route checks started in
/// a profile another test leaves current (the community saves the
/// shop tests load), as the shared state let them start before the
/// test sandbox. Prints, asserts nothing.
#[test]
#[ignore = "probe: the race checks in another test's profile"]
fn race_checks_in_a_foreign_profile_probe() {
    testkit::game();
    let saves: [(&str, &[u8]); 2] = [
        (
            "community_body_rx7",
            &testkit::private("tests/fixtures/native_saves/community_body_rx7.sav"),
        ),
        (
            "stage5_start",
            &testkit::private("tests/fixtures/native_saves/stage5_start.sav"),
        ),
    ];
    for (name, bytes) in saves {
        profiles::set_current("Other", nfsu2::formats::native_save::NativeSave::parse(bytes).unwrap());
        let mut game = Game::new().unwrap();
        println!("{name}: profile {:?}", game.profile.as_ref().map(|p| (&p.name, &p.car, p.stage, p.perf.len())));
        for (kind, track, variant, laps) in [
            (career::EventKind::Circuit, 4002, 0, 2),
            (career::EventKind::Circuit, 4083, 1, 2),
            (career::EventKind::Sprint, 4107, 0, 1),
            (career::EventKind::Sprint, 4122, 1, 1),
        ] {
            let rivals = rivals_run(&mut game, kind, track, variant, laps);
            let lost: Vec<_> = rivals.iter().filter(|r| !r.2 || r.1 > 0.05).collect();
            println!("{name}: {kind:?} {track} variant {variant}: lost {lost:?} of {rivals:?}");
        }
        drop(game);
        let (failures, unfinished) = wrong_way_tracks(&[(career::EventKind::StreetX, 2)]);
        println!("{name}: wrong way street x: failures {failures:?} unfinished {unfinished:?}");
        profiles::clear_current();
    }
}

/// The opponents keep the race route: in full races on tracks the AI
/// once lost its way on, every rival finishes and none drives back
/// along the road by more than a twentieth of a lap.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rivals_keep_the_race_route() {
    testkit::game();
    let mut game = Game::new().unwrap();
    println!("profile {:?}", game.profile.as_ref().map(|p| (&p.name, &p.car, p.stage, p.perf.len())));
    let mut lost = Vec::new();
    for (kind, track, variant, laps) in [
        (career::EventKind::Circuit, 4002, 0, 2),
        (career::EventKind::Circuit, 4083, 1, 2),
        (career::EventKind::Sprint, 4107, 0, 1),
        (career::EventKind::Sprint, 4122, 1, 1),
    ] {
        let rivals = rivals_run(&mut game, kind, track, variant, laps);
        println!("{kind:?} {track} variant {variant}: {rivals:?}");
        assert!(!rivals.is_empty(), "no rivals in {kind:?} {track}");
        for (name, back, finished) in rivals {
            if !finished || back > 0.05 {
                lost.push(format!(
                    "{kind:?} {track} variant {variant}: {name} finished {finished}, back {back:.2} laps"
                ));
            }
        }
    }
    assert!(lost.is_empty(), "{lost:?}");
}
