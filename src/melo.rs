//! The MeloMvr ("Mellow" mover, 0x5B6D10, vtable 0x7A2540; mode 3 of the
//! mode switch 0x5F46B0): the simple car model a traffic car falls to when
//! it is hit (controller slot +0x38, 0x408ED0). It shares only the rigid
//! body (0x5A7F40, the physics' RigidBody) with the physics mover; its
//! chassis ("Melov8r", 0x5B0040, vtable 0x7A2568) owns the suspension,
//! the tyres' forces, the four wheel spins and a gearbox, with no clutch,
//! no engine inertia and no drivetrain coupling. See docs/formats.md,
//! "Traffic" → "MeloMvr".
//!
//! Its frame (slot +0x04, 0x5AADA0): the set-up below, then ⌊dt · 70⌋ + 1
//! equal sub-steps (0x7FC040 = 1/70 s), each the generic Euler step
//! (0x5AA490) of the body and the chassis (whose derivative, slot +0x18
//! 0x598AD0, is the wheel spins' rate) followed by the force pass for the
//! next one (0x5AB0D0): the state objects' slot +0x1C (the chassis'
//! 0x5AB1A0: suspension 0x5A3710, then the gearbox), the bump stops
//! (0x5A3E90), then 0x5AB140: the tyres (slot +0x14, 0x5A38F0), the aero
//! (0x59E630), the hop forces (0x5A2FE0) and the airborne terms
//! (0x5A3420). The force pass has no other caller: it is the MeloMvr
//! sim's post-step (vtable 0x7A2534 slot +4), so each sub-step first
//! integrates the sums the previous pass left (the first sub-step of a
//! frame the previous frame's last pass, with that frame's inputs), then
//! runs the pass on the rows as the Euler step left them, then 0x5BB0E0
//! cleans the rows.

use glam::{Mat3, Vec3};

use crate::car::Car;
use crate::drive::{AiControls, CarState};
use crate::world::World;

/// Sub-steps per second (0x7FC040 = 1/70 s).
const SUBSTEP: f32 = 1.0 / 70.0;
/// The springs and dampers both work at 0.9 of the record's rates
/// (0x5A3710).
const SPRING_SCALE: f32 = 0.9;
/// Tyre force per kN of load per m/s of slip (0x5A38F0).
const TYRE_GAIN: f32 = 1.25;
/// The tyres' horizontal force cap, × load × grip (0x5A38F0; 2 online
/// (0x89E7E3 / 0x89010C) and 3.2 on tracks with flag 0x2000 at record
/// +0x94, neither of which the clone's free roam and races use).
const TYRE_CAP: f32 = 4.0;
/// Over this much slip (× grip) the tyre gain drops to the skill grip.
const TYRE_SOFT: f32 = 0.05;
/// rad/s → rpm (0x7A1D30).
const RPM: f32 = 9.549_304;
/// The wheels lock with the brake over 0.95 (0x78A7DC) while turning
/// under 30 rad/s (0x7A1960), and free again under 0.2 (0x784554).
const LOCK_BRAKE: f32 = 0.95;
const LOCK_SPIN: f32 = 30.0;
const FREE_BRAKE: f32 = 0.2;
/// Wheel inertia in the spin rate (0x7A1984).
const WHEEL_INERTIA: f32 = 0.008;
/// The brake's spin clamp and gain: −2.5 × 0.2 × brake × clamp(ω, ±20)
/// (0x7A195C, 0x7846EC, 0x7A1D68, 0x7850D8).
const BRAKE_GAIN: f32 = -0.5;
const BRAKE_SPIN: f32 = 20.0;
/// No engine torque with the gas under 0.05 (0x7844F4) at idle.
const IDLE_GAS: f32 = 0.05;
/// The tyre reaction's lever over the body's speed (graph 0x86B578, built
/// at 0x77DF40 from 0x7FC8C8): {0, 0.05}, {2, 0.322}, {20, 0.5}.
const REACTION: [(f32, f32); 3] = [(0.0, 0.05), (2.0, 0.322), (20.0, 0.5)];
/// Spin damping with fewer than two wheels down (0x5A3420, graph 0x86B498
/// from 0x7FC8E0): −g(|ω|)·m·ω.
const AIR_SPIN: [(f32, f32); 3] = [(0.0, 0.0), (0.1, 0.0), (2.0, 20.0)];
/// The bump stop's push per m/s closing per tonne (0x5A3E90).
const BUMP: f32 = 0.2;
/// The first up-shift's scale by drive layout (front, all, rear;
/// 0x7FC2B8 + 0x144·layout, applied by the ctor 0x5B0040).
const FIRST_SHIFT: [f32; 3] = [1.0, 1.2, 1.1];

