//! Drag races' lanes: the AI racers' "Win drag race" goal (vtable
//! 0x7847D0) drives AIActionDriveTrafficLane (type 7, vtable 0x785820) on
//! a traffic lane of the racer route, and the player's DragCarController
//! (0x4299C0, vtable 0x785518) steers the player's car along a lane with
//! the AI driver (DragDriver 0x427F20, update 0x4220A0), the player asking
//! for lane changes (0x4102B0 → request +0x7B4, think 0x42DF10). See
//! docs/formats.md, "Race types" → Drag.

use glam::Vec3;

use crate::ai::route::{AiRoutes, RacerRoute};
use crate::opponents::GameRng;

/// The lane record's rate the lane setter gives a racer's lane (0x405410:
/// 0x7837D8), metres of offset per metre along the path (0x40FC80).
pub const AI_RATE: f32 = 0.151;
/// 0x420D40's margin to the lane room with a rate other than 999 (0x7837EC),
/// m; 0x41D530 adds the car's own half extent across (its road entry's
/// +0x08).
pub const ROOM_MARGIN: f32 = 1.0;
/// The player's lane change ends once the car is this close to the new
/// lane's centre (0x42DF10: |offset| ≤ 0x783824).
pub const LANE_HOLD: f32 = 0.5;
/// The player's lane rate by speed (table 0x81E370, built at start-up by
/// 0x77A730 from 0x7E9C68 through 0x576780): (0 mph, 0.75), (100 mph,
/// 0.25), linear between and held past the ends (0x5767A0), read at the
/// car's |velocity| (+0x430) × 2.237.
pub fn change_rate(speed: f32) -> f32 {
    let mph = speed.abs() * 2.237;
    0.75 + (0.25 - 0.75) * (mph / 100.0).clamp(0.0, 1.0)
}

/// The drag lane-change table (0x40B110): per track (0x1073 → 0x7E9728,
/// 16 entries; 0x1074 → 0x7E98A8, 18; 0x1075 → 0x7E9A58, 12), entries
/// {from lane, to lane, start, end, flag, backward} of 24 bytes. A change
/// from → to is refused while the racer's race coordinate lies in
/// [start, end], the start 80 m (0x783CCC) earlier for an AI racer (owner
/// kind car +0x14 → +4 = 2, 0x41263D..0x41264E; **observed** under Wine,
/// the player's is 1, the rivals' 2, traffic 3) when the entry's flag
/// byte is set (every entry has it), for the race's direction. (had read kind 2 as the player's
/// DragCarController and gave the player the 80 m.)
const BLOCKS: [(u32, &[(u8, u8, f32, f32, bool)]); 3] = [
    (0x1073, &[
        (2, 3, 847.0, 874.0, false), (4, 3, 847.0, 874.0, false), (4, 5, 847.0, 874.0, false), (5, 4, 847.0, 874.0, false),
        (2, 3, 988.0, 1012.0, false), (3, 2, 988.0, 1012.0, false), (3, 4, 988.0, 1012.0, false), (5, 4, 988.0, 1012.0, false),
        (2, 3, 1270.0, 1295.0, true), (4, 3, 1270.0, 1295.0, true), (4, 5, 1270.0, 1295.0, true), (5, 4, 1270.0, 1295.0, true),
        (2, 3, 1409.0, 1434.0, true), (3, 2, 1409.0, 1434.0, true), (3, 4, 1409.0, 1434.0, true), (5, 4, 1409.0, 1434.0, true),
    ]),
    (0x1074, &[
        (2, 3, 849.0, 873.0, false), (3, 2, 849.0, 873.0, false), (3, 4, 849.0, 873.0, false), (5, 4, 849.0, 873.0, false),
        (2, 3, 988.0, 1012.0, false), (3, 2, 988.0, 1012.0, false), (3, 4, 988.0, 1012.0, false), (4, 3, 988.0, 1012.0, false),
        (4, 5, 988.0, 1012.0, false), (3, 2, 1270.0, 1295.0, true), (3, 4, 1270.0, 1295.0, true), (4, 3, 1270.0, 1295.0, true),
        (4, 5, 1270.0, 1295.0, true), (5, 4, 1270.0, 1295.0, true), (2, 3, 1409.0, 1434.0, true), (4, 3, 1409.0, 1434.0, true),
        (4, 5, 1409.0, 1434.0, true), (5, 4, 1409.0, 1434.0, true),
    ]),
    (0x1075, &[
        (3, 2, 847.0, 874.0, false), (3, 4, 847.0, 874.0, false), (5, 4, 847.0, 874.0, false),
        (2, 3, 988.0, 1012.0, false), (4, 3, 988.0, 1012.0, false), (4, 5, 988.0, 1012.0, false),
        (3, 2, 1270.0, 1295.0, true), (3, 4, 1270.0, 1295.0, true), (5, 4, 1270.0, 1295.0, true),
        (2, 3, 1409.0, 1434.0, true), (4, 3, 1409.0, 1434.0, true), (4, 5, 1409.0, 1434.0, true),
    ]),
];

