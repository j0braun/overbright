//! The cars a pixel tests, per light cluster (docs/lamps-shadows.md,
//! "Per-pixel work"). world.wgsl's `car_visibility` multiplies, per lamp, a
//! factor for every car whose soft ellipsoid lies between the pixel and the
//! lamp, and `contact_visibility` one for every car within 4 m; a car that
//! cannot do either for any point of a cluster gives exactly 1 there. Beside
//! every entry of a cluster's lamp list the clusters buffer holds the cars
//! that can shade the cluster from that lamp, and per cluster the cars that
//! can shade it from the player's headlight and those whose contact shadow
//! can reach it (a bit each): the pixel tests only those, in the same
//! order, so the products are the same.
//!
//! The tests are conservative: the points a lamp lights in a cluster lie in
//! a sphere (the cluster's, or the lamp's own reach when that is smaller);
//! every segment from the lamp to such a point lies in the capsule of that
//! radius around the segment from the lamp to the sphere's centre; a car
//! whose shading ellipsoid (`car_visibility`'s `closest` below 1.3, inside
//! a sphere of 1.3 × 1.15 × its largest half extent) misses that capsule,
//! or whose 12 m reach misses the cluster, is left out.

use super::*;

/// Word offset, in the clusters buffer, from a lamp entry to its cars
/// (world.wgsl `LAMP_CAR_MASKS`): the masks follow the index list.
pub const LAMP_CAR_MASKS: usize = MAX_INDICES;
/// Where each cluster's contact-shadow cars sit (a word per cluster,
/// world.wgsl `CONTACT_CARS`): after the entries' cars.
pub const CONTACT_CARS: usize = CLUSTERS * 2 + 2 * MAX_INDICES;
/// Where each cluster's cars that can shade it from the player's headlight
/// sit (a word per cluster, world.wgsl `HEADLIGHT_CARS`).
pub const HEADLIGHT_CARS: usize = CONTACT_CARS + CLUSTERS;
/// The clusters buffer's length in words: the header, the index list, the
/// cars of its entries, its clusters' contact-shadow and headlight cars.
pub const CLUSTER_WORDS: usize = HEADLIGHT_CARS + CLUSTERS;
/// `contact_visibility`'s reach: a car farther than this from the pixel
/// leaves it as it is.
const CONTACT_REACH: f32 = 4.0;

/// `car_visibility`'s shading ellipsoid in the unit sphere's space (the
/// box's half extents × 1.15) reaches `closest` 1.3; past it a car's factor
/// is exactly 1 (`smoothstep(0.7, 1.3, closest)`).
const SHADE_REACH: f32 = 1.3 * 1.15;
/// `car_visibility`'s first test: a car farther than this from the pixel
/// shades it from no lamp.
const CAR_REACH: f32 = 12.0;
/// Against the GPU's rounding (positions, the cluster a pixel falls in):
/// metres added to every distance compared, and a share of the cluster's
/// radius.
const MARGIN: f32 = 0.05;
const MARGIN_SHARE: f32 = 0.02;
/// The last slice's far end for its bounding sphere (it holds every depth
/// past the clusters' far plane; a lamp's reach bounds it below).
const LAST_SLICE_FAR: f64 = 1.0e4;

/// Each cluster's bounding sphere (world centre, radius) for the view
/// `view_proj` seen from `eye` along `forward`, as world.wgsl assigns a
/// pixel to a cluster: its NDC's tile and its view depth's slice (depths
/// below the near plane in the first, past the far one in the last).
pub fn cluster_spheres(view_proj: Mat4, eye: Vec3, forward: Vec3) -> Vec<(Vec3, f32)> {
    use glam::{DMat4, DVec3, DVec4};
    // Relative to the eye, in f64: the rays through the tiles' corners,
    // each scaled to view depth 1.
    let vp = view_proj.as_dmat4() * DMat4::from_translation(eye.as_dvec3());
    let inv = vp.inverse();
    let f = forward.as_dvec3();
    let (tx, ty) = (TILES_X as usize, TILES_Y as usize);
    let mut rays = Vec::with_capacity((tx + 1) * (ty + 1));
    for j in 0..=ty {
        for i in 0..=tx {
            let x = -1.0 + 2.0 * i as f64 / tx as f64;
            let y = -1.0 + 2.0 * j as f64 / ty as f64;
            let p = inv * DVec4::new(x, y, 0.5, 1.0);
            let d = p.truncate() / p.w;
            rays.push(d / d.dot(f).max(1e-9));
        }
    }
    let k = SLICES as f64 / (FAR as f64 / NEAR as f64).ln();
    let depth = |s: u32| NEAR as f64 * (s as f64 / k).exp();
    let mut out = Vec::with_capacity(CLUSTERS);
    for s in 0..SLICES {
        let z0 = if s == 0 { 0.0 } else { depth(s) };
        let z1 = if s == SLICES - 1 { LAST_SLICE_FAR } else { depth(s + 1) };
        for y in 0..ty {
            for x in 0..tx {
                let corners = [
                    rays[y * (tx + 1) + x],
                    rays[y * (tx + 1) + x + 1],
                    rays[(y + 1) * (tx + 1) + x],
                    rays[(y + 1) * (tx + 1) + x + 1],
                ];
                let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
                for r in corners {
                    for z in [z0, z1] {
                        lo = lo.min(r * z);
                        hi = hi.max(r * z);
                    }
                }
                let centre = (lo + hi) * 0.5;
                let mut radius = 0.0f64;
                for r in corners {
                    for z in [z0, z1] {
                        radius = radius.max((r * z - centre).length());
                    }
                }
                let radius = radius as f32;
                out.push((eye + centre.as_vec3(), radius * (1.0 + MARGIN_SHARE) + MARGIN));
            }
        }
    }
    out
}

