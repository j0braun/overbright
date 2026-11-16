use super::*;

impl CarState {
    /// One frame of driving (0x5B72E0). Per frame: gear choice (0x5ABC00)
    /// and the brake and grip set-up (0x5A4D90, 0x5A49B0). Then the frame is
    /// split into ⌊dt / 4.4 ms⌋ + 1 equal sub-steps (0x7FC070), each one
    /// force pass (0x5B0530) and an explicit Euler step of every state
    /// (0x5AA490).
    pub fn step(&mut self, car: &Car, world: &World, controls: Controls, dt: f32) {
        let sample = self.begin_world_update(dt);
        step_cars(
            &mut [Racer { car, state: self, input: Input::Player(controls) }],
            &mut CarField::default(),
            world,
            dt,
        );
        self.finish_world_update(sample);
    }

    /// Low-level physics with knock-over props; returns the props moved.
    /// Unlike `step`, this does not publish world acceleration: game callers
    /// may invoke it repeatedly inside one outer update. Bracket the entire
    /// interval with `begin_world_update` / `finish_world_update`.
    ///
    /// [`step`]: CarState::step
    pub fn step_with_props(
        &mut self,
        car: &Car,
        world: &World,
        props: &mut crate::smackable::Props,
        controls: Controls,
        dt: f32,
        sounds: &mut ContactSounds,
    ) -> Vec<(usize, glam::Mat4)> {
        step_world(
            &mut [Racer { car, state: self, input: Input::Player(controls) }],
            &mut CarField::default(),
            props,
            world,
            dt,
            sounds,
        )
    }

    /// One frame driven by an AI driver: the same frame as [`step`], with
    /// the driver's wheel angle held for the frame (the mover copies
    /// +0x1E0 into the steering, 0x5ABC00, instead of the player's
    /// steering 0x416BB0), its pedals through the mover's check alone (no
    /// player reverse latch) and its gear request handed to the gearbox
    /// (0x5AA970: −1 the automatic choice, otherwise that gear).
    ///
    /// [`step`]: CarState::step
    pub fn step_ai(&mut self, car: &Car, world: &World, c: AiControls, dt: f32) {
        let sample = self.begin_world_update(dt);
        step_cars(&mut [Racer { car, state: self, input: Input::Ai(c) }], &mut CarField::default(), world, dt);
        self.finish_world_update(sample);
    }

    /// The drag race's part of the crash check (0x59FB80, each push): a
    /// push of 30 or more totals the car. Its second test, the body's
    /// vt+0x14 block +0x38 under −0.5, reads the orientation matrix's
    /// translation row (row 3 z), which the rigid body keeps at 0 (its
    /// position lives at +0x20; 0x59B778 resets row 3 to (0, 0, 0, 1)), so
    /// it never totals a car.
    pub(super) fn check_totaled(&mut self, h: &Handling, magnitude: f32) {
        if h.race.drag() && magnitude >= 30.0 {
            self.totaled = true;
        }
    }

    /// The mode update at the start of the car's frame (0x5FC060): crash
    /// mode ends once all four wheels have been down with no contact for
    /// 0.1 s; any contact or lifted wheel restarts that.
    pub(super) fn update_mode(&mut self, contacts: bool, dt: f32) {
        if !self.crash {
            return;
        }
        if contacts || self.wheels_down() != 4 {
            self.crash_clear = None;
            return;
        }
        match self.crash_clear {
            None => self.crash_clear = Some(0.0),
            Some(t) => {
                let t = t + dt;
                if t >= 0.1 {
                    self.crash = false;
                    self.crash_clear = None;
                } else {
                    self.crash_clear = Some(t);
                }
            }
        }
    }

    /// The wall drive (0x5A49B0, 0x5A4D24–0x5A4D7B, each frame): a car
    /// whose tune sends more than 0.8 of the torque to the rear (tuning
    /// +0x1C0 > 0x7850EC) gets a live rear share of 0.65 (0x3F266666) while
    /// under 60 mph (+0x430 × 2.237 < 0x787590) and touching a wall (the
    /// collision body's +0x120, its live contact records with a track
    /// polygon of |n.z| < 0.1), else its own share again; other cars keep
    /// the constructor's. Live (2026-10-31): a 240SX that hits a wall, or a
    /// Corolla reversing off a kerb, shows drive torque on its front wheels
    /// (+0x108) until it leaves the wall.
    pub(super) fn wall_drive(&self, h: &Handling, drivetrain: &mut Drivetrain) {
        let share = h.gearbox.rear_split;
        if share > 0.8 {
            let walls = self.records.values().filter(|r| r.upright).count();
            // Car +0x430 = |v|: the speed the previous frame's end
            // (0x5A5550) or a velocity set since (0x592170, a contact) wrote,
            // i.e. the velocity's length as it stands here, not the signed
            // forward speed (`fld [ecx+0x430]` at 0x5A4D2A).
            drivetrain.rear_share = if self.velocity.length() * MPH < 60.0 && walls > 0 { 0.65 } else { share };
        }
    }

