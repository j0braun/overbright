use glam::Vec3;

use super::*;
use crate::render::Camera;

/// The chase views' world push (0x453570 → 0x444530, for kinds 0, 1 and
/// 5, after the shake), so the eye does not sit behind a wall. From
/// `centre` (the car body's centre, 0x444530) to the eye:
///
/// * the sweep (0x444190 → 0x5D3A00(1, 1, …, 0.7, 1)): the nearest
///   crossing of the segment with a gathered polygon (0x5D3800: the
///   polygon's box against the segment's grown by 0.7, then 0x5D3700: the
///   ends on opposite sides of its plane, the crossing inside its edges by
///   0x5D35C0; either side counts) pulls the eye in to 0.7 m (0x7870A0)
///   short of it along the segment;
/// * then up to three passes (0x4441F5..0x444357): every polygon whose
///   surface has no flag at +0x2D of its 0x98-byte row (0x800EED; set for
///   19 and 21–28) and whose edges hold the eye (0x5D35C0) at a distance
///   d ∈ [0, r) on its normal's side pushes by (r − d) along the normal, r
///   = 0.7 (0x78709C) or for a wall the mover's vtable +0x30 (0x445A60:
///   0.7, 3 looking back); the pushes are averaged; a pass with none ends.
///
/// The view is then turned so the car's centre stays where it was on
/// screen (0x444390; here as yaw / pitch offsets). Returns whether the
/// sweep hit. 0x444530's last step (the eye raised to a sphere of the car
/// body's +0xF0 to +0x110 distance + 1 around the centre) is
/// [`keep_out_lifted`]; the lift over traffic that follows it (0x44B040,
/// not a sideways push) is [`lift_over_traffic`].
pub fn keep_out(camera: &mut Camera, centre: Vec3, polygons: &[crate::ground::TrackPolygon]) -> bool {
    keep_out_with(camera, centre, polygons, WALL_RADIUS[0])
}

/// Whether the segment crosses a polygon (0x5D3930 → 0x5D3800: the
/// polygon's box against the segment's grown by `grow`, then the ends on
/// opposite sides of its plane and the crossing inside its edges; either
/// side counts) — the sight test's piece (0x5E7EE0, `grow` 0.5).
pub fn crosses(a: Vec3, b: Vec3, polygons: &[crate::ground::TrackPolygon], grow: f32) -> bool {
    let (lo, hi) = (a.min(b) - grow, a.max(b) + grow);
    polygons.iter().any(|p| {
        let n = p.count.clamp(3, 4);
        let (mut plo, mut phi) = (p.corners[0], p.corners[0]);
        for c in &p.corners[1..n] {
            plo = plo.min(*c);
            phi = phi.max(*c);
        }
        if plo.x > hi.x || plo.y > hi.y || plo.z > hi.z || phi.x < lo.x || phi.y < lo.y || phi.z < lo.z {
            return false;
        }
        let v0 = p.corners[0];
        let (da, db) = ((a - v0).dot(p.normal), (b - v0).dot(p.normal));
        if da * db >= 0.0 {
            return false;
        }
        let hit = a + (b - a) * (da.abs() / (da.abs() + db.abs()));
        (0..n).all(|i| (p.corners[i] - hit).cross(p.corners[(i + 1) % n] - hit).dot(p.normal) >= 0.0)
    })
}

/// [`keep_out`] with the mover's wall radius (0x4442A7: polygons with flag
/// +0x0A bit 2 take vtable +0x30, the rest 0.7).
pub fn keep_out_with(camera: &mut Camera, centre: Vec3, polygons: &[crate::ground::TrackPolygon], wall: f32) -> bool {
    keep_out_lifted(camera, centre, polygons, wall, None)
}

/// The car body's box (0x5AE530, read live under Wine: body +0xE0 / +0xF0
/// / +0x110 of the player's 206 = (−2.2268, −0.8421, −0.1058) / (1.6394,
/// 0.8440, 0.9465) / (−0.2937, 0.0010, 0.4203), the hull less the centre of
/// mass): its hull's vertices relative to the centre of mass (record
/// +0x110, the shape's +0x40 from 0x5AEC30); +0x110 the middle of their
/// bounds (0x5A1A00), +0xE0 / +0xF0 their minimum / maximum with 0
/// included (the loop's minima and maxima start at 0). Returns +0x110 and
/// the radius of 0x444530's last step, |+0xF0 − +0x110| + 1 (0x78708C,
/// 0x444886). 0x444530's centre is car +0x50 (the car's origin, the centre
/// of mass less the turned record offset; live: +0x60 − rows · (0.3, 0,
/// 0.3) on the 206) plus +0x110 turned by the car's rows (0x4445A5): the
/// offset from the centre of mass added to the origin (verified in the
/// disassembly; the frames are mixed as the game mixes them).
pub fn body_box(hull: &[Vec3], centre_of_mass: Vec3) -> Option<(Vec3, f32)> {
    let lo = hull.iter().copied().reduce(Vec3::min)? - centre_of_mass;
    let hi = hull.iter().copied().reduce(Vec3::max)? - centre_of_mass;
    let mid = (lo + hi) * 0.5;
    Some((mid, (hi.max(Vec3::ZERO) - mid).length() + 1.0))
}