/// The squared distance from `c` to the segment from `a` to `b`.
fn segment_distance_squared(c: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((c - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    (a + ab * t - c).length_squared()
}

/// The cars a car's shading ellipsoid holds: its centre and the radius of
/// the sphere around it.
fn car_reach(o: &GpuOccluder) -> (Vec3, f32) {
    let h = o.x[3].max(o.y[3]).max(o.z[3]);
    (Vec3::new(o.centre[0], o.centre[1], o.centre[2]), SHADE_REACH * h)
}

/// The view the clusters cut (as [`build`] bins): `view_proj` seen from
/// `eye` along `forward`.
#[derive(Clone, Copy)]
pub struct ClusterView {
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub forward: Vec3,
}

impl ClusterView {
    /// Every cluster a sphere may touch: its box's tiles and its depths'
    /// slices (as [`build`] bins a lamp), the sphere a little wider
    /// against rounding.
    fn each_cluster(&self, centre: Vec3, radius: f32, f: &mut dyn FnMut(usize)) {
        let radius = radius * (1.0 + MARGIN_SHARE) + MARGIN;
        let (x0, x1, y0, y1) = screen_rect(self.view_proj, centre, radius);
        let z = (centre - self.eye).dot(self.forward);
        for s in slice_of(z - radius)..=slice_of(z + radius) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    f(((s * TILES_Y + y) * TILES_X + x) as usize);
                }
            }
        }
    }
}

/// The cars whose contact shadow can reach each cluster (its sphere within
/// `contact_visibility`'s 4 m of the car's centre), a word per cluster.
pub fn contact_masks(occluders: &[GpuOccluder], spheres: &[(Vec3, f32)], view: &ClusterView) -> Vec<u32> {
    let mut out = vec![0u32; CLUSTERS];
    if spheres.len() < CLUSTERS {
        return out;
    }
    for (k, o) in occluders.iter().take(32).enumerate() {
        let c = Vec3::new(o.centre[0], o.centre[1], o.centre[2]);
        view.each_cluster(c, CONTACT_REACH, &mut |cl| {
            let (centre, radius) = spheres[cl];
            let reach = radius + CONTACT_REACH + MARGIN;
            if (c - centre).length_squared() <= reach * reach {
                out[cl] |= 1 << k;
            }
        });
    }
    out
}

/// The cars that can come between each cluster and the player's headlight
/// at `eye` (`headlight_visibility` and the player's beam), a word per
/// cluster; never car 0 (the headlight's own, which they skip).
pub fn headlight_masks(eye: Vec3, occluders: &[GpuOccluder], spheres: &[(Vec3, f32)], view: &ClusterView) -> Vec<u32> {
    let mut out = vec![0u32; CLUSTERS];
    if spheres.len() < CLUSTERS {
        return out;
    }
    for (k, o) in occluders.iter().enumerate().take(32).skip(1) {
        let (c, r) = car_reach(o);
        view.each_cluster(c, CAR_REACH, &mut |cl| {
            let (centre, radius) = spheres[cl];
            let (reach, shade) = (radius + CAR_REACH + MARGIN, radius + r + MARGIN);
            if (c - centre).length_squared() <= reach * reach
                && segment_distance_squared(c, eye, centre) <= shade * shade
            {
                out[cl] |= 1 << k;
            }
        });
    }
    out
}

