use super::*;
/// How far a reflection ray looks (m).
pub const REACH: f32 = 300.0;
/// The TLAS's instances: the world and at most this many cars.
pub const MAX_CARS: usize = 96;
/// Car triangles' room in the colour buffer (all cached models).
pub(in crate::render) const CAR_COLOR_ROOM: u32 = 1 << 20;
/// Instance masks.
pub(in crate::render) const MASK_WORLD: u8 = 0x01;
pub(in crate::render) const MASK_CAR: u8 = 0x02;

/// A triangle's colour as the shader reads it: RGB8 of the colour / 2
/// (world.efx's `× 2` in the shader), A 255.
pub fn pack(rgb: [f32; 3]) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    q(rgb[0]) | q(rgb[1]) << 8 | q(rgb[2]) << 16 | 0xFF << 24
}

/// The mean RGBA (0..1) of a texture's top level, sampled on a grid of at
/// most 64 × 64 texels.
pub fn texture_mean(t: &Texture) -> [f32; 4] {
    let rgba = t.decode_rgba();
    let (w, h) = (t.width.max(1) as usize, t.height.max(1) as usize);
    if rgba.len() < w * h * 4 {
        return [1.0; 4];
    }
    let (sx, sy) = ((w / 64).max(1), (h / 64).max(1));
    let mut sum = [0.0f64; 4];
    let mut n = 0.0f64;
    for y in (0..h).step_by(sy) {
        for x in (0..w).step_by(sx) {
            let p = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            for c in 0..4 {
                sum[c] += p[c] as f64;
            }
            n += 1.0;
        }
    }
    sum.map(|s| (s / n.max(1.0) / 255.0) as f32)
}

/// A texture left out of the traced scene: additive ones (glows, light
/// cones), and alpha-tested ones mostly transparent (fences, foliage
/// cards: a triangle is all or nothing here).
pub(in crate::render) fn skipped(t: &Texture, mean: [f32; 4]) -> bool {
    matches!(t.blend, Blend::Additive | Blend::DestAlphaAdditive)
        || ((t.alpha_test || t.blend == Blend::Alpha) && mean[3] < 0.5)
}

/// Triangles in one space: positions (three per triangle, unindexed) and a
/// colour each ([`pack`]).
#[derive(Default)]
pub struct Geometry {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<u32>,
}

impl Geometry {
    pub fn triangles(&self) -> usize {
        self.colors.len()
    }
}

/// The world's traced geometry: every placement's low LOD in world space,
/// but the sky dome, the hidden markers (GPS aid, dyno rollers) and the
/// skipped textures ([`skipped`]). `means`: texture means by hash, filled.
pub fn world_geometry(world: &World, means: &mut HashMap<u32, [f32; 4]>) -> Geometry {
    let mut g = Geometry::default();
    let mut hidden: Vec<usize> = world.gps_aid.into_iter().collect();
    if let Some(first) = world.dyno_rollers {
        hidden.extend(first..first + 4);
    }
    for (i, p) in world.placements.iter().enumerate() {
        let Some(si) = p.low_solid else { continue };
        if hidden.contains(&i) {
            continue;
        }
        let solid = &world.solids[si];
        if solid.name == crate::render::SKYDOME_SOLID {
            continue;
        }
        let m = p.transform;
        for mat in &solid.materials {
            let hash = solid.textures.get(mat.texture_slot as usize).copied();
            let tex = hash.and_then(|h| world.textures.get(&h));
            let mean = match tex {
                Some(t) => *means.entry(t.hash).or_insert_with(|| texture_mean(t)),
                None => [1.0; 4],
            };
            if tex.is_some_and(|t| skipped(t, mean)) {
                continue;
            }
            let end = (mat.first_index + mat.index_count) as usize;
            let Some(ix) = solid.indices.get(mat.first_index as usize..end.min(solid.indices.len())) else { continue };
            for tri in ix.chunks_exact(3) {
                let vs = [tri[0], tri[1], tri[2]].map(|i| solid.vertices.get(i as usize));
                let [Some(a), Some(b), Some(c)] = vs else { continue };
                let mut rgb = [0.0f32; 3];
                for v in [a, b, c] {
                    g.positions.push(m.transform_point3(Vec3::from(v.position)).to_array());
                    // D3DCOLOR bytes B, G, R, A.
                    rgb[0] += v.color[2] as f32 / 765.0;
                    rgb[1] += v.color[1] as f32 / 765.0;
                    rgb[2] += v.color[0] as f32 / 765.0;
                }
                g.colors.push(pack([rgb[0] * mean[0], rgb[1] * mean[1], rgb[2] * mean[2]]));
            }
        }
    }
    g
}

