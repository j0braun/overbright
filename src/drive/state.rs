use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct WheelState {
    /// Spin, rad/s.
    pub spin: f32,
    /// Suspension compression, m (positive = compressed).
    pub compression: f32,
    /// The damper's compression rate, m/s (suspension +0x20): 0x5A87A0
    /// writes it only when the sub-step (0x7FC800) is over 0.0001 s, else
    /// the last one stays (a zero-length frame keeps it).
    pub compression_rate: f32,
    pub on_ground: bool,
    pub sliding: bool,
    /// Longitudinal slip, r·ω − v along the wheel, m/s.
    pub slip: f32,
    /// The slip the wheel exports to the car (wheel +0x100 → car +0x3B4 +
    /// 8i): `slip` while the tyre slides, 0 while it grips
    /// ([`exported_slip`](super::simulate::exported_slip)); the skid sound,
    /// the burnout score (0x5F7755), the Burnout replay camera (0x441D82) and
    /// the AI's nitrous test (0x402660) read the car's copy.
    pub exported_slip: f32,
    /// Load on the tyre, kN.
    pub load: f32,
    /// Brakes locked this wheel (+0x10C).
    pub locked: bool,
    /// Foot-brake and handbrake inputs for this wheel (+0x178, +0x17C).
    pub foot_brake: f32,
    pub handbrake: f32,
    /// Grip multiplier (+0x180).
    pub grip: f32,
    /// The drift assist's grip factor (+0x188): 1, except on the rear
    /// wheels in drift events (0x5AC270).
    pub assist_grip: f32,
    /// Net torque averaged over the last two steps (+0x120).
    pub net_torque: f32,
    /// Drive less brake torque after the rolling constraint, kN·m (+0x108).
    pub torque: f32,
    /// Longitudinal tyre force after the friction circle, kN (+0x124).
    pub fx: f32,
    /// Lateral tyre force after the friction circle, kN (+0x128).
    pub fy: f32,
    /// |force| over the friction circle's limit (+0x170, 0x59D610): 1
    /// while sliding or locked, 0 with no contact.
    pub circle: f32,
    /// Roll angle for drawing, radians.
    pub roll: f32,
    /// The spinner's angular speed (car object +0x4F8 + 4i) and angle
    /// (+0x4E8 + 4i), rad/s and radians: 0x5ECAD0 each step.
    pub spinner_rate: f32,
    pub spinner: f32,
    /// The surface under the wheel (+0x114, collision face byte 2).
    pub surface: u8,
    /// The contact's sideways speed across the wheel, m/s (the second of
    /// the car's per-wheel slip pair, +0x3B8; the AI's slip tracker reads
    /// it).
    pub lateral_slip: f32,
}

impl WheelState {
    /// Raw suspension load (0x5A8AA0..5A8B22), before tyre camber.
    pub(super) fn suspension_load(&self, s: &crate::handling::Suspension, other_compression: f32, preload: f32) -> f32 {
        let x = self.compression;
        let rate = self.compression_rate;
        ((x.max(0.0) * s.progression + 1.0) * s.spring * x
            + if rate > 0.0 { s.bump } else { s.rebound } * rate
            + preload
            + (x - other_compression) * s.anti_roll)
            .max(0.0)
    }
}

/// Snapshot token for one complete world interval, independent of any
/// CarState replacement during reset/contact processing.
#[derive(Clone, Copy, Debug)]
pub struct WorldVelocitySample {
    pub(super) velocity: Vec3,
    pub(super) dt: f32,
}

/// The held mover's spring point (0x599210 / 0x5A5C30, mover +0x90 the
/// saved centre of mass, +0xE0 the point, +0xF0 its velocity) and the force
/// the last integration took (rigid body +0x130, the tidy-up's copy).
#[derive(Clone, Copy, Debug)]
pub(super) struct Hold {
    pub(super) target: Vec3,
    pub(super) point: Vec3,
    pub(super) velocity: Vec3,
    pub(super) last_force: Vec3,
}

/// A force pass as the next sub-step's integration reads it: the body's
/// force and torque sums (the wheels' torques, forces and flags stay in
/// their [`WheelState`]s), and the state the pass was taken on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StoredForces {
    pub(super) force: Vec3,
    pub(super) torque: Vec3,
    /// Position, orientation, velocity and angular momentum after the pass.
    pub(super) at: (Vec3, Quat, Vec3, Vec3),
}

