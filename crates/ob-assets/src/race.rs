//! Race modes and the handling changes they bring (see docs/formats.md,
//! "Race modes").
//!
//! The game keys its race-specific physics on flags set from the race's
//! type (race parameters +0xF, at 0x5323A0): type 4 sets 0x89E7D8 (drag),
//! type 5 sets 0x89E7D9 (drift), type 2 sets 0x89E7E3; a fourth flag,
//! 0x89E7E2, is set elsewhere (0x526738). Each has online twins
//! (0x890108, 0x890104/0x890114, 0x89010C, 0x890100/0x890110) that the
//! clone does not need. Free roam has none set.

use crate::handling::Handling;

pub use crate::race_mode::RaceMode;

/// The performance mode handed to 0x5B4DD0: 3 drift, 10 type 2,
/// 2 drag, else 0.
fn tune_table(mode: RaceMode) -> Option<&'static [[f32; 22]; 3]> {
    match mode {
        RaceMode::Drift => Some(&DRIFT_TUNE),
        RaceMode::StreetX => Some(&STREET_X_TUNE),
        RaceMode::Drag => Some(&DRAG_TUNE),
        _ => None,
    }
}

/// The race tunes (0x7FC2E4 drift, 0x7FC33C type 2, 0x7FC394 drag), one row
/// per drivetrain layout (front-, all-, rear-wheel drive), each 11 values at
/// no upgrades followed by 11 at full upgrades.
static DRIFT_TUNE: crate::exe_tables::Table<[[f32; 22]; 3]> = crate::exe_tables::Table::new(|t| &t.drift_tune);
static STREET_X_TUNE: crate::exe_tables::Table<[[f32; 22]; 3]> = crate::exe_tables::Table::new(|t| &t.street_x_tune);
static DRAG_TUNE: crate::exe_tables::Table<[[f32; 22]; 3]> = crate::exe_tables::Table::new(|t| &t.drag_tune);

/// Which row of a layout table (0x5A7760): by the rear torque share,
/// under 0.2 front-, over 0.8 rear-wheel drive.
pub fn layout(h: &Handling) -> usize {
    let s = h.gearbox.rear_split;
    if s > 0.8 {
        2
    } else if s >= 0.2 {
        1
    } else {
        0
    }
}

