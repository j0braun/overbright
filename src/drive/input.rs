use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub throttle: f32,
    pub brake: f32,
    pub handbrake: f32,
    /// -1 (left) .. 1 (right).
    pub steer: f32,
    /// The Nitrous key held (JOY_EVENT_NITRO_BOOST 0xA3 / _ALTERNATE
    /// 0xA4, Controls row 7, action 6).
    pub nitrous: bool,
    /// The hydraulics input (controls +0x1EC bounce, +0x1FC pressure;
    /// `crate::hydraulics::Controller`).
    pub hydraulics: crate::hydraulics::Input,
}

/// What an AI driver hands the car each frame (the CarDriver block the
/// mover reads in 0x5ABC00): the front-wheel angle it has worked out
/// (+0x1E0), the pedals (+0x20C gas, +0x214 brake, +0x210 handbrake) and
/// its gear request (+0x1E4).
#[derive(Clone, Copy, Debug, Default)]
pub struct AiControls {
    /// Front-wheel angle, degrees, positive = left.
    pub steer_degrees: f32,
    pub gas: f32,
    pub brake: f32,
    pub handbrake: f32,
    /// −1 the automatic gears, 0 reverse, 1 neutral, n ≥ 2 that gear.
    pub gear: i32,
    /// The driver's nitrous request (+0x1FF, set by 0x40E390 from the
    /// controller's 0x40D850): the AI update 0x420810 starts a shot
    /// (0x59FB00) whenever none runs.
    pub nitrous: bool,
    /// The update in which the race goes green: the driver's block
    /// already reads the release (brake 0, gear request −1), but the
    /// mover ran before the release was written, so this update's
    /// physics still sees the countdown's block (gas, brake 1, gear
    /// request 1 = neutral). **Observed** under Wine (see
    /// docs/tuning-physics-parity.md): at the first status-4 frame the
    /// rivals' drivetrains are still in neutral with the clutch locked
    /// (+0x44 = 3) and the shift to 1st starts one update later. The
    /// release is the hold's vt+0x30 (0x410030 writes +0x1E4 = −1).
    pub mover_held: bool,
}

/// Who drives a car's frame: the player's input or an AI driver.
#[derive(Clone, Copy, Debug)]
pub enum Input {
    Player(Controls),
    Ai(AiControls),
    /// The player in a drag race (DragDriver, 0x4220A0): the AI driver's
    /// front-wheel angle, degrees (0x421030 on the lane path), with the
    /// player's pedals (+0x67C / +0x680 / +0x684) and gearbox.
    Drag(Controls, f32),
    /// An AI driver's controls on the MeloMvr (mode 3, 0x5B6D10) instead
    /// of the physics mover: the car's [`CarState::melo`] chassis runs its
    /// frame (`crate::melo`), the collision step is the same.
    Melo(AiControls),
    /// A racer on the BeltMover (mode 4, 0x59EC90): its frame has already
    /// placed the car, so the car's own frame does nothing; it still takes
    /// part in the collision step, and a resolved contact with a car or a
    /// prop hands it to the physics mover for the rest of the frame and
    /// sets [`CarState::belt_hit`] (0x402450: +0x730 = 2 s, then 0x5F4410's
    /// mode; track contacts are left out, see `step_world`).
    Belt(AiControls),
}

/// The mover's pedal check (0x5ABC00): when the gas beats the brake (and
/// the handbrake is under the gas), a brake or handbrake under half travel
/// is let off; otherwise a gas under half travel is. URL events skip it.
pub fn pedal_overlap(gas: f32, brake: f32, handbrake: f32) -> (f32, f32, f32) {
    let (mut gas, mut brake, mut handbrake) = (gas, brake, handbrake);
    if gas > brake {
        if handbrake < gas {
            if brake < 0.5 {
                brake = 0.0;
            }
            if handbrake < 0.5 {
                handbrake = 0.0;
            }
        }
    } else if gas < 0.5 {
        gas = 0.0;
    }
    (gas, brake, handbrake)
}