#[derive(Clone, Debug)]
pub struct CarState {
    /// Car-space origin in the world.
    pub position: Vec3,
    pub orientation: Quat,
    /// The rigid body's rows as the last sub-step's 0x5BB0E0 left them, with
    /// the `orientation` they were published as: while `orientation` is
    /// unchanged the next sub-step steps these exact rows, so
    /// the quaternion round trip adds no rounding between sub-steps.
    pub(super) body_rows: Option<(Quat, Mat3)>,
    /// Velocity of the centre of mass, m/s.
    pub velocity: Vec3,
    /// Completed outer-world velocity delta (car +80, 604750).
    pub world_acceleration: Vec3,
    /// Angular velocity, world space, rad/s (from the angular momentum).
    pub spin: Vec3,
    /// Angular momentum, world space, t·m²/s (rigid body +0x80).
    pub angular_momentum: Vec3,
    /// Damping that builds while the car sits still (rigid body +0x90).
    pub rest_damping: f32,
    /// The at-rest class the last sub-step's rest test gave (0x5992C0,
    /// kept by the mover at +0x54 and passed back in): 0 moving, 1 nearly
    /// still (momenta × 0.95), 2 at rest (momenta zeroed, no gravity on
    /// the next step: body +0x160 = 0).
    pub rest: u8,
    /// Rear-grip bonus ramp tied to the handbrake (mover +0x70).
    pub handbrake_ramp: f32,
    /// The wheel hop after a spinning launch ([`CarState::wheel_hop`],
    /// 0x5B6310): its count (car +0x46C; 0 off, 1 and 2 also fix the
    /// differential's split, 0x594A40), the step timer (+0x470), the grip
    /// it takes (+0x474) and the calm time (+0x478).
    pub hop: i32,
    pub hop_timer: f32,
    pub hop_grip: f32,
    pub hop_calm: f32,
    /// The wheels' load term as the hop's start test reads it (+0x164 of
    /// the force pass after the last integration; `end_loads`).
    pub hop_loads: [f32; 4],
    /// The length of the sub-step that brought the car to its pose (the
    /// original's 0x7FC800 when its suspensions measure at that sub-step's
    /// end, 0x5A87A0 from 0x5B04E0); 0 after a measurement outside the
    /// sub-step loop, which keeps the rate (round 15).
    pub last_sub: f32,
    /// The frame's step (0x865150) and whether this mover call re-runs the
    /// span left after a resolved contact (0x6012C0): the player's input
    /// update (steering, pedals) runs once a frame, before the physics,
    /// and the wheel hop (0x5B6310) counts the frame's dt on every mover
    /// call (round 15).
    pub frame_dt: f32,
    pub restep: bool,
    /// The shown rpm (car +0x440 = physics view +0x400; 1000 from the
    /// view's constructor 0x59F993): what the tachometer (0x4C6933), the
    /// engine sound (SFXCTL_Physics 0x484C50) and the dyno read, smoothed
    /// from the engine by [`shown_rpm_step`] in the mover's publish
    /// (0x5A5966..0x5A5A61).
    pub shown_rpm: f32,
    /// A DUMMY_SNAPSHOT ran before this world update
    /// ([`CarState::snapshot_take`]).
    pub snapshot_cleared: bool,
    /// The force pass the next sub-step integrates ([`StoredForces`]): the
    /// original's mover runs it at the end of every sub-step, after the
    /// components' tidy-ups and the rest test (0x5B04E0 → vt+8 = 0x5B0530),
    /// so a frame's first sub-step integrates forces taken before that
    /// frame's inputs.
    pub(super) forces: Option<StoredForces>,
    pub(super) frame_pedals: Controls,
    /// The gas (input block +0x20C) and |v| (car +0x430) the player's
    /// nitrous key handler reads (0x6034C6 → 0x5ECDC0): the key dispatch
    /// runs before the frame's input and physics update (**observed**
    /// under Wine), so these are the previous world update's,
    /// sampled as this update's mover frame starts. A world update, not a
    /// display frame: the same at any frame rate.
    pub nitrous_key: (f32, f32),
    /// Seconds since the body last hit a wall.
    pub since_collision: f32,
    /// The last hit the style manager's contact callback took (0x56FB30,
    /// MiscStyleManager +0x2C / +0x30 / +0x34 / +0x38; see
    /// [`CarState::style_contact`]): seconds since it, its strength (the
    /// contact record's summed push, record +0x60) and whether it costs a
    /// drift (kind 3, not a car, or a traffic car hit).
    pub last_hit: Option<(f32, f32, bool)>,
    /// The hits that passed 0x56FB30's gate since the style detectors last
    /// took them (only kept with `misc_style`).
    pub style_hits: Vec<crate::style::Hit>,
    /// The car's racer has a misc style manager (racer +0x1DC: the local
    /// human, 0x5FC3A6), so its pushes reach 0x56FB30 (0x5A9956).
    pub misc_style: bool,
    /// The car is the player's and its camera event kind (player +0x58)
    /// is 6, the crash camera: its track impacts pick their sample at
    /// volume + 60 (0x47A770).
    pub crash_camera: bool,
    /// A traffic car (owner kind 3: car +0x14 → +4).
    pub traffic: bool,
    /// The car's pairs with other cars after the last step (pair list
    /// 0x86B490), oldest first, for the near-miss detector 0x56F180;
    /// only kept with `misc_style`.
    pub near: Vec<crate::style::NearPair>,
    /// Manual transmission (racer +0x20; forced in a drag race).
    pub manual: bool,
    /// Car +0x464 = 1: the driver's Stability Control is off (controller
    /// byte +0x21, copied by 0x5F3E20 from the player's option byte
    /// 0x83AA7A + 0x28 × player at 0x5F1802). With one human, the race
    /// start (0x403C20 from 0x60A88C) gives every AI racer player 1's
    /// value; the car constructor zeroes it (0x591ED5), so traffic has 0
    // prov: inferred drive.suspension_load
    /// (**inferred**: no other writer reaches traffic). Gates
    /// [`launch_grip`].
    pub stability_off: bool,
    /// The wheels down when the frame began, as the mover hands them to
    /// the aero (0x5ABC00 → aero +0x14 = car +0x424 / 4). The drivetrain
    /// consumes that same exported count at 0x594A40 and 0x5AA840.
    pub(super) aero_down: usize,
    /// The car's gear (+0x434) as the rest test reads it: the drivetrain's
    /// gear (+0x3C) copied once a frame, after the sub-steps, by
    /// Car_SpeedAndWheelSpin (0x5A5924, called at 0x5B74E1 after the
    /// sub-step loop of 0x5B72E0), so during a frame's sub-steps it is the
    /// gear at the frame's start.
    pub(super) frame_gear: usize,
    /// Exported forward speed (native physics-view +0x3EC/full-car +0x42C).
    /// Mover exits and explicit placement/script writers publish it; substeps,
    /// contact impulses and rewinds retain the previous export.
    pub(super) tyre_speed: f32,
    /// The race start's hold (human driver +0x2B4, set and cleared through
    /// its vtable slot 0x1C, 0x410430): while set the human driver update
    /// (0x41E37E) asks for neutral (+0x1E4 = 1) and full brake (0x5EB900
    /// with 1.0) after the pedals, so the engine revs on the line.
    pub held: bool,
    /// The dyno's hold (mover +0x84, set by 0x599210 from the dyno pull's
    /// reset 0x56D350): 0x5A49B0 then puts the aero factor (+0x30) at 0,
    /// no drag or downforce (docs/formats.md, "The pre-race hold").
    pub aero_off: bool,
    /// The dyno pull's boost: the force pass (0x5B0530, 0x5B0545..0x5B0562)
    /// sets the engine's power factor (+0x80) to 1.5 at the start of every
    /// sub-step while the mover is held (+0x84) and the car's dyno manager
    /// (car +0x1C → +0x1E0) is in state 2, the pull. Set by
    /// [`crate::dyno::DynoPull`] for its state 2.
    pub dyno_boost: bool,
    /// The hold's position spring (0x599210 saves the pose,
    /// 0x5A5C30 runs at the end of every force pass while mover +0x84 is
    /// set): see [`CarState::set_hold`].
    pub(super) hold: Option<Hold>,
    /// A gear the player's input asks for once (+0x1E4 written by the
    /// input's vt+0x30, 0x407310: first gear, 2, with a manual gearbox).
    pub gear_request: Option<usize>,
    /// Yaw-torque limit this frame, kN·m; 0 = none (rigid body +0x200).
    pub yaw_limit: f32,
    /// Steering wheel angle at the road wheels, degrees, positive = left.
    pub steer_degrees: f32,
    /// The steering lock last allowed, degrees (PlayerSteering +0x80).
    pub steer_limit: f32,
    /// The running mean of the steering input (PlayerSteering +0x14).
    pub steer_average: SteerAverage,
    /// The running mean of the input's rate of change (PlayerSteering
    /// +0x48) and last frame's shaped input (+0x84).
    pub steer_rate_average: RateAverage,
    pub last_steer_input: f32,
    /// The yaw rate (world z of [`Self::spin`]) when the last frame's
    /// steering ran, and that frame's step: the car's angular acceleration
    /// (car +0x120, (ω +0x110 − last frame's +0x530) / dt, 0x604750) is
    /// rebuilt from them for the drift steering's ratio boost.
    pub last_yaw_rate: f32,
    pub last_steer_dt: f32,
    /// Sliding far enough that counter-steer is always allowed (+0x10).
    pub drifting: bool,
    pub wheels: [WheelState; 4],
    /// Engine, clutch and gearbox; created on the first step.
    pub drivetrain: Option<Drivetrain>,
    /// The player input's reverse latch (+0x2F0) and its gear request
    /// (+0x1E4: reverse, else automatic).
    pub reverse_latched: bool,
    pub reverse_requested: bool,
    /// The car's ground-query state (car +0x4).
    pub ground: crate::ground::GroundQuery,
    /// Drag factor of the surfaces under the wheels (aero +0x34).
    pub surface_drag: f32,
    /// The distance query's remembered vertices per track polygon the car
    /// is paired with (pair +0x118), kept while the polygon stays a
    /// contact candidate.
    pub pairs: crate::fxhash::FxHashMap<(usize, usize), Option<narrow::PairCache>>,
    /// Seconds left of the drag start's full-gas window (input +0x2CC,
    /// 0.25 s from the green light, 0x407310).
    pub launch_timer: f32,
    /// The gas the input handed the car last frame (input +0x20C), which
    /// the URL rate limit works from.
    pub last_gas: f32,
    /// Where the car last touched a wall (world), midway between the
    /// closest points of its hull and the wall polygon.
    pub wall_contact: Option<Vec3>,
    /// Crash mode (car +0x38 = 1, set by a car-against-car hit of 30 or
    /// more, 0x59FB80): no rest or angular damping, gravity 11, no aero,
    /// gas at most 0.4, whole inertia in contacts (0x5A49B0, 0x593A50).
    pub crash: bool,
    /// Seconds crash mode has had all four wheels down and no contact
    /// (car +0x54C, 0x5FC060); it ends at 0.1.
    pub crash_clear: Option<f32>,
    /// Set when a car-against-car push of 30 or more puts the car in crash
    /// mode (0x59FB80 → 0x5F6BF0, which keeps the clock in +0x548 for a
    /// human car); the race takes and clears it (`event_camera`).
    pub crash_hit: bool,
    /// The rigid body's ground inertia (+0x158, 0x5A49B0): outside crash
    /// mode with a wheel down the yaw inertia is × record +0x270.
    pub ground_inertia: bool,
    /// Contact records with track polygons, by pair (pair +0x244).
    pub records: crate::fxhash::FxHashMap<(usize, usize), ContactRecord>,
    /// When each track pair was made (its place in the pair list); kept
    /// while the polygon is a candidate or its record lives (0x592E70).
    pub(super) pair_made: crate::fxhash::FxHashMap<(usize, usize), u64>,
    /// The car's live contact records, track and car, in the pair list's
    /// order, as of the end of the last [`step_cars`].
    pub contacts: Vec<ContactNote>,
    /// Collides with other cars (collision body +0x190; cleared by the
    /// race when a racer ends a drift or Street X race, or is knocked out,
    /// 0x606CC0).
    pub car_collisions: bool,
    /// The race start holds the body out of every car pair (
    /// **observed** under Wine: no pair test 0x5B5060 and no contact
    /// 0x5B2620 in the start scene or the countdown): in the scene the
    /// partner filter 0x593990 refuses (collision body +0x17 = 1), in the
    /// countdown the body is not swept (+0x19 = 0). Set by the race for
    /// race status 2 and 3, clear from the green.
    pub start_hold: bool,
    /// The world under the car is loaded (car +0x5C0 / +0x5CC; its
    /// collision body's [+0x60] +8 is set exactly then). The body's partner
    /// filter 0x593990 (vt+0x54 of vtable 0x7A2478, **verified**) refuses
    /// every partner — track polygon, car or prop — while [body +0x60] +8
    /// is null, and the sweep 0x5B23C0 asks it first (vt+0x54(0)) before it
    /// sweeps the body at all: an unloaded car meets nothing. The race and
    /// the traffic set it each frame; true for the player.
    pub streamed: bool,
    /// Ghosted against other cars (car +0x998, or the timer +0x974 /
    /// +0x978 in seconds left): touching cars take no impulse (0x5B5060).
    pub ghost: bool,
    pub ghost_timer: f32,
    /// The largest impulse a contact gave this car in the last frame,
    /// capped at 40 (0x59FB80; in drag races 30 totals the car).
    pub hardest_impulse: f32,
    /// Contacts of this car the world collision step resolved (0x5FFA60's
    /// loop calls 0x5B2620 on each, which first tells both bodies, vtable
    /// 0x7A2478 slot +0x48 = 0x593950 → 0x5F4970, the car's controller
    /// slot +0x38): track, car and prop contacts alike. Counted up by
    /// [`step_world`]; whoever reads it clears it.
    pub resolved: u32,
    /// The human's car (car +0x18 set: the player's car, also under the
    /// autopilot past the line). Set by the race step.
    pub human: bool,
    /// A contact with the human's car was resolved (0x402450 sees the
    /// other car's +0x18): an AI racer goes back to the physics mover;
    /// the race step restarts its hold and clears this.
    pub human_hit: bool,
    /// A contact of a car on the BeltMover ([`Input::Belt`]) was resolved
    /// (0x402450 on a mode-4 car: controller +0x730 = 2 s, 0x783510); the
    /// race step takes it off the belt and clears this.
    pub belt_hit: bool,
    /// Totaled (car +0x964, a latch): in a drag race (0x89E7D8 or
    /// 0x890108) the crash check 0x59FB80 sets it on a racer's car (car
    /// +0x1C set) for a push of 30 or more (|J| capped at 40), or for any
    /// push while the body's vt+0x14 +0x38 is under −0.5 (taken as the
    /// car's up vector's z: on its roof, inferred). The drag drop-out
    /// (0x6076E0) then ends the run with racer status 9 "Totaled".
    pub totaled: bool,
    /// The nitrous tank (car +0x454; 0x591CE0 at set-up).
    pub nitrous: Option<crate::nitrous::Tank>,
    /// The Nitrous key's last state: the player's events act on a change
    /// (0x602BE0 cases 0xA3 / 0xA4 → 0x6034C6).
    pub nitrous_held: bool,
    /// The MeloMvr's chassis (0x5B6D10, mode 3) while that mover drives
    /// the car ([`Input::Melo`]); None on the physics mover.
    pub melo: Option<crate::melo::Melo>,
    /// The brake pedal the input block last handed the car (+0x30 →
    /// +0x214), as the brake-light flag 0x5F62C0 reads it.
    pub brake_input: f32,
    /// Seconds since the brake pedal was last over 0.05 (0x5F62C0 stamps
    /// car +0x948 with the clock 0x865188 then); infinite before it ever
    /// was (+0x948 = 0).
    pub since_brake: f32,
    /// The draft behind other cars (car +0x4DC, 0x5EBD90 each frame from
    /// 0x609F00): the drag is × (1 − it) (aero +0x0C, 0x599180 /
    /// 0x5A3390 → 0x59E681).
    pub draft: f32,
    /// The race catch-up (car +0x468; 1 unless a Quick Race with Catch Up
    /// raises an AI racer's, 0x4172D0). 0x599180 hands it each frame to
    /// the engine (+0x90, torque × it), the rigid body (+0x208, the top
    /// speed × it, 0x59C1F0), the aero (+0x08: drag and downforce take
    /// |v| / it, 0x59E677) and the wheels (+0x184, the friction circle ×
    /// it, 0x59D69E).
    pub catch_up: f32,
    /// The hydraulics' corners (`crate::hydraulics::Suspension`): heights,
    /// states, kicks and the frame's mounts.
    pub hydraulics: crate::hydraulics::Suspension,
    /// The scene car mover (mode 5) while a scene holds the car
    /// ([`nis::NisMover`]); None outside scenes (its placement count starts
    /// again from 0, as the scene setup's reset 0x43B172 does).
    pub nis: Option<nis::NisMover>,
}

