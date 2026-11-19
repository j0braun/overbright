//! `WorldRenderer::new`'s texture stages: the texture slots numbered
//! by first use, Surface Detail's per-slot jobs, and the
//! textures uploaded (arrays and the track's or pack's cache, or a bind
//! group each).

use crate::render::*;

/// The world's texture slots as they are numbered: by first use, slot 0
/// the plain white stand-in. [`assign_textures`] numbers the slots its
/// tables use, then the draw list the rest, in group order.
pub(in crate::render) struct TextureSlots<'w> {
    world: &'w World,
    /// Each slot's texture (None: the white stand-in), uploaded once the
    /// slots are known.
    pub(in crate::render) sources: Vec<Option<&'w Texture>>,
    /// The slot of draws whose texture is not loaded.
    pub(in crate::render) missing: usize,
    texture_slot: HashMap<u32, usize>,
}

impl<'w> TextureSlots<'w> {
    /// A loaded texture's slot, numbered at its first use.
    pub(in crate::render) fn slot_of(&mut self, hash: u32) -> Option<usize> {
        let world: &'w World = self.world;
        let t = world.textures.get(&hash)?;
        Some(*self.texture_slot.entry(hash).or_insert_with(|| {
            self.sources.push(Some(t));
            self.sources.len() - 1
        }))
    }
}

/// The texture tables [`assign_textures`] builds.
pub(in crate::render) struct Textures<'w> {
    pub(in crate::render) sampler: wgpu::Sampler,
    pub(in crate::render) texture_layout: wgpu::BindGroupLayout,
    pub(in crate::render) slots: TextureSlots<'w>,
    pub(in crate::render) animations: Vec<Animation>,
    pub(in crate::render) animation_of: HashMap<u32, usize>,
    pub(in crate::render) default_texture: u32,
    pub(in crate::render) pool_texture: Option<(usize, PipelineKey)>,
    pub(in crate::render) reflection_glow_texture: Option<(usize, PipelineKey)>,
    pub(in crate::render) particle_textures: HashMap<u32, (usize, PipelineKey)>,
    pub(in crate::render) glow_textures: HashMap<u32, (usize, PipelineKey)>,
    pub(in crate::render) sky_table: [usize; 8],
}

/// --- textures ----------------------------------------------------
pub(in crate::render) fn assign_textures<'w>(device: &wgpu::Device, world: &'w World, opts: &crate::render::RenderOptions) -> Textures<'w> {
    // g_TextureFiltering (BASEMIN / BASEMAGTEXTUREFILTER, 0x5D2096).
    let sampler = device.create_sampler(&display::texture_sampler("world", opts.anisotropy));
    let texture_layout = make_texture_layout(device, "texture", true);
    // Each slot's texture (None: the white stand-in), uploaded once the
    // slots are known.
    // Slot 0: plain white, for draws with no texture at all (and the
    // default texture's stand-in when GLOBALB is not loaded).
    let mut slots = TextureSlots { world, sources: vec![None], missing: 0, texture_slot: HashMap::new() };
    let missing = slots.missing;
    let mut slot_of = |hash: u32| slots.slot_of(hash);

    // Flip-books, keyed by the base texture hash solids refer to. In hash
    // order: their frames take the first slots, and the map's own order
    // changes run to run (the slots, the prepared cache's arrays and
    // `w.tdmap` would too).
    let mut animations = Vec::new();
    let mut animation_of: HashMap<u32, usize> = HashMap::new();
    let mut by_hash: Vec<_> = world.texture_animations.iter().collect();
    by_hash.sort_unstable_by_key(|(hash, _)| **hash);
    for (hash, anim) in by_hash {
        let frames: Vec<usize> = anim.frames.iter().filter_map(|&f| slot_of(f)).collect();
        if frames.len() > 1 {
            animation_of.insert(*hash, animations.len());
            animations.push(Animation { fps: anim.fps as f32, frames });
        }
    }

    let default_texture = bin_hash("DEFAULTALPHA");
    let pool_hash = bin_hash("2PLAYERHEADLIGHT1");
    let pool_texture = slot_of(pool_hash).zip(world.textures.get(&pool_hash)).map(|(slot, t)| {
        (slot, PipelineKey { effect: Effect::World, blend: t.blend, alpha_test: t.alpha_test })
    });
    let glow_hash = bin_hash("HEADLIGHTGLOW");
    let reflection_glow_texture = slot_of(glow_hash).zip(world.textures.get(&glow_hash)).map(|(slot, t)| {
        (slot, PipelineKey { effect: Effect::NoFog, blend: t.blend, alpha_test: t.alpha_test })
    });
    let mut particle_textures = HashMap::new();
    for d in &world.emitters.descriptions {
        if particle_textures.contains_key(&d.texture) {
            continue;
        }
        // Particles_Render (0x48BDF0) draws with the effect bound at
        // 0x5CAE8B: worldnodepth (0x86EB5C), world.efx's pixel shader
        // with a vertex shader that writes no fog.
        if let Some((slot, t)) = slot_of(d.texture).zip(world.textures.get(&d.texture)) {
            particle_textures.insert(d.texture, (slot, PipelineKey { effect: Effect::NoFog, blend: t.blend, alpha_test: t.alpha_test }));
        }
    }
    // The falling rain (0x613A80: RAINDROP through the particle batch
    // 0x5BB440), drawn by 0x61F130 (0x5CB47B) with world.efx bound
    // (0x86EB28 at 0x5CB42C): fogged.
    let rain = bin_hash("RAINDROP");
    if let Some((slot, t)) = slot_of(rain).zip(world.textures.get(&rain)) {
        particle_textures.insert(rain, (slot, PipelineKey { effect: Effect::World, blend: t.blend, alpha_test: t.alpha_test }));
    }
    let mut glow_textures = HashMap::new();
    // NEONBLUR: the cars' neon tubes (`shadow::neon_tube_quads`), drawn
    // with the glows (0x61ACA0 → 0x615A20 → 0x60D800).
    for hash in crate::glow::texture_hashes().into_iter().chain([bin_hash(crate::shadow::NEON_TUBE_TEXTURE)]) {
        if let Some((slot, t)) = slot_of(hash).zip(world.textures.get(&hash)) {
            glow_textures.insert(hash, (slot, PipelineKey { effect: Effect::NoFog, blend: t.blend, alpha_test: t.alpha_test }));
        }
    }
    let sky_table = SKY_TABLE.map(|name| slot_of(bin_hash(name)).unwrap_or(missing));
    Textures { sampler, texture_layout, slots, animations, animation_of, default_texture, pool_texture, reflection_glow_texture, particle_textures, glow_textures, sky_table }
}

