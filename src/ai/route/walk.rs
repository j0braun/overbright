use super::*;

/// One FuturePoint (0x7C bytes in the game).
#[derive(Clone, Debug)]
pub(super) struct FuturePoint {
    pub(super) record: usize,
    pub(super) cp: usize,
    /// The backward class (vtable 0x784AA0): travelled against the order.
    pub(super) backward: bool,
    /// +0x54: distance along the list; +0x58: distance past the control
    /// point (inserted points).
    pub(super) s: f32,
    pub(super) offset: f32,
    /// +0x04: the racing line's lateral adjustment.
    pub(super) adjust: f32,
    /// +0x08 + 4·(lane − 2): lanes 2–5's lateral adjustments (0x41A180
    /// bends them as it bends lane 0's; travelled backward the lanes are
    /// 6–9 in the record and the slots stay 2–5).
    pub(super) lane_adjust: [f32; 4],
    /// +0x40 lane 0, +0x44.. lanes 2–5; −1 not yet set.
    pub(super) speed: [f32; 5],
    /// +0x74: inserted across a junction gap (0x418DA0); its racing-line
    /// point once smoothed (0x41A180).
    pub(super) inserted: Option<Vec3>,
}

/// A turn entry (planner +0xD4.., five at most): the point before the
/// turn's junction, and the turn's place along the list.
#[derive(Clone, Debug)]
pub(super) struct Turn {
    /// Index of the point before the turn (entry +0x148).
    pub(super) fp: usize,
    pub(super) s: f32,
    /// +1 when the turn bends left (the outside is to the right).
    pub(super) side: f32,
}

/// One racer's walk along the route.
#[derive(Clone, Debug)]
pub struct RacerRoute {
    pub(super) points: Vec<FuturePoint>,
    pub(super) turns: Vec<Turn>,
    pub(super) current: usize,
    pub(super) distance: f32,
    pub(super) lateral: f32,
    /// How far the car is past the road's edge (≤ 0 on the road).
    pub(super) off_road: f32,
    /// The direction the list is walked in (FuturePoint class).
    pub(super) backward: bool,
    /// Native ordered 16-slot storage; goal 5 search imports remain unwired.
    pub(super) decisions: JunctionCache,
    /// The list's s at the last turn taken (planner +0x354).
    pub(super) last_turn: f32,
    /// The walk ran out (0x40ABB0: the car is told to stop).
    pub dead_end: bool,
    /// Race seconds, for the branch hold.
    pub clock: f32,
    /// Seconds after the start before branches (type-4 junctions) may be
    /// taken (0x5EF000: 12 − 0.5·(n − 100) for racer number n 100…105, the
    /// track's +0xE4 being −1; other numbers none, [`start_hold`]). The
    /// race rivals set it from their number; [`RacerRoute::start`] keeps 12 s
    /// for the encounter racers, whose car +0x0A is not read (**open**).
    pub branch_hold: f32,
    /// A drag race's racer (0x89E7D8 / 0x890108): 0x422220 joins records by
    /// dropping the list's last point instead of walking past the new one.
    pub drag: bool,
    /// The human controller's walk (controller kind 1, 0x413CC0's first
    /// test: junction types 2 and 8 taken before the hold and every other
    /// test, 0x5DCBF0(2)).
    pub human: bool,
    /// The generator the junction draws take (0x413CC0: the world's,
    /// 0x890080 +0xF8, which `racer_think` swaps in for the walk).
    pub rng: GameRng,
    /// The car's race coordinate (racer +0x8FC), which the road node's
    /// search holds against each record's coordinate range first (0x413590
    /// → 0x5D44D0); None when the caller has none (free roam).
    pub race_coord: Option<f32>,
    /// The skill row's odds for the retyped junctions (AI tuning +0x524,
    /// +0x528, +0x52C of the row, 0x413CC0): [0, 0, 1] takes none.
    pub junction_odds: [f32; 3],
    /// Active goal 6: owner XY and second-place XY, refreshed before walking.
    pub outrun_leader_positions: Option<(Vec2, Vec2)>,
    pub(super) outrun_chaser: bool,
    pub(super) search_rebuild_pending: bool,
    /// AIActionJustDrive's path (action +0x20), kept between frames.
    pub(super) line: Vec<LinePoint>,
    /// Driver event 4: the path was planned afresh since the
    /// driver last took it — 0x420D40's 0x41D160(0, 0) (0x420DCA /
    /// 0x420E97) empties action +0x20 and stamps its +0x1E0 with the tick
    /// (0x865188), which 0x420810 then finds equal and posts driver +0x250
    /// = 4 (0x4208CA). Taken by [`RacerRoute::take_replanned`].
    pub(super) replanned: bool,
}

/// Goal 6's 0x413F50 junction predicate. `second` is the second racer
/// in race order, not an arbitrary pursuit target. The x87 RNG fraction
/// remains extended precision until comparison; rounding to f32 first
/// changes decisions immediately below the .5/.75 boundaries.
pub(super) fn outrun_leader_junction(
    flags: u32,
    record_blocked: bool,
    car: Vec2,
    second: Vec2,
    rng: &mut GameRng,
) -> bool {
    let kind = flags & 15;
    if kind == 2 {
        return true;
    }
    if matches!(kind, 5 | 7 | 10) || record_blocked || flags & (1 << 20) != 0 {
        return false;
    }
    let dx = f64::from(car.x) - f64::from(second.x);
    let dy = f64::from(car.y) - f64::from(second.y);
    let mut odds = if (dx * dx + dy * dy).sqrt() < 150.0 { 0.75f32 } else { 0.1f32 };
    if kind == 4 && flags & (1 << 26) != 0 {
        odds = if flags & (1 << 27) != 0 { 0.333 } else { 0.5 };
    }
    f64::from(rng.below(0x7fff_ffff)) / 2_147_483_648.0 < f64::from(odds)
}

