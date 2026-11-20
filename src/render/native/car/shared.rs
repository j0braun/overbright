//! What a device's car renderers share: modules, layouts, pipelines (moved from car.rs).

use super::*;

/// What a device's car renderers share (one per device, [`Gpu`]):
/// car.efx's shader, its bind group and pipeline layouts and its
/// pipelines per colour format, built once rather than per car, so a car
/// coming onto the road (traffic, a rival) compiles no shader.
pub struct CarShared {
    shader: wgpu::ShaderModule,
    /// The other modules, built on first use, by [`module_index`]: Enhanced
    /// Car Paint (car.wgsl + car_paint.wgsl), Wet Car, Surface Detail
    /// (car_surface.wgsl) and the temporal pass's biased copies; the
    /// native pipelines never use them.
    modules: [std::sync::OnceLock<wgpu::ShaderModule>; CarKey::SLOTS],
    pub(crate) frame_layout: wgpu::BindGroupLayout,
    pub(crate) draw_layout: wgpu::BindGroupLayout,
    pub(crate) texture_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    pipelines: std::sync::Mutex<Vec<((wgpu::TextureFormat, CarMode), std::sync::Arc<Pipelines>)>>,
    /// LOD Fade's (clone deviation) pipelines, by format and scene mode.
    fade_pipelines: std::sync::Mutex<Vec<((wgpu::TextureFormat, CarMode), std::sync::Arc<Pipelines>)>>,
    /// The (black) cube a renderer starts with ([`start_envmap`]).
    pub(super) placeholder: std::sync::OnceLock<wgpu::TextureView>,
    /// The device's pipeline cache (`DiskPipelineCache`).
    pub(crate) cache: Option<wgpu::PipelineCache>,
    /// The device's [`crate::render::Gpu::build_tag`].
    pub(crate) tag: u64,
    /// Screen-Space Reflections (clone deviation): the mask module
    /// (car.wgsl + car_ssr.wgsl) and pipelines by Reversed-Z, built on
    /// first use; the mask this frame's world view left for the cars
    /// (armed by `WorldRenderer::plan_views`, none without SSR).
    ssr_module: [std::sync::OnceLock<wgpu::ShaderModule>; 2],
    ssr_pipelines: std::sync::Mutex<Vec<((bool, bool), std::sync::Arc<SsrPipelines>)>>,
    ssr_mask: std::sync::Mutex<Option<SsrMaskTargets>>,
    /// Surface Detail (docs/notes/graphics/surface-maps.md "Cars"): the flat 1 × 1
    /// map every texture without one binds, the maps' sampler, and the maps
    /// made so far by key (shared by every car with the texture: traffic).
    surface: std::sync::OnceLock<(wgpu::TextureView, wgpu::Sampler)>,
    pub(super) surface_maps: std::sync::Mutex<HashMap<u64, Option<wgpu::TextureView>>>,
    /// Enhanced Glass: its modules (dry, wet × Enhanced Paint),
    /// layouts, pipelines and the scene's copy, made on first use.
    pub(crate) glass: std::sync::OnceLock<glass::GlassShared>,
    /// The draw index (docs/notes/performance/car-draw-index.md): the device has immediates, so
    /// every car module is its [`immediate_form`] and each draw sets its
    /// entry ([`draw_entry`]); without them the modules read the entry from
    /// the instance index as before.
    pub(crate) immediates: bool,
}

/// The SSR mask's targets the cars draw into after the car pass
/// ([`CarShared::arm_ssr_mask`]; docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections").
#[derive(Clone)]
pub struct SsrMaskTargets {
    pub term: wgpu::TextureView,
    pub surface: wgpu::TextureView,
    pub tint: wgpu::TextureView,
    /// The mask's own depth (the road's), single-sampled.
    pub depth: wgpu::TextureView,
    pub reversed: bool,
}

/// The mask pipelines by (alpha-blended, alpha test); the additive and
/// destination-alpha draws (lights, decals) leave the mask as it is.
pub(super) type SsrPipelines = HashMap<(bool, bool), wgpu::RenderPipeline>;

