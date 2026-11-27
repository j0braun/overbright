use super::*;

/// The route manager's speed mode (0x5E9E90 → 0x88E9A8) from the track's
/// flags (track record +0x94): bit 0 or 2 → 2, bit 1 → 0, bit 3 → 1, bit 11
/// or 12 → 4, bit 13 → 5 for track numbers 0x125D–0x1261 and 6 for any
/// other (0x5E9EF9: 5 + (n ≠ clamp(n, 0x125D, 0x1261))), else 3.
pub fn speed_mode(track_flags: u32, number: u16) -> usize {
    match track_flags {
        f if f & 1 != 0 || f & 4 != 0 => 2,
        f if f & 2 != 0 => 0,
        f if f & 8 != 0 => 1,
        f if f & 0x1800 != 0 => 4,
        f if f & 0x2000 != 0 => {
            if (0x125D..=0x1261).contains(&number) {
                5
            } else {
                6
            }
        }
        _ => 3,
    }
}

/// The class a race's racing lines are computed for (0x5DDFD0, route
/// manager +0x4674): the least 0x5B5340 of the AI racers' infos (kind 2),
/// else of the humans' (kind 1), else 3. `ai` is the least class of the
/// AI racers' cars, `career` a race started from a career event.
/// * A career race's AI racer infos still read class 0 (info +0x24) when
///   the track load (0x57A73B → 0x5E9E90 → 0x5DDFD0) asks, and 0x5B5340
///   sets a class up only from 0xFF, so the field's class is 0. Observed
///   under Wine: a stage-4 URL circuit (track 4714) and a stage-3 Street X
///   (4603), the infos' bytes 0 until after +0x4674 = 0 and the grip
///   0x876D2C (class 0 × the mode's factor) were set, then the cars' own
///   class (2 and 3); the URL rivals' path point speeds equal the class-0
///   bytes. The writer of that 0 is not traced.
/// * A Quick Race keeps the cars' own class (4014 circuit: class 3, the
///   lane bytes read under Wine, `race_4014_speeds_match_the_original`).
/// * Without AI racers the clone keeps 3 (natively the human's class; the
///   lines are then not driven by a racer).
pub fn field_class(career: bool, ai: Option<usize>) -> usize {
    match ai {
        Some(_) if career => 0,
        Some(c) => c,
        None => 3,
    }
}

/// The braking and grip factors by speed mode (0x7A4668 rows of 8 floats,
/// +0x18 and +0x1C, copied to 0x876D28 / 0x876D2C by 0x5D4C40).
const MODE_DECEL: [f32; 7] = [1.0; 7];
const MODE_GRIP: [f32; 7] = [1.0, 0.7, 1.0, 1.0, 0.95, 1.25, 1.0];
/// The neighbours a curvature is taken through are at least this far
/// (0x876D24 = 900 = 30², 0x7A4790); the speeds then pass between direct
/// neighbours (1 m, 0x7A4794). Each search tests at most 10 points.
const CURVE_REACH: f32 = 30.0;
const NEAR_REACH: f32 = 1.0;
/// The speed passes repeat until nothing changes, at most this often.
const SPEED_ROUNDS: usize = 10;

/// One racing line's point in the speed precompute (0x5E8790's 400-byte
/// work entries hold one per lane per control point: +0x10 lane 0, +0xD0
/// lane 1, more where junctions join).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct LineNode {
    record: usize,
    cp: usize,
    lane: usize,
}

/// A line point's sample: its neighbours ahead (+0x20) and behind (+0x24)
/// along the lane's direction of travel.
#[derive(Clone, Copy, Debug, Default)]
struct LineSample {
    ahead: Option<Link>,
    behind: Option<Link>,
}

/// A link to another line point's sample (its entry's lane, sample index).
type Link = (LineNode, usize);

