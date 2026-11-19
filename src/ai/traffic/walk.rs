use super::*;

/// Lane points handed to the driver (0x41D530 fills the action's path up
/// to 20, 0x415D50).
pub(super) const PATH_POINTS: usize = 20;

/// The walk's least and most points (0x422220: more than 9; 0x28).
pub(super) const WALK_MIN: usize = 10;

pub(super) const WALK_MAX: usize = 40;

/// A control point walked in a direction, with the forward traffic lane
/// (2–5) the walk is on there (0 in the placer's walk).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Step {
    pub(super) record: usize,
    pub(super) cp: usize,
    pub(super) backward: bool,
    pub(super) lane: usize,
}

/// One traffic car's lane: the control points ahead, already decided at
/// the junctions, each with its lane (2–5 forward; read as 6–9 backward).
#[derive(Clone, Debug)]
pub struct TrafficLane {
    pub(super) steps: VecDeque<Step>,
    pub(super) rng: GameRng,
    /// The "Obey traffic intersections" action while one is running: the
    /// intersection (action +0x210) and whether the car has stopped at its
    /// line (+0x214).
    pub(super) obey: Option<(usize, bool)>,
    /// "Avoid Cars as a traffic car" while it runs: its state.
    pub(super) avoid: Option<Avoid>,
    /// How many of the first steps the placement's walk built: those got
    /// their lane room (0x41FD50) when 0x42C950 ran 0x428030 for the car
    /// (0x42A190 / 0x42A360); the walk's later points keep the zero room
    /// FuturePoint's ctor (0x40AD20) gives them.
    pub(super) roomed: usize,
    /// Set when a think takes a new "Avoid Cars as a traffic car" (type 2):
    /// the controller then switches a car on its TrafficMover (mode 2) over
    /// loaded world collision (car +0x5CC) to the MeloMvr (mode 3; 0x429FF8
    /// stores the new action, 0x42A004–0x42A028 test its type +0x204 for 8,
    /// 1 or 2 and call 0x5F46B0(3); **verified**). See [`TrafficLane::take_avoid`].
    pub(super) took_avoid: bool,
    /// The walk point last dropped behind the car (probe use: `off_centre`).
    pub(super) behind: Option<Step>,
    /// The driven path (car +0x30 → +0x10), kept from think to think:
    /// 0x420D40 plans afresh (0x41D160(0, 0)) only when 0x41BFE0 says so,
    /// else, with the driver's byte +0x1D9 set — **observed** 1 for every
    /// traffic car under Wine (2026-11-04, career free roam, 16 of 16) —
    /// it keeps the path's front (see [`TrafficLane::update`]). Each point
    /// with its walk point and lateral offset.
    pub(super) kept: Vec<(Step, f32, f32, Vec3)>,
    /// The car's heading at the last think (0x404EF0 drops the path's
    /// points behind the car along it).
    pub(super) heading: Option<Vec2>,
    /// The walk's distance (m) at its first step, counted from the walk's
    /// start: the points dropped behind the car so far (0x419670 takes
    /// their s off every point and off +0x354, 0x41970E).
    pub(super) walked: f32,
    /// Walk +0x354: where the walk took its last turn (the s of its last
    /// point then, the junction's, 0x41EDC7; −99999 after a reset, 0x411C0C).
    pub(super) last_turn: f32,
    /// The walk's last step is a turn's target added with nothing after it
    /// in that extension: the junction's place in its point's list. The
    /// turn's points carry +0x58 ≠ 0 (0x41EE20), so the next extension's
    /// 0x40A620 drops them again.
    pub(super) turned: Option<usize>,
    /// The car's own walk point (the road entry's +0x28; the walk's first,
    /// walk +0xAD = 0 under Wine): its index in [`TrafficLane::walk_points`]
    /// and its +0x58 (metres along `steps[0]`). The walk keeps the control
    /// point behind the car and the 8 m points on its segment.
    pub(super) lead: (usize, f32),
    /// A drag race's traffic (0x89E7D8 / 0x890108): 0x422220 keeps points
    /// under 3 m apart there.
    pub(super) drag: bool,
}

/// Whether a lane (0–9) exists where a control point lies: its record's
/// lane byte (header +0x82 + lane, 0x410CA0).
pub(super) fn has_lane(f: &RouteFile, s: Step, l: usize) -> bool {
    f.records[s.record].lanes[lane(l, s.backward)]
}

