use crate::*;

/// One explore frame's cars: the traffic's think (seeing the encounter
/// racers placed as racers, as a race's traffic sees the rivals), one
/// collision step for the player, the encounter racers on the road and the
/// traffic (0x609CF0 → 0x5FFA60) in the traffic's car field (the
/// encounters' own without traffic), then `traffic_settle`.
pub(crate) fn step_explore(
    d: &mut Drive,
    t: Option<&mut Traffic>,
    e: Option<&mut Encounters>,
    player: Controls,
    dt: f32,
    rng: &mut nfsu2::opponents::GameRng,
    sounds: &mut nfsu2::drive::ContactSounds,
) {
    step_explore_traced(d, t, e, player, dt, None, rng, sounds);
}

/// One encounter recovery placement as the post-motion hook consumed it:
/// the world step, the steps in the frame, the velocity before and after.
pub(crate) type RecoveryPhase = (u32, u32, Vec3, Vec3);

/// [`step_explore`] with a trace sink for the encounter recovery
/// placements (the tests'; None in the game). With a sink each placement
/// is also checked to keep the post-motion nitrous tank.
pub(crate) fn step_explore_traced(
    d: &mut Drive,
    mut t: Option<&mut Traffic>,
    mut e: Option<&mut Encounters>,
    player: Controls,
    dt: f32,
    mut trace: Option<&mut Vec<RecoveryPhase>>,
    rng: &mut nfsu2::opponents::GameRng,
    sounds: &mut nfsu2::drive::ContactSounds,
) {
    use nfsu2::drive::{Input, Racer, step_world};
    if t.is_none() && e.as_ref().is_none_or(|e| e.cars.iter().all(|c| !c.active)) {
        d.state.near.clear();
        d.step(player, dt, rng, sounds);
        d.roam.save_place(&d.state, &d.world);
        return;
    }
    let racers: Vec<nfsu2::ai::traffic::Other> = e
        .as_deref()
        .map_or(Vec::new(), |e| e.cars.iter().filter(|c| c.active).map(|c| traffic_body(&c.car, &c.state)).collect());
    let before = t.as_deref_mut().map(|t| {
        traffic_think(
            &d.world,
            t,
            &d.state,
            nfsu2::ai::traffic::Other::half_box(&d.car.hull),
            &racers,
            None,
            None,
            d.human_road.node,
            false,
            dt,
        )
    });
    let placed: Vec<usize> =
        t.as_deref().map_or(Vec::new(), |t| (0..t.cars.len()).filter(|&i| t.cars[i].lane.is_some()).collect());
    // The slice's cars by identity, so the field's pairs follow their cars
    // as traffic is placed and parked.
    let ids: Vec<u64> = std::iter::once(0)
        .chain(
            e.as_deref()
                .into_iter()
                .flat_map(|e| e.cars.iter().enumerate().filter(|(_, c)| c.active).map(|(j, _)| 1 + j as u64)),
        )
        .chain(placed.iter().map(|&i| TRAFFIC_ID + i as u64))
        .collect();
    match (t.as_deref_mut(), e.as_deref_mut()) {
        (Some(t), _) => t.field.bind(&ids),
        (None, Some(e)) => e.field.bind(&ids),
        _ => {}
    }
    // The human controller's road (0x42CAA0 → 0x40A390 / 0x42A190)
    // on the free-roam routes, in the controllers' pass before the motion,
    // from the car's centre of mass (+0x60); half
    // the body's width stands for 0x5EC260's (+0xF4 − +0xE4) / 2.
    if let Some(e) = e.as_deref() {
        let com = belt_car_position(&d.car, &d.state);
        let half = d.car.car_type.handling.dimensions[1] * 0.5;
        let step = d.human_road.update(&e.routes, com, d.state.velocity, d.state.forward(), d.state.speed(), half);
        d.node_counts.0 += step.node_updates;
        d.node_counts.1 += step.node_searches;
    }
    let player_sample = d.state.begin_world_update(dt);
    let encounter_samples: Vec<_> = e.as_ref().map_or(Vec::new(), |e| {
        e.cars.iter().enumerate().filter(|(_, c)| c.active).map(|(i, c)| (i, c.state.begin_world_update(dt))).collect()
    });
    if let Some(e) = e.as_deref_mut() {
        let list = d.world.visible_sections_at(d.state.position).map(|(_, l)| l);
        for c in e.cars.iter_mut().filter(|c| c.active && c.belt.belt.is_some()) {
            let loaded = streamed_in(&d.world, list, c.state.position);
            if step_belt_state(&d.world, &c.car, &mut c.state, &mut c.belt, &*c.accel, &c.driver_plan, loaded, dt) {
                c.belt_block_ticks = 4000; // 591550 drop notification:1s
            }
        }
    }
    let steps = world_steps(dt);
    let sub = dt / steps as f32;
    for step in 0..steps {
        if let Some(e) = e.as_deref_mut() {
            let list = d.world.visible_sections_at(d.state.position).map(|(_, l)| l);
            for c in e.cars.iter_mut().filter(|c| c.active) {
                let loaded = streamed_in(&d.world, list, c.state.position);
                // An unloaded car's body meets nothing (0x593990).
                c.state.streamed = loaded;
                encounter_belt_contact(c, loaded);
            }
        }

        d.state.misc_style = true;
        let mut cars = vec![Racer { car: &d.car, state: &mut d.state, input: Input::Player(player) }];
        let mut field = None;
        let consume_recovery =
            step + 1 == steps && e.as_ref().is_some_and(|e| e.cars.iter().any(|c| c.active && c.recovery_pending));
        let mut recoveries = Vec::new();
        if consume_recovery {
            recoveries.push(None);
        }
        let mut history = None;
        if let Some(e) = e.as_deref_mut() {
            for (i, c) in e.cars.iter_mut().enumerate().filter(|(_, c)| c.active) {
                let input = if c.belt.belt.is_some() {
                    Input::Belt(c.controls)
                } else if c.state.melo.is_some() {
                    Input::Melo(c.controls)
                } else {
                    Input::Ai(c.controls)
                };
                cars.push(Racer { car: &c.car, state: &mut c.state, input });
                if consume_recovery {
                    recoveries.push(c.recovery_pending.then(|| EncounterReset {
                        belt: &mut c.belt,
                        driver_plan: &mut c.driver_plan,
                        route: &mut c.route,
                        driver: &mut c.driver,
                        controller: &mut c.controller,
                        controls: &mut c.controls,
                        pending: &mut c.recovery_pending,
                        cap: c.cap,
                        world_id: (i + 1) as i16,
                        routes: &e.routes,
                    }));
                }
            }
            field = Some(&mut e.field);
            history = Some(&mut e.road_prev);
        }
        if let Some(t) = t.as_deref_mut() {
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
            field = Some(&mut t.field);
        }
        if let Some(field) = field {
            // Existing outer subdivision/contact cadence is retained. A
            // request waits for complete outer motion, then runs before the
            // final contact pass and acceleration publication.
            if !consume_recovery {
                d.posed.extend(step_world(&mut cars, field, &mut d.props, &d.world, sub, sounds));
                continue;
            }
            d.posed.extend(nfsu2::drive::step_world_with_post_motion(
                &mut cars,
                field,
                &mut d.props,
                &d.world,
                sub,
                sounds,
                |i, car| {
                    if let Some(mut reset) = recoveries.get_mut(i).and_then(Option::take) {
                        let before = trace.is_some().then(|| (car.state.velocity, car.state.nitrous));
                        if reset.apply(&d.world, car.car, car.state) {
                            if let (Some(trace), Some(before)) = (trace.as_deref_mut(), before) {
                                assert_eq!(car.state.nitrous, before.1, "placement preserves the post-motion tank");
                                trace.push((step, steps, before.0, car.state.velocity));
                            }
                            car.input = Input::Ai(*reset.controls);
                            if let Some(history) = history.as_deref_mut() {
                                history.insert(reset.world_id as usize, car.state.velocity);
                            }
                        }
                    }
                },
            ));
        }
    }
    if let (Some(t), Some(before)) = (t.as_deref_mut(), before) {
        traffic_settle(&d.world, &mut t.cars, &placed, &before, dt);
    }
    d.state.finish_world_update(player_sample);
    d.impact_trigger();
    if let Some(e) = e.as_deref_mut() {
        for (i, sample) in encounter_samples {
            let c = &mut e.cars[i];
            let loaded =
                streamed_in(&d.world, d.world.visible_sections_at(d.state.position).map(|(_, l)| l), c.state.position);
            encounter_belt_contact(c, loaded);
            c.state.finish_world_update(sample);
        }
    }
    // The chase eye lifted over the drawn traffic it sits in (0x44B040).
    d.chase_view_lifted(dt, &traffic_lift_cars(t.as_deref()));
    // The racer's saved places (0x602AEE), as in a race (`Progress::save_place`).
    d.roam.save_place(&d.state, &d.world);
    // The world clock (0x7FB718), the sky cycle (0x60FAB0) and the rain run
    // on every in-world frame, as `Drive::step` does; this path had left
    // them out, so free roam with traffic stayed on the sunrise sky.
    d.tick_clock(dt, rng);
}