/// The chassis' state (0x5B0040's object; offsets are the chassis').
#[derive(Clone, Debug)]
pub struct Melo {
    /// Wheel spins, rad/s (+0x20 + 4·i).
    pub spin: [f32; 4],
    /// Suspension compression, m (+0x30 + 4·i).
    pub compression: [f32; 4],
    /// Spring force (the load), kN (+0x50 + 4·i).
    pub load: [f32; 4],
    /// The wheel touches the ground (+0x60 + 4·i).
    pub contact: [bool; 4],
    /// The tyre's car-space force (x, y) after the steer, kN, held within
    /// 50 (+0x70 + 8·i); the spin rate reads x as the tyre's reaction.
    pub tyre_force: [(f32, f32); 4],
    /// Longitudinal and lateral slip, m/s (+0x90 + 8·i).
    pub slip: [(f32, f32); 4],
    /// Skidding: slip over 6 m/s (+0xB0 + i).
    pub skid: [bool; 4],
    /// Locked by the brake (+0xB4 + 4·i): no spin, no spin rate.
    pub locked: [bool; 4],
    /// Compression past the limit, m (+0xC4 + 4·i), for the bump stop.
    over: [f32; 4],
    /// The ground normal under each wheel (+0xE0 + 0x10·i).
    normal: [Vec3; 4],
    /// Engine rpm (+0x120), held within idle … max.
    pub rpm: f32,
    /// Gear (+0x124): 0 reverse, 2.. forward (1 neutral never taken).
    pub gear: usize,
    /// The skill grip the soft tyre gain uses (+0x144): 0.25 + 0.025 ×
    /// the car's rating on the 0–10 scale (0x5B69A0(1, −1) on car +0x10),
    /// clamped.
    pub skill_grip: f32,
    /// +0x140: the grip (1, less in the rain).
    pub grip: f32,
    /// Up- and down-shift rpm by gear (+0x19C, +0x1C0; 0x5AC7E0).
    up_rpm: Vec<f32>,
    down_rpm: Vec<f32>,
    /// The force and torque sums the last force pass (0x5AB0D0) left for
    /// the next sub-step's integration (rigid body +0x110 / +0x120,
    /// gravity included); `None` until the mover's first pass.
    pending: Option<(Vec3, Vec3)>,
}

impl Melo {
    /// The mover's set-up (0x5B6D10 → 0x5B0040, then 0x5A2A70): the wheel
    /// spins (car +0x2F0 …), gear (+0x3F4) and rpm (+0x400) carried over
    /// from the car as the last mover left them; `rating` the car's rating
    /// on the skills' 0–10 scale.
    pub fn new(car: &Car, state: &CarState, rating: f32) -> Melo {
        let h = &car.car_type.handling;
        let (mut up_rpm, down_rpm) = crate::handling::shift_points(h);
        let split = h.gearbox.rear_split;
        let layout = if split > 0.8 { 2 } else if split >= 0.2 { 1 } else { 0 };
        if let Some(u) = up_rpm.get_mut(crate::handling::NEUTRAL) {
            *u *= FIRST_SHIFT[layout];
        }
        let (gear, rpm) = state.drivetrain.as_ref().map_or((2, h.engine.idle_rpm), |d| (d.gear, d.rpm()));
        Melo {
            spin: std::array::from_fn(|i| state.wheels[i].spin),
            compression: [0.0; 4],
            load: [0.0; 4],
            contact: [false; 4],
            tyre_force: [(0.0, 0.0); 4],
            slip: [(0.0, 0.0); 4],
            skid: [false; 4],
            locked: [false; 4],
            over: [0.0; 4],
            normal: [Vec3::Z; 4],
            rpm: rpm.clamp(h.engine.idle_rpm, h.engine.max_rpm),
            gear: gear.min(h.gearbox.ratios.len() - 1),
            skill_grip: 0.25 + 0.025 * rating.clamp(0.0, 10.0),
            grip: 1.0,
            up_rpm,
            down_rpm,
            pending: None,
        }
    }

    /// The AI nitrous band on the MeloMvr (0x402590, car +0x28 = 3): the
    /// rpm within [A, A + 0.95·(B − A)], A the gear's down-shift rpm
    /// (chassis +0x1C0 + 4·gear) and B its up-shift rpm (+0x19C + 4·gear).
    /// The original reads the rpm at car +0x440, which the write-back
    /// fills from the chassis' (0x5A2B70).
    pub fn in_nitrous_band(&self) -> bool {
        let (Some(&a), Some(&b)) = (self.down_rpm.get(self.gear), self.up_rpm.get(self.gear)) else { return false };
        let top = (b - a) * 0.95 + a;
        self.rpm == self.rpm.max(a).min(top)
    }

