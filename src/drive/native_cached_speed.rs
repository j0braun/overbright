//! Retained native landing inputs; no computed expected forces.
use super::*;

#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn native_landing_sliding_uses_cached_forward_speed() {
    let bytes = &crate::testkit::private("docs/evidence/cached-forward-speed/landing.bin");
    assert_eq!(bytes.len(), 6 * 112);
    let mut max_error = 0.0f32;
    for row in bytes.chunks_exact(112) {
        let u = |o| u32::from_le_bytes(row[o..o+4].try_into().unwrap());
        let f = |o| f32::from_bits(u(o));
        let v = |o| Vec3::new(f(o),f(o+4),f(o+8));
        let (step, wheel) = (u(0),u(4));
        let mut state = CarState::new(Vec3::ZERO, 0.0);
        // The oracle supplies the native retained scalar. Runtime frame
        // production/outer export lifetime is not established by this test.
        state.tyre_speed = f(16);
        state.velocity = v(20);
        state.orientation = Quat::from_rotation_arc(Vec3::X, v(32).normalize());
        let live_speed = state.velocity.dot(v(32));
        assert!(live_speed * MPH > 1.0);
        assert!(state.exported_forward_speed() * MPH < 1.0);
        let surface = surface(u(52) as u8);
        let mu = surface.slide * f(96) * f(56)
            * launch_grip(state.exported_forward_speed(), u(48) != 0, f(44));
        let (slip, lateral, load) = (f(60),f(64),f(68));
        let (fx,fy) = sliding_forces(slip,lateral,load,mu,state.exported_forward_speed());
        // These rear tyres have zero drive torque, |vx|<1, and a circle
        // limit above the sliding result, so final native Fx/Fy directly
        // observe the sliding-force consumer (no constraint/drag adjustment).
        assert!(slip.abs() < 1.0 && f(44)==0.0);
        let limit = surface.grip * f(92) * f(56) * load
            * launch_grip(state.exported_forward_speed(),false,f(44));
        assert!((fx*fx+fy*fy).sqrt() < limit);
        let error=(fx-f(72)).abs().max((fy-f(76)).abs());
        max_error=max_error.max(error);
        assert!(error<=1e-4,"step {step} wheel {wheel}: ({fx},{fy}) != ({},{})",f(72),f(76));
        let old=sliding_forces(slip,lateral,load,mu,live_speed);
        assert!((old.0-f(72)).abs()>1.0, "fixture must distinguish live speed");
    }
    eprintln!("6 native rear landing forces: maximum error {max_error}");
}

#[test]
fn tyre_speed_snapshot_survives_substep_velocity_changes() {
    let mut state=CarState::new(Vec3::ZERO,0.0);
    state.velocity=state.forward()*0.1;
    state.publish_forward_speed();
    let retained=state.exported_forward_speed();
    for speed in [0.6, 15.0, -2.0] {
        state.velocity=state.forward()*speed;
        assert_eq!(state.exported_forward_speed(),retained);
        assert_ne!(state.speed(),retained);
    }
    state.publish_forward_speed();
    assert_eq!(state.exported_forward_speed(),state.speed());
}