/// The LOD bit a car's traced model takes its parts at (LOD C: the in-game
/// draw's third, from 20 "size" down).
pub(in crate::render) const CAR_LOD: u8 = 1 << 2;

/// A car's traced geometry in car space: its parts (in-game scene, no
/// wheels or parts that follow them) at [`CAR_LOD`] (LOD A when the car
/// has nothing at C). Colour: the texture's mean × the light material's
/// mid diffuse (min + range / 2), halved for [`pack`]'s `× 2`.
pub fn car_geometry(car: &Car, means: &mut HashMap<u32, [f32; 4]>) -> Geometry {
    let take = |lod: u8| {
        car.parts.iter().filter(move |p| {
            p.scene != crate::car::Scene::FrontEnd
                && !p.is_wheel
                && p.follows_wheel.is_none()
                && p.lods.is_none_or(|l| l & lod != 0)
        })
    };
    let lod = if take(CAR_LOD).any(|p| p.lods.is_some()) { CAR_LOD } else { 1 };
    let mut g = Geometry::default();
    for part in take(lod) {
        let Some(solid) = car.solids.get(part.solid) else { continue };
        let m: Mat4 = part.transform;
        for mat in &solid.materials {
            let hash = solid.textures.get(mat.texture_slot as usize).copied().unwrap_or(0);
            let tex = car.part_texture_for(part, hash);
            let mean = match tex {
                Some(t) => *means.entry(t.hash ^ 0x5A5A_0000).or_insert_with(|| texture_mean(t)),
                None => [1.0; 4],
            };
            if tex.is_some_and(|t| skipped(t, mean)) {
                continue;
            }
            let diffuse = solid
                .light_materials
                .get(mat.light_material as usize)
                .and_then(|&h| car.part_light_material_for(part, h))
                .map_or([0.6; 3], |lm| {
                    let c = lm.constants(mat.flags);
                    std::array::from_fn(|i| (c.diffuse_min[i] + 0.5 * c.diffuse_range[i]).clamp(0.05, 1.0))
                });
            let first = mat.first_index as usize;
            let end = (first + mat.index_count as usize).min(solid.indices.len());
            for (k, tri) in solid.indices[first.min(end)..end].chunks_exact(3).enumerate() {
                if part.hidden_tris.contains(&((first / 3 + k) as u32)) {
                    continue;
                }
                let vs = [tri[0], tri[1], tri[2]].map(|i| solid.vertices.get(i as usize));
                let [Some(a), Some(b), Some(c)] = vs else { continue };
                for v in [a, b, c] {
                    g.positions.push(m.transform_point3(Vec3::from(v.position)).to_array());
                }
                g.colors.push(pack(std::array::from_fn(|i| mean[i] * diffuse[i] * 0.5)));
            }
        }
    }
    g
}

/// A car model's cache key: its name, parts and their materials.
pub fn car_key(car: &Car) -> u64 {
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(car.name.as_bytes());
    for p in &car.parts {
        h.update(&(p.solid as u64).to_le_bytes());
        h.update(&[p.lods.unwrap_or(0xFF), p.is_wheel as u8]);
        h.update(
            &p.transform.to_cols_array().map(f32::to_bits).iter().flat_map(|b| b.to_le_bytes()).collect::<Vec<u8>>(),
        );
    }
    let mut paints: Vec<(u32, u32)> = car.light_material_remap.iter().map(|(&a, &b)| (a, b)).collect();
    paints.sort_unstable();
    for (a, b) in paints {
        h.update(&a.to_le_bytes());
        h.update(&b.to_le_bytes());
    }
    let mut tex: Vec<(u32, u32)> = car.texture_remap.iter().map(|(&a, &b)| (a, b)).collect();
    tex.sort_unstable();
    for (a, b) in tex {
        h.update(&a.to_le_bytes());
        h.update(&b.to_le_bytes());
    }
    h.digest()
}

/// A 3 × 4 row-major affine matrix (a TLAS instance's) from `m`.
pub fn instance_transform(m: Mat4) -> [f32; 12] {
    let r = |i: usize| [m.x_axis[i], m.y_axis[i], m.z_axis[i], m.w_axis[i]];
    let (a, b, c) = (r(0), r(1), r(2));
    [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3], c[0], c[1], c[2], c[3]]
}
