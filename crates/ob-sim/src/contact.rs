//! The game's collision response for a car against the track (the world
//! collision step 0x5FFA60; see docs/formats.md, "Contacts").
//!
//! Each frame, after the car has moved, every track polygon near it whose
//! distance to the car's convex hull is within 1 mm (0x5B3D30 over the
//! distance query 0x5B33A0) becomes a contact. An overlapping one is moved
//! back in time by bisection (eight halvings, 0x5B4BF0) to where hull and
//! polygon just touch. The earliest contact is resolved first (0x5B2620):
//! the car goes back to that moment, takes one impulse (0x5AF0F0), and is
//! simulated again for the rest of the frame; then the contacts are found
//! afresh. At most 20 contacts a frame (0x802080) and two per polygon
//! (0x802084).
//!
//! The distance query is the game's own ([`crate::narrow`]).

use glam::Vec3;

/// Hull and polygon overlap at or under this distance, m (0x5B3D30).
pub const OVERLAP: f32 = 1e-5;
/// They touch at or under this distance, m.
pub const TOUCH: f32 = 0.001;
/// Contacts resolved per frame (0x802080) and per polygon (0x802084).
pub const MAX_EVENTS: usize = 20;
pub const MAX_PER_POLYGON: usize = 2;

/// How far to move a hull that overlaps a polygon for the whole frame
/// (0x5B4300): along `normal` (the polygon's, facing the hull) by 0.005 m,
/// then by steps growing 0.01 a try while under 0.1, the vertical part held
/// to ±0.15 m, until the two no longer overlap; at most 20 tries. `at`:
/// the hull body's position (the pair's dirty test, narrow::distance_at);
/// `side`: the hull's support side ([`crate::narrow::Side`]).
pub fn push_out(
    hull: &[Vec3],
    polygon: &[Vec3],
    at: Vec3,
    normal: Vec3,
    side: crate::narrow::Side,
    cache: &mut Option<crate::narrow::PairCache>,
) -> Vec3 {
    let (mut distance_out, mut step) = (0.0f32, 0.005f32);
    let mut offset = Vec3::ZERO;
    for _ in 0..20 {
        distance_out += step;
        if step < 0.1 {
            step += 0.01;
        }
        offset = normal * distance_out;
        offset.z = offset.z.clamp(-0.15, 0.15);
        let moved: Vec<Vec3> = hull.iter().map(|p| *p + offset).collect();
        if crate::narrow::distance_at(&moved, polygon, at + offset, Vec3::ZERO, [side, crate::narrow::Side::NONE], cache).0 > OVERLAP {
            break;
        }
    }
    offset
}

/// Restitution and friction of the car against a track polygon by its
/// normal's z (0x5A0CA0, 0x5A0BF0; the car's values win, 0x5941D0): walls
/// (|n.z| < 0.1) bounce 0.05 and grip 0.3 × 0.3; floors (|n.z| > 0.7) bounce
/// 0.1 and grip 0.3 × (1 + 0.17 per wheel off the ground), at most 1; the
/// rest 0.1 and 0.3.
pub fn materials(normal_z: f32, wheels_down: usize) -> (f32, f32) {
    let nz = normal_z.abs();
    let friction = if nz > 0.7 {
        0.3 * (1.0 + (4 - wheels_down.min(4)) as f32 * 0.17)
    } else if nz < 0.1 {
        0.3 * 0.3
    } else {
        0.3
    };
    let restitution = if nz < 0.1 { 0.5 * 0.1 } else { 0.1 };
    (restitution, friction.min(1.0))
}

/// A rigid body as the impulse sees it.
pub struct Body {
    /// Velocity of the centre of mass and angular velocity, world space.
    pub velocity: Vec3,
    pub spin: Vec3,
    pub mass: f32,
    /// The inverse inertia the impulse applies to a world vector: the
    /// rigid body's body-space one, not turned (0x593A50; 0x5AF3E1).
    pub inverse_inertia: Box<dyn Fn(Vec3) -> Vec3>,
    /// The car's up axis, and whether the arm is levelled: all four wheels
    /// down on a contact whose record has had a crash (record +0x10, set
    /// only by a car-against-car crash, 0x59FB80).
    pub up: Vec3,
    pub all_wheels_down: bool,
    /// Crash mode (car +0x38 = 1): 0x593A50 and 0x5A0B10 read the car's own
    /// flag whatever the partner is, so against the track too the inertia
    /// and the angular part of the push are taken whole, not × 0.4.
    pub crash: bool,
}

