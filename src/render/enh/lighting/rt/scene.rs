use super::*;
/// The device feature the trace needs.
pub const FEATURE: wgpu::Features = wgpu::Features::EXPERIMENTAL_RAY_QUERY;

/// The adapter can trace: ray queries offered on Vulkan, on a discrete
/// GPU (`NFSU2_RT=1`: any; `NFSU2_RT=0`: never). The tier is offered then.
pub fn adapter_can(adapter: &wgpu::Adapter) -> bool {
    let info = adapter.get_info();
    let allowed = match crate::render::debug_env::nfsu2_rt().as_deref() {
        Some("0") => false,
        Some("1") => true,
        _ => info.device_type == wgpu::DeviceType::DiscreteGpu,
    };
    allowed && info.backend == wgpu::Backend::Vulkan && adapter.features().contains(FEATURE)
}

/// The features to ask the device for: ray query only when the tier is
/// chosen as the device is made (options.txt's `enh.ssr 3`, or the
/// bench's / shot's `NFSU2_BENCH_ENH` / `NFSU2_SHOT_ENH` asking for it,
/// or `NFSU2_RT=1`) on an adapter that [`adapter_can`]. Otherwise the
/// device is made exactly as before (no experimental API on the default
/// path), so the tier takes effect after a restart.
pub fn wanted(adapter: &wgpu::Adapter) -> wgpu::Features {
    // Modern Lighting's "Modern + RT" (enh.modern_lighting 3) asks too.
    let env_asks =
        [crate::render::debug_env::nfsu2_bench_enh as fn() -> Option<String>, crate::render::debug_env::nfsu2_shot_enh]
            .iter()
            .any(|k| {
                k().is_some_and(|v| {
                    v.split(';').any(|l| {
                        matches!(
                            l.split_whitespace().collect::<Vec<_>>()[..],
                            ["enh.ssr", "3"] | ["enh.modern_lighting", "3"]
                        )
                    })
                })
            });
    let asked = crate::qol::ssr_requested() == 3
        || crate::qol::modern_lighting_requested() == 3
        || env_asks
        || crate::render::debug_env::nfsu2_rt().is_some_and(|v| v == "1");
    if asked && adapter_can(adapter) { FEATURE } else { wgpu::Features::empty() }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::render) struct Uniform {
    /// The eye in the world; w the reach.
    eye: [f32; 4],
    misc: [f32; 4],
    /// x: the cars' glows.
    cars: [f32; 4],
    /// The cars' glows' surface records (rt.wgsl `rt_car_glows`).
    glows: [Surface; MAX_CAR_GLOWS],
}

pub(in crate::render) struct CarEntry {
    blas: wgpu::Blas,
    /// Its first triangle in the colour buffer.
    base: u32,
    last_used: u64,
}

/// A region's traced scene ([`frame`]).
pub(in crate::render) struct Scene {
    key: (String, usize, usize),
    world_blas: Option<wgpu::Blas>,
    world_triangles: u32,
    /// The neon and lit signs ([`emissive_geometry`]), not opaque.
    emissive_blas: Option<wgpu::Blas>,
    colors: wgpu::Buffer,
    /// The next free car slot in `colors` (triangles).
    car_next: u32,
    cars: HashMap<u64, CarEntry>,
    means: HashMap<u32, [f32; 4]>,
    tlas: [wgpu::Tlas; 3],
    uniforms: [wgpu::Buffer; 3],
    /// The world triangles' surfaces ([`Surface`]) and the hit texture
    /// array (layers of [`HIT_TEXTURE`] square), its sampler, its layer count.
    surfaces: wgpu::Buffer,
    hit_tex: wgpu::TextureView,
    hit_samp: wgpu::Sampler,
    hit_layers: u32,
    frame: u64,
    /// The cache's clock: +1 per world tick (`render::enh::tick_views`;
    /// per draw without published ticks), which the models' last use and
    /// the build budget count in.
    steps: u64,
    gate: crate::render::enh::tick_views::TickGate,
}

