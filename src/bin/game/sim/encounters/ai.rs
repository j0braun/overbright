use crate::*;

/// One of career free roam's encounter racers (0x53F010: racer numbers
/// 100–102), parked until the encounter manager places it (0x42A5E0).
pub(crate) struct EncounterCar {
    /// Shared with the worker that builds its renderer ahead (`render::cars_ahead`).
    pub(crate) car: std::sync::Arc<Car>,
    pub(crate) belt: BeltState,
    pub(crate) belt_hold: i32,
    pub(crate) belt_block_ticks: i32,
    pub(crate) belt_rating: f32,
    pub(crate) driver_plan: (Vec<Vec3>, f32),
    /// Car +96C, retained independently of the current action until motion consumes it.
    pub(crate) recovery_pending: bool,
    /// Its renderer is `RenderState.cars[id]`.
    pub(crate) id: CarId,
    pub(crate) state: CarState,
    /// On the road (0x5ED440: its z off the parking height 0x8025B8).
    pub(crate) active: bool,
    /// The slot's stamp (+0x00: when placed, the manager's seconds; None
    /// before the first, which the game's tick count always passes) and the
    /// engage test's held flag (+0x08).
    pub(crate) placed: Option<f32>,
    pub(crate) held: bool,
    /// Done after an outrun (car +0x998) since when (+0x974; the hold
    /// +0x978 is `ENCOUNTER_HOLD`).
    pub(crate) done: Option<f32>,
    /// The racer's route, target speed, driver and GetUnstuck, as the race
    /// rivals' (the clone's stand-in for the encounter goals, see
    /// `Encounters`).
    pub(crate) route: Option<nfsu2::ai::route::RacerRoute>,
    pub(crate) speed: nfsu2::ai::route::SpeedTarget,
    pub(crate) driver: nfsu2::ai::driver::AiDriver,
    pub(crate) controls: nfsu2::drive::AiControls,
    /// The racer's own seed (its row's draw at the set-up, kept for the RNG
    /// unit). No think draws from it: the speed offset draws the world's
    /// generator (0x41C71E: 0x5EA510 with ECX = [0x890080]).
    #[allow(dead_code)]
    pub(crate) rng: nfsu2::opponents::GameRng,
    pub(crate) accel: Box<dyn Fn(f32) -> f32>,
    pub(crate) unstuck: nfsu2::ai::driver::Unstuck,
    /// The driver's throttle cap (+0x22C: the difficulty's 0x783A28 entry
    /// for a racer).
    pub(crate) controller: nfsu2::ai::encounter_controller::Controller,
    pub(crate) cap: f32,
    /// The manager's side state of the car's slot (+0x1B14 + 0x10·k,
    /// 0x42A860; 1 from the placement, 0x42A840).
    pub(crate) side: nfsu2::ai::actions::PursuitSide,
}

/// Full car+60 and cached human world+80 are COM positions (609D65,
/// 609E11; native road placement5F4F75 publishes COM height). CarState
/// stores the mesh origin, so both sides need the same body-frame offset.
pub(crate) fn belt_car_position(car: &Car, state: &CarState) -> Vec3 {
    state.position + glam::Mat3::from_quat(state.orientation) * Vec3::from(car.car_type.handling.centre_of_mass)
}

pub(crate) fn encounter_mover_kind(r: &EncounterCar) -> u32 {
    if r.belt.belt.is_some() {
        4
    } else if r.state.melo.is_some() {
        3
    } else {
        1
    }
}

pub(crate) fn encounter_leave_belt(r: &mut EncounterCar) {
    if r.belt.belt.take().is_none() {
        return;
    }
    if r.belt_hold > 0 {
        r.state.leave_melo();
    } else {
        r.state.melo = Some(nfsu2::melo::Melo::new(&r.car, &r.state, r.belt_rating));
    }
}

/// Existing contact notifications represent native 402450's ordinary/human
/// cases. Other native forced-contact variants remain unrepresented.
pub(crate) fn encounter_belt_contact(r: &mut EncounterCar, loaded: bool) {
    if std::mem::take(&mut r.state.human_hit) {
        r.belt_hold = HOLD_TICKS;
        if loaded {
            r.state.leave_melo();
        }
    }
    if std::mem::take(&mut r.state.belt_hit) {
        r.belt_block_ticks = 8000; // 402450:2s ×4000
        if loaded {
            encounter_leave_belt(r);
        }
    }
}