/// A 0x576AB0 running average (set up by 0x579F90 with a window and 120
/// samples a second, 0x783AD4: window × 120 slots). Each sample carries its
/// time; samples older than the window drop out, and the mean (+0xC of the
/// object) is the sum over the samples kept.
#[derive(Clone, Debug)]
pub struct RunningAverage<const N: usize> {
    values: [f32; N],
    times: [f32; N],
    count: u8,
    next: usize,
    oldest: usize,
    sum: f32,
    pub mean: f32,
    /// The time in clock ticks (0x865188, 4000 a second), whole as the
    /// game's clock is; samples are timed by ticks × 0.00025.
    ticks: f64,
    window: f32,
}

/// The player steering's running mean of its shaped input (PlayerSteering
/// +0x14, set up by the steering's constructor 0x405C60 with a 0.55 s
/// window (0x783AD8): 66 slots). The input update feeds it once a frame
/// after the steering (0x41E03B, the clock 0x865188 × 0.00025 as the
/// time); its mean (+0x20) is what the lock gain (0x405E5F), the held lock
/// (0x406077) and the rate gain (0x4061AA) read, so a digital key's lock
/// and rate grow over the half second it is held.
pub type SteerAverage = RunningAverage<66>;

/// The player steering's running mean of how fast its input changes
/// (PlayerSteering +0x48, 0x405C60 with a 0.15 s window (0x783ADC): 18
/// slots). 0x4060E0 feeds it each frame, before the rate, with curve
/// 0x7E9F28 (1, 1.05, 1.1, 1.5, 2.2, 3.1 over 0..10 /s) of |shaped input −
/// last frame's (+0x84, stored at 0x41E042)| / dt, and its mean (+0x54)
/// multiplies the steering rate (0x406199): a key press or a reversal
/// steers up to 3.1 / (the window's samples) faster for 0.15 s.
pub type RateAverage = RunningAverage<18>;

impl Default for RunningAverage<66> {
    fn default() -> Self {
        Self::new(0.55)
    }
}

impl Default for RunningAverage<18> {
    fn default() -> Self {
        Self::new(0.15)
    }
}

impl<const N: usize> RunningAverage<N> {
    pub(super) fn new(window: f32) -> Self {
        RunningAverage {
            values: [0.0; N],
            times: [0.0; N],
            count: 0,
            next: 0,
            oldest: 0,
            sum: 0.0,
            mean: 0.0,
            ticks: 0.0,
            window,
        }
    }

    /// 0x576AB0: a new sample `dt` after the last one.
    pub fn add(&mut self, value: f32, dt: f32) {
        self.ticks += f64::from(dt) * 4000.0;
        let now = self.ticks.round() as f32 * 0.00025;
        let i = self.next;
        if self.values[i] == 0.0 && self.times[i] == 0.0 {
            self.count = self.count.saturating_add(1);
        } else {
            self.sum -= self.values[i];
        }
        self.sum += value;
        self.values[i] = value;
        self.times[i] = now;
        while self.window < now - self.times[self.oldest] {
            let o = self.oldest;
            if self.times[o] > 0.0 {
                self.sum -= self.values[o];
                self.values[o] = 0.0;
                self.times[o] = 0.0;
                self.count = self.count.saturating_sub(1);
            }
            self.oldest = (o + 1) % N;
        }
        self.next = (i + 1) % N;
        self.mean = if self.count > 0 { self.sum / f32::from(self.count) } else { 0.0 };
    }
}

/// The player steering's wheel angle as PlayerSteering stores it (+0x7C, a
/// u16 binary angle: 0x416D90…0x416DB4 write, 0x416C20…0x416C4D read).
pub(super) fn quantise_steer(degrees: f32) -> f32 {
    let word = (((degrees * 65536.0) as i32) / 360) as u16;
    let a = f32::from(word) * (360.0 / 65536.0);
    if a >= 180.0 { a - 360.0 } else { a }
}