/// Surface Detail's per-slot state ([`classify_surfaces`]).
pub(in crate::render) struct SurfaceJobs {
    pub(in crate::render) surface_slots: surface::SlotSurfaces,
    pub(in crate::render) surface_jobs: Vec<(usize, Texture, crate::surface_maps::Class, bool)>,
}

pub(in crate::render) fn classify_surfaces(sources: &[Option<&Texture>], draws: &[Draw]) -> SurfaceJobs {
    // Surface Detail (clone deviation, docs/notes/graphics/surface-maps.md): each slot's
    // material from its name and draws (cheap), and the slots that may
    // get a normal map (judged and made later, in the background).
    let (surface_classes, surface_reflect) = {
        let mut uses = vec![(0u32, 0u32); sources.len()];
        for d in draws {
            if let TextureRef::Fixed(slot) = d.texture {
                match d.key.effect {
                    Effect::Reflect => uses[slot].0 += 1,
                    Effect::World => uses[slot].1 += 1,
                    _ => {}
                }
            }
        }
        let reflect: Vec<bool> = uses.iter().map(|u| u.0 > u.1).collect();
        let classes: Vec<crate::surface_maps::Class> = sources
            .iter()
            .enumerate()
            .map(|(i, t)| t.map_or(crate::surface_maps::Class::Skip, |t| crate::surface_maps::slot_class(t, reflect[i], uses[i] == (0, 0))))
            .collect();
        (classes, reflect)
    };
    let surface_slots = surface::SlotSurfaces::new(&surface_classes);
    let surface_jobs: Vec<(usize, Texture, crate::surface_maps::Class, bool)> = sources
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.filter(|t| crate::surface_maps::map_candidate(t, surface_classes[i])).map(|t| (i, t.clone(), surface_classes[i], surface_reflect[i])))
        .collect();
    SurfaceJobs { surface_slots, surface_jobs }
}