pub(crate) fn encounter_mover_timers(r: &mut EncounterCar, loaded: bool, dt: f32) {
    if r.belt_hold > 0 {
        r.belt_hold = (r.belt_hold - (dt * 4000.0) as i32).max(0);
        if r.belt_hold == 0 {
            if r.state.wheels_down() < 4 {
                r.belt_hold = 4000;
            } else if loaded {
                //42C2BC: BL is the cached loaded flag from42C0D7.
                r.belt.belt = None;
                r.state.melo = Some(nfsu2::melo::Melo::new(&r.car, &r.state, r.belt_rating));
            }
        } else if loaded {
            //42C2CF..DE enforces physics even when mode4 was retained while
            // unloaded. It is independent of the +730 contact-block timer.
            r.belt.belt = None;
            r.state.leave_melo();
        }
    }
    if r.belt_block_ticks > 0 {
        if loaded {
            encounter_leave_belt(r);
        }
        // 577320: subtract trunc(PC24(dt*4000)), clamp below zero.
        r.belt_block_ticks = (r.belt_block_ticks - (dt * 4000.0) as i32).max(0);
    }
}

/// An encounter racer's own inputs to `encounter_racer_think`: the road
/// entries it sees and its own, its contacts, its slot of `count`, the
/// world's clock, its goal (5 chase, 6 lead, 1 after the end), the chase's target, the player's
/// place, the outrun's race clock and start tick, the road entries'
/// sample interval, the human's body centre and whether its world is
/// loaded.
#[derive(Clone, Copy)]
pub(crate) struct EncounterThinkIn<'a> {
    pub(crate) others: &'a [nfsu2::ai::actions::RoadCar],
    pub(crate) me: nfsu2::ai::actions::RoadCar,
    pub(crate) hits: &'a [usize],
    pub(crate) touching: bool,
    pub(crate) slot: i32,
    pub(crate) count: i32,
    pub(crate) world_clock: &'a nfsu2::ai::encounter_controller::WorldClock,
    pub(crate) goal: nfsu2::ai::encounter_controller::EncounterGoal,
    pub(crate) target_position: Vec3,
    pub(crate) player: Vec2,
    pub(crate) race_clock: f32,
    pub(crate) race_start_tick: u32,
    pub(crate) sample_dt: f32,
    pub(crate) human_position: Vec3,
    pub(crate) loaded: bool,
}

/// The goal's speed for the driver (0x41C5E0, the goal's speed slot):
/// in the outrun (race type 7) goals 5 / 6 take the route's speed × 0.84
/// for the leader, × the catch-up 1 (`update_outrun`); goal 1 after the end
/// runs with race type 0 (0x5F84A0 clears 0x89E7B0), so the offline
/// racer's speed (`update`) × the catch-up 0.75 that 0x4172D0 gives with no
/// race on (0x783B74, 0x417337) — as goal 9 does outside the pursuit.
/// Goal 1's slot +0x4C (0x40DDD0) caps at 1e9: no cap. **Observed** under
/// Wine (outrun_brake_p14 b1–b3, the rival's row from the end): goal 1,
/// action 0 then 6, `ai_catch` 0.75, target 43 / 31 / 39 m/s after the
/// outrun's 62 / 45 / 58.
fn goal_speed(
    speed: &mut nfsu2::ai::route::SpeedTarget,
    ai: &AiData,
    route: &nfsu2::ai::route::RacerRoute,
    rng: &mut nfsu2::opponents::GameRng,
    clock: f32,
    goal: nfsu2::ai::encounter_controller::EncounterGoal,
    dt: f32,
) -> f32 {
    use nfsu2::ai::encounter_controller::EncounterGoal;
    match goal {
        EncounterGoal::RoadRace => speed.update(&ai.tuning, route, rng, f64::from(clock), None, dt) * 0.75,
        EncounterGoal::Chase => speed.update_outrun(&ai.tuning, route, rng, f64::from(clock), false, dt),
        EncounterGoal::Lead => speed.update_outrun(&ai.tuning, route, rng, f64::from(clock), true, dt),
    }
}

