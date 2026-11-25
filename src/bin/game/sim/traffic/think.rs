use crate::*;

/// The traffic's part of a frame before the collision step. The placer
/// (0x42F910) counts down `PLACE_INTERVAL`; each pass walks the slots
/// round-robin from where the last stopped and, for the first car that is
/// parked or farther than `DESPAWN` from the player (0x417BB0), tries a
/// placement (`TrafficLane::spawn`, kept `spacing` apart, 0x418240); a car
/// it cannot place is parked (+0x77C). Outside a drag race a pass handles
/// one car. A placed car takes the TrafficMover (0x5F46B0(2)) with the
/// lane's speed. Its driver is the racers' low-level driver, which the
/// traffic controller builds as well (0x426580 → 0x425AB0), handed the
/// path and target speed of the controller's last think
/// (`TrafficLane::think`; the cars think in turns, `think_slot`); the
/// mover moves it by the driver's wheel angle and pedals. `racers` are the
/// race's other cars (position, velocity), which the traffic's avoidance
/// sees as it sees the player. `road` is a race's road: in a coordinate
/// race (0x88F2BE) the placer's distances are race coordinate (0x409890,
/// 0x411060, 0x4112A0 on 0x5D5CE0) for points that have one (0x5DCC90);
/// the clone takes the larger of the 2D distance and the race-coordinate
// prov: inferred game.TrafficFrame
/// difference of the two places located on the road (**inferred**).
/// Returns each car's place and heading before its mover's frame.
pub(crate) struct TrafficFrame {
    pub(crate) poses: Vec<(Vec3, f32)>,
    pub(crate) samples: Vec<Option<nfsu2::drive::WorldVelocitySample>>,
}

pub(crate) fn traffic_think(
    world: &World,
    t: &mut Traffic,
    player: &CarState,
    player_half: Vec2,
    racers: &[nfsu2::ai::traffic::Other],
    road: Option<&nfsu2::formats::roads::RoadRoute>,
    drag: Option<DragPlace>,
    node: Option<(usize, usize)>,
    wrong_way: bool,
    dt: f32,
) -> TrafficFrame {
    use nfsu2::ai::traffic::think_slot;
    t.clock += dt;
    // The intersections let go of the right of way (0x4111C0, each frame).
    t.crossings.tick();
    let mut think = vec![false; t.cars.len()];
    // The traffic's race coordinates (car +0x8FC) move on in steps.
    if let Some(road) = road {
        for c in t.cars.iter_mut().filter(|c| c.lane.is_some()) {
            c.refresh_coord(road);
        }
    }
    t.timer -= dt;
    if t.timer <= 0.0 {
        traffic_place_pass(world, t, player, racers, road, drag, node, wrong_way);
    }
    // The thinks take turns by frame (0x429D90 on the frame loop's counter
    // 0x7FB724, one per call of 0x57A0F0 from the loop at 0x5814FA).
    t.updates += 1;
    if let Some(f) = think_slot(t.updates, t.cars.len()).and_then(|i| think.get_mut(i)) {
        *f = true;
    }
    traffic_stream(world, t, player);
    // The road entries' objects (0x8A2CA4's nodes): the player, the
    // racers and the placed traffic, each with its collision box.
    let others = traffic_others(t, player, player_half, racers);
    let mut before = vec![(Vec3::ZERO, 0.0); t.cars.len()];
    let mut samples = vec![None; t.cars.len()];
    for (i, (c, before)) in t.cars.iter_mut().zip(&mut before).enumerate() {
        let lanes =
            TrafficLanes { routes: &t.routes, crossings: &mut t.crossings, clock: t.clock, rng: &mut t.field.rng };
        traffic_car_frame(world, i, c, before, &mut samples[i], &think, &others, lanes, dt);
    }
    TrafficFrame { poses: before, samples }
}

