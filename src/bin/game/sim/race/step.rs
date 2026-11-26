use crate::*;

/// A car as the traffic's road entries see it (a node of 0x8A2CA4's
/// list): place, velocity, facing and collision box.
pub(crate) fn traffic_body(car: &Car, state: &CarState) -> nfsu2::ai::traffic::Other {
    nfsu2::ai::traffic::Other {
        position: state.position.truncate(),
        z: state.position.z,
        velocity: state.velocity.truncate(),
        forward: (state.orientation * Vec3::X).truncate(),
        half: nfsu2::ai::traffic::Other::half_box(&car.hull),
        traffic: None,
        human: false,
        id: 0,
    }
}

/// The race set-up's traffic fill (0x6065C0 → 0x42F7A0), once, when the
/// load has not made it (`Game::race_world_cars` makes it in the load, as
/// the game does): on the race's first frame, or the start scene's first.
pub(crate) fn race_traffic_fill(d: &Drive, race: &mut Race, clock: f32, drag: bool) {
    if let Some(t) = race.racers.traffic.as_ref().filter(|t| !t.filled && t.pool.is_none()) {
        let racers: Vec<(Vec2, Vec2)> =
            race.racers.rivals.iter().map(|r| (r.state.position.truncate(), r.state.velocity.truncate())).collect();
        let place = DragPlace {
            coord: race.racers.me.along,
            lane: race.dragger.as_ref().and_then(|s| s.lane.lane),
            clock: clock.min(-1e-3),
            start_line: race.route.start_at,
            start_gap: race.start_gap,
        };
        // The fill's draws are the world's (0x42E2C0 / 0x411110 / 0x42E4A0
        // on [0x890080] +0xF8): the race field's generator.
        let plan = traffic_fill_plan(
            t,
            t.cars.len(),
            d.state.position.truncate(),
            &racers,
            Some((&race.route.road, place)),
            drag,
            &mut race.racers.cars.rng,
        );
        if let Some(t) = race.racers.traffic.as_mut() {
            traffic_fill_apply(&d.world, t, plan, true, Some(&race.route.road));
        }
    }
}

// Samples belong to the motion interval after controller decisions. Scene
// placement reuses them so controller resets do not become acceleration.
pub(crate) struct RaceWorldSamples {
    pub(crate) player: nfsu2::drive::WorldVelocitySample,
    pub(crate) rivals: Vec<Option<nfsu2::drive::WorldVelocitySample>>,
}

/// What every phase of a race frame reads: the race clock at the frame's
/// start, whether the race is a drag race, the update's seconds.
#[derive(Clone, Copy)]
pub(crate) struct RaceTick {
    pub(crate) clock: f32,
    pub(crate) drag: bool,
    pub(crate) dt: f32,
}

/// One race frame for every car: each rival's route, target speed and
/// driver (the controllers' think 0x42ABD0, then the drivers 0x421030),
/// then the player and the rivals through one world collision step
/// (0x609CF0 → 0x5FFA60), and the rivals' progress.
pub(crate) fn step_race(
    d: &mut Drive,
    race: &mut Race,
    player: Controls,
    ai: Option<&AiData>,
    dt: f32,
    sounds: &mut nfsu2::drive::ContactSounds,
) -> RaceWorldSamples {
    let clock = race.timing.clock;
    let mut rival_samples = vec![None; race.racers.rivals.len()];
    let drag = d.car.car_type.handling.race.drag();
    let tick = RaceTick { clock, drag, dt };
    race_ai_pass(d, race, ai, &mut rival_samples, tick);
    let me = race_player_input(d, race, player, tick);
    // The race's traffic (`traffic_think`) sees the rivals as the player
    // (a replay's snapshots hold its state).
    // The race set-up's fill (0x6065C0 → 0x42F7A0 → 0x42E4A0 in a drag
    // race, 0x42E2C0 in the others), on the race's first frame (the
    // game's set-up runs it while loading).
    race_traffic_fill(d, race, clock, drag);
    let player_sample = race_world_step(d, race, me, ai, &mut rival_samples, tick, sounds);
    race_post(d, race, player_sample, rival_samples, tick)
}

