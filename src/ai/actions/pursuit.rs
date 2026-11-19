//! The encounter cars' pursuit of the player before a challenge (
//! docs/original/outrun.md "Encounter pursuit"): the manager's side machine 0x42A860
//! (run per slot every update in state 3, TRYING_TO_ENGAGE), the goal
//! AIGoalOffsetPursuit (type 9, vtable 0x784BF8, set by 0x428630, chooser
//! slot +0x38 = 0x42DA70) and its default action "Drive at offset" (type
//! 0x13, priority 0, no end: vtable 0x785A18 built by the goal's slot +0x3C
//! 0x426F20, update 0x42BC50). **Verified** (REA / dis) and **observed**
//! under Wine (scenarios encounter_pass / encounter_follow, encpoll.py).

use super::*;

/// The goal's following distance (0x42A860's 10.0 → goal +0x1C → action
/// +0x214): the point the car keeps is this far ahead of its target.
pub const PURSUIT_DISTANCE: f32 = 10.0;
/// 0x42A860's side offsets (goal +0x20 → action +0x218), m.
pub const PURSUIT_SIDE: f32 = 5.0;
/// Action 0x13's near state (+0x21C): entered within 30 m (0x7835B4) of
/// the point, left beyond 50 m (0x7835B8).
const NEAR_ENTER: f32 = 30.0;
const NEAR_LEAVE: f32 = 50.0;
/// Its lane record's rate (0x783760) and the speed band: within 2 m
/// (0x78376C) the target's own speed, else ±2 m/s (0x783770).
const PURSUIT_RATE: f32 = 0.15;
const SPEED_BAND: f32 = 2.0;
const SPEED_STEP: f32 = 2.0;
/// The speed floor for an AI car (owner class 2, [car +0x14] +4 == 2):
/// 60 mph (0x7835C4 = 26.821636).
pub const PURSUIT_FLOOR: f32 = 26.821_636;

/// One encounter slot's side state (manager +0x1B14 + 0x10·k; 1 from the
/// placement 0x42A5E0 → 0x42A840): 1 behind or ahead (offset 0), 2 on the
/// −5 side, 3 on the +5 side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PursuitSide {
    pub state: u8,
}

impl Default for PursuitSide {
    fn default() -> PursuitSide {
        PursuitSide { state: 1 }
    }
}

impl PursuitSide {
    /// 0x42A860 for one slot: `along` and `across` are 0x41C100's two
    /// offsets of the player (the car's distance along the road ahead of the
    /// player, and its offset off the line less the player's). In state 1
    /// it stays while |along| ≥ 30 m, else takes the side `across` is on
    /// (≥ 0: state 3, +5; else state 2, −5); in another state |along| ≥ 20
    /// m goes back to 1 (offset 0), else state 3 holds +5 until across <
    /// −2 (→ 2), and any other state holds −5 until across > 2 (→ 3; a
    /// state not 1 or 3 keeps its number there). Returns the offset handed
    /// to 0x428630.
    pub fn step(&mut self, along: f32, across: f32) -> f32 {
        if self.state == 1 {
            if along.abs() >= 30.0 {
                return 0.0;
            }
        } else if along.abs() >= 20.0 {
            self.state = 1;
            return 0.0;
        }
        match self.state {
            1 => {
                if across >= 0.0 {
                    self.state = 3;
                    PURSUIT_SIDE
                } else {
                    self.state = 2;
                    -PURSUIT_SIDE
                }
            }
            3 => {
                if across < -2.0 {
                    self.state = 2;
                    -PURSUIT_SIDE
                } else {
                    PURSUIT_SIDE
                }
            }
            _ => {
                if across <= 2.0 {
                    -PURSUIT_SIDE
                } else {
                    self.state = 3;
                    PURSUIT_SIDE
                }
            }
        }
    }
}

