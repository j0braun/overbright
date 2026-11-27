//! The temporal pass (docs/notes/graphics/temporal.md; a **clone deviation**, opt-in,
//! nothing like it in SPEED2.EXE): TAA (the Anti-Aliasing row's "TAA") and
//! the temporal upscaler (the Upscaling row) in one resolve, temporal.wgsl.
//!
//! * **Jitter**: the main view's projection moves by a Halton (2, 3)
//!   sub-pixel offset each drawn frame ([`Camera::jitter`]); 8 phases, more
//!   when upscaling (8 × the pixel ratio², ≤ 32). Texture LOD bias is not
//!   changed (the world's samplers are shared with the native path).
//! * **Motion**: no velocity buffer. Each output pixel takes the closest
//!   surface of its input neighbourhood (a cross of five), reconstructs its position
//!   from the depth (camera-relative, so f32 holds across Bayview), and
//!   reprojects it with last frame's view; a point inside a drawn car's
//!   box moves with that car's pose instead (rigid, drawn pose to drawn
//!   pose, i.e. render frame to render frame with the interpolated
//!   poses); points in a wheel are reactive (less history).
//! * **History**: Rgba16Float at the output size (colour and the scene's
//!   alpha, the bleach mask), Catmull-Rom sampled, clipped to the
//!   neighbourhood's YCoCg box, reset on cuts ([`is_cut`]) and whenever a
//!   frame is drawn without the pass.
//! * Glow sprites, light streaks, particles (nitrous flames, smoke, rain)
//!   and the HUD are drawn after the pass, at the output's size and
//!   without the jitter ([`late_viewport`]).

use crate::render::Gpu;
use crate::render::camera::Camera;
use glam::{Mat4, Vec2, Vec3};
use std::collections::HashMap;

mod compute;
pub use compute::car_rect;

pub const HISTORY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The most cars with their own motion (the nearest).
pub const MAX_CARS: usize = 32;
/// The jitter's phases at 1×.
pub const PHASES: u32 = 8;
/// A camera move this far (m) in one drawn frame is a cut.
pub const CUT_DISTANCE: f32 = 12.0;
/// A turn of the view this large (radians) in one drawn frame is a cut.
pub const CUT_ANGLE: f32 = 0.6;
/// History weight of the current frame (TAA; upscaling keeps a little
/// more history, its frames hold fewer samples each).
pub const BLEND_TAA: f32 = 0.1;
pub const BLEND_UPSCALE: f32 = 0.08;
/// More of the current frame per pixel of motion (history resampled each
/// frame softens), up to [`VELOCITY_BLEND_MAX`].
pub const VELOCITY_BLEND: f32 = 0.2;
pub const VELOCITY_BLEND_MAX: f32 = 0.3;

/// The texture LOD bias under the temporal pass, quarter mip levels: the
/// render share's log2 (Upscaling: textures as sharp as at the screen's
/// size) less half a level (the jitter's accumulation softens; FSR 2
/// recommends a whole level, half was kept against shimmer on the
/// original's high-contrast textures). TAA −0.5, Quality −1.0, Balanced
/// −1.25, Performance −1.5.
pub fn mip_bias_q(opts: &crate::render::RenderOptions) -> i8 {
    // FSR 2's own bias (log2(render / display) − 1).
    if let Some(r) = crate::render::fsr2::ratio(opts.upscale) {
        return crate::render::fsr2::mip_bias_q(r);
    }
    bias_for(crate::render::RenderOptions::upscale_share(opts.upscale).unwrap_or(1.0))
}

/// [`mip_bias_q`] for a render share of each side.
pub fn bias_for(share: f64) -> i8 {
    ((share.log2() - 0.5) * 4.0).round() as i8
}

/// A world / car module with its main texture sampled at the temporal
/// pass's LOD bias (pipeline constant `MIP_BIAS`): world.wgsl's
/// `sample_tex` (texture or array form) and car.wgsl's diffuse. The native
/// modules are never touched (byte-identical Original).
pub fn biased_source(source: &str) -> String {
    let s = source
        .replace("textureSample(tex, samp, uv, imm.layer)", "textureSampleBias(tex, samp, uv, imm.layer, MIP_BIAS)")
        .replace(
            "textureSample(tex[imm.array], samp, uv, imm.layer)",
            "textureSampleBias(tex[imm.array], samp, uv, imm.layer, MIP_BIAS)",
        )
        .replace("textureSample(tex, samp, uv)", "textureSampleBias(tex, samp, uv, MIP_BIAS)")
        .replace(
            "textureSample(diffuse_tex, diffuse_sampler, in.uv)",
            "textureSampleBias(diffuse_tex, diffuse_sampler, in.uv, MIP_BIAS)",
        );
    format!("override MIP_BIAS: f32 = 0.0;\n{s}")
}

