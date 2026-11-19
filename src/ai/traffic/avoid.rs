use super::*;

/// Another object on the road as the traffic's road entries see it (a node
/// of 0x8A2CA4's list: +0x0C its position, +0x10 its velocity, +0x18 its
/// forward vector, +0x1C the car, whose collision shape gives the box):
/// where it is, how it moves and, for a traffic car, its slot in the pool.
#[derive(Clone, Copy, Debug)]
pub struct Other {
    pub position: Vec2,
    /// Its height (0x4198C0 weighs the walk point's z off it ×10).
    pub z: f32,
    pub velocity: Vec2,
    /// The way it faces (node +0x18).
    pub forward: Vec2,
    /// Half its collision shape's box (0x5AACE0; a hull is shape type 1,
    /// the bounds of its vertices, 0x5A1A00): x along the car, y across.
    pub half: Vec2,
    pub traffic: Option<usize>,
    /// A human racer's car (its controller kind, car +0x14 → +4, is 1:
    /// 0x614890).
    pub human: bool,
    /// The car's identity across frames (the avoidance tracks one by it,
    /// action +0x218).
    pub id: usize,
}

impl Other {
    /// Half the box of a hull's vertices (car space, x forward, y left).
    pub fn half_box(hull: &[Vec3]) -> Vec2 {
        if hull.is_empty() {
            return Vec2::ZERO;
        }
        let (lo, hi) = hull.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), v| {
            (lo.min(v.truncate()), hi.max(v.truncate()))
        });
        (hi - lo) * 0.5
    }
}

/// "Avoid Cars as a traffic car" (type 2, vtable 0x785950): the object it
/// tracks (+0x218, by `Other::id`), the time to it when it took it
/// (+0x21C) and its look (+0x220).
#[derive(Clone, Copy, Debug)]
pub(super) struct Avoid {
    pub(super) target: usize,
    pub(super) time: f32,
    pub(super) look: f32,
    /// The speed it asks for (+0x1F0; probe use).
    pub(super) asked: f32,
}

/// One of the controller's road entries (0x6148F0, 0x30 bytes at planner
/// +0x08, the planner being controller +0x54; verified): the object's
/// velocity in the frame of its own walk point less the car's in the frame
/// of the car's (each rotated by 0x4000 − the point's angle): +0x00 to the
/// right, +0x04 along
/// (closing while negative); its box's half extents across (+0x08) and
/// along (+0x0C) the car's heading — cos²Δ·half y + sin²Δ·half x and the
/// other way round, Δ the object's forward against the car's heading
/// (Cos16 0x43C370); its place along the walk (+0x10, s) and
/// its lateral (+0x14 / +0x18 + 4·lane: off each lane, read through
/// 0x60BFB0), both at the walk point nearest the object (0x419B20 →
/// 0x4198C0: by the 2D distance to the point's place on the lane plus 10
/// × the height difference; s the point's +0x54, the walk's distance —
// prov: inferred ai.traffic.half_box
/// its +0x58 taken as 0, **inferred** — plus the object's distance along
/// it (vt+0x2C), the lateral the point's vt+0x34 less the lane's offset,
/// 0x4123C0).
/// **Observed** under Wine (2026-10-31, the entries a think leaves: the
/// think's end 0x41BF80 clears the count, not the entries): 0.91 / 2.30
/// and 0.92 / 2.02 for cars in line, 2.04 / 1.18 for one across.
#[derive(Clone, Copy, Debug)]
pub(super) struct Entry {
    pub(super) side: f32,
    pub(super) closing: f32,
    pub(super) across: f32,
    pub(super) along: f32,
    pub(super) s: f32,
    /// The walk point it is placed at, and its lateral off that point's
    /// line (right positive).
    pub(super) step: usize,
    /// The walk point's metres along its control point (+0x58).
    pub(super) d: f32,
    pub(super) lateral: f32,
    /// The walk point's per-lane offsets (FuturePoint +0x04, +0x08..+0x14:
    /// lane 0, lanes 2–5), which 0x4123C0 subtracts.
    pub(super) x: [f32; 5],
    pub(super) other: Other,
}

/// What 0x402ED0 returns for the object it takes: the object, its time
/// (param 5) and its distance (param 6).
#[derive(Clone, Copy, Debug)]
pub(super) struct Seen {
    pub(super) id: usize,
    pub(super) time: f32,
    pub(super) dist: f32,
}