/// 0x41C100 (ECX = the car's controller, the target the player): the car's
/// distance along its list ahead of the target and its offset off the line
/// less the target's. With the target in the controller's road entries
/// (`listed`, the last think's list +0x6E4): their s and offsets
/// (0x60C000 / 0x60C020, both filed by 0x419B20, `locate_object`). Else
/// from the list itself (+0x6E8: 0x419BC0 → 0x419B20 for both) when both s
/// lie within the first and last points' (0x401090). Else 0x5D5D60's
/// free-roam distance and 0.
pub fn pursuit_offsets(route: &RacerRoute, routes: &AiRoutes, me: &RoadCar, target: &RoadCar, listed: bool) -> (f32, f32) {
    if let (Some(a), Some(b)) = (route.locate_object(routes, me.position), route.locate_object(routes, target.position)) {
        let within = |x: f32| route.list_span().is_some_and(|(lo, hi)| x >= lo && x <= hi);
        if listed || (within(a.s) && within(b.s)) {
            return (a.s - b.s, a.lateral - b.lateral);
        }
    }
    (free_roam_along(me, target), 0.0)
}

/// 0x5D5D60 with no race coordinates (0x88F298 +0x26 clear): the 2D
/// distance from the target, negative when the car is behind it along the
/// target's way (+0x70 above 1 m/s, else +0x90).
pub(super) fn free_roam_along(me: &RoadCar, target: &RoadCar) -> f32 {
    let d = (me.position - target.position).truncate();
    let way = crate::ai::route::travel(target.velocity.length(), target.velocity, target.forward.extend(0.0));
    let dist = d.length();
    if d.dot(way) < 0.0 { -dist } else { dist }
}

impl Actions {
    /// 0x428630 (ECX = the controller): the goal becomes AIGoalOffsetPursuit
    /// on `target` with the distance and the lateral offset. A new goal
    /// (the old one not type 9) deletes the running action (0x402070: the
    /// next think chooses afresh); the same goal takes the new values, and
    /// a running action 0x13 too (+0x210 / +0x214 / +0x218).
    pub fn set_pursuit(&mut self, target: usize, distance: f32, offset: f32) {
        if !matches!(self.goal, Goal::OffsetPursuit { .. }) {
            self.kind = Kind::JustDrive;
            self.priority = -1;
            self.target = None;
            self.near = false;
        }
        self.goal = Goal::OffsetPursuit { target, distance, offset };
    }

