//! The paused traffic car's coast, driver event 4 (docs/notes/parity/race-ai-parity.md "The paused car coasts").

use crate::*;

/// Native (**verified**): a car the placer cannot place is paused
/// (0x42FAAB → 0x4020D0: mode 0, inactive, +0x77D) with its walk and its
/// driver's path kept; its controller update 0x429D90 returns at once, but
/// the world update 0x609CF0 runs its driver (0x421030) every frame, and
/// when the world under it loads (0x5F6CE0 → 0x408C40 → 0x5F46B0(0x5F4410))
/// it takes the TrafficMover and coasts on the old path, still paused and
/// inactive; a car paused in mode 0 with no change of the world under it
/// moves nothing (0x50D510). **Observed** under Wine
/// (`docs/evidence/traf-3/native-paused-coast.txt`: car +0x77D set, +0x550
/// clear, mode 0 at a fixed place for 2.6 s, then mode 2 at its last speed
/// 20.39 m/s along its walk, then mode 4 when the world unloads).
///
/// The situation is made directly, not waited for: Quick Race 4014 at
/// traffic 3, throttle only, and 8 s after the green two placed traffic cars
/// that are moving on their lanes are paused as the placer pauses them
/// (`pause_traffic`, with their walk and path); for one the world under it
/// is marked as not loaded the frame before, so the next stream step sees
/// it load (+0x5C0 0 → 1), the other keeps its world (mode 0). Then 5 s.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_parked_car_coasts_inactive_on_its_old_walk() {
    let mut game = Game::new().unwrap();
    game.settings.qr[0].traffic = 3;
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::QuickRace, MenuKind::Main).unwrap();
    game.act(Action::QrMode(screens::QrMode::Circuit), MenuKind::QrMode).unwrap();
    for _ in 0..3 {
        game.menu_key(Key::Accept).unwrap();
    }
    if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
        game.menu_key(Key::Accept).unwrap();
    }
    assert!(matches!(game.mode, Mode::Race(_)), "no race ({})", mode_name(&game.mode));
    let step = |game: &mut Game| {
        game.platform.keys.insert(KeyCode::ArrowUp);
        game.update(1.0 / 60.0).unwrap();
    };
    // To the green and 8 s on.
    let mut after_green = 0;
    for _ in 0..(120 * 60) {
        step(&mut game);
        let Mode::Race(rc) = &game.mode else { panic!("the race ended") };
        if rc.racers.traffic.as_ref().is_some_and(|t| t.cars.iter().any(|c| c.lane.is_some())) && rc.timing.clock >= 0.0 {
            after_green += 1;
        }
        if after_green >= 8 * 60 {
            break;
        }
    }
    assert!(after_green >= 8 * 60, "no green with placed traffic");
    // Pause two moving, placed cars on loaded world.
    let (coaster, parked) = {
        let Mode::Race(rc) = &mut game.mode else { unreachable!() };
        let t = rc.racers.traffic.as_mut().expect("traffic");
        let mut pick = t
            .cars
            .iter()
            .enumerate()
            .filter(|(_, c)| c.lane.is_some() && c.plan.is_some() && c.state.streamed && c.mover.is_some())
            .filter(|(_, c)| c.mover.is_some_and(|m| m.speed > 5.0))
            .map(|(i, _)| i);
        let (a, b) = (pick.next().expect("a moving placed traffic car"), pick.next().expect("a second one"));
        for i in [a, b] {
            let c = &mut t.cars[i];
            let (lane, plan) = (c.lane.take(), c.plan.take());
            crate::sim::traffic::think::pause_traffic(c, lane, plan);
        }
        // The world under the first loads at the next stream step.
        t.cars[a].state.streamed = false;
        (a, b)
    };
    let (mut coasting, mut moved, mut frozen) = (0usize, 0usize, 0usize);
    let mut last: [Option<Vec3>; 2] = [None; 2];
    for _ in 0..(5 * 60) {
        step(&mut game);
        let Mode::Race(rc) = &game.mode else { break };
        let Some(t) = rc.racers.traffic.as_ref() else { break };
        for (k, i) in [coaster, parked].into_iter().enumerate() {
            let c = &t.cars[i];
            if !c.held {
                last[k] = None; // placed again (0x417BB0 retries a parked car): the episode ended
                continue;
            }
            // Paused: never active, never a placed car (no road entry,
            // no collision body, not drawn).
            assert!(!c.active && c.lane.is_none(), "car {i}: a paused car is inactive and unplaced");
            let p = c.state.position;
            if k == 0 && c.coast.is_some() && c.mover.is_some() {
                coasting += 1;
                moved += usize::from(last[k].is_some_and(|q| q.distance(p) > 0.05));
            } else if k == 1 && c.coast.is_some() && c.mover.is_none() && c.belt.belt.is_none() && c.state.melo.is_none() {
                // Mode 0 moves nothing.
                if let Some(q) = last[k] {
                    assert!(q.distance(p) < 1e-4, "car {i}: mode 0 moved {} m", q.distance(p));
                    frozen += 1;
                }
            }
            last[k] = Some(p);
        }
    }
    println!("paused: {coasting} coasting car-frames ({moved} moving), {frozen} in mode 0");
    // How long the coaster's episode lasts depends on when the placer
    // tries it again (its slot and the random draws before): half a
    // second of coasting shows the behaviour.
    assert!(coasting > 30 && moved * 2 > coasting && frozen > 60, "{coasting} / {moved} / {frozen}");
}

