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
/// The neon and lit signs ([`emissive_geometry`]): only the reflections'
/// glow ray sees them (every other ray's mask leaves this bit out).
pub(in crate::render) const MASK_EMISSIVE: u8 = 0x04;

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
/// colour each ([`pack`]); for the world also each triangle's surface
/// ([`Surface`]) and the textures its layers hold.
#[derive(Default)]
pub struct Geometry {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<u32>,
    /// Per triangle (world only): its surface for the textured hit.
    pub surfaces: Vec<Surface>,
    /// The hit texture array's layers: the texture hash each holds.
    pub layers: Vec<u32>,
}

/// The side of a layer of the hit texture array (texels; a mip chain down
/// to 1).
pub const HIT_TEXTURE: u32 = 128;
/// The hit texture array's mip levels.
pub const HIT_MIPS: u32 = HIT_TEXTURE.trailing_zeros() + 1;
/// A triangle without a layer (flat colour).
pub const NO_LAYER: u32 = u32::MAX;

/// A world triangle's surface as rt.wgsl's textured hit reads it (three
/// `vec4<u32>`): the three UVs (f32), the three baked vertex colours (RGBA8,
/// D3DCOLOR's B, G, R turned to R, G, B), the layer and the texture LOD's
/// triangle term (log2 of texels per metre at the layer's size, f32).
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Surface {
    pub words: [u32; 12],
}

impl Surface {
    pub fn new(uv: [[f32; 2]; 3], color: [[u8; 4]; 3], layer: u32, lod: f32) -> Surface {
        // Shift the UVs by whole tiles (the sampler repeats) so they stay small.
        let (du, dv) = (uv[0][0].floor(), uv[0][1].floor());
        let rgba = |c: [u8; 4]| u32::from_le_bytes([c[2], c[1], c[0], 255]);
        let f = |v: f32| v.to_bits();
        Surface {
            words: [
                f(uv[0][0] - du),
                f(uv[0][1] - dv),
                f(uv[1][0] - du),
                f(uv[1][1] - dv),
                f(uv[2][0] - du),
                f(uv[2][1] - dv),
                rgba(color[0]),
                rgba(color[1]),
                rgba(color[2]),
                layer,
                f(lod),
                0,
            ],
        }
    }
}

/// The texture LOD's triangle term (ray cones, Akenine-Möller et al.,
/// "Texture Level of Detail Strategies for Real-Time Ray Tracing", Ray
/// Tracing Gems ch. 20, eq. 3): half the log2 of the triangle's texel area
/// (at the layer's size) over its world area; the shader adds log2 of the
/// cone's width at the hit.
pub fn lod_term(p: [Vec3; 3], uv: [[f32; 2]; 3]) -> f32 {
    let world = (p[1] - p[0]).cross(p[2] - p[0]).length();
    let (a, b) = ([uv[1][0] - uv[0][0], uv[1][1] - uv[0][1]], [uv[2][0] - uv[0][0], uv[2][1] - uv[0][1]]);
    let texels = (a[0] * b[1] - a[1] * b[0]).abs() * (HIT_TEXTURE * HIT_TEXTURE) as f32;
    if world <= 1e-8 || texels <= 1e-8 {
        return 0.0;
    }
    0.5 * (texels / world).log2()
}

