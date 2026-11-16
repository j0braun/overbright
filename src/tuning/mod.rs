//! The dyno's performance tuning: which tuning parameters a car may
//! adjust, from the performance parts fitted to it.
//!
//! Career chunk 0x34A1A (read by 0x512C00 into 0x86B340 / 0x86B344 through
//! 0x59AE30) holds 24-byte records: +0x00 a performance part's id, +0x04
//! its level (1–3), +0x05 the tuning parameter it opens (0–24), +0x06 the
//! slider's kind, +0x08 / +0x0C the parameter's range at that level and
//! +0x10 / +0x14 the range the slider shows (−99.99 when it shows none).
//! A parameter is open when a part of one of its records is fitted; the
//! fitted record of the highest level gives the range (0x59AE50). The
//! gear-ratio parameters 10–15 also need the gear: parameter − 8 below the
//! car's forward gears + 2 (0x59B090, tuning block +0x1C8).

use crate::formats::bundle::{read_f32, read_u32};
mod parse;
mod settings;
mod view_model;
mod writer;

pub use parse::*;
pub use settings::*;
pub use view_model::*;
pub use writer::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn unlock(part: u8, level: u8, param: u8) -> Unlock {
        Unlock { part, level, param, kind: 1, range: (0.0, 1.0), display: (-99.99, -99.99) }
    }

    #[test]
    fn the_highest_fitted_level_opens() {
        let table = [unlock(8, 1, 9), unlock(9, 2, 9), unlock(11, 3, 9)];
        let fitted = |p: u8| p == 8 || p == 9;
        assert_eq!(opener(&table, &fitted, 9).map(|u| u.level), Some(2));
        assert!(opener(&table, &|_| false, 9).is_none());
    }

    #[test]
    fn gear_ratios_need_the_gear() {
        let table: Vec<Unlock> = (10..16).map(|p| unlock(11, 3, p)).collect();
        let fitted = |p: u8| p == 11;
        // Five forward gears: parameters 10..=14 (param - 8 < 7).
        assert!(tunable(&table, &fitted, 5, 14));
        assert!(!tunable(&table, &fitted, 5, 15));
        assert!(tunable(&table, &fitted, 6, 15));
    }

    #[test]
    fn restore_resets_the_parameter_and_the_nitrous_partner() {
        let mut s = Settings::default();
        s.set_t(23, 4, 0.5);
        s.set_t(23, 0, -0.3);
        s.set_t(16, 0, 0.4);
        s.set_t(17, 0, -0.4);
        s.set_t(3, 0, 1.0);
        s.restore(23);
        assert!((0..9).all(|i| s.t(23, i) == 0.0));
        assert_eq!(s.t(3, 0), 1.0);
        s.restore(17);
        assert_eq!((s.t(16, 0), s.t(17, 0)), (0.0, 0.0));
        s.restore(3);
        assert!(s.is_default());
    }

    #[test]
    fn slider_bytes_round_trip() {
        let mut s = Settings::default();
        assert_eq!(s.t(18, 0), 0.0);
        s.set_t(18, 0, 0.5);
        assert_eq!(s.0[18], 150);
        assert!((s.t(18, 0) - 0.5).abs() < 1e-6);
        s.set_t(23, 4, -1.0);
        assert_eq!(s.0[0x17 + 4], 0);
        s.set_t(24, 8, 2.0);
        assert_eq!(s.0[0x28], 200);
    }

    #[test]
    fn brake_balance_moves_inside_its_range() {
        let mut r = vec![0u8; 0x400];
        put(&mut r, 0x38C, 0.5);
        let stock = r.clone();
        let table = [Unlock { part: 40, level: 1, param: 18, kind: 4, range: (0.4, 0.6), display: (-1.0, 1.0) }];
        let mut s = Settings::default();
        s.set_t(18, 0, 1.0);
        apply(&mut r, &stock, &table, &|p| p == 40, &s);
        assert!((get(&r, 0x38C) - 0.6).abs() < 1e-6);
        s.set_t(18, 0, -0.25);
        let mut r2 = stock.clone();
        apply(&mut r2, &stock, &table, &|p| p == 40, &s);
        assert!((get(&r2, 0x38C) - 0.45).abs() < 1e-6);
    }

    fn opens(part: u8, param: u8, kind: u8, range: (f32, f32)) -> Unlock {
        Unlock { part, level: 3, param, kind, range, display: (-99.99, -99.99) }
    }

    #[test]
    fn nitrous_trades_against_its_partner() {
        let mut r = vec![0u8; 0x890];
        put(&mut r, 0x3B0, 6.0);
        put(&mut r, 0x3BC, 0.5);
        let stock = r.clone();
        let table = [opens(14, 16, 4, (1.0, 11.0)), opens(14, 17, 4, (0.1, 0.9))];
        let mut s = Settings::default();
        s.set_t(16, 0, 0.5);
        apply(&mut r, &stock, &table, &|p| p == 14, &s);
        assert!((get(&r, 0x3B0) - 11.0).abs() < 1e-5);
        assert!((get(&r, 0x3BC) - 0.1).abs() < 1e-5);
    }

    #[test]
    fn a_raised_gear_pushes_the_one_below() {
        let mut r = vec![0u8; 0x890];
        r[0x2D8..0x2DC].copy_from_slice(&2u32.to_le_bytes());
        put(&mut r, 0x110 + 0x50, 0.3);
        put(&mut r, 0x110 + 0x1F4, 7000.0);
        put(&mut r, 0x110 + 0x1B8, 4.0);
        put(&mut r, 0x2E8, 3.0);
        put(&mut r, 0x2EC, 2.0);
        let stock = r.clone();
        let table = [opens(11, 10, 1, (-1.0, 1.0)), opens(11, 11, 1, (-1.0, 1.0))];
        let mut s = Settings::default();
        s.set_t(11, 0, 1.0);
        apply(&mut r, &stock, &table, &|p| p == 11, &s);
        let (first, second) = (get(&r, 0x2E8), get(&r, 0x2EC));
        assert!(second > 2.5 && first >= second + 0.05 - 1e-5, "{first} {second}");
    }

    #[test]
    fn the_ecu_keeps_the_torque_total() {
        let mut r = vec![0u8; 0x890];
        put(&mut r, 0x300, 1000.0);
        put(&mut r, 0x308, 7000.0);
        for k in 0..9 {
            put(&mut r, 0x310 + 4 * k, 0.1 + 0.01 * k as f32);
        }
        let stock = r.clone();
        let table = [opens(51, 23, 1, (-0.08, 0.05))];
        let mut s = Settings::default();
        for k in 0..9 {
            s.set_t(23, k, if k < 4 { -1.0 } else { 0.5 });
        }
        let sum = |r: &[u8]| (0..9).map(|k| get(r, 0x310 + 4 * k)).sum::<f32>();
        let before = sum(&r);
        apply(&mut r, &stock, &table, &|p| p == 51, &s);
        assert!((sum(&r) - before).abs() < 1e-4);
        assert!(get(&r, 0x310) < 0.1);
    }
    #[test]
    fn a_raised_ecu_point_lifts_its_neighbours_and_keeps_the_total() {
        let mut r = vec![0u8; 0x890];
        put(&mut r, 0x300, 1000.0);
        put(&mut r, 0x308, 7000.0);
        for k in 0..9 {
            put(&mut r, 0x310 + 4 * k, 0.3);
        }
        let stock = r.clone();
        let table = [opens(51, 23, 4, (0.0, 1.0))];
        let s = nudge(&r, &stock, &table, &|p| p == 51, &Settings::default(), 23, 4, 1.0);
        let mut tuned = r.clone();
        apply(&mut tuned, &stock, &table, &|p| p == 51, &s);
        let pts: Vec<f32> = (0..9).map(|k| get(&tuned, 0x310 + 4 * k)).collect();
        assert!(pts[4] > 0.3 && pts[3] > 0.3 && pts[5] > 0.3, "{pts:?}");
        assert!(pts[0] < 0.3 && pts[8] < 0.3, "{pts:?}");
        assert!((pts.iter().sum::<f32>() - 2.7).abs() < 0.05, "{pts:?}");
    }
}