    /// The car's movement over `dt` without the track's contacts: the
    /// physics mover's update (0x5B72E0), also run for the rest of a frame
    /// after a contact (0x6012C0).
    pub(super) fn simulate(&mut self, car: &Car, world: &World, input: Input, dt: f32, contacts: bool) {
        // The nitrous key's view: the state the previous world
        // update left, before this one's inputs.
        if !self.restep {
            self.nitrous_key = (self.gas(), self.velocity.length());
        }
        if let Input::Belt(_) = input {
            return;
        }
        // The green's update: the mover still reads the countdown's block
        // (see `AiControls::mover_held`).
        let input = match input {
            Input::Ai(c) if c.mover_held => {
                Input::Ai(AiControls { brake: 1.0, gear: crate::handling::NEUTRAL as i32, mover_held: false, ..c })
            }
            other => other,
        };
        if let Input::Melo(c) = input {
            self.melo_frame(car, world, c, dt);
            return;
        }
        let h = &car.car_type.handling;
        let mut drivetrain = self.mover_frame_start(h, input, contacts, dt);
        self.nitrous_frame(h, input, &mut drivetrain);

        let pedals = self.pedals_frame(h, input, &drivetrain, dt);
        self.gearbox_frame(h, input, &mut drivetrain, pedals);
        self.grip_frame(h, world, pedals, &drivetrain, dt);

        // The rest damping (rigid body +0x150) is switched on every frame
        // outside crash mode (0x5A4B04); in game (flow state 4, 0x5A4B7B) it
        // goes off again with the gas over 0.3 (0x78758C), or with all four
        // wheels down (car +0x424) on a slope: |forward.z| (car +0x98, read
        // as mover +0x58 → +0x58) over 0.15 (0x7A1A24). So a car rolling on
        // level ground with the gas off is damped (verified live: +0x150 = 1
        // while coasting, D climbing at 0.1/s below 4 m/s).
        let slope = (Mat3::from_quat(self.orientation) * Vec3::X).z.abs();
        let holding = !self.crash && !(pedals.throttle > 0.3 || (self.wheels_down() == 4 && slope > 0.15));

        let steps = (dt / MAX_SUBSTEP) as u32 + 1;
        let sub = dt / steps as f32;
        let player = matches!(input, Input::Player(_));
        // The player's steering runs once a frame, in the input update
        // (0x41E2C0 → 0x41DFC0 → 0x416BB0), before the physics.
        if player && !self.restep {
            self.steer(h, pedals, dt);
        }
        if !self.restep {
            self.frame_pedals = pedals;
        }
        self.aero_down = self.wheels_down();
        self.frame_gear = drivetrain.gear;
        #[cfg(test)]
        native_export_lifetime::observe(self, true);
        if self.restep {
            // After a contact the refresh (0x593710 → 0x5B04E0) took a new
            // force pass on the state after the impulse.
            self.forces = None;
        }
        for _ in 0..steps {
            self.substep(car, world, &mut drivetrain, pedals, player, holding, sub);
            self.last_sub = sub;
        }
        self.end_slip(car, world);
        self.shown_rpm = shown_rpm_step(
            self.shown_rpm,
            drivetrain.rpm(),
            h.engine.inertia,
            drivetrain.pending && drivetrain.target_gear > 2,
            dt,
        );
        self.drivetrain = Some(drivetrain);
        self.publish_forward_speed();
        #[cfg(test)]
        native_export_lifetime::observe(self, false);
        // The shot's time is the clock's (0x592490), so a re-step adds none.
        if let Some(t) = &mut self.nitrous
            && !self.restep
        {
            t.advance(dt);
        }
    }

