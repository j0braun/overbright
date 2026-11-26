use super::*;

impl Drivetrain {
    /// 0x594720 (**verified**): the three shift offsets (+0x78 up, +0x7C
    /// down, +0x80 2nd gear) set to 0, then each Rand_Float(hi − lo) + lo
    /// on the world generator (0x5EA510 on [0x890080] +0xF8) in that
    /// order, from the ranges +0xCC/+0xD0, +0xD4/+0xD8, +0xDC/+0xE0. The
    /// draws are skipped (the offsets left 0) when the replay manager's
    /// +0xA0 is set and its +0x41 is not 0 (0x594731..0x594743; what those
    /// fields are is **open**: the clone draws in a replay's frames as in
    /// the live ones, which keeps a replay the race it recorded). Where no
    /// world generator is lent (a lone free-roam car, the dyno, tests) the
    /// clone draws from the drivetrain's own sequence.
    pub fn reroll(&mut self, h: &Handling) {
        for (j, [lo, hi]) in self.jitter.iter_mut().zip(h.shift_ranges) {
            *j = match super::world_unit(hi - lo) {
                Some(u) => u + lo,
                None => {
                    self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    lo + (self.seed >> 8) as f32 / 16_777_216.0 * (hi - lo)
                }
            };
        }
    }

    /// The random shift offsets (+0x78 up, +0x7C down, +0x80 2nd gear).
    pub fn offsets(&self) -> [f32; 3] {
        self.jitter
    }

    /// 0x594680: the up-shift rpm for a gear, capped at the redline. 2nd
    /// gear (index 3) has its own offset; `settled` uses the fixed −100.
    pub fn up_shift_rpm(&self, h: &Handling, gear: usize, settled: bool) -> f32 {
        let offset = if settled {
            -100.0
        } else if gear == 3 {
            self.jitter[2]
        } else {
            self.jitter[0]
        };
        // A drag race lets the engine run to its maximum (0x594680).
        let cap = if h.race.drag() { h.engine.max_rpm } else { h.engine.redline_rpm };
        (offset + self.up_rpm[gear]).min(cap)
    }

    /// Carry a per-wheel quantity (FL, FR, RL, RR) to the crank: each
    /// driven axle's mean × its final drive, halved when both axles drive,
    /// × the gear ratio. Wheel speeds give the input shaft speed (0x594600),
    /// wheel accelerations its acceleration (0x594850).
    pub fn to_crank(&self, g: &Gearbox, wheels: [f32; 4]) -> f32 {
        let mut x = 0.0;
        if g.rear_split >= 0.001 {
            x = (wheels[2] + wheels[3]) * 0.5 * g.final_drive_rear;
        }
        if g.rear_split <= 0.999 {
            x += (wheels[0] + wheels[1]) * 0.5 * g.final_drive_front;
            if g.rear_split >= 0.001 {
                x *= 0.5;
            }
        }
        x * g.ratios[self.gear]
    }

    /// Crank-to-wheel ratio of each axle in the current gear (front, rear).
    pub fn axle_ratios(&self, g: &Gearbox) -> [f32; 2] {
        [g.ratios[self.gear] * g.final_drive_front, g.ratios[self.gear] * g.final_drive_rear]
    }

    /// Send a crank torque to the wheels (0x594A40). The crank feels it back
    /// as reaction, scaled by the wheels on the ground / 4 (at least 1/4).
    /// Returns each wheel's torque given its differential share (`shares`,
    /// FL FR RL RR, each axle's pair summing to 1). Nothing in neutral.
    pub fn distribute(&mut self, g: &Gearbox, torque: f32, wheels_down: usize, shares: [f32; 4]) -> [f32; 4] {
        if self.gear == NEUTRAL {
            return [0.0; 4];
        }
        self.sent += torque;
        self.reaction -= (wheels_down as f32 * 0.25).max(0.25) * torque;
        let t = torque * g.ratios[self.gear];
        let front = (1.0 - self.rear_share) * g.final_drive_front * t;
        let rear = g.final_drive_rear * self.rear_share * t;
        [front * shares[0], front * shares[1], rear * shares[2], rear * shares[3]]
    }