    /// 0x42DA70, AIGoalOffsetPursuit's chooser: GetUnstuck (outside, as
    /// for the race goal), IJustGotHit, AvoidAvoidable, AccidentRecovery —
    /// the race chooser's tests — then the goal's default action 0x13 at
    /// priority 0 (no Block, no Pass).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn choose_pursuit(
        &mut self,
        scene: &Scene,
        rng: &mut GameRng,
        list: &[Entry],
        own: usize,
        ty: i32,
        pri: i32,
        target: usize,
    ) -> (Kind, i32, Option<usize>, Option<Plan>) {
        if ty != 0xB
            && pri <= HIT_PRIORITY
            && !scene.held
            && let Some(id) = hitter(list, own, scene.hits)
        {
            // 0x42DB59: 0x40D5C0 before the constructor.
            racer_honk(rng, list, own, id);
            return (Kind::GotHit, HIT_PRIORITY, Some(id), None);
        }
        if ty != 1
            && pri <= AVOID_PRIORITY
            && let Some((id, t)) = sweep(&self.last_path, list, own, &scene.me, CLEAR_LOOK)
            // 0x42DBD8: 0x40D5C0 before vt+0x9C.
            && { racer_honk(rng, list, own, id); true }
            && self.willing(id, scene.clock, scene.odds, rng)
        {
            self.reach = t;
            self.reach_end = t + AVOID_EXTRA;
            return (Kind::Avoid, AVOID_PRIORITY, Some(id), None);
        }
        if ty != 0xD && pri <= RECOVER_PRIORITY && scene.wheels_down < 2 && scene.up_z <= UPRIGHT {
            return (Kind::Recover, RECOVER_PRIORITY, None, None);
        }
        (Kind::OffsetPursuit, 0, Some(target), None)
    }

    /// 0x42BC50, "Drive at offset": with the target's road entry
    /// (0x60C100), d = −(the target's s less the car's + the distance);
    /// the near state (+0x21C) is entered when |d| < 30 m and left when
    /// |d| > 50 m or the target has no entry. Near: the lane record becomes
    /// {0, the target's offset off the line + the goal's offset, 0.15}
    /// (0x4053B0), the path is [`RacerRoute::offset_path`], and the speed
    /// (+0x1F0, +0x1FC = 0) is the target's speed (+0x430) within |d| < 2
    /// m, else 2 m/s less (d ≥ 0: the car is more than the distance ahead)
    /// or more, at least 0 and for an AI car at least 60 mph. Not near:
    /// JustDrive's path (0x42B340) and the controller's own speed (+0x1FC
    /// = 1). Pedals on (+0x1FD).
    pub(super) fn offset_pursuit(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        fresh: bool,
    ) -> Steer {
        let Goal::OffsetPursuit { target, distance, offset } = self.goal else {
            let path = self.just_drive(route, scene, list, own, fresh);
            return Steer { path, cap: None, pedals: true, reset: false };
        };
        let entry = list.iter().find(|e| e.car.id == target).copied();
        let d = entry.map(|e| -(gap(list, own, &e) + distance));
        self.near = near_state(self.near, d);
        let (Some(e), Some(d), true) = (entry, d, self.near) else {
            let path = self.just_drive(route, scene, list, own, fresh);
            return Steer { path, cap: None, pedals: true, reset: false };
        };
        self.plan = Plan { lane: 0, offset: e.at.lateral + offset, rate: PURSUIT_RATE };
        let me = &scene.me;
        let path = route.offset_path(
            scene.routes,
            self.plan.rate,
            me.position,
            me.forward.extend(0.0),
            me.velocity.length(),
            scene.drift,
            self.plan.offset,
        );
        Steer { path, cap: Some(pursuit_speed(d, e.car.velocity.length())), pedals: true, reset: false }
    }
}

/// Action 0x13's near state (+0x21C, 0x42BC94..0x42BCCF): entered when
/// |d| < 30 m, kept while |d| ≤ 50 m, left without the target's entry.
pub(super) fn near_state(near: bool, d: Option<f32>) -> bool {
    match d {
        None => false,
        Some(d) if !near => d.abs() < NEAR_ENTER,
        Some(d) => d.abs() <= NEAR_LEAVE,
    }
}

