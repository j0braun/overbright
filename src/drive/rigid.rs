//! The rigid body's orientation step, as SPEED2.EXE does it.
//!
//! The body keeps its orientation as three rows (rigid body +0x30, +0x40,
//! +0x50: the car's forward, left and up axes in world space; glam columns
//! here = native rows, as in `nis.rs`). Each sub-step of the drive mover
//! (0x5B72E0), the MeloMvr (0x5AADA0) and the NIS mover (0x5AB4xx):
//!
//! 1. 0x5AA490 takes every member's derivative first; the rigid body's
//!    (0x59C1F0) gives each row's as `ω × row` (0x59C130, ω = body +0xC0
//!    as the sub-step starts), and the integrator adds `h·d` to every state
//!    float: a plain Euler step, `row += h·(ω × row)` ([`euler_rows`]).
//! 2. The post-step's tidy-up 0x59C600 takes ω = R·I⁻¹·Rᵀ·L from these
//!    rows **before** they are cleaned up, held to 250 rad/s
//!    ([`spin_from_rows`]).
//! 3. After 0x5AA490 returns the mover re-orthonormalises the rows with
//!    0x5BB0E0 (0x5B7455–0x5B745D in the drive mover; 0x5AB091/0x5AB09E in
//!    the MeloMvr; 0x5AB51F/0x5AB52C in the NIS mover): x = normalise(row0),
//!    z = normalise(x × row1), y = z × x ([`orthonormal`]).
//!
//! Every function keeps the original's float operation order (x87 at
//! 24-bit precision gives the f32 result of each operation), so a replay of
//! the original's own sub-step states matches it to the bit
//! (`native_integrator_replay`).

use glam::{Mat3, Vec3};

/// 0x43D1E0 (out = a × b) in its operation order. The same products and
/// differences as 0x59C130's ω × row.
#[inline]
pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

/// The length the game computes everywhere: √((x² + y²) + z²).
#[inline]
pub(crate) fn length(v: Vec3) -> f32 {
    ((v.x * v.x + v.y * v.y) + v.z * v.z).sqrt()
}

/// 0x43CF10: v × (1 / |v|); a zero vector gives (1, 0, 0).
#[inline]
pub(crate) fn normalize(v: Vec3) -> Vec3 {
    let l = length(v);
    if l == 0.0 {
        return Vec3::X;
    }
    let r = 1.0 / l;
    Vec3::new(r * v.x, r * v.y, r * v.z)
}

/// 0x5BB0E0: x = normalise(row0), z = normalise(x × row1), y = z × x.
/// A zero row gives (1, 0, 0) for x or z (0x43CF10).
pub(crate) fn orthonormal(rows: Mat3) -> Mat3 {
    let x = normalize(rows.x_axis);
    let z = normalize(cross(x, rows.y_axis));
    Mat3::from_cols(x, cross(z, x), z)
}

/// 0x59C130 + 0x5AA490: one Euler step of the rows with the spin the
/// sub-step starts with, `row += h·(ω × row)` (each float `s + h·d`).
pub(crate) fn euler_rows(rows: Mat3, spin: Vec3, h: f32) -> Mat3 {
    let step = |row: Vec3| {
        let d = cross(spin, row);
        Vec3::new(h * d.x + row.x, h * d.y + row.y, h * d.z + row.z)
    };
    Mat3::from_cols(step(rows.x_axis), step(rows.y_axis), step(rows.z_axis))
}

