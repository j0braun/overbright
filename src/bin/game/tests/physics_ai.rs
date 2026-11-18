use crate::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn traffic_pose_exports_mover_speed_not_motion_projection() {
    let root = testkit::game_root();
    let car = Car::load(std::path::Path::new(&root), "240SX").unwrap();
    let world = World::flat(2000.0);
    let mut state = CarState::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
    let before = (state.position, state.heading());
    place_on_road(&world, &car, &mut state, (Vec2::new(1.0, 2.0), 1.0), 0.0, 12.5, before, 1.0 / 60.0);
    assert_eq!(state.exported_forward_speed(), 12.5);
    assert!(
        (state.speed() - 12.5).abs() > 1.0,
        "existing ground/translation approximation must distinguish parameter from projection"
    );
}

/// The stage-4 drag's frame steps (native ticks / 1800 s) summed
/// in f32 land past 1.0 at frame 60, where the original's race clock
/// reads exactly 4000 ticks; HEAD START's "more than 1 s" (0x56F730)
/// must not fire there (natively it fired at frame 61).
#[test]
fn head_start_reads_whole_ticks() {
    let ticks = [30u32, 29, 31, 30, 29, 31, 30, 30, 29, 31];
    let mut clock = 0.0f32;
    for k in 0..60 {
        clock += ticks[k % ticks.len()] as f32 / 1800.0;
    }
    assert!(clock > 1.0, "the f32 sum overshoots: {clock}");
    assert_eq!(whole_ticks(clock), 1.0);
    assert!(whole_ticks(clock + 1.0 / 60.0) > 1.0);
    assert_eq!(whole_ticks(1.000_000_1), 1.0);
}

/// The nitrous sound at high speed must stop after slowing down: a shot started with the tank over
/// 1.5 (0x4850A1) asks for GB_NOS, an 89.5 s NIS stream; the shot's end
/// stops the NIS stream at once (0x484FAB..0x484FFE, 0x7227CB), so it
/// sounds only while the shot runs. Free roam, the throttle 1 s, then
/// the nitrous 1.5 s, then both let go and the brake. Real assets, no
/// GPU.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn gb_nos_stream_ends_with_the_shot() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("GB_NOS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    // A 4 s tank filled to twice its size (0x592650: up to 2 tanks).
    let mut tank = nfsu2::nitrous::Tank::new(4.0, 1);
    tank.refill(1.0);
    assert!(tank.fraction() > 1.5);
    game.sim.drive.as_mut().unwrap().state.nitrous = Some(tank);
    let gb_nos = |game: &Game| {
        let a = game.audio.as_ref().unwrap();
        a.streams.nis_kind == 1 && a.streams.nis_busy(&a.mixer)
    };
    let (mut started, mut ended_at, mut shot_end) = (None, None, None);
    for i in 0..8 * 60 {
        game.platform.keys.clear();
        if i < 150 {
            game.platform.keys.insert(KeyCode::ArrowUp);
            if i >= 60 {
                game.platform.keys.insert(KeyCode::AltLeft);
            }
        } else {
            game.platform.keys.insert(KeyCode::ArrowDown);
        }
        game.step_for(1.0 / 60.0).unwrap();
        game.audio.as_ref().unwrap().mixer.pump(800, &mut Vec::new());
        let stage = game.sim.drive.as_ref().unwrap().state.drivetrain.as_ref().map_or(0, |t| t.nitrous);
        if gb_nos(&game) {
            started.get_or_insert(i);
        } else if started.is_some() {
            ended_at.get_or_insert(i);
        }
        if started.is_some() && stage == 0 {
            shot_end.get_or_insert(i);
        }
    }
    let started = started.expect("GB_NOS asked for at the shot's start (tank over 1.5)");
    assert!((60..90).contains(&started), "GB_NOS started at frame {started}");
    let (ended_at, shot_end) =
        (ended_at.expect("GB_NOS still playing 5 s after the shot (89.5 s stream)"), shot_end.unwrap());
    assert_eq!(ended_at, shot_end, "the NIS stream stops in the frame the shot ends");
    let a = game.audio.as_ref().unwrap();
    assert!(a.car_sfx.nitrous_field() == 0 && !a.streams.nis_busy(&a.mixer));
}