/// The car's inertia diagonal as the rigid body uses it (0x59B950: the
/// plain tensor +0x170, or the ground one +0x1B0 with the yaw term ×
/// record +0x270).
pub(crate) fn inertia_of(h: &Handling, ground: bool) -> Vec3 {
    let i = Vec3::from(h.inertia);
    if ground { Vec3::new(i.x, i.y, i.z * h.unknown_270) } else { i }
}

// Native x87 PC24 evaluates this order before writing the exported scalar.
pub(super) fn exported_forward_speed(velocity: Vec3, forward: Vec3) -> f32 {
    (velocity.z * forward.z + velocity.y * forward.y) + velocity.x * forward.x
}
pub(super) fn placement_velocity(forward: Vec3, speed: f32) -> Vec3 {
    forward * (speed + 0.1)
}

impl CarState {
    /// A car at rest at `position` (car origin, on the road) facing `heading`
    /// (radians around +z, 0 = +x).
    /// A racer put on its start point (0x604270 → 0x5F5020 → 0x5F4A00):
    /// `at` is the point on the ground (a grid slot after 0x5EB5E0, its z
    /// the ground under the marker) and `heading` the turn about z.
    ///
    /// Each wheel's rest point (`at` + the heading's turn × record +0x120 +
    /// 0x30·i) is dropped to the ground (0x5E72F0, `World::grid_ground`).
    /// When all four find ground, the body's axes come from those four
    /// ground points p0..p3 (front left, front right, rear right, rear left):
    /// forward A = (p1 − p2) + (p0 − p3), left B = (p3 − p2) + (p0 − p1),
    /// both normalised (0x43CF10), up C = A × B (0x43D1E0); and the centre of
    /// mass's height is the ground points' mean + (the mean of radius −
    /// wheel z (+0x130 − +0x128 per wheel) + centre-of-mass z (+0x118)) /
    /// |C.z|. Otherwise the point and the heading stay as given. The car's
    /// position (racer +0x50, its object's +0x10) is then that point less the
    /// record's centre of mass (+0x110..+0x118, or the rigid body's +0x1F0
    /// once it has one), not turned by the heading (0x5F5072..0x5F5095);
    /// but the rigid body keeps the point as its centre of mass (observed),
    /// so the clone's origin is the point less the turned offset.
    // prov: inferred drive.placed
    /// **Inferred**: the axes are made orthonormal here (the game keeps B
    /// unorthogonalised).
    pub fn placed(car: &crate::car::CarType, world: &crate::world::World, at: Vec3, heading: f32) -> CarState {
        let rot = Mat3::from_rotation_z(heading);
        let com = Vec3::from(car.handling.centre_of_mass);
        let mut ground = [Vec3::ZERO; 4];
        let mut all = true;
        let (mut sum, mut lift) = (0.0f32, 0.0f32);
        for (i, w) in car.wheels.iter().enumerate() {
            let p = at + rot * w.position;
            match world.grid_ground(p) {
                Some(z) => {
                    ground[i] = Vec3::new(p.x, p.y, z);
                    sum += z;
                }
                None => all = false,
            }
            lift += w.radius - w.position.z;
        }
        let mut centre = at;
        let mut state_rot = Quat::from_rotation_z(heading);
        if all {
            let [p0, p1, p2, p3] = ground;
            let norm = |v: Vec3| if v.length() != 0.0 { v.normalize() } else { Vec3::X };
            let a = norm((p1 - p2) + (p0 - p3));
            let b = norm((p3 - p2) + (p0 - p1));
            let c = a.cross(b);
            centre.z = sum * 0.25 + (lift * 0.25 + com.z) / c.z.abs();
            let up = c.normalize_or_zero();
            if up.z > 0.0 {
                state_rot = Quat::from_mat3(&Mat3::from_cols(a, up.cross(a), up)).normalize();
            }
        }
        // The rigid body is built on +0x60 (the point, its centre of mass):
        // under Wine (2026-10-31, reports) a resume with the kept place
        // (career +0x8750) written to heading 90° left car +0x60 on the
        // point exactly and +0x50 at the point less the *turned* offset
        // once the body ran (0.25 m off in y, not in x). So the clone's body
        // origin is the point less the offset turned by the body's axes;
        // the unrotated +0x50 above lasts only until the first update
        // (**observed**; the rigid body's constructor reading +0x60 is not
        // traced).
        CarState::new(centre - state_rot * com, 0.0).with_orientation(state_rot)
    }