/// The group-1 layout the RT trace pipelines are made with (`rt.wgsl`'s
/// bindings, seen by the trace's fragment pass or by the ray list's
/// compute pass, `visibility`); one per device and stage, so a warm-up's
/// pipelines are the frame's (docs/first-use-builds.md).
pub fn layout(gpu: &Gpu, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayout {
    crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
        label: Some("rt"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty: wgpu::BindingType::AccelerationStructure { vertex_return: false },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// What a frame's trace binds at group 1 ([`super::ssr::Frame::rt`]).
#[derive(Clone)]
pub struct FrameRt {
    pub tlas: wgpu::Tlas,
    pub colors: wgpu::Buffer,
    pub uniform: wgpu::Buffer,
    /// The textured hit's inputs (rt.wgsl bindings 3-5).
    pub surfaces: wgpu::Buffer,
    pub hit_tex: wgpu::TextureView,
    pub hit_samp: wgpu::Sampler,
    /// The eye in the world (the reflections' quarter-rate history follows it).
    pub eye: Vec3,
}

impl FrameRt {
    /// The group-1 entries ([`layout`]) for this frame's scene.
    pub fn entries(&self) -> [wgpu::BindGroupEntry<'_>; 6] {
        let entry = |binding, resource| wgpu::BindGroupEntry { binding, resource };
        [
            entry(0, self.tlas.as_binding()),
            entry(1, self.colors.as_entire_binding()),
            entry(2, self.uniform.as_entire_binding()),
            entry(3, self.surfaces.as_entire_binding()),
            entry(4, wgpu::BindingResource::TextureView(&self.hit_tex)),
            entry(5, wgpu::BindingResource::Sampler(&self.hit_samp)),
        ]
    }
}

/// The hit texture array's sampler: trilinear, repeating (the world's UVs tile).
pub fn hit_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("rt hit textures"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

/// The hit texture array made on the GPU (the region's load stays short):
/// each layer from its world texture as the game stores it (DXT with its
/// mip chain, uploaded as is where the device takes BC), all its levels in
/// one compute dispatch, each from the source's level of that size
/// (`rt_hit_shrink.wgsl`: trilinear, so a box of the source's texels); a texture with too few levels for
/// that (one level, larger than twice the layer) is filtered on the CPU
/// ([`hit_layer`]); none: white.
pub fn hit_texture_gpu(gpu: &Gpu, textures: &[Option<&Texture>]) -> wgpu::TextureView {
    assert_eq!(HIT_MIPS, 8, "rt_hit_shrink.wgsl writes eight levels");
    let device = &gpu.device;
    let n = (textures.len() as u32).max(1);
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rt hit textures"),
        size: wgpu::Extent3d { width: HIT_TEXTURE, height: HIT_TEXTURE, depth_or_array_layers: n },
        mip_level_count: HIT_MIPS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let c = wgpu::ShaderStages::COMPUTE;
    let mut entries = vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: c,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: c,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ];
    for m in 0..HIT_MIPS {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2 + m,
            visibility: c,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: wgpu::TextureFormat::Rgba8Unorm,
                view_dimension: wgpu::TextureViewDimension::D2,
            },
            count: None,
        });
    }
    let layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("rt hit shrink"), entries: &entries });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rt hit shrink"),
        source: wgpu::ShaderSource::Wgsl(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/rt_hit_shrink.wgsl")).into(),
        ),
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rt hit shrink"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("rt hit shrink"),
        layout: Some(&pl),
        module: &shader,
        entry_point: Some("cs_shrink"),
        compilation_options: Default::default(),
        cache: None,
    });
    let samp = hit_sampler(device);
    let bc = device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
    let white: Vec<u8> = vec![255u8; (0..HIT_MIPS).map(|m| ((HIT_TEXTURE >> m) as usize).pow(2) * 4).sum()];
    for chunk in (0..n).collect::<Vec<u32>>().chunks(128) {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rt hit shrink") });
        let mut groups = Vec::new();
        for &l in chunk {
            let Some(t) = textures.get(l as usize).copied().flatten() else {
                hit_layer_write(gpu, &tex, l, &white);
                continue;
            };
            let ((format, w, h, mips), bytes) = crate::render::native::world::textures::texture_layer(Some(t), bc);
            // Too few levels to box down on the GPU: the CPU's filter.
            if mips == 1 && w.max(h) > 2 * HIT_TEXTURE {
                hit_layer_write(gpu, &tex, l, &hit_layer(t));
                continue;
            }
            let src = wgpu::util::DeviceExt::create_texture_with_data(
                device,
                &gpu.queue,
                &wgpu::TextureDescriptor {
                    label: Some("rt hit source"),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: mips,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &bytes,
            );
            let src_view = src.create_view(&Default::default());
            let views: Vec<wgpu::TextureView> = (0..HIT_MIPS)
                .map(|m| {
                    tex.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_mip_level: m,
                        mip_level_count: Some(1),
                        base_array_layer: l,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect();
            let mut e = vec![
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&samp) },
            ];
            for (m, v) in views.iter().enumerate() {
                e.push(wgpu::BindGroupEntry { binding: 2 + m as u32, resource: wgpu::BindingResource::TextureView(v) });
            }
            groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rt hit shrink"),
                layout: &layout,
                entries: &e,
            }));
        }
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("rt hit shrink"), timestamp_writes: None });
            cp.set_pipeline(&pipeline);
            for g in &groups {
                cp.set_bind_group(0, g, &[]);
                cp.dispatch_workgroups(HIT_TEXTURE / 8, HIT_TEXTURE / 8, 1);
            }
        }
        gpu.queue.submit([enc.finish()]);
    }
    tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() })
}

