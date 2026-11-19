//! Building a car renderer from a car model (moved from car.rs).

use super::*;

impl CarRenderer {
    /// `opts`: the settings in force at the build (the texture filter).
    pub fn new(gpu: &Gpu, car: &Car, color_format: wgpu::TextureFormat, opts: &crate::render::RenderOptions) -> CarRenderer {
        let _t = crate::prof::acc::span("car renderer builds");
        let device = &gpu.device;
        let bc = device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);

        // Textures, with a missing-texture fallback. (The window glass is
        // remapped to GLOBALB's WINDOW by the car, 0x615290.)
        // g_TextureFiltering (BASEMIN / BASEMAGTEXTUREFILTER, 0x5D2096).
        let sampler = device.create_sampler(&crate::render::display::texture_sampler("car", opts.anisotropy));
        let shared = gpu.car_shared();
        let texture_layout = &shared.texture_layout;
        let flat = shared.surface_defaults(device);
        let white = upload_rgba(gpu, 1, 1, &[255, 255, 255, 255]);
        let mut textures = vec![bind_car_texture(device, texture_layout, &white, &sampler, flat)];
        let mut texture_views = vec![white];
        let mut pack_textures = vec![None];
        const WHITE: usize = 0;
        let mut slot: HashMap<u32, usize> = HashMap::new();
        // Surface Detail (G7c2): per texture the UV triangles of the draws
        // whose material has relief, by class (surface_maps::car_class).
        let mut surface_regions: HashMap<
            usize,
            (crate::formats::texture::Texture, Vec<crate::surface_maps::CarRegion>),
        > = HashMap::new();

        let mut vertices = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let mut draws = Vec::new();
        let mut glass_box = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (part_index, part) in car.parts.iter().enumerate() {
            let solid = &car.solids[part.solid];
            if solid.normals.len() != solid.vertices.len() {
                continue; // not a lit car mesh
            }
            let base_vertex = vertices.len() as i32;
            let first = indices.len() as u32;
            vertices.extend(solid.vertices.iter().zip(&solid.normals).map(|(v, n)| CarVertex {
                position: v.position,
                normal: *n,
                uv: v.uv,
            }));
            indices.extend_from_slice(&solid.indices);
            // Fix Original Model Errors: left-out triangles drawn degenerate.
            for &t in &part.hidden_tris {
                let at = first as usize + 3 * t as usize;
                if let Some(tri) = indices.get_mut(at..at + 3) {
                    tri.fill(0);
                }
            }
            for m in &solid.materials {
                let hash = solid.textures.get(m.texture_slot as usize).copied().unwrap_or(0);
                let (texture, blend, alpha_test, cover_bucket) = match car.part_texture_for(part, hash) {
                    Some(t) => {
                        let id = *slot.entry(t.hash).or_insert_with(|| {
                            let view = upload_texture_for(gpu, t, bc, crate::render::texture_pack::Domain::Car);
                            textures.push(bind_car_texture(device, texture_layout, &view, &sampler, flat));
                            texture_views.push(view);
                            pack_textures.push(Some(t.clone()));
                            textures.len() - 1
                        });
                        (id, t.blend, t.alpha_test, (t.late_buckets, t.rendering_order))
                    }
                    None => (WHITE, Blend::None, false, (false, 0)),
                };
                let material = solid
                    .light_materials
                    .get(m.light_material as usize)
                    .and_then(|h| car.part_light_material_for(part, *h))
                    .or_else(|| car.light_material_for(bin_hash("DEFAULT")));
                // The material record's own flags (+0x38), as 0x5C5BB1 queues them
                // for 0x5BD4B0 (not the mesh header's).
                let constants = material.map(|l| l.constants(m.flags)).unwrap_or_default();
                let paint = material.map_or(PaintType::Native, |l| PaintType::of(&l.name)).params();
                let wet_class = material
                    .map_or(crate::render::wet_car::WetClass::None, |l| crate::render::wet_car::WetClass::of(&l.name));
                let glass = if blend == Blend::Alpha { material.map_or(0, |l| glass::kind(&l.name)) } else { 0 };
                if texture != WHITE
                    && let Some(t) = car.part_texture_for(part, hash)
                {
                    let class = crate::surface_maps::car_class(&t.name, material.map_or("", |l| l.name.as_str()));
                    if class.params().normal > 0.0 {
                        let range = solid
                            .indices
                            .get(m.first_index as usize..(m.first_index + m.index_count) as usize)
                            .unwrap_or(&[]);
                        let tris = range.chunks_exact(3).filter_map(|c| {
                            let uv = |k: usize| solid.vertices.get(c[k] as usize).map(|v| v.uv);
                            Some([uv(0)?, uv(1)?, uv(2)?])
                        });
                        let entry = surface_regions.entry(texture).or_insert_with(|| (t.clone(), Vec::new()));
                        match entry.1.iter_mut().find(|r| r.class == class) {
                            Some(r) => r.tris.extend(tris),
                            None => entry.1.push(crate::surface_maps::CarRegion { class, tris: tris.collect() }),
                        }
                    }
                }
                let centre = glass::centre(&solid.vertices, &solid.indices, m.first_index, m.index_count);
                if glass != 0 {
                    glass::extend_box(&mut glass_box, &solid.vertices, &solid.indices, m.first_index, m.index_count);
                }
                draws.push(Draw {
                    blend,
                    alpha_test,
                    texture,
                    cover_bucket,
                    submission: draws.len(),
                    first_index: first + m.first_index,
                    index_count: m.index_count,
                    base_vertex,
                    part: part_index,
                    constants,
                    paint,
                    wet_class,
                    glass,
                    centre,
                    glass_after: false,
                });
            }
        }
        // Opaque first, blended after.
        draws.sort_by_key(|d| d.blend != Blend::None);
        glass::mark_after_glass(&mut draws);
        if indices.len() % 2 == 1 {
            indices.push(0);
        }

