use super::*;

/// An object's place on a racer's list (see [`RacerRoute::locate`]).
#[derive(Clone, Copy, Debug)]
pub struct Located {
    pub s: f32,
    pub lateral: f32,
    pub room: (f32, f32),
    pub direction: Vec2,
    pub line_speed: f32,
}

impl RacerRoute {
    /// The racer's road at its current point (car +0x2C → +0xC), for the
    /// road-edge test 0x40DF50.
    pub fn edge(&self, routes: &AiRoutes) -> Option<crate::ai::driver::RoadEdge> {
        let p = self.points.get(self.current)?;
        let f = &routes.file;
        let c = f.records.get(p.record)?.points.get(p.cp)?;
        Some(crate::ai::driver::RoadEdge {
            position: c.position.truncate(),
            tangent: c.tangent,
            travel: f.direction(p.record, p.cp, p.backward),
            half_width: c.half_width,
            backward: p.backward,
        })
    }

    /// The list's nodes for the crash camera (0x5F1FE0 case 0x0F, from
    /// the car's own node, `current`): each node's distance (+0x54), its
    /// points `inset` m inside its left and right road edges (0x4122C0 at
    /// the node's own place with vt+0xC + inset / vt+0x10 − inset: the
    /// half widths, as the turn circle's 5 m inset reads them), and the
    // prov: inferred ai.route.node_edges
    /// edges themselves (+0x18 / +0x2C, **inferred** as the half widths).
    /// The list holds the points 0x418DA0 inserts every 8 m across a
    /// junction's gap too: read under Wine (the route list car +0x2C
    /// → vt+0x24 = +0x68 at the Quick Race grid), node +0x54 ran 0, 8.0,
    /// 20.6, 28.6, 38.7, 53.3, 61.3, 70.3, 84.4, 92.4, … where the clone's
    /// points without them are 0, 20.6, 38.7, 53.3, 70.3, 84.4. An inserted
    /// point's places are its control point's at its own offset
    // prov: inferred ai.route.node_edges-2
    /// (**inferred**; same half widths, as `..last.clone()` keeps them).
    ///
    /// The list starts at the car's node as 0x5F1EF0 finds it (0x4198C0
    /// with flag 1): the nearest node, then on while it lies behind the car
    /// (its +0x54 less the car's distance along it below 0) — the first
    /// node at or ahead of the car. (The clone once started at `current`,
    /// the node behind: every crash camera place came one node, 8 m, short
    /// of the original's in four captures.)
    pub fn node_edges(&self, routes: &AiRoutes, inset: f32) -> Vec<crate::event_camera::NodeEdges> {
        let f = &routes.file;
        let first = self.current.min(self.points.len());
        let first = (first..self.points.len()).find(|&i| self.points[i].s >= self.distance).unwrap_or(first);
        self.points[first..]
            .iter()
            .map(|p| {
                let cp = &f.records[p.record].points[p.cp];
                let (left, right) = if p.backward {
                    (cp.half_width[1], cp.half_width[0])
                } else {
                    (cp.half_width[0], cp.half_width[1])
                };
                let at = |l: f32| f.point(p.record, p.cp, p.backward, p.offset, l);
                crate::event_camera::NodeEdges {
                    s: p.s,
                    inner: [at(inset - left), at(right - inset)],
                    edge: [at(-left), at(right)],
                }
            })
            .collect()
    }

    /// The car's distance along the list.
    /// How far the racer is to the side of its racing line, m (signed).
    pub fn lateral(&self) -> f32 {
        self.lateral
    }

    pub fn distance(&self) -> f32 {
        self.distance
    }

    /// Where another object lies on this racer's list, as the road-entry
    /// builder (0x6148F0 → 0x419C10) files it: its distance along the list
    /// (entry +0x10), its offset off the racing line (0x60BFB0 on lane 0),
    /// the room there off the racing line (the FuturePoint's +0x18 / +0x2C,
    /// the road's edges, as `line_path`'s room without the margin), the
    /// list's travel direction there and its racing-line speed (the point's
    /// lane-0 byte +0x2C, 0x4024F0). None when it is off the list's span
    /// (before its first point or past its last). The point is found as
    /// `project` finds the car's (nearest racing-line point, then walked
    // prov: inferred ai.route.locate
    /// to the one at or before it; **inferred**). The road entries file
    /// objects by 0x419C10 → 0x419B20 instead, traced:
    /// [`RacerRoute::locate_object`].
    pub fn locate(&self, routes: &AiRoutes, position: Vec3) -> Option<Located> {
        let f = &routes.file;
        let score = |p: &FuturePoint| {
            let q = self.racing_point(f, p);
            q.truncate().distance(position.truncate())
                + (f.records[p.record].points[p.cp].position.z - position.z).abs() * 10.0
        };
        let mut i =
            (0..self.points.len()).min_by(|&a, &b| score(&self.points[a]).total_cmp(&score(&self.points[b])))?;
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
        if s < self.points[0].s || s > self.points[self.points.len() - 1].s {
            return None;
        }
        Some(self.located_at(routes, i, s, position))
    }