    /// One frame (0x5AADA0) driven by an AI driver's controls: the wheel
    /// angle (+0x1E0 → chassis +0x130), gas (+0x20C → +0x128), brake
    /// (the larger of +0x214 and half the handbrake +0x210 → +0x12C) and
    /// gear request (slot +0x20 → +0x134: −1 automatic, 0 reverse).
    pub fn update(&mut self, state: &mut CarState, car: &Car, world: &World, c: AiControls, dt: f32) {
        let h = &car.car_type.handling;
        // The surfaces' air-drag factor (+0x50) over the wheels down, 1
        // with none (aero +0x34); crash mode (car +0x38 = 1): gravity 11,
        // no rest hold, the aero factor (+0x30) a quarter per wheel down.
        let down: Vec<u8> = (0..4).filter(|&i| self.contact[i]).map(|i| state.wheels[i].surface).collect();
        let (gravity, aero, surface_drag) = if state.crash {
            (crate::drive::CRASH_GRAVITY, down.len() as f32 * 0.25, 1.0)
        } else {
            let drag = if down.is_empty() { 1.0 } else { down.iter().map(|&s| crate::drive::surface(s).drag).sum::<f32>() / down.len() as f32 };
            (crate::drive::GRAVITY, 1.0, drag)
        };
        // The body's inertia (0x5AAF37): the ground tensor with a wheel
        // down; leaving it divides L.z by record +0x270.
        let ground = !down.is_empty();
        if state.ground_inertia && !ground && h.unknown_270 != 0.0 {
            state.angular_momentum.z /= h.unknown_270;
        }
        state.ground_inertia = ground;
        // The aero object's inputs (0x5AAFD2..0x5AAFF2, each frame): +0x10
        // the AI driver's gas (+0x20C, the throttle of the drag's (2 − t)
        // term) and +0x14 the wheels down (car +0x424, the last write-back)
        // × 0.25.
        // 0x5A3390 (arg car +0x468; 0x7FC0A0 is 1, so the arg is used): k =
        // the catch-up (max(controller +0x770, 1) for an AI racer, else 1)
        // into aero +0x08 (|v| ÷ k), body +0x208 (the top speed × k), the
        // grip +0x140 and the chassis' +0x13C (the torque curve read at
        // rpm ÷ k, the drive × (1 + 3(k − 1)), the driven wheels' lever ÷
        // (1 + 2.5(k − 1)), 0x598B26 / 0x598C6E). Before this fix the clone took the
        // drag as 2·c (t = 0), the downforce as 40 / 41 % of the sum and k as 1.
        let scale = state.catch_up.max(1.0);
        let controls = Pedals { steer: c.steer_degrees.to_radians(), gas: c.gas, brake: c.brake.max(c.handbrake * 0.5), gear: c.gear, gravity, aero, surface_drag, holding: !state.crash,
            throttle: c.gas, down_share: down.len() as f32 * 0.25, scale };
        // The grip (+0x140, k from 0x5A3390; 0x7A1998 = 1 in its
        // fast-and-no-stability-control case) × (1 − 0.025·rain at the car)
        // (0x5AB01F → 0x5ED5E0, 0x7A198C = 0.975): it scales the soft
        // slip threshold, the soft gain and the force cap (0x5A3AF0,
        // 0x5A3B09, 0x5A3CBA).
        self.grip = scale * (1.0 - crate::weather::wetness_at(world, state.position.truncate()) * (1.0 - 0.975));
        let preload = self.preload(car, state);
        let steps = (dt / SUBSTEP) as u32 + 1;
        let sub = dt / steps as f32;
        for _ in 0..steps {
            self.substep(state, car, world, &controls, preload, sub);
        }
        // The write-back to the car (0x5A2B70, scalar store 0x5A2DAB).
        state.publish_forward_speed();
        state.steer_degrees = c.steer_degrees;
        for i in 0..4 {
            let w = &mut state.wheels[i];
            w.spin = self.spin[i];
            w.compression = self.compression[i];
            w.on_ground = self.contact[i];
            w.load = self.load[i];
            w.locked = self.locked[i];
            w.slip = self.slip[i].0;
            // The write-back 0x5A2B70 copies the chassis' pair (+0x90 + 8i)
            // to car +0x3B4 + 8i as it stands.
            w.exported_slip = self.slip[i].0;
            w.lateral_slip = self.slip[i].1;
            w.sliding = self.skid[i];
            w.roll = (w.roll + w.spin * dt) % std::f32::consts::TAU;
        }
    }

