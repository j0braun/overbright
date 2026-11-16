use super::*;

/// Rear-wheel grip with Stability Control off (0x59D4A0, applied by the
/// friction circle 0x59D610 and the tyre's sliding μ 0x5A8290 when car
/// +0x464 is 1): 0.85 (0x7FC71C) up to 13.41 m/s (30 mph), rising linearly
/// (× 0.11185) to 1 at 22.35 m/s (50 mph), for a rear wheel (wheel-table
/// index ≥ 2) that is not locked (+0x10C) and spins forwards (+0x28 ≥ 0).
pub(super) fn launch_grip(forward_speed: f32, locked: bool, spin: f32) -> f32 {
    if locked || spin < 0.0 {
        return 1.0;
    }
    let t = ((forward_speed - 13.410818) * 0.11185).clamp(0.0, 1.0);
    (1.0 - 0.85) * t + 0.85
}

/// The shown rpm's step (0x5A5966..0x5A5A61, the mover's publish, once per
/// mover call): a fall faster than `rate` = −table / crank inertia (engine
/// data +0xC) is held to it (only when the call's dt is above 0), then the
/// shown value moves 55 % (0x7A20AC) of the way, keeping 45 % (0x7A20B0).
/// The table 0x7A1A34 is indexed by trunc(record +0x2F4 + 0.5) plus 4
/// while an up-shift past 1st is pending (drivetrain +0x48 and target
/// +0x38 > 2): 2.5 for every index 0..3, 15 shifting (**verified** values;
/// the index's float is not traced, every entry of a row being equal).
/// The 0x7FBED8 override (a tweak, 100 = off) is not taken.
pub fn shown_rpm_step(shown: f32, engine_rpm: f32, inertia: f32, shifting: bool, dt: f32) -> f32 {
    let rate = -(if shifting { 15.0f32 } else { 2.5 }) / inertia;
    let mut target = engine_rpm;
    if dt > 0.0 && (target - shown) / dt < rate {
        let held = rate * dt + shown;
        if held > target {
            target = held;
        }
    }
    shown * 0.45 + target * 0.55
}

/// 5A856A..5A85F2: low-speed sliding uses the retained signed forward
/// speed (physics-view +0x3EC); arithmetic order stays unchanged here.
pub(super) fn sliding_forces(slip: f32, lateral: f32, load: f32, mu: f32, forward_speed: f32) -> (f32, f32) {
    let len = (slip * slip + lateral * lateral).sqrt();
    let k = load / len * mu;
    let (mut fx, mut fy) = (slip * k, -k * lateral);
    if forward_speed * MPH < 1.0 && mu > 0.1 {
        fx /= mu;
        fy /= mu;
    }
    (fx, fy)
}

/// Gripping wheel slip removal in 0x59D360 (after torque and drag).
/// 0x59D3DE compares the substep against float 0x78435C (strictly > .001).
pub(super) fn gripping_wheel_spin_rate(spin_rate: f32, slip: f32, radius: f32, dt: f32) -> f32 {
    if dt > 0.001 { spin_rate - slip / (radius * dt) } else { spin_rate }
}

/// Lateral tyre force for a load (kN) and slip angle (radians), 0x59D520.
pub(super) fn lateral_force(tyre: &crate::handling::Tyre, load: f32, slip_angle: f32) -> f32 {
    let x = slip_angle.to_degrees() / tyre.slip_angle_scale * 12.0 * 0.5;
    let load_x = 0.8 * load / tyre.load_scale;
    let curve = |i: usize| sample(&LATERAL[i.min(6)], 0.0, 10.0, load_x);
    let i = x as usize;
    let f = if i >= 6 {
        curve(6)
    } else {
        let t = x - i as f32;
        curve(i) * (1.0 - t) + curve(i + 1) * t
    };
    f * tyre.lateral_scale * 2.5
}

impl CarState {
    /// The car's slip angle in radians as the physics reads it (0x59FA20 →
    /// 0x591DA0): heading minus direction of travel, zero unless moving
    /// forwards faster than 1 m/s.
    pub(super) fn body_slip(&self) -> f32 {
        if self.speed() <= 1.0 {
            return 0.0;
        }
        self.slip_degrees().to_radians()
    }

    /// Per-frame brake inputs and grip per wheel (0x5A4D90) and the yaw
    /// limit (0x5A49B0).
    ///
    /// `wet` is the rain at the car (0x5ED5E0 → 0x61E390): the base grip
    /// is × (1 − 0.025·wet) (0x5A4E75, 0x7A19EC = 0.975).
    pub(super) fn frame_setup(&mut self, h: &Handling, pedals: Controls, wet: f32, dt: f32) {
        let mph = self.speed() * MPH;
        let down = self.wheels_down();
        let slip = self.body_slip();
        // Grip base (mover +0x6C = 1), ×0.85 with no wheel down (not in
        // drift events).
        let base = if down == 0 && !h.race.drift() { 0.85 } else { 1.0 };
        let base = base * (1.0 - wet * (1.0 - 0.975));
        // The rear-grip bonus fades in at 1/s after the handbrake is let go
        // and drops to half at 20/s while it is held (mover +0x70).
        self.handbrake_ramp = if pedals.handbrake < 0.5 {
            (self.handbrake_ramp + dt).min(1.0)
        } else {
            (self.handbrake_ramp - dt * 20.0).max(0.5)
        };
        let forward_z = (self.orientation * Vec3::X).z;
        for i in 0..4 {
            let burnout = pedals.throttle > 0.8 && pedals.brake > 0.5 && mph < 10.0 && i >= 2;
            self.wheels[i].foot_brake = if burnout {
                mph * 0.05
            } else {
                let share = if i < 2 { 1.0 - h.brake_balance } else { h.brake_balance };
                2.0 * share * pedals.brake
            };
            let mut hand = HANDBRAKE_WHEELS[i] * pedals.handbrake;
            if hand > 0.2 && (down == 0 || (slip.abs() > 0.3 && mph < 80.0)) {
                hand += 0.5;
            }
            self.wheels[i].handbrake = hand;
            // The rear sliding bonus is off in drift and URL events.
            self.wheels[i].grip = if i < 2 || h.race.drift() || h.race.url() {
                base
            } else {
                let bonus = self.rear_grip_bonus(h, slip, hand, down, forward_z);
                ((bonus - 1.0) * self.handbrake_ramp + 1.0) * base
            };
        }
        // The drag factor of the surfaces under the wheels that are down,
        // averaged; 1 with none down (0x5A49B0 → aero +0x34).
        self.surface_drag = if down == 0 || self.crash {
            1.0
        } else {
            self.wheels.iter().filter(|w| w.on_ground).map(|w| surface(w.surface).drag).sum::<f32>() / down as f32
        };
        let lz = self.angular_momentum.z;
        self.yaw_limit = if down == 0 {
            h.yaw_limit_airborne
        } else if h.yaw_limit_ground != 0.0 {
            h.yaw_limit_ground
        } else if slip * lz < 0.0 && slip.abs() > 0.1 {
            20.0
        } else {
            0.0
        };
        if self.hold.is_some() {
            // Held (mover +0x84): the yaw-torque limit is 1e−14
            // (0x5A4C53, 0x283424DC), and the undriven wheels lose all grip
            // with the gas at exactly 1 (0x5A5124..0x5A5198).
            self.yaw_limit = f32::from_bits(0x2834_24DC);
            let split = h.gearbox.rear_split;
            for (i, w) in self.wheels.iter_mut().enumerate() {
                let undriven = if split < 0.2 { i >= 2 } else { split > 0.8 && i < 2 };
                if undriven && pedals.throttle == 1.0 {
                    w.grip *= 0.0;
                }
            }
        }
    }