/// 0x410CA0: a traffic lane of a record for a direction, as a forward index
/// (2–5): up to ten random picks of the four, then the first that exists.
pub(super) fn pick_lane(f: &RouteFile, s: Step, rng: &mut GameRng) -> Option<usize> {
    for _ in 0..10 {
        let l = 2 + (rng.below(400_000) / 100_000) as usize;
        if has_lane(f, s, l) {
            return Some(l);
        }
    }
    (2..=5).find(|&l| has_lane(f, s, l))
}

/// 0x415B70: the lane a path keeps at a control point: its lane if the
/// record has it, else the nearest that it has, one up then one down, up
/// to three apart; else its lane.
pub(super) fn keep_lane(f: &RouteFile, s: Step, l: usize) -> usize {
    if !(2..=5).contains(&l) || has_lane(f, s, l) {
        return l;
    }
    for k in 1..=3 {
        for c in [l + k, l.wrapping_sub(k)] {
            if (2..=5).contains(&c) && has_lane(f, s, c) {
                return c;
            }
        }
    }
    l
}

pub(super) fn walk(f: &RouteFile, s: Step, rng: &mut GameRng) -> Option<Step> {
    walk_gated(f, s, rng, 0, true).map(|w| w.0)
}

/// A junction a walk step took: its place in the point's junction list and
/// whether it is a turn (bit 26).
pub(super) type Taken = Option<(usize, bool)>;

/// [`walk`] as the route manager's extension runs it (0x422220 →
/// 0x41EC20; verified): the point's junctions before `skip` in its list are
/// passed over (a re-walk after 0x40A620 dropped a turn: those were
/// declined before, which the controller's junction cache 0x41CFA0 keeps),
/// and a turn (bit 26) other than a link the goal must take (0x5DCBF0(1),
/// high nibble 2, controller slot +0x44) is considered only when `turns`
/// — the walk's last point lies more than 30 m past the point of its last
/// turn (walk +0x354). Returns the step and the junction taken.
pub(super) fn walk_gated(f: &RouteFile, s: Step, rng: &mut GameRng, skip: usize, turns: bool) -> Option<(Step, Taken)> {
    let point = &f.records[s.record].points[s.cp];
    for (pos, j) in
        f.junctions_at(point).map(|j| f.junctions[j]).enumerate().filter(|(_, j)| j.direction() == s.backward)
    {
        if j.kind() == 3 {
            return None;
        }
        if pos < skip || (j.turn() && !turns && j.flags >> 4 & 0xF != 2) {
            continue;
        }
        let target = || {
            let (rid, tcp) = j.target();
            let record = f.record(rid)?;
            let cp = usize::try_from(tcp).ok().filter(|&c| c < f.records[record].points.len())?;
            Some(Step { record, cp, backward: s.backward ^ j.flips(), lane: s.lane })
        };
        let take = match j.flags >> 4 & 0xF {
            2 => true,
            4 => {
                let odds = if j.turn() && s.lane != 0 {
                    let fits = target().is_some_and(|t| has_lane(f, t, s.lane))
                        && (2..=5).rev().find(|&l| has_lane(f, s, l)) == Some(s.lane);
                    if fits { 0.5 } else { 0.0 }
                } else {
                    0.5
                };
                rng.unit(1.0) < odds
            }
            _ => false,
        };
        if take {
            return target().map(|t| (Step { lane: keep_lane(f, t, t.lane), ..t }, Some((pos, j.turn()))));
        }
    }
    let on =
        if s.backward { s.cp.checked_sub(1) } else { Some(s.cp + 1).filter(|&c| c < f.records[s.record].points.len()) };
    on.map(|cp| {
        let t = Step { cp, ..s };
        (Step { lane: keep_lane(f, t, s.lane), ..t }, None)
    })
}