/// The road entries take the objects within max(125 m (0x7E8E60), 4 s
/// (controller slot +0x60, 0x408C70 → 0x783B80) × the car's speed) of the
/// car (0x404910). 0x403A40's second chance for an object further off and
// prov: inferred ai.traffic.ENTRY_REACH
/// the cap of 15 entries are not ported (**inferred**: the clone's pools
/// stay under it).
pub(super) const ENTRY_REACH: f32 = 125.0;

/// 0x40F4A0 (verified): the lane (2–5) whose offset at a control point
/// (0x401C40) lies nearest the car's lateral off it (forward its own
/// direction, backward 0x401AA0's, right positive), among the lanes the
/// record has that way (+0x82 + lane: 2–5 forward, 6–9 backward); none when
/// the record has none. The controller's direction byte (+0x10) that picks
// prov: inferred ai.traffic.standing_lane
/// the side is taken as the point's travel direction (**inferred**).
pub(super) fn standing_lane(f: &RouteFile, st: Step, p: Vec2) -> Option<usize> {
    let cp = &f.records[st.record].points[st.cp];
    let lat = f.lateral(st.record, st.cp, st.backward, p);
    (2..=5usize).filter(|&l| has_lane(f, st, l)).min_by(|&a, &b| {
        (lane_offset(cp, lane(a, st.backward)) - lat)
            .abs()
            .total_cmp(&(lane_offset(cp, lane(b, st.backward)) - lat).abs())
    })
}

/// 0x422220's least step for a new walk point (0x783C7C).
pub(super) const MIN_STEP: f32 = 3.0;

/// 0x4198C0 (flags 0, from point 0): the place with the least 2D distance
/// (clamped to 0..99999) plus 10 × |its control point's height − z|; after
/// a point d metres off it skips ⌊0.02·d⌋ − 2 more when that is positive
/// (0x75BED4 on d × −0.02, 0x7852AC).
pub(super) fn nearest_place(places: &[(Vec2, f32)], p: Vec2, z: f32) -> usize {
    let (mut i, mut best, mut at) = (0usize, f32::MAX, 0usize);
    while i < places.len() {
        let (q, qz) = places[i];
        let d = q.distance(p).clamp(0.0, 99_999.0);
        let m = d + 10.0 * (qz - z).abs();
        if m < best {
            best = m;
            at = i;
        }
        i += 1 + ((0.02 * d) as usize).saturating_sub(2);
    }
    at
}

pub(super) const ENTRY_REACH_TIME: f32 = 4.0;

/// 0x402ED0's side window: both objects' +0x08 plus this.
pub(super) const WINDOW_SLACK: f32 = 0.5;

/// Under 0.1 m/s an object within 20 m is reached at once (0x402ED0).
pub(super) const STANDING_REACH: f32 = 20.0;

/// No time (0x47C34F80).
pub(super) const NEVER: f32 = 99_999.0;

/// 0x402CC0 (object, car): the object in the car's frame (0x591FD0 on
/// 0x591DF0's matrix: the velocity's direction over 1 m/s, reversed while
// prov: inferred ai.traffic.box_ahead
/// the car backs up (**inferred** for its +0x3EC test), else the heading):
/// ahead (≥ 0) and within cos²Δ·half y + sin²Δ·half x of the object plus
/// the car's half width across, its distance is the forward less the
/// object's and the car's half lengths (≥ 0), else none (99999); the car's
// prov: inferred ai.traffic.box_ahead-2
/// box (its body +0xE0 / +0xF0) is taken as its hull's (**inferred**).
/// Over 0.1 m/s with the relative velocity's part along the car's velocity
/// pointing back (and over 0.001 m/s) the time is the distance over it.
pub(super) fn box_ahead(own: &Other, speed: f32, e: &Entry) -> (f32, f32) {
    let o = &e.other;
    let v = own.velocity;
    let mut u = if v.length_squared() > 1.0 { v.normalize() } else { own.forward.normalize_or_zero() };
    if v.length_squared() > 1.0 && v.dot(own.forward) < 0.0 {
        u = -u;
    }
    let d = o.position - own.position;
    let (fwd, lat) = (d.dot(u), d.dot(Vec2::new(u.y, -u.x)));
    let c = o.forward.normalize_or_zero().dot(own.forward.normalize_or_zero());
    let k = c * c;
    let mut dist = NEVER;
    if fwd >= 0.0 && lat.abs() <= k * o.half.y + (1.0 - k) * o.half.x + own.half.y {
        dist = (fwd - o.half.x - own.half.x).max(0.0);
    }
    let mut time = NEVER;
    if speed > 0.1 && v.length_squared() > 0.0 {
        let a = (o.velocity - v).dot(v) / v.length_squared();
        let along = v * a;
        if along.dot(v) < 0.0 && along.length() > 0.001 {
            time = dist / along.length();
        }
    }
    (dist, time)
}

