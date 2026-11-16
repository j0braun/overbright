use super::*;

pub(super) struct Targets {
    key: ((u32, u32), u32),
    depth: wgpu::TextureView,
    out: wgpu::TextureView,
    tiles: wgpu::Buffer,
    ntiles: u32,
    /// The temporal resolve's ping-pong targets, the frames resolved,
    /// and the last frame's view (eye-relative view-projection, eye,
    /// low-res viewport).
    resolved: [wgpu::TextureView; 2],
    pub(super) frame: u32,
    pub(super) prev: Option<(Mat4, Vec3, [f32; 4])>,
    /// The drawn frames' place on the tick axis (the history's
    /// weight follows the frame's span, `render::enh::tick_views`).
    pub(super) span: crate::render::enh::tick_views::FrameSpan,
}

/// The composite's tiles (low-res pixels).
const TILE: u32 = 16;
/// The scene ambient's blocks across and down the target.
pub const AMBIENT_W: u32 = 32;
pub const AMBIENT_H: u32 = 18;

/// The renderer (module docs).
pub struct VolumeFx {
    pub(super) budget: Budget,
    look: Look,
    uniform: wgpu::Buffer,
    lights: wgpu::Buffer,
    pub(super) particles: wgpu::Buffer,
    bricks: wgpu::Buffer,
    bounds: wgpu::Buffer,
    dens: wgpu::TextureView,
    light: wgpu::TextureView,
    light_dir: wgpu::TextureView,
    noise: wgpu::TextureView,
    sampler: wgpu::Sampler,
    noise_sampler: wgpu::Sampler,
    splat_group: wgpu::BindGroup,
    resolve_group: wgpu::BindGroup,
    light_group: wgpu::BindGroup,
    splat: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    bounds_pass: wgpu::ComputePipeline,
    lighting: wgpu::ComputePipeline,
    march_layouts: [wgpu::BindGroupLayout; 2],
    march: [wgpu::RenderPipeline; 2],
    /// The temporal resolve ([`Look::temporal`]).
    temporal_layout: wgpu::BindGroupLayout,
    temporal: wgpu::RenderPipeline,
    full_layouts: [wgpu::BindGroupLayout; 2],
    full_modules: [wgpu::ShaderModule; 2],
    /// The scene's block averages ([`AMBIENT_W`] × [`AMBIENT_H`] and
    /// their mean) and the pass that takes them, for a plain or a
    /// multisampled target.
    pub(super) ambient: wgpu::Buffer,
    ambient_layouts: [wgpu::BindGroupLayout; 2],
    ambient_passes: [[wgpu::ComputePipeline; 2]; 2],
    pipelines: std::sync::Mutex<std::collections::HashMap<(wgpu::TextureFormat, u32, bool), wgpu::RenderPipeline>>,
    pub(super) targets: std::sync::Mutex<Option<Targets>>,
    view: Option<View>,
    built: bool,
    /// The grid's anchor ([`grid_origin_at`]): [`AHEAD`], [`BELOW`] unless set.
    anchor: [f32; 2],
    /// The march's step length × this ([`VolumeFx::set_step_scale`];
    /// 1 unless set).
    step_scale: f32,
    /// The share of the march's start jitter (1 unless set).
    jitter: f32,
    /// The grid's min corner of the last build (the medium's placement).
    origin: Option<Vec3>,
    /// Added to [`march_div`] for this renderer (thin, soft media
    /// such as dust and steam march coarser; [`VolumeFx::set_coarser`]).
    coarser: u32,
}

static PLAIN_RESOLVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `cs_resolve` clearing the accumulators with an exchange each and marking the bricks voxel by voxel
/// (true), as before it read the bounds once a workgroup and gathered the bricks there: the same volumes and
/// marks. For the test that compares the two.
pub fn set_plain_resolve(on: bool) {
    PLAIN_RESOLVE.store(on, std::sync::atomic::Ordering::Relaxed);
}

