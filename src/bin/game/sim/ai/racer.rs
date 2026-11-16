use crate::*;

/// An event's racers over its races (a tournament has 2–3): the
/// opponents' line-up and each racer's points so far (the player first),
/// and the player's times.
#[derive(Clone, Debug, Default)]
pub(crate) struct Field {
    pub(crate) lineup: Vec<nfsu2::opponents::Entrant>,
    pub(crate) points: Vec<i32>,
    pub(crate) times: Vec<f32>,
    /// The rivals' cars as the line-up's set-up fitted them (their looks
    /// and performance, 0x5260C0 → 0x6399E0; a Quick Race's 0x53EEA0 →
    /// 0x532020), in entry order: the keys of their loads.
    pub(crate) looks: Vec<crate::loads::preload::RivalKey>,
    /// The grid slots the set-up numbered (0x5264B0, racer info +0x03),
    /// the player's first, then the rivals' in entry order.
    /// The flow's track switch for Restart (message 9) and Next Race (10)
    /// runs the event's set-up without the line-up, the looks and the grid
    /// (0x581830 → 0x5322F0(event, 0): 0x5260C0 and 0x5264B0 skipped), and
    /// for a Quick Race (game mode 2) no set-up at all, so both keep them
    /// (**verified**, docs/formats.md "Race types: Restart and Next Race keep the
    /// field").
    pub(crate) slots: Vec<usize>,
}

/// An AI racer's controller state (controller type 3, 0x4260B0): its
/// place on the AI route (FuturePoints), target speed, low-level driver
/// (0x425AB0), GetUnstuck action and drag lane.
pub(crate) struct RacerAi<'a> {
    pub(crate) route: &'a mut Option<nfsu2::ai::route::RacerRoute>,
    pub(crate) speed: &'a mut nfsu2::ai::route::SpeedTarget,
    pub(crate) driver: &'a mut nfsu2::ai::driver::AiDriver,
    pub(crate) unstuck: &'a mut nfsu2::ai::driver::Unstuck,
    /// The racer's actions and the cars its road entries see; None drives
    /// the plain line (the player's car past the finish).
    pub(crate) actions: Option<RacerActions<'a>>,
    pub(crate) lane: &'a mut nfsu2::ai::drag::DragLane,
    /// The DriftDriver (0x425E40) in place of `driver`: every AI racer in
    /// a drift race, and the player's car past a drift race's finish.
    pub(crate) drift: Option<&'a mut nfsu2::ai::driver::DriftDriver>,
    /// Where the think's path and target speed go (the driver's +0x10
    /// path and +0x230, which the BeltMover reads), when asked.
    pub(crate) plan: Option<&'a mut (Vec<Vec3>, f32)>,
}

/// A racer's actions and what they see this frame: the cars of the road
/// entries, its own, Block's tuning row, the racers it is in contact with
/// (bind identities, see `CarField::partners`) and whether it touches any
/// car.
pub(crate) struct RacerActions<'a> {
    pub(crate) actions: &'a mut nfsu2::ai::actions::Actions,
    pub(crate) cars: &'a [nfsu2::ai::actions::RoadCar],
    pub(crate) me: nfsu2::ai::actions::RoadCar,
    pub(crate) block_row: usize,
    pub(crate) hits: Vec<usize>,
    pub(crate) touching: bool,
    /// This frame is the controller's full think (0x42AE2F); between them
    /// the action's last output is kept (only Block updates, 0x42ADA4).
    pub(crate) full: bool,
    pub(crate) retained: &'a mut Option<nfsu2::ai::actions::Steer>,
}

