use crate::*;

/// An opponent in the race: its driver, car, where the car is and its run.
pub(crate) struct Rival {
    pub(crate) name: String,
    pub(crate) car: Car,
    /// Its renderer is `RenderState.cars[id]`.
    pub(crate) id: CarId,
    pub(crate) state: CarState,
    pub(crate) progress: Progress,
    /// Racer info +0x902 (0x6373F0's sixth argument): the Block row of the
    /// AI tuning (+0x934 + 8·row) — a career entry's byte +0x0B (1 in
    /// every event), 1 from the Quick Race set-up (0x53EF75).
    pub(crate) block_row: usize,
    /// Its place on the AI route (FuturePoints), target speed state and
    /// low-level driver (0x425AB0), and the driver's last outputs.
    pub(crate) route: Option<nfsu2::ai::route::RacerRoute>,
    pub(crate) speed: nfsu2::ai::route::SpeedTarget,
    pub(crate) driver: nfsu2::ai::driver::AiDriver,
    pub(crate) controls: nfsu2::drive::AiControls,
    /// The racer's own random numbers (car +0xF8).
    pub(crate) rng: nfsu2::opponents::GameRng,
    /// The car's acceleration curve (perf object +0x55C) for the route's
    /// speed profile.
    pub(crate) accel: Box<dyn Fn(f32) -> f32>,
    /// Its GetUnstuck action (type 9).
    pub(crate) unstuck: nfsu2::ai::driver::Unstuck,
    /// Its driving action and lane record (JustDrive / Pass / Block,
    /// controller +0x6F8 / +0x30C; see `ai::actions`).
    pub(crate) actions: nfsu2::ai::actions::Actions,
    /// Its traffic lane in a drag race (the lane action, controller +0x30C).
    pub(crate) lane: nfsu2::ai::drag::DragLane,
    /// In a drift race: its driver is the DriftDriver (0x4260B0 builds
    /// 0x425E40 for every AI racer, controller type 3, with the drift flag
    /// and no drag flag), and it scores as the player does (0x5F7A10: a
    /// style manager and drift manager for owner kind 2 too, since
    /// 0x802228 is 1 and never written; 0x56D7E0 rate 0x79DAF0[difficulty]).
    pub(crate) drift: Option<nfsu2::ai::driver::DriftDriver>,
    pub(crate) style: Option<nfsu2::drift::StyleManager>,
    pub(crate) scorer: Option<nfsu2::drift::DriftManager>,
    /// The timed refill's last time (racer +0x28: the green light, then
    /// each refill) and road percent (+0x2C, 0 from 0x5FC4B4).
    pub(crate) refill: (f32, f32),
    /// The controller's physics hold (+0x734, ticks of 1/4000 s counting
    /// down; see `racer_mover`): while it runs the racer is on the physics
    /// mover, after it on the MeloMvr.
    pub(crate) hold: i32,
    /// Its grid slot, 0-based (racer info +0x03 − 1, 0x5264B0): the
    /// starting position it is placed on and the start scene's
    /// `ZPM_Car<slot + 1>`.
    pub(crate) slot: usize,
    /// The off-screen mover (mode 4) and its timers (see `racer_belt`).
    pub(crate) belt: BeltState,
    /// The controller's full-think schedule (0x42ABD0, owner class 2: the
    /// world update counter mod 8 = its place among the AI racers, 0.2 s
    /// since the last) and the action's output kept between full thinks.
    pub(crate) think: nfsu2::ai::encounter_controller::ThinkSchedule,
    pub(crate) retained: Option<nfsu2::ai::actions::Steer>,
}