/// The impulse at a contact against the static track (0x5AF0F0), and the
/// changes it makes to linear and angular momentum (0x5A0B10 → 0x59BD90).
/// `arm` runs from the centre of mass to the contact; `normal` points from
/// the polygon to the car. `None` when the contact is separating.
pub fn impulse(body: &Body, arm: Vec3, normal: Vec3, restitution: f32, friction: f32) -> Option<(Vec3, Vec3)> {
    let v = body.velocity + body.spin.cross(arm);
    let vn = v.dot(normal);
    if vn >= 0.0 {
        return None;
    }
    let tangent = v - normal * vn;
    // The car collides as if at least 1.85 t (0x593A10), and turns 0.4 as
    // easily as its inertia says outside crash mode, 1.0 in it (0x593A50:
    // `cmp [car+0x38],1` → 0x784250 = 1.0 else 0x7A1890 = 0.4).
    let inverse_mass = (1.0 / body.mass).min(1.0 / 1.85);
    let factor = if body.crash { 1.0 } else { 0.4 };
    let rot = |x: Vec3| (body.inverse_inertia)(x) * factor;
    let k = inverse_mass + normal.dot(rot(arm.cross(normal)).cross(arm));
    let mut j = -(1.0 + restitution) * vn / k;
    // At least enough for 0.3 m/s (0x7FBF24).
    j = j.max(0.3 / inverse_mass);

    // Coulomb friction against the sliding direction, ramped in below 1 m/s.
    let slide = tangent.length();
    let mut f = friction * j;
    if slide < 1.0 {
        f *= slide;
    }
    let friction_impulse = if slide > 1e-5 { -tangent / slide * f } else { Vec3::ZERO };
    let total = normal * j + friction_impulse;

    // With all four wheels down the push acts level with the centre of
    // mass: the arm loses its part along the car's up.
    let arm = if body.all_wheels_down { arm - body.up * arm.dot(body.up) } else { arm };

    // Applied at the car's real mass, scaled as for 1.85 t (0x5A0B10), each
    // part capped (0x59BD90); the angular part × 0.4, or whole in crash
    // mode (0x5A0B10 passes 1.0 / 0.4 by car +0x38).
    let (linear, angular) = apply(body.mass, body.crash, total, arm);
    Some((linear, angular))
}

/// Car against car (0x5A0CA0 / 0x5A0BF0 with another car, combined by the
/// larger, 0x5AF0F0 and 0x5941D0): restitution 0.1, friction 0.3.
pub const CAR_RESTITUTION: f32 = 0.1;
pub const CAR_FRICTION: f32 = 0.3;

/// A car in a car-against-car contact.
pub struct Mover {
    pub velocity: Vec3,
    pub spin: Vec3,
    pub mass: f32,
    /// Body-space inverse inertia, applied to world vectors unturned (the rigid body's tensor,
    /// 0x593A50 before its 0.4 factor).
    pub inverse_inertia: Box<dyn Fn(Vec3) -> Vec3>,
    /// Crash mode (car +0x38 = 1): the inertia and the angular part of
    /// the push are taken whole, not × 0.4.
    pub crash: bool,
}

impl Mover {
    fn inverse_mass(&self) -> f32 {
        (1.0 / self.mass).min(1.0 / 1.85)
    }
    fn rot(&self, x: Vec3) -> Vec3 {
        (self.inverse_inertia)(x) * if self.crash { 1.0 } else { 0.4 }
    }
    pub fn point_velocity(&self, arm: Vec3) -> Vec3 {
        self.velocity + self.spin.cross(arm)
    }
}

