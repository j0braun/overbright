use super::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Uniform {
    clip_to_rel: [[f32; 4]; 4],
    rel_to_prev: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    prev_forward: [f32; 4],
    dims: [f32; 4],
    depth: [f32; 4],
    medium: [f32; 4],
    ambient: [f32; 4],
    glow: [f32; 4],
    counts: [f32; 4],
    look: [f32; 4],
    smoke: [f32; 4],
    smoke_dims: [f32; 4],
    size: [f32; 4],
    wind: [f32; 4],
    shape: [f32; 4],
    tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuLight {
    position: [f32; 4],
    colour: [f32; 4],
    axis: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuSpot {
    eye: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    dir: [f32; 4],
    colour: [f32; 4],
}

/// The noise volume's edge (texels, tiling).
const NOISE: u32 = 32;

fn noise_bytes() -> Vec<u8> {
    // SplitMix64 (Steele, Lea & Flood 2014): one random byte a texel; the trilinear sampler makes value noise.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..NOISE * NOISE * NOISE)
        .map(|_| {
            x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 56) as u8
        })
        .collect()
}

fn volume(gpu: &Gpu, label: &str, dims: [u32; 3], format: wgpu::TextureFormat, storage: bool) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: dims[0], height: dims[1], depth_or_array_layers: dims[2] },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | if storage { wgpu::TextureUsages::STORAGE_BINDING } else { wgpu::TextureUsages::COPY_DST },
        view_formats: &[],
    })
}

const CELL: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub struct Atmosphere {
    inject_layout: wgpu::BindGroupLayout,
    integrate_layout: wgpu::BindGroupLayout,
    apply_layouts: [wgpu::BindGroupLayout; 2],
    inject: wgpu::ComputePipeline,
    integrate: wgpu::ComputePipeline,
    /// One volume, the energy bound: the veil floor's pass, its histogram and its 1 × 1 result.
    floor_layout: wgpu::BindGroupLayout,
    floor: wgpu::ComputePipeline,
    veil_bins: wgpu::Buffer,
    veil: wgpu::TextureView,
    apply_modules: [wgpu::ShaderModule; 2],
    apply: std::collections::HashMap<(wgpu::TextureFormat, bool, bool), wgpu::RenderPipeline>,
    uniform: wgpu::Buffer,
    lights: wgpu::Buffer,
    spots: wgpu::Buffer,
    views: wgpu::Buffer,
    /// Ping-pong cells (this frame's, the last frame's) and the integrated volume.
    cells: [wgpu::TextureView; 2],
    integrated: wgpu::TextureView,
    noise: wgpu::TextureView,
    placeholder_maps: wgpu::TextureView,
    placeholder_smoke: wgpu::TextureView,
    shadow_samp: wgpu::Sampler,
    rep: wgpu::Sampler,
    clamp: wgpu::Sampler,
    frame: u32,
    /// The last frame's projection × rotation and eye (the history's view).
    prev: Option<(Mat4, Vec3)>,
    /// The drawn frames' place on the tick axis (`render::enh::tick_views`).
    span: crate::render::enh::tick_views::FrameSpan,
    /// The grid the cells are made for (the tier's, [`grid`]).
    dims: [u32; 3],
}

