use super::*;

/// The summed pushes (contact record +0x60) a racer contact needs before
/// the drag got-hit action is proposed: 0x403410 asks 0x402AE0 with
/// 0x78373C = 8.3333 (the road race's IJustGotHit asks with −1, any).
pub const DRAG_HIT_IMPULSE: f32 = f32::from_bits(0x4105_5532);
/// 0x403450's priority while a wall contact is live (0x12C).
pub const DRAG_WALL_PRIORITY: i32 = 300;
/// 0x403410's priority with a hitter (0x190).
pub const DRAG_HIT_PRIORITY: i32 = 400;

/// The contacts the drag race's chooser reads besides the road entries.
#[derive(Clone, Copy, Debug, Default)]
pub struct DragTouch<'a> {
    /// A live contact record against an upright track polygon (0x403450:
    /// 0x5A0170 on each of the body's records, car +0x3C, count +0x18).
    pub wall: bool,
    /// The racers whose contact record with the car has summed pushes of
    /// at least [`DRAG_HIT_IMPULSE`] (0x402AE0's record +0x60 test).
    pub hard: &'a [usize],
}

impl Actions {
    /// GetUnstuck replaced the drag action (priority 500): the next choice
    /// starts afresh.
    pub fn drag_reset(&mut self) {
        self.kind = Kind::DragLane;
        self.priority = -1;
        self.target = None;
    }

    /// The "Win drag race" goal's think (chooser 0x42D470, its actions'
    /// done tests and updates) after GetUnstuck, which the caller runs.
    /// The running action's done test (slot 6) first: the wall action
    /// 0x4034A0 once no wall contact is live, the got-hit action 0x41CC90
    /// once 0x402AE0(planner, −1, 1) finds no racer in contact (a new one
    /// is taken over), AvoidAvoidable at once (see `drag_avoid`). Then the
    /// chooser's first proposal, which replaces the action only with a
    /// higher priority (or when it is done): type 0x12 ("Pick route to
    /// avoid animation", 600) needs controller +0x780, which only the
    /// constructors write (0x4205A4, 0x4261BF: 0), so it is never proposed;
    /// a live wall contact gives the wall action (300); a racer contact
    /// over [`DRAG_HIT_IMPULSE`] the got-hit action (400); then, with the
    /// running action not AvoidAvoidable and its priority at most 300,
    /// AvoidAvoidable; else the lane action (0). The clone runs this on
    /// the controller's full think only (the caller keeps the running
    /// action's last steer between them).
    /// `lane_path` is the lane action's path (`DragLane::ai_path`). Returns
    /// the steer of the action that runs; None for the lane action's path.
    #[allow(clippy::too_many_arguments)]
    pub fn drag_think(
        &mut self,
        route: &RacerRoute,
        scene: &Scene,
        lane: &mut crate::ai::drag::DragLane,
        driven: &[Vec3],
        touch: &DragTouch,
        rng: &mut GameRng,
    ) -> Option<Steer> {
        let accel = scene.me.forward_accel;
        // An action has thought (the controller's full-think schedule reads it).
        self.last_velocity = Some(scene.me.velocity);
        let (list, own) = entries(route, scene, accel);
        let done = match self.kind {
            Kind::DragWall => !touch.wall,
            Kind::DragHit => {
                self.target = own.and_then(|o| hitter(&list, o, scene.hits));
                self.target.is_none()
            }
            Kind::DragLane => false,
            _ => true,
        };
        let (ty, pri) = if done { (0x16, -1) } else { (self.kind.type_id(), self.priority) };
        let hard = own.and_then(|o| hitter(&list, o, touch.hard));
        let proposal = if touch.wall {
            Some((Kind::DragWall, DRAG_WALL_PRIORITY, None))
        } else if let Some(h) = hard {
            Some((Kind::DragHit, DRAG_HIT_PRIORITY, Some(h)))
        } else if ty != Kind::Avoid.type_id() && pri <= AVOID_PRIORITY {
            // AvoidAvoidable's proposal and, taken (300 over the lane
            // action or a done action), its update.
            if let Some(steer) = self.drag_avoid(route, scene, lane, driven, rng) {
                self.kind = Kind::Avoid;
                self.priority = AVOID_PRIORITY;
                self.start = scene.clock;
                return Some(steer);
            }
            None
        } else {
            None
        };
        let (kind, p, target) = proposal.unwrap_or((Kind::DragLane, 0, None));
        if p > pri {
            self.kind = kind;
            self.priority = p;
            self.target = target;
            self.start = scene.clock;
        }
        match self.kind {
            Kind::DragWall => Some(self.drag_wall(route, scene, lane, driven)),
            Kind::DragHit => Some(self.drag_hit(route, scene, lane, driven, &list, own)),
            _ => None,
        }
    }