static PLAIN_MARCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Renderers made from now on march with every branch in (true), as before the march was specialised to its
/// look: the same frames. For the test that compares the two.
pub fn set_plain_march(on: bool) {
    PLAIN_MARCH.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The march's override constants for `look` (march.wgsl `FINE`, `DIRECTIONAL`, `SHADOW`): the branches its
/// look takes.
pub(super) fn march_constants(look: &Look) -> [(&'static str, f64); 3] {
    let plain = PLAIN_MARCH.load(std::sync::atomic::Ordering::Relaxed);
    let on = |x: f32| if x > 0.0 || plain { 1.0 } else { 0.0 };
    [("FINE", on(look.fine)), ("DIRECTIONAL", on(look.directional)), ("SHADOW", on(look.shadow))]
}

/// The volume effects' programs (the same for every budget and look) and
/// their composite into `format` × `samples` against a depth of
/// `depth_samples`, made now through a smallest copy (a warm-up,
/// docs/first-use-builds.md). Nothing is drawn.
pub fn warm_programs(gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32) {
    let smallest = Budget { dims: [8, 8, 8], voxel: 1.0, particles: 64, steps: 8 };
    // (The march per override set: one look of each the users' looks take, [`WARM_LOOKS`].)
    for look in WARM_LOOKS {
        VolumeFx::new(gpu, smallest, look).warm(gpu, format, samples, depth_samples);
    }
    drop(ParticlePool::new(gpu, 64));
}

/// One look for each set of the march's override constants ([`march_constants`]) the renderer's users take:
/// Ambient Detail's ([`LOOK`]: the surface shadow), the Cinematic smoke's (the fine octave, the directional
/// shading, the shadow) and the wet spray's and the water's mist (none). [`warm_programs`] makes their
/// marches.
pub const WARM_LOOKS: [Look; 3] =
    [LOOK, Look { fine: 1.0, directional: 1.0, ..LOOK }, Look { shadow: 0.0, ..LOOK }];

impl VolumeFx {
    pub fn new(gpu: &Gpu, budget: Budget, look: Look) -> VolumeFx {
        assert!(budget.dims.iter().all(|d| d % 8 == 0), "dims: multiples of 8");
        let device = &gpu.device;
        let [nx, ny, nz] = budget.dims;
        let voxels = (nx * ny * nz) as u64;
        use wgpu::BufferUsages as U;
        let buffer = |label, size: u64, usage| {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false },
            )
        };
        // Both start as zeros: `zeroed_buffer` makes them so at creation (a
        // second write of zeros copied the same bytes again from the CPU:
        // ~30 ms of the main thread in the frame that made Ambient Detail's
        // volume).
        let acc = buffer("vfx accumulators", voxels * CH * 4, U::STORAGE | U::COPY_DST);
        let particles = buffer(
            "vfx particles",
            budget.particles as u64 * std::mem::size_of::<Particle>() as u64,
            U::STORAGE | U::COPY_DST | U::COPY_SRC,
        );
        let nbricks = voxels / (BRICK * BRICK * BRICK) as u64;
        let bricks = buffer("vfx bricks", nbricks * 4, U::STORAGE | U::COPY_DST);
        let bounds = buffer("vfx bounds", (BOUNDS * 4) as u64, U::STORAGE | U::COPY_DST);
        gpu.queue.write_buffer(&bounds, 0, bytemuck::cast_slice(&empty_bounds()));
        let uniform_buf = buffer("vfx volume", std::mem::size_of::<VolUniform>() as u64, U::UNIFORM | U::COPY_DST);
        let lights = buffer("vfx lights", std::mem::size_of::<LightsUniform>() as u64, U::UNIFORM | U::COPY_DST);
        let volume = |label, dims: [u32; 3], format| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: dims[0], height: dims[1], depth_or_array_layers: dims[2] },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D3,
                    format,
                    usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let dens = volume("vfx density", budget.dims, wgpu::TextureFormat::Rgba16Float);
        let albedo = volume("vfx albedo", budget.dims, wgpu::TextureFormat::Rgba8Unorm);
        let emission = volume("vfx emission", budget.dims, wgpu::TextureFormat::Rgba16Float);
        let light = volume("vfx light", [nx / 2, ny / 2, nz / 2], wgpu::TextureFormat::Rgba16Float);
        // The light volume's dominant light direction (xyz; its
        // length: how one-sided the direct light is).
        let light_dir = volume("vfx light dir", [nx / 2, ny / 2, nz / 2], LDIR);
        let clamp = wgpu::AddressMode::ClampToEdge;
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: clamp,
            address_mode_v: clamp,
            address_mode_w: clamp,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let noise_sampler = repeat_sampler(gpu);
        let noise = noise_view(gpu);

        let [_, splat_src, resolve_src, light_src, march_src, full_src, temporal_src] = sections();
        let cs = wgpu::ShaderStages::COMPUTE;
        let fs = wgpu::ShaderStages::FRAGMENT;
        let vf = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let f3 = wgpu::TextureSampleType::Float { filterable: true };
        let nf = wgpu::TextureSampleType::Float { filterable: false };
        let d3 = wgpu::TextureViewDimension::D3;
        let d2 = wgpu::TextureViewDimension::D2;
        use wgpu::BindingResource as R;

        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
        };
        let splat_layout = layout(
            "vfx splat",
            &[
                uniform(0, cs, false),
                storage(1, true, cs),
                tex(2, cs, f3, d3, false),
                samp(3, cs),
                storage(4, false, cs),
                storage(5, false, cs),
            ],
        );
        let splat_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vfx splat"),
            layout: &splat_layout,
            entries: &[
                entry(0, uniform_buf.as_entire_binding()),
                entry(1, particles.as_entire_binding()),
                entry(2, R::TextureView(&noise)),
                entry(3, R::Sampler(&noise_sampler)),
                entry(4, acc.as_entire_binding()),
                entry(5, bounds.as_entire_binding()),
            ],
        });
        let resolve_layout = layout(
            "vfx resolve",
            &[
                uniform(0, cs, false),
                storage(1, false, cs),
                storage(2, false, cs),
                store(3, wgpu::TextureFormat::Rgba16Float),
                store(4, wgpu::TextureFormat::Rgba8Unorm),
                store(5, wgpu::TextureFormat::Rgba16Float),
                storage(6, false, cs),
            ],
        );
        let resolve_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vfx resolve"),
            layout: &resolve_layout,
            entries: &[
                entry(0, uniform_buf.as_entire_binding()),
                entry(1, acc.as_entire_binding()),
                entry(2, bricks.as_entire_binding()),
                entry(3, R::TextureView(&dens)),
                entry(4, R::TextureView(&albedo)),
                entry(5, R::TextureView(&emission)),
                entry(6, bounds.as_entire_binding()),
            ],
        });
        let light_layout = layout(
            "vfx light",
            &[
                uniform(0, cs, false),
                uniform(1, cs, false),
                tex(2, cs, f3, d3, false),
                tex(3, cs, f3, d3, false),
                samp(4, cs),
                storage(5, true, cs),
                store(6, wgpu::TextureFormat::Rgba16Float),
                tex(7, cs, f3, d3, false),
                store(8, LDIR),
            ],
        );
        let light_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vfx light"),
            layout: &light_layout,
            entries: &[
                entry(0, uniform_buf.as_entire_binding()),
                entry(1, lights.as_entire_binding()),
                entry(2, R::TextureView(&dens)),
                entry(3, R::TextureView(&albedo)),
                entry(4, R::Sampler(&sampler)),
                entry(5, bricks.as_entire_binding()),
                entry(6, R::TextureView(&light)),
                entry(7, R::TextureView(&emission)),
                entry(8, R::TextureView(&light_dir)),
            ],
        });
        let splat =
            compute_pipelines(gpu, "vfx splat", &splat_layout, &module(gpu, "vfx splat", splat_src), &["cs_splat"])
                .pop()
                .unwrap();
        let mut rp = compute_pipelines(
            gpu,
            "vfx resolve",
            &resolve_layout,
            &module(gpu, "vfx resolve", resolve_src),
            &["cs_resolve", "cs_bounds"],
        );
        let bounds_pass = rp.pop().unwrap();
        let resolve = rp.pop().unwrap();
        let lighting =
            compute_pipelines(gpu, "vfx light", &light_layout, &module(gpu, "vfx light", light_src), &["cs_light"])
                .pop()
                .unwrap();

        // The march (and the nearest-depth downsample, a second target),
        // for a plain or a multisampled scene depth.
        let march_layouts = [false, true].map(|ms| {
            layout(
                "vfx march",
                &[
                    uniform(0, vf, false),
                    tex(1, fs, f3, d3, false),
                    tex(2, fs, f3, d3, false),
                    samp(3, fs),
                    tex(4, fs, f3, d3, false),
                    samp(5, fs),
                    storage(6, true, fs),
                    tex(7, fs, wgpu::TextureSampleType::Depth, d2, ms),
                    storage(8, true, vf),
                    storage(9, false, fs),
                    storage(10, true, fs),
                    tex(11, fs, f3, d3, false),
                ],
            )
        });
        let march = [false, true].map(|ms| {
            let m = module(
                gpu,
                "vfx march",
                march_src
                    .replace("DEPTH_TEXTURE", if ms { "texture_depth_multisampled_2d" } else { "texture_depth_2d" }),
            );
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("vfx march"),
                bind_group_layouts: &[Some(&march_layouts[usize::from(ms)])],
                immediate_size: 0,
            });
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some("vfx march"),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &m,
                    entry_point: Some("vs_march"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &m,
                    entry_point: Some("fs_march"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &march_constants(&look),
                        ..Default::default()
                    },
                    targets: &[
                        Some(wgpu::ColorTargetState { format: OUT, blend: None, write_mask: wgpu::ColorWrites::ALL }),
                        Some(wgpu::ColorTargetState {
                            format: LDEPTH,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        });
        // The temporal resolve (this frame's march, its depths, the
        // history) into one of two ping-pong targets.
        let temporal_layout = layout(
            "vfx temporal",
            &[
                uniform(0, vf, false),
                tex(1, fs, f3, d2, false),
                tex(2, fs, nf, d2, false),
                tex(3, fs, f3, d2, false),
                samp(4, fs),
                storage(5, true, vf),
            ],
        );
        let temporal = {
            let m = module(gpu, "vfx temporal", temporal_src);
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("vfx temporal"),
                bind_group_layouts: &[Some(&temporal_layout)],
                immediate_size: 0,
            });
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some("vfx temporal"),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &m,
                    entry_point: Some("vs_temporal"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &m,
                    entry_point: Some("fs_temporal"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: OUT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        };
        let full_layout = |ms: bool| {
            layout(
                "vfx full",
                &[
                    uniform(0, vf, false),
                    tex(1, fs, wgpu::TextureSampleType::Depth, d2, ms),
                    tex(2, fs, nf, d2, false),
                    tex(3, fs, f3, d2, false),
                    samp(4, fs),
                    storage(5, true, vf),
                    storage(6, true, vf),
                ],
            )
        };
        let full_layouts = [full_layout(false), full_layout(true)];
        // The scene ambient (block averages of the target, then their mean).
        let ambient =
            buffer("vfx ambient", ((AMBIENT_W * AMBIENT_H + 1) * 16) as u64, U::STORAGE | U::COPY_DST | U::COPY_SRC);
        gpu.queue.write_buffer(&ambient, 0, &vec![0u8; ((AMBIENT_W * AMBIENT_H + 1) * 16) as usize]);
        let ambient_layouts =
            [false, true].map(|ms| layout("vfx ambient", &[tex(0, cs, nf, d2, ms), storage(1, false, cs)]));
        let ambient_passes = [false, true].map(|ms| {
            let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/volume_fx_ambient.wgsl"))
                .replace("SCENE_TEXTURE", if ms { "texture_multisampled_2d<f32>" } else { "texture_2d<f32>" });
            let m = module(gpu, "vfx ambient", src);
            let mut v = compute_pipelines(
                gpu,
                "vfx ambient",
                &ambient_layouts[usize::from(ms)],
                &m,
                &["cs_ambient", "cs_ambient_mean"],
            );
            let mean = v.pop().unwrap();
            [v.pop().unwrap(), mean]
        });
        let full_modules = [
            module(gpu, "vfx full", full_src.replace("DEPTH_TEXTURE", "texture_depth_2d")),
            module(gpu, "vfx full ms", full_src.replace("DEPTH_TEXTURE", "texture_depth_multisampled_2d")),
        ];
        VolumeFx {
            budget,
            look,
            uniform: uniform_buf,
            lights,
            particles,
            bricks,
            bounds,
            dens,
            light,
            light_dir,
            noise,
            sampler,
            noise_sampler,
            splat_group,
            resolve_group,
            light_group,
            splat,
            resolve,
            bounds_pass,
            lighting,
            march_layouts,
            march,
            temporal_layout,
            temporal,
            full_layouts,
            full_modules,
            ambient,
            ambient_layouts,
            ambient_passes,
            pipelines: Default::default(),
            targets: Default::default(),
            view: None,
            built: false,
            anchor: [AHEAD, BELOW],
            step_scale: 1.0,
            jitter: 1.0,
            origin: None,
            coarser: 0,
        }
    }

    /// Move the grid's anchor: its centre `ahead` m in front of the eye,
    /// from `below` m under it (default [`AHEAD`], [`BELOW`]).
    pub fn set_anchor(&mut self, ahead: f32, below: f32) {
        self.anchor = [ahead, below];
    }

    /// The march's step length × `scale` (default 1; the spray
    /// takes shorter steps: its puffs are a few voxels across, and the
    /// default steps left the start jitter's pattern in them without a
    /// temporal pass).
    pub fn set_step_scale(&mut self, scale: f32) {
        self.step_scale = scale.clamp(0.25, 1.0);
    }

    /// The share of the march's start jitter (default 1, the
    /// interleaved gradient noise in full). Without a temporal pass to
    /// resolve it, a coarse march (4K with supersampling: a low-res pixel
    /// is ~3 output pixels) shows its lattice; short steps (above) need
    /// less of it.
    pub fn set_jitter(&mut self, share: f32) {
        self.jitter = share.clamp(0.0, 1.0);
    }

    /// docs/atmosphere.md: the last build's density volume (r: the
    /// extinction, 1/m), its min corner and voxel edge (m) and its voxels,
    /// for the Next-Gen atmosphere (the haze behind the medium is hidden by
    /// it). None before the first build.
    pub fn medium(&self) -> Option<(wgpu::TextureView, Vec3, f32, [u32; 3])> {
        self.origin.map(|o| (self.dens.clone(), o, self.budget.voxel, self.budget.dims))
    }

    /// March this renderer `extra` steps coarser than [`march_div`]
    /// (at most ⅙ resolution); for thin, soft media. Default 0.
    pub fn set_coarser(&mut self, extra: u32) {
        self.coarser = extra.min(4);
    }

    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// The particle buffer (STORAGE; [`Budget::particles`] entries) for GPU
    /// producers; entries past the count given to [`VolumeFx::build`] are
    /// ignored, entries with radius 0 skipped.
    pub fn particles(&self) -> &wgpu::Buffer {
        &self.particles
    }

    /// Write CPU-made particles at `offset`.
    pub fn write_particles(&self, gpu: &Gpu, offset: u32, particles: &[Particle]) {
        let n = particles.len().min(self.budget.particles.saturating_sub(offset) as usize);
        if n > 0 {
            gpu.queue.write_buffer(
                &self.particles,
                offset as u64 * std::mem::size_of::<Particle>() as u64,
                bytemuck::cast_slice(&particles[..n]),
            );
        }
    }

    /// The frame's camera and lights (before [`VolumeFx::build`]).
    pub fn frame(&mut self, gpu: &Gpu, view: View, lights: &SceneLights) {
        self.view = Some(view);
        gpu.queue.write_buffer(&self.lights, 0, bytemuck::bytes_of(&lights_uniform(lights, view.eye)));
    }

    /// The frame's volumes from the first `count` particles (after the
    /// producers' passes in `encoder`).
    pub fn build(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, count: u32) {
        let Some(view) = self.view else { return };
        let b = self.budget;
        let count = count.min(b.particles);
        let origin = grid_origin_at(b, view.eye, view.forward, self.anchor);
        let l = self.look;
        let wind = Fields::default().wind;
        let u = VolUniform {
            origin: origin.extend(b.voxel).to_array(),
            dims: [b.dims[0], b.dims[1], b.dims[2], count],
            eye: view.eye.extend(view.time).to_array(),
            ambient: l.ambient.extend(l.g).to_array(),
            look2: [l.erosion, l.detail, l.thin_white, debug_mode()],
            inv_vp: view.view_proj.inverse().to_cols_array_2d(),
            forward: view.forward.extend(250.0).to_array(),
            viewport: [0.0, 0.0, 1.0, 1.0],
            sizes: [1.0; 4],
            depth: [view.near, view.far, if view.reversed { 1.0 } else { 0.0 }, 1.0],
            look3: [l.shade, l.lamp, l.spot, l.point],
            wind: wind.extend(b.steps as f32).to_array(),
            vp: view.view_proj.to_cols_array_2d(),
            look4: [l.sharp, l.fine.max(0.0), l.directional.clamp(0.0, 1.0), l.step.clamp(0.5, 2.0)],
            look6: {
                let [st, ji] = march_tuning(self.step_scale, self.jitter);
                [st, ji, if PLAIN_RESOLVE.load(std::sync::atomic::Ordering::Relaxed) { 1.0 } else { 0.0 }, 0.0]
            },
            fog_value: view.fog.map_or([0.0; 4], |(v, _)| [v[0], v[1], v[2], 1.0]),
            fog_color: view.fog.map_or([0.0; 4], |(_, c)| [c[0], c[1], c[2], 0.0]),
            look5: [l.scene, l.near.max(0.0), l.shadow.clamp(0.0, 1.0), 0.0],
            // (Written by the draw.)
            prev_vp: [[0.0; 4]; 4],
            prev_eye: [0.0; 4],
            prev_view: [0.0; 4],
            temporal: [0.0; 4],
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        encoder.clear_buffer(&self.bricks, 0, None);
        let mut pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("vfx build"), timestamp_writes: None });
        if count > 0 {
            pass.set_pipeline(&self.splat);
            pass.set_bind_group(0, &self.splat_group, &[]);
            pass.dispatch_workgroups(count.min(65535), count.div_ceil(65535), 1);
        }
        pass.set_bind_group(0, &self.resolve_group, &[]);
        pass.set_pipeline(&self.resolve);
        pass.dispatch_workgroups(b.dims[0] / 4, b.dims[1] / 4, b.dims[2] / 4);
        pass.set_pipeline(&self.bounds_pass);
        pass.dispatch_workgroups(1, 1, 1);
        pass.set_pipeline(&self.lighting);
        pass.set_bind_group(0, &self.light_group, &[]);
        pass.dispatch_workgroups(b.dims[0] / 8, b.dims[1] / 8, b.dims[2] / 8);
        drop(pass);
        self.built = true;
        self.origin = Some(origin);
        crate::prof::gpu::mark(encoder, "graphics: volume fx build");
    }

    /// The composite for a target `format` × `samples` over a plain or
    /// multisampled depth.
    fn composite_pipeline(
        &self,
        gpu: &Gpu,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        let key = (format, samples, ms);
        let mut map = self.pipelines.lock().unwrap();
        if let Some(p) = map.get(&key) {
            return p.clone();
        }
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("vfx full"),
            bind_group_layouts: &[Some(&self.full_layouts[usize::from(ms)])],
            immediate_size: 0,
        });
        let module = &self.full_modules[usize::from(ms)];
        // The composite: premultiplied over; the alpha (the bleach mask) as
        // the native alpha blend writes it (as the particle tiers).
        let blend = Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        });
        let p = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("vfx composite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_tiles"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_composite"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        map.insert(key, p.clone());
        p
    }

    /// The draw's pipelines for these arguments built now (no first-use
    /// hitch).
    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32) {
        self.composite_pipeline(gpu, format, samples, depth_samples > 1);
    }

    fn targets(&self, gpu: &Gpu, low: (u32, u32), div: u32) -> std::sync::MutexGuard<'_, Option<Targets>> {
        let mut t = self.targets.lock().unwrap();
        if t.as_ref().is_none_or(|t| t.key != (low, div)) {
            let make = |label, format| {
                gpu.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: low.0, height: low.1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let ntiles = low.0.div_ceil(TILE) * low.1.div_ceil(TILE);
            let tiles = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vfx tiles"),
                size: ntiles as u64 * 4,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            *t = Some(Targets {
                key: (low, div),
                depth: make("vfx depth", LDEPTH),
                out: make("vfx march", OUT),
                tiles,
                ntiles,
                resolved: [make("vfx resolved 0", OUT), make("vfx resolved 1", OUT)],
                frame: 0,
                prev: None,
                span: Default::default(),
            });
        }
        t
    }

    /// March and composite over `target` (format, samples), soft against
    /// `depth` (its samples), with the temporal pass's viewport (full-res
    /// pixels) when given.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        viewport: Option<[f32; 4]>,
    ) {
        if !self.built {
            return;
        }
        let ms = depth_samples > 1;
        let ext = depth.texture().size();
        let size = (ext.width, ext.height);
        let div = (march_div(size.1) + self.coarser).min(6);
        let low = (size.0.div_ceil(div).max(1), size.1.div_ceil(div).max(1));
        let mut guard = self.targets(gpu, low, div);
        let t = guard.as_mut().unwrap();
        let (ldepth, out, tiles, ntiles) = (t.depth.clone(), t.out.clone(), t.tiles.clone(), t.ntiles);
        encoder.clear_buffer(&tiles, 0, None);
        let d = div as f32;
        let vp = viewport.map(|[x, y, w, h]| [x / d, y / d, w / d, h / d]).unwrap_or([
            0.0,
            0.0,
            size.0 as f32 / d,
            size.1 as f32 / d,
        ]);
        let tail: [[f32; 4]; 2] = [vp, [low.0 as f32, low.1 as f32, size.0 as f32, size.1 as f32]];
        gpu.queue.write_buffer(
            &self.uniform,
            std::mem::offset_of!(VolUniform, viewport) as u64,
            bytemuck::cast_slice(&tail),
        );
        // The temporal resolve's last view (none after a cut: the eye
        // jumped more than 25 m) and this frame's index (the march's
        // turning start offset).
        let view = self.view.expect("built");
        let temporal = self.look.temporal > 0.0;
        let prev = t.prev.filter(|(_, eye, _)| temporal && eye.distance(view.eye) < 25.0);
        // The history keeps h^span over a frame of `span` ticks
        // (exactly h at a steady 60 Hz), the same time constant at every rate.
        let span = if temporal { t.span.span(crate::render::enh::tick_views::frame_at()) } else { 1.0 };
        let tu: (Mat4, [f32; 4], [f32; 4], [f32; 4]) = (
            prev.map_or(Mat4::IDENTITY, |p| p.0),
            prev.map_or([0.0; 4], |p| (p.1 - view.eye).extend(1.0).to_array()),
            prev.map_or([0.0; 4], |p| p.2),
            [crate::render::enh::tick_views::history_keep(self.look.temporal.clamp(0.0, 0.95), span), (t.frame % 1024) as f32, if temporal { 1.0 } else { 0.0 }, 0.0],
        );
        let mut bytes = Vec::with_capacity(112);
        bytes.extend_from_slice(bytemuck::bytes_of(&tu.0.to_cols_array_2d()));
        bytes.extend_from_slice(bytemuck::cast_slice(&[tu.1, tu.2, tu.3]));
        gpu.queue.write_buffer(&self.uniform, std::mem::offset_of!(VolUniform, prev_vp) as u64, &bytes);
        let (resolved, history) =
            (t.resolved[(t.frame % 2) as usize].clone(), t.resolved[((t.frame + 1) % 2) as usize].clone());
        if temporal {
            t.prev = Some((view.view_proj, view.eye, vp));
            t.frame = t.frame.wrapping_add(1);
        }
        drop(guard);
        gpu.queue.write_buffer(
            &self.uniform,
            std::mem::offset_of!(VolUniform, depth) as u64 + 12,
            bytemuck::bytes_of(&d),
        );
        use wgpu::BindingResource as R;
        let full_group = |lo: &wgpu::TextureView, smoke: &wgpu::TextureView| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vfx full"),
                layout: &self.full_layouts[usize::from(ms)],
                entries: &[
                    entry(0, self.uniform.as_entire_binding()),
                    entry(1, R::TextureView(depth)),
                    entry(2, R::TextureView(lo)),
                    entry(3, R::TextureView(smoke)),
                    entry(4, R::Sampler(&self.sampler)),
                    entry(5, self.bounds.as_entire_binding()),
                    entry(6, tiles.as_entire_binding()),
                ],
            })
        };
        fn pass<'a>(
            encoder: &'a mut wgpu::CommandEncoder,
            label: &str,
            view: &wgpu::TextureView,
            clear: Option<wgpu::Color>,
        ) -> wgpu::RenderPass<'a> {
            let load = clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear);
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
        }
        // (What ran since the build, e.g. the native particles, apart.)
        crate::prof::gpu::mark(encoder, "graphics: volume fx (passes before the draw)");
        // 0. The scene around the medium (block averages of the
        // target as drawn so far) for its ambient; a target the passes
        // cannot read keeps the flat ambient.
        let readable = self.look.scene > 0.0 && target.texture().usage().contains(wgpu::TextureUsages::TEXTURE_BINDING);
        gpu.queue.write_buffer(
            &self.uniform,
            std::mem::offset_of!(VolUniform, look5) as u64 + 12,
            bytemuck::bytes_of(&if readable { 1.0f32 } else { 0.0 }),
        );
        if readable {
            let tms = target.texture().sample_count() > 1;
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vfx ambient"),
                layout: &self.ambient_layouts[usize::from(tms)],
                entries: &[entry(0, R::TextureView(target)), entry(1, self.ambient.as_entire_binding())],
            });
            let mut p = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("vfx ambient"),
                timestamp_writes: None,
            });
            p.set_bind_group(0, &group, &[]);
            p.set_pipeline(&self.ambient_passes[usize::from(tms)][0]);
            p.dispatch_workgroups(AMBIENT_W, AMBIENT_H, 1);
            p.set_pipeline(&self.ambient_passes[usize::from(tms)][1]);
            p.dispatch_workgroups(1, 1, 1);
        }
        // 1. The march, and the nearest view depth at the reduced size.
        {
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vfx march"),
                layout: &self.march_layouts[usize::from(ms)],
                entries: &[
                    entry(0, self.uniform.as_entire_binding()),
                    entry(1, R::TextureView(&self.dens)),
                    entry(2, R::TextureView(&self.light)),
                    entry(3, R::Sampler(&self.sampler)),
                    entry(4, R::TextureView(&self.noise)),
                    entry(5, R::Sampler(&self.noise_sampler)),
                    entry(6, self.bricks.as_entire_binding()),
                    entry(7, R::TextureView(depth)),
                    entry(8, self.bounds.as_entire_binding()),
                    entry(9, tiles.as_entire_binding()),
                    entry(10, self.ambient.as_entire_binding()),
                    entry(11, R::TextureView(&self.light_dir)),
                ],
            });
            let attach = |view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })
            };
            let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vfx march"),
                color_attachments: &[attach(&out), attach(&ldepth)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            p.set_pipeline(&self.march[usize::from(ms)]);
            p.set_bind_group(0, &group, &[]);
            p.draw(0..6, 0..1);
        }
        crate::prof::gpu::mark(encoder, "graphics: volume fx march");
        // 1b. The temporal resolve (with the history), the composite's
        // input and the next frame's history.
        let smoke = if temporal {
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vfx temporal"),
                layout: &self.temporal_layout,
                entries: &[
                    entry(0, self.uniform.as_entire_binding()),
                    entry(1, R::TextureView(&out)),
                    entry(2, R::TextureView(&ldepth)),
                    entry(3, R::TextureView(&history)),
                    entry(4, R::Sampler(&self.sampler)),
                    entry(5, self.bounds.as_entire_binding()),
                ],
            });
            let mut p = pass(encoder, "vfx temporal", &resolved, Some(wgpu::Color::TRANSPARENT));
            p.set_pipeline(&self.temporal);
            p.set_bind_group(0, &group, &[]);
            p.draw(0..6, 0..1);
            drop(p);
            crate::prof::gpu::mark(encoder, "graphics: volume fx temporal");
            resolved
        } else {
            out
        };
        // 2. Upsampled over the scene.
        {
            let group = full_group(&ldepth, &smoke);
            let mut p = pass(encoder, "vfx composite", target, None);
            p.set_pipeline(&self.composite_pipeline(gpu, format, samples, ms));
            p.set_bind_group(0, &group, &[]);
            p.draw(0..6, 0..ntiles);
        }
        crate::prof::gpu::mark(encoder, "graphics: volume fx composite");
    }
}