/// The radical inverse of `i` in `base` (Halton), in [0, 1).
pub fn halton(mut i: u32, base: u32) -> f32 {
    let mut f = 1.0f32;
    let mut r = 0.0f32;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// The number of jitter phases for an input `input` big shown `output`
/// big: 8 at 1×, 8 × the pixel ratio when upscaling (≤ 32).
pub fn phases(input: (u32, u32), output: (u32, u32)) -> u32 {
    let ratio = (output.0 as f32 * output.1 as f32) / (input.0.max(1) as f32 * input.1.max(1) as f32);
    ((PHASES as f32 * ratio.max(1.0)).ceil() as u32).clamp(PHASES, 32)
}

/// Frame `frame`'s jitter in input pixels (x right, y down), in
/// (−0.5, 0.5): Halton (2, 3) from index 1, centred.
pub fn jitter_px(frame: u64, phases: u32) -> Vec2 {
    let i = (frame % u64::from(phases.max(1))) as u32 + 1;
    Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

/// A pixel jitter as the projection's clip-space offset ([`Camera::jitter`]).
pub fn jitter_ndc(px: Vec2, size: (u32, u32)) -> Vec2 {
    Vec2::new(2.0 * px.x / size.0.max(1) as f32, -2.0 * px.y / size.1.max(1) as f32)
}

/// The viewport (x, y, w, h) that draws a jittered view's geometry back at
/// its unjittered place in a target `target` big (the late sprites): the
/// jitter in the target's pixels taken off.
pub fn late_viewport(jitter: Vec2, input: (u32, u32), target: (u32, u32)) -> [f32; 4] {
    let sx = target.0 as f32 / input.0.max(1) as f32;
    let sy = target.1 as f32 / input.1.max(1) as f32;
    [-jitter.x * sx, -jitter.y * sy, target.0 as f32, target.1 as f32]
}

/// Whether the view from `prev` to `cur` is a cut (history dropped): a
/// jump, a sharp turn, another kind of camera or a field of view change.
pub fn is_cut(prev: &Camera, cur: &Camera) -> bool {
    let fov = |c: &Camera| c.game_fov.unwrap_or(c.fov_y);
    prev.position.distance(cur.position) > CUT_DISTANCE
        || prev.forward().dot(cur.forward()) < CUT_ANGLE.cos()
        || prev.game_fov.is_some() != cur.game_fov.is_some()
        || (fov(prev) / fov(cur).max(1e-4) - 1.0).abs() > 0.15
}

/// A drawn car's motion inputs: its key (the same car in the next frame),
/// its drawn transform (car → world), its box and wheels in car space.
#[derive(Clone, Copy, Debug)]
pub struct Car {
    pub key: u64,
    pub transform: Mat4,
    pub lo: Vec3,
    pub hi: Vec3,
    /// Wheel centres (car space), radii, half widths.
    pub wheels: [(Vec3, f32, f32); 4],
}

impl Car {
    /// From a car's shadow box (car space) and wheel table: the box made
    /// to hold the body and wheels (inferred margins: 0.15 m around, up to
    /// 1.7 m high), so its pixels move with it.
    pub fn new(key: u64, transform: Mat4, shadow_box: (Vec3, Vec3), wheels: [(Vec3, f32, f32); 4]) -> Car {
        let mut lo = shadow_box.0.min(shadow_box.1);
        let mut hi = shadow_box.0.max(shadow_box.1);
        for (c, r, hw) in wheels {
            lo = lo.min(c - Vec3::new(r, hw, r));
            hi = hi.max(c + Vec3::new(r, hw, r));
        }
        let pad = Vec3::splat(0.15);
        lo -= pad;
        hi += pad;
        // The floor just over the tyres' contact: the road under the car
        // keeps the camera's motion.
        let ground = wheels.iter().map(|(c, r, _)| c.z - r).fold(f32::INFINITY, f32::min);
        if ground.is_finite() {
            lo.z = lo.z.max(ground + 0.06);
        }
        hi.z = hi.z.max(lo.z + 1.7);
        Car { key, transform, lo, hi, wheels }
    }
}

/// What the pass needs of a drawn frame.
#[derive(Clone, Debug)]
pub struct FrameInput {
    /// The main view's camera as drawn, without the jitter.
    pub camera: Camera,
    pub aspect: f32,
    /// This frame's jitter (input px, [`jitter_px`]).
    pub jitter: Vec2,
    pub reversed: bool,
    pub cars: Vec<Car>,
    /// A cut the game knows of (a load, a teleport).
    pub cut: bool,
    /// What draws the view (mode, camera mover, scene): history is kept
    /// only while it stays the same (menu ↔ world, camera changes, NIS).
    pub key: u64,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuCar {
    inv: [[f32; 4]; 4],
    motion: [[f32; 4]; 4],
    lo: [f32; 4],
    hi: [f32; 4],
    wheel: [[f32; 4]; 4],
    wheel_hw: [f32; 4],
    sphere: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    clip_to_rel: [[f32; 4]; 4],
    prev_rel_to_clip: [[f32; 4]; 4],
    input: [f32; 4],
    output: [f32; 4],
    jitter: [f32; 4],
    params: [f32; 4],
    debug: [f32; 4],
    /// X the frame's span in ticks (`render::enh::tick_views`).
    rate: [f32; 4],
}

/// The view as the pass reads it: rotation-only view and projections.
fn rotation_view(c: &Camera) -> Mat4 {
    Mat4::look_to_rh(Vec3::ZERO, c.forward(), c.up)
}

/// The last frame the pass ran: its unjittered camera and car poses.
#[derive(Clone, Debug)]
struct Last {
    key: u64,
    camera: Camera,
    aspect: f32,
    reversed: bool,
    cars: HashMap<u64, Mat4>,
}

/// The matrices of a frame against the last (camera-relative to this
/// frame's eye): jittered clip → relative world, relative world → last
/// clip, and each car's motion. Exposed for the tests.
pub struct Reprojection {
    pub clip_to_rel: Mat4,
    pub prev_rel_to_clip: Mat4,
    /// (inverse of the car's relative transform, its motion, fresh)
    pub cars: Vec<(Mat4, Mat4, bool)>,
}

/// [`Reprojection`] of `input` (its jitter `ndc` in clip space) against
/// the last frame (`None`: no history, the view against itself).
pub fn reprojection(input: &FrameInput, ndc: Vec2, last: Option<(&Camera, f32, &HashMap<u64, Mat4>)>) -> Reprojection {
    let cam = &input.camera;
    let p_now = projection_of(cam, input.aspect, input.reversed, ndc);
    let clip_to_rel = (p_now * rotation_view(cam)).inverse();
    let (prev_cam, prev_aspect, prev_cars) = match last {
        Some((c, a, cars)) => (*c, a, Some(cars)),
        None => (*cam, input.aspect, None),
    };
    let p_prev = projection_of(&prev_cam, prev_aspect, input.reversed, Vec2::ZERO);
    let prev_rel_to_clip = p_prev * rotation_view(&prev_cam) * Mat4::from_translation(cam.position - prev_cam.position);
    let to_rel = Mat4::from_translation(-cam.position);
    let cars = input
        .cars
        .iter()
        .map(|c| {
            let now_rel = to_rel * c.transform;
            let inv = now_rel.inverse();
            match prev_cars
                .and_then(|m| m.get(&c.key))
                .filter(|p| p.w_axis.truncate().distance(c.transform.w_axis.truncate()) < CUT_DISTANCE)
            {
                Some(prev) => (inv, to_rel * *prev * inv, false),
                None => (inv, Mat4::IDENTITY, prev_cars.is_some()),
            }
        })
        .collect();
    Reprojection { clip_to_rel, prev_rel_to_clip, cars }
}

/// The projection of `c` at `aspect` with a clip-space jitter `ndc`.
fn projection_of(c: &Camera, aspect: f32, reversed: bool, ndc: Vec2) -> Mat4 {
    let mut j = *c;
    j.jitter = ndc;
    j.projection_depth(aspect, reversed)
}

/// History and pipelines of the temporal pass.
pub struct Temporal {
    layout: wgpu::BindGroupLayout,
    resolve: wgpu::RenderPipeline,
    copies: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    mrt: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    depth: wgpu::RenderPipeline,
    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    /// TAA's resolve as a compute pass where the device gives the same
    /// history with it (`compute`); None: the fragment pass.
    compute: Option<compute::Compute>,
    uniforms: wgpu::Buffer,
    cars: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// The two history images, their size and whether the compute pass
    /// writes them (storage binding).
    history: Option<(((u32, u32), bool), [wgpu::TextureView; 2])>,
    /// Which history the next frame writes.
    current: usize,
    last: Option<Last>,
    /// Whether the history holds the last drawn frame.
    valid: bool,
    /// Drawn frames with the pass (the jitter's index).
    pub frame: u64,
    /// The drawn frames' place on the tick axis.
    span: crate::render::enh::tick_views::FrameSpan,
    /// docs/notes/graphics/fsr2.md: FSR 2's passes, run in place of the resolve
    /// when an FSR 2 mode is chosen.
    pub(crate) fsr2: Option<crate::render::fsr2::Fsr2>,
}

impl Temporal {
    pub fn new(gpu: &Gpu) -> Temporal {
        let device = &gpu.device;
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("temporal"),
            entries: &compute::input_entries(wgpu::ShaderStages::FRAGMENT),
        });
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("temporal"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/temporal.wgsl")).into(),
            ),
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("temporal"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let resolve = color_pipeline(gpu, &pipeline_layout, &shader, "fs_resolve", HISTORY_FORMAT);
        let depth = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("temporal depth"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_depth"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
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
        let uniforms = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("temporal"),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let cars = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("temporal cars"),
                size: (std::mem::size_of::<GpuCar>() * MAX_CARS) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("temporal history"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let compute = compute::Compute::new(gpu, &resolve, &layout, &sampler);
        Temporal {
            layout,
            compute,
            resolve,
            copies: HashMap::new(),
            mrt: HashMap::new(),
            depth,
            shader,
            pipeline_layout,
            uniforms,
            cars,
            sampler,
            history: None,
            current: 0,
            last: None,
            valid: false,
            frame: 0,
            span: Default::default(),
            fsr2: None,
        }
    }

    /// The programs of a frame into a scene of `format` made ahead
    /// (docs/developing/first-use-builds.md): the copy to the scene (TAA) and the
    /// two-target resolve (Upscaling).
    pub fn warm(&mut self, gpu: &Gpu, format: wgpu::TextureFormat) {
        if !self.copies.contains_key(&format) {
            let p = color_pipeline(gpu, &self.pipeline_layout, &self.shader, "fs_copy", format);
            self.copies.insert(format, p);
        }
        if !self.mrt.contains_key(&format) {
            let p = mrt_pipeline(gpu, &self.pipeline_layout, &self.shader, format);
            self.mrt.insert(format, p);
        }
    }

    /// FSR 2's programs (its passes and its output into a scene of
    /// `format`) made ahead, as the temporal frame's first FSR 2 run makes
    /// them (docs/developing/first-use-builds.md).
    pub fn warm_fsr2(gpu: &Gpu, format: wgpu::TextureFormat) {
        crate::render::fsr2::Fsr2::new(gpu, std::mem::size_of::<GpuCar>() * MAX_CARS).warm(gpu, format);
    }

    /// Whether TAA's resolve runs as the compute pass on this device.
    pub fn compute_pass(&self) -> bool {
        self.compute.is_some()
    }

    /// Drop the history: the next frame starts afresh (a cut, a frame
    /// drawn without the pass).
    pub fn reset(&mut self) {
        self.valid = false;
        self.last = None;
    }

    /// Whether the history holds the last frame.
    pub fn valid(&self) -> bool {
        self.valid
    }

    /// This frame's jitter (input pixels) for an input `input` big shown
    /// `output` big. No jitter while there is no history to spread it
    /// over (a single still is the plain image).
    pub fn next_jitter(&self, input: (u32, u32), output: (u32, u32), opts: &crate::render::RenderOptions) -> Vec2 {
        if opts.fsr2 {
            return jitter_px(self.frame, crate::render::fsr2::phase_count(input.0, output.0));
        }
        jitter_px(self.frame, phases(input, output))
    }

    /// Resolve `color` / `depth` (the jittered scene, `input` big) with
    /// the history into `out` (`output` big, the post passes' scene); with
    /// `out_depth` (upscaling) the input's depth is written there at the
    /// output's size for the late sprites.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        encoder: &mut wgpu::CommandEncoder,
        frame: &FrameInput,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        input: (u32, u32),
        out: &wgpu::TextureView,
        out_format: wgpu::TextureFormat,
        output: (u32, u32),
        out_depth: Option<&wgpu::TextureView>,
    ) {
        let fsr2 = opts.fsr2;
        // TAA's resolve as the compute pass (Upscaling keeps the fragment
        // pass: its second target is the scene, not a storage texture).
        let compute_on = self.compute.is_some() && input == output;
        let key = (output, compute_on);
        if fsr2 {
            // FSR 2 keeps its own history (fsr2.rs); the TAA history is dropped.
            self.history = None;
        } else if self.history.as_ref().is_none_or(|(k, _)| *k != key) {
            let make = || {
                gpu.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("temporal history"),
                        size: wgpu::Extent3d {
                            width: output.0.max(1),
                            height: output.1.max(1),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: HISTORY_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | if key.1 {
                                wgpu::TextureUsages::STORAGE_BINDING
                            } else {
                                wgpu::TextureUsages::empty()
                            },
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            self.history = Some((key, [make(), make()]));
            self.reset();
        }
        let last = self.last.as_ref().filter(|l| {
            self.valid
                && !frame.cut
                && l.key == frame.key
                && l.reversed == frame.reversed
                && !is_cut(&l.camera, &frame.camera)
        });
        let valid = last.is_some();
        // The frame's span in ticks: the shader scales the blend to
        // it (1 − (1 − a)^span, the motion per tick), exactly the old blend
        // at a steady 60 Hz.
        let span = self.span.span(crate::render::enh::tick_views::frame_at());
        let r = reprojection(frame, jitter_ndc(frame.jitter, input), last.map(|l| (&l.camera, l.aspect, &l.cars)));
        if debug_view() > 2.5 {
            if let Some(l) = last {
                eprintln!(
                    "taa: eye {:?} -> {:?} fwd {:?} -> {:?} fov {:?}/{:?} jitter {:?}",
                    l.camera.position,
                    frame.camera.position,
                    l.camera.forward(),
                    frame.camera.forward(),
                    l.camera.game_fov,
                    frame.camera.game_fov,
                    frame.jitter
                );
            }
        }
        let upscale = input != output;
        let u = Uniforms {
            clip_to_rel: r.clip_to_rel.to_cols_array_2d(),
            prev_rel_to_clip: r.prev_rel_to_clip.to_cols_array_2d(),
            input: [input.0 as f32, input.1 as f32, 1.0 / input.0.max(1) as f32, 1.0 / input.1.max(1) as f32],
            output: [output.0 as f32, output.1 as f32, 1.0 / output.0.max(1) as f32, 1.0 / output.1.max(1) as f32],
            jitter: [
                frame.jitter.x,
                frame.jitter.y,
                if valid { 1.0 } else { 0.0 },
                if frame.reversed { 1.0 } else { 0.0 },
            ],
            params: [
                if upscale { BLEND_UPSCALE } else { BLEND_TAA },
                visible_cars(frame) as f32,
                output.0 as f32 / input.0.max(1) as f32,
                if upscale { 1.25 } else { 1.0 },
            ],
            debug: [
                debug_view(),
                crate::render::debug_env::nfsu2_taa_vel().unwrap_or(VELOCITY_BLEND),
                crate::render::debug_env::nfsu2_taa_vel_max().unwrap_or(VELOCITY_BLEND_MAX),
                1.0,
            ],
            rate: [span, 0.0, 0.0, 0.0],
        };
        gpu.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let mut cars: Vec<GpuCar> = vec![bytemuck::Zeroable::zeroed(); MAX_CARS];
        // Only cars that can be on screen (their sphere not behind the eye).
        let forward = frame.camera.forward();
        let mut n = 0;
        for (c, (inv, motion, fresh)) in frame.cars.iter().zip(&r.cars) {
            if n == MAX_CARS {
                break;
            }
            let centre = c.transform.transform_point3((c.lo + c.hi) * 0.5) - frame.camera.position;
            let radius = (c.hi - c.lo).length() * 0.5;
            if centre.dot(forward) < -radius {
                continue;
            }
            let i = n;
            n += 1;
            cars[i] = GpuCar {
                inv: inv.to_cols_array_2d(),
                motion: motion.to_cols_array_2d(),
                lo: [c.lo.x, c.lo.y, c.lo.z, if *fresh { 1.0 } else { 0.0 }],
                hi: [c.hi.x, c.hi.y, c.hi.z, 0.0],
                wheel: c.wheels.map(|(p, r, _)| [p.x, p.y, p.z, r]),
                wheel_hw: c.wheels.map(|(_, _, hw)| hw),
                sphere: [centre.x, centre.y, centre.z, radius * radius],
            };
        }
        if fsr2 {
            let projection = projection_of(&frame.camera, frame.aspect, frame.reversed, Vec2::ZERO);
            let f = crate::render::fsr2::Frame {
                uniforms: crate::render::fsr2::Uniforms {
                    clip_to_rel: u.clip_to_rel,
                    prev_rel_to_clip: u.prev_rel_to_clip,
                    render: u.input,
                    display: u.output,
                    jitter: [
                        frame.jitter.x,
                        frame.jitter.y,
                        if valid { 0.0 } else { 1.0 },
                        if frame.reversed { 1.0 } else { 0.0 },
                    ],
                    params: [
                        n as f32,
                        if matches!(out_format, wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float) {
                            1.0
                        } else {
                            0.0
                        },
                        crate::render::fsr2::phase_count(input.0, output.0) as f32,
                        crate::render::fsr::sharpness(
                            crate::render::debug_env::nfsu2_fsr2_sharpen()
                                .unwrap_or(crate::render::fsr2::SHARPNESS_STOPS),
                        ),
                    ],
                    depth: crate::render::fsr2::depth_factors(projection),
                    misc: [0.0; 4],
                    luma: [0.0; 4],
                    extra: [0.0; 4],
                    anim: [0.0; 4],
                },
                cars: bytemuck::cast_slice::<GpuCar, u8>(&cars),
                color,
                depth,
                out,
                out_format,
                out_depth,
                valid,
            };
            self.fsr2
                .get_or_insert_with(|| crate::render::fsr2::Fsr2::new(gpu, std::mem::size_of::<GpuCar>() * MAX_CARS))
                .run(gpu, encoder, &f);
            self.valid = true;
            self.frame = self.frame.wrapping_add(1);
            self.last = Some(Last {
                key: frame.key,
                camera: frame.camera.unjittered(),
                aspect: frame.aspect,
                reversed: frame.reversed,
                cars: frame.cars.iter().map(|c| (c.key, c.transform)).collect(),
            });
            return;
        }
        self.fsr2 = None;
        gpu.queue.write_buffer(&self.cars, 0, bytemuck::cast_slice::<GpuCar, u8>(&cars));
        let (_, h) = self.history.as_ref().unwrap();
        let (write, read) = (h[self.current].clone(), h[1 - self.current].clone());
        let group = |color: &wgpu::TextureView, history: &wgpu::TextureView| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("temporal"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(color) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(history) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 4, resource: self.uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: self.cars.as_entire_binding() },
                ],
            })
        };
        let resolve_group = group(color, &read);
        // The copy's group must not hold the scene it writes (the input, at
        // 1×): the other history stands in for the unused colour.
        let copy_group = group(&read, &write);
        if upscale {
            // The input is the stage's own scene: history and output in
            // one pass.
            if !self.mrt.contains_key(&out_format) {
                let p = mrt_pipeline(gpu, &self.pipeline_layout, &self.shader, out_format);
                self.mrt.insert(out_format, p);
            }
            let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("temporal resolve"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &write,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: out,
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
            p.set_pipeline(&self.mrt[&out_format]);
            p.set_bind_group(0, &resolve_group, &[]);
            p.draw(0..3, 0..1);
            drop(p);
            crate::prof::gpu::mark(encoder, "graphics: temporal resolve");
        } else {
            if let Some(c) = self.compute.as_ref().filter(|_| compute_on) {
                // The cars' screen rectangles, in `cars`' order.
                let to_clip = r.clip_to_rel.inverse();
                let mut rects = [[0i32; 4]; MAX_CARS];
                for (rect, car) in rects.iter_mut().zip(&cars[..n]) {
                    let s = car.sphere;
                    *rect = car_rect(to_clip, Vec3::new(s[0], s[1], s[2]), s[3].sqrt(), input, output, frame.jitter);
                }
                gpu.queue.write_buffer(&c.rects, 0, bytemuck::cast_slice(&rects));
                let inputs = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("temporal compute inputs"),
                    layout: &c.inputs,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(color) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&read) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                        wgpu::BindGroupEntry { binding: 4, resource: self.uniforms.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 5, resource: self.cars.as_entire_binding() },
                    ],
                });
                c.dispatch(encoder, &inputs, &c.outputs_group(gpu, &write), output);
            } else {
                pass(encoder, "temporal resolve", &write, &self.resolve, &resolve_group);
            }
            crate::prof::gpu::mark(encoder, "graphics: temporal resolve");
            if !self.copies.contains_key(&out_format) {
                let p = color_pipeline(gpu, &self.pipeline_layout, &self.shader, "fs_copy", out_format);
                self.copies.insert(out_format, p);
            }
            pass(encoder, "temporal to the scene", out, &self.copies[&out_format], &copy_group);
        }
        if let Some(d) = out_depth {
            let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("temporal depth"),
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
            p.set_pipeline(&self.depth);
            p.set_bind_group(0, &copy_group, &[]);
            p.draw(0..3, 0..1);
        }
        crate::prof::gpu::mark(encoder, "graphics: temporal output");
        self.current = 1 - self.current;
        self.valid = true;
        self.frame = self.frame.wrapping_add(1);
        self.last = Some(Last {
            key: frame.key,
            camera: frame.camera.unjittered(),
            aspect: frame.aspect,
            reversed: frame.reversed,
            cars: frame.cars.iter().map(|c| (c.key, c.transform)).collect(),
        });
    }
}