impl Atmosphere {
    pub fn new(gpu: &Gpu) -> Atmosphere {
        let device = &gpu.device;
        use wgpu::ShaderStages as S;
        let cs = S::COMPUTE;
        let fs = S::FRAGMENT;
        let uniform_e = |b, v| wgpu::BindGroupLayoutEntry {
            binding: b,
            visibility: v,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage_e = |b| wgpu::BindGroupLayoutEntry {
            binding: b,
            visibility: cs,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let tex_e = |b, v, dim, sample_type| wgpu::BindGroupLayoutEntry {
            binding: b,
            visibility: v,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: dim, multisampled: false },
            count: None,
        };
        let samp_e = |b, v, ty| wgpu::BindGroupLayoutEntry {
            binding: b,
            visibility: v,
            ty: wgpu::BindingType::Sampler(ty),
            count: None,
        };
        let store_e = |b| wgpu::BindGroupLayoutEntry {
            binding: b,
            visibility: cs,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: CELL,
                view_dimension: wgpu::TextureViewDimension::D3,
            },
            count: None,
        };
        let f3 = wgpu::TextureViewDimension::D3;
        let filt = wgpu::TextureSampleType::Float { filterable: true };
        let inject_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere inject"),
            entries: &[
                uniform_e(0, cs),
                storage_e(1),
                storage_e(2),
                storage_e(3),
                tex_e(4, cs, wgpu::TextureViewDimension::D2Array, wgpu::TextureSampleType::Depth),
                samp_e(5, cs, wgpu::SamplerBindingType::Comparison),
                tex_e(6, cs, f3, filt),
                samp_e(7, cs, wgpu::SamplerBindingType::Filtering),
                tex_e(8, cs, f3, filt),
                samp_e(9, cs, wgpu::SamplerBindingType::Filtering),
                store_e(10),
            ],
        });
        let veil_bins_e = wgpu::BindGroupLayoutEntry {
            binding: 16,
            visibility: cs,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let integrate_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere integrate"),
            entries: &[
                uniform_e(0, cs),
                samp_e(9, cs, wgpu::SamplerBindingType::Filtering),
                tex_e(11, cs, f3, filt),
                tex_e(12, cs, f3, filt),
                store_e(13),
                veil_bins_e,
            ],
        });
        let floor_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere veil floor"),
            entries: &[
                uniform_e(0, cs),
                veil_bins_e,
                wgpu::BindGroupLayoutEntry {
                    binding: 17,
                    visibility: cs,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let apply_layout = |ms: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("atmosphere apply"),
                entries: &[
                    uniform_e(0, fs),
                    samp_e(9, fs, wgpu::SamplerBindingType::Filtering),
                    wgpu::BindGroupLayoutEntry {
                        binding: 14,
                        visibility: fs,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: ms,
                        },
                        count: None,
                    },
                    tex_e(15, fs, f3, filt),
                    tex_e(
                        18,
                        fs,
                        wgpu::TextureViewDimension::D2,
                        wgpu::TextureSampleType::Float { filterable: false },
                    ),
                ],
            })
        };
        let apply_layouts = [apply_layout(false), apply_layout(true)];
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/atmosphere.wgsl"));
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("atmosphere"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let compute = |layout: &wgpu::BindGroupLayout, entry: &str| {
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("atmosphere"),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: gpu.disk_pipeline_cache(),
            })
        };
        let inject = compute(&inject_layout, "cs_inject");
        let integrate = compute(&integrate_layout, "cs_integrate");
        let floor = compute(&floor_layout, "cs_floor");
        let apply_modules = [false, true].map(|ms| {
            let src = if ms { ms_source(source) } else { source.to_string() };
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("atmosphere apply"),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        });
        let buf = |label, size: usize, usage| {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor {
                    label: Some(label),
                    size: size as u64,
                    usage: usage | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            )
        };
        let uniform = buf("atmosphere", std::mem::size_of::<Uniform>(), wgpu::BufferUsages::UNIFORM);
        let lights =
            buf("atmosphere lights", MAX_LIGHTS * std::mem::size_of::<GpuLight>(), wgpu::BufferUsages::STORAGE);
        let spots = buf("atmosphere spots", MAX_SPOTS * std::mem::size_of::<GpuSpot>(), wgpu::BufferUsages::STORAGE);
        let views = buf(
            "atmosphere shadow views",
            MAX_LAYERS * std::mem::size_of::<crate::render::lamps::GpuShadowView>(),
            wgpu::BufferUsages::STORAGE,
        );
        let veil_bins = buf(
            "atmosphere veil histogram",
            (VEIL_BINS + VEIL_TILES[0] * VEIL_TILES[1]) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        // The tiles' minima start empty (all bits set: cs_integrate's atomicMin).
        gpu.queue.write_buffer(&veil_bins, (VEIL_BINS * 4) as u64, &vec![0xFF; VEIL_TILES[0] * VEIL_TILES[1] * 4]);
        let veil = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("atmosphere veil floor"),
                // Texel (0, 0) the frame's floor, (1 + x, y) the tiles' (atmosphere.wgsl `cs_floor`).
                size: wgpu::Extent3d {
                    width: 1 + VEIL_TILES[0] as u32,
                    height: VEIL_TILES[1] as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let dims = grid(&TIER_NEXT_GEN);
        let (cells, integrated) = cell_volumes(gpu, dims);
        let noise_tex = volume(gpu, "atmosphere noise", [NOISE; 3], wgpu::TextureFormat::R8Unorm, false);
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &noise_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &noise_bytes(),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(NOISE), rows_per_image: Some(NOISE) },
            wgpu::Extent3d { width: NOISE, height: NOISE, depth_or_array_layers: NOISE },
        );
        let noise = noise_tex.create_view(&Default::default());
        let placeholder_maps = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("atmosphere no shadows"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
        let placeholder_smoke = volume(gpu, "atmosphere no smoke", [1, 1, 1], wgpu::TextureFormat::Rgba16Float, false)
            .create_view(&Default::default());
        let sampler = |label, mode, compare| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: mode,
                address_mode_v: mode,
                address_mode_w: mode,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare,
                ..Default::default()
            })
        };
        let shadow_samp =
            sampler("atmosphere shadows", wgpu::AddressMode::ClampToEdge, Some(wgpu::CompareFunction::LessEqual));
        let rep = sampler("atmosphere noise", wgpu::AddressMode::Repeat, None);
        let clamp = sampler("atmosphere clamp", wgpu::AddressMode::ClampToEdge, None);
        Atmosphere {
            inject_layout,
            integrate_layout,
            apply_layouts,
            inject,
            integrate,
            floor_layout,
            floor,
            veil_bins,
            veil,
            apply_modules,
            apply: Default::default(),
            uniform,
            lights,
            spots,
            views,
            cells,
            integrated,
            noise,
            placeholder_maps,
            placeholder_smoke,
            shadow_samp,
            rep,
            clamp,
            frame: 0,
            prev: None,
            span: Default::default(),
            dims,
        }
    }

    /// The pipeline a [`Atmosphere::run`] into `format` against a depth
    /// that is multisampled when `ms` uses, made now (a warm-up's copy).
    pub fn warm(mut self, gpu: &Gpu, format: wgpu::TextureFormat, ms: bool) {
        self.apply_pipeline(gpu, format, ms, false);
        self.apply_pipeline(gpu, format, ms, true);
    }

    /// `tinted`: the rain's tint folded in (the scene × the blend constant).
    fn apply_pipeline(&mut self, gpu: &Gpu, format: wgpu::TextureFormat, ms: bool, tinted: bool) -> wgpu::RenderPipeline {
        if let Some(p) = self.apply.get(&(format, ms, tinted)) {
            return p.clone();
        }
        let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("atmosphere apply"),
            bind_group_layouts: &[Some(&self.apply_layouts[usize::from(ms)])],
            immediate_size: 0,
        });
        let module = &self.apply_modules[usize::from(ms)];
        // scene × T + S; the alpha (the bleach mask) kept.
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: if tinted { wgpu::BlendFactor::Constant } else { wgpu::BlendFactor::SrcAlpha },
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let p = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("atmosphere apply"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(if tinted { "fs_apply_tint" } else { "fs_apply" }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        self.apply.insert((format, ms, tinted), p.clone());
        p
    }

    /// The medium over `scene` (`size`, `format`) for `frame`'s view.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        depth: &wgpu::TextureView,
        samples: u32,
        camera: &Camera,
        aspect: f32,
        reversed: bool,
        next: &NextGen,
        tint: Option<[f32; 3]>,
    ) -> bool {
        let l = look();
        // The rain's tint folds in only where the medium's alpha is 1 (no veil: `Look::veil` 0).
        let tint = tint.filter(|_| l.veil == 0.0);
        let tier = next.tier;
        let dims = grid(&tier);
        if dims != self.dims {
            // A tier switch: the tier's grid, no history (the cells were another grid's).
            (self.cells, self.integrated) = cell_volumes(gpu, dims);
            self.dims = dims;
            self.prev = None;
        }
        let far = tier.far.max(NEAR * 2.0);
        let fwd = camera.forward();
        let rot = Mat4::look_to_rh(Vec3::ZERO, fwd, camera.up);
        // The slices' view depth along the frustum's centre: a projection with NEAR / FAR.
        let proj = camera.projection_depth(aspect, reversed);
        let clip_to_rel = (proj * rot).inverse();
        let eye = camera.position;
        let (rel_to_prev, valid) = match self.prev {
            // A cut (more than 30 m in a frame) drops the history.
            Some((pr, pe)) if pe.distance(eye) < 30.0 => (pr * Mat4::from_translation(eye - pe), 1.0),
            _ => (Mat4::IDENTITY, 0.0),
        };
        // A tier without history: every frame's cells from its own view alone (the slices' centres, no blend).
        self.prev = tier.history.then_some((proj * rot, eye));
        // The new cell's weight over the frame's span in ticks
        // (exactly `blend` at a steady 60 Hz): the history's time constant
        // is the same at every display rate.
        let span = self.span.span(crate::render::enh::tick_views::frame_at());
        self.frame = self.frame.wrapping_add(1);
        let (haze, height) = medium(next.rain);
        let scatter = crate::render::enh::volumes::medium::medium(next.rain).scatter_scale();
        let car = crate::render::car_lights::look();
        let ground = eye.z - 2.0;
        let lamp_mean = {
            let lamps: Vec<&Light> = next.lights.iter().filter(|x| x.kind == LightKind::Lamp).collect();
            if lamps.is_empty() {
                Vec3::new(0.45, 0.42, 0.38)
            } else {
                lamps.iter().map(|x| x.colour).sum::<Vec3>() / lamps.len() as f32
            }
        };
        let glow = lamp_mean * l.glow * (1.0 + 0.6 * next.rain);
        let smoke = next.smoke.as_ref();
        // NFSU2_ATMO_NO_SHAFTS (a comparison aid): the lamps unshadowed.
        let shadows = next
            .shadows
            .as_ref()
            .filter(|(_, v)| tier.shafts && !v.is_empty() && !crate::env_set!("NFSU2_ATMO_NO_SHAFTS"));
        let u = Uniform {
            clip_to_rel: clip_to_rel.to_cols_array_2d(),
            rel_to_prev: rel_to_prev.to_cols_array_2d(),
            eye: eye.extend(next.time).to_array(),
            forward: fwd.extend(if reversed { 1.0 } else { 0.0 }).to_array(),
            prev_forward: [l.veil, if PLAIN_INJECT.load(std::sync::atomic::Ordering::Relaxed) { 1.0 } else { 0.0 }, l.floor, valid],
            dims: [dims[0] as f32, dims[1] as f32, dims[2] as f32, self.frame as f32],
            depth: [NEAR, far, (far / NEAR).ln(), if valid > 0.5 { jitter(self.frame) } else { 0.5 }],
            medium: [haze, height, ground, next.rain],
            ambient: (next.fog_color * l.ambient).extend(l.noise).to_array(),
            glow: glow.extend(l.glow_height).to_array(),
            counts: [
                next.lights.len().min(MAX_LIGHTS) as f32,
                next.spots.len().min(MAX_SPOTS) as f32,
                if shadows.is_some() { 1.0 } else { 0.0 },
                if smoke.is_some() { 1.0 } else { 0.0 },
            ],
            // One rain rule for every light (`Medium::scatter_scale`): the haze's extinction × thickness, the
            // light every lamp, sign and headlight scatters × √thickness (the gains here × the scale).
            look: [
                l.g,
                if valid > 0.5 { crate::render::enh::tick_views::history_weight(l.blend, span) } else { 1.0 },
                l.lamp * scatter,
                l.emitter * scatter,
            ],
            smoke: smoke.map_or([0.0; 4], |s| (s.origin - eye).extend(s.voxel).to_array()),
            smoke_dims: smoke
                .map_or([1.0, 1.0, 1.0, l.knee], |s| [s.dims[0] as f32, s.dims[1] as f32, s.dims[2] as f32, l.knee]),
            size: [size.0 as f32, size.1 as f32, 1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32],
            wind: [1.6, 0.7, l.spot * next.spot_scale * scatter, if crate::env_set!("NFSU2_ATMO_DEBUG") { 1.0 } else { 0.0 }],
            shape: [car.aureole.0, car.aureole.1, l.car_lamp * scatter, l.tile],
            tint: tint.map_or([0.0; 4], |[r, g, b]| [r, g, b, 0.0]),
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let lights: Vec<GpuLight> = next
            .lights
            .iter()
            .take(MAX_LIGHTS)
            .map(|x| GpuLight {
                position: (x.position - eye).extend(x.radius).to_array(),
                colour: x.colour.extend(if shadows.is_some() { x.layer as f32 } else { -1.0 }).to_array(),
                axis: x
                    .axis
                    .extend(match x.kind {
                        LightKind::Lamp => 0.0,
                        LightKind::Emitter => 1.0,
                        LightKind::Point => 2.0,
                    })
                    .to_array(),
            })
            .collect();
        if !lights.is_empty() {
            gpu.queue.write_buffer(&self.lights, 0, bytemuck::cast_slice(&lights));
        }
        let spots: Vec<GpuSpot> = next
            .spots
            .iter()
            .take(MAX_SPOTS)
            .map(|s| GpuSpot {
                eye: (s.eye - eye).extend(s.reach).to_array(),
                right: s.right.extend(s.across).to_array(),
                up: s.up.extend(s.down).to_array(),
                dir: s.dir.extend(if shadows.is_some() { s.layer as f32 } else { -1.0 }).to_array(),
                colour: s.colour.extend(0.0).to_array(),
            })
            .collect();
        if !spots.is_empty() {
            gpu.queue.write_buffer(&self.spots, 0, bytemuck::cast_slice(&spots));
        }
        if let Some((_, v)) = shadows {
            let n = v.len().min(MAX_LAYERS);
            gpu.queue.write_buffer(&self.views, 0, bytemuck::cast_slice(&v[..n]));
        }
        let cur = (self.frame % 2) as usize;
        let ms = samples > 1;
        let pipeline = self.apply_pipeline(gpu, format, ms, tint.is_some());
        let (cells, history) = (&self.cells[cur], &self.cells[1 - cur]);
        let device = &gpu.device;
        use wgpu::BindingResource as R;
        let e = |binding, resource| wgpu::BindGroupEntry { binding, resource };
        let maps = shadows.map_or(&self.placeholder_maps, |(m, _)| m);
        let smoke_view = smoke.map_or(&self.placeholder_smoke, |s| &s.density);
        let inject_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere inject"),
            layout: &self.inject_layout,
            entries: &[
                e(0, self.uniform.as_entire_binding()),
                e(1, self.lights.as_entire_binding()),
                e(2, self.spots.as_entire_binding()),
                e(3, self.views.as_entire_binding()),
                e(4, R::TextureView(maps)),
                e(5, R::Sampler(&self.shadow_samp)),
                e(6, R::TextureView(&self.noise)),
                e(7, R::Sampler(&self.rep)),
                e(8, R::TextureView(history)),
                e(9, R::Sampler(&self.clamp)),
                e(10, R::TextureView(cells)),
            ],
        });
        let integrate_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere integrate"),
            layout: &self.integrate_layout,
            entries: &[
                e(0, self.uniform.as_entire_binding()),
                e(9, R::Sampler(&self.clamp)),
                e(11, R::TextureView(smoke_view)),
                e(12, R::TextureView(cells)),
                e(13, R::TextureView(&self.integrated)),
                e(16, self.veil_bins.as_entire_binding()),
            ],
        });
        let floor_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere veil floor"),
            layout: &self.floor_layout,
            entries: &[
                e(0, self.uniform.as_entire_binding()),
                e(16, self.veil_bins.as_entire_binding()),
                e(17, R::TextureView(&self.veil)),
            ],
        });
        let mut pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("atmosphere"), timestamp_writes: None });
        pass.set_pipeline(&self.inject);
        pass.set_bind_group(0, &inject_group, &[]);
        pass.dispatch_workgroups(dims[0].div_ceil(8), dims[1].div_ceil(8), dims[2]);
        pass.set_pipeline(&self.integrate);
        pass.set_bind_group(0, &integrate_group, &[]);
        pass.dispatch_workgroups(dims[0].div_ceil(8), dims[1].div_ceil(8), 1);
        pass.set_pipeline(&self.floor);
        pass.set_bind_group(0, &floor_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
        drop(pass);
        let apply_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere apply"),
            layout: &self.apply_layouts[usize::from(ms)],
            entries: &[
                e(0, self.uniform.as_entire_binding()),
                e(9, R::Sampler(&self.clamp)),
                e(14, R::TextureView(depth)),
                e(15, R::TextureView(&self.integrated)),
                e(18, R::TextureView(&self.veil)),
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("atmosphere apply"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: scene,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &apply_group, &[]);
        if let Some([r, g, b]) = tint {
            pass.set_blend_constant(wgpu::Color { r: f64::from(r), g: f64::from(g), b: f64::from(b), a: 1.0 });
        }
        pass.draw(0..3, 0..1);
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: atmosphere");
        tint.is_some()
    }
}

