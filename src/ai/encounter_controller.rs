//! Persistent controller scheduling for encounter racers (42ABD0).
//! World inputs refer to simulation updates, never rendered-frame ordinals.

#[derive(Clone, Copy, Debug)]
pub struct ThinkInput {
    pub dt: f32,
    pub tick: u32,
    pub world_update: i32,
    pub owner_class: u32,
    pub owner_kind: u32,
    pub owner_id: i16,
    pub racer_slot: i32,
    pub racer_count: i32,
    pub drag: bool,
    pub mover_four: bool,
    pub action_present: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ThinkSchedule {
    pub last_tick: u32,
    pub remaining: f32,
}

impl ThinkSchedule {
    /// 40D550 uses signed owner+08 (world list ID), not racer+0A.
    pub fn reset_placement(&mut self, world_id: i16) {
        self.remaining = (f64::from(world_id) * f64::from(0.1f32)
            * f64::from(-0.04545454680919647f32)) as f32;
    }

    /// True enters full think, including the missing-action bypass.
    pub fn full_think(&mut self, input: ThinkInput) -> bool {
        self.remaining -= input.dt;
        let delta = input.tick.wrapping_sub(self.last_tick) as i32;
        // x87 multiplies the signed integer by the f32 constant without
        // rounding the product to f32 before comparing with f32 .2.
        let elapsed = f64::from(delta) * f64::from(0.00025f32) >= f64::from(0.2f32);
        let due = match input.owner_class {
            2 => {
                let divisor = if input.drag && !input.mover_four { 4 } else { 8 };
                let selected = input.world_update % divisor;
                selected < input.racer_count && selected == input.racer_slot && elapsed
            }
            1 => input.owner_kind == 2 || (input.world_update % 8 == i32::from(input.owner_id) && elapsed),
            _ => self.remaining <= 0.0,
        };
        let full = due || !input.action_present;
        if full {
            self.last_tick = input.tick;
            self.remaining += 0.1f32;
        }
        full
    }
}

/// 576D00 reset and 57A0F0 end-of-world-update clock. This advances once
/// per simulated world update, outside the car loop and physics substeps.
#[derive(Clone, Debug)]
pub struct WorldClock {
    pub tick: u32,
    pub update: i32,
}
impl Default for WorldClock {
    fn default() -> Self { Self { tick: 4000, update: 0 } }
}
impl WorldClock {
    pub fn finish_update(&mut self, dt: f32) {
        if dt == 0.0 { return; }
        self.tick = self.tick.wrapping_add((f64::from(dt) * 4000.0 + 0.5).trunc() as i32 as u32);
        self.update = self.update.wrapping_add(1);
    }
}

/// Controller timers +7A8/+7AC/+7B0. 416F70 updates the flat-road timer
/// only when the strategy callback reaches the route checks.
#[derive(Clone, Debug, Default)]
pub struct Nitrous {
    pub started: u32,
    pub ended: u32,
    pub flat_since: u32,
}
impl Nitrous {
    pub fn elapsed(now: u32, then: u32) -> f64 {
        f64::from(now.wrapping_sub(then) as i32) * f64::from(0.00025f32)
    }
    /// Race7, lane0, non-last-lap branch established by the context capture.
    pub fn wanted(&mut self, tick: u32, running: bool, cooldown: f32,
        speed: f32, road: (Option<f32>, bool), engine_ok: bool) -> bool {
        if running && Self::elapsed(tick, self.started) > 2.0 { return false; }
        if !running && Self::elapsed(tick, self.ended) <= f64::from(cooldown) { return false; }
        if !road.1 { self.flat_since = 0; }
        else if self.flat_since == 0 { self.flat_since = tick; }
        let flat = self.flat_since != 0 && Self::elapsed(tick, self.flat_since) > 1.0;
        (road.0.is_some_and(|limit| f64::from(speed) / f64::from(limit) < 1.0) || flat) && engine_ok
    }
    pub fn may_stop(&self, tick: u32, minimum: f32) -> bool {
        Self::elapsed(tick, self.started) > f64::from(minimum)
    }
}

/// Owns the action and its retained driver output across full/fallback calls.
#[derive(Clone, Debug, Default)]
pub struct Controller {
    pub schedule: ThinkSchedule,
    pub goal: Option<bool>, // Some(true): chase; Some(false): lead.
    pub actions: Option<crate::ai::actions::Actions>,
    pub output: Option<crate::ai::actions::Steer>,
    pub target_speed: f32,
    pub suspended: bool,
    pub nitrous: Nitrous,
    /// Callback dispatch counts for the most recent controller call.
    pub speed_callbacks: u8,
    pub search_callbacks: u8,
}
impl Controller {
    /// 40D550 resets the placement timer and suspension, but does not
    /// change the last full-think tick or destroy the current action.
    pub fn reset_placement(&mut self, world_id: i16) {
        self.schedule.reset_placement(world_id);
        self.suspended = false;
    }

