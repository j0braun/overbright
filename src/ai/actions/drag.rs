use super::*;

impl Actions {
    /// AvoidAvoidable in a drag race ("Win drag race", chooser 0x42D470):
    /// after GetUnstuck and the drag hit actions (types 0x12 / 0xF / 0xC,
    /// **not ported**), 0x42BA20 sweeps the path the car drives (`driven`,
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
        if scene.held {
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
                let path = route.lane_path(scene.routes, me.position, to, crate::ai::drag::AI_RATE);
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
        Some(Steer { path: driven.to_vec(), cap: Some(cap), pedals: true, reset: false })
    }
}