/// 0x40B110: whether the table lets a racer change from → to at race
/// coordinate `coord` (`ai_racer`: owner kind 2, the start 80 m early).
pub fn table_allows(track: u32, backward: bool, from: usize, to: usize, coord: f32, ai_racer: bool) -> bool {
    let Some((_, rows)) = BLOCKS.iter().find(|(t, _)| *t == track) else { return true };
    let early = if ai_racer { 80.0 } else { 0.0 };
    !rows.iter().any(|&(a, b, start, end, back)| back == backward && a as usize == from && b as usize == to && (start - early..=end).contains(&coord))
}

/// 0x412500(from, to, car): whether a racer may take lane `to` from
/// `from`. `to` must be a traffic lane (2–5; lane 0 is never asked here)
/// on the car's record (0x40AE90). With a car whose +0x24 is not 2
/// (`run_test`: the rivals' is 3; the player's 2 while racing, **observed**
/// under Wine) it also needs to run to the list's end (0x40AA90) or past
/// `from`'s by 25 m (0x40AF30). In a drag race from a lane the record has, each lane
/// stepped through on the way must exist, not stop before the one left
/// (0x40AF30 with −0.1 m) and pass the table (0x40B110) at the car's race
/// coordinate (car +0x8FC, 0x4125E9: `RacerRoute::race_coord`, the race
/// road's coordinate; **observed** under Wine, the player's +0x8FC on
/// S4_H_DRAG_15's grid 378.05, the clone's race coordinate 377.9, its
/// distance along its list −20.6; the list distance only when the caller
/// has no race coordinate).
pub fn lane_allowed(route: &RacerRoute, routes: &AiRoutes, from: usize, to: usize, run_test: bool, ai_racer: bool) -> bool {
    if !route.has_lane(routes, to) {
        return false;
    }
    let reaches = |a: usize, b: usize, d: f32| route.lane_end(routes, a) - route.lane_end(routes, b) >= d;
    if run_test && !route.lane_runs(routes, to) && !reaches(to, from, 25.0) {
        return false;
    }
    if !(2..=5).contains(&from) || !route.has_lane(routes, from) {
        return true;
    }
    let coord = route.race_coord.unwrap_or(route.distance());
    let mut at = from;
    while at != to {
        let next = if to > at { at + 1 } else { at - 1 };
        if !route.has_lane(routes, next) || !reaches(next, at, -0.1) || !table_allows(routes.track, routes.backward, at, next, coord, ai_racer) {
            return false;
        }
        at = next;
    }
    true
}

/// The player's lane request (DragCarController +0x7B4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Request {
    /// 0: towards the lower lanes (0x41D720(−4)).
    Left,
    /// 1: towards the higher lanes (0x41D720(+4)).
    Right,
    /// 2: keep to the nearest lane (0x40FDE0).
    #[default]
    Keep,
}

/// A racer's lane in a drag race: the controller's lane record (+0x30C:
/// lane, offset 0, rate) and, for the player, the DragCarController's lane
/// change state.
#[derive(Clone, Debug, Default)]
pub struct DragLane {
    /// The lane (2–5, read as 6–9 travelled backward); None until the lane
    /// action first takes the nearest one.
    pub lane: Option<usize>,
    /// The lane record's rate (+0x308 + 8).
    pub rate: f32,
    pub request: Request,
    /// The lanes a change is leaving (+0x7B8 / +0x7BC; 0xFF none).
    window: Option<(usize, usize)>,
    /// A change under way (+0x7C0).
    changing: bool,
    /// The left / right input held (+0x791 / +0x790).
    held: [bool; 2],
}