    pub(super) fn with_orientation(mut self, q: Quat) -> CarState {
        self.orientation = q;
        self
    }

    pub fn new(position: Vec3, heading: f32) -> CarState {
        CarState {
            position,
            orientation: Quat::from_rotation_z(heading),
            body_rows: None,
            velocity: Vec3::ZERO,
            world_acceleration: Vec3::ZERO,
            spin: Vec3::ZERO,
            angular_momentum: Vec3::ZERO,
            rest_damping: 0.0,
            rest: 0,
            handbrake_ramp: 1.0,
            hop: 0,
            hop_timer: 0.0,
            hop_grip: 1.0,
            hop_calm: 0.0,
            hop_loads: [0.0; 4],
            last_sub: 0.0,
            frame_dt: 0.0,
            shown_rpm: 1000.0,
            snapshot_cleared: false,
            forces: None,
            restep: false,
            frame_pedals: Controls::default(),
            nitrous_key: (0.0, 0.0),
            since_collision: 100.0,
            last_hit: None,
            style_hits: Vec::new(),
            misc_style: false,
            crash_camera: false,
            traffic: false,
            near: Vec::new(),
            yaw_limit: 0.0,
            steer_degrees: 0.0,
            steer_limit: 0.0,
            steer_average: SteerAverage::default(),
            steer_rate_average: RateAverage::default(),
            last_steer_input: 0.0,
            last_yaw_rate: 0.0,
            last_steer_dt: 0.0,
            drifting: false,
            wheels: [WheelState { grip: 1.0, assist_grip: 1.0, ..Default::default() }; 4],
            drivetrain: None,
            reverse_latched: false,
            reverse_requested: false,
            ground: Default::default(),
            surface_drag: 1.0,
            pairs: Default::default(),
            launch_timer: 0.0,
            last_gas: 0.0,
            wall_contact: None,
            crash: false,
            crash_clear: None,
            crash_hit: false,
            ground_inertia: false,
            records: Default::default(),
            pair_made: Default::default(),
            contacts: Vec::new(),
            car_collisions: true,
            start_hold: false,
            streamed: true,
            ghost: false,
            ghost_timer: 0.0,
            hardest_impulse: 0.0,
            resolved: 0,
            human: false,
            human_hit: false,
            belt_hit: false,
            totaled: false,
            nitrous: None,
            nitrous_held: false,
            manual: false,
            stability_off: false,
            aero_down: 0,
            frame_gear: crate::handling::NEUTRAL,
            tyre_speed: 0.0,
            held: false,
            aero_off: false,
            dyno_boost: false,
            hold: None,
            gear_request: None,
            melo: None,
            brake_input: 0.0,
            since_brake: f32::INFINITY,
            draft: 0.0,
            catch_up: 1.0,
            hydraulics: Default::default(),
            nis: None,
        }
    }