/// The cars of each lamp entry of `words` (the clusters' header and index
/// list, [`build`]) for the `lights` they index: one word per entry, in the
/// index list's order (`words[CLUSTERS * 2 ..]`).
pub fn car_masks(
    words: &[u32],
    lights: &[GpuLamp],
    occluders: &[GpuOccluder],
    spheres: &[(Vec3, f32)],
    view: &ClusterView,
) -> Vec<u32> {
    let mut out = vec![0u32; words.len().saturating_sub(CLUSTERS * 2)];
    if occluders.is_empty() || out.is_empty() || spheres.len() < CLUSTERS {
        return out;
    }
    let cars: Vec<(Vec3, f32)> = occluders.iter().take(32).map(car_reach).collect();
    // Per cluster: the cars whose 12 m reach touches it.
    let mut in_reach = vec![0u32; CLUSTERS];
    for (k, &(c, _)) in cars.iter().enumerate() {
        view.each_cluster(c, CAR_REACH, &mut |cl| {
            let (centre, radius) = spheres[cl];
            let reach = radius + CAR_REACH + MARGIN;
            if (c - centre).length_squared() <= reach * reach {
                in_reach[cl] |= 1 << k;
            }
        });
    }
    // Per light: the cars that can come between it and a point it lights
    // (within its reach), so the clusters test only those.
    let near: Vec<u32> = lights
        .iter()
        .map(|l| {
            let (p, reach) = (Vec3::new(l.position[0], l.position[1], l.position[2]), l.position[3]);
            let mut m = 0u32;
            for (k, &(c, r)) in cars.iter().enumerate() {
                if (c - p).length() < reach + r + MARGIN {
                    m |= 1 << k;
                }
            }
            m
        })
        .collect();
    for (c, &(centre, radius)) in spheres.iter().enumerate().take(CLUSTERS) {
        if in_reach[c] == 0 {
            continue;
        }
        let (first, count) = (words[c * 2] as usize, words[c * 2 + 1] as usize);
        for at in first..first + count {
            let li = words[at] as usize;
            let Some(&cand) = near.get(li) else { continue };
            let cand = cand & in_reach[c];
            if cand == 0 {
                continue;
            }
            let l = &lights[li];
            let p = Vec3::new(l.position[0], l.position[1], l.position[2]);
            let reach = l.position[3];
            // The lit points of the cluster: in its sphere and in the lamp's.
            let (rc, rr) = if reach + MARGIN < radius { (p, reach + MARGIN) } else { (centre, radius) };
            let mut m = 0u32;
            let mut bits = cand;
            while bits != 0 {
                let k = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let (cc, r) = cars[k];
                let (reach, shade) = (rr + CAR_REACH + MARGIN, rr + r + MARGIN);
                if (cc - rc).length_squared() <= reach * reach && segment_distance_squared(cc, p, rc) <= shade * shade {
                    m |= 1 << k;
                }
            }
            out[at - CLUSTERS * 2] = m;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// world.wgsl `car_visibility`'s factor for one car (f32, as the GPU).
    fn factor(p: Vec3, l: Vec3, o: &GpuOccluder) -> f32 {
        let c = Vec3::new(o.centre[0], o.centre[1], o.centre[2]);
        let rel0 = p - c;
        if rel0.dot(rel0) > 144.0 {
            return 1.0;
        }
        let rel1 = l - c;
        let ax = |v: [f32; 4]| Vec3::new(v[0], v[1], v[2]);
        let s = Vec3::new(o.x[3], o.y[3], o.z[3]) * 1.15;
        let a = Vec3::new(rel0.dot(ax(o.x)), rel0.dot(ax(o.y)), rel0.dot(ax(o.z))) / s;
        let b = Vec3::new(rel1.dot(ax(o.x)), rel1.dot(ax(o.y)), rel1.dot(ax(o.z))) / s;
        if a.dot(a) < 1.0 {
            return 1.0;
        }
        let d = b - a;
        let t = (-a.dot(d) / d.dot(d).max(1e-6)).clamp(0.0, 1.0);
        let closest = (a + d * t).length();
        let x = ((closest - 0.7) / 0.6).clamp(0.0, 1.0);
        let k = x * x * (3.0 - 2.0 * x);
        0.4 * (1.0 - k) + k
    }

    /// The cluster world.wgsl's `lamp_light` puts p in (f32, as the GPU).
    fn cluster_of(p: Vec3, view_proj: Mat4, eye: Vec3, forward: Vec3) -> Option<usize> {
        let clip = view_proj * p.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = glam::Vec2::new(clip.x, clip.y) / clip.w;
        if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 {
            return None;
        }
        let [tx, ty, slices, k] = cluster_constants();
        let x = ((ndc.x * 0.5 + 0.5) * tx).floor().clamp(0.0, tx - 1.0) as usize;
        let y = ((ndc.y * 0.5 + 0.5) * ty).floor().clamp(0.0, ty - 1.0) as usize;
        let z = (p - eye).dot(forward);
        let s = ((z.max(NEAR) / NEAR).ln() * k).floor().clamp(0.0, slices - 1.0) as usize;
        Some((s * TILES_Y as usize + y) * TILES_X as usize + x)
    }

    fn rng(seed: &mut u32) -> f32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        *seed as f32 / u32::MAX as f32
    }

    /// A car left out of a cluster's lamp entry gives exactly 1 at every
    /// pixel of that cluster the lamp lights: a street of lamps with cars
    /// on it, seen from a chase camera (also tilted and rolled), pixels on
    /// the road, on walls and in the air; and the product over the kept cars
    /// is the product over all of them, bit for bit.
    #[test]
    fn left_out_cars_shade_nothing() {
        let mut seed = 0x1234_5678u32;
        let mut checked = (0usize, 0usize, 0usize);
        for scene in 0..12 {
            let r = &mut seed;
            let yaw = rng(r) * 6.28;
            let pitch = -0.05 - 0.25 * rng(r);
            let base = Vec3::new(rng(r) * 4000.0 - 2000.0, rng(r) * 4000.0 - 2000.0, rng(r) * 20.0);
            let mut cam = crate::render::Camera::new(base, yaw, pitch);
            if scene % 3 == 1 {
                cam.up = Vec3::new(0.2, 0.1, 1.0).normalize();
            }
            let eye = cam.position;
            let forward = cam.forward();
            let view_proj = cam.view_projection(16.0 / 9.0);
            let ahead = Vec3::new(forward.x, forward.y, 0.0).normalize_or(Vec3::X);
            let side = ahead.cross(Vec3::Z);
            let ground = base.z - 1.5;
            // Lamps along the street (both sides, 6–9 m up, 30 m reach), and
            // a few big ones.
            let lamps: Vec<Lamp> = (0..40)
                .map(|i| {
                    let along = i as f32 * 12.0 - 60.0 + rng(r) * 4.0;
                    let across = if i % 2 == 0 { -7.0 } else { 7.0 } + rng(r) * 2.0;
                    let outer = if i % 9 == 0 { 60.0 + rng(r) * 100.0 } else { 20.0 + rng(r) * 15.0 };
                    let pos = eye + ahead * along + side * across;
                    Lamp {
                        section: 1,
                        light: WorldLight {
                            kind: 2,
                            sub: 0,
                            on: true,
                            colour: [150, 150, 150],
                            position: Vec3::new(pos.x, pos.y, ground + 6.0 + rng(r) * 3.0),
                            outer,
                            direction: Vec3::NEG_Z,
                            intensity: 1.0,
                            inner: 8.0,
                            cone: 0.0,
                        },
                    }
                })
                .collect();
            // Cars on the road ahead and around the eye (the player's just
            // ahead), turned every way.
            let cars: Vec<GpuOccluder> = (0..15)
                .map(|i| {
                    let along = if i == 0 { 6.0 } else { rng(r) * 90.0 - 15.0 };
                    let p = eye + ahead * along + side * (rng(r) * 10.0 - 5.0);
                    let transform = Mat4::from_translation(Vec3::new(p.x, p.y, ground))
                        * Mat4::from_rotation_z(rng(r) * 6.28)
                        * Mat4::from_rotation_x((rng(r) - 0.5) * 0.3);
                    Occluder { transform, min: Vec3::new(-2.2, -0.9, 0.0), max: Vec3::new(2.2, 0.9, 1.3), glow: 0.0 }
                        .gpu()
                })
                .collect();
            let c = build(&lamps, &|_| true, view_proj, eye, forward, &|_| -1);
            let spheres = cluster_spheres(view_proj, eye, forward);
            let masks = car_masks(&c.words, &c.lights, &cars, &spheres, &ClusterView { view_proj, eye, forward });
            assert_eq!(masks.len(), c.words.len() - CLUSTERS * 2);
            let kept: usize = masks.iter().map(|m| m.count_ones() as usize).sum();
            let entries_with_cars = masks.iter().filter(|&&m| m != 0).count();
            checked.2 += kept;
            assert!(entries_with_cars > 0, "scene {scene}: some lamp keeps a car");
            for n in 0..30_000 {
                // On the road, on a wall beside it, or anywhere in the view.
                let p = match n % 3 {
                    0 => {
                        let q = eye + ahead * (rng(r) * 120.0 - 10.0) + side * (rng(r) * 30.0 - 15.0);
                        Vec3::new(q.x, q.y, ground)
                    }
                    1 => {
                        let q = eye + ahead * (rng(r) * 120.0 - 10.0) + side * if rng(r) < 0.5 { -10.0 } else { 10.0 };
                        Vec3::new(q.x, q.y, ground + rng(r) * 12.0)
                    }
                    _ => {
                        eye + (forward + side * (rng(r) * 2.0 - 1.0) + Vec3::Z * (rng(r) - 0.6))
                            * (1.0 + rng(r) * 150.0)
                    }
                };
                let Some(cl) = cluster_of(p, view_proj, eye, forward) else { continue };
                let (first, count) = (c.words[cl * 2] as usize, c.words[cl * 2 + 1] as usize);
                for at in first..first + count {
                    let l = &c.lights[c.words[at] as usize];
                    let lp = Vec3::new(l.position[0], l.position[1], l.position[2]);
                    // Only where the lamp lights p (`lamp_weight` > 0).
                    if (lp - p).length_squared() >= l.position[3] * l.position[3] {
                        continue;
                    }
                    let mask = masks[at - CLUSTERS * 2];
                    let (mut all, mut some) = (1.0f32, 1.0f32);
                    for (k, o) in cars.iter().enumerate() {
                        let f = factor(p, lp, o);
                        all *= f;
                        if mask & (1 << k) != 0 {
                            some *= f;
                        } else {
                            assert_eq!(f, 1.0, "scene {scene}: car {k} left out of cluster {cl} shades {p} from {lp}");
                        }
                        if f < 1.0 {
                            checked.1 += 1;
                        }
                    }
                    assert_eq!(all.to_bits(), some.to_bits());
                    checked.0 += 1;
                }
            }
        }
        // The scenes hold shadows, and most cars were left out.
        assert!(checked.1 > 1000, "{checked:?}");
        assert!(checked.0 > 100_000, "{checked:?}");
    }

    /// world.wgsl reads the masks where they are written, and its loop
    /// takes the cars in their order.
    #[test]
    fn the_shader_reads_the_masks_where_they_are() {
        use crate::render::core::shader::registry::{ShaderId, text};
        let wgsl = text(ShaderId::World);
        assert!(wgsl.contains(&format!("const LAMP_CAR_MASKS: u32 = {LAMP_CAR_MASKS}u;")));
        assert!(wgsl.contains("car_visibility(p, l.position.xyz, clusters[first + i + LAMP_CAR_MASKS])"));
        // Modern Lighting's near cars take the same entry's cars (their
        // factor, `mix(1 - ML_CAR_DEPTH, 1, smoothstep(0.7, 1.3, closest))`,
        // is 1 past the same `closest`).
        assert!(wgsl.contains("vis = ml_cars(p, l.position.xyz, owner, clusters[first + i + LAMP_CAR_MASKS]);"));
        assert!(wgsl.contains("if k == skip || (cars & (1u << k)) == 0u { continue; }"));
        assert!(wgsl.contains("vis *= mix(1.0 - ML_CAR_DEPTH, 1.0, smoothstep(0.7, 1.3, length(a + d * t)));"));
        assert!(wgsl.contains("let i = firstTrailingBit(m);\n        m &= m - 1u;"));
        assert!(wgsl.contains(&format!("const CONTACT_CARS: u32 = {CONTACT_CARS}u;")));
        assert!(wgsl.contains("let cars = clusters[CONTACT_CARS + cluster_at(p, screen)];"));
        // With subgroups the cars' bits are walked for the subgroup together
        // (scalar reads), each lane taking only its own, in order.
        assert!(wgsl.contains("var m = subgroupOr(cars);"));
        assert!(wgsl.contains("if (cars & (1u << i)) == 0u { continue; }"));
        assert!(wgsl.contains(&format!("const HEADLIGHT_CARS: u32 = {HEADLIGHT_CARS}u;")));
        assert_eq!(CLUSTER_WORDS, CLUSTERS * 2 + MAX_INDICES + LAMP_CAR_MASKS + 2 * CLUSTERS);
    }

    /// world.wgsl `contact_visibility`'s factor for one car (f32).
    fn contact_factor(p: Vec3, o: &GpuOccluder) -> f32 {
        let c = Vec3::new(o.centre[0], o.centre[1], o.centre[2]);
        let rel = p - c;
        if rel.dot(rel) > 16.0 {
            return 1.0;
        }
        let ax = |v: [f32; 4]| Vec3::new(v[0], v[1], v[2]);
        let q = Vec3::new(rel.dot(ax(o.x)), rel.dot(ax(o.y)), rel.dot(ax(o.z)));
        let h = Vec3::new(o.x[3], o.y[3], o.z[3]);
        let e = ((glam::Vec2::new(q.x.abs(), q.y.abs()) - glam::Vec2::new(h.x, h.y) * 0.75).max(glam::Vec2::ZERO))
            / (glam::Vec2::new(h.x, h.y) * 0.25);
        let sm = |a: f32, b: f32, x: f32| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let footprint = 1.0 - sm(0.0, 1.0, e.length());
        let above = q.z + h.z;
        let vertical = (1.0 - sm(0.0, h.z, above)) * (1.0 - sm(0.0, 0.6, -above - 0.3));
        1.0 - CONTACT_DEPTH * footprint * vertical * o.centre[3]
    }

    /// A car left out of a cluster's contact cars leaves every pixel of
    /// the cluster as it is.
    #[test]
    fn left_out_contact_cars_shade_nothing() {
        let mut seed = 4242u32;
        let mut shaded = 0usize;
        for _ in 0..10 {
            let r = &mut seed;
            let cam = crate::render::Camera::new(
                Vec3::new(rng(r) * 3000.0, rng(r) * 3000.0, 1.5),
                rng(r) * 6.28,
                -0.2 * rng(r),
            );
            let (eye, forward) = (cam.position, cam.forward());
            let vp = cam.view_projection(16.0 / 9.0);
            let ahead = Vec3::new(forward.x, forward.y, 0.0).normalize_or(Vec3::X);
            let cars: Vec<GpuOccluder> = (0..20)
                .map(|i| {
                    let p = eye
                        + ahead * (if i == 0 { 5.0 } else { rng(r) * 60.0 - 5.0 })
                        + ahead.cross(Vec3::Z) * (rng(r) * 12.0 - 6.0);
                    let t = Mat4::from_translation(Vec3::new(p.x, p.y, 0.0)) * Mat4::from_rotation_z(rng(r) * 6.28);
                    Occluder { transform: t, min: Vec3::new(-2.2, -0.9, 0.0), max: Vec3::new(2.2, 0.9, 1.3), glow: 0.0 }
                        .gpu()
                })
                .collect();
            let spheres = cluster_spheres(vp, eye, forward);
            let masks = contact_masks(&cars, &spheres, &ClusterView { view_proj: vp, eye, forward });
            for _ in 0..40_000 {
                let q = eye + ahead * (rng(r) * 70.0 - 5.0) + ahead.cross(Vec3::Z) * (rng(r) * 16.0 - 8.0);
                let p = Vec3::new(q.x, q.y, rng(r) * 2.0 - 0.3);
                let Some(cl) = cluster_of(p, vp, eye, forward) else { continue };
                for (k, o) in cars.iter().enumerate() {
                    let f = contact_factor(p, o);
                    if masks[cl] & (1 << k) == 0 {
                        assert_eq!(f, 1.0, "car {k} left out of cluster {cl} darkens {p}");
                    } else if f < 1.0 {
                        shaded += 1;
                    }
                }
            }
        }
        assert!(shaded > 1000, "{shaded}");
    }

    /// The lists cut to the clusters a lamp's sphere reaches hold, for every
    /// pixel, the same lamps that light it, in the same order (the pixel's
    /// sum is the same), with fewer entries.
    #[test]
    fn tight_lists_light_every_pixel_the_same() {
        let mut seed = 777u32;
        let (mut loose_n, mut tight_n, mut lit) = (0usize, 0usize, 0usize);
        for scene in 0..8 {
            let r = &mut seed;
            let mut cam = crate::render::Camera::new(
                Vec3::new(rng(r) * 3000.0, rng(r) * 3000.0, 2.0 + rng(r) * 30.0),
                rng(r) * 6.28,
                -0.4 * rng(r),
            );
            if scene % 2 == 1 {
                cam.up = Vec3::new(0.15, -0.1, 1.0).normalize();
            }
            let (eye, forward) = (cam.position, cam.forward());
            let vp = cam.view_projection(16.0 / 9.0);
            let lamps: Vec<Lamp> = (0..400)
                .map(|i| {
                    let p = eye + Vec3::new(rng(r) * 500.0 - 250.0, rng(r) * 500.0 - 250.0, rng(r) * 25.0 - 12.0);
                    let mut light = WorldLight {
                        kind: 2,
                        sub: 0,
                        on: true,
                        colour: [150, 150, 150],
                        position: p,
                        outer: if i % 7 == 0 { 40.0 + rng(r) * 150.0 } else { 5.0 + rng(r) * 30.0 },
                        direction: Vec3::NEG_Z,
                        intensity: 1.0,
                        inner: 3.0,
                        cone: 0.0,
                    };
                    if i % 11 == 0 {
                        // A car headlight (its cone's sphere).
                        light.kind = 3;
                        light.sub = crate::render::modern_light_bold::SUB_HEAD;
                        light.direction = Vec3::new(rng(r) - 0.5, rng(r) - 0.5, -0.1).normalize();
                        light.cone = 1.2;
                    }
                    Lamp { section: 1, light }
                })
                .collect();
            let loose = build(&lamps, &|_| true, vp, eye, forward, &|_| -1);
            let spheres = cluster_spheres(vp, eye, forward);
            let tight = build_in(&lamps, &|_| true, vp, eye, forward, &|_| -1, Some(&spheres));
            assert_eq!(loose.lights, tight.lights);
            loose_n += loose.words.len();
            tight_n += tight.words.len();
            let inv = vp.inverse();
            for _ in 0..20_000 {
                let (x, y) = (rng(r) * 2.0 - 1.0, rng(r) * 2.0 - 1.0);
                let far = inv * glam::Vec4::new(x, y, 0.5, 1.0);
                let dir = (far.truncate() / far.w - eye).normalize();
                let p = eye + dir * (0.5 + rng(r).powi(2) * 400.0);
                let Some(cl) = cluster_of(p, vp, eye, forward) else { continue };
                // The lamps whose sphere holds p (`lamp_weight` > 0 needs
                // it; a headlight's light is inside its binning sphere).
                let reach = |c: &Clusters| -> Vec<u32> {
                    let (first, count) = (c.words[cl * 2] as usize, c.words[cl * 2 + 1] as usize);
                    c.words[first..first + count]
                        .iter()
                        .copied()
                        .filter(|&i| {
                            let (centre, radius) = bin_sphere(&lamps[c.kept[i as usize]].light);
                            (p - centre).length() < radius
                        })
                        .collect()
                };
                let (a, b) = (reach(&loose), reach(&tight));
                assert_eq!(a, b, "scene {scene} cluster {cl} at {p}");
                lit += a.len();
            }
        }
        assert!(lit > 10_000, "{lit}");
        assert!(tight_n * 10 < loose_n * 9, "{tight_n} of {loose_n} words");
    }

    /// A car left out of a cluster's headlight cars does not shade any of
    /// its pixels from the player's headlight; the player's own car (0) is
    /// never in them.
    #[test]
    fn left_out_headlight_cars_shade_nothing() {
        let mut seed = 99_991u32;
        let mut shaded = 0usize;
        for _ in 0..10 {
            let r = &mut seed;
            let cam = crate::render::Camera::new(
                Vec3::new(rng(r) * 3000.0, rng(r) * 3000.0, 2.0),
                rng(r) * 6.28,
                -0.1 * rng(r),
            );
            let (eye, forward) = (cam.position, cam.forward());
            let vp = cam.view_projection(16.0 / 9.0);
            let ahead = Vec3::new(forward.x, forward.y, 0.0).normalize_or(Vec3::X);
            let side = ahead.cross(Vec3::Z);
            let cars: Vec<GpuOccluder> = (0..16)
                .map(|i| {
                    let p = eye + ahead * (if i == 0 { 6.0 } else { rng(r) * 50.0 }) + side * (rng(r) * 8.0 - 4.0);
                    let t =
                        Mat4::from_translation(Vec3::new(p.x, p.y, eye.z - 1.6)) * Mat4::from_rotation_z(rng(r) * 6.28);
                    Occluder { transform: t, min: Vec3::new(-2.2, -0.9, 0.0), max: Vec3::new(2.2, 0.9, 1.3), glow: 0.0 }
                        .gpu()
                })
                .collect();
            // The player's headlight: at the front of car 0, 0.75 m up.
            let light =
                Vec3::new(cars[0].centre[0], cars[0].centre[1], cars[0].centre[2]) + ahead * 2.0 + Vec3::Z * 0.75;
            let spheres = cluster_spheres(vp, eye, forward);
            let masks = headlight_masks(light, &cars, &spheres, &ClusterView { view_proj: vp, eye, forward });
            for _ in 0..40_000 {
                let q = eye + ahead * (rng(r) * 70.0) + side * (rng(r) * 20.0 - 10.0);
                let p = Vec3::new(q.x, q.y, eye.z - 1.6 + rng(r) * 2.0);
                let Some(cl) = cluster_of(p, vp, eye, forward) else { continue };
                assert_eq!(masks[cl] & 1, 0);
                for (k, o) in cars.iter().enumerate().skip(1) {
                    let f = factor(p, light, o);
                    if masks[cl] & (1 << k) == 0 {
                        assert_eq!(f, 1.0, "car {k} left out of cluster {cl} shades {p} from the headlight");
                    } else if f < 1.0 {
                        shaded += 1;
                    }
                }
            }
        }
        assert!(shaded > 1000, "{shaded}");
    }

    /// `lamp_light`'s early outs (a lamp outside its cone or behind the
    /// face skipped, the loop left once every channel reached LAMP_MAX)
    /// give `lamp_shade` the same term, bit for bit: the terms are never
    /// negative and only min(sum, LAMP_MAX) is used.
    #[test]
    fn early_outs_keep_the_lamp_term() {
        let mut seed = 2024u32;
        let r = &mut seed;
        for _ in 0..20_000 {
            let n = Vec3::new(rng(r) - 0.5, rng(r) - 0.5, rng(r) - 0.2).normalize();
            let terms: Vec<(Vec3, f32, f32, f32)> = (0..rng(r).mul_add(12.0, 0.0) as usize)
                .map(|_| {
                    let colour = Vec3::new(rng(r), rng(r), rng(r)) * 0.3;
                    let w = if rng(r) < 0.2 { 0.0 } else { rng(r) };
                    let ndl = (Vec3::new(rng(r) - 0.5, rng(r) - 0.5, rng(r) - 0.5).normalize().dot(n)).max(0.0);
                    let vis = 0.3 + 0.7 * rng(r);
                    (colour, w, ndl, vis)
                })
                .collect();
            let full = terms.iter().fold(Vec3::ZERO, |s, &(c, w, ndl, vis)| s + c * (w * w * ndl * vis));
            let mut cut = Vec3::ZERO;
            for &(c, w, ndl, vis) in &terms {
                if w <= 0.0 || ndl <= 0.0 {
                    continue;
                }
                cut += c * (w * w * ndl * vis);
                if cut.cmpge(Vec3::splat(LAMP_MAX)).all() {
                    break;
                }
            }
            let (a, b) = (full.min(Vec3::splat(LAMP_MAX)), cut.min(Vec3::splat(LAMP_MAX)));
            assert_eq!(a.to_array().map(f32::to_bits), b.to_array().map(f32::to_bits), "{terms:?}");
        }
        use crate::render::core::shader::registry::{ShaderId, text};
        let wgsl = text(ShaderId::World);
        assert!(wgsl.contains("} else if all(sum >= vec3<f32>(LAMP_MAX)) {"));
        assert!(wgsl.contains("if g_surface.on < 0.5 && headroom > 0.0 {"));
    }

    /// world.wgsl `headlight_clip` (HeadlightClipTex sampled linearly, f32).
    fn headlight_clip(u: f32) -> f32 {
        let texel = |i: f32| {
            if !(33.0..=47.0).contains(&i) {
                0.0
            } else {
                (255.0 * (1.0 - (i - 33.0) * (1.0 / 15.0)).sqrt()).trunc() / 255.0
            }
        };
        let x = u.clamp(0.0, 1.0) * 64.0 - 0.5;
        let i = x.floor();
        let (a, b) = (texel(i.clamp(0.0, 63.0)), texel((i + 1.0).clamp(0.0, 63.0)));
        a * (1.0 - (x - i)) + b * (x - i)
    }

    /// `beam_light`'s first test (the clip texture's texel position between
    /// 32 and 48) holds exactly where `headlight_clip` is above 0, for every
    /// f32 between its ends and the whole range around them.
    #[test]
    fn the_beams_clip_range_is_where_the_clip_is_lit() {
        let check = |u: f32| {
            let x = u.clamp(0.0, 1.0) * 64.0 - 0.5;
            assert_eq!(x > 32.0 && x < 48.0, headlight_clip(u) > 0.0, "u {u} x {x}");
        };
        // Every f32 around both ends (texel positions 32 and 48).
        for end in [32.5f32 / 64.0, 48.5 / 64.0] {
            let mut u = end - 0.01;
            while u < end + 0.01 {
                check(u);
                u = f32::from_bits(u.to_bits() + 1);
            }
        }
        for k in -2000..=2000 {
            check(k as f32 / 1000.0);
        }
        use crate::render::core::shader::registry::{ShaderId, text};
        let wgsl = text(ShaderId::World);
        assert!(wgsl.contains("if !(x > 32.0 && x < 48.0) { continue; }"));
        assert!(wgsl.contains("return trunc(255.0 * sqrt(1.0 - (i - 33.0) * (1.0 / 15.0))) / 255.0;"));
    }

    /// The clusters' spheres hold the points world.wgsl assigns to them.
    #[test]
    fn cluster_spheres_hold_their_pixels() {
        let mut seed = 99u32;
        for _ in 0..6 {
            let r = &mut seed;
            let mut cam = crate::render::Camera::new(
                Vec3::new(rng(r) * 3000.0, rng(r) * 3000.0, 5.0),
                rng(r) * 6.28,
                -0.3 * rng(r),
            );
            cam.up = Vec3::new(rng(r) * 0.3, 0.0, 1.0).normalize();
            let (eye, forward) = (cam.position, cam.forward());
            let vp = cam.view_projection(21.0 / 9.0);
            let spheres = cluster_spheres(vp, eye, forward);
            let inv = vp.inverse();
            for _ in 0..20_000 {
                let (x, y) = (rng(r) * 2.0 - 1.0, rng(r) * 2.0 - 1.0);
                let far = inv * glam::Vec4::new(x, y, 0.5, 1.0);
                let dir = (far.truncate() / far.w - eye).normalize();
                let p = eye + dir * (0.3 + rng(r).powi(3) * 900.0);
                let Some(cl) = cluster_of(p, vp, eye, forward) else { continue };
                let (c, rad) = spheres[cl];
                assert!((p - c).length() <= rad, "{p} outside cluster {cl} ({c}, {rad})");
            }
        }
    }
}