    /// 402070: changed goal deletes its action; same-goal retarget keeps it.
    pub fn assign_goal(&mut self, chase: bool) -> bool {
        if self.goal == Some(chase) { return false; }
        self.goal = Some(chase);
        self.actions = None;
        true
    }
}

/// Finite-input 402180 selector. World-load and full-think dispatch are callers'
/// responsibilities. Kept explicit so native records exercise production gates.
#[derive(Clone, Copy, Debug)]
pub struct BeltGate {
    pub special_track: bool,
    pub drag: bool,
    pub drift: bool,
    pub path_points: i32,
    pub timer730: u32,
    pub timer734: u32,
    pub forward_speed: f32,
    pub wheels_down: i32,
    pub action: Option<u8>,
    pub skill: i32,
    pub human_distance: f32,
    pub race_clock: f32,
    pub finished: bool,
}
impl BeltGate {
    pub fn wanted(self) -> bool {
        let running = |t| t != 0 && t != 0x7fff_ffff;
        !self.special_track && !self.drag && !self.drift && self.path_points >= 3
            && !running(self.timer730) && !running(self.timer734)
            && self.forward_speed >= 5.0 && self.wheels_down >= 1
            && self.action.is_some_and(|a| !matches!(a, 8 | 9 | 10 | 11 | 13))
            && self.skill != -2 && self.human_distance >= 120.0
            && self.race_clock >= 30.0 && !self.finished
    }
}
/// 43CE10 / 5F1170 finite PC24 distance order.
pub fn belt_human_distance(a: glam::Vec3, b: glam::Vec3) -> f32 {
    let d = a - b;
    ((d.z*d.z + d.y*d.y) + d.x*d.x).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn belt_gate_matches_203_native_race7_decisions() {
        let csv = &crate::testkit::private_str("docs/evidence/outrun/belt-race7-gates.csv");
        let mut count = 0;
        for line in csv.lines().filter(|l| !l.starts_with('#')) {
            let v: Vec<i64> = line.split(',').map(|v| v.parse().unwrap()).collect();
            let gate = BeltGate { special_track:false, drag:false, drift:false,
                path_points:v[1] as i32, timer730:v[2] as u32, timer734:v[3] as u32,
                forward_speed:f32::from_bits(v[4] as u32), wheels_down:v[5] as i32,
                action:Some(v[6] as u8), skill:v[7] as i32,
                human_distance:f32::from_bits(v[8] as u32), race_clock:f32::from_bits(v[9] as u32), finished:false };
            let point = |offset| glam::Vec3::new(f32::from_bits(v[offset] as u32),
                f32::from_bits(v[offset+1] as u32), f32::from_bits(v[offset+2] as u32));
            assert_eq!(belt_human_distance(point(11),point(14)).to_bits(),gate.human_distance.to_bits(), "native distance record {}",v[0]);
            assert_eq!(gate.wanted(), v[10]!=0, "native record {}",v[0]);
            count+=1;
        }
        assert_eq!(count,203);
    }


    #[test]
    fn nitrous_uses_signed_tick_thresholds_and_callback_sampled_flatness() {
        let mut nos = Nitrous::default();
        assert!(!nos.wanted(19999, false, 5.0, 10.0, (Some(20.0), true), true));
        assert!(nos.wanted(20000, false, 5.0, 10.0, (Some(20.0), true), true));
        assert_eq!(nos.flat_since, 20000);
        assert!(!nos.wanted(23999, false, 5.0, 20.0, (Some(20.0), true), true));
        assert!(nos.wanted(24000, false, 5.0, 20.0, (Some(20.0), true), true));
        assert!(!nos.wanted(24001, false, 5.0, 20.0, (Some(20.0), false), true));
        assert_eq!(nos.flat_since, 0);
        nos.started = 40000;
        assert!(nos.wanted(47999, true, 5.0, 10.0, (Some(20.0), true), true));
        assert!(!nos.wanted(48000, true, 5.0, 10.0, (Some(20.0), true), true));
        assert!(!nos.may_stop(41999, 0.5));
        assert!(nos.may_stop(42000, 0.5));
        assert!(Nitrous::elapsed(100, u32::MAX - 99) > 0.049);
        assert!(Nitrous::elapsed(0, 1) < 0.0);
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn world_clock_matches_all_509_live_update_boundaries() {
        let raw = &crate::testkit::private("docs/evidence/outrun/think-followup/think.bin");
        let word = |row: &[u8], index: usize| u32::from_le_bytes(row[index*4..index*4+4].try_into().unwrap());
        let rows: Vec<_> = raw.chunks_exact(256).filter(|r| word(r,0)==1).collect();
        for pair in rows.windows(2) {
            let mut clock = WorldClock { tick: word(pair[0],2), update: word(pair[0],1) as i32 };
            clock.finish_update(f32::from_bits(word(pair[0],41)));
            assert_eq!(clock.tick, word(pair[1],2));
            assert_eq!(clock.update as u32, word(pair[1],1));
        }
        assert_eq!(rows.len(), 510);
    }

    #[test]
    fn retarget_retains_action_but_goal_change_destroys_it() {
        let mut controller = Controller::default();
        controller.assign_goal(false);
        controller.actions = Some(crate::ai::actions::Actions::default());
        controller.schedule.last_tick = 1234;
        assert!(!controller.assign_goal(false));
        assert!(controller.actions.is_some());
        assert!(controller.assign_goal(true));
        assert!(controller.actions.is_none());
        assert_eq!(controller.schedule.last_tick, 1234);
    }

    #[test]
    fn placement_reset_recovers_suspension_without_fabricating_a_goal_reset() {
        let mut controller = Controller::default();
        controller.assign_goal(false);
        controller.actions = Some(crate::ai::actions::Actions::default());
        controller.schedule.last_tick = 12345;
        controller.suspended = true;
        controller.assign_goal(true);
        assert!(controller.suspended, "402070 does not clear +710");
        controller.actions = Some(crate::ai::actions::Actions::default());
        controller.reset_placement(2);
        assert!(!controller.suspended, "40D590 clears +710");
        assert!(controller.actions.is_some());
        assert_eq!(controller.schedule.last_tick, 12345);
        assert_eq!(controller.schedule.remaining.to_bits(), (-0.00909091f32).to_bits());
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn scheduling_matches_all_original_gate_cases() {
        let inputs = &crate::testkit::private("docs/evidence/outrun/think-followup/gate-inputs.bin");
        let outputs = &crate::testkit::private("docs/evidence/outrun/think-followup/gate.bin");
        assert_eq!(inputs.len()/68, outputs.len()/28);
        for (index, (input, output)) in inputs.chunks_exact(68).zip(outputs.chunks_exact(28)).enumerate() {
            let word = |bytes: &[u8], index: usize| u32::from_le_bytes(bytes[index*4..index*4+4].try_into().unwrap());
            let u = |index| word(input, index);
            let mut schedule = ThinkSchedule { last_tick: u(10), remaining: f32::from_bits(u(11)) };
            let full = schedule.full_think(ThinkInput {
                dt: f32::from_bits(u(12)), tick: u(9), world_update: u(1) as i32,
                owner_class: u(0), owner_kind: u(6), owner_id: u(5) as i16,
                racer_slot: (u(4) as i32).wrapping_sub(u(2) as i32), racer_count: (u(3) as i32).wrapping_sub(u(2) as i32),
                drag: u(8) != 0, mover_four: u(7) == 4, action_present: u(13) != 0,
            });
            assert_eq!(full, word(output, 0) == 1, "case {index}");
            assert_eq!(schedule.last_tick, word(output, 1), "case {index}");
            assert_eq!(schedule.remaining.to_bits(), word(output, 2), "case {index}");
        }
    }
}