impl DragLane {
    /// An AI racer's lane (the lane action, 0x4242D0, on the "Win drag
    /// race" goal, whose lane class (slot +0x20, 0x40CAA0) is 2: traffic
    /// lanes), the path for its driver. A racer not yet on a traffic lane
    /// takes the nearest (0x420ED0: 0x40F4A0, then 0x405410 sets the lane
    /// with offset 0 and `AI_RATE`). Each frame the route update (0x428720)
    /// moves a racer whose lane its point's record lacks (0x40AE90) or
    /// that ends before the list does (0x40AA90; an AI racer, car +0x24 ≠
    /// 2) to a lane one to three away, a coin flip (0x5EA510(1) on
    /// 0x890080, the world's random numbers at +0xF8, 0x428B0B) deciding
    /// which side is tried first; a
    /// lane qualifies when `lane_allowed` (0x412500) lets it, the run test
    /// only for a racer whose own lane is gone. With none it keeps its
    /// lane (the nearest-lane fallback is only for driver kinds 1 and 3).
    /// `half_width`: the car's half extent across (the margin's own part).
    pub fn ai_path(&mut self, route: &RacerRoute, routes: &AiRoutes, position: Vec3, half_width: f32, rng: &mut GameRng) -> Vec<Vec3> {
        match self.lane {
            Some(l) if (2..=5).contains(&l) => {
                let gone = !route.has_lane(routes, l);
                if gone || !route.lane_runs(routes, l) {
                    let first_down = rng.unit(1.0) < 0.5;
                    let found = (1..=3).find_map(|k| {
                        let (down, up) = (l.checked_sub(k), Some(l + k));
                        let order = if first_down { [down, up] } else { [up, down] };
                        order.into_iter().flatten().filter(|c| (2..=5).contains(c)).find(|&c| lane_allowed(route, routes, l, c, gone, true))
                    });
                    if let Some(c) = found {
                        self.lane = Some(c);
                    }
                }
            }
            _ => {
                self.lane = route.nearest_lane(routes, position, None);
                self.rate = AI_RATE;
            }
        }
        self.lane.map_or_else(|| route.path(routes), |l| route.lane_path(routes, position, l, self.rate, ROOM_MARGIN + half_width))
    }

    /// The path on the lane record as it stands (0x420D40 → 0x41D530), built
    /// from where the car is now: what AvoidAvoidable drives when 0x42BF00
    /// finds no lane to move to (0x42BFA9). None without a traffic lane.
    pub fn record_path(&self, route: &RacerRoute, routes: &AiRoutes, position: Vec3, half_width: f32) -> Option<Vec<Vec3>> {
        let l = self.lane.filter(|l| (2..=5).contains(l))?;
        Some(route.lane_path(routes, position, l, self.rate, ROOM_MARGIN + half_width))
    }

    /// The player's steering input as the lane events (0x4102B0: 0xB8 /
    /// 0xBA left, 0xB9 / 0xBB right, each acted on when it changes):
    /// pressing asks for that side (unless already asked) and clears the
    /// window; letting go asks to keep the nearest lane.
    pub fn input(&mut self, left: bool, right: bool) {
        for (side, down) in [(0, left), (1, right)] {
            if self.held[side] == down {
                continue;
            }
            self.held[side] = down;
            let want = if side == 0 { Request::Left } else { Request::Right };
            if down {
                if self.request != want {
                    self.request = want;
                    self.window = None;
                    self.changing = false;
                }
            } else {
                self.request = Request::Keep;
                self.changing = false;
            }
        }
    }