#[cfg(test)]
mod curve_tests {
    use super::*;

    /// A car whose ECU envelope is lo_k = 100 + 10k and hi_k = lo_k + 50
    /// (native 100 .. 230), opened by kind 4 over 100 .. 230.
    fn ecu_car() -> (Vec<u8>, Vec<u8>, Vec<Unlock>) {
        let mut stock = vec![0u8; 0x900];
        for k in 0..9 {
            put(&mut stock, 0x110 + 0x200 + 4 * k, 100.0 + 10.0 * k as f32);
            put(&mut stock, 0x110 + 0x4E0 + 4 * k, 50.0);
        }
        let mut record = stock.clone();
        for k in 0..9 {
            put(&mut record, 0x310 + 4 * k, 150.0 + k as f32);
        }
        let table =
            vec![Unlock { part: 8, level: 1, param: 23, kind: 4, range: (100.0, 230.0), display: (-99.99, -99.99) }];
        (record, stock, table)
    }

    #[test]
    fn the_axis_rounds_out_to_fifties() {
        assert_eq!(curve_axis(73.9, 219.9), (50.0, 250.0));
        assert_eq!(curve_axis(200.0, 200.0), (200.0, 250.0));
        assert_eq!(curve_axis(0.0, 0.0), (0.0, 50.0));
        assert_eq!(curve_axis(-5.0, 30.0), (0.0, 50.0));
        assert_eq!(curve_axis(100_000.0, 230_000.0), (100_000.0, 230_050.0));
        // C truncation (0x550590): -50 / 50 is -1, so the max is 0; -100 gives -50.
        assert_eq!(curve_axis(0.0, -50.0), (0.0, 0.0));
        assert_eq!(curve_axis(0.0, -100.0), (0.0, -50.0));
    }