/// One layer's levels (back to back, [`hit_layer`]) into `tex`.
fn hit_layer_write(gpu: &Gpu, tex: &wgpu::Texture, l: u32, bytes: &[u8]) {
    let mut at = 0usize;
    for m in 0..HIT_MIPS {
        let side = HIT_TEXTURE >> m;
        let len = (side * side * 4) as usize;
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: tex, mip_level: m, origin: wgpu::Origin3d { x: 0, y: 0, z: l }, aspect: wgpu::TextureAspect::All },
            &bytes[at..at + len],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(side * 4), rows_per_image: Some(side) },
            wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        );
        at += len;
    }
}

/// A hit texture array of `layers` layers (at least one), each filled from
/// `data` (a layer's levels back to back, [`hit_layer`]) where given, white
/// elsewhere.
pub fn hit_texture(gpu: &Gpu, layers: u32, data: &mut dyn FnMut(u32) -> Option<Vec<u8>>) -> wgpu::TextureView {
    let n = layers.max(1);
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rt hit textures"),
        size: wgpu::Extent3d { width: HIT_TEXTURE, height: HIT_TEXTURE, depth_or_array_layers: n },
        mip_level_count: HIT_MIPS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for l in 0..n {
        let bytes = data(l).unwrap_or_else(|| vec![255u8; (0..HIT_MIPS).map(|m| ((HIT_TEXTURE >> m) as usize).pow(2) * 4).sum()]);
        let mut at = 0usize;
        for m in 0..HIT_MIPS {
            let side = HIT_TEXTURE >> m;
            let len = (side * side * 4) as usize;
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: m,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: l },
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes[at..at + len],
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(side * 4), rows_per_image: Some(side) },
                wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
            );
            at += len;
        }
    }
    tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() })
}

pub(in crate::render) static SCENE: Mutex<Option<Scene>> = Mutex::new(None);

/// The device can trace (asked for and given, [`wanted`]).
pub fn available(gpu: &Gpu) -> bool {
    gpu.device.features().contains(FEATURE)
}

/// Drop the traced scene (a region left, the row turned off).
pub fn release() {
    *SCENE.lock().unwrap() = None;
}

pub(in crate::render) fn blas_for(
    device: &wgpu::Device,
    label: &str,
    triangles: u32,
) -> (wgpu::Blas, wgpu::BlasTriangleGeometrySizeDescriptor) {
    blas_with(device, label, triangles, wgpu::AccelerationStructureGeometryFlags::OPAQUE)
}

/// [`blas_for`] with the geometry's flags (empty: not opaque, a ray query
/// sees each triangle as a candidate).
pub(in crate::render) fn blas_with(
    device: &wgpu::Device,
    label: &str,
    triangles: u32,
    flags: wgpu::AccelerationStructureGeometryFlags,
) -> (wgpu::Blas, wgpu::BlasTriangleGeometrySizeDescriptor) {
    let size = wgpu::BlasTriangleGeometrySizeDescriptor {
        vertex_format: wgpu::VertexFormat::Float32x3,
        vertex_count: triangles * 3,
        index_format: None,
        index_count: None,
        flags,
    };
    let blas = device.create_blas(
        &wgpu::CreateBlasDescriptor {
            label: Some(label),
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        },
        wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors: vec![size.clone()] },
    );
    (blas, size)
}

