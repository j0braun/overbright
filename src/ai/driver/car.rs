use super::*;

/// What the action layer and the race hand the driver each frame.
pub struct DriverInputs<'a> {
    /// Up to 20 racing-line points ahead, world.
    pub path: &'a [Vec3],
    /// Target speed, m/s; negative to reverse (driver +0x230).
    pub target_speed: f32,
    /// Pedals allowed (+0x23C).
    pub enabled: bool,
    /// Past the green light.
    pub green: bool,
    /// The countdown (race state 3).
    pub countdown: bool,
    /// Seconds since the green light, negative before.
    pub since_green: f32,
    /// The game clock, s.
    pub clock: f64,
}

/// One AI racer's driver (the 0x460-byte object 0x425AB0 builds).
#[derive(Clone, Debug)]
pub struct AiDriver {
    /// +0x22C: the most gas it gives.
    throttle_cap: f32,
    /// +0x224: the look-ahead point.
    pub target: Vec2,
    /// +0x1E0: the front-wheel angle, 16-bit.
    steer: u16,
    /// +0x1E4: the gear request.
    gear: i32,
    /// The pedals as the car last read them (+0x20C, +0x214, +0x210).
    gas: f32,
    brake: f32,
    handbrake: f32,
    /// +0x240: while running and above 20 mph, the handbrake alone.
    pub handbrake_timer: f32,
    /// +0x244: going the way the target asks.
    direction_ok: bool,
    /// +0x248: while running, the table steering (mode 5).
    pub table_timer: f32,
    /// +0x24C: stuck: everything on.
    pub stuck: bool,
    /// +0x250: 1/2 re-seed the steering trackers, 3 the speed tracker, 4
    /// a new path (no rate on the next update).
    pub event: u8,
    /// +0x258: when the front wheels left the ground.
    takeoff: Option<f64>,
    /// +0x25C: the nitrous hold (set while a shot runs, 0x421F45).
    nitrous_hold: bool,
    /// +0x260: last frame's target speed.
    pub prev_target: f32,
    /// +0x230: this frame's target speed (vt+0x38; 0 with no path).
    pub target_speed: f32,
    /// +0x10: the path's point count this frame.
    pub path_len: usize,
    /// +0x264, +0x268, +0x26C: heading error, cross-track angle, blend.
    heading: Tracker,
    cross: Tracker,
    blend: Tracker,
    /// +0x270: speed error; +0x274 / +0x278: wheel slip beyond 4 m/s.
    speed: Tracker,
    lon_slip: Tracker,
    lat_slip: Tracker,
    /// +0x280: the steering PID (AdaptivePIDControllerComplicated, terms
    /// at +0x328).
    steer_pid: AdaptivePid,
    /// +0x334: the speed PID (AdaptivePIDControllerSimple, vtable
    /// 0x784614) and its own tracker.
    speed_pid: AdaptivePid,
    speed_pid_tracker: Tracker,
    /// The racer has finished (its status record +0x10 ≠ 0): no traction
    /// cap.
    pub finished: bool,
    /// The car's mover as the driver reads it each frame (car +0x28: 1 full
    /// physics, every racer here; 2 the TrafficMover; 3 the MeloMvr; 4 the
    /// 0x5A9040 one) and whether the car is traffic (car +0x24 = 4): they
    /// pick the driver's mode ([`AiDriver::mode`]).
    pub mover: u32,
    pub traffic: bool,
}

impl AiDriver {
    pub fn new(_handling: &Handling, _suv: bool, throttle_cap: f32) -> AiDriver {
        let steer_pid = AdaptivePid::new(0.2);
        let speed_pid = AdaptivePid::new(0.2);
        AiDriver {
            throttle_cap,
            target: Vec2::ZERO,
            steer: 0,
            gear: 1,
            gas: 0.0,
            brake: 0.0,
            handbrake: 0.0,
            handbrake_timer: 0.0,
            direction_ok: true,
            table_timer: 0.0,
            stuck: false,
            event: 0,
            takeoff: None,
            nitrous_hold: false,
            prev_target: 0.0,
            target_speed: 0.0,
            path_len: 0,
            heading: Tracker::new(5, 5, 30.0),
            cross: Tracker::new(5, 5, 30.0),
            blend: Tracker::new(5, 5, 30.0),
            speed: Tracker::new(4, 4, 30.0),
            lon_slip: Tracker::new(4, 4, 30.0),
            lat_slip: Tracker::new(4, 4, 30.0),
            steer_pid,
            speed_pid,
            speed_pid_tracker: Tracker::new(4, 4, 30.0),
            finished: false,
            mover: 1,
            traffic: false,
        }
    }