/// The two-body impulse (0x5AF0F0) between cars A and B: `normal` points
/// from A to B, the arms run from each centre of mass to the contact. The
/// relative velocity is B's at the point less A's; separating (v·n ≥ 0)
/// gives `None`. j = −(1 + e)·vn / (1/m′A + 1/m′B + n·((I′A⁻¹(rA × n)) ×
/// rA) + n·((I′B⁻¹(rB × n)) × rB)), at least 0.3 / max(1/m′A, 1/m′B);
/// friction μ·j against the sliding direction (× the sliding speed under
/// 1 m/s, none under 1e-5). Returns the impulse on B (A takes its
/// opposite) before it is applied.
pub fn exchange(a: &Mover, b: &Mover, arm_a: Vec3, arm_b: Vec3, normal: Vec3, restitution: f32, friction: f32) -> Option<Vec3> {
    let v = b.point_velocity(arm_b) - a.point_velocity(arm_a);
    let vn = v.dot(normal);
    if vn >= 0.0 {
        return None;
    }
    let (ia, ib) = (a.inverse_mass(), b.inverse_mass());
    let k = ia + ib + normal.dot(a.rot(arm_a.cross(normal)).cross(arm_a)) + normal.dot(b.rot(arm_b.cross(normal)).cross(arm_b));
    let j = (-(1.0 + restitution) * vn / k).max(0.3 / ia.max(ib));
    let tangent = v - normal * vn;
    let slide = tangent.length();
    let mut f = friction * j;
    if slide < 1.0 {
        f *= slide;
    }
    let friction_impulse = if slide > 1e-5 { -tangent / slide * f } else { Vec3::ZERO };
    Some(normal * j + friction_impulse)
}

/// How a car takes an impulse (0x5A0B10 → 0x59BD90): scaled by m/1.85
/// for cars under 1.85 t, the linear part capped at 1000 and the angular
/// one at 10, the latter × 0.4 outside crash mode. Returns the changes to
/// momentum and angular momentum.
pub fn apply(mass: f32, crash: bool, impulse: Vec3, arm: Vec3) -> (Vec3, Vec3) {
    let scale = if 1.0 / mass > 1.0 / 1.85 { mass / 1.85 } else { 1.0 };
    let factor = if crash { 1.0 } else { 0.4 };
    ((impulse * scale).clamp_length_max(1000.0), arm.cross(impulse * scale).clamp_length_max(10.0) * factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_on_wall_hit_bounces_a_little() {
        let body = Body {
            velocity: Vec3::new(-10.0, 0.0, 0.0),
            spin: Vec3::ZERO,
            mass: 1.3,
            inverse_inertia: Box::new(|v| v),
            up: Vec3::Z,
            all_wheels_down: true,
            crash: false,
        };
        let (dp, _) = impulse(&body, Vec3::new(-2.0, 0.0, 0.0), Vec3::X, 0.05, 0.09).unwrap();
        // Effective mass 1.85 t, applied to 1.3 t scaled by 1.3/1.85.
        let dv = dp.x / 1.3;
        assert!((dv - 10.5).abs() < 0.01, "{dv}");
    }

    /// A car in crash mode (car +0x38 = 1) takes a track
    /// polygon's push with its whole inverse inertia and the whole angular
    /// part: 0x593A50 / 0x5A0B10 test the car's own flag, not the partner.
    #[test]
    fn crash_mode_takes_the_track_push_whole() {
        let body = |crash| Body {
            velocity: Vec3::new(-10.0, 0.0, 0.0),
            spin: Vec3::ZERO,
            mass: 1.3,
            inverse_inertia: Box::new(|v| v),
            up: Vec3::Z,
            all_wheels_down: false,
            crash,
        };
        let arm = Vec3::new(-2.0, 0.5, 0.3);
        let (l0, a0) = impulse(&body(false), arm, Vec3::X, 0.05, 0.09).unwrap();
        let (l1, a1) = impulse(&body(true), arm, Vec3::X, 0.05, 0.09).unwrap();
        // The denominator 1/m' + n·((f·I⁻¹(r × n)) × r) grows with f, so j shrinks.
        assert!(l1.length() < l0.length());
        // The same push through 0x5A0B10 gives 1.0/0.4 of the angular change.
        let (_, whole) = apply(1.3, true, l1 / (1.3 / 1.85), arm);
        assert!((a1 - whole).length() < 1e-4, "{a1} {whole}");
        assert!(a0.length() > 0.0 && a1.length() > a0.length() * 0.4 * (l1.length() / l0.length()) * 0.99);
    }
}
