//! FSR 2 temporal upscaling (docs/notes/graphics/fsr2.md; a **clone deviation**,
//! opt-in on the Upscaling row, nothing like it in SPEED2.EXE): a WGSL
//! port of AMD FidelityFX Super Resolution 2.2 (MIT; THIRD-PARTY-NOTICES.md)
//! run by the temporal pass ([`crate::render::temporal::Temporal`]) in place of
//! its own resolve. The jitter, the cuts, the late sprites and the motion
//! (depth + rigid car poses) are the TAA pass's; this module owns FSR 2's passes and
//! their textures.
//!
//! Passes per frame (compute, `fsr2.wgsl`): motion (render size) →
//! reconstruct and dilate (nearest depth, dilated motion, the previous
//! depth scattered, lock luma) → shading-change luma (1/32) → depth clip
//! (disocclusion, prepared colour, reactive / composition masks) → lock
//! (thin features) → accumulate (display size: Lanczos upsample, Lanczos
//! history, rectification, locks, luma instability) → RCAS or a copy into
//! the scene (fragment), and the display-size depth for the late sprites.

use crate::render::Gpu;
use crate::render::temporal::{GpuCar, MAX_CARS};

/// FSR 2's quality modes: the Upscaling row's values 7..=11 and their
/// upscale ratio per side (ffxFsr2GetUpscaleRatioFromQualityMode; Native
/// AA is FSR 2 at 1.0).
pub const MODES: [(u8, &str, f64); 5] = [
    (7, "Native AA", 1.0),
    (8, "Quality", 1.5),
    (9, "Balanced", 1.7),
    (10, "Performance", 2.0),
    (11, "Ultra Performance", 3.0),
];

/// The upscale ratio of Upscaling value `v` when it is an FSR 2 mode.
pub fn ratio(v: u8) -> Option<f64> {
    MODES.iter().find(|m| m.0 == v).map(|m| m.2)
}

/// The render size for `display` at `ratio` (ffxFsr2GetRenderResolutionFromQualityMode:
/// each side divided and truncated, at least 1).
pub fn render_size(display: (u32, u32), ratio: f64) -> (u32, u32) {
    (((f64::from(display.0) / ratio) as u32).max(1), ((f64::from(display.1) / ratio) as u32).max(1))
}

/// The jitter's phase count (ffxFsr2GetJitterPhaseCount: 8 × the width
/// ratio², truncated).
pub fn phase_count(render_width: u32, display_width: u32) -> u32 {
    ((8.0 * (f64::from(display_width) / f64::from(render_width.max(1))).powi(2)) as u32).max(1)
}

/// The texture LOD bias FSR 2 asks for (log2(render / display) − 1), in
/// quarter mip levels.
pub fn mip_bias_q(ratio: f64) -> i8 {
    (((1.0 / ratio).log2() - 1.0) * 4.0).round() as i8
}