/// The values every rival's think reads this frame (`race_ai_pass`).
struct RivalFrame<'a> {
    ai: &'a AiData,
    routes: &'a nfsu2::ai::route::AiRoutes,
    race_type: nfsu2::race::RaceMode,
    length: f32,
    me_progress: f32,
    human_speed: f32,
    human_at: Vec3,
    human_list: Option<&'a [u32]>,
    place: usize,
    scale: f32,
    road_cars: &'a [nfsu2::ai::actions::RoadCar],
    rival_count: i32,
    catch_up: Option<bool>,
    laps: u32,
    route: &'a Route,
    human_line_mean: f32,
    ai_frames: u64,
    scene_camera: bool,
}

/// The AI's part of a race frame (with the race's AI routes and data):
/// the human's line-speed sample, the cars the road entries see, each
/// rival's think, belt and nitrous (`rival_think`, `rival_after_think`),
/// and the autopilot past the line.
fn race_ai_pass(
    d: &Drive,
    race: &mut Race,
    ai: Option<&AiData>,
    rival_samples: &mut [Option<nfsu2::drive::WorldVelocitySample>],
    tick: RaceTick,
) {
    let RaceTick { dt, .. } = tick;
    if let (Some(routes), Some(ai)) = (race.racers.ai_routes.as_ref(), ai) {
        let race_type = d.car.car_type.handling.race;
        let length = race.route.road.length;
        let (me_progress, human_speed) = (race.racers.me.progress(), d.state.velocity.length());
        // The human car's place as the world copies it (world +0x80, the
        // body's centre, 0x609CF0).
        let human_at = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
        race.racers.ai_frames += 1;
        // The sections the streamer holds with the human here (see
        // `streamed_in`).
        let human_list = d.world.visible_sections_at(d.state.position).map(|(_, l)| l);
        // The human's line-speed sample (player update 0x602660, 0x6027B0..
        // 0x60288C): once a second (+0xA4: ticks less round(dt·4000 + 0.5),
        // then 1.0 s again, 0x7A551C), when the car has a road node
        // (0x40F690; the human mask of 0x413590), car +0x430 over its control
        // point's racing-line speed (byte +0x2C, × 65536 × 2⁻¹⁶); no line
        // speed gives 0, or 1 when moving at 0.01 m/s or more.
        race.human_line_timer -= dt;
        if race.human_line_timer <= 0.0 {
            let f = d.state.forward();
            let node = nfsu2::ai::route::RacerRoute::find_flagged(
                routes,
                d.state.position,
                Vec2::new(f.x, f.y).normalize_or(Vec2::X),
                Some(race.racers.me.along),
                false,
                0x0800_0000,
            )
            .or_else(|| {
                nfsu2::ai::route::RacerRoute::find_flagged(
                    routes,
                    d.state.position,
                    Vec2::new(f.x, f.y).normalize_or(Vec2::X),
                    None,
                    false,
                    0x0800_0000,
                )
            });
            if let Some((rec, cp, _)) = node {
                let line = f32::from(routes.file.records[rec].points[cp].lane_speed[0]);
                race.human_line.push(if line > 0.0 {
                    human_speed / line
                } else if human_speed >= 0.01 {
                    1.0
                } else {
                    0.0
                });
            }
            race.human_line_timer = 1.0;
        }
        // The human's place (racer status +0x0A, 1 for the leader: seen under
        // Wine, the leading rival's reads 1, the last of four 4).
        let place = race.order.iter().position(|&i| i == 0).map_or(1, |p| p + 1);
        let scale = race
            .catch_up_difficulty
            .map_or(1.0, |diff| ai.tuning.catch_up_scale(diff, d.car.car_type.handling.performance_class));
        // The cars the racers' road entries see (0x6148F0 walks every
        // object node): the human (id 0), the rivals (1…) and the race's
        // traffic (1000…), with their accelerations from last frame's
        // velocities. Block's tuning row is each racer's +0x902
        // (`Rival::block_row`).
        let mut road_cars = vec![road_car(0, &d.car, &d.state, race.replay.road_prev.get(&0).copied(), true, true, dt)];
        human_road_path(&mut road_cars[0], &d.human_road, routes, &d.car, &d.state);
        for (i, r) in race.racers.rivals.iter().enumerate() {
            // The path it drives (car +0x30 → +0x10), as the racers' sweeps
            // read it (0x4275E0 → 0x60BE20).
            let mut c = road_car(i + 1, &r.car, &r.state, r.actions.last_velocity, true, false, dt);
            c.path = nfsu2::ai::actions::CarPath::from_points(&r.actions.last_path);
            road_cars.push(c);
        }
        if let Some(t) = race.racers.traffic.as_ref() {
            // Only road objects (`TrafficCar::road_object`): a pooled car's
            // last place and velocity are no obstacle for the racers' sweeps.
            for (k, c) in t.cars.iter().enumerate().filter(|(_, c)| c.road_object()) {
                let mut rc = road_car(
                    1000 + k,
                    &c.car,
                    &c.state,
                    race.replay.road_prev.get(&(1000 + k)).copied(),
                    false,
                    false,
                    dt,
                );
                traffic_road_path(&mut rc, c, &t.routes);
                road_cars.push(rc);
            }
        }
        race.replay.road_prev =
            road_cars.iter().filter(|c| c.id == 0 || c.id >= 1000).map(|c| (c.id, c.velocity)).collect();
        let rival_count = race.racers.rivals.len() as i32;
        let f = RivalFrame {
            ai,
            routes,
            race_type,
            length,
            me_progress,
            human_speed,
            human_at,
            human_list,
            place,
            scale,
            road_cars: &road_cars,
            rival_count,
            catch_up: race.catch_up,
            laps: race.laps,
            route: &race.route,
            human_line_mean: race.human_line.mean,
            // The racer controllers' think turn (0x42ABD0 from 0x42AE2F)
            // and the full think's mover choice read the world update
            // counter 0x7FB724 (++ at the update's end, 0x57A0F0), the
            // race's `world_updates` (not yet counted for this update),
            // not a count of AI passes: observed in circuit_s1_pinned,
            // rival k's action changes decided on updates with 0x7FB724 ≡ k
            // (mod 8), 0x7FB724 = 1222 at the green.
            ai_frames: race.replay.world_updates,
            scene_camera: race.scene_camera,
        };
        for (i, r) in race.racers.rivals.iter_mut().enumerate() {
            let (loaded, plan) = rival_think(d, r, i, &f, &mut race.racers.cars, tick);
            rival_after_think(d, r, i, &f, loaded, &plan, &mut rival_samples[i], tick);
        }
        // Past the line the player's car is an AI racer's (0x606CC0 →
        // 0x5EBA40(3), see `Autopilot`).
        if let Some(a) = race.autopilot.as_mut() {
            autopilot_think(
                d,
                a,
                race.racers.me.along,
                race.racers.me.finished.is_some(),
                ai,
                routes,
                &mut race.racers.cars.rng,
                tick,
            );
        }
    }
}

