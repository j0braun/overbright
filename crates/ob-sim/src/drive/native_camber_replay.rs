use super::*;

#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn camber_load_matches_nonunit_native_frames() {
    let mut count = 0;
    let mut legacy_differences = 0;
    for line in crate::testkit::private_str("docs/evidence/full-phase-inputs/camber.csv")
        .lines()
        .skip(1)
    {
        let row: Vec<f32> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let axle = Vec3::new(row[5], row[6], row[7]);
        let normal = Vec3::new(row[8], row[9], row[10]);
        let actual = tyre_camber_load(row[4], axle, normal);
        assert_eq!(
            actual.to_bits(),
            row[11].to_bits(),
            "step {} wheel {}: {actual} != {}",
            row[0],
            row[1],
            row[11]
        );
        let a = axle.dot(normal);
        let legacy = row[4] * (1.0 - 4.0 * a * a).max(0.3);
        if legacy.to_bits() != row[11].to_bits() {
            legacy_differences += 1;
        }
        count += 1;
    }
    assert_eq!(count, 1236);
    assert!(legacy_differences > 0);
    eprintln!(
        "{count} native camber loads match bit-for-bit; unit-axis simplification differs in {legacy_differences}"
    );
}
