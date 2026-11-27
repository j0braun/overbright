use super::*;

#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn native_wheel_velocity_is_measured_at_hub() {
    let mut max_error = 0.0f32;
    let mut old_failures = 0;
    let mut count = 0;
    for line in crate::testkit::private_str("docs/evidence/full-phase-inputs/hub.csv")
        .lines()
        .skip(1)
    {
        let r: Vec<f32> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let v = |o| Vec3::new(r[o], r[o + 1], r[o + 2]);
        let rot = Mat3::from_cols(v(8), v(11), v(14));
        let mount = v(27) - Vec3::Z * r[33];
        let local = mount - v(30) + Vec3::Z * r[34];
        assert!(
            (local - v(17)).abs().max_element() <= 1e-7,
            "native local hub construction: step {} wheel {}",
            r[0],
            r[1]
        );
        let arm = wheel_hub_arm(rot, mount, v(30), r[34]);
        assert!(
            (arm - rot * v(17)).abs().max_element() <= 1e-7,
            "native transformed hub"
        );
        let expected = v(24);
        let actual = wheel_hub_velocity(v(2), v(5), arm);
        let error = (actual - expected).abs().max_element();
        max_error = max_error.max(error);
        assert!(
            error <= 1e-5,
            "step {} wheel {}: {actual:?} != {expected:?}",
            r[0],
            r[1]
        );
        let old = wheel_hub_velocity(v(2), v(5), arm - v(20) * r[23]);
        if (old - expected).abs().max_element() > 1e-5 {
            old_failures += 1;
        }
        count += 1;
    }
    assert_eq!(count, 2236);
    assert!(
        old_failures > 0,
        "counterfactual contact-point velocity must fail"
    );
    eprintln!(
        "{count} native hub velocities: max error {max_error}; old contact-point failures {old_failures}"
    );
}
