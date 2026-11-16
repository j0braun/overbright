//! `WorldRenderer::new`'s GPU stages: the world's and the sprites'
//! buffers, the views' frame uniforms and the environment cube, the
//! reflection image's binding, and the pipelines.

use crate::render::*;

/// The world's and the sprites' buffers ([`make_buffers`]).
pub(in crate::render) struct Buffers {
    pub(in crate::render) vertices: wgpu::Buffer,
    pub(in crate::render) indices: wgpu::Buffer,
    pub(in crate::render) instances: wgpu::Buffer,
    pub(in crate::render) pool_vertices: wgpu::Buffer,
    pub(in crate::render) pool_indices: wgpu::Buffer,
    pub(in crate::render) pool_instances: wgpu::Buffer,
    pub(in crate::render) particle_vertices: wgpu::Buffer,
    pub(in crate::render) glow_vertices: wgpu::Buffer,
    pub(in crate::render) reflection_glow_vertices: wgpu::Buffer,
    pub(in crate::render) reflection_glow_indices: wgpu::Buffer,
    pub(in crate::render) reflection_glow_instance: wgpu::Buffer,
    pub(in crate::render) particle_indices: wgpu::Buffer,
    pub(in crate::render) shadow_vertices: wgpu::Buffer,
    pub(in crate::render) particle_instance: wgpu::Buffer,
}

pub(in crate::render) fn make_buffers(device: &wgpu::Device, geometry: (crate::cache::Bytes, crate::cache::Bytes), instance_data: &[[[f32; 4]; 4]]) -> Buffers {
    let buffer = |label, contents: &[u8], usage| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
    };
    let vertices = buffer("world vertices", &geometry.0, wgpu::BufferUsages::VERTEX);
    let indices = buffer("world indices", &geometry.1, wgpu::BufferUsages::INDEX);
    drop(geometry);
    let instances = buffer("world instances", bytemuck::cast_slice(&instance_data), wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
    let pool_vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("headlight pool vertices"),
        size: (MAX_POOLS * 4 * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let pool_indices = buffer("headlight pool indices", bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]), wgpu::BufferUsages::INDEX);
    let pool_instances = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("headlight pool instances"),
        size: (MAX_POOLS * 64) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let particle_vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particle vertices"),
        size: (MAX_PARTICLE_QUADS * 4 * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let glow_vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glow vertices"),
        size: (MAX_PARTICLE_QUADS * 4 * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let quad_indices: Vec<u32> = (0..MAX_PARTICLE_QUADS.max(MAX_SHADOW_QUADS) as u32).flat_map(|q| [0, 1, 2, 0, 2, 3].map(|i| q * 4 + i)).collect();
    let reflection_glow_vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("reflection glow vertices"),
        size: (MAX_REFLECTION_GLOWS * 4 * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let reflection_glow_indices = buffer("reflection glow indices", bytemuck::cast_slice(&(0..MAX_REFLECTION_GLOWS as u32).flat_map(|q| [0, 1, 2, 0, 2, 3].map(|i| q * 4 + i)).collect::<Vec<u32>>()), wgpu::BufferUsages::INDEX);
    let reflection_glow_instance = buffer("reflection glow instance", bytemuck::cast_slice(&instance_matrix(Mat4::IDENTITY, false, 0.0, false)), wgpu::BufferUsages::VERTEX);
    let particle_indices = buffer("particle indices", bytemuck::cast_slice(&quad_indices), wgpu::BufferUsages::INDEX);
    let shadow_vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shadow vertices"),
        size: (MAX_SHADOW_QUADS * 4 * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let particle_instance = buffer("particle instance", bytemuck::cast_slice(&instance_matrix(Mat4::IDENTITY, false, 0.0, false)), wgpu::BufferUsages::VERTEX);
    Buffers {
        vertices,
        indices,
        instances,
        pool_vertices,
        pool_indices,
        pool_instances,
        particle_vertices,
        glow_vertices,
        reflection_glow_vertices,
        reflection_glow_indices,
        reflection_glow_instance,
        particle_indices,
        shadow_vertices,
        particle_instance,
    }
}

/// The views' frame uniforms, HEADLIGHTTEX and the environment cube
/// ([`make_frames`]).
pub(in crate::render) struct Frames {
    pub(in crate::render) frame_layout: wgpu::BindGroupLayout,
    pub(in crate::render) headlight_view: wgpu::TextureView,
    pub(in crate::render) headlight_sampler: wgpu::Sampler,
    pub(in crate::render) main_frame: (wgpu::Buffer, wgpu::BindGroup),
    pub(in crate::render) reflection_frame: (wgpu::Buffer, wgpu::BindGroup),
    pub(in crate::render) reflection_effect_frame: (wgpu::Buffer, wgpu::BindGroup),
    pub(in crate::render) rear_frame: (wgpu::Buffer, wgpu::BindGroup),
    pub(in crate::render) env_frames: [(wgpu::Buffer, wgpu::BindGroup); 6],
    pub(in crate::render) env: EnvCube,
    /// The six faces' frames in one buffer, where the cube is drawn in
    /// one multiview pass.
    pub(in crate::render) env_frames_all: Option<(wgpu::Buffer, wgpu::BindGroup)>,
}

/// --- per-pass uniforms --------------------------------------------
pub(in crate::render) fn make_frames(gpu: &Gpu, world: &World, default_texture: u32, bc: bool, opts: &crate::render::RenderOptions) -> Frames {
    let device = &gpu.device;
    // Binding 1/2: HEADLIGHTTEX, the texture `HEADLIGHTS` (0x5BC700:
    // 0x4901D0 with return-default set), for worldheadlight.
    let frame_layout = frame_layout(device);
    let headlight_view = world
        .textures
        .get(&bin_hash("HEADLIGHTS"))
        .or_else(|| world.textures.get(&default_texture))
        .map_or_else(|| upload_rgba(gpu, 1, 1, &[0, 0, 0, 0]), |t| upload_texture_for(gpu, t, bc, texture_pack::Domain::World));
    // The projection runs off the texture's edges: clamp (inferred;
    // its border texels are black either way).
    let headlight_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("headlight"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    // Bindings 3–8 of every view but the main one with the lamps on:
    // placeholders (their frames never turn the lamps on).
    let lamp_placeholder = lamps::LampBindings::placeholder(device);
    let make_frame_of = |label, count: u64| {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: std::mem::size_of::<FrameUniform>() as u64 * count,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &frame_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&headlight_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&headlight_sampler) },
            ]
            .into_iter()
            .chain(lamp_placeholder.entries())
            .collect::<Vec<_>>(),
        });
        (buffer, group)
    };
    let make_frame = |label| make_frame_of(label, 1);
    let main_frame = make_frame("frame");
    let reflection_frame = make_frame("reflection frame");
    let reflection_effect_frame = make_frame("authored reflection effects");
    let rear_frame = make_frame("rear view frame");
    let env_frames = std::array::from_fn(|_| make_frame("environment face"));
    let env = EnvCube::new(gpu, opts.car_reflection_size.max(ENV_SIZE));
    let multiview = device.features().contains(wgpu::Features::MULTIVIEW) && device.limits().max_multiview_view_count >= 6 && !debug_env::nfsu2_no_multiview();
    let env_frames_all = multiview.then(|| make_frame_of("environment faces", 6));
    Frames { frame_layout, headlight_view, headlight_sampler, main_frame, reflection_frame, reflection_effect_frame, rear_frame, env_frames, env, env_frames_all }
}

