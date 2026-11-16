use super::*;

pub(super) struct GpuSmoke2 {
    pub(super) params: Params,
    pub(super) pool: wgpu::Buffer,
    births: wgpu::Buffer,
    ticks: wgpu::Buffer,
    descs: wgpu::Buffer,
    colors: wgpu::Buffer,
    frame: wgpu::Buffer,
    full: wgpu::Buffer,
    color_count: usize,
    layers: std::collections::HashMap<u32, u32>,
    /// Each layer's mean opacity (the native sprite's "mass").
    mass: Vec<f32>,
    /// Each layer's opacity-weighted mean colour (the volumetric smoke's albedo).
    tint: Vec<[f32; 3]>,
    compute_group: wgpu::BindGroup,
    spawn: wgpu::ComputePipeline,
    update: wgpu::ComputePipeline,
    puff_layout: wgpu::BindGroupLayout,
    puff_pipeline: wgpu::RenderPipeline,
    full_layouts: [wgpu::BindGroupLayout; 2],
    full_modules: [wgpu::ShaderModule; 2],
    pipelines: std::sync::Mutex<std::collections::HashMap<(u8, wgpu::TextureFormat, u32, bool), wgpu::RenderPipeline>>,
    targets: std::sync::Mutex<Option<Targets>>,
    texture: wgpu::TextureView,
    noise: wgpu::TextureView,
    sampler: wgpu::Sampler,
    noise_sampler: wgpu::Sampler,
    head: u32,
    pub(super) written: u32,
    near_far: (f32, f32, bool),
    /// The volumetric renderer (None: the billboards,
    /// `NFSU2_SMOKE2_G10=1`, kept for comparisons).
    pub(super) volume: Option<crate::render::smoke_volume::SmokeVolume>,
    pub(super) frame_view: Option<crate::render::volume_fx::View>,
}

/// `NFSU2_SMOKE2_G10=1`: the billboard look instead of the volume
/// (comparisons only).
fn billboards() -> bool {
    static B: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *B.get_or_init(|| crate::render::debug_env::nfsu2_smoke2_g10())
}

