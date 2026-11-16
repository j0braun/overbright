//! Screen-space reflections on the wet road (the Graphics page's
//! "Screen-Space Reflections", a **clone deviation**, off by default;
//! docs/wet-roads-ssr.md). The game has none: its road reflection is the
//! planar capture (view 4, docs/formats.md "Road reflections"), which
//! holds no cars and only the low LODs. That capture stays the base.
//!
//! The world pass leaves a mask of the road it drew
//! ([`crate::render::WorldRenderer::wet_mask`]): per pixel the reflection term it
//! added (unweighted) and its weight × fog, the surface's normal,
//! roughness and view depth. Here, after the multisampled scene is
//! resolved: the scene is copied; a depth pyramid (linear view depth, the
//! nearest per cell) is built at the trace's size (half or full); a
//! stackless hi-Z ray march per road pixel finds what the reflected view
//! meets on screen, with a confidence (screen edges, reach, rays turning
//! to the camera); a spatial filter (no history: works without TAA)
//! blurs it by the roughness, stretched along the screen's vertical; and
//! the composite swaps the world pass's planar term for the traced one by
//! that confidence, wherever the mask's surface is still the front one
//! (cars drawn after it are left alone). Misses and off-screen rays keep
//! the planar image; its bright authored highlights (flares, headlight
//! pools) are kept either way.

use crate::render::Gpu;

/// The mask the main view left ([`crate::render::WorldRenderer::wet_mask`]): its
/// targets at the scene's size (the road's and, over it, the cars'), and
/// the view's fog and reflection mode.
#[derive(Clone)]
pub struct Mask {
    /// (the reflection term as drawn, unweighted; its weight (× fog)).
    pub term: wgpu::TextureView,
    /// (world normal x, y; roughness; view depth).
    pub surface: wgpu::TextureView,
    /// (the weight per channel over `term`'s a; the kind: 0 road, 0.5 /
    /// 1 a car facing up / down).
    pub tint: wgpu::TextureView,
    /// The main view's FogValue (x, y, z) and colour; None with the fog
    /// off.
    pub fog: Option<([f32; 3], [f32; 3])>,
    /// Enhanced Reflections' true-mirror capture (else the native
    /// per-prop flip) under the road's term.
    pub enhanced: bool,
    /// Wet Roads drawing a wet road this frame (its uniform on). A dry
    /// road's traced reflection is held to the native strength.
    pub wet: bool,
    /// The mask's depth (scene size, cleared to the far value of the
    /// view's depth): the scene-sized passes run only where it was drawn.
    /// None: every pixel.
    pub depth: Option<wgpu::TextureView>,
}

/// What a frame's reflections need ([`crate::render::postfx::PostFx::set_ssr`]).
#[derive(Clone)]
pub struct Frame {
    pub mask: Mask,
    /// The scene's depth (multisampled when `samples` > 1).
    pub depth: wgpu::TextureView,
    pub samples: u32,
    /// The view's near and far planes, and whether the depth is reversed.
    pub near: f32,
    pub far: f32,
    pub reversed: bool,
    /// The projection's x and y scales (m00, m11).
    pub scale: (f32, f32),
    /// The view's right, up and forward axes in the world.
    pub basis: [glam::Vec3; 3],
    /// Traced at the screen's full size (else half).
    pub full: bool,
    /// Ray-traced reflections ([`crate::render::rt`]): the trace pass in its
    /// ray-query variant, completing the screen march's misses.
    pub rt: Option<crate::render::rt::FrameRt>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    size: [f32; 4],
    depth: [f32; 4],
    proj: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    forward: [f32; 4],
    fog: [f32; 4],
    fog_color: [f32; 4],
}

/// The pyramid's format (linear depth, metres) and the trace's.
const HIZ_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
pub(crate) const TRACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The trace-sized mask depth (fs_linear: 0 trace and filter, 0.5
/// filter only, 1 neither) and the mask's kind and depth per trace pixel
/// (the filter's taps; the mask's own f16 values).
const SKIP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth16Unorm;
const KINDS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;
/// The pyramid's levels at most.
const MAX_LEVELS: u32 = 8;
/// How far a ray looks (m), and its steps at half and full size.
const REACH: f32 = 150.0;
const STEPS_HALF: f32 = 48.0;
const STEPS_FULL: f32 = 64.0;

/// Pipelines by pass: copy, linear, down, trace, denoise, composite;
/// the copy and composite tested against the mask's depth.
enum Pass {
    Copy,
    Linear,
    Down,
    Trace,
    Denoise,
    Composite,
    CopyMasked,
    CompositeMasked,
}

pub(crate) struct Targets {
    /// The scene's size and the trace's.
    size: (u32, u32),
    trace_size: (u32, u32),
    copy: wgpu::TextureView,
    /// The copy is the pool's [`crate::render::pool::Slot::SceneCopy`] (cleared
    /// and rewritten before each read); held while these targets live.
    _copy: std::sync::Arc<crate::render::pool::Target>,
    /// The whole pyramid, and each level alone.
    hiz: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
    pub(crate) trace: wgpu::TextureView,
    filtered: wgpu::TextureView,
    /// The trace-sized mask depth and kinds ([`SKIP_FORMAT`],
    /// [`KINDS_FORMAT`]).
    pub(crate) skip: wgpu::TextureView,
    kinds: wgpu::TextureView,
    /// The ray-traced trace's ray list ([`crate::render::rt::list_path`]):
    /// made with the targets of a traced frame on a device that takes it
    /// (the trace target then a storage texture too); None: the rays stay
    /// in the trace's fragment pass.
    pub(crate) list: Option<crate::render::rt::ListBuffers>,
}

impl Targets {
    /// The trace's size.
    pub(crate) fn trace_size(&self) -> (u32, u32) {
        self.trace_size
    }
}

pub struct Ssr {
    layouts: [wgpu::BindGroupLayout; 2],
    /// [single, multisampled depth] × the passes ([`Pass`]).
    pipelines: [Vec<wgpu::RenderPipeline>; 2],
    scene_format: wgpu::TextureFormat,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    targets: Option<Targets>,
    dummy_depth: [wgpu::TextureView; 2],
    dummy: wgpu::TextureView,
    /// Group 1's layout and the ray-query trace pipelines (single,
    /// multisampled depth), made on first use.
    rt: Option<(wgpu::BindGroupLayout, [Option<wgpu::RenderPipeline>; 2])>,
    /// The ray list's programs ([`crate::render::rt::list_path`]), made on
    /// first use.
    rt_list: Option<crate::render::rt::ListTrace>,
    /// Tests: the rays kept in the trace's fragment pass (the other form,
    /// on the same device).
    #[cfg(test)]
    fragment_rays: bool,
}

fn target(gpu: &Gpu, label: &str, size: (u32, u32), format: wgpu::TextureFormat, levels: u32) -> wgpu::Texture {
    target_with(gpu, label, size, format, levels, wgpu::TextureUsages::empty())
}

fn target_with(
    gpu: &Gpu,
    label: &str,
    (w, h): (u32, u32),
    format: wgpu::TextureFormat,
    levels: u32,
    extra: wgpu::TextureUsages,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // (The tests read the targets back.)
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | extra
            | if cfg!(test) { wgpu::TextureUsages::COPY_SRC } else { wgpu::TextureUsages::empty() },
        view_formats: &[],
    })
}