    /// A frame on the MeloMvr instead of the physics mover.
    fn melo_frame(&mut self, car: &Car, world: &World, c: AiControls, dt: f32) {
        let mut melo = self.melo.take().unwrap_or_else(|| crate::melo::Melo::new(car, self, 0.0));
        melo.update(self, car, world, c, dt);
        // The brake flag is the car update's (0x5F62C0 from 0x6049D9,
        // the driver's +0x214), whichever mover runs: on the MeloMvr
        // too (round 13; before, a racer's brake lights froze there).
        self.note_brake(c.brake, dt);
        self.melo = Some(melo);
        // A running shot's time is the clock less its start tick
        // (0x592490), whichever mover runs; the MeloMvr's own nitrous
        // stage (Melov8r +0x198, read at 0x598B62) is written only by
        // its constructors (0x5A8199, 0x5B01D2), so a shot adds no
        // torque here.
        if let Some(t) = &mut self.nitrous
            && !self.restep
        {
            t.advance(dt);
        }
    }

    /// The mover's frame start (0x5B72E0): the hydraulics' kick flags
    /// cleared and the mounts set from the heights (0x5AB9A0); AI
    /// drivers have no hydraulics input. Returns the drivetrain taken out
    /// of the car for the frame.
    fn mover_frame_start(&mut self, h: &Handling, input: Input, contacts: bool, dt: f32) -> Drivetrain {
        self.hydraulics.frame(match input {
            Input::Player(c) | Input::Drag(c, _) => c.hydraulics,
            Input::Ai(_) | Input::Melo(_) | Input::Belt(_) => Default::default(),
        });
        self.update_mode(contacts, dt);
        self.update_inertia(h);
        let mut drivetrain = self.drivetrain.take().unwrap_or_else(|| Drivetrain::new(h));
        self.wall_drive(h, &mut drivetrain);
        // No engine heat on the line (see `release_hold`).
        if self.held {
            drivetrain.heat = -1.0;
        }
        drivetrain
    }

    /// The nitrous tank's frame: filled at set-up, shots started and ended
    /// by the driver, the stage into the engine.
    fn nitrous_frame(&mut self, h: &Handling, input: Input, drivetrain: &mut Drivetrain) {
        // The tank: filled at the car's set-up (0x59F9DA) and again when
        // its size no longer matches the record (0x5F6C8C, at the racer's
        // reset; the clone fills it once).
        let tank = self.nitrous.get_or_insert_with(|| crate::nitrous::Tank::new(h.nitrous_capacity, h.nitrous_shots));
        // The player's Nitrous events (0x6034C6): pressed, a shot starts
        // when one may (0x5EB570 → 0x59FB00); let go, a running shot ends
        // (0x5EB540, 0x7FBEDC = 1 → 0x592510). An AI driver with its
        // request (+0x1FF) starts one from its update (0x420810) whenever
        // none runs; only an empty tank ends it (0x59FA40).
        if let Input::Ai(c) = input
            && c.nitrous
            && tank.level() == 0
        {
            tank.start();
        }
        if let Input::Player(c) | Input::Drag(c, _) = input {
            if c.nitrous != self.nitrous_held {
                if c.nitrous {
                    tank.start();
                } else if tank.level() > 0 {
                    tank.stop();
                }
                self.nitrous_held = c.nitrous;
            }
        }
        // The mover (0x5ABD75): the shot's stage into engine +0x84, an
        // empty one ending.
        drivetrain.nitrous = tank.level();
    }

    /// The frame's pedals (and the steering angle of a drag or AI driver).
    fn pedals_frame(&mut self, h: &Handling, input: Input, drivetrain: &Drivetrain, dt: f32) -> Controls {
        let (gear, rpm) = (drivetrain.gear, drivetrain.rpm());
        let pedals = match input {
            Input::Player(_) | Input::Drag(..) if self.restep => self.frame_pedals,
            Input::Player(controls) => self.pedals(h, controls, gear, rpm, dt),
            Input::Drag(controls, angle) => {
                self.steer_degrees = angle;
                self.pedals(h, controls, gear, rpm, dt)
            }
            Input::Ai(c) | Input::Melo(c) | Input::Belt(c) => {
                self.steer_degrees = c.steer_degrees;
                self.ai_pedals(h, c)
            }
        };
        pedals
    }