    /// The DragCarController's think (0x42DF10), the player's lane path.
    /// With a window set, once the car's nearest lane (0x40F4A0) is outside
    /// it and the car within `LANE_HOLD` of that lane's centre (0x419BC0)
    /// the change is over. Then the request: Left / Right shift the lane
    /// (0x41D720(∓4)); when that moves it and no change is under way, the
    /// window becomes (the car's lane, 5) for Left or (2, the car's lane)
    /// for Right and a change is under way. Keep (0x40FDE0) takes the
    /// nearest lane outside the window, or with none clears it and takes
    /// the nearest. The lane's rate is `change_rate` at the car's speed.
    pub fn player_path(&mut self, route: &RacerRoute, routes: &AiRoutes, position: Vec3, speed: f32, half_width: f32) -> Vec<Vec3> {
        if let Some((lo, hi)) = self.window
            && let Some(at) = route.nearest_lane(routes, position, None)
            && (at < lo || at > hi)
            && route.lane_lateral(routes, position, at).abs() <= LANE_HOLD
        {
            self.window = None;
            self.changing = false;
        }
        match self.request {
            Request::Left | Request::Right => {
                let left = self.request == Request::Left;
                if self.shift(route, routes, position, speed, if left { -4 } else { 4 })
                    && !self.changing
                    && let Some(at) = route.nearest_lane(routes, position, None)
                {
                    self.window = Some(if left { (at, 5) } else { (2, at) });
                    self.changing = true;
                }
            }
            Request::Keep => {
                let lane = route.nearest_lane(routes, position, self.window).or_else(|| {
                    self.window = None;
                    self.changing = false;
                    route.nearest_lane(routes, position, None)
                });
                if lane.is_some() {
                    self.lane = lane;
                }
                self.rate = change_rate(speed);
            }
        }
        self.lane.map_or_else(|| route.path(routes), |l| route.lane_path(routes, position, l, self.rate, ROOM_MARGIN + half_width))
    }

