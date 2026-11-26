use crate::*;

/// One race frame after the green light, live or replayed: the cars, the
/// clock and the player's run, drift scoring and the wrong-way test; past
/// the line an AI racer controller drives the player's car (`Autopilot`). A replayed frame first
/// applies the recorded shift / reset (live, the key did it already).
pub(crate) fn race_frame(
    d: &mut Drive,
    race: &mut Race,
    frame: &ReplayFrame,
    ai: Option<&AiData>,
    triggers: &career::RouteTriggers,
    lap_info: bool,
    replaying: bool,
    rng: &mut nfsu2::opponents::GameRng,
    sounds: &mut nfsu2::drive::ContactSounds,
) {
    let dt = frame.dt;
    if replaying {
        if let Some(up) = frame.shift {
            d.state.shift(&d.car, up);
        }
        if frame.reset {
            reset_to_road(d, race);
        }
    }
    if race.racers.me.finished.is_none() {
        let tank_before = d.state.nitrous.map(|t| t.running.is_some());
        step_race(d, race, frame.controls, ai, dt, sounds);
        race.update(d.state.position.truncate(), dt);
        race.racers.me.track(&d.state);
        let v = d.state.speed();
        if v > 0.0 && v < 1000.0 {
            race.racers.me.odometer += dt * v;
        }
        race.racers.me.save_place(&d.state, &d.world);
        race.drift_frame(&d.state, &d.car, dt);
        race.style_frame(d, triggers.shortcut_at(d.state.position.truncate()), lap_info, tank_before, dt);
        race.drift_rivals(dt);
        race.ai_refill();
        if race.drag {
            race.racers.me.drag_drop_out(&d.state, race.timing.clock);
        }
        // 0x600720 first looks up the route triggers of kinds 0–3 at the
        // road locator's point (0x5DED20): a kind-3 one (the only kind of
        // the retail data) places the car (0x5FBE10) and answers "not
        // wrong way". The locator's point (+0x08) is the car's place when
        // it last moved on (0x5DA340); the clone takes the car's place.
        let block = race.racers.ai_routes.as_ref().and_then(|r| {
            r.blocks.iter().find(|z| z.trigger.contains(d.state.position.truncate())).and_then(|z| z.place())
        });
        if let Some((at, heading)) = block {
            let z = d.world.road_below(at.extend(d.state.position.z + 3.0)).map_or(d.state.position.z, |(z, _)| z);
            d.place_reset(at.extend(z) + Vec3::Z * 5.0, heading);
            d.state.set_placement_speed(RESET_SPEED);
            // 0x5FBE10 places with the stamp flag (0x5FBE98 pushes 1).
            d.stamp_placement();
        }
        race.wrong_way = match road_heading(race).filter(|_| block.is_none()) {
            Some(road) => {
                wrong_way(race.wrong_way, d.state.heading(), road, d.state.velocity.truncate(), d.state.wheels_down())
            }
            None => false,
        };
        // The finish of the local player (0x606CC0 → 0x5EA540, once, its
        // last instruction a jump to 0x592510 on the car's tank) ends a
        // running nitrous shot: the key handlers are removed right after
        // (0x605C80), so no release would end it.
        if race.racers.me.finished.is_some()
            && let Some(t) = d.state.nitrous.as_mut()
        {
            t.stop();
        }
    } else {
        // Past the line an AI racer controller drives the player's car
        // (`Autopilot`); in a drift race its driver is the DriftDriver
        // (0x425E40). A dyno test run ends otherwise (0x600406), so it
        // still coasts to a stop (**inferred**, not traced).
        let drift = d.car.car_type.handling.race.drift();
        if race.autopilot.is_none()
            && race.test.is_none()
            && let Some(routes) = race.racers.ai_routes.as_ref()
        {
            race.autopilot = Some(Autopilot::new(d, routes, drift));
        }
        step_race(d, race, Controls { brake: 0.6, ..Default::default() }, ai, dt, sounds);
        race.update(d.state.position.truncate(), dt);
        race.drift_rivals(dt);
        race.ai_refill();
    }
    // The world clock, sky cycle and rain run through the race as through
    // its start scene and countdown (which tick it themselves), and
    // through a replay: the time step 0x57A0F0 runs on every simulated
    // frame, and the replay's snapshots restore the world time
    // ("WorldTime", 0x57BBB0), so the grip's rain (0x5ED5E0) and the
    // falling drops play as in the race.
    d.tick_clock(dt, rng);
}