    /// The gearbox's frame: the gear request or choice, the clutch hold,
    /// the catch-up and the engine's throttle.
    fn gearbox_frame(&mut self, h: &Handling, input: Input, drivetrain: &mut Drivetrain, pedals: Controls) {
        // Up-shifts wait until every wheel is down and none spins (wheel
        // +0x11C set and slip +0x118 under 4, signed: 0x5AA970 compares the
        // slip itself, so a wheel turning slower than the road, slip
        // negative, does not hold the shift; live, a 240SX kicked to 40 m/s
        // in 1st shifted at rear slip −7.5 m/s).
        let gripping = self.wheels.iter().all(|w| w.on_ground && w.slip < 4.0);
        // The mover hands 0x5AA970 the car's shown rpm (+0x440, the last
        // publish; 0x5ABDA2), which the gear choice and the drag shift
        // grading read.
        drivetrain.shown_rpm = Some(self.shown_rpm);
        match input {
            Input::Ai(c) if c.gear >= 0 => drivetrain.request_gear(h, c.gear as usize),
            Input::Ai(_) | Input::Melo(_) | Input::Belt(_) => drivetrain.choose_gear(h, false, gripping),
            // The race start's hold asks for neutral (0x41E38F).
            _ if self.held => drivetrain.request_gear(h, crate::handling::NEUTRAL),
            // A manual gearbox takes the player's shifts, reverse while the
            // pedals ask for it (+0x1E4 = 0) and first gear from vt+0x30;
            // 0x5AA970 with code −1 runs for the automatic one.
            _ if self.manual => {
                if self.reverse_requested {
                    drivetrain.request_gear(h, 0);
                } else if let Some(g) = self.gear_request.take() {
                    drivetrain.request_gear(h, g);
                }
            }
            _ => drivetrain.choose_gear(h, self.reverse_requested, gripping),
        }
        // The human driver's update (0x41E2C0, transmission mode not 3):
        // in a drift race the handbrake over 0.6 holds the clutch open
        // (+0x74); outside one it is left clear. AI drivers never set it.
        drivetrain.clutch_held = match input {
            Input::Player(controls) | Input::Drag(controls, _) => h.race.drift() && controls.handbrake > 0.6,
            Input::Ai(_) | Input::Melo(_) | Input::Belt(_) => false,
        };
        // The catch-up into the engine (0x599180 → engine +0x90).
        drivetrain.catch_up = self.catch_up;
        // The engine's throttle (0x5ABC00): nothing up to 0.15; a light
        // pedal in a forward gear under 3000 rpm opens it to half.
        drivetrain.throttle = if pedals.throttle <= 0.15 {
            0.0
        } else if drivetrain.gear > NEUTRAL && drivetrain.rpm() < 3000.0 && pedals.throttle < 0.5 {
            0.5
        } else {
            pedals.throttle
        };
    }

    /// The frame's grip set-up: brakes and grip per wheel, the nitrous
    /// grip, the shift bonus and the wheel hop.
    fn grip_frame(&mut self, h: &Handling, world: &World, pedals: Controls, drivetrain: &Drivetrain, dt: f32) {
        let wet = crate::weather::wetness_at(world, self.position.truncate());
        self.frame_setup(h, pedals, wet, dt);
        self.note_brake(pedals.brake, dt);
        // 0x5A5106: a running shot's stage scales each wheel's grip
        // (+0x180) by 0x7A17AC[stage].
        if drivetrain.nitrous > 0 {
            for w in &mut self.wheels {
                w.grip *= crate::nitrous::GRIP[drivetrain.nitrous.min(3)];
            }
        }
        self.shift_bonus_grip(h, drivetrain);
        self.wheel_hop(h, if self.frame_dt > 0.0 { self.frame_dt } else { dt });
    }