impl CarShared {
    pub(crate) fn new(device: &wgpu::Device, cache: Option<wgpu::PipelineCache>, tag: u64) -> CarShared {
        // S4: every car module form built once, so a broken one panics here.
        super::shader_variants::startup_check();
        let texture_layout = car_texture_layout(device);
        // Frame group: uniforms + environment cube map.
        let frame_layout = crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
            label: Some("car frame"),
            entries: &[
                uniform_entry(0, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // Per-draw values, one 560-byte entry each, bound once: a draw
        // reads its entry by its instance index.
        let draw_layout = crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
            label: Some("car draw"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let immediates = device.features().contains(wgpu::Features::IMMEDIATES);
        let shader = crate::render::core::device_cache::uncached_module(device, tag, wgpu::ShaderModuleDescriptor {
            label: Some("car"),
            source: wgpu::ShaderSource::Wgsl(if immediates {
                built(module_source_for(CarKey::default(), true)).into()
            } else {
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).into()
            }),
        });
        let layout = crate::render::core::device_cache::uncached_pipeline_layout(device, &wgpu::PipelineLayoutDescriptor {
            label: Some("car"),
            bind_group_layouts: &[Some(&frame_layout), Some(&draw_layout), Some(&texture_layout)],
            immediate_size: draw_immediate_size(immediates),
        });
        CarShared {
            shader,
            modules: std::array::from_fn(|_| Default::default()),
            frame_layout,
            draw_layout,
            texture_layout,
            layout,
            pipelines: Default::default(),
            fade_pipelines: Default::default(),
            placeholder: Default::default(),
            cache,
            tag,
            ssr_module: Default::default(),
            ssr_pipelines: Default::default(),
            ssr_mask: Default::default(),
            surface: Default::default(),
            surface_maps: Default::default(),
            glass: Default::default(),
            immediates,
        }
    }

    /// The pipelines into `format`, built on first use.
    #[track_caller]
    pub(super) fn pipelines(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> std::sync::Arc<Pipelines> {
        self.pipelines_n(device, format, CarMode::NATIVE)
    }

    /// The module for `mode`: car.wgsl, or its forms with Enhanced Car
    /// Paint, Wet Car, Surface Detail and the temporal pass's LOD bias.
    fn module(&self, device: &wgpu::Device, mode: CarMode) -> &wgpu::ShaderModule {
        let i = module_index(mode);
        if i == 0 {
            return &self.shader;
        }
        self.modules[i].get_or_init(|| {
            let source = built(module_source_for(CarKey::of(mode), self.immediates));
            crate::render::core::device_cache::uncached_module(device, self.tag, wgpu::ShaderModuleDescriptor {
                label: Some("car (enhanced)"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        })
    }

    /// Surface Detail's flat stand-in map and the maps' sampler.
    pub(super) fn surface_defaults(&self, device: &wgpu::Device) -> &(wgpu::TextureView, wgpu::Sampler) {
        self.surface.get_or_init(|| (crate::render::surface::flat_map(device), crate::render::surface::sampler(device)))
    }

    /// The pipelines into `format` for the scene's `mode` (the
    /// anti-aliasing, and the Graphics page's Reversed-Z and
    /// alpha-to-coverage).
    #[track_caller]
    pub(super) fn pipelines_n(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        mode: CarMode,
    ) -> std::sync::Arc<Pipelines> {
        let mut all = self.pipelines.lock().unwrap();
        if let Some((_, p)) = all.iter().find(|(f, _)| *f == (format, mode)) {
            return p.clone();
        }
        crate::render::note_pipeline(self.tag);
        let p = std::sync::Arc::new(car_pipelines(
            device,
            self.module(device, mode),
            &self.layout,
            format,
            mode.0,
            self.cache.as_ref(),
            false,
        ));
        all.push(((format, mode), p.clone()));
        p
    }

    /// The SSR mask for the cars drawn from now on (None: SSR off or no
    /// game view this frame).
    pub(crate) fn arm_ssr_mask(&self, mask: Option<SsrMaskTargets>) {
        *self.ssr_mask.lock().unwrap() = mask;
    }

    pub(super) fn armed_ssr_mask(&self) -> Option<SsrMaskTargets> {
        self.ssr_mask.lock().unwrap().clone()
    }

    /// The mask pipelines for the depth convention, built on first use.
    /// `surface`: Surface Detail's car maps in force (the mask takes the
    /// mapped normal).
    pub(super) fn ssr_pipelines(&self, device: &wgpu::Device, reversed: bool, surface: bool) -> std::sync::Arc<SsrPipelines> {
        let mut all = self.ssr_pipelines.lock().unwrap();
        if let Some((_, p)) = all.iter().find(|(r, _)| *r == (reversed, surface)) {
            return p.clone();
        }
        let module = self.ssr_module[usize::from(surface)].get_or_init(|| {
            crate::render::core::device_cache::uncached_module(device, self.tag, wgpu::ShaderModuleDescriptor {
                label: Some("car (ssr mask)"),
                source: wgpu::ShaderSource::Wgsl(built(ssr_mask_source_for(surface, self.immediates)).into()),
            })
        });
        crate::render::note_pipeline(self.tag);
        let p = std::sync::Arc::new(ssr_mask_pipelines(device, module, &self.layout, reversed, self.cache.as_ref()));
        all.push(((reversed, surface), p.clone()));
        p
    }

    /// LOD Fade's pipelines into `format` for the scene's `mode`, built on
    /// first use (the first car queued with LOD Fade on).
    #[track_caller]
    pub(super) fn fade_pipelines_n(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        mode: CarMode,
    ) -> std::sync::Arc<Pipelines> {
        let mut all = self.fade_pipelines.lock().unwrap();
        if let Some((_, p)) = all.iter().find(|(f, _)| *f == (format, mode)) {
            return p.clone();
        }
        crate::render::note_pipeline(self.tag);
        let p = std::sync::Arc::new(car_pipelines(
            device,
            self.module(device, mode),
            &self.layout,
            format,
            mode.0,
            self.cache.as_ref(),
            true,
        ));
        all.push(((format, mode), p.clone()));
        p
    }
}

/// The immediates a car pipeline layout declares: the draw's entry
/// (`car_draw`, a u32) when the device has them, none otherwise.
pub(crate) fn draw_immediate_size(immediates: bool) -> u32 {
    if immediates { std::mem::size_of::<u32>() as u32 } else { 0 }
}

/// Draw `d` with the values of entry `entry`: the entry in the
/// immediates first when the device has them (`immediates`; the pipeline
/// set before), and as the instance index either way (the fallback
/// modules read it there).
#[inline]
pub(crate) fn draw_entry(pass: &mut wgpu::RenderPass<'_>, immediates: bool, d: &Draw, entry: u32) {
    if immediates {
        pass.set_immediates(0, &entry.to_le_bytes());
    }
    pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, entry..entry + 1);
}

/// The car texture group: the texture and its sampler (0, 1, car.efx's)
/// and Surface Detail's map and its sampler (2, 3; read by
/// car_surface.wgsl only).
fn car_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let texture = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let sampler = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    };
    crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
        label: Some("car texture"),
        entries: &[texture(0), sampler(1), texture(2), sampler(3)],
    })
}

pub(super) fn bind_car_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    map: &(wgpu::TextureView, wgpu::Sampler),
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&map.0) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&map.1) },
        ],
    })
}