    /// 0x4249C0, the wall action's update: the lane nearest the car
    /// (0x40F4A0 at the car's place) becomes the record's lane with offset
    /// 0 and rate 0.151; the path on the record (0x420D40), the cap as
    /// speed (+0x1F0 = controller vt+0x7C, +0x1FC = 0), pedals on.
    fn drag_wall(&mut self, route: &RacerRoute, scene: &Scene, lane: &mut crate::ai::drag::DragLane, driven: &[Vec3]) -> Steer {
        let me = &scene.me;
        if let Some(l) = route.nearest_lane(scene.routes, me.position, None) {
            lane.lane = Some(l);
            lane.rate = crate::ai::drag::AI_RATE;
        }
        let path = lane.record_path(route, scene.routes, me.position, me.half.y).unwrap_or_else(|| driven.to_vec());
        self.last_path = path.clone();
        Steer { path, cap: Some(CAP), pedals: true, reset: false }
    }

    /// 0x4246B0, the got-hit action's update. With the hitter's entry
    /// (+0x218): the car's lane L (0x40F4A0 at its place) and the hitter's
    /// (at the hitter's place); when both exist, x = the hitter's offset
    /// less the car's (0x60C020). Another lane: the car keeps L. The same
    /// lane: `yield_` (bl) is set when the car is off L's centre by at
    /// least its half width (entry +0x08) and no less than the hitter (else
    /// clear when the hitter is not the human), or, the car within its half
    /// width and the hitter within its own, when the hitter is not behind
    /// (0x60C000 ≥ 0); the car within and the hitter not falls to the first
    /// test. The lane on the hitter's side (L − 1 for x < 0, else L + 1) is
    /// asked of 0x412500; allowed and not `yield_`, the car keeps L; else
    /// the lane on the other side when 0x412500 allows it, else L. The
    /// record becomes (lane, 0, 0.151) (0x405410). Then the path on the
    /// record (0x420D40), the cap as speed, pedals on. The hitter's lane
    /// and offsets are read on the car's own route point (**inferred**: the
    /// original reads the hitter's at its own road point).
    fn drag_hit(
        &mut self,
        route: &RacerRoute,
        scene: &Scene,
        lane: &mut crate::ai::drag::DragLane,
        driven: &[Vec3],
        list: &[Entry],
        own: Option<usize>,
    ) -> Steer {
        let me = &scene.me;
        let routes = scene.routes;
        if let Some(own) = own
            && let Some(e) = self.tracked(list).copied()
            && let Some(l) = route.nearest_lane(routes, me.position, None)
            && let Some(lh) = route.nearest_lane(routes, e.car.position, None)
        {
            let mine = &list[own];
            let x = e.at.lateral - mine.at.lateral;
            let mut to = l;
            if l == lh {
                let a = route.lane_lateral(routes, me.position, l).abs();
                let b = route.lane_lateral(routes, e.car.position, l).abs();
                let yield_ = if mine.across > a && b < e.across {
                    gap(list, own, &e) >= 0.0
                } else {
                    !(a < b) || e.car.human
                };
                let allowed = |to: Option<usize>| {
                    to.filter(|t| (2..=5).contains(t))
                        .is_some_and(|t| crate::ai::drag::lane_allowed(route, routes, l, t, true, true))
                };
                let (near, far) = if x < 0.0 { (l.checked_sub(1), Some(l + 1)) } else { (Some(l + 1), l.checked_sub(1)) };
                if (!allowed(near) || yield_) && allowed(far) {
                    to = far.unwrap_or(l);
                }
            }
            lane.lane = Some(to);
            lane.rate = crate::ai::drag::AI_RATE;
        }
        let path = lane.record_path(route, routes, me.position, me.half.y).unwrap_or_else(|| driven.to_vec());
        self.last_path = path.clone();
        Steer { path, cap: Some(CAP), pedals: true, reset: false }
    }