impl GpuSmoke2 {
    pub(super) fn new(
        gpu: &Gpu,
        params: Params,
        library: &Library,
        textures: &std::collections::HashMap<u32, crate::formats::texture::Texture>,
    ) -> GpuSmoke2 {
        let device = &gpu.device;
        let buffer = |label, size: u64, usage| {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false },
            )
        };
        use wgpu::BufferUsages as U;
        let pool = buffer("smoke2 pool", params.pool as u64 * PUFF_SIZE, U::STORAGE | U::COPY_DST | U::COPY_SRC);
        gpu.queue.write_buffer(&pool, 0, &vec![0u8; (params.pool as u64 * PUFF_SIZE) as usize]);
        let births = buffer("smoke2 births", MAX_BIRTHS as u64 * 48, U::STORAGE | U::COPY_DST);
        let ticks = buffer("smoke2 ticks", MAX_TICKS as u64 * TICK_STRIDE, U::UNIFORM | U::COPY_DST);
        let descs = buffer("smoke2 descriptions", MAX_DESCS as u64 * DESC_SIZE, U::STORAGE | U::COPY_DST);
        let colors = buffer("smoke2 colours", MAX_COLORS as u64 * COLOR_SIZE, U::STORAGE | U::COPY_DST);
        let frame = buffer("smoke2 frame", std::mem::size_of::<FrameUniform>() as u64, U::UNIFORM | U::COPY_DST);
        let full = buffer("smoke2 full", std::mem::size_of::<FullUniform>() as u64, U::UNIFORM | U::COPY_DST);

        // The smoke descriptions' textures, one array layer each.
        let mut layers = std::collections::HashMap::new();
        let mut mass = Vec::new();
        let mut tint: Vec<[f32; 3]> = Vec::new();
        let mut data = Vec::new();
        for d in library.descriptions.iter().filter(|d| is_smoke(d)) {
            if layers.contains_key(&d.texture) {
                continue;
            }
            if let Some(t) = textures.get(&d.texture) {
                layers.insert(d.texture, layers.len() as u32);
                let px = resample(&t.decode_rgba(), t.width, t.height);
                mass.push(px.chunks(4).map(|p| p[3] as f32 / 255.0).sum::<f32>() / (TEX * TEX) as f32);
                // The opacity-weighted mean colour (the volume's albedo).
                let (mut c, mut w) = ([0.0f32; 3], 0.0f32);
                for p in px.chunks(4) {
                    let a = p[3] as f32 / 255.0;
                    for k in 0..3 {
                        c[k] += p[k] as f32 / 255.0 * a;
                    }
                    w += a;
                }
                tint.push(if w > 0.0 { c.map(|x| x / w) } else { [0.5; 3] });
                data.extend(px);
            }
        }
        if data.is_empty() {
            mass.push(0.25);
            tint.push([0.5; 3]);
            for y in 0..TEX {
                for x in 0..TEX {
                    let (u, v) = ((x as f32 + 0.5) / TEX as f32 * 2.0 - 1.0, (y as f32 + 0.5) / TEX as f32 * 2.0 - 1.0);
                    let a = ((1.0 - (u * u + v * v)).max(0.0) * 255.0) as u8;
                    data.extend([128, 128, 128, a]);
                }
            }
        }
        let count = (data.len() / (TEX * TEX * 4) as usize) as u32;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("smoke2 textures"),
            size: wgpu::Extent3d { width: TEX, height: TEX, depth_or_array_layers: count },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(TEX * 4), rows_per_image: Some(TEX) },
            wgpu::Extent3d { width: TEX, height: TEX, depth_or_array_layers: count },
        );
        let texture = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let n = NOISE as u32;
        let ntex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("smoke2 noise"),
            size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: n },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &ntex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            noise_volume(),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 4), rows_per_image: Some(n) },
            wgpu::Extent3d { width: n, height: n, depth_or_array_layers: n },
        );
        let noise = ntex.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let repeat = wgpu::AddressMode::Repeat;
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: repeat,
            address_mode_v: repeat,
            address_mode_w: repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let (compute_src, draw_src, full_src) = sections();
        let module = |label, src: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let compute_module = module("smoke2 compute", compute_src);
        let draw_module = module("smoke2 draw", draw_src);
        let full_modules = [
            module("smoke2 full", full_src.replace("DEPTH_TEXTURE", "texture_depth_2d")),
            module("smoke2 full ms", full_src.replace("DEPTH_TEXTURE", "texture_depth_multisampled_2d")),
        ];
        let cs = wgpu::ShaderStages::COMPUTE;
        let f3 = wgpu::TextureSampleType::Float { filterable: true };
        let compute_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("smoke2 compute"),
            entries: &[
                storage(0, false, cs),
                storage(1, true, cs),
                storage(2, true, cs),
                uniform(3, true, cs),
                tex_entry(4, cs, f3, wgpu::TextureViewDimension::D3, false),
                samp_entry(5, cs),
            ],
        });
        let compute_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smoke2 compute"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: pool.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: births.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: descs.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &ticks,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<TickUniform>() as u64),
                    }),
                },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&noise) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&noise_sampler) },
            ],
        });
        let cpl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("smoke2 compute"),
            bind_group_layouts: &[Some(&compute_layout)],
            immediate_size: 0,
        });
        let compute = |entry| {
            crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&cpl),
                module: &compute_module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: gpu.disk_pipeline_cache(),
            })
        };
        let spawn = compute("cs_spawn");
        let update = compute("cs_update");
        let vf = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let fs = wgpu::ShaderStages::FRAGMENT;
        let nf = wgpu::TextureSampleType::Float { filterable: false };
        let puff_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("smoke2 puffs"),
            entries: &[
                storage(0, true, vf),
                storage(1, true, vf),
                storage(2, true, vf),
                uniform(3, false, vf),
                tex_entry(4, fs, nf, wgpu::TextureViewDimension::D2, false),
                tex_entry(5, fs, f3, wgpu::TextureViewDimension::D2Array, false),
                samp_entry(6, fs),
                tex_entry(7, fs, f3, wgpu::TextureViewDimension::D3, false),
                samp_entry(8, fs),
            ],
        });
        // The OIT pair: the weighted colour sum (one + one) and the
        // revealage product (zero + one − src).
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let reveal = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrc,
            operation: wgpu::BlendOperation::Add,
        };
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("smoke2 puffs"),
            bind_group_layouts: &[Some(&puff_layout)],
            immediate_size: 0,
        });
        let puff_pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("smoke2 puffs"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &draw_module,
                entry_point: Some("vs_puff"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &draw_module,
                entry_point: Some("fs_puff"),
                compilation_options: Default::default(),
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: ACCUM,
                        blend: Some(wgpu::BlendState { color: add, alpha: add }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: REVEAL,
                        blend: Some(wgpu::BlendState { color: reveal, alpha: reveal }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        let full_layout = |ms: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("smoke2 full"),
                entries: &[
                    uniform(0, false, fs),
                    tex_entry(1, fs, wgpu::TextureSampleType::Depth, wgpu::TextureViewDimension::D2, ms),
                    tex_entry(2, fs, nf, wgpu::TextureViewDimension::D2, false),
                    tex_entry(3, fs, nf, wgpu::TextureViewDimension::D2, false),
                    tex_entry(4, fs, f3, wgpu::TextureViewDimension::D2, false),
                    samp_entry(5, fs),
                ],
            })
        };
        let full_layouts = [full_layout(false), full_layout(true)];
        let volume =
            (!billboards()).then(|| crate::render::smoke_volume::SmokeVolume::new(gpu, &pool, &descs, &colors));
        GpuSmoke2 {
            params,
            pool,
            births,
            ticks,
            descs,
            colors,
            frame,
            full,
            color_count: usize::MAX,
            layers,
            mass,
            tint,
            compute_group,
            spawn,
            update,
            puff_layout,
            puff_pipeline,
            full_layouts,
            full_modules,
            pipelines: Default::default(),
            targets: Default::default(),
            texture,
            noise,
            sampler,
            noise_sampler,
            head: 0,
            written: 0,
            near_far: (0.25, 5000.0, false),
            volume,
            frame_view: None,
        }
    }

    pub(super) fn tables(&mut self, gpu: &Gpu, library: &Library, palettes: &[crate::particles::Cubic]) {
        let n = library.descriptions.len().min(MAX_DESCS);
        let count = n + palettes.len();
        if count == self.color_count {
            return;
        }
        self.color_count = count;
        let mut descs: Vec<[f32; 28]> = Vec::with_capacity(n);
        let mut colors: Vec<[f32; 16]> = Vec::with_capacity(count);
        for d in library.descriptions.iter().take(n) {
            let mut row = [0.0f32; 28];
            for r in 0..4 {
                row[r * 4..r * 4 + 4].copy_from_slice(&d.size.0[r]);
            }
            let max_px = if d.max_pixels == 0.0 { 512.0 } else { d.max_pixels.min(512.0) };
            let layer = self.layers.get(&d.texture).copied().unwrap_or(0) as f32;
            row[16..20].copy_from_slice(&[d.drag, d.gravity, max_px, layer]);
            let aspect = [[1.0, 1.0], [1.0, 0.5], [0.5, 1.0], [1.0, 1.0]][d.aspect.min(3) as usize];
            row[20..24].copy_from_slice(&[
                aspect[0],
                aspect[1],
                (1u32 << d.columns_log2.min(7)) as f32,
                (1u32 << d.rows_log2.min(7)) as f32,
            ]);
            row[24] = self.mass.get(layer as usize).copied().unwrap_or(0.25);
            row[25..28].copy_from_slice(&self.tint.get(layer as usize).copied().unwrap_or([0.5; 3]));
            descs.push(row);
            colors.push(bytemuck::cast(d.color.0));
        }
        for p in palettes.iter().take(MAX_COLORS - n) {
            colors.push(bytemuck::cast(p.0));
        }
        gpu.queue.write_buffer(&self.descs, 0, bytemuck::cast_slice(&descs));
        gpu.queue.write_buffer(&self.colors, 0, bytemuck::cast_slice(&colors));
    }

    pub(super) fn simulate(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        ticks: &[Tick],
    ) {
        if ticks.is_empty() {
            return;
        }
        let mut first = ticks.len();
        let mut births = 0;
        while first > 0 && births + ticks[first - 1].births.len() <= MAX_BIRTHS {
            first -= 1;
            births += ticks[first].births.len();
        }
        let ticks = &ticks[first..];
        let p = self.params;
        let slots = p.billows + 1;
        let mut all: Vec<Birth> = Vec::with_capacity(births);
        let mut uniforms = vec![0u8; ticks.len() * TICK_STRIDE as usize];
        let mut plan = Vec::with_capacity(ticks.len());
        for (i, tick) in ticks.iter().enumerate() {
            // Each tick's own time and smoothed rain.
            let (t, wet) = (tick.time as f32, tick.wet);
            let offset = all.len() as u32;
            let count = (tick.births.len() as u32).min(p.pool / slots);
            all.extend_from_slice(&tick.births[..count as usize]);
            let mut cars = [[0.0f32; 4]; 16];
            let mut n = 0;
            for (ci, c) in tick.cars.iter().enumerate().take(MAX_CARS) {
                if let Some((at, v)) = c {
                    cars[ci * 2] = at.extend(1.0).to_array();
                    cars[ci * 2 + 1] = v.extend(0.0).to_array();
                    n = ci + 1;
                }
            }
            let u = TickUniform {
                dt: [tick.dt, t, wet, 0.0],
                counts: [offset, count, self.head, p.pool],
                tier: [p.billows as f32, p.size, p.linger, p.alpha],
                wind: crate::render::gpu_particles::WIND.extend(p.turbulence).to_array(),
                haze: [p.haze_size, p.haze_life, p.haze_alpha, p.haze_growth],
                ncars: [n as u32, (p.growth * 1000.0).round() as u32, 0, 0],
                cars,
            };
            let at = i * TICK_STRIDE as usize;
            uniforms[at..at + std::mem::size_of::<TickUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
            let spawned = count * slots;
            self.head = (self.head + spawned) % p.pool;
            self.written = (self.written + spawned).min(p.pool);
            plan.push((spawned, self.written));
        }
        if !all.is_empty() {
            gpu.queue.write_buffer(&self.births, 0, bytemuck::cast_slice(&all));
        }
        gpu.queue.write_buffer(&self.ticks, 0, &uniforms);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("smoke2 simulate"),
            timestamp_writes: None,
        });
        for (i, &(spawned, written)) in plan.iter().enumerate() {
            pass.set_bind_group(0, &self.compute_group, &[(i as u64 * TICK_STRIDE) as u32]);
            if spawned > 0 {
                pass.set_pipeline(&self.spawn);
                pass.dispatch_workgroups(spawned.div_ceil(64), 1, 1);
            }
            if written > 0 {
                pass.set_pipeline(&self.update);
                pass.dispatch_workgroups(written.div_ceil(64), 1, 1);
            }
        }
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: smoke cinematic simulate");
    }

    pub(super) fn frame(&mut self, gpu: &Gpu, view: &View, time: f32) {
        let cam = &view.camera;
        let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
        let view_proj = cam.projection_depth(view.aspect, view.reversed) * rot;
        let eye = cam.position;
        let (fwd, right) = (cam.forward(), cam.forward().cross(cam.up).normalize_or(Vec3::X));
        let up = right.cross(fwd);
        self.near_far = (cam.near, cam.far, view.reversed);
        self.frame_view = Some(crate::render::volume_fx::View {
            eye,
            forward: fwd,
            view_proj,
            near: cam.near,
            far: cam.far,
            reversed: view.reversed,
            time,
            fog: view.fog,
        });
        let mut u = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            eye: eye.extend(view.pixels_per_metre).to_array(),
            right: right.extend(cam.near).to_array(),
            up: up.extend(cam.far).to_array(),
            forward: fwd.extend(if view.reversed { 1.0 } else { 0.0 }).to_array(),
            counts: [0, 0, 0, u32::from(view.modern)],
            params: [time, 1.0, 1.0, self.params.linger],
            sizes: [0.0; 4],
            // Ambient floor (the underside), steam's whiteness.
            params2: [0.0, 0.5, 0.8, debug_mode()],
            lamps: [[0.0; 4]; 2 * MAX_LAMPS],
            spots: [[0.0; 4]; 3 * MAX_SPOTS],
            points: [[0.0; 4]; 2 * MAX_POINTS],
        };
        let mut n = 0;
        for (p, r, c) in view.lamps.iter().take(self.params.lamps.min(MAX_LAMPS)) {
            u.lamps[n * 2] = (*p - eye).extend(*r).to_array();
            u.lamps[n * 2 + 1] = c.extend(0.0).to_array();
            n += 1;
        }
        u.counts[0] = n as u32;
        let mut cars: Vec<&(Mat4, [u8; 3])> = view.cars.iter().collect();
        cars.sort_by(|a, b| a.0.w_axis.truncate().distance(eye).total_cmp(&b.0.w_axis.truncate().distance(eye)));
        let (mut s, mut q) = (0, 0);
        let tail = [Vec3::new(-2.1, 0.6, 0.85), Vec3::new(-2.1, -0.6, 0.85)];
        for (m, colour) in cars {
            let at = m.w_axis.truncate();
            if at.distance(eye) > 80.0 {
                continue;
            }
            if s < MAX_SPOTS {
                let forward = m.x_axis.truncate().normalize_or(Vec3::X);
                let up = m.z_axis.truncate().normalize_or(Vec3::Z);
                let head = at + forward * 2.0 + up * 0.7;
                let dir = (forward - Vec3::Z * 0.1).normalize_or(forward);
                let c = colour.map(|b| (f32::from(b) / 128.0).min(1.0));
                u.spots[s * 3] = (head - eye).extend(28.0).to_array();
                u.spots[s * 3 + 1] = dir.extend(0.82).to_array();
                u.spots[s * 3 + 2] = [c[0], c[1], c[2], 0.94];
                s += 1;
            }
            for t in tail {
                if q < MAX_POINTS {
                    u.points[q * 2] =
                        (m.transform_point3(t) - eye).extend(crate::render::gpu_particles::TAIL_RADIUS).to_array();
                    u.points[q * 2 + 1] = crate::render::gpu_particles::TAIL_COLOUR.extend(0.0).to_array();
                    q += 1;
                }
            }
        }
        u.counts[1] = s as u32;
        u.counts[2] = q as u32;
        // The sizes are set per draw (`draw`); the rest here.
        gpu.queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&u));
    }

    /// 0: downsample, 1: composite.
    fn full_pipeline(
        &self,
        gpu: &Gpu,
        which: u8,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        let key = (which, format, samples, ms);
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(p) = pipelines.get(&key) {
            return p.clone();
        }
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("smoke2 full"),
            bind_group_layouts: &[Some(&self.full_layouts[usize::from(ms)])],
            immediate_size: 0,
        });
        let module = &self.full_modules[usize::from(ms)];
        // The composite: premultiplied over; the alpha (the bleach mask) as
        // the native alpha blend writes it (as High / Ultra).
        let blend = (which == 1).then_some(wgpu::BlendState {
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
            label: Some(if which == 0 { "smoke2 downsample" } else { "smoke2 composite" }),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_full"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(if which == 0 { "fs_downsample" } else { "fs_composite" }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        pipelines.insert(key, p.clone());
        p
    }

    pub(super) fn downsample_pipeline(&self, gpu: &Gpu, ms: bool) -> wgpu::RenderPipeline {
        self.full_pipeline(gpu, 0, LDEPTH, 1, ms)
    }

    pub(super) fn composite_pipeline(
        &self,
        gpu: &Gpu,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        self.full_pipeline(gpu, 1, format, samples, ms)
    }

    fn targets(
        &self,
        gpu: &Gpu,
        size: (u32, u32),
    ) -> (wgpu::TextureView, wgpu::TextureView, wgpu::TextureView, u32, (u32, u32)) {
        let div = lowres_div(size.1);
        let mut t = self.targets.lock().unwrap();
        if t.as_ref().is_none_or(|t| t.size != size || t.div != div) {
            let low = lowres_size(size, div);
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
            *t = Some(Targets {
                size,
                div,
                depth: make("smoke2 depth", LDEPTH),
                accum: make("smoke2 accum", ACCUM),
                reveal: make("smoke2 reveal", REVEAL),
            });
        }
        let t = t.as_ref().unwrap();
        (t.depth.clone(), t.accum.clone(), t.reveal.clone(), div, lowres_size(size, div))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
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
        if self.written == 0 {
            return;
        }
        // The volumetric look (volume_fx) instead of the billboards.
        if let Some(v) = self.volume.as_ref() {
            v.draw(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
            return;
        }
        let ms = depth_samples > 1;
        let ext = depth.texture().size();
        let size = (ext.width, ext.height);
        let (ldepth, accum, reveal, div, low) = self.targets(gpu, size);
        let (near, far, reversed) = self.near_far;
        let full = FullUniform {
            depth: [near, far, if reversed { 1.0 } else { 0.0 }, div as f32],
            sizes: [low.0 as f32, low.1 as f32, size.0 as f32, size.1 as f32],
            debug: [debug_mode(), 0.0, 0.0, 0.0],
        };
        gpu.queue.write_buffer(&self.full, 0, bytemuck::bytes_of(&full));
        // The frame uniform's sizes (offset of `sizes` in FrameUniform).
        let sizes = Vec4::new(low.0 as f32, low.1 as f32, size.0 as f32, size.1 as f32);
        gpu.queue.write_buffer(
            &self.frame,
            std::mem::offset_of!(FrameUniform, sizes) as u64,
            bytemuck::bytes_of(&sizes.to_array()),
        );
        let full_group = |lo: &wgpu::TextureView, a: &wgpu::TextureView, r: &wgpu::TextureView| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("smoke2 full"),
                layout: &self.full_layouts[usize::from(ms)],
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.full.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(lo) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(a) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(r) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            })
        };
        fn attach(view: &wgpu::TextureView, clear: wgpu::Color) -> Option<wgpu::RenderPassColorAttachment<'_>> {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
            })
        }
        // 1. The nearest view depth at the reduced size. (The downsample's
        // bind group reads accum / reveal too; they are not attachments
        // in that pass. A dummy-free layout: bind the low-res depth only
        // where it is not the attachment, so bind accum in its place.)
        {
            let group = full_group(&accum, &accum, &reveal);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("smoke2 downsample"),
                color_attachments: &[attach(&ldepth, wgpu::Color::BLACK)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.downsample_pipeline(gpu, ms));
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        // 2. The puffs into the OIT pair.
        {
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("smoke2 puffs"),
                layout: &self.puff_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.pool.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.descs.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.colors.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: self.frame.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&ldepth) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.texture) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.noise) },
                    wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::Sampler(&self.noise_sampler) },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("smoke2 puffs"),
                color_attachments: &[
                    attach(&accum, wgpu::Color::TRANSPARENT),
                    attach(&reveal, wgpu::Color { r: 1.0, g: 0.0, b: 0.0, a: 0.0 }),
                ],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some([x, y, w, h]) = viewport {
                let d = div as f32;
                pass.set_viewport(x / d, y / d, w / d, h / d, 0.0, 1.0);
            }
            pass.set_pipeline(&self.puff_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..6, 0..self.written);
        }
        // 3. Upsampled over the scene.
        {
            let group = full_group(&ldepth, &accum, &reveal);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("smoke2 composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.composite_pipeline(gpu, format, samples, ms));
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        crate::prof::gpu::mark(encoder, "graphics: smoke cinematic draw");
    }
}
