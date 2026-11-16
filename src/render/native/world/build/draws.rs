//! `WorldRenderer::new`'s draw list: per instance group its spheres,
//! instance matrices and draws (built side by side), the texture slots
//! numbered in group order, and the draws sorted as 0x5CF430 flushes them.

use crate::render::*;

use super::textures::TextureSlots;

/// A sphere around spheres.
pub(in crate::render) fn enclose(spheres: &[(Vec3, f32)]) -> (Vec3, f32) {
    let (lo, hi) = spheres.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(lo, hi), &(c, r)| (lo.min(c - r), hi.max(c + r)));
    let centre = (lo + hi) * 0.5;
    let radius = spheres.iter().map(|&(c, r)| c.distance(centre) + r).fold(0.0f32, f32::max);
    if radius.is_finite() && centre.is_finite() { (centre, radius) } else { (Vec3::ZERO, f32::INFINITY) }
}

// Each placement's matrix as the road reflection draws it (m22
// negated when it has a matrix), and how far each stretches.
fn mirror(p: &crate::world::Placement) -> Mat4 {
    let mut m = p.transform;
    if p.flags & NO_MATRIX_FLAG == 0 {
        m.z_axis.z = -m.z_axis.z;
    }
    m
}

// Per group, side by side: its instances' spheres and the spheres
// round them, their instance matrices, and its materials' draws
// with each texture still a hash. The slots are numbered after, in
// group order, as one serial walk did (first use), so the list is
// the same.
enum Tex {
    Sky(usize),
    Animated(usize),
    Hash(u32),
    Missing,
}
struct Proto {
    tex: Tex,
    key: PipelineKey,
    rendering_order: u8,
    late_buckets: bool,
    first_index: u32,
    index_count: u32,
    // Left out by NFS_ONLY_TEXTURE (its slot still numbered).
    skip: bool,
    water: bool,
}
struct GroupBuilt {
    spheres: Vec<(Vec3, f32)>,
    mirrored: Vec<(Vec3, f32)>,
    bound: (Vec3, f32),
    mirrored_bound: (Vec3, f32),
    matrices: Vec<[[f32; 4]; 4]>,
    protos: Vec<Proto>,
}