/// Active goal 5 / 6 / 1 controller (goal 9 is `goal9_think`'s); native race
/// actions are reused with retained output between thinks.
pub(crate) fn encounter_racer_think(r: &mut EncounterCar, ctx: ThinkCtx, input: EncounterThinkIn) {
    use nfsu2::ai::{actions::Actions, encounter_controller::ThinkInput};
    let ThinkCtx { ai, routes, field_rng: rng, clock, dt } = ctx;
    let EncounterThinkIn { slot, count, world_clock, goal, target_position, player, race_clock, loaded, .. } = input;
    let chase = goal == nfsu2::ai::encounter_controller::EncounterGoal::Chase;
    r.belt_rating = ai.bounds.scale(r.car.car_type.handling.performance_rating);
    encounter_mover_timers(r, loaded, dt);
    r.driver.mover = encounter_mover_kind(r);
    r.controller.speed_callbacks = 0;
    r.controller.search_callbacks = 0;
    let h = &r.car.car_type.handling;
    let suv = r.car.car_type.suv;
    if r.controller.assign_goal(goal) {
        r.unstuck.since = None;
    }
    let full = r.controller.schedule.full_think(ThinkInput {
        dt,
        tick: world_clock.tick,
        world_update: world_clock.update,
        owner_class: 2,
        owner_kind: 3,
        owner_id: (100 + slot) as i16,
        racer_slot: slot,
        racer_count: count,
        drag: false,
        mover_four: r.belt.belt.is_some(),
        action_present: r.controller.actions.is_some(),
    });
    let ready =
        !full && r.unstuck.since.is_none() && r.controller.actions.as_ref().is_some_and(Actions::ready_between_thinks);
    let skill = r.speed.skill();
    let minimum = ai.tuning.nitrous_seconds(skill);
    let engine_ok = r.state.ai_nitrous_ok(h, minimum, ai.tuning.nitrous_gear(skill, false));
    let tick = world_clock.tick;
    let fallback_running = !full && r.state.nitrous.as_mut().is_some_and(|t| t.level() != 0);
    if fallback_running
        && !engine_ok
        && r.controller.nitrous.may_stop(tick, minimum)
        && let Some(tank) = r.state.nitrous.as_mut().filter(|t| t.running.is_some())
    {
        tank.stop();
        r.controller.nitrous.ended = tick;
    }
    let mut target = r.controller.target_speed;
    if let Some(route) = r.route.as_mut() {
        route.clock = race_clock;
        route.set_outrun_chaser(chase);
        // Goal 6's junctions (slot +0x28, 0x41BD00 → 0x413F50) weigh the
        // second car's distance; goal 1's (0x41BCF0 → 0x413CC0) are the
        // ordinary racer's.
        route.outrun_leader_positions = (goal == nfsu2::ai::encounter_controller::EncounterGoal::Lead)
            .then_some((r.state.position.truncate(), player));
        if !r.controller.suspended {
            // Fallback speed runs even when action virtual+10 is false.
            if !full {
                r.controller.speed_callbacks += 1;
                target = goal_speed(&mut r.speed, ai, route, &mut *rng, clock, goal, dt);
            }
            if full || ready {
                if full {
                    r.controller.actions.get_or_insert_with(Actions::default);
                    if chase {
                        let direction = if r.state.velocity.length() >= 1.0 {
                            r.state.velocity.truncate()
                        } else {
                            r.state.forward().truncate()
                        };
                        r.controller.search_callbacks += 1;
                        route.search_outrun_chaser(routes, r.state.position, direction, target_position);
                    }
                }
                std::mem::swap(&mut route.rng, rng);
                let travel = nfsu2::ai::route::travel(r.state.speed(), r.state.velocity, r.state.forward());
                route.update(routes, r.state.position, r.state.speed(), travel, h.performance_class, suv, &*r.accel);
                std::mem::swap(&mut route.rng, rng);
                if route.node().is_none() || route.past_end() {
                    // 0x428720 returns false without a list and also when
                    // the car's own entry lies past the last point
                    // (0x428C88..0x428CAD); 0x42B002's identical test after
                    // a true return therefore never suspends. Goal5/6 +1C
                    // is true: in race7 a full think's failure goes straight
                    // to cleanup (0x42AF6E → 0x42AFD1), preserving the
                    // driver's output; the ready fallback's (0x42ADA4)
                    // invokes 402530 and suspends.
                    if !full {
                        r.controller.suspended = true;
                    }
                } else {
                    let parts = EncounterParts {
                        state: &mut r.state,
                        belt: &r.belt,
                        controller: &mut r.controller,
                        speed: &mut r.speed,
                        unstuck: &mut r.unstuck,
                        placed: r.placed,
                        driver: &r.driver,
                    };
                    let think = EncounterThinkStart { full, skill, minimum, engine_ok, tick };
                    let ctx = ThinkCtx { ai, routes, field_rng: &mut *rng, clock, dt };
                    target = encounter_actions_step(route, parts, h, ctx, &input, think);
                }
            }
        }
    }
    target = encounter_driver_step(r, target, race_clock, clock, dt);
    encounter_belt_gate(r, full, &input, target);
    encounter_world_notice(r, loaded, target);
    r.driver.mover = encounter_mover_kind(r);
}

