use super::*;
/// The cluster grid's constants for world.wgsl (`Frame.cluster`): tiles x,
/// tiles y, slices, slices / ln(far / near); and `NEAR` in the next.
pub fn cluster_constants() -> [f32; 4] {
    [TILES_X as f32, TILES_Y as f32, SLICES as f32, SLICES as f32 / (FAR / NEAR).ln()]
}

/// The slice of view depth `z` (as world.wgsl computes it).
pub fn slice_of(z: f32) -> u32 {
    let k = SLICES as f32 / (FAR / NEAR).ln();
    ((z.max(NEAR) / NEAR).ln() * k).floor().clamp(0.0, (SLICES - 1) as f32) as u32
}

/// The clustered light list of one frame: per cluster (offset, count) into
/// the index list, then the indices, as one u32 array (world.wgsl
/// `clusters`), and the lights in upload order.
pub struct Clusters {
    pub lights: Vec<GpuLamp>,
    /// The kept lamps' indices into the renderer's lamp list, in upload order.
    pub kept: Vec<usize>,
    pub words: Vec<u32>,
}

/// Bin `lamps` (those whose section is visible) for the view `view_proj`
/// seen from `eye` along `forward`. `shadow(i)` gives a kept lamp's
/// shadow layer.
pub fn build(
    lamps: &[Lamp],
    visible: &dyn Fn(u32) -> bool,
    view_proj: Mat4,
    eye: Vec3,
    forward: Vec3,
    shadow: &dyn Fn(usize) -> i32,
) -> Clusters {
    build_in(lamps, visible, view_proj, eye, forward, shadow, None)
}

/// [`build`], with the clusters' bounding spheres ([`cluster_spheres`]): a
/// lamp is listed only in the clusters of its box that its sphere can
/// reach (a point it cannot reach takes none of its light, so a pixel's
/// sum is the same; the box's corners and the slices beside the sphere
/// held most of the lists' entries).
pub fn build_in(
    lamps: &[Lamp],
    visible: &dyn Fn(u32) -> bool,
    view_proj: Mat4,
    eye: Vec3,
    forward: Vec3,
    shadow: &dyn Fn(usize) -> i32,
    spheres: Option<&[(Vec3, f32)]>,
) -> Clusters {
    // Candidates: visible section, within reach, in front, inside the view.
    let planes = super_planes(view_proj);
    let mut cand: Vec<(f32, usize)> = lamps
        .iter()
        .enumerate()
        .filter(|(_, l)| visible(l.section))
        .filter_map(|(i, l)| {
            let p = l.light.position;
            let r = l.light.outer;
            let dist = (p - eye).length();
            if dist - r > REACH || (p - eye).dot(forward) < -r {
                return None;
            }
            if planes.iter().any(|pl| pl.truncate().dot(p) + pl.w < -r) {
                return None;
            }
            Some((dist - r, i))
        })
        .collect();
    cand.sort_by(|a, b| a.0.total_cmp(&b.0));
    cand.truncate(MAX_LIGHTS);

    let mut lights = Vec::with_capacity(cand.len());
    let mut kept = Vec::with_capacity(cand.len());
    // Each kept lamp's cluster range (slices, rows, columns).
    let mut ranges = Vec::with_capacity(cand.len());
    // Per cluster: its list's length first, then (below) the next free word.
    let mut fill = vec![0u32; CLUSTERS];
    // A lamp's clusters: its box's, those its sphere can reach.
    let each = |(s0, s1, y0, y1, x0, x1): (u32, u32, u32, u32, u32, u32),
                (centre, radius): (Vec3, f32),
                f: &mut dyn FnMut(usize)| {
        for s in s0..=s1 {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let c = ((s * TILES_Y + y) * TILES_X + x) as usize;
                    if let Some(sp) = spheres
                        && (sp[c].0 - centre).length() > sp[c].1 + radius
                    {
                        continue;
                    }
                    f(c);
                }
            }
        }
    };
    for (_, i) in cand {
        let l = &lamps[i].light;
        lights.push(gpu_lamp(l, shadow(i)));
        kept.push(i);
        let (centre, radius) = bin_sphere(l);
        let (x0, x1, y0, y1) = screen_rect(view_proj, centre, radius);
        let z = (centre - eye).dot(forward);
        let (s0, s1) = (slice_of(z - radius), slice_of(z + radius));
        let r = ((s0, s1, y0, y1, x0, x1), (centre, radius));
        each(r.0, r.1, &mut |c| fill[c] += 1);
        ranges.push(r);
    }
    // The lists laid out in cluster order, each cut to `cap` (when all of
    // them would not fit the index list: the longest lists cut evenly,
    // rather than the last clusters emptied); then filled in lamp
    // order, so each holds its first (nearest) lamps, as appending every
    // lamp to its clusters' lists and copying the lists' heads gave
    // (`build_reference` in the tests).
    let cap = list_cap(&fill);
    let mut words = vec![0u32; CLUSTERS * 2];
    let mut total = 0usize;
    for (c, n) in fill.iter_mut().enumerate() {
        let len = (*n as usize).min(cap).min(MAX_INDICES.saturating_sub(total));
        words[c * 2] = (CLUSTERS * 2 + total) as u32;
        words[c * 2 + 1] = len as u32;
        *n = 0;
        total += len;
    }
    words.resize(CLUSTERS * 2 + total, 0);
    for (index, (r, sphere)) in ranges.into_iter().enumerate() {
        each(r, sphere, &mut |c| {
            let (at, len) = (words[c * 2] as usize, words[c * 2 + 1]);
            if fill[c] < len {
                words[at + fill[c] as usize] = index as u32;
                fill[c] += 1;
            }
        });
    }
    Clusters { lights, kept, words }
}