/// The SSR mask pipelines (car_ssr.wgsl): opaque and alpha-blended draws,
/// with and without the alpha test, single-sampled, into the mask's three
/// targets, depth-tested and written against the mask's depth. The cull
/// is the car pass's (blended draws CW).
fn ssr_mask_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    reversed: bool,
    cache: Option<&wgpu::PipelineCache>,
) -> SsrPipelines {
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
    let make = |blended: bool, alpha_test: bool| {
        let constants = [("MASK_BLENDED", if blended { 1.0 } else { 0.0 })];
        let target = |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
        crate::render::core::device_cache::timed_render_pipeline(device, &wgpu::RenderPipelineDescriptor {
            label: Some("car ssr mask"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CarVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState { cull_mode: blended.then_some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(if reversed {
                    wgpu::CompareFunction::GreaterEqual
                } else {
                    wgpu::CompareFunction::LessEqual
                }),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(if alpha_test { "fs_ssr_mask_alpha_test" } else { "fs_ssr_mask" }),
                compilation_options: wgpu::PipelineCompilationOptions { constants: &constants, ..Default::default() },
                targets: &[
                    target(wgpu::TextureFormat::Rgba16Float),
                    target(wgpu::TextureFormat::Rgba16Float),
                    target(wgpu::TextureFormat::Rgba8Unorm),
                ],
            }),
            multiview_mask: None,
            cache,
        })
    };
    [(false, false), (false, true), (true, false), (true, true)]
        .into_iter()
        .map(|(b, t)| ((b, t), make(b, t)))
        .collect()
}