        let init = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
        };
        let vertices = init("car vertices", bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX);
        let indices = init("car indices", bytemuck::cast_slice(&indices), wgpu::BufferUsages::INDEX);

        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("car frame"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let envmap = shared.placeholder.get_or_init(|| start_envmap(gpu));
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("car frame"),
            layout: &shared.frame_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(envmap) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        let draw_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("car draws"),
            size: DRAW_STRIDE * draws.len().max(1) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler_clone = sampler.clone();
        let draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("car draw"),
            layout: &shared.draw_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: draw_buffer.as_entire_binding() }],
        });

        let pipelines = shared.pipelines(device, color_format);
        let shared = shared.clone();
        let mut surface_jobs: Vec<SurfaceJob> =
            surface_regions.into_iter().map(|(texture, (t, regions))| SurfaceJob { texture, t, regions }).collect();
        surface_jobs.sort_by_key(|j| j.texture);

        CarRenderer {
            meter_at: None,
            meter_share: 1.0,
            place_blink: Default::default(),
            underside_seen: false,
            shared,
            pipelines,
            reflection_pipelines: None,
            color_format,
            frame_buffer,
            env_sampler: sampler,
            frame_group,
            environment: None,
            world_environment: false,
            draw_buffer,
            draw_group,
            surface_textures: vec![None; textures.len()],
            textures,
            texture_views,
            texture_sampler: sampler_clone,
            pack_seen: crate::render::pack_reload::generation(),
            pack_textures,
            surface_jobs,
            surface_builder: None,
            vertices,
            indices,
            draws,
            part_transforms: car.parts.iter().map(|p| p.transform).collect(),
            part_wheels: car.parts.iter().map(|p| p.follows_wheel).collect(),
            wheel_centres: std::array::from_fn(|i| {
                car.car_type.wheels[i].position + glam::Vec3::X * car.wheel_shift[i]
            }),
            part_pivots: car.parts.iter().map(|p| p.pivot_y).collect(),
            part_harmonic: car.parts.iter().map(|p| p.harmonic_scale).collect(),
            part_scene: car.parts.iter().map(|p| p.scene).collect(),
            part_spinner: car.parts.iter().map(|p| p.spinner).collect(),
            part_audio: car.parts.iter().map(|p| p.audio_paint).collect(),
            part_anim: car.parts.iter().map(|p| p.anim.map(|(c, o)| (c, p.anim_group, o))).collect(),
            part_hood_gate: car.parts.iter().map(|p| p.hood_gate).collect(),
            part_harmonic_anim: car.parts.iter().map(|p| p.harmonic_anim).collect(),
            part_harmonic_glow: car.parts.iter().map(|p| p.harmonic_glow).collect(),
            glow: [[0; 4]; 2],
            part_lods: car.parts.iter().map(|p| p.lods).collect(),
            lod: Some(0),
            poses: [[None; 2]; 4],
            queued: Default::default(),
            queued_fade: Default::default(),
            mask_queued: Default::default(),
            glass_split: Default::default(),
            glass_queued: Default::default(),
            glass_eye: Default::default(),
            glass_with: Default::default(),
            glass_box,
            glass_rect: Default::default(),
            batch_frame_group: None,
            sampled: None,
            main_lod: None,
            switch: None,
            drawing_fade: None,
            fade_pipelines: None,
            wet: Default::default(),
            wet_ticks: [None; 2],
            wet_now: None,
        }
    }
}