/// The sphere a light is binned with: its reach around it, but a
/// Modern Lighting car headlight (`modern_light_bold::car_lights`, kind 3,
/// colour.w −2) lights only its cone (world.wgsl `ml_now`: 0 past the cone,
/// `ml_near_pool` 0 past the reach), so it takes the smallest sphere around
/// that spherical sector: centre on the axis at reach / (2 cos θ), the same
/// radius (θ ≤ 60°; the apex, the rim and the cap's tip on or inside it).
/// Only the clusters a light cannot reach lose it; the candidates, their
/// order and the upload are as before. A 28 m headlight with cos θ 0.82:
/// 17.1 m around a point 17.1 m ahead (a fifth of the sphere's volume).
pub fn bin_sphere(l: &WorldLight) -> (Vec3, f32) {
    let c = cone_threshold(l);
    if l.sub == crate::render::modern_light_bold::SUB_HEAD && c >= 0.5 {
        let d = l.direction.normalize_or(Vec3::NEG_Z);
        // A small margin over the exact bound (f32 rounding at the rim).
        let r = l.outer / (2.0 * c);
        return (l.position + d * r, r * 1.001 + 0.01);
    }
    (l.position, l.outer)
}

/// The most lights a cluster's list keeps: unlimited while every list
/// fits the index list, else the largest cap whose lists fit.
pub fn list_cap(counts: &[u32]) -> usize {
    let need: usize = counts.iter().map(|&n| n as usize).sum();
    if need <= MAX_INDICES {
        return usize::MAX;
    }
    let (mut lo, mut hi) = (0usize, counts.iter().copied().max().unwrap_or(0) as usize);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if counts.iter().map(|&n| (n as usize).min(mid)).sum::<usize>() <= MAX_INDICES {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// The frustum's six planes (inward normals; xyz·p + w ≥ 0 inside).
pub(in crate::render) fn super_planes(m: Mat4) -> [Vec4; 6] {
    let r = |i| m.row(i);
    let (r0, r1, r2, r3) = (r(0), r(1), r(2), r(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2].map(|p| p / p.truncate().length().max(1e-12))
}

/// The tiles a sphere may cover: its box's corners projected; a corner
/// behind the eye makes it the whole screen.
pub(in crate::render) fn screen_rect(view_proj: Mat4, centre: Vec3, radius: f32) -> (u32, u32, u32, u32) {
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for i in 0..8 {
        let corner = centre
            + Vec3::new(
                if i & 1 == 0 { -radius } else { radius },
                if i & 2 == 0 { -radius } else { radius },
                if i & 4 == 0 { -radius } else { radius },
            );
        let c = view_proj * corner.extend(1.0);
        if c.w <= 1e-3 {
            return (0, TILES_X - 1, 0, TILES_Y - 1);
        }
        let ndc = glam::Vec2::new(c.x, c.y) / c.w;
        lo = lo.min(ndc);
        hi = hi.max(ndc);
    }
    let tile = |v: f32, n: u32| ((v * 0.5 + 0.5) * n as f32).floor().clamp(0.0, (n - 1) as f32) as u32;
    (tile(lo.x, TILES_X), tile(hi.x, TILES_X), tile(lo.y, TILES_Y), tile(hi.y, TILES_Y))
}