    /// The driver's mode and whether it steers by the table gains, by the
    /// car's mover (0x421030 at 0x421116; car +0x28): full physics (1) mode 1
    /// on the adaptive PID; the MeloMvr (3) mode 4 on the table for a traffic
    /// car (+0x24 = 4), else mode 0 on the PID; mover 4 mode 0 on the PID
    /// (whose steering still takes the table formula, see `update`); any
    /// other — the TrafficMover (2) — mode 3 on the table. The steering
    /// table's timer and the stuck flag then make it 5 and 6.
    pub fn mode(&self) -> (usize, bool) {
        match self.mover {
            1 => (1, false),
            3 if self.traffic => (4, true),
            3 | 4 => (0, false),
            _ => (3, true),
        }
    }

    /// One frame (0x421030).
    pub fn update(
        &mut self,
        car: &CarState,
        handling: &Handling,
        suv: bool,
        input: &DriverInputs,
        dt: f32,
    ) -> AiControls {
        let now = input.clock;
        let dt_floor = dt.max(1e-4);
        let mut wheel_deg = self.steer as f32 * 0.005_493_164;
        if wheel_deg > 180.0 {
            wheel_deg -= 360.0;
        }
        let speed_abs = car.velocity.length();
        let forward = car.speed();
        let v = if forward < 0.0 { -speed_abs } else { speed_abs };

        // The look-ahead point (0x415F80); with no path the target speed
        // is dropped.
        let mut target_speed = input.target_speed;
        if input.path.is_empty() {
            target_speed = 0.0;
        }
        self.target_speed = target_speed;
        self.path_len = input.path.len();
        // The driver measures from the car's centre of mass (car +0x60:
        // 0x403AF0, 0x403BA0, 0x415630, 0x4054D0), not the body origin.
        let centre = centre_of_mass(car, handling);
        self.target = self.target_point(car, centre, handling, input);

        let mut front_down = false;
        let mut driven_down = false;
        for i in 0..4 {
            if car.wheels[i].on_ground {
                front_down |= i < 2;
                driven_down |= driven(handling, i);
            }
        }
        // The mode by the car's mover (`mode`; racers run full physics:
        // mode 1, no table steering), unless the controller's timer or the
        // stuck flag say otherwise.
        let (mut mode, table_mode) = self.mode();
        self.table_timer -= dt;
        if self.table_timer > 0.0 {
            mode = 5;
        }
        if self.stuck {
            mode = 6;
        }
        let (e_heading, e_blend, e_cross) = self.angle_errors(car, centre, input.path);
        match self.event {
            1 => {
                self.heading.reset(e_heading);
                self.cross.reset(e_cross);
                self.blend.reset(e_blend);
                self.event = 0;
            }
            2 => {
                self.heading.reset_rate(e_heading);
                self.cross.reset_rate(e_cross);
                self.blend.reset_rate(e_blend);
                self.event = 0;
            }
            _ => {}
        }

        let mut p_error = 0.0;
        let mut delta;
        if front_down {
            let fresh = self.event == 4;
            self.takeoff = None;
            self.heading.update(e_heading, dt, fresh, false);
            self.cross.update(e_cross, dt, fresh, false);
            self.blend.update(e_blend, dt, fresh, false);
            if fresh {
                self.event = 0;
            }
            // The heading and blended errors weighted by their squares
            // when they agree, the heading alone when they don't.
            let (a, b) = (self.heading.cur, self.blend.cur);
            let (wa, wb) = if a * b >= 0.0 {
                let sum = a * a + b * b;
                if sum >= 1e-5 { (a * a / sum, b * b / sum) } else { (0.5, 0.5) }
            } else {
                (1.0, 0.0)
            };
            let p = wa * a + wb * b;
            let i = (wa * self.heading.integral() + wb * self.blend.integral()).clamp(-I_LIMIT, I_LIMIT);
            let d = (wa * self.heading.rate() + wb * self.blend.rate()).clamp(-1.0, 1.0);
            p_error = p;
            // The table formula for table modes, mover 4 and mode 5
            // (0x42172C: a table mode, +0x28 = 4 or mode 5).
            if table_mode || self.mover == 4 || mode == 5 {
                delta = -(TABLE_P[mode] * p + TABLE_D[mode] * d + TABLE_I[mode] * i);
            } else {
                // The rate |error| changes at against the rate it should
                // shrink at drives the gains.
                let r = if d * p < 0.0 { -d.abs() } else { d.abs() };
                let (rates, bounds) = if suv { STEER_GAINS_SUV } else { STEER_GAINS_CAR };
                let pid = &mut self.steer_pid;
                pid.terms = [-p, -i, -d];
                pid.period = 0.2;
                pid.thresholds = [0.01; 3];
                pid.rates = rates;
                pid.bounds = bounds;
                let x = r.to_degrees() - pairs(&SHRINK_RATE, p.abs().to_degrees());
                pid.adapt(x, dt, now);
                if forward < 10.0 {
                    pid.gains = [
                        table(&SEED_KP, 0.0, 160.0, speed_abs),
                        table(&SEED_KI, 0.0, 160.0, speed_abs),
                        table(&SEED_KD, 0.0, 160.0, speed_abs),
                    ];
                }
                delta = pid.output();
                if pid.terms[2] > 999.0 {
                    delta *= 999.0;
                }
            }
            // Pinned: the first of the car's records (car +0x3C, 0x5A0830)
            // made over 0.6 s ago and pushed in the last 0.1 s, against an
            // upright polygon (0x5A0170) or a car under 60 mph (0x592C70,
            // the other car's +0x430 < 26.82): steer by the lock curve at
            // the signed speed, growing over a second, away from the
            // contact's side of the way of travel (0x591FD0).
            if let Some(n) = car.contacts.iter().find(|n| {
                n.age < 0.1
                    && n.life > 0.6
                    && match n.touching {
                        crate::drive::Touching::Car { speed } => speed < 26.821_636,
                        crate::drive::Touching::Track { upright } => upright,
                    }
            }) {
                let (right, _) = travel_frame(car, n.point);
                let angle = ((n.life - 0.6) / 1.0).min(1.0) * table(&*LOCK, 0.0, 160.0, v) * 0.017_453_292;
                delta = if right <= 0.0 { -angle } else { angle };
            }
        } else {
            let since = *self.takeoff.get_or_insert(now);
            self.event = 0;
            // Past 0.3 s against the slip, within ±5° on the MeloMvr
            // (+0x28 = 3); the gains' caps by SUV only on full physics.
            delta = if (now - since) as f32 <= 0.3 {
                radians16(self.steer)
            } else if self.mover == 3 {
                (-slip_angle(car)).clamp(-0.087_266_46, 0.087_266_46)
            } else {
                -slip_angle(car)
            };
            let (kp, kd) = if suv && self.mover == 1 { (0.4, 0.1) } else { (0.15, 0.05) };
            let g = &mut self.steer_pid.gains;
            g[0] = g[0].min(kp);
            g[1] = g[1].min(0.0);
            g[2] = g[2].min(kd);
        }
        let delta = delta.clamp(-0.785_398_2, 0.785_398_2);

        if self.event == 3 {
            self.speed.reset(target_speed.abs() - speed_abs);
            self.event = 0;
        }
        self.speed.update(target_speed.abs() - speed_abs, dt, false, false);
        self.track_slip(car, dt);

        // Pedals by the target's direction.
        let mut steer = delta;
        let mut gear = -1;
        let (mut gas, mut brake) = (0.0, 0.0);
        let control: bool;
        if driven_down {
            if target_speed.abs() < 2.0 {
                brake = 1.0;
                control = self.stuck;
            } else if target_speed <= 0.0 {
                if v > 7.0 {
                    self.direction_ok = false;
                } else if v < 2.0 {
                    self.direction_ok = true;
                }
                if self.direction_ok {
                    gear = 0;
                    steer = -delta;
                    control = true;
                } else {
                    self.speed.reset(0.0);
                    brake = 0.9;
                    control = self.stuck;
                }
            } else {
                if v < -7.0 {
                    self.direction_ok = false;
                } else if v > -2.0 {
                    self.direction_ok = true;
                }
                if self.direction_ok {
                    control = true;
                } else {
                    self.speed.reset(0.0);
                    steer = -delta;
                    brake = 0.9;
                    control = self.stuck;
                }
            }
        } else {
            gas = self.throttle_cap;
            self.speed.reset(0.0);
            control = self.stuck;
        }
        if control {
            (gas, brake) = self.speed_control(car, handling, target_speed, mode, dt, input);
        }

        // The wheel angle: the steering over the lock, moving at most
        // 100 lock-widths a second.
        let lock = if table_mode || handling.race.drag() || !handling.race.drift() { table_fn_lock(v) } else { 45.0 };
        let wanted = (steer / (lock * 0.017_453_292)).clamp(-1.0, 1.0);
        let current = wheel_deg / lock;
        let mut change = wanted - current;
        if table_mode {
            let mut rate = table(&TABLE_RATE_ERROR, 0.0, 95.0, p_error.abs().to_degrees())
                * table(&*TABLE_RATE_SPEED, 0.0, 160.0, v);
            if (current > 0.0 && wanted < current) || (current < 0.0 && current < wanted) {
                rate *= 2.0;
            }
            change = change.clamp(-rate * dt_floor, rate * dt_floor);
        } else {
            change = change.clamp(-100.0 * dt_floor, 100.0 * dt_floor);
        }
        let mut new_deg = (change + current) * lock;
        // The table modes scale it by H +0x270 (0x421E8C; H = car +0x20, the
        // tuning block at record +0x110, so record +0x380: the steering
        // ratio, 1 for the traffic models), not record +0x270.
        if table_mode {
            new_deg *= handling.steering_ratio;
        }

        // The countdown revs at the line; otherwise the pedals only when
        // allowed.
        if input.countdown && !input.green {
            gear = 1;
            brake = 1.0;
            gas = 1.0;
        } else if !input.enabled {
            gas = 0.0;
            brake = 0.0;
        }
        // The nitrous hold (0x421F45..0x421FB0): while a shot runs (0x59FA40
        // on the car, its stage 1–3) the hold (+0x25C) is set; while it is
        // set the pedals are the throttle cap (+0x22C) and no brake, until
        // a drag race (0x89E7D8 / 0x890108) or a target (+0x230) under the
        // last frame's (+0x260) clears it — after the shot too.
        if car.nitrous.as_ref().is_some_and(|t| t.running_stage() != 0) {
            self.nitrous_hold = true;
        }
        if self.nitrous_hold {
            if handling.race.drag() || target_speed < self.prev_target {
                self.nitrous_hold = false;
            } else {
                gas = self.throttle_cap;
                brake = 0.0;
            }
        }
        self.prev_target = target_speed;
        let mut handbrake = 0.0;
        self.handbrake_timer -= dt;
        if self.handbrake_timer > 0.0 && speed_abs > 8.940_545 {
            brake = 0.0;
            handbrake = 1.0;
            gas = 0.0;
        }
        self.steer = (((new_deg * 65536.0) as i32) / 360) as u16;
        self.gear = gear;
        if self.stuck {
            brake = 1.0;
            handbrake = 1.0;
            gas = 0.0;
        }
        // The car reads the block through its overlap check and writes it
        // back (0x5ABC00), which next frame's speed control starts from.
        let (gas, brake, handbrake) = if handling.race.url() {
            (gas, brake, handbrake)
        } else {
            crate::drive::pedal_overlap(gas, brake, handbrake)
        };
        self.gas = gas;
        self.brake = brake;
        self.handbrake = handbrake;
        AiControls {
            steer_degrees: self.steer as i16 as f32 * (360.0 / 65536.0),
            gas,
            brake,
            handbrake,
            gear,
            nitrous: false,
            mover_held: false,
        }
    }