/// One instance group built (run side by side for all groups by
/// [`build_draws`]): the body of the parallel closure `new` had.
#[allow(clippy::too_many_arguments)]
fn build_group(
    world: &World,
    solid_spheres: &[(Vec3, f32)],
    stretches: &[(f32, f32)],
    animation_of: &HashMap<u32, usize>,
    only_texture: &Option<String>,
    default_texture: u32,
    solid_index: usize,
    transforms: &[usize],
) -> GroupBuilt {
    let (local_centre, local_radius) = solid_spheres[solid_index];
    // As the vertex shader places an instance: the matrix's 3×3
    // and translation (instance_matrix's extra values aside); in
    // the road reflection with m22 negated when it has a matrix.
    let sphere = |m: Mat4, stretch: f32| (m.transform_point3(local_centre), local_radius * stretch);
    let spheres: Vec<(Vec3, f32)> = transforms.iter().map(|&i| sphere(world.placements[i].transform, stretches[i].0)).collect();
    let mirrored: Vec<(Vec3, f32)> = transforms.iter().map(|&i| sphere(mirror(&world.placements[i]), stretches[i].1)).collect();
    let (bound, mirrored_bound) = (enclose(&spheres), enclose(&mirrored));
    let matrices = transforms
        .iter()
        .map(|&i| {
            let p = &world.placements[i];
            instance_matrix(p.transform, p.flags & NO_MATRIX_FLAG == 0, p.cull_radius, p.flags & HIDDEN_FLAG != 0)
        })
        .collect();
    let solid = &world.solids[solid_index];
    let is_sky = solid.name == SKYDOME_SOLID;
    let mesh_effect = if solid.mesh_flags & MESH_FLAG_REFLECTIVE != 0 { Effect::Reflect } else { Effect::World };
    let mut protos = Vec::with_capacity(solid.materials.len());
    for material in &solid.materials {
        if material.index_count == 0 {
            continue;
        }
        // A reference to a texture no loaded pack has gets the
        // default texture's info, DEFAULTALPHA (GLOBALB; 0x4901D0
        // with return-default set falls back to 0x82DB08, the
        // texture of hash 0x66994F4A, 0x48FDE0), blend state and all.
        let hash = solid.textures.get(material.texture_slot as usize).copied()
            .map(|h| if world.textures.contains_key(&h) || animation_of.contains_key(&h) { h } else { default_texture });
        let texture_info = hash.and_then(|h| world.textures.get(&h));
        let sky = if is_sky { texture_info.and_then(sky_part) } else { None };
        let tex = match hash {
            _ if sky.is_some() => Tex::Sky(sky.unwrap()),
            Some(h) => match animation_of.get(&h) {
                Some(&a) => Tex::Animated(a),
                None => Tex::Hash(h),
            },
            None => Tex::Missing,
        };
        let (blend, alpha_test, rendering_order, late_buckets) =
            texture_info.map_or((Blend::None, false, 0, false), |t| (t.blend, t.alpha_test, t.rendering_order, t.late_buckets));
        // 0x5C5940: additive textures on the plain world effect are
        // drawn with worldnofog. (The dome's flags are 0: world.)
        let effect = match (mesh_effect, blend) {
            (Effect::World, Blend::Additive) => Effect::NoFog,
            (e, _) => e,
        };
        // Diagnostics: NFS_ONLY_TEXTURE=<substring> keeps only draws
        // whose texture name contains it.
        let skip = only_texture.as_ref().is_some_and(|pattern| !texture_info.is_some_and(|t| t.name.contains(pattern.as_str())));
        protos.push(Proto { tex, key: PipelineKey { effect, blend, alpha_test }, rendering_order, late_buckets, first_index: material.first_index, index_count: material.index_count, skip, water: !matches!(mesh_effect, Effect::Reflect) && texture_info.is_some_and(|t| water::kind_of(&t.name).is_some()) });
    }
    GroupBuilt { spheres, mirrored, bound, mirrored_bound, matrices, protos }
}

/// The draw list and the instance groups' spheres ([`build_draws`]).
pub(in crate::render) struct Draws {
    pub(in crate::render) draws: Vec<Draw>,
    pub(in crate::render) sky_instances: Vec<u32>,
    /// NFS_ONLY_TEXTURE (diagnostics).
    pub(in crate::render) only_texture: Option<String>,
    pub(in crate::render) group_bounds: Vec<(Vec3, f32)>,
    pub(in crate::render) mirrored_bounds: Vec<(Vec3, f32)>,
    pub(in crate::render) slot_group: Vec<u32>,
    pub(in crate::render) group_local: Vec<(Vec3, f32)>,
    pub(in crate::render) slot_spheres: Vec<[(Vec3, f32); 2]>,
}