/// The player's car past the line on the AI racer's controller (0x606CC0
/// → 0x5EBA40(3), see `Autopilot`): `racer_think` on the line, parked once
/// its status says done.
#[allow(clippy::too_many_arguments)]
fn autopilot_think(
    d: &Drive,
    a: &mut Autopilot,
    along: f32,
    parked: bool,
    ai: &AiData,
    routes: &nfsu2::ai::route::AiRoutes,
    field_rng: &mut nfsu2::opponents::GameRng,
    tick: RaceTick,
) {
    let RaceTick { clock, dt, .. } = tick;
    let race_type = d.car.car_type.handling.race;
    let accel = nfsu2::handling::acceleration(&d.car.car_type.handling);
    if let Some(route) = a.route.as_mut() {
        route.race_coord = Some(along);
    }
    let mut ai_state = RacerAi {
        route: &mut a.route,
        speed: &mut a.speed,
        driver: &mut a.driver,
        unstuck: &mut a.unstuck,
        actions: None,
        lane: &mut a.lane,
        drift: a.drift.as_mut(),
        plan: None,
    };
    a.controls = racer_think(
        &d.car,
        &d.state,
        &mut ai_state,
        &mut a.rng,
        &accel,
        ThinkCtx { ai, routes, field_rng, clock, dt },
        RacerThinkIn {
            race_type,
            scene_camera: false,
            progress: None,
            finished: !a.racing || parked,
            parked,
            catch_up: None,
        },
    );
}