    /// The car update's brake-light step (0x5F62C0, from 0x6049D9): note
    /// this frame's brake pedal (+0x214) and, over 0.05 (0x7A5640), stamp
    /// the time (+0x948).
    pub fn note_brake(&mut self, brake: f32, dt: f32) {
        self.brake_input = brake;
        self.since_brake = if brake > 0.05 { 0.0 } else { self.since_brake + dt };
    }

    /// The brake-light flag (car +0x944, 0x5F62C0): for owner kinds 2 and
    /// 4 (`holds`: the racers) while the last stamp is at most 0.5 s old
    /// (0x7A5644; ticks × 0.00025, 0x784268); for the others (the player,
    /// the traffic) while the pedal is over 0.05.
    pub fn brake_lights(&self, holds: bool) -> bool {
        if holds { self.since_brake <= 0.5 } else { self.brake_input > 0.05 }
    }

    /// A race's car set-up: the player's transmission is forced to manual
    /// in a drag race (0x5EB440, handed to the input at 0x5F6D60 through
    /// vt+0x34, 0x407280, which also sets the gear request to the current
    /// gear). The launch window waits for the green light
    /// ([`CarState::release_hold`]).
    pub fn start_race(&mut self, race: crate::race::RaceMode) {
        self.launch_timer = 0.0;
        self.manual = race.drag();
        self.gear_request = None;
    }