    /// A gear request (the tail of 0x5AA970): a new target other than the
    /// current gear; going up from gear 1 or higher in a drag race grades
    /// the shift (0x5A1710 → 0x594F30) at the engine's rpm.
    /// A manual shift, the human input's handlers for Transmission Manual
    /// (input mode +0x2C0 = 1; 0x407FC0 up, 0x4080B0 down, **verified**):
    /// they step the gear request +0x1E4 (here the target gear, which is
    /// the request the gearbox was last given) and 0x5AA970 takes it.
    /// Up: not in reverse (request 0), at most the top gear (the gear
    /// count + 1). Down: only from first gear (2) or higher, so first goes
    /// to neutral and neutral stays (**observed** under Wine in free roam:
    /// 1st → N → N → 1st, never reverse).
    pub fn shift(&mut self, h: &Handling, up: bool) {
        let request = self.target_gear;
        let top = h.gearbox.ratios.len() - 1;
        let gear = if up {
            if request == 0 || request >= top {
                return;
            }
            request + 1
        } else {
            if request < NEUTRAL + 1 {
                return;
            }
            request - 1
        };
        self.request(h, gear);
    }

    pub fn request(&mut self, h: &Handling, gear: usize) {
        if gear == self.gear || gear == self.target_gear {
            return;
        }
        self.target_gear = gear;
        if gear > self.gear && self.gear >= 1 && h.race.drag() {
            self.shift_bonus = self.shift_quality(h, self.choice_rpm(), self.gear);
        }
    }

    /// The up-shift rpm of a gear (drivetrain +0x84 + 4 · gear, read by
    /// 0x5947C0), 0 past the table.
    pub fn shift_point_rpm(&self, gear: usize) -> f32 {
        self.up_rpm.get(gear).copied().unwrap_or(0.0)
    }

    /// A gear asked for outright (0x5AA970 with a code ≥ 0: 0 reverse,
    /// 1 neutral, n a forward gear), as the AI driver's gear request does.
    pub fn request_gear(&mut self, h: &Handling, gear: usize) {
        self.request(h, gear.min(h.gearbox.ratios.len() - 1));
    }

    /// A gear change is under way (+0x48; 0x5ECD80).
    pub fn shifting(&self) -> bool {
        self.pending
    }

    /// After each integration: the engine's tidy-up (0x5AA770: idle..max,
    /// fuel cut within 25 rpm of max until 1.25% under it, reaction
    /// cleared), then the clutch state machine (0x5A1410) on the input
    /// shaft speed from the wheels.
    pub fn post(&mut self, h: &Handling, input_w: f32, dt: f32) {
        let e = &h.engine;
        let (idle_w, max_w) = (e.idle_rpm * 0.10471967, e.max_rpm * 0.10471967);
        self.engine_w = self.engine_w.max(idle_w).min(max_w);
        let rpm = self.rpm();
        self.reaction = 0.0;
        // Engine heat (0x5A11F0, from the tidy-up): over the redline +0.45/s,
        // else −0.1/s; at 1.2 the engine blows (2) and stays so. Held while
        // a shift bonus runs in 1st.
        if (0.0..2.0).contains(&self.heat) && (self.gear != 2 || self.shift_bonus <= 0.0) {
            let rate = if self.rpm() <= h.engine.redline_rpm { -0.1 } else { 0.45 };
            self.heat = (self.heat + rate * dt).max(0.0);
            if self.heat >= 1.2 {
                self.heat = 2.0;
            }
        }

        if self.limiter {
            if rpm < e.max_rpm - e.max_rpm * 0.0125 {
                self.limiter = false;
            }
        } else if rpm > e.max_rpm - 25.0 {
            self.limiter = true;
        }
        let throttle = if self.limiter { 0.0 } else { self.power * self.throttle };
        self.turbo.update(rpm, e.idle_rpm, e.max_rpm, throttle, self.limiter, dt);

        self.input_w = input_w;
        // While the clutch is held (+0x74) the power is 1 in every state
        // and each state ends in Changing (0x5A15B3.., 0x5A1544..).
        let held = self.clutch_held;
        match self.shift {
            Shift::Disengaging => {
                self.power = if held { 1.0 } else { 0.0 };
                if self.clutch <= 0.0 {
                    self.clutch = 0.0;
                    self.shift = Shift::Changing;
                }
            }
            Shift::Changing => {
                self.power = if held { 1.0 } else { 0.0 };
                self.gear = self.target_gear;
                if self.pending || !self.stalling(e) {
                    if !held {
                        self.shift = Shift::Engaging;
                    }
                    self.reroll(h);
                }
            }
            Shift::Engaging => {
                self.power = if held { 1.0 } else { 0.7 };
                if self.clutch >= 1.0 {
                    self.clutch = 1.0;
                    // Locks once engine and gearbox agree within 50 rad/s
                    // (0x594EE0), or in neutral.
                    if (self.engine_w - self.input_w).abs() < 50.0 || self.gear == NEUTRAL {
                        self.shift = Shift::Locked;
                    }
                }
                if self.stalling(e) && self.gear != NEUTRAL {
                    self.shift = Shift::Disengaging;
                }
            }
            Shift::Locked => {
                if self.shift_bonus > 0.0 && self.bonus_since.is_none() {
                    self.bonus_since = Some(self.clock);
                }
                self.power = 1.0;
                if self.gear != NEUTRAL && self.stalling(e) {
                    self.shift = Shift::Disengaging;
                }
                if self.gear == self.target_gear {
                    self.pending = false;
                } else {
                    self.pending = true;
                    self.shift = Shift::Disengaging;
                }
            }
        }
        if held {
            self.shift = Shift::Changing;
        }
        // A good drag shift boosts the engine for half a second after the
        // clutch locks: power 1 + 2·bonus, fading out; then the bonus ends.
        if let Some(since) = self.bonus_since {
            let elapsed = self.clock - since;
            if self.shift == Shift::Locked {
                let boost = (0.5 - elapsed) / 0.5 * self.shift_bonus * 2.0;
                if boost > 0.0 {
                    self.power = boost + 1.0;
                }
                if elapsed >= 0.5 {
                    self.bonus_since = None;
                    self.shift_bonus = 0.0;
                }
            }
        }
    }

