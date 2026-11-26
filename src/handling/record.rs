use super::*;

pub(super) fn read_u32(r: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(r[at..at + 4].try_into().unwrap())
}

fn floats<const N: usize>(r: &[u8], at: usize) -> [f32; N] {
    std::array::from_fn(|i| read_f32(r, at + i * 4))
}

/// A table sampled evenly between `x0` and `x1`, read with linear
/// interpolation and clamped at both ends (0x5764E0).
#[derive(Clone, Copy, Debug)]
pub struct Curve<const N: usize> {
    pub x0: f32,
    pub x1: f32,
    pub values: [f32; N],
}

impl<const N: usize> Curve<N> {
    pub fn at(&self, x: f32) -> f32 {
        // The game stores (N - 1) / (x1 - x0) and truncates towards zero.
        let f = (x - self.x0) * ((N - 1) as f32 / (self.x1 - self.x0));
        if !(f >= 0.0) {
            return self.values[0];
        }
        let i = f as usize;
        if i >= N - 1 {
            return self.values[N - 1];
        }
        let t = f - i as f32;
        (1.0 - t) * self.values[i] + t * self.values[i + 1]
    }
}

/// Engine data, record +0x300.
#[derive(Clone, Debug)]
pub struct Engine {
    pub idle_rpm: f32,
    pub redline_rpm: f32,
    pub max_rpm: f32,
    /// Crank inertia, t·m² (+0x30C; the engine's ω' divides by it, 0x5A12A0).
    pub inertia: f32,
    /// Full-throttle torque, kN·m, over idle..max rpm (+0x310).
    pub torque: Curve<9>,
    /// Nine values at +0x334 over idle..max rpm (engine curve at +0x4C).
    /// Not read by the torque function; turbo cars have non-zero values.
    pub curve_334: Curve<9>,
    /// Closed-throttle drag torque, kN·m, over idle..max rpm (+0x358).
    pub braking: Curve<3>,
    /// The nitrous curve (engine +0x60: 0x7FBF38 over idle..redline,
    /// 0x5AA65B).
    pub nitrous: Curve<9>,
}

/// Gearbox and driveline, record +0x2C0.
#[derive(Clone, Debug)]
pub struct Gearbox {
    /// +0x2C0 (0.8 on every car): where between idle and redline the
    /// shift out of neutral sits (0x5AC7E0).
    pub neutral_shift: f32,
    /// +0x2C4 (500 on every car); not yet traced.
    pub unknown_2c4: f32,
    pub final_drive_front: f32,
    pub final_drive_rear: f32,
    /// Share of drive torque sent to the rear axle: 1 RWD, 0 FWD (0x594A40).
    pub rear_split: f32,
    /// Clutch coupling, kN·m per rad/s of slip (0x594CC0).
    pub clutch_stiffness: f32,
    /// Ratios indexed by gear: 0 reverse, 1 neutral, 2.. forward.
    pub ratios: Vec<f32>,
    /// Clutch engagement rate per second (0x5949C0).
    pub clutch_rate: f32,
}

/// Tyre block, record +0x1E0 (front) / +0x200 (rear), read through
/// wheel +0x18.
#[derive(Clone, Copy, Debug)]
pub struct Tyre {
    /// +0: slip-angle scale, degrees (12 on every car): the lateral curve
    /// index is `angle° / scale · 6` (0x59D520).
    pub slip_angle_scale: f32,
    /// +4: load scale for the lateral curves.
    pub load_scale: f32,
    /// +8: lateral force scale.
    pub lateral_scale: f32,
    /// +0xC: friction-circle grip, × load (0x59D610).
    pub grip: f32,
    /// +0x10: sliding friction, × load (0x5A8290).
    pub sliding: f32,
}

/// Suspension block, record +0x280 (front) / +0x2A0 (rear), used by the
/// suspension update 0x5A87A0.
#[derive(Clone, Copy, Debug)]
pub struct Suspension {
    /// +0: spring progression: k·(1 + progression·x).
    pub progression: f32,
    /// +4: spring rate, kN/m.
    pub spring: f32,
    /// +8: damping while compressing, kN·s/m.
    pub bump: f32,
    /// +0xC: damping while extending, kN·s/m.
    pub rebound: f32,
    /// +0x10: anti-roll bar, kN/m of left/right difference.
    pub anti_roll: f32,
    /// +0x14: compression limit, m.
    pub max_compression: f32,
    /// +0x18: extension limit (negative), m.
    pub max_extension: f32,
    /// +0x1C: not read by the update (0.15 on the 240SX); ride height?
    pub unknown_1c: f32,
}