    /// The green light: the hold clears (0x410430 with 0), which calls the
    /// input's vt+0x30 ([`CarState::drive_off`]).
    /// The engine heat (+0x8C) stays off (−1) through the hold and reads 0
    /// from the green light in a drag race (**observed** under Wine: −1
    /// while revving at the limiter on the line, 0 on the first frames of
    /// the race; the store that clears it is not traced).
    pub fn release_hold(&mut self, race: crate::race::RaceMode) {
        self.held = false;
        if race.drag()
            && let Some(t) = self.drivetrain.as_mut()
        {
            t.heat = 0.0;
        }
        self.drive_off(race);
    }

    /// The player input's vt+0x30 (0x407310), run at the hold's release
    /// and by the pedal update (0x407A30) once the gear request is no
    /// longer reverse: with a manual gearbox (transmission mode 1 or 3)
    /// the request becomes first gear (2), and in a drag race (0x89E7D8 /
    /// 0x890108) the launch window +0x2CC opens for 0.25 s (0x783B04 ×
    /// 4000 ticks); with the automatic one the request goes back to −1.
    pub(super) fn drive_off(&mut self, race: crate::race::RaceMode) {
        if self.manual {
            self.gear_request = Some(crate::handling::NEUTRAL + 1);
            if race.drag() {
                self.launch_timer = 0.25;
            }
        }
    }

