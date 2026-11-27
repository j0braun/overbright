use super::*;

impl Actions {
    /// 0x408280 (the controller's vt+0x9C, asked before a pass or an
    /// avoid): an object refused within the last second is refused again;
    /// else a draw on the world's random numbers (0x890080 +0xF8, 0x5EA510)
    /// under the skill row's odds takes it, and a refusal is remembered.
    pub(super) fn willing(&mut self, id: usize, clock: f32, odds: f32, rng: &mut GameRng) -> bool {
        if self.refused.is_some_and(|(_, t)| clock - t >= REFUSE_FOR) {
            self.refused = None;
        }
        if self.refused.is_some_and(|(r, _)| r == id) {
            return false;
        }
        if rng.unit(1.0) < odds {
            return true;
        }
        self.refused = Some((id, clock));
        false
    }

    /// GetUnstuck runs (priority 500, chosen over the running action):
    /// the action is replaced, so the choice after it starts afresh (status
    /// done, as `think` with `fresh`), and its lane path is the driver's.
    pub fn get_unstuck(&mut self, path: &[Vec3]) {
        self.kind = Kind::JustDrive;
        self.priority = -1;
        self.target = None;
        self.last_path = path.to_vec();
    }

    /// One think: the action manager (0x429F56: status 0x403120, chooser
    /// 0x42D0F0, a proposal replacing the action only with a higher
    /// priority, or when the action is done) and the action's update; the
    /// path and speed for the driver. `fresh` replans from the car
    /// (GetUnstuck).
    #[allow(clippy::too_many_arguments)]
    pub fn think(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        tuning: &AiTuning,
        rng: &mut GameRng,
        fresh: bool,
        _dt: f32,
    ) -> Steer {
        let accel = scene.me.forward_accel;
        self.last_velocity = Some(scene.me.velocity);
        let (list, own) = entries(route, scene, accel);
        self.listed = list.iter().map(|e| e.car.id).collect();
        // 0x41D530's room margin takes the own entry's +0x08 (the planner's
        // entries of this think).
        if let Some(o) = own {
            route.own_across = list[o].across;
        }
        // The car's own entry is always in the original's list (0x6148F0
        // keeps the node whose car is the car), so a running
        // AccidentRecovery goes on to its reset (0x424210 reads no entry)
        // where the clone cannot place the car on its list (a car fallen
        // off the road). **Inferred** for the other actions, which
        // the clone still drops there.
        if own.is_none() && !fresh && self.kind == Kind::Recover {
            let steer = self.recover(route, scene, false);
            self.last_path = steer.path.clone();
            return steer;
        }
        let Some(own) = own.filter(|_| !fresh) else {
            // GetUnstuck (priority 500) runs instead; the next choice
            // starts afresh (status done).
            self.kind = Kind::JustDrive;
            self.priority = -1;
            self.target = None;
            let path = route.line_path(
                scene.routes,
                self.plan.rate,
                scene.me.position,
                scene.me.forward.extend(0.0),
                scene.me.velocity.length(),
                scene.drift,
                fresh,
                self.plan.offset,
            );
            self.last_path = path.clone();
            return Steer { path, cap: None, pedals: true, reset: false };
        };
        let done = self.done(route, scene, tuning, &list, own, accel, fresh);
        let (ty, pri) = if done { (0x16, -1) } else { (self.kind.type_id(), self.priority) };
        // The chooser (0x42D0F0) after GetUnstuck: IJustGotHit,
        // AvoidAvoidable, AccidentRecovery, Block, Pass, JustDrive; the
        // first proposal made is the one the manager weighs.
        let next = match self.goal {
            Goal::Race => self.choose(route, scene, tuning, rng, &list, own, accel, fresh, ty, pri),
            Goal::OffsetPursuit { target, .. } => self.choose_pursuit(scene, rng, &list, own, ty, pri, target),
        };
        let (kind, p, target, plan) = next;
        if p > pri {
            self.nitrous_request = false;
            if let Some(plan) = plan {
                self.plan = plan;
                self.stamp = None;
            }
            if kind == Kind::Recover {
                self.reset_made = false;
            }
            self.kind = kind;
            self.priority = p;
            self.target = target;
            self.start = scene.clock;
            self.since = scene.clock;
        }
        self.update_action(route, scene, &list, own, accel, fresh)
    }

    /// Action virtual +10: Block updates between full controller thinks;
    /// the other supported action vtables return false (404800).
    pub fn ready_between_thinks(&self) -> bool {
        self.kind == Kind::Block
    }

