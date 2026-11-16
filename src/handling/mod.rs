//! The game's drivetrain, from the car-type record (GLOBALB 0x34600) and the
//! physics code that reads it.
//!
//! Units are the game's: tonnes, metres, seconds, so forces are kN and
//! torques kN·m (the 240SX peaks at 0.216 kN·m, the real KA24DE's 216 N·m).
//!
//! The physics code works on a tuning block at record +0x110 (upgrade
//! packages add into it at 0x5997F0); the engine reads record +0x300
//! (constructor 0x5AA5B0) and the gearbox record +0x2C0 (constructor
//! 0x5AF9E0). Record offsets are used here.

use crate::formats::bundle::read_f32;
mod drivetrain;
mod record;

pub use drivetrain::*;
pub use record::*;

#[cfg(test)]
mod gearing_tests;
#[cfg(test)]
mod tests {
    use super::{Curve, Turbo};

    /// 0x5A0F40: off the gas the boost is the vacuum by rpm, on it it
    /// spools to max / 0.19, and letting go starts the blow-off, which
    /// ends within half a second.
    #[test]
    fn turbo_spools_and_blows_off() {
        let mut t = Turbo { max: 0.19, ..Turbo::default() };
        t.update(800.0, 800.0, 7000.0, 0.0, false, 0.01);
        assert_eq!(t.boost, 0.0);
        t.update(3900.0, 800.0, 7000.0, 0.0, false, 0.01);
        assert!((t.boost + 0.25).abs() < 1e-6, "{}", t.boost);
        for _ in 0..400 {
            t.update(6000.0, 800.0, 7000.0, 1.0, false, 0.01);
        }
        assert_eq!(t.boost, 1.0);
        for _ in 0..60 {
            t.update(6000.0, 800.0, 7000.0, 0.0, false, 0.01);
        }
        assert_eq!(t.state, 0);
        assert!(t.boost <= 0.0);
        // No turbo: the boost stays 0.
        let mut none = Turbo::default();
        none.update(6000.0, 800.0, 7000.0, 1.0, false, 0.01);
        assert_eq!(none.boost, 0.0);
    }

    #[test]
    fn curve_clamps_and_interpolates() {
        let c = Curve { x0: 1000.0, x1: 3000.0, values: [1.0, 3.0, 2.0] };
        assert_eq!(c.at(0.0), 1.0);
        assert_eq!(c.at(1500.0), 2.0);
        assert_eq!(c.at(2000.0), 3.0);
        assert_eq!(c.at(2500.0), 2.5);
        assert_eq!(c.at(9000.0), 2.0);
    }

    /// A handling record with only the engine range and four gears set.
    fn test_handling() -> super::Handling {
        let mut r = vec![0u8; 0x1000];
        r[0x300..0x304].copy_from_slice(&1000.0f32.to_le_bytes());
        r[0x308..0x30C].copy_from_slice(&8000.0f32.to_le_bytes());
        r[0x2D8..0x2DC].copy_from_slice(&4u32.to_le_bytes());
        for (i, ratio) in [-3.0f32, 0.0, 3.0, 2.0, 1.4, 1.0].iter().enumerate() {
            r[0x2E0 + 4 * i..0x2E4 + 4 * i].copy_from_slice(&ratio.to_le_bytes());
        }
        super::parse_handling(&r)
    }

    #[test]
    fn nitrous_adds_its_curve_times_the_power() {
        use super::Drivetrain;
        let mut r = vec![0u8; 0x1000];
        r[0x300..0x304].copy_from_slice(&1000.0f32.to_le_bytes());
        r[0x304..0x308].copy_from_slice(&5000.0f32.to_le_bytes());
        r[0x308..0x30C].copy_from_slice(&8000.0f32.to_le_bytes());
        r[0x3BC..0x3C0].copy_from_slice(&0.2f32.to_le_bytes());
        let h = super::parse_handling(&r);
        let mut d = Drivetrain::new(&h);
        d.throttle = 1.0;
        // At redline the curve ends at 0.7 (0x7FBF38[8]).
        d.engine_w = 5000.0 / super::RPM_PER_RAD_S;
        let plain = d.engine_torque(&h.engine, h.nitrous_power);
        d.nitrous = 1;
        let boosted = d.engine_torque(&h.engine, h.nitrous_power);
        assert!((boosted - plain - 0.7 * 0.2).abs() < 1e-5);
    }