/// A rival's think up to its driver (0x42ABD0 → 0x421030): its mover and
/// belt timers, the bench's tank, the catch-up's inputs, the road entries
/// it sees, the full-think schedule and `racer_think`. Returns whether its
/// world is loaded and the think's path and target speed.
fn rival_think(
    d: &Drive,
    r: &mut Rival,
    i: usize,
    f: &RivalFrame,
    cars: &mut nfsu2::drive::CarField,
    tick: RaceTick,
) -> (bool, (Vec<Vec3>, f32)) {
    let RaceTick { clock, dt, .. } = tick;
    let RivalFrame { ai, routes, race_type, length, me_progress, human_speed, human_list, place, scale, .. } = *f;
    let (road_cars, rival_count) = (f.road_cars, f.rival_count);
    racer_mover(r, race_type, clock, ai.bounds.scale(r.car.car_type.handling.performance_rating), dt);
    // Bench aid (not native): NFSU2_BENCH_RIVAL_NOS=seconds gives
    // each rival a one-shot tank of that size before the green.
    if clock < 0.0
        && let Some(secs) = debug_env().bench_rival_nos.as_deref().and_then(|v| v.parse::<f32>().ok())
        && r.state.nitrous.is_none_or(|t| t.size == 0)
    {
        r.state.nitrous = Some(nfsu2::nitrous::Tank::new(secs, 1));
    }
    // The think's start (0x42AC04): with the +0x730 timer running a
    // loaded car on mode 4 goes back to 0x5F4410's mode; then the
    // timer counts down (0x577320).
    let loaded = streamed_in(&d.world, human_list, r.state.position);
    // An unloaded car's body meets nothing (0x593990).
    r.state.streamed = loaded;
    if r.belt.block > 0.0 {
        if loaded {
            leave_belt(r, race_type, ai.bounds.scale(r.car.car_type.handling.performance_rating));
        }
        r.belt.block = (r.belt.block - dt).max(0.0);
    }
    // The driver reads the car's mover (+0x28; 0x421030).
    r.driver.mover = if r.belt.belt.is_some() {
        4
    } else if r.state.melo.is_some() {
        3
    } else {
        1
    };
    // The race-progress speed fed once the race is on (racer status
    // +0x18 × the road's length; the clone's races all run on race
    // coordinates, 0x88F2BE).
    let progress = (clock >= 0.0 && r.progress.finished.is_none())
        .then(|| (r.progress.laps.as_ref().map_or(0.0, |l| l.rate) * length, dt));
    // The catch-up's inputs (0x4172D0 / 0x4084A0): the gap to the
    // human along the race, both speeds, the laps, how far the race
    // is done, the skill row, the traffic curve choice, the time.
    let skill = r.speed.skill();
    let catch_up = f.catch_up.map(|alt| nfsu2::ai::route::CatchUp {
        gap: (r.progress.progress() - me_progress) * length,
        own_speed: r.state.velocity.length(),
        human_speed,
        laps: f.laps,
        progress: (r.progress.progress() / race_laps(f.route, f.laps) as f32).clamp(0.0, 1.0),
        skill,
        alt,
        race_time: clock.max(0.0),
        street_x: race_type.street_x(),
        line: Some(nfsu2::ai::route::LineSpeed { ratio: f.human_line_mean, place }),
        scale,
    });
    if let Some(route) = r.route.as_mut() {
        route.race_coord = Some(r.progress.along);
    }
    if debug_env().debug_actions && (clock * 2.0).floor() != ((clock - dt) * 2.0).floor() {
        let a = &r.actions;
        eprintln!(
            "actions t={clock:.1} rival{} at=({:.1},{:.1}) v={:.1} {:?} pri={} tgt={:?} plan=({:.2},{:.3})",
            i + 1,
            r.state.position.x,
            r.state.position.y,
            r.state.speed(),
            a.kind,
            a.priority,
            a.target,
            a.plan.offset,
            a.plan.rate
        );
    }
    let me = road_cars
        .iter()
        .find(|c| c.id == i + 1)
        .copied()
        .unwrap_or_else(|| road_car(i + 1, &r.car, &r.state, None, true, false, dt));
    let partners = cars.partners(i as u64 + 1);
    let hits: Vec<usize> = partners.iter().filter(|&&o| o < TRAFFIC_ID).map(|&o| o as usize).collect();
    // The drag got-hit proposal's partners: a record whose summed pushes
    // (+0x60) reach 0x78373C (0x403410 → 0x402AE0).
    let me_id = i as u64 + 1;
    let mut hard: Vec<usize> = cars
        .records()
        .into_iter()
        .filter(|&(_, _, _, j)| j >= nfsu2::ai::actions::DRAG_HIT_IMPULSE)
        .filter_map(|(a, b, _, _)| {
            if a == me_id {
                Some(b)
            } else if b == me_id {
                Some(a)
            } else {
                None
            }
        })
        .filter(|&o| o < TRAFFIC_ID)
        .map(|o| o as usize)
        .collect();
    hard.sort_unstable();
    hard.dedup();
    // 0x403450: a live record against an upright track polygon (0x5A0170).
    let wall = r.state.contacts.iter().any(|n| n.touching == nfsu2::drive::Touching::Track { upright: true });
    let full = r.think.full_think(nfsu2::ai::encounter_controller::ThinkInput {
        // The race's own world-update count and clock (both kept
        // by the replay's snapshot) stand for 0x7FB724 / 0x865188.
        dt,
        // Rounded to the whole tick: the race clock is a sum of f32 frame
        // steps a hair off the tick count (truncating gave 799 for the
        // 800 ticks of 12 frames, so the 0.2 s gate, 0x42AE2F, missed a
        // turn natively taken: circuit_s1_pinned rival 3, update 44).
        tick: ((clock as f64 + 1000.0) * 4000.0).round() as u32,
        world_update: f.ai_frames as i32,
        owner_class: 2,
        owner_kind: 2,
        owner_id: (100 + i) as i16,
        racer_slot: i as i32,
        racer_count: rival_count,
        drag: race_type.drag(),
        mover_four: r.belt.belt.is_some(),
        action_present: r.actions.last_velocity.is_some() && r.retained.is_some(),
    });
    let acts = RacerActions {
        actions: &mut r.actions,
        cars: road_cars,
        me,
        block_row: r.block_row,
        hits,
        hard,
        wall,
        touching: !partners.is_empty(),
        full,
        retained: &mut r.retained,
    };
    let mut plan = (Vec::new(), 0.0);
    let mut ai_state = RacerAi {
        route: &mut r.route,
        speed: &mut r.speed,
        driver: &mut r.driver,
        unstuck: &mut r.unstuck,
        actions: Some(acts),
        lane: &mut r.lane,
        drift: r.drift.as_mut(),
        plan: Some(&mut plan),
    };
    r.controls = racer_think(
        &r.car,
        &r.state,
        &mut ai_state,
        &mut r.rng,
        &*r.accel,
        ThinkCtx { ai, routes, field_rng: &mut cars.rng, clock, dt },
        RacerThinkIn {
            race_type,
            scene_camera: f.scene_camera,
            progress,
            finished: r.progress.finished.is_some(),
            parked: r.progress.finished.is_some(),
            catch_up,
        },
    );
    (loaded, plan)
}