/// The placer's pass (0x42F910, its timer run out): round-robin over the
/// slots from the last one looked at, each car far from the player (or
/// parked) given a new place (`TrafficLane::spawn_from` /
/// `spawn_drag`) or parked; one car a pass, 16 in a drag race.
fn traffic_place_pass(
    world: &World,
    t: &mut Traffic,
    player: &CarState,
    racers: &[nfsu2::ai::traffic::Other],
    road: Option<&nfsu2::formats::roads::RoadRoute>,
    drag: Option<DragPlace>,
    node: Option<(usize, usize)>,
    wrong_way: bool,
) {
    use nfsu2::ai::traffic::{
        DESPAWN, JAM_CAR, JAM_PLAYER, JAM_REACH, PLACE_DRAG, PLACE_INTERVAL, PLACE_INTERVAL_DRAG, TrafficLane, spacing,
    };
    let me = player.position;
    let dist = |a: Vec2, b: Vec2| match road {
        Some(r) => a.distance(b).max(r.difference(r.locate_once(a), r.locate_once(b)).abs()),
        None => a.distance(b),
    };
    t.timer = if drag.is_some() { PLACE_INTERVAL_DRAG } else { PLACE_INTERVAL };
    let (forward, speed) = (player.orientation * Vec3::X, player.velocity.truncate().length());
    let n = t.cars.len();
    let list = world.visible_sections_at(player.position).map(|(_, l)| l);
    // The player's race coordinate (racer +0x8FC): the drag placement's;
    // in the other races the player's place located on the race road, as
    // the cars' (**inferred**, as `dist`).
    let me_coord = drag.map(|p| p.coord).or_else(|| road.map(|r| r.locate_once(me.truncate())));
    // 0x42F910 draws the spacing (0x418240) for each human racer
    // before it walks the slots, on every pass, whether or not a car is
    // then tried (**verified**, the decompile 0x42F910: the loop over
    // 0x8900BC before the slot loop; its 0x5F0E40(−1) entry is taken as
    // none — the placer's record of 4 floats + 1 int a placing pass have no room for it,
    // **inferred**). 0x4112A0, the jam: the player under 30 m/s
    // (+0x430) and a traffic car under 1 m/s (+0x430) that is not parked
    // (+0x77C) within 250 m of race coordinate (|0x5D5CE0(racer +0x8FC,
    // car +0x8FC)|, no 2D test); in free roam every +0x8FC reads 0, so
    // any such car.
    let jammed = speed < JAM_PLAYER
        && t.cars.iter().any(|c| {
            c.lane.is_some()
                && c.state.velocity.length() < JAM_CAR
                && match (road, me_coord) {
                    (Some(r), Some(mc)) => r.difference(c.race_coord(r), mc).abs() < JAM_REACH,
                    _ => true,
                }
        });
    let gap = spacing(&t.zones, &t.spacing, t.level, me.truncate(), jammed, &mut t.field.rng);
    let mut tries = 0;
    // Round-robin from the slot after the last one looked at (+0x04);
    // until 2026-10-31 the clone stepped from the moving cursor, so a
    // pass skipped slots (1, 2, 3, … apart) and some cars were never
    // looked at.
    let from = t.cursor;
    for k in 1..=n {
        let i = (from + k) % n;
        t.cursor = i;
        // 0x417BB0: a placed car stays while near a player — in a
        // coordinate race 0x410DE0 → 0x409890, |0x5F12B0| < 250 on race
        // coordinate alone (verified; the drag race takes that, with
        // the player's coordinate and the car's +0x8FC, which moves on
        // in steps, `TrafficCar::race_coord`,
        // prov: inferred game.traffic_think
        // **inferred**; the other races keep `dist`).
        // Every race is a coordinate race (0x88F2BE), so 0x410DE0
        // takes 0x409890 on the car's +0x8FC alone in all of them (was
        // the larger of 2D and race coordinate outside drag races). The
        // world's slots 0x5F12B0 measures from are the player's alone
        // (world +0x14 = 1, slot 1 unset: **observed** under Wine).
        let near = match (road, me_coord) {
            (Some(road), Some(mc)) => road.difference(t.cars[i].race_coord(road), mc).abs() < DESPAWN,
            _ => dist(t.cars[i].state.position.truncate(), me.truncate()) <= DESPAWN,
        };
        if t.cars[i].lane.is_some() && near {
            continue;
        }
        // 0x4112A0 measures |0x5D5CE0(player +0x8FC, car +0x8FC)|, and
        // in free roam every car's +0x8FC reads 0 under Wine (player,
        // racers and traffic), so there any placed car under 1 m/s
        // anywhere is a jam (**verified** in memory); in a race the
        // race coordinates (the clone: `dist`).
        // The world under the car's place now (car +0x5C0 / +0x5CC, which
        // 0x609CF0's 0x5F6CE0 loop updates for every car before the thinks).
        let old_loaded = streamed_in(world, list, t.cars[i].state.position);
        // 0x42F800 (verified): which human racer (0x42F812, 0x5EA4F0 on
        // 0x8900BC = 1) and the teleporter odds (0x42F852, 0x5EA510(1.0)
        // under 1.0, or 0.35 under 20 m/s), both drawn before the spacing
        // is looked at; 0x42E790 then returns at once (no teleporters in
        // the retail routes, 0x883DC8 = 0: no draw).
        nfsu2::ai::traffic::placer_try_draws(&mut t.field.rng, 1);
        // 0x42E9F0's keep-side rule reads the car's race coordinate while it
        // is not parked (+0x77C; the clone's parked car has no lane).
        let own =
            (!t.cars[i].held && t.cars[i].lane.is_some()).then(|| road.map(|r| t.cars[i].race_coord(r))).flatten();
        let others: Vec<Vec2> = t
            .cars
            .iter()
            .enumerate()
            .filter(|&(j, c)| j != i && c.lane.is_some())
            .map(|(_, c)| c.state.position.truncate())
            .chain(racers.iter().map(|r| r.position))
            .collect();
        // The try leaves the car as it is; only a placement or the
        // pause (below) changes it. A paused car (+0x77D) the try fails
        // again stays as it is, coasting or not (0x4020D0 returns at once).
        let was = if t.cars[i].held {
            None
        } else {
            let c = &mut t.cars[i];
            Some((c.lane.take(), c.plan.take(), c.mover.take(), c.belt.belt.take()))
        };
        t.crossings.leave(i);
        let placed = match (gap, drag, road) {
            (None, _, _) => None,
            (Some(gap), Some(p), Some(road)) => {
                // 0x418110 on the point (verified): before the green
                // (0x402000 under 0x783BAC = 0) a track whose byte +0xF4
                // + direction is clear refuses a point less than its
                // float +0xF8 + 4·direction past the start line (race
                // status +0xBF8 by +0xBF6 × 0.01 × the road length);
                // then in a race (status 4) one within 250 m of race
                // coordinate of the player (0x409890 → 0x5F12B0), and
                // one a car is near (0x411060: a traffic car, car +0x14
                // → +4 = 3, within the spacing either way; the others —
                // the player, the racers — with the point less than the
                // spacing ahead of them). The traffic's race
                // coordinates are their +0x8FC (`TrafficCar::race_coord`),
                // the racers' their places located on the race road
                // prov: inferred game.traffic_think-2
                // (**inferred**, as `dist` does).
                let coords: Vec<(f32, bool)> = t
                    .cars
                    .iter()
                    .enumerate()
                    .filter(|&(j, c)| j != i && c.lane.is_some())
                    .map(|(_, c)| (c.race_coord(road), true))
                    .chain(racers.iter().map(|r| (road.locate_once(r.position), false)))
                    .chain(std::iter::once((p.coord, false)))
                    .collect();
                let flat: Vec<Vec2> = others.iter().copied().chain(std::iter::once(me.truncate())).collect();
                let gate = |at: Vec2, c: f32, coordinate: bool| {
                    drag_gate(road, &p, p.clock >= 0.0, gap, &coords, &flat, me.truncate(), at, c, coordinate)
                };
                TrafficLane::spawn_drag(&t.routes, me.truncate(), p.coord, p.lane, t.with, &gate, &mut t.field.rng)
            }
            (Some(gap), None, Some(road)) => {
                // A coordinate race outside drag races: 0x42E9F0 with the
                // gate 0x418110 (`drag_gate`, the same function) and its
                // race rules (`TrafficLane::spawn_race`).
                let mc = me_coord.unwrap_or(0.0);
                let p = DragPlace { coord: mc, lane: None, clock: 0.0, start_line: 0.0, start_gap: None };
                let coords: Vec<(f32, bool)> = t
                    .cars
                    .iter()
                    .enumerate()
                    .filter(|&(j, c)| j != i && c.lane.is_some())
                    .map(|(_, c)| (c.race_coord(road), true))
                    .chain(racers.iter().map(|r| (road.locate_once(r.position), false)))
                    .chain(std::iter::once((mc, false)))
                    .collect();
                let flat: Vec<Vec2> = others.iter().copied().chain(std::iter::once(me.truncate())).collect();
                let gate = |at: Vec2, c: f32, coordinate: bool| {
                    drag_gate(road, &p, true, gap, &coords, &flat, me.truncate(), at, c, coordinate)
                };
                let diff = |a: f32, b: f32| road.difference(a, b);
                let race = nfsu2::ai::traffic::RacePlace { diff: &diff, coord: mc, own, gate: &gate, wrong_way };
                TrafficLane::spawn_race(
                    &t.routes,
                    me,
                    forward.truncate().normalize_or_zero(),
                    speed,
                    t.with,
                    gap,
                    &others,
                    &dist,
                    &mut t.field.rng,
                    &mut t.start,
                    true,
                    node,
                    Some(&race),
                )
            }
            (Some(gap), _, _) => TrafficLane::spawn_from(
                &t.routes,
                me,
                forward.truncate().normalize_or_zero(),
                speed,
                t.with,
                gap,
                &others,
                &dist,
                &mut t.field.rng,
                &mut t.start,
                road.is_some(),
                node,
            ),
        };
        t.tries.0 += 1;
        let failed = placed.is_none();
        if let Some((lane, at, heading)) = placed {
            t.tries.1 += 1;
            place_traffic(world, &mut t.cars[i], &t.routes, lane, at, heading);
            // A placed car's race coordinate is its point's (observed).
            t.cars[i].coord = road.map(|r| (r.locate_once(at.truncate()), at.truncate()));
            // Car +0x550: 0x42A050 → 0x402110 on a paused car, then
            // 0x5FBEE0's 0x5F6CE0 (see `TrafficCar::active`).
            let c = &mut t.cars[i];
            c.active = placed_active(c.held, c.active, old_loaded, streamed_in(world, list, c.state.position));
            c.held = false;
            // A placed car waits for its think's turn (`think_slot`):
            // the original's cars switch to the MeloMvr for a car ahead
            // 0.05–0.5 s after placement (p10 / median), i.e. at
            // their first think, not on the placement frame. Until then
            // it drives its lane's path at the lane's speed (the path
            // build without the action choice; **inferred**: what the
            // controller hands the driver before its first think is not
            // traced). The clone once thought at once, so its
            // switches happened on the placement frame.
            let c = &mut t.cars[i];
            let speed = c.mover.map_or(0.0, |m| m.speed);
            let position = c.state.position;
            c.plan = c.lane.as_mut().and_then(|l| l.update(&t.routes, position, speed));
        }
        if failed && let Some((lane, plan, _, _)) = was {
            // 0x42FAAB → 0x4020D0: paused and inactive, mode 0; the walk
            // and the driver's path stay (`TrafficCar::coast`).
            pause_traffic(&mut t.cars[i], lane, plan);
        }
        // A pass tries one car, 16 in a drag race (0x42F910).
        tries += 1;
        if tries >= if drag.is_some() { PLACE_DRAG } else { 1 } {
            break;
        }
    }
}