/// The road walk the placers share (0x415130), on the racing line: from a
/// control point travelled `backward` or not, at least `metres` on, a
/// point's segment a step, taking junctions as [`walk`] does. None at a
/// dead end. A probe helper (the clone's own pursuit); the placers search
/// with [`placer_walk`] (traffic) and [`racer_search`] (encounters).
pub fn road_walk(
    f: &RouteFile,
    record: usize,
    cp: usize,
    backward: bool,
    metres: f32,
    rng: &mut GameRng,
) -> Option<(usize, usize, bool)> {
    let mut s = Step { record, cp, backward, lane: 0 };
    let mut moved = 0.0;
    while moved < metres {
        moved += f.segment(s.record, s.cp, s.backward).max(1e-3);
        s = walk(f, s, rng)?;
    }
    Some((s.record, s.cp, s.backward))
}

/// 0x418DA0's spacing of the points it puts between two control points
/// (0x783C68).
pub(super) const SUB_POINT: f32 = 8.0;

/// 0x41A180's tangent length, × the run's length.
pub(super) const SPLINE_TANGENT: f32 = 0.75;

/// A lane's reach while nothing on the walk ends it (0x418AE0: 99999,
/// 0x47C34F80).
pub(super) const LANE_REACH_NONE: f32 = 99_999.0;

/// 0x412500's margin: a lane that does not reach the walk's end may still
/// be taken when it reaches this much (m) farther than the car's
/// (0x40AF30 with 25.0, 0x41C80000).
pub(super) const LANE_REACH_MORE: f32 = 25.0;

/// The lanes a FuturePoint keeps an offset for (+0x04, +0x08..+0x14).
pub(super) const WALK_LANES: [usize; 5] = [0, 2, 3, 4, 5];

/// The points 0x418DA0 puts on a control point's segment: trunc(segment /
/// 8) − 1 (0x418E8B..0x418E9A).
pub(super) fn sub_points(segment: f32) -> usize {
    ((segment / SUB_POINT).max(0.0) as usize).saturating_sub(1)
}

/// The step between two walk points (0x40CBC0, verified: what 0x422220 adds
/// to the walk's s for a new point): the next control point's offset from
/// the last projected (2D) on the last's direction of travel. Within a
/// record about the point's segment; across a junction the jump between the
/// two roads (observed under Wine: 540:1 → 495:1 at s + 4.9, 731:10 → 477:24
/// at s + 13.3, where the records' own segments are 25.1 and 5.0 m).
pub(super) fn gap(f: &RouteFile, a: Step, b: Step) -> f32 {
    let (pa, pb) =
        (f.records[a.record].points[a.cp].position.truncate(), f.records[b.record].points[b.cp].position.truncate());
    (pb - pa).dot(f.direction(a.record, a.cp, a.backward))
}

/// One of the walk's FuturePoints: its control point (`TrafficLane::steps`
/// index), +0x58 (metres along it), +0x54 (s, from the walk's first) and
/// its lane offsets (+0x04 lane 0, +0x08.. lanes 2–5).
#[derive(Clone, Copy, Debug)]
pub(super) struct WalkPoint {
    pub(super) step: usize,
    pub(super) d: f32,
    pub(super) s: f32,
    pub(super) x: [f32; 5],
}