/// A car as the racers' road entries see it (`ai::actions::RoadCar`):
// prov: inferred game.road_car
/// half its length and width (handling +0x224 / +0x228, **inferred** for
/// the shape extents 0x5AACE0 reads), its acceleration along its velocity
/// from the retained world acceleration (0x60BE40 → 0x5EC340). Legacy
/// caller history remains in the interface but no longer determines it.
pub(crate) fn road_car(
    id: usize,
    car: &Car,
    state: &CarState,
    _last: Option<Vec3>,
    racer: bool,
    human: bool,
    _dt: f32,
) -> nfsu2::ai::actions::RoadCar {
    let dims = car.car_type.handling.dimensions;
    let v = state.velocity;
    let accel = state.velocity_acceleration();
    let f = state.forward();
    nfsu2::ai::actions::RoadCar {
        id,
        position: state.position,
        velocity: v,
        forward: Vec2::new(f.x, f.y).normalize_or(Vec2::X),
        accel,
        forward_accel: state.forward_acceleration(),
        half: Vec2::new(dims[0], dims[1]) * 0.5,
        racer,
        human,
        traffic: !racer,
        ghosted: state.ghost_timer > 0.0,
        acceleration: state.world_acceleration,
        // The human's path is its controller's per-frame build (0x42CAA0;
        // `ai::actions::human_path`); the racers' are set by the caller.
        path: if human {
            nfsu2::ai::actions::human_path(
                state.position,
                v,
                Vec2::new(f.x, f.y).normalize_or(Vec2::X),
                state.speed(),
                state.exported_forward_speed(),
            )
            .unwrap_or_default()
        } else {
            nfsu2::ai::actions::CarPath::default()
        },
    }
}

/// The human's path at 10 m/s and above (0x42CAA0 → 0x4221F0,
/// `HumanRoad::road_path`): the road path on the human walk in place of
/// `road_car`'s line form, from the car's +0x60 (its centre of mass, as
/// the walk's update takes it); without a walk the line form stays.
pub(crate) fn human_road_path(
    c: &mut nfsu2::ai::actions::RoadCar,
    road: &nfsu2::ai::route::HumanRoad,
    routes: &nfsu2::ai::route::AiRoutes,
    car: &Car,
    state: &CarState,
) {
    if state.exported_forward_speed() < nfsu2::ai::route::ROAD_PATH_SPEED {
        return;
    }
    let at = crate::sim::encounters::ai::belt_car_position(car, state);
    if let Some(p) = road.road_path(routes, at, state.velocity, state.forward(), state.speed()) {
        c.path = nfsu2::ai::actions::CarPath::from_points(&p);
    }
}

/// What every AI racer's think reads, the race rivals' (`racer_think`)
/// and the encounter racers' (`encounter_racer_think`) alike: the AI
/// tuning, the AI routes, the world's random numbers (+0xF8, shared by
/// every racer in turn), the clock the speed target and GetUnstuck read
/// (a race's clock, the encounter manager's seconds in free roam) and the
/// update's seconds.
pub(crate) struct ThinkCtx<'a> {
    pub(crate) ai: &'a AiData,
    pub(crate) routes: &'a nfsu2::ai::route::AiRoutes,
    pub(crate) field_rng: &'a mut nfsu2::opponents::GameRng,
    pub(crate) clock: f32,
    pub(crate) dt: f32,
}

/// A race racer's own inputs to `racer_think`: the race type, the start
/// scene's camera (race status 2), the race-progress speed fed while the
/// racer races, its status record +0x10 ≠ 0 (`finished`), the Park goal
/// (`parked`) and the catch-up's inputs.
pub(crate) struct RacerThinkIn {
    pub(crate) race_type: nfsu2::race::RaceMode,
    pub(crate) scene_camera: bool,
    pub(crate) progress: Option<(f32, f32)>,
    pub(crate) finished: bool,
    pub(crate) parked: bool,
    pub(crate) catch_up: Option<nfsu2::ai::route::CatchUp>,
}