/// The start hold of racer number `number` (0x5EF000, ECX = the race
/// status: outside drag and track 1099, a number n in 100…105 holds while
/// the race clock (since status +0x10) is at most 12 + 0.5·(100 − n) s,
/// 0x802278 / 0x80227C; other numbers none). The number is car +0x0A, the
/// set-up entry's byte +1 (0x604442); **observed** under Wine (Quick Race
/// 4014): the rivals in world order read 100, 101, 102, so the clone gives
/// rival i the number 100 + i (**inferred** for other race kinds).
/// The road-node search's radius (0x413590 → 0x5E2E50: 999999.0, 0x783498).
const SEARCH_RADIUS: f32 = 999_999.0;

/// The car's way for a road-node search without an old node (0x42CDC0,
/// 0x42D040..0x42D05C): its velocity (+0x70) when car +0x430 is above
/// 1.0 (0x784250), else its forward axis (+0x90).
pub fn travel(speed: f32, velocity: Vec3, forward: Vec3) -> Vec2 {
    if speed > 1.0 { velocity.truncate() } else { forward.truncate() }
}

pub fn start_hold(number: i32) -> f32 {
    if (100..=105).contains(&number) { 12.0 + 0.5 * (100 - number) as f32 } else { f32::NEG_INFINITY }
}

impl RacerRoute {
    /// Start on the road node's search, walked from there: as the racers'
    /// placement does (0x5EF229–0x5EF24D), the search without off-segment
    /// points (vt+0x30(0) = 0x42CDC0), then with them when that found
    /// none; see [`RacerRoute::find`]. `direction` is the car's way.
    pub fn start(routes: &AiRoutes, position: Vec3, direction: Vec2) -> Option<RacerRoute> {
        RacerRoute::start_in(routes, position, direction, false)
    }

    /// [`RacerRoute::start`] for a racer of a drag race (`drag`: 0x89E7D8 /
    /// 0x890108), whose walk joins records as 0x422220's drag branch does.
    pub fn start_in(routes: &AiRoutes, position: Vec3, direction: Vec2, drag: bool) -> Option<RacerRoute> {
        let (record, cp, backward) = RacerRoute::find(routes, position, direction, None, false)
            .or_else(|| RacerRoute::find(routes, position, direction, None, true))?;
        Some(RacerRoute::from_node_in(routes, position, record, cp, backward, drag))
    }

    pub(super) fn from_node(routes: &AiRoutes, position: Vec3, record: usize, cp: usize, backward: bool) -> RacerRoute {
        RacerRoute::from_node_in(routes, position, record, cp, backward, false)
    }

    fn from_node_in(routes: &AiRoutes, position: Vec3, record: usize, cp: usize, backward: bool, drag: bool) -> RacerRoute {
        let mut route = RacerRoute {
            drag,
            points: vec![FuturePoint {
                record,
                cp,
                backward,
                s: 0.0,
                offset: 0.0,
                adjust: 0.0,
                lane_adjust: [0.0; 4],
                speed: [-1.0; 5],
                inserted: None,
            }],
            turns: Vec::new(),
            current: 0,
            distance: 0.0,
            lateral: 0.0,
            off_road: 0.0,
            backward,
            decisions: JunctionCache::default(),
            last_turn: f32::MIN,
            dead_end: false,
            clock: 0.0,
            branch_hold: 12.0,
            human: false,
            rng: GameRng::default(),
            race_coord: None,
            junction_odds: [0.0, 0.0, 1.0],
            outrun_leader_positions: None,
            outrun_chaser: false,
            search_rebuild_pending: false,
            replanned: false,
            line: Vec::new(),
        };
        let f = &routes.file;
        route.project(f, position);
        route.extend(f, LOOK_AHEAD);
        route.smooth(f, 0);
        route
    }

    /// Goal assignment/exit: 402070 invokes the old goal's +48 before
    /// destroying its action. A repeated assignment of goal 5 keeps it.
    pub fn set_outrun_chaser(&mut self, active: bool) {
        if self.outrun_chaser && !active {
            self.decisions.clear_search();
        }
        self.outrun_chaser = active;
    }

    /// 420910 -> 41E540, before pruning and route update on a full think.
    /// Direction is velocity at speed >=1m/s, forward otherwise. Target
    /// is its native road point when present, world position otherwise.
    pub fn search_outrun_chaser(&mut self, routes: &AiRoutes, position: Vec3, direction: Vec2, target: Vec3) -> bool {
        let found = crate::ai::outrun_search::search(&routes.file, position, direction, target, 400.0, false)
            .or_else(|| crate::ai::outrun_search::search(&routes.file, position, -direction, target, 400.0, false));
        let Some(found) = found else { return false };
        self.decisions.import_search(&found.decisions);
        self.search_rebuild_pending = true;
        // RacerRoute represents the road node through its first point;
        // seed the cleared list from the new node before update extends it.
        self.points.clear();
        self.points.push(FuturePoint {
            record: found.record,
            cp: found.point,
            backward: !found.forward,
            s: 0.0,
            offset: 0.0,
            adjust: 0.0,
            lane_adjust: [0.0; 4],
            speed: [-1.0; 5],
            inserted: None,
        });
        self.turns.clear();
        self.current = 0;
        self.backward = !found.forward;
        self.dead_end = false;
        self.project(&routes.file, position);
        true
    }