/// RCAS's sharpening in stops (0 sharpest; FSR 2's `sharpness` s maps to
/// 2 − 2s stops). 0.25 stops = sharpness 0.875: measured against SSAA 2x
/// (docs/notes/graphics/fsr2.md "Sharpness").
pub const SHARPNESS_STOPS: f32 = 0.25;
/// The wheels' reactive value (their spin moves texture the rigid motion
/// does not), as the TAA pass's ≥ 0.35 blend.
pub const WHEEL_REACTIVE: f32 = 0.5;
/// FSR 2's thin-feature locks: on. They were off at first: NFSU2's
/// flip-book and scrolling textures (the chevron barriers, blinking signs)
/// were not masked and locks held their old frames as dotted trails; the
/// world's animation mask (`WorldRenderer::anim_mask`, docs/notes/graphics/fsr2.md
/// "Animation mask") now keeps locks off those textures, as FSR 2's
/// transparency-and-composition mask does (`NFSU2_FSR2_LOCKS=0` off).
pub const LOCKS: bool = true;
/// The history rectification box's largest scale (FSR 2: up to 20 by the
/// upscale ratio, 11.4 at Quality): 2, as the same flip-books smeared
/// inside a looser box (docs/notes/graphics/fsr2.md "Locks").
pub const BOX_SCALE_MAX: f32 = 2.0;
/// The history-disagreement reactive's gain (docs/notes/graphics/fsr2.md "Animated
/// textures"; 0 off).
pub const AUTO_REACTIVE: f32 = 4.0;
/// The composition mask's gain on the YCoCg change the screen-space
/// composites made (SSR, volumetric fog).
pub const COMPOSITION_GAIN: f32 = 8.0;
const SCENE_FORMAT_HISTORY: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// `NFSU2_FSR2_DEBUG=1` motion, `=2` masks (R reactive, G composition, B
/// depth clip), `=3` locks; `=6` zero motion.
fn debug_view() -> f32 {
    static V: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *V.get_or_init(|| crate::render::debug_env::nfsu2_fsr2_debug().unwrap_or(0.0))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub clip_to_rel: [[f32; 4]; 4],
    pub prev_rel_to_clip: [[f32; 4]; 4],
    pub render: [f32; 4],
    pub display: [f32; 4],
    pub jitter: [f32; 4],
    pub params: [f32; 4],
    pub depth: [f32; 4],
    pub misc: [f32; 4],
    pub luma: [f32; 4],
    pub extra: [f32; 4],
    /// X the animation mask is bound (0/1).
    pub anim: [f32; 4],
}

/// FSR 2's DeviceToViewSpaceTransformFactors of a projection `p`
/// (depth d → view distance (factor 1) / (d − factor 0); factors 2, 3:
/// 1 / the x and y scales).
pub fn depth_factors(p: glam::Mat4) -> [f32; 4] {
    [-p.z_axis.z, p.w_axis.z, 1.0 / p.x_axis.x, 1.0 / p.y_axis.y]
}

/// The shading-change luma's block in render pixels (FSR 2: 32, its SPD
/// mip 4; smaller here, docs/notes/graphics/fsr2.md "Locks"). A multiple of 8.
pub const LUMA_BLOCK: u32 = 32;

fn luma_block() -> u32 {
    (crate::render::debug_env::nfsu2_fsr2_luma_block().unwrap_or(LUMA_BLOCK as f32) as u32 / 8).clamp(1, 8) * 8
}

#[derive(Clone, Copy)]
enum B {
    Uniform,
    Tex { filterable: bool },
    Depth,
    Sampler,
    Store(wgpu::TextureFormat),
    Buf { read_only: bool },
}

fn layout(gpu: &Gpu, label: &str, vis: wgpu::ShaderStages, entries: &[(u32, B)]) -> wgpu::BindGroupLayout {
    let entries: Vec<wgpu::BindGroupLayoutEntry> = entries
        .iter()
        .map(|&(binding, b)| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: match b {
                B::Uniform => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                B::Tex { filterable } => wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                B::Depth => wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                B::Sampler => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                B::Store(format) => wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                B::Buf { read_only } => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            },
            count: None,
        })
        .collect();
    // From the device's cache: the passes' pipelines, keyed by their
    // layouts, are then the same objects for every `Fsr2` on the device
    // (one made ahead, `Fsr2::warm`, and the frame's).
    crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor { label: Some(label), entries: &entries })
}

/// `fsr2.wgsl` split at its `//!pass` markers: the common part and each
/// pass's source (common prepended).
pub fn pass_source(name: &str) -> String {
    let src = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::Fsr2);
    let mut parts = src.split("//!pass ");
    let common = parts.next().unwrap_or("");
    for p in parts {
        let (head, body) = p.split_once('\n').unwrap_or((p, ""));
        if head.trim() == name {
            return format!("{common}\n{body}");
        }
    }
    panic!("fsr2.wgsl has no pass {name}");
}

struct Compute {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

fn compute(gpu: &Gpu, name: &str, entries: &[(u32, B)]) -> Compute {
    let layout = layout(gpu, name, wgpu::ShaderStages::COMPUTE, entries);
    let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
        label: Some(name),
        source: wgpu::ShaderSource::Wgsl(pass_source(name).into()),
    });
    let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
        label: Some(name),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
        label: Some(name),
        layout: Some(&pl),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: gpu.disk_pipeline_cache(),
    });
    Compute { layout, pipeline }
}

