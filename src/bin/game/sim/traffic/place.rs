use crate::*;

/// Puts a car moved by its TrafficMover on the road (0x5F5020 → 0x5F4A00):
/// x/y and heading from the mover; each wheel's car-space x/y turned by the
/// heading finds the ground under it (0x5E72F0; the clone's `road_below`);
/// when all four do, the body lies on their plane — forward the front
/// minus the rear wheels (w1 − w2 + w0 − w3), left the left minus the right
/// (w3 − w2 + w0 − w1), each normalised, up their cross product — at the
/// mean ground height plus (the mean of each wheel's radius less its z,
/// H +0x20 / +0x18 + 0x30·i, plus the centre of mass's z, H +0x08 or the
/// rigid body's +0x1F8) / |up.z| — the height of the centre of mass,
/// which the clone turns into its car-space origin's (x/y stay the
/// mover's, taken as the origin's, inferred); otherwise level on the
/// heading at the height it had. The game keeps the rows as they come;
/// the clone squares them up for its quaternion. The velocity goes along
/// the move at the mover's speed and the yaw rate from the heading's
/// change (0x408820). `from` and `old` are the car's place and heading
/// before the mover's frame.
pub(crate) fn place_on_mover(
    world: &nfsu2::world::World,
    car: &Car,
    state: &mut CarState,
    m: &nfsu2::ai::traffic::TrafficMover,
    before: (Vec3, f32),
    dt: f32,
) {
    place_on_road(world, car, state, (m.position, before.0.z), m.heading, m.speed, before, dt);
}

/// 0x5F5020 for a mover's x / y (`at.0`, with the height to search down
/// from, `at.1`), heading and speed; see [`place_on_mover`].
pub(crate) fn place_on_road(
    world: &nfsu2::world::World,
    car: &Car,
    state: &mut CarState,
    (position, z_from): (Vec2, f32),
    heading: f32,
    speed: f32,
    (from, old): (Vec3, f32),
    dt: f32,
) {
    struct M {
        position: Vec2,
        heading: f32,
        speed: f32,
    }
    let m = M { position, heading, speed };
    let from_z = z_from;
    let wheels = &car.car_type.wheels;
    let forward = Vec3::new(m.heading.cos(), m.heading.sin(), 0.0);
    let left = Vec3::new(-forward.y, forward.x, 0.0);
    let ground: Vec<Option<Vec3>> = wheels
        .iter()
        .map(|w| {
            let at = Vec3::new(m.position.x, m.position.y, from_z)
                + forward * w.position.x
                + left * w.position.y
                + Vec3::Z * w.position.z;
            world.road_below(at + Vec3::Z * 3.0).map(|(z, _)| Vec3::new(at.x, at.y, z))
        })
        .collect();
    let (to, axes) = match ground.iter().copied().collect::<Option<Vec<Vec3>>>() {
        Some(g) => {
            let a = ((g[1] - g[2]) + (g[0] - g[3])).normalize_or(forward);
            let b = ((g[3] - g[2]) + (g[0] - g[1])).normalize_or(left);
            let up = a.cross(b);
            let com = Vec3::from(car.car_type.handling.centre_of_mass);
            let lift = wheels.iter().map(|w| w.radius - w.position.z).sum::<f32>() / 4.0 + com.z;
            let z = g.iter().map(|p| p.z).sum::<f32>() / 4.0 + lift / up.z.abs().max(1e-3);
            let up = up.normalize_or(Vec3::Z);
            let axes = (a, up.cross(a), up);
            let below = (axes.0 * com.x + axes.1 * com.y + axes.2 * com.z).z;
            (Vec3::new(m.position.x, m.position.y, z - below), axes)
        }
        None => (Vec3::new(m.position.x, m.position.y, from_z), (forward, left, Vec3::Z)),
    };
    let moved = to - from;
    state.orientation = glam::Quat::from_mat3(&glam::Mat3::from_cols(axes.0, axes.1, axes.2)).normalize();
    state.velocity =
        if m.speed.abs() <= 1e-5 || moved.length() <= 1e-6 { Vec3::ZERO } else { moved.normalize() * m.speed };
    // Traffic/belt export 408B10 uses mover speed, even when the existing
    // ground-placement approximation produces a different velocity projection.
    state.set_exported_forward_speed(m.speed);
    let turn = m.heading - old;
    state.spin = Vec3::Z * turn.sin().atan2(turn.cos()) / dt.max(1e-6);
    state.angular_momentum = Vec3::ZERO;
    state.position = to;
    // Each wheel rolls at speed / radius and is set down (0x592050 /
    // 0x592090 with 1, meaning inferred), no slip.
    for (ws, wheel) in state.wheels.iter_mut().zip(&car.car_type.wheels) {
        ws.spin = m.speed / wheel.radius;
        ws.on_ground = true;
        ws.sliding = false;
        ws.slip = 0.0;
    }
}

