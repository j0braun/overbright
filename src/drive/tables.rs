use super::*;

/// One frame at 60 Hz, the step callers advance by; [`CarState::step`]
/// splits it into the game's sub-steps.
pub const STEP: f32 = 1.0 / 60.0;
/// The longest physics sub-step, s (0x7FC070).
pub(super) const MAX_SUBSTEP: f32 = 0.0044;
/// m/s → mph, as the physics writes it (2.237).
pub(super) const MPH: f32 = 2.237;
/// Handbrake share per wheel (0x7FC074): rear only.
pub(super) const HANDBRAKE_WHEELS: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
/// Angular damping over |ω| 0..10 rad/s (0x7FC92C), × angular momentum.
pub(crate) const ANGULAR_DAMPING: [f32; 6] = [0.05, 0.05, 0.25, 0.7, 2.0, 5.0];
/// Where the rest damping settles over max(|v|/4, |ω|/2) 0..1 (0x7FC940).
pub(crate) const REST_DAMPING: [f32; 8] = [1.5, 1.0, 0.6, 0.22, 0.06, 0.03, 0.015, 0.0];

/// Gravity used by the physics (0x7FBEF0).
pub(crate) const GRAVITY: f32 = 9.8;
/// Gravity in crash mode (rigid body +0x160 = 0x7FC084).
pub(crate) const CRASH_GRAVITY: f32 = 11.0;
/// Wheel (and attached driveline) inertia, t·m² (0x7FC6F0).
pub(super) const WHEEL_INERTIA: f32 = 0.01;
/// A wheel slides once |r·ω − v| exceeds this, m/s (0x7FC704).
pub(super) const SLIDE_SLIP: f32 = 0.5;
/// Drive force is boosted by this before the friction circle (0x7FC720).
pub(super) const DRIVE_BOOST: f32 = 1.5;

/// Lateral force curves (0x7FC7D4): one per 2° of slip angle from 0° to
/// 12°+, each over scaled load 0..10 in 6 steps. The 0° curve's values sit
/// in zero-filled data (0x86B34C) that nothing writes, so it is all zeros.
pub(super) static LATERAL: crate::exe_tables::Table<[[f32; 6]; 7]> = crate::exe_tables::Table::new(|t| &t.lateral);

/// One tyre, between the force pass and the friction circle.
#[derive(Clone, Copy, Debug)]
pub(super) struct TyreContact {
    pub(super) contact: Vec3,
    /// The wheel's forward, left (axle) and up axes.
    pub(super) wf: Vec3,
    pub(super) wl: Vec3,
    pub(super) up: Vec3,
    pub(super) fx: f32,
    pub(super) fy: f32,
    pub(super) load: f32,
    /// Drive minus brake torque at the wheel, kN·m (wheel +0x108).
    pub(super) torque: f32,
    /// Rolling: neither sliding nor locked.
    pub(super) gripping: bool,
    pub(super) locked: bool,
    pub(super) sliding: bool,
    pub(super) vx: f32,
    /// Slip angle of the contact's travel, radians.
    pub(super) angle: f32,
}

/// A row of the surface table (0x800EC0, 0x98 bytes each): the factors the
/// physics reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Surface {
    /// Friction-circle grip (+0x44, 0x59D610).
    pub(super) grip: f32,
    /// Sliding friction (+0x48, 0x5A8290).
    pub(super) slide: f32,
    /// Rolling resistance (+0x4C, 0x59D360).
    pub(super) roll: f32,
    /// Air-drag factor while on it (+0x50, 0x5A49B0).
    pub(crate) drag: f32,
}

/// The game's 29 surfaces. 0..7 and 16..21 are plain (asphalt and the
/// like); 8, 9 and 22..28 lose a little grip; 10..12 roll heavier; 13..15
/// lose grip and roll much heavier, 15 also dragging fivefold.
pub(super) const SURFACES: [[f32; 4]; 29] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [0.97, 0.9579, 1.0, 1.0],
    [0.99, 0.9813, 1.0, 1.0],
    [1.0, 1.0, 1.01, 1.0],
    [1.0, 1.0, 1.02, 1.0],
    [1.0, 1.0, 1.02, 1.0],
    [0.96, 0.9513, 1.1, 1.0],
    [0.96, 0.948, 1.5, 1.0],
    [0.96, 0.948, 1.05, 5.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [0.99, 0.981, 1.0, 1.0],
    [0.99, 0.981, 1.0, 1.0],
    [0.99, 0.981, 1.0, 1.0],
    [0.99, 0.9813, 1.0, 1.0],
    [0.99, 0.981, 1.0, 1.0],
    [0.99, 0.981, 1.0, 1.0],
    [0.99, 0.9813, 1.0, 1.0],
];

pub(crate) fn surface(index: u8) -> Surface {
    let [grip, slide, roll, drag] = SURFACES.get(index as usize).copied().unwrap_or(SURFACES[0]);
    Surface { grip, slide, roll, drag }
}