/// Free roam traffic over a minute with the player standing at the
/// free-roam start: the cars on their TrafficMover keep to their lanes'
/// centres (outside "Avoid Cars", whose lane changes move them on
/// purpose). On the racers' driver mode they weaved, median 2.0 m off
/// with 67% of car-frames over 1.5 m; on the TrafficMover's mode 3
/// (0x421030, 0x415F80) about 0.2 m and 0.4%.
/// A Drive on `region` with `car` and nothing drawn, as `drive_in`
/// builds it (no traffic, no encounters).
fn headless_drive(root: &std::path::Path, region: &str, car: &str) -> Drive {
    let world = World::load(root, region).unwrap();
    let car = Car::load(root, car).unwrap();
    let chase = ChaseCamera::new(&car, VIEW_ORDER[0]);
    let state = CarState::new(Vec3::ZERO, 0.0);
    let camera = chase.clone().update(&state, 0.0, nfsu2::qol::fov_offset());
    let mut d = Drive {
        region: region.into(),
        world,
        car,
        car_id: CarId::new(),
        state,
        chase,
        fov_offset: nfsu2::qol::fov_offset(),
        camera,
        fog: FogState::default(),
        tint: ScreenTintState::default(),
        screen_flash: Default::default(),
        horizon_fog: Default::default(),
        motion: Default::default(),
        time_of_day: TimeOfDay::default(),
        clock: 1.0,
        world_clock: Default::default(),
        weather: nfsu2::weather::Weather::new(),
        human_road: Default::default(),
        event_mover: 0,
        node_counts: (0, 0),
        props: Props::default(),
        posed: Default::default(),
        effects: Effects::default(),
        barriers: BarrierSetup::default(),
        flare_mirrors: Default::default(),
        flare_pools: Default::default(),
        flare_pools_clock: 0.0,
        vis: Default::default(),
        flare_lights: Vec::new(),
        hydro: Default::default(),
        placed_stamp: None,
        roam: Default::default(),
        impact: Default::default(),
    };
    d.props = Props::new(&d.world);
    d
}