    /// 0x402590 returns 1 when the rpm differs from its clamp into [A, A +
    /// 0.95 (B − A)] (0x40264A): `in_nitrous_band` is its negation, true
    /// inside (the AI may fire only there, 0x40D850).
    #[test]
    fn nitrous_band_is_the_clamp_test_of_0x402590() {
        let (h, mut d) = drag_in_first();
        d.down_rpm = vec![0.0; d.up_rpm.len()];
        d.down_rpm[2] = 3000.0;
        d.jitter = [0.0; 3];
        let b = d.up_shift_rpm(&h, 2, false);
        let top = 3000.0 + (b - 3000.0) * 0.95;
        assert!(d.in_nitrous_band(&h, 3000.0));
        assert!(d.in_nitrous_band(&h, (3000.0 + top) / 2.0));
        assert!(d.in_nitrous_band(&h, top));
        assert!(!d.in_nitrous_band(&h, 2999.0));
        assert!(!d.in_nitrous_band(&h, top + 1.0));
    }

    /// A test car in a drag race: redline 7000 (1st gear's up-shift 6900),
    /// class 0 (1st gear's window 360 rpm), the clutch locked in 1st.
    fn drag_in_first() -> (super::Handling, super::Drivetrain) {
        let mut h = test_handling();
        h.engine.redline_rpm = 7000.0;
        h.performance_class = 0;
        h.race = crate::race::RaceMode::Drag;
        let mut d = super::Drivetrain::new(&h);
        d.up_rpm = super::shift_points(&h).0;
        (d.gear, d.target_gear) = (2, 2);
        (h, d)
    }

    /// 0x5ABD83..0x5ABDC0: the gear choice and the drag shift grading
    /// (0x5A1710 → 0x594F30) read the car's shown rpm (+0x440), not the
    /// engine's. An engine 700 rpm past the shift point would grade −1
    /// (the slow clutch); the shown rpm 5 past it grades 1.
    #[test]
    fn drag_up_shift_is_chosen_and_graded_at_the_shown_rpm() {
        let (h, mut d) = drag_in_first();
        d.engine_w = 7600.0 / super::RPM_PER_RAD_S;
        // Shown under the shift point: no shift although the engine is over.
        d.shown_rpm = Some(6800.0);
        d.choose_gear(&h, false, true);
        assert_eq!(d.target_gear, 2);
        d.shown_rpm = Some(6905.0);
        d.choose_gear(&h, false, true);
        assert_eq!(d.target_gear, 3);
        assert_eq!(d.shift_bonus, 1.0);
        // Without a shown rpm (tools) the engine's is read: over-revved.
        let (h, mut d) = drag_in_first();
        d.engine_w = 7600.0 / super::RPM_PER_RAD_S;
        d.choose_gear(&h, false, true);
        assert_eq!((d.target_gear, d.shift_bonus), (3, -1.0));
    }