/// The 16 lattice heights of the road-roughness noise (0x800BF8).
pub(super) const BUMP_TABLE: [f32; 16] =
    [-0.5, -0.2, 0.5, 0.2, -0.4, -0.1, 0.2, 0.5, -0.2, -0.5, 0.4, -0.3, 0.3, 0.0, -0.2, 0.1];

/// A surface's roughness: lattice frequency (row +0x18 from 0x800EE0, i.e.
/// 0x800EF8 + 0x98·surface) and amplitude in metres (0x800EFC + 0x98·surface).
pub(super) fn bump_row(index: u8) -> (f32, f32) {
    match index {
        8 | 15 => (1.0, 0.05),
        9 | 10 => (1.0, 0.02),
        11 => (3.0, 0.03),
        12 => (3.0, 0.048),
        13 | 17..=21 => (1.0, 0.01),
        14 => (1.0, 0.03),
        16 => (1.1, 0.03),
        _ => (1.0, 0.0),
    }
}

/// The surface's bump under a point (0x5D3CB0): a bilinear value noise over
/// the scaled x and y. The lattice corner (i, j) takes table entry
/// (−5·i·j − 3·(i+1)·(j+1)) & 15 (wrapping i32 maths); the cell's corner is
/// floor(f·x), floor(f·y) (_ftol 0x75BED4 truncates, then 1 is taken off when
/// that lies above the value). The sum is × the surface's amplitude.
pub(crate) fn surface_bump(x: f32, y: f32, surface: u8) -> f32 {
    let (f, amplitude) = bump_row(surface);
    if amplitude == 0.0 {
        return 0.0;
    }
    // x87: the products stay in extended precision; the floor is stored as f32.
    let (px, py) = (f as f64 * x as f64, f as f64 * y as f64);
    let floor = |v: f64| {
        let t = v.trunc() as f32;
        if t as f64 > v { t - 1.0 } else { t }
    };
    let (fx, fy) = (floor(px), floor(py));
    let (tx, ty) = (px - fx as f64, py - fy as f64);
    let (a, b) = (fx as i32, fy as i32);
    let h = |i: i32, j: i32| {
        let k = i
            .wrapping_mul(j)
            .wrapping_mul(-5)
            .wrapping_sub(i.wrapping_add(1).wrapping_mul(j.wrapping_add(1)).wrapping_mul(3));
        BUMP_TABLE[(k & 15) as usize] as f64
    };
    let v = h(a, b) * (1.0 - tx) * (1.0 - ty)
        + h(a.wrapping_add(1), b) * tx * (1.0 - ty)
        + h(a, b.wrapping_add(1)) * (1.0 - tx) * ty
        + h(a.wrapping_add(1), b.wrapping_add(1)) * tx * ty;
    (v * amplitude as f64) as f32
}

/// The arctangent's table (0x7EACB0), computed from its formula.
pub(super) use crate::exe_tables::generated::atan_table;

/// The game's 16-bit two-argument arctangent (0x43C570, the tyres'
/// slip angle among others): the smaller component over the larger, × 256
/// truncated, looks up the table 0x7EACB0 (bytes, × 32); so a ratio under
/// 1/256 gives 0 and the steps are 32 units (0.0031 rad). Equal components
/// give 0x2000 (0 for 0, 0), then the quadrant: x < 0 → 0x8000 − a,
/// y < 0 → −a, both → a + 0x8000.
pub(crate) fn atan16(x: f32, y: f32) -> u16 {
    let (mut x, mut y, mut q) = (x, y, 0u8);
    if x < 0.0 {
        x = -x;
        q = 1;
    }
    if y < 0.0 {
        q ^= 3;
        y = -y;
    }
    let a: u16 = if x > y {
        (atan_table()[((y / x) * 256.0) as i32 as usize & 0xFF] as u16) << 5
    } else if y > x {
        0x4000 - ((atan_table()[((x / y) * 256.0) as i32 as usize & 0xFF] as u16) << 5)
    } else if y == 0.0 {
        0
    } else {
        0x2000
    };
    match q {
        0 => a,
        3 => 0u16.wrapping_sub(a),
        1 => 0x8000u16.wrapping_sub(a),
        _ => a.wrapping_add(0x8000),
    }
}

pub(crate) fn graph(points: &[(f32, f32)], x: f32) -> f32 {
    if x <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x < x1 {
            return y0 + (x - x0) / (x1 - x0) * (y1 - y0);
        }
    }
    points[points.len() - 1].1
}

/// Evenly sampled table, linear, clamped (the game's 0x5764E0).
pub(crate) fn sample(values: &[f32], x0: f32, x1: f32, x: f32) -> f32 {
    let f = (x - x0) * ((values.len() - 1) as f32 / (x1 - x0));
    if !(f >= 0.0) {
        return values[0];
    }
    let i = f as usize;
    if i >= values.len() - 1 {
        return values[values.len() - 1];
    }
    let t = f - i as f32;
    values[i] * (1.0 - t) + values[i + 1] * t
}