impl AiRoutes {
    /// The racing lines' speeds (lanes 0 and 1, CP +0x2C / +0x2D) as the
    /// route manager computes them at race load (0x5DDFD0 then 0x5E8790
    /// with +0x4678 set and +0x4679 clear): every listed record's (+0x70
    /// bit 6, not bit 9) line point gets the class's top speed (0x5D9350),
    /// then the curvature limit √(g·9.8 / max(|κ|, g·9.8 / top²)) with κ
    /// through the line points 30 m behind and ahead (0x5E3EB0, 0x5E5AB0,
    /// 0x5D4AE0, 0x5D8F10), then each is held to what a car can brake down
    /// to the point ahead at the class's braking (0x5D8FD0 → 0x5D4B90) and
    /// reach from the point behind with the class's acceleration curve
    /// (0x5D9150 → 0x5D9080), repeated until nothing changes. Speeds are
    /// whole m/s bytes throughout. `class` is the best (lowest) of the AI
    /// racers' performance classes (0x5DDFD0), `suv` when an AI racer
    /// drives an SUV (0x5D4C40), `mode` from [`speed_mode`]. With `lap`
    /// (a race whose road is built, 0x88F2BE, on a track without +0x94
    /// bit 1) the junction types are adjusted last (see [`RaceLap`]).
    pub fn racing_speeds(&mut self, curves: &ClassCurves, class: usize, suv: bool, mode: usize, lap: Option<RaceLap>) {
        let c = class.min(3);
        let m = mode.min(6);
        let decel = MODE_DECEL[m] * if suv { DECEL_SUV[c] } else { DECEL[c] };
        let grip = MODE_GRIP[m] * if suv { GRIP_SUV[c] } else { GRIP[c] };
        let top = curves.top[c];
        let f = &mut self.file;
        let listed = |f: &RouteFile, r: usize| f.records[r].flags >> 6 & 1 != 0 && f.records[r].flags >> 9 & 1 == 0;
        // The line points and their samples, entries by record id (the
        // route manager's 1500 slots): within a record lane 0 runs with the
        // control points, lane 1 against them (0x5D4A30: one sample a lane,
        // linked to the neighbouring points' first samples).
        let mut samples: crate::fxhash::FxHashMap<LineNode, Vec<LineSample>> = Default::default();
        let mut order = Vec::new();
        for id in 0..0x5DC {
            let Some(r) = f.record(id as i16) else { continue };
            if !listed(f, r) {
                continue;
            }
            let n = f.records[r].points.len();
            for cp in 0..n {
                order.push((r, cp));
                let prev = cp.checked_sub(1).map(|k| (r, k));
                let next = (cp + 1 < n).then_some((r, cp + 1));
                for lane in 0..2 {
                    let node = |p: Option<(usize, usize)>| p.map(|(record, cp)| (LineNode { record, cp, lane }, 0));
                    let (ahead, behind) = if lane == 0 { (node(next), node(prev)) } else { (node(prev), node(next)) };
                    samples.insert(LineNode { record: r, cp, lane }, vec![LineSample { ahead, behind }]);
                }
            }
        }
        // The join pass (0x5E8B05–0x5E8EC1), junctions in table order: not
        // traffic (7, 8), not turns (bit 26), between two different
        // records without bits 9 and 10. The source (record +0x0A, point
        // +0x06) runs on lane bit 18, the target (0x5D86E0) on bit 18 ^ 23;
        // each point must carry its lane's flag (CP +0x0C bit 18 for lane 0,
        // bit 11 for lane 1).
        let live = f.junctions.len();
        let usable = |f: &RouteFile, id: i16| {
            f.record(id).filter(|&r| f.records[r].flags >> 9 & 1 == 0 && f.records[r].flags >> 10 & 1 == 0)
        };
        let lane_flag = |flags: u32, lane: usize| flags >> if lane == 1 { 11 } else { 18 } & 1 != 0;
        let point = |f: &RouteFile, (r, p): (i16, i16)| {
            f.record(r).and_then(|r| usize::try_from(p).ok().filter(|&p| p < f.records[r].points.len()).map(|p| (r, p)))
        };
        let own_of = |j: &crate::formats::ai_routes::Junction| (j.target_record[1], j.target_point[1]);
        for k in 0..live {
            let j = f.junctions[k];
            let kind = j.kind();
            if kind == 7 || kind == 8 || j.turn() {
                continue;
            }
            let (Some(_), Some(_)) = (usable(f, j.target_record[1]), usable(f, j.target().0)) else { continue };
            if j.target_record[1] == j.target().0 {
                continue;
            }
            let (Some((sr, sp)), Some((tr, tp))) = (point(f, own_of(&j)), point(f, j.target())) else { continue };
            let sl = usize::from(j.direction());
            let tl = sl ^ usize::from(j.flips());
            if !lane_flag(f.records[sr].points[sp].flags, sl) || !lane_flag(f.records[tr].points[tp].flags, tl) {
                continue;
            }
            let s_node = LineNode { record: sr, cp: sp, lane: sl };
            let t_node = LineNode { record: tr, cp: tp, lane: tl };
            if !samples.contains_key(&s_node) || !samples.contains_key(&t_node) {
                continue;
            }
            // 0x5D8770: the junction at the target leading back. When it is
            // a race link the target's lane loses the point behind it, and
            // that point's lane its links ahead (0x5E8CDA–0x5E8D32).
            let own_pt = point(f, own_of(&j));
            let partner = if Some((tr, tp)) == own_pt {
                Some(j)
            } else {
                let d = j.direction() ^ j.flips();
                f.records[tr].points[tp]
                    .junctions
                    .iter()
                    .map(|&x| usize::from(x))
                    .filter(|&x| x != 0xFFFF && x < live)
                    .map(|x| f.junctions[x])
                    .find(|x| x.direction() != d && point(f, x.target()) == own_pt)
            };
            if partner.is_some_and(|x| x.kind() == 2)
                && let Some((old, _)) = samples.get_mut(&t_node).unwrap()[0].behind.take()
                && let Some(list) = samples.get_mut(&LineNode { lane: tl, ..old })
            {
                list.iter_mut().for_each(|x| x.ahead = None);
            }
            let list = &samples[&s_node];
            let (s_last, s_count) = (list[list.len() - 1], list.len());
            let list = &samples[&t_node];
            let (t_last, t_count, t_behind) = (list[list.len() - 1], list.len(), list[0].behind.is_some());
            if kind == 2 {
                // A race link takes the source's place ahead (its old
                // point loses its links behind); the target takes the
                // source behind, on a new sample when it has one.
                if let Some((old, _)) = samples[&s_node][0].ahead
                    && let Some(list) = samples.get_mut(&LineNode { lane: sl, ..old })
                {
                    list.iter_mut().for_each(|x| x.behind = None);
                }
                if t_behind {
                    samples.get_mut(&t_node).unwrap().push(LineSample { behind: Some((s_node, 0)), ..t_last });
                    samples.get_mut(&s_node).unwrap()[0].ahead = Some((t_node, t_count));
                } else {
                    samples.get_mut(&s_node).unwrap()[0].ahead = Some((t_node, 0));
                    samples.get_mut(&t_node).unwrap()[0].behind = Some((s_node, 0));
                }
            } else if t_behind {
                // Anything else adds a sample to the source, and to the
                // target when that has a point behind already.
                samples.get_mut(&s_node).unwrap().push(LineSample { ahead: Some((t_node, t_count)), ..s_last });
                samples.get_mut(&t_node).unwrap().push(LineSample { behind: Some((s_node, s_count)), ..t_last });
            } else {
                samples.get_mut(&s_node).unwrap().push(LineSample { ahead: Some((t_node, 0)), ..s_last });
                samples.get_mut(&t_node).unwrap()[0].behind = Some((s_node, s_count));
            }
        }
        // The line's points (0x5E3EB0): the lane's position off the control
        // point (0x5DD370 forward for lane 0, 0x5DD4A0 backward for lane 1).
        let pos = |f: &RouteFile, n: LineNode| {
            let cp = &f.records[n.record].points[n.cp];
            f.point(n.record, n.cp, n.lane == 1, 0.0, lane_offset(cp, n.lane))
        };
        // The neighbour a sample's curvature or speed is taken from
        // (0x5E5930 ahead, 0x5E59F0 behind): along the chain the first of
        // at most 10 points on the far side of the control point's
        // direction of travel and `reach` or more away; the 11th, or the
        // chain's end, if none is.
        let find = |f: &RouteFile,
                    samples: &crate::fxhash::FxHashMap<LineNode, Vec<LineSample>>,
                    n: LineNode,
                    first: Option<Link>,
                    ahead: bool,
                    reach: f32|
         -> Option<Link> {
            let here = pos(f, n);
            let dir = f.direction(n.record, n.cp, n.lane == 1);
            let centre = f.records[n.record].points[n.cp].position.truncate();
            let mut cand = first?;
            for _ in 0..10 {
                let link = samples
                    .get(&cand.0)
                    .and_then(|s| s.get(cand.1))
                    .and_then(|s| if ahead { s.ahead } else { s.behind });
                let Some(next) = link else { return Some(cand) };
                let p = pos(f, cand.0);
                let d = (p.truncate() - centre).dot(dir);
                if (if ahead { d > 0.0 } else { d < 0.0 }) && (p - here).length_squared() >= reach * reach {
                    return Some(cand);
                }
                cand = next;
            }
            Some(cand)
        };
        let set = |f: &mut RouteFile, n: LineNode, v: f32| {
            f.records[n.record].points[n.cp].lane_speed[n.lane] = ((v * 65536.0) as i32 >> 16) as u8
        };
        let get = |f: &RouteFile, n: LineNode| f32::from(f.records[n.record].points[n.cp].lane_speed[n.lane]);
        // The curvature limit (0x5D8F10): the least over the samples.
        let a_lat = grip * 9.8;
        let floor = a_lat / (top * top);
        let mut nodes: Vec<LineNode> =
            order.iter().flat_map(|&(record, cp)| (0..2).map(move |lane| LineNode { record, cp, lane })).collect();
        for &n in &nodes {
            let mut v = f32::MAX;
            for s in &samples[&n] {
                let k = match (s.behind, s.ahead) {
                    (Some(_), Some(_)) => {
                        let b = find(f, &samples, n, s.behind, false, CURVE_REACH).unwrap();
                        let a = find(f, &samples, n, s.ahead, true, CURVE_REACH).unwrap();
                        curvature(pos(f, b.0).truncate(), pos(f, n).truncate(), pos(f, a.0).truncate())
                    }
                    _ => 0.0,
                };
                v = v.min((a_lat / k.abs().max(floor)).sqrt());
            }
            set(f, n, v);
        }
        // The neighbours the speeds pass between (0x5E5AB0 at 1 m).
        let near: crate::fxhash::FxHashMap<LineNode, Vec<(Option<Link>, Option<Link>)>> = nodes
            .iter()
            .map(|&n| {
                (
                    n,
                    samples[&n]
                        .iter()
                        .map(|s| {
                            (
                                find(f, &samples, n, s.ahead, true, NEAR_REACH),
                                find(f, &samples, n, s.behind, false, NEAR_REACH),
                            )
                        })
                        .collect(),
                )
            })
            .collect();
        // 0x5D4B90: v ≤ √(v_ahead² + 2·braking·d); 0x5D9080: v ≤ √(v_behind²
        // + 2·a(v_behind)·d), d the straight distance between line points.
        let brake = |f: &mut RouteFile, n: LineNode| {
            let mut changed = false;
            for &(ahead, _) in &near[&n] {
                if let Some((o, _)) = ahead {
                    let (v, vo) = (get(f, n), get(f, o));
                    let reach = (vo * vo + 2.0 * decel * pos(f, n).distance(pos(f, o))).sqrt();
                    if v > reach {
                        set(f, n, reach);
                        changed = true;
                    }
                }
            }
            changed
        };
        let speed_up = |f: &mut RouteFile, n: LineNode| {
            let mut changed = false;
            for &(_, behind) in &near[&n] {
                if let Some((o, _)) = behind {
                    let (v, vo) = (get(f, n), get(f, o));
                    let reach = (vo * vo + 2.0 * curves.accel(c, vo) * pos(f, n).distance(pos(f, o))).sqrt();
                    if v > reach {
                        set(f, n, reach);
                        changed = true;
                    }
                }
            }
            changed
        };
        // Each round first across the junctions (0x5E90A0–0x5E924A: not 7
        // or 8, not turns, two different records without bits 9 and 10, no
        // lane flags asked): the source's lane brakes, the target's speeds
        // up; then in entry order lane 1 brakes and lane 0 speeds up, and in
        // reverse lane 0 brakes and lane 1 speeds up (0x5E9250 / 0x5E928C).
        let mut across = Vec::new();
        for j in &f.junctions {
            let kind = j.kind();
            if kind == 7 || kind == 8 || j.turn() || j.target_record[1] == j.target().0 {
                continue;
            }
            let (Some(_), Some(_)) = (usable(f, j.target_record[1]), usable(f, j.target().0)) else { continue };
            let (Some((sr, sp)), Some((tr, tp))) = (point(f, own_of(j)), point(f, j.target())) else { continue };
            let sl = usize::from(j.direction());
            let s_node = LineNode { record: sr, cp: sp, lane: sl };
            let t_node = LineNode { record: tr, cp: tp, lane: sl ^ usize::from(j.flips()) };
            if samples.contains_key(&s_node) && samples.contains_key(&t_node) {
                across.push((s_node, t_node));
            }
        }
        nodes.retain(|n| n.lane == 0);
        for _ in 0..SPEED_ROUNDS {
            let mut changed = false;
            for &(s_node, t_node) in &across {
                changed |= brake(f, s_node);
                changed |= speed_up(f, t_node);
            }
            for &n in &nodes {
                changed |= brake(f, LineNode { lane: 1, ..n });
                changed |= speed_up(f, n);
            }
            for &n in nodes.iter().rev() {
                changed |= brake(f, n);
                changed |= speed_up(f, LineNode { lane: 1, ..n });
            }
            if !changed {
                break;
            }
        }
        // The junction-type adjustment (0x5E92E1–0x5E95EA): each junction
        // of type 0, 1 or 4 between two different records without bits 9
        // and 10, whose ends carry their lanes' flags, compares the race
        // coordinates 10 s ahead (0x5DDDF0, t = 10.0 at 0x7A4798) from its
        // own point (lane bit 18) and from its target (lane bit 18 ^ 23):
        // d = target − own, wrapped by the lap in a looped race (|d| past
        // half the lap, 0x88F2B4); d < −20 (0x7A479C) makes it type 1 and
        // d > 20 type 0.
        let Some(lap) = lap else { return };
        // 0x5DDDF0: from a line point along the lane's chain (the first
        // sample's ahead links), each step taking the 3D distance over the
        // two speeds' mean, and 3 s and a jump to the target's lane (bit 23
        // ^ bit 18) at a turn link (type 2, bit 26) of the point's live
        // junctions whose bit 18 is the lane; at `t` the race coordinate
        // (CP +0x08) of the next point, the start's when the chain ends.
        let ahead_after = |f: &RouteFile, start: LineNode, t_max: f32| -> f32 {
            let coord = |n: LineNode| f.records[n.record].points[n.cp].coord;
            let speed = |n: LineNode| f32::from(f.records[n.record].points[n.cp].lane_speed[n.lane]);
            // The samples' near links (+0x28, 0x5E5AB0 at 1 m).
            let next_of = |l: Link| near.get(&l.0).and_then(|s| s.get(l.1)).and_then(|s| s.0);
            let mut cur = next_of((start, 0));
            let (mut prev_p, mut prev_v, mut t) = (pos(f, start), speed(start), 0.0f32);
            while t < t_max {
                let Some(c) = cur else { return coord(start) };
                let (p, v) = (pos(f, c.0), speed(c.0));
                t += (prev_p - p).length() / ((v + prev_v) * 0.5);
                let turn = f.records[c.0.record].points[c.0.cp]
                    .junctions
                    .iter()
                    .map(|&j| usize::from(j))
                    .filter(|&j| j != 0xFFFF && j < live)
                    .map(|j| f.junctions[j])
                    .find(|j| j.kind() == 2 && j.turn() && usize::from(j.direction()) == c.0.lane);
                cur = match turn {
                    Some(j) => {
                        t += 3.0;
                        let lane = usize::from(j.direction() ^ j.flips());
                        point(f, j.target())
                            .map(|(record, cp)| (LineNode { record, cp, lane }, 0))
                            .filter(|n| samples.contains_key(&n.0))
                    }
                    None => next_of(c),
                };
                prev_p = p;
                prev_v = v;
                if cur.is_none() {
                    return coord(start);
                }
            }
            cur.map_or(coord(start), |c| coord(c.0))
        };
        for k in 0..live {
            let j = f.junctions[k];
            if !matches!(j.kind(), 0 | 1 | 4) {
                continue;
            }
            let (Some(sr), Some(tr)) = (usable(f, j.target_record[1]), usable(f, j.target_record[0])) else { continue };
            let (Ok(sp), Ok(tp)) = (usize::try_from(j.target_point[1]), usize::try_from(j.target_point[0])) else {
                continue;
            };
            if sp >= f.records[sr].points.len()
                || tp >= f.records[tr].points.len()
                || j.target_record[1] == j.target_record[0]
            {
                continue;
            }
            let sl = usize::from(j.direction());
            let tl = sl ^ usize::from(j.flips());
            if !lane_flag(f.records[sr].points[sp].flags, sl) || !lane_flag(f.records[tr].points[tp].flags, tl) {
                continue;
            }
            let own = ahead_after(f, LineNode { record: sr, cp: sp, lane: sl }, TYPE_HORIZON);
            let other = ahead_after(f, LineNode { record: tr, cp: tp, lane: tl }, TYPE_HORIZON);
            let mut d = other - own;
            if lap.wrap {
                let half = lap.length * 0.5;
                if d < -half {
                    d += lap.length;
                } else if d > half {
                    d -= lap.length;
                }
            }
            if d < -TYPE_GAP {
                f.junctions[k].flags = f.junctions[k].flags & 0xFFFF_FFF1 | 1;
            } else if d > TYPE_GAP {
                f.junctions[k].flags &= 0xFFFF_FFF0;
            }
        }
    }
}

/// The race road's lap as the route data holds it (0x88F298): +0x18 the
/// length (0x88F2B0: 1.0 from 0x5D9A70 without a race; Race_BuildRoute
/// 0x5E5BC0 stores its road's length there, +0x1C half of it, 0x88F2B4),
/// +0x24 whether it loops (0x88F2BC, the road record's byte +3).
#[derive(Clone, Copy, Debug)]
pub struct RaceLap {
    pub length: f32,
    pub wrap: bool,
}

/// 0x5E8790's junction-type test: the race coordinates 10 s ahead
/// (0x7A4798) and the gap that retypes (0x7A479C).
const TYPE_HORIZON: f32 = 10.0;
const TYPE_GAP: f32 = 20.0;