/// "Avoid Cars as a traffic car" (0x424CB0): its look (+0x220, 30 m,
/// 0x41F00000 when it takes a car) and the time to reach the car under
/// which it slows (1.5 s).
pub(super) const AVOID_LOOK: f32 = 30.0;

pub(super) const AVOID_TIME: f32 = 1.5;

/// The avoidance goes on while the tracked car pulls away by no more than
/// this (0x7843E8, 5 m/s) and keeps a car that reaches it within this
/// (0x783740, 5 s); it changes lanes while the tracked car closes at less
/// than 40 m/s (0x783750) and grows its look while under 0.5 m/s.
pub(super) const AVOID_AWAY: f32 = 5.0;

pub(super) const AVOID_LANE_TIME: f32 = 5.0;

pub(super) const AVOID_CLOSING: f32 = 40.0;

pub(super) const AVOID_STANDING: f32 = 0.5;

impl TrafficLane {
    /// The road entries (0x6148F0, run by the path build 0x428720 on each
    /// think): the car's own and every object within the reach
    /// ([`ENTRY_REACH`], 3D from the car, 0x61498C..0x614C17). The relative
    /// velocity (+0x00 right, +0x04 along) is each one's velocity turned
    /// into the frame of its own walk point (0x419C10's, 0x614D78..
    /// 0x614EA7: 0x40ADD0, the point's control point's direction of travel)
    /// less the car's turned into the frame of the car's point on its lane
    /// (0x419BC0, 0x6149E5..0x614ADF; verified 2026-11-19 — the clone once
    /// turned both by the walk's first control point, which on a bend made
    /// an oncoming car's closing speed and side wrong). 0x6148F0's race-coordinate test (|0x5D5CE0| ≤ 3 × the reach) is
    /// not applied (the clone's traffic carries no +0x8FC; it reads 0 in
    // prov: inferred ai.traffic.entries
    /// free roam, **inferred** for races).
    pub(super) fn entries(&self, f: &RouteFile, own: &Other, speed: f32, others: &[Other]) -> Vec<Entry> {
        let Some(&s0) = self.steps.front() else { return Vec::new() };
        let reach = ENTRY_REACH.max(ENTRY_REACH_TIME * speed);
        let heading = own.forward.normalize_or_zero();
        // The walk from the car's own point (its first).
        let mut walk = self.walk_points(f);
        let first = walk.iter().position(|w| w.step == 0).unwrap_or(0) + self.lead.0;
        walk.drain(..first.min(walk.len().saturating_sub(1)));
        let places = self.walk_places(f, &walk);
        // A walk point's frame: (right, along) by its control point's
        // direction of travel.
        let frame = |w: &WalkPoint, v: Vec2| {
            let st = self.steps[w.step];
            let d = f.direction(st.record, st.cp, st.backward);
            Vec2::new(v.dot(Vec2::new(d.y, -d.x)), v.dot(d))
        };
        // The car's point on its own lane (0x419BC0 with the planner's lane).
        let li = WALK_LANES.iter().position(|&l| l == s0.lane).unwrap_or(0);
        let lane_places: Vec<(Vec2, f32)> =
            walk.iter().zip(&places).map(|(w, p)| (self.walk_place(f, w, li), p.1)).collect();
        let mine = frame(&walk[nearest_place(&lane_places, own.position, own.z)], own.velocity);
        let entry = |o: &Other| {
            let c = o.forward.normalize_or_zero().dot(heading);
            let k = c * c;
            let at = nearest_place(&places, o.position, o.z);
            let w = walk[at];
            let st = self.steps[w.step];
            let rel = frame(&w, o.velocity) - mine;
            Entry {
                side: rel.x,
                closing: rel.y,
                across: k * o.half.y + (1.0 - k) * o.half.x,
                along: (1.0 - k) * o.half.y + k * o.half.x,
                // +0x54 − +0x58 + the distance along its control point.
                s: w.s - w.d + f.along(st.record, st.cp, st.backward, o.position),
                step: w.step,
                d: w.d,
                lateral: f.lateral(st.record, st.cp, st.backward, o.position),
                x: w.x,
                other: *o,
            }
        };
        let out: Vec<Entry> = std::iter::once(entry(own))
            .chain(
                others
                    .iter()
                    .filter(|o| o.position.extend(o.z).distance(own.position.extend(own.z)) <= reach)
                    .map(entry),
            )
            .collect();
        #[cfg(feature = "dev-harness")]
        probe::record(self, f, own.id, &out);
        out
    }