    /// The update in which the race goes green (race status 3 → 4), before
    /// the racer has its goal: the input's brake reads 0 and the gear
    /// request −1 (none), while the gas (1 from the countdown) and the
    /// wheel angle stay as they were; the target speed is still 0
    /// (**observed** under Wine on every rival, docs/notes/parity/race-ai-parity.md:
    /// the first frame with status 4 has gas 1, brake 0, gear request −1,
    /// steer 0, +0x230 = 0, no goal; the store that clears the brake is
    /// not traced). The next update's speed control (0x41D8D0) starts from
    /// gas − brake = 1, so the launch is full throttle.
    pub fn green_release(&mut self) -> AiControls {
        self.brake = 0.0;
        self.handbrake = 0.0;
        self.gear = -1;
        self.target_speed = 0.0;
        AiControls {
            steer_degrees: self.steer as i16 as f32 * (360.0 / 65536.0),
            gas: self.gas,
            brake: 0.0,
            handbrake: 0.0,
            gear: -1,
            nitrous: false,
            mover_held: true,
        }
    }

    /// The start scene's hold (race status 2): neutral, brake 1, no gas;
    /// the wheel angle kept (see `racer_think`).
    pub fn scene_hold(&mut self) -> AiControls {
        self.gas = 0.0;
        self.brake = 1.0;
        self.gear = 1;
        AiControls {
            steer_degrees: self.steer as i16 as f32 * (360.0 / 65536.0),
            gas: 0.0,
            brake: 1.0,
            handbrake: 0.0,
            gear: 1,
            nitrous: false,
            mover_held: false,
        }
    }