/// `NFSU2_TAA_DEBUG=1`: the motion (R, G: |x|, |y| px / 10; B: history
/// valid); `=2`: car (R), wheel (G), fresh car (B) over the image.
fn debug_view() -> f32 {
    static V: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *V.get_or_init(|| crate::render::debug_env::nfsu2_taa_debug().unwrap_or(0.0))
}

/// The cars of `frame` that can be on screen (sphere not behind the eye),
/// at most [`MAX_CARS`]: the count the shader loops over.
fn visible_cars(frame: &FrameInput) -> usize {
    let forward = frame.camera.forward();
    frame
        .cars
        .iter()
        .filter(|c| {
            (c.transform.transform_point3((c.lo + c.hi) * 0.5) - frame.camera.position).dot(forward)
                >= -(c.hi - c.lo).length() * 0.5
        })
        .count()
        .min(MAX_CARS)
}

fn mrt_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let target = |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
    crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
        label: Some("fs_resolve_mrt"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_resolve_mrt"),
            compilation_options: Default::default(),
            targets: &[target(HISTORY_FORMAT), target(format)],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: gpu.disk_pipeline_cache(),
    })
}

fn color_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    entry: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: gpu.disk_pipeline_cache(),
    })
}

fn pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    p.set_pipeline(pipeline);
    p.set_bind_group(0, group, &[]);
    p.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera(eye: Vec3, fwd: Vec3) -> Camera {
        let mut c = Camera::new(eye, fwd.x.atan2(fwd.y), fwd.z.asin());
        c.game_fov = Some(1.248_277);
        c
    }

    /// A world point seen this frame (jittered) reprojects to where the
    /// last frame's unjittered view drew it, within a hundredth of a pixel
    /// (1280 × 960), near and far.
    #[test]
    fn camera_motion_reprojects_world_points() {
        let size = (1280u32, 960u32);
        let aspect = size.0 as f32 / size.1 as f32;
        let prev = camera(Vec3::new(-888.29254, -408.7542, 23.44223), Vec3::new(0.6638114, -0.7479, -3.52e-5));
        let mut now = camera(Vec3::new(-888.1559, -408.91153, 23.442448), Vec3::new(0.66418755, -0.747566, -3.37e-5));
        let jitter = Vec2::new(-0.375, 0.2777);
        let input = FrameInput { camera: now, aspect, jitter, reversed: false, cars: Vec::new(), cut: false, key: 0 };
        let r = reprojection(&input, jitter_ndc(jitter, size), Some((&prev, aspect, &HashMap::new())));
        now.jitter = jitter_ndc(jitter, size);
        for d in [3.0f32, 20.0, 80.0, 400.0] {
            let p = now.position + now.forward() * d + now.right() * (d * 0.3) - Vec3::Z * (d * 0.1);
            // this frame's jittered clip and depth
            let c = now.view_projection(aspect) * p.extend(1.0);
            let ndc = c.truncate() / c.w;
            let rel = r.clip_to_rel * ndc.extend(1.0);
            let rel = rel.truncate() / rel.w;
            assert!((rel - (p - now.position)).length() < 0.002 * d, "{d}: {rel:?} vs {:?}", p - now.position);
            let pc = r.prev_rel_to_clip * rel.extend(1.0);
            let want = prev.view_projection(aspect) * p.extend(1.0);
            let px = |c: glam::Vec4| Vec2::new(c.x / c.w * size.0 as f32 * 0.5, c.y / c.w * size.1 as f32 * 0.5);
            assert!((px(pc) - px(want)).length() < 0.05, "{d}: {:?} vs {:?}", px(pc), px(want));
        }
    }

    /// A point on a moving car reprojects to where the car's last pose put
    /// it in the last frame's view (rigid motion), and a car without a
    /// last pose is fresh.
    #[test]
    fn car_motion_follows_the_car() {
        let size = (1280u32, 960u32);
        let aspect = size.0 as f32 / size.1 as f32;
        let prev = camera(Vec3::new(100.0, 50.0, 3.0), Vec3::new(0.0, 1.0, -0.1).normalize());
        let now = camera(Vec3::new(100.0, 50.4, 3.0), Vec3::new(0.02, 1.0, -0.1).normalize());
        let pose = |y: f32, yaw: f32| {
            Mat4::from_rotation_translation(glam::Quat::from_rotation_z(yaw), Vec3::new(100.0, y, 1.5))
        };
        let (m_prev, m_now) = (pose(58.0, 0.3), pose(58.5, 0.32));
        let wheels = [(Vec3::new(1.3, 0.8, -0.2), 0.33, 0.12); 4];
        let car = Car::new(7, m_now, (Vec3::new(-2.0, -0.9, -0.5), Vec3::new(2.0, 0.9, 0.0)), wheels);
        let input = FrameInput {
            camera: now,
            aspect,
            jitter: Vec2::ZERO,
            reversed: false,
            cars: vec![car],
            cut: false,
            key: 0,
        };
        let last: HashMap<u64, Mat4> = [(7u64, m_prev)].into();
        let r = reprojection(&input, Vec2::ZERO, Some((&prev, aspect, &last)));
        let (inv, motion, fresh) = r.cars[0];
        assert!(!fresh);
        let local = Vec3::new(1.0, 0.5, 0.3);
        let rel = m_now.transform_point3(local) - now.position;
        assert!((inv.transform_point3(rel) - local).length() < 1e-3);
        let moved = motion.transform_point3(rel);
        let got = r.prev_rel_to_clip * moved.extend(1.0);
        let want = prev.view_projection(aspect) * m_prev.transform_point3(local).extend(1.0);
        let px = |c: glam::Vec4| Vec2::new(c.x / c.w * size.0 as f32 * 0.5, c.y / c.w * size.1 as f32 * 0.5);
        assert!((px(got) - px(want)).length() < 0.05, "{:?} vs {:?}", px(got), px(want));
        // No last pose: fresh (its pixels take no history).
        let r = reprojection(&input, Vec2::ZERO, Some((&prev, aspect, &HashMap::new())));
        assert!(r.cars[0].2);
    }

    /// The jitter: Halton (2, 3) from index 1, centred, in (−0.5, 0.5),
    /// repeating after its phases, with a mean near zero.
    #[test]
    fn jitter_sequence() {
        assert_eq!(halton(1, 2), 0.5);
        assert_eq!(halton(2, 2), 0.25);
        assert!((halton(1, 3) - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(jitter_px(0, 8), Vec2::new(0.0, 1.0 / 3.0 - 0.5));
        assert_eq!(jitter_px(8, 8), jitter_px(0, 8));
        let mut mean = Vec2::ZERO;
        for i in 0..8 {
            let j = jitter_px(i, 8);
            assert!(j.x.abs() < 0.5 && j.y.abs() < 0.5);
            mean += j / 8.0;
        }
        assert!(mean.length() < 0.07, "{mean:?}");
        // One pixel of jitter is two over the size in clip space, y up.
        assert_eq!(jitter_ndc(Vec2::new(0.5, 0.5), (100, 50)), Vec2::new(0.01, -0.02));
        // The late sprites' viewport takes it off at the target's scale.
        assert_eq!(late_viewport(Vec2::new(0.25, -0.5), (640, 480), (1280, 960)), [-0.5, 1.0, 1280.0, 960.0]);
    }

    /// Phases and the upscaler's sizes.
    #[test]
    fn upscaler_sizes_and_phases() {
        assert_eq!(phases((1920, 1080), (1920, 1080)), 8);
        assert_eq!(phases((960, 540), (1920, 1080)), 32);
        assert_eq!(phases((1286, 724), (1920, 1080)), 18);
        assert_eq!(crate::qol::upscale_size((1920, 1080), crate::qol::upscale_share(1).unwrap()), (1286, 724));
        assert_eq!(crate::qol::upscale_size((1920, 1080), crate::qol::upscale_share(2).unwrap()), (1114, 626));
        assert_eq!(crate::qol::upscale_size((3840, 2160), crate::qol::upscale_share(3).unwrap()), (1920, 1080));
        assert_eq!(crate::qol::upscale_share(0), None);
    }

    /// Cuts drop the history: a jump, a sharp turn, a camera of another
    /// kind, a zoom; an ordinary frame of a chase at speed does not.
    #[test]
    fn cuts_reset_the_history() {
        let a = camera(Vec3::new(0.0, 0.0, 2.0), Vec3::Y);
        let mut b = a;
        b.position.y += 1.5; // 90 m/s at 60 fps
        b.yaw += 0.05;
        assert!(!is_cut(&a, &b));
        let mut jump = a;
        jump.position.x += 20.0;
        assert!(is_cut(&a, &jump));
        let mut turn = a;
        turn.yaw += 1.0;
        assert!(is_cut(&a, &turn));
        let mut free = a;
        free.game_fov = None;
        assert!(is_cut(&a, &free));
        let mut zoom = a;
        zoom.game_fov = Some(0.8);
        assert!(is_cut(&a, &zoom));
    }

    /// The LOD bias by render share, and the biased modules sample through
    /// `MIP_BIAS` where the native ones sample plainly.
    #[test]
    fn mip_bias_and_biased_modules() {
        assert_eq!(bias_for(1.0), -2);
        assert_eq!(bias_for(0.67), -4);
        assert_eq!(bias_for(0.58), -5);
        assert_eq!(bias_for(0.5), -6);
        // The world module's MIP_BIAS form (core::shader's composer).
        let world = crate::render::world_module_source(false, false, false, false, false);
        let b = crate::render::world_module_source(false, false, false, true, false);
        assert!(
            b.starts_with("override MIP_BIAS")
                && b.contains("textureSampleBias(tex, samp, uv, MIP_BIAS)")
                && !world.contains("MIP_BIAS")
        );
        let car = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        assert!(
            biased_source(car).contains("textureSampleBias(diffuse_tex, diffuse_sampler, in.uv, MIP_BIAS)")
                && !car.contains("MIP_BIAS")
        );
    }
}