/// Driving parity (the maintainer's private notes): the original's 240SX slalom
/// at 60 mph, logged under Wine at 31–90 frames a second (wined3d),
/// replayed frame by frame from the same state with its pedals and
/// steering input. SPEED2.EXE runs one world update per frame (main
/// loop 0x5811D0 → 0x609CF0), so the player's steering (0x416BB0,
/// running means 0x576AB0) runs once a frame; the clone's former split
/// of every frame into 1/60 s steps ran it twice on frames over 16.7 ms
/// and steered ~10 % slower (5.2° of heading off after 2 s, 2.7 m after
/// 4 s; now 1.2° / 0.8 m).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn slalom_steers_like_the_original_at_low_frame_rates() {
    let root = testkit::game_root();
    let mut d = headless_drive(std::path::Path::new(&root), "L4RA", "240SX");
    let frames =
        &testkit::private_str("docs/evidence/drive-parity/240sx-slalom-50fps.frames");
    let expected_csv = testkit::private_str("docs/evidence/drive-parity/240sx-slalom-50fps.csv");
    let expected: Vec<Vec<&str>> =
        expected_csv
            .lines()
            .skip(1)
            .map(|l| l.split(',').collect())
            .collect();
    let mut k = 0;
    let (mut worst_steer, mut heading_2s, mut dist_4s) = (0.0f32, None, None);
    for line in frames.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            Some("sync") => {
                // The steering's running means (0.55 s / 0.15 s) need a
                // history of idle frames, as the replay's car had: drive
                // a second untouched first, then take the logged state.
                let v: Vec<f32> = f[1..].iter().map(|x| x.parse().unwrap()).collect();
                apply_replay_sync(&mut d, &v);
                for _ in 0..50 {
                    step_explore(&mut d, None, None, Controls::default(), 0.02, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
                }
                apply_replay_sync(&mut d, &v);
            }
            Some("f") => {
                let dt: f32 = f[1].parse().unwrap();
                let v: Vec<f32> = f[3..8].iter().map(|x| x.parse().unwrap()).collect();
                let controls = Controls {
                    throttle: v[0],
                    brake: v[1],
                    handbrake: v[2],
                    steer: -v[3],
                    nitrous: v[4] != 0.0,
                    ..Default::default()
                };
                step_explore(&mut d, None, None, controls, dt, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
                let e = &expected[k];
                let num = |i: usize| e[i].parse::<f32>().unwrap();
                let t_of = |e: &Vec<&str>| e[1].parse::<f32>().unwrap();
                // The wheel angle this frame steered with (the original's input update runs
                // before its mover call, which the logger reads).
                if t_of(e) < 0.6 {
                    worst_steer = worst_steer.max((d.state.steer_degrees - num(4)).abs());
                }
                let t = num(1);
                let fwd = d.state.forward();
                let heading = fwd.y.atan2(fwd.x).to_degrees();
                let cm = d.state.position
                    + glam::Mat3::from_quat(d.state.orientation) * Vec3::from(d.car.car_type.handling.centre_of_mass);
                if heading_2s.is_none() && t >= 2.0 {
                    heading_2s = Some(((heading - num(8) + 180.0).rem_euclid(360.0) - 180.0).abs());
                }
                if dist_4s.is_none() && t >= 4.0 {
                    dist_4s = Some(cm.distance(Vec3::new(num(5), num(6), num(7))));
                }
                k += 1;
            }
            _ => {}
        }
    }
    assert_eq!(k, expected.len());
    // First 0.6 s of steering (before the speeds part): within 1°.
    assert!(worst_steer < 1.0, "steering angle off by {worst_steer}°");
    let (h, p) = (heading_2s.unwrap(), dist_4s.unwrap());
    assert!(h < 2.0, "heading off by {h}° after 2 s (old 1/60 s split: 5.2°)");
    assert!(p < 1.5, "position off by {p} m after 4 s (old 1/60 s split: 2.7 m)");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn world_acceleration_outer_frames_include_real_contact() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("AccelerationTest");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.explore(None).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    // Drive::step has multiple low-level world calls, but publishes
    // the complete variable-duration interval exactly once.
    for dt in [0.05, 1.0 / 60.0, 0.03125] {
        let sample = d.state.begin_world_update(dt);
        d.step(Controls { throttle: 1.0, ..Default::default() }, dt, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
        let actual = d.state.world_acceleration;
        let mut expected = d.state.clone();
        expected.finish_world_update(sample);
        assert_eq!(actual, expected.world_acceleration);
    }
    let e = game.sim.encounters.as_mut().unwrap();
    e.ready();
    for c in &mut e.cars {
        c.active = false;
    }
    let r = &mut e.cars[0];
    r.active = true;
    r.state = CarState::placed(&r.car.car_type, &d.world, d.state.position + Vec3::X * 2.0, std::f32::consts::PI);
    r.route = nfsu2::ai::route::RacerRoute::start(&e.routes, r.state.position, r.state.forward().truncate());
    r.state.velocity = r.state.forward() * 100.0;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    r.state.publish_forward_speed();
    r.state.world_acceleration = Vec3::new(11.0, 22.0, 33.0);
    let pending = r.state.begin_world_update(0.05);
    let pre_reset_velocity = r.state.velocity;
    assert!(reset_encounter(&d.world, r, &e.routes, 1));
    let placed_speed = r.state.exported_forward_speed();
    assert!((0.0..=RESET_SPEED).contains(&placed_speed));
    assert_eq!(
        r.state.velocity,
        r.state.forward() * (placed_speed + 0.1),
        "native placement retains its argument separately from the +0.1 velocity"
    );
    assert_eq!(r.state.world_acceleration, Vec3::ZERO, "59F890 clears body acceleration during recovery placement");
    let mut in_flight = r.state.clone();
    in_flight.finish_world_update(pending);
    assert_eq!(
        in_flight.world_acceleration,
        (r.state.velocity - pre_reset_velocity) * (1.0f32 / 0.05),
        "a previously captured interval remains independent of reset state"
    );
    let zero = r.state.begin_world_update(0.0);
    r.state.finish_world_update(zero);
    assert_eq!(r.state.world_acceleration, Vec3::ZERO);
    let after_reset = r.state.begin_world_update(0.05);
    step_explore(d, None, Some(e), Controls::default(), 0.05, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    let r = &mut e.cars[0];
    let mut expected = r.state.clone();
    expected.finish_world_update(after_reset);
    assert_eq!(
        r.state.world_acceleration, expected.world_acceleration,
        "controller-side reset must precede the outer velocity snapshot"
    );
    // Then force a real head-on car-pair contact inside a new interval.
    r.state = CarState::placed(&r.car.car_type, &d.world, d.state.position + Vec3::X * 2.0, std::f32::consts::PI);
    r.state.velocity = -Vec3::X * 20.0;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    r.state.publish_forward_speed();
    d.state.velocity = Vec3::X * 20.0;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    d.state.publish_forward_speed();
    // The optional phase hook sees initial motion before contact
    // impulses and is not repeated by collision re-simulation.
    let (mut probe_me, mut probe_rival) = (d.state.clone(), r.state.clone());
    let mut probe_field = nfsu2::drive::CarField::default();
    probe_field.bind(&[0, 1]);
    let mut calls = [0; 2];
    nfsu2::drive::step_world_with_post_motion(
        &mut [
            nfsu2::drive::Racer {
                car: &d.car,
                state: &mut probe_me,
                input: nfsu2::drive::Input::Player(Controls::default()),
            },
            nfsu2::drive::Racer { car: &r.car, state: &mut probe_rival, input: nfsu2::drive::Input::Ai(r.controls) },
        ],
        &mut probe_field,
        &mut nfsu2::smackable::Props::default(),
        &d.world,
        0.05,
        &mut Default::default(),
        |i, car| {
            calls[i] += 1;
            assert_eq!(car.state.hardest_impulse, 0.0, "motion hook precedes contact impulses");
        },
    );
    assert_eq!(calls, [1, 1]);
    assert!(!probe_field.partners(0).is_empty(), "hook fixture reaches car contacts afterward");
    let human_sample = d.state.begin_world_update(0.05);
    let rival_sample = r.state.begin_world_update(0.05);
    step_explore(d, None, Some(e), Controls::default(), 0.05, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    let r = &e.cars[0];
    assert!(!e.field.partners(0).is_empty(), "fixture reached the real car-pair contact path");
    for (state, sample) in [(&d.state, human_sample), (&r.state, rival_sample)] {
        let mut expected = state.clone();
        expected.finish_world_update(sample);
        assert_eq!(state.world_acceleration, expected.world_acceleration);
    }
    assert!(d.state.world_acceleration.length() > 1.0);
    d.state.velocity = Vec3::ZERO;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    d.state.publish_forward_speed();
    d.state.world_acceleration = Vec3::new(-3.0, 4.0, 12.0);
    let first = road_car(0, &d.car, &d.state, None, true, true, 0.0);
    let stale = road_car(0, &d.car, &d.state, Some(Vec3::splat(100.0)), true, true, 0.2);
    assert_eq!(first.accel, 13.0, "stationary native road entries retain acceleration magnitude");
    assert_eq!(first.accel, stale.accel, "controller history and cadence do not redefine world acceleration");
    assert_eq!(first.forward_accel, d.state.forward_acceleration(), "own-forward projection remains separate");
    let r = &mut e.cars[0];
    r.state.velocity = r.state.forward() * 40.0;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    r.state.publish_forward_speed();
    r.recovery_pending = true;
    r.controller.suspended = true;
    let sample = r.state.begin_world_update(0.05);
    let before_motion = r.state.velocity;
    let mut trace = Vec::new();
    step_explore_traced(d, None, Some(e), Controls::default(), 0.05, Some(&mut trace), &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    assert_eq!(trace.len(), 1, "recovery runs once in the complete outer interval");
    assert_eq!(trace[0].0 + 1, trace[0].1, "recovery waits until final subdivision motion");
    assert_eq!(trace[0].1, 1, "one world step per frame (main loop 0x5811D0)");
    assert_ne!(trace[0].2, before_motion, "mover motion precedes request consumption");
    let r = &e.cars[0];
    assert!(!r.recovery_pending, "physics consumes a retained recovery request");
    assert!(!r.controller.suspended, "physics placement resets controller lifecycle");
    let mut expected = r.state.clone();
    expected.finish_world_update(sample);
    assert_eq!(
        r.state.world_acceleration, expected.world_acceleration,
        "recovery velocity change belongs inside the outer physics interval"
    );
    let mut trace = Vec::new();
    step_explore_traced(d, None, Some(e), Controls::default(), 0.025, Some(&mut trace), &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    assert!(
        trace.is_empty(),
        "consumed requests do not repeat on the next world update"
    );
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drift_results_use_scores_despite_opposite_finish_order() {
    testkit::game();
    for (scores, finishes, expected_order, won) in [
        ([100.0, 400.0, 300.0, 200.0], [10.0, 20.0, 30.0, 40.0], [1, 2, 3, 0], false),
        ([400.0, 100.0, 300.0, 200.0], [40.0, 10.0, 20.0, 30.0], [0, 2, 3, 1], true),
    ] {
        let mut game = Game::new().unwrap();
        let mut profile = Profile::new("DriftOrderTest");
        profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        let event = game
            .game_data.career
            .events
            .iter()
            .position(|e| e.kind == career::EventKind::Drift && e.opponents.len() == 3 && e.races.len() == 1)
            .unwrap();
        let event_id = game.game_data.career.events[event].index;
        profile.completed.remove(&event_id);
        profile.filed.remove(&event_id);
        game.profile = Some(profile);
        game.start_race(event, false, false).unwrap();
        game.scenes.shop = None;
        let expected_rows = {
            let Mode::Race(race) = &mut game.mode else { panic!("drift race") };
            assert!(!race.quick && race.test.is_none());
            assert_eq!(race.racers.rivals.len(), 3);
            assert_eq!(race.scoring.board.len(), 4);
            race.timing.clock = 50.0;
            race.over = 100.0;
            race.racers.me.finished = Some(finishes[0]);
            for (i, rival) in race.racers.rivals.iter_mut().enumerate() {
                rival.progress.finished = Some(finishes[i + 1]);
            }
            // Controlled already-banked scores: no teleports, ongoing
            // drift chains, or assumptions about emergent AI performance.
            for (i, style) in
                std::iter::once(&mut race.scoring.style).chain(race.racers.rivals.iter_mut().map(|r| &mut r.style)).enumerate()
            {
                let style = style.as_mut().unwrap();
                let drift = style.tricks[nfsu2::drift::DRIFT].as_mut().unwrap();
                drift.banked = scores[i];
                drift.state = nfsu2::drift::TrickState::Idle;
                drift.points = 0.0;
                style.total = scores[i];
            }
            let progress: Vec<_> = std::iter::once(&race.racers.me).chain(race.racers.rivals.iter().map(|r| &r.progress)).collect();
            assert!(progress.iter().all(|p| p.dropped.is_none() && p.ended.is_none()));
            assert_eq!(progress.iter().map(|p| p.finished.unwrap()).collect::<Vec<_>>(), finishes);
            assert_eq!(race.racers.me.finish_order(race.racers.rivals.iter().map(|r| &r.progress)), if won { 4 } else { 1 });
            assert_eq!(scores[0] > scores[1..].iter().copied().reduce(f32::max).unwrap(), won);
            race.refresh_board();
            assert_eq!(race.scoring.board.iter().map(|b| b.racer.unwrap()).collect::<Vec<_>>(), expected_order);
            // Expectations come from the supplied racer IDs/scores, not
            // from whatever order refresh_board happened to produce.
            expected_order
                .iter()
                .enumerate()
                .map(|(place, &id)| {
                    let row = race.scoring.board.iter().find(|b| b.racer == Some(id)).unwrap();
                    (row.name.clone(), row.car.clone(), scores[id], screens::race_points(place + 1))
                })
                .collect::<Vec<_>>()
        };
        game.advance_for(1.0 / 60.0).unwrap();
        let Mode::Menu(Menu::Results(results, _)) = &game.mode else { panic!("production results transition") };
        assert!(results.scores);
        assert_eq!(results.rows, expected_rows);
        assert_eq!(results.player, Some(if won { 0 } else { 3 }));
        let result = game.race_flow.result.as_ref().expect("career win/loss dispatched");
        assert_eq!(result.event, event_id);
        assert_eq!(result.won, won);
        assert_eq!(game.profile.as_ref().unwrap().filed.contains(&event_id), won);
    }
}

/// The world clock (0x7FB718) and the sky cycle (0x60FAB0) run through
/// free roam with its traffic and through a race, and each world load
/// starts them again at 1.0 s (0x576D00 from the world object's ctor
/// 0x5FF4A0 and dtor 0x5FF700). Under Wine: the
/// sunrise pair until clock 60, night from about 81 s; resuming a career
/// read 1.93 s on its first in-world frame. The clone had frozen the
/// clock whenever traffic ran (`step_explore`'s field path) and in races.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn world_clock_runs_in_free_roam_and_races() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("Player");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.explore(None).unwrap();
    let start = game.sim.drive.as_ref().unwrap().clock;
    assert_eq!(game.sim.drive.as_ref().unwrap().world_clock.update, 0);
    assert_eq!(game.sim.drive.as_ref().unwrap().world_clock.tick, 4000);
    assert!((start - 1.0).abs() < 0.01, "a load starts the clock at 1 s: {start}");
    // 90 s of free roam on the tick cadence: 5400 race ticks of
    // 1/60 s (30, 29, 31 of 1/1800 s).
    let mut ticks = 0;
    for _ in 0..(90 * 30) {
        ticks += game.advance_for_each(1.0 / 30.0, |_, _| Ok(())).unwrap();
    }
    assert_eq!(ticks, 90 * 60, "two race ticks per 1/30 s");
    assert!(
        game.sim.traffic.as_ref().is_some_and(|t| t.cars.iter().any(|c| c.lane.is_some())),
        "free roam ran with traffic"
    );
    let d = game.sim.drive.as_ref().unwrap();
    assert_eq!(d.world_clock.update, ticks as i32, "one scheduler update per world update");
    // The 1/4000 s clock: each tick adds its dt rounded (67, 64, 69 over
    // a 30, 29, 31 cycle = 200), so 90 s of ticks are 360 000 within the
    // rounding of the cycle's phase.
    assert!(d.world_clock.tick.abs_diff(4000 + 90 * 4000) <= 2, "the 1/4000 s clock after 90 s: {}", d.world_clock.tick);
    assert!((d.clock - 91.0).abs() < 0.5, "free-roam clock after 90 s: {}", d.clock);
    assert_eq!(d.time_of_day.sky(), nfsu2::sky::SkyFrame { set: 2, morph: None }, "night after the first fade");
    assert_eq!(d.time_of_day.fog_weight(), 0.0);
    // A race loads the world again: the clock starts over, then runs.
    let race = game
        .game_data.career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| {
            e.kind == career::EventKind::Circuit
                && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
        })
        .map(|e| e.index)
        .unwrap();
    game.start_race(race, false, false).unwrap();
    let loaded = game.sim.drive.as_ref().unwrap().clock;
    let scheduler_loaded = game.sim.drive.as_ref().unwrap().world_clock.clone();
    assert!(loaded < 2.0, "a race load restarts the clock: {loaded}");
    // 120 s: the start scene's 1/30 s ticks, then the race's 1/60 s ones.
    let (mut raced, mut ticks) = (0, 0);
    for _ in 0..(120 * 30) {
        ticks += game.advance_for_each(1.0 / 30.0, |_, _| Ok(())).unwrap();
        if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.0) {
            raced += 1;
        }
    }
    assert!(raced > 30 * 30, "the race ran past the green ({raced} frames)");
    let d = game.sim.drive.as_ref().unwrap();
    assert_eq!(d.world_clock.update - scheduler_loaded.update, ticks as i32);
    // The 1/4000 s clock follows real time on the ticks: 120 s within
    // one tick's quantum (the scene's 60, 60, 60, 60, 61 and the race's
    // cycle each end on whole real periods only at their cycle ends).
    let dtick = d.world_clock.tick.wrapping_sub(scheduler_loaded.tick);
    assert!(dtick.abs_diff(120 * 4000) <= 140, "the 1/4000 s clock after 120 s: {dtick}");
    assert!((d.clock - (loaded + 120.0)).abs() < 1.0, "race clock after 120 s: {} (loaded at {loaded})", d.clock);
    assert_eq!(d.time_of_day.sky().set, 2, "night in the race after its first minute");
    // Back to free roam: another load, the sunrise pair again.
    game.explore(None).unwrap();
    let d = game.sim.drive.as_ref().unwrap();
    assert!((d.clock - 1.0).abs() < 0.01, "free roam after the race starts at 1 s: {}", d.clock);
    assert_eq!(d.time_of_day.sky(), nfsu2::sky::SkyFrame { set: 0, morph: None });
}