/// 0x4020D0 on a car not yet paused (**verified**): the body's
/// pause count +0x17 up (0x593100: 0x593990 then refuses every pair, so
/// 0x592E70 drops the car's contacts), mode 0 (0x5F46B0(0): a mover that
/// moves nothing, its slot +0x10 0x50D510 false), inactive (0x5ED5D0(0)),
/// +0x77D. The controller's walk and the driver's path are left alone:
/// the car keeps them in `coast` and the speed in its state (+0x430).
pub(crate) fn pause_traffic(
    c: &mut TrafficCar,
    lane: Option<nfsu2::ai::traffic::TrafficLane>,
    plan: Option<(Vec<Vec3>, f32)>,
) {
    c.held = true;
    c.active = false;
    c.lane = None;
    c.coast = lane;
    c.plan = plan;
    c.mover = None;
    c.belt.belt = None;
    c.state.leave_melo();
}

/// A placed car's +0x550 (see `TrafficCar::active`): a paused car
/// (`held`, +0x77D) is resumed by the controller's slot +0x34 (0x42A050 →
/// 0x402110: body 0x593110(0), then active only when its body passes
/// 0x593990 — the world under its old place loaded, car +0x5CC as this
/// update's 0x5F6CE0 left it); 0x5FBEE0's own 0x5F6CE0 then sees +0x5C0
/// go 0 → 1 when the new place is loaded and the old was not, and 0x408C40
/// → 0x4022D0(1) activates the (no longer paused) car. A car that was not
/// paused keeps its flag through 0x402110.
pub(crate) fn placed_active(held: bool, active: bool, old_loaded: bool, new_loaded: bool) -> bool {
    let active = if held { old_loaded } else { active };
    active || (!old_loaded && new_loaded)
}