/// A rival's think after its driver: the full think's mover choice and
/// the world notice (the belt), its world-update sample, the BeltMover's
/// frame, the catch-up into the physics, a reset, and the nitrous.
#[allow(clippy::too_many_arguments)]
fn rival_after_think(
    d: &Drive,
    r: &mut Rival,
    i: usize,
    f: &RivalFrame,
    loaded: bool,
    plan: &(Vec<Vec3>, f32),
    sample: &mut Option<nfsu2::drive::WorldVelocitySample>,
    tick: RaceTick,
) {
    let RaceTick { clock, drag, dt } = tick;
    let RivalFrame { ai, routes, race_type, human_at, .. } = *f;
    // The full think's mover choice (0x42B259, see `belt_wanted`):
    // on the racer's turn of eight frames once 0.2 s have passed.
    let rating = ai.bounds.scale(r.car.car_type.handling.performance_rating);
    r.belt.since_think += dt;
    if r.belt.since_think >= 0.2 && f.ai_frames % 8 == i as u64 {
        r.belt.since_think = 0.0;
        if !loaded {
            if r.belt.belt.is_some() {
                build_belt(r, &plan.0, plan.1);
            }
        } else if belt_wanted(r, race_type, clock, human_at, plan.0.len()) {
            if r.belt.belt.is_none() {
                r.state.leave_melo();
            }
            build_belt(r, &plan.0, plan.1);
        } else {
            leave_belt(r, race_type, rating);
        }
    }
    // The world notice (0x5F6CE0 → 0x4022D0, slot +0x10): a racer
    // whose section is streamed out goes to mode 4.
    if !loaded && r.belt.belt.is_none() {
        r.state.leave_melo();
        build_belt(r, &plan.0, plan.1);
    }
    *sample = Some(r.state.begin_world_update(dt));
    // The BeltMover's frame (0x59EC90; the movers run after the
    // thinks and drivers, 0x609CF0).
    if r.belt.belt.is_some() {
        step_belt(&d.world, r, plan, loaded, dt);
    }
    // The car's catch-up (car +0x468 = max(controller +0x770, 1),
    // 0x4172D0) into its physics (0x599180).
    r.state.catch_up = r.speed.catch_up().max(1.0);
    if std::mem::take(&mut r.actions.reset_pending) {
        let dir = f.route.road.direction(r.progress.along);
        reset_rival(&d.world, r, routes, dir);
        // Car_PlaceOnRoad (0x5FBEE0 → 0x5F52D0) stamps car +0x9C0 with the
        // sim tick at every exit (0x5F5477 / 0x5F548C / 0x5F54A1): GetUnstuck
        // waits 2 s after the reset (0x5ED590 > 2.0).
        r.unstuck.placed = Some(clock);
    }
    // The racer's driving action (lane 0x4242D0, JustDrive
    // 0x42B570, 0x42F5EE) asks the controller each frame (0x40E390,
    // racers: car +0x14 → +4 = 2) through slot +0xA0 (0x416F70):
    // in a drag race straight to 0x40D850. Its strategy in other
    // races (timers +0x7A8 / +0x7AC / +0x7B0, skill row +0x568 …
    // +0x574 and +0x589, the road ahead) is traced (formats.md,
    // Difficulty) but not ported, so only drag racers fire it here.
    let h = &r.car.car_type.handling;
    let skill = r.speed.skill();
    r.controls.nitrous =
        drag && r.state.ai_nitrous_ok(h, ai.tuning.nitrous_seconds(skill), ai.tuning.nitrous_gear(skill, drag));
    // The controller's think ends a running shot of any AI racer
    // (car +0x14 → +4 = 2) once 0x40D850 fails (0x42AD24..0x42AD55 →
    // 0x592510) and slot +0xB4 (0x408430) allows, which in a drag
    // race (0x89E7D8) it always does; slot +0xA8 (0x41E500) stamps
    // +0x7AC. The request (0x40D850 again) starts the next shot as
    // soon as it passes: a drag rival's tank goes out in several
    // shots (observed natively 24000 → 20137 → 14401 → 1465).
    if drag && r.state.nitrous.as_mut().is_some_and(|t| drag_nitrous_end(t, r.controls.nitrous)) {
        r.belt.nos_end = Some(clock);
    }
    if !drag {
        // Outside drag (0x416F70 → 0x40E390 / 0x420810): the timer
        // +0x7B0, then the request, which starts a shot whenever
        // none runs; the think (0x42ABD0) ends one that has run
        // +0x570 s once the request fails, and an empty tank ends it
        // (0x59FA40); each end stamps +0x7AC.
        let flat = r.route.as_ref().is_some_and(|x| x.nitrous_road().1);
        r.belt.nos_flat = if flat { r.belt.nos_flat + dt } else { 0.0 };
        let ok = r.state.ai_nitrous_ok(h, ai.tuning.nitrous_seconds(skill), ai.tuning.nitrous_gear(skill, false));
        let wanted = ok && ai_nitrous_wanted(r, &ai.tuning, skill, f.laps, clock);
        let min_run = ai.tuning.nitrous_seconds(skill);
        if let Some(tank) = r.state.nitrous.as_mut().filter(|t| t.size > 0) {
            match tank.running {
                None if wanted => {
                    tank.start();
                }
                Some(t) if tank.remaining() <= 0.0 || (t >= min_run && !wanted) => {
                    tank.stop();
                    r.belt.nos_end = Some(clock);
                }
                _ => {}
            }
            // The shot's clock where the physics mover does not run it.
            if r.state.melo.is_some() || r.belt.belt.is_some() {
                tank.advance(dt);
            }
        }
    }
}