#[derive(Clone, Debug)]
pub struct Handling {
    /// Tonnes (+0x220; scaled by the weight-reduction upgrade, 0x5997F0).
    pub mass: f32,
    /// +0x224..0x22C: length, width, height in metres (inferred from values).
    pub dimensions: [f32; 3],
    /// Body-space inertia diagonal, t·m²: the diagonal of a 4×4 matrix at
    /// +0x230 (roll, pitch, yaw).
    pub inertia: [f32; 3],
    /// Centre of mass in car space (+0x110..+0x118; rigid body +0x1F0).
    pub centre_of_mass: [f32; 3],
    /// Aerodynamic drag, kN per (m/s)² (+0x370; the aero update 0x59E630).
    pub drag: f32,
    /// Front / rear downforce, kN per (m/s)² (+0x374 / +0x378).
    pub downforce: [f32; 2],
    /// Steering ratio (+0x380), scales target angle and speed (0x416BB0).
    pub steering_ratio: f32,
    /// Foot-brake torque per unit pedal, kN·m (+0x384, ×4 at 0x5A8290).
    pub brake: f32,
    /// Handbrake torque, kN·m (+0x388, ×10).
    pub handbrake: f32,
    /// Brake balance (+0x38C): front ×2(1 − b), rear ×2b (0x5A4D90).
    pub brake_balance: f32,
    /// +0x37C: downforce share in the suspension preload (0x5AB9A0).
    pub preload_downforce: f32,
    /// +0x390, +0x394: rear-grip bonus bases while sliding (0x5990D0).
    pub drift_grip: [f32; 2],
    /// +0x398: yaw-torque limit with Stability Control on (car +0x464
    /// clear), kN·m (0x5A49B0 at 0x5A4C65).
    pub yaw_limit_on: f32,
    /// +0x39C: yaw-torque limit with Stability Control off; 0 = only
    /// while counter-steering (then 20).
    pub yaw_limit_off: f32,
    /// +0x3A0: top speed, mph (the rigid body's limiter, 0x59C1F0).
    pub top_speed_mph: f32,
    /// The car's computed top speed, m/s (0x5B5940); set by
    /// [`setup_performance`].
    /// The car type's place in the type table (0x8A1CCC, the set-up
    /// object's +0; GLOBALB's car-type chunk order).
    pub type_index: usize,
    pub computed_top_speed: f32,
    /// The performance object's +0x10, the gearing-limited top speed
    /// (0x5B5AC0 at 0x5B5CB5: the speed of the 3 mph step at which the top
    /// gear passes its shift rpm, at most 92 steps), set by
    /// [`super::setup_performance`]; the racer info's +0x30 = [car +0x10]
    /// +0x20 the chase camera's shake, the typed replay camera and the AI
    /// belt read (**verified** + **measured**).
    pub gearing_top_speed: f32,
    /// Performance class 0 (fastest) .. 3, from the acceleration curve
    /// (0x5B4DD0), or 3 when record +0x844 is set; the drivetrain's +0xF0.
    pub performance_class: usize,
    /// Record +0x844: the class is fixed at 3.
    pub fixed_class: bool,
    /// The performance rating the class comes from (the racer's
    /// performance object +8, 0x5B4DD0): the area between acceleration and
    /// drag; left 0 when the class is fixed.
    pub performance_rating: f32,
    /// Record +0x270, scaled by the race tunes: the ground yaw-inertia
    /// factor (the rigid body's +0x1B0 tensor, 0x59B950).
    pub unknown_270: f32,
    /// The drag tachometer's needle angles, degrees, at 0 and at the
    /// maximum rpm with one local human (record +0x7FC / +0x800, read
    /// through car +0x0C + 0x7F0 +0x0C / +0x10 by 0x4AB210; the 206:
    /// −43 / 43, observed in memory under Wine).
    pub drag_needle: [f32; 2],
    /// The nitrous tank, seconds (record +0x3B0 = tuning +0x2A0; tuning
    /// slider 16), 0 on stock cars.
    pub nitrous_capacity: f32,
    /// Record +0x3B4 (tuning +0x2A4, an int): the shots it divides into
    /// for the tank's unread +0x418.
    pub nitrous_shots: i32,
    /// The shot's push (record +0x3BC = tuning +0x2AC; slider 17): × the
    /// nitrous curve × the open throttle on top of the engine torque.
    pub nitrous_power: f32,
    /// Front and rear wheel radii (record +0x130, +0x190).
    pub wheel_radii: [f32; 2],
    /// The race this car is set up for.
    pub race: crate::race::RaceMode,
    /// The race difficulty (0x89E7F4, copied at race start 0x53231B from the
    /// option 0x83AC30, which boots at 0; career events set their own).
    pub difficulty: usize,
    /// The drivetrain's shift-window scale (+0xE4, read by 0x594F10 for
    /// the HUD's shift light 0x4C68D0 and the shift rating 0x594F30): 1.0,
    /// except a human's in a drag race outside the career, where 0x5B6130
    /// sets it from the difficulty table 0x7A19F0 ([`drag_shift_scale`]).
    /// Read live under Wine: 1.0 for the player in a Quick Race circuit
    /// (drivetrain = car +0x34 → +0x4C, class +0xF0 = 3), 2026-10-31.
    pub shift_window_scale: f32,
    /// The ranges the drivetrain's random shift offsets are drawn from at
    /// every gear change (0x594720): +0xCC..+0xD0 the up-shift offset
    /// +0x78, +0xD4..+0xD8 the down-shift offset +0x7C, +0xDC..+0xE0 the
    /// 2nd-gear up-shift offset +0x80. The constructor (0x5AFAC3..) gives
    /// −100..0 for all three (0x7FC294, 0x7A18C4..0x7A18CC); an AI racer's
    /// set-up in a drag race takes its skill row's (0x5B61F4..0x5B626A,
    /// [`drag_ai_shift_ranges`]).
    pub shift_ranges: [[f32; 2]; 3],
    /// The drivetrain's extra clutch rate factor (+0xE8, 0x5949D2): 1.0,
    /// except an AI racer's in a drag race outside the career
    /// (0x5B61DE: 0x7A19FC[difficulty] = 0.4, 0.8, 1.0).
    pub clutch_scale: f32,
    /// The car's upgrade state, one float per performance category (the
    /// tuned object +0x2E0: 3 × the category's mean level, 0x5A6590 via
    /// 0x5AC500; `performance::state_float`), 0..3: what the race tunes
    /// blend their two halves by (0x5A7540). All 0 for a stock car.
    pub upgrade_state: [f32; 10],
    /// Front, rear.
    pub tyres: [Tyre; 2],
    pub suspension: [Suspension; 2],
    pub engine: Engine,
    pub gearbox: Gearbox,
}

