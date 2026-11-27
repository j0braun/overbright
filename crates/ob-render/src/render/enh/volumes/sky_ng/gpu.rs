use super::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Uniform {
    inv_view_proj: [[f32; 4]; 4],
    view_rot_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    fog_value: [f32; 4],
    fog_color: [f32; 4],
    p0: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
    main_rows: [[f32; 4]; PAIRS * ROWS * 3],
    cap_rows: [[f32; 4]; PAIRS * ROWS * 3],
}

/// What one frame's sky needs from the renderer.
pub struct SkyParams {
    /// The main view's projection x its view's rotation only (no translation: the rays are unprojected
    /// about the origin, exact in f32 far from the world's origin).
    pub view_proj: Mat4,
    pub eye: glam::Vec3,
    pub fog_value: [f32; 3],
    pub fog_color: [f32; 3],
    pub time: f32,
    /// The pair drawn and, while fading, the pair faded from and SKYMORPHALPHA.
    pub set: usize,
    pub morph: Option<(usize, f32)>,
}

/// The sky at one tick, as its panorama rows are drawn from it
/// ("history" class: the panorama is filled a quarter of its rows a tick,
/// the tick's phase `tick % 4`, each from its own tick's eye, clock, rain
/// and sky pair; a frame commits the ticks since the last one, so the
/// panorama at a tick is the same at every display rate).
#[derive(Clone, Copy, Debug)]
pub struct SkyTick {
    pub tick: u64,
    pub eye: glam::Vec3,
    pub time: f32,
    pub rain: f32,
    pub set: usize,
    pub morph: Option<(usize, f32)>,
}

/// The panorama's per-tick uniforms (one frame commits at most this many
/// ticks: the 3 of a slow frame, or a full fill).
const PANO_SLOTS: usize = 4;

/// One slot's stride in the panorama's uniform buffer.
const PANO_STRIDE: u64 = (std::mem::size_of::<Uniform>() as u64).div_ceil(256) * 256;

type PipelineKey = (wgpu::TextureFormat, u32, bool);

/// The shader's hemisphere: 48 x 16 quads, two triangles each.
const DOME_VERTICES: u32 = 48 * 16 * 6;
/// The panorama (azimuth × sqrt(elevation)).
pub const PANORAMA: (u32, u32) = (1024, 256);
/// The city glow's gain (inferred, by eye).
pub const GLOW: f32 = 0.9;

/// Design choice: the Next-Gen sky is a night sky.
/// Its weight this frame from the game's own sky state ([`crate::sky`]: the
/// table's pairs 2 and 6 are `SKY_NIGHT_A`, 0 sunrise and 4 sunset; while the
/// dome morphs (0x5CF687, SKYMORPHALPHA = t / 20 s) the weight follows the
/// same fade from the pair faded from to the pair drawn). 0: the native
/// painted dome alone (sunrise / sunset), 1: the Next-Gen sky (night).
pub fn night_weight(set: usize, morph: Option<(usize, f32)>) -> f32 {
    let night = |pair: usize| if pair % 4 == 2 { 1.0 } else { 0.0 };
    match morph {
        Some((from, alpha)) => {
            let a = alpha.clamp(0.0, 1.0);
            night(from) * (1.0 - a) + night(set) * a
        }
        None => night(set),
    }
}

/// The flattened rows of a [`Palette`] (made at every world load: < 5 ms, so the row can be switched on in game).
pub type Rows = ([[f32; 4]; PAIRS * ROWS * 3], [[f32; 4]; PAIRS * ROWS * 3]);

pub fn rows_of(palette: &Palette) -> Rows {
    let flat = |s: &[Stats; PAIRS]| {
        let mut out = [[0f32; 4]; PAIRS * ROWS * 3];
        for p in 0..PAIRS {
            for r in 0..ROWS {
                for k in 0..3 {
                    out[p * ROWS * 3 + r * 3 + k] = s[p][r][k];
                }
            }
        }
        out
    };
    (flat(&palette.main), flat(&palette.cap))
}