    /// The suspension's preload per wheel (0x598A00, from 0x5A29B0 each
    /// frame): the weight plus the downforce (front + rear) × the aero
    /// factor × the forward speed (≥ 0) × record +0x37C, split by where
    /// the centre of mass sits between the axles (0x7FC06C = 1).
    fn preload(&self, car: &Car, state: &CarState) -> [f32; 4] {
        let h = &car.car_type.handling;
        let table = &car.car_type.wheels;
        let (front_x, rear_x) = (table[0].position.x, table[3].position.x);
        let rear_share = 0.5 * (front_x - h.centre_of_mass[0]) / (front_x - rear_x);
        let f = (h.downforce[0] + h.downforce[1]) * state.speed().max(0.0) * h.preload_downforce + h.mass * crate::drive::GRAVITY;
        std::array::from_fn(|i| f * if i < 2 { 0.5 - rear_share } else { rear_share })
    }

    fn substep(&mut self, state: &mut CarState, car: &Car, world: &World, p: &Pedals, preload: [f32; 4], dt: f32) {
        let h = &car.car_type.handling;
        let com_local = Vec3::from(h.centre_of_mass);
        let rot = state.body_rows();
        let com = state.position + rot * com_local;
        // **Inferred**: on the mover's first sub-step no pass has run yet;
        // the clone runs one on the pose as it stands (what the new rigid
        // body's sums hold natively, ctor 0x5A7F40, is not traced).
        let (force, torque) = match self.pending.take() {
            Some(sums) => sums,
            None => self.force_pass(state, car, world, p, preload, rot, com, dt),
        };
        // 0x5AA490: every member's derivative first: the chassis' wheel
        // spin rates (0x598AD0) from the last pass's tyre forces and |v|
        // as it stands; then the Euler step of the body and the spins.
        let rates = self.spin_rates(h, state.velocity.length(), p);
        let (stepped, new_com) = integrate(state, h, p, rot, com, force, torque, dt);
        for i in 0..4 {
            self.spin[i] += rates[i] * dt;
        }
        // The post-step: the force pass for the next sub-step,
        // on the rows the Euler step left (0x5BB0E0 comes after 0x5AA490).
        self.pending = Some(self.force_pass(state, car, world, p, preload, stepped, new_com, dt));
        let rows = crate::drive::rigid::orthonormal(stepped);
        state.set_body_rows(rows);
        state.position = new_com - rows * com_local;
    }

    /// The force pass (0x5AB0D0): suspension and gearbox (0x5AB1A0), the
    /// bump stops (0x5A3E90), then 0x5AB140's tyres, aero and airborne
    /// terms, on the body at `rot` / `com`. Returns the sums with gravity
    /// (queued by the tidy-up 0x59C600).
    #[allow(clippy::too_many_arguments)]
    fn force_pass(&mut self, state: &mut CarState, car: &Car, world: &World, p: &Pedals, preload: [f32; 4], rot: Mat3, com: Vec3, dt: f32) -> (Vec3, Vec3) {
        let h = &car.car_type.handling;
        let table = &car.car_type.wheels;
        let (fwd, left, up) = (rot * Vec3::X, rot * Vec3::Y, rot * Vec3::Z);
        let com_local = Vec3::from(h.centre_of_mass);
        let omega = state.spin;
        let mut force = Vec3::ZERO;
        let mut torque = Vec3::ZERO;
        apply(&mut force, &mut torque, Vec3::new(0.0, 0.0, -h.mass * p.gravity), Vec3::ZERO);

        let frame = BodyFrame { rot, fwd, left, up, com, com_local, omega };
        self.suspension(state, h, table, world, &frame, preload, dt);
        self.gearbox(h, p);
        self.bump_stops(state, h, table, &frame, &mut force, &mut torque);
        self.tyre_forces(state, table, p, &frame, &mut force, &mut torque);
        aero_forces(state, h, p, &frame, &mut force, &mut torque);

        // Airborne (0x5A3420, not in crash mode): with fewer than two
        // wheels down, −g(|ω|)·m·ω. Its righting torques with no wheel
        // down have strengths (cos − 1)·100·m ≤ 0, which it zeroes.
        let wheels_down = self.contact.iter().filter(|&&c| c).count();
        if !state.crash && wheels_down < 2 {
            torque += -omega * crate::drive::graph(&AIR_SPIN, omega.length()) * h.mass;
        }
        (force, torque)
    }