    /// The road node's search (0x42CDC0 → 0x413590, "Win road race" mask
    /// 4: control points with flag bit 11, 0x5E2E50). Each race record
    /// offers its nearest such point (3D, the z from +0x12; radius
    /// 999999, 0x783498); of those the nearest wins whose segment holds
    /// the car (not past either half width, 0x40CD00, nor outside the
    /// segment's length, 0x40CE90). The first pass takes only records
    /// whose coordinate range (+0x38 / +0x3C, 0x5D44D0: wrapping when the
    /// last is not above the first) holds the car's race coordinate
    /// (racer +0x8FC); the second takes all records, and with
    /// `off_segment` (the per-frame search's argument 1) also points off
    /// their segment. In a race (0x88F2BE) the direction is the record's
    /// (0x40F750: backward when +0x70 bit 1 is set); otherwise the one its
    /// tangent shares with `direction` (0x42CDC0).
    pub fn find(
        routes: &AiRoutes,
        position: Vec3,
        direction: Vec2,
        coord: Option<f32>,
        off_segment: bool,
    ) -> Option<(usize, usize, bool)> {
        RacerRoute::find_flagged(routes, position, direction, coord, off_segment, 0x800)
    }

    /// [`RacerRoute::find`] for control points with `flag` in +0x0C: 0x800
    /// for an AI racer ("Win road race", mask 4), 0x0800_0000 for a human
    /// racer (car +0x14 → +4 = 1: mask 0x400, 0x413590).
    pub fn find_flagged(
        routes: &AiRoutes,
        position: Vec3,
        direction: Vec2,
        coord: Option<f32>,
        off_segment: bool,
        flag: u32,
    ) -> Option<(usize, usize, bool)> {
        RacerRoute::find_with(routes, position, direction, coord, off_segment, flag, routes.race_coords)
    }

    /// The human car's road node (car +0x2C): the human mask's search
    /// (0x0800_0000) with the way its tangent shares with `direction`, in
    /// a race too. Read under Wine (Quick Race, the 206 standing on
    /// the start straight): turned from −x to +x, the car's list ran 655/5 →
    /// 655/4 → 655/3 … and then 655/5 → 655/6 → 476/2 …, so the human's list
    /// follows the car's heading, not the race records' way.
    pub fn start_human(routes: &AiRoutes, position: Vec3, direction: Vec2) -> Option<RacerRoute> {
        let (record, cp, backward) =
            RacerRoute::find_with(routes, position, direction, None, false, 0x0800_0000, false)
                .or_else(|| RacerRoute::find_with(routes, position, direction, None, true, 0x0800_0000, false))?;
        let mut route = RacerRoute::from_node(routes, position, record, cp, backward);
        // The human's number (car +0x0A) is 0 (observed under Wine): no
        // start hold (0x5EF000 holds only 100…105).
        route.branch_hold = start_hold(0);
        route.human = true;
        Some(route)
    }