/// What a traffic car's frame shares with the others besides the cars:
/// the AI routes, the intersections and the traffic's clock.
struct TrafficLanes<'a> {
    routes: &'a nfsu2::formats::ai_routes::RouteFile,
    crossings: &'a mut nfsu2::ai::traffic::Crossings,
    clock: f32,
    /// The world generator (the traffic's field's), for 0x40D5C0's draws.
    rng: &'a mut nfsu2::opponents::GameRng,
}

/// One placed traffic car's frame before the collision step: the stuck
/// test (0x408DE0), its think on its turn (`traffic_lane_think`), the
/// stop (`traffic_stop`), then its driver and mover (`traffic_drive`).
#[allow(clippy::too_many_arguments)]
fn traffic_car_frame(
    world: &World,
    i: usize,
    c: &mut TrafficCar,
    before: &mut (Vec3, f32),
    sample: &mut Option<nfsu2::drive::WorldVelocitySample>,
    think: &[bool],
    others: &[nfsu2::ai::traffic::Other],
    lanes: TrafficLanes,
    dt: f32,
) {
    use nfsu2::ai::traffic::{STALL_SPEED, STALL_TARGET, STUCK_IMPULSE};
    let TrafficLanes { routes, crossings, clock, rng } = lanes;
    // The contacts the coming collision step resolves (`traffic_settle`).
    c.state.resolved = 0;
    if c.lane.is_none() {
        // A paused car with its walk: no think and no stuck test
        // (0x429D90 returns on +0x77D), its driver on the last path and
        // its mover, with no collision (0x593990 refuses its pairs).
        if c.coast.is_some() {
            traffic_drive(world, c, before, sample, clock, dt);
            if let Some(s) = sample.take() {
                c.state.finish_world_update(s);
            }
        }
        return;
    }
    // 0x408DE0, every update before the turn gate: stuck against
    // something (a record's pushes over 30, or under 1 m/s while the
    // driver wants over 1 m/s, both only while the body has live
    // records) stops the car for good.
    let speed = c.mover.map_or(c.state.velocity.length(), |m| m.speed);
    let mut stop = false;
    let mut why = "";
    if !c.state.contacts.is_empty() {
        let target = c.plan.as_ref().map_or(0.0, |p| p.1);
        if c.state.contacts.iter().any(|n| n.impulse > STUCK_IMPULSE) {
            stop = true;
            why = if c
                .state
                .contacts
                .iter()
                .any(|n| n.impulse > STUCK_IMPULSE && matches!(n.touching, nfsu2::drive::Touching::Car { .. }))
            {
                "impulse-car"
            } else {
                "impulse-track"
            };
        } else if speed < STALL_SPEED && target > STALL_TARGET {
            stop = true;
            why = "stall";
        }
    }
    if !c.stopped
        && !stop
        && (think[i] || c.plan.is_none())
        && traffic_lane_think(c, i, speed, &mut stop, &mut why, others, routes, crossings, rng)
    {
        return;
    }
    // Stopped (+0x710): each think (0x429F2B) and each stuck test aim
    // the driver 1 m ahead on the heading (0x4023E0) at target 0, and
    // the car leaves its intersections (0x413800).
    if stop || (c.stopped && think[i]) {
        traffic_stop(c, i, stop, why, speed, others, crossings, clock);
    }
    traffic_drive(world, c, before, sample, clock, dt);
}

/// A traffic car's think on its turn (`TrafficLane::think`): its plan,
/// the walk's end and dead ends, parking at the walk's end, the switch
/// to the MeloMvr on a new "Avoid Cars" action and the belt's rebuild.
/// Returns true when the car was parked (its frame ends).
#[allow(clippy::too_many_arguments)]
fn traffic_lane_think(
    c: &mut TrafficCar,
    i: usize,
    speed: f32,
    stop: &mut bool,
    why: &mut &'static str,
    others: &[nfsu2::ai::traffic::Other],
    routes: &nfsu2::formats::ai_routes::RouteFile,
    crossings: &mut nfsu2::ai::traffic::Crossings,
    rng: &mut nfsu2::opponents::GameRng,
) -> bool {
    use nfsu2::ai::traffic::Other;
    let Some(lane) = c.lane.as_mut() else { unreachable!("a placed car thinks") };
    let rest: Vec<Other> = others.iter().copied().filter(|o| o.traffic != Some(i)).collect();
    // An inactive car is not listed (`traffic_others`); its own entry
    // (0x6148F0 always keeps the car itself) is built the same way.
    let own = others.iter().find(|o| o.traffic == Some(i)).copied().unwrap_or(Other {
        position: c.state.position.truncate(),
        z: c.state.position.z,
        velocity: c.mover.map_or(c.state.velocity.truncate(), |m| Vec2::from_angle(m.heading) * m.speed),
        forward: c.mover.map_or((c.state.orientation * Vec3::X).truncate(), |m| Vec2::from_angle(m.heading)),
        half: Other::half_box(&c.car.hull),
        traffic: Some(i),
        human: false,
        id: i,
    });
    c.plan = lane.think(routes, i, c.state.position, speed, &own, crossings, &rest, rng);
    // 0x428720 false (the car past the walk's last point, 0x429E6B
    // → 0x429E8D): stopped for good, as is a dead end within 1 s
    // (0x417750). Then 0x40ABB0 parks a car on the walk's last
    // segment whose point's record has +0x0C = 1 (0x429EDF).
    if !*stop && c.plan.is_none() {
        *why = "walk-end";
    } else if !*stop && lane.ends_ahead(routes, c.state.position.truncate(), speed) {
        *why = "dead-end";
    }
    *stop |= c.plan.is_none() || lane.ends_ahead(routes, c.state.position.truncate(), speed);
    if !*stop && lane.parks_at_end(routes) {
        // 0x429F0D → 0x4020D0: paused and inactive, mode 0 (the
        // walk and the path just handed to the driver stay).
        let lane = c.lane.take();
        let plan = c.plan.take();
        pause_traffic(c, lane, plan);
        crossings.leave(i);
        return true;
    }
    // A new "Avoid Cars" action (type 2) switches a car on its
    // TrafficMover to the MeloMvr (0x42A004–0x42A028: types 8, 1, 2
    // with car +0x5CC, mode 2 → 0x5F46B0(3); **verified**, and
    // observed under Wine: cars queued behind the standing player
    // in mode 3). The clone's world is always loaded.
    let took = lane.take_avoid();
    if !*stop && took && c.mover.is_some() {
        c.mover = None;
        c.state.melo = Some(nfsu2::melo::Melo::new(&c.car, &c.state, c.rating));
    }
    // The think's end hands the driver its path (0x422150) and, on
    // mode 4, rebuilds the belt from it (0x4221A2 → 0x5918B0).
    if c.belt.belt.is_some()
        && let Some(plan) = c.plan.as_ref()
    {
        let speed = traffic_belt_speed(c);
        build_belt_state(&c.car, &c.state, &mut c.belt, &*c.accel, speed, &plan.0, plan.1);
    }
    false
}

