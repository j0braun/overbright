//! The scene car mover (mode 5, NISMover): the target setter 0x5A40B0, the
//! target application 0x5A44B0 (separate momentum and cached velocity are
//! intentional: the impulse branch refreshes neither cache) and the mover's
//! frame 0x5AB3D0, driving the race-start scene cars (docs/original/nis-mover.md).
use glam::{Mat3, Quat, Vec2, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct TargetBody {
    pub position: Vec3,
    pub orientation: Mat3,
    pub velocity: Vec3,
    pub spin: Vec3,
    pub momentum: Vec3,
    pub angular_momentum: Vec3,
    pub mass: f32,
    pub base_inverse_inertia: Vec3,
    pub selected_inverse_inertia: Vec3,
}

/// `heading_degrees` is the mover's stored target heading; dt is positive
/// frame time. `impulse` corresponds to the native placement counter > 3.
pub fn apply_target(
    body: &mut TargetBody,
    target: Vec3,
    heading_degrees: f32,
    dt: f32,
    impulse: bool,
    global_xy: Vec2,
) {
    // Live career NISMover entries use x87 PC24/nearest. Each arithmetic
    // operation rounds to a 24-bit significand even without an f32 store.
    let inverse_dt = 1.0f32 / dt;
    let desired = Vec3::new(
        (target.x - body.position.x) * inverse_dt,
        (target.y - body.position.y) * inverse_dt,
        0.0,
    );
    let forward = body.orientation.x_axis;
    let heading = f32::from(crate::ai::driver::angle16(forward.x, forward.y)) * (360.0 / 65536.0);
    let mut error = heading_degrees - heading;
    if error < -180.0 {
        error += 360.0;
    }
    if error > 180.0 {
        error -= 360.0;
    }
    let yaw = error * inverse_dt * f32::from_bits(0x3c8efa35);
    if impulse {
        let correction = Vec3::new(
            (desired.x - body.velocity.x) * body.mass,
            (desired.y - body.velocity.y) * body.mass,
            0.0,
        );
        let correction = capped(correction, 1000.0);
        body.momentum += correction;
        let moment = capped(Vec3::new(0.0, 0.0, -0.2).cross(correction), 10.0);
        body.angular_momentum += moment;
        body.angular_momentum.x += global_xy.x;
        body.angular_momentum.y += global_xy.y;
        body.angular_momentum.z +=
            (yaw - body.spin.z) / body.selected_inverse_inertia.z;
    } else {
        body.velocity.x = desired.x;
        body.velocity.y = desired.y;
        body.spin.z = yaw;
        body.momentum = body.velocity * body.mass;
        body.angular_momentum = body.orientation
            * ((body.orientation.transpose() * body.spin) / body.base_inverse_inertia);
    }
}

// 0x59BC70 evaluates the norm in x87, then stores scaled components.
fn capped(vector: Vec3, maximum: f32) -> Vec3 {
    let [x, y, z] = vector.to_array();
    let length = ((y * y + z * z) + x * x).sqrt();
    if length > maximum { vector * (maximum / length) } else { vector }
}


// ---------------------------------------------------------------------
// The scene car mover (mode 5), end to end. Native chain per scene frame:
// NIS_UpdateCars 0x439240 -> NIS_PlaceSceneCar 0x4309D0 -> target setter
// 0x5A40B0 (this file's `NisMover::set_target`), then in the world step the
// mover frame 0x5AB3D0 (`CarState::step_nis`): preload 0x5A29B0/0x598A00,
// target application 0x5A44B0 (`apply_target`), 1/70 s substeps of explicit
// Euler 0x5AA490 with the rigid body's derivative 0x59C1F0, the NIS chassis
// post 0x5AB890/0x5AB1A0/0x5A3710 (wheel spin overwrite, suspension) and
// the NIS force pass 0x5AB140 -> 0x5AB550 (tyre loads along the body's up
// only: the x/y tyre forces are zeroed at 0x5AB806) -> 0x5A46B0 (authored
// body torque). See docs/original/nis-mover.md for what is traced and what is not.

use super::{CarState, GRAVITY, ANGULAR_DAMPING, REST_DAMPING, sample, inertia_of};

/// A scene that holds the cars runs the game at 1/30 s frames at least
/// (0x437910 -> 0x435F40; the clone's `display::SCENE_FRAME` limiter), so the
/// native mover never sees a shorter frame. Callers stepping faster (tests,
/// no limiter) let the time build up to one scene frame first: a shorter
/// target interval would turn the (target - body)/dt velocity of 0x5A44B0
/// into a stiff, oscillating correction. The shops' scenes, not under the
/// limiter here, keep the 30 fps run's steps instead ([`ScenePace`]).
pub const SCENE_FRAME: f32 = 1.0 / 30.0;

/// Native substep bound float[0x7FC040].
pub const SUBSTEP: f32 = 0.014285714365541935;

/// The 30 fps schedule of a scene car whose scene runs at the display's
/// rate (the shops' drive-in / drive-out: docs/playing/native-deviations.md "High
/// refresh rates"; the original runs every scene under its 1/30 s limiter).
/// The mover takes the steps a 30 fps run takes, whatever the world's
/// updates: each step's frame time from the limiter's clock
/// ([`FrameClock::frame_limited`] at 1/30 s: 60, 60, 60, 60, 61 ticks of
/// 1/1800 s, as the original's sim clock counts them in the drive-in) and its
/// target at the scene frame those steps reach, so the path per step is a
/// function of the scene alone. A step is taken once the world's scene
/// time reaches the start of the frame it ends (one step ahead), so the car
/// can be drawn between its last two steps at the world's time
/// ([`ScenePace::drawn`]) and moves on every drawn frame.
///
/// [`FrameClock::frame_limited`]: crate::frame_clock::FrameClock::frame_limited
#[derive(Clone, Copy, Debug)]
pub struct ScenePace {
    /// The 1/30 s limiter's clock the steps' frame times come from.
    pub clock: crate::frame_clock::FrameClock,
    /// The scene frame the schedule starts at.
    pub base: f32,
    /// Ticks (1/1800 s) of the steps taken so far.
    pub ticks: i64,
    /// Ticks of the newest step.
    pub span: i64,
    /// Ticks of the newest step still ahead of the world's scene time.
    pub ahead: i64,
    /// The drawn pose before the newest step.
    pub before: crate::interpolate::CarPose,
}

impl ScenePace {
    /// A schedule starting at scene frame `base`, the body at `state`.
    pub fn new(base: f32, state: &CarState) -> Self {
        ScenePace {
            clock: Default::default(),
            base,
            ticks: 0,
            span: 0,
            ahead: 0,
            before: crate::interpolate::CarPose::of(state),
        }
    }

    /// The world's scene time moved on by `dt` (whole ticks of 1/1800 s).
    pub fn advance(&mut self, dt: f32) {
        self.ahead -= (f64::from(dt) * 1800.0).round() as i64;
    }

    /// The next step when one is due: its frame time and the scene frame
    /// it targets (the scene clock moves first, then the mover, as in a
    /// 30 fps frame).
    pub fn due(&mut self) -> Option<(f32, f32)> {
        if self.ahead > 0 {
            return None;
        }
        let (dt, ticks) = self.clock.frame_limited(crate::frame_clock::RAW_30, SCENE_FRAME);
        self.ticks += i64::from(ticks);
        self.span = i64::from(ticks);
        self.ahead += self.span;
        Some((dt, self.base + self.ticks as f32 / 60.0))
    }

    /// How far the world's scene time is through the newest step (0..1).
    pub fn fraction(&self) -> f32 {
        if self.span <= 0 { 1.0 } else { (1.0 - self.ahead as f32 / self.span as f32).clamp(0.0, 1.0) }
    }

    /// The car (`state`, after the newest step) as drawn at the world's
    /// scene time: between the pose before that step and after it.
    pub fn drawn(&self, state: &CarState) -> crate::interpolate::CarPose {
        let now = crate::interpolate::CarPose::of(state);
        crate::interpolate::CarPose::blend(&self.before, &now, self.fraction(), self.span as f32 / 1800.0).unwrap_or(now)
    }
}

/// Mover state kept between frames: mover +0x60..+0x94 and chassis +0x130,
/// plus the body's force/torque accumulators (+0x110/+0x120) and the NIS
/// suspension's loads/contact flags (chassis +0x50/+0x60).
#[derive(Clone, Copy, Debug, Default)]
pub struct NisMover {
    /// Placement count, the scene slot's +0x0C (0x827BA4), stored at +0x60.
    pub count: i32,
    /// Smoothed target +0x70 and last incoming target +0x80.
    pub target: Vec3,
    pub history: Vec3,
    /// Target heading in degrees (+0x90).
    pub heading: f32,
    /// Changed-target flag +0x94 (cleared by the frame after application).
    pub pending: bool,
    /// Front steer as a 16-bit angle (chassis +0x130).
    pub steer: u16,
    /// The scene's nitrous flame intensity for this car (car effect 10,
    /// `shop_scene::wheels::effect`), 0 when off; for the flame emitter.
    pub nitro: f32,
    /// Frame time not yet given to the mover (see [`SCENE_FRAME`]).
    pub elapsed: f32,
    /// The shop drive-in / drive-out scenes' own 1/30 s schedule
    /// ([`ScenePace`]); None where the frames given are the scene's frames.
    pub pace: Option<ScenePace>,
    force: Vec3,
    torque: Vec3,
    loads: [f32; 4],
    contact: [bool; 4],
    primed: bool,
}

/// One scene frame's authored controls for the car (event types 4/5/6,
/// chassis +0x250 brake multiplier, +0x254 burnout additive, +0x258/+0x25C
/// body torque).
pub type NisControls = crate::shop_scene::wheels::Controls;

/// 0x401090: clamp(x, lo, hi) as max-then-min.
fn clamp_native(x: f32, lo: f32, hi: f32) -> f32 {
    let v = if x <= lo { lo } else { x };
    if hi < v { hi } else { v }
}

const ANGLE16_TO_DEG: f32 = 0.005493164; // float[0x784248]

fn heading_degrees(state: &CarState) -> f32 {
    let f = state.forward();
    f32::from(crate::ai::driver::angle16(f.x, f.y)) * ANGLE16_TO_DEG
}

impl NisMover {
    /// 0x4309D0 + 0x5A40B0 for one scene frame. `at` is the scene channel's
    /// x / y through the origin (the body's centre of mass, body +0x20),
    /// `heading16` the turned forward axis' 0x43E6D0 angle, `frame_dt`
    /// g_FrameTime 0x865150. Returns whether the car was teleported
    /// (0x5FBD70: first placement, or more than 1.1 m / 15 degrees off).
    pub fn set_target(&mut self, state: &mut CarState, car: &crate::car::Car, world: &crate::world::World, at: Vec2, heading16: u16, frame_dt: f32) -> bool {
        let count = self.count;
        let mut teleported = false;
        // 0x4309D0: the slot's count 0 places the car first.
        if count == 0 {
            place(state, car, world, at, heading16);
            teleported = true;
            self.primed = false;
        }
        let rot = Mat3::from_quat(state.orientation);
        let com = state.position + rot * Vec3::from(car.car_type.handling.centre_of_mass);
        let incoming = Vec3::new(at.x, at.y, com.z);
        let target_heading = f32::from(heading16) * ANGLE16_TO_DEG;
        let current = heading_degrees(state);
        // 0x5A40DC..: the wrapped heading error.
        let mut error = target_heading - current;
        if -error > 180.0 {
            error = (360.0 - current) + target_heading;
        } else if -error < -180.0 {
            error = (target_heading - 360.0) - current;
        }
        let d = incoming.truncate() - com.truncate();
        if d.length() > 1.1 || error.abs() > 15.0 {
            place(state, car, world, at, heading16);
            self.heading = target_heading;
            self.target = incoming;
            self.history = incoming;
            teleported = true;
            self.primed = false;
        } else {
            // 0x43D190 with epsilon 0.001 (float 0x3A83126F) against +0x70.
            let same = (self.target - incoming).abs().max_element() <= 0.001;
            if !same || self.heading != target_heading {
                self.pending = true;
                self.heading = target_heading;
                self.target = if count < 4 { incoming } else { self.history * 0.25 + incoming * 0.75 };
                self.history = incoming;
                // Steering: heading error x 25 within 65 degrees/s of the
                // current steer; above 3.2 m/s the unsigned angle between
                // the car's forward axis and its velocity (0x4010E0, the
                // arccosine table) within the same bounds; then +-45.
                let mut current_steer = f32::from(self.steer) * ANGLE16_TO_DEG;
                if current_steer >= 180.0 {
                    current_steer -= 360.0;
                }
                let lo = current_steer - frame_dt * 65.0;
                let hi = frame_dt * 65.0 + current_steer;
                let mut steer = clamp_native(error * 25.0, lo, hi);
                let v = state.velocity;
                if v.length() > 3.2 {
                    let f = state.forward();
                    let c = f.dot(v) / (v.length() * f.length());
                    let a = 0x4000u16.wrapping_sub(crate::ai::driver::asin16(c));
                    steer = clamp_native(f32::from(a) * ANGLE16_TO_DEG, lo, hi);
                }
                let steer = clamp_native(steer, -45.0, 45.0);
                let scaled = (steer * 65536.0) as i32;
                self.steer = (scaled / 360) as u16;
            }
        }
        self.count = count + 1;
        teleported
    }

    /// Suspension loads (chassis +0x50) and contact flags (+0x60).
    pub fn loads(&self) -> ([f32; 4], [bool; 4]) {
        (self.loads, self.contact)
    }

    /// The front wheels' steer in degrees for the clone's wheel draw.
    pub fn steer_degrees(&self) -> f32 {
        f32::from(self.steer as i16) * ANGLE16_TO_DEG
    }
}

/// 0x5FBD70 (via 0x5F52D0): the car put at the pose on the ground, at rest.
/// The clone's `CarState::placed` drops the wheels the same way as the
/// race-start placement; wheel phases, drivetrain and controller state
// prov: inferred drive.nis.place
/// stay (**inferred**: the controller resets 0x42CDC0/0x40D550 that the
/// native teleport also runs are not applied, so a rival keeps its route).
fn place(state: &mut CarState, car: &crate::car::Car, world: &crate::world::World, at: Vec2, heading16: u16) {
    let heading = f32::from(heading16) * (std::f32::consts::TAU / 65536.0);
    // 0x4309D0 passes the car's own height (+0x68), not the channel's: the
    // wheels are dropped from there (a bridge or roof above the scene must
    // not catch them).
    let com = state.position + Mat3::from_quat(state.orientation) * Vec3::from(car.car_type.handling.centre_of_mass);
    let placed = CarState::placed(&car.car_type, world, Vec3::new(at.x, at.y, com.z), heading);
    state.position = placed.position;
    state.orientation = placed.orientation;
    state.velocity = Vec3::ZERO;
    state.spin = Vec3::ZERO;
    state.angular_momentum = Vec3::ZERO;
    state.rest_damping = 0.0;
}

impl CarState {
    /// The NIS mover's world-step frame 0x5AB3D0 for `dt` seconds.
    pub fn step_nis(&mut self, car: &crate::car::Car, world: &crate::world::World, mover: &mut NisMover, controls: NisControls, dt: f32) {
        if !(dt > 0.0) {
            return;
        }
        let h = &car.car_type.handling;
        let table = &car.car_type.wheels;
        // 0x5AB3DE..0x5AB414: rest damping and angular damping on (body
        // +0x150/+0x154 = 1); a selected ground inertia (+0x158) is left,
        // keeping the yaw rate (+0x88 divided by the ground ratio).
        if self.ground_inertia {
            self.angular_momentum.z /= h.unknown_270;
            self.ground_inertia = false;
        }
        let inertia = inertia_of(h, false);
        let com_local = Vec3::from(h.centre_of_mass);
        // 0x5A29B0 -> 0x598A00: the suspension preloads, front (0.5 - share)
        // and rear share of the weight plus the downforce preload.
        let front_x = table[0].position.x;
        let rear_x = table[3].position.x;
        let rear_share = 0.5 * (front_x - com_local.x) / (front_x - rear_x);
        let supported = (h.downforce[0] + h.downforce[1]) * self.exported_forward_speed().max(0.0) * h.preload_downforce + h.mass * GRAVITY;
        let preload = [supported * (0.5 - rear_share), supported * (0.5 - rear_share), supported * rear_share, supported * rear_share];

        let rot = Mat3::from_quat(self.orientation);
        let mut body = TargetBody {
            position: self.position + rot * com_local,
            orientation: rot,
            velocity: self.velocity,
            spin: self.spin,
            momentum: self.velocity * h.mass,
            angular_momentum: self.angular_momentum,
            mass: h.mass,
            base_inverse_inertia: Vec3::ONE / inertia,
            selected_inverse_inertia: Vec3::ONE / inertia,
        };
        // 0x5AB4BD: the stored target, count > 3 selecting the impulses.
        apply_target(&mut body, mover.target, mover.heading, dt, mover.count > 3, Vec2::ZERO);
        mover.pending = false;
        if !mover.primed {
            // First frame on the mover: its suspension and forces from the
            // pose as placed (the constructor's 0x5A4420 at 0x5B72BB).
            self.nis_post(car, world, mover, &body, controls, preload, 0.0);
            mover.primed = true;
        }
        let steps = (dt / SUBSTEP) as i32 + 1;
        let sub = dt / steps as f32;
        for _ in 0..steps {
            // 0x5AA490: derivatives of every component first, then Euler.
            let spin_now = body.spin.length();
            let speed_now = body.velocity.length();
            let mut dp = mover.force;
            let mut dl = mover.torque;
            // 0x59C1F0, +0x154: angular damping by the spin's table.
            dl -= body.angular_momentum * sample(&ANGULAR_DAMPING, 0.0, 10.0, spin_now);
            let settle = (speed_now / 4.0).max(spin_now / 2.0);
            let rest_rate = if sample(&REST_DAMPING, 0.0, 1.0, settle) <= self.rest_damping { -10.0 } else { 0.1 };
            // +0x150: rest damping on the momenta.
            dl -= body.angular_momentum * (self.rest_damping * 2.0);
            dp -= body.momentum * (self.rest_damping * 8.0);
            if self.yaw_limit != 0.0 {
                dl.z = dl.z.clamp(-self.yaw_limit, self.yaw_limit);
            }
            body.position += body.velocity * sub;
            let w = body.spin;
            let skew = Mat3::from_cols(Vec3::new(0.0, w.z, -w.y), Vec3::new(-w.z, 0.0, w.x), Vec3::new(w.y, -w.x, 0.0));
            body.orientation += (skew * body.orientation) * sub;
            body.momentum += dp * sub;
            body.angular_momentum += dl * sub;
            self.rest_damping = (self.rest_damping + rest_rate * sub).max(0.0);
            // 0x5BB0E0 after each substep: orthonormal axes.
            body.orientation = orthonormal(body.orientation);
            body.velocity = body.momentum / h.mass;
            if body.velocity.length() > 200.0 {
                body.velocity = body.velocity.clamp_length_max(200.0);
                body.momentum = body.velocity * h.mass;
            }
            let r = body.orientation;
            body.spin = (r * ((r.transpose() * body.angular_momentum) / inertia)).clamp_length_max(250.0);
            self.nis_post(car, world, mover, &body, controls, preload, sub);
        }
        self.orientation = Quat::from_mat3(&body.orientation).normalize();
        let rot = Mat3::from_quat(self.orientation);
        self.position = body.position - rot * com_local;
        self.velocity = body.velocity;
        self.spin = body.spin;
        self.angular_momentum = body.angular_momentum;
        self.steer_degrees = mover.steer_degrees();
        // 0x5A2B70: the car's exported speed and wheel states.
        self.publish_forward_speed();
    }

    /// The component posts (0x5AB890 -> 0x5AB1A0 -> 0x5A3710: suspension
    /// compression and load, then the wheel spins overwritten from the
    /// body's forward speed and the authored burnout) and the force pass for
    /// the next substep (0x5AB550: load x body up at each mount, the slip
    /// diagnostics and skid flag; 0x5A46B0 authored torque; gravity).
    fn nis_post(&mut self, car: &crate::car::Car, world: &crate::world::World, mover: &mut NisMover, body: &TargetBody, controls: NisControls, preload: [f32; 4], sub: f32) {
        let h = &car.car_type.handling;
        let table = &car.car_type.wheels;
        let rot = body.orientation;
        let up = rot.z_axis;
        let com_local = Vec3::from(h.centre_of_mass);
        let origin = body.position - rot * com_local;
        let forward_speed = body.velocity.dot(rot.x_axis);
        let heading = f32::from(crate::ai::driver::angle16(rot.x_axis.x, rot.x_axis.y)) * 9.58738e-5;
        let steer = f32::from(mover.steer) * 9.58738e-5;
        let mut force = Vec3::ZERO;
        let mut torque = Vec3::ZERO;
        let apply = |force: &mut Vec3, torque: &mut Vec3, f: Vec3, arm: Vec3| {
            let f = f.clamp_length_max(1200.0);
            *force += f;
            *torque += arm.cross(f).clamp_length_max(100.0);
        };
        // Gravity (the body's tidy-up, +0x160 = 9.8).
        apply(&mut force, &mut torque, Vec3::new(0.0, 0.0, -h.mass * GRAVITY), Vec3::ZERO);
        for i in 0..4 {
            let spec = &table[i];
            let s = &h.suspension[i / 2];
            let mount = self.mount(spec.position, i);
            let rest = origin + rot * mount;
            // 0x598D60: the ground under the mount, 0x5A3710: compression
            // (ground z - mount z) / (normal . up) + radius, clamped.
            let ground = world.ground_for(&mut self.ground, rest);
            let raw = match ground {
                Some((z, n, _)) if n.dot(up) >= 0.001 => (z - rest.z) / n.dot(up),
                _ => -100.0,
            } + spec.radius;
            let wheel = &mut self.wheels[i];
            let old = wheel.compression;
            let (x, contact) = if raw < s.max_extension {
                (s.max_extension, false)
            } else {
                (raw.min(s.max_compression), true)
            };
            if let Some((_, _, surface)) = ground {
                wheel.surface = surface;
            }
            // A zero-length priming pass keeps no rate (**inferred**).
            let rate = if sub > 0.0 { (x - old) / sub.max(0.0001) } else { 0.0 };
            wheel.compression = x;
            wheel.compression_rate = rate;
            let load = if contact {
                ((x.max(0.0) * s.progression + 1.0) * s.spring * 0.9 * x + if rate <= 0.0 { s.rebound } else { s.bump } * 0.9 * rate + preload[i]).max(0.0)
            } else {
                0.0
            };
            wheel.on_ground = contact;
            wheel.load = load;
            // 0x5AB890: spin from the forward speed and the burnout.
            let spin = controls.spin(forward_speed, spec.radius, h.gearbox.rear_split, i);
            wheel.spin = spin;
            wheel.roll = (wheel.roll + spin * sub) % std::f32::consts::TAU;
            wheel.spinner_rate += (spin - wheel.spinner_rate) * (0.0035 / 0.0225) * sub;
            wheel.spinner = (wheel.spinner + wheel.spinner_rate * sub) % std::f32::consts::TAU;
            // 0x5AB550: slip at the hub, planar yaw only; skid above 6 m/s.
            let arm = rot * (spec.position - com_local);
            let hx = body.velocity.x - arm.y * body.spin.z;
            let hy = arm.x * body.spin.z + body.velocity.y;
            let angle = heading + if i < 2 { steer } else { 0.0 };
            let (sin, cos) = angle.sin_cos();
            let lateral = cos * hy - sin * hx;
            let longitudinal = cos * hx + sin * hy - spec.radius * spin;
            if contact {
                wheel.slip = -longitudinal;
                wheel.exported_slip = wheel.slip;
                wheel.lateral_slip = lateral;
                wheel.sliding = longitudinal.hypot(lateral) > 6.0;
            } else {
                wheel.slip = 0.0;
                wheel.exported_slip = 0.0;
                wheel.lateral_slip = 0.0;
                wheel.sliding = false;
            }
            mover.loads[i] = load;
            mover.contact[i] = contact;
            // The load along the body's up at the mount (x/y zeroed).
            apply(&mut force, &mut torque, up * load, arm);
        }
        // 0x5A46B0: authored body torque about forward / left, z untouched.
        let [first, second] = controls.torque;
        torque.x += first * rot.x_axis.x + second * rot.y_axis.x;
        torque.y += first * rot.x_axis.y + second * rot.y_axis.y;
        mover.force = force;
        mover.torque = torque;
    }
}

/// The body and wheel state a scene mover owns. The clone still runs the
/// held race-start physics step for the car's drivetrain, lights and AI;
/// the scene car's pose is put back before its mover runs, so that step
/// cannot move it (native: the car is on the NIS mover, not the physics one).
#[derive(Clone, Debug)]
pub struct Kinematics {
    position: Vec3,
    orientation: Quat,
    velocity: Vec3,
    spin: Vec3,
    angular_momentum: Vec3,
    rest_damping: f32,
    rest: u8,
    ground_inertia: bool,
    wheels: [super::WheelState; 4],
    steer_degrees: f32,
    tyre_speed: f32,
}

impl CarState {
    pub fn kinematics(&self) -> Kinematics {
        Kinematics {
            position: self.position,
            orientation: self.orientation,
            velocity: self.velocity,
            spin: self.spin,
            angular_momentum: self.angular_momentum,
            rest_damping: self.rest_damping,
            rest: self.rest,
            ground_inertia: self.ground_inertia,
            wheels: self.wheels,
            steer_degrees: self.steer_degrees,
            tyre_speed: self.tyre_speed,
        }
    }

    pub fn restore_kinematics(&mut self, k: &Kinematics) {
        self.position = k.position;
        self.orientation = k.orientation;
        self.velocity = k.velocity;
        self.spin = k.spin;
        self.angular_momentum = k.angular_momentum;
        self.rest_damping = k.rest_damping;
        self.rest = k.rest;
        self.ground_inertia = k.ground_inertia;
        self.wheels = k.wheels;
        self.steer_degrees = k.steer_degrees;
        self.tyre_speed = k.tyre_speed;
    }
}

/// 0x5BB0E0 (formats.md, rigid body integration): x normalised,
/// z = x × y normalised, y = z × x; one helper with the drive mover and
/// the MeloMvr (`super::rigid`: 0x43CF10's (1, 0, 0) for a zero
/// vector, z included).
fn orthonormal(m: Mat3) -> Mat3 {
    super::rigid::orthonormal(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn target_application_matches_64_pc24_snapshots_bitwise() {
        let raw = &crate::testkit::private("docs/evidence/nis-mover-step/target-pc24-native.bin");
        let mut index = 0;
        for rotated in [false, true] {
            for impulse in [false, true] {
                for ground in [false, true] {
                    for dt in [0.016, 0.1] {
                        for (heading, dx, dy) in [
                            (0.0, 0.02, -0.03),
                            (359.0, -0.1, 0.2),
                            (15.0, 2.0, -3.0),
                            (181.0, 20.0, -30.0),
                        ] {
                            let pre = &raw[index * 1536..index * 1536 + 768];
                            let post = &raw[index * 1536 + 768..(index + 1) * 1536];
                            let f = |bytes: &[u8], offset| {
                                f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
                            };
                            let v = |bytes: &[u8], offset| {
                                Vec3::new(
                                    f(bytes, offset),
                                    f(bytes, offset + 4),
                                    f(bytes, offset + 8),
                                )
                            };
                            let base = Vec3::new(f(pre, 0x170), f(pre, 0x184), f(pre, 0x198));
                            let mut body = TargetBody {
                                position: v(pre, 0x20),
                                orientation: Mat3::from_cols(
                                    v(pre, 0x30),
                                    v(pre, 0x40),
                                    v(pre, 0x50),
                                ),
                                velocity: v(pre, 0xa0),
                                spin: v(pre, 0xc0),
                                momentum: v(pre, 0x70),
                                angular_momentum: v(pre, 0x80),
                                mass: f(pre, 0x164),
                                base_inverse_inertia: base,
                                selected_inverse_inertia: if ground {
                                    Vec3::new(f(pre, 0x1b0), f(pre, 0x1c4), f(pre, 0x1d8))
                                } else {
                                    base
                                },
                            };
                            assert_eq!(
                                body.orientation.x_axis,
                                if rotated { Vec3::Y } else { Vec3::X }
                            );
                            apply_target(
                                &mut body,
                                Vec3::new(10.0 + dx, 20.0 + dy, 100.0),
                                heading,
                                dt,
                                impulse,
                                Vec2::new(0.0125, -0.025),
                            );
                            for (name, got, expected) in [
                                ("position", body.position, v(post, 0x20)),
                                ("velocity", body.velocity, v(post, 0xa0)),
                                ("spin", body.spin, v(post, 0xc0)),
                                ("momentum", body.momentum, v(post, 0x70)),
                                ("angular momentum", body.angular_momentum, v(post, 0x80)),
                            ] {
                                for component in 0..3 {
                                    assert_eq!(
                                        got[component].to_bits(),
                                        expected[component].to_bits(),
                                        "case{index} {name}[{component}]: {} != {}",
                                        got[component],
                                        expected[component]
                                    );
                                }
                            }
                            assert_eq!(
                                body.orientation,
                                Mat3::from_cols(v(post, 0x30), v(post, 0x40), v(post, 0x50))
                            );
                            index += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(index, 64);
        assert_eq!(raw.len(), index * 1536);
    }
}

/// Replays of the original's IntroNis13 race start (Wine capture,
/// the maintainer's private capture): each scene car's
/// incoming channel targets (mover +0x80) and authored controls drive the
/// clone's mover at the native 30 Hz scene rate on flat ground; the body's
/// planar track, forward speed and wheel spins must stay within a few
/// percent of the original's. Ignored without NFSU2_ROOT (needs a car).
#[cfg(test)]
mod replay {
    use super::*;
    use crate::car::Car;
    use crate::world::World;

    struct Row {
        car: usize,
        count: i32,
        com: Vec2,
        fwd: Vec3,
        up: Vec3,
        vel: Vec3,
        #[allow(dead_code)] // parsed for evidence parity; not asserted yet
        wheels: [f32; 4],
        brake: f32,
        additive: f32,
        torque: [f32; 2],
        incoming: Vec2,
        heading: f32,
    }

    fn rows() -> Vec<Row> {
        let text = &crate::testkit::private_str("docs/evidence/nis-mover/native-intro13-quickrace.tsv");
        text.lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| {
                let v: Vec<f32> = l.split('\t').map(|x| x.parse().unwrap()).collect();
                let v3 = |i: usize| Vec3::new(v[i], v[i + 1], v[i + 2]);
                Row {
                    car: v[1] as usize,
                    count: v[2] as i32,
                    com: Vec2::new(v[3], v[4]),
                    fwd: v3(6),
                    up: v3(9),
                    vel: v3(12),
                    wheels: [v[18], v[19], v[20], v[21]],
                    brake: v[27],
                    additive: v[28],
                    torque: [v[29], v[30]],
                    incoming: Vec2::new(v[33], v[34]),
                    heading: v[35],
                }
            })
            .collect()
    }

    fn peugeot() -> Car {
        let root = crate::testkit::game_root();
        Car::load_physics(&root, "PEUGOT", &[], &|_| false, None).unwrap()
    }

    struct Result {
        worst_track: f32,
        worst_speed: f32,
        burnout_front: f32,
        max_tilt: f32,
    }

    /// Runs scene car `n` from placement count 0 to the last capture.
    fn replay(car: &Car, rows: &[&Row]) -> Result {
        let world = World::flat(2000.0);
        let origin = Vec3::new(rows[0].com.x, rows[0].com.y, 0.0);
        // Mover +0x60 holds the count passed to the call (the slot's count
        // before its increment), so a sample's count is its call index.
        // Incoming targets per call index, linear between
        // captured samples and extrapolated back to the first placement.
        let mut unique: Vec<&Row> = Vec::new();
        for r in rows {
            if unique.last().is_none_or(|u| u.count != r.count) {
                unique.push(r);
            }
        }
        let at = |call: f32| -> (Vec2, f32) {
            let k = unique.iter().position(|r| r.count as f32 >= call).unwrap_or(unique.len() - 1).max(1);
            let (a, b) = (unique[k - 1], unique[k]);
            let t = (call - a.count as f32) / (b.count - a.count) as f32;
            let p = a.incoming.lerp(b.incoming, t) - origin.truncate();
            let mut dh = b.heading - a.heading;
            dh -= 360.0 * (dh / 360.0).round();
            (p, a.heading + dh * t)
        };
        let control = |call: f32| {
            let r = rows.iter().rev().find(|r| r.count as f32 <= call).copied().unwrap_or(rows[0]);
            NisControls { brake: r.brake, additive: r.additive, torque: r.torque }
        };
        let mut state = CarState::new(Vec3::ZERO, 0.0);
        let mut mover = NisMover::default();
        let dt = 1.0 / 30.0;
        let last = rows.last().unwrap().count + 2;
        let mut result = Result { worst_track: 0.0, worst_speed: 0.0, burnout_front: 0.0, max_tilt: 0.0 };
        let mut history: Vec<(Vec2, f32)> = Vec::new();
        for call in 0..last {
            let (p, heading) = at(call as f32);
            let h16 = (heading.rem_euclid(360.0) / ANGLE16_TO_DEG).round() as u32 as u16;
            mover.set_target(&mut state, car, &world, p, h16, dt);
            let controls = control(call as f32);
            state.step_nis(car, &world, &mut mover, controls, dt);
            let rot = Mat3::from_quat(state.orientation);
            let com = state.position + rot * Vec3::from(car.car_type.handling.centre_of_mass);
            result.max_tilt = result.max_tilt.max(rot.z_axis.truncate().length());
            let previous = history.last().copied();
            history.push((com.truncate(), state.speed()));
            // The poll is asynchronous with the game's frame: a sample's
            // count is the scene update's, and its body is either the world
            // step before that update (most samples: the com lags the
            // incoming target by one frame) or the one right after it. Each sample is scored
            // against the nearer of the clone's two bodies. Calls 0..2 have
            // no captured target before them (the first sample is count 2):
            // extrapolated, so not scored.
            for r in rows.iter().filter(|r| r.count == call && r.count >= 5) {
                let native = r.com - origin.truncate();
                // Native forward speed on its slope; the clone's ground is flat.
                let native_speed = r.vel.dot(r.fwd);
                let score = |(c, v): (Vec2, f32)| (c.distance(native), (v - native_speed).abs() / native_speed.abs().max(1.0));
                let (mut track, mut speed_error) = score((com.truncate(), state.speed()));
                if let Some(p) = previous {
                    let (t2, s2) = score(p);
                    if t2 < track {
                        (track, speed_error) = (t2, s2);
                    }
                }
                let speed = state.speed();
                if std::env::var_os("NIS_REPLAY_TRACE").is_some() {
                    eprintln!("n{} c{} native {:?} v{:.3} clone {:?} v{:.3} track {:.4} tgt {:?}", r.car, r.count, native, native_speed, com.truncate(), speed, track, mover.target.truncate());
                }
                result.worst_track = result.worst_track.max(track);
                result.worst_speed = result.worst_speed.max(speed_error);
                // Wheel spins follow 0x5AB890 from the native speed.
                let spec = &car.car_type.wheels;
                for i in 0..4 {
                    let expected = controls.spin(speed, spec[i].radius, car.car_type.handling.gearbox.rear_split, i);
                    assert!((state.wheels[i].spin - expected).abs() < 1e-3);
                }
                if r.additive > 0.0 {
                    result.burnout_front = result.burnout_front.max(state.wheels[0].spin.abs());
                }
                let _ = r.up;
            }
        }
        result
    }

    /// 0x5A40B0's gate: more than 1.1 m or 15 degrees off places the car
    /// again (0x5FBD70, at rest); within it the target is smoothed (count
    /// > 3: 0.25 old incoming + 0.75 new) and the car is driven there.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn target_gate_teleports_far_targets_and_drives_near_ones() {
        let car = peugeot();
        let world = World::flat(2000.0);
        let mut state = CarState::new(Vec3::ZERO, 0.0);
        let mut mover = NisMover::default();
        let dt = 1.0 / 30.0;
        for call in 0..6 {
            let at = Vec2::new(call as f32 * 0.1, 0.0);
            let teleported = mover.set_target(&mut state, &car, &world, at, 0, dt);
            assert_eq!(teleported, call == 0, "call {call}");
            state.step_nis(&car, &world, &mut mover, NisControls::default(), dt);
        }
        assert_eq!(mover.count, 6);
        assert!((mover.target.x - (0.25 * 0.4 + 0.75 * 0.5)).abs() < 1e-5);
        assert!((state.speed() - 3.0).abs() < 0.3, "{}", state.speed());
        let far = mover.set_target(&mut state, &car, &world, Vec2::new(3.0, 0.0), 0, dt);
        assert!(far);
        assert_eq!(state.velocity, Vec3::ZERO);
        let turned = mover.set_target(&mut state, &car, &world, Vec2::new(3.0, 0.0), (20.0 / ANGLE16_TO_DEG) as u16, dt);
        assert!(turned, "a heading 20 degrees off places the car again");
    }

    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn intro13_scene_cars_follow_the_original_tracks() {
        let car = peugeot();
        let all = rows();
        let mut results = Vec::new();
        for n in 1..=4 {
            let rows: Vec<&Row> = all.iter().filter(|r| r.car == n).collect();
            let r = replay(&car, &rows);
            let travelled = rows.first().unwrap().com.distance(rows.last().unwrap().com);
            eprintln!("car{n}: travelled {travelled:.2} m, worst track {:.4} m, worst speed {:.2} %, burnout front {:.2} rad/s, tilt {:.4}", r.worst_track, r.worst_speed * 100.0, r.burnout_front, r.max_tilt);
            results.push((n, travelled, r));
        }
        for (n, _travelled, r) in results {
            // Measured 2026-11-04: worst 2.4 mm and 2.8 % (car3, 12.7 m).
            assert!(r.worst_track < 0.01, "car{n} track off by {} m", r.worst_track);
            assert!(r.worst_speed < 0.04, "car{n} speed off by {} %", r.worst_speed * 100.0);
            // The original's up axis moves up to 0.12 (car3, slope and
            // the burst at the direct-branch start) on its own ground.
            assert!(r.max_tilt < 0.15, "car{n} tipped {}", r.max_tilt);
            if n == 1 {
                // The burnout's body torque rocks the car: the original's up
                // axis leaves its rest by up to 0.0083 over the sequence;
                // the clone's (flat ground) 0.0094.
                assert!((0.004..0.016).contains(&r.max_tilt), "car1 rocking {}", r.max_tilt);
                // Car1's scripted burnout: FWD Peugeot fronts at 10 turns/s.
                assert!((r.burnout_front.abs() - 10.0 * 360.0 * 0.017453292).abs() < 0.5, "{}", r.burnout_front);
            }
        }
    }
}