/// The world's textures on the GPU ([`upload_textures`]).
pub(in crate::render) struct Uploaded {
    pub(in crate::render) pack_slots: Vec<Option<u32>>,
    pub(in crate::render) texture_scrolls: Vec<[f32; 8]>,
    pub(in crate::render) texture_arrays: Option<TextureArrays>,
    pub(in crate::render) texture_bind_groups: Vec<wgpu::BindGroup>,
    pub(in crate::render) sky_groups: Vec<wgpu::BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::render) fn upload_textures(
    gpu: &Gpu,
    world: &World,
    sources: &[Option<&Texture>],
    cached: &Option<prepared::Cached>,
    bc: bool,
    max_layers: usize,
    only_texture: &Option<String>,
    geometry: &(crate::cache::Bytes, crate::cache::Bytes),
    sampler: &wgpu::Sampler,
    texture_layout: &wgpu::BindGroupLayout,
    sky_table: &[usize; 8],
    vertex_count: usize,
    instance_data: &[[[f32; 4]; 4]],
    draws: &[Draw],
    animations: &[Animation],
) -> Uploaded {
    let device = &gpu.device;
    let t_arrays = crate::prof::span("WorldRenderer::new: texture arrays");
    // pack_reload: the slots' textures, to reload them live.
    let pack_slots: Vec<Option<u32>> = sources.iter().map(|t| t.map(|t| t.hash)).collect();
    // The textures: in arrays, or a bind group each.
    let texture_scrolls: Vec<[f32; 8]> = sources.iter().map(|t| t.map_or([0.0; 8], |t| scroll_params(&t.scroll))).collect();
    let texture_arrays = (device.features().contains(wgpu::Features::IMMEDIATES) && !debug_env::nfsu2_no_texture_arrays())
        .then(|| {
            let slot_hashes: Vec<Option<u32>> = sources.iter().map(|t| t.map(|t| t.hash)).collect();
            let max_dimension = device.limits().max_texture_dimension_2d;
            // docs/modding/texture-pack-cache.md: with a world pack, the
            // arrays with it applied come from a file of their own,
            // valid while the pack's files are as they were (`key`);
            // the track's own file keeps the native ones.
            let pack_key = {
                let _t = crate::prof::span("WorldRenderer::new: texture arrays: pack key");
                let slots: Vec<(u32, &str)> = sources.iter().flatten().map(|t| (t.hash, t.name.as_str())).collect();
                texture_pack::fingerprint(texture_pack::Domain::World, &slots, max_dimension)
            };
            let pack = texture_pack::enabled(texture_pack::Domain::World);
            let from_cache = match &pack_key {
                _ if !pack => cached.as_ref().and_then(|c| c.arrays(&slot_hashes)),
                None => None,
                Some(key) => world.cache.as_ref().filter(|_| only_texture.is_none()).and_then(|c| c.pack_file()).and_then(|f| prepared::pack_arrays(&f, bc, max_layers, key, &slot_hashes)),
            };
            let made = from_cache.is_none();
            let t_build = crate::prof::span("WorldRenderer::new: texture arrays: build");
            let arrays = from_cache.unwrap_or_else(|| prepared::arrays_with_pack(&sources, bc, max_layers, max_dimension));
            drop(t_build);
            log::info!("world texture arrays: {} arrays, {:.1} MB{}{}", arrays.arrays.len(), arrays.bytes() as f64 / 1e6, if pack { " with the world pack" } else { "" }, if made { "" } else { " (from the cache)" });
            let t_upload = crate::prof::span("WorldRenderer::new: texture arrays: upload");
            let uploaded = TextureArrays::new(gpu, &sampler, &arrays);
            drop(t_upload);
            // A first load leaves the data in the track's cache (not a
            // diagnostic subset of it); with a pack, in the pack's file.
            if made && only_texture.is_none() && let Some(c) = &world.cache {
                if !pack {
                    if let Some(sections) = prepared::to_sections(&geometry, &arrays, &slot_hashes, bc, max_layers) {
                        c.store_gpu(sections, prepared::texture_map(&arrays, &sources));
                    }
                } else if let Some(sections) = pack_key.as_deref().and_then(|key| prepared::pack_sections(&arrays, &slot_hashes, bc, max_layers, key)) {
                    c.store_pack(sections);
                }
            }
            uploaded
        });
    let texture_bind_groups: Vec<wgpu::BindGroup> = match &texture_arrays {
        Some(_) => Vec::new(),
        None => sources
            .iter()
            .map(|t| {
                let view = match t {
                    Some(t) => upload_texture_for(gpu, t, bc, texture_pack::Domain::World),
                    None => upload_rgba(gpu, 1, 1, &[255, 255, 255, 255]),
                };
                bind_world_texture(device, &texture_layout, &sampler, &view, &t.map_or(Scroll::default(), |t| t.scroll.clone()))
            })
            .collect(),
    };
    // SKYMORPHTEXTURE (group 3): the sky table's textures on their own.
    let morph_layout_group = match &texture_arrays {
        Some(_) => make_texture_layout(device, "skymorph texture", false),
        None => make_texture_layout(device, "skymorph texture", true),
    };
    let sky_groups: Vec<wgpu::BindGroup> = sky_table
        .iter()
        .map(|&slot| match &texture_arrays {
            Some(a) => bind_texture(device, &morph_layout_group, &a.view(slot), &sampler),
            None => texture_bind_groups[slot].clone(),
        })
        .collect();
    log::info!(
        "world renderer: {} vertices, {} instances, {} draws, {} textures, {} animations",
        vertex_count,
        instance_data.len(),
        draws.len(),
        sources.len(),
        animations.len()
    );

    drop(t_arrays);
    Uploaded { pack_slots, texture_scrolls, texture_arrays, texture_bind_groups, sky_groups }
}