/// One AI racer's think (the controller's 0x42ABD0, then its driver's
/// 0x421030): the route, the target speed, the path (the drag lane in a
/// drag race, else AIActionJustDrive's racing line), GetUnstuck and the
/// driver's outputs. `progress` is the race-progress speed fed while the
/// racer races; `finished` its status record +0x10 ≠ 0. The racer's own
/// generator (`_own_rng`) is not drawn by the think: its speed offset, like
/// its junction and pass draws, comes from the world's (`ctx.field_rng`,
/// 0x41C71E).
pub(crate) fn racer_think(
    car: &Car,
    state: &CarState,
    r: &mut RacerAi,
    _own_rng: &mut nfsu2::opponents::GameRng,
    accel: &dyn Fn(f32) -> f32,
    mut ctx: ThinkCtx,
    input: RacerThinkIn,
) -> nfsu2::drive::AiControls {
    let (clock, dt) = (ctx.clock, ctx.dt);
    let RacerThinkIn { race_type, scene_camera, progress, finished, parked, catch_up } = input;
    let h = &car.car_type.handling;
    // Before the green (race status 2 / 3) the racers have no goal yet
    // (goal +0x14 and the running action read 0 under Wine through the
    // whole countdown; the race goal, 0x42C060 → 0x42AA20(1) — 2 / 3 in a
    // drag race —, and its first action come in the update after the
    // green): no action, no path, target speed 0, so the driver's countdown
    // branch (0x421030 with race status 3: gear request 1, 0x5EB920(1),
    // 0x5EB900(1)) revs the engine with the wheels straight (steer 0,
    // **observed**). The update that turns the race green leaves the
    // pedals as `AiDriver::green_release` says.
    // A drag race's rivals do the same (observed, drag_s1: gas 1, brake 0
    // at the green, then the drag cap 0.93).
    let countdown = clock < 0.0;
    if countdown && clock + dt >= -1.0e-4 && r.drift.is_none() && !parked {
        if let Some(route) = r.route.as_mut() {
            route.clock = clock;
        }
        return r.driver.green_release();
    }
    let (path, mut target, mut pedals) = racer_route_step(car, state, r, accel, &mut ctx, race_type, catch_up);
    // A done racer's goal is "Park" (0x606CC0 → 0x42B330 → 0x42AA20(11):
    // goal type 11, vtable 0x784968, for every racer on the AI racer
    // controller, the player's car past the line included). Its chooser
    // (slot +0x38 0x402780 → +0x3C 0x427060) proposes only "Stop driving"
    // (vtable 0x78591C, type 16, priority 600), whose update 0x42F6E0
    // keeps the line path and asks speed 0 (+0x1F0 = 0, +0x1FC clear,
    // pedals +0x1FD on); the goal's speed 0x41C990 takes the least of
    // that and its slot +0x4C (0x401D30: 0.0), so the driver brakes to a
    // stop and stands. No GetUnstuck: the Park chooser proposes nothing
    // else.
    // `parked`: the racer's status says done (the benches and tests drive
    // the player's car on the controller without a finish).
    if parked {
        target = 0.0;
        pedals = true;
    }
    racer_unstuck_step(state, h, r, ctx.routes, race_type, clock, progress, parked, &mut target);
    racer_driver_step(car, state, r, &path, target, pedals, clock, scene_camera, finished, dt)
}