    pub(super) fn find_with(
        routes: &AiRoutes,
        position: Vec3,
        direction: Vec2,
        coord: Option<f32>,
        off_segment: bool,
        flag: u32,
        race_way: bool,
    ) -> Option<(usize, usize, bool)> {
        let f = &routes.file;
        let holds = |range: (f32, f32), c: f32| {
            let (a, b) = range;
            if b <= a { c >= a || c <= b } else { c >= a && c <= b }
        };
        for pass in 0..2 {
            let mut best: Option<(f32, usize, usize)> = None;
            for (ri, r) in f.records.iter().enumerate().filter(|(_, r)| r.is_race()) {
                if pass == 0 && coord.is_some_and(|c| !holds(r.coord_range, c)) {
                    continue;
                }
                let Some((ci, d)) = r
                    .points
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.flags & flag != 0)
                    .map(|(ci, c)| (ci, c.position.distance(position)))
                    // 0x5E2E50's radius 999999 (0x783498).
                    .filter(|&(_, d)| d <= SEARCH_RADIUS)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                else {
                    continue;
                };
                let c = &r.points[ci];
                let along = f.along(ri, ci, false, position.truncate());
                let side = f.lateral(ri, ci, false, position.truncate());
                let inside = (0.0..=c.length).contains(&along) && side >= -c.half_width[0] && side <= c.half_width[1];
                if ((off_segment && pass > 0) || inside) && best.is_none_or(|b| d < b.0) {
                    best = Some((d, ri, ci));
                }
            }
            if let Some((_, record, cp)) = best {
                let backward = if race_way {
                    f.records[record].flags >> 1 & 1 != 0
                } else {
                    f.records[record].points[cp].tangent.dot(direction) <= 0.0
                };
                return Some((record, cp, backward));
            }
        }
        None
    }

    /// One frame (0x428720 / 0x428030): follow the car, drop the points
    /// well behind it, extend to the look-ahead and rebuild the speeds.
    /// `accel` is the car's acceleration at a speed (`handling::acceleration`);
    /// `travel` the car's way for a search without a node ([`travel`]).
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        routes: &AiRoutes,
        position: Vec3,
        speed: f32,
        travel: Vec2,
        class: usize,
        suv: bool,
        accel: &dyn Fn(f32) -> f32,
    ) {
        let f = &routes.file;
        if self.outrun_leader_positions.is_some() || self.outrun_chaser {
            // 42AF40 -> 41D0E0 -> 428720: prune against the incoming
            // list, before this update can remove or append points.
            self.prune_outrun_leader_decisions(f);
        }
        if self.points.is_empty() {
            // 0x428720 with an empty list tests the node (+0x0C); a failed
            // search left it null (0x42CE9C), so vt+0x30(1) = 0x42CDC0
            // searches again, every frame until a node is found, and
            // 0x428030 rebuilds the list from it the same frame. With no
            // old node the way is the car's ([`travel`], 0x42D040..0x42D0A4;
            // in a race the record's, 0x40F750).
            let Some((record, cp, backward)) = RacerRoute::find(routes, position, travel, self.race_coord, true)
            else {
                return;
            };
            self.restart(f, position, record, cp, backward);
        }
        self.project(f, position);
        // 0x428720: the road node follows the list's point at the car when
        // the car's distance lies from 50 m before the list's first point
        // (0x785B80) up to its last, and the car is within twice its half
        // extent (0x60C1B0 × 2, the clone's 2.2 m) of that point's road
        // edge (0x40ADA0; the AI racer controller's vt+0x8C, 0x404800, is
        // false). Otherwise the node searches again (vt+0x30(1) =
        // 0x42CDC0: list and junction decisions cleared, 0x413590 with
        // off-segment points allowed in its second pass) and the list is
        // rebuilt from it (0x428030 with no point index), every frame.
        let (first, last) = (self.points[0].s, self.points[self.points.len() - 1].s);
        // After the goal-5 search (0x41E540 cleared the list, 0x411BE0)
        // 0x428720 has no list: it keeps the search's node only when the
        // car lies on that node's own segment (0x428893..0x42895E,
        // `node_holds`), else it searches again as below.
        let refind = if std::mem::take(&mut self.search_rebuild_pending) {
            let p = &self.points[0];
            !node_holds(f, p.record, p.cp, position)
        } else {
            self.off_road > OFF_ROAD || self.distance < first - 50.0 || self.distance >= last
        };
        if refind {
            let p = &self.points[self.current];
            // The old node's way (its tangent, or the reversed one, 0x401AA0).
            let dir = f.direction(p.record, p.cp, p.backward);
            match RacerRoute::find(routes, position, dir, self.race_coord, true) {
                Some((record, cp, backward)) => self.restart(f, position, record, cp, backward),
                None => {
                    // 0x42CE9C: node null, list cleared; searched again on
                    // the next frames (above).
                    self.points.clear();
                    self.dead_end = true;
                    return;
                }
            }
        }
        // Keep 20 points behind the one the car is at.
        let drop = self.current.saturating_sub(KEEP_BEHIND);
        if drop > 0 {
            self.points.drain(..drop);
            self.current -= drop;
            self.turns.retain_mut(|t| {
                t.fp = t.fp.wrapping_sub(drop);
                t.fp < self.points.len()
            });
        }
        // 0x428A04..0x428A41: vt+0x60 × car +0x430 against 125 m, the speed
        // signed (a reversing racer walks 125 m).
        let ahead = LOOK_AHEAD.max(speed * LOOK_AHEAD_TIME);
        let target = self.points[self.current].s + ahead;
        let from = self.points.len().saturating_sub(1);
        self.extend(f, target);
        self.speeds(f, class, suv, accel);
        self.smooth(f, from);
    }

    /// The list rebuilt from a searched node (0x42CDC0's node, then
    /// 0x428030 with no point index).
    fn restart(&mut self, f: &RouteFile, position: Vec3, record: usize, cp: usize, backward: bool) {
        self.points = vec![FuturePoint {
            record,
            cp,
            backward,
            s: 0.0,
            offset: 0.0,
            adjust: 0.0,
            lane_adjust: [0.0; 4],
            speed: [-1.0; 5],
            inserted: None,
        }];
        self.turns.clear();
        self.current = 0;
        self.distance = 0.0;
        self.backward = backward;
        self.dead_end = false;
        self.decisions.clear();
        self.last_turn = f32::MIN;
        self.line.clear();
        self.project(f, position);
        self.extend(f, LOOK_AHEAD);
        self.smooth(f, 0);
    }

    /// 0x42A190's test for the human walk: the list point the car is at
    /// (0x419B20, [`RacerRoute::locate_object`]) when the car's distance
    /// lies from 50 m before the first point up to the last (both kept) and
    /// it is at most `half_width` outside the road there (0x40ADA0: the
    /// offset past the right edge for an offset > 0, else past the left).
    pub(super) fn keeps_human(&self, routes: &AiRoutes, position: Vec3, half_width: f32) -> Option<usize> {
        let (first, last) = self.list_span()?;
        let at = self.locate_object(routes, position)?;
        let out = if at.lateral > 0.0 { at.lateral - at.room.1 } else { at.room.0 - at.lateral };
        if at.s < first - 50.0 || at.s > last || out > half_width {
            return None;
        }
        self.nearest_index(routes, position)
    }

    /// 0x4198C0 with flags 0: the index [`RacerRoute::locate_object`] files
    /// a position at.
    pub(super) fn nearest_index(&self, routes: &AiRoutes, position: Vec3) -> Option<usize> {
        let f = &routes.file;
        let (mut best, mut k) = (None::<(usize, f32)>, 0usize);
        while k < self.points.len() {
            let p = &self.points[k];
            let d = self.racing_point(f, p).truncate().distance(position.truncate()).clamp(0.0, 99_999.0);
            let score = d + (f.records[p.record].points[p.cp].position.z - position.z).abs() * 10.0;
            if best.is_none_or(|b| score < b.1) {
                best = Some((k, score));
            }
            k += 1 + (-2 - (d * -0.02) as i32).max(0) as usize;
        }
        best.map(|b| b.0)
    }

    /// 0x4120C0: the points before `i` dropped; the point at the car.
    pub(super) fn drop_before(&mut self, i: usize) -> FuturePoint {
        self.points.drain(..i);
        self.turns.retain_mut(|t| {
            t.fp = t.fp.wrapping_sub(i);
            t.fp < self.points.len()
        });
        self.current = self.current.saturating_sub(i);
        self.points[0].clone()
    }

    /// 0x428030 for the human walk: the car followed on the list and the
    /// list extended `reach` past the car's point (the racers' walk
    /// extension, `extend`).
    pub(super) fn extend_human(&mut self, f: &RouteFile, position: Vec3, reach: f32) {
        self.project(f, position);
        let target = self.points[self.current].s + reach;
        let from = self.points.len().saturating_sub(1);
        self.extend(f, target);
        self.smooth(f, from);
    }

    /// 42B002..42B02D: the own entry is strictly beyond the last point.
    pub fn past_end(&self) -> bool {
        self.points.last().is_some_and(|point| self.distance > point.s)
    }

    /// The control point the racer is at and whether it travels it
    /// backward: the walk's current FuturePoint, which the clone takes for
    /// the controller's road node (+0x0C, +0x10 = !backward; inferred).
    pub fn node(&self) -> Option<(usize, usize, bool)> {
        self.points.get(self.current).map(|p| (p.record, p.cp, p.backward))
    }

    /// 0x41D0E0 invalidates ordinary entries whose junction source is absent
    /// from the active walk. Search-imported entries are exempt natively;
    /// this cache contains no such entries (goal 5 is not wired).
    pub(super) fn prune_outrun_leader_decisions(&mut self, f: &RouteFile) {
        self.decisions.retain_ordinary(|j| {
            let Some(junction) = f.junctions.get(j) else { return false };
            // 5D3F90 reads +0A/+06, independent of junction kind. The
            // existing target() helper reads the destination for ordinary
            // links and must not be used for this membership test.
            let Some(record) = f.record(junction.target_record[1]) else { return false };
            let Ok(cp) = usize::try_from(junction.target_point[1]) else { return false };
            if cp >= f.records[record].points.len() {
                return false;
            }
            // 411C80 compares only record id and point index; direction
            // and inserted/smoothed point status do not affect retention.
            self.points.iter().any(|p| p.record == record && p.cp == cp)
        });
    }

    /// Whether a racer takes a junction (0x41EC20 with the "Win road race"
    /// goal, slot 10 → 0x413CC0): race links (type 2) always; traffic
    /// types (5, 7, 10), bit 20 and records with +0x70 bit 10 never;
    /// nothing in the hold; then branches (type 4) half the time (a third
    /// with bit 27) and the retyped junctions (0 / 1) by the skill row's
    /// odds; the decision kept (0x41CFA0).
    pub(super) fn accept(&mut self, f: &RouteFile, j: usize) -> bool {
        let junction = f.junctions[j];
        // 0x413CC0: a human controller (kind 1) takes types 2 and 8
        // (0x5DCBF0(2)) before anything else; type 2 for anyone (0x5DCBF0(0)).
        if junction.kind() == 2 || (self.human && junction.kind() == 8) {
            return true;
        }
        if self.outrun_chaser {
            let cacheable = !matches!(junction.kind(), 2 | 5 | 7 | 10);
            if cacheable && let Some(decision) = self.decisions.get(j) {
                return decision;
            }
            // Goal5 +28 accepts only type2 (handled above), without RNG.
            if cacheable {
                self.decisions.insert(j, false);
            }
            return false;
        }
        if let Some((car, second)) = self.outrun_leader_positions {
            // Goal 6 cache eligibility (0x41BD20) differs from its
            // acceptance predicate. In particular type 9 is never cached.
            let cacheable = !matches!(junction.kind(), 2 | 5 | 7 | 9 | 10);
            if cacheable && let Some(d) = self.decisions.get(j) {
                return d;
            }
            let blocked = f.record(junction.target().0).is_some_and(|r| f.records[r].flags & (1 << 10) != 0);
            let take = outrun_leader_junction(junction.flags, blocked, car, second, &mut self.rng);
            if cacheable {
                self.decisions.insert(j, take);
            }
            return take;
        }
        if let Some(d) = self.decisions.get(j) {
            return d;
        }
        let blocked = matches!(junction.kind(), 5 | 7 | 10) || junction.flags >> 20 & 1 != 0;
        let record_blocked = f.record(junction.target().0).is_some_and(|r| f.records[r].flags >> 10 & 1 != 0);
        if blocked || record_blocked || self.clock <= self.branch_hold {
            return false;
        }
        let r = self.rng.unit(1.0);
        let take = if junction.kind() == 4 {
            r < if junction.flags >> 27 & 1 != 0 { 0.333 } else { 0.5 }
        } else {
            // 0x413E33–0x413EC9: the skill row's three odds (+0x524,
            // +0x528, +0x52C; row = clamp(vt+0x78, 0, 9)): r ≤ a takes a
            // type-1 junction (the junction-type adjustment's "falls 20 m
            // behind in 10 s"), a < r ≤ b a type 0 ("gains 20 m"), b < r ≤ c
            // none, r > c a type 0. Every row's a is 0 and c is 1, b runs
            // 0, 0, 0.05, 0.2, 0.35, 0.5, 0.7, 0.7, 0.85, 0.9; other types
            // (none in the race files: 3 ends the walk first, 6, 8, 9) are
            // never type 0 or 1.
            let [a, b, c] = self.junction_odds;
            let kind = junction.kind();
            if r <= a {
                kind == 1
            } else if r <= b {
                kind == 0
            } else if r <= c {
                false
            } else {
                kind == 0
            }
        };
        if std::env::var_os("NFSU2_DEBUG_JUNCTIONS").is_some() {
            eprintln!(
                "junction {j} kind {} r {r:.3} odds {:?} take {take} clock {:.1}",
                junction.kind(),
                self.junction_odds,
                self.clock
            );
        }
        self.decisions.insert(j, take);
        take
    }

    /// One step of the walk (0x41EC20): a junction of the walking
    /// direction that the racer takes, else the next control point. None
    /// at a type-3 junction against the direction or past the record's end.
    pub(super) fn step(
        &mut self,
        f: &RouteFile,
        record: usize,
        cp: usize,
        forward: bool,
    ) -> Option<(usize, usize, bool, Option<usize>)> {
        let point = &f.records[record].points[cp];
        let here: Vec<usize> = f.junctions_at(point).collect();
        for j in here {
            let junction = f.junctions[j];
            if junction.direction() == forward {
                continue;
            }
            if junction.kind() == 3 {
                return None;
            }
            let after_turn = !junction.turn() || self.points.last().is_some_and(|l| l.s - self.last_turn > 30.0);
            if junction.kind() == 2 || (after_turn && self.accept(f, j)) {
                let (rid, tcp) = junction.target();
                let r = f.record(rid)?;
                let tcp = usize::try_from(tcp).ok().filter(|&c| c < f.records[r].points.len())?;
                return Some((r, tcp, forward ^ junction.flips(), Some(j)));
            }
        }
        if forward {
            (cp + 1 < f.records[record].points.len()).then_some((record, cp + 1, true, None))
        } else {
            cp.checked_sub(1).map(|c| (record, c, false, None))
        }
    }

    /// Extend the list (0x422220): a point per control point walked, until
    /// the last is past `target` with more than nine points, or 40.
    pub(super) fn extend(&mut self, f: &RouteFile, target: f32) {
        let mut guard = 0;
        // The walk's cursor (0x422220's record, point and direction): it
        // moves on past a point 0x418C50 drops, so the next step starts
        // from the dropped point, not the list's last.
        let mut cursor: Option<(usize, usize, bool)> = None;
        while self.points.len() < MAX_POINTS
            && !(self.points.len() > 9 && self.points.last().is_some_and(|l| l.s >= target))
        {
            guard += 1;
            if guard > 100 {
                return;
            }
            let mut last = self.points.last().cloned().expect("a point");
            let (from_record, from_cp, mut forward) = cursor.take().unwrap_or((last.record, last.cp, !last.backward));
            let Some((mut record, mut cp, f2, mut junction)) = self.step(f, from_record, from_cp, forward) else {
                self.dead_end = true;
                return;
            };
            forward = f2;
            let prev_pos = f.records[last.record].points[last.cp].position;
            let d = match junction.map(|j| f.junctions[j]) {
                None => {
                    // Along the last point's direction to the new point.
                    let p = f.records[record].points[cp].position.truncate();
                    f.along(last.record, last.cp, last.backward, p) - last.offset
                }
                Some(j) if !j.turn() && self.drag => {
                    // A drag race (0x422220's branch for 0x89E7D8 /
                    // 0x890108): while the new point lies under 3 m on,
                    // the list's last point is dropped (its segment added to
                    // the distance, so the new point keeps its place); a list
                    // of one counts from 0. Record 278's last point on 4213
                    // backward gives way to record 277's first (observed).
                    let mut d =
                        f.along(last.record, last.cp, last.backward, f.records[record].points[cp].position.truncate());
                    let mut base = last.s;
                    while d < 3.0 && !self.points.is_empty() {
                        let n = self.points.len();
                        if n < 2 {
                            d = 0.0;
                            base = 0.0;
                        } else {
                            let step = self.points[n - 1].s - self.points[n - 2].s;
                            d += step;
                            base -= step;
                        }
                        self.points.pop();
                    }
                    if self.points.is_empty() {
                        // A list of one counts from 0: the new point starts
                        // the list again.
                        self.current = 0;
                        self.turns.clear();
                        self.points.push(FuturePoint {
                            record,
                            cp,
                            backward: !forward,
                            s: 0.0,
                            offset: 0.0,
                            adjust: 0.0,
                            lane_adjust: [0.0; 4],
                            speed: [-1.0; 5],
                            inserted: None,
                        });
                        continue;
                    }
                    self.current = self.current.min(self.points.len() - 1);
                    self.turns.retain(|t| t.fp < self.points.len());
                    // The new point's place stays base + d; it now follows
                    // the list's new last point.
                    last = self.points.last().cloned().expect("a point");
                    base + d - last.s
                }
                Some(j) if !j.turn() => {
                    // Offline, points closer than 3 m ahead are walked past.
                    let mut d =
                        f.along(last.record, last.cp, last.backward, f.records[record].points[cp].position.truncate());
                    let mut n = 0;
                    while d < 3.0 && n < 100 {
                        let Some((r, c, fw, jj)) = self.step(f, record, cp, forward) else {
                            self.dead_end = true;
                            return;
                        };
                        (record, cp, forward) = (r, c, fw);
                        junction = jj.or(junction);
                        d = f.along(
                            last.record,
                            last.cp,
                            last.backward,
                            f.records[record].points[cp].position.truncate(),
                        );
                        n += 1;
                    }
                    d
                }
                Some(_) => f.records[record].points[cp].position.truncate().distance(prev_pos.truncate()),
            };
            let backward = !forward;
            let new = FuturePoint {
                record,
                cp,
                backward,
                s: last.s + d,
                offset: 0.0,
                adjust: 0.0,
                lane_adjust: [0.0; 4],
                speed: [-1.0; 5],
                inserted: None,
            };
            let turn = junction.is_some_and(|j| f.junctions[j].turn());
            if turn {
                // A turn entry (0x41EE20): the point before and its side.
                let din = f.direction(last.record, last.cp, last.backward);
                let dout = f.direction(record, cp, backward);
                let side = if din.perp_dot(dout) > 0.0 { 1.0 } else { -1.0 };
                if self.turns.len() < 5 {
                    self.turns.push(Turn { fp: self.points.len() - 1, s: (last.s + new.s) * 0.5, side });
                }
                self.last_turn = last.s;
                self.points.push(new);
                continue;
            }
            // 0x418C50: a record's last point (no junction, flags
            // 0x1F80000) within 5 m of the last point is skipped.
            let cpd = &f.records[record].points[cp];
            if cpd.junctions[0] == 0xFFFF && cpd.flags & 0x1F8_0000 == 0x1F8_0000 && record != last.record && d <= 5.0 {
                cursor = Some((record, cp, forward));
                continue;
            }
            // 0x418DA0: points every 8 m across the gap (trunc(d/8) − 1 of
            // them), along the last point's direction, smoothed onto a
            // Hermite curve (0x41A180, tangents 0.75 × the span). Every new
            // point that is not a turn entry and passes 0x418C50 gets them
            // (0x4224A7 / 0x4224B6 → 0x4226E1), junction or not: read under
            // Wine, the Quick Race grid's list held flagged (+0x74)
            // points 8 m after record 476 cp 2 → cp 1 (20.6 m) and record 655
            // cp 4 → 3 (17 m), inside one record. The clone had them across
            // junctions only.
            if d > 8.0 {
                let p0 = self.racing_point(f, &last);
                let p1 = self.line_point(f, &new, 0.0, 0.0);
                let t0 = f.direction(last.record, last.cp, last.backward);
                let t1 = f.direction(record, cp, backward);
                let span = p0.truncate().distance(p1.truncate());
                let n = (d / 8.0) as usize;
                for k in 1..n {
                    if self.points.len() >= MAX_POINTS - 1 {
                        break;
                    }
                    let t = (8.0 * k as f32) / d;
                    let h = hermite(p0.truncate(), t0 * span * 0.75, p1.truncate(), t1 * span * 0.75, t);
                    let z = p0.z + (p1.z - p0.z) * t;
                    self.points.push(FuturePoint {
                        offset: last.offset + 8.0 * k as f32,
                        s: last.s + 8.0 * k as f32,
                        inserted: Some(h.extend(z)),
                        speed: [-1.0; 5],
                        ..last.clone()
                    });
                }
            }
            self.points.push(new);
        }
    }

    /// The speeds (0x40A8C0, then the turns 0x422780).
    pub(super) fn speeds(&mut self, f: &RouteFile, class: usize, suv: bool, accel: &dyn Fn(f32) -> f32) {
        let c = class.min(3);
        let (decel, grip) = if suv { (DECEL_SUV[c], GRIP_SUV[c]) } else { (DECEL[c], GRIP[c]) };
        let first = self.points.iter().position(|p| (p.speed[0] + 1.0).abs() < 0.001).unwrap_or(self.points.len());
        for i in first..self.points.len() {
            let p = &self.points[i];
            let cp = &f.records[p.record].points[p.cp];
            let mut speed = [0.0; 5];
            speed[0] = f32::from(cp.lane_speed[lane(0, p.backward)]);
            for l in 0..4 {
                speed[l + 1] = f32::from(cp.lane_speed[lane(l + 2, p.backward)]);
            }
            self.points[i].speed = speed;
        }
        self.propagate(first, true, decel, accel);
        for t in self.turns.clone() {
            let (entry, exit) = (t.fp, t.fp + 1);
            if exit >= self.points.len() {
                continue;
            }
            let v_t = if self.points.len() < 2 || exit + 1 >= self.points.len() {
                TURN_CAP
            } else {
                // 0x41F8E0(turn, ±75, mode 1): the circle (0x5D4780 on entry
                // +0xDC, +0xE4, +0xEC) runs through the points midway to the
                // 75 m marks, s_t ± 37.5 (0x784260 × 75, 0x783C74), on the
                // point at or before (behind) / at or after (ahead) that s:
                // 8 m (0x783C9C) to the turn's side of its plain lane-0
                // line (extra = ±8 − its +0x04 adjust), kept 5 m (0x783C98)
                // inside that side's edge (vt+0x10 / vt+0xC: the half width
                // less the lane line). The middle point is the line at the
                // turn's place (the game's apex +0xE4 from 0x41EE20 is not
                // fully traced).
                let mid = |d: f32| {
                    let s = t.s + d * 0.5;
                    let p = if d < 0.0 {
                        self.points[..=entry].iter().rev().find(|p| p.s <= s).unwrap_or(&self.points[0])
                    } else {
                        self.points[exit..].iter().find(|p| p.s >= s).unwrap_or(&self.points[self.points.len() - 1])
                    };
                    let cp = &f.records[p.record].points[p.cp];
                    let base = lane_offset(cp, lane(0, p.backward));
                    let (left, right) = if p.backward {
                        (cp.half_width[1], cp.half_width[0])
                    } else {
                        (cp.half_width[0], cp.half_width[1])
                    };
                    let l = if t.side > 0.0 { (base + 8.0).min(right - 5.0) } else { (base - 8.0).max(5.0 - left) };
                    self.line_point(f, p, s - p.s + p.offset, l - base - p.adjust)
                };
                let a = mid(-75.0);
                let b = self.point_at(f, t.s, 0.0);
                let c3 = mid(75.0);
                turn_speed(grip, curvature(a.truncate(), b.truncate(), c3.truncate()))
            };
            let into = self.points[entry].s - t.s;
            let out = self.points[exit].s - t.s;
            let v_in = (v_t * v_t - 2.0 * decel * into).max(0.0).sqrt();
            let v_out = (v_t * v_t + 2.0 * accel(v_t) * out).max(0.0).sqrt();
            let e = &mut self.points[entry].speed;
            e[0] = e[0].min(v_in);
            for v in &mut e[1..] {
                *v = v.min(v_t - 0.2 * into);
            }
            let x = &mut self.points[exit].speed;
            x[0] = x[0].min(v_out);
            for v in &mut x[1..] {
                *v = v.min(v_t + 0.2 * out);
            }
            if entry > 0 {
                self.propagate(entry - 1, false, decel, accel);
            }
            self.propagate(exit + 1, true, decel, accel);
        }
    }

    /// 0x40A6E0: from `start`, each point is held to what the car can reach
    /// from its neighbour — forward with the car's acceleration, backward
    /// with the braking; the traffic lanes ±0.2 m/s per metre.
    pub(super) fn propagate(&mut self, start: usize, forward: bool, decel: f32, accel: &dyn Fn(f32) -> f32) {
        let n = self.points.len();
        if n == 0 {
            return;
        }
        let mut i = start as isize;
        let mut prev = if forward { start.saturating_sub(1) } else { (start + 1).min(n - 1) };
        while i >= 0 && (i as usize) < n {
            let iu = i as usize;
            let (vp, sp, lanes) = (self.points[prev].speed[0], self.points[prev].s, self.points[prev].speed);
            let ds = self.points[iu].s - sp;
            let a = if forward { accel(vp) } else { -decel };
            let reach = (2.0 * a * ds + vp * vp).max(0.0).sqrt();
            let p = &mut self.points[iu];
            p.speed[0] = p.speed[0].min(reach);
            let k = if forward { 0.2 } else { -0.2 };
            for l in 1..5 {
                p.speed[l] = p.speed[l].min(k * ds + lanes[l]);
            }
            prev = iu;
            i += if forward { 1 } else { -1 };
        }
    }
}