    /// The drift events' assist (0x5AC270), at the end of each force pass.
    /// With the slip angle (heading less direction of travel, no speed
    /// gate) turning the same way as the yaw rate and two or more wheels
    /// down, a yaw torque k·g(|slip|)·ω_z about the car's up axis, k by
    /// layout and difficulty (0x7FC2D0, all negative: it damps the spin)
    /// and g the player's graph (0x86B3C8). The rear wheels' grip factor
    /// becomes c(w·|ω_z| + (1 − w)·|slip|)·s, c the curve 0x7FC910, w and s
    /// by layout (0x7FC2DC, 0x7FC2E0); it applies from the next pass.
    pub(super) fn drift_assist(&mut self, h: &Handling, up: Vec3, down: usize) -> Vec3 {
        let layout = crate::race::layout(h);
        let v = self.velocity;
        let travel = if v.x == 0.0 && v.y == 0.0 { 0.0 } else { v.y.atan2(v.x) };
        let pi = std::f32::consts::PI;
        let slip = (self.heading() - travel + pi).rem_euclid(std::f32::consts::TAU) - pi;
        let yaw_rate = self.spin.z;
        let mut torque = Vec3::ZERO;
        if slip * yaw_rate > 0.0 && down >= 2 {
            let k = DRIFT_ASSIST_DAMPING[layout][h.difficulty.min(2)];
            torque = up * (graph(&DRIFT_ASSIST_SLIP, slip.abs()) * k * yaw_rate);
        }
        let [w, scale] = DRIFT_ASSIST_REAR[layout];
        let rear = sample(&DRIFT_REAR_GRIP, 0.0, 1.0, yaw_rate.abs() * w + (1.0 - w) * slip.abs()) * scale;
        self.wheels[2].assist_grip = rear;
        self.wheels[3].assist_grip = rear;
        torque
    }

    /// After a well-timed shift (drivetrain +0x68 > 0) every wheel grips
    /// more: × 1 + min(1, bonus / k), k by drivetrain layout and the car's
    /// performance class (0x5B6310, table 0x7FC2A8, rows 0x144 apart).
    pub(super) fn shift_bonus_grip(&mut self, h: &Handling, drivetrain: &Drivetrain) {
        if drivetrain.shift_bonus <= 0.0 {
            return;
        }
        let k = SHIFT_BONUS_GRIP[crate::race::layout(h)][h.performance_class.min(3)];
        let factor = (drivetrain.shift_bonus / k).min(1.0) + 1.0;
        for wheel in &mut self.wheels {
            wheel.grip *= factor;
        }
    }