    /// A manual shift request (ignored with the automatic gearbox).
    pub fn shift(&mut self, car: &crate::car::Car, up: bool) {
        if !self.manual {
            return;
        }
        let h = &car.car_type.handling;
        let shown = self.shown_rpm;
        let drivetrain = self.drivetrain.get_or_insert_with(|| Drivetrain::new(h));
        drivetrain.shown_rpm = Some(shown);
        drivetrain.shift(h, up);
    }

    pub fn forward(&self) -> Vec3 {
        self.orientation * Vec3::X
    }

    /// Radians around +z; 0 faces +x.
    /// Back on the physics mover from the MeloMvr (0x5F46B0(1)): the new
    /// mover takes the car's pose, velocity and spin, and — as the
    /// MeloMvr's set-up does the other way (0x5A2A70) — the wheel spins
    /// (already written back each frame), gear and rpm the last mover left
    // prov: inferred drive.leave_melo
    /// on the car (**inferred** for the physics mover's set-up 0x5B5CD0).
    pub fn leave_melo(&mut self) {
        if let Some(m) = self.melo.take()
            && let Some(d) = self.drivetrain.as_mut()
        {
            d.gear = m.gear;
            d.target_gear = m.gear;
            d.engine_w = m.rpm / crate::handling::RPM_PER_RAD_S;
        }
    }

    pub fn heading(&self) -> f32 {
        let f = self.forward();
        f.y.atan2(f.x)
    }

    /// Publish a native parameter writer (placement or traffic mover), whose
    /// scalar need not equal the live velocity projection. Synthetic initial
    /// conditions must explicitly supply this scalar or call `publish_forward_speed`.
    pub fn set_exported_forward_speed(&mut self, speed: f32) {
        self.tyre_speed = speed;
    }

    /// Mover export 5A564C..5A5665: PC24 Z+Y then X. Runtime axes still come
    /// from the clone's unit quaternion, not the native raw matrix.
    pub fn publish_forward_speed(&mut self) {
        self.set_exported_forward_speed(exported_forward_speed(self.velocity, self.forward()));
    }

    /// 5F5335 / 5F536F: the placement argument is published separately from
    /// axis * (argument + 0.1). This installs motion only; callers own pose,
    /// component and controller reset (whose existing approximations remain).
    pub fn set_placement_speed(&mut self, speed: f32) {
        self.set_exported_forward_speed(speed);
        self.velocity = placement_velocity(self.forward(), speed);
    }

    /// Native tyre consumers read the exported speed, not changing body velocity.
    pub fn exported_forward_speed(&self) -> f32 {
        self.tyre_speed
    }

    /// Metres per second along the car's forward axis.
    pub fn speed(&self) -> f32 {
        self.velocity.dot(self.forward())
    }

    /// Front-wheel angle, radians (positive = left).
    pub fn steer_angle(&self) -> f32 {
        self.steer_degrees.to_radians()
    }

    pub fn gear_name(&self) -> String {
        match self.drivetrain.as_ref().map(|d| d.gear) {
            Some(0) => "R".into(),
            Some(NEUTRAL) | None => "N".into(),
            Some(g) => (g - 1).to_string(),
        }
    }

