use super::*;

/// 0x41D2D0: which side to pass a target on (1 the side the car is on,
/// 2 the left / minus side, 3 the right).
pub(super) fn side(route: &RacerRoute, scene: &Scene, list: &[Entry], own: usize, e: &Entry) -> u8 {
    let mine = &list[own];
    let (tgt, me) = (e.at.lateral, mine.at.lateral);
    let ahead = (e.at.s - mine.at.s) - (mine.along + SIDE_CLEAR + e.along) > 0.0;
    // The tests are skipped (side 3) only for a controller of class 3
    // (car +0x14 → +4) with the target under 5 m to the right; a racer is
    // class 2 (the human 1), so they always run here.
    let bend = route.bend(scene.routes);
    let side = if me.abs() > 1.0 || tgt.abs() > 1.0 || gap(list, own, e) <= 50.0 || bend.abs() < 0.12 {
        if (me - tgt).abs() < 2.0 && ahead {
            1
        } else if me <= tgt {
            2
        } else {
            3
        }
    } else if bend <= 0.0 {
        2
    } else {
        3
    };
    let need = mine.across + e.across + SIDE_CLEAR;
    let left_room = e.at.room.0 <= tgt - need;
    let right_room = e.at.room.1 >= tgt + need;
    if (side == 1 || side == 2) && !left_room {
        if right_room && ahead {
            return 3;
        }
    } else if (side == 1 || side == 3) && !right_room && left_room && ahead {
        return 2;
    }
    side
}

/// 0x429890: the pass plan beside a target (its offset ± both +0x08 +
/// 3 m on the side 0x41D2D0 picks, kept while the plan already clears
/// it; rate 0.05), then 0x428DF0 over 3 s.
#[allow(clippy::too_many_arguments)]
pub(super) fn pass_plan(
    route: &RacerRoute,
    scene: &Scene,
    list: &[Entry],
    own: usize,
    e: &Entry,
    plan: &mut Plan,
    fresh: bool,
) -> bool {
    let mine = &list[own];
    let side = side(route, scene, list, own, e);
    let need = mine.across + e.across + PASS_SIDE;
    let tgt = e.at.lateral;
    let mut offset = plan.offset;
    if need.abs() > (tgt - plan.offset).abs() {
        offset = match side {
            2 => tgt - need,
            3 => tgt + need,
            _ if tgt - mine.at.lateral > 0.0 => tgt - need,
            _ if tgt - mine.at.lateral < 0.0 => tgt + need,
            _ => plan.offset,
        };
    }
    if plan.lane != -1 {
        plan.offset = offset;
        plan.rate = PASS_RATE;
    }
    let path = trial(route, scene, *plan, fresh);
    clear(&path, list, own, &scene.me, CLEAR_LOOK)
}

/// 0x420D40 on a copy of the racer's path state with a plan.
pub(super) fn trial(route: &RacerRoute, scene: &Scene, plan: Plan, fresh: bool) -> Vec<Vec3> {
    let mut r = route.clone();
    let me = &scene.me;
    r.line_path(
        scene.routes,
        plan.rate,
        me.position,
        me.forward.extend(0.0),
        me.velocity.length(),
        scene.drift,
        fresh,
        plan.offset,
    )
}

/// The entry indices 0x42B5B0 examines, in its order: count − 1 down to 0.
pub(super) fn pass_candidates(count: usize) -> std::iter::Rev<std::ops::Range<usize>> {
    (0..count).rev()
}

/// 0x42B5B0: a car ahead to pass and the plan for it.
pub(super) fn find_pass(
    route: &RacerRoute,
    scene: &Scene,
    list: &[Entry],
    own: usize,
    plan: Plan,
    accel: f32,
    fresh: bool,
) -> Option<(usize, Plan)> {
    if scene.held || scene.clock <= PASS_AFTER {
        return None;
    }
    let mine = &list[own];
    let mut found = None;
    let mut last = plan;
    // Entries count − 1 down to 0 (0x42B661..0x42B7BE: the counter starts
    // at the entry count and the entry at count − 1, stepping −0x30 until
    // the counter reaches 0, so index 0, the farthest car ahead, is looked
    // at too; the own entry fails `g > 0`). One shared out-plan written per
    // eligible candidate (0x42B75D–0x42B78A), the id of the last success
    // kept, the loop not stopped (0x42B79E–0x42B7AD): `last` / `found`.
    for k in pass_candidates(list.len()) {
        let e = &list[k];
        let g = gap(list, own, e) - e.along - mine.along;
        if !(g > 0.0 && e.car.racer && (reachable(&scene.me, accel, e, g) || (g < 10.0 && e.closing < 1.0))) {
            continue;
        }
        last = plan;
        if pass_plan(route, scene, list, own, e, &mut last, fresh) {
            found = Some(e.car.id);
        }
    }
    found.map(|id| (id, last))
}

