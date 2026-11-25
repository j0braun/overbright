//! The racers' driving actions under the "Win road race" goal (vtable
//! 0x784768, name 0x7847B8; chooser slot +0x38 0x42EFE0 → 0x42D0F0):
//! JustDrive, Pass, Block, IJustGotHit, AvoidAvoidable and
//! AccidentRecovery, on the controller's road entries and its lane record
//! (+0x30C {lane, offset, rate}). See docs/formats.md, "Racer actions".
//! GetUnstuck stays in `driver::Unstuck`.

use glam::{Vec2, Vec3};

use crate::ai::route::{AiRoutes, AiTuning, Located, RacerRoute};
use crate::opponents::GameRng;

mod drag;
mod entries;
mod honk;
mod manager;
mod plans;
mod pursuit;
mod sweep;

pub use drag::{DRAG_HIT_IMPULSE, DRAG_HIT_PRIORITY, DRAG_WALL_PRIORITY, DragTouch};
pub use pursuit::{PURSUIT_DISTANCE, PURSUIT_FLOOR, PURSUIT_SIDE, PursuitSide, pursuit_offsets};

pub use entries::*;
#[allow(unused_imports)]
use honk::*;
#[allow(unused_imports)]
use plans::*;
#[allow(unused_imports)]
use sweep::*;
pub use sweep::{TRAFFIC_OWN_PATH_EDGE, human_path};

/// Pass: priority (0x42B5B0's return), time limit (the 5.0 the chooser
/// gives 0x423610), the race time before passes start (0x42B5B0: 5 s past
/// the start, 0x890118 +4), the plan's clear look (0x783670), the side gap
/// (0x429890's literal 3.0), the plan's rate (0x7E8E64).
pub const PASS_PRIORITY: i32 = 100;
pub const PASS_LIMIT: f32 = 5.0;
pub const PASS_AFTER: f32 = 5.0;
pub const CLEAR_LOOK: f32 = 3.0;
pub const PASS_SIDE: f32 = 3.0;
pub const PASS_RATE: f32 = 0.05;
/// The pass's end tests (0x42B7F0 / 0x40E4A0): 3 s (0x783688) after the
/// target was taken; done once it is 10 m (0x783684) behind bumper to
/// bumper, given up once it is more than 10 m (0x78368C) ahead and out of
/// reach.
pub const PASS_SETTLE: f32 = 3.0;
pub const PASS_CLEARED: f32 = 10.0;
pub const PASS_LOST: f32 = 10.0;
/// 0x41D2D0's side test: 5 m clear ahead, within 1 m of the line, a 50 m
/// gap, a 0.12 bend, offsets within 2 m.
const SIDE_CLEAR: f32 = 5.0;
/// Block (0x414130): the car above 0.8 of its racing-line speed, a
/// follower faster than 10 m/s and within 50 m, its time to reach the
/// car under 2 s (30 s for the human, who is blocked anyway within 35 m).
const BLOCK_PACE: f32 = 0.8;
const BLOCK_SPEED: f32 = 10.0;
const BLOCK_RANGE: f32 = 50.0;
const BLOCK_HUMAN: f32 = 35.0;
/// Block's rate (0x423A10): 0.15 (0x7E9BF4) within 10 m, else the
/// curve 0x81E7DC (built by 0x77A660 from the two pairs at 0x7E9BF0)
/// over the time to reach.
const BLOCK_RATE: f32 = 0.15;
const BLOCK_CURVE: [(f32, f32); 2] = [(2.0, 0.15), (10.0, 0.05)];
/// JustDrive (0x42B340): back to the plain line once nothing ahead is
/// reached within 0.75 s (0x78367C) and that line is clear for 3 s.
const JUST_DRIVE_CLEAR: f32 = 0.75;
/// The road-entry builder's reach (0x404910: at least 125 m, 0x7E8E60).
const REACH: f32 = 125.0;
/// The racer controller's reach time (vt+0x60 = 0x408240: 3.0 s).
const REACH_TIME: f32 = 3.0;
/// The builder's list (0x6148F0): 15 objects besides the car.
const ENTRIES: usize = 15;