/// The think's route (the walk, its junction draws on the world's
/// generator), target speed and path with the action's cap and pedals
/// (`racer_drag_actions_step` / `racer_line_actions_step`); none before
/// the green.
#[allow(clippy::too_many_arguments)]
fn racer_route_step(
    car: &Car,
    state: &CarState,
    r: &mut RacerAi,
    accel: &dyn Fn(f32) -> f32,
    ctx: &mut ThinkCtx,
    race_type: nfsu2::race::RaceMode,
    catch_up: Option<nfsu2::ai::route::CatchUp>,
) -> (Vec<Vec3>, f32, bool) {
    let (ai, routes, clock, dt) = (ctx.ai, ctx.routes, ctx.clock, ctx.dt);
    let field_rng = &mut *ctx.field_rng;
    let h = &car.car_type.handling;
    let suv = car.car_type.suv;
    let (mut path, mut target) = (Vec::new(), 0.0);
    let mut pedals = true;
    let countdown = clock < 0.0;
    if let Some(route) = r.route.as_mut().filter(|_| !countdown) {
        // The walk's junction choice (0x413CC0): the race clock for the
        // branch hold and the skill row's odds for the retyped junctions
        // (row = clamp(vt+0x78, 0, 9): the human's −2 reads row 0).
        route.clock = clock;
        route.junction_odds = ai.tuning.junction_odds(r.speed.skill());
        // Its draws (0x413DCB, 0x413E57: 0x5EA510 with ECX = 0x890080)
        // are the world's random numbers (+0xF8), shared by every racer
        // in turn, not a generator of the racer's own.
        std::mem::swap(&mut route.rng, field_rng);
        let travel = nfsu2::ai::route::travel(state.speed(), state.velocity, state.forward());
        route.update(routes, state.position, state.speed(), travel, h.performance_class, suv, accel);
        std::mem::swap(&mut route.rng, field_rng);
        // The catch-up option (0x89E7F8): every race type's defaults
        // (0x4FEF80 … 0x4FF160, through 0x526990) clear it and only the
        // Quick Race set-ups (0x526730 / 0x526840, 0x53F160 …, 0x601E34)
        // write it, from the mode record's +0x05.
        // Its offset draw (0x41C71E: 0x5EA510 with ECX = [0x890080]) is
        // the world's generator too, not the racer's own.
        target = r.speed.update(&ai.tuning, route, field_rng, f64::from(clock), catch_up, dt);
        let r = ActionsParts {
            speed: &*r.speed,
            unstuck: &mut *r.unstuck,
            actions: r.actions.as_mut(),
            lane: &mut *r.lane,
        };
        path = if race_type.drag() {
            racer_drag_actions_step(car, state, route, r, ai, routes, field_rng, clock, &mut target)
        } else {
            racer_line_actions_step(
                car,
                state,
                route,
                r,
                ai,
                routes,
                field_rng,
                race_type,
                (clock, dt),
                &mut target,
                &mut pedals,
            )
        };
    }
    (path, target, pedals)
}

/// The parts of a racer's controller its path choice reads besides the
/// route (which it borrows from the controller).
struct ActionsParts<'p, 'a> {
    speed: &'p nfsu2::ai::route::SpeedTarget,
    unstuck: &'p mut nfsu2::ai::driver::Unstuck,
    actions: Option<&'p mut RacerActions<'a>>,
    lane: &'p mut nfsu2::ai::drag::DragLane,
}

