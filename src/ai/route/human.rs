//! The human controller's road node and walk (controller vtable
//! 0x785230: +0x0C the node, +0x10 the way flag, +0x6C / +0x70 the walk).
//! Each world update 0x42CAA0 checks the way (0x40A390), then 0x42A190
//! keeps the node from the walk or searches it again (vt+0x30 = 0x411490 →
//! 0x40F180) and rebuilds the walk from it (0x428030, reach max(125 m, 2 s
//! × |v|), vt+0x60 = 0x40A240 = 2.0). **Verified** (REA / dis; the search
//! model observed under Wine, 33 of 34 placements exact).

use glam::{Vec2, Vec3};

use super::{AiRoutes, RacerRoute};
use crate::formats::ai_routes::RouteFile;

/// Control point flag bit 27: the human controller's road points (0x5E2E50
/// mask 0x400).
const HUMAN_POINT: u32 = 0x0800_0000;
/// 0x411490's re-pick radius in the node's record (0x42480000).
const KEEP_RADIUS: f32 = 50.0;
/// 0x40CC40 / 0x40CBC0: the re-picked point is dropped when the car is
/// more than 5 m outside its road (0x40A00000).
const KEEP_OUTSIDE: f32 = 5.0;
/// 0x40F180's radius (flag 0x800) and height window (flag 0x2000,
/// 0x78378C).
const SEARCH_RADIUS: f32 = 100.0;
const SEARCH_DZ: f32 = 4.0;
/// The walk's reach (0x7E8E60) and time (0x40A240).
const REACH: f32 = 125.0;
const REACH_TIME: f32 = 2.0;
/// 0x40A390: the car's way is its velocity above 10 mph (0x78426C ×
/// +0x430 against 0x784560), else its forward axis; the node's way turns
/// when they meet below −0.707 (0x7846E8).
const WAY_MPH: f32 = 10.0;
const WAY_TURN: f32 = -0.707;

/// What one update did, for the entry counters (`n_node_update` 0x411490,
/// `n_node_search` 0x40F180).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HumanStep {
    pub node_updates: u32,
    pub node_searches: u32,
}

/// The human controller's road state.
#[derive(Clone, Debug, Default)]
pub struct HumanRoad {
    /// +0x0C: the node (record, control point).
    pub node: Option<(usize, usize)>,
    /// +0x10 = 0: the walk runs against the record (1 from the constructor
    /// 0x40F630).
    pub backward: bool,
    pub walk: Option<RacerRoute>,
}

/// How far `position` lies outside the road of control point `cp` of
/// `record` (the observed model of 0x40CC40 / 0x40CBC0 and 0x40F180's 0x40 test):
/// across, L = (car − point) · (ty, −tx), L − the right half width (+0x16)
/// for L > 0, else −(L + the left one, +0x14); along, d = (car − point) ·
/// t, d − the segment length (+0x10) for d > 0, else −(d + the previous
/// point's length). Negative inside.
pub fn outside(f: &RouteFile, record: usize, cp: usize, position: Vec3) -> (f32, f32) {
    let r = &f.records[record];
    let c = &r.points[cp];
    let t = c.tangent;
    let rel = position.truncate() - c.position.truncate();
    let l = rel.dot(Vec2::new(t.y, -t.x));
    let across = if l > 0.0 { l - c.half_width[1] } else { -(l + c.half_width[0]) };
    let d = rel.dot(t);
    let prev = if cp > 0 { r.points[cp - 1].length } else { 0.0 };
    let along = if d > 0.0 { d - c.length } else { -(d + prev) };
    (across, along)
}

/// 0x5E2E50 for mask 0x400 in one record: the nearest bit-27 point (3D)
/// within `radius`; the scan skips −2 − trunc(−0.02 × d) points when that
/// is positive (0x7852AC: only from 150 m away).
fn nearest_in(f: &RouteFile, record: usize, position: Vec3, radius: f32) -> Option<(usize, f32)> {
    let pts = &f.records[record].points;
    let (mut best, mut k) = (None::<(usize, f32)>, 0usize);
    while k < pts.len() {
        let d = pts[k].position.distance(position);
        if pts[k].flags & HUMAN_POINT != 0 && d <= radius && best.is_none_or(|b| d < b.1) {
            best = Some((k, d));
        }
        let skip = -2 - (d * -0.02) as i32;
        k += 1 + skip.max(0) as usize;
    }
    best
}