    /// The launch wheel hop (0x5B6310, from the drive mover each frame,
    /// 0x5B734F, after the shift bonus; physics round 14, verified in the
    /// original's logs). It runs for driver types 1–3 (car +0x24: the
    /// player is 1, so free roam too) going forward (car +0x42C > 0), not
    /// ghosted (0x5EBFB0), outside URL events (0x89E7E2), offline and with
    /// the race on (status 4; the clone: not held on the line). Off, it
    /// starts under 15 mph when a wheel spins: the largest
    /// max(0, min(load/3 · slip, slip)) (+0x164, +0x118) over 0.5 m/s gives
    /// g = graph 0x7FC924 of it, the grip +0x474 = g / k (k the shift bonus
    /// table's, 0x7FC2A8) and the count trunc((1.5 − g)·6 + the wheel's
    /// index & 1). On, with the heading within 0.3 rad of the travel
    /// (0x591DA0, 0 under 1 m/s; past it the hop ends): while a wheel's
    /// slip is 2.5 m/s or more the count drops by one every 0.5 s, and
    /// after 1 s with none it ends. While on, FL and RR keep
    /// 1 − (1 − grip)·0.35 of their grip (+0x180) on an even count and
    /// 1 − (1 − grip)·0.65 on an odd one, FR and RL the other way round.
    pub(super) fn wheel_hop(&mut self, h: &Handling, dt: f32) {
        let fwd = self.forward();
        // Every speed test of 0x5B6310 reads the car's exported
        // forward speed (car +0x42C at 0x5B633A; the physics view's copy
        // +0x3EC at 0x5B64F7 and 0x5B660C), not a fresh v·forward.
        let forward_speed = self.exported_forward_speed();
        if self.held || h.race.url() || self.ghost_timer > 0.0 || forward_speed <= 0.0 {
            return;
        }
        if self.hop == 0 {
            if forward_speed * 2.237 < 15.0 {
                let (mut most, mut wheel) = (0.0f32, 0usize);
                for (i, w) in self.wheels.iter().enumerate() {
                    let v = (self.hop_loads[i] * (1.0 / 3.0) * w.slip).min(w.slip);
                    if most < v {
                        most = v;
                        wheel = i;
                    }
                }
                if most > 0.5 {
                    let g = graph(&HOP_GRIP, most);
                    let k = SHIFT_BONUS_GRIP[crate::race::layout(h)][h.performance_class.min(3)];
                    self.hop_grip = 1.0 / k * g;
                    self.hop = ((1.5 - g) * 6.0 + (wheel & 1) as f32) as i32;
                    self.hop_timer = 0.5;
                    self.hop_calm = 0.0;
                }
            }
        } else {
            // 0x591DA0: the heading (car +0x40C) less the direction of
            // travel, in (−π, π]; 0 under 1 m/s.
            let err = if forward_speed >= 1.0 {
                let d = fwd.y.atan2(fwd.x) - self.velocity.y.atan2(self.velocity.x);
                (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
            } else {
                0.0
            };
            if err.abs() <= 0.3 {
                if self.wheels.iter().any(|w| w.slip >= 2.5) {
                    self.hop_calm = 0.0;
                    self.hop_timer -= dt;
                    if self.hop_timer < 0.0 {
                        self.hop -= 1;
                        self.hop_timer = 0.5;
                    }
                } else {
                    self.hop_calm += dt;
                    if self.hop_calm > 1.0 {
                        self.hop = 0;
                    }
                }
            } else {
                self.hop = 0;
            }
        }
        if self.hop != 0 {
            let even = self.hop % 2 == 0;
            for (i, w) in self.wheels.iter_mut().enumerate() {
                // FL and RR: 0.35 (0x7FC0FC) on an even count; FR and RL on
                // an odd one (C's remainder: −1 counts as not 1).
                let low = if i == 0 || i == 3 { even } else { self.hop % 2 == 1 };
                let f = if low { HOP_SIDE } else { 1.0 - HOP_SIDE };
                w.grip *= 1.0 - (1.0 - self.hop_grip) * f;
            }
        }
    }

    /// Rear grip while sliding (0x5990D0): 1 + |forward.z| on a slope, plus
    /// up to 0.35 more growing with slip angle and speed, from bases that
    /// are mostly there for the two seconds after a collision (graph
    /// 0x802570). None while the handbrake is held under 20° of slip.
    pub(super) fn rear_grip_bonus(&self, h: &Handling, slip: f32, hand: f32, down: usize, forward_z: f32) -> f32 {
        let after_crash = if self.since_collision < 4.0 {
            graph(&[(0.0, 0.0), (0.4, 1.0), (1.0, 1.0), (2.0, 0.0)], self.since_collision)
        } else {
            0.0
        };
        let [a, b] = h.drift_grip;
        let base = if down == 0 { a + b } else { b * after_crash + a };
        let plain = forward_z.abs() + 1.0;
        if hand > 0.5 && slip.abs() < 20f32.to_radians() {
            return plain;
        }
        plain + (base * (self.velocity.length() / 30.0) * slip.abs()).min(0.35)
    }

    /// One physics sub-step: the force pass (0x5B0530) and its integration.
    pub(super) fn substep(
        &mut self,
        car: &Car,
        world: &World,
        drivetrain: &mut Drivetrain,
        controls: Controls,
        _player: bool,
        holding: bool,
        dt: f32,
    ) {
        self.debug_substep('A', car, drivetrain, dt);
        let h = &car.car_type.handling;
        let table = &car.car_type.wheels;
        // The forces this sub-step integrates: the pass the previous
        // sub-step's post-step left (0x5B04E0 → vt+8 = 0x5B0530), taken
        // before this frame's input, brake, steering, gear and grip
        // set-up (0x5B72E0 runs those before its sub-step loop and no
        // force pass in between). A car with none, or moved since (a
        // contact's refresh runs the post-step on the state after the
        // impulse, 0x593710 → 0x5B04E0; a teleport), takes a pass on the
        // state as it stands; the tidy-up before it clears the crank's
        // reaction (0x5AA770).
        let fingerprint = self.force_fingerprint();
        let (force, torque) = match self.forces.take().filter(|f| f.at == fingerprint) {
            Some(f) => (f.force, f.torque),
            None => {
                drivetrain.reaction = 0.0;
                self.force_pass(car, world, drivetrain, dt)
            }
        };
        // The first sub-step after a DUMMY_SNAPSHOT (`snapshot_take`).
        let snapped = self.snapshot_cleared;
        self.wheel_spin(h, table, drivetrain, snapped, dt);

        let rot = self.body_rows();
        let (fwd, up) = (rot * Vec3::X, rot * Vec3::Z);
        let com_local = Vec3::from(h.centre_of_mass);
        let com = self.position + rot * com_local;
        let pose = SubstepPose { rot, fwd, up, com, com_local };
        let (speed_now, spin_now) = (self.velocity.length(), self.spin.length());
        let rot = self.integrate(h, &pose, force, torque, speed_now, spin_now, holding, dt);

        // The components' updates (0x5B04E0's list, vt+0x1C): the engine's
        // and the clutch's tidy-up (0x5AA770, 0x5A1410) on the input shaft
        // from the wheels as they stand.
        self.debug_substep('B', car, drivetrain, dt);
        let spins: [f32; 4] = std::array::from_fn(|i| self.wheels[i].spin);
        drivetrain.post(h, drivetrain.to_crank(&h.gearbox, spins), dt);
        self.rest_test(h, drivetrain, controls, rot);
        // Then the force pass for the next sub-step (vt+8), its
        // suspensions measured over this sub-step.
        self.last_sub = dt;
        let (force, torque) = self.force_pass(car, world, drivetrain, dt);
        self.forces = Some(StoredForces { force, torque, at: self.force_fingerprint() });
    }

    /// The state a stored force pass belongs to.
    fn force_fingerprint(&self) -> (Vec3, Quat, Vec3, Vec3) {
        (self.position, self.orientation, self.velocity, self.angular_momentum)
    }

    /// The force pass (0x5B0530, with the bump stops of 0x5ABB10 before
    /// it): the suspensions, the tyres, the rolling constraint and the
    /// friction circles, the drivetrain's coupling, and the body's force
    /// and torque sums the next integration reads. `dt` is the sub-step
    /// the pass follows (0x7FC800: the hydraulics and the held spring).
    fn force_pass(&mut self, car: &Car, world: &World, drivetrain: &mut Drivetrain, dt: f32) -> (Vec3, Vec3) {
        let h = &car.car_type.handling;
        let table = &car.car_type.wheels;
        // 0x5B0530's opening: the held car on the dyno pulls at 1.5 power.
        if self.aero_off && self.dyno_boost {
            drivetrain.power = 1.5;
        }

        let rot = self.body_rows();
        let (fwd, up) = (rot * Vec3::X, rot * Vec3::Z);
        let com_local = Vec3::from(h.centre_of_mass);
        let com = self.position + rot * com_local;
        let pose = SubstepPose { rot, fwd, up, com, com_local };

        let (compressions, contacts) = self.wheel_contacts(world, h, table, &pose);
        let bump_visited = self.bump_stops(h, table, drivetrain, &pose, &compressions, &contacts);
        let forward_speed = self.exported_forward_speed();

        // Forces gathered for this step, each through the game's caps
        // (0x59BB10). Gravity is put in by the previous step's tidy-up
        // (0x59C600, rigid body +0x160 = 9.8).
        let mut force = Vec3::ZERO;
        let mut torque = Vec3::ZERO;
        self.body_forces(h, drivetrain, &pose, &mut force, &mut torque);

        let split = h.gearbox.rear_split;
        let down = contacts.iter().filter(|c| c.is_some()).count();
        let mut tyres = self.tyre_forces(
            h,
            table,
            drivetrain,
            &pose,
            &compressions,
            &contacts,
            bump_visited,
            forward_speed,
        );
        self.rolling_constraint(h, table, drivetrain, &pose, force, &mut tyres);
        self.friction_circle(h, &tyres, &contacts, &pose, forward_speed, &mut force, &mut torque);
        self.engine_settle(h, drivetrain, split);

        // Spin damping with fewer than two wheels down (0x5A5B70, graph
        // 0x7FC8F8): −g(|ω|)·m·ω below 0.5 rad/s.
        let spin_now = self.spin.length();
        if down < 2 && spin_now < 0.5 {
            let g = graph(&[(0.0, 0.0), (0.4, 0.0), (2.0, 20.0)], spin_now);
            torque += -self.spin * g * h.mass;
        }

        if h.race.drift() {
            torque += self.drift_assist(h, up, down);
        }

        self.hydraulics_pass(h, table, &pose, &compressions, &contacts, dt);
        // The force pass ends with the held mover's spring (0x5B068C).
        self.hold_spring(h, table, pose.com, pose.rot, dt);
        (force, torque)
    }

    /// Suspension (0x5A87A0). Compression is measured along the car's up
    /// from where the wheel rests to the ground (0x59D260), plus the
    /// radius; it is clamped to the travel, and a wheel past full droop
    /// has no contact.
    fn wheel_contacts(
        &mut self,
        world: &World,
        h: &Handling,
        table: &[crate::car::Wheel; 4],
        pose: &SubstepPose,
    ) -> ([f32; 4], [Option<(Vec3, Vec3)>; 4]) {
        let SubstepPose { rot, up, .. } = *pose;
        let mut compressions = [0.0f32; 4];
        let mut contacts: [Option<(Vec3, Vec3)>; 4] = [None; 4];
        for i in 0..4 {
            let w = &table[i];
            let s = &h.suspension[i / 2];
            let rest = self.position + rot * self.mount(w.position, i);
            let (x, n, surface) = self.wheel_compression(world, rest, up, w.radius);
            // Surface 1 when there is no ground (0x5E72F0).
            // Drag and street-X events drive every wheel on surface 1
            // (0x59CBA0).
            self.wheels[i].surface = if h.race.drag() || h.race.street_x() { 1 } else { surface };
            if x > s.max_compression {
                compressions[i] = s.max_compression;
            } else if x < s.max_extension {
                compressions[i] = s.max_extension;
                continue;
            } else {
                compressions[i] = x;
            }
            let n = n.unwrap();
            let hub = rest + up * compressions[i];
            contacts[i] = Some((hub - n * w.radius, n));
        }
        (compressions, contacts)
    }

    /// Bump stops (0x5ABB10, from the mover's update 0x5B04E0 at the end
    /// of the previous sub-step, after the suspensions measured the new
    /// pose and after the rest test): unless that test held the car, a
    /// suspension at full compression (+0x1C equal to the record's +0x14)
    /// takes 0x59E1B0's impulse at its hub (+0x30: the rest point plus
    /// the compression along up), 0.225 × mass × the hub's closing speed
    /// along the wheel's ground normal (+0x150) when it closes, through
    /// 0x59BC70 (momentum capped at 1000, its moment at 10); then the
    /// velocities from the momenta (0x5A0D90).
    /// Returns whether a suspension was visited (0x5ABB10's zero-time
    /// refresh request).
    fn bump_stops(
        &mut self,
        h: &Handling,
        table: &[crate::car::Wheel; 4],
        drivetrain: &mut Drivetrain,
        pose: &SubstepPose,
        compressions: &[f32; 4],
        contacts: &[Option<(Vec3, Vec3)>; 4],
    ) -> bool {
        let SubstepPose { rot, up, com, .. } = *pose;
        let mut bump_visited = false;
        if self.rest == 0 {
            let mut bump_contacts = [None; 4];
            for i in 0..4 {
                let s = &h.suspension[i / 2];
                let Some((_, n)) = contacts[i] else { continue };
                if compressions[i] < s.max_compression {
                    continue;
                }
                let arm = self.position + rot * self.mount(table[i].position, i) + up * compressions[i] - com;
                bump_contacts[i] = Some((arm, n));
            }
            let (linear, angular, hit) = bump_stop_impulses(self.velocity, self.spin, h.mass, bump_contacts);
            bump_visited = hit;
            // Native keeps both velocity caches fixed across all wheels,
            // then 0x5A0D90 refreshes them from the accumulated momenta.
            self.velocity += linear / h.mass;
            self.angular_momentum += angular;
            if hit {
                self.spin_from_momentum(h);
                // Engine/clutch portion of 0x5A0D90. No integration time
                // elapses, but the discrete clutch state still advances.
                let spins = std::array::from_fn(|i| self.wheels[i].spin);
                drivetrain.post(h, drivetrain.to_crank(&h.gearbox, spins), 0.0);
            }
        }
        bump_visited
    }

    /// Gravity and the aerodynamics, into the step's force and torque;
    /// returns |v| as the aerodynamics read it.
    fn body_forces(
        &self,
        h: &Handling,
        drivetrain: &Drivetrain,
        pose: &SubstepPose,
        force: &mut Vec3,
        torque: &mut Vec3,
    ) -> f32 {
        let SubstepPose { fwd, up, .. } = *pose;
        // A car the last rest test held (class 2) has body +0x160 = 0: no
        // gravity this step (0x5A5E30).
        let gravity = if self.rest == 2 {
            0.0
        } else if self.crash {
            CRASH_GRAVITY
        } else {
            GRAVITY
        };
        apply(force, torque, Vec3::new(0.0, 0.0, -h.mass * gravity), Vec3::ZERO);
        // Crash mode turns the aero factor (+0x30) off: no drag or downforce.
        let aero = if self.crash || self.aero_off { 0.0 } else { 1.0 };

        // Aerodynamics (0x59E630). The mover hands it, each frame
        // (0x5ABC00), the engine's throttle t (aero +0x10) and the wheels
        // down / 4 (aero +0x14, from car +0x424). Drag (2 − t)·c·|v|·v
        // (0x7FBED0 = 2), at the centre of mass, or with two or more
        // wheels down 0.1·(1 − t) m below it (world z, 0x7FBED4 = −0.1).
        // Downforce: shape = max(v̂·forward, 0)² (at least 0.4) ×
        // max(up.z, 0) (1 with two or more wheels down) × |v|; with a
        // wheel down the front record value 1 m ahead of the centre of
        // mass and the rear one 1 m behind (0x59E852); with none, 40% and
        // 41% of their sum there.
        let speed_now = self.velocity.length();
        let t = drivetrain.throttle;
        let down_share = self.aero_down as f32 * 0.25;
        let drag_arm = if down_share >= 0.5 { Vec3::new(0.0, 0.0, -0.1 * (1.0 - t)) } else { Vec3::ZERO };
        apply(
            force,
            torque,
            -self.velocity
                * (speed_now / self.catch_up)
                * h.drag
                * (2.0 - t)
                * self.surface_drag
                * aero
                * (1.0 - self.draft),
            drag_arm,
        );
        if speed_now > 1e-4 && aero > 0.0 {
            let along = (self.velocity / speed_now).dot(fwd).max(0.0);
            // In drift events along⁴ with no floor.
            let shape = if h.race.drift() { along.powi(4) } else { (along * along).max(0.4) };
            let tilt = if down_share >= 0.5 { 1.0 } else { up.z.max(0.0) };
            let k = shape * tilt * speed_now / self.catch_up;
            let (front, rear) = if down_share == 0.0 {
                let total = (h.downforce[0] + h.downforce[1]) * k;
                (total * 0.4, total * 0.41)
            } else {
                (h.downforce[0] * k, h.downforce[1] * k)
            };
            apply(force, torque, -up * front, fwd);
            apply(force, torque, -up * rear, -fwd);
        }
        speed_now
    }

    /// The suspension loads and tyre forces of the four wheels (0x5A8290).
    #[allow(clippy::too_many_arguments)]
    fn tyre_forces(
        &mut self,
        h: &Handling,
        table: &[crate::car::Wheel; 4],
        drivetrain: &mut Drivetrain,
        pose: &SubstepPose,
        compressions: &[f32; 4],
        contacts: &[Option<(Vec3, Vec3)>; 4],
        bump_visited: bool,
        forward_speed: f32,
    ) -> [Option<TyreContact>; 4] {
        let SubstepPose { rot, up, com_local, .. } = *pose;
        // Static load share per wheel for the suspension preload (0x5AB9A0):
        // the weight plus speed × downforce × record +0x37C, split by where
        // the centre of mass sits between the axles.
        let front_x = table[0].position.x;
        let rear_x = table[3].position.x;
        let rear_share = 0.5 * (front_x - com_local.x) / (front_x - rear_x);
        let supported =
            (h.downforce[0] + h.downforce[1]) * self.speed().max(0.0) * h.preload_downforce + h.mass * GRAVITY;

        // The clutch's torque to the wheels (0x594CC0 → 0x594A40), split by
        // the differential.
        let shares: [f32; 4] = std::array::from_fn(|i| self.differential_share(i));
        let drive = match drivetrain.clutch_torque(h) {
            Some(t) => drivetrain.distribute(&h.gearbox, t, self.drivetrain_wheels_down(), shares),
            None => [0.0; 4],
        };
        let angles: [f32; 4] = std::array::from_fn(|i| if i < 2 { self.front_wheel_angle(table, i) } else { 0.0 });

        // Tyres (0x5A8290), with the lock test (0x59D410).
        let rest_points: [Vec3; 4] = std::array::from_fn(|i| self.position + rot * self.mount(table[i].position, i));
        // 0x5A89EB..5A8A78 caches the rigid body's velocity at the hub
        // (suspension +0x30), consumed by 0x5A8325. Tyre forces still act
        // at the road contact below; that is a different lever arm.
        let hub_arms: [Vec3; 4] =
            std::array::from_fn(|i| wheel_hub_arm(rot, self.mount(table[i].position, i), com_local, compressions[i]));
        let mut tyres: [Option<TyreContact>; 4] = [None; 4];
        for i in 0..4 {
            let w = &table[i];
            let s = &h.suspension[i / 2];
            let tyre = &h.tyres[i / 2];
            let (wf, wl) = {
                let (sa, ca) = angles[i].sin_cos();
                (rot * Vec3::new(ca, sa, 0.0), rot * Vec3::new(-sa, ca, 0.0))
            };
            let load = self.wheel_load(i, s, compressions, contacts, bump_visited, supported, rear_share, wl);
            let contact = contacts[i].map_or(rest_points[i], |c| c.0);
            tyres[i] = Some(self.tyre_force(
                i,
                h,
                w,
                tyre,
                load,
                drive[i],
                contact,
                hub_arms[i],
                wf,
                wl,
                up,
                forward_speed,
            ));
        }
        tyres
    }

    /// One wheel's suspension state and load: spring, dampers, preload and
    /// anti-roll.
    #[allow(clippy::too_many_arguments)]
    fn wheel_load(
        &mut self,
        i: usize,
        s: &crate::handling::Suspension,
        compressions: &[f32; 4],
        contacts: &[Option<(Vec3, Vec3)>; 4],
        bump_visited: bool,
        supported: f32,
        rear_share: f32,
        wl: Vec3,
    ) -> f32 {
        // The anti-roll bar reads the other side's suspension +0x1C as
        // it stands (0x5A87A0 via +0x18): the mover's component list
        // updates the four wheels' suspensions in turn (0x5B04E0, read live), so the
        // first pass's left wheels see the right one's old compression
        // and right wheels the left one's new one. A zero refresh after
        // rest/bump repeats the loads with current neighbours on both sides.
        let other_x = self.suspension_neighbour_compression(i, compressions, bump_visited);
        let state = &mut self.wheels[i];
        let x = compressions[i];
        // The rate (+0x20) only over 0.0001 s (0x5A8877): the original
        // steps zero-length frames too (logged under Wine, round 13).
        // The original measures at the end of each sub-step, so the
        // rate is over the sub-step that moved the car there, not the
        // one about to run (they differ when a frame's sub-step count
        // changes: 1/120 s frames take two, 0.0089 s ones three; round
        // 15, Corolla handbrake log `c2`).
        if self.last_sub > 0.0001 {
            state.compression_rate = (x - state.compression) / self.last_sub;
        }
        state.compression = x;
        state.on_ground = contacts[i].is_some();

        // Spring, dampers, preload, anti-roll; never pulls. The load
        // falls off with the wheel's camber against the ground
        // (0x59D170): double-cross projection, including nonunit wheel axes.
        let load = match contacts[i] {
            Some((_, n)) => {
                let preload = supported * if i < 2 { 0.5 - rear_share } else { rear_share };
                let spring = state.suspension_load(s, other_x, preload);
                tyre_camber_load(spring, wl, n)
            }
            None => 0.0,
        };
        state.load = load;
        load
    }

    /// One wheel's tyre force (0x5A8290) with the lock test (0x59D410).
    #[allow(clippy::too_many_arguments)]
    fn tyre_force(
        &mut self,
        i: usize,
        h: &Handling,
        w: &crate::car::Wheel,
        tyre: &crate::handling::Tyre,
        load: f32,
        drive: f32,
        contact: Vec3,
        hub_arm: Vec3,
        wf: Vec3,
        wl: Vec3,
        up: Vec3,
        forward_speed: f32,
    ) -> TyreContact {
        let state = &mut self.wheels[i];
        // Drive torque, then the brakes against the spin.
        let mut torque = if state.locked { 0.0 } else { drive };
        let foot = h.brake * state.foot_brake * 4.0;
        let hand = h.handbrake * state.handbrake * 10.0;
        if !state.locked {
            let against = if state.spin > 0.0 { -1.0 } else { 1.0 };
            torque += against * foot + against * hand;
        }

        let v = wheel_hub_velocity(self.velocity, self.spin, hub_arm);
        let (vx, vy) = (v.dot(wf), v.dot(wl));
        // Slip angle of the contact's travel, measured from forward or,
        // when rolling backwards, from backward.
        // The game's table arctangent (0x5A835D → 0x43C570), so slip
        // angles under 1/256 read 0 (no side force) and the rest come in
        // 0.0031 rad steps: a small yaw left by the clutch bite is not
        // taken out (round 14).
        let mut angle = atan16(vx, vy) as f32 * 9.587_38e-5;
        if angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        if vx < 0.0 {
            angle += std::f32::consts::PI;
        }
        if angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        let slip = w.radius * state.spin - vx;
        state.slip = slip;
        state.lateral_slip = vy;
        // ABS: braking with slip beyond -0.5 m/s scales the torque by
        // 0.5 / -slip, unless the handbrake is on.
        if slip < -SLIDE_SLIP && torque < 0.0 && hand == 0.0 {
            torque *= SLIDE_SLIP / -slip;
        }

        let surface = surface(state.surface);
        let mut mu = surface.slide * tyre.sliding * state.grip;
        if self.stability_off && i >= 2 {
            mu *= launch_grip(forward_speed, state.locked, state.spin);
        }
        let len = (slip * slip + vy * vy).sqrt();
        let mut sliding = false;
        let (mut fx, mut fy) = (0.0, 0.0);
        if len != 0.0 && (vx != 0.0 || vy != 0.0) {
            // Lock test: brakes beat what the road and the spin can
            // turn the wheel with. Just past it the wheel stops without
            // locking.
            let road = vx.abs() * load / (vx * vx + vy * vy).sqrt() * mu;
            let brakes = (foot + hand) * 1.2;
            if brakes <= w.radius * road + state.spin.abs() * 0.1 {
                state.locked = false;
            } else {
                state.locked = brakes > 1.0;
                state.spin = 0.0;
            }
            if slip.abs() > SLIDE_SLIP || angle.abs() > tyre.slip_angle_scale || state.locked {
                sliding = true;
                (fx, fy) = sliding_forces(slip, vy, load, mu, forward_speed);
            }
        }
        if !sliding {
            fx = torque / w.radius;
            let side = if angle <= 0.0 { 1.0 } else { -1.0 };
            let dir = if vx <= 0.0 { -1.0 } else { 1.0 };
            fy = lateral_force(tyre, load, angle.abs()) * side * dir;
        }
        state.sliding = sliding;
        TyreContact {
            contact,
            wf,
            wl,
            up,
            fx,
            fy,
            load,
            torque,
            gripping: !sliding && !state.locked,
            locked: state.locked,
            sliding,
            vx,
            angle,
        }
    }

    /// The rolling constraint (0x5ABE00): five passes. Each starts from
    /// the car's acceleration in its plane (linear plus yaw) under all
    /// the forces so far, a; each rolling wheel's drive force then moves
    /// by I/r × (its own spin-up at the contact, r(T − r·Fx)/I, less a
    /// along the wheel) / r. With the clutch locked each pass ends with
    /// the crank torque that keeps engine and gearbox turning together
    /// (0x594DB0), sent to the wheels like the engine's own.
    fn rolling_constraint(
        &self,
        h: &Handling,
        table: &[crate::car::Wheel; 4],
        drivetrain: &mut Drivetrain,
        pose: &SubstepPose,
        force: Vec3,
        tyres: &mut [Option<TyreContact>; 4],
    ) {
        let SubstepPose { rot, fwd, com, .. } = *pose;
        let to_plane = |v: Vec3| glam::Vec2::new(v.dot(fwd), v.dot(rot * Vec3::Y));
        let external = to_plane(force);
        let shares: [f32; 4] = std::array::from_fn(|i| self.differential_share(i));
        for _ in 0..5 {
            let (mut total, mut yaw) = (external, 0.0);
            for t in tyres.iter().flatten() {
                let f = to_plane(t.wf * t.fx + t.wl * t.fy);
                total += f;
                yaw += to_plane(t.contact - com).perp_dot(f);
            }
            let (a, alpha) = (total / h.mass, yaw / inertia_of(h, self.ground_inertia).z);
            for (i, t) in tyres.iter_mut().enumerate() {
                let Some(t) = t else { continue };
                if !t.gripping {
                    continue;
                }
                let p = to_plane(t.contact - com);
                // The game takes the contact's acceleration along the
                // wheel's steer matrix row (+0xB0, +0xC0) = (cos δ, −sin δ),
                // not along its forward column (cos δ, sin δ): a steered
                // wheel reads the car's sideways acceleration mirrored
                // (0x5ABE00), so in a turn the front tyres push forward
                // instead of dragging.
                let wf = to_plane(t.wf);
                let along = (a + glam::Vec2::new(-alpha * p.y, alpha * p.x)).dot(glam::Vec2::new(wf.x, -wf.y));
                let r = table[i].radius;
                t.fx += (r * (t.torque - r * t.fx) / WHEEL_INERTIA - along) / r * WHEEL_INERTIA;
            }
            let spin_up: [f32; 4] =
                std::array::from_fn(|i| tyres[i].map_or(0.0, |t| (t.torque - table[i].radius * t.fx) / WHEEL_INERTIA));
            let input_accel = drivetrain.to_crank(&h.gearbox, spin_up);
            if let Some(extra) = drivetrain.coupling_torque(h, input_accel, WHEEL_INERTIA) {
                let add = drivetrain.distribute(&h.gearbox, extra, self.drivetrain_wheels_down(), shares);
                for (i, t) in tyres.iter_mut().enumerate() {
                    if let Some(t) = t
                        && !t.locked
                    {
                        t.torque += add[i];
                    }
                }
            }
        }
    }

    /// Friction circle (0x59D610) and the wheel's averaged net torque for
    /// its spin (0x59D360); the tyre forces go into the step's sums.
    #[allow(clippy::too_many_arguments)]
    fn friction_circle(
        &mut self,
        h: &Handling,
        tyres: &[Option<TyreContact>; 4],
        contacts: &[Option<(Vec3, Vec3)>; 4],
        pose: &SubstepPose,
        forward_speed: f32,
        force: &mut Vec3,
        torque: &mut Vec3,
    ) {
        let com = pose.com;
        for i in 0..4 {
            let Some(t) = tyres[i] else { continue };
            let tyre = &h.tyres[i / 2];
            let state = &mut self.wheels[i];
            let (mut fx, mut fy) = (t.fx, t.fy);
            let boosted = t.torque > 0.0 && !t.locked;
            if boosted {
                fx *= DRIVE_BOOST;
            }
            let mut limit =
                surface(state.surface).grip * tyre.grip * state.assist_grip * self.catch_up * state.grip * t.load;
            if self.stability_off && i >= 2 {
                limit *= launch_grip(forward_speed, t.locked, state.spin);
            }
            let len = (fx * fx + fy * fy).sqrt();
            state.circle = if limit > 0.001 { (len / limit).min(1.0) } else { 1.0 };
            if t.sliding || t.locked {
                state.circle = 1.0;
            }
            state.torque = t.torque;
            if len <= limit || len <= 0.001 {
                if boosted {
                    fx /= DRIVE_BOOST;
                }
            } else {
                // 0x59D610 scales the forces (+0x124, +0x128) and sets
                // +0x170 only; the sliding flag +0x160 stays as the tyre
                // force (0x5A8577 / 0x5A8616) left it.
                fx *= limit / len;
                fy *= limit / len;
            }
            // Cornering costs drive: above 1 m/s, sin(slip angle) × 15% of
            // the side force comes off the drive force.
            if t.vx.abs() > 1.0 {
                fx -= t.angle.sin() * fy * 0.15;
            }
            state.fx = fx;
            state.fy = fy;
            if contacts[i].is_some() {
                // Along the wheel's own axes, the load along the car's up
                // (0x7FC7FC = 1).
                let f = t.wf * fx + t.wl * fy + t.up * t.load;
                apply(force, torque, f, t.contact - com);
            }
        }
    }

    /// The engine settles against the gearbox (0x5AA840); spinning the
    /// driven wheels under power sets them to the engine (0x5A12E0).
    fn engine_settle(&mut self, h: &Handling, drivetrain: &mut Drivetrain, split: f32) {
        let g = &h.gearbox;
        let spins: [f32; 4] = std::array::from_fn(|i| self.wheels[i].spin);
        let input_w = drivetrain.to_crank(g, spins);
        let driven_slip = {
            let (mut sum, mut n) = (0.0, 0.0);
            for i in 0..4 {
                if (i >= 2 && split >= 0.001) || (i < 2 && split <= 0.999) {
                    sum += self.wheels[i].slip;
                    n += 1.0;
                }
            }
            sum / n
        };
        if let Some(crank_w) = drivetrain.settle(h, input_w, driven_slip, self.drivetrain_wheels_down()) {
            self.match_wheels(g, drivetrain, crank_w, input_w);
        }
    }

    /// Wheel spin (0x59D360, the wheel's derivative): the net torque
    /// averaged with the last step's, less rolling resistance 0.002·ω; a
    /// wheel that grips has its slip taken out within the step. It reads
    /// the stored pass's torque (+0x108), force (+0x124), slip (+0x118) and
    /// flags (+0x10C locked, +0x160 sliding) as the wheel holds them. Then
    /// the drivetrain's integration, the wheels' roll angles and the
    /// spinners.
    fn wheel_spin(
        &mut self,
        h: &Handling,
        table: &[crate::car::Wheel; 4],
        drivetrain: &mut Drivetrain,
        snapped: bool,
        dt: f32,
    ) {
        for i in 0..4 {
            let r = table[i].radius;
            let state = &mut self.wheels[i];
            // After a snapshot the derivative reads the drive torque +0x108
            // and force +0x124 the take zeroed: the running mean only halves.
            state.net_torque = if snapped {
                state.net_torque * 0.5
            } else {
                (state.torque - r * state.fx + state.net_torque) * 0.5
            };
            let mut spin_rate = (state.net_torque - 0.002 * surface(state.surface).roll * state.spin) / WHEEL_INERTIA;
            if snapped {
                // The take also cleared the lock and sliding flags (the slip
                // +0x118 stays): 0x59D360 takes the whole slip out of every
                // wheel within the sub-step (observed under Wine,
                // `substep-capture.txt`: a COROLLA's spinning rear wheel
                // 47.3 → 1.25 rad/s in the first sub-step, then spinning up
                // again).
                if state.locked {
                    spin_rate = 0.0;
                }
                spin_rate = gripping_wheel_spin_rate(spin_rate, state.slip, r, dt);
            } else if state.locked {
                spin_rate = 0.0;
            } else if !state.sliding {
                spin_rate = gripping_wheel_spin_rate(spin_rate, state.slip, r, dt);
            }
            state.spin += spin_rate * dt;
        }
        drivetrain.integrate(h, dt);
        for w in &mut self.wheels {
            w.roll = (w.roll + w.spin * dt) % std::f32::consts::TAU;
            // The spinners (0x5ECAD0, from 0x6012F7): the rate follows the
            // wheel's angular speed (+0x330 + 4i, v / r at 0x5F537C) at
            // 0.0035 / 0.0225 (0x7A5660, 0x7A565C) per second, the angle
            // integrates the new rate.
            w.spinner_rate += (w.spin - w.spinner_rate) * (0.0035 / 0.0225) * dt;
            w.spinner = (w.spinner + w.spinner_rate * dt) % std::f32::consts::TAU;
        }
    }

    /// The hydraulics (0x5A51C0, the force pass's last step), skipped
    /// with a live contact (car +0x3C → +0x18): the corners move and a
    /// bounce kicks the wheels through RigidBody_ApplyImpulse 0x59BC70
    /// (momentum capped at 1000, its moment at 10), each at the body's
    /// velocity before the kicks.
    fn hydraulics_pass(
        &mut self,
        h: &Handling,
        table: &[crate::car::Wheel; 4],
        pose: &SubstepPose,
        compressions: &[f32; 4],
        contacts: &[Option<(Vec3, Vec3)>; 4],
        dt: f32,
    ) {
        let SubstepPose { rot, up, com, .. } = *pose;
        if !self.in_contact() {
            let wheels: [crate::hydraulics::Wheel; 4] = std::array::from_fn(|i| crate::hydraulics::Wheel {
                compression: compressions[i],
                arm: self.position + rot * self.mount(table[i].position, i) + up * compressions[i] - com,
                normal: contacts[i].map_or(up, |c| c.1),
            });
            let (v, w) = (self.velocity, self.spin);
            let kicks = self.hydraulics.force_pass(&wheels, h.mass, &|r| v + w.cross(r), dt);
            for (j, arm) in &kicks {
                let j = j.clamp_length_max(1000.0);
                self.velocity += j / h.mass;
                self.angular_momentum += arm.cross(j).clamp_length_max(10.0);
            }
            if !kicks.is_empty() {
                self.spin_from_momentum(h);
            }
        }
    }

    /// The rigid body's own terms (0x59C1F0), explicit Euler (0x5AA490) and
    /// the tidy-up (0x59C600). Returns the new orientation's matrix.
    #[allow(clippy::too_many_arguments)]
    fn integrate(
        &mut self,
        h: &Handling,
        pose: &SubstepPose,
        mut force: Vec3,
        mut torque: Vec3,
        speed_now: f32,
        spin_now: f32,
        holding: bool,
        dt: f32,
    ) -> Mat3 {
        let SubstepPose { rot, fwd, com, com_local, .. } = *pose;
        // The rigid body's own terms (0x59C1F0). A top speed of 0 (the
        // performance ECUs 0x2D, 0x2E and 0x33 clear it) means no limiter:
        // the function skips the test when body +0x204 is 0.
        // A DUMMY_SNAPSHOT emptied the sums this integration reads
        // (`snapshot_take`): gravity (0x7FBEF0 × mass) alone.
        if std::mem::take(&mut self.snapshot_cleared) {
            force = Vec3::new(0.0, 0.0, -h.mass * GRAVITY);
            torque = Vec3::ZERO;
        }
        let speed_xy = self.velocity.truncate().length() * 2.2360249;
        let slope = fwd.z.abs().min(0.3);
        let grade = if fwd.z <= 0.0 { 1.0 + slope * 0.3333334 } else { 1.0 - slope * 0.3333334 };
        // Held, body +0x204 is 0 (0x5A4D0A): no limiter.
        if h.top_speed_mph != 0.0
            && self.hold.is_none()
            && grade * (h.top_speed_mph * self.catch_up + 1.9) < speed_xy
            && fwd.dot(force) > 0.0
        {
            // Over the top speed: forward drive becomes a little drag.
            let mut local = rot.transpose() * force;
            local.x = (-slope * 5.0).min(-0.5);
            force = rot * local;
        }
        let damping = if self.crash { 0.0 } else { sample(&ANGULAR_DAMPING, 0.0, 10.0, spin_now) };
        let mut dl = torque - self.angular_momentum * damping;
        let settle = (speed_now / 4.0).max(spin_now / 2.0);
        let rest_rate = if sample(&REST_DAMPING, 0.0, 1.0, settle) <= self.rest_damping { -10.0 } else { 0.1 };
        // The tidy-up's copy of the sums (+0x130) the held spring reads.
        if let Some(hold) = self.hold.as_mut() {
            hold.last_force = force;
        }
        let mut dv = force / h.mass;
        if holding {
            dl -= self.angular_momentum * (2.0 * self.rest_damping);
            dv -= self.velocity * (8.0 * self.rest_damping);
        }
        if self.yaw_limit != 0.0 {
            dl.z = dl.z.clamp(-self.yaw_limit, self.yaw_limit);
        }

        // Explicit Euler (0x5AA490), then the tidy-up (0x59C600). The rows
        // take a plain Euler step with the sub-step's starting spin,
        // row += h·(ω × row) (0x59C130), not a rotation.
        let new_com = com + self.velocity * dt;
        let stepped = super::rigid::euler_rows(rot, self.spin, dt);
        if !dv.is_finite() {
            dv = Vec3::ZERO;
        }
        self.velocity += dv * dt;
        self.angular_momentum += dl * dt;
        self.rest_damping = (self.rest_damping + rest_rate * dt).max(0.0);
        if self.velocity.length() > 200.0 && h.mass > 0.7 {
            self.velocity = self.velocity.clamp_length_max(200.0);
        }
        // ω from the rows as the Euler step left them (the tidy-up runs
        // inside 0x5AA490, before the mover's 0x5BB0E0), then the rows made
        // orthonormal again (0x5B7455–0x5B745D) and published.
        self.spin = super::integrate::spin_of(h, self.ground_inertia, stepped, self.angular_momentum);
        let rows = super::rigid::orthonormal(stepped);
        self.set_body_rows(rows);
        self.position = new_com - rows * com_local;
        // The rest test (0x5ABB10, in the same post-step 0x5B04E0) still
        // sees the un-cleaned rows.
        stepped
    }

    /// The rest test (0x5ABB10 → 0x5A5E30, from 0x5B04E0 after the
    /// components' updates; the bump stops that follow it are at the
    /// start of the next sub-step, above).
    /// The car's gear +0x434 (0x5992C0 at 0x599324), not the drivetrain's
    /// live one: it lags a frame, so the frame in which the clutch puts
    /// 1st in still holds a car at rest (velocity and wheel spins zeroed;
    /// observed under Wine, launch frame 4).
    fn rest_test(&mut self, h: &Handling, drivetrain: &mut Drivetrain, controls: Controls, rot: Mat3) {
        self.rest = self.rest_class(controls.throttle, self.frame_gear == NEUTRAL, rot);
        match self.rest {
            // The drive mover passes itself (0x5ABB10 → 0x5A5E30's fifth
            // argument), so its wheel spins (+0x28) go with the momenta:
            // × 0.95, or zeroed with a wheel down (car +0x424 > 0).
            1 => {
                self.velocity *= 0.95;
                self.angular_momentum *= 0.95;
                for w in &mut self.wheels {
                    w.spin *= 0.95;
                }
            }
            2 => {
                self.velocity = Vec3::ZERO;
                self.angular_momentum = Vec3::ZERO;
                if self.wheels_down() > 0 {
                    for w in &mut self.wheels {
                        w.spin = 0.0;
                    }
                }
            }
            _ => {}
        }
        if self.rest != 0 {
            // A held car runs the components' updates again (0x5ABB10 →
            // 0x5A0D90 with the sub-step 0x7FC800 = 0): the velocities from
            // the momenta, and the engine's and the clutch's tidy-up once
            // more, on the wheels the test just stopped (no heat or turbo
            // time). So the clutch state machine takes two steps per
            // sub-step at rest: Disengaging → Changing → Engaging in one
            // (round 15, 206 log `x2`: its at-rest cycle has a period of two
            // sub-steps, not three). The old bump test mistook the native
            // distance query's near-zero result for geometric overlap;
            // its strict separating-plane check now distinguishes them.
            // 0x5A0D90 → the tidy-up 0x59C600 on the rows as they stand.
            self.spin = super::integrate::spin_of(h, self.ground_inertia, rot, self.angular_momentum);
            let spins = std::array::from_fn(|i| self.wheels[i].spin);
            drivetrain.post(h, drivetrain.to_crank(&h.gearbox, spins), 0.0);
        }
    }
}
/// The car's pose at the start of a sub-step, shared by its stages.
#[derive(Clone, Copy)]
struct SubstepPose {
    rot: Mat3,
    fwd: Vec3,
    up: Vec3,
    com: Vec3,
    com_local: Vec3,
}

/// A force through the game's caps (0x59BB10) into the step's sums.
fn apply(force: &mut Vec3, torque: &mut Vec3, f: Vec3, arm: Vec3) {
    let f = f.clamp_length_max(1200.0);
    *force += f;
    *torque += arm.cross(f).clamp_length_max(100.0);
}

// 0x59D1C4..59D22B: do not simplify using unit-length axes. The native
// wheel frame includes trigonometric approximation scale (including at zero).
pub(super) fn tyre_camber_load(raw_load: f32, axle: Vec3, normal: Vec3) -> f32 {
    if !(raw_load > 0.0) {
        return 0.0;
    }
    let projected = axle.cross(normal).cross(axle);
    let dot = (projected.y * normal.y + projected.z * normal.z) + projected.x * normal.x;
    let factor = dot * 4.0 - 3.0;
    (if factor < 0.3 { 0.3 } else { factor }) * raw_load
}

// 0x5A8A1D / 0x5A8A2C: point velocity at the wheel hub, not the road contact.
pub(super) fn wheel_hub_arm(rotation: Mat3, mount: Vec3, centre_of_mass: Vec3, compression: f32) -> Vec3 {
    rotation * (mount - centre_of_mass + Vec3::Z * compression)
}

pub(super) fn wheel_hub_velocity(velocity: Vec3, spin: Vec3, hub_arm: Vec3) -> Vec3 {
    velocity + spin.cross(hub_arm)
}

// 0x59E1B0 reads cached velocities throughout 0x5ABB10's wheel loop.
// Return momenta; the caller refreshes velocity once after all visits.
pub(super) fn bump_stop_impulses(
    velocity: Vec3,
    spin: Vec3,
    mass: f32,
    contacts: [Option<(Vec3, Vec3)>; 4],
) -> (Vec3, Vec3, bool) {
    let mut linear = Vec3::ZERO;
    let mut angular = Vec3::ZERO;
    let mut visited = false;
    for (arm, normal) in contacts.into_iter().flatten() {
        visited = true;
        let closing = -normal.dot(velocity + spin.cross(arm));
        if closing < 0.0 {
            continue;
        }
        let impulse = (normal * closing * 0.225 * mass).clamp_length_max(1000.0);
        linear += impulse;
        angular += arm.cross(impulse).clamp_length_max(10.0);
    }
    (linear, angular, visited)
}
