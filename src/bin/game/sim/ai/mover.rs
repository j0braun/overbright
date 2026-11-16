use crate::*;

/// The AI racer controller's mover choice (0x42C0C0, before its think
/// 0x42ABD0; verified in the code and live, Quick Race circuit): before
/// the green (0x5EEF00 false) the hold is 5 s; after it the hold counts
/// down (0x577320: − trunc(dt × 4000) ticks, 0 when used up) and keeps
/// the physics mover (mode 1); when it runs out with all four wheels
/// down (car +0x424 = 4) the racer takes 0x5F4410's mode, the MeloMvr (3:
/// the controller's slot +0x50, 0x41E510, is false with the hold at 0),
/// else the hold gets 1 s more (4000 ticks). A finished racer (car +0x1C
/// and 0x5EE260) is on the physics mover. Drag races (0x89E7D8) and drift
/// races (0x401950) keep every racer on it (0x5F4410 returns 1). A contact
/// with the human's car (0x402450: the other car's +0x18 set) puts the
/// racer back on the physics mover with a fresh 5 s hold
/// (`CarState::human_hit`, switched inside the collision step). Live
/// (Wine, Resort Loop, 3 racers): mode 1 until 5 s after the green, then
/// mode 3 for the rest of the lap. Mode 4 (0x5A9040, a mover that runs
/// the car along its path, vtable 0x7A214C, update 0x59EC90) replaces 1 / 3
/// while the world under the car is not loaded (car +0x5CC null, 0x42C0D7,
/// 0x4024CE): the clone keeps the whole world loaded, so never.
pub(crate) fn racer_mover(r: &mut Rival, race_type: nfsu2::race::RaceMode, clock: f32, rating: f32, dt: f32) {
    if race_type.drag() || race_type.drift() {
        return;
    }
    if clock < 0.0 {
        r.hold = HOLD_TICKS;
    }
    let physics = if r.progress.finished.is_some() {
        true
    } else if r.hold == 0 {
        return;
    } else {
        r.hold -= (dt * 4000.0) as i32;
        if r.hold > 0 {
            true
        } else {
            r.hold = 0;
            if r.state.wheels_down() < 4 {
                r.hold = 4000;
                return;
            }
            false
        }
    };
    if physics {
        r.state.leave_melo();
        r.belt.belt = None;
    } else if r.state.melo.is_none() && r.belt.belt.is_none() {
        r.state.melo = Some(nfsu2::melo::Melo::new(&r.car, &r.state, rating));
    }
}