    /// The point to steer at (0x415F80): the look-ahead distance by speed
    /// and performance class along the path (0x415630), or straight ahead
    /// without one, from the centre of mass `pos` (car +0x60).
    pub(super) fn target_point(&self, car: &CarState, pos: Vec3, h: &Handling, input: &DriverInputs) -> Vec2 {
        let forward = car.speed();
        // Modes 3 and 4 (the TrafficMover; a traffic car on the MeloMvr)
        // look max(time × forward speed, least) ahead (0x415FC0), the rest
        // by the table (0x415FED; the class factor of a car on mover 3 or 4
        // is table 0x7E9DE4, not ported: no such car here).
        let look = match self.mode() {
            (m @ (3 | 4), _) => (forward * LOOK_TIME[m]).max(LOOK_LEAST[m]),
            _ => table(&LOOKAHEAD, 0.0, 160.0, forward) * table(&LOOKAHEAD_CLASS, 0.0, 3.0, h.performance_class as f32),
        };
        let path = input.path;
        if path.is_empty() {
            let ahead = look.max(5.0);
            let dir = if forward >= 5.0 {
                scaled(car.velocity, ahead)
            } else {
                car.forward().truncate().normalize_or_zero().extend(0.0) * ahead
            };
            return (pos + dir).truncate();
        }
        path_point(car, pos, path, look, input.since_green).truncate()
    }
}