/// The player's input this frame: the autopilot's past the line, else the
/// drag steering's in a drag race, else the keys'.
fn race_player_input(d: &Drive, race: &mut Race, player: Controls, tick: RaceTick) -> nfsu2::drive::Input {
    use nfsu2::drive::Input;
    let RaceTick { clock, drag, dt } = tick;
    let mut me = Input::Player(player);
    if let Some(a) = race.autopilot.as_ref() {
        me = Input::Ai(a.controls);
    } else if drag && let Some(routes) = race.racers.ai_routes.as_ref() {
        let steer = race.dragger.get_or_insert_with(|| DragSteer::new(&d.car, &d.state, routes));
        // The player's race coordinate (car +0x8FC), which the lane table
        // 0x40B110 reads through 0x412500.
        if let Some(route) = steer.route.as_mut() {
            route.race_coord = Some(race.racers.me.along);
        }
        if let Some(angle) = steer.steer(&d.car, &d.state, routes, player, clock, dt) {
            me = Input::Drag(player, angle);
        }
    }
    me
}

/// The world's part of a race frame: the traffic's think (after the
/// green), the car field's slice, the world-update samples and the
/// replay's snapshot, then the collision steps (0x609CF0 → 0x5FFA60) with
/// the racers' contact holds, and the traffic's settle. Returns the
/// player's world-update sample.
fn race_world_step(
    d: &mut Drive,
    race: &mut Race,
    me: nfsu2::drive::Input,
    ai: Option<&AiData>,
    rival_samples: &mut [Option<nfsu2::drive::WorldVelocitySample>],
    tick: RaceTick,
    sounds: &mut nfsu2::drive::ContactSounds,
) -> nfsu2::drive::WorldVelocitySample {
    use nfsu2::drive::{Input, Racer, step_world};
    let RaceTick { clock, drag, dt } = tick;
    let mut traffic = if clock < 0.0 { None } else { race.racers.traffic.as_mut() };
    let before = traffic.as_deref_mut().map(|t| {
        let racers: Vec<nfsu2::ai::traffic::Other> =
            race.racers.rivals.iter().map(|r| traffic_body(&r.car, &r.state)).collect();
        // In a drag race the placer takes the drag placement (0x42F800 →
        // 0x42E620): the player's race coordinate (car +0x8FC), its lane,
        // the race clock and the track's start gap (0x418110).
        let place = drag.then(|| DragPlace {
            coord: race.racers.me.along,
            lane: race.dragger.as_ref().and_then(|s| s.lane.lane),
            clock,
            start_line: race.route.start_at,
            start_gap: race.start_gap,
        });
        let coords = RaceCoords {
            me: race.racers.me.along,
            racers: race.racers.rivals.iter().map(|r| r.progress.along).collect(),
        };
        // The traffic's draws are the world generator's: the race
        // field's, lent to the traffic for its think.
        std::mem::swap(&mut t.field.rng, &mut race.racers.cars.rng);
        let before = traffic_think(
            &d.world,
            t,
            &d.state,
            nfsu2::ai::traffic::Other::half_box(&d.car.hull),
            &racers,
            Some(&race.route.road),
            place,
            d.human_road.node,
            // The human's wrong-way byte (+0x248) for 0x42E9F0.
            race.wrong_way,
            Some(&coords),
            dt,
        );
        std::mem::swap(&mut t.field.rng, &mut race.racers.cars.rng);
        before
    });
    let placed: Vec<usize> =
        traffic.as_deref().map_or(Vec::new(), |t| (0..t.cars.len()).filter(|&i| t.cars[i].lane.is_some()).collect());
    // A racer on the BeltMover is moved by it alone (`Input::Belt`: its own
    // frame does nothing), but its body takes part in the collision step.
    let ids: Vec<u64> = std::iter::once(0)
        .chain(1..=race.racers.rivals.len() as u64)
        .chain(placed.iter().map(|&i| TRAFFIC_ID + i as u64))
        .collect();
    race.racers.cars.bind(&ids);
    // The human controller's road (0x42CAA0) in the controllers' pass
    // before the motion, on the race's routes (as in free roam,
    // `step_explore`).
    if let Some(routes) = race.racers.ai_routes.as_ref() {
        let com = belt_car_position(&d.car, &d.state);
        let half = d.car.car_type.handling.dimensions[1] * 0.5;
        let step = d.human_road.update(routes, com, d.state.velocity, d.state.forward(), d.state.speed(), half);
        d.node_counts.0 += step.node_updates;
        d.node_counts.1 += step.node_searches;
    }
    let player_sample = d.state.begin_world_update(dt);
    for (r, sample) in race.racers.rivals.iter().zip(rival_samples.iter_mut()) {
        if sample.is_none() {
            *sample = Some(r.state.begin_world_update(dt));
        }
    }
    // The replay manager's DUMMY_SNAPSHOT (0x57D010 → 0x57C190, before
    // the world update whose counter 0x7FB724 is a multiple of 100; see
    // `CarState::snapshot_take` and docs/notes/parity/tuning-physics-parity.md).
    if race.replay.world_updates % 100 == 0 {
        d.state.snapshot_take();
        for r in race.racers.rivals.iter_mut() {
            r.state.snapshot_take();
        }
    }
    race.replay.world_updates += 1;
    // No car pairs in the start scene or the countdown.
    d.state.start_hold = clock < 0.0;
    for r in race.racers.rivals.iter_mut() {
        r.state.start_hold = clock < 0.0;
    }
    let steps = world_steps(dt);
    let sub = dt / steps as f32;
    for _ in 0..steps {
        d.state.misc_style = true;
        d.state.human = true;
        let mut cars = vec![Racer { car: &d.car, state: &mut d.state, input: me }];
        for r in race.racers.rivals.iter_mut() {
            let input = if r.belt.belt.is_some() {
                Input::Belt(r.controls)
            } else if r.state.melo.is_some() {
                Input::Melo(r.controls)
            } else {
                Input::Ai(r.controls)
            };
            cars.push(Racer { car: &r.car, state: &mut r.state, input });
        }
        if let Some(t) = traffic.as_deref_mut() {
            for c in t.cars.iter_mut().filter(|c| c.lane.is_some()) {
                let input = if c.belt.belt.is_some() {
                    Input::Belt(c.controls)
                } else if c.mover.is_some() {
                    Input::Ai(c.controls)
                } else {
                    Input::Melo(c.controls)
                };
                c.state.traffic = true;
                cars.push(Racer { car: &c.car, state: &mut c.state, input });
            }
        }
        d.posed.extend(step_world(&mut cars, &mut race.racers.cars, &mut d.props, &d.world, sub, sounds));
        // A racer the human's car touched holds the physics mover 5 s more
        // (0x402450).
        for r in &mut race.racers.rivals {
            if std::mem::take(&mut r.state.human_hit) {
                r.hold = HOLD_TICKS;
            }
            // Any resolved contact of a mode-4 car (0x402450): controller
            // +0x730 = 2 s (0x783510), then 0x5F4410's mode where the world
            // under it is loaded (car +0x5CC), else mode 4 still.
            if std::mem::take(&mut r.state.belt_hit) {
                r.belt.block = BELT_CONTACT_BLOCK;
                if streamed_in(
                    &d.world,
                    d.world.visible_sections_at(d.state.position).map(|(_, l)| l),
                    r.state.position,
                ) {
                    let rating = ai.map_or(0.0, |ai| ai.bounds.scale(r.car.car_type.handling.performance_rating));
                    leave_belt(r, d.car.car_type.handling.race, rating);
                }
            }
        }
    }
    if let (Some(t), Some(before)) = (traffic, before) {
        traffic_settle(&d.world, &mut t.cars, &placed, &before, dt);
    }
    player_sample
}