static PLAIN_INJECT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `cs_inject` with every light tested in every cell (true), as before the workgroup's light cull: the
/// same cells (the cull leaves out only lights that reach none of the workgroup's cells, and the cells walk
/// the rest in the same order). For the test that compares the two.
pub fn set_plain_inject(on: bool) {
    PLAIN_INJECT.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// A tier's grid (`NFSU2_ATMO_DIMS=x,y,z` overrides every tier's, tuning only).
fn grid(tier: &Tier) -> [u32; 3] {
    static D: std::sync::OnceLock<Option<[u32; 3]>> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        let v: Vec<u32> =
            crate::render::debug_env::nfsu2_atmo_dims().split(',').filter_map(|x| x.trim().parse().ok()).collect();
        (v.len() == 3 && v.iter().all(|&x| (8..=512).contains(&x))).then(|| [v[0], v[1], v[2]])
    })
    .unwrap_or(tier.dims)
}

/// The ping-pong cells and the integrated volume for a grid.
fn cell_volumes(gpu: &Gpu, dims: [u32; 3]) -> ([wgpu::TextureView; 2], wgpu::TextureView) {
    let cells = [0, 1].map(|_| volume(gpu, "atmosphere cells", dims, CELL, true).create_view(&Default::default()));
    let integrated = volume(gpu, "atmosphere integrated", dims, CELL, true).create_view(&Default::default());
    (cells, integrated)
}

/// The apply pass over a multisampled depth (sample 0).
pub(super) fn ms_source(source: &str) -> String {
    source.replace("var depth_tex: texture_depth_2d;", "var depth_tex: texture_depth_multisampled_2d;")
}