pub(super) fn source(group: u32, pano_group: u32, reflection: bool) -> String {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/sky_ng.wgsl"))
        .replace("SKYGROUP", &group.to_string())
        .replace("PANOGROUP", &pano_group.to_string());
    if reflection {
        return src;
    }
    match (src.find("// REFL BEGIN"), src.find("// REFL END")) {
        (Some(a), Some(b)) => format!("{}{}", &src[..a], &src[b..]),
        _ => src,
    }
}

/// The sky's GPU state, made when the row is on.
pub struct NightSky {
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    compute: wgpu::ComputePipeline,
    compute_groups: (wgpu::BindGroup, wgpu::BindGroup),
    /// The main view's pipelines by target (the combination matrix:
    /// one slot was replaced while the render workers still drew the last
    /// frame in the old format — a validation panic when High Precision Post
    /// changed live; now each pass takes the one for its own target).
    pipeline: Mutex<Vec<(PipelineKey, wgpu::RenderPipeline)>>,
    refl_pipeline: wgpu::RenderPipeline,
    rows: Rows,
    /// The panorama's per-tick uniforms ([`PANO_SLOTS`] slots) and their
    /// groups; the frame's dispatches (slot, row phase: 4 every row).
    pano_buffer: wgpu::Buffer,
    pano_groups: Vec<wgpu::BindGroup>,
    pano_plan: Mutex<Vec<(usize, u32)>>,
    /// The panorama holds every row (since the sky was last visible).
    filled: std::sync::atomic::AtomicBool,
    /// The frame's uniform as prepared (the ticks' rows patch it).
    frame_uniform: Mutex<Option<Uniform>>,
    /// [`night_weight`] of the frame prepared (f32 bits): at 0 nothing is drawn.
    weight: AtomicU32,
}