pub(super) fn hermite(p0: Vec2, t0: Vec2, p1: Vec2, t1: Vec2, t: f32) -> Vec2 {
    let (t2, t3) = (t * t, t * t * t);
    p0 * (2.0 * t3 - 3.0 * t2 + 1.0) + t0 * (t3 - 2.0 * t2 + t) + p1 * (-2.0 * t3 + 3.0 * t2) + t1 * (t3 - t2)
}

/// Signed curvature through three points (0x5D4780): one over the radius
/// of the circle through them (at least 1 mm), negative when they turn
/// clockwise; 0 when they are in line.
pub(super) fn curvature(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    let (u, v) = (a - b, c - b);
    let d = 2.0 * (u.x * v.y - u.y * v.x);
    if d.abs() < 1e-9 {
        return 0.0;
    }
    let (uu, vv) = (u.length_squared(), v.length_squared());
    let centre = Vec2::new(v.y * uu - u.y * vv, u.x * vv - v.x * uu) / d;
    let k = 1.0 / centre.length().max(0.001);
    if v.y * u.x - v.x * u.y < 0.0 { -k } else { k }
}

/// A turn's speed (0x5D46B0): √(g·9.8 / max(|κ|, g·9.8 / 60²)).
pub(super) fn turn_speed(grip: f32, kappa: f32) -> f32 {
    let floor = grip * 9.8 / (TURN_CAP * TURN_CAP);
    (grip * 9.8 / kappa.abs().max(floor)).sqrt()
}