    /// An object's place on the list as the road entries file it (0x6148F0 →
    /// 0x419C10 → 0x419B20): the list point 0x4198C0 finds with flags
    /// 0 — the least 2D distance to the point's racing point (0x4122C0 at
    /// +0x54) plus 10 (0x783CC4) × the height difference to its control
    /// point (+0x12 / 8), no walk to a neighbour — its s there (0x40AE30:
    /// +0x54 − +0x58 + the segment's along, extrapolated before the first
    /// point and past the last) and the offset off the line at that point
    /// (0x4123C0). Only an empty list gives none (0x419B20 → 0, 0x43BD50).
    /// **Verified** (dis 0x4198C0–0x419B05, 0x419B20, 0x40AE30).
    pub fn locate_object(&self, routes: &AiRoutes, position: Vec3) -> Option<Located> {
        let f = &routes.file;
        // The scan steps 1 + max(0, −2 − trunc(−0.02 × d)) points (0x7852AC):
        // it skips points only from 150 m away; d is capped at 99999
        // (0x784360) and the least score wins, the first on a tie.
        let (mut best, mut k) = (None::<(usize, f32)>, 0usize);
        while k < self.points.len() {
            let p = &self.points[k];
            let d = self.racing_point(f, p).truncate().distance(position.truncate()).clamp(0.0, 99_999.0);
            let score = d + (f.records[p.record].points[p.cp].position.z - position.z).abs() * 10.0;
            if best.is_none_or(|b| score < b.1) {
                best = Some((k, score));
            }
            let skip = -2 - (d * -0.02) as i32;
            k += 1 + skip.max(0) as usize;
        }
        let i = best?.0;
        let p = &self.points[i];
        let s = p.s - p.offset + f.along(p.record, p.cp, p.backward, position.truncate());
        Some(self.located_at(routes, i, s, position))
    }

    /// The list's span: its first and last points' distances (+0x54).
    pub fn list_span(&self) -> Option<(f32, f32)> {
        Some((self.points.first()?.s, self.points.last()?.s))
    }

    fn located_at(&self, routes: &AiRoutes, i: usize, s: f32, position: Vec3) -> Located {
        let f = &routes.file;
        let p = &self.points[i];
        let cp = &f.records[p.record].points[p.cp];
        let line = lane_offset(cp, lane(0, p.backward)) + p.adjust;
        let side = f.lateral(p.record, p.cp, p.backward, position.truncate());
        let (left, right) =
            if p.backward { (cp.half_width[1], cp.half_width[0]) } else { (cp.half_width[0], cp.half_width[1]) };
        Located {
            s,
            lateral: side - line,
            room: (-left - line, right - line),
            direction: f.direction(p.record, p.cp, p.backward),
            line_speed: f32::from(cp.lane_speed[0]),
        }
    }

    /// The room off the racing line (lane 0: FuturePoint +0x18 / +0x2C, as
    /// [`RacerRoute::locate`] gives it) at every walk point from the one at
    /// `from` to the one at `to` (both by their distance along the list, the
    /// point at or before each), for 0x428FD0's slow test.
    pub fn rooms_between(&self, routes: &AiRoutes, from: f32, to: f32) -> Vec<(f32, f32)> {
        let f = &routes.file;
        let at = |s: f32| self.points.iter().rposition(|p| p.s <= s).unwrap_or(0);
        let (a, b) = (at(from), at(to));
        if self.points.is_empty() || b < a {
            return Vec::new();
        }
        self.points[a..=b]
            .iter()
            .map(|p| {
                let cp = &f.records[p.record].points[p.cp];
                let line = lane_offset(cp, lane(0, p.backward)) + p.adjust;
                let (left, right) = if p.backward {
                    (cp.half_width[1], cp.half_width[0])
                } else {
                    (cp.half_width[0], cp.half_width[1])
                };
                (-left - line, right - line)
            })
            .collect()
    }