/// A stopped traffic car (+0x710): the stop's log line when it stops,
/// out of its intersections, its driver aimed 1 m ahead at target 0.
#[allow(clippy::too_many_arguments)]
fn traffic_stop(
    c: &mut TrafficCar,
    i: usize,
    stop: bool,
    why: &str,
    speed: f32,
    others: &[nfsu2::ai::traffic::Other],
    crossings: &mut nfsu2::ai::traffic::Crossings,
    clock: f32,
) {
    if stop && !c.stopped {
        let me = c.state.position.truncate();
        let near = others
            .iter()
            .filter(|o| o.traffic != Some(i))
            .min_by(|a, b| a.position.distance(me).total_cmp(&b.position.distance(me)));
        let near = near.map_or(String::from("-"), |o| {
            format!(
                "{}:{:.1}",
                if o.human {
                    "player".to_string()
                } else if let Some(j) = o.traffic {
                    format!("t{j}")
                } else {
                    "racer".to_string()
                },
                o.position.distance(me)
            )
        });
        let imp = c.state.contacts.iter().map(|n| n.impulse).fold(0.0f32, f32::max);
        // The hardest record's age (seconds since it was made).
        let life = c.state.contacts.iter().max_by(|a, b| a.impulse.total_cmp(&b.impulse)).map_or(0.0, |n| n.life);
        traffic_stop_log(
            clock,
            i,
            &c.state,
            speed,
            c.belt.belt.is_some(),
            c.mover.is_some(),
            &format!("{why}\t{near}\t{imp:.1}\t{life:.2}"),
        );
    }
    c.stopped = true;
    crossings.leave(i);
    let h = c.state.heading();
    c.plan = Some((vec![c.state.position + Vec3::new(h.cos(), h.sin(), 0.0)], 0.0));
}

/// A traffic car's driver (0x421030 with the car's mover and kind) and
/// its mover's frame: the BeltMover (mode 4) or the TrafficMover, which
/// places it on the road; a car without a plan only samples.
fn traffic_drive(
    world: &World,
    c: &mut TrafficCar,
    before: &mut (Vec3, f32),
    sample: &mut Option<nfsu2::drive::WorldVelocitySample>,
    clock: f32,
    dt: f32,
) {
    use nfsu2::ai::driver::DriverInputs;
    let Some((path, target)) = c.plan.as_ref() else {
        *sample = Some(c.state.begin_world_update(dt));
        return;
    };
    *before = (c.state.position, c.state.heading());
    let h = &c.car.car_type.handling;
    let input = DriverInputs {
        path,
        target_speed: *target,
        enabled: true,
        green: true,
        countdown: false,
        since_green: clock,
        clock: f64::from(clock),
    };
    // The driver reads the car's mover (+0x28: 2 the TrafficMover, 3
    // the MeloMvr after a hit) and kind (+0x24 = 4, traffic), which pick
    // its mode (0x421030, 0x415F80).
    c.driver.mover = if c.belt.belt.is_some() {
        4
    } else if c.mover.is_some() {
        2
    } else if c.coast.is_some() {
        // A paused car's mode 0.
        0
    } else {
        3
    };
    c.driver.traffic = true;
    c.controls = c.driver.update(&c.state, h, c.car.car_type.suv, &input, dt);
    // After placement/controller/driver, before TrafficMover motion.
    *sample = Some(c.state.begin_world_update(dt));
    if c.belt.belt.is_some() {
        // Mode 4 (0x59EC90) over an unloaded world: no drop test.
        let plan = (path.clone(), *target);
        step_belt_state(world, &c.car, &mut c.state, &mut c.belt, &*c.accel, &plan, false, dt);
    } else if let Some(m) = c.mover.as_mut() {
        let wheels = &c.car.car_type.wheels;
        let ctl = c.controls;
        m.step(
            wheels[3].position.x,
            wheels[0].position.x - wheels[3].position.x,
            ctl.steer_degrees.to_radians(),
            ctl.gas,
            ctl.brake,
            dt,
        );
        // The car update's brake-light step (0x5F62C0) runs on the
        // TrafficMover too (the driver's pedal, +0x214).
        c.state.note_brake(ctl.brake, dt);
        let m = *m;
        place_on_mover(world, &c.car, &mut c.state, &m, *before, dt);
    }
}