    /// The wheels' exported slip and the hop's load terms as the frame
    /// leaves them. The original's mover takes the force pass after each
    /// integration, for the next one (0x5B04E0 → 0x5B0530), and so does
    /// the clone's sub-step: the tyre force (0x5A8290) has written each
    /// wheel's slip (+0x118, r·ω − v along the wheel at the hub, 0x5A83E0)
    /// and load term (+0x164) from the state the last sub-step ended in,
    /// which the per-frame readers (the wheel hop 0x5B6310, the AI's
    /// nitrous test) see (observed under Wine, `tools/parity/subtrap.py`:
    /// the launch's frame-5 rear slip 0.698 from the end state).
    pub(super) fn end_slip(&mut self, car: &Car, _world: &World) {
        let table = &car.car_type.wheels;
        let h = &car.car_type.handling;
        let rot = Mat3::from_quat(self.orientation);
        let com_local = Vec3::from(h.centre_of_mass);
        for i in 0..4 {
            let arm = wheel_hub_arm(rot, self.mount(table[i].position, i), com_local, self.wheels[i].compression);
            let angle = if i < 2 { self.front_wheel_angle(table, i) } else { 0.0 };
            let (sa, ca) = angle.sin_cos();
            let wf = rot * Vec3::new(ca, sa, 0.0);
            let v = wheel_hub_velocity(self.velocity, self.spin, arm);
            let w = &mut self.wheels[i];
            w.exported_slip = exported_slip(w.slip, v.dot(wf), w.lateral_slip, w.locked, h.tyres[i / 2].slip_angle_scale);
        }
        self.hop_loads = std::array::from_fn(|i| self.wheels[i].load);
        if crate::env_value_os!("NFSU2_SUBSTEP_DUMP")
            .is_some_and(|n| car.car_type.name.eq_ignore_ascii_case(&n.to_string_lossy()))
        {
            eprintln!("SUBSTEP END loads {:?}", self.hop_loads);
        }
    }

    /// Debug aid (not native): NFSU2_SUBSTEP_DUMP=NAME prints car NAME's state at the
    /// points tools/parity/subtrap.py stops the original (A before the integration, B after it).
    pub(super) fn debug_substep(&self, at: char, car: &Car, d: &Drivetrain, dt: f32) {
        let Some(name) = crate::env_value_os!("NFSU2_SUBSTEP_DUMP") else { return };
        if !car.car_type.name.eq_ignore_ascii_case(&name.to_string_lossy()) {
            return;
        }
        let w = |i: usize| {
            (
                self.wheels[i].spin,
                self.wheels[i].torque,
                self.wheels[i].slip,
                self.wheels[i].locked as u8,
                self.wheels[i].sliding as u8,
                self.wheels[i].load,
            )
        };
        eprintln!(
            "SUBSTEP {at} x {:.2} ghost {:.3} held {} hyd {} restep {} recs {} dt {dt:.6} v {:.5} {:.5} {:.5} eng {:.4} clutch {:.6} shift {} gear {} in {:.4} sent {:.5} rest {} frame_gear {} down {} w {:?} {:?} {:?} {:?}",
            self.position.x,
            self.ghost_timer,
            self.held as u8,
            self.hydraulics.moving(),
            self.restep as u8,
            self.records.len(),
            self.velocity.x,
            self.velocity.y,
            self.velocity.z,
            d.engine_w,
            d.clutch,
            d.shift as u8,
            d.gear,
            d.input_w,
            d.sent,
            self.rest,
            self.frame_gear,
            self.aero_down,
            w(0),
            w(1),
            w(2),
            w(3)
        );
    }

    // 0x5ABB10 requests the zero-time traversal after rest or any bump
    // visit, including zero impulse. Its repeated suspension load uses
    // current neighbours; 0x5B0590 consumes that final load for forces.
    pub(super) fn suspension_neighbour_compression(
        &self,
        i: usize,
        compressions: &[f32; 4],
        bump_visited: bool,
    ) -> f32 {
        if self.rest != 0 || bump_visited { compressions[i ^ 1] } else { self.wheels[i ^ 1].compression }
    }

    /// Set the driven wheels so the gearbox input turns at `crank_w`
    /// (0x5A12E0): each driven axle moves by the difference over its ratio.
    /// Under 40 mph, or with both of an axle's wheels in the air, the pair
    /// first takes its mean speed.
    pub(super) fn match_wheels(
        &mut self,
        g: &crate::handling::Gearbox,
        drivetrain: &Drivetrain,
        crank_w: f32,
        input_w: f32,
    ) {
        let slow = self.speed() * MPH < 40.0;
        let [front, rear] = drivetrain.axle_ratios(g);
        for (first, ratio, driven) in [(2, rear, g.rear_split >= 0.001), (0, front, g.rear_split <= 0.999)] {
            if !driven {
                continue;
            }
            let (a, b) = (first, first + 1);
            let together = slow || (!self.wheels[a].on_ground && !self.wheels[b].on_ground);
            let delta = (crank_w - input_w) / ratio;
            if together {
                let mean = (self.wheels[a].spin + self.wheels[b].spin) * 0.5;
                self.wheels[a].spin = mean;
                self.wheels[b].spin = mean;
            }
            self.wheels[a].spin += delta;
            self.wheels[b].spin += delta;
        }
    }