    #[test]
    fn the_ecu_display_maps_the_native_envelope() {
        let (record, stock, table) = ecu_car();
        let fitted = |p: u8| p == 8;
        let mut rec = record.clone();
        let t = Tuner {
            rec: &mut rec,
            stock: &stock,
            table: &table,
            fitted: &fitted,
            gears: 0,
            s: Settings::default(),
            bases: [0.0; 23],
            curve_bases: [[0.0; 9]; 2],
            spread: false,
        };
        // Metric: the native 100 .. 230 becomes 100000 .. 230000 (× 1000).
        assert_eq!(t.display_limits(23, true), (100_000.0, 230_000.0));
        assert_eq!(t.to_display(23, 100.0, true), 100_000.0);
        assert_eq!(t.to_display(23, 230.0, true), 230_000.0);
        assert_eq!(t.to_display(23, 50.0, true), 100_000.0);
        assert_eq!(t.to_display(23, 165.0, true), 165_000.0);
    }

    #[test]
    fn the_curve_axis_and_points_come_from_the_record() {
        let (record, stock, table) = ecu_car();
        let fitted = |p: u8| p == 8;
        let graph = curve_graph(&record, &stock, &table, &fitted, &Settings::default(), 23, true);
        assert_eq!(graph.axis, (100_000.0, 230_050.0));
        // Kind 4 gives every point the opener's 100 .. 230; with the slider
        // at 0 (byte 100) the value is the point's base.
        let remap = |x: f32| 100_000.0 + (x.clamp(100.0, 230.0) - 100.0) * 1000.0;
        for k in 0..9 {
            let b = base(&record, &stock, 23, k).unwrap_or(0.0);
            let p = graph.points[k];
            assert!((p.low - 100_000.0).abs() < 0.5, "low {k}");
            assert!((p.high - 230_000.0).abs() < 0.5, "high {k}");
            assert!((p.value - remap(b)).abs() < 0.5, "value {k}");
        }
    }

    #[test]
    fn an_untunable_curve_has_the_game_fallbacks() {
        let (record, stock, _) = ecu_car();
        let graph = curve_graph(&record, &stock, &[], &|_| false, &Settings::default(), 23, true);
        assert_eq!(graph.axis, (0.0, 50.0));
        assert!(graph.points.iter().all(|p| *p == CurvePoint { value: 0.0, low: 0.0, high: 1.0 }));
    }
}