impl NightSky {
    /// The sky over the world's palette rows; `frame_layout` the world views' group 0 (the reflection views').
    pub fn new(
        gpu: &Gpu,
        rows: &Rows,
        frame_layout: &wgpu::BindGroupLayout,
        reflection_format: wgpu::TextureFormat,
    ) -> NightSky {
        let device = &gpu.device;
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("sky next-gen"),
            source: wgpu::ShaderSource::Wgsl(source(0, 1, false).into()),
        });
        let refl_shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("sky next-gen (reflections)"),
            source: wgpu::ShaderSource::Wgsl(source(1, 2, true).into()),
        });
        use wgpu::ShaderStages as S;
        let uniform_e = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: S::VERTEX_FRAGMENT | S::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bgl = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("sky next-gen"),
            entries: &[
                uniform_e,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: S::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: S::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let cbgl0 = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("sky panorama uniform"),
            entries: &[uniform_e],
        });
        let cbgl1 = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("sky panorama"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: S::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        });
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("sky next-gen"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let refl_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("sky next-gen (reflections)"),
            bind_group_layouts: &[Some(frame_layout), Some(&bgl)],
            immediate_size: 0,
        });
        let clayout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("sky panorama"),
            bind_group_layouts: &[Some(&cbgl0), Some(&cbgl1)],
            immediate_size: 0,
        });
        let compute = crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
            label: Some("sky panorama"),
            layout: Some(&clayout),
            module: &shader,
            entry_point: Some("cs_panorama"),
            compilation_options: Default::default(),
            cache: gpu.disk_pipeline_cache(),
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky next-gen"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pano = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sky panorama"),
            size: wgpu::Extent3d { width: PANORAMA.0, height: PANORAMA.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let pano_view = pano.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky panorama"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky next-gen"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&pano_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let pano_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky panorama ticks"),
            size: PANO_STRIDE * PANO_SLOTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pano_groups = (0..PANO_SLOTS)
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("sky panorama tick"),
                    layout: &cbgl0,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &pano_buffer,
                            offset: i as u64 * PANO_STRIDE,
                            size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                        }),
                    }],
                })
            })
            .collect();
        let compute_groups = (
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sky panorama uniform"),
                layout: &cbgl0,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sky panorama"),
                layout: &cbgl1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&pano_view),
                }],
            }),
        );
        // The reflection views: their own format, single sample, native depth (cleared to 1, compared less-equal).
        let refl_pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("sky next-gen (reflections)"),
            layout: Some(&refl_layout),
            vertex: wgpu::VertexState {
                module: &refl_shader,
                entry_point: Some("vs_refl"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &refl_shader,
                entry_point: Some("fs_refl"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: reflection_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::render::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        log::info!("sky next-gen: on, palette from the sky table, panorama {}x{}", PANORAMA.0, PANORAMA.1);
        NightSky {
            shader,
            layout,
            buffer,
            group,
            compute,
            compute_groups,
            pipeline: Mutex::new(Vec::new()),
            refl_pipeline,
            rows: *rows,
            pano_buffer,
            pano_groups,
            pano_plan: Mutex::new(Vec::new()),
            filled: std::sync::atomic::AtomicBool::new(false),
            frame_uniform: Mutex::new(None),
            weight: AtomicU32::new(0),
        }
    }

    /// Whether the frame prepared shows the Next-Gen sky at all ([`night_weight`] > 0).
    fn visible(&self) -> bool {
        f32::from_bits(self.weight.load(Ordering::Relaxed)) > 0.0
    }

    /// The main view's pipeline for a target of `format` × `samples`,
    /// made where missing ([`NightSky::prepare`], a drive's warm-up).
    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, reversed: bool) {
        let key = (format, samples, reversed);
        let mut slot = self.pipeline.lock().unwrap();
        if !slot.iter().any(|(k, _)| *k == key) {
            let pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some("sky next-gen"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        // The colour over the dome's by the night weight's veil (at night 1: it
                        // replaces the dome's, as before); its alpha (the bleach bypass mask) stays.
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::Zero,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                // Only where the depth buffer holds the dome (6334 m and farther) or nothing: the world's pixels fail.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::render::DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(if reversed {
                        wgpu::CompareFunction::Greater
                    } else {
                        wgpu::CompareFunction::Less
                    }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            });
            slot.push((key, pipeline));
            if slot.len() > 4 {
                slot.remove(0);
            }
        }
    }

    /// This frame's uniforms and the pipeline for the main view's target (`format`, `samples`).
    pub fn prepare(&self, gpu: &Gpu, p: &SkyParams, format: wgpu::TextureFormat, samples: u32, reversed: bool) {
        self.warm(gpu, format, samples, reversed);
        let (from, alpha) = p.morph.unwrap_or((p.set, 1.0));
        let weight = night_weight(p.set, p.morph);
        self.weight.store(weight.to_bits(), Ordering::Relaxed);
        let u = Uniform {
            inv_view_proj: p.view_proj.inverse().to_cols_array_2d(),
            view_rot_proj: p.view_proj.to_cols_array_2d(),
            eye: p.eye.extend(1.0).to_array(),
            fog_value: [p.fog_value[0], p.fog_value[1], p.fog_value[2], 0.0],
            fog_color: [p.fog_color[0], p.fog_color[1], p.fog_color[2], 1.0],
            p0: [
                p.time + crate::render::debug_env::nfsu2_night_sky_time().unwrap_or(0.0),
                rain(),
                (p.set / 2).min(PAIRS - 1) as f32,
                (from / 2).min(PAIRS - 1) as f32,
            ],
            p1: [
                alpha,
                crate::render::debug_env::nfsu2_night_sky_cover().unwrap_or(0.0).clamp(-0.4, 0.5),
                if crate::render::debug_env::nfsu2_night_sky_debug().unwrap_or(0.0) > 0.5 { 1.0 } else { 0.0 },
                1.0,
            ],
            // The panorama's row phase: set per tick (`plan_rows`).
            p2: [
                4.0,
                crate::render::debug_env::nfsu2_sky_glow().unwrap_or(GLOW),
                weight,
                0.0,
            ],
            main_rows: self.rows.0,
            cap_rows: self.rows.1,
        };
        gpu.queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&u));
        *self.frame_uniform.lock().unwrap() = Some(u);
    }

    /// The panorama's rows of the ticks since the last frame
    /// (`ticks`, oldest first; after [`NightSky::prepare`]): each tick its
    /// quarter (phase `tick % 4`) from its own eye, clock, rain and sky
    /// pair; the first frame the sky is visible again every row, from the
    /// newest tick (the frame's values when no tick came). Not visible: no
    /// rows.
    pub fn plan_rows(&self, gpu: &Gpu, ticks: &[SkyTick]) {
        let mut plan = self.pano_plan.lock().unwrap();
        plan.clear();
        if !self.visible() {
            self.filled.store(false, Ordering::Relaxed);
            return;
        }
        let Some(base) = *self.frame_uniform.lock().unwrap() else { return };
        let patch = |k: &SkyTick, phase: u32| {
            let mut u = base;
            let (from, alpha) = k.morph.unwrap_or((k.set, 1.0));
            u.eye = k.eye.extend(1.0).to_array();
            u.p0 = [
                k.time + crate::render::debug_env::nfsu2_night_sky_time().unwrap_or(0.0),
                crate::render::debug_env::nfsu2_night_sky_rain().unwrap_or(k.rain),
                (k.set / 2).min(PAIRS - 1) as f32,
                (from / 2).min(PAIRS - 1) as f32,
            ];
            u.p1[0] = alpha;
            u.p2[0] = phase as f32;
            u.p2[2] = night_weight(k.set, k.morph);
            u
        };
        let mut list: Vec<(Uniform, u32)> = Vec::new();
        if !self.filled.load(Ordering::Relaxed) {
            list.push(ticks.last().map_or((base, 4), |k| (patch(k, 4), 4)));
            self.filled.store(true, Ordering::Relaxed);
        } else {
            for k in ticks.iter().rev().take(PANO_SLOTS).rev() {
                let phase = (k.tick % 4) as u32;
                list.push((patch(k, phase), phase));
            }
        }
        for (i, (u, phase)) in list.iter().enumerate() {
            gpu.queue.write_buffer(&self.pano_buffer, i as u64 * PANO_STRIDE, bytemuck::bytes_of(u));
            plan.push((i, *phase));
        }
    }

    /// The panorama's rows planned for this frame ([`NightSky::plan_rows`]),
    /// before the views that sample it.
    pub fn update(&self, encoder: &mut wgpu::CommandEncoder) {
        let plan = std::mem::take(&mut *self.pano_plan.lock().unwrap());
        if plan.is_empty() || !self.visible() {
            return;
        }
        let mut pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sky panorama"), timestamp_writes: None });
        pass.set_pipeline(&self.compute);
        pass.set_bind_group(1, &self.compute_groups.1, &[]);
        for (slot, phase) in plan {
            // Every row, or a quarter (the rows of the tick's phase, p2.x:
            // cs_panorama maps the invocations onto them).
            let rows = if phase >= 4 { PANORAMA.1 } else { PANORAMA.1 / 4 };
            pass.set_bind_group(0, &self.pano_groups[slot], &[]);
            pass.dispatch_workgroups(PANORAMA.0.div_ceil(8), rows.div_ceil(8), 1);
        }
        drop(pass);
        crate::prof::gpu::mark(encoder, "world: sky panorama");
    }

    /// The sky over the dome's pixels, in the main view's pass (its own group 0; the caller rebinds its own).
    /// `mode`: the pass's scene mode (its target is the float format with
    /// High Precision Post, else the scene's own), as [`NightSky::prepare`] had it.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, mode: crate::render::SceneMode) -> bool {
        if !self.visible() {
            return false;
        }
        let found = self
            .pipeline
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|((f, n, r), _)| {
                (*f == crate::render::HDR_FORMAT) == mode.hdr && *n == mode.samples && *r == mode.reversed
            })
            .map(|(_, p)| p.clone());
        let Some(pipeline) = found else { return false };
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..DOME_VERTICES, 0..1);
        true
    }

    /// The sky where a reflection view drew nothing (`frame`: its group 0, already bound), last in its pass.
    pub fn draw_reflection(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.visible() {
            return;
        }
        pass.set_pipeline(&self.refl_pipeline);
        pass.set_bind_group(1, &self.group, &[]);
        pass.draw(0..DOME_VERTICES, 0..1);
    }
}
