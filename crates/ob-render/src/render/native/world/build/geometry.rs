//! `WorldRenderer::new`'s geometry stages: all solids back to back,
//! the placements grouped by solid and view, and the sections' bounds.

use crate::render::*;

use super::draws::enclose;

/// The solids' bytes back to back ([`prepare_geometry`]).
pub(in crate::render) struct Geometry {
    pub(in crate::render) bc: bool,
    pub(in crate::render) max_layers: usize,
    /// The track's cache for this device, if it has one.
    pub(in crate::render) cached: Option<prepared::Cached>,
    /// Per solid: (base_vertex, first_index).
    pub(in crate::render) solid_base: Vec<(i32, u32)>,
    pub(in crate::render) vertex_count: usize,
    /// The vertex and index buffers' bytes.
    pub(in crate::render) geometry: (crate::cache::Bytes, crate::cache::Bytes),
}

/// --- geometry: all solids back to back ---------------------------
pub(in crate::render) fn prepare_geometry(device: &wgpu::Device, world: &World) -> Geometry {
    // (Their bytes from the track's cache when it has them for this
    // device, see `prepared`.)
    let bc = device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
    let max_layers = device.limits().max_texture_array_layers.max(1) as usize;
    let cached = world.cache.as_ref().and_then(|c| c.file()).and_then(|f| prepared::Cached::open(f, bc, max_layers));
    let mut solid_base = Vec::with_capacity(world.solids.len()); // (base_vertex, first_index)
    let (mut vertex_count, mut index_count) = (0usize, 0usize);
    for solid in &world.solids {
        solid_base.push((vertex_count as i32, index_count as u32));
        vertex_count += solid.vertices.len();
        index_count += solid.indices.len();
    }
    let file_geometry = world.cache.as_ref().and_then(|c| c.file()).and_then(prepared::file_geometry);
    let geometry = match &file_geometry {
        Some(c) if c.0.len() == vertex_count * std::mem::size_of::<Vertex>() && c.1.len() == index_count.next_multiple_of(2) * 2 => c.clone(),
        _ => prepared::geometry(world),
    };
    Geometry { bc, max_layers, cached, solid_base, vertex_count, geometry }
}

/// The placements grouped ([`build_groups`]), and what the draw list
/// fills in per placement and instance.
pub(in crate::render) struct Groups {
    /// (solid, section, view flags, LOD bits) → its placements.
    pub(in crate::render) groups: std::collections::BTreeMap<(usize, u32, u16, u8), Vec<usize>>,
    pub(in crate::render) placement_slots: Vec<Vec<u32>>,
    pub(in crate::render) instance_data: Vec<[[f32; 4]; 4]>,
}

pub(in crate::render) fn build_groups(world: &World) -> Groups {
    // Grouped by solid and by the views that draw it: the main view
    // takes each placement's top LOD, the road reflection and the cube
    // faces its `low_solid` (0x5D672A).
    // At a low world detail the main view takes LOD B instead
    // (`mid_solid`, 0x5D6717).
    let mut groups: std::collections::BTreeMap<(usize, u32, u16, u8), Vec<usize>> = Default::default();
    for (i, p) in world.placements.iter().enumerate() {
        let flags = p.flags & VIEW_FLAGS;
        let mut lods: Vec<(usize, u8)> = vec![(p.solid, MAIN_LOD)];
        for (solid, bit) in [(p.mid_solid, MAIN_LOD_B), (p.low_solid, LOW_LOD)] {
            let Some(solid) = solid else { continue };
            match lods.iter_mut().find(|(s, _)| *s == solid) {
                Some((_, bits)) => *bits |= bit,
                None => lods.push((solid, bit)),
            }
        }
        for (solid, bits) in lods {
            groups.entry((solid, p.section, flags, bits)).or_default().push(i);
        }
    }
    let placement_slots = vec![Vec::new(); world.placements.len()];
    let instance_data: Vec<[[f32; 4]; 4]> = Vec::with_capacity(world.placements.len());
    Groups { groups, placement_slots, instance_data }
}

/// The draws by section and group ([`build_bounds`]).
pub(in crate::render) struct Bounds {
    pub(in crate::render) sections: Vec<(u32, Vec<u32>, bool)>,
    pub(in crate::render) group_section: Vec<u32>,
    pub(in crate::render) group_draws: Vec<Vec<u32>>,
    pub(in crate::render) group_key: Vec<(bool, u16, u8, std::ops::Range<u32>)>,
    pub(in crate::render) section_groups: Vec<Vec<u32>>,
    pub(in crate::render) section_bounds: Vec<((Vec3, f32), (Vec3, f32), bool)>,
}

pub(in crate::render) fn build_bounds(draws: &[Draw], group_bounds: &[(Vec3, f32)], mirrored_bounds: &[(Vec3, f32)]) -> Bounds {
    let sections: Vec<(u32, Vec<u32>, bool)> = {
        let mut by: std::collections::BTreeMap<u32, (Vec<u32>, bool)> = Default::default();
        for (i, d) in draws.iter().enumerate() {
            let e = by.entry(d.section).or_default();
            e.0.push(i as u32);
            e.1 |= d.key.effect == Effect::Reflect;
        }
        by.into_iter().map(|(s, (d, r))| (s, d, r)).collect()
    };
    let mut group_section = vec![0u32; group_bounds.len()];
    let mut group_draws: Vec<Vec<u32>> = vec![Vec::new(); group_bounds.len()];
    let mut group_key = vec![(false, 0u16, 0u8, 0..0); group_bounds.len()];
    for (i, d) in draws.iter().enumerate() {
        group_draws[d.group as usize].push(i as u32);
        group_key[d.group as usize] = (d.sky, d.flags, d.lods, d.instances.clone());
    }
    let mut section_groups = Vec::with_capacity(sections.len());
    let section_bounds = sections
        .iter()
        .enumerate()
        .map(|(index, (_, list, _))| {
            let mut groups: Vec<u32> = list.iter().map(|&i| draws[i as usize].group).collect();
            groups.sort_unstable();
            groups.dedup();
            for &g in &groups {
                group_section[g as usize] = index as u32;
            }
            let spheres = |bounds: &[(Vec3, f32)]| groups.iter().map(|&g| bounds[g as usize]).collect::<Vec<_>>();
            let sky = list.iter().any(|&i| draws[i as usize].sky);
            let bounds = (enclose(&spheres(&group_bounds)), enclose(&spheres(&mirrored_bounds)), sky);
            section_groups.push(groups);
            bounds
        })
        .collect::<Vec<_>>();
    Bounds { sections, group_section, group_draws, group_key, section_groups, section_bounds }
}