/// A drag racer's path: the drag lane, or AvoidAvoidable's lane change.
#[allow(clippy::too_many_arguments)]
fn racer_drag_actions_step(
    car: &Car,
    state: &CarState,
    route: &mut nfsu2::ai::route::RacerRoute,
    mut r: ActionsParts,
    ai: &AiData,
    routes: &nfsu2::ai::route::AiRoutes,
    field_rng: &mut nfsu2::opponents::GameRng,
    clock: f32,
    target: &mut f32,
) -> Vec<Vec3> {
    // The original's drag rivals ask the controller's cap, 1000
    // (0x784550), as target (+0x230, observed from the first think
    // in every drag run), not the line speed.
    *target = DRAG_TARGET_SPEED;
    let lane_path = r.lane.ai_path(route, routes, state.position, field_rng);
    // `NFSU2_DRAG_LANE_LOG=1` (harness knob): an AI racer's lane as the player's.
    if debug_env().drag_lane_log.is_some() {
        let has: Vec<usize> = (2..=5).filter(|&l| route.has_lane(routes, l)).collect();
        let ends: Vec<i32> = (2..=5).map(|l| route.lane_end(routes, l) as i32).collect();
        let lat: Vec<String> =
            has.iter().map(|&l| format!("{l}:{:.2}", route.lane_lateral(routes, state.position, l))).collect();
        eprintln!(
            "AILANE {} t {clock:.3} d {:.1} coord {:.1} pos {:.1} {:.1} v {:.1} lane {:?} has {has:?} ends {ends:?} lat {}",
            car.car_type.name,
            route.distance(),
            route.race_coord.unwrap_or(f32::NAN),
            state.position.x,
            state.position.y,
            state.speed(),
            r.lane.lane,
            lat.join(",")
        );
    }
    // AvoidAvoidable on the "Win drag race" goal (0x42D470 →
    // 0x42BA20 / 0x42F330 → 0x42BF00): a lane one to three away
    // that is clear for 3 s when the path driven meets a car.
    match r.actions.as_mut() {
        Some(a) if r.unstuck.since.is_none() => {
            let held = route.clock <= route.branch_hold;
            let up_z = (glam::Mat3::from_quat(state.orientation) * Vec3::Z).z;
            let scene = nfsu2::ai::actions::Scene {
                routes,
                me: a.me,
                others: a.cars,
                clock,
                rate: nfsu2::ai::drag::AI_RATE,
                street_x: false,
                drift: false,
                drag: true,
                held,
                block_row: a.block_row,
                hits: &a.hits,
                touching: a.touching,
                wheels_down: state.wheels_down(),
                up_z,
                forward_speed: state.speed(),
                odds: ai.tuning.pass_odds(r.speed.skill()),
            };
            let driven = std::mem::take(&mut a.actions.last_path);
            let driven = if driven.is_empty() { lane_path.clone() } else { driven };
            match a.actions.drag_avoid(route, &scene, r.lane, &driven, field_rng) {
                Some(steer) => {
                    if let Some(cap) = steer.cap {
                        *target = target.min(cap);
                    }
                    a.actions.last_path = steer.path.clone();
                    steer.path
                }
                None => {
                    a.actions.last_path = lane_path.clone();
                    lane_path
                }
            }
        }
        _ => lane_path,
    }
}