/// [`keep_out_with`] for a car (0x444530 with its car): after the pushes,
/// an eye nearer the centre than `lift` is raised onto that sphere,
/// keeping its x and y (z = centre z + √(lift² − horizontal²)), before
/// the view is turned to the centre.
pub fn keep_out_lifted(
    camera: &mut Camera,
    centre: Vec3,
    polygons: &[crate::ground::TrackPolygon],
    wall: f32,
    lift: Option<f32>,
) -> bool {
    const R: f32 = 0.7;
    // Surfaces whose row at 0x800EED is set (19, 21..=28).
    let skip = |s: u8| s == 19 || (21..=28).contains(&s);
    let inside = |p: &crate::ground::TrackPolygon, at: Vec3| -> bool {
        let n = p.count.clamp(3, 4);
        (0..n).all(|i| {
            let a = p.corners[i] - at;
            let b = p.corners[(i + 1) % n] - at;
            a.cross(b).dot(p.normal) >= 0.0
        })
    };
    let before = camera.position;
    let mut eye = before;
    // The sweep.
    // The segment's box: the centre, and the eye grown by 0.7 (0x43D540 /
    // 0x43D610).
    let (lo, hi) = (centre.min(eye - R), centre.max(eye + R));
    let mut best = 999_999.0f32;
    for p in polygons {
        let n = p.count.clamp(3, 4);
        let (mut plo, mut phi) = (p.corners[0], p.corners[0]);
        for c in &p.corners[1..n] {
            plo = plo.min(*c);
            phi = phi.max(*c);
        }
        if plo.x > hi.x || plo.y > hi.y || plo.z > hi.z || phi.x < lo.x || phi.y < lo.y || phi.z < lo.z {
            continue;
        }
        let v0 = p.corners[0];
        let (a, b) = ((centre - v0).dot(p.normal), (eye - v0).dot(p.normal));
        if a * b >= 0.0 {
            continue;
        }
        let hit = centre + (eye - centre) * (a.abs() / (a.abs() + b.abs()));
        if inside(p, hit) {
            best = best.min((hit - centre).length());
        }
    }
    let span = eye - centre;
    let hit = span.length() > best;
    if hit {
        eye = centre + span.normalize_or(Vec3::X) * (best - R);
    }
    // The pushes.
    for _ in 0..3 {
        let (mut push, mut count) = (Vec3::ZERO, 0);
        for p in polygons.iter().filter(|p| !skip(p.surface)) {
            if !inside(p, eye) {
                continue;
            }
            let d = (eye - p.corners[0]).dot(p.normal);
            let r = if p.flags & 0x02 != 0 { wall } else { R };
            if (0.0..r).contains(&d) {
                push += p.normal * (r - d);
                count += 1;
            }
        }
        if count == 0 {
            break;
        }
        eye += push / count as f32;
    }
    if let Some(r) = lift {
        let d = eye - centre;
        if d.length() < r {
            eye.z = centre.z + (r * r - d.truncate().length_squared()).sqrt();
        }
    }
    if eye == before {
        return hit;
    }
    // Keep the car's centre where it was in the view.
    let to = |from: Vec3| {
        let v = centre - from;
        (v.x.atan2(v.y), v.z.atan2(v.truncate().length()))
    };
    let (yaw0, pitch0) = to(before);
    let (yaw1, pitch1) = to(eye);
    camera.yaw += yaw1 - yaw0;
    camera.pitch += pitch1 - pitch0;
    camera.position = eye;
    hit
}

/// A car the chase eye may sit in (0x44AC10 reads it from the world's car
/// list 0x890080): its origin (car +0x50), velocity (+0x70), rows (+0x90)
/// and its body's box (body +0xE0 / +0xF0, relative to the centre of mass,
/// 0 included) and middle (+0x110), as [`body_box`] reads them.
#[derive(Clone, Copy, Debug)]
pub struct LiftCar {
    pub origin: Vec3,
    pub velocity: Vec3,
    pub rotation: glam::Mat3,
    pub min: Vec3,
    pub max: Vec3,
    pub middle: Vec3,
}