fn tex(gpu: &Gpu, label: &str, size: (u32, u32), format: wgpu::TextureFormat, storage: bool) -> wgpu::TextureView {
    let mut usage = wgpu::TextureUsages::TEXTURE_BINDING;
    if storage {
        usage |= wgpu::TextureUsages::STORAGE_BINDING;
    }
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// FSR 2's textures for one render / display size.
struct Targets {
    render: (u32, u32),
    display: (u32, u32),
    motion: wgpu::TextureView,
    dilated_motion: [wgpu::TextureView; 2],
    dilated_depth: wgpu::TextureView,
    lock_luma: wgpu::TextureView,
    prepared: wgpu::TextureView,
    masks: wgpu::TextureView,
    opaque: wgpu::TextureView,
    composition: wgpu::TextureView,
    luma: wgpu::TextureView,
    luma_size: (u32, u32),
    reconstructed: wgpu::Buffer,
    new_locks: wgpu::Buffer,
    history: [wgpu::TextureView; 2],
    lock_status: [wgpu::TextureView; 2],
    luma_history: [wgpu::TextureView; 2],
    output: wgpu::TextureView,
}

impl Targets {
    fn new(gpu: &Gpu, render: (u32, u32), display: (u32, u32)) -> Targets {
        use wgpu::TextureFormat::*;
        let r = |l: &str, f| tex(gpu, l, render, f, true);
        let d = |l: &str, f| tex(gpu, l, display, f, true);
        let luma_size = (render.0.div_ceil(luma_block()), render.1.div_ceil(luma_block()));
        let buf = |label: &str, n: u32| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: u64::from(n.max(1)) * 4,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Targets {
            render,
            display,
            motion: r("fsr2 motion", Rgba16Float),
            dilated_motion: [r("fsr2 dilated motion", Rgba16Float), r("fsr2 dilated motion", Rgba16Float)],
            dilated_depth: r("fsr2 dilated depth", R32Float),
            lock_luma: r("fsr2 lock luma", R32Float),
            prepared: r("fsr2 prepared colour", Rgba16Float),
            masks: r("fsr2 masks", Rgba8Unorm),
            opaque: r("fsr2 opaque colour", Rgba16Float),
            composition: r("fsr2 composition mask", Rgba16Float),
            luma: tex(gpu, "fsr2 shading luma", luma_size, Rgba16Float, true),
            luma_size,
            reconstructed: buf("fsr2 reconstructed depth", render.0 * render.1),
            new_locks: buf("fsr2 new locks", display.0 * display.1),
            history: [d("fsr2 history", SCENE_FORMAT_HISTORY), d("fsr2 history", SCENE_FORMAT_HISTORY)],
            lock_status: [d("fsr2 lock status", Rgba16Float), d("fsr2 lock status", Rgba16Float)],
            luma_history: [d("fsr2 luma history", Rgba8Unorm), d("fsr2 luma history", Rgba8Unorm)],
            output: d("fsr2 output", Rgba16Float),
        }
    }
}

/// What [`Fsr2::run`] needs of the frame (built by the temporal pass).
pub struct Frame<'a> {
    pub uniforms: Uniforms,
    pub cars: &'a [u8],
    pub color: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub out: &'a wgpu::TextureView,
    pub out_format: wgpu::TextureFormat,
    pub out_depth: Option<&'a wgpu::TextureView>,
    /// Whether the history is valid (false: reset).
    pub valid: bool,
}