    /// docs/drag-stage4-parity.md: the drag tachometer's light
    /// (0x4C7090 → 0x594F30 without the flag) reads the shown rpm (+0x440)
    /// and the tachometer's last gear (HUD +0x80), as the grading reads the
    /// shown rpm. Class 0, 1st gear (index 2): up-shift 6900, window 360.
    /// The light is blue from 6400 (max(500, 360) under the point) while a
    /// shift grades above 0 only from 6558 (0.95 · 360 under it): an
    /// up-shift between is "Short Shift!" under a blue light (native).
    #[test]
    fn drag_light_reads_the_shown_rpm_and_blue_starts_before_the_grade() {
        let (h, mut d) = drag_in_first();
        let grade = |d: &mut super::Drivetrain, shown: f32| {
            let mut d = d.clone();
            d.shown_rpm = Some(shown);
            d.shift(&h, true);
            d.shift_bonus
        };
        // The engine leads the shown rpm while revving: the light follows
        // the shown one (off at 6300 although the engine is at 6600).
        d.engine_w = 6600.0 / super::RPM_PER_RAD_S;
        d.shown_rpm = Some(6300.0);
        assert_eq!(d.drag_shift_light(&h, 2), None);
        // 6450: blue (rating 0) and the shift grades 0, "Short Shift!".
        d.shown_rpm = Some(6450.0);
        assert_eq!(d.drag_shift_light(&h, 2), Some(0.0));
        assert_eq!(grade(&mut d, 6450.0), 0.0);
        // 6700: blue (1 − 200 / 342 = 0.415), graded 0.5, "Good Shift!".
        d.shown_rpm = Some(6700.0);
        let r = d.drag_shift_light(&h, 2).unwrap();
        assert!((r - (1.0 - 200.0 / 342.0)).abs() < 1e-4 && r <= 0.5, "{r}");
        assert_eq!(grade(&mut d, 6700.0), 0.5);
        // 6800: green (0.708), graded 0.85, "Perfect Shift!"; 6950: red.
        d.shown_rpm = Some(6800.0);
        assert!(d.drag_shift_light(&h, 2).unwrap() > 0.5);
        assert_eq!(grade(&mut d, 6800.0), 0.85);
        d.shown_rpm = Some(6950.0);
        assert_eq!(d.drag_shift_light(&h, 2), Some(-1.0));
        assert_eq!(grade(&mut d, 6950.0), -1.0);
        // The needle's blue blend from max(500, w) under the point over 0.5 w.
        d.shown_rpm = Some(6650.0);
        assert!((d.drag_needle_blend(&h, 2) - 1.0).abs() < 1e-5);
        // The light is for the gear passed (the HUD's last), not the
        // drivetrain's: in 2nd gear's window (175) 6700 rates 0.
        d.shown_rpm = Some(6700.0);
        assert_eq!(d.drag_shift_light(&h, 3), Some(0.0));
    }

    /// Every up-shift of the finished-career save's Skyline (class 0, up-shift rpm 9400
    /// in every gear) the original graded under Wine in a stage-4 drag
    /// (S4_H_DRAG_15 runs h15_b..f, and two edge points of h15_n / d9_a:
    /// q = 0.5002 and 8.9 rpm past the point; the maintainer's tool `dragshift.py`, evidence
    /// the maintainer's private capture): (gear index, the shown rpm on
    /// the frame before the request, the original's +0x68). The clone's
    /// grading gives the same for each.
    #[test]
    fn drag_grades_match_the_originals_skyline_shifts() {
        const NATIVE: [(usize, f32, f32); 20] = [
            (3, 9316.9, 0.85),
            (2, 9408.9, 1.0),
            (2, 8938.9, 0.0),
            (3, 9022.1, 0.0),
            (4, 9078.0, 0.0),
            (5, 9068.0, 0.0),
            (2, 9150.9, 0.5),
            (3, 9283.5, 0.5),
            (4, 9302.6, 0.5),
            (5, 9432.2, -1.0),
            (2, 9340.8, 0.85),
            (3, 9346.5, 0.85),
            (4, 9388.7, 0.85),
            (5, 9475.9, -1.0),
            (2, 9449.6, -1.0),
            (3, 9426.3, -1.0),
            (4, 9494.1, -1.0),
            (2, 9808.5, -1.0),
            (3, 9900.4, -1.0),
            (4, 9855.8, -1.0),
        ];
        let (mut h, mut d) = drag_in_first();
        h.performance_class = 0;
        h.shift_window_scale = 1.0;
        d.up_rpm = vec![0.0, 9324.0, 9400.0, 9400.0, 9400.0, 9400.0, 9400.0, 10500.0];
        for (gear, rpm, native) in NATIVE {
            assert_eq!(d.shift_quality(&h, rpm, gear), native, "gear {gear} at {rpm}");
        }
    }