/// The road's driving direction under the player's car as the wrong-way
/// test 0x600720 reads it: the car's road locator (car +0x8D8, run by
/// 0x5DA340 each frame) holds its node at +0x18 (car +0x8F0) and the
/// spline's driving direction at +0x28 (car +0x900, 0x5D9E40: the
/// tangent, negated on a segment run backward, +0x7C < 0). With no node
/// the test answers "not wrong" (0x600738). A road not built from the
/// network has no nodes: the clone falls back on its polyline there.
pub(crate) fn road_heading(race: &Race) -> Option<Vec2> {
    let road = &race.route.road;
    if road.nodes.is_empty() {
        return Some(road.direction(race.racers.me.along));
    }
    race.racers.me.locator.node.map(|_| race.racers.me.locator.direction)
}

/// The human's reset (car frame 0x6012C0 with car +0x96C set → 0x6009B0,
/// **verified** except where marked; the R key and a replayed reset). The
/// kept speed is max(speed +0x42C, 0) × max(road direction (car +0x900,
/// the race road's on race coordinates) · forward (+0x90), 0), at most
/// 8 m/s (0x8021CC). Then the first of: a kind-4 route trigger holding the
/// car (0x5D9C00(point, 4, 0)) places it at the trigger facing the
/// trigger's direction at 8 m/s (Car_PlaceAt 0x5FBD70); the human's road
/// node (car +0x2C → +0x0C) within 150 m (3D, the node 1 m up) places it
/// on the node facing the node's way — the record's direction in a race
/// (+0x70 bit 1 reverses: the backward tangent, 0x401AA0) — with the
/// kept speed (Car_PlaceOnRoad 0x5FBEE0); else the place after the saved
/// places' count (racer +0xBC + 1 & 3; the oldest), its heading, at rest.
// prov: inferred game.reset_to_road
/// **Inferred**: the human's road node is the AI racer controller's route
/// node when one drives the car, else the node search a human racer's
/// controller makes (0x42CDC0 → 0x413590 with mask 0x400: control points
/// with flag bit 27; the human controller's own updates are not traced);
/// the car's point is its position (body +0x110 not traced). **Not
/// ported**: the line test between the node and the car (0x5E7EE0 /
/// 0x5E8120 on the world's polygons) that swaps in 0x40F180's nearest
/// clear node, and the drag lane's lateral offset.
pub(crate) fn reset_to_road(d: &mut Drive, race: &Race) {
    let st = &d.state;
    let forward = st.forward().truncate().normalize_or_zero();
    let road = road_heading(race).unwrap_or_else(|| race.route.road.direction(race.racers.me.along));
    let kept = (st.speed().max(0.0) * road.dot(forward).max(0.0)).min(RESET_SPEED);
    let at = st.position;
    let mut placed: Option<(Vec3, f32, f32)> = None;
    if let Some(routes) = race.racers.ai_routes.as_ref() {
        if let Some(z) = routes.resets.iter().find(|z| z.trigger.contains(at.truncate())) {
            // The game passes z 0 (0x600B37); the clone drops the car
            // from the trigger's own height (+0x14) onto the road.
            placed = Some((z.trigger.position, z.direction.y.atan2(z.direction.x), RESET_SPEED));
        } else {
            let f = &routes.file;
            let node = race.autopilot.as_ref().and_then(|a| a.route.as_ref()).and_then(|r| r.node()).or_else(|| {
                let h = st.heading();
                nfsu2::ai::route::RacerRoute::find_flagged(
                    routes,
                    at,
                    Vec2::new(h.cos(), h.sin()),
                    Some(race.racers.me.along),
                    true,
                    0x0800_0000,
                )
            });
            if let Some((record, cp, backward)) = node {
                let p = f.records[record].points[cp].position;
                if p.distance(at - Vec3::Z) < 150.0 {
                    let backward = if routes.race_coords { f.records[record].flags >> 1 & 1 != 0 } else { backward };
                    let dir = f.direction(record, cp, backward);
                    placed = Some((p, dir.y.atan2(dir.x), kept));
                }
            }
        }
    }
    let (p, heading, speed) = placed.unwrap_or_else(|| {
        let (p, h) = race.racers.me.places[(race.racers.me.place_index.wrapping_add(1) & 3) as usize];
        if race.racers.me.place_index == 0 {
            // No place saved yet: the race road under the car.
            let a = race.route.road.point_at(race.racers.me.along);
            (a.extend(at.z), road.y.atan2(road.x), 0.0)
        } else {
            (p, h, 0.0)
        }
    });
    d.place_reset(p + Vec3::Z * 5.0, heading);
    d.state.set_placement_speed(speed);
    // Each placement of 0x6009B0 is stamped (flag 1: 0x600DAC / 0x600DF1);
    // a replayed reset as the live one.
    d.stamp_placement();
}