/// The traced scene's world half for `world` (`region`), made where the
/// scene in `guard` is not that world's: the world's geometry gathered,
/// its colours, the BLAS created and its build submitted, the TLAS and
/// uniforms made (empty of cars).
fn ensure_world(gpu: &Gpu, world: &World, region: &str, guard: &mut Option<Scene>) {
    let device = &gpu.device;
    let key = (region.to_string(), world.solids.len(), world.placements.len());
    if guard.as_ref().is_some_and(|s| s.key == key) {
        return;
    }
    *guard = None;
    let span = crate::prof::span("rt: world geometry");
    let started = std::time::Instant::now();
    let mut means = HashMap::new();
    let mut g = world_geometry(world, &mut means);
    let emissive = emissive_geometry(world, &mut g);
    drop(span);
    let geometry_ms = started.elapsed().as_secs_f64() * 1e3;
    let world_triangles = g.triangles() as u32;
    log::info!(
        "rt: world {} triangles ({} textures), {} glowing",
        world_triangles,
        means.len(),
        emissive.triangles()
    );
    // The textured hit's surfaces and layers (as many as the device's
    // array takes; the rest keep the flat colour).
    let span = crate::prof::span("rt: hit textures");
    let hit_layers = (g.layers.len() as u32).min(device.limits().max_texture_array_layers);
    let t0 = std::time::Instant::now();
    let textures: Vec<Option<&Texture>> =
        (0..hit_layers).map(|l| g.layers.get(l as usize).and_then(|h| world.textures.get(h))).collect();
    let hit_tex = if crate::render::debug_env::nfsu2_rt_cpu_layers() {
        hit_texture(gpu, hit_layers, &mut |l| textures[l as usize].map(hit_layer))
    } else {
        hit_texture_gpu(gpu, &textures)
    };
    log::info!("rt: hit textures made in {:.0} ms", t0.elapsed().as_secs_f64() * 1e3);
    let surfaces = wgpu::util::DeviceExt::create_buffer_init(
        device,
        &wgpu::util::BufferInitDescriptor {
            label: Some("rt surfaces"),
            contents: &{
                let mut v: Vec<u8> = bytemuck::cast_slice(&g.surfaces).to_vec();
                v.extend_from_slice(bytemuck::cast_slice(&emissive.surfaces));
                v
            },
            usage: wgpu::BufferUsages::STORAGE,
        },
    );
    let hit_samp = hit_sampler(device);
    drop(span);
    log::info!("rt: {} hit texture layers ({:.1} MB of surfaces)", hit_layers, g.surfaces.len() as f64 * 48.0 / 1e6);
    // Initialised at creation (the rule: `zeroed_buffer`): the
    // cars' colours are written later, frame by frame, into its tail.
    let colors = crate::render::zeroed_buffer(
        device,
        &wgpu::BufferDescriptor {
            label: Some("rt colours"),
            size: ((world_triangles + CAR_COLOR_ROOM) as u64 * 4).max(16),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        },
    );
    gpu.queue.write_buffer(&colors, 0, bytemuck::cast_slice(&g.colors));
    let tlas = std::array::from_fn(|i| {
        device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some(["rt tlas 0", "rt tlas 1", "rt tlas 2"][i]),
            max_instances: 2 + MAX_CARS as u32,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        })
    });
    let uniforms = std::array::from_fn(|_| {
        crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("rt"),
                size: std::mem::size_of::<Uniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        )
    });
    let world_blas = (world_triangles > 0).then(|| {
        let (blas, size) = blas_for(device, "rt world", world_triangles);
        let vb = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("rt world positions"),
                contents: bytemuck::cast_slice(&g.positions),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            },
        );
        let mut enc =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rt world blas") });
        enc.build_acceleration_structures(
            [&wgpu::BlasBuildEntry {
                blas: &blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                    size: &size,
                    vertex_buffer: &vb,
                    first_vertex: 0,
                    vertex_stride: 12,
                    index_buffer: None,
                    first_index: None,
                    transform_buffer: None,
                    transform_buffer_offset: None,
                }]),
            }],
            std::iter::empty(),
        );
        gpu.queue.submit([enc.finish()]);
        blas
    });
    let emissive_blas = (emissive.triangles() > 0).then(|| {
        let n = emissive.triangles() as u32;
        let (blas, size) = blas_with(device, "rt glow", n, wgpu::AccelerationStructureGeometryFlags::empty());
        let vb = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("rt glow positions"),
                contents: bytemuck::cast_slice(&emissive.positions),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            },
        );
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rt glow blas") });
        enc.build_acceleration_structures(
            [&wgpu::BlasBuildEntry {
                blas: &blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                    size: &size,
                    vertex_buffer: &vb,
                    first_vertex: 0,
                    vertex_stride: 12,
                    index_buffer: None,
                    first_index: None,
                    transform_buffer: None,
                    transform_buffer_offset: None,
                }]),
            }],
            std::iter::empty(),
        );
        gpu.queue.submit([enc.finish()]);
        blas
    });
    log::info!(
        "rt: the region's traced scene in {:.0} ms (geometry {:.0} ms)",
        started.elapsed().as_secs_f64() * 1e3,
        geometry_ms
    );
    *guard = Some(Scene {
        key,
        world_blas,
        world_triangles,
        emissive_blas,
        colors,
        car_next: 0,
        cars: HashMap::new(),
        means,
        tlas,
        uniforms,
        surfaces,
        hit_tex,
        hit_samp,
        hit_layers,
        frame: 0,
        steps: 0,
        gate: Default::default(),
    });
}

