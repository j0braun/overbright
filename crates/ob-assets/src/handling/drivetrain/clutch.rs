use super::*;

/// Clutch speed factor by shift state (0x7FC298): opening, open, closing,
/// locked.
pub const CLUTCH_DIRECTION: [f32; 4] = [-3.0, 0.0, 1.5, 0.0];
/// Engine-braking relief in 1st gear over the net crank torque (0x7FC8C0).
pub const FIRST_GEAR_RELIEF: [(f32, f32); 5] =
    [(-10.0, 1.0), (-7.5, 0.96), (-3.5, 0.925), (-0.3, 0.875), (-0.05, 0.0)];
/// Its scale by the car's performance class (table 0x7A18B4, indexed by
/// the drivetrain's +0xF0, which the car's set-up 0x5B6130 fills with the
/// class).
pub const FIRST_GEAR_RELIEF_SCALE: [f32; 4] = [0.0, -0.2, -0.4, -0.7];

/// Piecewise-linear through `points`, clamped at both ends (0x40C840).
pub fn graph(points: &[(f32, f32)], x: f32) -> f32 {
    if x <= points[0].0 {
        return points[0].1;
    }
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    points[points.len() - 1].1
}

impl Drivetrain {
    /// The crank torque sent through the clutch at the start of a sub-step
    /// (0x594CC0). Locked: the engine's torque. Slipping: the clutch
    /// stiffness × engagement × the speed difference held to ±300 rad/s; the
    /// stiffness halves when that difference swings sign beyond 2000 rpm
    /// both ways. Open: none.
    pub fn clutch_torque(&mut self, h: &Handling) -> Option<f32> {
        self.sent = 0.0;
        match self.shift {
            Shift::Disengaging | Shift::Engaging => {
                let g = &h.gearbox;
                let slip = (self.engine_w - self.input_w).clamp(-300.0, 300.0);
                let mut k = g.clutch_stiffness;
                let rpm = (self.input_w - self.engine_w) * RPM_PER_RAD_S;
                let last = self.last_slip_rpm;
                if last != 0.0 && rpm * last < 0.0 && rpm.abs() > 2000.0 && last.abs() > 2000.0 {
                    k *= 0.5;
                }
                self.last_slip_rpm = rpm;
                Some(k * self.clutch * slip)
            }
            Shift::Locked => Some(self.engine_torque(&h.engine, h.nitrous_power)),
            Shift::Changing => None,
        }
    }

    /// With the clutch locked, the crank torque that makes the engine and
    /// the gearbox input accelerate alike (0x594DB0), given the input
    /// shaft's acceleration from the wheels (0x594850) and the driven
    /// wheels' inertia.
    pub fn coupling_torque(&self, h: &Handling, input_accel: f32, wheel_inertia: f32) -> Option<f32> {
        if self.shift != Shift::Locked {
            return None;
        }
        let g = &h.gearbox;
        let inertia = h.engine.inertia;
        let [front, rear] = self.axle_ratios(g);
        let mut compliance = 1.0 / inertia;
        if g.rear_split <= 0.999 {
            compliance += (1.0 - self.rear_share) / wheel_inertia * front * front * 0.5;
        }
        if g.rear_split >= 0.001 {
            compliance += self.rear_share / wheel_inertia * rear * rear * 0.5;
        }
        let crank_accel = (self.engine_torque(&h.engine, h.nitrous_power) + self.reaction) / inertia;
        Some((crank_accel - input_accel) / compliance)
    }

    /// After the friction circles (0x5AA840). In gear with the clutch
    /// locked and a wheel down: under power with the driven wheels
    /// spinning (throttle over 0.2, mean slip over 0.1 m/s) the wheels are
    /// set to the engine — the returned crank speed, for 0x5A12E0 —;
    /// otherwise the engine is pulled toward the input shaft, by no more
    /// than its own torque. In 1st, engine braking is partly relieved.
    pub fn settle(&mut self, h: &Handling, input_w: f32, driven_slip: f32, wheels_down: usize) -> Option<f32> {
        let mut matched = None;
        if self.gear != NEUTRAL && self.shift == Shift::Locked && wheels_down > 0 {
            if self.throttle > 0.2 && driven_slip > 0.1 {
                self.input_w = self.engine_w;
                matched = Some(self.engine_w);
            } else {
                let limit = self.engine_torque(&h.engine, h.nitrous_power).abs();
                self.reaction += ((input_w - self.engine_w) * h.gearbox.clutch_stiffness).clamp(-limit, limit);
            }
        }
        if self.gear == 2 {
            let torque = self.engine_torque(&h.engine, h.nitrous_power);
            let net = self.reaction + torque;
            if self.reaction < torque && net < 0.0 {
                self.reaction +=
                    graph(&FIRST_GEAR_RELIEF, net) * FIRST_GEAR_RELIEF_SCALE[h.performance_class.min(3)] * net;
            }
        }
        matched
    }

    /// Clutch speed, engagement per second (0x5949C0): the gearbox rate
    /// (data +0x1C) × the drivetrain's factor +0xE8 ([`Handling::clutch_scale`])
    /// × the drag grade's factor × the direction for the shift state.
    pub fn clutch_rate(&self, h: &Handling) -> f32 {
        let g = &h.gearbox;
        let mut rate = g.clutch_rate * h.clutch_scale;
        // A good drag shift closes the clutch faster, a bad one slower from
        // 2nd up (0x5949C0).
        if self.shift_bonus > 0.0 {
            rate *= self.shift_bonus + 1.0;
        } else if self.shift_bonus < 0.0 && self.target_gear >= 3 {
            rate *= 0.35;
        }
        rate * CLUTCH_DIRECTION[self.shift as usize]
    }
}