impl TrafficLane {
    /// The route manager's walk extension (0x428030 → 0x422220; verified):
    /// the walk keeps the car's nearest point and grows until its last
    /// point lies at least `reach` on from it — max(125 m (0x7E8E60), 4 s
    /// (controller slot +0x60) × the car's speed, 0x428A10..0x428A41) —
    /// and holds more than 9 points, at most 40 (0x28). Before this fix the clone
    /// walked a fixed 21 control points (often 200–400 m), so an
    /// intersection far past the original's walk already held its cars in
    /// "Obey traffic intersections" and kept them from "Avoid Cars".
    ///
    /// Turns (verified; observed under Wine): each extension first
    /// drops the walk's trailing points whose +0x58 is not 0 (0x40A620) —
    /// the 8 m points and a turn's junction point and target, which
    /// 0x41EE20 adds with offsets along their segments — and a turn is
    /// considered only more than 30 m past the walk's last one (0x41EC20,
    /// walk +0x354). So a turn taken where the walk just reached its length
    /// is gone at the next think, and re-walking from its point the turn is
    /// not considered again: the car goes straight on. Only a turn followed
    /// by more control points in the same extension (the placement's build,
    /// or a turn short of the reach: the length after it is relative to the
    /// walk's first point) stays. Under Wine a car's walk took
    /// junction 947 (record 734 cp 16 → 492 cp 4) at 13.98 s and held it
    /// straight on 2 s later with +0x354 at the junction's s
    /// (the maintainer's private captures); the original's traffic turned at
    /// intersection 3 in 0 of 18 passes in freeroam_stand, the clone's in
    /// 51 of 104 before.
    pub(super) fn extend(&mut self, f: &RouteFile, reach: f32) {
        let mut skip = 0;
        if let Some(pos) = self.turned.take()
            && self.steps.len() > 1
        {
            self.steps.pop_back();
            skip = pos + 1;
        }
        let segs: Vec<f32> = (0..self.steps.len().saturating_sub(1)).map(|i| self.seg(f, i)).collect();
        // Both from the car's point (the walk's first).
        let mut length: f32 = segs.iter().sum::<f32>() - self.lead.1;
        // The walk's points: one per control point and 0x418DA0's between.
        let mut points =
            (self.steps.len() + segs.iter().map(|&g| sub_points(g)).sum::<usize>()).saturating_sub(self.lead.0);
        // 0x422300: on while length ≤ reach (`fcomp; test ah,0x41; jnp`),
        // length relative to the walk's first point (0x419670 rebases +0x54).
        while points < WALK_MAX && (points < WALK_MIN || length <= reach) {
            let Some(&last) = self.steps.back() else { return };
            let at = self.walked + length;
            let turns = at - self.last_turn > TURN_GAP;
            let mut next = walk_gated(f, last, &mut self.rng, std::mem::take(&mut skip), turns);
            // 0x422220 (0x4223AE..0x42241B): outside a drag race a point less
            // than 3 m (0x783C7C) on from the last along its direction
            // (0x40CBC0) is passed over and the walk goes on from it, at
            // most 100 times (then the extension stops).
            let mut tries = 0;
            while !self.drag && next.is_some_and(|(s, _)| gap(f, last, s) < MIN_STEP) {
                tries += 1;
                if tries > 100 {
                    return;
                }
                next = next
                    .and_then(|(s, taken)| walk_gated(f, s, &mut self.rng, 0, turns).map(|(t, k)| (t, k.or(taken))));
            }
            match next {
                Some((s, taken)) => {
                    let g = gap(f, last, s);
                    length += g;
                    points += 1 + sub_points(g);
                    self.steps.push_back(s);
                    self.turned = None;
                    if let Some((pos, true)) = taken {
                        // 0x41EE20 cuts the corner (the source's points
                        // within it go, one at the corner + 3 m, the target
                        // moved on); 0x422220 then counts the walk's length
                        // as the target's s plus the straight distance from
                        // the junction's point to it (fStack_40 added again at
                        // 0x4226C7): about the junction's s + 2 × that
                        // distance (observed: s 117 → target 123, the
                        // extension stopping there with a reach of 125).
                        let jt = f.records[last.record].points[last.cp]
                            .position
                            .truncate()
                            .distance(f.records[s.record].points[s.cp].position.truncate());
                        // 0x41EE20 (verified): both are relative to
                        // the walk's first point — after 0x41EE20 the length is
                        // reloaded from the new last point's +0x54 and jt added
                        // (0x422668..0x4226DF, 0x4226C7), and 0x419670
                        // rebases with every drop behind the car (read under
                        // Wine: J 734:14 cut, corner 115.9, target 492:6 at
                        // s 123.3, +0x354 122.5; `docs/evidence/traf-fix/`).
                        // So a turn short of the reach by more than the
                        // corner keeps the extension going and stays; one
                        // taken where the walk reaches its length (the car
                        // moves 5–8 m a think) is dropped at the next pass.
                        // The clone once added the walk's absolute
                        // distance (`at`), which cut every extension after
                        // a turn once the car had moved. The target's s is
                        // taken as the junction's + jt (0x41EE20's corner cut
                        // puts it 4–5 m shorter in the two cases read:
                        // **inferred** stand-in).
                        length = (at - self.walked) + 2.0 * jt;
                        self.last_turn = at;
                        self.turned = Some(pos);
                    }
                }
                None => return,
            }
        }
    }