    // The drivetrain reads physics-view +0x3E4 (full car +0x424),
    // not the suspension contacts just measured in this sub-step.
    pub(super) fn drivetrain_wheels_down(&self) -> usize {
        self.aero_down
    }

    /// The share of an axle's drive torque that wheel `i` gets (0x594A40):
    /// each wheel's share of the axle's wheel spin, held between 35% and
    /// 65% (0x7FBFFC); beyond that the faster wheel gets only 17.5%. With a
    /// wheel off the ground the band closes to 50%, so the slower wheel
    /// takes 82.5% as soon as they differ.
    pub(super) fn differential_share(&self, i: usize) -> f32 {
        const BAND: f32 = 0.35;
        let other = i ^ 1;
        let (me, them) = (&self.wheels[i], &self.wheels[other]);
        let low = if me.on_ground && them.on_ground { BAND } else { 0.5 };
        // The wheel hop's last two counts fix the split (car +0x46C): 1
        // gives FR and RL 1 − low, 2 gives them low (0x594A40).
        if self.hop == 1 || self.hop == 2 {
            let second = i == 1 || i == 2;
            let p = if self.hop == 1 { 1.0 - low } else { low };
            return if second { p } else { 1.0 - p };
        }
        let total = me.spin + them.spin;
        if total.abs() <= 1e-5 {
            return 0.5;
        }
        let share = me.spin / total;
        if share < low {
            1.0 - BAND * 0.5
        } else if 1.0 - low < share {
            BAND * 0.5
        } else {
            share
        }
    }
}

/// The longitudinal slip a wheel exports to the car (wheel +0x100, which
/// the physics mover's export 0x5A5550 copies to car +0x3B4 + 8i): the tyre
/// pass 0x5A8290 stores the slip (+0x118, r·ω − v) there in its sliding
/// branch — slip over 0.5 m/s (0x7FC704), the contact's travel angle over
/// the tyre record's +0 or the wheel locked, with any travel at all — and 0
/// in its gripping branch (0x5A86A1). `forward` and `lateral` are the hub's
/// speed along and across the wheel.
pub(crate) fn exported_slip(slip: f32, forward: f32, lateral: f32, locked: bool, angle_limit: f32) -> f32 {
    if (slip * slip + lateral * lateral).sqrt() == 0.0 || (forward == 0.0 && lateral == 0.0) {
        return 0.0;
    }
    let mut angle = atan16(forward, lateral) as f32 * 9.587_38e-5;
    if angle > std::f32::consts::PI {
        angle -= std::f32::consts::TAU;
    }
    if forward < 0.0 {
        angle += std::f32::consts::PI;
        if angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
    }
    if slip.abs() > SLIDE_SLIP || angle.abs() > angle_limit || locked { slip } else { 0.0 }
}

#[cfg(test)]
mod export_tests {
    use super::exported_slip;

    /// The tyre pass's export (wheel +0x100 → car +0x3B4 + 8i), held to the
    /// original's record of the first drag's launch (60 fps, rival CORSA,
    /// frame 12): its front wheels turned 0.33 m/s faster than they rolled
    /// (the clone's +0x118 there) while the tyres still gripped, and car
    /// +0x3B4 / +0x3BC read 0.00; at frame 18 they slid and it read the
    /// slip (11.42 on the front right, the clone 11.23).
    #[test]
    fn a_gripping_tyre_exports_no_slip() {
        let angle_limit = 0.25;
        assert_eq!(exported_slip(0.33, 0.10, 0.0, false, angle_limit), 0.0);
        assert_eq!(exported_slip(-0.01, 8.0, 0.0, false, angle_limit), 0.0);
        assert_eq!(exported_slip(11.23, 0.73, 0.0, false, angle_limit), 11.23);
        // Just past 0.5 m/s (0x7FC704) the slip goes out as it is.
        assert_eq!(exported_slip(0.51, 0.15, 0.0, false, angle_limit), 0.51);
        assert_eq!(exported_slip(-0.6, 5.0, 0.0, false, angle_limit), -0.6);
        // A travel angle over the tyre record's +0, or a locked wheel, slides too.
        assert_eq!(exported_slip(0.2, 1.0, 1.0, false, angle_limit), 0.2);
        assert_eq!(exported_slip(-0.2, 1.0, 0.0, true, angle_limit), -0.2);
        // No travel at all: the gripping branch (0x5A8290's first test).
        assert_eq!(exported_slip(3.0, 0.0, 0.0, false, angle_limit), 0.0);
    }
}