/// The point `look` metres along the path from the car (0x415630): the
/// car's position with no path, `look` towards a lone point, else along
/// the segments from the first point ahead of the travel, corners counting
/// 7× in the first 5 s of the race (0x402000), past the end straight on.
/// `pos` is where the car is measured from: the racers' and traffic's
/// driver passes the centre of mass (car +0x60, 0x41564A / 0x4156DD).
pub fn path_point(car: &CarState, pos: Vec3, path: &[Vec3], look: f32, since_green: f32) -> Vec3 {
    {
        if path.is_empty() {
            return pos;
        }
        if path.len() == 1 {
            return pos + scaled(path[0] - pos, look);
        }
        // The first point ahead along the velocity (0x404EF0).
        let travel = flat_unit(car.velocity);
        let first = path
            .iter()
            .position(|p| flat_unit(*p - pos).dot(travel) > 0.0 && (*p - pos).truncate().length_squared() > 0.0)
            .unwrap_or(path.len() - 1);
        // Distances along the path, corners counting extra: 7× in the
        // first 5 s of the race (0x402000: −1 before it), then 1×.
        let k = if since_green < 5.0 { 7.0 } else { 1.0 };
        let mut acc = vec![0.0f32; path.len() + 1];
        acc[first + 1] = path[first].distance(pos);
        let mut j = first;
        if acc[first + 1] < look {
            j = first + 1;
            let (mut seg_prev, mut before) = (acc[first + 1], pos);
            while j < path.len() {
                let seg = path[j].distance(path[j - 1]);
                acc[j + 1] = (seg + seg_prev - path[j].distance(before)) * k + acc[j] + seg;
                if look <= acc[j + 1] {
                    break;
                }
                seg_prev = seg;
                before = path[j - 1];
                j += 1;
            }
        }
        if j < path.len() {
            let (base, from) = if first < j { (path[j - 1], acc[j]) } else { (pos, 0.0) };
            let t = (look - from) / (acc[j + 1] - from);
            return base + (path[j] - base) * t;
        }
        let n = path.len();
        let last = path[n - 1];
        last + scaled(last - path[n - 2], look - acc[n])
    }
}