/// 0x414130: a follower to block and the block's priority.
pub(super) fn find_block(scene: &Scene, tuning: &AiTuning, list: &[Entry], own: usize) -> Option<(usize, i32)> {
    let mine = &list[own];
    let pace = scene.me.velocity.length() / mine.at.line_speed.max(0.1);
    if pace <= BLOCK_PACE || scene.held {
        return None;
    }
    for e in list.iter().skip(own + 1) {
        if !(e.car.racer && e.car.velocity.length() >= BLOCK_SPEED) {
            continue;
        }
        // The first such follower decides.
        let (lo, hi) = e.at.room;
        let on_road = if e.at.lateral <= 0.0 { lo < e.at.lateral } else { e.at.lateral < hi };
        let mut g = gap(list, own, e);
        if tuning.block_bumper(scene.block_row) {
            g += e.along + mine.along;
        }
        if !on_road || g >= 0.0 || g <= -BLOCK_RANGE || e.closing <= -5.0 {
            return None;
        }
        let mut pick = false;
        if e.closing > 0.001 {
            let t = (g / e.closing).abs();
            pick = t < if e.car.human { 30.0 } else { 2.0 } && t < 99_999.0;
        }
        if !pick && !(e.car.human && g > -BLOCK_HUMAN) {
            return None;
        }
        return Some((e.car.id, tuning.block_priority(scene.block_row)));
    }
    None
}

/// 0x402AE0(planner, −1, 1): of the racers in contact with the car that
/// have a road entry, the one it reacts to: the human over a rival; with
/// both alike, one ahead (gap > 0) over one behind, else the lesser gap.
pub(super) fn hitter(list: &[Entry], own: usize, hits: &[usize]) -> Option<usize> {
    let mut best: Option<&Entry> = None;
    for &id in hits {
        let Some(c) = list.iter().find(|e| e.car.id == id && e.car.racer) else { continue };
        let take = match best {
            None => true,
            Some(b) if c.car.human && !b.car.human => true,
            Some(b) if c.car.human != b.car.human => false,
            Some(b) => {
                let (gb, gc) = (gap(list, own, b), gap(list, own, c));
                (gb < 0.0 && gc > 0.0) || gc < gb
            }
        };
        if take {
            best = Some(c);
        }
    }
    best.map(|e| e.car.id)
}