impl CarState {
    /// Player steering (0x416BB0 with 0x405DC0 and 0x4060E0). The wheel
    /// angle moves towards `lock × ratio × response(input)`, where the lock
    /// is the speed curve, grown when off the throttle or on the brakes,
    /// held while the stick is past half, and replaced by a share of the
    /// slip angle when counter-steering a slide.
    pub(super) fn steer(&mut self, h: &Handling, controls: Controls, dt: f32) {
        let speed = self.velocity.length();
        // The game's steering is positive to the left.
        let raw = -controls.steer.clamp(-1.0, 1.0);
        // The response curve by game mode (0x416C08: 0x7E9E4C + 20 × 1, or
        // × 4 in the 110°/s modes, drift and URL).
        let input = sample(
            if h.race.drift() || h.race.url() { &*STEER_RESPONSE_DRIFT } else { &*STEER_RESPONSE },
            -1.0,
            1.0,
            raw,
        );
        // The car's yaw acceleration over the last frame (car +0x128).
        let yaw_accel =
            if self.last_steer_dt > 0.0 { (self.spin.z - self.last_yaw_rate) / self.last_steer_dt } else { 0.0 };
        self.last_yaw_rate = self.spin.z;
        self.last_steer_dt = dt;
        // In a drift race (0x416C5C: 0x89E7D9 / its online twins, not
        // drag), steering the way the yaw is speeding up raises the ratio
        // (+0x380) for both the target and the rate by 0x7E9F50 over |yaw
        // acceleration| (rad/s²): 1 at 0, 1.1, 1.2, 1.25, 1.3, 1.35 at 5.
        let ratio = if h.race.drift() && input * yaw_accel > 0.0 {
            h.steering_ratio
                * graph(&[(0.0, 1.0), (1.0, 1.1), (2.0, 1.2), (3.0, 1.25), (4.0, 1.3), (5.0, 1.35)], yaw_accel.abs())
        } else {
            h.steering_ratio
        };
        let speed_factor = sample(&STEER_SPEED, 0.0, 160.0, speed);

        // Lifting off and braking add up to the speed factor again in lock.
        let pedals = controls.throttle + 1.0 - (controls.handbrake + controls.brake) * 0.5;
        let pedal_bonus = (1.0 - pedals * 0.5) * speed_factor + 1.0;
        // The lock and rate gains and the held lock read the running mean
        // of the shaped input (+0x20), not this frame's input.
        let held = self.steer_average.mean.abs();
        let mut lock = sample(&LOCK_GAIN, 0.0, 1.0, held) * sample(&MAX_ANGLE, 0.0, 160.0, speed) * pedal_bonus;

        // For a second after touching a wall (body +0x140), the lock is
        // held back more the faster the car goes: × g + (1 − g)(1 − x),
        // with x = forward speed / (0.7 × the computed top speed), at most
        // 1, and g rising over that second (0x405E8E, graph 0x7E9F58).
        if self.since_collision < 1.0 && h.computed_top_speed > 0.0 {
            let x = (self.speed() / (h.computed_top_speed * 0.7)).min(1.0);
            let g = graph(&[(0.0, 0.2), (0.2, 0.5), (0.5, 0.7), (0.7, 1.0)], self.since_collision);
            lock *= (1.0 - x) * (1.0 - g) + g;
        }

        let slip = self.slip_degrees();
        if self.drifting {
            if slip.abs() < DRIFT_OFF_DEGREES {
                self.drifting = false;
            }
        } else if slip.abs() > DRIFT_ON_DEGREES && speed >= DRIFT_ON_SPEED {
            self.drifting = true;
        }
        let counter_steering = input * slip < 0.0;
        if counter_steering && (self.drifting || slip.abs() > lock) {
            let share = sample(&COUNTER_STEER, 0.0, 1.0, input.abs());
            lock = (slip.abs() * share).min(45.0 * share);
        } else if held >= 0.5 {
            lock = lock.max(self.steer_limit);
        }
        lock = lock.min(45.0);
        self.steer_limit = lock;

        let target = (lock * ratio * input).clamp(-45.0, 45.0);
        // 180°/s, 110°/s in drift and URL events (0x4060E0, 0x783AE4 / 0x783AE8).
        let base_rate = if h.race.drift() || h.race.url() { 110.0 } else { 180.0 };
        // 0x4060E0: first the input's rate of change joins the +0x48 mean
        // (curve 0x7E9F28 of |input − last frame's| / dt), then that mean
        // (+0x54) scales the rate.
        if dt > 0.0 {
            self.steer_rate_average
                .add(sample(&INPUT_RATE_GAIN, 0.0, 10.0, ((input - self.last_steer_input) / dt).abs()), dt);
        }
        let rate = base_rate * speed_factor * self.steer_rate_average.mean * sample(&RATE_GAIN, 0.0, 1.0, held);
        let step = rate * ratio * dt;
        let angle = target.clamp(self.steer_degrees - step, self.steer_degrees + step);
        // 0x416D71…0x416DB4: kept as a 16-bit binary angle (+0x7C), the
        // degrees × 65536 truncated, then / 360 toward zero; read back
        // (0x416C20) as word × 360 / 65536, less 360 from 180 up. The input
        // update hands that word to the car (0x41E048). The |angle| < 0.0
        // (0x783AF4) test at 0x416D77 that would zero it never holds.
        self.steer_degrees = quantise_steer(angle);
        // After the steering, the shaped input joins the mean (0x41E03B).
        self.steer_average.add(input, dt);
        self.last_steer_input = input;
    }