    /// 0x5B6130 for a drag AI racer: the skill row's offset ranges, which
    /// the gear change's reroll (0x594720) draws from, and outside the
    /// career the clutch factor +0xE8 = 0x7A19FC[difficulty], which scales
    /// the clutch rate (0x5949D2).
    #[test]
    fn drag_ai_set_up_takes_the_ranges_and_the_clutch_factor() {
        let ranges = [[-500.0, 140.0], [0.0, 0.01], [-650.0, 200.0]];
        let (mut h, _) = drag_in_first();
        super::drag_ai_shift_setup(&mut h, true, Some(ranges));
        assert_eq!((h.shift_ranges, h.clutch_scale), (ranges, 1.0));
        let mut quick = h.clone();
        quick.difficulty = 0;
        super::drag_ai_shift_setup(&mut quick, false, None);
        assert_eq!(quick.clutch_scale, 0.4);
        // Not a drag race: nothing changes.
        let mut roam = test_handling();
        super::drag_ai_shift_setup(&mut roam, false, Some(ranges));
        assert_eq!((roam.shift_ranges, roam.clutch_scale), ([[-100.0, 0.0]; 3], 1.0));
        let mut d = super::Drivetrain::seeded(&h, 7);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..200 {
            d.reroll(&h);
            let o = d.offsets();
            assert!(
                (-500.0..=140.0).contains(&o[0]) && (0.0..=0.01).contains(&o[1]) && (-650.0..=200.0).contains(&o[2]),
                "{o:?}"
            );
            (lo, hi) = (lo.min(o[0]), hi.max(o[0]));
        }
        assert!(lo < -400.0 && hi > 40.0, "spread {lo} {hi}");
        // The clutch closes at 0.4 of the rate.
        let rate = |h: &super::Handling| {
            let mut d = super::Drivetrain::new(h);
            d.shift = super::Shift::Engaging;
            d.clutch = 0.0;
            d.integrate(h, 0.01);
            d.clutch
        };
        let mut g = h.clone();
        g.gearbox.clutch_rate = 4.0;
        let mut q = g.clone();
        q.clutch_scale = 0.4;
        assert!((rate(&q) / rate(&g) - 0.4).abs() < 1e-5, "{} {}", rate(&q), rate(&g));
    }

    #[test]
    fn held_clutch_opens_in_every_state_with_full_power() {
        use super::{Drivetrain, Shift};
        let h = test_handling();
        let mut d = Drivetrain::new(&h);
        d.gear = 2;
        d.target_gear = 2;
        d.throttle = 1.0;
        let input_w = 300.0;
        d.post(&h, input_w, 0.01);
        assert_eq!(d.shift, Shift::Locked);
        // Held (0x41E2C0 → +0x74): Locked ends in Changing, power 1, no
        // torque through the clutch, which stays where it was.
        d.clutch_held = true;
        d.post(&h, input_w, 0.01);
        assert_eq!(d.shift, Shift::Changing);
        assert_eq!(d.power, 1.0);
        assert_eq!(d.clutch_torque(&h), None);
        d.integrate(&h, 0.01);
        assert_eq!(d.clutch, 1.0);
        d.post(&h, input_w, 0.01);
        assert_eq!(d.shift, Shift::Changing);
        // Let go: Changing → Engaging (power 0.7) and the clutch bites.
        d.clutch_held = false;
        d.post(&h, input_w, 0.01);
        assert_eq!(d.shift, Shift::Engaging);
        assert!(d.clutch_torque(&h).is_some());
    }
}

/// The step of the game's performance tables: 3 mph in m/s (0x7A2770).
pub(crate) const PERFORMANCE_STEP: f32 = 1.3410819;

