use super::*;

/// The GPU half.
pub(super) struct GpuSmoke {
    pub(super) params: TierParams,
    pub(super) pool: wgpu::Buffer,
    births: wgpu::Buffer,
    ticks: wgpu::Buffer,
    descs: wgpu::Buffer,
    colors: wgpu::Buffer,
    frame: wgpu::Buffer,
    color_count: usize,
    layers: std::collections::HashMap<u32, u32>,
    compute_layout: wgpu::BindGroupLayout,
    compute_group: wgpu::BindGroup,
    spawn: wgpu::ComputePipeline,
    update: wgpu::ComputePipeline,
    draw_layouts: [wgpu::BindGroupLayout; 2],
    shader: wgpu::ShaderModule,
    shader_ms: wgpu::ShaderModule,
    pipelines: std::sync::Mutex<std::collections::HashMap<(wgpu::TextureFormat, u32, bool), wgpu::RenderPipeline>>,
    /// The half-size draw, when the device can composite it.
    pub(super) half: Option<HalfSize>,
    /// The frame's near, far and reversed depth (for the composite).
    frame_depth: [f32; 3],
    texture: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Next pool slot, and slots written so far (≤ pool).
    head: u32,
    written: u32,
}

const DESC_SIZE: u64 = 96;
const COLOR_SIZE: u64 = 64;
const MAX_DESCS: usize = 512;
const MAX_COLORS: usize = 1024;
/// The smoke textures' common size in the array.
const TEX: u32 = 128;