/// A texture as one layer of the hit texture array: its top level box-filtered
/// to [`HIT_TEXTURE`] square, then the mip chain (RGBA8, levels back to back).
pub fn hit_layer(t: &Texture) -> Vec<u8> {
    let rgba = t.decode_rgba();
    let (w, h) = (t.width.max(1) as usize, t.height.max(1) as usize);
    let n = HIT_TEXTURE as usize;
    let mut level = vec![0u8; n * n * 4];
    if rgba.len() >= w * h * 4 {
        for y in 0..n {
            let (y0, y1) = (y * h / n, ((y + 1) * h / n).max(y * h / n + 1));
            for x in 0..n {
                let (x0, x1) = (x * w / n, ((x + 1) * w / n).max(x * w / n + 1));
                let mut s = [0u32; 4];
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        for c in 0..4 {
                            s[c] += rgba[(yy * w + xx) * 4 + c] as u32;
                        }
                    }
                }
                let k = ((y1 - y0) * (x1 - x0)) as u32;
                for c in 0..4 {
                    level[(y * n + x) * 4 + c] = ((s[c] + k / 2) / k) as u8;
                }
            }
        }
    } else {
        level.fill(255);
    }
    let mut out = level.clone();
    let mut side = n;
    while side > 1 {
        let half = side / 2;
        let mut next = vec![0u8; half * half * 4];
        for y in 0..half {
            for x in 0..half {
                for c in 0..4 {
                    let at = |xx: usize, yy: usize| level[(yy * side + xx) * 4 + c] as u32;
                    let s = at(2 * x, 2 * y) + at(2 * x + 1, 2 * y) + at(2 * x, 2 * y + 1) + at(2 * x + 1, 2 * y + 1);
                    next[(y * half + x) * 4 + c] = ((s + 2) / 4) as u8;
                }
            }
        }
        out.extend_from_slice(&next);
        level = next;
        side = half;
    }
    out
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
    let mut layers: HashMap<u32, u32> = HashMap::new();
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
            // The texture's layer in the hit array (the first triangle using it adds it).
            let layer = match tex {
                Some(t) => match layers.get(&t.hash) {
                    Some(&l) => l,
                    None => {
                        let l = g.layers.len() as u32;
                        g.layers.push(t.hash);
                        layers.insert(t.hash, l);
                        l
                    }
                },
                None => NO_LAYER,
            };
            let end = (mat.first_index + mat.index_count) as usize;
            let Some(ix) = solid.indices.get(mat.first_index as usize..end.min(solid.indices.len())) else { continue };
            for tri in ix.chunks_exact(3) {
                let vs = [tri[0], tri[1], tri[2]].map(|i| solid.vertices.get(i as usize));
                let [Some(a), Some(b), Some(c)] = vs else { continue };
                let pw = [a, b, c].map(|v| m.transform_point3(Vec3::from(v.position)));
                let uv = [a.uv, b.uv, c.uv];
                g.surfaces.push(Surface::new(uv, [a.color, b.color, c.color], layer, lod_term(pw, uv)));
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

/// The world's light-giving geometry for the reflections (an instance of
/// its own, [`MASK_EMISSIVE`], not opaque): the neon and the lit signs
/// (additive textures, `modern_light_bold::emissive_name`) of every
/// placement's main LOD (the low LOD often leaves them out), in world space,
/// each triangle's surface indexed after the opaque world's; their textures
/// join `g`'s layers. A ray sums what it passes through (additive blending
/// is order-free), rt.wgsl `rt_glow`.
pub fn emissive_geometry(world: &World, g: &mut Geometry) -> Geometry {
    let mut e = Geometry::default();
    let mut layers: HashMap<u32, u32> = g.layers.iter().enumerate().map(|(i, &h)| (h, i as u32)).collect();
    for p in &world.placements {
        let Some(solid) = world.solids.get(p.solid) else { continue };
        if solid.name == crate::render::SKYDOME_SOLID || p.flags & crate::world::HIDDEN_FLAG != 0 {
            continue;
        }
        let m = p.transform;
        for mat in &solid.materials {
            let Some(t) = solid.textures.get(mat.texture_slot as usize).and_then(|h| world.textures.get(h)) else {
                continue;
            };
            if !matches!(t.blend, Blend::Additive | Blend::DestAlphaAdditive)
                || !crate::render::modern_light_bold::emissive_name(&t.name)
            {
                continue;
            }
            let layer = *layers.entry(t.hash).or_insert_with(|| {
                g.layers.push(t.hash);
                g.layers.len() as u32 - 1
            });
            let end = (mat.first_index + mat.index_count) as usize;
            let Some(ix) = solid.indices.get(mat.first_index as usize..end.min(solid.indices.len())) else { continue };
            for tri in ix.chunks_exact(3) {
                let [Some(a), Some(b), Some(c)] = [tri[0], tri[1], tri[2]].map(|i| solid.vertices.get(i as usize))
                else {
                    continue;
                };
                let pw = [a, b, c].map(|v| m.transform_point3(Vec3::from(v.position)));
                // Huge quads are sky / horizon cards, not signs (as the emitters).
                let area = (pw[1] - pw[0]).cross(pw[2] - pw[0]).length() * 0.5;
                if !(area > 1e-4 && area < 400.0) {
                    continue;
                }
                let uv = [a.uv, b.uv, c.uv];
                e.surfaces.push(Surface::new(uv, [a.color, b.color, c.color], layer, lod_term(pw, uv)));
                e.positions.extend(pw.map(|v| v.to_array()));
                e.colors.push(0);
            }
        }
    }
    for glow in world_glows(world) {
        let s = glow.surface();
        e.positions.extend(glow.octahedron());
        for _ in 0..GLOW_TRIANGLES {
            e.surfaces.push(s);
            e.colors.push(0);
        }
    }
    e
}

/// A surface record's layer word for a glow sprite (rt.wgsl `rt_glow`).
pub const GLOW_MARK: u32 = 0xFFFF_FFFE;
/// The triangles of one glow's bounding octahedron.
pub const GLOW_TRIANGLES: usize = 8;
/// The cars' glows the reflections test at most (each ray tests them all:
/// the nearest cars' four lights).
pub const MAX_CAR_GLOWS: usize = 16;

/// A light glow sprite (the native flare's first layer, docs/formats.md "Light glow
/// sprites") as the reflections see it: a soft disc around `centre` whose
/// world radius grows with the reflected path as the sprite's does with
/// depth (`size` × clamp(distance / view scale, `scale`)), coloured by
/// the layer's colour × alpha (additive), and with the facing term
/// (`exponent` > 0, flag 4) toward `direction`.
#[derive(Clone, Copy, Debug)]
pub struct Glow {
    pub centre: Vec3,
    pub direction: Vec3,
    pub size: f32,
    pub scale: [f32; 2],
    pub rgb: [f32; 3],
    pub exponent: f32,
}

impl Glow {
    /// A flare of `kind` at `intensity` (1 for the street lamps, the
    /// brake lights' 0.85), its first layer; None for a type without one.
    pub fn from_flare(position: Vec3, direction: Vec3, kind: u8, flags: u8, intensity: f32) -> Option<Glow> {
        let l = crate::glow::TYPES.get(kind as usize)?[0]?;
        let alpha = (l.alpha * 2.0 * intensity).min(255.0) / 255.0;
        let size = l.size[0] + (l.size[1] - l.size[0]) * intensity;
        Some(Glow {
            centre: position,
            direction: direction.normalize_or(Vec3::NEG_Z),
            size,
            scale: l.scale,
            rgb: l.rgb.map(|c| c / 255.0 * intensity.min(1.0) * alpha),
            exponent: if flags & 4 != 0 { l.exponent } else { 0.0 },
        })
    }

    /// The largest world radius the glow reaches.
    pub fn reach(&self) -> f32 {
        self.size * self.scale[1]
    }

    /// Its surface record (the same for the octahedron's eight triangles).
    pub fn surface(&self) -> Surface {
        let f = |v: f32| v.to_bits();
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let d = self.direction;
        let s = |v: f32| ((v.clamp(-1.0, 1.0) * 127.0).round() as i32 as u32) & 0xFF;
        Surface {
            words: [
                f(self.centre.x),
                f(self.centre.y),
                f(self.centre.z),
                f(self.size),
                f(self.scale[0]),
                f(self.scale[1]),
                c(self.rgb[0]) | c(self.rgb[1]) << 8 | c(self.rgb[2]) << 16,
                s(d.x) | s(d.y) << 8 | s(d.z) << 16,
                f(self.exponent),
                GLOW_MARK,
                0,
                0,
            ],
        }
    }

    /// The octahedron around the glow's reach (its inscribed sphere holds it).
    pub fn octahedron(&self) -> [[f32; 3]; 3 * GLOW_TRIANGLES] {
        let a = self.reach().max(0.05) * 1.75;
        let c = self.centre;
        let v = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z].map(|e| (c + e * a).to_array());
        let mut out = [[0.0; 3]; 3 * GLOW_TRIANGLES];
        let mut i = 0;
        for x in [0, 1] {
            for y in [2, 3] {
                for z in [4, 5] {
                    out[i] = v[x];
                    out[i + 1] = v[y];
                    out[i + 2] = v[z];
                    i += 3;
                }
            }
        }
        out
    }
}

