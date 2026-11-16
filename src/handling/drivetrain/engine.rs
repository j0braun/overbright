use super::*;

impl Drivetrain {
    /// The rpm the gear choice and the shift grading read: the car's
    /// shown rpm (+0x440) when known, else the engine's.
    pub(super) fn choice_rpm(&self) -> f32 {
        self.shown_rpm.unwrap_or_else(|| self.rpm())
    }

    pub fn rpm(&self) -> f32 {
        self.engine_w * RPM_PER_RAD_S
    }

    /// The AI's nitrous band (0x402590, drivetrain kind 1; an AI shot may
    /// run only inside it, see `CarState::ai_nitrous_ok`): the current
    /// gear's down-shift rpm A (0x5946F0(gear, 0): +0x7C offset + per-gear
    /// +0xA8) up to A + 0.95 · (B − A), B its up-shift rpm (0x594680(gear,
    /// 0)); whether `rpm` (the car's shown rpm, car +0x440, 0x402616)
    /// equals its clamp into it (max with A, then min with the top).
    pub fn in_nitrous_band(&self, h: &Handling, rpm: f32) -> bool {
        let a = self.jitter[1] + self.down_rpm.get(self.gear).copied().unwrap_or(0.0);
        let b = self.up_shift_rpm(h, self.gear, false);
        rpm == rpm.max(a).min(a + (b - a) * 0.95)
    }

    /// Engine torque at the crank (0x594310): the pedal (engine +0x78) ×
    /// the shift-state power factor (+0x80), plus a running nitrous shot
    /// (stage +0x84 > 0: the nitrous curve × the same opening × the car's
    /// nitrous power × 0x7A17A0[stage]), less engine braking.
    pub fn engine_torque(&self, e: &Engine, nitrous_power: f32) -> f32 {
        let rpm = self.rpm();
        let open = if self.limiter { 0.0 } else { self.throttle * self.power };
        let mut drive = e.torque.at(rpm) * self.catch_up * open;
        if self.nitrous > 0 {
            drive += open * e.nitrous.at(rpm) * nitrous_power * crate::nitrous::TORQUE[self.nitrous.min(3)];
        }
        let mut drag = -(1.0 - open) * e.braking.at(rpm);
        if self.limiter {
            drag *= 2.0;
        }
        // A hot engine loses up to three quarters of its power past 0.6
        // heat (0x7FBFF0).
        let drive = if self.heat > 0.6 { drive * (1.0 - (self.heat.min(1.0) - 0.6) / 0.4 * 0.75) } else { drive };
        let total = drive + drag;
        if total < 0.0 && !self.limiter {
            // 0.65 when the driveline is in gear 1 (neutral), else 0.85.
            return total * if self.gear == NEUTRAL { 0.65 } else { 0.85 };
        }
        total
    }

    /// The integration step for the engine and clutch (derivatives 0x5A12A0,
    /// 0x5949C0; Euler 0x5AA490).
    pub fn integrate(&mut self, h: &Handling, dt: f32) {
        let accel = (self.engine_torque(&h.engine, h.nitrous_power) + self.reaction) / h.engine.inertia;
        let rate = self.clutch_rate(h);
        self.engine_w += accel * dt;
        self.clutch += rate * dt;
        self.clock += dt;
    }

    /// The engine is blown (0x5942D0: heat +0x8C exactly 2; 0x5ECE10 asks
    /// it of a car whose drivetrain kind +0x28 is 1).
    pub fn blown(&self) -> bool {
        self.heat == 2.0
    }

    /// The drag HUD's heat (0x5ECCE0): engine heat clamped to 0..1.
    pub fn heat_fraction(&self) -> f32 {
        if self.heat <= 0.0 { 0.0 } else { self.heat.min(1.0) }
    }

    /// The anti-stall test (0x594EA0): throttle under 0.3 and the input
    /// shaft at or under idle.
    pub(super) fn stalling(&self, e: &Engine) -> bool {
        self.throttle < 0.3 && self.input_w <= e.idle_rpm * 0.10471967 + 0.0001
    }
}