impl LiftCar {
    /// From the car's hull and record centre of mass (see [`body_box`]).
    pub fn new(
        origin: Vec3,
        velocity: Vec3,
        rotation: glam::Mat3,
        hull: &[Vec3],
        centre_of_mass: Vec3,
    ) -> Option<LiftCar> {
        let lo = hull.iter().copied().reduce(Vec3::min)? - centre_of_mass;
        let hi = hull.iter().copied().reduce(Vec3::max)? - centre_of_mass;
        Some(LiftCar {
            origin,
            velocity,
            rotation,
            min: lo.min(Vec3::ZERO),
            max: hi.max(Vec3::ZERO),
            middle: (lo + hi) * 0.5,
        })
    }
}

/// The car the eye is inside (0x44AC10, **verified** in the disassembly
/// 0x44AC10..0x44AFE4; constants read from the image): the first car of
/// `cars` (the caller passes the world list's traffic cars — with the flag
/// 1 the chase passes, owner kind [car +0x14] +4 = 3 — that are drawn, +0x550,
/// and in this view's mask, +0x5B4; not the camera's own car) with
///
/// * its origin under 12 m (0x7870C8) from the eye;
/// * unless `scene` (0x571260: flow state [[0x8900D8]] 6): with the
///   relative velocity own − car (+0x70) over 15 m/s (0x7870C4), the car
///   is skipped when both the eye-to-car direction and the own-to-car
///   direction dot under 0.8 (0x7870C0) with that velocity's direction
///   (0x43CF10: a zero vector normalises to +x);
/// * the eye within R = |box max − middle| + 1 (0x5EC260 / 0x5EC250) of
///   its origin, and the eye's z inside the z-range of the box's eight
///   corners (taken through the car's rows and origin) grown by 0.5.
///
/// Returns its origin and R².
pub fn car_holding_eye(
    eye: Vec3,
    own_origin: Vec3,
    own_velocity: Vec3,
    cars: &[LiftCar],
    scene: bool,
) -> Option<(Vec3, f32)> {
    let unit = |v: Vec3| v.try_normalize().unwrap_or(Vec3::X);
    for c in cars {
        let to = c.origin - eye;
        let d2 = to.length_squared();
        if !(d2 < 12.0 * 12.0) {
            continue;
        }
        if !scene {
            let rel = own_velocity - c.velocity;
            if 15.0 * 15.0 < rel.length_squared() {
                let v = unit(rel);
                if unit(to).dot(v) < 0.8 && v.dot(unit(c.origin - own_origin)) < 0.8 {
                    continue;
                }
            }
        }
        let r = (c.max - c.middle).length() + 1.0;
        let r2 = r * r;
        if !(d2 < r2) {
            continue;
        }
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for k in 0..8 {
            let corner = Vec3::new(
                if k & 2 != 0 { c.max.x } else { c.min.x },
                if k & 4 != 0 { c.max.y } else { c.min.y },
                if k & 1 != 0 { c.min.z } else { c.max.z },
            );
            let z = (c.origin + c.rotation * corner).z;
            lo = lo.min(z);
            hi = hi.max(z);
        }
        if eye.z.max(lo - 0.5).min(hi + 0.5) == eye.z {
            return Some((c.origin, r2));
        }
    }
    None
}

/// The chase eye lifted over the traffic it sits in (0x44B040, called by
/// the chase update 0x453570 at 0x453BDD with flag 1, right after the world
/// push 0x444530, for the chase kinds 0, 1 and 5; **verified**): while
/// [`car_holding_eye`] finds a car, eye z = √(R² + 0.1 − dx² − dy²) + the
/// car's origin z — onto the top of its sphere, x and y kept. Then the view
/// is turned so the camera's car (its origin, +0x50) keeps its screen spot
/// (0x444390; as the world push does it, by yaw / pitch offsets). Returns
/// whether the eye moved (0x7EB5C0 holds the car's id then, −1 without).
pub fn lift_over_traffic(
    camera: &mut Camera,
    own_origin: Vec3,
    own_velocity: Vec3,
    cars: &[LiftCar],
    scene: bool,
) -> bool {
    let before = camera.position;
    let mut eye = before;
    // Each lift leaves the eye 0.1 m² outside that car's sphere; the cap
    // only guards against float trouble.
    for _ in 0..64 {
        let Some((origin, r2)) = car_holding_eye(eye, own_origin, own_velocity, cars, scene) else { break };
        let dx = eye.x - origin.x;
        let dy = eye.y - origin.y;
        eye.z = (r2 + 0.1 - (dx * dx + dy * dy)).sqrt() + origin.z;
    }
    if eye == before {
        return false;
    }
    let to = |from: Vec3| {
        let v = own_origin - from;
        (v.x.atan2(v.y), v.z.atan2(v.truncate().length()))
    };
    let (yaw0, pitch0) = to(before);
    let (yaw1, pitch1) = to(eye);
    camera.yaw += yaw1 - yaw0;
    camera.pitch += pitch1 - pitch0;
    camera.position = eye;
    true
}