/// 0x59C600's ω = R·I⁻¹·Rᵀ·L (0x43C9D0 transpose, two 0x43C990 → 0x4013F0
/// products, 0x441000 on L) for a body-space diagonal inverse inertia
/// `inv` (rigid body +0x170 / +0x1B0), held to 250 rad/s (0x7A1B24).
/// `rows` are the rows as the Euler step left them.
pub(crate) fn spin_from_rows(rows: Mat3, inv: Vec3, l: Vec3) -> Vec3 {
    // R[k][i]: row k's component i (glam column k).
    let r = |k: usize, i: usize| rows.col(k)[i];
    // M1 = I⁻¹·R: one non-zero product per element.
    let m1 = |k: usize, j: usize| inv[k] * r(k, j);
    // M2 = Rᵀ·M1 = Σk R[k][i]·M1[k][j], summed in 0x4013F0's order for
    // element (i, j) (the k = 3 product is a zero: rows have w = 0).
    const ORDER: [[[usize; 3]; 3]; 3] = [
        [[0, 2, 1], [1, 2, 0], [2, 0, 1]],
        [[2, 1, 0], [2, 0, 1], [2, 1, 0]],
        [[2, 1, 0], [2, 0, 1], [2, 1, 0]],
    ];
    let m2 = |i: usize, j: usize| {
        let o = ORDER[i][j];
        (r(o[0], i) * m1(o[0], j) + r(o[1], i) * m1(o[1], j)) + r(o[2], i) * m1(o[2], j)
    };
    // 0x441000: out = L·M2 (row vector), z term first; the translation
    // row of M2 is zero.
    let w = Vec3::new(
        (m2(2, 0) * l.z + m2(1, 0) * l.y) + m2(0, 0) * l.x,
        (m2(2, 1) * l.z + m2(0, 1) * l.x) + m2(1, 1) * l.y,
        (m2(2, 2) * l.z + m2(0, 2) * l.x) + m2(1, 2) * l.y,
    );
    let n = length(w);
    if n > 250.0 {
        let k = 250.0 / n;
        Vec3::new(k * w.x, k * w.y, k * w.z)
    } else {
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0x5BB0E0 keeps an orthonormal set and gives (1, 0, 0) for zero rows.
    #[test]
    fn orthonormal_cleans_a_skewed_set() {
        let m = orthonormal(Mat3::from_cols(Vec3::new(2.0, 0.1, 0.0), Vec3::new(0.2, 1.0, 0.05), Vec3::Z));
        assert!((m.x_axis.length() - 1.0).abs() < 1e-6 && (m.z_axis.length() - 1.0).abs() < 1e-6);
        assert!(m.x_axis.dot(m.y_axis).abs() < 1e-6 && m.x_axis.dot(m.z_axis).abs() < 1e-6);
        assert_eq!(orthonormal(Mat3::ZERO).x_axis, Vec3::X);
        assert_eq!(orthonormal(Mat3::ZERO).z_axis, Vec3::X, "0x43CF10 gives (1,0,0) for z too");
    }

    /// The Euler step is not a rotation: it grows each row by about
    /// (ωh)²/2, which the quaternion step it replaces did not.
    #[test]
    fn euler_step_is_first_order() {
        let spin = Vec3::new(0.0, 0.0, 3.0);
        let h = 1.0 / 240.0;
        let m = euler_rows(Mat3::IDENTITY, spin, h);
        let grow = m.x_axis.length() - 1.0;
        let want = 0.5 * (3.0 * h) * (3.0 * h);
        assert!((grow - want).abs() < 2e-7, "{grow} {want}");
        assert_eq!(m.x_axis, Vec3::new(1.0, 3.0 * h, 0.0));
    }
}

/// The original's own sub-steps, replayed one at a time. The
/// player's Peugeot in the first drag (STAGE_1_DRAG_1, frames 19–168 from
/// the green), stopped before (A) and after (B) each 0x5AA490 call
/// (`tools/parity/subtrap.py`, fixture by `tools/parity/subtrap_fixture.py`;
/// docs/evidence/native-integrator/). Every pair must match to the bit:
/// the rows' Euler step (0x59C130 + 0x5AA490), ω from the un-cleaned rows
/// (0x59C600) and 0x5BB0E0 to the next A. The quaternion rotation the
/// clone used before is shown to miss by far more than float rounding.
#[cfg(test)]
mod native_integrator_replay {
    use super::*;
    use glam::Quat;

    struct Pair {
        h: f32,
        rows_a: Mat3,
        spin_a: Vec3,
        rows_b: Mat3,
        l_b: Vec3,
        spin_b: Vec3,
        inv: Vec3,
        next: Option<Mat3>,
    }

    fn pairs() -> Vec<Pair> {
        let raw = &crate::testkit::private("docs/evidence/native-integrator/drag-s1-substeps.bin");
        raw.chunks_exact(41 * 4)
            .map(|c| {
                let w = |i: usize| u32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap());
                let f = |i: usize| f32::from_bits(w(i));
                let v = |i: usize| Vec3::new(f(i), f(i + 1), f(i + 2));
                let m = |i: usize| Mat3::from_cols(v(i), v(i + 3), v(i + 6));
                Pair {
                    h: f(0),
                    rows_a: m(1),
                    spin_a: v(10),
                    rows_b: m(13),
                    l_b: v(22),
                    spin_b: v(25),
                    inv: v(28),
                    next: (w(31) == 1).then(|| m(32)),
                }
            })
            .collect()
    }

    fn bits(m: Mat3) -> [u32; 9] {
        m.to_cols_array().map(f32::to_bits)
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn original_sub_steps_replay_to_the_bit() {
        let all = pairs();
        assert_eq!(all.len(), 262);
        let (mut next_checked, mut quat_worst) = (0, 0.0f32);
        for (k, p) in all.iter().enumerate() {
            assert_eq!(bits(euler_rows(p.rows_a, p.spin_a, p.h)), bits(p.rows_b), "pair {k}: Euler rows");
            let w = spin_from_rows(p.rows_b, p.inv, p.l_b);
            assert_eq!(w.to_array().map(f32::to_bits), p.spin_b.to_array().map(f32::to_bits), "pair {k}: spin");
            if let Some(next) = p.next {
                assert_eq!(bits(orthonormal(p.rows_b)), bits(next), "pair {k}: 0x5BB0E0");
                next_checked += 1;
                // The old step: rotate the orientation quaternion by |ω|h about ω.
                let q = Quat::from_mat3(&p.rows_a).normalize();
                let q2 = (Quat::from_axis_angle(p.spin_a.normalize(), p.spin_a.length() * p.h) * q).normalize();
                let old = Mat3::from_quat(q2);
                let err = (0..3).map(|c| (old.col(c) - next.col(c)).abs().max_element()).fold(0.0, f32::max);
                quat_worst = quat_worst.max(err);
            }
        }
        assert!(next_checked > 250, "{next_checked}");
        // The rotation lands up to ~1e-7 off per step here (ω ≈ 0.05 rad/s,
        // h = 1/240 s): an error the original never makes.
        assert!(quat_worst > 5e-8, "{quat_worst}");
    }
}