/// An AI racer's reset (AccidentRecovery's request, car +0x96C; the car
/// frame 0x6012C0 for a controller of class 2 goes on at 0x6013AF): the
/// kept speed is max(speed +0x42C, 0) × max(the race road's direction
/// (car +0x900 / +0x904 on race coordinates) · forward, 0), at most 8 m/s
/// (0x8021CC); Car_PlaceOnRoad (0x5FBEE0) puts it on the controller's
/// road node (+0x2C → +0x0C, with its record from 0x40F690) facing the
/// node's way, reversed by the record's +0x70 bit 1 on race coordinates,
/// at lateral 0. No 150 m test as for the human. **Not ported**: the
/// drag lane's offset (0x403DB0 / 0x5E2DB0). The clone takes the walk's
/// current FuturePoint for the node (`RacerRoute::node`, inferred).
pub(crate) fn reset_rival(world: &World, r: &mut Rival, routes: &nfsu2::ai::route::AiRoutes, road: Vec2) {
    let Some((record, cp, backward)) = r.route.as_ref().and_then(|x| x.node()) else { return };
    let st = &r.state;
    let forward = st.forward().truncate().normalize_or_zero();
    let kept = (st.speed().max(0.0) * road.dot(forward).max(0.0)).min(RESET_SPEED);
    let f = &routes.file;
    let backward = if routes.race_coords { f.records[record].flags >> 1 & 1 != 0 } else { backward };
    let dir = f.direction(record, cp, backward);
    let p = f.records[record].points[cp].position + Vec3::Z * 5.0;
    let ground = world.road_height_below(p + Vec3::Z * 3.0).unwrap_or(p.z);
    let barriers = std::mem::take(&mut r.state.ground.barriers);
    let (manual, held, stability_off) = (r.state.manual, r.state.held, r.state.stability_off);
    r.state = CarState::placed(&r.car.car_type, world, Vec3::new(p.x, p.y, ground), dir.y.atan2(dir.x));
    r.state.ground.barriers = barriers;
    r.state.manual = manual;
    r.state.held = held;
    r.state.stability_off = stability_off;
    r.state.set_placement_speed(kept);
    // Car_PlaceOnRoad with the stamp flag (0x601512 pushes 1 → 0x5FC027
    // → 0x5EBF80): the 2 s hold, ghosted against the other cars
    // (0x5EBFB0) and drawn blinking (0x60D500).
    r.state.ghost_timer = nfsu2::racer_reset::PLACE_HOLD;
    // The placement 0x5F52D0 keeps the car's mode (+0x28), switches to mode
    // 0 (0x5F46B0(0) at 0x5F52E7: the old mover is destroyed) and, once the
    // car is reset and laid on the road, back to the kept mode (0x5F53F4): a
    // fresh mover. A car on the BeltMover gets a belt with no path, which
    // stops it until the think's end builds one (0x5918B0; `step_belt`);
    // the clone had kept the old belt, whose spline and speed from before
    // the reset (a car falling at 80 m/s) went on moving it.
    if r.belt.belt.is_some() {
        r.belt.belt = None;
        build_belt_state(&r.car, &r.state, &mut r.belt, &*r.accel, kept, &[], 0.0);
    }
}

/// The reset's speed cap and a reset zone's speed, m/s (0x8021CC).
pub(crate) const RESET_SPEED: f32 = 8.0;