/// Any other racer's path: GetUnstuck's lane path while it runs, else
/// the running action's (the full think or the fallback between them),
/// else the plain racing line.
#[allow(clippy::too_many_arguments)]
fn racer_line_actions_step(
    car: &Car,
    state: &CarState,
    route: &mut nfsu2::ai::route::RacerRoute,
    r: ActionsParts,
    ai: &AiData,
    routes: &nfsu2::ai::route::AiRoutes,
    field_rng: &mut nfsu2::opponents::GameRng,
    race_type: nfsu2::race::RaceMode,
    (clock, dt): (f32, f32),
    target: &mut f32,
    pedals: &mut bool,
) -> Vec<Vec3> {
    let h = &car.car_type.handling;
    // AIActionJustDrive's racing-line record rate (0x42B357):
    // 0.2 (0x783680) in Street X, else 0.05 (0x7E8E64), its path
    // kept between frames (0x420D40). Only this 0x420D40 branch of
    // 0x42B340 is modelled (its 0x41D160, 0x428DF0, 0x40FBE0 and
    // 0x401D40 calls are open, see formats).
    let rate = if h.race.street_x() { 0.2 } else { 0.05 };
    let fresh = r.unstuck.since.is_some();
    match r.actions {
        // A running GetUnstuck is the controller's action (priority
        // 500, the manager keeps it over the others): its own lane
        // path (0x423E20 → 0x420D40 at rate 0.25, offset 0; the
        // first planned from the car, 0x420940), its pedals on
        // (+0x1FD = 1) and no cap (+0x1FC = 0).
        // GetUnstuck is the controller's running action, so its
        // end leaves an action present and the next choice waits
        // for the racer's full think (0x42ABD0's slot), as every
        // other action change does; the driver keeps its last path
        // until then.
        a if fresh => {
            *pedals = true;
            let first = r.unstuck.first_path();
            let path = route.line_path(
                routes,
                nfsu2::ai::driver::UNSTUCK_LANE_RATE,
                state.position,
                state.forward(),
                state.velocity.length(),
                h.race.drift(),
                first,
                0.0,
            );
            if let Some(a) = a {
                a.actions.get_unstuck(&path);
                *a.retained = Some(nfsu2::ai::actions::Steer {
                    path: path.clone(),
                    cap: None,
                    pedals: true,
                    reset: false,
                });
            }
            path
        }
        Some(a) => {
            // 0x5EF000: the start hold (the route's branch hold).
            let held = route.clock <= route.branch_hold;
            let up_z = (glam::Mat3::from_quat(state.orientation) * Vec3::Z).z;
            let scene = nfsu2::ai::actions::Scene {
                routes,
                me: a.me,
                others: a.cars,
                clock,
                rate,
                street_x: h.race.street_x(),
                drift: h.race.drift(),
                drag: race_type.drag(),
                held,
                block_row: a.block_row,
                hits: &a.hits,
                touching: a.touching,
                wheels_down: state.wheels_down(),
                up_z,
                forward_speed: state.speed(),
                odds: ai.tuning.pass_odds(r.speed.skill()),
            };
            // The full think (0x42ABD0 → 0x42AF40 …) chooses and
            // updates the action; between full thinks the
            // controller's fallback (0x42AD16) runs only the goal's
            // speed and, for an action whose vt+0x10 is true (Block),
            // the route update and the action's update (0x42ADA4 →
            // +0xB0); otherwise the driver keeps the last path
            // (**verified** schedule, outrun-think-scheduling.md;
            // **observed** in races: each rival's action changes
            // only on frames ≡ its slot + const mod 8).
            let steer = if a.full || a.retained.is_none() {
                a.actions.think(route, &scene, &ai.tuning, field_rng, fresh, dt)
            } else if a.actions.ready_between_thinks() {
                a.actions.refresh(route, &scene, dt).unwrap_or_else(|| a.retained.clone().unwrap())
            } else {
                a.retained.clone().unwrap()
            };
            *a.retained = Some(nfsu2::ai::actions::Steer { reset: false, ..steer.clone() });
            // The action's speed (goal slot +0x34, 0x41C8B1: the
            // least of the line speed and +0x1F0 unless +0x1FC),
            // its pedals (+0x1FD → driver +0x23C, 0x421F1A) and its
            // reset request (car +0x96C, the car frame 0x6012C0).
            if let Some(cap) = steer.cap {
                *target = target.min(cap);
            }
            *pedals = steer.pedals;
            steer.path
        }
        None => route.line_path(
            routes,
            rate,
            state.position,
            state.forward(),
            state.velocity.length(),
            h.race.drift(),
            fresh,
            0.0,
        ),
    }
}

/// Seconds since car +0x9C0 (0x5ED590: (sim tick − stamp) × 0.00025;
/// GetUnstuck waits until it is above 2.0, 0x40E7F5 / 0x414454): since the
/// last reset's placement (0x5FBEE0 → 0x5F52D0 stamps every placement),
/// else since the grid, which is at least the countdown.
fn since_placed(placed: Option<f32>, clock: f32) -> f32 {
    placed.map_or(clock + COUNTDOWN, |t| clock - t)
}

#[cfg(test)]
mod placement_tests {
    /// A reset re-stamps the placement, so GetUnstuck's 2 s
    /// settle runs again after it; without one the grid's stamp counts.
    #[test]
    fn a_reset_restarts_the_settle_clock() {
        assert!(super::since_placed(None, 10.0) > 2.0);
        assert_eq!(super::since_placed(Some(10.0), 11.5), 1.5);
        assert!(super::since_placed(Some(10.0), 12.01) > 2.0);
    }
}