    /// How far `p` lies off the lane's centre line through the walk's first
    /// few points (signed, right positive; the nearest piece). A measure for
    /// the tests, not a game quantity.
    pub fn off_centre(&self, f: &RouteFile, p: Vec2) -> Option<f32> {
        // From the last point dropped: the car may be short of the walk's
        // first (its nearest) point.
        let centres: Vec<Vec2> =
            self.behind.iter().chain(self.steps.iter().take(5)).map(|&s| point(f, s, 0.0).truncate()).collect();
        centres
            .windows(2)
            .map(|w| {
                let d = (w[1] - w[0]).normalize_or_zero();
                let r = p - (w[0] + d * (p - w[0]).dot(d).clamp(0.0, (w[1] - w[0]).length()));
                if r.dot(Vec2::new(d.y, -d.x)) < 0.0 { -r.length() } else { r.length() }
            })
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
    }

    /// Whether "Avoid Cars as a traffic car" is running (its lane changes
    /// move the car off its lane's centre on purpose).
    /// Whether a think since the last call took a new avoidance (type 2),
    /// which hands a car on the TrafficMover to the MeloMvr (0x42A013).
    pub fn take_avoid(&mut self) -> bool {
        std::mem::take(&mut self.took_avoid)
    }

    /// The tracked object's id while avoiding (probe).
    pub fn avoid_target(&self) -> Option<usize> {
        self.avoid.map(|a| a.target)
    }

    /// "Avoid Cars" while it runs: the tracked object's id, its time
    /// (+0x21C), the look (+0x220) and the speed asked for (+0x1F0)
    /// (probe use).
    pub fn avoid_state(&self) -> Option<(usize, f32, f32, f32)> {
        self.avoid.map(|a| (a.target, a.time, a.look, a.asked))
    }

    pub fn avoiding(&self) -> bool {
        self.avoid.is_some()
    }

    /// The walk's control points as `record:cp[b]/lane`, and walk +0x354
    /// relative to the first point (probe use).
    pub fn walk_desc(&self) -> String {
        let s: Vec<String> = self
            .steps
            .iter()
            .map(|s| format!("{}:{}{}/{}", s.record, s.cp, if s.backward { "b" } else { "" }, s.lane))
            .collect();
        format!("[{}] walked {:.1} last_turn {:.1} turned {:?}", s.join(" "), self.walked, self.last_turn, self.turned)
    }

    /// The walk's length in points (probe use).
    pub fn walk_len(&self) -> usize {
        self.steps.len()
    }

    /// Whether "Obey traffic intersections" is running (probe use).
    pub fn obeying(&self) -> bool {
        self.obey.is_some()
    }

    /// The walk's current point: record, control point, backward, lane
    /// (2–5), and the lane's direction there (probe use).
    pub fn current(&self, f: &RouteFile) -> Option<(usize, usize, bool, usize, Vec2)> {
        let s = *self.steps.front()?;
        Some((s.record, s.cp, s.backward, s.lane, f.direction(s.record, s.cp, s.backward)))
    }

    /// 0x417750: whether the walk ends at a dead end (a junction of low
    /// nibble 3 in the walk's direction on its last point) that the car
    /// reaches in under [`END_STOP_TIME`] at max(speed, 0.1). The car's
    /// place on the walk is its distance along the first point
    /// (**inferred** for 0x417750's road entry +0x18).
    pub fn ends_ahead(&self, f: &RouteFile, p: Vec2, speed: f32) -> bool {
        let Some(&last) = self.steps.back() else { return false };
        let cp = &f.records[last.record].points[last.cp];
        if !f.junctions_at(cp).map(|j| f.junctions[j]).any(|j| j.direction() == last.backward && j.kind() == 3) {
            return false;
        }
        let to_last: f32 = (0..self.steps.len() - 1).map(|i| self.seg(f, i)).sum();
        let s0 = self.steps[0];
        let gap = to_last - f.along(s0.record, s0.cp, s0.backward, p).max(0.0);
        gap / speed.max(0.1) < END_STOP_TIME
    }

    /// 0x40ABB0 (verified): the walk holds at most one point past the car
    /// (0x40AB10 on the car's s) and that point's record has +0x0C = 1
    /// (unless walk +0xC8, whose writer is not traced; taken as clear,
    // prov: inferred ai.traffic.parks_at_end
    /// **inferred**) — the car is parked (+0x77C). No shipped route record
    /// has +0x0C = 1 in its file (all 17,226 read 0), so for those the
    /// car drives on to the walk's end and stops for good there (0x428720
    /// false).
    pub fn parks_at_end(&self, f: &RouteFile) -> bool {
        let Some(&last) = self.steps.back() else { return false };
        self.steps.len() <= 2 && f.records[last.record].kind == 1
    }