    /// 42ADA4 -> controller+B0: update the current action after a successful
    /// fallback route update. No completion tests, proposal or RNG draw.
    pub fn refresh(&mut self, route: &mut RacerRoute, scene: &Scene, _dt: f32) -> Option<Steer> {
        let accel = scene.me.forward_accel;
        let (list, own) = entries(route, scene, accel);
        let own = own?;
        self.last_velocity = Some(scene.me.velocity);
        Some(self.update_action(route, scene, &list, own, accel, false))
    }

    pub(super) fn update_action(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        accel: f32,
        fresh: bool,
    ) -> Steer {
        self.nitrous_query = false;
        let steer = match self.kind {
            Kind::JustDrive => {
                self.nitrous_query = true;
                let path = self.just_drive(route, scene, &list, own, fresh);
                Steer { path, cap: None, pedals: true, reset: false }
            }
            Kind::Pass => {
                let path = self.pass(route, scene, &list, own, fresh);
                Steer { path, cap: None, pedals: true, reset: false }
            }
            Kind::Block => {
                // 0x423A10 sets +0x1F0 = the cap, +0x1FC = 1: the line speed.
                let path = self.block(route, scene, &list, own, fresh);
                Steer { path, cap: None, pedals: true, reset: false }
            }
            Kind::GotHit => self.got_hit(route, scene, &list, own, fresh),
            Kind::Avoid => self.avoid(route, scene, &list, own, accel, fresh),
            Kind::Recover => self.recover(route, scene, fresh),
            Kind::OffsetPursuit => self.offset_pursuit(route, scene, list, own, fresh),
            // The drag race's own actions run in `drag_think` only.
            Kind::DragLane | Kind::DragWall | Kind::DragHit => {
                let path = self.drive(route, scene, fresh);
                Steer { path, cap: None, pedals: true, reset: false }
            }
        };
        self.last_path = steer.path.clone();
        steer
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn choose(
        &mut self,
        route: &RacerRoute,
        scene: &Scene,
        tuning: &AiTuning,
        rng: &mut GameRng,
        list: &[Entry],
        own: usize,
        accel: f32,
        fresh: bool,
        ty: i32,
        pri: i32,
    ) -> (Kind, i32, Option<usize>, Option<Plan>) {
        // IJustGotHit (0x42D1C9: type ≠ 0xB, priority ≤ 400; 0x403320: not
        // in the start hold, a racer in contact).
        if ty != 0xB
            && pri <= HIT_PRIORITY
            && !scene.contact_held
            && let Some(id) = hitter(list, own, scene.hits)
        {
            // 0x42D1F5 / 0x42D7F5: 0x40D5C0 before the constructor.
            racer_honk(rng, list, own, id);
            return (Kind::GotHit, HIT_PRIORITY, Some(id), None);
        }
        // AvoidAvoidable (0x42D22F: type ≠ 1, priority ≤ 300; 0x42BA20:
        // the first entry the path driven meets within 3 s; then the
        // controller's vt+0x9C).
        if ty != 1
            && pri <= AVOID_PRIORITY
            && let Some((id, t)) = sweep(&self.last_path, list, own, &scene.me, CLEAR_LOOK)
        {
            // 0x42D274 / 0x42D874: 0x40D5C0 before vt+0x9C.
            racer_honk(rng, list, own, id);
            if self.willing(id, scene.clock, scene.odds, rng) {
                self.reach = t;
                self.reach_end = t + AVOID_EXTRA;
                return (Kind::Avoid, AVOID_PRIORITY, Some(id), None);
            }
        }
        // AccidentRecovery (0x42D2D2: type ≠ 0xD, priority ≤ 500, fewer
        // than two wheels down and not upright, 0x403380).
        if ty != 0xD && pri <= RECOVER_PRIORITY && scene.wheels_down < 2 && scene.up_z <= UPRIGHT {
            return (Kind::Recover, RECOVER_PRIORITY, None, None);
        }
        if !scene.street_x
            && ty != 4
            && pri <= tuning.block_priority(scene.block_row)
            && let Some((id, p)) = find_block(scene, tuning, list, own)
        {
            return (Kind::Block, p, Some(id), None);
        }
        if ty != 0
            && pri <= PASS_PRIORITY
            && let Some((id, plan)) = find_pass(route, scene, list, own, self.plan, accel, fresh)
        {
            if self.willing(id, scene.clock, scene.odds, rng) {
                return (Kind::Pass, PASS_PRIORITY, Some(id), Some(plan));
            }
        }
        (Kind::JustDrive, 0, None, None)
    }

    pub(super) fn tracked<'a>(&self, list: &'a [Entry]) -> Option<&'a Entry> {
        let id = self.target?;
        list.iter().find(|e| e.car.id == id)
    }

    /// 0x403120: slot 8 (time limit), slot 7 (given up), slot 6 (done);
    /// any of them ends the action (0x401990).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn done(
        &mut self,
        route: &RacerRoute,
        scene: &Scene,
        tuning: &AiTuning,
        list: &[Entry],
        own: usize,
        accel: f32,
        fresh: bool,
    ) -> bool {
        let mine = &list[own];
        match self.kind {
            Kind::JustDrive => false,
            Kind::Pass => {
                let limit = scene.clock - self.start > PASS_LIMIT;
                // Slot 7 (0x40E4A0): out of reach more than 10 m ahead.
                let Some(e) = self.tracked(list) else { return true };
                let g = gap(list, own, e);
                let lost = scene.clock - self.since > PASS_SETTLE
                    && g > 0.0
                    && !reachable(&scene.me, accel, e, g)
                    && g > PASS_LOST;
                // Slot 6 (0x42B7F0): with the first entry not the target,
                // 0x42B5B0 may name another car, which is then passed
                // with its plan (+0x21C restarts); then done 3 s on once
                // the target is 10 m behind (or gone).
                if list.first().is_some_and(|f| Some(f.car.id) != self.target)
                    && let Some((id, plan)) = find_pass(route, scene, list, own, self.plan, accel, fresh)
                    && Some(id) != self.target
                {
                    self.plan = plan;
                    self.target = Some(id);
                    self.since = scene.clock;
                }
                let passed = scene.clock - self.since >= PASS_SETTLE
                    && self.tracked(list).is_none_or(|e| gap(list, own, e) < -(mine.along + e.along + PASS_CLEARED));
                limit || lost || passed
            }
            Kind::Block => {
                // Slot 7 (0x40E5D0): the target alongside or ahead.
                if let Some(e) = self.tracked(list)
                    && gap(list, own, e) + e.along + mine.along > 0.0
                {
                    return true;
                }
                // Slot 6 (0x41CC40): no follower to block any more; another
                // one is taken over.
                match find_block(scene, tuning, list, own) {
                    None => true,
                    Some((id, _)) => {
                        self.target = Some(id);
                        false
                    }
                }
            }
            // Slot 6 (0x41CC90): no racer in contact any more; another one
            // is taken over. Slots 7 / 8: never (0x403170, no time limit).
            Kind::GotHit => match hitter(list, own, scene.hits) {
                None => true,
                Some(id) => {
                    self.target = Some(id);
                    false
                }
            },
            // Slot 6 (0x42BA80): done with no target or no entry for it;
            // else the path driven is swept again (0x428CE0, 3 s): a hit
            // is taken as the new object (+0x21C its time); without one
            // the action ends once its time + 2 s (+0x22C) has run since
            // the start (+0x228) — for a traffic object only once it is
            // 10 m behind (0x785BAC). In a drag race a hit is taken only
            // prov: open ai.actions.done
            // when 0x4140E0 passes it (**open**; the clone takes none) and
            // the action ends otherwise. With a traffic object's +0x7A0
            // set (+0x220) the test is the object's road section's
            // prov: open ai.actions.done-2
            // (0x409320 / 0x409360 / 0x5D5CE0, **open**; the clone takes
            // +0x220 as clear).
            Kind::Avoid => {
                let expired = scene.clock - self.start >= self.reach_end;
                let Some(e) = self.tracked(list) else { return true };
                let hit = sweep(&self.last_path, list, own, &scene.me, CLEAR_LOOK);
                if scene.drag {
                    return true;
                }
                if let Some((id, t)) = hit {
                    self.target = Some(id);
                    self.reach = t;
                    return false;
                }
                if !e.car.racer && gap(list, own, e) >= AVOID_BEHIND {
                    return false;
                }
                expired
            }
            // Slot 6 (0x4033D0): the reset asked for, or all four wheels
            // down and upright.
            Kind::Recover => self.reset_made || (scene.wheels_down == 4 && scene.up_z > UPRIGHT),
            Kind::OffsetPursuit => false,
            Kind::DragLane | Kind::DragWall | Kind::DragHit => true,
        }
    }

    /// 0x42B340: the plain line once clear, else the kept plan.
    pub(super) fn just_drive(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        fresh: bool,
    ) -> Vec<Vec3> {
        let plain = Plan::plain(scene.rate);
        let mut least = 99_999.0f32;
        for e in &list[..own] {
            if e.closing.abs() > 0.1 {
                let t = -1.0 / e.closing * gap(list, own, e);
                if t > 0.0 && t < least {
                    least = t;
                }
            }
        }
        let differs =
            self.plan.lane != 0 || self.plan.offset.abs() > 0.001 || (self.plan.rate - scene.rate).abs() > 0.001;
        if least > JUST_DRIVE_CLEAR && differs {
            let path = trial(route, scene, plain, fresh);
            if clear(&path, list, own, &scene.me, CLEAR_LOOK) {
                self.plan = plain;
            }
        }
        if self.plan.lane != 0 {
            self.plan = plain;
        }
        self.drive(route, scene, fresh)
    }

    /// 0x42B940: the plan beside the target again while it is ahead.
    /// Its 0x40E390 (nitrous, the target's |+0x04| ≤ 5) asks the
    /// controller's slot +0xA0 (0x416F70); encounter callers consume the
    /// retained query/request state.
    pub(super) fn pass(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        fresh: bool,
    ) -> Vec<Vec3> {
        if let Some(e) = self.tracked(list)
            && gap(list, own, e) > 0.0
        {
            self.nitrous_query = e.closing.abs() <= 5.0;
            let mut plan = self.plan;
            if pass_plan(route, scene, list, own, e, &mut plan, fresh) {
                self.plan = plan;
            }
        }
        self.drive(route, scene, fresh)
    }

    /// 0x423A10: onto the follower's offset, at 0.15 within 10 m, else by
    /// the curve over the time to reach. Its 0x5ECF30({255, 0, 0, 0}, 1)
    /// sets the car's debug tint (car +0x97C / +0x980…, read only by the
    /// state dump 0x5FBB97) and +0x1F8 = 2 goes to the driver's +0x238
    /// (0x4054A0), which only the dumps 0x405A80 / 0x415DD0 read: neither
    /// changes the driving.
    pub(super) fn block(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        fresh: bool,
    ) -> Vec<Vec3> {
        let mut offset = 0.0;
        let mut rate = BLOCK_RATE;
        if let Some(e) = self.tracked(list) {
            offset = e.at.lateral;
            let g = gap(list, own, e);
            if g >= 10.0 && e.closing.abs() > 0.001 {
                rate = crate::ai::route::pairs(&BLOCK_CURVE, g / e.closing);
            }
        }
        if self.plan.lane != -1 {
            self.plan.rate = rate;
            self.plan.offset = offset;
        }
        self.drive(route, scene, fresh)
    }

    /// 0x423E20 (IJustGotHit's update): away from the hitter. With its
    /// entry the plan offset becomes, when the hitter is not behind by
    /// more than both +0x0C less 2.8 m, the least / greatest of the kept
    /// offset and the hitter's offset ∓ (both +0x08 + 5) on the side away
    /// from it (0x60C020: its offset less the car's, ≥ 0 → minus), at rate
    /// 0.25; behind, the kept offset at 0.05; no entry: offset 0 at 0.25.
    /// The speed (outside Street X and drift, with the entry): the human
    /// hitter ahead by more than both +0x0C less 2.8 m is followed at its
    /// speed (0x402940(…, 0, 0.2): +0x1F0 = |v|, +0x1F4 = its bumper gap /
    /// closing, at least 0.1), behind it the cap, beside it the line
    /// speed; a rival hitter the cap. Unless the line speed, under 20 m/s
    // prov: open ai.actions.got_hit
    /// it is 20 once the race has run 10 s. **Open**: 0x40DF50(car, 0) =
    /// 1 (the car's road-edge state, taken as never) with the hitter ahead
    /// slows by 0.2 s of braking (0x40DEE0).
    pub(super) fn got_hit(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        fresh: bool,
    ) -> Steer {
        let mine = &list[own];
        let e = self.tracked(list).copied();
        let (mut offset, mut rate) = (0.0, HIT_RATE);
        let (mut ahead, mut behind) = (false, false);
        if let Some(e) = e.as_ref() {
            let (tgt, me) = (e.at.lateral, mine.at.lateral);
            let away = if tgt - me < 0.0 {
                self.plan.offset.max(tgt + HIT_CLEAR + mine.across + e.across)
            } else {
                self.plan.offset.min(tgt - mine.across - e.across - HIT_CLEAR)
            };
            let g = gap(list, own, e);
            let sum = e.along + mine.along;
            ahead = sum - HIT_OVERLAP < g;
            behind = g < HIT_OVERLAP - sum;
            if behind {
                offset = self.plan.offset;
                rate = PASS_RATE;
            } else {
                offset = away;
            }
        }
        if self.plan.lane != -1 {
            self.plan.offset = offset;
            self.plan.rate = rate;
        }
        let path = self.drive(route, scene, fresh);
        let mut cap = None;
        if !scene.street_x
            && !scene.drift
            && let Some(e) = e.as_ref()
        {
            cap = Some(if e.car.human {
                if ahead {
                    e.car.velocity.length().max(0.0)
                } else if behind {
                    CAP
                } else {
                    return Steer { path, cap: None, pedals: true, reset: false };
                }
            } else {
                CAP
            });
        }
        if let Some(c) = cap.as_mut()
            && *c < HIT_FLOOR
            && scene.clock > HIT_FLOOR_AFTER
        {
            *c = HIT_FLOOR;
        }
        Steer { path, cap, pedals: true, reset: false }
    }

    /// 0x42F330 (AvoidAvoidable's update, the "Win road race" goal: slot
    /// +0x20 = 1 → 0x428FD0). With the object's entry and outside the
    /// start hold: the avoid plan (first side: minus when the object's
    /// +0x00 > 0; the slow test, +0x42C < 25 mph, 0x783734). When that
    /// fails and the object is neither traffic (0x6148D0) nor ghosted
    /// (0x60BF80 → 0x5EBFB0, the car's ghost timer +0x974 / +0x978,
    /// **verified**): with +0x21C ≈ 0 (0x4010C0, 0.001) it is
    /// "near" when ahead; else near when the centre gap in +0x21C seconds
    /// ((a·t/2 + closing)·t + gap, a = 0x5EC3B0) is over 3 m, "far"
    /// otherwise. Near (not Street X / drift): the cap is the speed less a
    /// quarter of the closing when closing (at least 20). Far: twice the
    /// cap, with a nitrous ask (0x40E390). Otherwise the line
    /// speed (the cap in a drag race).
    pub(super) fn avoid(
        &mut self,
        route: &mut RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        accel: f32,
        fresh: bool,
    ) -> Steer {
        let e = self.tracked(list).copied();
        let (mut near, mut far) = (false, false);
        if let Some(e) = e.as_ref()
            && !scene.contact_held
        {
            let minus_first = e.rel_lateral > 0.0;
            // 0x42F330: slow = +0x42C (the forward speed) under 11.18 m/s
            // (0x783734, 25 mph).
            let slow = scene.forward_speed < AVOID_SLOW;
            let ok = self.avoid_plan(route, scene, list, own, minus_first, slow, fresh);
            // 0x42F330: unless the plan was found, or the object is traffic
            // (0x6148D0) or ghosted (0x60BF80 → 0x5EBFB0: car +0x974 /
            // +0x978), the near / far speed.
            if !ok && !e.car.traffic && !e.car.ghosted {
                if self.reach.abs() <= 0.001 {
                    near = gap(list, own, e) > 0.0;
                } else {
                    let t = self.reach;
                    let d = (accel * t * 0.5 + e.closing) * t + gap(list, own, e);
                    if d > AVOID_AHEAD {
                        near = true;
                    } else {
                        far = true;
                    }
                }
            }
        }
        // The path on the plan as it now stands (0x420D40).
        let path = self.drive(route, scene, fresh);
        let special = scene.street_x || scene.drift;
        let cap = if near && !special {
            let c = e.map_or(0.0, |e| e.closing.min(0.0));
            Some((scene.me.velocity.length() + AVOID_CLOSING * c).max(AVOID_FLOOR))
        } else if far {
            self.nitrous_query = true;
            Some(2.0 * CAP)
        } else if scene.drag {
            Some(CAP)
        } else {
            None
        };
        Steer { path, cap, pedals: true, reset: false }
    }

    /// 0x424210 (AccidentRecovery's update): the path on the kept plan, a
    /// speed of 0 with the pedals off (+0x1F0 = 0, +0x1FD = 0), and 5 s
    /// after the start the reset request once (0x5EBF70: car +0x96C,
    /// +0x214).
    pub(super) fn recover(&mut self, route: &mut RacerRoute, scene: &Scene, fresh: bool) -> Steer {
        let path = self.drive(route, scene, fresh);
        let mut reset = false;
        if scene.clock - self.start >= RECOVER_RESET && !self.reset_made {
            self.reset_made = true;
            reset = true;
        }
        self.reset_pending |= reset;
        Steer { path, cap: Some(0.0), pedals: false, reset }
    }

    pub(super) fn drive(&self, route: &mut RacerRoute, scene: &Scene, fresh: bool) -> Vec<Vec3> {
        let me = &scene.me;
        route.line_path(
            scene.routes,
            self.plan.rate,
            me.position,
            me.forward.extend(0.0),
            me.velocity.length(),
            scene.drift,
            fresh,
            self.plan.offset,
        )
    }
}