/// The parts of an encounter racer the action step changes or reads
/// besides its route (which it borrows from the car).
struct EncounterParts<'p> {
    state: &'p mut CarState,
    belt: &'p BeltState,
    controller: &'p mut nfsu2::ai::encounter_controller::Controller,
    speed: &'p mut nfsu2::ai::route::SpeedTarget,
    unstuck: &'p mut nfsu2::ai::driver::Unstuck,
    placed: Option<f32>,
    driver: &'p nfsu2::ai::driver::AiDriver,
}

/// What the think's start worked out for the action step: the full
/// think, the skill row, the shot's minimum, the engine's go and the
/// world's tick.
struct EncounterThinkStart {
    full: bool,
    skill: usize,
    minimum: f32,
    engine_ok: bool,
    tick: u32,
}

/// The route's action step (a walk with a node): the full think's
/// nitrous stop, GetUnstuck or the action's think (the full think) or its
/// refresh (the ready fallback), the second speed callback and the
/// action's nitrous request. Returns the target speed.
fn encounter_actions_step(
    route: &mut nfsu2::ai::route::RacerRoute,
    r: EncounterParts,
    h: &nfsu2::handling::Handling,
    ctx: ThinkCtx,
    input: &EncounterThinkIn,
    think: EncounterThinkStart,
) -> f32 {
    use nfsu2::ai::actions::{Actions, Scene, Steer};
    let ThinkCtx { ai, routes, field_rng: rng, clock, dt } = ctx;
    let EncounterThinkIn {
        others, me, hits, touching, slot, world_clock, goal, race_clock, race_start_tick, sample_dt, ..
    } = *input;
    let EncounterThinkStart { full, skill, minimum, engine_ok, tick } = think;
    if full && r.state.nitrous.as_mut().is_some_and(|t| t.level() != 0) {
        let wanted = r.belt.belt.is_none()
            && r.controller.nitrous.wanted(
                tick,
                true,
                ai.tuning.nitrous_gap(skill, false),
                r.state.velocity.length(),
                route.nitrous_road(),
                engine_ok,
            );
        if !wanted && r.controller.nitrous.may_stop(tick, minimum) {
            r.state.nitrous.as_mut().unwrap().stop();
            r.controller.nitrous.ended = tick;
        }
    }
    let held = f64::from(world_clock.tick.wrapping_sub(race_start_tick) as i32) * f64::from(0.00025f32)
        <= f64::from(nfsu2::ai::route::start_hold(100 + slot));
    let scene = Scene {
        routes,
        me,
        others,
        clock: race_clock,
        rate: 0.05,
        street_x: false,
        drift: false,
        drag: false,
        held,
        block_row: r.speed.skill(),
        hits,
        touching,
        wheels_down: r.state.wheels_down(),
        up_z: (glam::Mat3::from_quat(r.state.orientation) * Vec3::Z).z,
        forward_speed: r.state.speed(),
        odds: ai.tuning.pass_odds(r.speed.skill()),
    };
    if full {
        let actions = r.controller.actions.as_mut().unwrap();
        let since = r.placed.map_or(0.0, |t| clock - t);
        let was_reversing = r.unstuck.since.is_some();
        let reverse = r.unstuck.update(
            &r.state,
            h,
            route.facing_point(routes, false),
            Some(r.driver.target),
            route.edge(routes).as_ref(),
            since,
            None,
            f64::from(clock),
            actions.priority,
            held || r.state.wheels_down() <= 2,
        );
        r.controller.output = Some(if let Some(reverse) = reverse {
            if !was_reversing {
                *actions = Actions::default();
            }
            let fresh = r.unstuck.first_path();
            Steer {
                path: route.line_path(
                    routes,
                    nfsu2::ai::driver::UNSTUCK_LANE_RATE,
                    r.state.position,
                    r.state.forward(),
                    r.state.velocity.length(),
                    false,
                    fresh,
                    0.0,
                ),
                cap: Some(reverse),
                pedals: true,
                reset: false,
            }
        } else {
            actions.think(route, &scene, &ai.tuning, rng, false, sample_dt)
        });
    } else {
        r.controller.output = r.controller.actions.as_mut().unwrap().refresh(route, &scene, sample_dt);
    }
    // 420810 calls speed again after an action update. On
    // ready fallback this is its second speed callback.
    r.controller.speed_callbacks += 1;
    let target = goal_speed(&mut *r.speed, ai, route, &mut *rng, clock, goal, dt);
    if let Some(actions) = r.controller.actions.as_mut().filter(|_| r.unstuck.since.is_none()) {
        let running = r.state.nitrous.as_mut().is_some_and(|t| t.level() != 0);
        if actions.nitrous_query {
            actions.nitrous_request = !running
                && r.belt.belt.is_none()
                && r.controller.nitrous.wanted(
                    tick,
                    false,
                    ai.tuning.nitrous_gap(skill, false),
                    r.state.velocity.length(),
                    route.nitrous_road(),
                    engine_ok,
                );
        }
        if actions.nitrous_request
            && !running
            && let Some(tank) = r.state.nitrous.as_mut()
        {
            // 420810 stamps +7A8 before requesting the shot.
            r.controller.nitrous.started = tick;
            tank.start();
        }
    }
    target
}