    /// The suspension (0x5A3710 over 0x598D60): each wheel's place, its
    /// car-space position less the centre of mass less the ride offset
    /// (+0x1E8 + 4·i, 0: the hop target 0x5A2FE0 ramps it to is 0 for
    /// traffic), finds the ground (0x5E72F0, normal into +0xE0); the
    /// compression is the drop to it over n·up plus the radius (−100
    /// when n·up < 0.001), within the record's travel.
    #[allow(clippy::too_many_arguments)]
    fn suspension(
        &mut self,
        state: &mut CarState,
        h: &crate::handling::Handling,
        table: &[crate::car::Wheel; 4],
        world: &World,
        frame: &BodyFrame,
        preload: [f32; 4],
        dt: f32,
    ) {
        let BodyFrame { rot, up, com, com_local, .. } = *frame;
        for i in 0..4 {
            let w = &table[i];
            let s = &h.suspension[i / 2];
            let at = com + rot * (w.position - com_local);
            let (g, n) = match world.ground_for(&mut state.ground, at) {
                Some((z, n, surface)) => {
                    state.wheels[i].surface = if h.race.drag() || h.race.street_x() { 1 } else { surface };
                    (z, n)
                }
                None => (-10000.0, Vec3::Z),
            };
            self.normal[i] = n;
            let d = n.dot(up);
            let x = if d >= 0.001 { (g - at.z) / d } else { -100.0 } + w.radius;
            let old = self.compression[i];
            if x >= s.max_extension {
                if x <= s.max_compression {
                    self.compression[i] = x;
                    self.over[i] = 0.0;
                } else {
                    self.compression[i] = s.max_compression;
                    self.over[i] = x - s.max_compression;
                }
                self.contact[i] = true;
            } else {
                self.compression[i] = s.max_extension;
                self.contact[i] = false;
                self.over[i] = 0.0;
            }
            let x = self.compression[i];
            let rate = (x - old) / dt.max(0.0001);
            self.load[i] = if self.contact[i] {
                let damper = if rate <= 0.0 { s.rebound } else { s.bump };
                ((x.max(0.0) * s.progression + 1.0) * s.spring * SPRING_SCALE * x + damper * SPRING_SCALE * rate + preload[i]).max(0.0)
            } else {
                0.0
            };
        }
    }

    /// The gearbox (0x5AB1A0): the brake locks and frees the wheels;
    /// the rpm comes from the driven wheels' mean spin by the final
    /// drive (front under a 0.2 rear split, rear over 0.8, else the mean
    /// of both) through the gear's ratio; automatic shifts up past the
    /// gear's up-shift rpm and down under its down-shift rpm (from first
    /// gear up), 0 asks for reverse; held within idle … max.
    fn gearbox(&mut self, h: &crate::handling::Handling, p: &Pedals) {
        for i in 0..4 {
            if p.brake > LOCK_BRAKE && self.spin[i].abs() < LOCK_SPIN {
                self.locked[i] = true;
                self.spin[i] = 0.0;
            }
            if p.brake < FREE_BRAKE {
                self.locked[i] = false;
            }
        }
        let g = &h.gearbox;
        let front = (self.spin[0] + self.spin[1]) * g.final_drive_front * 0.5 * RPM;
        let rear = (self.spin[2] + self.spin[3]) * g.final_drive_rear * 0.5 * RPM;
        let wheel_rpm = if g.rear_split < 0.2 { front } else if g.rear_split > 0.8 { rear } else { (front + rear) * 0.5 };
        let top = g.ratios.len() - 1;
        let mut rpm = wheel_rpm * g.ratios[self.gear];
        match p.gear {
            -1 => {
                self.gear = self.gear.max(2);
                rpm = wheel_rpm * g.ratios[self.gear];
                while self.gear < top && rpm > self.up_rpm[self.gear] {
                    self.gear += 1;
                    rpm = wheel_rpm * g.ratios[self.gear];
                }
                while self.gear > 2 && rpm < self.down_rpm[self.gear] {
                    self.gear -= 1;
                    rpm = wheel_rpm * g.ratios[self.gear];
                }
            }
            0 => {
                self.gear = 0;
                rpm = wheel_rpm * g.ratios[0];
            }
            _ => {}
        }
        self.rpm = rpm.clamp(h.engine.idle_rpm, h.engine.max_rpm);
    }

    /// Bump stops (0x5A3E90): a wheel past its travel pushes along the
    /// ground normal at the hub by 0.2 × mass × the closing speed.
    fn bump_stops(
        &self,
        state: &CarState,
        h: &crate::handling::Handling,
        table: &[crate::car::Wheel; 4],
        frame: &BodyFrame,
        force: &mut Vec3,
        torque: &mut Vec3,
    ) {
        let BodyFrame { rot, com_local, omega, .. } = *frame;
        for i in 0..4 {
            if self.over[i] <= 0.0 {
                continue;
            }
            let arm = rot * (table[i].position - com_local + Vec3::Z * self.compression[i]);
            let closing = -self.normal[i].dot(state.velocity + omega.cross(arm));
            if closing > 0.0 {
                apply(force, torque, self.normal[i] * closing * BUMP * h.mass, arm);
            }
        }
    }

