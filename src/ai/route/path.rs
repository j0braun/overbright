use super::*;

/// A point of the JustDrive path: the FuturePoint's distance, the offset
/// off the racing line, the place.
#[derive(Clone, Debug)]
pub(super) struct LinePoint {
    pub(super) s: f32,
    pub(super) lateral: f32,
    pub(super) pos: Vec3,
}

impl RacerRoute {
    /// Whether the driven path was planned afresh (0x420D40's 0x41D160(0,
    /// 0)) since the last call: 0x420810, handing the path to the driver,
    /// then posts driver event 4 (+0x250, 0x4208CA; the trackers skip the
    /// rate for that update). The single-player branch of its test (0x89E7D8,
    /// 0x890108, 0x890114, 0x89E7D9, 0x890104 all clear) always posts.
    pub fn take_replanned(&mut self) -> bool {
        std::mem::take(&mut self.replanned)
    }

    /// The racing-line points of the next points for the driver (≤ 20).
    pub fn path(&self, routes: &AiRoutes) -> Vec<Vec3> {
        self.points
            .iter()
            .filter(|p| p.s > self.distance)
            .take(PATH_POINTS)
            .map(|p| self.racing_point(&routes.file, p))
            .collect()
    }

    /// The point GetUnstuck's slot 6 faces (0x4143F0): 0x4032A0 takes the
    /// list entry (+0x324, count +0x320) `ahead` past the car's point (the
    /// racer's slot +0x308 block's +0x28) — 3, or 1 in a drift or Street X
    /// race (0x89E7D9 / 0x890104 / 0x89E7E3 / 0x89010C) — clamped to the
    /// last, and 0x4122C0 places it at its own distance (+0x54) on the
    /// racer's lane (+0x30C; lane 0 here), 2D.
    pub fn facing_point(&self, routes: &AiRoutes, near: bool) -> Option<Vec2> {
        let i = (self.current + if near { 1 } else { 3 }).min(self.points.len().checked_sub(1)?);
        Some(self.racing_point(&routes.file, &self.points[i]).truncate())
    }

    /// AIActionJustDrive's path (0x42B340 → 0x420D40 → 0x41D530) on its
    /// racing-line lane record {lane 0, offset 0, `rate`}. The rate is 0.2
    /// (0x783680) in Street X (0x89E7E3 or 0x89010C), else 0.05
    /// (0x7E8E64). The path (≤ 20 points, 0x415D50) is kept from frame to
    /// frame (0x420D40 starts from the driven path, car +0x30 → +0x10):
    /// it is planned again from the car (0x41D160(0, 0)) when 0x41BFE0
    /// says so — the path is empty, or the car's offset off the line
    /// (0x60BFB0) differs from the path's at the car (0x419C60) by more
    /// than 1.5 m (0x7E8D94; 4.5 in drift races) and by more than the
    /// path point's distance (0x419CC0 → 0x40AE30) less the car's;
    /// otherwise the points behind the car go (0x404EF0 / 0x40F7A0: the
    /// first point ahead of it along its heading, car +0x60 / +0x70) and
    /// the path is cut after the first point max(30, |v| / 2) m on
    /// (0x41D160 → 0x415590; 0x7837E8 / 0x7837E0). 0x41D530 then adds
    /// a point per FuturePoint past the last (0x4198C0: at least 0.5 m
    /// on), the lateral offset starting at the last point's (0x4123C0) —
    /// or, planned afresh, at the car's from the point after the car's —
    /// and moving to 0 by `rate` per metre (0x40FC80; the step, vt+0x18
    /// 0x40B050, is the distance from the previous FuturePoint), held in the lane's
    /// room less the margin (1 m, 0x7837EC; 5 m in drift races, 0x420DAE)
    /// with the rate dropped to 0 once the room holds the offset against
    /// its motion. Inserted (junction-gap) points keep their place.
    ///
    /// `fresh` plans it from the car regardless (the clone's GetUnstuck,
    // prov: inferred ai.route.facing_point
    /// during which the game runs that action instead; **inferred**: that
    /// JustDrive then starts again from the car).
    ///
    /// `target` is the lane record's offset (+0x30C +0x04): 0x40FC80 moves
    /// the offset towards it, not towards 0 (its fourth argument; 0 for
    /// JustDrive's plain line, the pass / block offsets of the racer
    /// actions, see `ai::actions`).
    #[allow(clippy::too_many_arguments)]
    pub fn line_path(
        &mut self,
        routes: &AiRoutes,
        rate: f32,
        position: Vec3,
        forward: Vec3,
        speed: f32,
        drift: bool,
        fresh: bool,
        target: f32,
    ) -> Vec<Vec3> {
        self.line_path_with(routes, rate, position, forward, speed, drift, fresh, target, KEEP_RATE, false)
    }