    /// Each walk point's place on lane 0 (0x4122C0 at its +0x54, lane 0:
    /// the lane's offset plus its +0x04) and its control point's height.
    pub(super) fn walk_places(&self, f: &RouteFile, walk: &[WalkPoint]) -> Vec<(Vec2, f32)> {
        walk.iter()
            .map(|w| {
                (
                    self.walk_place(f, w, 0),
                    f.records[self.steps[w.step].record].points[self.steps[w.step].cp].position.z,
                )
            })
            .collect()
    }

    /// An entry's lateral off lane `l` (0x60BFB0: off the lane's offset at
    /// its walk point).
    pub(super) fn off_lane(&self, f: &RouteFile, e: &Entry, l: usize) -> f32 {
        let st = self.steps[e.step.min(self.steps.len() - 1)];
        let x = match l {
            0 => e.x[0],
            2..=5 => e.x[l - 1],
            _ => 0.0,
        };
        e.lateral - lane_offset(&f.records[st.record].points[st.cp], lane(l, st.backward)) - x
    }

    /// 0x402ED0 (lane, lateral, look): over the entries ahead of the car
    /// (0x6148F0 sorts them by the signed squared distance, descending, so
    /// the walk from the car's own index down takes the nearest ahead
    /// first; with the car's s at most theirs), the first in the side
    /// window — its lateral off the lane within its +0x08 + 0.5 m + the
    /// car's +0x08 of the car's own (or with `on` that lane, centred:
    /// 0x4146C0 passes 0) — with a time under 99999 or 0. Its gap is
    /// bumper to bumper (s less both +0x0C, at least 0), its time the
    /// lesser of gap / |+0x04| (closing under −0.01) and 0x402CC0's
    /// ([`box_ahead`]); 0 when the gap or 0x402CC0's distance is within
    /// the look, or within 20 m with the car under 0.1 m/s; an object in
    /// the window with neither is passed over. The distance it hands back
    /// is the lesser of the two.
    pub(super) fn car_ahead(
        &self,
        f: &RouteFile,
        own: &Other,
        speed: f32,
        entries: &[Entry],
        on: Option<usize>,
        look: f32,
    ) -> Option<Seen> {
        let s0 = *self.steps.front()?;
        let me = entries.first()?;
        let l = on.unwrap_or(s0.lane);
        let side = if on.is_some() { 0.0 } else { self.off_lane(f, me, l) };
        let mine = self.off_lane(f, me, s0.lane);
        let mut ahead: Vec<(f32, &Entry)> = entries[1..]
            .iter()
            .filter(|e| e.s >= me.s)
            .map(|e| ((e.s - me.s).powi(2) + (self.off_lane(f, e, s0.lane) - mine).powi(2), e))
            .collect();
        ahead.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, e) in ahead {
            if (self.off_lane(f, e, l) - side).abs() > e.across + WINDOW_SLACK + me.across {
                continue;
            }
            let gap = (e.s - e.along - me.s - me.along).max(0.0);
            let t1 = if e.closing < -0.01 { gap / e.closing.abs() } else { NEVER };
            let (d2, t2) = box_ahead(own, speed, e);
            let dist = d2.min(gap);
            if gap <= look || d2 <= look || ((gap <= STANDING_REACH || d2 <= STANDING_REACH) && speed < 0.1) {
                return Some(Seen { id: e.other.id, time: 0.0, dist });
            }
            let time = t1.min(t2);
            if time < NEVER {
                return Some(Seen { id: e.other.id, time, dist });
            }
        }
        None
    }

    /// The tracked object's entry (0x60C130: among the road entries, so
    /// ahead or behind, within the reach).
    pub(super) fn tracked<'a>(entries: &'a [Entry], id: usize) -> Option<&'a Entry> {
        entries.iter().skip(1).find(|e| e.other.id == id)
    }

    /// The avoidance's end (vtable slot +0x18, 0x41CD10): done once the
    /// tracked object is gone from the entries; while it is ahead (0x60C000
    /// ≥ 0) and not pulling away by more than 5 m/s (its +0x04 at most
    /// 0x7843E8) it goes on; otherwise done unless 0x402ED0 on the lane
    /// with the action's look finds an object reaching in 5 s or less
    /// (0x783740) that 0x4140E0 keeps, which it then tracks (a new one:
    /// look 30 m, its time). 0x4140E0's drop of a car crossing at an
    /// intersection never applies here (no intersection ahead).
    pub(super) fn avoid_done(&mut self, f: &RouteFile, own: &Other, speed: f32, entries: &[Entry], a: Avoid) -> bool {
        let Some(t) = Self::tracked(entries, a.target) else { return true };
        if t.s - entries[0].s >= 0.0 && t.closing <= AVOID_AWAY {
            return false;
        }
        match self.car_ahead(f, own, speed, entries, None, a.look) {
            Some(s) if s.time <= AVOID_LANE_TIME => {
                if s.id != a.target {
                    self.avoid = Some(Avoid { target: s.id, time: s.time, look: AVOID_LOOK, asked: MOVER_TOP_SPEED });
                }
                false
            }
            _ => true,
        }
    }

    /// 0x4146C0: whether the car may take lane `l` (2–5): 0x412500 from
    /// the car's lane ([`TrafficLane::lane_allowed`]: the lane exists where
    /// it is and reaches the walk's end or 25 m farther) and 0x402ED0 on it (lateral 0, the action's look)
    /// finds nothing, or an object 0x4140E0 drops, or one reaching in over
    /// 5 s — or one that does is not the tracked one and reaches later than
    /// it (+0x21C).
    pub(super) fn lane_free(
        &self,
        f: &RouteFile,
        own: &Other,
        speed: f32,
        entries: &[Entry],
        a: Avoid,
        l: usize,
    ) -> bool {
        let Some(&s0) = self.steps.front() else { return false };
        if !(2..=5).contains(&l) || (l != s0.lane && !self.lane_allowed(f, s0.lane, l)) {
            return false;
        }
        match self.car_ahead(f, own, speed, entries, Some(l), a.look) {
            Some(s) if s.time <= AVOID_LANE_TIME => s.time > a.time && s.id != a.target,
            _ => true,
        }
    }

    /// Moves the car to lane `l` (0x405410 on the controller's lane
    /// +0x30C): the walk's points keep it where their records have it
    /// (0x415B70); the path then eases over at the lateral rate.
    pub(super) fn set_lane(&mut self, f: &RouteFile, l: usize) {
        for s in self.steps.iter_mut() {
            s.lane = keep_lane(f, *s, l);
        }
        // Planned afresh: 0x405410 sets the lane state's rate (+0x08) to
        // 0.151 (0x7E8E68), and 0x41BFE0 plans afresh while it reads that
        // (verified in both; that 0x41BFE0's argument is the planner's
        // prov: inferred ai.traffic.set_lane
        // +0x30C is **inferred** from its use as the lane).
        self.kept.clear();
    }

    /// "Avoid Cars as a traffic car"'s update (0x424CB0). While the
    /// tracked object is among the entries, is a human's car (0x614890) and
    /// closes at under 40 m/s (its
    /// +0x04 above −0x783750) the lane changes: 0x4146C0 on the lane, then
    /// ±1 and ±2, −1 first when its +0x00 (the relative velocity to the
    /// right) is positive, else +1; the first it may take is set
    /// (0x405410, the car's own lane too), and when none is the car is blocked. Then 0x402ED0 with
    /// the look on its lane: an object found becomes the tracked one (a new
    /// one: look 30 m) with its time (+0x21C); standing (under 0.5 m/s) the
    /// look grows to at least 30 m and the distance (and 0x40F4A0 asks for
    /// a lane: **not ported**). The speed (+0x1F0): with nothing found the
    /// cap, or the slow rule at distance 0 when blocked; with an object the
    /// slow rule when its time is under 1.5 s or the tracked object's gap
    /// (bumper to bumper, before the update; 0 unless the
    /// lane changes ran) is within the look, else the cap. The slow rule: 0 within 15 m (0x78374C), else 6 m/s
    /// (0x40C00000). Returns the limit.
    pub(super) fn avoid_update(&mut self, f: &RouteFile, own: &Other, speed: f32, entries: &[Entry]) -> f32 {
        let Some(mut a) = self.avoid else { return MOVER_TOP_SPEED };
        let Some(&s0) = self.steps.front() else { return MOVER_TOP_SPEED };
        let Some(me) = entries.first() else { return MOVER_TOP_SPEED };
        let mut blocked = false;
        let mut tracked_gap = 0.0;
        // Only for a tracked human's car (0x614890: its kind is 1) — so a
        // car avoiding traffic or a racer never changes lanes, and its
        // tracked gap stays 0, within any look: the slow rule holds it at
        // 6 m/s behind whatever 0x402ED0 finds (**observed** under Wine:
        // the queue's cars track traffic cars 45–125 m off at +0x1F0 = 6).
        if let Some(t) = Self::tracked(entries, a.target)
            && t.other.human
            && t.closing > -AVOID_CLOSING
        {
            tracked_gap = (t.s - me.s - t.along - me.along).max(0.0);
            let l = s0.lane as isize;
            let order = if t.side > 0.0 { [l, l - 1, l + 1, l - 2, l + 2] } else { [l, l + 1, l - 1, l + 2, l - 2] };
            match order
                .into_iter()
                .filter(|&c| c >= 0)
                .map(|c| c as usize)
                .find(|&c| self.lane_free(f, own, speed, entries, a, c))
            {
                // 0x424DFB (verified): 0x405410 also for the lane the car
                // is on, so while the tracked human's car is avoided the
                // path is planned afresh on every think.
                Some(c) => self.set_lane(f, c),
                None => blocked = true,
            }
        }
        let near = self.car_ahead(f, own, speed, entries, None, a.look);
        let slow = |dist: f32| if dist <= AHEAD_STOP { 0.0 } else { CROSSING_CRAWL };
        let limit = match near {
            None if blocked => slow(0.0),
            None => MOVER_TOP_SPEED,
            Some(s) => {
                if s.id != a.target {
                    a.target = s.id;
                    a.look = AVOID_LOOK;
                }
                a.time = s.time;
                if speed < AVOID_STANDING {
                    a.look = a.look.max(AVOID_LOOK).max(s.dist);
                    // 0x424F24..0x424F5A (verified): standing, the car takes
                    // the lane it stands in (0x40F4A0 on its road entry's walk
                    // point's control point, 0x401ED0) through 0x405410, which
                    // also starts the lane's offset afresh.
                    let st = self.steps[me.step.min(self.steps.len() - 1)];
                    if let Some(l) = standing_lane(f, st, own.position) {
                        self.set_lane(f, l);
                    }
                }
                if s.time < AVOID_TIME || tracked_gap <= a.look { slow(s.dist) } else { MOVER_TOP_SPEED }
            }
        };
        a.asked = limit;
        self.avoid = Some(a);
        limit
    }
}