/// GetUnstuck's test (0x40E7C0 and the manager's proposal): its reverse
/// speed replaces the target unless the racer is parked.
#[allow(clippy::too_many_arguments)]
fn racer_unstuck_step(
    state: &CarState,
    h: &nfsu2::handling::Handling,
    r: &mut RacerAi,
    routes: &nfsu2::ai::route::AiRoutes,
    race_type: nfsu2::race::RaceMode,
    clock: f32,
    progress: Option<(f32, f32)>,
    parked: bool,
    target: &mut f32,
) {
    let edge = r.route.as_ref().and_then(|x| x.edge(routes));
    // Car +0x9C0: the rivals are placed on the grid as the countdown
    // starts. The facing point (0x4143F0: one entry ahead in drift and
    // Street X, three otherwise) and the driver's last look-ahead point
    // (vt+0x80).
    let facing = r.route.as_ref().and_then(|x| x.facing_point(routes, race_type.drift() || race_type.street_x()));
    let look = r.drift.as_ref().map_or(r.driver.target, |d| d.target);
    // The running action's priority, for the manager's test of the
    // GetUnstuck proposal (−1 without the actions).
    let running = r.actions.as_ref().map_or(-1, |a| a.actions.priority);
    // 0x40E7C0 returns −1 in the racer's start hold (0x5EF000) and while
    // the car has two wheels or fewer down (0x60BF00: the own road entry's
    // object, 0x89E970, +0x1C → car +0x424 > 2).
    let held = r.route.as_ref().is_some_and(|x| x.clock <= x.branch_hold) || state.wheels_down() <= 2;
    if let Some(reverse) = r.unstuck.update(
        state,
        h,
        facing,
        Some(look),
        edge.as_ref(),
        // Seconds since car +0x9C0 (0x5ED590 on the sim tick): since the
        // last reset's placement, else since the grid (≥ the countdown).
        since_placed(r.unstuck.placed, clock),
        progress,
        f64::from(clock),
        running,
        held,
    ) && !parked
    {
        *target = reverse;
    }
}

/// The driver's frame (0x421030, or the DriftDriver's): the path and
/// target speed in, the pedals and steering out.
#[allow(clippy::too_many_arguments)]
fn racer_driver_step(
    car: &Car,
    state: &CarState,
    r: &mut RacerAi,
    path: &[Vec3],
    target: f32,
    pedals: bool,
    clock: f32,
    scene_camera: bool,
    finished: bool,
    dt: f32,
) -> nfsu2::drive::AiControls {
    use nfsu2::ai::driver::DriverInputs;
    let h = &car.car_type.handling;
    let suv = car.car_type.suv;
    if let Some(drift) = r.drift.as_deref_mut() {
        return drift.update(state, path, target, clock, dt);
    }
    r.driver.finished = finished;
    // 0x420810 posts driver event 4 when the think planned the
    // path afresh (`RacerRoute::take_replanned`).
    if r.route.as_mut().is_some_and(|route| route.take_replanned()) {
        r.driver.event = 4;
    }
    if let Some(plan) = r.plan.as_deref_mut() {
        *plan = (path.to_vec(), target);
    }
    let input = DriverInputs {
        path,
        target_speed: target,
        enabled: true,
        green: clock >= 0.0,
        countdown: clock < 0.0,
        since_green: clock,
        clock: f64::from(clock),
    };
    let mut out = r.driver.update(state, h, suv, &input, dt);
    if scene_camera && clock < 0.0 {
        // Race status 2 (the start scene's camera): the countdown branch
        // (0x421030) is status 3 only; the cars stand in neutral with the
        // engine at idle (**observed**: gear 1, ≈ 850–900 rpm at the
        // countdown's first frame; what the drivers do under the scene's
        // prov: inferred game.racer_think
        // NIS movers is **inferred**: no gas, the hold's brake, neutral).
        out = r.driver.scene_hold();
    }
    if !pedals {
        // Driver +0x23C clear: 0x5EB920(0) and 0x5EB900(0), no throttle
        // and no brake.
        out.gas = 0.0;
        out.brake = 0.0;
    }
    out
}