/// The car's acceleration over speed as the game works it out (0x5B5AC0):
/// a 64-point curve over 0..span m/s.
///
/// * A table from a standstill in 3 mph steps through the gears, rpm = v /
///   r · ratio · final drive (r and final drive blended by the rear torque
///   share), changing up at the automatic up-shift rpm, and acceleration =
///   full-throttle torque(rpm) · ratio · final drive / r / mass. The ratio
///   already moves up at the step where the rpm passes the shift point. At
///   most 92 steps; it ends when the gears run out. The game then pads the
///   table with zeros from its last entry on, so that entry reads 0.
/// * That table, spread over 0..(steps · 3 mph), resampled into 64 points.
fn acceleration_curve(h: &Handling) -> (Vec<f32>, f32) {
    let g = &h.gearbox;
    let split = g.rear_split;
    let final_drive = split * g.final_drive_rear + (1.0 - split) * g.final_drive_front;
    let inverse_radius = 1.0 / ((1.0 - split) * h.wheel_radii[0] + split * h.wheel_radii[1]);
    let (up, _) = shift_points(h);

    let mut table = Vec::new();
    let (mut v, mut gear) = (0.0f32, 2usize);
    // (fuzz: a gearbox with fewer than three ratios has no forward gear 2)
    while table.len() < 0x5C && gear < g.ratios.len() && gear < up.len() {
        let rpm = inverse_radius * v * g.ratios[gear] * final_drive * RPM_PER_RAD_S;
        if up[gear] <= rpm {
            gear += 1;
            if gear >= g.ratios.len() {
                break;
            }
        }
        table.push(h.engine.torque.at(rpm) * g.ratios[gear] * final_drive * inverse_radius / h.mass);
        v += PERFORMANCE_STEP;
    }
    let span = v;
    if let Some(last) = table.last_mut() {
        *last = 0.0;
    }
    let curve = (0..64).map(|i| sample_span(&table, span, i as f32 * span / 63.0)).collect();
    (curve, span)
}

/// The gearing-limited top speed (the performance object's +0x10, written
/// at 0x5B5CB5 in 0x5B5AC0): the speed reached when the table loop of
/// [`acceleration_curve`] ends, i.e. the first 3 mph step at which the top
/// gear passes its shift rpm, or 92 steps (traced).
pub fn gearing_top_speed(h: &Handling) -> f32 {
    acceleration_curve(h).1
}

/// Linear lookup in values spread evenly over 0..span, clamped.
fn sample_span(values: &[f32], span: f32, x: f32) -> f32 {
    let n = values.len();
    if n == 0 || span <= 0.0 {
        return 0.0;
    }
    if n == 1 {
        return values[0];
    }
    let f = (x / span * (n - 1) as f32).clamp(0.0, (n - 1) as f32);
    let i = (f as usize).min(n - 2);
    let t = f - i as f32;
    values[i] * (1.0 - t) + values[i + 1] * t
}

/// The shops' acceleration rating's area (0x59A360): from a standstill in
/// 3 mph steps, the acceleration curve less the drag v²·(+0x370)/mass,
/// summed while the curve stays ahead and up to 100 mph (44.70273 m/s).
pub fn acceleration_area(h: &Handling) -> f32 {
    let (curve, span) = acceleration_curve(h);
    let (mut v, mut area) = (0.0f32, 0.0f32);
    loop {
        let a = sample_span(&curve, span, v);
        let drag = v * h.drag * v / h.mass;
        if a < drag {
            break;
        }
        area += (a - drag) * PERFORMANCE_STEP;
        v += PERFORMANCE_STEP;
        if v >= 44.70273 {
            break;
        }
    }
    area
}

/// The car's acceleration at a speed (m/s → m/s²), as the performance
/// object's curve (+0x55C, built by 0x5B5AC0) reads through 0x5764E0; the
/// AI's speed profile accelerates along it (0x40A6E0, 0x422780).
pub fn acceleration(h: &Handling) -> impl Fn(f32) -> f32 + use<> {
    let (curve, span) = acceleration_curve(h);
    move |v| sample_span(&curve, span, v)
}