/// Harness knob `NFSU2_TRAFFIC_STOPLOG=<file>` (not the game's): one line
/// per traffic car stopped for good (+0x710) — race clock, car, place,
/// speed, mover and which rule stopped it.
pub(crate) fn traffic_stop_log(clock: f32, i: usize, s: &CarState, speed: f32, belt: bool, mover: bool, why: &str) {
    use std::io::Write;
    pub(crate) static LOG: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> = std::sync::OnceLock::new();
    let log = LOG.get_or_init(|| {
        debug_env().traffic_stoplog.clone().and_then(|p| std::fs::File::create(p).ok()).map(std::sync::Mutex::new)
    });
    if let Some(f) = log
        && let Ok(mut f) = f.lock()
    {
        let mode = if belt {
            4
        } else if mover {
            2
        } else {
            3
        };
        let _ = writeln!(f, "{clock:.2}\t{i}\t{:.1}\t{:.1}\t{speed:.2}\t{mode}\t{why}", s.position.x, s.position.y);
    }
}

/// A traffic car's speed for its belt's build (car +0x42C): the running
/// belt's, else (a belt that cannot run) the car's.
pub(crate) fn traffic_belt_speed(c: &TrafficCar) -> f32 {
    match c.belt.belt.as_ref() {
        Some(b) if b.valid => b.speed(),
        _ => c.state.exported_forward_speed(),
    }
}

/// The road entries' objects (0x8A2CA4's nodes): the player, the racers
/// and the placed traffic, each with its collision box.
pub(crate) fn traffic_others(
    t: &Traffic,
    player: &CarState,
    player_half: Vec2,
    racers: &[nfsu2::ai::traffic::Other],
) -> Vec<nfsu2::ai::traffic::Other> {
    use nfsu2::ai::traffic::Other;
    let me = player.position;
    let mut others: Vec<Other> = vec![Other {
        position: me.truncate(),
        z: me.z,
        velocity: player.velocity.truncate(),
        forward: (player.orientation * Vec3::X).truncate(),
        half: player_half,
        traffic: None,
        human: true,
        id: usize::MAX,
    }];
    others.extend(racers.iter().enumerate().map(|(k, r)| Other { traffic: None, id: usize::MAX - 1 - k, ..*r }));
    // Only active cars (+0x550, 0x6148F0; see `TrafficCar::active`); a
    // car's own entry is taken from its own state when it is not listed.
    others.extend(t.cars.iter().enumerate().filter(|(_, c)| c.road_object()).map(|(j, c)| Other {
        position: c.state.position.truncate(),
        z: c.state.position.z,
        velocity: c.mover.map_or(c.state.velocity.truncate(), |m| Vec2::from_angle(m.heading) * m.speed),
        forward: c.mover.map_or((c.state.orientation * Vec3::X).truncate(), |m| Vec2::from_angle(m.heading)),
        half: Other::half_box(&c.car.hull),
        traffic: Some(j),
        human: false,
        id: j,
    }));
    others
}

/// The traffic's mover by the world under each car (**verified**
/// in the disassembly, the stream rule `streamed_in` **observed**, see
/// formats.md "BeltMover"): 0x5F46B0 turns any request for a mode other
/// than 0 or 4 into mode 4 for a traffic car (+0x24 ≠ 1) while car +0x5C0
/// is clear (the world under it not loaded), so a car placed or filled
/// there runs on the BeltMover; 0x5F6CE0, when +0x5C0 changes, calls the
/// controller's slot +0x10 (0x408C40): loaded → 0x5F46B0(0x5F4410(car)) =
/// mode 2, the TrafficMover (from mode 4, also for a car that was on the
/// MeloMvr before it left the loaded world); not loaded → 0x4022D0(0):
/// the mover's slot +0x10 (0x570A70, true for the TrafficMover and the
/// MeloMvr) → mode 4. A belt is built from the driver's path at the switch
/// (the ctor 0x5A9040 → 0x5918B0) and again at the end of each think
/// (0x422150 → 0x5918B0 while on mode 4). Under Wine (
/// `circuit_traffic`) 11 of 12 race cars ran mode 4 at the green at
/// 5–10 m/s; the clone once kept every traffic car on its
/// TrafficMover.
pub(crate) fn traffic_stream(world: &World, t: &mut Traffic, player: &CarState) {
    use nfsu2::ai::traffic::TrafficMover;
    let list = world.visible_sections_at(player.position).map(|(_, l)| l);
    for c in t.cars.iter_mut().filter(|c| c.lane.is_some()) {
        let loaded = streamed_in(world, list, c.state.position);
        // The world under it loading (+0x5C0 0 → 1, 0x5F6CE0 → 0x408C40 →
        // 0x4022D0(1)) makes a car that is not paused active.
        if loaded && !c.state.streamed && !c.held {
            c.active = true;
        }
        // An unloaded car's body meets nothing (0x593990).
        c.state.streamed = loaded;
        match (loaded, c.belt.belt.is_some()) {
            (false, false) => {
                let speed = c.mover.map_or(c.state.exported_forward_speed(), |m| m.speed);
                c.mover = None;
                c.state.leave_melo();
                let plan = c.plan.clone().unwrap_or_default();
                build_belt_state(&c.car, &c.state, &mut c.belt, &*c.accel, speed, &plan.0, plan.1);
                if !c.belt.belt.as_ref().is_some_and(|b| b.valid) {
                    // A belt that cannot run (no path: a filled car before
                    // its first think) stops the car (0x59EC90's else:
                    // +0x70, +0x42C, +0x430 = 0).
                    c.state.velocity = Vec3::ZERO;
                    c.state.set_exported_forward_speed(0.0);
                }
            }
            (true, true) => {
                let speed = traffic_belt_speed(c);
                c.belt.belt = None;
                c.state.leave_melo();
                c.mover =
                    Some(TrafficMover { position: c.state.position.truncate(), heading: c.state.heading(), speed });
            }
            _ => {}
        }
    }
    // A paused car with its walk (`TrafficCar::coast`) on a change
    // of the world under it (+0x5C0, 0x5F6CE0 → 0x408C40): loaded →
    // 0x4022D0(1) leaves it inactive (+0x77D) and 0x5F46B0(0x5F4410) gives
    // it the TrafficMover at its speed (+0x430), from mode 0 or 4; not
    // loaded → 0x4022D0(0): the TrafficMover's slot +0x10 (0x570A70) is
    // true, so mode 4, the belt built from the driver's path; mode 0's
    // (0x50D510) is false, so a car paused there stays put. **Observed**
    // (`docs/evidence/traf-3/native-paused-coast.txt`): mode 0 → 2 at
    // 20.39 m/s, inactive, then 4.
    for c in t.cars.iter_mut().filter(|c| c.lane.is_none() && c.coast.is_some()) {
        let loaded = streamed_in(world, list, c.state.position);
        if loaded == c.state.streamed {
            continue;
        }
        c.state.streamed = loaded;
        if loaded {
            let speed = traffic_belt_speed(c);
            c.belt.belt = None;
            c.state.leave_melo();
            c.mover = Some(TrafficMover { position: c.state.position.truncate(), heading: c.state.heading(), speed });
        } else if let Some(m) = c.mover.take() {
            let plan = c.plan.clone().unwrap_or_default();
            build_belt_state(&c.car, &c.state, &mut c.belt, &*c.accel, m.speed, &plan.0, plan.1);
            if !c.belt.belt.as_ref().is_some_and(|b| b.valid) {
                c.state.velocity = Vec3::ZERO;
                c.state.set_exported_forward_speed(0.0);
            }
        }
    }
}