/// The racers' off-screen mover (mode 4, the BeltMover 0x5A9040,
/// `nfsu2::ai::belt`; verified in the code, **observed** under Wine, Quick
/// Race circuit 4014, the human parked at the start: every rival went to
/// mode 4 at race clock 30.0 s 395–434 m away with the world under it
/// loaded, back to the MeloMvr 91–165 m → 64–126 m from the human when it
/// came round, and to mode 4 again past 120 m; its speed 0.88–0.90 × the
/// driver's target). The racer controller's full think (0x42ABD0 from
/// 0x42AE2F: on the racer's turn, the world update counter 0x7FB724 mod 8 =
/// its place among the AI cars of the world list, once 0.2 s (0x784554)
/// have passed since its last, controller +0x778) ends (0x42B259) with,
/// for a car whose world is loaded (car +0x5CC; always in the clone),
/// 0x402180: true puts it on mode 4 (0x5F46B0(4), or rebuilds the belt
/// when already on it, 0x5918B0), false hands a mode-4 car back to
/// 0x5F4410's mode. 0x402180 is true when none of: the special track (0x89E7A0
/// = 1099 with 0x86B320 and 0x86B328 = 4; **not ported**, no shipped
/// track number), a drag (0x89E7D8 / 0x890108) or drift (0x89E7D9 /
/// 0x890104) race, a driver path of at most two points, the +0x730 or
/// +0x734 timer running, car +0x42C under 5 m/s (0x783504), no wheel down
/// (+0x424 < 1), no action or an action of type 8, 9 (GetUnstuck), 0xD
/// (AccidentRecovery), 0xA or 0xB (IJustGotHit), the human's skill row (−2,
/// slot +0x78), the nearest human car (0x5F1170: world +0x80 + 16·i, the
/// first +0x14 cars' positions, copied each frame by 0x609CF0) under 120 m
/// (0x78350C, 3D), the race clock (race status +0x14 / 4000) under 30 s
/// (0x783508), or the racer finished (car +0x1C, 0x5EE260). A car whose
/// world is not loaded runs mode 4 whatever 0x402180 says (the world notice
/// 0x4022D0 and the contact handler 0x402450 switch it; the clone keeps
/// every section loaded, so this part of the rule is **not ported**, see
/// formats.md "BeltMover").
pub(crate) fn belt_wanted(
    r: &Rival,
    race_type: nfsu2::race::RaceMode,
    clock: f32,
    human: Vec3,
    path_points: usize,
) -> bool {
    use nfsu2::ai::actions::Kind;
    !(race_type.drag()
        || race_type.drift()
        || path_points <= 2
        || r.belt.block > 0.0
        || r.hold > 0
        || belt_speed(r) < 5.0
        || r.state.wheels_down() < 1
        || r.unstuck.since.is_some()
        || matches!(r.actions.kind, Kind::Recover | Kind::GotHit)
        || r.state.position.distance(human) < 120.0
        || clock < 30.0
        || r.progress.finished.is_some())
}

/// Whether the world under a car is loaded (car +0x5CC, the collision pack
/// of its drivable section, 0x5E68E0 → 0x5DFEC0 / 0x591390 on the pack
/// table 0x88F768): the streamer holds the packs of the detail copies
/// (section + the detail limit, 0x800DFC = 20 in L4RA; 0x5D3C20 maps
/// them back) that the visible-section list (0x34153) of the human's
/// section names. **Observed** under Wine (Quick Race 4014, the human
/// parked at the grid): the 23 packs held were exactly those list entries
/// (721, 723, 724, 729–739, 1328 … 1833), and car +0x5CC matched this
/// rule in 487 of 487 samples of the rivals over 90 s. **Inferred**: the
/// list follows the human car's section (not the camera's) and changes at
/// once when it moves (the streamer's loading lag is not traced). No list
/// (no visibility data) counts as loaded.
pub(crate) fn streamed_in(world: &World, list: Option<&[u32]>, at: Vec3) -> bool {
    let Some(list) = list else { return true };
    let vis = &world.visibility;
    match vis.section_at(at.truncate()) {
        Some(s) => list.contains(&(s + vis.detail_limit)),
        None => true,
    }
}

/// Car +0x42C: the speed along the heading, which on mode 4 is the
/// belt's own (0x59EC90 writes it).
pub(crate) fn belt_speed(r: &Rival) -> f32 {
    r.belt.belt.as_ref().map_or(r.state.speed(), |b| b.speed())
}

/// The controller's +0x730 after a mode-4 car's contact (0x402450,
/// 0x783510), s.
pub(crate) const BELT_CONTACT_BLOCK: f32 = 2.0;

/// Back from mode 4 to 0x5F4410's mode (0x5F46B0(0x5F4410(car))): the
/// physics mover while the hold runs or in drag / drift, else the MeloMvr.
pub(crate) fn leave_belt(r: &mut Rival, race_type: nfsu2::race::RaceMode, rating: f32) {
    if r.belt.belt.take().is_none() {
        return;
    }
    if race_type.drag() || race_type.drift() || r.hold > 0 {
        r.state.leave_melo();
    } else {
        r.state.melo = Some(nfsu2::melo::Melo::new(&r.car, &r.state, rating));
    }
}