/// car.efx's pipelines for every blend and alpha test, drawing into
/// `color_format`.
fn car_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    mode: crate::render::SceneMode,
    cache: Option<&wgpu::PipelineCache>,
    fade: bool,
) -> HashMap<(Blend, bool), wgpu::RenderPipeline> {
    let constants = mode.fragment_constants();
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
    let make = |blend: Blend, alpha_test: bool| {
        let state = |src, dst| {
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: src, dst_factor: dst, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent::OVER,
            })
        };
        // 0x5BCC40's factors, as the world's (bytes 5 / 6
        // blend by the destination alpha, the world's bleach bypass mask).
        let color_blend =
            crate::render::native::world::pipelines::native_blend_factors(blend).and_then(|(s, d)| state(s, d));
        crate::render::core::device_cache::timed_render_pipeline(device, &wgpu::RenderPipelineDescriptor {
            label: Some("car"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CarVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            // 0x5BCD29 (the car effect, type 5): an opaque texture
            // (+0x56 = 0) draws with CULLMODE NONE and ZWRITE on, a
            // blended one with CULLMODE CW and ZWRITE off; CW is the
            // clone's back face (the side whose cull leaves the body's
            // outer skin).
            primitive: wgpu::PrimitiveState {
                cull_mode: (blend != Blend::None).then_some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(blend == Blend::None),
                depth_compare: Some(mode.compare()),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: mode.samples, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                // Alpha-to-coverage with the Graphics page's MSAA; LOD Fade
                // (clone deviation): the `_fade` forms add the screen-door
                // dither, discarded per pixel at 1 sample, with MSAA a
                // per-sample mask (`_ms`; ANDed with the coverage's). The
                // mask forms are invalid Vulkan SPIR-V as naga writes them,
                // so the 1-sample forms do not write it.
                entry_point: Some(match (alpha_test, mode.coverage, fade, mode.samples > 1) {
                    (true, true, false, _) => "fs_alpha_coverage",
                    (true, true, true, _) => "fs_alpha_coverage_fade",
                    (true, false, false, _) => "fs_alpha_test",
                    (true, false, true, false) => "fs_alpha_test_fade",
                    (true, false, true, true) => "fs_alpha_test_fade_ms",
                    (false, _, false, _) => "fs_main",
                    (false, _, true, false) => "fs_main_fade",
                    (false, _, true, true) => "fs_main_fade_ms",
                }),
                compilation_options: wgpu::PipelineCompilationOptions { constants: &constants, ..Default::default() },
                // car.efx technique "car", pass p0: ColorWriteEnable = 7 (RGB), so
                // the car leaves the back buffer's alpha (the bleach
                // bypass mask) as the world wrote it.
                targets: &[Some(wgpu::ColorTargetState {
                    format: mode.target(color_format),
                    blend: color_blend,
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache,
        })
    };
    [Blend::None, Blend::Alpha, Blend::Additive, Blend::DestAlpha, Blend::DestAlphaAdditive]
        .into_iter()
        .flat_map(|b| [false, true].map(|t| (b, t)))
        .map(|(b, t)| ((b, t), make(b, t)))
        .collect()
}

fn uniform_entry(binding: u32, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        },
        count: None,
    }
}