/// Native (**verified**): 0x420D40 plans the racer's path afresh
/// with 0x41D160(0, 0) (0x420DCA / 0x420E97), which empties the action's
/// path and stamps its +0x1E0 with the tick (0x865188); 0x420810, handing
/// the path to the driver, finds the stamp equal and posts driver +0x250 =
/// 4 (0x4208CA), which the driver's next update reads (no tracker rate).
/// A kept path (points behind dropped, cut, extended) posts nothing.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_path_planned_afresh_posts_driver_event_4() {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("Event4");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let routes = game.sim.encounters.as_ref().unwrap().routes.clone();
    let (rec, cp) = routes
        .file
        .records
        .iter()
        .enumerate()
        .find_map(|(i, r)| (r.points.len() > 8).then_some((i, 2)))
        .unwrap();
    let position = routes.file.records[rec].points[cp].position;
    let direction = routes.file.direction(rec, cp, false);
    let mut route = nfsu2::ai::route::RacerRoute::start(&routes, position, direction).expect("a route");
    let fwd = direction.extend(0.0);
    assert!(!route.take_replanned());
    // The first path: nothing to keep, planned from the car.
    let path = route.line_path(&routes, 0.05, position, fwd, 15.0, false, false, 0.0);
    assert!(!path.is_empty());
    assert!(route.take_replanned(), "a first path is planned afresh");
    assert!(!route.take_replanned(), "taken once");
    // The same place a moment on: the path is kept.
    let on = position + fwd * 0.2;
    route.line_path(&routes, 0.05, on, fwd, 15.0, false, false, 0.0);
    assert!(!route.take_replanned(), "a kept path posts nothing");
    // GetUnstuck's first path (0x420940: 0x41D160(0, 0)) is planned afresh.
    route.line_path(&routes, 0.05, on, fwd, 15.0, false, true, 0.0);
    assert!(route.take_replanned());
    // The driver takes event 4 as one update without the trackers' rate.
    let mut driver = nfsu2::ai::driver::AiDriver::new(
        &game.sim.drive.as_ref().unwrap().car.car_type.handling,
        false,
        1.0,
    );
    driver.event = 4;
    let d = game.sim.drive.as_ref().unwrap();
    let input = nfsu2::ai::driver::DriverInputs {
        path: &path,
        target_speed: 15.0,
        enabled: true,
        green: true,
        countdown: false,
        since_green: 10.0,
        clock: 10.0,
    };
    driver.update(&d.state, &d.car.car_type.handling, false, &input, 1.0 / 60.0);
    assert_eq!(driver.event, 0, "event 4 is read and cleared by the next update");
}