pub(super) fn tyre(r: &[u8], at: usize) -> Tyre {
    let [slip_angle_scale, load_scale, lateral_scale, grip, sliding] = floats(r, at);
    Tyre { slip_angle_scale, load_scale, lateral_scale, grip, sliding }
}

pub(super) fn suspension(r: &[u8], at: usize) -> Suspension {
    let [progression, spring, bump, rebound, anti_roll, max_compression, max_extension, unknown_1c] = floats(r, at);
    Suspension { progression, spring, bump, rebound, anti_roll, max_compression, max_extension, unknown_1c }
}

pub fn parse_handling(r: &[u8]) -> Handling {
    let idle = read_f32(r, 0x300);
    let max = read_f32(r, 0x308);
    // +0x2D8 holds the forward gear count; ratios start at +0x2E0 with
    // reverse and neutral first.
    let forward = read_u32(r, 0x2D8).min(7) as usize;
    Handling {
        mass: read_f32(r, 0x220),
        dimensions: floats(r, 0x224),
        inertia: [read_f32(r, 0x230), read_f32(r, 0x244), read_f32(r, 0x258)],
        centre_of_mass: floats(r, 0x110),
        drag: read_f32(r, 0x370),
        downforce: [read_f32(r, 0x374), read_f32(r, 0x378)],
        steering_ratio: read_f32(r, 0x380),
        brake: read_f32(r, 0x384),
        handbrake: read_f32(r, 0x388),
        brake_balance: read_f32(r, 0x38C),
        preload_downforce: read_f32(r, 0x37C),
        drift_grip: [read_f32(r, 0x390), read_f32(r, 0x394)],
        yaw_limit_on: read_f32(r, 0x398),
        yaw_limit_off: read_f32(r, 0x39C),
        top_speed_mph: read_f32(r, 0x3A0),
        computed_top_speed: 0.0,
        gearing_top_speed: 0.0,
        type_index: 0,
        performance_class: 3,
        fixed_class: read_u32(r, 0x844) != 0,
        performance_rating: 0.0,
        unknown_270: read_f32(r, 0x270),
        drag_needle: [read_f32(r, 0x7FC), read_f32(r, 0x800)],
        nitrous_capacity: read_f32(r, 0x3B0),
        nitrous_shots: read_u32(r, 0x3B4) as i32,
        nitrous_power: read_f32(r, 0x3BC),
        upgrade_state: [0.0; 10],
        wheel_radii: [read_f32(r, 0x130), read_f32(r, 0x190)],
        race: crate::race::RaceMode::FreeRoam,
        difficulty: 0,
        shift_window_scale: 1.0,
        shift_ranges: [[-100.0, 0.0]; 3],
        clutch_scale: 1.0,
        tyres: [tyre(r, 0x1E0), tyre(r, 0x200)],
        suspension: [suspension(r, 0x280), suspension(r, 0x2A0)],
        engine: Engine {
            idle_rpm: idle,
            redline_rpm: read_f32(r, 0x304),
            max_rpm: max,
            inertia: read_f32(r, 0x30C),
            torque: Curve { x0: idle, x1: max, values: floats(r, 0x310) },
            curve_334: Curve { x0: idle, x1: max, values: floats(r, 0x334) },
            braking: Curve { x0: idle, x1: max, values: floats(r, 0x358) },
            nitrous: Curve { x0: idle, x1: read_f32(r, 0x304), values: crate::nitrous::CURVE },
        },
        gearbox: Gearbox {
            neutral_shift: read_f32(r, 0x2C0),
            unknown_2c4: read_f32(r, 0x2C4),
            final_drive_front: read_f32(r, 0x2C8),
            final_drive_rear: read_f32(r, 0x2CC),
            rear_split: read_f32(r, 0x2D0),
            clutch_stiffness: read_f32(r, 0x2D4),
            ratios: (0..forward + 2).map(|g| read_f32(r, 0x2E0 + g * 4)).collect(),
            clutch_rate: read_f32(r, 0x2DC),
        },
    }
}