/// The world's street-lamp glows (every flare pack's flares, as the game
/// draws them at intensity 1).
pub fn world_glows(world: &World) -> Vec<Glow> {
    world
        .flare_packs
        .iter()
        .flat_map(|p| p.flares.iter())
        .filter_map(|f| Glow::from_flare(f.position, f.direction, f.kind, f.flags, 1.0))
        .collect()
}

/// A car's head and brake light glows at `m` (car → world): the
/// headlights' type 0, the brake lights' type 1 at the native 0.85 (not
/// braking: the traced scene does not know the brake), as 0x615640 lights them.
pub fn car_glows(car: &Car, m: Mat4, out: &mut Vec<Glow>) {
    use crate::glow::{LEFT_BRAKELIGHT, LEFT_HEADLIGHT, RIGHT_BRAKELIGHT, RIGHT_HEADLIGHT};
    for f in &car.light_flares {
        let (kind, intensity) = match f.hash {
            LEFT_HEADLIGHT | RIGHT_HEADLIGHT => (0, 1.0),
            LEFT_BRAKELIGHT | RIGHT_BRAKELIGHT => (1, 0.85),
            _ => continue,
        };
        let p = m.transform_point3(f.flare.position);
        let d = m.transform_vector3(f.flare.direction);
        if let Some(g) = Glow::from_flare(p, d, kind, f.flare.flags | 4, intensity) {
            out.push(g);
        }
    }
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