/// The think's output to the driver: the action's cap, the suspended
/// stop, the retained or new path, then the driver's frame (0x421030).
/// Returns the target speed the driver was given.
fn encounter_driver_step(r: &mut EncounterCar, mut target: f32, race_clock: f32, clock: f32, dt: f32) -> f32 {
    use nfsu2::ai::driver::DriverInputs;
    let h = &r.car.car_type.handling;
    let suv = r.car.car_type.suv;
    // Keep legacy other-car velocity history per world update. Own-action
    // acceleration now comes from the retained CarState world sample.
    if let Some(actions) = r.controller.actions.as_mut() {
        actions.last_velocity = Some(r.state.velocity);
    }
    let output = r.controller.output.as_ref();
    if let Some(cap) = output.and_then(|o| o.cap) {
        target = target.min(cap);
    }
    if r.controller.suspended {
        target = 0.0;
    }
    // A missing steering update preserves CarDriver's retained path; an
    // explicitly supplied empty path clears it. Native full route failure
    // reaches cleanup without calling the driver's path setter.
    if let Some(output) = output {
        r.driver_plan.0.clone_from(&output.path);
    }
    r.driver_plan.1 = target;
    let input = DriverInputs {
        path: &r.driver_plan.0,
        target_speed: target,
        enabled: true,
        green: true,
        countdown: false,
        since_green: race_clock,
        clock: f64::from(clock),
    };
    r.controller.target_speed = target;
    // 0x420810 posts driver event 4 when the think planned the
    // path afresh (`RacerRoute::take_replanned`).
    if r.route.as_mut().is_some_and(|route| route.take_replanned()) {
        r.driver.event = 4;
    }
    r.controls = r.driver.update(&r.state, h, suv, &input, dt);
    if r.controller.suspended {
        r.controls.gas = 0.0;
        r.controls.brake = 1.0;
    }
    if output.is_some_and(|o| !o.pedals) && !r.controller.suspended {
        r.controls.gas = 0.0;
        r.controls.brake = 0.0;
    }
    target
}