impl Actions {
    /// 0x428FD0 (the avoid plan, ECX the lane record): true with the kept
    /// plan clear (0x428DF0, 3 s) or the record's stamp (+0x0C) under 0.5 s
    /// old. Else for the rates 0.05, 0.10, 0.15 (0x7E8E64 + 0.05 up to
    /// 0.151), offsets stepping out from the kept one by max(room / 10, 1) m
    /// (the room at the car's point, lane 0: FuturePoint +0x18 … +0x2C), the
    /// first side then the other while inside the room, the first clear one
    /// taken (0x40FBE0) and true. The stamp is set only for a plan of rate
    /// 999 (|rate − 999| ≤ 0.001, 0x42925D / 0x42932F; the stepped plans'
    /// stamp is 0), so the 0.5 s hold is the fallback's. Failing all: with
    /// no object from the first test, or that object a racer (0x60BEB0),
    /// false and the plan kept; else the plans beside the object (its offset
    /// ∓ (both +0x08 + 5 m), widened to take the kept offset on its side),
    /// costs (1, 0) when the object lies below the kept offset (the plus side
    /// cheaper), else (0, 1); with a contact record (0x592C70) the two
    /// distances from the object when they differ by more than 2 m; with
    /// `slow` (+0x42C < 25 mph) a side whose object edge (its offset ∓ both
    /// +0x08) leaves the room at any walk point from the car's (entry +0x28)
    /// to the object's costs 99999 (0x429610..); the cheaper finite side
    /// (the plus side on a tie) is taken at rate 999 (0x7E8E68) with the
    /// stamp now (0x40FBE0) and its path (0x401D40); none finite leaves the
    /// plan; false. **Inferred**: the contact kind (0x592C70; the clone
    /// takes any car contact).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn avoid_plan(
        &mut self,
        route: &RacerRoute,
        scene: &Scene,
        list: &[Entry],
        own: usize,
        minus_first: bool,
        slow: bool,
        fresh: bool,
    ) -> bool {
        let me = &scene.me;
        let kept = self.plan;
        let first_hit_path = trial(route, scene, kept, fresh);
        let first_hit =
            if first_hit_path.is_empty() { None } else { sweep(&first_hit_path, list, own, me, CLEAR_LOOK) };
        if (!first_hit_path.is_empty() && first_hit.is_none())
            || self.stamp.is_some_and(|t| scene.clock - t < AVOID_HOLD)
        {
            return true;
        }
        let mine = &list[own];
        let (lo, hi) = mine.at.room;
        let step = ((hi - lo) * 0.1).max(1.0);
        let mut rate = PASS_RATE;
        while rate <= AVOID_RATE_TOP {
            let (mut minus_in, mut plus_in) = (true, true);
            let mut k = step;
            while minus_in || plus_in {
                let (minus, plus) = (kept.offset - k, kept.offset + k);
                minus_in = lo < minus;
                plus_in = plus < hi;
                let order = if minus_first {
                    [(minus, minus_in), (plus, plus_in)]
                } else {
                    [(plus, plus_in), (minus, minus_in)]
                };
                for (offset, inside) in order {
                    if !inside {
                        continue;
                    }
                    let plan = Plan { lane: kept.lane, offset, rate };
                    let p = trial(route, scene, plan, fresh);
                    if clear(&p, list, own, me, CLEAR_LOOK) {
                        self.plan = plan;
                        self.stamp = None;
                        return true;
                    }
                }
                k += step;
            }
            rate += AVOID_RATE_STEP;
        }
        let Some((id, _)) = first_hit else { return false };
        let Some(e) = list.iter().find(|e| e.car.id == id) else { return false };
        if e.car.racer {
            return false;
        }
        let w = e.across + mine.across;
        let need = w + AVOID_SIDE;
        let obj = e.at.lateral;
        let (mut lo2, mut hi2) = (obj - need, obj + need);
        if kept.offset <= obj {
            lo2 = lo2.min(kept.offset);
        } else {
            hi2 = hi2.max(kept.offset);
        }
        let (mut cost_minus, mut cost_plus) = if obj < kept.offset { (1.0, 0.0) } else { (0.0, 1.0) };
        if scene.touching {
            let (a, b) = ((lo2 - obj).abs(), (hi2 - obj).abs());
            if (a - b).abs() > 2.0 {
                cost_minus = a;
                cost_plus = b;
            }
        }
        if slow {
            let (edge_lo, edge_hi) = (obj - w, obj + w);
            for (rlo, rhi) in route.rooms_between(scene.routes, mine.at.s, e.at.s) {
                if edge_lo.max(rlo).min(rhi) != edge_lo {
                    cost_minus = AVOID_NO_ROOM;
                }
                if edge_hi.max(rlo).min(rhi) != edge_hi {
                    cost_plus = AVOID_NO_ROOM;
                }
            }
        }
        let (minus_ok, plus_ok) =
            ((cost_minus - AVOID_NO_ROOM).abs() > 0.001, (cost_plus - AVOID_NO_ROOM).abs() > 0.001);
        let offset = match (minus_ok, plus_ok) {
            (true, true) => {
                if cost_minus < cost_plus {
                    lo2
                } else {
                    hi2
                }
            }
            (true, false) => lo2,
            (false, true) => hi2,
            (false, false) => return false,
        };
        self.plan = Plan { lane: kept.lane, offset, rate: AVOID_FALLBACK_RATE };
        self.stamp = Some(scene.clock);
        false
    }
}