impl AiDriver {
    /// The three angle errors (0x4054D0), radians: heading against the
    /// target's direction; that blended with the direction of travel above
    /// 5 m/s; and the cross-track angle atan2(offset, 5 m); from the centre
    /// of mass `pos` (car +0x60, 0x4054E1 / 0x4054F2).
    pub(super) fn angle_errors(&self, car: &CarState, pos: Vec3, path: &[Vec3]) -> (f32, f32, f32) {
        let to_target = angle16(self.target.x - pos.x, self.target.y - pos.y);
        let f = car.forward();
        let heading = angle16(f.x, f.y);
        let e1 = radians16(heading.wrapping_sub(to_target));
        let w = pairs(&TRAVEL_WEIGHT, car.velocity.length());
        let e2 = if w <= 0.0 {
            e1
        } else {
            let travel = angle16(car.velocity.x, car.velocity.y);
            radians16(travel.wrapping_sub(to_target)) * w + (1.0 - w) * e1
        };
        let e3 = radians16(angle16(5.0, cross_track(path, pos)));
        (e1, e2, e3)
    }

    /// The wheels' slip beyond 4 m/s (0x7839F4 / 0x7839F8), summed
    /// (0x4057C0): along the wheels (negated when the car's forward speed
    /// +0x3EC is negative) into the tracker at +0x274, |across| into
    /// +0x278. 0x4057C0 reads the car's exported pair (car +0x3B4 / +0x3B8
    /// + 8i, from 0x4057E4), not the wheels' own slip: on the physics
    /// mover the wheels' +0x100 (`exported_slip`: the slip while the tyre
    /// slides, 0 while it grips, 0x5A8600 / 0x5A86A1) and +0x104 (the speed
    /// across the wheel, stored in both of the tyre pass's branches,
    /// 0x5A8606 / 0x5A86AB: `lateral_slip`); on the MeloMvr and the NIS
    /// mover the write-back 0x5A2B70 copies the chassis' pair +0x90 + 8i
    /// (`melo.rs` / `nis.rs` set both halves).
    pub(super) fn track_slip(&mut self, car: &CarState, dt: f32) {
        let (mut lon, mut lat) = (0.0, 0.0);
        for w in &car.wheels {
            if w.exported_slip.abs() > 4.0 {
                lon += w.exported_slip;
            }
            if w.lateral_slip.abs() > 4.0 {
                lat += w.lateral_slip.abs();
            }
        }
        if car.speed() < 0.0 {
            lon = -lon;
        }
        self.lon_slip.update(lon, dt, false, false);
        self.lat_slip.update(lat, dt, false, false);
    }