/// The pyramid's levels for a trace of `size`: down to a few texels.
pub fn levels(size: (u32, u32)) -> u32 {
    (32 - size.0.max(size.1).max(1).leading_zeros()).clamp(1, MAX_LEVELS)
}

/// The trace's size for a scene of `size` shown on a screen of `out`
/// (SSAA / Render Scale draw the scene at another size): the screen's
/// (Full) or half of it, never more than the scene's.
pub fn trace_size(size: (u32, u32), out: (u32, u32), full: bool) -> (u32, u32) {
    let base = (out.0.min(size.0), out.1.min(size.1));
    if full { (base.0.max(1), base.1.max(1)) } else { ((base.0 / 2).max(1), (base.1 / 2).max(1)) }
}

impl Ssr {
    pub fn new(gpu: &Gpu, scene_format: wgpu::TextureFormat) -> Ssr {
        let device = &gpu.device;
        let tex = |binding: u32, filterable: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = |multisampled: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("ssr"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled,
                        },
                        count: None,
                    },
                    tex(1, false),
                    tex(2, true),
                    tex(3, true),
                    tex(4, false),
                    tex(5, true),
                    tex(8, false),
                    tex(9, true),
                    tex(10, false),
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        // The vertex stage too: vs_mask's depth.
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                        },
                        count: None,
                    },
                ],
            })
        };
        let layouts = [layout(false), layout(true)];
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("ssr"),
                size: std::mem::size_of::<Uniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ssr"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let dummy_depth = [1u32, 4].map(|samples| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("ssr dummy depth"),
                    size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: crate::render::DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        });
        let dummy = target(gpu, "ssr dummy", (1, 1), TRACE_FORMAT, 1).create_view(&Default::default());
        let pipelines = Self::pipelines(gpu, &layouts, scene_format);
        Ssr {
            layouts,
            pipelines,
            scene_format,
            uniform,
            sampler,
            targets: None,
            dummy_depth,
            dummy,
            rt: None,
            rt_list: None,
            #[cfg(test)]
            fragment_rays: false,
        }
    }

    /// The ray-traced trace's programs for a depth multisampled when `ms`
    /// (the path the device takes), made now: a drive's warm-up
    /// (docs/first-use-builds.md).
    pub fn warm_rt(&mut self, gpu: &Gpu, ms: bool) {
        crate::render::rt::warm_trace(gpu, &self.layouts, &mut self.rt, &mut self.rt_list, ms);
    }

    fn pipelines(
        gpu: &Gpu,
        layouts: &[wgpu::BindGroupLayout; 2],
        scene_format: wgpu::TextureFormat,
    ) -> [Vec<wgpu::RenderPipeline>; 2] {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
        [false, true].map(|ms| {
            // The multisampled variant reads sample 0 of the depth.
            let src = if ms { multisampled(source) } else { source.to_string() };
            let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("ssr"),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("ssr"),
                bind_group_layouts: &[Some(&layouts[usize::from(ms)])],
                immediate_size: 0,
            });
            let make =
                |vs: &str, entry: &str, formats: &[wgpu::TextureFormat], depth: Option<wgpu::DepthStencilState>| {
                    let targets: Vec<_> = formats
                        .iter()
                        .map(|&format| {
                            Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })
                        })
                        .collect();
                    crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                        label: Some(entry),
                        layout: Some(&pl),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some(vs),
                            compilation_options: Default::default(),
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some(entry),
                            compilation_options: Default::default(),
                            targets: &targets,
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: depth,
                        multisample: wgpu::MultisampleState::default(),
                        multiview_mask: None,
                        cache: gpu.disk_pipeline_cache(),
                    })
                };
            vec![
                make("vs_main", "fs_copy", &[scene_format], None),
                make(
                    "vs_main",
                    "fs_linear",
                    &[HIZ_FORMAT, KINDS_FORMAT],
                    Some(depth_state(SKIP_FORMAT, true, wgpu::CompareFunction::Always)),
                ),
                make("vs_main", "fs_down", &[HIZ_FORMAT], None),
                make("vs_trace", "fs_trace", &[TRACE_FORMAT], Some(skip_test())),
                make("vs_filter", "fs_denoise", &[TRACE_FORMAT], Some(skip_test())),
                make("vs_main", "fs_composite", &[scene_format], None),
                make("vs_mask", "fs_copy", &[scene_format], Some(mask_test())),
                make("vs_mask", "fs_composite", &[scene_format], Some(mask_test())),
            ]
        })
    }

    /// The targets for a scene of `size` shown at `out`; `traced`: the
    /// frame's trace is ray-traced (the ray list made where the device
    /// takes it, for the targets' life: a frame without a traced scene
    /// keeps them).
    fn ensure_targets(&mut self, gpu: &Gpu, size: (u32, u32), out: (u32, u32), full: bool, traced: bool) {
        let trace_size = trace_size(size, out, full);
        let list = traced && crate::render::rt::list_path(gpu) && crate::render::rt::list_fits(gpu, trace_size);
        #[cfg(test)]
        let list = list && !self.fragment_rays;
        if self
            .targets
            .as_ref()
            .is_some_and(|t| t.size == size && t.trace_size == trace_size && (t.list.is_some() == list || !traced))
        {
            return;
        }
        // Drop the old ones first (a pooled target lives while held).
        self.targets = None;
        if traced {
            let how = if list { "from a list in a compute pass" } else { "in the trace's fragment pass" };
            log::info!("ssr: ray-traced trace {}x{}, its rays {how}", trace_size.0, trace_size.1);
        }
        let n = levels(trace_size);
        let copy = gpu.pool.get(
            &gpu.device,
            crate::render::pool::Slot::SceneCopy,
            "ssr scene copy",
            size,
            self.scene_format,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let hiz = target(gpu, "ssr depth pyramid", trace_size, HIZ_FORMAT, n);
        let levels = (0..n)
            .map(|l| {
                hiz.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: l,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        self.targets = Some(Targets {
            size,
            trace_size,
            copy: copy.view.clone(),
            _copy: copy,
            hiz: hiz.create_view(&Default::default()),
            levels,
            // The list's compute pass writes its rays' results into it.
            trace: target_with(
                gpu,
                "ssr trace",
                trace_size,
                TRACE_FORMAT,
                1,
                if list { wgpu::TextureUsages::STORAGE_BINDING } else { wgpu::TextureUsages::empty() },
            )
            .create_view(&Default::default()),
            filtered: target(gpu, "ssr filtered", trace_size, TRACE_FORMAT, 1).create_view(&Default::default()),
            skip: target(gpu, "ssr skip depth", trace_size, SKIP_FORMAT, 1).create_view(&Default::default()),
            kinds: target(gpu, "ssr mask kinds", trace_size, KINDS_FORMAT, 1).create_view(&Default::default()),
            list: list.then(|| crate::render::rt::ListBuffers::new(gpu, trace_size)),
        });
    }

    /// The reflections of `frame` put into `scene` (`size` big, in
    /// `scene_format`, shown at `out`); `glow`: this frame's glow sprites
    /// (scene size), when they go to the streak buffer rather than the
    /// scene.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        size: (u32, u32),
        out: (u32, u32),
        scene_format: wgpu::TextureFormat,
        frame: &Frame,
        glow: Option<&wgpu::TextureView>,
    ) {
        if scene_format != self.scene_format {
            self.scene_format = scene_format;
            self.pipelines = Self::pipelines(gpu, &self.layouts, scene_format);
            self.targets = None;
        }
        self.ensure_targets(gpu, size, out, frame.full, frame.rt.is_some());
        let t = self.targets.as_ref().unwrap();
        let ms = usize::from(frame.samples > 1);
        let [right, up, forward] = frame.basis;
        let u = Uniform {
            size: [size.0 as f32, size.1 as f32, t.trace_size.0 as f32, t.trace_size.1 as f32],
            depth: [frame.near, frame.far, if frame.reversed { 1.0 } else { 0.0 }, (t.levels.len() - 1) as f32],
            proj: [frame.scale.0, frame.scale.1, REACH, if frame.full { STEPS_FULL } else { STEPS_HALF }],
            right: right.extend(crate::env_parsed!("NFSU2_SSR_DEBUG", f32).unwrap_or(0.0)).to_array(),
            // NFSU2_SSR_NOGLOW=1 (a check aid): the glow sprites left out.
            up: up.extend(if glow.is_some() && !crate::env_set!("NFSU2_SSR_NOGLOW") { 1.0 } else { 0.0 }).to_array(),
            forward: forward.extend(if frame.mask.enhanced { 1.0 } else { 0.0 }).to_array(),
            fog: frame.mask.fog.map_or([0.0; 4], |(v, _)| [v[0], v[1], v[2], 1.0]),
            fog_color: {
                let mut c = frame.mask.fog.map_or([0.0; 4], |(_, c)| [c[0], c[1], c[2], 0.0]);
                c[3] = if frame.mask.wet { 1.0 } else { 0.0 };
                c
            },
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let dummy_depth = &self.dummy_depth[ms];
        let group = |depth: &wgpu::TextureView,
                     hiz: &wgpu::TextureView,
                     color: &wgpu::TextureView,
                     mask: bool,
                     ssr: &wgpu::TextureView,
                     kinds: &wgpu::TextureView| {
            let (term, surface, tint) = if mask {
                (&frame.mask.term, &frame.mask.surface, &frame.mask.tint)
            } else {
                (&self.dummy, &self.dummy, &self.dummy)
            };
            let glow = glow.filter(|_| mask).unwrap_or(&self.dummy);
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ssr"),
                layout: &self.layouts[ms],
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(hiz) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(color) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(term) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(surface) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(ssr) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 7, resource: self.uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(tint) },
                    wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(glow) },
                    wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(kinds) },
                ],
            })
        };
        let pipelines = &self.pipelines[ms];
        // The scene-sized copy and composite only where the world's
        // mask was drawn (its depth not at the clear value), when the mask
        // has a depth of the scene's size; the debug views take every pixel.
        let debug = crate::env_parsed!("NFSU2_SSR_DEBUG", f32).unwrap_or(0.0) != 0.0;
        let mask_depth = frame.mask.depth.as_ref().filter(|d| {
            let t = d.texture();
            !debug
                && !crate::env_set!("NFSU2_SSR_FULL")
                && t.format() == crate::render::DEPTH_FORMAT
                && t.sample_count() == 1
                && (t.width(), t.height()) == size
        });
        let pass = |encoder: &mut wgpu::CommandEncoder,
                    label: &str,
                    target: &[&wgpu::TextureView],
                    load: wgpu::LoadOp<wgpu::Color>,
                    depth: Option<(&wgpu::TextureView, bool)>,
                    p: Pass,
                    bind: &wgpu::BindGroup| {
            let colors: Vec<_> = target
                .iter()
                .map(|&view| {
                    Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                    })
                })
                .collect();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &colors,
                // (view, written): written, cleared first; else read only.
                depth_stencil_attachment: depth.map(|(view, write)| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: write
                        .then_some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipelines[p as usize]);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        };
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let dummy = &self.dummy;
        // The scene's copy (what the composite adds to; the trace reads
        // the scene itself, unchanged until the composite).
        match mask_depth {
            Some(d) => pass(
                encoder,
                "ssr copy",
                &[&t.copy],
                clear,
                Some((d, false)),
                Pass::CopyMasked,
                &group(dummy_depth, dummy, scene, false, dummy, dummy),
            ),
            None => pass(
                encoder,
                "ssr copy",
                &[&t.copy],
                clear,
                None,
                Pass::Copy,
                &group(dummy_depth, dummy, scene, false, dummy, dummy),
            ),
        }
        // The pyramid (and the trace-sized mask depth and kinds).
        pass(
            encoder,
            "ssr depth pyramid",
            &[&t.levels[0], &t.kinds],
            clear,
            Some((&t.skip, true)),
            Pass::Linear,
            &group(&frame.depth, dummy, dummy, true, dummy, dummy),
        );
        for l in 1..t.levels.len() {
            pass(
                encoder,
                "ssr depth pyramid level",
                &[&t.levels[l]],
                clear,
                None,
                Pass::Down,
                &group(dummy_depth, &t.levels[l - 1], dummy, false, dummy, dummy),
            );
        }
        crate::prof::gpu::mark(encoder, "graphics: ssr copy and depth pyramid");
        let trace_group = group(dummy_depth, &t.hiz, scene, true, &self.dummy, &self.dummy);
        match &frame.rt {
            Some(rt) => match &t.list {
                Some(list) => crate::render::rt::ssr_trace_list(
                    gpu,
                    encoder,
                    &self.layouts,
                    &mut self.rt_list,
                    ms,
                    rt,
                    t,
                    list,
                    &trace_group,
                    crate::render::rt::ListInputs {
                        term: &frame.mask.term,
                        surface: &frame.mask.surface,
                        uniform: &self.uniform,
                    },
                ),
                None => crate::render::rt::ssr_trace(gpu, encoder, &self.layouts, &mut self.rt, ms, rt, t, &trace_group),
            },
            None => {
                pass(encoder, "ssr trace", &[&t.trace], clear, Some((&t.skip, false)), Pass::Trace, &trace_group);
                crate::prof::gpu::mark(encoder, "graphics: ssr trace");
            }
        }
        pass(
            encoder,
            "ssr filter",
            &[&t.filtered],
            clear,
            Some((&t.skip, false)),
            Pass::Denoise,
            &group(dummy_depth, &self.dummy, &self.dummy, true, &t.trace, &t.kinds),
        );
        crate::prof::gpu::mark(encoder, "graphics: ssr filter");
        let composite_group = group(&frame.depth, &self.dummy, &t.copy, true, &t.filtered, &self.dummy);
        match mask_depth {
            // Only the mask's pixels: the others keep the scene (the
            // composite's `base` there).
            Some(d) => pass(
                encoder,
                "ssr composite",
                &[scene],
                wgpu::LoadOp::Load,
                Some((d, false)),
                Pass::CompositeMasked,
                &composite_group,
            ),
            None => pass(encoder, "ssr composite", &[scene], clear, None, Pass::Composite, &composite_group),
        }
        crate::prof::gpu::mark(encoder, "graphics: ssr composite");
    }
}