    /// The replay manager's DUMMY_SNAPSHOT (0x57C190, every 100 world
    /// updates while it records): a take (+0x40 = 1) into an empty buffer,
    /// so only the serialisers' take branches act. The rigid body's
    /// (0x59B700 → vt+0x1C = 0x59C600, the tidy-up) empties the force and
    /// torque sums the last force pass left for the next integration (body
    /// +0x110 = (0, 0, −m·g), +0x120 = 0; observed under Wine:
    /// `snapshot-take-diff.txt`). The clone builds and integrates a sub-step's
    /// forces in one go, so the next sub-step integrates gravity alone.
    ///
    /// The wheels' take branch (their serialiser 0x59C990..0x59CB86, run for
    /// a take +0x40 or a restore +0x41) zeroes, among the rest, the lock
    /// flag +0x10C (0x59CA6A) and the sliding flag +0x160 (0x59CAC6): a
    /// wheel the brakes locked as the car stopped is free again after the
    /// next snapshot (observed under Wine, the finished-career save's Skyline's four
    /// wheels unlocked at the drag's green, `subtrap.py`; the clone's front
    /// pair stayed locked from braking to the line, its launch lost the
    /// front drive for a frame).
    pub fn snapshot_take(&mut self) {
        self.snapshot_cleared = true;
        for w in &mut self.wheels {
            w.locked = false;
            w.sliding = false;
        }
    }

    /// The car's shown rpm (car +0x440): the MeloMvr's own, else the
    /// physics mover's smoothed value ([`shown_rpm_step`]); idle before
    /// the first step.
    pub fn shown_rpm(&self) -> f32 {
        if let Some(m) = &self.melo {
            return m.rpm;
        }
        if self.drivetrain.is_some() { self.shown_rpm } else { self.rpm() }
    }

    pub fn rpm(&self) -> f32 {
        // On the MeloMvr its own rpm (the movers write the car's rpm).
        if let Some(m) = &self.melo {
            return m.rpm;
        }
        self.drivetrain.as_ref().map_or(0.0, |d| d.rpm())
    }

    /// Wheel poses for drawing: suspension lift, steer and roll.
    pub fn wheel_poses(&self, car: &Car) -> [WheelPose; 4] {
        let table = &car.car_type.wheels;
        std::array::from_fn(|i| WheelPose {
            // The hub (+0x30): the mount lowered by the hydraulics' height
            // plus the compression (**inferred**: the wheel's model follows
            // the hub).
            lift: self.wheels[i].compression - self.hydraulics.mount[i],
            steer: if i < 2 { self.front_wheel_angle(table, i) } else { 0.0 },
            roll: self.wheels[i].roll,
            spinner: self.wheels[i].spinner,
            // The front wheels' x-turn (`Car::camber`). The brakes take one
            // turn: in game 0x6316E0 fills the frames (0x63182C → 0x5EC800)
            // prov: inferred drive.wheel_poses
            // before each car's 0x620AF0 (**inferred** from the order of
            // the calls; the original's chase-view trace showed no front
            // brake to measure).
            tilt: car.camber,
            brake_tilt: car.camber,
        })
    }

    /// Where the jump camera's flight starts (0x5EC3E0): the mean over the
    /// four wheels of carWorld × wheelMatrix_i × (0, 0, −r_i), the wheel
    /// matrices being car +0x130 + 0x40·i, which 0x5921C0 builds from the
    /// wheel's place with z + body +0x340 + 4·i, the wheel's dynamic
    /// vertical offset (**observed** under Wine: ECX = car +
    /// 0x40, z = place z + offset over 25 samples). The clone's offset is
    /// the one it draws the wheel by (`wheel_poses` lift; **inferred** to be
    /// the same quantity).
    pub fn flight_start(&self, car: &Car) -> Vec3 {
        let poses = self.wheel_poses(car);
        car.car_type
            .wheels
            .iter()
            .zip(poses.iter())
            .map(|(w, p)| self.position + self.orientation * (w.position + Vec3::Z * p.lift - Vec3::Z * w.radius))
            .sum::<Vec3>()
            * 0.25
    }

    /// A wheel's suspension mount in car space (0x59E0D0: the wheel's
    /// point with the hydraulics' height taken off z, set each frame by
    /// 0x5AB9A0).
    pub(super) fn mount(&self, at: Vec3, i: usize) -> Vec3 {
        at - Vec3::Z * self.hydraulics.mount[i]
    }

    /// Car space → world.
    pub fn transform(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.orientation, self.position)
    }

    /// The car's slip angle, degrees: heading minus direction of travel,
    /// zero below 1 m/s (0x59FA20, 0x591DA0). Positive when the nose points
    /// left of where the car is going.
    pub fn slip_degrees(&self) -> f32 {
        let v = self.velocity;
        if v.length() <= 1.0 {
            return 0.0;
        }
        let d = self.heading() - v.y.atan2(v.x);
        let d = (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        d.to_degrees()
    }

    /// Wheels touching the ground (car +0x424).
    pub fn wheels_down(&self) -> usize {
        self.wheels.iter().filter(|w| w.on_ground).count()
    }
}
