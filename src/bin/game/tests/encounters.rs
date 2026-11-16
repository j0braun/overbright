use crate::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The encounter racers' racing lines are computed for the best class
/// of the loaded, performance-matched cars (0x5DDFD0 reads the racers'
/// matched performance), not the stock car types' (under Wine the
/// finished-career save gave class 0 with stock-class-3 cars;
/// docs/outrun-ai-strength.md).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn encounter_class_comes_from_the_upgraded_cars() {
    let root = testkit::game_root();
    let root = std::path::Path::new(&root);
    let raw = nfsu2::formats::jdlz::decompress(&nfsu2::game_fs::read(&root, "GLOBAL/GlobalB.lzc").unwrap()).unwrap();
    let career = nfsu2::formats::bundle::chunks(&raw, 0).find(|c| c.id == 0x8003_4A10).expect("career");
    let packages = nfsu2::performance::parse(career.child(0x34A15).expect("packages").body);
    let stock = Car::load(root, "COROLLA").unwrap();
    let upgraded = Car::load_upgraded(root, "COROLLA", &Default::default(), &packages, &|_| true).unwrap();
    let (s, u) = (stock.car_type.handling.performance_class, upgraded.car_type.handling.performance_class);
    assert!(u < s, "fully upgraded class {u} beats stock {s}");
    assert_eq!(encounter_class([&stock, &upgraded].into_iter()), u);
    assert_eq!(encounter_class(std::iter::empty()), 3);
}