    /// Gas and brake for the target speed (0x41D8D0).
    pub(super) fn speed_control(
        &mut self,
        car: &CarState,
        h: &Handling,
        target: f32,
        mode: usize,
        dt: f32,
        input: &DriverInputs,
    ) -> (f32, f32) {
        let t = target.abs();
        let v = car.velocity.length();
        let (gas, brake);
        if self.stuck || t >= 5.0 {
            let (u, lower, upper);
            if t <= 30.0 {
                let d = self.speed.rate().clamp(-10.0, 10.0);
                let i = self.speed.integral();
                let (kp, ki, kd);
                if t + 2.0 <= v {
                    (kp, ki, kd, lower, upper) = (SPEED_PB[mode], SPEED_IB[mode], SPEED_DB[mode], -1.0, 0.0);
                } else {
                    (kp, ki, kd, lower, upper) =
                        (SPEED_PA[mode], SPEED_IA[mode], SPEED_DA[mode], 0.0, self.throttle_cap);
                }
                u = kp * self.speed.cur + ki * i + kd * d;
            } else {
                let pid = &mut self.speed_pid;
                pid.period = 0.2;
                pid.bounds = [(-0.3, -0.2), (0.0, 0.0), (-0.6, 0.0)];
                pid.term_limits[2] = (-10.0, 10.0);
                pid.thresholds = [0.01; 3];
                pid.rates = [5e-6; 3];
                let jump = (target - self.prev_target).abs() > 1.0;
                let c = pairs(&SPEED_RATE, v - t);
                let tr = &mut self.speed_pid_tracker;
                tr.update(v - t, dt, jump, false);
                pid.terms = [tr.cur, tr.integral(), tr.rate()];
                pid.adapt(tr.rate() - c, dt, input.clock);
                if v < 10.0 {
                    pid.gains = [-1.0, 0.0, 0.0];
                }
                u = pid.output();
                (lower, upper) = (-1.0, 1.0);
            }
            let rate = table(&PEDAL_RATE, 0.0, 160.0, v) * dt;
            let pedal = (self.gas - self.brake + u.clamp(-rate, rate)).max(lower).min(upper);
            // The traction cap from the wheels' slip along the road.
            let mut cap = 1.0;
            if self.traction_capped(car, input) {
                let s = &self.lon_slip;
                let (a, b, c) = if pedal >= 0.0 { (-0.2, -0.02, -0.005) } else { (0.02, 0.02, 0.005) };
                cap = (a * s.cur + b * s.integral() + c * s.rate() + 1.0).max(0.0);
            }
            if self.lat_slip.cur > 999.0 || self.steer_pid.terms[2].abs() > 999.0 {
                cap = 0.0;
            }
            if pedal >= 0.0 {
                gas = pedal.min(cap);
                brake = 0.0;
            } else {
                gas = 0.0;
                brake = (-pedal).min(cap);
            }
        } else {
            let gap = (t - v).abs();
            if gap > 1.0 {
                if t <= v {
                    gas = 0.0;
                    brake = 1.0;
                } else {
                    gas = 1.0;
                    brake = 0.0;
                }
            } else {
                gas = 0.0;
                brake = 0.0;
            }
        }
        let _ = h;
        (gas.max(0.0).min(self.throttle_cap), brake.clamp(0.0, 1.0))
    }

    /// Whether the slip caps the pedal (0x410040): a racer still racing,
    /// past the first 4 s of the race unless sliding over 5°.
    pub(super) fn traction_capped(&self, car: &CarState, input: &DriverInputs) -> bool {
        let slip = slip_angle(car);
        let early = input.since_green < 0.0 || input.since_green <= 4.0;
        let sliding = slip.abs() > 5.0f32.to_radians();
        !self.finished && (!early || sliding)
    }
}

/// The steering lock at signed speed, 0..45°.
pub(super) fn table_fn_lock(v: f32) -> f32 {
    table(&*LOCK, 0.0, 160.0, v).clamp(0.0, 45.0)
}

/// Whether wheel `i` is driven (0x592880): front with under 20% to the
/// rear, rear with over 80%, else all.
pub(super) fn driven(h: &Handling, i: usize) -> bool {
    let split = h.gearbox.rear_split;
    if split < 0.2 {
        i < 2
    } else if split > 0.8 {
        i >= 2
    } else {
        true
    }
}

/// The body's slip angle (0x59FA20): heading less direction of travel,
/// none under 1 m/s forward.
pub(super) fn slip_angle(car: &CarState) -> f32 {
    if car.speed() < 1.0 {
        return 0.0;
    }
    car.slip_degrees().to_radians()
}

/// A vector scaled to `length` (0x43CF90; (length, 0, 0) for a zero one).
pub(super) fn scaled(v: Vec3, length: f32) -> Vec3 {
    let l = v.length();
    if l == 0.0 { Vec3::new(length, 0.0, 0.0) } else { v * (length / l) }
}

/// The car's signed distance to the right of the path segment it is on
/// (0x404AF0): the last segment whose start it is past; 3D distance to a
/// lone point; 0 without a path.
pub(super) fn cross_track(path: &[Vec3], p: Vec3) -> f32 {
    match path.len() {
        0 => return 0.0,
        1 => return p.distance(path[0]),
        _ => {}
    }
    let (mut rel, mut seg) = (p - path[0], path[1] - path[0]);
    if seg.dot(rel) > 0.0 {
        for k in 1..path.len() - 1 {
            let (next_seg, next_rel) = (path[k + 1] - path[k], p - path[k]);
            if next_seg.dot(next_rel) <= 0.0 {
                break;
            }
            seg = next_seg;
            rel = next_rel;
        }
    }
    seg.cross(Vec3::Z).normalize_or_zero().dot(rel)
}