/// The road entries each car's last think left, in the original's
/// `trafwalk.py` entries format (probe use).
#[cfg(feature = "dev-harness")]
pub mod probe {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static LAST: RefCell<HashMap<usize, String>> = RefCell::new(HashMap::new());
    }

    pub(super) fn record(t: &TrafficLane, f: &RouteFile, id: usize, es: &[Entry]) {
        let Some(s0) = t.steps.front() else { return };
        let mut line = format!("L{}", s0.lane);
        for (k, e) in es.iter().enumerate() {
            let who = if k == 0 { "*".to_string() } else { format!("{}", e.other.id as i64) };
            line += &format!(
                " {k}={who}:{:.1}:{:.2}:{:.2}:{:.2}:{:.2}:{:.2}:{}:{:.0}:{:.2}:{:.2}",
                e.s,
                t.off_lane(f, e, s0.lane),
                e.across,
                e.along,
                e.closing,
                e.side,
                e.step,
                e.d,
                e.x[s0.lane - 1],
                e.lateral
            );
        }
        // The car's own walk point's place on its lane.
        if let Some(e) = es.first() {
            let st = t.steps[e.step.min(t.steps.len() - 1)];
            let cp = &f.records[st.record].points[st.cp];
            let q = f.point(st.record, st.cp, st.backward, e.d, lane_offset(cp, lane(s0.lane, st.backward)) + e.x[s0.lane - 1]);
            line += &format!(" @{:.2},{:.2}", q.x, q.y);
        }
        LAST.with(|m| m.borrow_mut().insert(id, line));
    }

    /// The car's last entries (`id` its `Other::id`), taken.
    pub fn take(id: usize) -> Option<String> {
        LAST.with(|m| m.borrow_mut().remove(&id))
    }
}
