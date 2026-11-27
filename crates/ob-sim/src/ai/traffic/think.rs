use super::*;

/// The traffic controller's stuck test (0x408DE0, **verified** in the
/// decompile and the asm: run by the think 0x429D90 on every world update
/// for every car not paused, before the turn gate): while the car's
/// collision body has live contact records (body +0x3C → +0x18, walked by
/// 0x5A0830), a record whose summed pushes (+0x60) exceed 30 (0x783B8C),
/// or the car under 1 m/s (+0x430, 0x784250) while its driver's target
/// (driver vt+0x74 = 0x40FFF0: +0x230) is over 1 m/s, stops it for good:
/// 0x4023E0 aims the driver 1 m ahead along the heading, the driver's
/// speed slot +0x38 (0x4054A0) gets (0, 0, 0, 1) — target 0 — and the
/// controller's +0x710 is set. From then on every think (0x429E50 →
/// 0x429F2B) does only that; the placement clears it (0x5FBEE0 → slot
/// +0x34, 0x42A050). The car is not parked (+0x77C), so it stays placed
/// while within 250 m of the player and counts for the jam test.
pub const STUCK_IMPULSE: f32 = 30.0;

pub const STALL_SPEED: f32 = 1.0;

pub const STALL_TARGET: f32 = 1.0;

/// 0x417750's horizon (**verified**, asm 0x417750–0x41787F): a think whose
/// walk ends on a point with a low-nibble-3 junction in its direction
/// stops the car for good (+0x710, as [`STUCK_IMPULSE`]) once that point is
/// under 1 s away (its +0x54 less the car's, over max(speed, 0.1)).
pub const END_STOP_TIME: f32 = 1.0;