    /// The AI driver's pedals as the mover reads them (0x5ABC00): the
    /// overlap check, skipped in URL events; reverse comes from the gear
    /// request, not a latch. Crash mode holds the gas at 0.4.
    pub(super) fn ai_pedals(&mut self, h: &Handling, c: AiControls) -> Controls {
        let (mut gas, brake, handbrake) =
            if h.race.url() { (c.gas, c.brake, c.handbrake) } else { pedal_overlap(c.gas, c.brake, c.handbrake) };
        if self.crash {
            gas = gas.min(0.4);
        }
        self.reverse_requested = c.gear == 0;
        self.last_gas = gas;
        Controls { throttle: gas, brake, handbrake, steer: 0.0, nitrous: false, ..Default::default() }
    }

    /// This frame's gas as the car's input block holds it (car +0x30 →
    /// +0x20C, the pedals the mover read: the player's after the pedal
    /// update, an AI driver's after the overlap check).
    pub fn gas(&self) -> f32 {
        self.frame_pedals.throttle
    }

    /// Set the frame's gas outside a mover step (synthetic states: tests,
    /// probes).
    pub fn set_gas(&mut self, gas: f32) {
        self.frame_pedals.throttle = gas;
    }

    /// The player's pedals as the car reads them (input block +0x20C gas,
    /// +0x210 handbrake, +0x214 brake), from the player input update
    /// (0x407A30) and the mover's pedal check (0x5ABC00).
    ///
    /// Reverse: the brake held (over 0.6, gas under 0.5) latches it, and
    /// it lets go once the brake does, unless the car is already backing
    /// faster than 2 m/s. Latched, near a standstill (−2..0.5 m/s) the brake
    /// pedal becomes a reverse throttle at twice the pedal, and backing
    /// faster the brake is the throttle and the gas the brake. Whenever
    /// that reverse throttle beats the gas by 0.2 it is the gas, and the
    /// input asks for reverse (+0x1E4 = 0); otherwise for the automatic
    /// gears. Then the stronger of gas and brake (with the handbrake under
    /// the gas) clears the other below half travel.
    ///
    /// Race branches: in URL events the speed the latch reads is divided by
    /// 4·|yaw rate| + 1, the gas goes through a curve (0x7EA014) and then a
    /// rate limit from last frame's gas (+1/s up, −1.5/s down), and gas and
    /// brake are both kept (0x5ABC00 skips its check). For 0.25 s after a
    /// drag race's green light the gas is doubled and at least 0.7.
    pub(super) fn pedals(&mut self, h: &Handling, controls: Controls, gear: usize, rpm: f32, dt: f32) -> Controls {
        let race = h.race;
        let mut speed = self.speed();
        if race.url() {
            speed /= self.spin.z.abs() * 4.0 + 1.0;
        }
        let (gas, brake) = (controls.throttle, controls.brake);
        if brake > 0.6 && gas < 0.5 {
            self.reverse_latched = true;
        } else if speed > -2.0 {
            self.reverse_latched = false;
        }
        let (reverse, mut gas, mut brake) = if speed > 0.5 || !self.reverse_latched {
            (0.0, gas, brake)
        } else if speed < -2.0 {
            (brake, 0.0, gas)
        } else {
            (brake, gas, 0.0)
        };
        let reverse = (reverse * 2.0).min(1.0);
        if race.drag() && self.launch_timer > 0.0 {
            self.launch_timer = (self.launch_timer - dt).max(0.0);
            if self.launch_timer > 0.0 {
                gas = (gas * 2.0).clamp(0.7, 1.0);
            }
        }
        let was_reverse = self.reverse_requested;
        self.reverse_requested = gas + 0.2 < reverse;
        // In a drag race in neutral (car +0x434 = 1), as in URL, the gas
        // moves at most +1/s and −1.5/s from last frame's (+0x20C); a pedal
        // over 0.6 (+0x2D4) below idle + 1000 rpm (H +0x1F0, record
        // +0x300; the car's rpm +0x440 unless 0x7FBED8 overrides it, 100
        // = none) gets at least 0.6 (0x407D57..0x407E17).
        let limited = race.url() || (race.drag() && gear == crate::handling::NEUTRAL);
        gas = if self.reverse_requested {
            reverse
        } else if limited {
            let curved = if race.url() { sample(&URL_GAS, 0.0, 1.0, gas) } else { gas };
            let mut g = curved.clamp(self.last_gas - dt * 1.5, self.last_gas + dt);
            if !race.url() && controls.throttle > 0.6 && rpm < h.engine.idle_rpm + 1000.0 && g < 0.6 {
                g = 0.6;
            }
            g.clamp(0.0, 1.0)
        } else {
            gas.clamp(0.0, 1.0)
        };
        // Back out of reverse the input's vt+0x30 runs (0x407E4E).
        // (Never under the hold, which puts the request back to neutral.)
        if was_reverse && !self.reverse_requested && !self.held {
            self.drive_off(race);
        }
        // The race start's hold: full brake after the pedals (0x41E399).
        if self.held {
            brake = 1.0;
        }
        // Crash mode holds the gas at 0.4 (0x5A49B0, before the throttle).
        if self.crash {
            gas = gas.min(0.4);
        }
        if race.url() {
            self.last_gas = gas;
            return Controls {
                throttle: gas,
                brake,
                handbrake: controls.handbrake,
                steer: controls.steer,
                nitrous: controls.nitrous,
                hydraulics: controls.hydraulics,
            };
        }

        let mut handbrake = controls.handbrake;
        if gas > brake {
            if handbrake < gas {
                if brake < 0.5 {
                    brake = 0.0;
                }
                if handbrake < 0.5 {
                    handbrake = 0.0;
                }
            }
        } else if gas < 0.5 {
            gas = 0.0;
        }
        self.last_gas = gas;
        Controls {
            throttle: gas,
            brake,
            handbrake,
            steer: controls.steer,
            nitrous: controls.nitrous,
            hydraulics: controls.hydraulics,
        }
    }