/// ssr.wgsl for a multisampled depth: its sample 0 (no gather there).
pub(crate) fn multisampled(source: &str) -> String {
    const GATHER: &str = "    return textureGather(depth_tex, samp, (vec2<f32>(c) + 1.0) / p.size.xy); // MULTISAMPLE GATHER";
    assert!(source.contains(GATHER));
    source
        .replace(GATHER, "    return vec4<f32>(raw_depth(c), raw_depth(c + vec2<i32>(1, 0)), raw_depth(c + vec2<i32>(0, 1)), raw_depth(c + vec2<i32>(1, 1)));")
        .replace("texture_depth_2d", "texture_depth_multisampled_2d")
}

/// A depth state over `format` (no bias, no stencil).
fn depth_state(format: wgpu::TextureFormat, write: bool, compare: wgpu::CompareFunction) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias: Default::default(),
    }
}

/// The trace and the filter against the trace-sized mask depth (vs_trace's
/// 0.25 / vs_filter's 0.75 greater than fs_linear's 0 / 0.5 / 1).
pub(crate) fn skip_test() -> wgpu::DepthStencilState {
    depth_state(SKIP_FORMAT, false, wgpu::CompareFunction::Greater)
}

/// The copy and the composite against the world's mask depth: not its
/// clear value (vs_mask's).
fn mask_test() -> wgpu::DepthStencilState {
    depth_state(crate::render::DEPTH_FORMAT, false, wgpu::CompareFunction::NotEqual)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/chrome-flicker.md: the trace adds the glow sprites only
    /// away from cars and from 2–4 m of path on (a road ray under a car took
    /// its neon glow and the filter drew magenta streaks behind it).
    #[test]
    fn traced_glow_skips_cars_and_short_paths() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
        let line = src.lines().find(|l| l.contains("colour += min(textureSampleLevel(glow_tex")).unwrap();
        assert!(line.contains("smoothstep(2.0, 4.0, path)"), "{line}");
        assert!(src.contains("if p.up.w > 0.5 && !near_car {"));
        assert!(src.contains("near_car = hs > 0.0 && is_car(hk);"));
    }

    const N: u32 = 64;
    const NEAR: f32 = 0.5;
    const FAR: f32 = 1000.0;

    /// The synthetic scene's view depth at pixel row `y` (x right, y up, z
    /// ahead; m00 = m11 = 1): a road 1 m below the eye up to 10 m, then a
    /// wall at 10 m filling the rest.
    fn scene_z(y: u32) -> f32 {
        let ndc_y = 1.0 - (y as f32 + 0.5) / N as f32 * 2.0;
        if ndc_y < -0.1 { 1.0 / -ndc_y } else { 10.0 }
    }

    fn texture(gpu: &Gpu, format: wgpu::TextureFormat, samples: u32) -> wgpu::Texture {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: N, height: N, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    fn write(gpu: &Gpu, t: &wgpu::Texture, data: &[u8], bytes_per_texel: u32) {
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: t,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(N * bytes_per_texel),
                rows_per_image: Some(N),
            },
            wgpu::Extent3d { width: N, height: N, depth_or_array_layers: 1 },
        );
    }

    /// The scene's depth as the native (non-reversed) projection stores it.
    fn depth_pass(gpu: &Gpu, view: &wgpu::TextureView, samples: u32) {
        let mut src = String::from(
            "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> { let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)); return vec4<f32>(q * 2.0 - 1.0, 0.5, 1.0); }\n",
        );
        src += &format!(
            "@fragment fn fs(@builtin(position) p: vec4<f32>) -> @builtin(frag_depth) f32 {{ let ny = 1.0 - p.y / {N}.0 * 2.0; var z = 10.0; if ny < -0.1 {{ z = 1.0 / -ny; }} return {FAR} * (z - {NEAR}) / (z * ({FAR} - {NEAR})); }}\n"
        );
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::render::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
    }

    /// The traced reflection of a red wall on a mirror road, at half and
    /// full size, with single and multisampled depth, on a wet road (a dry
    /// one is held to the planar term's luminance): road pixels whose
    /// mirror ray meets the wall turn red by the mask's weight; pixels off
    /// the mask, and the alpha, stay as they were; no validation error.
    #[test]
    #[ignore = "requires a GPU; run explicitly for the SSR passes"]
    fn traced_wall_reflects_on_the_mirror_road() {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let f16 = |v: f32| half_bits(v).to_le_bytes();
        // Scene: wall red, road grey (alpha 0.5, the bleach mask).
        let mut colour = Vec::new();
        let (mut term, mut surface) = (Vec::new(), Vec::new());
        for y in 0..N {
            for _ in 0..N {
                let z = scene_z(y);
                let road = z < 10.0;
                colour.extend(if road { [51u8, 51, 51, 128] } else { [255, 0, 0, 128] });
                // The planar term black, weight 1 on the road.
                for v in [0.0, 0.0, 0.0, if road { 1.0 } else { 0.0 }] {
                    term.extend(f16(v));
                }
                // World normal up (z), roughness 0, the view depth.
                for v in [0.0, 0.0, 0.0, if road { z } else { 0.0 }] {
                    surface.extend(f16(v));
                }
            }
        }
        for (samples, full) in [(1u32, false), (1, true), (4, false)] {
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let scene = texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, 1);
            write(&gpu, &scene, &colour, 4);
            let t = texture(&gpu, wgpu::TextureFormat::Rgba16Float, 1);
            write(&gpu, &t, &term, 8);
            let sf = texture(&gpu, wgpu::TextureFormat::Rgba16Float, 1);
            write(&gpu, &sf, &surface, 8);
            let depth = texture(&gpu, crate::render::DEPTH_FORMAT, samples).create_view(&Default::default());
            depth_pass(&gpu, &depth, samples);
            let tint = texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, 1);
            write(&gpu, &tint, &[255u8, 255, 255, 0].repeat((N * N) as usize), 4);
            let frame = Frame {
                mask: Mask {
                    term: t.create_view(&Default::default()),
                    surface: sf.create_view(&Default::default()),
                    tint: tint.create_view(&Default::default()),
                    fog: None,
                    enhanced: true,
                    wet: true,
                    depth: (samples == 1).then(|| depth.clone()),
                },
                depth,
                samples,
                near: NEAR,
                far: FAR,
                reversed: false,
                scale: (1.0, 1.0),
                basis: [glam::Vec3::X, glam::Vec3::Z, glam::Vec3::Y],
                full,
                rt: None,
            };
            let mut ssr = Ssr::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            ssr.run(
                &gpu,
                &mut encoder,
                &scene.create_view(&Default::default()),
                (N, N),
                (N, N),
                wgpu::TextureFormat::Rgba8Unorm,
                &frame,
                None,
            );
            gpu.queue.submit([encoder.finish()]);
            let error = pollster::block_on(scope.pop());
            assert!(error.is_none(), "samples {samples} full {full}: {error:?}");
            let out = crate::render::read_back(&gpu, &scene, N, N);
            let px = |x: u32, y: u32| &out[((y * N + x) * 4) as usize..((y * N + x) * 4 + 4) as usize];
            // Row 48 (ndc y −0.52, the road at 1.9 m): its mirror ray rises
            // to the wall (at 10 m, 3.8 m up, on screen). Red added.
            let road = px(32, 48);
            assert!(road[0] > 150 && road[1] < 60, "samples {samples} full {full}: road {road:?}");
            assert_eq!(road[3], 128, "alpha kept");
            // The wall (no mask) unchanged.
            assert_eq!(px(32, 10), [255, 0, 0, 128]);
        }
    }

    /// On a wet road (Wet Roads on: no dry-road hold), a wall the
    /// road's rays meet reflects red; flagged as an overlay in the mask
    /// (the GPS arrow: tint.a 1, its view depth) it is no hit: the road
    /// keeps its planar term (black here); the wall itself unchanged.
    #[test]
    #[ignore = "requires a GPU; run explicitly for the SSR passes"]
    fn overlay_is_left_out_of_traced_reflections() {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let f16 = |v: f32| half_bits(v).to_le_bytes();
        let (mut colour, mut term, mut surface, mut tint): (Vec<u8>, Vec<u8>, Vec<u8>, Vec<[u8; 4]>) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for y in 0..N {
            for _ in 0..N {
                let z = scene_z(y);
                let road = z < 10.0;
                colour.extend(if road { [51u8, 51, 51, 128] } else { [255, 0, 0, 128] });
                for v in [0.0, 0.0, 0.0, if road { 1.0 } else { 0.0 }] {
                    term.extend(f16(v));
                }
                // Road: normal up, its depth; the wall: the overlay's depth.
                for v in [0.0, 0.0, 0.0, z] {
                    surface.extend(f16(v));
                }
                tint.push(if road { [255u8, 255, 255, 0] } else { [0, 0, 0, 255] });
            }
        }
        // Unflagged first (the control: the wall reflects), then flagged.
        let plain: Vec<u8> =
            tint.iter().flat_map(|t| [t[0].max(255 - t[3]), t[1].max(255 - t[3]), t[2].max(255 - t[3]), 0]).collect();
        let flagged: Vec<u8> = tint.concat();
        for (flag, samples, full) in
            [false, true].into_iter().flat_map(|f| [(f, 1u32, false), (f, 1, true), (f, 4, false)])
        {
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let scene = texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, 1);
            write(&gpu, &scene, &colour, 4);
            let t = texture(&gpu, wgpu::TextureFormat::Rgba16Float, 1);
            write(&gpu, &t, &term, 8);
            let sf = texture(&gpu, wgpu::TextureFormat::Rgba16Float, 1);
            write(&gpu, &sf, &surface, 8);
            let depth = texture(&gpu, crate::render::DEPTH_FORMAT, samples).create_view(&Default::default());
            depth_pass(&gpu, &depth, samples);
            let tn = texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, 1);
            write(&gpu, &tn, if flag { &flagged } else { &plain }, 4);
            let frame = Frame {
                mask: Mask {
                    term: t.create_view(&Default::default()),
                    surface: sf.create_view(&Default::default()),
                    tint: tn.create_view(&Default::default()),
                    fog: None,
                    enhanced: true,
                    wet: true,
                    depth: (samples == 1).then(|| depth.clone()),
                },
                depth,
                samples,
                near: NEAR,
                far: FAR,
                reversed: false,
                scale: (1.0, 1.0),
                basis: [glam::Vec3::X, glam::Vec3::Z, glam::Vec3::Y],
                full,
                rt: None,
            };
            let mut ssr = Ssr::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            ssr.run(
                &gpu,
                &mut encoder,
                &scene.create_view(&Default::default()),
                (N, N),
                (N, N),
                wgpu::TextureFormat::Rgba8Unorm,
                &frame,
                None,
            );
            gpu.queue.submit([encoder.finish()]);
            let error = pollster::block_on(scope.pop());
            assert!(error.is_none(), "flag {flag} samples {samples} full {full}: {error:?}");
            let out = crate::render::read_back(&gpu, &scene, N, N);
            let px = |x: u32, y: u32| &out[((y * N + x) * 4) as usize..((y * N + x) * 4 + 4) as usize];
            let red = (40..N).filter(|&y| px(32, y)[0] > 150).count();
            if flag {
                assert_eq!(red, 0, "samples {samples} full {full}: the road took the overlay");
            } else {
                assert!(red > 0, "samples {samples} full {full}: the control wall reflects");
            }
            assert_eq!(px(32, 10), [255, 0, 0, 128]);
        }
    }

    /// A small traced world for the ray list's test (x right, y ahead, z
    /// up; the eye at the origin, as the synthetic screen's basis): the
    /// road's plane 1 m under the eye, the wall at 10 m, a ceiling over
    /// both and a wall behind the eye, each in 8 × 8 cells of their own
    /// colour (every hit's triangle tells); and a car's box (instance 1).
    fn traced_world(gpu: &Gpu) -> crate::render::rt::FrameRt {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut colors: Vec<u32> = Vec::new();
        let faces: [([f32; 3], [f32; 3], [f32; 3], [f32; 3]); 4] = [
            ([-40.0, 0.0, -1.0], [80.0, 0.0, 0.0], [0.0, 10.0, 0.0], [0.3, 0.3, 0.3]),
            ([-40.0, 10.0, -1.0], [80.0, 0.0, 0.0], [0.0, 0.0, 31.0], [1.0, 0.1, 0.1]),
            ([-40.0, -20.0, 8.0], [80.0, 0.0, 0.0], [0.0, 30.0, 0.0], [0.1, 0.9, 0.2]),
            ([-40.0, -15.0, -1.0], [80.0, 0.0, 0.0], [0.0, 0.0, 9.0], [0.2, 0.3, 1.0]),
        ];
        for (o, a, b, rgb) in faces {
            let p = |u: f32, v: f32| [o[0] + a[0] * u + b[0] * v, o[1] + a[1] * u + b[1] * v, o[2] + a[2] * u + b[2] * v];
            for j in 0..8 {
                for i in 0..8 {
                    let (u0, u1, v0, v1) = (i as f32 / 8.0, (i + 1) as f32 / 8.0, j as f32 / 8.0, (j + 1) as f32 / 8.0);
                    positions.extend_from_slice(&[p(u0, v0), p(u1, v0), p(u0, v1), p(u1, v0), p(u1, v1), p(u0, v1)]);
                    let shade = 0.6 + 0.4 * (((i + j) % 3) as f32 / 2.0);
                    let c = crate::render::rt::pack(rgb.map(|v| v * shade * 0.5));
                    colors.extend_from_slice(&[c, c]);
                }
            }
        }
        // A car's box around the car patch's points (instance 1): their
        // rays start inside it and go on through it to the world (rt_fill's
        // second cast).
        let world = colors.len() as u32;
        let car: Vec<[f32; 3]> = {
            let (lo, hi) = ([-1.5f32, 1.0, -1.6], [0.5f32, 3.5, -0.4]);
            let c = |i: usize| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }];
            let faces = [[0, 1, 3, 2], [4, 5, 7, 6], [0, 1, 5, 4], [2, 3, 7, 6], [0, 2, 6, 4], [1, 3, 7, 5]];
            faces.iter().flat_map(|f| [c(f[0]), c(f[1]), c(f[2]), c(f[0]), c(f[2]), c(f[3])]).collect()
        };
        colors.extend(std::iter::repeat_n(crate::render::rt::pack([0.45, 0.4, 0.05]), car.len() / 3));
        let device = &gpu.device;
        let geometry = |positions: &[[f32; 3]], label| {
            let (blas, size) = crate::render::rt::blas_for(device, label, positions.len() as u32 / 3);
            let vb = wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(positions),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            });
            (blas, size, vb)
        };
        let (wb, ws, wv) = geometry(&positions, "test world");
        let (cb, cs, cv) = geometry(&car, "test car");
        let mut tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: None,
            max_instances: 2,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });
        let identity = crate::render::rt::instance_transform(glam::Mat4::IDENTITY);
        tlas[0] = Some(wgpu::TlasInstance::new(&wb, identity, 0, crate::render::rt::MASK_WORLD));
        tlas[1] = Some(wgpu::TlasInstance::new(&cb, identity, world, crate::render::rt::MASK_CAR));
        let entry = |blas, size, vertex_buffer| wgpu::BlasBuildEntry {
            blas,
            geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                size,
                vertex_buffer,
                first_vertex: 0,
                vertex_stride: 12,
                index_buffer: None,
                first_index: None,
                transform_buffer: None,
                transform_buffer_offset: None,
            }]),
        };
        let mut enc = device.create_command_encoder(&Default::default());
        enc.build_acceleration_structures([&entry(&wb, &ws, &wv), &entry(&cb, &cs, &cv)], [&tlas]);
        gpu.queue.submit([enc.finish()]);
        let buffer = |contents: &[u8], usage| {
            wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor {
                label: None,
                contents,
                usage,
            })
        };
        crate::render::rt::FrameRt {
            tlas,
            colors: buffer(bytemuck::cast_slice(&colors), wgpu::BufferUsages::STORAGE),
            uniform: buffer(
                bytemuck::cast_slice(&[0.0f32, 0.0, 0.0, crate::render::rt::REACH, 2.0, 0.0, 0.0, 0.0]),
                wgpu::BufferUsages::UNIFORM,
            ),
        }
    }

    /// The ray list (docs/rt-reflections.md "The ray list") against the
    /// fragment form it replaces, on a device that takes it: a road with
    /// rough and mirror parts and a patch of car facing the eye, the screen
    /// march's wall on screen and a small traced world around it, wet and
    /// dry, at half and full size, with single and multisampled depth. The
    /// trace target is the fragment form's bit for bit, and so is the
    /// picture; the list holds each pixel that needs a ray once, and only
    /// those: the trace pixels whose march result (the plain SSR trace's,
    /// as the f16 target holds it) has a confidence below 0.98, each with
    /// that result (0 for the early outs and misses); no validation error.
    #[test]
    #[ignore = "requires a GPU with ray queries; run explicitly for the ray list"]
    fn ray_list_traces_what_the_fragment_form_traces() {
        let _globals = crate::qol::test_globals();
        let mut e = crate::qol::Enhancements::default();
        e.ssr = 3;
        e.apply();
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        assert!(crate::render::rt::list_path(&gpu), "a device that traces from the list");
        let rt = traced_world(&gpu);
        let f16 = |v: f32| half_bits(v).to_le_bytes();
        let (mut colour, mut term, mut surface) = (Vec::new(), Vec::new(), Vec::new());
        let mut mask0 = vec![false; (N * N) as usize];
        for y in 0..N {
            for x in 0..N {
                let z = scene_z(y);
                let road = z < 10.0;
                // A car's patch (kind 1 + roughness) facing the eye: its rays
                // turn back past the screen.
                let car = road && (20..28).contains(&x) && (50..58).contains(&y);
                colour.extend(if road { [51u8, 51, 51, 128] } else { [255, 0, 0, 128] });
                let w = if road { 0.25 + 0.75 * (x % 4) as f32 / 3.0 } else { 0.0 };
                for v in [0.1, 0.2, 0.3, w] {
                    term.extend(f16(v));
                }
                let rough = (x / 8) as f32 * 0.06;
                let (nx, ny, kind) = if car { (0.2, -0.6, 1.0 + rough) } else { (0.0, 0.0, rough) };
                for v in [nx, ny, kind, if road { z } else { 0.0 }] {
                    surface.extend(f16(v));
                }
                mask0[(y * N + x) as usize] = road;
            }
        }
        for (wet, samples, full) in [(true, 1u32, true), (true, 1, false), (false, 1, true), (true, 4, true)] {
            let scene = texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, 1);
            let t = texture(&gpu, wgpu::TextureFormat::Rgba16Float, 1);
            write(&gpu, &t, &term, 8);
            let sf = texture(&gpu, wgpu::TextureFormat::Rgba16Float, 1);
            write(&gpu, &sf, &surface, 8);
            let depth = texture(&gpu, crate::render::DEPTH_FORMAT, samples).create_view(&Default::default());
            depth_pass(&gpu, &depth, samples);
            let tint = texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, 1);
            write(&gpu, &tint, &[255u8, 255, 255, 0].repeat((N * N) as usize), 4);
            let frame = |rt: Option<crate::render::rt::FrameRt>| Frame {
                mask: Mask {
                    term: t.create_view(&Default::default()),
                    surface: sf.create_view(&Default::default()),
                    tint: tint.create_view(&Default::default()),
                    fog: Some(([60.0, 0.01, 0.2], [0.1, 0.12, 0.2])),
                    enhanced: true,
                    wet,
                    depth: (samples == 1).then(|| depth.clone()),
                },
                depth: depth.clone(),
                samples,
                near: NEAR,
                far: FAR,
                reversed: false,
                scale: (1.0, 1.0),
                basis: [glam::Vec3::X, glam::Vec3::Z, glam::Vec3::Y],
                full,
                rt,
            };
            let what = format!("wet {wet} samples {samples} full {full}");
            // (the trace target's f16 texels, the picture, the list)
            let run = |rt: Option<crate::render::rt::FrameRt>, fragment_rays: bool| {
                let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
                write(&gpu, &scene, &colour, 4);
                let mut ssr = Ssr::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
                ssr.fragment_rays = fragment_rays;
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                let view = scene.create_view(&Default::default());
                ssr.run(&gpu, &mut encoder, &view, (N, N), (N, N), wgpu::TextureFormat::Rgba8Unorm, &frame(rt), None);
                gpu.queue.submit([encoder.finish()]);
                let error = pollster::block_on(scope.pop());
                assert!(error.is_none(), "{what}: {error:?}");
                let targets = ssr.targets.as_ref().unwrap();
                let (w, h) = targets.trace_size;
                let trace: Vec<u16> =
                    bytemuck::cast_slice(&crate::render::read_back(&gpu, targets.trace.texture(), w, h)).to_vec();
                let list = targets.list.as_ref().map(|l| l.read(&gpu));
                ((w, h), trace, crate::render::read_back(&gpu, &scene, N, N), list)
            };
            let (size, plain, _, _) = run(None, false);
            let (_, fragment, fragment_picture, none) = run(Some(rt.clone()), true);
            assert!(none.is_none(), "{what}: the fragment form keeps no list");
            let (_, listed, listed_picture, list) = run(Some(rt.clone()), false);
            let (head, px, c) = list.expect("the list form");
            assert!(fragment == listed, "{what}: the trace target differs from the fragment form's");
            assert!(fragment_picture == listed_picture, "{what}: the picture differs from the fragment form's");
            assert_ne!(plain, fragment, "{what}: the rays changed the trace (a test that traces)");
            // The pixels that need a ray: the trace pixels on the mask
            // (its texel as mask_texel takes it) whose march result's
            // confidence is below 0.98.
            let (w, h) = size;
            let mut need = std::collections::BTreeSet::new();
            for y in 0..h {
                for x in 0..w {
                    let m = |q: u32, s: u32| (((q as f32 + 0.5) * N as f32 / s as f32) as u32).min(N - 1);
                    if !mask0[(m(y, h) * N + m(x, w)) as usize] {
                        continue;
                    }
                    let a = half_value(plain[((y * w + x) * 4 + 3) as usize]);
                    assert!((a - 0.98).abs() > 0.001, "{what}: ({x}, {y}) confidence {a} at the test's edge");
                    if a < 0.98 {
                        need.insert((x, y));
                    }
                }
            }
            assert_eq!(head[0] as usize, px.len());
            assert_eq!(head[1], (head[0]).div_ceil(64), "{what}: the dispatch's groups");
            assert_eq!(head[2..], [1, 1]);
            let listed: std::collections::BTreeSet<(u32, u32)> = px.iter().map(|e| (e & 0x7FFF, (e >> 15) & 0x7FFF)).collect();
            assert_eq!(listed.len(), px.len(), "{what}: a pixel listed twice");
            assert!(listed == need, "{what}: {} listed, {} need a ray", listed.len(), need.len());
            assert!(need.len() > 50 && need.len() < (w * h) as usize, "{what}: {} of {} need a ray", need.len(), w * h);
            let mut zeros = 0;
            for (e, c) in px.iter().zip(&c) {
                let i = (((e >> 15) & 0x7FFF) * w + (e & 0x7FFF)) as usize * 4;
                let texel = &plain[i..i + 4];
                if e >> 31 == 1 {
                    zeros += 1;
                    assert_eq!(texel, [0, 0, 0, 0], "{what}: an early out's result");
                } else {
                    // The march's f32 result, as the target rounds it
                    // (toward zero here; to nearest on other devices).
                    for (&t, &v) in texel.iter().zip(c) {
                        assert!(half_rounds(v).contains(&t), "{what}: a listed result {c:?}, the target's {texel:?}");
                    }
                }
            }
            assert!(zeros > 0 && zeros < px.len(), "{what}: {zeros} of {} listed with a result of 0", px.len());
            let traced = fragment.chunks(4).zip(plain.chunks(4)).filter(|(a, b)| a != b).count();
            eprintln!("{what}: {} of {} trace pixels listed ({zeros} with a result of 0), {traced} changed by their rays", px.len(), w * h);
        }
    }

    /// An IEEE half's value (finite).
    fn half_value(b: u16) -> f32 {
        let sign = if b & 0x8000 != 0 { -1.0 } else { 1.0 };
        let (e, m) = (i32::from((b >> 10) & 0x1F), f32::from(b & 0x3FF));
        sign * if e == 0 { m * 2f32.powi(-24) } else { (1.0 + m / 1024.0) * 2f32.powi(e - 15) }
    }

    /// The halves an f32 in the halves' finite range rounds to: toward
    /// zero, and to the nearest.
    fn half_rounds(v: f32) -> [u16; 2] {
        let sign = if v.is_sign_negative() { 0x8000u16 } else { 0 };
        let a = v.abs();
        // The largest half not above a, by its bits (they grow with the value).
        let mut lo = 0u16;
        for bit in (0..15).rev() {
            let t = lo | 1 << bit;
            if t < 0x7C00 && half_value(t) <= a {
                lo = t;
            }
        }
        let near = if lo < 0x7BFF && half_value(lo + 1) - a < a - half_value(lo) { lo + 1 } else { lo };
        [sign | lo, sign | near]
    }

    /// An f32 as IEEE half bits (normal range only, enough for the test).
    fn half_bits(v: f32) -> u16 {
        if v == 0.0 {
            return 0;
        }
        let b = v.to_bits();
        let sign = ((b >> 16) & 0x8000) as u16;
        let exp = ((b >> 23) & 0xFF) as i32 - 127 + 15;
        let man = ((b >> 13) & 0x3FF) as u16;
        sign | ((exp as u16) << 10) | man
    }

    /// Both variants of the passes parse and validate (naga, no GPU):
    /// the multisampled one reads sample 0 instead of gathering; the
    /// masked passes' vertex entries and the trace's early outs are there.
    #[test]
    fn masked_variants_validate() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
        let ms = multisampled(src);
        assert!(!ms.contains("textureGather") && ms.contains("texture_depth_multisampled_2d"));
        for s in [src.to_string(), ms] {
            let module = wgpu::naga::front::wgsl::parse_str(&s).expect("parse");
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("valid");
            for entry in [
                "vs_main",
                "vs_trace",
                "vs_filter",
                "vs_mask",
                "fs_linear",
                "fs_trace",
                "fs_denoise",
                "fs_copy",
                "fs_composite",
            ] {
                assert!(module.entry_points.iter().any(|e| e.name == entry), "{entry}");
            }
        }
        // fs_linear's mask depth repeats the trace's and the filter's early
        // outs (the same tests on the same texel).
        assert!(src.contains("    if term.a <= 0.002 || z <= 0.0 {\n        return vec4<f32>(0.0);\n    }"));
        assert!(src.contains("        mask = select(0.5, 0.0, !(term.a <= 0.002 || surface.w <= 0.0));"));
        assert!(src.contains("    if textureLoad(term_tex, m, 0).a <= 0.002 {\n        return centre;\n    }"));
    }

    #[test]
    fn pyramid_levels_and_trace_sizes() {
        assert_eq!(trace_size((1920, 1080), (1920, 1080), false), (960, 540));
        assert_eq!(trace_size((1920, 1080), (1920, 1080), true), (1920, 1080));
        assert_eq!(trace_size((1, 1), (1, 1), false), (1, 1));
        // SSAA / Render Scale: traced at the screen's size (or half), not
        // the supersampled scene's; a scene below the screen caps it.
        assert_eq!(trace_size((2715, 1527), (1920, 1080), false), (960, 540));
        assert_eq!(trace_size((7680, 4320), (1920, 1080), true), (1920, 1080));
        assert_eq!(trace_size((960, 540), (1920, 1080), true), (960, 540));
        // 960 → 10 levels, held to MAX_LEVELS; tiny targets keep one.
        assert_eq!(levels((960, 540)), MAX_LEVELS);
        assert_eq!(levels((1, 1)), 1);
        assert_eq!(levels((5, 3)), 3);
    }
}