    /// The lane's speed byte where the car is (m/s): what the placement
    /// hands 0x5FBEE0 → 0x5F52D0 as the car's speed (0x42E9F0 passes
    /// control point +0x2C + lane × 1/65536 · 65536).
    pub fn lane_speed(&self, f: &RouteFile) -> f32 {
        self.steps.front().map_or(0.0, |&s| speed_byte(f, s))
    }

    /// How far along the walk each traffic lane 2–5 goes on (walk +0xB4 +
    /// 4·(lane − 2)), and the walk's last point's s (+0x54), both from the
    /// walk's first control point. 0x418AE0 (verified, called by 0x428030
    /// after every extension) sets the reaches to 99999 and, from the car's
    /// point on, for the first point and every point whose record differs
    /// from the point before's, calls 0x411CE0: each lane its record lacks
    /// (byte +0x82 + lane, 6–9 for a point walked backward) takes the least
    /// of its reach and the s of the point's +0x68 (the point appended
    /// before it, 0x418F9C; null, s 0, for the walk's first point after
    /// 0x41970B). **Observed** under Wine (career free roam, the player
    /// standing at the garage exit; 20 of 20 changes at record 492): a car
    /// changes lanes when the walk holds the second point of a record
    /// without its lane, not the first (the junction's target), so the
    /// clone takes the s of the first point on that record (**inferred**:
    /// what +0x68 holds for a junction's target point is not traced).
    // prov: inferred ai.traffic.lane_reach
    pub(super) fn lane_reach(&self, f: &RouteFile) -> ([f32; 4], f32) {
        let mut reach = [LANE_REACH_NONE; 4];
        let mut s = 0.0;
        for (k, &st) in self.steps.iter().enumerate() {
            if k > 0 {
                s += self.seg(f, k - 1);
            }
            let first_on_record = k == 0 || self.steps[k - 1].record != st.record;
            if first_on_record {
                for (i, r) in reach.iter_mut().enumerate() {
                    if !has_lane(f, st, i + 2) {
                        *r = r.min(if k == 0 { 0.0 } else { s });
                    }
                }
            }
        }
        (reach, s)
    }

    /// 0x412500 for a traffic car outside a drag race (verified; the car's
    /// +0x24 is not 2, so the reach tests apply): lane `to` (2–5) exists
    /// where the car is (0x40AE90 on its walk point) and either reaches the
    /// walk's end (0x40AA90: the last point's s at most its reach) or
    /// reaches at least 25 m (0x41C80000) farther than lane `from`
    /// (0x40AF30). In a drag race 0x412500 also tests each lane between
    /// (0x40AF30 with −0.1 and 0x40B110): **not ported**.
    pub(super) fn lane_allowed(&self, f: &RouteFile, from: usize, to: usize) -> bool {
        let Some(&s0) = self.steps.front() else { return false };
        if !(2..=5).contains(&to) || !has_lane(f, s0, to) {
            return false;
        }
        let (reach, last) = self.lane_reach(f);
        let r = |l: usize| if (2..=5).contains(&l) { reach[l - 2] } else { LANE_REACH_NONE };
        last <= r(to) || r(to) - r(from) >= LANE_REACH_MORE
    }

    /// The walk's step from point `i` to the next ([`gap`]); the last
    /// point's own segment.
    pub(super) fn seg(&self, f: &RouteFile, i: usize) -> f32 {
        match (self.steps.get(i), self.steps.get(i + 1)) {
            (Some(&a), Some(&b)) => gap(f, a, b),
            (Some(a), None) => f.segment(a.record, a.cp, a.backward),
            _ => 0.0,
        }
    }