    /// 0x402660's slip: the mean |slip| of the driven wheels as the car
    /// exports it (car +0x40 → +0x374 + 8i = car +0x3B4 + 8i, read at
    /// 0x40268F; the wheel's +0x100, 0 while the tyre grips), the wheels
    /// 0x592880 counts by the rear share.
    pub fn ai_driven_slip(&self, split: f32) -> f32 {
        let driven: &[usize] = if split < 0.2 {
            &[0, 1]
        } else if split <= 0.8 {
            &[0, 1, 2, 3]
        } else {
            &[2, 3]
        };
        driven.iter().map(|&i| self.wheels[i].exported_slip.abs()).sum::<f32>() / driven.len() as f32
    }

    /// The AI racer's nitrous test (controller slot +0xA0, 0x40D850):
    /// a shot running or at least `min_seconds` in the tank (0x592490
    /// against skill row +0x570), all four wheels down (car +0x424 = 4),
    /// no live contact with an upright track polygon (0x5A0830 / 0x5A0170;
    /// car contacts do not count), the driven wheels'
    /// mean |slip| (car +0x3B4 + 8i, the wheels 0x592880 counts by the
    /// rear share +0x1C0: under 0.2 the front pair, over 0.8 the rear,
    /// else all four) not over 2 m/s (0x402660, 0x78353C), a gear of at
    /// least `min_gear` (car +0x434) and 0x402590 false: the rpm (car
    /// +0x440) **inside** the gear's band [A, A + 0.95·(B − A)] (0x402590
    /// returns 1 when the rpm differs from its clamp into the band,
    /// 0x40264A; the clone had it the other way round, so its AI
    /// shots ran only past 95 % of the band and ended at each shift). On
    /// the physics mover (car +0x28 = 1) A / B are 0x5946F0 / 0x594680
    /// (gear, 0); on the MeloMvr (3) the chassis' down / up rpm (+0x1C0 /
    /// +0x19C by gear, 0x4025D1); on any other mover the test passes.
    pub fn ai_nitrous_ok(&self, h: &Handling, min_seconds: f32, min_gear: usize) -> bool {
        let Some(tank) = &self.nitrous else { return false };
        if (tank.running.is_none() && tank.remaining() < min_seconds)
            || self.wheels_down() != 4
            || self.upright_contact()
        {
            return false;
        }
        if self.ai_driven_slip(h.gearbox.rear_split) > 2.0 {
            return false;
        }
        // On the MeloMvr the car's gear (+0x434) is the chassis' own and
        // so is the band (0x4025C8: car +0x28 = 3).
        if let Some(m) = &self.melo {
            return m.gear >= min_gear && m.in_nitrous_band();
        }
        let Some(d) = &self.drivetrain else { return false };
        d.gear >= min_gear && d.in_nitrous_band(h, self.shown_rpm)
    }