/// The full think's belt gate (0x42B26C → 0x402180) on a loaded world,
/// or the unloaded mode-4 car's belt rebuilt.
fn encounter_belt_gate(r: &mut EncounterCar, full: bool, input: &EncounterThinkIn, target: f32) {
    let EncounterThinkIn { race_clock, human_position, loaded, .. } = *input;
    if full && loaded {
        let gate = nfsu2::ai::encounter_controller::BeltGate {
            special_track: false,
            drag: false,
            drift: false,
            path_points: r.driver_plan.0.len() as i32,
            timer730: r.belt_block_ticks as u32,
            timer734: r.belt_hold as u32,
            forward_speed: r.state.exported_forward_speed(),
            wheels_down: r.state.wheels_down() as i32,
            action: if r.unstuck.since.is_some() {
                Some(9)
            } else {
                r.controller.actions.as_ref().map(|a| a.kind.type_id() as u8)
            },
            skill: r.speed.skill() as i32,
            human_distance: nfsu2::ai::encounter_controller::belt_human_distance(
                belt_car_position(&r.car, &r.state),
                human_position,
            ),
            race_clock,
            finished: false,
        };
        if gate.wanted() {
            let speed = r.belt.belt.as_ref().map_or(r.state.exported_forward_speed(), |b| b.speed());
            if r.belt.belt.is_none() {
                r.state.leave_melo();
            }
            build_belt_state(&r.car, &r.state, &mut r.belt, &*r.accel, speed, &r.driver_plan.0, target);
        } else {
            encounter_leave_belt(r);
        }
    } else if full && r.belt.belt.is_some() {
        // 0x42B2A9: unloaded, a mode-4 car's belt is rebuilt (0x5918B0).
        let speed = r.belt.belt.as_ref().map_or(0.0, |b| b.speed());
        build_belt_state(&r.car, &r.state, &mut r.belt, &*r.accel, speed, &r.driver_plan.0, target);
    }
}

/// The world notice (controller slot +0x10 = 0x4022D0, from 0x5F6CE0): a
/// car whose section is streamed out (car +0x5CC null) goes to mode 4 when
/// its mover agrees (MeloMvr / physics slot +0x10 = 0x570A70: always);
/// loaded again it changes no mover (the next full think's gate does).
/// **Observed** under Wine (2026-11-04, outrun-live a_loss): the
/// challenged car went to mode 4 at race clock 10.19 s, the frame +0x5CC
/// cleared, 114 m from the player.
pub(crate) fn encounter_world_notice(r: &mut EncounterCar, loaded: bool, target: f32) {
    if !loaded && r.belt.belt.is_none() {
        r.state.leave_melo();
        let speed = r.state.exported_forward_speed();
        build_belt_state(&r.car, &r.state, &mut r.belt, &*r.accel, speed, &r.driver_plan.0, target);
    }
}

/// Recovery placement uses the existing road-node approximation (see
/// formats.md, AccidentRecovery). Preserve drivetrain/tank and unrelated car
/// state; a fresh CarState here would silently refill/reinitialize components.
pub(crate) struct EncounterReset<'a> {
    pub(crate) belt: &'a mut BeltState,
    pub(crate) driver_plan: &'a mut (Vec<Vec3>, f32),
    pub(crate) route: &'a mut Option<nfsu2::ai::route::RacerRoute>,
    pub(crate) driver: &'a mut nfsu2::ai::driver::AiDriver,
    pub(crate) controller: &'a mut nfsu2::ai::encounter_controller::Controller,
    pub(crate) controls: &'a mut nfsu2::drive::AiControls,
    pub(crate) pending: &'a mut bool,
    pub(crate) cap: f32,
    pub(crate) world_id: i16,
    pub(crate) routes: &'a nfsu2::ai::route::AiRoutes,
}