/// What the drag placement (0x42E620) reads of the race: the player's race
/// coordinate and lane (2–5), the race clock, the start line's race
/// coordinate and the track's start gap (None when its byte +0xF4 +
/// direction is set).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DragPlace {
    pub(crate) coord: f32,
    pub(crate) lane: Option<usize>,
    pub(crate) clock: f32,
    pub(crate) start_line: f32,
    pub(crate) start_gap: Option<f32>,
}

/// Puts a traffic car on its lane at `at` (0x42C950 / 0x42C670): on the
/// ground, heading `heading`, with the TrafficMover (0x5F46B0(2)), which
/// holds 55 mph at most, and a fresh driver (the racers' low-level driver,
/// 0x426580 → 0x425AB0). 0x5FBEE0 → 0x5F52D0 sets the car's +0x42C to the
/// lane's speed byte but its velocity to the heading × (that + 0.1 m/s),
/// and 0x592170 its +0x430 to the velocity's length; the mover then takes
/// +0x430 as its speed (0x408740), so a car starts at the lane speed +
/// 0.1 (**verified**; **observed** under Wine, 2026-10-31: placed cars at
/// 20.1000 m/s on 20 m/s lanes, held there, the driver's gas and brake 0
/// from the first frame). Its four wheels stay down: the mover sets their
/// flags (car +0x390, 0x592050 with ECX = car + 0x40) every frame
/// (0x408BAC), and in memory every traffic car's, parked ones too, read 1
/// in every sample, so the driver's first frame runs the speed control
/// (0x41D8D0) rather than the no-wheels-down full throttle (0x421030),
/// which gave the clone's 2 % overshoot (20.4 m/s).
pub(crate) fn place_traffic(
    world: &World,
    c: &mut TrafficCar,
    routes: &nfsu2::formats::ai_routes::RouteFile,
    lane: nfsu2::ai::traffic::TrafficLane,
    at: Vec3,
    heading: f32,
) {
    use nfsu2::ai::traffic::{TRAFFIC_THROTTLE_CAP, TrafficMover};
    /// 0x5F52D0's 0.1 m/s on the velocity.
    const PLACE_SPEED_ADD: f32 = 0.1;
    let w = c.car.car_type.wheels[0];
    let ground = world.road_height_below(at + Vec3::Z * 3.0).unwrap_or(at.z);
    c.state = CarState::new(Vec3::new(at.x, at.y, ground - (w.position.z - w.radius)), heading);
    c.state.start_race(RaceMode::FreeRoam);
    let speed = lane.lane_speed(routes) + PLACE_SPEED_ADD;
    c.state.set_placement_speed(lane.lane_speed(routes));
    for w in c.state.wheels.iter_mut() {
        w.on_ground = true;
    }
    c.lane = Some(lane);
    c.coast = None;
    c.mover = Some(TrafficMover { position: at.truncate(), heading, speed });
    c.state.melo = None;
    c.belt.belt = None;
    c.plan = None;
    // 0x5FBEE0 → controller slot +0x34 (0x42A050) clears +0x710.
    c.stopped = false;
    c.driver = nfsu2::ai::driver::AiDriver::new(&c.car.car_type.handling, c.car.car_type.suv, TRAFFIC_THROTTLE_CAP);
}