    /// Two-wheel Ackermann (0x59DF00): the inside wheel takes the steering
    /// angle δ, the outside one Lδ / (L + δW).
    pub(super) fn front_wheel_angle(&self, table: &[crate::car::Wheel; 4], i: usize) -> f32 {
        let delta = self.steer_angle();
        let length = table[0].position.x - table[3].position.x;
        let track = table[0].position.y - table[1].position.y;
        let outer = length * delta.abs() / (length + delta.abs() * track);
        // Wheel 0 is front left: inside when turning left (δ > 0).
        let inside = (i == 0) == (delta > 0.0);
        delta.signum() * if inside { delta.abs() } else { outer }
    }
}

#[cfg(test)]
mod ai_slip_tests {
    use super::CarState;
    use glam::Vec3;

    /// The AI's nitrous slip test (0x402660) reads the car's exported slip
    /// (car +0x3B4 + 8i): gripping tyres turning faster than they roll
    /// count nothing, sliding ones their |slip|, over the driven wheels.
    #[test]
    fn ai_nitrous_reads_the_exported_slip() {
        let mut s = CarState::new(Vec3::ZERO, 0.0);
        for w in &mut s.wheels {
            w.slip = 3.0;
        }
        assert_eq!(s.ai_driven_slip(1.0), 0.0);
        s.wheels[2].exported_slip = -5.0;
        s.wheels[3].exported_slip = 3.0;
        assert_eq!(s.ai_driven_slip(1.0), 4.0);
        assert_eq!(s.ai_driven_slip(0.0), 0.0);
        assert_eq!(s.ai_driven_slip(0.5), 2.0);
    }
}