    /// 0x41D720(n): from the car's nearest lane, step |n| lanes towards
    /// lower (n < 0) or higher lanes while `lane_allowed` (0x412500(lane,
    /// lane ± 1, car), with the player's car (0x41D80C / 0x41D894: [esi+4])
    /// lets the next one. The player's car +0x24 is 2 while racing
    /// (**observed** under Wine, 1 at the green, 2 at frame 600), which
    /// skips the run test; its owner kind (car +0x14 → +4) is 1, so the
    /// table's rows start on time (no 80 m).
    /// The lane record becomes (that lane, 0, `change_rate`). Whether the
    /// lane moved off the car's.
    fn shift(&mut self, route: &RacerRoute, routes: &AiRoutes, position: Vec3, speed: f32, n: i32) -> bool {
        let Some(from) = route.nearest_lane(routes, position, None) else { return false };
        let mut to = from;
        for _ in 0..n.unsigned_abs() {
            let next = if n < 0 { to.checked_sub(1) } else { Some(to + 1) };
            match next.filter(|&c| (2..=5).contains(&c) && lane_allowed(route, routes, to, c, false, false)) {
                Some(c) => to = c,
                None => break,
            }
        }
        self.lane = Some(to);
        self.rate = change_rate(speed);
        to != from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_rate_by_speed() {
        assert_eq!(change_rate(0.0), 0.75);
        assert!((change_rate(50.0 / 2.237) - 0.5).abs() < 1e-5);
        assert_eq!(change_rate(80.0), 0.25);
    }

    #[test]
    fn drag_table_blocks_changes() {
        // 4211 forward: 2 → 3 refused for an AI racer (owner kind 2) from
        // 80 m before 847 to 874.
        assert!(table_allows(0x1073, false, 2, 3, 760.0, true));
        assert!(!table_allows(0x1073, false, 2, 3, 767.0, true));
        assert!(!table_allows(0x1073, false, 2, 3, 874.0, true));
        assert!(table_allows(0x1073, false, 2, 3, 875.0, true));
        // The player (owner kind 1): from the row's start only.
        assert!(table_allows(0x1073, false, 2, 3, 846.0, false));
        assert!(!table_allows(0x1073, false, 2, 3, 847.0, false));
        // Not listed: 3 → 2 there; the backward rows; other tracks.
        assert!(table_allows(0x1073, false, 3, 2, 850.0, true));
        assert!(table_allows(0x1073, true, 2, 3, 850.0, true));
        assert!(table_allows(4000, false, 2, 3, 850.0, true));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// The lane table (0x40B110 via 0x412500) is read at the car's race
    /// coordinate (car +0x8FC), not its distance along its list. On
    /// S4_H_DRAG_15 (4213 backward) the player's grid is at race coordinate
    /// 378 (observed under Wine) and list distance about −20: a 3 → 2 change
    /// is refused from 1270 to 1295 of race coordinate for the player, from
    /// 1190 (80 m early) for an AI racer (owner kind 2).
    fn lane_table_reads_the_race_coordinate() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RB/Routes4213B.bin")).expect("reading the game's data file");
        let routes = AiRoutes { file: crate::formats::ai_routes::RouteFile::parse(&data), track: 0x1075, backward: true, race_coords: false, resets: Vec::new(), blocks: Vec::new() };
        let heading = glam::Vec2::new(0.927, 0.374);
        let mut route = RacerRoute::start(&routes, Vec3::new(-281.3, -2121.1, -1.8), heading).expect("on the drag road");
        assert!(route.has_lane(&routes, 2) && route.has_lane(&routes, 3));
        assert!(route.distance() < 100.0);
        route.race_coord = Some(378.0);
        assert!(lane_allowed(&route, &routes, 3, 2, false, true));
        assert!(lane_allowed(&route, &routes, 3, 2, false, false));
        route.race_coord = Some(1230.0);
        assert!(!lane_allowed(&route, &routes, 3, 2, false, true));
        assert!(lane_allowed(&route, &routes, 3, 2, false, false), "the player has no 80 m");
        assert!(lane_allowed(&route, &routes, 2, 3, false, true));
        route.race_coord = Some(1280.0);
        assert!(!lane_allowed(&route, &routes, 3, 2, false, false));
        route.race_coord = Some(1296.0);
        assert!(lane_allowed(&route, &routes, 3, 2, false, true));
    }

    /// The lane room a FuturePoint gives 0x40FC80 (+0x1C.. / +0x30..,
    /// filled by 0x41FD50 from 0x40B840 / 0x40B780 on a backward walk):
    /// from the lane's position to the road's edges. Under Wine (stage-4
    /// drag on 4213 backward, the RX7's walk, record 270 point 6, its lane
    /// adjustments 0) the rooms of lanes 2–5 read −6.466 / 24.542, −12.449 /
    /// 18.559, −17.943 / 13.065, −24.048 / 6.960.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn lane_room_matches_the_originals_walk_points() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RB/Routes4213B.bin")).expect("reading the game's data file");
        let f = crate::formats::ai_routes::RouteFile::parse(&data);
        let r = f.record(270).expect("record 270");
        let native = [(-6.46555, 24.5423), (-12.4485, 18.5593), (-17.943, 13.0648), (-24.0481, 6.95975)];
        for (l, (lo, hi)) in (2..=5).zip(native) {
            let (a, b) = crate::ai::route::lane_room(&f, r, 6, true, l);
            assert!((a - lo).abs() < 0.02 && (b - hi).abs() < 0.02, "lane {l}: {a} / {b} against {lo} / {hi}");
        }
    }

