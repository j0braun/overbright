//! Native airborne engine reaction: current suspension contacts differ from
//! the exported car count, which remains fixed until the world update ends.
use super::*;

#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn native_airborne_drivetrain_uses_exported_contact_count() {
    const RECORD: usize = 960;
    // A recorded capture of the original (private), replayed on the stock
    // PEUGOT record (regenerated from the game).
    let raw = crate::testkit::private("docs/evidence/cached-wheel-count/airborne.bin");
    let handling = crate::testkit::handling_peugot();
    assert_eq!(raw.len(), RECORD * 10);
    let mut reaction_error = 0.0f32;
    for row in raw.chunks_exact(RECORD) {
        let u = |o| u32::from_le_bytes(row[o..o + 4].try_into().unwrap());
        let f = |o| f32::from_bits(u(o));
        let step = u(0);
        let engine = &row[32..208];
        let clutch = &row[208..464];
        let mut h = crate::handling::parse_handling(&handling);
        h.performance_class = u(208 + 0xf0) as usize;
        let mut state = CarState::new(Vec3::ZERO, 0.0);
        state.aero_down = u(20) as usize;
        for i in 0..4 {
            state.wheels[i].on_ground = u(464 + i * 4) != 0;
            state.wheels[i].spin = f(480 + i * 4);
            state.wheels[i].slip = f(496 + i * 4);
        }
        assert_eq!(state.wheels_down(), 0);
        assert_eq!(state.aero_down, 4);
        let mut drive = Drivetrain::native_force_boundary(&h, engine, clutch);
        // These actual captures are engaging, not locked: all five rolling
        // constraint passes return no extra coupling torque.
        assert_eq!(drive.shift, crate::handling::Shift::Engaging);
        assert_eq!(drive.gear, 2);
        assert_eq!(drive.coupling_torque(&h, 0.0, WHEEL_INERTIA), None);
        let torque = drive.clutch_torque(&h).unwrap();
        let shares = std::array::from_fn(|i| state.differential_share(i));
        let torques = drive.distribute(&h.gearbox, torque, state.drivetrain_wheels_down(), shares);
        for (i, actual) in torques.into_iter().enumerate() {
            assert!((actual - f(512 + i * 4)).abs() <= 1e-6, "step {step} wheel {i}: {actual} != {}", f(512 + i * 4));
        }
        let input = drive.to_crank(&h.gearbox, state.wheels.map(|w| w.spin));
        let slip = (state.wheels[0].slip + state.wheels[1].slip) * 0.5;
        assert_eq!(drive.settle(&h, input, slip, state.drivetrain_wheels_down()), None);
        let expected = f(528 + 0x7c);
        reaction_error = reaction_error.max((drive.reaction - expected).abs());
        assert!((drive.reaction - expected).abs() <= 2e-8, "step {step} reaction {} != {expected}", drive.reaction);
        drive.integrate(&h, f(28));
        assert_eq!(drive.engine_w.to_bits(), f(528 + 0x14).to_bits(), "step {step} engine speed");
        assert_eq!(drive.clutch.to_bits(), f(704 + 0x14).to_bits(), "step {step} clutch engagement");
    }
    eprintln!("10 native airborne states: engine/clutch integration bit-exact; max reaction error {reaction_error}");
}