/// The race's traffic before the green (2026-10-31, race7; **observed**
/// under Wine in a Quick Race drag, memory log `race7-tmp/probe7.log`,
/// and **verified** in the disassembly unless marked): the start scene's
/// pause (0x4099B0) switches each car to mode 0 and stops its think, but
/// the car then gets its TrafficMover back when the world under it loads
/// (0x5F6CE0: car +0x5C0 0 → 1 calls the controller's slot +0x10, 0x408C40
/// → 0x5F46B0(0x5F4410(car)) = mode 2; the log shows mode 4 at the load,
/// then mode 2, vtable 0x78456C, through the scene), and before race
/// status 4 every car's brake input (car +0x30 → +0x214) is held at 1
/// (0x5F8B00, from 0x5FBBF0; the log: brake 1.00, gas 0.00 from the load
/// to race state 3, when the resumed think sets gas 1). So each car brakes
/// on its TrafficMover (0x408820) from the fill's lane speed to a stop:
/// 20 m/s → 0 over ~12.1 m by the braking table, as logged (14.28, 9.61,
/// 6.88, 4.15, 1.92, 0.09 m/s 0.24 s apart; 12.2 m in race5's log). The
/// steering is the controller's last (none since the fill: straight).
/// `gas` is 0 through the scene and 1 in the countdown (race state 3, the
/// resumed think's, logged), where 1 × accel − 1 × brake keeps a stopped
/// car stopped; a car still rolling there (the scene skipped with Enter)
// prov: inferred game.traffic_coast
/// keeps braking (**inferred**: the resumed think's steering is not
/// modelled).
pub(crate) fn traffic_coast(
    world: &World,
    t: &mut Traffic,
    player: &CarState,
    player_half: Vec2,
    racers: &[nfsu2::ai::traffic::Other],
    gas: f32,
    dt: f32,
) {
    // The world under each car picks its mover (`traffic_stream`): in the
    // scene a car over an unloaded world stays in the pause's mode 0
    // (0x4099B0; it moves nothing); from race state 3 (the countdown,
    // `gas` 1) 0x409A30's 0x5F4410 mode 2 becomes mode 4 there, and the
    // resumed think drives its belt (**observed** under Wine: 11 of
    // 12 circuit cars on mode 4 at 5–10 m/s at the green).
    traffic_stream(world, t, player);
    if gas > 0.0 {
        use nfsu2::ai::traffic::{Other, think_slot};
        t.updates += 1;
        let slot = think_slot(t.updates, t.cars.len());
        let others = traffic_others(t, player, player_half, racers);
        for (i, c) in t.cars.iter_mut().enumerate() {
            if c.belt.belt.is_none() {
                continue;
            }
            let speed = traffic_belt_speed(c);
            let Some(lane) = c.lane.as_mut() else { continue };
            if !c.stopped && slot == Some(i) {
                let rest: Vec<Other> = others.iter().copied().filter(|o| o.traffic != Some(i)).collect();
                let own = others.iter().find(|o| o.traffic == Some(i)).copied().unwrap_or(Other {
                    traffic: Some(i),
                    id: i,
                    ..traffic_body(&c.car, &c.state)
                });
                if let Some(plan) =
                    lane.think(&t.routes, i, c.state.position, speed, &own, &mut t.crossings, &rest, &mut t.field.rng)
                {
                    lane.take_avoid();
                    build_belt_state(&c.car, &c.state, &mut c.belt, &*c.accel, speed, &plan.0, plan.1);
                    c.plan = Some(plan);
                }
            }
            let sample = c.state.begin_world_update(dt);
            let plan = c.plan.clone().unwrap_or_default();
            step_belt_state(world, &c.car, &mut c.state, &mut c.belt, &*c.accel, &plan, false, dt);
            c.state.finish_world_update(sample);
        }
    }
    for c in t.cars.iter_mut().filter(|c| c.lane.is_some()) {
        let Some(m) = c.mover.as_mut() else { continue };
        let before = (c.state.position, c.state.heading());
        let sample = c.state.begin_world_update(dt);
        let wheels = &c.car.car_type.wheels;
        m.step(
            wheels[3].position.x,
            wheels[0].position.x - wheels[3].position.x,
            c.controls.steer_degrees.to_radians(),
            gas,
            1.0,
            dt,
        );
        c.state.note_brake(1.0, dt);
        let m = *m;
        place_on_mover(world, &c.car, &mut c.state, &m, before, dt);
        c.state.finish_world_update(sample);
    }
}