/// The 0-60 mph and 0-100 mph time factors by car type (0x7FC120 /
/// 0x7FC1D8, indexed by the set-up object's type index +0, the car type's
/// place in the 0x8A1CCC table): 46 floats each, the first 31 set.
static ZERO_60_FACTOR: crate::exe_tables::Table<[f32; 31]> = crate::exe_tables::Table::new(|t| &t.zero_60_factor);
static ZERO_100_FACTOR: crate::exe_tables::Table<[f32; 31]> = crate::exe_tables::Table::new(|t| &t.zero_100_factor);

/// The set-up's launch figures (0x5B06B0, set-up object +0x14..+0x20):
/// seconds to 60 and to 100 mph (each × its car type's factor), seconds
/// and m/s at 400.5 m. 0 when never reached.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LaunchTimes {
    pub zero_60: f32,
    pub zero_100: f32,
    pub quarter: f32,
    pub quarter_speed: f32,
}

/// 0x5B06B0, run by the set-up 0x5B4DD0 after the acceleration rating: a
/// standing start at full throttle in 1/60 s steps (0x7A1ACC), at most
/// 999,999. rpm = v / r · ratio · final drive · 9.549304 (r and final drive
/// blended by the rear torque share, from first gear on); while rpm stays
/// under the gear's up-shift rpm (0x5AC7E0) − 100 (0x7FC294) the push is
/// torque(rpm) · ratio · final drive / r / mass, else a shift step: no
/// push, the clutch s (1 at the start) moving by −3/s (0x7FC298) on the way
/// out — at 0 it turns, s = 0 — then +1.5/s (0x7FC2A0) back, and at 1 the
/// next gear; a shift step always hands the next step to a push at the
/// current gear (the flag it sets is cleared by the push). Each step a =
/// push − v²·drag/mass, x += (v + a·h/2)·h, v += a·h, t += h; the first t
/// with v·2.237 ≥ 60 (× the type's 0x7FC120 factor) is the 0-60 time, ≥
/// 100 (× 0x7FC1D8) the 0-100 time, and x ≥ 400.5 (0x7A20C0) gives the
// prov: inferred handling.launch_times
/// quarter's t and v; it stops once all four are non-zero. **Inferred**: a
/// type index past the tables reads 0 (the game reads on into the next
/// table), gears past the car's read ratio 0.
pub fn launch_times(h: &Handling, type_index: usize) -> LaunchTimes {
    const H: f32 = 1.0 / 60.0;
    let g = &h.gearbox;
    let split = g.rear_split;
    let final_drive = split * g.final_drive_rear + (1.0 - split) * g.final_drive_front;
    let radius = (1.0 - split) * h.wheel_radii[0] + split * h.wheel_radii[1];
    let (up, _) = shift_points(h);
    let f60 = ZERO_60_FACTOR.get(type_index).copied().unwrap_or(0.0);
    let f100 = ZERO_100_FACTOR.get(type_index).copied().unwrap_or(0.0);
    let mut out = LaunchTimes::default();
    let (mut v, mut t, mut x) = (0.0f32, 0.0f32, 0.0f32);
    let (mut clutch, mut out_stroke, mut shifting) = (1.0f32, true, false);
    let mut gear = 2usize;
    for _ in 0..999_999 {
        let ratio = g.ratios.get(gear).copied().unwrap_or(0.0);
        let inverse_radius = 1.0 / radius;
        let rpm = inverse_radius * v * ratio * final_drive * 9.549_304;
        let push = if shifting || -100.0 + up.get(gear).copied().unwrap_or(0.0) > rpm {
            shifting = false;
            h.engine.torque.at(rpm) * ratio * final_drive * inverse_radius / h.mass
        } else {
            shifting = true;
            clutch += if out_stroke { -3.0 } else { 1.5 } * H;
            if clutch > 0.0 {
                if clutch >= 1.0 {
                    shifting = false;
                    gear += 1;
                    clutch = 1.0;
                    out_stroke = true;
                }
            } else {
                clutch = 0.0;
                out_stroke = false;
            }
            0.0
        };
        t += H;
        let dv = (push - v * h.drag * v / h.mass) * H;
        x += (0.5 * dv + v) * H;
        v += dv;
        if out.zero_60 == 0.0 && v * 2.237 >= 60.0 {
            out.zero_60 = t * f60;
        }
        if out.zero_100 == 0.0 && v * 2.237 >= 100.0 {
            out.zero_100 = t * f100;
        }
        if out.quarter == 0.0 && x >= 400.5 {
            out.quarter = t;
            out.quarter_speed = v;
        }
        // The game's own stop (all four non-zero), or (the same
        // figures) a time whose type factor is 0 — a type past the tables,
        // e.g. traffic: `t · 0` stays 0 however long the loop runs, so the
        // game's remaining steps (up to 999,999, ~50 ms a car) change
        // nothing returned.
        let settled = |t: f32, f: f32| t != 0.0 || f == 0.0;
        if settled(out.zero_60, f60) && settled(out.zero_100, f100) && out.quarter != 0.0 && out.quarter_speed != 0.0 {
            break;
        }
    }
    out
}