/// --- draw list ---------------------------------------------------
#[allow(clippy::too_many_arguments)]
pub(in crate::render) fn build_draws(
    world: &World,
    groups: &std::collections::BTreeMap<(usize, u32, u16, u8), Vec<usize>>,
    solid_base: &[(i32, u32)],
    placement_slots: &mut [Vec<u32>],
    instance_data: &mut Vec<[[f32; 4]; 4]>,
    slots: &mut TextureSlots,
    animation_of: &HashMap<u32, usize>,
    default_texture: u32,
) -> Draws {
    let missing = slots.missing;
    let mut slot_of = |hash: u32| slots.slot_of(hash);
    let mut draws = Vec::new();
    let mut sky_instances = Vec::new();
    let only_texture = debug_env::nfs_only_texture();
    let t_spheres = crate::prof::span("WorldRenderer::new: solid spheres");
    // Each solid's bounding sphere (all its vertices), and per instance
    // group the sphere around all its instances.
    let solid_spheres: Vec<(Vec3, f32)> = {
        use rayon::prelude::*;
        world.solids.par_iter()
        .map(|solid| {
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for v in &solid.vertices {
                let p = Vec3::from(v.position);
                lo = lo.min(p);
                hi = hi.max(p);
            }
            if solid.vertices.is_empty() {
                return (Vec3::ZERO, 0.0);
            }
            let centre = (lo + hi) * 0.5;
            let radius = solid.vertices.iter().map(|v| Vec3::from(v.position).distance(centre)).fold(0.0f32, f32::max);
            (centre, radius)
        })
        .collect()
    };
    drop(t_spheres);
    let t_groups = crate::prof::span("WorldRenderer::new: groups and draws");
    let mut group_bounds: Vec<(Vec3, f32)> = Vec::with_capacity(groups.len());
    let mut mirrored_bounds: Vec<(Vec3, f32)> = Vec::with_capacity(groups.len());
    let mut slot_group: Vec<u32> = Vec::with_capacity(world.placements.len());
    let mut group_local: Vec<(Vec3, f32)> = Vec::with_capacity(groups.len());
    let mut slot_spheres: Vec<[(Vec3, f32); 2]> = Vec::with_capacity(world.placements.len());
    let stretches: Vec<(f32, f32)> = {
        use rayon::prelude::*;
        world.placements.par_iter().map(|p| (max_stretch(glam::Mat3::from_mat4(p.transform)), max_stretch(glam::Mat3::from_mat4(mirror(p))))).collect()
    };
    let built: Vec<GroupBuilt> = {
        use rayon::prelude::*;
        let list: Vec<(&(usize, u32, u16, u8), &Vec<usize>)> = groups.iter().collect();
        list.par_iter()
            .map(|&(&(solid_index, _, _, _), transforms)| build_group(world, &solid_spheres, &stretches, animation_of, &only_texture, default_texture, solid_index, transforms))
            .collect()
    };
    for ((&(solid_index, section, flags, lods), transforms), g) in groups.iter().zip(built) {
        let solid = &world.solids[solid_index];
        let first_instance = instance_data.len() as u32;
        let group = group_bounds.len() as u32;
        let (local_centre, local_radius) = solid_spheres[solid_index];
        group_bounds.push(g.bound);
        mirrored_bounds.push(g.mirrored_bound);
        group_local.push((local_centre, local_radius));
        slot_spheres.extend(g.spheres.iter().zip(&g.mirrored).map(|(&a, &b)| [a, b]));
        for (k, &i) in transforms.iter().enumerate() {
            placement_slots[i].push(first_instance + k as u32);
            slot_group.push(group);
        }
        instance_data.extend(g.matrices);
        let instances = first_instance..instance_data.len() as u32;
        let (base_vertex, first_index) = solid_base[solid_index];
        let is_sky = solid.name == SKYDOME_SOLID;
        if is_sky {
            sky_instances.extend(instances.clone());
        }
        for p in g.protos {
            let texture = match p.tex {
                Tex::Sky(k) => TextureRef::Sky(k),
                Tex::Animated(a) => TextureRef::Animated(a),
                Tex::Hash(h) => TextureRef::Fixed(slot_of(h).unwrap_or(missing)),
                Tex::Missing => TextureRef::Fixed(missing),
            };
            if p.skip {
                continue;
            }
            draws.push(Draw {
                sky: is_sky,
                section,
                flags,
                lods,
                key: p.key,
                rendering_order: p.rendering_order,
                late_buckets: p.late_buckets,
                texture,
                first_index: first_index + p.first_index,
                index_count: p.index_count,
                base_vertex,
                instances: instances.clone(),
                group,
                water: p.water,
            });
        }
    }
    drop(t_groups);
    // Sky (0x60F890, flushed on its own), then the world in the order
    // 0x5CF430 flushes 0x5CEB40's buckets: textures without entry byte
    // 0x54 (list heads 0x870D30) before those with it (0x8712F0); in
    // each, rendering order (+0x51) 0..7; in each, effect index 0..22
    // (0x86EB28 order). Blend plays no part and nothing is sorted by
    // depth. Within a bucket the game keeps the cull's submission
    // order, textures by first use; the clone keeps its own grouping
    // prov: inferred render.new
    // there (**inferred**), and the reflection and cube views, whose
    // draws all become worldnofog, use the main view's order.
    draws.sort_by_key(|d| (!d.sky, d.late_buckets, d.rendering_order, d.key.effect.index()));
    Draws { draws, sky_instances, only_texture, group_bounds, mirrored_bounds, slot_group, group_local, slot_spheres }
}