    /// The walk as the original holds it (verified, 2026-11-05; observed
    /// under Wine, docs/world-systems-parity.md §8): a FuturePoint per
    /// control point (+0x58 = 0) and, before each next one, 0x418DA0's
    /// trunc(segment / 8) − 1 points every 8 m (0x783C68) along the same
    /// control point (+0x54 = its s + 8k, +0x58 = 8k, +0x74 = 1), then
    /// 0x41A180's smoothing: between two control-point points A and B with
    /// such points between, per lane (0, 2–5) a cubic Hermite (0x574840,
    /// 0x574C40) from A's place on the lane to B's, the tangents 0.75 × (s_B
    /// − s_A) long (0x43CEB0): at A the chord from the walk point before A
    /// (at the walk's first point its vt+0x20: the control point's direction
    /// against the travel), at B the chord to the next control-point point
    /// (none: B's travel direction, vt+0x1C); each point between takes as its
    /// lane offset (+0x04 lane 0, +0x08 + 4·(lane − 2)) the curve's lateral
    /// off its line at u = (s − s_A) / (s_B − s_A) less the lane's offset
    /// (0x4123C0). So 0x4123C0's laterals are taken off a smooth lane, not
    /// off the control point's straight line. The control points' own
    /// offsets stay 0 for a traffic walk (0x426810 starts them at 0,
    /// 0x422220 carries them on and 0x40AA20 only decays lane 0's).
    pub(super) fn walk_points(&self, f: &RouteFile) -> Vec<WalkPoint> {
        let mut walk = Vec::with_capacity(self.steps.len() * 2);
        let mut s = 0.0;
        for (i, st) in self.steps.iter().enumerate() {
            walk.push(WalkPoint { step: i, d: 0.0, s, x: [0.0; 5] });
            let seg = self.seg(f, i);
            let _ = st;
            if i + 1 < self.steps.len() {
                for k in 1..=sub_points(seg) {
                    let d = SUB_POINT * k as f32;
                    walk.push(WalkPoint { step: i, d, s: s + d, x: [0.0; 5] });
                }
            }
            s += seg;
        }
        let anchors: Vec<usize> = (0..walk.len()).filter(|&i| walk[i].d == 0.0).collect();
        let scale =
            |v: Vec2, len: f32| if v.length_squared() > 0.0 { v.normalize() * len } else { Vec2::new(1.0, 0.0) };
        for j in 0..anchors.len().saturating_sub(1) {
            let (a, b) = (anchors[j], anchors[j + 1]);
            if b - a < 2 {
                continue;
            }
            let span = walk[b].s - walk[a].s;
            let len = SPLINE_TANGENT * span;
            let (sa, sb) = (self.steps[walk[a].step], self.steps[walk[b].step]);
            for li in 0..WALK_LANES.len() {
                let p0 = self.walk_place(f, &walk[a], li);
                let p1 = self.walk_place(f, &walk[b], li);
                let t0 = if a == 0 {
                    scale(f.direction(sa.record, sa.cp, !sa.backward), len)
                } else {
                    scale(p0 - self.walk_place(f, &walk[a - 1], li), len)
                };
                let t1 = match anchors.get(j + 2) {
                    Some(&c) => scale(self.walk_place(f, &walk[c], li) - p1, len),
                    None => scale(f.direction(sb.record, sb.cp, sb.backward), len),
                };
                let c4 = t0 + t1 - 2.0 * (p1 - p0);
                let c5 = 3.0 * (p1 - p0) - t1 - 2.0 * t0;
                for m in a + 1..b {
                    let u = (walk[m].s - walk[a].s) / span;
                    let h = ((c4 * u + c5) * u + t0) * u + p0;
                    let st = self.steps[walk[m].step];
                    let cp = &f.records[st.record].points[st.cp];
                    walk[m].x[li] = f.lateral(st.record, st.cp, st.backward, h)
                        - lane_offset(cp, lane(WALK_LANES[li], st.backward));
                }
            }
        }
        walk
    }

    /// A walk point's place on lane `WALK_LANES[li]` (0x4122C0 at its own
    /// +0x54: +0x58 along its control point's direction, the lane's offset
    /// plus the point's own offset to the right), 2D.
    pub(super) fn walk_place(&self, f: &RouteFile, w: &WalkPoint, li: usize) -> Vec2 {
        let st = self.steps[w.step];
        let cp = &f.records[st.record].points[st.cp];
        f.point(st.record, st.cp, st.backward, w.d, lane_offset(cp, lane(WALK_LANES[li], st.backward)) + w.x[li])
            .truncate()
    }
}