/// Action 0x13's speed when near (+0x1F0, 0x42BDEB..0x42BE9E): the
/// target's speed within |d| < 2 m, else 2 m/s less (d ≥ 0) or more; at
/// least 0, and for an AI car (owner class 2; every user of `Actions`) at
/// least 60 mph — a floor, not a cap (observed under Wine: 28.4–46.6 m/s
/// with the player at 26–45 m/s, encounter_pass).
pub(super) fn pursuit_speed(d: f32, target_speed: f32) -> f32 {
    let speed = if d.abs() < SPEED_BAND {
        target_speed
    } else if d >= 0.0 {
        target_speed - SPEED_STEP
    } else {
        target_speed + SPEED_STEP
    };
    speed.max(0.0).max(PURSUIT_FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 0x42A860: from the placement's state 1 the side is taken within 30
    // m, kept with the ±2 m hysteresis, and dropped (offset 0) beyond 20 m.
    #[test]
    fn the_pursuit_side_machine_has_the_native_thresholds() {
        let mut s = PursuitSide::default();
        assert_eq!(s.step(35.0, 1.0), 0.0);
        assert_eq!(s.state, 1);
        assert_eq!(s.step(29.0, -0.1), -5.0);
        assert_eq!(s.state, 2);
        assert_eq!(s.step(10.0, 1.9), -5.0, "within the 2 m band the side holds");
        assert_eq!(s.step(10.0, 2.1), 5.0);
        assert_eq!(s.state, 3);
        assert_eq!(s.step(10.0, -1.9), 5.0);
        assert_eq!(s.step(10.0, -2.1), -5.0);
        assert_eq!(s.step(-20.0, 0.0), 0.0, "|along| ≥ 20 m outside state 1: back to 1");
        assert_eq!(s.state, 1);
        assert_eq!(s.step(25.0, 0.0), 5.0, "state 1 takes a side below 30 m, across ≥ 0 is +5");
        // A state that is neither 1 nor 3 (0) acts as 2 without being set to 2.
        let mut z = PursuitSide { state: 0 };
        assert_eq!(z.step(5.0, 0.0), -5.0);
        assert_eq!(z.state, 0);
    }

    fn car(id: usize, at: Vec2, v: Vec2) -> RoadCar {
        RoadCar {
            id,
            position: at.extend(0.0),
            velocity: v.extend(0.0),
            forward: v.normalize_or(Vec2::X),
            accel: 0.0,
            forward_accel: 0.0,
            half: Vec2::new(2.2, 0.9),
            racer: true,
            human: id == 0,
            traffic: false,
            ghosted: false,
            acceleration: Vec3::ZERO,
            path: CarPath::default(),
        }
    }

    // 0x5D5D60 with no race coordinates: the 2D distance, signed by the
    // target's way (velocity above 1 m/s, else forward).
    #[test]
    fn off_the_list_the_along_offset_is_the_signed_distance() {
        let target = car(0, Vec2::ZERO, Vec2::new(0.0, 20.0));
        assert_eq!(free_roam_along(&car(1, Vec2::new(3.0, 4.0), Vec2::ZERO), &target), 5.0);
        assert_eq!(free_roam_along(&car(1, Vec2::new(3.0, -4.0), Vec2::ZERO), &target), -5.0);
        // At rest (0.5 m/s) the target's forward axis decides.
        let slow = RoadCar { forward: Vec2::new(0.0, -1.0), ..car(0, Vec2::ZERO, Vec2::new(0.0, 0.5)) };
        assert_eq!(free_roam_along(&car(1, Vec2::new(3.0, 4.0), Vec2::ZERO), &slow), -5.0);
    }

    // 0x42BC50: near within 30 m of the point the distance ahead of the
    // target, kept to 50 m; the target's speed ±2 m/s, at least 60 mph.
    #[test]
    fn drive_at_offset_keeps_the_target_speed_near_the_point() {
        assert!(near_state(false, Some(29.9)));
        assert!(!near_state(false, Some(30.0)));
        assert!(near_state(true, Some(-50.0)));
        assert!(!near_state(true, Some(50.1)));
        assert!(!near_state(true, None), "no entry for the target: not near");
        assert_eq!(pursuit_speed(1.9, 35.0), 35.0);
        assert_eq!(pursuit_speed(2.0, 35.0), 33.0, "d ≥ 0: the car is further ahead, slower");
        assert_eq!(pursuit_speed(-2.0, 35.0), 37.0);
        assert_eq!(pursuit_speed(-2.0, 10.0), PURSUIT_FLOOR, "60 mph is the floor");
    }

    // 0x428630: a new type-9 goal drops the running action; the same goal
    // only takes the new values.
    #[test]
    fn a_new_pursuit_goal_drops_the_action_and_a_retarget_keeps_it() {
        let mut a = Actions { kind: Kind::Avoid, priority: AVOID_PRIORITY, ..Actions::default() };
        a.set_pursuit(0, PURSUIT_DISTANCE, 0.0);
        assert_eq!((a.kind, a.priority), (Kind::JustDrive, -1));
        a.kind = Kind::OffsetPursuit;
        a.priority = 0;
        a.near = true;
        a.set_pursuit(0, PURSUIT_DISTANCE, -5.0);
        assert_eq!((a.kind, a.priority, a.near), (Kind::OffsetPursuit, 0, true));
        assert_eq!(a.goal, Goal::OffsetPursuit { target: 0, distance: 10.0, offset: -5.0 });
    }
}