    /// AvoidAvoidable with no lane to move to drives the lane record's path
    /// rebuilt from where the car is (0x42F330 → 0x42BF00 → 0x42BFA9 →
    /// 0x420D40, every update); the clone had kept the path driven before,
    /// so a rival that had just moved its record to a new lane went on along
    /// the old one into its end (stage-4 drag replays, docs/drag-stage4-parity.md 8.6).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn avoid_without_a_lane_drives_the_records_path_from_here() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RB/Routes4213B.bin")).expect("reading the game's data file");
        let routes = AiRoutes { file: crate::formats::ai_routes::RouteFile::parse(&data), track: 0x1075, backward: true, race_coords: false, resets: Vec::new(), blocks: Vec::new() };
        let heading = glam::Vec2::new(0.927, 0.374);
        let here = Vec3::new(-281.3, -2121.1, -1.8);
        let route = RacerRoute::start(&routes, here, heading).expect("on the drag road");
        let mut lane = DragLane::default();
        assert!(lane.record_path(&route, &routes, here, 0.9).is_none(), "no record yet");
        lane.lane = Some(3);
        lane.rate = AI_RATE;
        let path = lane.record_path(&route, &routes, here, 0.9).unwrap();
        assert_eq!(path, route.lane_path(&routes, here, 3, AI_RATE, ROOM_MARGIN + 0.9));
        // From another place, another path: nothing of the earlier one kept.
        let moved = here + Vec3::new(0.0, 3.0, 0.0);
        assert_ne!(lane.record_path(&route, &routes, moved, 0.9).unwrap(), path);
    }

    /// The drag racers' walk against the original's (a stage-4 drag on 4213
    /// backward, three rivals' walk points read under Wine at frames 900 …
    /// 1600, docs/evidence/native-ai/walk-adjust-native.tsv). In a drag race
    /// 0x422220 joins two records by dropping the list's last point when the
    /// next record's first lies under 3 m on (record 278's last point gives way
    /// to record 277's first), and 0x41A180 bends lanes 2–5 as it bends the
    /// racing line. Each native point (record, point, +0x58) is in the clone's
    /// walk started at the rival's pose, but for those behind the start, and the
    /// lane adjustments of the 8 m points match the original's where both
    /// walks had the point before (the start's first points excepted).
    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn drag_walks_join_records_and_bend_lanes_as_the_original() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RB/Routes4213B.bin")).expect("reading the game's data file");
        let routes = AiRoutes { file: crate::formats::ai_routes::RouteFile::parse(&data), track: 0x1075, backward: true, race_coords: false, resets: Vec::new(), blocks: Vec::new() };
        let tsv = crate::testkit::private_str("docs/evidence/native-ai/walk-adjust-native.tsv");
        let (mut key, mut list) = (String::new(), Vec::new());
        let (mut found, mut total) = (0, 0);
        let mut checked = Vec::new();
        for line in tsv.lines().skip(1) {
            let v: Vec<&str> = line.split('\t').collect();
            let num = |i: usize| v[i].parse::<f32>().unwrap();
            if format!("{}-{}", v[0], v[1]) != key {
                key = format!("{}-{}", v[0], v[1]);
                let (pos, fwd) = (Vec3::new(num(2), num(3), num(4)), glam::Vec2::new(num(5), num(6)));
                let mut route = RacerRoute::start_in(&routes, pos, fwd, true).expect("on the drag road");
                for _ in 0..3 {
                    route.update(&routes, pos, num(7), fwd * num(7), 0, false, &|_| 1.0);
                }
                list = route.lane_adjusts(&routes.file);
            }
            let (rec, cp, o): (u16, usize, f32) = (v[9].parse().unwrap(), v[10].parse().unwrap(), num(12));
            total += 1;
            if let Some(p) = list.iter().find(|p| p.0 == rec && p.1 == cp && (p.2 - o).abs() < 0.05) {
                found += 1;
                checked.push((v[0].to_string(), rec, cp, o, p.4, [num(15), num(16), num(17), num(18)]));
            }
        }
        assert!(found * 100 >= total * 90, "{found} of {total} native walk points in the clone's walks");
        // Record 278's 8 m point bends lane 2 towards record 277's (−3.50 natively).
        let r278 = checked.iter().find(|c| c.1 == 278 && c.2 == 1 && c.3 == 8.0).expect("record 278 point 1 + 8 m");
        assert!((r278.4[0] - r278.5[0]).abs() < 0.15, "{r278:?}");
        let r268 = checked.iter().find(|c| c.0 == "1500" && c.1 == 268 && c.2 == 1 && c.3 == 8.0).expect("record 268 point 1 + 8 m");
        assert!((0..4).all(|i| (r268.4[i] - r268.5[i]).abs() < 0.15), "{r268:?}");
    }

    #[test]
    fn lane_events_change_on_edges() {
        let mut l = DragLane::default();
        l.input(true, false);
        assert_eq!(l.request, Request::Left);
        l.window = Some((3, 5));
        // Held: nothing changes.
        l.input(true, false);
        assert_eq!(l.window, Some((3, 5)));
        l.input(false, false);
        assert_eq!(l.request, Request::Keep);
        l.input(false, true);
        assert_eq!(l.request, Request::Right);
        assert_eq!(l.window, None);
    }
}