/// --- reflection image binding (group 2) ----------------------------
/// Its layout and sampler, and group 2 while drawing the reflection
/// itself.
pub(in crate::render) fn make_reflection_binding(gpu: &Gpu) -> (wgpu::BindGroupLayout, wgpu::Sampler, wgpu::BindGroup) {
    let device = &gpu.device;
    let reflection_layout = make_texture_layout(device, "reflection", false);
    let reflection_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("reflection"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let black = upload_rgba(gpu, 1, 1, &[0, 0, 0, 255]);
    let no_reflection = bind_texture(device, &reflection_layout, &black, &reflection_sampler);
    (reflection_layout, reflection_sampler, no_reflection)
}

/// The scene's, the reflection's, the glows' and the cube's pipelines
/// ([`make_pipelines`]).
pub(in crate::render) struct Pipelines {
    pub(in crate::render) arrays: bool,
    pub(in crate::render) glow_keys: Vec<PipelineKey>,
    pub(in crate::render) pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(in crate::render) reflection_pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(in crate::render) glow_pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(in crate::render) glow_streak_pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(in crate::render) env_multiview: Option<EnvMultiview>,
}

pub(in crate::render) fn make_pipelines(
    gpu: &Gpu,
    color_format: wgpu::TextureFormat,
    texture_arrays: &Option<TextureArrays>,
    glow_textures: &HashMap<u32, (usize, PipelineKey)>,
    env_frames_all: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    env: &EnvCube,
) -> Pipelines {
    let device = &gpu.device;
    let t_pipelines = crate::prof::span("WorldRenderer::new: pipelines");
    // --- pipelines: the device's, shared by its world renderers --------
    let shared = gpu.world_shared();
    let arrays = texture_arrays.is_some();
    let all = PipelineKey::all();
    // Only reflection pipelines need the planar clipping entry points;
    // ordinary/native fragments retain their original early-depth behavior.
    let mut reflection_keys = all.clone();
    reflection_keys.extend(all.iter().filter(|k| k.effect == Effect::NoFog).map(|k| PipelineKey { effect: Effect::PlanarNoFog, ..*k }));
    let glow_keys: Vec<PipelineKey> = glow_textures.values().map(|&(_, k)| k).collect();
    let pipelines_for = |format, mask, keys: &[PipelineKey]| shared.pipelines(device, arrays, format, mask, keys);
    let ((pipelines, reflection_pipelines), (glow_pipelines, glow_streak_pipelines)) = rayon::join(
        || rayon::join(|| pipelines_for(color_format, wgpu::ColorWrites::ALL, &all), || pipelines_for(REFLECTION_FORMAT, wgpu::ColorWrites::ALL, &reflection_keys)),
        || rayon::join(|| pipelines_for(color_format, wgpu::ColorWrites::COLOR, &glow_keys), || pipelines_for(crate::render::postfx::STREAK_FORMAT, wgpu::ColorWrites::COLOR, &glow_keys)),
    );
    let env_multiview = env_frames_all.map(|frames| {
        let keys: Vec<PipelineKey> = {
            let mut k: Vec<PipelineKey> = all.iter().map(|k| k.in_reflection()).collect();
            k.sort();
            k.dedup();
            k
        };
        let depth = env_multiview_depth(gpu, env.size);
        let array = wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() };
        EnvMultiview {
            frames,
            color: env.texture.create_view(&array),
            depth: depth.create_view(&array),
            pipelines: shared.pipelines_for(device, arrays, true, SceneMode::NATIVE, REFLECTION_FORMAT, wgpu::ColorWrites::ALL, &keys),
        }
    });
    drop(t_pipelines);
    Pipelines { arrays, glow_keys, pipelines, reflection_pipelines, glow_pipelines, glow_streak_pipelines, env_multiview }
}