/// The upgrade state the race tunes blend by (tuned object +0x2E0, filled
/// by 0x5AC500 through the category functions: 3 × each category's mean
/// level, 0x5A6590), for the parts `fitted`. The library half of
/// Whoever builds a car's handling from its upgraded record
/// stores this in [`Handling::upgrade_state`] (stock and race handling
/// alike), so `Car::set_race` → [`setup_performance`] tunes by it.
pub fn upgrade_state(packages: &[crate::performance::Package], fitted: &dyn Fn(u8) -> bool) -> [f32; 10] {
    std::array::from_fn(|c| crate::performance::state_float(packages, fitted, c as u8))
}

/// Class thresholds on the area between acceleration and drag (0x7A1A8C).
const CLASS_AREA: [f32; 4] = [0.0, 273.0, 401.0, 500.0];

/// Set a car up for a race (0x5B4DD0): its performance class from the
/// stock acceleration curve — the area, in 3 mph steps, between it and the
/// drag v²·(+0x370)/mass up to where drag wins; class 0 above 500, 1 above
/// 401, 2 above 273, else 3 — then the race's tune ([`crate::race`]) and,
/// from the curve again, the top speed (0x5B5940): the first 3 mph step at
/// which the drag exceeds the curve, capped at record +0x3A0 mph.
pub fn setup_performance(h: &mut Handling, race: crate::race::RaceMode) {
    h.race = race;
    let (curve, span) = acceleration_curve(h);
    if h.fixed_class {
        h.performance_class = 3;
    } else {
        let (mut v, mut area) = (0.0f32, 0.0f32);
        while v <= span {
            let a = sample_span(&curve, span, v);
            let drag = v * h.drag * v / h.mass;
            if a < drag {
                break;
            }
            area += (a - drag) * PERFORMANCE_STEP;
            v += PERFORMANCE_STEP;
        }
        h.performance_class = (0..4).rev().find(|&i| CLASS_AREA[i] < area).map_or(3, |i| 3 - i);
        h.performance_rating = area;
    }
    let (curve, span) = if crate::race::apply_tune(h, race) { acceleration_curve(h) } else { (curve, span) };
    // P+0x10: 0x5B5AC0 writes the curve's span each time it runs, so the
    // last build's (after the race's tune) stays (0x5B5CB5).
    h.gearing_top_speed = span;
    let mut speed = 0.0f32;
    while speed <= span {
        if speed * h.drag * speed / h.mass > sample_span(&curve, span, speed) {
            break;
        }
        speed += PERFORMANCE_STEP;
        if speed > span {
            break;
        }
    }
    if h.top_speed_mph > 0.0 {
        speed = speed.min(h.top_speed_mph * 0.44702727);
    }
    h.computed_top_speed = speed;
}