    /// AvoidAvoidable in a drag race ("Win drag race", chooser 0x42D470):
    /// after GetUnstuck and the drag hit actions (types 0x12 / 0xF / 0xC,
    /// see `drag_think`), 0x42BA20 sweeps the path the car drives (`driven`,
    /// car +0x30 → +0x10) for 3 s and takes the first road entry it meets
    /// (priority 300; no willingness draw here, 0x40D5C0 only honks /
    /// flashes at a human). Its update 0x42F330 then, outside the start
    /// hold, asks the goal's lane class (slot +0x20 = 2) for 0x42BF00: from
    /// the lane record's lane (the nearest, 0x420ED0, when not 2–5) the
    /// lanes one, two and three away, the lower one first when the object's
    /// +0x00 > 0 (`minus_first`), each through 0x428E70: every lane stepped
    /// through allowed (0x412500), the lane's path at 0.151 clear of the
    /// entries for 3 s (0x428DF0) — the first such lane becomes the record
    /// (lane, 0, 0.151). With none, a racer object gives the near / far
    /// speed (as `avoid`); a traffic object (0x6148D0) keeps the line
    /// speed (+0x1F0 = the cap in a drag race): the car drives on. The
    /// action ends at the next think in the clone (0x42BA80's drag test
    // prov: open ai.actions.drag_avoid
    /// 0x4140E0 is **open**, see `done`), so the chooser sweeps again each
    /// think. Returns None when nothing is met (the lane action drives on).
    pub fn drag_avoid(
        &mut self,
        route: &RacerRoute,
        scene: &Scene,
        lane: &mut crate::ai::drag::DragLane,
        driven: &[Vec3],
        rng: &mut GameRng,
    ) -> Option<Steer> {
        if scene.contact_held {
            return None;
        }
        let accel = scene.me.forward_accel;
        let (list, own) = entries(route, scene, accel);
        let own = own?;
        let me = &scene.me;
        let (id, t) = sweep(driven, &list, own, me, CLEAR_LOOK)?;
        // 0x42D68C: 0x40D5C0 before the constructor.
        racer_honk(rng, &list, own, id);
        let e = *list.iter().find(|e| e.car.id == id)?;
        let from = lane
            .lane
            .filter(|l| (2..=5).contains(l))
            .or_else(|| route.nearest_lane(scene.routes, me.position, None))?;
        let minus_first = e.rel_lateral > 0.0;
        for k in 1..=3usize {
            let (minus, plus) = (from.checked_sub(k), Some(from + k));
            let order = if minus_first { [minus, plus] } else { [plus, minus] };
            for to in order.into_iter().flatten() {
                if !(2..=5).contains(&to) || !crate::ai::drag::lane_allowed(route, scene.routes, from, to, true, true) {
                    continue;
                }
                let path = route.lane_path(scene.routes, me.position, to, crate::ai::drag::AI_RATE, crate::ai::drag::ROOM_MARGIN + me.half.y);
                if clear(&path, &list, own, me, CLEAR_LOOK) {
                    lane.lane = Some(to);
                    lane.rate = crate::ai::drag::AI_RATE;
                    self.kind = Kind::Avoid;
                    self.target = Some(id);
                    self.last_path = path.clone();
                    return Some(Steer { path, cap: None, pedals: true, reset: false });
                }
            }
        }
        if !e.car.racer {
            return None;
        }
        // prov: open ai.actions.drag_avoid-2
        // 0x42F330's near / far for a racer object (0x60BF80 **open**,
        // taken as false).
        let near = if t.abs() <= 0.001 {
            gap(&list, own, &e) > 0.0
        } else {
            (accel * t * 0.5 + e.closing) * t + gap(&list, own, &e) > AVOID_AHEAD
        };
        let cap = if near {
            (me.velocity.length() + AVOID_CLOSING * e.closing.min(0.0)).max(AVOID_FLOOR)
        } else {
            self.nitrous_query = true;
            2.0 * CAP
        };
        // 0x42BF00 found no lane: it rebuilds the action's path on the
        // controller's lane record (0x42BFA9 → 0x420D40) from where the car
        // is now, every update; the path driven before is not kept.
        let path = lane.record_path(route, scene.routes, me.position, me.half.y).unwrap_or_else(|| driven.to_vec());
        self.last_path = path.clone();
        Some(Steer { path, cap: Some(cap), pedals: true, reset: false })
    }
}