/// The car's centre of mass in the world (the original's car +0x60): the
/// body origin plus the handling's offset turned with the body.
pub(super) fn centre_of_mass(car: &CarState, h: &Handling) -> Vec3 {
    car.position + car.orientation * Vec3::from(h.centre_of_mass)
}

#[cfg(test)]
mod native_tests {
    use super::*;

    fn handling() -> Handling {
        crate::handling::parse_handling(&vec![0u8; 0x1000])
    }

    fn input(path: &[Vec3], target: f32) -> DriverInputs<'_> {
        DriverInputs { path, target_speed: target, enabled: true, green: true, countdown: false, since_green: 10.0, clock: 10.0 }
    }

    /// 0x4057C0 sums the car's exported slip pair (car +0x3B4 / +0x3B8 +
    /// 8i), not the wheels' own slip: a wheel whose own slip is over 4 m/s
    /// but whose export is 0 adds nothing. Under Wine (stage-4 drag, three
    /// rivals, 1895 frames each) the trackers +0x274 / +0x278 equalled the
    /// sums of the previous frame's exported pair on every frame.
    #[test]
    fn the_slip_tracker_reads_the_exported_pair() {
        let h = handling();
        let mut d = AiDriver::new(&h, false, 1.0);
        let mut car = CarState::new(Vec3::ZERO, 0.0);
        car.velocity = Vec3::new(20.0, 0.0, 0.0);
        car.publish_forward_speed();
        for w in &mut car.wheels {
            w.slip = 9.0;
            w.exported_slip = 0.0;
            w.lateral_slip = 0.0;
        }
        d.track_slip(&car, 1.0 / 60.0);
        assert_eq!((d.lon_slip.cur, d.lat_slip.cur), (0.0, 0.0));
        car.wheels[2].exported_slip = 6.5;
        car.wheels[3].exported_slip = -5.0;
        car.wheels[0].exported_slip = 3.9;
        car.wheels[1].lateral_slip = -4.5;
        d.track_slip(&car, 1.0 / 60.0);
        assert_eq!(d.lon_slip.cur, 1.5);
        assert_eq!(d.lat_slip.cur, 4.5);
    }

    /// 0x421F45..0x421FB0: while a nitrous shot runs (0x59FA40) the driver
    /// holds the throttle cap and no brake whatever the target, until the
    /// target falls under the last frame's (and never in a drag race).
    /// Under Wine an outrun rival given goal 1 at the end (target 43 m/s at
    /// 57 m/s) kept accelerating through its shot.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_running_shot_holds_the_throttle() {
        let h = handling();
        let path = [Vec3::new(40.0, 0.0, 0.0), Vec3::new(400.0, 0.0, 0.0)];
        let run = |shot: bool| {
            let mut d = AiDriver::new(&h, false, 1.0);
            let mut car = CarState::new(Vec3::ZERO, 0.0);
            car.velocity = Vec3::new(57.0, 0.0, 0.0);
            car.publish_forward_speed();
            for w in &mut car.wheels {
                w.on_ground = true;
            }
            let mut tank = crate::nitrous::Tank::new(6.0, 3);
            if shot {
                assert!(tank.start());
            }
            car.nitrous = Some(tank);
            let mut c = d.update(&car, &h, false, &input(&path, 43.0), 1.0 / 60.0);
            for _ in 0..30 {
                c = d.update(&car, &h, false, &input(&path, 43.0), 1.0 / 60.0);
            }
            (d, car, c)
        };
        let (_, _, free) = run(false);
        assert!(free.gas < 0.01, "no shot: no throttle 14 m/s over the target ({}, {})", free.gas, free.brake);
        let (mut d, mut car, held) = run(true);
        assert_eq!((held.gas, held.brake), (1.0, 0.0), "a running shot holds the throttle");
        // The shot over, the hold stays while the target holds ...
        car.nitrous.as_mut().unwrap().stop();
        let c = d.update(&car, &h, false, &input(&path, 43.0), 1.0 / 60.0);
        assert_eq!((c.gas, c.brake), (1.0, 0.0));
        // ... and a lower target ends it.
        d.update(&car, &h, false, &input(&path, 42.0), 1.0 / 60.0);
        assert!(!d.nitrous_hold);
    }
}