/// A traffic car's identity in a car field's slice ([`nfsu2::drive::CarField::bind`]):
/// this plus its slot (the player is 0, the racers 1, 2, …).
pub(crate) const TRAFFIC_ID: u64 = 1000;

/// The traffic's part of a frame after the collision step: a car on its
/// TrafficMover is put back on it unless the step resolved a contact of
/// it. Each contact the step resolves (0x5FFA60's loop → 0x5B2620),
/// with a wall, a prop or a car, first tells both bodies (car body vtable
/// 0x7A2478 slot +0x48, 0x593950 → 0x5F4970 → the controller's slot
/// +0x38, 0x408ED0, which ignores the other body); that switches a
/// traffic car over loaded world collision (car
/// +0x5CC, always in the clone) to mode 3, the MeloMvr (0x5B6D10,
/// `nfsu2::melo`), which drives it until it is placed again, as the game
/// keeps it in mode 3 until then (0x42C930) in a world that stays loaded.
pub(crate) fn traffic_settle(world: &World, cars: &mut [TrafficCar], placed: &[usize], before: &TrafficFrame, dt: f32) {
    for &i in placed {
        let c = &mut cars[i];
        let hit = std::mem::take(&mut c.state.resolved) > 0;
        // A contact of a mode-4 car: 0x408ED0 asks for mode 3, which
        // 0x5F46B0 turns back into 4 over an unloaded world (no change, the
        // belt runs on).
        c.state.belt_hit = false;
        if let Some(m) = c.mover {
            if hit {
                c.mover = None;
                c.state.melo = Some(nfsu2::melo::Melo::new(&c.car, &c.state, c.rating));
            } else {
                place_on_mover(world, &c.car, &mut c.state, &m, before.poses[i], dt);
            }
        }
        if let Some(sample) = before.samples[i] {
            c.state.finish_world_update(sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::placed_active;

    /// Car +0x550 at a placement (0x5FBEE0 → 0x42A050 → 0x402110,
    /// then 0x5F6CE0 → 0x408C40 → 0x4022D0(1); body test 0x593990): a car
    /// the placer had parked (0x42FAAB → 0x4020D0) comes back inactive when
    /// the world under both its old and its new place is unloaded, and
    /// active otherwise; a car that was not parked keeps its flag unless
    /// the world under it loads.
    #[test]
    fn a_parked_car_placed_over_unloaded_world_stays_inactive() {
        // held, old unloaded, new unloaded: inactive (observed under Wine:
        // every placement from parked onto mode 4 read +0x550 = 0).
        assert!(!placed_active(true, false, false, false));
        // held, old loaded: 0x402110 activates it.
        assert!(placed_active(true, false, true, false));
        assert!(placed_active(true, false, true, true));
        // held, old unloaded, new loaded: +0x5C0 0 → 1 activates it.
        assert!(placed_active(true, false, false, true));
        // not held: 0x402110 does nothing; only a load activates.
        assert!(!placed_active(false, false, false, false));
        assert!(placed_active(false, false, false, true));
        assert!(placed_active(false, true, false, false));
    }
}

/// Probe (NFSU2_TRAFFIC_CONTACTLOG): every car-car contact record that
/// involves a traffic car, written when it ends: the clock it was first
/// seen, the kinds of the two cars (P the player, R an encounter car, T
/// traffic) and its largest summed pushes (+0x60). `encounters` is the
/// count of encounter slots (their identities follow the player's).
pub(crate) fn traffic_contact_log(clock: f32, field: &nfsu2::drive::CarField, encounters: usize) {
    use std::collections::HashMap;
    use std::io::Write;
    type Live = HashMap<u64, (String, f32, f32)>;
    static LOG: std::sync::OnceLock<Option<std::sync::Mutex<(std::fs::File, Live)>>> = std::sync::OnceLock::new();
    let log = LOG.get_or_init(|| {
        debug_env()
            .traffic_contactlog
            .clone()
            .and_then(|p| std::fs::File::create(p).ok())
            .map(|f| std::sync::Mutex::new((f, HashMap::new())))
    });
    let Some(m) = log else { return };
    let Ok(mut g) = m.lock() else { return };
    let (f, live) = &mut *g;
    let kind = |id: u64| {
        if id == 0 {
            'P'
        } else if id <= encounters as u64 {
            'R'
        } else {
            'T'
        }
    };
    let mut seen = std::collections::HashSet::new();
    for (a, b, rec, imp) in field.records() {
        let (ka, kb) = (kind(a), kind(b));
        if ka != 'T' && kb != 'T' {
            continue;
        }
        seen.insert(rec);
        let mut k = [ka, kb];
        k.sort_unstable();
        let e = live.entry(rec).or_insert((format!("{}-{}", k[0], k[1]), 0.0, clock));
        e.1 = e.1.max(imp);
    }
    let gone: Vec<u64> = live.keys().copied().filter(|r| !seen.contains(r)).collect();
    for r in gone {
        if let Some((k, imp, t0)) = live.remove(&r) {
            let _ = writeln!(f, "rec\t{t0:.2}\t{k}\t{imp:.1}");
        }
    }
}