impl EncounterReset<'_> {
    pub(crate) fn apply(&mut self, world: &World, car: &Car, state: &mut CarState) -> bool {
        *self.pending = false;
        let Some((record, cp, backward)) = self.route.as_ref().and_then(|route| route.node()) else {
            return false;
        };
        let dir = self.routes.file.direction(record, cp, backward);
        let kept = (state.speed().max(0.0) * dir.dot(state.forward().truncate()).max(0.0)).min(RESET_SPEED);
        let p = self.routes.file.records[record].points[cp].position;
        let ground = world.road_height_below(p + Vec3::Z * 8.0).unwrap_or(p.z);
        let pose = CarState::placed(&car.car_type, world, Vec3::new(p.x, p.y, ground), dir.y.atan2(dir.x));
        state.position = pose.position;
        state.orientation = pose.orientation;
        state.set_placement_speed(kept);
        // 5F52D0 -> 59F890(body = car+40), stores at body+40/+44/+48.
        // The independent outer-world token still publishes its full delta.
        state.world_acceleration = Vec3::ZERO;
        state.spin = Vec3::ZERO;
        state.angular_momentum = Vec3::ZERO;
        state.rest_damping = 0.0;
        state.rest = 0;
        let barriers = std::mem::take(&mut state.ground.barriers);
        state.ground = pose.ground;
        state.ground.barriers = barriers;
        self.belt.belt = None;
        *self.driver_plan = Default::default();
        *self.route = nfsu2::ai::route::RacerRoute::start(self.routes, state.position, dir);
        *self.driver = nfsu2::ai::driver::AiDriver::new(&car.car_type.handling, car.car_type.suv, self.cap);
        self.controller.output = None;
        self.controller.target_speed = 0.0;
        self.controller.reset_placement(self.world_id);
        if let Some(actions) = self.controller.actions.as_mut() {
            actions.last_velocity = Some(state.velocity);
        }
        *self.controls = Default::default();
        true
    }
}

#[cfg(all(test, feature = "dev-harness"))]
pub(crate) fn reset_encounter(
    world: &World,
    r: &mut EncounterCar,
    routes: &nfsu2::ai::route::AiRoutes,
    world_id: i16,
) -> bool {
    EncounterReset {
        belt: &mut r.belt,
        driver_plan: &mut r.driver_plan,
        route: &mut r.route,
        driver: &mut r.driver,
        controller: &mut r.controller,
        controls: &mut r.controls,
        pending: &mut r.recovery_pending,
        cap: r.cap,
        world_id,
        routes,
    }
    .apply(world, &r.car, &mut r.state)
}

/// An encounter racer from its loaded car, parked (`Encounters::ready`).
pub(crate) fn encounter_car(car: Car, row: usize, seed: u32, throttle: usize) -> EncounterCar {
    let h = &car.car_type.handling;
    // CarDriver +0x22C (0x425B52): 0x783A28[0x89E804], the throttle
    // level as the last set-up left it (the city starts do not set
    // it, `Game::race_globals`).
    let cap = nfsu2::ai::driver::THROTTLE_CAP[throttle.min(3)];
    let driver = nfsu2::ai::driver::AiDriver::new(h, car.car_type.suv, cap);
    let accel: Box<dyn Fn(f32) -> f32> = Box::new(nfsu2::handling::acceleration(h));
    EncounterCar {
        car: std::sync::Arc::new(car),
        belt: Default::default(),
        belt_hold: 0,
        belt_block_ticks: 0,
        belt_rating: 0.0,
        driver_plan: Default::default(),
        recovery_pending: false,
        id: CarId::new(),
        state: CarState::new(Vec3::splat(1e6), 0.0),
        active: false,
        placed: None,
        held: false,
        done: None,
        route: None,
        speed: nfsu2::ai::route::SpeedTarget::new(row),
        driver,
        controls: Default::default(),
        rng: nfsu2::opponents::GameRng(seed),
        accel,
        unstuck: Default::default(),
        controller: Default::default(),
        cap,
        side: Default::default(),
    }
}