/// IJustGotHit (update 0x423E20): the rate off the hitter (0x783718), the
/// clearance beyond both half widths (0x783714), the overlap along the
/// road that still counts as beside (0x78370C), the speed floor and the
/// race time it holds from (0x783710, 0x784560).
const HIT_PRIORITY: i32 = 400;
const HIT_RATE: f32 = 0.25;
const HIT_CLEAR: f32 = 5.0;
const HIT_OVERLAP: f32 = 2.8;
const HIT_FLOOR: f32 = 20.0;
const HIT_FLOOR_AFTER: f32 = 10.0;
/// The controller's speed cap (slot +0x7C, 0x5A9F60 → 0x784550).
const CAP: f32 = 1000.0;
/// AvoidAvoidable (0x42BA20 / 0x42F330 / 0x428FD0 / 0x42BA80).
const AVOID_PRIORITY: i32 = 300;
const AVOID_EXTRA: f32 = 2.0;
const AVOID_HOLD: f32 = 0.5;
const AVOID_RATE_TOP: f32 = 0.151;
/// 0x428FD0's cost of a side with no room (the slow test).
const AVOID_NO_ROOM: f32 = 99_999.0;
const AVOID_RATE_STEP: f32 = 0.05;
const AVOID_FALLBACK_RATE: f32 = 999.0;
const AVOID_SIDE: f32 = 5.0;
const AVOID_AHEAD: f32 = 3.0;
const AVOID_CLOSING: f32 = -0.25;
const AVOID_FLOOR: f32 = 20.0;
/// 0x42F330's slow test: +0x42C under 11.175682 m/s (0x783734, 25 mph).
const AVOID_SLOW: f32 = 11.175_682;
const AVOID_BEHIND: f32 = -10.0;
/// AccidentRecovery (0x42D2D7 gate, update 0x424210, done 0x4033D0): the
/// up axis's z for upright (0x78371C), the reset after 5 s (0x783720).
const RECOVER_PRIORITY: i32 = 500;
const UPRIGHT: f32 = 0.707;
const RECOVER_RESET: f32 = 5.0;
/// The controller's refusal memory (0x408280: +0x798 / +0x79C, 1 s,
/// 0x783B44).
const REFUSE_FOR: f32 = 1.0;

/// The controller's goal (+0x6FC) as far as it changes the actions: the
/// race goal ("Win road race", chooser 0x42D0F0) or AIGoalOffsetPursuit
/// (type 9, chooser 0x42DA70; target +0x18, distance +0x1C, lateral offset
/// +0x20), the encounter cars' goal before a challenge (`pursuit`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Goal {
    #[default]
    Race,
    OffsetPursuit { target: usize, distance: f32, offset: f32 },
}

/// The controller's running action (+0x6F8) and lane record.
#[derive(Clone, Debug)]
pub struct Actions {
    /// The goal the chooser belongs to.
    pub goal: Goal,
    /// Action 0x13's near state (+0x21C, 0 / 1).
    pub near: bool,
    /// The objects of the last think's road entries (controller +0x6E4),
    /// for 0x41C100 between thinks.
    pub listed: Vec<usize>,
    pub kind: Kind,
    pub priority: i32,
    /// Race clock at the action's start (+0x08) and at the pass's last
    /// target change (+0x21C).
    pub start: f32,
    pub since: f32,
    pub target: Option<usize>,
    pub plan: Plan,
    /// The car's velocity last frame, for its acceleration.
    pub last_velocity: Option<Vec3>,
    /// The lane record's stamp (+0x0C): when an avoid plan was taken
    /// (0x428FD0; none for its 999 rate).
    pub stamp: Option<f32>,
    /// AvoidAvoidable's time to the object (+0x21C) and its end
    /// (+0x22C = the first time + 2 s, from +0x228 = the start).
    pub reach: f32,
    pub reach_end: f32,
    /// AccidentRecovery's reset request made (+0x214).
    pub reset_made: bool,
    /// The controller's refused object and when (+0x798 / +0x79C).
    pub refused: Option<(usize, f32)>,
    /// The path last driven (car +0x30 → +0x10), for AvoidAvoidable.
    pub last_path: Vec<Vec3>,
    /// A reset asked for and not yet carried out (car +0x96C).
    pub reset_pending: bool,
    /// Whether this update called 40E390; +1FF persists when it did not.
    pub nitrous_query: bool,
    pub nitrous_request: bool,
}