    /// 0x419E10(lane, from the car's point, −1): how the list bends ahead
    /// of the car, from the racing line at the car's point, the middle one
    /// and the last (0x4122C0 on each). The three points are traced; the
    // prov: inferred ai.route.bend
    /// value made of them (0x419F39 on) is **inferred**: the cross product
    /// of the two unit chords, positive when the road bends left (the
    /// side test of 0x41D2D0 reads its sign, |value| < 0.12 as straight).
    pub fn bend(&self, routes: &AiRoutes) -> f32 {
        let n = self.points.len();
        if n < self.current + 2 {
            return 0.0;
        }
        let f = &routes.file;
        let a = self.racing_point(f, &self.points[self.current]).truncate();
        let b = self.racing_point(f, &self.points[self.current + (n - 1 - self.current) / 2]).truncate();
        let c = self.racing_point(f, &self.points[n - 1]).truncate();
        let (u, v) = ((b - a).normalize_or_zero(), (c - b).normalize_or_zero());
        u.x * v.y - u.y * v.x
    }

    /// Whether a forward traffic lane (2–5) exists on the record of the
    /// point the car is at (0x40AE90: record header +0x82 + lane, read as
    /// 6–9 travelled backward).
    pub fn has_lane(&self, routes: &AiRoutes, l: usize) -> bool {
        let Some(p) = self.points.get(self.current) else { return false };
        (2..=5).contains(&l) && routes.file.records[p.record].lanes[lane(l, p.backward)]
    }

    /// Whether a traffic lane runs from the car's point to the end of the
    /// list (0x40AA90: the lane's end along the list, +0xAC + 4·lane, not
    /// before the last point's s; the end taken as the first point whose
    /// record lacks the lane).
    pub fn lane_runs(&self, routes: &AiRoutes, l: usize) -> bool {
        (2..=5).contains(&l)
            && self.points.iter().skip(self.current).all(|p| routes.file.records[p.record].lanes[lane(l, p.backward)])
    }

    /// Where a lane stops along the list (0x40AF30 reads the route list's
    /// +0xAC + 4·lane): the s of the first point from the car's on whose
    /// record it is missing, else the last point's s (**inferred**: the
    /// list's end for a lane that runs to it).
    pub fn lane_end(&self, routes: &AiRoutes, l: usize) -> f32 {
        let pts = self.points.iter().skip(self.current);
        let mut last = self.distance;
        for p in pts {
            if !(2..=5).contains(&l) || !routes.file.records[p.record].lanes[lane(l, p.backward)] {
                return p.s;
            }
            last = p.s;
        }
        last
    }

    /// How far the car is to the right of a traffic lane's centre at the
    /// point it is at (0x60BFB0 / 0x419BC0: the lane's position by
    /// 0x401C40).
    pub fn lane_lateral(&self, routes: &AiRoutes, position: Vec3, l: usize) -> f32 {
        let Some(p) = self.points.get(self.current) else { return 0.0 };
        let f = &routes.file;
        let adj = if (2..=5).contains(&l) { p.lane_adjust[l - 2] } else { 0.0 };
        f.lateral(p.record, p.cp, p.backward, position.truncate())
            - lane_offset(&f.records[p.record].points[p.cp], lane(l, p.backward))
            - adj
    }

    /// The traffic lane nearest the car (0x40F4A0): of the lanes 2–5 its
    /// point's record has (6–9 travelled backward, given back as 2–5), the
    /// one whose position (0x401C40) is closest to the car's lateral; with
    /// a window only lanes outside it. None when there is none.
    pub fn nearest_lane(&self, routes: &AiRoutes, position: Vec3, window: Option<(usize, usize)>) -> Option<usize> {
        let p = self.points.get(self.current)?;
        let f = &routes.file;
        let cp = &f.records[p.record].points[p.cp];
        let side = f.lateral(p.record, p.cp, p.backward, position.truncate());
        (2..=5usize)
            .filter(|&l| f.records[p.record].lanes[lane(l, p.backward)])
            .filter(|&l| window.is_none_or(|(lo, hi)| l < lo || l > hi))
            .min_by(|&a, &b| {
                (lane_offset(cp, lane(a, p.backward)) + p.lane_adjust[a - 2] - side)
                    .abs()
                    .total_cmp(&(lane_offset(cp, lane(b, p.backward)) + p.lane_adjust[b - 2] - side).abs())
            })
    }

