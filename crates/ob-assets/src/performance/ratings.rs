use crate::formats::bundle::read_f32;

/// The stat ranges the shops' ratings normalise by: career chunk 0x34A32,
/// 30 floats the loader (0x599472) copies to 0x86B3D0. [0..1] top speed
/// (m/s), [2..3] the acceleration area, [4..5] peak torque (kN·m), [6..7]
/// peak power (kW), [8..9] the nitrous product, [12..27] the handling
/// terms (see [`ratings`]).
pub type StatRanges = [f32; 30];

/// The shops' three bars, 0.03–1 (0x5B53B0, 0x5B6A30, 0x5B54A0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ratings {
    pub acceleration: f32,
    pub top_speed: f32,
    pub handling: f32,
}

/// (x − lo) / (hi − lo) for the range starting at `ranges[i]`.
pub fn unit(ranges: &StatRanges, i: usize, x: f32) -> f32 {
    (x - ranges[i]) / (ranges[i + 1] - ranges[i])
}

/// The ratings' clamp: at most 1, and 0.03 for anything not above 0.03.
pub fn rating(x: f32) -> f32 {
    if x > 0.03 { x.min(1.0) } else { 0.03 }
}

/// Rate an (upgraded) car-type record as the shops do, on the free-roam
/// set-up (0x5B4FD0 with no race flags hands 0x5B4DD0 mode 0). The handling
/// object these read is record + 0xE0; offsets below are the record's.
/// * Acceleration (0x5B53B0): the area of [`crate::handling::acceleration_area`]
///   over ranges [2..3] × 0.82, the nitrous product (+0x3BC × +0x3B0) over
///   [8..9] × 0.1 and round(+0x3C4) / 3 × 0.08.
/// * Top speed (0x5B6A30): the computed top speed (0x5B5940) over [0..1]
///   × 0.915 and the nitrous product over [8..9] × 0.085.
/// * Handling (0x5B54A0): nine terms weighted 0.14, 0.1, 0.15, 0.15, 0.1,
///   0.15, 0.15, 0.08, 0.08 (0x7A1A9C): the tyres' grip and sliding
///   (+0x1EC, +0x1F0, +0x20C, +0x210) / 4 / mass over [14..15], the brakes
///   (+0x384 × 0.65 + +0x388 × 0.35) / mass over [16..17], the springs
///   (+0x284 × +0x280 + +0x290, front and rear) / 4 / mass over [12..13],
///   1 − mass over [18..19], the rear torque share s (+0x2D0) as
///   (s + 0.2) × 1.25 up to 0.6 else (1 − s) × 2.5, 1 − the centre of mass
///   height (+0x118) over [20..21], 1 − +0x270 over [22..23], and the front
///   and rear downforce (+0x374, +0x378) over [24..25] and [26..27].
pub fn ratings(record: &[u8], ranges: &StatRanges) -> Ratings {
    let mut h = crate::handling::parse_handling(record);
    crate::handling::setup_performance(&mut h, crate::race_mode::RaceMode::FreeRoam);
    let f = |o: usize| read_f32(record, o);
    let mass = h.mass;
    let nitrous = unit(ranges, 8, f(0x3BC) * f(0x3B0));
    let acceleration = unit(ranges, 2, crate::handling::acceleration_area(&h)) * (1.0 - (0.08 + 0.1))
        + nitrous * 0.1
        + ((f(0x3C4) + 0.5) as i32) as f32 * 0.333_333_34 * 0.08;
    let top_speed = unit(ranges, 0, h.computed_top_speed) * (1.0 - 0.085) + nitrous * 0.085;
    let split = f(0x2D0);
    let terms = [
        unit(ranges, 14, (f(0x1F0) + f(0x1EC) + f(0x210) + f(0x20C)) * 0.5 / mass * 0.5),
        unit(ranges, 16, (f(0x384) * 0.65 + f(0x388) * 0.35) / mass),
        unit(ranges, 12, (f(0x284) * f(0x280) + f(0x290) + f(0x2A4) * f(0x2A0) + f(0x2B0)) * 0.5 / mass * 0.5),
        1.0 - unit(ranges, 18, mass),
        if split <= 0.6 { (split + 0.2) * 1.25 } else { (1.0 - split) * 2.5 },
        1.0 - unit(ranges, 20, f(0x118)),
        1.0 - unit(ranges, 22, f(0x270)),
        unit(ranges, 24, f(0x374)),
        unit(ranges, 26, f(0x378)),
    ];
    const WEIGHTS: [f32; 9] = [0.14, 0.1, 0.15, 0.15, 0.1, 0.15, 0.15, 0.08, 0.08];
    let handling = terms.iter().zip(WEIGHTS).fold(0.0, |sum, (t, w)| w * t + sum);
    Ratings { acceleration: rating(acceleration), top_speed: rating(top_speed), handling: rating(handling) }
}

/// Peak torque (kN·m) and peak power (kW) of a record's engine (0x5B5820,
/// 0x5B56F0): the full-throttle torque sampled from idle in 50 rpm steps
/// while under the maximum rpm; power is torque × 1000 × rpm × 2π / 60000.
pub fn peaks(record: &[u8]) -> (f32, f32) {
    let e = crate::handling::parse_handling(record).engine;
    let (mut torque, mut power) = (0.0f32, 0.0f32);
    let mut rpm = e.idle_rpm;
    while rpm < e.max_rpm {
        let t = e.torque.at(rpm);
        torque = torque.max(t);
        power = power.max(t * 1000.0 * rpm * 0.000_104_712_04);
        rpm += 50.0;
    }
    (torque, power)
}

/// A peak as its bar shows it (the `normalise` flag of 0x5B5820 /
/// 0x5B56F0): clamped to the range at `ranges[i]` and scaled to 0–1.
pub fn bar(ranges: &StatRanges, i: usize, peak: f32) -> f32 {
    (peak.clamp(ranges[i], ranges[i + 1]) - ranges[i]) / (ranges[i + 1] - ranges[i])
}