/// Asset-backed dispatch regression: a shot already active on an encounter
/// must reach the same effect path as the player's shot. No GPU needed.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn encounter_nitro_particles_keep_slot_identity() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ENCOUNTER_FLAME_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    profile.stage = 1;
    profile.perf.extend(
        game.game_data.career
            .packages
            .iter()
            .filter(|p| p.category == 2 && p.level == 1)
            .flat_map(|p| p.parts.iter().map(|x| x.id)),
    );
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.settings.custom[10] = 1;
    let d = game.sim.drive.as_mut().unwrap();
    d.effects.system.particles.clear();
    d.state.drivetrain = None; // Only the opponent has an active shot.
    let at = d.state.position;
    d.camera.position = at - Vec3::X * 6.0 + Vec3::Z * 2.0;
    let encounters = game.sim.encounters.as_mut().unwrap();
    encounters.ready();
    assert!(encounters.cars.len() >= 2);
    for (i, c) in encounters.cars.iter_mut().enumerate() {
        c.active = i == 1; // An inactive hole must not renumber slot 1.
        c.state = CarState::new(at + Vec3::Y * (i as f32 * 4.0), 0.0);
        c.state.velocity = Vec3::X * 20.0;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        c.state.publish_forward_speed();
        let mut drivetrain = nfsu2::handling::Drivetrain::new(&c.car.car_type.handling);
        drivetrain.nitrous = 1;
        c.state.drivetrain = Some(drivetrain);
        c.state.set_gas(1.0); // on the gas: the flame, not the purge (0x5ECDC0)
        assert!(!c.car.exhausts.is_empty());
    }
    game.particle_effects(1.0 / 60.0, &crate::sim::TickSettings::take(&game.settings.custom));
    let flames = &game.sim.drive.as_ref().unwrap().effects.flames;
    assert!(flames.keys().any(|&(owner, _, _)| owner == 2), "active encounter slot 1 never reached flame emission");
    assert!(!flames.keys().any(|&(owner, _, _)| owner == 1), "parked slot emitted a flame");
    let particles = &game.sim.drive.as_ref().unwrap().effects.system.particles;
    assert!(particles.iter().any(|p| p.key == FLAME_KEY + 2));
    assert!(!particles.iter().any(|p| p.key == FLAME_KEY));
    // Activating the earlier slot cannot transfer the existing emitter's owner.
    game.sim.encounters.as_mut().unwrap().cars[0].active = true;
    game.particle_effects(1.0 / 60.0, &crate::sim::TickSettings::take(&game.settings.custom));
    let flames = &game.sim.drive.as_ref().unwrap().effects.flames;
    assert!(flames.keys().any(|&(owner, _, _)| owner == 1));
    assert!(flames.keys().any(|&(owner, _, _)| owner == 2));
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn encounter_recovery_placement_then_contact_consumes_once() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("RecoveryContactTest");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    let encounters = game.sim.encounters.as_mut().unwrap();
    encounters.ready();
    for car in &mut encounters.cars {
        car.active = false;
    }
    let r = &mut encounters.cars[0];
    r.active = true;
    r.state = CarState::placed(&r.car.car_type, &d.world, d.state.position, 0.0);
    r.route = nfsu2::ai::route::RacerRoute::start(&encounters.routes, r.state.position, r.state.forward().truncate());
    assert!(reset_encounter(&d.world, r, &encounters.routes, 1));
    let landing = r.state.position;
    let forward = r.state.forward();
    let heading = forward.y.atan2(forward.x);
    // Put the player at the recovery landing, away from the old rival.
    d.state = CarState::placed(&d.car.car_type, &d.world, landing + forward, heading + std::f32::consts::PI);
    d.state.human = true;
    d.state.velocity = -forward * 10.0;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    d.state.publish_forward_speed();
    r.state.position -= forward * 20.0;
    r.state.velocity = forward * 15.0;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    r.state.publish_forward_speed();
    r.recovery_pending = true;
    r.controller.suspended = true;
    let old_position = r.state.position;
    let samples = [d.state.begin_world_update(0.05), r.state.begin_world_update(0.05)];
    let mut trace = Vec::new();
    step_explore_traced(d, None, Some(encounters), Controls::default(), 0.05, Some(&mut trace), &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    let r = &encounters.cars[0];
    assert_eq!(trace.len(), 1, "placement is not repeated during contact restep");
    assert_eq!(trace[0].0 + 1, trace[0].1);
    assert_eq!(trace[0].1, 1, "one world step per frame (0x5811D0)");
    assert!(!encounters.field.partners(0).is_empty(), "placed rival produces a live pair contact record");
    assert!(r.state.resolved > 0 && d.state.resolved > 0, "landing pair reaches collision resolution");
    assert!(!r.recovery_pending);
    assert!(!r.controller.suspended);
    assert!(r.state.position.distance(old_position) > 1.0);
    for (state, sample) in [(&d.state, samples[0]), (&r.state, samples[1])] {
        assert!(state.position.is_finite() && state.velocity.is_finite() && state.orientation.is_finite());
        let mut expected = state.clone();
        expected.finish_world_update(sample);
        assert_eq!(
            state.world_acceleration, expected.world_acceleration,
            "outer acceleration includes placement and contact response"
        );
    }
    let mut trace = Vec::new();
    step_explore_traced(d, None, Some(encounters), Controls::default(), 0.025, Some(&mut trace), &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    assert!(
        trace.is_empty(),
        "consumed recovery does not repeat with surviving contacts"
    );
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn encounter_loaded_belt_enters_after_native_gates() {
    testkit::game();
    use nfsu2::ai::encounter_controller::WorldClock;
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("BeltTest");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let e = game.sim.encounters.as_mut().unwrap();
    e.ready();
    let routes = e.routes.clone();
    let ai = game.sim.ai.as_ref().unwrap();
    let r = &mut e.cars[0];
    let (record, cp) = routes
        .file
        .records
        .iter()
        .enumerate()
        .find_map(|(i, record)| record.points.iter().position(|point| point.flags & 0x800 != 0).map(|cp| (i, cp)))
        .unwrap();
    let position = routes.file.records[record].points[cp].position;
    let direction = routes.file.direction(record, cp, false);
    r.state = CarState::new(position, direction.y.atan2(direction.x));
    r.state.velocity = direction.extend(0.0) * 20.0;
    r.state.publish_forward_speed();
    for wheel in &mut r.state.wheels {
        wheel.on_ground = true;
    }
    r.route = nfsu2::ai::route::RacerRoute::start(&routes, position, direction);
    let mut rng = nfsu2::opponents::GameRng(1);
    let mut clock = WorldClock { tick: 120000, update: 0 };
    let call = |r: &mut EncounterCar,
                clock: &mut WorldClock,
                rng: &mut nfsu2::opponents::GameRng,
                seconds: f32,
                gap: f32,
                full: bool| {
        clock.tick += if full { 1000 } else { 67 };
        clock.update = if full { 0 } else { 7 };
        let human = (position.truncate() - direction * gap).extend(position.z);
        let me = road_car(1, &r.car, &r.state, Some(r.state.velocity), true, false, 1.0 / 60.0);
        encounter_racer_think(
            r,
            ThinkCtx { ai, routes: &routes, field_rng: rng, clock: seconds, dt: 1.0 / 60.0 },
            EncounterThinkIn {
                others: &[me],
                me,
                hits: &[],
                touching: false,
                slot: 0,
                count: 3,
                world_clock: clock,
                chase: false,
                target_position: position,
                player: human.truncate(),
                race_clock: seconds,
                race_start_tick: 4000,
                sample_dt: 1.0 / 60.0,
                human_position: human,
                loaded: true,
            },
        );
    };
    call(r, &mut clock, &mut rng, 29.9, 180.0, true);
    assert!(r.driver_plan.0.len() > 2);
    assert_eq!(encounter_mover_kind(r), 1, "false predicate leaves an existing non4 mover unchanged");
    let path = r.driver_plan.0.clone();
    call(r, &mut clock, &mut rng, 29.9, 180.0, false);
    assert!(r.belt.belt.is_none(), "pre30 action fallback cannot enter belt");
    assert_eq!(r.driver_plan.0, path);
    call(r, &mut clock, &mut rng, 30.0, 60.0, true);
    call(r, &mut clock, &mut rng, 30.0, 60.0, false);
    assert_eq!(encounter_mover_kind(r), 1, "near-player full/fallback leaves an existing non4 mover unchanged");
    call(r, &mut clock, &mut rng, 30.0, 180.0, true);
    assert_eq!(r.driver.mover, 4, "loaded active-outrun leader must use BeltMover after30s/far-distance gates");
    let path = r.driver_plan.0.clone();
    // A full route failure supplies no new steering update. Its driver
    // keeps its last actual path, including for the belt's final decision.
    r.route = None;
    r.controller.output = None;
    call(r, &mut clock, &mut rng, 30.2, 180.0, true);
    assert_eq!(r.driver_plan.0, path);
    assert_eq!(r.driver.mover, 4);
    call(r, &mut clock, &mut rng, 30.2, 60.0, false);
    assert_eq!(r.driver.mover, 4, "fallback does not run final mover decision");
    call(r, &mut clock, &mut rng, 30.4, 60.0, true);
    assert_eq!(r.driver.mover, 3, "rejected loaded belt returns to Melo");
    call(r, &mut clock, &mut rng, 30.6, 180.0, true);
    assert_eq!(r.driver.mover, 4);
    r.controller.output = Some(nfsu2::ai::actions::Steer { path: Vec::new(), cap: None, pedals: true, reset: false });
    call(r, &mut clock, &mut rng, 30.8, 180.0, true);
    assert!(r.driver_plan.0.is_empty(), "explicit empty driver path is not a missing update");
    assert_eq!(r.driver.mover, 3);

    // Rebuild via the same decision, then run actual explore motion. The
    // supplied route is a test setup, not a native trajectory replay.
    r.controller.output.as_mut().unwrap().path = path;
    call(r, &mut clock, &mut rng, 31.0, 180.0, true);
    assert_eq!(r.driver.mover, 4);
    let saved_belt = r.belt.clone();
    r.belt.belt = None;
    r.state.leave_melo();
    r.belt_hold = 4000;
    encounter_mover_timers(r, false, 1.0);
    assert_eq!(encounter_mover_kind(r), 1, "unloaded expiry must not create Melo from physics");
    r.belt = saved_belt.clone();
    r.belt_hold = 4000;
    encounter_mover_timers(r, false, 1.0);
    assert_eq!(r.belt_hold, 0);
    assert_eq!(encounter_mover_kind(r), 4, "unloaded hold expiry preserves mover");
    r.belt_hold = 8000;
    encounter_mover_timers(r, true, 0.25);
    assert_eq!(r.belt_hold, 7000);
    assert_eq!(encounter_mover_kind(r), 1, "loaded live hold evicts retained belt");
    r.belt = saved_belt.clone();
    r.belt_hold = 4000;
    encounter_mover_timers(r, true, 1.0);
    assert_eq!(encounter_mover_kind(r), 3, "loaded expiry restores fallback even from retained belt");
    r.belt = saved_belt;
    r.state.leave_melo();
    r.active = true;
    let before = r.state.position;
    let d = game.sim.drive.as_mut().unwrap();
    d.state.position = before - direction.extend(0.0) * 180.0;
    step_explore(d, None, Some(e), Controls::default(), 1.0 / 240.0, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    let r = &mut e.cars[0];
    assert!(r.state.position.distance(before) > 0.01, "actual world path moves the belt");
    assert!(r.belt.belt.is_some());
    assert!(r.state.position.is_finite() && r.state.velocity.is_finite());
    let belt_time = r.belt.belt.as_ref().unwrap().t;
    // The native final race0 group remains4. Ending the active goal must
    // retain that mover through the existing post-outrun parking interval.
    assert!(e.outrun.is_none());
    e.think(ai, 1.0 / 240.0, d, None, &[], None);
    assert!(e.cars[0].belt.belt.is_some());
    step_explore(d, None, Some(e), Controls::default(), 1.0 / 240.0, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    assert!(e.cars[0].belt.belt.as_ref().unwrap().t > belt_time);
    // The world notice (0x4022D0): streamed out, a MeloMvr car goes to
    // mode 4; loaded, the notice changes no mover.
    let r = &mut e.cars[0];
    r.belt.belt = None;
    r.state.melo = Some(nfsu2::melo::Melo::new(&r.car, &r.state, 5.0));
    encounter_world_notice(r, true, 20.0);
    assert_eq!(encounter_mover_kind(r), 3);
    encounter_world_notice(r, false, 20.0);
    assert_eq!(encounter_mover_kind(r), 4, "an unloaded car takes the belt");

    // Real collision-world contact with the human must leave the belt,
    // retain the physical hold, and not resurrect mode4 next substep.
    let r = &mut e.cars[0];
    let forward = r.state.forward();
    d.state = CarState::placed(
        &d.car.car_type,
        &d.world,
        r.state.position + forward,
        r.state.heading() + std::f32::consts::PI,
    );
    d.state.human = true;
    d.state.velocity = -forward * 10.0;
    d.state.publish_forward_speed();
    step_explore(d, None, Some(e), Controls::default(), 1.0 / 60.0, &mut nfsu2::opponents::GameRng::default(), &mut Default::default());
    let r = &mut e.cars[0];
    assert!(r.belt.belt.is_none(), "resolved human contact exits mode4");
    assert_eq!(r.belt_hold, HOLD_TICKS);
    assert_eq!(r.belt_block_ticks, 8000);
    assert!(r.state.melo.is_none());
    for wheel in &mut r.state.wheels {
        wheel.on_ground = true;
    }
    encounter_mover_timers(r, true, 4.0);
    assert_eq!(r.belt_hold, 4000);
    assert!(r.state.melo.is_none());
    encounter_mover_timers(r, true, 1.0);
    assert_eq!(r.belt_hold, 0);
    assert_eq!(encounter_mover_kind(r), 3);
    // Geometry-only adapter check at the120m boundary: distinct rotated
    // COM offsets must not be mistaken for the two mesh origins.
    std::sync::Arc::make_mut(&mut r.car).car_type.handling.centre_of_mass = [2.0, 0.0, 1.0];
    let mut a = CarState::new(Vec3::ZERO, 0.0);
    let b = CarState::new(Vec3::new(120.1, 0.0, 0.0), std::f32::consts::FRAC_PI_2);
    a.orientation = glam::Quat::IDENTITY;
    assert!(a.position.distance(b.position) > 120.0);
    let gap = nfsu2::ai::encounter_controller::belt_human_distance(
        belt_car_position(&r.car, &a),
        belt_car_position(&r.car, &b),
    );
    assert!(gap < 120.0, "COM-aware gate rejects despite origins farther than120m: {gap}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn encounter_controller_full_fallback_and_reset_lifecycle() {
    testkit::game();
    use nfsu2::ai::{
        actions::{Actions, Kind},
        encounter_controller::WorldClock,
    };
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ControllerTest");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let e = game.sim.encounters.as_mut().unwrap();
    e.ready();
    let slot = 0;
    let routes = e.routes.clone();
    let ai = game.sim.ai.as_ref().unwrap();
    let r = &mut e.cars[slot];
    let (record, cp) = routes
        .file
        .records
        .iter()
        .enumerate()
        .find_map(|(i, record)| record.points.iter().position(|point| point.flags & 0x800 != 0).map(|cp| (i, cp)))
        .unwrap();
    let position = routes.file.records[record].points[cp].position;
    let direction = routes.file.direction(record, cp, false);
    r.state = CarState::new(position, direction.y.atan2(direction.x));
    r.state.velocity = direction.extend(0.0) * 20.0;
    // Explicit synthetic/replay motion initialization; no native placement offset.
    r.state.publish_forward_speed();
    r.route = nfsu2::ai::route::RacerRoute::start(&routes, position, direction);
    r.controller = Default::default();
    let mut rng = nfsu2::opponents::GameRng(1);
    let mut clock = WorldClock { tick: 100000, update: 7 };
    let call = |r: &mut EncounterCar,
                routes: &nfsu2::ai::route::AiRoutes,
                clock: &WorldClock,
                rng: &mut nfsu2::opponents::GameRng,
                chase| {
        let me = road_car(slot + 1, &r.car, &r.state, Some(r.state.velocity), true, false, 1.0 / 60.0);
        encounter_racer_think(
            r,
            ThinkCtx { ai, routes, field_rng: rng, clock: 30.0, dt: 1.0 / 60.0 },
            EncounterThinkIn {
                others: &[me],
                me,
                hits: &[],
                touching: false,
                slot: slot as i32,
                count: 3,
                world_clock: clock,
                chase,
                target_position: position,
                player: position.truncate(),
                race_clock: 30.0,
                race_start_tick: 4000,
                sample_dt: 1.0 / 60.0,
                human_position: position,
                loaded: true,
            },
        );
    };
    call(r, &routes, &clock, &mut rng, false);
    assert_eq!(r.controller.speed_callbacks, 1);
    assert!(r.controller.actions.is_some());
    let path = r.controller.output.as_ref().unwrap().path.clone();
    let started = r.controller.actions.as_ref().unwrap().start;
    clock.tick += 67;
    call(r, &routes, &clock, &mut rng, false);
    assert_eq!(r.controller.speed_callbacks, 1);
    assert_eq!(r.controller.search_callbacks, 0);
    assert_eq!(r.controller.actions.as_ref().unwrap().start, started);
    assert_eq!(r.controller.output.as_ref().unwrap().path, path);
    r.controller.actions.as_mut().unwrap().kind = Kind::Block;
    call(r, &routes, &clock, &mut rng, false);
    assert_eq!(r.controller.speed_callbacks, 2, "ready fallback calls speed twice");
    assert_eq!(r.controller.actions.as_ref().unwrap().kind, Kind::Block);
    // Keep valid geometry, but make node finding impossible. This exercises
    // the production route-failure dispatch without a native-behavior claim
    // about the deliberately invalid fixture's road flags.
    let mut missing = routes.clone();
    for record in &mut missing.file.records {
        for point in &mut record.points {
            point.flags = 0;
        }
    }
    r.state.position += Vec3::new(100000.0, 100000.0, 0.0);
    r.controller.actions = None;
    call(r, &missing, &clock, &mut rng, false);
    assert!(!r.controller.suspended, "full goal6 failure preserves output");
    assert_eq!(r.controller.speed_callbacks, 0);
    r.controller.actions = Some(Actions::default());
    r.controller.actions.as_mut().unwrap().kind = Kind::Block;
    call(r, &missing, &clock, &mut rng, false);
    assert!(r.controller.suspended, "ready fallback failure suspends");
    call(r, &missing, &clock, &mut rng, true);
    assert!(r.controller.suspended, "goal reassignment does not clear suspension");
    assert_eq!(r.controller.speed_callbacks, 0);
    r.state.position = position;
    r.route = nfsu2::ai::route::RacerRoute::start(&routes, position, direction);
    r.controller.reset_placement((slot + 1) as i16);
    r.controller.actions = None;
    call(r, &routes, &clock, &mut rng, true);
    assert!(!r.controller.suspended);
    assert_eq!(r.controller.search_callbacks, 1);
    assert_eq!(r.controller.speed_callbacks, 1);
    assert!(r.controller.output.as_ref().is_some_and(|o| !o.path.is_empty()));
    r.state.nitrous = Some(nfsu2::nitrous::Tank::new(4.0, 2));
    r.state.nitrous.as_mut().unwrap().start();
    r.state.nitrous.as_mut().unwrap().advance(0.75);
    let tank = r.state.nitrous;
    let last_tick = r.controller.schedule.last_tick;
    r.controller.suspended = true;
    assert!(reset_encounter(&game.sim.drive.as_ref().unwrap().world, r, &routes, (slot + 1) as i16));
    assert_eq!(r.state.nitrous, tank, "recovery must not refill or discard the tank");
    assert!(!r.controller.suspended);
    assert_eq!(r.controller.schedule.last_tick, last_tick);
    assert!(r.controller.actions.is_some(), "placement retains the action");
    // docs/outrun-ai-strength.md §3: the chase search's origin is
    // a record's first point travelled backward (the traced spot); the
    // list ends at it and the car is past its end. 0x428720 returns
    // false: the full think keeps its output (race 7, goal +1C), no
    // suspension; the ready fallback's 0x428720 searches a new node.
    let spot = Vec3::new(-220.79, -1577.06, 13.61);
    let heading = Vec2::new(0.9994, -0.0357);
    let player = Vec3::new(-197.13, -1579.65, 13.61);
    let (pr, pc) = nfsu2::formats::ai_routes::PointGrid::new(&routes.file).nearest(&routes.file, player).unwrap();
    let target = routes.file.records[pr].points[pc].position;
    r.state = CarState::new(spot, heading.y.atan2(heading.x));
    r.state.velocity = heading.extend(0.0) * 33.4;
    r.state.publish_forward_speed();
    r.route = nfsu2::ai::route::RacerRoute::start(&routes, spot, heading);
    r.controller = Default::default();
    r.controller.assign_goal(false);
    r.controller.actions = Some(Actions::default());
    r.controller.output = Some(nfsu2::ai::actions::Steer { path: vec![spot], cap: None, pedals: true, reset: false });
    let chase = |r: &mut EncounterCar, clock: &WorldClock, rng: &mut nfsu2::opponents::GameRng| {
        let me = road_car(slot + 1, &r.car, &r.state, Some(r.state.velocity), true, false, 1.0 / 60.0);
        encounter_racer_think(
            r,
            ThinkCtx { ai, routes: &routes, field_rng: rng, clock: 30.0, dt: 1.0 / 60.0 },
            EncounterThinkIn {
                others: &[me],
                me,
                hits: &[],
                touching: false,
                slot: slot as i32,
                count: 3,
                world_clock: clock,
                chase: true,
                target_position: target,
                player: player.truncate(),
                race_clock: 30.0,
                race_start_tick: 4000,
                sample_dt: 1.0 / 60.0,
                human_position: player,
                loaded: true,
            },
        );
    };
    clock.tick += 4000;
    chase(r, &clock, &mut rng);
    assert_eq!(r.controller.search_callbacks, 1, "goal change: full think with the search");
    assert!(r.route.as_ref().unwrap().past_end(), "the search's list ends behind the car");
    assert!(!r.controller.suspended, "a full think's route failure does not suspend in race 7");
    assert_eq!(r.controller.output.as_ref().unwrap().path, vec![spot], "the driver's output is kept");
    r.controller.actions = Some(Actions::default());
    r.controller.actions.as_mut().unwrap().kind = Kind::Block;
    clock.tick += 67;
    chase(r, &clock, &mut rng);
    assert_eq!(r.controller.search_callbacks, 0, "ready fallback");
    assert!(!r.controller.suspended, "the ready fallback finds a new node");
    assert!(!r.route.as_ref().unwrap().past_end());
}

fn finished_career_game() -> Game {
    testkit::game();
    let mut game = Game::new().unwrap();
    let s = nfsu2::formats::native_save::NativeSave::parse(&testkit::private("tests/fixtures/native_saves/career_complete.sav"))
    .unwrap();
    game.profile = Some(Profile::from_native(&s, career::NativeContext::get().unwrap()));
    game.rng = nfsu2::opponents::GameRng::boot(12345);
    game
}

fn line_up(game: &mut Game) -> Vec<String> {
    let e = game.sim.encounters.as_mut().unwrap();
    e.ready();
    e.cars.iter().map(|c| c.car.car_type.name.clone()).collect()
}

/// Free-roam set-ups in a row (a garage jump and back, nothing else
/// drawn between) draw other encounter cars: the set-up's draws move
/// the game's random numbers on (0x53F010 → 0x4FEAF0 on 0x7EA624;
/// observed under Wine with a community save, docs/drag-drift-outrun-
/// parity.md §0). Before, the draws ran on a copy left behind, so
/// every load drew the same three cars.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn free_roam_loads_in_a_row_draw_new_encounter_cars() {
    let mut game = finished_career_game();
    let mut seen = Vec::new();
    for _ in 0..4 {
        let before = game.rng.0;
        game.explore(None).unwrap();
        assert_ne!(game.rng.0, before, "the set-up's draws move the random numbers on");
        seen.push(line_up(&mut game));
    }
    assert!(seen.windows(2).all(|w| w[0] != w[1]), "line-ups {seen:?}");
}

/// Outrun Variety (clone deviation) on: a racer that has been out and
/// is parked again gets a new car for its slot, of a type not out in
/// the other slots, by the set-up's rule; its slot stamps kept. Off:
/// nothing changes (native).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn outrun_variety_redraws_a_parked_racer() {
    let mut game = finished_career_game();
    game.explore(None).unwrap();
    let before = line_up(&mut game);
    // It sets the Outrun Variety global (docs/test-determinism.md).
    super::test_isolation::exclusive();
    nfsu2::qol::set_outrun_variety(false);
    game.sim.encounters.as_mut().unwrap().cars[1].active = true;
    game.outrun_variety_frame();
    game.sim.encounters.as_mut().unwrap().cars[1].active = false;
    game.outrun_variety_frame();
    assert!(game.sim.encounters.as_ref().unwrap().variety_load.is_none(), "off: native, no redraw");
    assert_eq!(line_up(&mut game), before);
    let e = game.sim.encounters.as_mut().unwrap();
    e.cars[1].active = true;
    e.cars[1].placed = Some(3.0);
    game.outrun_variety_step();
    assert!(game.sim.encounters.as_ref().unwrap().variety_load.is_none(), "not while it is out");
    game.sim.encounters.as_mut().unwrap().cars[1].active = false;
    game.outrun_variety_step();
    let job = &game.sim.encounters.as_ref().unwrap().variety_load;
    assert_eq!(job.as_ref().map(|j| j.0), Some(1));
    while game.sim.encounters.as_ref().unwrap().variety_load.as_ref().is_some_and(|j| !j.3.is_finished()) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    game.outrun_variety_step();
    let e = game.sim.encounters.as_ref().unwrap();
    assert!(e.variety_load.is_none() && !e.variety[1]);
    assert_eq!(e.cars[1].placed, Some(3.0));
    let after: Vec<String> = e.cars.iter().map(|c| c.car.car_type.name.clone()).collect();
    assert_eq!((&after[0], &after[2]), (&before[0], &before[2]));
    assert!(!before.contains(&after[1]), "{before:?} -> {after:?}");
}