    /// Action 0x13 "Drive at offset" (vtable 0x785A18, update 0x42BC50)
    /// near its target: when the driven path's offset at the car (0x419C60)
    /// is within 1.5 m (0x7E8D94) of the car's own off the line (0x60BFB0
    /// of its entry), the path is kept without 0x420D40's replan test: the
    /// driven path (0x401D40), cut after the first point max(0.75 × |v|,
    /// 30 m) on (0x41D160 with 0x783764 / 0x783768), then extended on the
    /// lane record by 0x41D530 with the 1 m margin (0x78375C). Otherwise
    /// it is 0x420D40, [`RacerRoute::line_path`]. **Verified** (dis
    /// 0x42BCE2–0x42BDE6).
    #[allow(clippy::too_many_arguments)]
    pub fn offset_path(
        &mut self,
        routes: &AiRoutes,
        rate: f32,
        position: Vec3,
        forward: Vec3,
        speed: f32,
        drift: bool,
        target: f32,
    ) -> Vec<Vec3> {
        let heading = forward.truncate();
        let at = self
            .line
            .iter()
            .position(|p| (p.pos - position).truncate().dot(heading) > 0.0)
            .unwrap_or(self.line.len().saturating_sub(1));
        let near = self.line.get(at).is_some_and(|p| (self.lateral - p.lateral).abs() < REPLAN);
        if near {
            self.line_path_with(routes, rate, position, forward, speed, drift, false, target, OFFSET_KEEP_RATE, true)
        } else {
            self.line_path(routes, rate, position, forward, speed, drift, false, target)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn line_path_with(
        &mut self,
        routes: &AiRoutes,
        rate: f32,
        position: Vec3,
        forward: Vec3,
        speed: f32,
        drift: bool,
        fresh: bool,
        target: f32,
        keep_rate: f32,
        keep: bool,
    ) -> Vec<Vec3> {
        let f = &routes.file;
        if self.points.get(self.current).is_none() {
            return Vec::new();
        }
        let heading = forward.truncate();
        let ahead = |q: Vec3| (q - position).truncate().dot(heading) > 0.0;
        // 0x404EF0: the path point the car is at, the first ahead of it
        // (else the last).
        let at = self.line.iter().position(|p| ahead(p.pos)).unwrap_or(self.line.len().saturating_sub(1));
        let threshold = if drift { REPLAN_DRIFT } else { REPLAN };
        let replan = !keep
            && (fresh
                || self.line.get(at).is_none_or(|p| {
                    let off = (self.lateral - p.lateral).abs();
                    off > threshold && off > p.s - self.distance
                }));
        if replan {
            self.line.clear();
            self.replanned = true;
        } else {
            self.line.drain(..at);
            match self.line.iter().position(|p| ahead(p.pos)) {
                None => self.line.clear(),
                Some(k) => {
                    self.line.drain(..k);
                    // 0x415590: the first point `keep` metres on, 2D.
                    let keep = (speed * keep_rate).max(KEEP_MIN);
                    let mut d = (self.line[0].pos - position).truncate().length();
                    let mut j = 0;
                    while d < keep && j + 1 < self.line.len() {
                        d += (self.line[j + 1].pos - self.line[j].pos).truncate().length();
                        j += 1;
                    }
                    self.line.truncate(j + 1);
                }
            }
        }
        let (mut lateral, from) = match self.line.last() {
            Some(l) => (l.lateral, l.s + 0.5),
            None => (self.lateral, self.distance),
        };
        let margin = if drift { ROOM_MARGIN_DRIFT } else { ROOM_MARGIN };
        let mut rate = rate;
        for i in 1..self.points.len() {
            if self.line.len() >= PATH_POINTS {
                break;
            }
            let p = &self.points[i];
            if p.s <= from || (self.line.is_empty() && i <= self.current) {
                continue;
            }
            let step = rate * (p.s - self.points[i - 1].s).max(0.0);
            let moved = if lateral > target { (lateral - step).max(target) } else { (lateral + step).min(target) };
            // The room (0x40FC80, lane 0: +0x18 + margin .. +0x2C −
            // margin), taken as the road's edges off the racing line; 0
            // kept inside it outside drift races, crossed bounds meeting
            // at their mean in them.
            let cp = &f.records[p.record].points[p.cp];
            let line = lane_offset(cp, lane(0, p.backward)) + p.adjust;
            // The travel direction's left and right halves (as `project`'s
            // edge: backward the record's halves swap sides).
            let (left, right) =
                if p.backward { (cp.half_width[1], cp.half_width[0]) } else { (cp.half_width[0], cp.half_width[1]) };
            let (mut lo, mut hi) = (-left - line + margin, right - line - margin);
            if !drift {
                (lo, hi) = (lo.min(0.0), hi.max(0.0));
            } else if lo > hi {
                (lo, hi) = ((lo + hi) / 2.0, (lo + hi) / 2.0);
            }
            let held = moved.clamp(lo, hi);
            // 0x41D530: held back against its motion, the rate drops to 0.
            if (held - target).abs() > 0.001 && (held - lateral) * (target - lateral).signum() < 0.0 {
                rate = 0.0;
            }
            lateral = held;
            // 0x415D50: every point, a junction gap's inserted ones too
            // (+0x74), is 0x4122C0 at its own distance with the lateral:
            // the control point's geometry at the point's offset, lane 0's
            // offset, its adjust (+0x04, which the smoothing 0x41A180 set
            // so that the line passes the Hermite curve) and the lateral.
            // The clone took an inserted point's smoothed place itself,
            // without the lateral, so a car off its line got a path that
            // zigzagged between the line and its lateral every 8 m
            // (docs/outrun-ai-strength.md).
            let pos = self.line_point(f, p, p.offset, lateral);
            self.line.push(LinePoint { s: p.s, lateral, pos });
        }
        self.line.iter().map(|p| p.pos).collect()
    }

    /// The driver's path on a traffic lane (0x420D40 → 0x41D530 in a drag
    /// race, where 0x415B70's lane keeping is skipped): a point for each
    /// FuturePoint ahead (at most 20) on the lane, its lateral offset
    /// starting at the car's off the lane's centre (0x60BFB0) and moving to
    /// the lane record's offset (+0x04, 0) by `rate` per metre (0x40FC80;
    /// the step taken as the distance from the last point). The clamp to the
    /// lane's room (0x40FC80 with future point +0x18.. / +0x2C..) is not
    /// ported.
    pub fn lane_path(&self, routes: &AiRoutes, position: Vec3, l: usize, rate: f32) -> Vec<Vec3> {
        let f = &routes.file;
        let Some(here) = self.points.get(self.current) else { return Vec::new() };
        let mut lateral = self.lane_lateral(routes, position, l);
        let mut last = here.s;
        let mut path = Vec::with_capacity(PATH_POINTS);
        for p in self.points.iter().filter(|p| p.s > self.distance).take(PATH_POINTS) {
            let step = rate * (p.s - last).max(0.0);
            last = p.s;
            lateral = if lateral > 0.0 { (lateral - step).max(0.0) } else { (lateral + step).min(0.0) };
            let cp = &f.records[p.record].points[p.cp];
            path.push(f.point(p.record, p.cp, p.backward, p.offset, lane_offset(cp, lane(l, p.backward)) + lateral));
        }
        path
    }

    /// A point's place on the racing line (0x4122C0 at its own s, lane 0).
    pub(super) fn smooth(&mut self, f: &RouteFile, from: usize) {
        let n = self.points.len();
        if n == 0 {
            return;
        }
        let mut i = from.min(n - 1) as isize;
        let mut plain = 0;
        while i >= 0 && plain < 2 {
            if self.points[i as usize].inserted.is_none() {
                plain += 1;
            }
            i -= 1;
        }
        let mut i0 = i.max(0) as usize;
        let scaled = |v: Vec2, len: f32| {
            let m = v.length() / len;
            if m != 0.0 { v / m } else { Vec2::X }
        };
        while i0 < n {
            while i0 < n && self.points[i0].inserted.is_some() {
                i0 += 1;
            }
            let mut i1 = i0 + 1;
            while i1 < n && self.points[i1].inserted.is_some() {
                i1 += 1;
            }
            if i0 < n && i1 < n && i1 - i0 > 1 {
                let (a, b) = (&self.points[i0], &self.points[i1]);
                let (s0, span) = (a.s, b.s - a.s);
                let len = span * 0.75;
                let p0 = self.racing_point(f, a).truncate();
                let p1 = self.racing_point(f, b).truncate();
                let t0 = if i0 > 0 {
                    scaled(p0 - self.racing_point(f, &self.points[i0 - 1]).truncate(), len)
                } else {
                    scaled(f.direction(a.record, a.cp, a.backward), len)
                };
                let next = (i1 + 1..n).find(|&k| self.points[k].inserted.is_none());
                let t1 = match next {
                    Some(k) => scaled(self.racing_point(f, &self.points[k]).truncate() - p1, len),
                    None => scaled(f.direction(b.record, b.cp, b.backward), len),
                };
                for k in i0 + 1..i1 {
                    let h = hermite(p0, t0, p1, t1, (self.points[k].s - s0) * (1.0 / span));
                    let p = &self.points[k];
                    let cp = &f.records[p.record].points[p.cp];
                    let delta =
                        f.lateral(p.record, p.cp, p.backward, h) - (lane_offset(cp, lane(0, p.backward)) + p.adjust);
                    self.points[k].adjust += delta;
                }
            }
            i0 = i1;
        }
    }

    pub(super) fn racing_point(&self, f: &RouteFile, p: &FuturePoint) -> Vec3 {
        if let Some(q) = p.inserted {
            return q;
        }
        self.line_point(f, p, p.offset, 0.0)
    }

    pub(super) fn line_point(&self, f: &RouteFile, p: &FuturePoint, d: f32, extra: f32) -> Vec3 {
        let cp = &f.records[p.record].points[p.cp];
        let l = lane_offset(cp, lane(0, p.backward)) + p.adjust + extra;
        f.point(p.record, p.cp, p.backward, d, l)
    }

    /// The racing line at a distance along the list, `extra` metres to
    /// the right (0x4122C0 from the point at or before `s`).
    pub(super) fn point_at(&self, f: &RouteFile, s: f32, extra: f32) -> Vec3 {
        let p = self.points.iter().rev().find(|p| p.s <= s).unwrap_or(&self.points[0]);
        self.line_point(f, p, s - p.s + p.offset, extra)
    }

    /// Where the car is along the list (0x419B20): the nearest point by
    /// ground distance plus 10× the height difference (0x4198C0), then the
    /// one whose stretch holds the car; its distance (0x40AE30).
    pub(super) fn project(&mut self, f: &RouteFile, position: Vec3) {
        let score = |p: &FuturePoint| {
            let q = self.racing_point(f, p);
            q.truncate().distance(position.truncate())
                + (f.records[p.record].points[p.cp].position.z - position.z).abs() * 10.0
        };
        let Some(mut i) =
            (0..self.points.len()).min_by(|&a, &b| score(&self.points[a]).total_cmp(&score(&self.points[b])))
        else {
            return;
        };
        let s_of = |i: usize| {
            let p = &self.points[i];
            p.s - p.offset + f.along(p.record, p.cp, p.backward, position.truncate())
        };
        let mut s = s_of(i);
        while i + 1 < self.points.len() && self.points[i + 1].s <= s {
            i += 1;
            s = s_of(i);
        }
        while i > 0 && self.points[i].s > s {
            i -= 1;
            s = s_of(i);
        }
        let p = &self.points[i];
        let cp = &f.records[p.record].points[p.cp];
        let side = f.lateral(p.record, p.cp, p.backward, position.truncate());
        // The road's right half is the record's right forward, its left
        // backward.
        let edge = if (side >= 0.0) != p.backward { cp.half_width[1] } else { cp.half_width[0] };
        self.off_road = side.abs() - edge;
        self.lateral = side - lane_offset(cp, lane(0, p.backward)) - p.adjust;
        self.current = i;
        self.distance = s;
    }
}