    /// Automatic gear choice (0x5AA970 with code −1, called each step by
    /// 0x5ABC00). Out of neutral it asks for 1st. With the clutch locked it
    /// shifts up at the gear's up-shift rpm if every wheel is on the ground
    /// and not spinning, and down once the engine falls below both the
    /// gear's down-shift rpm and the previous gear's shift point carried
    /// through the ratio change, less 200 rpm. Reverse is the player's
    /// choice (brake at a standstill), not part of this function.
    pub fn choose_gear(&mut self, h: &Handling, reverse: bool, wheels_gripping: bool) {
        if reverse {
            self.request(h, 0);
            return;
        }
        if self.gear <= NEUTRAL {
            self.request(h, 2);
            return;
        }
        if self.shift != Shift::Locked {
            return;
        }
        let g = &h.gearbox;
        let gear = self.gear;
        let rpm = self.choice_rpm();
        let up = self.up_shift_rpm(h, gear, false);
        if rpm >= up && gear < g.ratios.len() - 1 {
            if wheels_gripping {
                self.request(h, gear + 1);
            }
            return;
        }
        let (down_a, down_b) = if gear == 2 {
            // 1st: never below; the first threshold only matters off throttle.
            (0.0, 0.0)
        } else {
            let carried = g.ratios[gear] / g.ratios[gear - 1] * self.up_shift_rpm(h, gear - 1, true) - 200.0;
            (self.jitter[1] + self.down_rpm[gear], carried)
        };
        if rpm <= down_a && rpm <= down_b {
            self.request(h, gear - 1);
        }
    }
}

/// Per-gear shift points (0x5AC7E0), indexed like the ratios. Neutral's
/// up-shift sits `neutral_shift` of the way from idle to redline; every
/// forward gear shifts up 100 rpm under the redline (the torque-matching
/// search behind the flag 0x86B33C is off: nothing sets it), and the top
/// gear at max rpm + 500. A gear's down-shift rpm is the previous gear's
/// up-shift rpm carried through the ratio change.
pub fn shift_points(h: &Handling) -> (Vec<f32>, Vec<f32>) {
    let g = &h.gearbox;
    let e = &h.engine;
    let n = g.ratios.len();
    let mut up = vec![0.0; n];
    let mut down = vec![0.0; n];
    up[NEUTRAL] = (e.redline_rpm - e.idle_rpm) * g.neutral_shift + e.idle_rpm;
    for gear in 2..n - 1 {
        up[gear] = e.redline_rpm - 100.0;
        down[gear + 1] =
            if g.ratios[gear].abs() <= 1e-5 { 0.0 } else { g.ratios[gear + 1] / g.ratios[gear] * up[gear] };
    }
    up[n - 1] = e.max_rpm + 500.0;
    (up, down)
}