/// The traced scene's world half made now, at a drive's load or beside a
/// world loaded ahead, not in the drive's first frame ([`frame`] makes
/// it on first use: about 0.1 s of gathering on the calling thread). The
/// same objects from the same world; the first [`frame`] finds them made.
/// Nothing when the device cannot trace.
pub fn prepare(gpu: &Gpu, world: &World, region: &str) {
    if available(gpu) {
        ensure_world(gpu, world, region, &mut SCENE.lock().unwrap());
    }
}

/// This frame's traced scene: made for `world` (`region`) on first use
/// (the world's BLAS, once; [`prepare`] makes it ahead), each car model's
/// BLAS on first sight, the TLAS rebuilt with `cars` (model, car → world)
/// where they are now. The builds are submitted here, ahead of the
/// frame's own work. None when the device cannot trace.
pub fn frame(gpu: &Gpu, world: &World, region: &str, cars: &[(&Car, Mat4)], eye: Vec3) -> Option<FrameRt> {
    if !available(gpu) {
        return None;
    }
    let device = &gpu.device;
    let mut guard = SCENE.lock().unwrap();
    ensure_world(gpu, world, region, &mut guard);
    let s = guard.as_mut().unwrap();
    s.frame += 1;
    // The cache steps once per world tick (a frame between two
    // ticks builds and evicts nothing), so which models are built and kept
    // at tick n is the same at every display rate.
    let ticks = match s.gate.due(crate::render::enh::tick_views::frame().as_deref()) {
        crate::render::enh::tick_views::Due::Draw => 1,
        crate::render::enh::tick_views::Due::Ticks(v) => v.len() as u64,
    };
    s.steps += ticks;
    let now = s.steps;
    // New car models: their BLAS (and colours), at most a few a tick.
    let mut builds: Vec<(wgpu::Blas, wgpu::BlasTriangleGeometrySizeDescriptor, wgpu::Buffer)> = Vec::new();
    let mut placed: Vec<(u64, Mat4)> = Vec::with_capacity(cars.len().min(MAX_CARS));
    for &(car, m) in cars.iter().take(MAX_CARS) {
        let k = car_key(car);
        if let Some(e) = s.cars.get_mut(&k) {
            e.last_used = now;
            placed.push((k, m));
            continue;
        }
        if builds.len() as u64 >= 4 * ticks {
            continue;
        }
        let g = car_geometry(car, &mut s.means);
        let n = g.triangles() as u32;
        if n == 0 {
            continue;
        }
        if s.car_next + n > CAR_COLOR_ROOM {
            // Full: start the car cache over (models are rebuilt as seen).
            s.cars.clear();
            s.car_next = 0;
            placed.clear();
            if n > CAR_COLOR_ROOM {
                continue;
            }
        }
        let base = s.world_triangles + s.car_next;
        gpu.queue.write_buffer(&s.colors, base as u64 * 4, bytemuck::cast_slice(&g.colors));
        let (blas, size) = blas_for(device, "rt car", n);
        let vb = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("rt car positions"),
                contents: bytemuck::cast_slice(&g.positions),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            },
        );
        builds.push((blas.clone(), size, vb));
        s.cars.insert(k, CarEntry { blas, base, last_used: now });
        s.car_next += n;
        placed.push((k, m));
    }
    // Models unseen for ten seconds' worth of ticks go.
    if ticks > 0 && s.cars.len() > MAX_CARS {
        s.cars.retain(|_, e| now - e.last_used < 600);
    }
    let ring = (s.frame % 3) as usize;
    let mut instances = 0usize;
    {
        let tlas = &mut s.tlas[ring];
        if let Some(b) = &s.world_blas {
            tlas[instances] = Some(wgpu::TlasInstance::new(b, instance_transform(Mat4::IDENTITY), 0, MASK_WORLD));
            instances += 1;
        }
        if let Some(b) = &s.emissive_blas {
            tlas[instances] =
                Some(wgpu::TlasInstance::new(b, instance_transform(Mat4::IDENTITY), s.world_triangles, MASK_EMISSIVE));
            instances += 1;
        }
        for (k, m) in &placed {
            let Some(e) = s.cars.get(k) else { continue };
            tlas[instances] = Some(wgpu::TlasInstance::new(&e.blas, instance_transform(*m), e.base, MASK_CAR));
            instances += 1;
        }
        for i in instances..=MAX_CARS + 1 {
            tlas[i] = None;
        }
    }
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rt tlas") });
    let entries: Vec<wgpu::BlasBuildEntry> = builds
        .iter()
        .map(|(blas, size, vb)| wgpu::BlasBuildEntry {
            blas,
            geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                size,
                vertex_buffer: vb,
                first_vertex: 0,
                vertex_stride: 12,
                index_buffer: None,
                first_index: None,
                transform_buffer: None,
                transform_buffer_offset: None,
            }]),
        })
        .collect();
    // The cars' glows: those of the cars nearest the eye (the list is
    // nearest first), one record each after the load's.
    let mut glows = Vec::new();
    for &(car, m) in cars.iter().take(MAX_CARS) {
        car_glows(car, m, &mut glows);
        if glows.len() >= MAX_CAR_GLOWS {
            break;
        }
    }
    glows.truncate(MAX_CAR_GLOWS);
    let mut records = [Surface::default(); MAX_CAR_GLOWS];
    for (r, g) in records.iter_mut().zip(&glows) {
        *r = g.surface();
    }
    enc.build_acceleration_structures(entries.iter(), [&s.tlas[ring]]);
    crate::prof::gpu::mark(&mut enc, "graphics: rt tlas");
    gpu.queue.write_buffer(
        &s.uniforms[ring],
        0,
        bytemuck::bytes_of(&Uniform {
            eye: [eye.x, eye.y, eye.z, REACH],
            // y: textured hits (NFSU2_RT_FLAT=1: the flat colour, a check aid); z: the layers.
            misc: [
                instances as f32,
                if crate::render::debug_env::nfsu2_rt_flat() { 0.0 } else { 1.0 },
                s.hit_layers as f32,
                if crate::render::debug_env::nfsu2_rt_debug_glow() {
                    1.0
                } else if crate::render::debug_env::nfsu2_rt_debug_primary() {
                    2.0
                } else {
                    0.0
                },
            ],
            cars: [glows.len() as f32, 0.0, 0.0, 0.0],
            glows: records,
        }),
    );
    gpu.queue.submit([enc.finish()]);
    Some(FrameRt {
        tlas: s.tlas[ring].clone(),
        colors: s.colors.clone(),
        uniform: s.uniforms[ring].clone(),
        surfaces: s.surfaces.clone(),
        hit_tex: s.hit_tex.clone(),
        hit_samp: s.hit_samp.clone(),
        eye,
    })
}

/// The car cache's tick-stepped state (its clock and the models
/// kept with their last use) as a digest.
pub(in crate::render) fn digest(h: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    if let Some(s) = SCENE.lock().unwrap().as_ref() {
        s.steps.hash(h);
        let mut cars: Vec<(u64, u64)> = s.cars.iter().map(|(k, e)| (*k, e.last_used)).collect();
        cars.sort_unstable();
        cars.hash(h);
    }
}