    /// The racing line's speed at a distance along the list (0x411DD0,
    /// lane 0): between the point at or before `s` and the next one on
    /// another control point, by their control points' distances.
    /// What 0x416F70 reads of the walk: the FuturePoint lane-0 speed at
    /// the car's point (0x40AFC0; None when unset) and whether the speeds
    /// do not drop from the car's point to the list's end (0x412220).
    pub fn nitrous_road(&self) -> (Option<f32>, bool) {
        if self.points.is_empty() {
            return (None, false);
        }
        let i = self.current.min(self.points.len() - 1);
        let here = self.points[i].speed[0];
        let flat = self.points[i..]
            .windows(2)
            .all(|w| w[1].speed[0] < 0.0 || w[0].speed[0] < 0.0 || w[1].speed[0] >= w[0].speed[0]);
        ((here > 0.0).then_some(here), flat)
    }

    /// Check aid (docs/outrun-ai-strength.md): the next `n` points from
    /// the car's as (record, control point, s, lane-0 speed).
    pub fn upcoming(&self, n: usize) -> Vec<(usize, usize, f32, f32)> {
        self.points.iter().skip(self.current).take(n).map(|p| (p.record, p.cp, p.s, p.speed[0])).collect()
    }

    /// The list's points as (record id, control point, +0x58, lane 0's
    /// adjustment +0x04, lanes 2–5's +0x08..+0x14).
    pub fn lane_adjusts(&self, f: &RouteFile) -> Vec<(u16, usize, f32, f32, [f32; 4])> {
        self.points.iter().map(|p| (f.records[p.record].id, p.cp, p.offset, p.adjust, p.lane_adjust)).collect()
    }

    /// Check aid: the list's points (record, cp, backward, s, offset,
    /// adjust, inserted, racing point) and the kept line (s, lateral, pos).
    pub fn debug_lists(&self, f: &RouteFile) -> String {
        let pts: Vec<String> = self
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let q = self.racing_point(f, p);
                format!(
                    "{}{} {}/{}{} s={:.1} off={:.1} adj={:.2} ins={} rp=({:.1},{:.1})",
                    if i == self.current { "*" } else { " " },
                    i,
                    p.record,
                    p.cp,
                    if p.backward { "b" } else { "" },
                    p.s,
                    p.offset,
                    p.adjust,
                    p.inserted.is_some(),
                    q.x,
                    q.y
                )
            })
            .collect();
        let line: Vec<String> = self
            .line
            .iter()
            .map(|l| format!("s={:.1} lat={:.2} ({:.1},{:.1})", l.s, l.lateral, l.pos.x, l.pos.y))
            .collect();
        format!(
            "distance {:.1} lateral {:.2}\n{}\nline:\n{}",
            self.distance,
            self.lateral,
            pts.join("\n"),
            line.join("\n")
        )
    }

    pub fn speed_at(&self, s: f32) -> f32 {
        let Some(i) = self
            .points
            .iter()
            .enumerate()
            .filter(|(_, p)| p.s <= s)
            .min_by(|a, b| (s - a.1.s).total_cmp(&(s - b.1.s)))
            .map(|(i, _)| i)
        else {
            return self.points.last().map_or(0.0, |p| p.speed[0]);
        };
        let a = &self.points[i];
        let base = a.s - a.offset;
        match self.points[i + 1..].iter().find(|p| (p.record, p.cp) != (a.record, a.cp)) {
            Some(b) => (b.speed[0] - a.speed[0]) * ((s - base) / (b.s - base)) + a.speed[0],
            None => a.speed[0],
        }
    }
}

impl RacerRoute {
    pub fn debug(&self) -> String {
        format!(
            "{} points, {} turns, current {}, s {:.1}..{:.1}, speeds {:?}",
            self.points.len(),
            self.turns.len(),
            self.current,
            self.points.first().map_or(0.0, |p| p.s),
            self.points.last().map_or(0.0, |p| p.s),
            self.points.iter().map(|p| format!("{}:{} s{:.0} v{:.1}", p.record, p.cp, p.s, p.speed[0])).collect::<Vec<_>>()
        )
    }
}