    /// Tyres (0x5A38F0). At a wheel on the ground the contact's speed
    /// in the plane (yaw only: v + ω.z × arm) splits along and across
    /// the wheel (the heading, plus the steer for the fronts); the
    /// longitudinal slip is that less radius × spin. The force is
    /// −gain × load × slip × 1.25 each way, the gain 1 up to 0.05 m/s of
    /// slip and the skill grip past it; turned back by the steer into
    /// car space, with the load as its up part, then into the world;
    /// its horizontal part is held within 4 × load, and the load's own
    /// slope part is taken off: all of it with the wheel locked or the
    /// brake over 0.4 at a standstill (|v| < 0.1), else |left.xy| of it
    /// while under the cap. Applied at the wheel's arm.
    fn tyre_forces(
        &mut self,
        state: &CarState,
        table: &[crate::car::Wheel; 4],
        p: &Pedals,
        frame: &BodyFrame,
        force: &mut Vec3,
        torque: &mut Vec3,
    ) {
        let BodyFrame { rot, fwd, left, up, com_local, omega, .. } = *frame;
        let heading = fwd.y.atan2(fwd.x);
        for i in 0..4 {
            let w = &table[i];
            let arm = rot * (w.position - com_local);
            let steer = if i < 2 { p.steer } else { 0.0 };
            let (mut fx, mut fy) = (0.0, 0.0);
            if self.contact[i] {
                let vx = state.velocity.x - arm.y * omega.z;
                let vy = arm.x * omega.z + state.velocity.y;
                let (sa, ca) = (heading + steer).sin_cos();
                let lat = ca * vy - sa * vx;
                let lon = ca * vx + sa * vy - w.radius * self.spin[i];
                self.slip[i] = (lon, lat);
                let s = lon.hypot(lat);
                let gain = if TYRE_SOFT * self.grip < s { self.skill_grip * self.grip } else { 1.0 };
                self.skid[i] = s > 6.0;
                let along = -(gain * self.load[i] * lon * TYRE_GAIN);
                let across = -(gain * self.load[i] * lat * TYRE_GAIN);
                let (ss, cs) = steer.sin_cos();
                fx = along * cs - across * ss;
                fy = across * cs + along * ss;
            } else {
                self.skid[i] = false;
            }
            let mut f = rot * Vec3::new(fx, fy, self.load[i]);
            let (cx, cy) = (fx, fy);
            let scale = 50.0 / cx.hypot(cy).max(50.0);
            self.tyre_force[i] = (cx * scale, cy * scale);
            if self.contact[i] {
                let horizontal = f.x.hypot(f.y);
                let cap = TYRE_CAP * self.load[i] * self.grip;
                if cap < horizontal {
                    f.x *= cap / horizontal;
                    f.y *= cap / horizontal;
                }
                let lift = self.load[i] / up.z;
                if self.locked[i] || (p.brake > 0.4 && state.velocity.length() < 0.1) {
                    f.x -= lift * up.x;
                    f.y -= lift * up.y;
                } else if horizontal < cap {
                    f.x -= left.x.abs() * up.x * lift;
                    f.y -= left.y.abs() * up.y * lift;
                }
            }
            apply(force, torque, f, arm);
        }
    }