/// The drag race set-up's traffic fill (0x6065C0's third loop → 0x42F7A0 →
/// 0x42E4A0; verified, the disassembly; **observed** under Wine on the
/// Bayview Bridge drag: ten cars on the control points from 452.5 to
/// 945, one a point, at race status 0): each pool car in turn
/// ([`nfsu2::ai::traffic::TrafficLane::fill_drag`]) gated by 0x418110 with
/// the fill step as the spacing and no car of its own, the cars already
/// placed and the racers listed (race status 0: no 250 m test). A car the
/// fill cannot place stays parked; 0x42F7A0 would then try 0x42E0B0, the
// prov: inferred game.traffic_fill
/// teleporters (0x883DC8 of them, none in the retail data — **inferred**
/// from 0x42E790 failing there), so the clone leaves it. Outside drag
/// races 0x42F7A0 goes to 0x42E0B0 and then 0x42E2C0
/// ([`nfsu2::ai::traffic::TrafficLane::fill_race`]: random race-record
/// points with bits 25 and 26), with the same gate and spacing.
pub(crate) fn traffic_fill(
    world: &World,
    t: &mut Traffic,
    player: &CarState,
    racers: &[(Vec2, Vec2)],
    road: &nfsu2::formats::roads::RoadRoute,
    p: DragPlace,
    drag: bool,
) {
    use nfsu2::ai::traffic::TrafficLane;
    t.filled = true;
    let me = player.position.truncate();
    for i in 0..t.cars.len() {
        let placed: Vec<Vec2> = t
            .cars
            .iter()
            .filter(|c| c.lane.is_some())
            .map(|c| c.state.position.truncate())
            .chain(racers.iter().map(|r| r.0))
            .chain(std::iter::once(me))
            .collect();
        let coords: Vec<(f32, bool)> = t
            .cars
            .iter()
            .filter(|c| c.lane.is_some())
            .map(|c| (road.locate_once(c.state.position.truncate()), false))
            .chain(racers.iter().map(|r| (road.locate_once(r.0), true)))
            .chain(std::iter::once((p.coord, false)))
            .collect();
        let gap = t.fill_step;
        let gate = |at: Vec2, c: f32, coordinate: bool| {
            drag_gate(road, &p, false, gap, &coords, &placed, me, at, c, coordinate)
        };
        let placed = if drag {
            TrafficLane::fill_drag(&t.routes, i, t.fill_step, p.lane, t.with, &gate, &mut t.field.rng)
        } else {
            TrafficLane::fill_race(&t.routes, t.with, &gate, &mut t.field.rng)
        };
        if let Some((lane, at, heading)) = placed {
            place_traffic(world, &mut t.cars[i], &t.routes, lane, at, heading);
        }
    }
    // Only the cars 0x42F7A0 placed join the world list (0x6066FF..0x606713:
    // the car is linked in when it returns one); the placer 0x42F910 walks
    // the world list's traffic (+0x18 .. +0x10) and does nothing while that
    // is empty (0x42F95B), so a pool car the fill could not place never
    // appears in the race (**verified**; **observed** under Wine: the
    // stage-4 drag S4_H_DRAG_15 made its 12 pool cars, car table 0x89E810
    // entries +4 = 3, but its world list held the 4 racers all race).
    t.cars.retain(|c| c.lane.is_some());
}

/// 0x418110, the drag placements' gate (verified, the disassembly): before
/// the green (no race status or the race clock 0x402000 under 0, and the
/// track's byte +0xF4 + direction clear) a point less than the start gap
/// (+0xF8 + 4·direction) past the start line (race status +0xBF8 by +0xBF6
/// × 0.01 × the road length) is refused. Then a point measured by race
/// coordinate (`coordinate`, 0x5DCC90) is refused in race status 4
/// (`racing`) within 250 m of the player's (0x409890 → 0x5F12B0) and when
/// a car is near (0x411060: an AI racer, controller type 3, within `gap`
/// either way; the others — the player, traffic — with the point less than
/// `gap` ahead of them); any other point in 2D: in race status 4 within
/// 250 m (0x78429C) of the player (0x409840 → 0x5F0FC0) and within `gap`
/// of any car (0x410FC0: every listed car not parked or hidden, the placed
/// one aside). `coords` are the cars' race coordinates (each with whether
/// it is an AI racer), `cars` their places. The cars' race coordinates are
// prov: inferred game.traffic_fill-2
/// their places located on the race road (**inferred**; a car placed by
/// 0x42C670 / 0x42C950 starts with the point's +0x08); 0x5F12B0 /
// prov: inferred game.traffic_fill-3
/// 0x5F0FC0 are taken as the player alone (**inferred**: which cars are
/// the first +0x14 of the world list is open).
#[allow(clippy::too_many_arguments)]
pub(crate) fn drag_gate(
    road: &nfsu2::formats::roads::RoadRoute,
    p: &DragPlace,
    racing: bool,
    gap: f32,
    coords: &[(f32, bool)],
    cars: &[Vec2],
    me: Vec2,
    at: Vec2,
    c: f32,
    coordinate: bool,
) -> bool {
    use nfsu2::ai::traffic::DESPAWN;
    if p.clock < 0.0 && p.start_gap.is_some_and(|g| road.difference(c, p.start_line) < g) {
        return false;
    }
    if coordinate {
        let near_player = racing && road.difference(c, p.coord).abs() < DESPAWN;
        let near_car = coords.iter().any(|&(o, racer)| {
            let d = road.difference(c, o);
            if racer { d.abs() < gap } else { d > 0.0 && d < gap }
        });
        !near_player && !near_car
    } else {
        let near_player = racing && at.distance(me) < DESPAWN;
        !near_player && !cars.iter().any(|&o| at.distance(o) < gap)
    }
}