fn storage(binding: u32, read_only: bool, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(super) fn uniform(binding: u32, dynamic: bool, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Bilinear resample of an RGBA8 image to TEX × TEX.
fn resample(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (TEX * TEX * 4) as usize];
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return out;
    }
    let px = |x: i32, y: i32, c: usize| {
        rgba[((y.clamp(0, h as i32 - 1) as u32 * w + x.clamp(0, w as i32 - 1) as u32) * 4) as usize + c] as f32
    };
    for y in 0..TEX {
        for x in 0..TEX {
            let fx = (x as f32 + 0.5) * w as f32 / TEX as f32 - 0.5;
            let fy = (y as f32 + 0.5) * h as f32 / TEX as f32 - 0.5;
            let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
            let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
            for c in 0..4 {
                let top = px(x0, y0, c) * (1.0 - ax) + px(x0 + 1, y0, c) * ax;
                let bottom = px(x0, y0 + 1, c) * (1.0 - ax) + px(x0 + 1, y0 + 1, c) * ax;
                out[((y * TEX + x) * 4) as usize + c] =
                    (top * (1.0 - ay) + bottom * ay).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

impl GpuSmoke {
    pub(super) fn new(
        gpu: &Gpu,
        params: TierParams,
        library: &Library,
        textures: &std::collections::HashMap<u32, crate::formats::texture::Texture>,
    ) -> GpuSmoke {
        let device = &gpu.device;
        let buffer = |label, size: u64, usage| {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false },
            )
        };
        use wgpu::BufferUsages as U;
        let pool = buffer("smoke pool", params.pool as u64 * 64, U::STORAGE | U::COPY_DST | U::COPY_SRC);
        // A fresh pool is all dead (life 0).
        gpu.queue.write_buffer(&pool, 0, &vec![0u8; params.pool as usize * 64]);
        let births = buffer("smoke births", MAX_BIRTHS as u64 * 48, U::STORAGE | U::COPY_DST);
        let ticks = buffer("smoke ticks", MAX_TICKS as u64 * TICK_STRIDE, U::UNIFORM | U::COPY_DST);
        let descs = buffer("smoke descriptions", MAX_DESCS as u64 * DESC_SIZE, U::STORAGE | U::COPY_DST);
        let colors = buffer("smoke colours", MAX_COLORS as u64 * COLOR_SIZE, U::STORAGE | U::COPY_DST);
        let frame = buffer("smoke frame", std::mem::size_of::<FrameUniform>() as u64, U::UNIFORM | U::COPY_DST);

        // The smoke descriptions' textures, one array layer each.
        let mut layers = std::collections::HashMap::new();
        let mut data = Vec::new();
        for d in library.descriptions.iter().filter(|d| is_smoke(d)) {
            if layers.contains_key(&d.texture) {
                continue;
            }
            if let Some(t) = textures.get(&d.texture) {
                layers.insert(d.texture, layers.len() as u32);
                data.extend(resample(&t.decode_rgba(), t.width, t.height));
            }
        }
        if data.is_empty() {
            // No texture: a soft round puff.
            for y in 0..TEX {
                for x in 0..TEX {
                    let (u, v) = ((x as f32 + 0.5) / TEX as f32 * 2.0 - 1.0, (y as f32 + 0.5) / TEX as f32 * 2.0 - 1.0);
                    let a = ((1.0 - (u * u + v * v)).max(0.0) * 255.0) as u8;
                    data.extend([128, 128, 128, a]);
                }
            }
        }
        let count = (data.len() / (TEX * TEX * 4) as usize) as u32;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("smoke textures"),
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
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(TEX * 4), rows_per_image: Some(TEX) },
            wgpu::Extent3d { width: TEX, height: TEX, depth_or_array_layers: count },
        );
        let texture = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let source =
            crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::GpuParticles);
        // One file, two modules: the common part with the compute half,
        // and with the draw half.
        let (common, rest) = source.split_once("// ---- compute ----").expect("compute marker");
        let (compute_src, draw_src) = rest.split_once("// ---- draw ----").expect("draw marker");
        let module = |label, src: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let compute_module = module("gpu particles compute", format!("{common}{compute_src}"));
        let draw = format!("{common}{draw_src}");
        let shader_ms = module("gpu particles ms", draw.replace("texture_depth_2d", "texture_depth_multisampled_2d"));
        let shader = module("gpu particles", draw);
        let cs = wgpu::ShaderStages::COMPUTE;
        let compute_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("smoke compute"),
            entries: &[storage(0, false, cs), storage(1, true, cs), storage(2, true, cs), uniform(3, true, cs)],
        });
        let compute_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smoke compute"),
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
            ],
        });
        let cpl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("smoke compute"),
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
        let draw_layout = |multisampled: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("smoke draw"),
                entries: &[
                    storage(0, true, vf),
                    storage(1, true, vf),
                    storage(2, true, vf),
                    uniform(3, false, vf),
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            })
        };
        let draw_layouts = [draw_layout(false), draw_layout(true)];
        GpuSmoke {
            params,
            pool,
            births,
            ticks,
            descs,
            colors,
            frame,
            color_count: usize::MAX,
            layers,
            compute_layout,
            compute_group,
            spawn,
            update,
            draw_layouts,
            shader,
            shader_ms,
            pipelines: Default::default(),
            half: HalfSize::new(gpu),
            frame_depth: [0.1, 1000.0, 0.0],
            texture,
            sampler,
            head: 0,
            written: 0,
        }
    }

    /// The description and colour tables (again when the colour slots grew).
    pub(super) fn tables(&mut self, gpu: &Gpu, library: &Library, palettes: &[crate::particles::Cubic]) {
        let n = library.descriptions.len().min(MAX_DESCS);
        let count = n + palettes.len();
        if count == self.color_count {
            return;
        }
        self.color_count = count;
        let mut descs: Vec<[f32; 24]> = Vec::with_capacity(n);
        let mut colors: Vec<[f32; 16]> = Vec::with_capacity(count);
        for d in library.descriptions.iter().take(n) {
            let mut row = [0.0f32; 24];
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
            descs.push(row);
            colors.push(bytemuck::cast(d.color.0));
        }
        for p in palettes.iter().take(MAX_COLORS - n) {
            colors.push(bytemuck::cast(p.0));
        }
        gpu.queue.write_buffer(&self.descs, 0, bytemuck::cast_slice(&descs));
        gpu.queue.write_buffer(&self.colors, 0, bytemuck::cast_slice(&colors));
    }

    /// Replay `ticks`: per tick spawn its births' puffs, then move every
    /// puff by its dt.
    pub(super) fn simulate(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, ticks: &[Tick]) {
        if ticks.is_empty() {
            return;
        }
        // Keep the newest ticks whose births fit.
        let mut first = ticks.len();
        let mut births = 0;
        while first > 0 && births + ticks[first - 1].births.len() <= MAX_BIRTHS {
            first -= 1;
            births += ticks[first].births.len();
        }
        let ticks = &ticks[first..];
        let mut all = Vec::with_capacity(births);
        let mut uniforms = vec![0u8; ticks.len() * TICK_STRIDE as usize];
        let p = self.params;
        let mut plan = Vec::with_capacity(ticks.len());
        for (i, tick) in ticks.iter().enumerate() {
            // Each tick's own time (was summed back from the
            // frame's, rounding by how many ticks the frame replayed).
            let t = tick.time as f32;
            let offset = all.len() as u32;
            let count = (tick.births.len() as u32).min(p.pool / p.children);
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
                dt: [tick.dt, t, 0.0, 0.0],
                counts: [offset, count, self.head, p.pool],
                tier: [p.children as f32, p.size, p.linger, p.alpha],
                wind: WIND.extend(TURBULENCE).to_array(),
                ncars: [n as u32, 0, 0, 0],
                cars,
            };
            let at = i * TICK_STRIDE as usize;
            uniforms[at..at + std::mem::size_of::<TickUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
            let spawned = count * p.children;
            self.head = (self.head + spawned) % p.pool;
            self.written = (self.written + spawned).min(p.pool);
            plan.push((spawned, self.written));
        }
        if !all.is_empty() {
            gpu.queue.write_buffer(&self.births, 0, bytemuck::cast_slice(&all));
        }
        gpu.queue.write_buffer(&self.ticks, 0, &uniforms);
        let mut pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("smoke simulate"), timestamp_writes: None });
        for (i, &(spawned, written)) in plan.iter().enumerate() {
            let offset = [(i as u64 * TICK_STRIDE) as u32];
            pass.set_bind_group(0, &self.compute_group, &offset);
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
        let _ = &self.compute_layout;
        crate::prof::gpu::mark(encoder, "graphics: particles simulate");
    }

    /// The frame's view and lights.
    pub(super) fn frame(&mut self, gpu: &Gpu, view: &View, time: f32) {
        let cam = &view.camera;
        let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
        let view_proj = cam.projection_depth(view.aspect, view.reversed) * rot;
        let eye = cam.position;
        let (fwd, right) = (cam.forward(), cam.forward().cross(cam.up).normalize_or(Vec3::X));
        let up = right.cross(fwd);
        self.frame_depth = [cam.near, cam.far, if view.reversed { 1.0 } else { 0.0 }];
        let mut u = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            eye: eye.extend(view.pixels_per_metre).to_array(),
            right: right.extend(cam.near).to_array(),
            up: up.extend(cam.far).to_array(),
            forward: fwd.extend(if view.reversed { 1.0 } else { 0.0 }).to_array(),
            counts: [0, 0, 0, self.params.pool],
            params: [time, self.params.detail, 1.0, self.params.linger],
            lamps: [[0.0; 4]; 2 * MAX_LAMPS],
            spots: [[0.0; 4]; 3 * MAX_SPOTS],
            points: [[0.0; 4]; 2 * MAX_POINTS],
            depth_map: [1.0, 1.0, 0.0, 0.0],
        };
        let mut n = 0;
        for (p, r, c) in view.lamps.iter().take(self.params.lamps.min(MAX_LAMPS)) {
            u.lamps[n * 2] = (*p - eye).extend(*r).to_array();
            u.lamps[n * 2 + 1] = c.extend(0.0).to_array();
            n += 1;
        }
        u.counts[0] = n as u32;
        // The cars nearest the eye: headlights (spots) and tail lights.
        let mut cars: Vec<&(Mat4, [u8; 3])> = view.cars.iter().collect();
        cars.sort_by(|a, b| a.0.w_axis.truncate().distance(eye).total_cmp(&b.0.w_axis.truncate().distance(eye)));
        let (mut s, mut q) = (0, 0);
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
                u.spots[s * 3] = (head - eye).extend(HEAD_RANGE).to_array();
                u.spots[s * 3 + 1] = dir.extend(HEAD_COS[0]).to_array();
                u.spots[s * 3 + 2] = [c[0], c[1], c[2], HEAD_COS[1]];
                s += 1;
            }
            for t in TAIL_AT {
                if q < MAX_POINTS {
                    u.points[q * 2] = (m.transform_point3(t) - eye).extend(TAIL_RADIUS).to_array();
                    u.points[q * 2 + 1] = TAIL_COLOUR.extend(0.0).to_array();
                    q += 1;
                }
            }
        }
        u.counts[1] = s as u32;
        u.counts[2] = q as u32;
        gpu.queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&u));
    }

    /// The puffs' pipeline into `format` (`samples`), soft against a
    /// depth of `depth_samples`: built on first use, or ahead by
    /// [`Detail::warm`].
    pub(super) fn pipeline(
        &self,
        gpu: &Gpu,
        format: wgpu::TextureFormat,
        samples: u32,
        depth_samples: u32,
    ) -> wgpu::RenderPipeline {
        let ms = depth_samples > 1;
        // samples 0: the half-size draw (`fs_half` into the two HALF_FORMAT targets).
        let key = (format, samples, ms);
        let half = samples == 0;
        let mut pipelines = self.pipelines.lock().unwrap();
        if !pipelines.contains_key(&key) {
            let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("smoke draw"),
                bind_group_layouts: &[Some(&self.draw_layouts[usize::from(ms)])],
                immediate_size: 0,
            });
            let module = if ms { &self.shader_ms } else { &self.shader };
            // Premultiplied colour over; the alpha (the bleach mask) as the
            // native alpha blend writes it (SRCALPHA / INVSRCALPHA on both).
            let blend = wgpu::BlendState {
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
            };
            // Half size: target 0 the colour over (rgb) and the
            // transmittance (alpha: · (1 − a)), target 1 the native alpha
            // blend's sum in r (a · a + (1 − a) · r, `fs_half` writing
            // (a · a, 0, 0, a)).
            let over = wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            };
            let keep = wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            };
            let half_targets = [
                Some(wgpu::ColorTargetState {
                    format: HALF_FORMAT,
                    blend: Some(wgpu::BlendState { color: over, alpha: keep }),
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: HALF_FORMAT,
                    blend: Some(wgpu::BlendState { color: over, alpha: keep }),
                    write_mask: wgpu::ColorWrites::RED,
                }),
            ];
            let full_target =
                [Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })];
            let pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some("smoke draw"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(if half { "fs_half" } else { "fs_main" }),
                    compilation_options: Default::default(),
                    targets: if half { &half_targets } else { &full_target },
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState { count: samples.max(1), ..Default::default() },
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            });
            pipelines.insert(key, pipeline);
        }
        pipelines[&key].clone()
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
        size: (u32, u32),
        viewport: Option<[f32; 4]>,
    ) {
        if self.written == 0 {
            return;
        }
        let ms = depth_samples > 1;
        let [vx, vy, vw, vh] = viewport.unwrap_or([0.0, 0.0, size.0 as f32, size.1 as f32]);
        let half = self.half.as_ref().filter(|_| half_size_on());
        let hsize = (((vw * 0.5).ceil() as u32).max(1), ((vh * 0.5).ceil() as u32).max(1));
        let map = match half {
            Some(_) => [vw / hsize.0 as f32, vh / hsize.1 as f32, vx, vy],
            None => [1.0, 1.0, 0.0, 0.0],
        };
        gpu.queue.write_buffer(
            &self.frame,
            std::mem::offset_of!(FrameUniform, depth_map) as u64,
            bytemuck::bytes_of(&map),
        );
        let pipeline = self.pipeline(gpu, format, if half.is_some() { 0 } else { samples }, depth_samples);
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smoke draw"),
            layout: &self.draw_layouts[usize::from(ms)],
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.pool.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.descs.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.colors.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.frame.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.texture) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        if let Some(h) = half {
            let (colour, alpha) = h.targets(gpu, hsize);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("smoke (half size)"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &colour,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &alpha,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..6, 0..self.written);
            drop(pass);
            h.composite(
                gpu,
                encoder,
                target,
                format,
                samples,
                depth,
                ms,
                (&colour, &alpha),
                hsize,
                map,
                viewport,
                &self.frame_depth,
            );
            crate::prof::gpu::mark(encoder, "graphics: particles draw");
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("smoke"),
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
        if let Some([x, y, w, h]) = viewport {
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
        }
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..6, 0..self.written);
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: particles draw");
    }
}