/// Apply the race's tune (0x5A7540): every value blends the table's two
/// halves (11 at no upgrades, 11 at full upgrades) by the car's upgrade
/// level in that value's category, s(c) = `h.upgrade_state[c]` ×
/// 0.33333334 (0x786348), as ((hi − lo)·s + lo): downforce
/// front and rear (aero, 9), record +0x270 and mass (weight, 0), the
/// centre of mass lowered (suspension, 5; not below 0), the tyres' grip
/// and sliding (tyres, 8) scaled; brake balance (brakes, 6; 0..1) and rear
/// torque share (drivetrain, 1) shifted; and the inertia rebuilt as a box
/// of the car's dimensions. A stock car (all levels 0) takes the first
/// half exactly. Returns whether a tune applied.
///
/// The drift and Street X tunes (0x5A7760, 0x5A77D0) then clear the tuned
/// block's nitrous capacity (+0x2A0, from record +0x3B0: stores at
/// 0x5A77BB / 0x5A782B); the drag tune (0x5A7840) keeps it. The tank
/// (0x591CE0) is then filled with 0 s, so no shot can start (0x59FAB0 →
/// 0x5EB570 refuses in the key handler 0x6034C6) and the style refill
/// (0x592650) adds nothing (**verified** under Wine, docs/notes/parity/streetx-nitrous.md).
pub fn apply_tune(h: &mut Handling, mode: RaceMode) -> bool {
    let Some(table) = tune_table(mode) else { return false };
    let t = &table[layout(h)];
    let s = |c: usize| h.upgrade_state[c] * 0.333_333_34;
    // Column i (0..11) blended with column 11 + i by category c.
    let blend = |i: usize, c: usize| (t[11 + i] - t[i]) * s(c) + t[i];
    let (aero_f, aero_r, k270, mass, com) = (blend(2, 9), blend(3, 9), blend(4, 0), blend(5, 0), blend(6, 5));
    let (grip_f, grip_r, slide_f, slide_r) = (blend(7, 8), blend(8, 8), blend(9, 8), blend(10, 8));
    let (brake_s, share_s) = (s(6), s(1));
    h.downforce[0] *= aero_f;
    h.downforce[1] *= aero_r;
    h.unknown_270 *= k270;
    h.mass *= mass;
    h.centre_of_mass[2] = (h.centre_of_mass[2] - com).max(0.0);
    h.tyres[0].grip *= grip_f;
    h.tyres[1].grip *= grip_r;
    h.tyres[0].sliding *= slide_f;
    h.tyres[1].sliding *= slide_r;
    // +0x27C = s(6)·(t11 − t0) + old + t0; +0x1C0 = s(1)·(t12 − t1) + t1 + old.
    h.brake_balance = ((brake_s * (t[11] - t[0]) + h.brake_balance) + t[0]).clamp(0.0, 1.0);
    h.gearbox.rear_split = (share_s * (t[12] - t[1]) + t[1]) + h.gearbox.rear_split;
    let [x, y, z] = h.dimensions;
    let m = h.mass / 12.0;
    h.inertia = [(z * z + y * y) * m, (z * z + x * x) * m, (y * y + x * x) * m];
    if matches!(mode, RaceMode::Drift | RaceMode::StreetX) {
        h.nitrous_capacity = 0.0;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car with a 2 s tank (record +0x3B0) and 2 shots (+0x3B4).
    fn car_with_nitrous() -> Handling {
        let mut r = vec![0u8; 0x1000];
        r[0x220..0x224].copy_from_slice(&1200.0f32.to_le_bytes());
        r[0x3B0..0x3B4].copy_from_slice(&2.0f32.to_le_bytes());
        r[0x3B4..0x3B8].copy_from_slice(&2i32.to_le_bytes());
        // An engine range and four gears, for the set-up's curves.
        r[0x300..0x304].copy_from_slice(&1000.0f32.to_le_bytes());
        r[0x308..0x30C].copy_from_slice(&8000.0f32.to_le_bytes());
        r[0x2D8..0x2DC].copy_from_slice(&4u32.to_le_bytes());
        for (i, ratio) in [-3.0f32, 0.0, 3.0, 2.0, 1.4, 1.0].iter().enumerate() {
            r[0x2E0 + 4 * i..0x2E4 + 4 * i].copy_from_slice(&ratio.to_le_bytes());
        }
        crate::handling::parse_handling(&r)
    }

    /// 0x5A7760 (drift) and 0x5A77D0 (Street X) clear the nitrous
    /// capacity; drag (0x5A7840) and the untuned modes keep it.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn drift_and_street_x_tunes_empty_the_tank() {
        for mode in [RaceMode::Drift, RaceMode::StreetX] {
            let mut h = car_with_nitrous();
            assert!(apply_tune(&mut h, mode));
            assert_eq!(h.nitrous_capacity, 0.0, "{mode:?}");
            let mut tank = crate::nitrous::Tank::new(h.nitrous_capacity, h.nitrous_shots);
            assert_eq!(tank.size, 0);
            assert!(!tank.start(), "{mode:?}: no shot from an empty tank");
            assert_eq!(tank.level(), 0);
        }
        let mut h = car_with_nitrous();
        assert!(apply_tune(&mut h, RaceMode::Drag));
        assert_eq!(h.nitrous_capacity, 2.0);
        for mode in [RaceMode::FreeRoam, RaceMode::Url] {
            let mut h = car_with_nitrous();
            assert!(!apply_tune(&mut h, mode));
            assert_eq!(h.nitrous_capacity, 2.0, "{mode:?}");
            let mut tank = crate::nitrous::Tank::new(h.nitrous_capacity, h.nitrous_shots);
            assert_eq!(tank.size, 8000);
            assert!(tank.start(), "{mode:?}");
        }
    }

    /// 0x5A7540 blends each tune value between the table's two
    /// halves by the upgrade state of its category, s = L·0.33333334:
    /// stock (L = 0) takes the first half, a fully upgraded car (L = 3)
    /// the second, a half-upgraded one the middle.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn tunes_blend_by_the_upgrade_level() {
        let base = car_with_nitrous();
        let tuned = |levels: [f32; 10]| {
            let mut h = base.clone();
            h.gearbox.rear_split = 0.1; // front-wheel drive: row 0
            h.upgrade_state = levels;
            assert!(apply_tune(&mut h, RaceMode::Drift));
            h
        };
        let stock = tuned([0.0; 10]);
        let full = tuned([3.0; 10]);
        let half_tyres = tuned([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.5, 0.0]);
        // Drift row 0: tyre grip 0.95 at no upgrades, 0.9 at full; +0x270 0.95 / 0.95.
        assert_eq!(stock.tyres[0].grip, base.tyres[0].grip * 0.95);
        assert_eq!(full.tyres[0].grip, base.tyres[0].grip * ((0.9f32 - 0.95) * (3.0 * 0.333_333_34) + 0.95));
        assert_eq!(full.tyres[1].sliding, base.tyres[1].sliding * ((0.9f32 - 1.0) * (3.0 * 0.333_333_34) + 1.0));
        assert_eq!(half_tyres.tyres[0].grip, base.tyres[0].grip * ((0.9f32 - 0.95) * (1.5 * 0.333_333_34) + 0.95));
        // Other categories at 0 keep the stock half.
        assert_eq!(half_tyres.mass, stock.mass);
        assert_eq!(half_tyres.gearbox.rear_split, stock.gearbox.rear_split);
        // The stock path is the old one bit for bit: mass × t5, share + t1.
        assert_eq!(stock.mass, base.mass * 1.0);
        assert_eq!(stock.gearbox.rear_split, 0.1 + 0.15);
    }

    /// The whole race set-up (0x5B4DD0 through `Car::set_race`'s
    /// `setup_performance`) keeps the cleared capacity.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn race_setup_has_no_nitrous_in_drift_and_street_x() {
        for (mode, cap) in [(RaceMode::Drift, 0.0), (RaceMode::StreetX, 0.0), (RaceMode::Drag, 2.0), (RaceMode::FreeRoam, 2.0)] {
            let mut h = car_with_nitrous();
            crate::handling::setup_performance(&mut h, mode);
            assert_eq!(h.nitrous_capacity, cap, "{mode:?}");
        }
    }
}
