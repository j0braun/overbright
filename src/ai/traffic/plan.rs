use super::*;

/// How fast the path's lateral offset returns to the lane's centre, metres
/// per metre along the road: the lane record's rate, which the traffic
/// controller's lane setter (slot +0x20, 0x40D4A0) sets to 0.151
/// (0x7837D8) with the offset 0 (0x40FC80 moves it by rate × the point's
/// segment).
pub(super) const LATERAL_RATE: f32 = 0.151;

/// Traffic lane speeds change by at most this much per metre along the
/// walk, both ways (0x40A8C0 / 0x40A6E0 on traffic lanes).
pub(super) const LANE_SPEED_SLOPE: f32 = 0.2;

/// 0x41C380's look ahead for the second lane speed: 1.3 s (0x78355C) of
/// the car's speed, at least 8 m (0x783560).
pub(super) const SPEED_LOOK_TIME: f32 = 1.3;

pub(super) const SPEED_LOOK_MIN: f32 = 8.0;

/// A walk considers a turn only this far (m) past its last one (0x41EC20:
/// 30 < the last point's s − walk +0x354).
pub(super) const TURN_GAP: f32 = 30.0;

/// 0x41BFE0's fresh-plan distance (0x7E8D94, 1.5 m) and 0x420D40's kept
/// length: |v| × 0.5 (0x7837E0), at least 30 m (0x7837E8).
pub(super) const REPLAN: f32 = 1.5;

pub(super) const KEEP_RATE: f32 = 0.5;

pub(super) const KEEP_MIN: f32 = 30.0;

/// A walk point's lane room as 0x41FD50 seeds it from slots +0x0C / +0x10
/// (forward 0x40B2A0 / 0x40B1F0, backward 0x40B840 / 0x40B780): from the
/// lane's position (0x401C40, plus the point's adjustment, 0 for traffic)
/// to the road's left and right edges, the half widths swapped backward.
/// 0x41FD50's pass from the far end back, which also limits each point's
// prov: open ai.traffic.room
/// room by the next one's, is **open** (its decompile is garbled) and not
/// applied.
pub(super) fn room(f: &RouteFile, s: Step) -> (f32, f32) {
    let cp = &f.records[s.record].points[s.cp];
    let c = lane_offset(cp, lane(s.lane, s.backward));
    let [l, r] = if s.backward { [cp.half_width[1], cp.half_width[0]] } else { cp.half_width };
    (-l - c, r - c)
}

/// 0x41D530's margin to the lane room: 1 m (0x420D40 with a rate other
/// than 999; 5 m in some race modes) plus the walk's +0x10 + 0x30·(+0x308)
// prov: open ai.traffic.LANE_MARGIN
/// float (**open**, taken as 0).
pub(super) const LANE_MARGIN: f32 = 1.0;

/// A walk point's slot +0x24 (forward class 0x784A48 → 0x40D0B0, backward
/// 0x784AA0 → 0x40D300): the control point's grade byte +0x21 / 128 in the
/// direction of travel; backward the negated grade of the point before it
/// in the record (0x5D8A00; the point's own when it is the first).
pub(super) fn grade(f: &RouteFile, s: Step) -> f32 {
    let points = &f.records[s.record].points;
    if s.backward { -points[s.cp.saturating_sub(1)].grade } else { points[s.cp].grade }
}

/// 0x41C380's crest cap (0x411FA0, then table 0x7E9BC0 through 0x40C840):
/// over consecutive walk points from the car's while the first lies within
/// `look` (max(1.3 s × speed, 20 m)) along the walk, the least grade change
/// (next − this) and where it starts; when that is under −0.1 (the road
/// tips over a crest) the cap is pulled to 10 m/s by the table (10 m → 1,
/// 40 m → 0 at the place, linear): (cap − 10)·(1 − t) + 10. `at` holds the
/// points' distances along the walk (0x411FA0 compares the walk points'
/// +0x54 with `look` as they are; the clone's walk starts at the car's
/// point).
pub(super) fn crest_cap(cap: f32, grades: &[f32], at: &[f32], look: f32) -> f32 {
    let mut least = (99_999.0f32, 99_999.0f32);
    for k in 0..grades.len().saturating_sub(1) {
        if look < at[k] {
            break;
        }
        let d = grades[k + 1] - grades[k];
        if d < least.0 {
            least = (d, at[k]);
        }
    }
    if least.0 >= CREST_DROP {
        return cap;
    }
    let t = ((CREST_TABLE[1].0 - least.1) / (CREST_TABLE[1].0 - CREST_TABLE[0].0)).clamp(0.0, 1.0);
    (cap - CREST_SPEED) * (1.0 - t) + CREST_SPEED
}