impl Default for Actions {
    fn default() -> Actions {
        Actions {
            goal: Goal::Race,
            near: false,
            listed: Vec::new(),
            kind: Kind::JustDrive,
            priority: 0,
            start: 0.0,
            since: 0.0,
            target: None,
            plan: Plan::plain(PASS_RATE),
            last_velocity: None,
            stamp: None,
            reach: 0.0,
            reach_end: 0.0,
            reset_made: false,
            refused: None,
            last_path: Vec::new(),
            reset_pending: false,
            nitrous_query: false,
            nitrous_request: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_block_requests_between_think_updates() {
        // compare-choosers.py checks original virtual+10 implementations.
        for kind in [Kind::JustDrive, Kind::Pass, Kind::Block, Kind::GotHit, Kind::Avoid, Kind::Recover] {
            let mut action = Actions::default();
            action.kind = kind;
            assert_eq!(action.ready_between_thinks(), kind == Kind::Block);
        }
    }

    #[test]
    fn reach_in_three_seconds() {
        let car = RoadCar {
            id: 0,
            position: Vec3::ZERO,
            velocity: Vec3::X * 40.0,
            forward: Vec2::X,
            accel: 0.0,
            forward_accel: 0.0,
            half: Vec2::new(2.2, 0.9),
            racer: true,
            human: false,
            traffic: false,
            ghosted: false,
            acceleration: Vec3::ZERO,
            path: CarPath::default(),
        };
        let at = Located { s: 20.0, lateral: 0.0, room: (-5.0, 5.0), direction: Vec2::X, line_speed: 40.0 };
        let e = Entry {
            car: RoadCar { velocity: Vec3::X * 35.0, ..car },
            at,
            rel_lateral: 0.0,
            closing: -5.0,
            across: 0.9,
            along: 2.2,
        };
        // 15 m closed in 3 s at 5 m/s.
        assert!(reachable(&car, 0.0, &e, 14.0));
        assert!(!reachable(&car, 0.0, &e, 16.0));
    }

    /// 0x42B5B0 (0x42B64A–0x42B7BE): the counter is the entry count and the
    /// first entry examined is count − 1; it steps down until the counter
    /// reaches 0, so index 0 (the farthest car ahead) is examined too.
    /// 0x404910 with the racer controller's vt+0x60 = 3.0 s (0x408240):
    /// max(125 m, 3 s × car +0x430); above 41.67 m/s it grows.
    #[test]
    fn the_racers_reach_grows_with_speed() {
        assert_eq!(entries::reach(0.0), 125.0);
        assert_eq!(entries::reach(-20.0), 125.0);
        assert_eq!(entries::reach(41.0), 125.0);
        assert_eq!(entries::reach(50.0), 150.0);
    }

    #[test]
    fn find_pass_examines_every_entry_down_to_index_0() {
        assert_eq!(plans::pass_candidates(4).collect::<Vec<_>>(), [3, 2, 1, 0]);
        assert_eq!(plans::pass_candidates(1).collect::<Vec<_>>(), [0]);
        assert_eq!(plans::pass_candidates(0).count(), 0);
    }

    #[test]
    fn the_human_hitter_first() {
        let car = |id: usize, s: f32, human: bool| {
            let c = RoadCar {
                id,
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                forward: Vec2::X,
                accel: 0.0,
                forward_accel: 0.0,
                half: Vec2::new(2.2, 0.9),
                racer: true,
                human,
                traffic: false,
                ghosted: false,
                acceleration: Vec3::ZERO,
                path: CarPath::default(),
            };
            Entry {
                car: c,
                at: Located { s, lateral: 0.0, room: (-5.0, 5.0), direction: Vec2::X, line_speed: 40.0 },
                rel_lateral: 0.0,
                closing: 0.0,
                across: 0.9,
                along: 2.2,
            }
        };
        let list = [car(2, 5.0, false), car(1, 0.0, false), car(0, -3.0, true), car(3, -4.0, false)];
        // 0x402AE0: the human over a rival; among rivals, ahead over behind.
        assert_eq!(hitter(&list, 1, &[2, 0, 3]), Some(0));
        assert_eq!(hitter(&list, 1, &[3, 2]), Some(2));
        assert_eq!(hitter(&list, 1, &[]), None);
    }

    fn car(id: usize, at: Vec2, v: Vec2, traffic: bool) -> RoadCar {
        RoadCar {
            id,
            position: at.extend(0.0),
            velocity: v.extend(0.0),
            forward: v.normalize_or(Vec2::X),
            accel: 0.0,
            forward_accel: 0.0,
            half: Vec2::new(2.2, 0.9),
            racer: !traffic,
            human: false,
            traffic,
            ghosted: false,
            acceleration: Vec3::ZERO,
            path: CarPath::default(),
        }
    }

    fn straight(from: Vec2, n: usize, step: f32) -> CarPath {
        let pts: Vec<Vec3> = (1..=n).map(|k| (from + Vec2::X * step * k as f32).extend(0.0)).collect();
        CarPath::from_points(&pts)
    }

    #[test]
    fn boxes_meet_when_their_intervals_overlap() {
        // 0x4044C0: two 2.2 × 0.9 boxes 10 m apart closing at 10 m/s touch
        // after (10 − 4.4) / 10 s; already overlapping gives 0; side by side
        // 2 m apart never.
        let b = Obb { u: Vec2::X, w: Vec2::Y, half: Vec2::new(2.2, 0.9) };
        assert!((contact_time(&b, &b, Vec2::new(10.0, 0.0), Vec2::new(-10.0, 0.0)) - 0.56).abs() < 1e-4);
        assert_eq!(contact_time(&b, &b, Vec2::new(1.0, 0.0), Vec2::new(-10.0, 0.0)), 0.0);
        assert_eq!(contact_time(&b, &b, Vec2::new(10.0, 2.0), Vec2::new(-10.0, 0.0)), 99_999.0);
    }

    /// 0x42CAA0 below 10 m/s (car +0x42C): 0x425910 → 0x420A60 with the
    /// reach max(125 m, 2.0 s × car +0x430) (human vt+0x60 = 0x40A240):
    /// standing, 5 points 25 m apart along the heading (observed under
    /// Wine); at 10 m/s the road path (not ported: None).
    #[test]
    fn the_humans_path_below_10_ms_is_a_straight_line() {
        let p = human_path(Vec3::new(1.0, 2.0, 0.0), Vec3::ZERO, Vec2::Y, 0.0, 0.0).unwrap();
        assert_eq!(p.n, 5);
        for k in 0..5 {
            assert!((p.p[k] - Vec2::new(1.0, 2.0 + 25.0 * k as f32)).length() < 1e-4, "{k}: {:?}", p.p[k]);
        }
        let fast = human_path(Vec3::ZERO, Vec3::X * 9.0, Vec2::Y, 9.0, 9.0).unwrap();
        assert!((fast.p[1] - Vec2::new(25.0, 0.0)).length() < 1e-4, "along the velocity above 1 m/s");
        assert!(human_path(Vec3::ZERO, Vec3::X * 70.0, Vec2::X, 70.0, 10.0).is_none());
    }

    /// 0x4275E0: a racer whose path is empty gives no contact time (the
    /// sweep's loop runs while i < count); only traffic gets the prediction.
    #[test]
    fn an_empty_racer_path_meets_nothing() {
        let me = car(1, Vec2::ZERO, Vec2::X * 10.0, false);
        let own = straight(Vec2::ZERO, 10, 3.0);
        let mut other = car(2, Vec2::new(20.0, 0.0), Vec2::X * -10.0, false);
        assert_eq!(swept(&me, &own, &other, 3.0), None, "an empty racer path");
        other.traffic = true;
        other.racer = false;
        assert!(swept(&me, &own, &other, 3.0).is_some(), "a traffic car's prediction meets it");
    }

    #[test]
    fn a_traffic_cars_path_is_predicted_from_its_motion() {
        // 0x420B80: a point every 0.1 s, those under 1 m apart refused: a car
        // at 15 m/s gets 20 points 1.5 m apart, a standing one one point;
        // braking harder than 0.75 |v| gives 0x420A60's line of five.
        let moving = car(1, Vec2::ZERO, Vec2::X * 15.0, true);
        assert_eq!(predicted(&moving).len(), PATH_MAX);
        let standing = car(1, Vec2::ZERO, Vec2::ZERO, true);
        assert_eq!(predicted(&standing).len(), 1);
        let braking = RoadCar { acceleration: Vec3::X * -8.0, ..car(1, Vec2::ZERO, Vec2::X * 6.0, true) };
        let line = predicted(&braking);
        assert_eq!(line.len(), 5);
        assert!((line[1] - Vec2::new(25.0, 0.0)).length() < 1e-3);
    }

    #[test]
    fn the_sweep_meets_slower_traffic_ahead_and_grows_its_box() {
        // A racer at 30 m/s behind a traffic car at 10 m/s 30 m ahead in its
        // lane: met in (30 − 4.4 · 1.15) / 20 ≈ 1.25 s. The same car 2.2 m to
        // the side is met only through the 1.3 box (0.9 + 1.17 > 2.0), and
        // a car under 2 m/s keeps its own size.
        let me = car(0, Vec2::ZERO, Vec2::X * 30.0, false);
        let path = straight(Vec2::ZERO, 20, 5.0);
        let ahead = car(1, Vec2::new(30.0, 0.0), Vec2::X * 10.0, true);
        let t = swept(&me, &path, &ahead, 3.0).expect("met");
        assert!((t - (30.0 - 2.2 - 2.2 * 1.3) / 20.0).abs() < 0.05, "{t}");
        let beside = car(1, Vec2::new(30.0, 2.0), Vec2::X * 10.0, true);
        assert!(swept(&me, &path, &beside, 3.0).is_some());
        let beside_racer = car(1, Vec2::new(30.0, 2.0), Vec2::X * 10.0, false);
        assert!(
            swept(
                &me,
                &path,
                &RoadCar { path: straight(Vec2::new(30.0, 2.0) - Vec2::X * 0.0, 20, 2.0).with_y(2.0), ..beside_racer },
                3.0
            )
            .is_none()
        );
    }

    #[test]
    fn traffic_on_its_road_is_swept_along_its_drivers_path() {
        // 0x4275E0: the prediction (20 × 0.1 s) of a traffic car at 20 m/s
        // ends 40 m on, so a racer closing at 20 m/s from 55 m back meets
        // nothing within it; along its driver's path (given while it is on
        // its road, vt+0x2C ≤ −0.25) it is met in (55 − 2.2 − 2.2 · 1.3) / 20
        // ≈ 2.5 s.
        let me = car(0, Vec2::ZERO, Vec2::X * 40.0, false);
        let path = straight(Vec2::ZERO, 20, 8.0);
        let far = car(1, Vec2::new(55.0, 0.0), Vec2::X * 20.0, true);
        assert!(swept(&me, &path, &far, 3.0).is_none(), "the prediction's 2 s horizon");
        let driven = RoadCar { path: straight(Vec2::new(55.0, 0.0), 20, 8.0), ..far };
        let t = swept(&me, &path, &driven, 3.0).expect("met along its driver's path");
        assert!((t - (55.0 - 2.2 - 2.2 * 1.3) / 20.0).abs() < 0.1, "{t}");
    }
}