    /// The chassis' derivative (slot +0x18, 0x598AD0): each wheel's spin
    /// rate. A locked wheel has none. The engine's torque at rpm / the
    /// catch-up k (+0x13C, 0x5A3390): full-throttle torque × gas less the
    /// closed-throttle drag × (1 − gas), 0 under 0.05 gas at idle; the
    /// drive is it × the axle's final drive × the gear's ratio × the axle's
    /// share of the split × ½. The brake −0.5 × brake × clamp(ω, ±20). The
    /// tyre's reaction is its car-space x force × the lever at the body's
    /// speed (`REACTION`, ÷ 1.25 on a driven wheel). The rate is (drive +
    /// brake − reaction) / 0.008. Nitrous (+0x198) is never on for
    /// traffic.
    fn spin_rates(&self, h: &crate::handling::Handling, speed: f32, p: &Pedals) -> [f32; 4] {
        let g = &h.gearbox;
        let e = &h.engine;
        let lever = crate::drive::graph(&REACTION, speed);
        // Chassis +0x13C = k (0x5A3390): the drive × (1 + 3(k − 1)) and the
        // driven lever's 1.25 × (1 + 2.5(k − 1)) (0x598C6E..0x598CB7).
        let k = p.scale;
        let (drive_scale, driven_lever) = if k == 1.0 { (1.0, 1.25) } else { (1.0 + 3.0 * (k - 1.0), (1.0 + 2.5 * (k - 1.0)) * 1.25) };
        let at = self.rpm / k;
        std::array::from_fn(|i| {
            if self.locked[i] {
                return 0.0;
            }
            let share = if i < 2 { 1.0 - g.rear_split } else { g.rear_split };
            let mut engine = e.torque.at(at) * p.gas - e.braking.at(at) * (1.0 - p.gas);
            if p.gas < IDLE_GAS && self.rpm == e.idle_rpm {
                engine = 0.0;
            }
            let final_drive = if i < 2 { g.final_drive_front } else { g.final_drive_rear };
            let drive = final_drive * g.ratios[self.gear] * engine * share * 0.5;
            let brake = BRAKE_GAIN * p.brake * self.spin[i].clamp(-BRAKE_SPIN, BRAKE_SPIN);
            let reaction = if share > 0.0 { lever / driven_lever } else { lever };
            (drive_scale * drive + brake - reaction * self.tyre_force[i].0) / WHEEL_INERTIA
        })
    }
}

/// The body's pose at the start of a MeloMvr sub-step, shared by its
/// stages.
#[derive(Clone, Copy)]
struct BodyFrame {
    rot: Mat3,
    fwd: Vec3,
    left: Vec3,
    up: Vec3,
    com: Vec3,
    com_local: Vec3,
    omega: Vec3,
}

/// Every force goes through the body's caps (0x59BB10).
fn apply(force: &mut Vec3, torque: &mut Vec3, f: Vec3, arm: Vec3) {
    let f = f.clamp_length_max(1200.0);
    *force += f;
    *torque += arm.cross(f).clamp_length_max(100.0);
}