/// FSR 2's pipelines and textures.
pub struct Fsr2 {
    motion: Compute,
    reconstruct: Compute,
    luma: Compute,
    snapshot: Compute,
    composition: Compute,
    depth_clip: Compute,
    lock: Compute,
    accumulate: Compute,
    out_layout: wgpu::BindGroupLayout,
    out_pipeline_layout: wgpu::PipelineLayout,
    out_shader: wgpu::ShaderModule,
    outputs: Vec<(Option<wgpu::TextureFormat>, bool, wgpu::RenderPipeline)>,
    uniforms: wgpu::Buffer,
    snap_uniforms: wgpu::Buffer,
    cars: wgpu::Buffer,
    sampler: wgpu::Sampler,
    targets: Option<Targets>,
    /// Which of the ping-pong textures this frame writes.
    current: usize,
    /// Whether this frame's composition mask was made ([`Fsr2::composite`]).
    composed: bool,
    /// This frame's animation mask (colour, depth;
    /// [`Fsr2::set_anim_mask`]), and the 1 × 1 stand-ins bound without one.
    anim: Option<(wgpu::TextureView, wgpu::TextureView)>,
    no_anim: (wgpu::TextureView, wgpu::TextureView),
}

impl Fsr2 {
    pub fn new(gpu: &Gpu, cars_size: usize) -> Fsr2 {
        use wgpu::TextureFormat::*;
        let device = &gpu.device;
        let (u, c) = ((0, B::Uniform), (1, B::Uniform));
        let tf = |b| (b, B::Tex { filterable: true });
        let tu = |b| (b, B::Tex { filterable: false });
        let motion =
            compute(gpu, "motion", &[u, c, (2, B::Depth), (3, B::Store(Rgba16Float)), tu(4), tu(5), (6, B::Depth)]);
        let reconstruct = compute(
            gpu,
            "reconstruct",
            &[
                u,
                (2, B::Depth),
                tu(3),
                tu(4),
                (5, B::Store(Rgba16Float)),
                (6, B::Store(R32Float)),
                (7, B::Store(R32Float)),
                (8, B::Buf { read_only: false }),
            ],
        );
        let luma = compute(gpu, "luma", &[u, tu(2), (3, B::Store(Rgba16Float))]);
        let snapshot = compute(gpu, "snapshot", &[u, tu(2), (3, B::Store(Rgba16Float))]);
        let composition = compute(gpu, "composition", &[u, tu(2), tu(3), (4, B::Store(Rgba16Float))]);
        let depth_clip = compute(
            gpu,
            "depth_clip",
            &[
                u,
                (2, B::Depth),
                tu(3),
                tu(5),
                tu(6),
                tu(7),
                tu(8),
                (9, B::Buf { read_only: true }),
                (10, B::Store(Rgba16Float)),
                (11, B::Store(Rgba8Unorm)),
            ],
        );
        let lock = compute(gpu, "lock", &[u, tu(2), (3, B::Buf { read_only: false })]);
        let accumulate = compute(
            gpu,
            "accumulate",
            &[
                u,
                tf(2),
                tf(3),
                tu(4),
                tf(5),
                tf(6),
                tf(7),
                tf(8),
                (9, B::Sampler),
                (10, B::Buf { read_only: false }),
                (11, B::Store(Rgba16Float)),
                (12, B::Store(Rgba16Float)),
                (13, B::Store(Rgba8Unorm)),
                (14, B::Store(Rgba16Float)),
            ],
        );
        let out_layout = layout(
            gpu,
            "fsr2 output",
            wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
            &[u, tf(2), (3, B::Depth)],
        );
        let out_shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("fsr2 output"),
            source: wgpu::ShaderSource::Wgsl(pass_source("output").into()),
        });
        let out_pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("fsr2 output"),
            bind_group_layouts: &[Some(&out_layout)],
            immediate_size: 0,
        });
        let uniforms = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("fsr2"),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let snap_uniforms = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("fsr2 snapshot"),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let cars = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("fsr2 cars"),
                size: cars_size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fsr2 linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let stand_in = |label: &str, format: wgpu::TextureFormat| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let no_anim = (
            stand_in("fsr2 no animation mask", Rg8Unorm),
            stand_in("fsr2 no animation depth", crate::render::DEPTH_FORMAT),
        );
        Fsr2 {
            motion,
            reconstruct,
            luma,
            snapshot,
            composition,
            depth_clip,
            lock,
            accumulate,
            out_layout,
            out_pipeline_layout,
            out_shader,
            outputs: Vec::new(),
            uniforms,
            snap_uniforms,
            cars,
            sampler,
            targets: None,
            current: 0,
            composed: false,
            anim: None,
            no_anim,
        }
    }

    fn ensure(
        &mut self,
        gpu: &Gpu,
        render: (u32, u32),
        display: (u32, u32),
        encoder: &mut wgpu::CommandEncoder,
    ) -> bool {
        if self.targets.as_ref().is_some_and(|t| t.render == render && t.display == display) {
            return false;
        }
        let t = Targets::new(gpu, render, display);
        encoder.clear_buffer(&t.new_locks, 0, None);
        self.targets = Some(t);
        true
    }

    fn snap_uniforms(&self, gpu: &Gpu, render: (u32, u32)) {
        let mut u: Uniforms = bytemuck::Zeroable::zeroed();
        u.render = [render.0 as f32, render.1 as f32, 1.0 / render.0.max(1) as f32, 1.0 / render.1.max(1) as f32];
        u.misc[1] = crate::render::debug_env::nfsu2_fsr2_tc_gain().unwrap_or(COMPOSITION_GAIN);
        // Its own buffer: the frame's uniforms are written later for the
        // same submission (queue writes land before the encoder runs).
        gpu.queue.write_buffer(&self.snap_uniforms, 0, bytemuck::bytes_of(&u));
    }

    fn dispatch(encoder: &mut wgpu::CommandEncoder, label: &str, c: &Compute, g: &wgpu::BindGroup, render: (u32, u32)) {
        let mut p =
            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(label), timestamp_writes: None });
        p.set_pipeline(&c.pipeline);
        p.set_bind_group(0, g, &[]);
        p.dispatch_workgroups(render.0.div_ceil(8), render.1.div_ceil(8), 1);
    }

    /// The scene `color` (`render` big) before SSR is drawn over it
    /// ([`Fsr2::composite`] after).
    pub fn snapshot(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        render: (u32, u32),
        display: (u32, u32),
    ) {
        self.ensure(gpu, render, display, encoder);
        self.snap_uniforms(gpu, render);
        let t = self.targets.as_ref().unwrap();
        let g = group(
            gpu,
            &self.snapshot.layout,
            &[
                (0, self.snap_uniforms.as_entire_binding()),
                (2, wgpu::BindingResource::TextureView(color)),
                (3, wgpu::BindingResource::TextureView(&t.opaque)),
            ],
        );
        Self::dispatch(encoder, "fsr2 snapshot", &self.snapshot, &g, render);
    }

    /// After SSR: the composition mask, what it changed against the
    /// snapshot.
    pub fn composite(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        render: (u32, u32),
    ) {
        let Some(t) = self.targets.as_ref().filter(|t| t.render == render) else { return };
        let g = group(
            gpu,
            &self.composition.layout,
            &[
                (0, self.snap_uniforms.as_entire_binding()),
                (2, wgpu::BindingResource::TextureView(color)),
                (3, wgpu::BindingResource::TextureView(&t.opaque)),
                (4, wgpu::BindingResource::TextureView(&t.composition)),
            ],
        );
        Self::dispatch(encoder, "fsr2 composition", &self.composition, &g, render);
        self.composed = true;
    }

    /// This frame's animation mask (`WorldRenderer::anim_mask`: R
    /// animated, G ripples, and its depth), render size; None: none.
    pub fn set_anim_mask(&mut self, mask: Option<(wgpu::TextureView, wgpu::TextureView)>) {
        self.anim = mask;
    }

    /// One frame of FSR 2: the jittered scene `f.color` / `f.depth`
    /// (render size) with the history into `f.out` (display size).
    pub fn run(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, f: &Frame) {
        let render = (f.uniforms.render[0] as u32, f.uniforms.render[1] as u32);
        let display = (f.uniforms.display[0] as u32, f.uniforms.display[1] as u32);
        let fresh = self.ensure(gpu, render, display, encoder);
        let mut u = f.uniforms;
        if fresh || !f.valid {
            u.jitter[2] = 1.0;
        }
        let t = self.targets.as_ref().unwrap();
        u.misc = [
            debug_view(),
            crate::render::debug_env::nfsu2_fsr2_tc_gain().unwrap_or(COMPOSITION_GAIN),
            crate::render::debug_env::nfsu2_fsr2_wheel().unwrap_or(WHEEL_REACTIVE),
            luma_block() as f32,
        ];
        u.luma = [t.luma_size.0 as f32, t.luma_size.1 as f32, 1.0 / t.luma_size.0 as f32, 1.0 / t.luma_size.1 as f32];
        u.extra = [
            if self.composed { 1.0 } else { 0.0 },
            crate::render::debug_env::nfsu2_fsr2_locks().unwrap_or(if LOCKS { 1.0 } else { 0.0 }),
            crate::render::debug_env::nfsu2_fsr2_box().unwrap_or(BOX_SCALE_MAX),
            crate::render::debug_env::nfsu2_fsr2_auto().unwrap_or(AUTO_REACTIVE),
        ];
        self.composed = false;
        let anim = self.anim.take();
        u.anim = [if anim.is_some() { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0];
        let (anim_color, anim_depth) = anim.as_ref().map_or((&self.no_anim.0, &self.no_anim.1), |(c, d)| (c, d));
        gpu.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        gpu.queue.write_buffer(&self.cars, 0, f.cars);
        let (w, r) = (self.current, 1 - self.current);
        let ub = self.uniforms.as_entire_binding();
        let tv = wgpu::BindingResource::TextureView;
        // the reconstructed previous depth cleared to the far plane (0)
        encoder.clear_buffer(&t.reconstructed, 0, None);
        let groups_r = (render.0.div_ceil(8), render.1.div_ceil(8));
        let groups_d = (display.0.div_ceil(8), display.1.div_ceil(8));
        let g_motion = group(
            gpu,
            &self.motion.layout,
            &[
                (0, ub.clone()),
                (1, self.cars.as_entire_binding()),
                (2, tv(f.depth)),
                (3, tv(&t.motion)),
                (4, tv(&t.composition)),
                (5, tv(anim_color)),
                (6, tv(anim_depth)),
            ],
        );
        let g_reconstruct = group(
            gpu,
            &self.reconstruct.layout,
            &[
                (0, ub.clone()),
                (2, tv(f.depth)),
                (3, tv(f.color)),
                (4, tv(&t.motion)),
                (5, tv(&t.dilated_motion[w])),
                (6, tv(&t.dilated_depth)),
                (7, tv(&t.lock_luma)),
                (8, t.reconstructed.as_entire_binding()),
            ],
        );
        let g_luma = group(gpu, &self.luma.layout, &[(0, ub.clone()), (2, tv(f.color)), (3, tv(&t.luma))]);
        let g_clip = group(
            gpu,
            &self.depth_clip.layout,
            &[
                (0, ub.clone()),
                (2, tv(f.depth)),
                (3, tv(f.color)),
                (5, tv(&t.motion)),
                (6, tv(&t.dilated_motion[w])),
                (7, tv(&t.dilated_motion[r])),
                (8, tv(&t.dilated_depth)),
                (9, t.reconstructed.as_entire_binding()),
                (10, tv(&t.prepared)),
                (11, tv(&t.masks)),
            ],
        );
        let g_lock = group(
            gpu,
            &self.lock.layout,
            &[(0, ub.clone()), (2, tv(&t.lock_luma)), (3, t.new_locks.as_entire_binding())],
        );
        let g_acc = group(
            gpu,
            &self.accumulate.layout,
            &[
                (0, ub.clone()),
                (2, tv(&t.prepared)),
                (3, tv(&t.masks)),
                (4, tv(&t.dilated_motion[w])),
                (5, tv(&t.history[r])),
                (6, tv(&t.lock_status[r])),
                (7, tv(&t.luma_history[r])),
                (8, tv(&t.luma)),
                (9, wgpu::BindingResource::Sampler(&self.sampler)),
                (10, t.new_locks.as_entire_binding()),
                (11, tv(&t.history[w])),
                (12, tv(&t.lock_status[w])),
                (13, tv(&t.luma_history[w])),
                (14, tv(&t.output)),
            ],
        );
        // One compute pass each, the GPU profile's marks between them.
        let run = |encoder: &mut wgpu::CommandEncoder,
                   c: &Compute,
                   g: &wgpu::BindGroup,
                   n: (u32, u32),
                   mark: &'static str| {
            let mut p =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(mark), timestamp_writes: None });
            p.set_pipeline(&c.pipeline);
            p.set_bind_group(0, g, &[]);
            p.dispatch_workgroups(n.0, n.1, 1);
            drop(p);
            crate::prof::gpu::mark(encoder, mark);
        };
        run(encoder, &self.motion, &g_motion, groups_r, "graphics: fsr2 motion");
        run(encoder, &self.reconstruct, &g_reconstruct, groups_r, "graphics: fsr2 reconstruct");
        run(encoder, &self.luma, &g_luma, (t.luma_size.0, t.luma_size.1), "graphics: fsr2 luma");
        run(encoder, &self.depth_clip, &g_clip, groups_r, "graphics: fsr2 depth clip");
        run(encoder, &self.lock, &g_lock, groups_r, "graphics: fsr2 lock");
        run(encoder, &self.accumulate, &g_acc, groups_d, "graphics: fsr2 accumulate");
        // The output into the scene: RCAS, or a copy.
        // NFSU2_FSR2_SHARPEN ≥ 9 (stops): no RCAS (a copy).
        let rcas = crate::render::debug_env::nfsu2_fsr2_sharpen().unwrap_or(SHARPNESS_STOPS) < 9.0;
        let out_group = group(gpu, &self.out_layout, &[(0, ub.clone()), (2, tv(&t.output)), (3, tv(f.depth))]);
        // The colour, then (with `out_depth`) the display-size depth: two
        // passes (one with both measured slower: 0.43 against 0.35 ms at
        // 4K, the shader-written depth).
        let pipeline = self.output_pipeline(gpu, Some(f.out_format), rcas);
        let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fsr2 output"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: f.out,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_pipeline(pipeline);
        p.set_bind_group(0, &out_group, &[]);
        p.draw(0..3, 0..1);
        drop(p);
        if let Some(d) = f.out_depth {
            let pipeline = self.output_pipeline(gpu, None, false);
            let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fsr2 depth"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: d,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            p.set_pipeline(pipeline);
            p.set_bind_group(0, &out_group, &[]);
            p.draw(0..3, 0..1);
        }
        crate::prof::gpu::mark(encoder, "graphics: fsr2 output");
        self.current = 1 - self.current;
    }

    /// The output pipelines a frame into a scene of `format` makes on its
    /// first run (the colour with RCAS or the copy, and the display-size
    /// depth), made now (docs/developing/first-use-builds.md); the passes' own are
    /// made by [`Fsr2::new`].
    pub fn warm(&mut self, gpu: &Gpu, format: wgpu::TextureFormat) {
        let rcas = crate::render::debug_env::nfsu2_fsr2_sharpen().unwrap_or(SHARPNESS_STOPS) < 9.0;
        self.output_pipeline(gpu, Some(format), rcas);
        self.output_pipeline(gpu, None, false);
    }

    /// The output pipeline into `format` (None: the depth alone).
    fn output_pipeline(&mut self, gpu: &Gpu, format: Option<wgpu::TextureFormat>, rcas: bool) -> &wgpu::RenderPipeline {
        if let Some(i) = self.outputs.iter().position(|(f, r, _)| *f == format && *r == rcas) {
            return &self.outputs[i].2;
        }
        let entry = match (format, rcas) {
            (None, _) => "fs_depth",
            (Some(_), true) => "fs_rcas",
            (Some(_), false) => "fs_copy",
        };
        let targets =
            [format.map(|format| wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let p = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(&self.out_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.out_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.out_shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: if format.is_some() { &targets } else { &[] },
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: format.is_none().then(|| wgpu::DepthStencilState {
                format: crate::render::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        self.outputs.push((format, rcas, p));
        &self.outputs.last().unwrap().2
    }
}

fn group(gpu: &Gpu, layout: &wgpu::BindGroupLayout, entries: &[(u32, wgpu::BindingResource)]) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry> =
        entries.iter().map(|(b, r)| wgpu::BindGroupEntry { binding: *b, resource: r.clone() }).collect();
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("fsr2"), layout, entries: &entries })
}

impl crate::render::temporal::Temporal {
    /// The scene `color` (`input` big) before SSR is drawn over it
    /// this frame (FSR 2's composition mask, [`Temporal::fsr2_composite`]
    /// after it).
    pub fn fsr2_snapshot(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        input: (u32, u32),
        output: (u32, u32),
    ) {
        self.fsr2
            .get_or_insert_with(|| Fsr2::new(gpu, std::mem::size_of::<GpuCar>() * MAX_CARS))
            .snapshot(gpu, encoder, color, input, output);
    }

    /// This frame's animation mask for FSR 2
    /// (`WorldRenderer::anim_mask`).
    pub fn fsr2_anim_mask(&mut self, gpu: &Gpu, mask: Option<(wgpu::TextureView, wgpu::TextureView)>) {
        if mask.is_some() || self.fsr2.is_some() {
            self.fsr2
                .get_or_insert_with(|| Fsr2::new(gpu, std::mem::size_of::<GpuCar>() * MAX_CARS))
                .set_anim_mask(mask);
        }
    }

    /// After SSR, the composition mask from the snapshot.
    pub fn fsr2_composite(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        input: (u32, u32),
    ) {
        if let Some(f) = self.fsr2.as_mut() {
            f.composite(gpu, encoder, color, input);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quality modes' render sizes, phases and LOD bias are FSR 2's
    /// (ffx_fsr2.cpp): 4K Quality 2560 × 1440 with 18 phases, Balanced
    /// 2258 × 1270 (23), Performance 1920 × 1080 (32), Ultra Performance
    /// 1280 × 720 (72); Native AA keeps the size (8).
    #[test]
    fn modes_sizes_phases_and_bias() {
        let k4 = (3840, 2160);
        let sizes: Vec<_> = MODES.iter().map(|m| render_size(k4, m.2)).collect();
        assert_eq!(sizes, vec![(3840, 2160), (2560, 1440), (2258, 1270), (1920, 1080), (1280, 720)]);
        let phases: Vec<_> = sizes.iter().map(|s| phase_count(s.0, k4.0)).collect();
        assert_eq!(phases, vec![8, 18, 23, 32, 72]);
        assert_eq!(MODES.map(|m| mip_bias_q(m.2)), [-4, -6, -7, -8, -10]);
        assert_eq!(ratio(8), Some(1.5));
        assert_eq!(ratio(3), None);
    }

    /// The depth factors turn device depth back into view distance, both
    /// depth directions (FSR 2's GetViewSpaceDepth).
    #[test]
    fn depth_factors_give_view_distance() {
        let mut c = crate::render::camera::Camera::new(glam::Vec3::ZERO, 0.0, 0.0);
        c.near = 0.5;
        c.far = 3000.0;
        for reversed in [false, true] {
            let p = c.projection_depth(16.0 / 9.0, reversed);
            let f = depth_factors(p);
            for z in [0.6f32, 3.0, 40.0, 900.0] {
                let clip = p * glam::Vec4::new(0.0, 0.0, -z, 1.0);
                let d = clip.z / clip.w;
                let back = f[1] / (d - f[0]);
                assert!((back - z).abs() < z * 1e-3, "{reversed} {z}: {back}");
            }
        }
    }

    /// Every pass parses and validates as WGSL (common part prepended).
    #[test]
    fn passes_are_valid_wgsl() {
        for name in
            ["motion", "reconstruct", "luma", "snapshot", "composition", "depth_clip", "lock", "accumulate", "output"]
        {
            let src = pass_source(name);
            let module = wgpu::naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
    }
}