impl TrafficLane {
    /// One think of the traffic controller (0x429D90; see [`think_slot`]
    /// for when): leave every intersection (0x413800), update the lane
    /// (`update`), register at the walk's first intersection ahead
    /// (0x417890), and while there is one run "Obey traffic intersections"
    /// (type 0x11, priority 600, proposed by the traffic goal's 0x426D70
    /// through 0x4149E0, done when the walk holds no intersection, 0x41CE30)
    /// instead of "Drive Traffic Lane" (type 7, 0x4242D0): the same path,
    /// its +0x1F0 cap lowering the goal's target speed (0x425150). With no
    /// intersection ahead and a car ahead ([`TrafficLane::car_ahead`]) it
    /// runs "Avoid Cars as a traffic car" (type 2, priority 300, 0x4145B0 →
    /// 0x424CB0, [`TrafficLane::avoid_update`]) until it is done
    /// ([`TrafficLane::avoid_done`], 0x41CD10). `own` is the car itself
    /// (its velocity, forward and box; its `position` the car's).
    #[allow(clippy::too_many_arguments)]
    pub fn think(
        &mut self,
        f: &RouteFile,
        me: usize,
        position: Vec3,
        speed: f32,
        own: &Other,
        crossings: &mut Crossings,
        others: &[Other],
        rng: &mut crate::opponents::GameRng,
    ) -> Option<(Vec<Vec3>, f32)> {
        crossings.leave(me);
        self.heading = Some(own.forward);
        let (path, target) = self.plan(f, position, speed, true)?;
        let p = position.truncate();
        let own = Other { position: p, z: position.z, ..*own };
        let found = (0..self.steps.len()).find_map(|k| {
            let s = self.steps[k];
            f.intersection_at(&f.records[s.record].points[s.cp]).map(|x| (k, x))
        });
        let Some((k, x)) = found else {
            self.obey = None;
            let entries = self.entries(f, &own, speed, others);
            if self.avoid.is_some_and(|a| self.avoid_done(f, &own, speed, &entries, a)) {
                self.avoid = None;
            }
            if self.avoid.is_none() {
                // The goal's proposal (0x4145B0): 0x402ED0 with look 99999,
                // so any object in the window is taken at time 0, then its
                // time within 5 s and 0x4140E0's keep (no intersection
                // ahead here: kept). Its pass for a racer over 20 m/s needs
                // the goal's kind 2 (slot +0x68 → 0x40C9C0); the traffic's
                // "Be traffic car" goal (0x7848A0) is kind 1, so it never
                // applies (docs/formats/ai.md "Actions").
                let near = self
                    .car_ahead(f, &own, speed, &entries, None, NEVER)
                    .filter(|s| s.time <= AVOID_LANE_TIME)
                    .map(|s| Avoid { target: s.id, time: s.time, look: AVOID_LOOK, asked: MOVER_TOP_SPEED });
                // 0x426CFF (the traffic goal 7's chooser 0x426C20; the
                // ambient goal 8's 0x426D70 is the same code, its call at
                // 0x426E4F): 0x40D5C0
                // before the action's constructor at the human's car (own
                // kind 4: always).
                if let Some(e) = near.and_then(|a| entries.iter().find(|e| e.other.id == a.target)).filter(|e| e.other.human) {
                    crate::rng_sites::honk(rng, e.closing, true);
                }
                self.took_avoid |= near.is_some();
                self.avoid = near;
            }
            if self.avoid.is_none() {
                return Some((path, target));
            }
            let limit = self.avoid_update(f, &own, speed, &entries);
            // A lane change rebuilds the path on the new lane.
            let (path, target) = self.update(f, position, speed)?;
            return Some((path, target.min(limit)));
        };
        let s0 = self.steps[0];
        let mut at = -f.along(s0.record, s0.cp, s0.backward, p);
        for w in 0..k {
            at += self.seg(f, w);
        }
        let step = self.steps[k];
        let id = f.records[step.record].id as i16;
        let road = f.intersections[x].roads.iter().position(|&r| r == id);
        let d = at - stop_back(f, x, road, step.backward) - STOP_LINE;
        // 0x417890: the distance and the time to the line (max(d / v, 0);
        // 0 past it, 99999 standing).
        let time = if d <= 0.0 {
            0.0
        } else if speed <= 0.001 {
            99_999.0
        } else {
            (d / speed).max(0.0)
        };
        if let Some(road) = road {
            crossings.slots[x][me] = Some(Slot { road, dist: d, time });
        }
        // The action: a new intersection starts it over (0x41CE30).
        let stopped = match self.obey {
            Some((y, stopped)) if y == x => stopped,
            _ => false,
        };
        let cap = MOVER_TOP_SPEED;
        let mut limit = cap;
        // A car ahead (0x425150: 0x402ED0 with look 30 m, then 0x4140E0,
        // which drops a traffic car crossing on the other road; a dropped
        // one is not passed over) whose time is under 1.5 s or distance
        // under 30 m: stop within 15 m, else at most 6 m/s.
        self.avoid = None;
        let entries = self.entries(f, &own, speed, others);
        let ahead = self
            .car_ahead(f, &own, speed, &entries, None, AHEAD_LOOK)
            .filter(|s| {
                !others
                    .iter()
                    .find(|o| o.id == s.id)
                    .and_then(|o| o.traffic)
                    .is_some_and(|t| crossings.crossing(x, me, t))
            })
            .filter(|s| s.time < AVOID_TIME || s.dist < AHEAD_LOOK);
        if let Some(s) = ahead {
            limit = if s.dist <= AHEAD_STOP { limit.min(0.0) } else { limit.min(CROSSING_CRAWL) };
        }
        let kind = f.intersections[x].kind;
        let wait = !stopped && (kind == 3 || (kind == 2 && road == Some(1)));
        let go = !wait && road.is_some_and(|r| crossings.may_go(x, kind, r));
        if go {
            crossings.claim(x, road.unwrap_or(0));
            limit = limit.min(cap);
        } else {
            limit = if d <= CROSSING_SLOW { limit.min(CROSSING_CRAWL) } else { cap.min(limit) };
        }
        let mut stopped = stopped;
        if !go && d < 0.0 {
            limit = limit.min(0.0);
            if speed < 0.5 {
                stopped = true;
            }
        }
        self.obey = Some((x, stopped));
        Some((path, target.min(limit)))
    }
}