/// 0x41C380's crest look: 1.3 s (0x783564) of speed, at least 20 m
/// (0x783568); a grade change under −0.1 (0x78356C) counts; the cap goes
/// towards 10 m/s (0x783570) by table 0x7E9BC0 ({10 m, 1}, {40 m, 0} at
/// 0x7E8DB0).
pub(super) const CREST_LOOK_TIME: f32 = 1.3;

pub(super) const CREST_LOOK_MIN: f32 = 20.0;

pub(super) const CREST_DROP: f32 = -0.1;

pub(super) const CREST_SPEED: f32 = 10.0;

pub(super) const CREST_TABLE: [(f32, f32); 2] = [(10.0, 1.0), (40.0, 0.0)];

/// A walk point's lane speed byte (control point +0x2C + lane), m/s.
pub(super) fn speed_byte(f: &RouteFile, s: Step) -> f32 {
    f32::from(f.records[s.record].points[s.cp].lane_speed[lane(s.lane, s.backward)])
}

/// The point `lateral` metres right of a step's lane centre (0x5DD370 with
/// the lane's position, 0x401C40).
pub(super) fn point(f: &RouteFile, s: Step, lateral: f32) -> Vec3 {
    let cp = &f.records[s.record].points[s.cp];
    f.point(s.record, s.cp, s.backward, 0.0, lane_offset(cp, lane(s.lane, s.backward)) + lateral)
}