/// The belt's build from the driver's path and target (0x5918B0 /
/// 0x5916A0), with 0x4038D0's mode-4 acceleration: the car's curve
/// (+0x55C, 0x5764E0) at v / catch-up (car +0x468), held to the drag over
/// the gearing top speed ([car +0x10] +0x20) when that is above 0, less
/// the drag (1 − draft) × H +0x260 × v² / mass, plus the slope term.
pub(crate) fn build_belt(r: &mut Rival, path: &[Vec3], target: f32) {
    let speed = belt_speed(r);
    build_belt_state(&r.car, &r.state, &mut r.belt, &*r.accel, speed, path, target);
}

pub(crate) fn build_belt_state(
    car: &Car,
    state: &CarState,
    belt: &mut BeltState,
    accel: &dyn Fn(f32) -> f32,
    speed: f32,
    path: &[Vec3],
    target: f32,
) {
    let h = &car.car_type.handling;
    let quarter = *belt.trap_speed.get_or_insert(h.gearing_top_speed);
    let (catch_up, draft) = (state.catch_up.max(1e-3), state.draft);
    let max_accel = |v: f32, slope: f32| {
        let v = v / catch_up;
        let mut a = accel(v);
        let drag = (1.0 - draft) * h.drag * v * v / h.mass;
        if quarter > 0.0 && quarter <= v && drag < a {
            a = drag;
        }
        a - drag + slope
    };
    let car = nfsu2::ai::belt::CarNow {
        position: state.position + state.orientation * Vec3::from(h.centre_of_mass),
        velocity: state.velocity,
        speed,
        heading: state.heading(),
    };
    let built = nfsu2::ai::belt::Belt::build(belt.belt.as_ref(), path, car, target, 0.0, max_accel);
    belt.belt = Some(built);
}

/// One BeltMover frame (0x59EC90): the drop test while the world under
/// the car is loaded (0x59EA30(0.5) over 1 m sets the controller's +0x730
/// to 1 s, 0x591550, so the next think hands the car back), then the move:
/// the car's centre at the belt's point, its heading and speed, laid on the
/// road (0x5F5020, loaded) with the wheels rolling. A belt that cannot run
/// is rebuilt from this frame's path (0x5918B0) and the car stopped
/// (+0x70 = 0, +0x42C = +0x430 = 0). Resolved contacts are handled by
/// drive::off_belt and the callers' contact notifications. The wheels'
/// effects within200m of a human (0x59DBC0) remain unported.
pub(crate) fn step_belt(world: &World, r: &mut Rival, plan: &(Vec<Vec3>, f32), loaded: bool, dt: f32) {
    if step_belt_state(world, &r.car, &mut r.state, &mut r.belt, &*r.accel, plan, loaded, dt) {
        r.belt.block = 1.0;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn step_belt_state(
    world: &World,
    car: &Car,
    state: &mut CarState,
    belt: &mut BeltState,
    accel: &dyn Fn(f32) -> f32,
    plan: &(Vec<Vec3>, f32),
    loaded: bool,
    dt: f32,
) -> bool {
    let h = &car.car_type.handling;
    let Some(b) = belt.belt.as_mut() else { return false };
    let dropped = loaded && b.valid && b.drop(h.downforce[0] + h.downforce[1], h.mass) > nfsu2::ai::belt::DROP_LIMIT;
    let before = (state.position, state.heading());
    match b.step(dt) {
        Some((at, heading, speed)) => {
            let com = Vec3::from(h.centre_of_mass);
            let (f, l) = (Vec2::from_angle(heading), Vec2::from_angle(heading).perp());
            let origin = at.truncate() - f * com.x - l * com.y;
            place_on_road(world, car, state, (origin, at.z), heading, speed, before, dt);
        }
        None => {
            let speed = belt.belt.as_ref().map_or(state.exported_forward_speed(), |b| b.speed());
            build_belt_state(car, state, belt, accel, speed, &plan.0, plan.1);
            state.velocity = Vec3::ZERO;
            // Existing belt rebuild fallback stops motion; publish its synthetic stop.
            state.set_exported_forward_speed(0.0);
        }
    }
    dropped
}