/// Aerodynamics (0x59E630, the physics mover's function on the
/// MeloMvr's aero object): drag (2 − t)·c·(|v| ÷ k)·v × the surfaces'
/// factor × the aero factor × (1 − draft) (aero +0x0C = 1 − car
/// +0x4DC, 0x5A3390), at the centre of mass or, with two or more
/// wheels down, 0.1·(1 − t) m below it; downforce shape × tilt ×
/// |v| ÷ k × the aero factor, with a wheel down the front value 1 m
/// ahead and the rear 1 m behind, with none 40 % / 41 % of the sum
/// (tilt 1 with two or more down, else max(up.z, 0)).
///
/// Returns |v| as the drag read it.
fn aero_forces(
    state: &CarState,
    h: &crate::handling::Handling,
    p: &Pedals,
    frame: &BodyFrame,
    force: &mut Vec3,
    torque: &mut Vec3,
) -> f32 {
    let BodyFrame { fwd, up, .. } = *frame;
    let speed_now = state.velocity.length();
    let (drag, drag_arm) = aero_drag(state.velocity, h.drag * p.surface_drag * p.aero * (1.0 - state.draft), p.throttle, p.scale, p.down_share);
    apply(force, torque, drag, drag_arm);
    if speed_now > 1e-4 && p.aero > 0.0 {
        let along = (state.velocity / speed_now).dot(fwd).max(0.0);
        let shape = if h.race.drift() { along.powi(4) } else { (along * along).max(0.4) };
        let tilt = if p.down_share >= 0.5 { 1.0 } else { up.z.max(0.0) };
        let k = shape * tilt * speed_now / p.scale * p.aero;
        let (front, rear) = if p.down_share == 0.0 {
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

/// The rigid body's own terms (0x59C1F0) and the Euler step, as the
/// physics mover's: the top-speed limiter, the angular damping, the
/// rest hold (body +0x150 = 1 outside crash mode, 0x5AAE6F). The rows
/// take the Euler step row += h·(ω × row) and ω comes from them before
/// they are cleaned. Returns the stepped rows and the new
/// centre of mass.
#[allow(clippy::too_many_arguments)]
fn integrate(
    state: &mut CarState,
    h: &crate::handling::Handling,
    p: &Pedals,
    rot: Mat3,
    com: Vec3,
    mut force: Vec3,
    torque: Vec3,
    dt: f32,
) -> (Mat3, Vec3) {
    let fwd = rot * Vec3::X;
    // 0x59C1F0 reads |ω| and |v| (+0xC0, +0xA0) as the derivative is taken.
    let spin_now = state.spin.length();
    let speed_now = state.velocity.length();
    let speed_xy = state.velocity.truncate().length() * 2.2360249;
    let slope = fwd.z.abs().min(0.3);
    let grade = if fwd.z <= 0.0 { 1.0 + slope * 0.3333334 } else { 1.0 - slope * 0.3333334 };
    if h.top_speed_mph != 0.0 && grade * (h.top_speed_mph * p.scale + 1.9) < speed_xy && fwd.dot(force) > 0.0 {
        let mut local = rot.transpose() * force;
        local.x = (-slope * 5.0).min(-0.5);
        force = rot * local;
    }
    let damping = if state.crash { 0.0 } else { crate::drive::sample(&crate::drive::ANGULAR_DAMPING, 0.0, 10.0, spin_now) };
    let mut dl = torque - state.angular_momentum * damping;
    let settle = (speed_now / 4.0).max(spin_now / 2.0);
    let rest_rate = if crate::drive::sample(&crate::drive::REST_DAMPING, 0.0, 1.0, settle) <= state.rest_damping { -10.0 } else { 0.1 };
    let mut dv = force / h.mass;
    if p.holding {
        dl -= state.angular_momentum * (2.0 * state.rest_damping);
        dv -= state.velocity * (8.0 * state.rest_damping);
    }
    if !dv.is_finite() {
        dv = Vec3::ZERO;
    }
    let new_com = com + state.velocity * dt;
    let stepped = crate::drive::rigid::euler_rows(rot, state.spin, dt);
    state.velocity += dv * dt;
    state.angular_momentum += dl * dt;
    state.rest_damping = (state.rest_damping + rest_rate * dt).max(0.0);
    if state.velocity.length() > 200.0 && h.mass > 0.7 {
        state.velocity = state.velocity.clamp_length_max(200.0);
    }
    let inertia = crate::drive::inertia_of(h, state.ground_inertia);
    let inv = Vec3::new(1.0 / inertia.x, 1.0 / inertia.y, 1.0 / inertia.z);
    state.spin = crate::drive::rigid::spin_from_rows(stepped, inv, state.angular_momentum);
    (stepped, new_com)
}

/// The drag of 0x59E630 (0x59E677..0x59E6AC): −(2 − t)·c·(|v| ÷ k)·v,
/// `c` the record's drag × the surfaces' factor × the aero factor × (1 −
/// draft), `t` aero +0x10 (the throttle), `k` aero +0x08 (the catch-up);
/// applied at the centre of mass, or with aero +0x14 (the wheels down ×
/// 0.25) at least 0.5 at (0, 0, −0.1·(1 − t)) (0x7FBED4). Returns the force
/// and its arm.
fn aero_drag(velocity: Vec3, c: f32, t: f32, k: f32, down_share: f32) -> (Vec3, Vec3) {
    let arm = if down_share >= 0.5 { Vec3::new(0.0, 0.0, -0.1 * (1.0 - t)) } else { Vec3::ZERO };
    (-velocity * (velocity.length() / k) * c * (2.0 - t), arm)
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    /// The MeloMvr hands 0x59E630 the driver's gas as the throttle, so
    /// a car on full gas has half the drag the clone gave it (it took
    /// t = 0); the catch-up k divides the speed (0x5A3390 → aero +0x08).
    #[test]
    fn melo_drag_takes_the_gas_and_the_catch_up() {
        let v = Vec3::new(40.0, 0.0, 0.0);
        let (coast, arm0) = super::aero_drag(v, 0.5, 0.0, 1.0, 1.0);
        let (full, arm1) = super::aero_drag(v, 0.5, 1.0, 1.0, 1.0);
        assert!((coast.x + 2.0 * 0.5 * 40.0 * 40.0).abs() < 1e-3, "{coast}");
        assert!((full.x * 2.0 - coast.x).abs() < 1e-3, "{full} vs {coast}");
        assert_eq!(arm0, Vec3::new(0.0, 0.0, -0.1));
        assert_eq!(arm1, Vec3::ZERO);
        // One wheel down: at the centre of mass.
        assert_eq!(super::aero_drag(v, 0.5, 0.0, 1.0, 0.25).1, Vec3::ZERO);
        let (caught, _) = super::aero_drag(v, 0.5, 1.0, 1.25, 1.0);
        assert!((caught.x * 1.25 - full.x).abs() < 1e-3);
    }
}

/// What the frame's set-up hands every sub-step.
struct Pedals {
    steer: f32,
    gas: f32,
    brake: f32,
    gear: i32,
    gravity: f32,
    aero: f32,
    surface_drag: f32,
    holding: bool,
    /// Aero +0x10: the driver's gas (the drag's throttle t).
    throttle: f32,
    /// Aero +0x14: the wheels down × 0.25.
    down_share: f32,
    /// k of 0x5A3390 (car +0x468).
    scale: f32,
}