/// 0x411490 (the human controller's vt+0x30): with a node, the nearest
/// road point of the node's own record within 50 m, dropped when the car is
/// more than 5 m outside its road; without one (or dropped), 0x40F180: over
/// the records with a traffic lane (+0x84..+0x8B, flag 0x80) the nearest
/// point within 100 m with |dz| < 4 m and the car inside its road, the
/// least 3D distance winning; failing that a second 0x40F180 without those
/// tests (any record, any distance). Returns the node and the 0x40F180
/// calls made.
pub fn search(f: &RouteFile, position: Vec3, node: Option<(usize, usize)>) -> (Option<(usize, usize)>, u32) {
    if let Some((record, _)) = node
        && let Some((cp, _)) = nearest_in(f, record, position, KEEP_RADIUS)
    {
        let (across, along) = outside(f, record, cp, position);
        if across <= KEEP_OUTSIDE && along <= KEEP_OUTSIDE {
            return (Some((record, cp)), 0);
        }
    }
    let mut best: Option<(f32, usize, usize)> = None;
    for (ri, r) in f.records.iter().enumerate().filter(|(_, r)| r.lanes[2..].iter().any(|&l| l)) {
        let Some((cp, d)) = nearest_in(f, ri, position, SEARCH_RADIUS) else { continue };
        if (r.points[cp].position.z - position.z).abs() >= SEARCH_DZ {
            continue;
        }
        let (across, along) = outside(f, ri, cp, position);
        if across < 0.0 && along < 0.0 && best.is_none_or(|b| d < b.0) {
            best = Some((d, ri, cp));
        }
    }
    if let Some((_, r, c)) = best {
        return (Some((r, c)), 1);
    }
    let any = (0..f.records.len())
        .filter_map(|ri| nearest_in(f, ri, position, f32::MAX).map(|(cp, d)| (d, ri, cp)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    (any.map(|(_, r, c)| (r, c)), 2)
}

impl HumanRoad {
    /// 0x40A390 (from 0x42CAA0): whether the node's way turns this update
    /// — its tangent (reversed when backward) against the car's way below
    /// a dot of −0.707. `speed` is car +0x430.
    pub fn way_turns(&self, f: &RouteFile, velocity: Vec3, forward: Vec3, speed: f32) -> bool {
        let Some((record, cp)) = self.node else { return false };
        let t = f.records[record].points[cp].tangent;
        let way = if self.backward { -t } else { t };
        let car = if speed * 2.237 > WAY_MPH { velocity.truncate().normalize_or_zero() } else { forward.truncate() };
        way.dot(car) < WAY_TURN
    }

    /// One world update of 0x42CAA0's road part: the way check (a turn
    /// clears the walk), then 0x42A190: the walk keeps the car when it has
    /// points, the car's distance along it (`locate_object`, 0x419B20) lies
    /// from 50 m before its first point up to its last, and the car is at
    /// most `half_width` (half the body's width, (+0xF4 − +0xE4) / 2 of
    /// 0x5EC260) outside the road there (0x40ADA0): the node becomes that
    /// point and the points before it go (0x401ED0 / 0x4120C0); otherwise
    /// [`search`] and the walk is cleared (0x411BE0). With a node the walk
    /// is extended to max(125 m, 2 s × |v|) past the car (0x428030); with
    /// none it stays empty.
    pub fn update(
        &mut self,
        routes: &AiRoutes,
        position: Vec3,
        velocity: Vec3,
        forward: Vec3,
        speed: f32,
        half_width: f32,
    ) -> HumanStep {
        let f = &routes.file;
        let mut step = HumanStep::default();
        // A node of another route file (the caller's world changed without a
        // new controller) is no node.
        if self.node.is_some_and(|(r, c)| f.records.get(r).is_none_or(|rec| c >= rec.points.len())) {
            *self = HumanRoad::default();
        }
        if self.way_turns(f, velocity, forward, speed) {
            self.backward = !self.backward;
            self.walk = None;
        }
        let kept = self.walk.as_ref().and_then(|w| w.keeps_human(routes, position, half_width));
        match kept {
            Some(i) => {
                let w = self.walk.as_mut().unwrap();
                let p = w.drop_before(i);
                self.node = Some((p.record, p.cp));
                self.backward = p.backward;
            }
            None => {
                step.node_updates = 1;
                let (node, searches) = search(f, position, self.node);
                step.node_searches = searches;
                self.node = node;
                self.walk = None;
            }
        }
        let Some((record, cp)) = self.node else {
            self.walk = None;
            return step;
        };
        let reach = REACH.max(REACH_TIME * speed);
        let walk = self.walk.get_or_insert_with(|| {
            let mut w = RacerRoute::from_node(routes, position, record, cp, self.backward);
            w.human = true;
            w.branch_hold = super::start_hold(0);
            w
        });
        walk.extend_human(f, position, reach);
        step
    }
}

/// 0x42CAA0's switch to the road path: car +0x42C (the forward speed) at
/// least 10 m/s (0x783C34).
pub const ROAD_PATH_SPEED: f32 = 10.0;

impl HumanRoad {
    /// The human controller's path at 10 m/s and above (0x42CAA0 →
    /// 0x4221F0 = 0x41D160(0, 0), which empties the path and clears its +1,
    /// then 0x41E860(path, cur = target = lat, rate 0);
    /// **verified** in the disassembly): `lat` is the car's raw
    /// offset off the walk's line (0x419BC0 → 0x419B20, `locate_object`;
    /// the smoothed +0x3DC is a lateral rate the path never reads). From the
    /// walk's current entry (+0x115 = the list's +0xAD, set by the route
    /// manager 0x428030 from 0x4198C0) while entries remain and the path
    /// holds under 20 points: the entry's room lo = vt+0x0C(k), hi =
    /// vt+0x10(k) (k = 0 outside a drag race, 0x4116A0: −half width −
    /// (lane line + adjust), +half width − the same, the sides by the
    /// entry's direction, `Located::room`); c = min(hi, max(lo, cur)); the
    /// point 0x4122C0(+0x54, c, k) appended by 0x404CD0 with the car
    /// ([`human_append`]); then cur = c (rate 0: a running clamp, it never
    /// returns toward the car's offset). None without a walk (the caller
    /// keeps the clone's fallback).
    pub fn road_path(&self, routes: &AiRoutes, position: Vec3, velocity: Vec3, forward: Vec3, speed: f32) -> Option<Vec<Vec3>> {
        let walk = self.walk.as_ref()?;
        let f = &routes.file;
        let mut cur = walk.locate_object(routes, position)?.lateral;
        let mut path: Vec<Vec3> = Vec::new();
        let mut i = walk.current;
        while i < walk.points.len() && path.len() < 20 {
            let p = &walk.points[i];
            let cp = &f.records[p.record].points[p.cp];
            let line = crate::formats::ai_routes::lane_offset(cp, crate::formats::ai_routes::lane(0, p.backward)) + p.adjust;
            let (left, right) =
                if p.backward { (cp.half_width[1], cp.half_width[0]) } else { (cp.half_width[0], cp.half_width[1]) };
            let (lo, hi) = (-left - line, right - line);
            let m = if cur > lo { cur } else { lo };
            let c = if hi < m { hi } else { m };
            let q = walk.line_point(f, p, p.offset, c);
            human_append(&mut path, q, position, velocity, forward, speed);
            cur = c;
            i += 1;
        }
        Some(path)
    }
}

/// 0x404CD0 as 0x41E860 calls it (its arguments 4–6 are the pushes left
/// for 0x5D89C0: lane k, 1, the car): a 21st point is refused; the first
/// point needs ≥ 1 m (0x7837B8) from the car's place (+0x60, 2D) and
/// either 0x4036E0's ahead test — (point − car) against the car's
/// velocity (+0x70; its forward +0x90 below 1 m/s, +0x430), both
/// normalized, dot > 0 — or more than 20 m (0x7837BC; 8 m, 0x7837C0, with
/// the path's +1, which 0x41D160 cleared); a later one needs ≥ 1 m from the
/// last and, under 20 m from it, must not turn back against the last
/// segment, whose start is the car's place while the path has one point.
/// **Verified** (dis 0x404CD0..0x404ECC, 0x4036E0).
pub fn human_append(path: &mut Vec<Vec3>, q: Vec3, car: Vec3, velocity: Vec3, forward: Vec3, speed: f32) {
    if path.len() >= 20 {
        return;
    }
    let q2 = q.truncate();
    match path.last() {
        None => {
            let d = (q2 - car.truncate()).length();
            if d < 1.0 {
                return;
            }
            let dir = if speed < 1.0 { forward.truncate() } else { velocity.truncate() };
            let ahead = (q2 - car.truncate()).normalize_or_zero().dot(dir.normalize_or_zero()) > 0.0;
            if !ahead && d <= 20.0 {
                return;
            }
        }
        Some(&last) => {
            let last2 = last.truncate();
            let d = (q2 - last2).length();
            if d < 1.0 {
                return;
            }
            if d < 20.0 {
                let prev = if path.len() >= 2 { path[path.len() - 2].truncate() } else { car.truncate() };
                if (q2 - last2).dot(last2 - prev) < 0.0 {
                    return;
                }
            }
        }
    }
    path.push(q);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ai_routes::{ControlPoint, RouteRecord};

    fn point(x: f32, flags: u32) -> ControlPoint {
        ControlPoint {
            position: Vec3::new(x, 0.0, 0.0),
            coord: 0.0,
            flags,
            length: 10.0,
            half_width: [5.0, 5.0],
            tangent: Vec2::X,
            junctions: [0xFFFF; 2],
            bank: 0.0,
            grade: 0.0,
            lane_position: [0; 10],
            lane_speed: [100; 10],
        }
    }

    fn record(id: u16, y: f32, lanes: bool) -> RouteRecord {
        let mut points: Vec<ControlPoint> = (0..8).map(|i| point(10.0 * i as f32, HUMAN_POINT | u32::from(id))).collect();
        for p in &mut points {
            p.position.y = y;
        }
        let mut l = [false; 10];
        l[2] = lanes;
        RouteRecord {
            id,
            name: String::new(),
            kind: 0,
            coord_range: (0.0, 0.0),
            bounds: [0.0; 4],
            flags: 0,
            fade: [false; 2],
            lanes: l,
            points,
        }
    }

    fn file() -> RouteFile {
        RouteFile { records: vec![record(1, 0.0, true), record(2, 30.0, true), record(3, 12.0, false)], ..RouteFile::default() }
    }

    // 0x40F180: the nearest lane record's point holding the car; records
    // without a traffic lane are not offered (record 3 lies nearer).
    #[test]
    fn the_global_search_takes_the_nearest_lane_road_holding_the_car() {
        let f = file();
        assert_eq!(search(&f, Vec3::new(24.0, 4.0, 0.0), None), (Some((0, 2)), 1));
        // Off every lane road (8 m beside record 1, 18 m from record 2):
        // the second pass, any record, nearest point.
        assert_eq!(search(&f, Vec3::new(24.0, 9.0, 0.0), None), (Some((2, 2)), 2));
        // More than 4 m below: the first pass refuses it.
        assert_eq!(search(&f, Vec3::new(24.0, 4.0, -5.0), None).1, 2);
    }

    // 0x411490 with a node: the node's own record within 50 m, kept up to
    // 5 m outside its road.
    #[test]
    fn the_node_is_kept_in_its_record_up_to_5_m_off_the_road() {
        let f = file();
        assert_eq!(search(&f, Vec3::new(41.0, 9.5, 0.0), Some((0, 0))), (Some((0, 4)), 0));
        assert_eq!(search(&f, Vec3::new(41.0, 10.5, 0.0), Some((0, 0))).1, 2, "5.5 m outside: dropped, no lane road holds the car");
    }

    #[test]
    fn outside_measures_across_and_along_the_road() {
        let f = file();
        assert_eq!(outside(&f, 0, 2, Vec3::new(24.0, -2.0, 0.0)), (-3.0, -6.0));
        assert_eq!(outside(&f, 0, 2, Vec3::new(18.0, 7.0, 0.0)), (2.0, -8.0));
    }

    // 0x40A390: the node's way turns when the car's way (forward axis
    // at 10 mph and below) meets it below −0.707.
    #[test]
    fn the_way_turns_against_the_car() {
        let f = file();
        let road = HumanRoad { node: Some((0, 2)), backward: false, walk: None };
        let back = Vec3::new(-1.0, 0.0, 0.0);
        assert!(road.way_turns(&f, Vec3::ZERO, back, 0.0));
        assert!(!road.way_turns(&f, Vec3::ZERO, Vec3::new(-0.7, 0.714, 0.0), 0.0));
        // Above 10 mph the velocity decides.
        assert!(!road.way_turns(&f, Vec3::new(10.0, 0.0, 0.0), back, 10.0));
    }
}