/// The end of a race frame: the racers' contact counts cleared, the
/// world-update samples finished, the chase camera, the rivals' progress
/// and who leaves the traffic's collisions.
fn race_post(
    d: &mut Drive,
    race: &mut Race,
    player_sample: nfsu2::drive::WorldVelocitySample,
    rival_samples: Vec<Option<nfsu2::drive::WorldVelocitySample>>,
    tick: RaceTick,
) -> RaceWorldSamples {
    let RaceTick { clock, drag, dt } = tick;
    // Every other resolved contact of a racer (a car that is not the
    // human's, a wall, a prop: 0x593950 passes a null car) gives the mover
    // 0x5F4410 picks (0x4024B8): 1 while the hold runs or in drag / drift,
    // else 3, which is the mover the racer is already on in every case the
    // clone reaches but a finished racer's (that one would ride the MeloMvr
    // prov: open game.step_race
    // to the end of the frame; **open**, not ported). Nothing else reads
    // the rivals' count.
    for r in &mut race.racers.rivals {
        r.state.resolved = 0;
    }
    d.state.finish_world_update(player_sample);
    d.impact_trigger();
    for (r, sample) in race.racers.rivals.iter_mut().zip(rival_samples.iter().copied()) {
        r.state.finish_world_update(sample.unwrap());
    }
    // The chase eye lifted over the drawn race traffic it sits in (0x44B040).
    d.chase_view_lifted(dt, &traffic_lift_cars(race.racers.traffic.as_ref().filter(|t| !t.paused)));
    for r in &mut race.racers.rivals {
        r.progress.update(&race.route, race.laps, clock + dt, r.state.position.truncate());
        r.progress.track(&r.state);
        if drag {
            r.progress.drag_drop_out(&r.state, clock + dt);
        }
    }
    let mode = d.car.car_type.handling.race;
    for (p, state) in std::iter::once((&race.racers.me, &mut d.state))
        .chain(race.racers.rivals.iter_mut().map(|r| (&r.progress, &mut r.state)))
    {
        if p.leaves_traffic(mode) {
            state.car_collisions = false;
        }
    }
    RaceWorldSamples { player: player_sample, rivals: rival_samples }
}