// PlayerSteering curves over speed 0..160 m/s: the maximum wheel angle in
// degrees (0x7E9ED8) and a speed factor used for both the pedal bonus and
// the steering rate (0x7E9F14).
pub(super) const MAX_ANGLE: [f32; 10] = [40.0, 20.0, 10.0, 5.0, 3.5, 3.1, 2.9, 2.9, 2.9, 2.9];
pub(super) const STEER_SPEED: [f32; 10] = [1.0, 0.92, 0.78, 0.64, 0.54, 0.48, 0.42, 0.38, 0.34, 0.32];
/// Stick response over input −1..1 (0x7E9E60): half a stick is 15%.
pub(super) static STEER_RESPONSE: crate::exe_tables::Table<[f32; 21]> = crate::exe_tables::Table::new(|t| &t.steer_response);
/// The same in drift and URL races (0x7E9E9C → 0x7E9330).
pub(super) static STEER_RESPONSE_DRIFT: crate::exe_tables::Table<[f32; 21]> = crate::exe_tables::Table::new(|t| &t.steer_response_drift);
/// Extra lock over |input| 0..1 (0x7E9F00).
pub(super) const LOCK_GAIN: [f32; 6] = [1.0, 1.0, 1.1, 1.2, 1.25, 1.35];
/// The steering rate's factor over |Δinput| / dt 0..10 per second
/// (0x7E9F28), averaged over 0.15 s ([`RateAverage`]).
pub(super) const INPUT_RATE_GAIN: [f32; 6] = [1.0, 1.05, 1.1, 1.5, 2.2, 3.1];
/// Extra steering speed over |input| 0..1 (0x7E9F3C).
pub(super) const RATE_GAIN: [f32; 6] = [1.0, 1.05, 1.1, 1.2, 1.3, 1.4];
/// How much of the slip angle a counter-steer may use, over |input| 0..1
/// (0x7E9EB0).
/// Drift assist yaw damping by layout (front, all, rear) and difficulty
/// (0x7FC2D0, rows 0x144 apart).
pub(super) const DRIFT_ASSIST_DAMPING: [[f32; 3]; 3] = [[-10.0, -7.0, -4.0], [-7.5, -5.0, -4.0], [-15.0, -10.0, -6.0]];
/// Its weight on the slip angle, radians (graph 0x86B3C8, built at
/// 0x77E040; the other copy at 0x86B570 ends at 1.0 instead of 0 and is for
/// cars whose player kind is 2).
pub(super) const DRIFT_ASSIST_SLIP: [(f32, f32); 7] = [
    (0.0, 0.0),
    (0.261_799_4, 0.1),
    (0.523_598_8, 0.45),
    (0.785_398_2, 0.85),
    (1.047_197_6, 0.95),
    (1.553_343, 1.15),
    (1.570_796_4, 0.0),
];
/// The rear grip blend weight and scale by layout (0x7FC2DC, 0x7FC2E0).
pub(super) const DRIFT_ASSIST_REAR: [[f32; 2]; 3] = [[0.35, 1.0], [0.55, 1.0], [0.5, 1.125]];
/// The rear grip curve (0x7FC910), 10 points over 0..1.
pub(super) const DRIFT_REAR_GRIP: [f32; 10] = [1.1, 0.93, 0.86, 0.82, 0.8, 0.792, 0.78, 0.77, 0.765, 0.76];
/// The URL events' gas curve (0x7EA014), 11 points over 0..1.
pub(super) const URL_GAS: [f32; 11] = [0.0, 0.353, 0.593, 0.71, 0.789, 0.847, 0.896, 0.931, 0.96, 0.984, 1.0];
/// The shift-bonus grip divisor by layout (front, all, rear) and
/// performance class (0x7FC2A8).
pub(super) const SHIFT_BONUS_GRIP: [[f32; 4]; 3] =
    [[0.55, 0.7, 0.85, 1.0], [5.0, 4.6, 4.3, 4.0], [0.6, 0.7, 0.85, 1.0]];
/// The wheel hop's grip by wheel slip (0x7FC924 → 0x7FC0CC).
pub(super) const HOP_GRIP: [(f32, f32); 6] =
    [(0.0, 1.0), (5.0, 0.8), (9.0, 0.9), (12.6, 0.833), (17.1, 0.72), (25.0, 0.65)];
/// The wheel hop's split (0x7FC0FC).
pub(super) const HOP_SIDE: f32 = 0.35;
pub(super) const COUNTER_STEER: [f32; 5] = [0.5, 0.6, 0.7, 0.8, 0.95];
/// The drift flag sets above 15° of slip at 30 mph or more, and clears
/// below 5° (0x7E9130, 0x7E9138, 0x7E9134).
pub(super) const DRIFT_ON_DEGREES: f32 = 15.0;
pub(super) const DRIFT_ON_SPEED: f32 = 13.410818;
pub(super) const DRIFT_OFF_DEGREES: f32 = 5.0;