impl TrafficLane {
    /// One frame: drop the control points the car is past (along the
    /// point's direction beyond its segment) and extend the lane. Returns
    /// the path for the driver and its target speed, or None once the car is past the
    /// walk's last point (0x428720 false: the car stops for good).
    ///
    /// Path (the lane action's update 0x4242D0 → 0x420D40 → 0x41D530, the
    /// path rebuilt each think): a point per walk point ahead — the
    /// control points and the 8 m points between them (0x415D50) — up
    /// to `PATH_POINTS`, on its lane with the point's own offset, the
    /// lateral offset starting at the
    /// car's own off the lane's centre and moving back to it by
    /// `LATERAL_RATE` per metre of segment (0x40FC80; carried over by the
    /// lanes' offset difference where `keep_lane` changes lanes, 0x415B70).
    /// Then the offset is held within the point's lane room (future point
    /// +0x18 / +0x1C.. low, +0x2C / +0x30.. high) less `LANE_MARGIN`, each
    /// bound kept on its side of 0 (0x40FC80): a point the placement's walk
    /// built has the room [`room`] gives; one walked since has none (both
    /// bounds 0), so the path there lies on the lane's centre.
    ///
    /// Target speed (the traffic goal's 0x41C380): the least of 55 mph
    /// (controller vt+0x7C, 0x41E7A0: +0x79C, which the placement sets to
    /// 55 mph, at most 0x7845A4), the lane's speed at the car and at
    /// max(1.3 s × speed, 8 m) ahead (0x411DD0: the lane speeds of the
    /// walk, linear between points); the lane action's +0x1F0 is the same
    /// 55 mph (controller slot +0x7C) with +0x1FC clear. Before that the
    /// 55 mph cap goes through [`crest_cap`] (0x41C45F).
    pub fn update(&mut self, f: &RouteFile, position: Vec3, speed: f32) -> Option<(Vec<Vec3>, f32)> {
        let p = position.truncate();
        // The car's walk point (the road entry's +0x28, 0x419C10 →
        // 0x419B20 → 0x4198C0 with flags 0; verified, 2026-11-01): the walk
        // point whose place on the lane (0x4122C0 at the point's +0x54,
        // lateral 0) is nearest in 2D plus 10 × |its z − the car's z|; the
        // path starts at the point after it (0x41D530: entry +0x28 + 1, at
        // most the last). Until 2026-11-01 the clone moved on only once the
        // car was past the point's segment along its direction, so a car
        // on the MeloMvr that had swung off the lane orbited the next point
        // (inside its turning circle) without ever passing it. The clone
        // drops the points before it (the walk's points behind the car are
        // prov: inferred ai.traffic.update
        // not kept, so the nearest never goes back: **inferred**); 0x4198C0's
        // skip of far points (its step through 0x75BED4) is not modelled
        // prov: inferred ai.traffic.update-2
        // (it only speeds up the search, **inferred**).
        let walk = self.walk_points(f);
        // The walk's first point (verified, 0x428030 → 0x4198C0 → 0x419670):
        // the route manager locates the car on its walk by 0x4198C0 with
        // flags 0 on the car's lane (controller vt+0x1C; lane 0 when not
        // positive), from walk +0xAD, sets +0xAD and for traffic drops the
        // points before it (a controller whose vt+0x28 is true keeps 20, 10
        // in race types 0 / 7). So +0xAD reads 0 (observed) and the walk
        // starts at the car's nearest point, the 8 m points included; with
        // its 10 × height term a car on a slope can sit metres before it
        // (observed under Wine: 4 m before to 12 m past).
        let li = WALK_LANES.iter().position(|&l| l == self.steps.front().map_or(0, |s| s.lane)).unwrap_or(0);
        let places: Vec<(Vec2, f32)> = walk
            .iter()
            .map(|w| {
                (
                    self.walk_place(f, w, li),
                    f.records[self.steps[w.step].record].points[self.steps[w.step].cp].position.z,
                )
            })
            .collect();
        let mut at = nearest_place(&places, p, position.z);
        let popped = walk.get(at).map_or(0, |w| w.step).min(self.steps.len().saturating_sub(1));
        if walk.get(at).is_some_and(|w| w.step != popped) {
            // The walk's last control point: no points of its own after it.
            at = walk.iter().position(|w| w.step == popped).unwrap_or(0);
        }
        for _ in 0..popped {
            self.walked += self.seg(f, 0);
            self.behind = self.steps.pop_front();
            self.roomed = self.roomed.saturating_sub(1);
        }
        // The car's point among its control point's points (those before it
        // are dropped from the walk; `walk_points` still builds them, for the
        // lane offsets' curve).
        let first = walk.iter().position(|w| w.step == popped).unwrap_or(0);
        self.lead = (at.saturating_sub(first), walk.get(at).map_or(0.0, |w| w.d));
        self.extend(f, ENTRY_REACH.max(ENTRY_REACH_TIME * speed));
        if self.steps.len() < 2 {
            return None;
        }
        // The path (0x41D530 → 0x415D50, **verified**): one point per walk
        // point past the car's — the control points' and the 8 m points
        // between them — at most 20 (0x415D50: the path's count byte under
        // 0x14), each 0x4122C0's place: along the point's control point by
        // its +0x58, across by the lane's offset (0x401C40) plus the
        // point's own Hermite offset for the lane (+0x04 / +0x08 + 4·(lane
        // − 2)) plus the path's lateral, which 0x40FC80 moves towards 0 by
        // the rate × the step to each point. Before this fix the clone's path held
        // the control points only (chords across the walk's curves).
        let walk = self.walk_points(f);
        let s0 = self.steps[0];
        let centre = |s: Step| lane_offset(&f.records[s.record].points[s.cp], lane(s.lane, s.backward));
        let lane_x = |w: &WalkPoint| {
            let st = self.steps[w.step];
            WALK_LANES.iter().position(|&l| l == st.lane).map_or(0.0, |li| w.x[li])
        };
        let lead = self.lead.0.min(walk.len().saturating_sub(1));
        let car_lateral = f.lateral(s0.record, s0.cp, s0.backward, p) - centre(s0) - walk.get(lead).map_or(0.0, lane_x);
        let mut lateral = car_lateral;
        let same = |a: Step, b: Step| a.record == b.record && a.cp == b.cp && a.backward == b.backward;
        // 0x420D40 with the driver's +0x1D9 set (traffic: observed): the
        // kept path's front stays unless 0x41BFE0 asks for a fresh plan —
        // the path is empty, or the car's offset off the lane differs from
        // the path's at the car (its first point ahead) by more than 1.5 m
        // (0x7E8D94) and by more than that point's distance ahead (the
        // prov: inferred ai.traffic.update-3
        // racers' rule, `RacerRoute::line_path`; 2D here, **inferred**).
        // Kept: the points behind the car along its heading go (0x404EF0 /
        // 0x40F7A0), the path is cut after the first point max(30, |v| / 2)
        // m on (0x41D160 → 0x415590), and 0x41D530 adds a point per walk
        // point past the last, its offset moving on from the last's. Until
        // 2026-11-04 the clone planned afresh every think from the point
        // after the car's nearest, so a car still short of its nearest
        // point already steered for the next one and cut every bend (the
        // freeroam_stand parity run: 2.5–4 m inside the original's lanes on
        // record 538, oncoming cars meeting within 2–2.6 m and standing in
        // "Avoid Cars" for seconds; docs/world-systems-parity.md).
        // A fresh path starts at the walk point after the car's (0x41D530:
        // the road entry's +0x28 + 1, at most the last).
        let mut from = (lead + 1).min(walk.len().saturating_sub(1));
        let mut path: Vec<Vec3> = Vec::with_capacity(PATH_POINTS);
        let mut kept: Vec<(Step, f32, f32, Vec3)> = Vec::new();
        if let Some(h) = self.heading.map(|h| h.normalize_or_zero()).filter(|h| *h != Vec2::ZERO) {
            let ahead = |q: Vec3| (q.truncate() - p).dot(h) > 0.0;
            let mut old = std::mem::take(&mut self.kept);
            let at = old.iter().position(|k| ahead(k.3)).unwrap_or(old.len().saturating_sub(1));
            let replan = old.get(at).is_none_or(|k| {
                let off = (car_lateral - k.2).abs();
                off > REPLAN && off > k.3.truncate().distance(p)
            });
            if !replan {
                old.drain(..at);
                if let Some(k) = old.iter().position(|k| ahead(k.3)) {
                    old.drain(..k);
                    let keep = (speed.abs() * KEEP_RATE).max(KEEP_MIN);
                    let mut d = (old[0].3.truncate() - p).length();
                    let mut j = 0;
                    while d < keep && j + 1 < old.len() {
                        d += (old[j + 1].3 - old[j].3).truncate().length();
                        j += 1;
                    }
                    old.truncate(j + 1);
                    // Where the walk goes on from the kept path's last point.
                    if let Some(last) = old.last()
                        && let Some(i) =
                            walk.iter().position(|w| same(self.steps[w.step], last.0) && (w.d - last.1).abs() < 1e-3)
                    {
                        lateral = last.2;
                        from = i + 1;
                        path.extend(old.iter().map(|k| k.3));
                        kept = old;
                    }
                }
            }
        }
        let fresh = kept.is_empty();
        if fresh {
            from = (lead + 1).min(walk.len().saturating_sub(1));
        }
        for k in from.max(1)..walk.len() {
            if path.len() >= PATH_POINTS {
                break;
            }
            let (pw, w) = (&walk[k - 1], &walk[k]);
            let (prev, s) = (self.steps[pw.step], self.steps[w.step]);
            if s.lane != prev.lane {
                lateral += centre(Step { lane: prev.lane, ..s }) - centre(s);
            }
            // The step to this point: the walk's s (0x422220's gaps, the
            // 8 m points' +0x58).
            let step = LATERAL_RATE * (w.s - pw.s);
            lateral = if lateral > 0.0 { (lateral - step).max(0.0) } else { (lateral + step).min(0.0) };
            // 0x40FC80's clamp: within the point's room less the margin,
            // the bounds never past 0 (outside the race modes that average
            // crossed bounds instead).
            let (lo, hi) = if w.step < self.roomed { room(f, s) } else { (0.0, 0.0) };
            lateral = lateral.clamp((lo + LANE_MARGIN).min(0.0), (hi - LANE_MARGIN).max(0.0));
            let cp = &f.records[s.record].points[s.cp];
            let q = f.point(
                s.record,
                s.cp,
                s.backward,
                w.d,
                lane_offset(cp, lane(s.lane, s.backward)) + lane_x(w) + lateral,
            );
            path.push(q);
            kept.push((s, w.d, lateral, q));
        }
        self.kept = kept;
        // The lane's speeds along the walk, limited to ±0.2 m/s per metre.
        let mut at = vec![0.0f32; self.steps.len()];
        for k in 1..at.len() {
            at[k] = at[k - 1] + self.seg(f, k - 1);
        }
        let mut v: Vec<f32> = self.steps.iter().map(|&s| speed_byte(f, s)).collect();
        for k in 1..v.len() {
            v[k] = v[k].min(v[k - 1] + LANE_SPEED_SLOPE * (at[k] - at[k - 1]));
        }
        for k in (0..v.len() - 1).rev() {
            v[k] = v[k].min(v[k + 1] + LANE_SPEED_SLOPE * (at[k + 1] - at[k]));
        }
        let speed_at = |s: f32| {
            let k = at.iter().rposition(|&a| a <= s).unwrap_or(0);
            match (at.get(k + 1), v.get(k + 1)) {
                (Some(&a1), Some(&v1)) if a1 > at[k] => {
                    v[k] + (v1 - v[k]) * ((s - at[k]) / (a1 - at[k])).clamp(0.0, 1.0)
                }
                _ => v[k],
            }
        };
        let s = f.along(s0.record, s0.cp, s0.backward, p).max(0.0);
        let ahead = (SPEED_LOOK_TIME * speed).max(SPEED_LOOK_MIN);
        let cap = crest_cap(
            MOVER_TOP_SPEED,
            &self.steps.iter().map(|&s| grade(f, s)).collect::<Vec<_>>(),
            &at,
            (CREST_LOOK_TIME * speed).max(CREST_LOOK_MIN),
        );
        let target = cap.min(speed_at(s)).min(speed_at(s + ahead));
        Some((path, target))
    }
}
