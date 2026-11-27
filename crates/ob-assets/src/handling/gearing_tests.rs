//! The gearing-limited top speed against the original.

/// Native (0x5B5AC0, writer 0x5B5CB5 of the performance object's +0x10,
/// the racer info's +0x30 = [car +0x10] +0x20): v = 0, gear 2; per step the
/// rpm v / r · ratio · final drive; at the gear's shift rpm the gear moves
/// up (past the top gear the loop ends); else a table entry and v += 3 mph
/// (1.3410819 m/s), at most 92 entries; +0x10 = v. Measured in the running
/// original (gdb watch on +0x10 during a career race build, stock cars).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn gearing_top_speed_matches_the_original() {
    let root = crate::testkit::game_root();
    let measured: [(&str, u32); 11] = [
        ("240SX", 50),
        ("PEUGOT", 55),
        ("FOCUS", 56),
        ("MIATA", 56),
        ("IS300", 59),
        ("A3", 64),
        ("TT", 74),
        ("RX8", 66),
        ("G35", 76),
        ("SKYLINE", 71),
        ("IMPREZAWRX", 68),
    ];
    let mut rows = Vec::new();
    for (name, steps) in measured {
        let car = crate::car::Car::load_physics(&root, name, &[], &|_| false, None).unwrap();
        let h = &car.car_type.handling;
        let mut want = 0.0f32;
        for _ in 0..steps {
            want += super::PERFORMANCE_STEP;
        }
        rows.push((name, h.gearing_top_speed, super::gearing_top_speed(h), want));
    }
    eprintln!("{rows:?}");
    for (name, field, f, want) in rows {
        assert_eq!(field.to_bits(), want.to_bits(), "{name}: {field} against the original's {want}");
        assert_eq!(f.to_bits(), want.to_bits(), "{name}");
    }
}