/// A racer's BeltMover state (mode 4, `nfsu2::ai::belt`): the mover when
/// on it, the controller's +0x730 timer (s; while it runs a loaded car on
/// mode 4 is handed back, 0x42AC04), the time since its last full think
/// (controller +0x778) and the speed above which the belt's acceleration
/// (controller +0x778) and the speed above which the belt's acceleration
/// is held to the drag (0x4038D0's [car +0x10] +0x20): the racer info's
/// +0x30, the performance object's gearing top speed (traced: writer
/// 0x5B5CB5 in 0x5B5AC0; 73.76 / 67.05 / 75.10 m/s for the 206 / 240SX /
/// MIATA under Wine, `Handling::gearing_top_speed`).
#[derive(Clone, Default)]
pub(crate) struct BeltState {
    pub(crate) belt: Option<nfsu2::ai::belt::Belt>,
    pub(crate) block: f32,
    pub(crate) since_think: f32,
    pub(crate) trap_speed: Option<f32>,
    /// AI nitrous outside drag (0x416F70): the race clock when the last
    /// shot ended (controller +0x7AC; None before any) and how long the
    /// road ahead has not slowed (+0x7B0).
    pub(crate) nos_end: Option<f32>,
    pub(crate) nos_flat: f32,
}

/// A race clock in whole game ticks (1/4000 s): the original's race time
/// is the race status's tick count +0x14 × 0.00025, the clone's a sum of
/// f32 frame steps that can sit a hair past a whole tick.
pub(crate) fn whole_ticks(clock: f32) -> f32 {
    (clock * 4000.0).round() / 4000.0
}

/// A drag rival's target speed (driver +0x230): the controller's cap
/// 0x784550 (observed in every drag_s1 run from the first think).
pub(crate) const DRAG_TARGET_SPEED: f32 = 1000.0;

/// AI nitrous outside a drag race (controller slot +0xA0, 0x416F70;
/// traced, see formats.md "AI nitrous outside drag"): false on mode 4 or
/// without a racer status. With st the part of the lap past the line, the
/// last lap and the end (last lap, st > 0.75): a row with +0x589 set
/// refuses a tank under 1 s before the end; a shot running longer than
/// +0x574 refuses; with none running the time since the last ended must
/// pass +0x568 (+0x56C on the last lap, 0 at the end). Then it wants a
/// shot (0x40D850 deciding, `CarState::ai_nitrous_ok`) on a traffic lane
/// (+0x30C 2–5; the clone's road racers keep lane 0), when the speed is
/// under 0.8 × the FuturePoint lane speed at the car, or when the lane
/// speeds ahead have not dropped for more than 1 s. Race type 7's values
/// (2 s, 1.0) are not used: the clone has no such race.
pub(crate) fn ai_nitrous_wanted(
    r: &Rival,
    tuning: &nfsu2::ai::route::AiTuning,
    skill: usize,
    laps: u32,
    clock: f32,
) -> bool {
    let Some(tank) = r.state.nitrous.as_ref() else { return false };
    if r.belt.belt.is_some() || r.progress.finished.is_some() || clock < 0.0 {
        return false;
    }
    let done = r.progress.laps.as_ref().map_or(0.0, |l| l.progress).max(0.0);
    let last = laps > 0 && done.floor() as u32 >= laps - 1;
    let end = last && done.fract() > 0.75;
    if tuning.nitrous_keeps_reserve(skill) && tank.remaining() < 1.0 && !end {
        return false;
    }
    match tank.running {
        Some(t) if t > tuning.nitrous_longest(skill) => return false,
        Some(_) => {}
        None => {
            let gap = if end { 0.0 } else { tuning.nitrous_gap(skill, last) };
            if r.belt.nos_end.is_some_and(|e| clock - e <= gap) {
                return false;
            }
        }
    }
    let (lane_speed, _) = r.route.as_ref().map_or((None, false), |x| x.nitrous_road());
    let slow = lane_speed.is_some_and(|v| r.state.speed() / v < 0.8);
    slow || r.belt.nos_flat > 1.0
}

/// A drag rival's shot end (0x42AD24..0x42AD55): with a shot running
/// and 0x40D850 failing (`ok` false) the shot ends (0x592510; slot +0xB4,
/// 0x408430, always allows it in a drag race). Returns whether it ended
/// (the caller stamps +0x7AC, slot +0xA8 0x41E500).
pub(crate) fn drag_nitrous_end(tank: &mut nfsu2::nitrous::Tank, ok: bool) -> bool {
    if ok || tank.running.is_none() {
        return false;
    }
    tank.stop();
    true
}

/// 5 s of physics hold (0x783518 / 0x78349C × 4000 + 0.5, truncated).
pub(crate) const HOLD_TICKS: i32 = 20000;
