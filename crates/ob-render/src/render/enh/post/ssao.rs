//! Ambient occlusion (the Graphics page's "Ambient Occlusion", a **clone
//! deviation**, off by default; docs/formats.md "Graphics enhancements").
//! The game has none: its lighting is baked into the vertex colours.
//!
//! A light screen-space estimate from the scene's depth alone: at half the
//! scene's size, twelve taps on a spiral within ~1.2 m of each pixel (in
//! view space, reconstructed from the depth and the view's projection),
//! each occluding by the cosine of its elevation over the pixel's plane
//! (normal from the depth's slopes; less a 0.15 bias for mesh facets),
//! falling off with distance (horizon-based in spirit, scale-free). Then a
//! 4 × 4 depth-weighted blur of it (half size), and at the scene's size
//! its 2 × 2 depth-weighted upsample multiplies the scene's colour, weighed by the pixel's fog factor (world.efx's
//! oFog: a fogged pixel is darkened only in its surface's share, as if
//! before the fog) and the row's strength. The sky (depth at the far
//! plane) is left alone; alpha (the bleach bypass mask) is untouched.
//!
//! How it is kept cheap (docs/notes/performance/integrated-gpu.md, "Ambient Occlusion:
//! the same occlusion for less"): the twelve taps' loads go out together
//! (a sky tap is chosen away, not branched over); the spiral is one table
//! turned once per pixel (one sine and cosine, not twelve of each), and a
//! tap's sample point and view position are one multiply-add each from the
//! pixel's own; the centre and its four neighbours come from two gathers.
//! The apply blends only pixels with something to darken: not the sky,
//! and not a pixel whose four texels are unoccluded (the blur leaves those
//! at 1 or the largest 16-bit float below it).
//!
//! When it multiplies (docs/notes/graphics/car-look.md "Darkening settings against emissive light and the car's shadow"): in game, on the lit
//! scene as soon as the world and the cars are drawn (and the race start's
//! characters, drawn before the shadows then), before the additive layers:
//! the cars' shadow texture and underglow, the particles (nitrous flames),
//! the glow sprites, the horizon fog ([`Ssao::run_into`], into the
//! multisampled scene when there is one). Occlusion darkens what the
//! surfaces reflect, not light that is emitted over them.

use crate::render::Gpu;

/// What a frame's occlusion needs: the scene's depth and how to read it.
#[derive(Clone)]
pub struct Frame {
    /// The scene's depth (multisampled when `samples` > 1).
    pub depth: wgpu::TextureView,
    pub samples: u32,
    /// The view's near and far planes, and whether the depth is reversed.
    pub near: f32,
    pub far: f32,
    pub reversed: bool,
    /// The projection's x and y scales (m00, m11).
    pub scale: (f32, f32),
    /// world.efx's FogValue (far, 1 / (far − start), minimum), or none.
    pub fog: Option<[f32; 3]>,
    /// The row's strength, 0..1.
    pub strength: f32,
    /// The cars whose underglow lights their footprint, in this
    /// pass's view space ([`glow_boxes`]); there the occlusion gives way
    /// to the glow as the contact shadow does (`lamps::contact_share`).
    pub glows: Vec<GlowBox>,
}

/// A car's box in the occlusion's view space (x right, y up, z ahead):
/// centre (w the underglow's light, 0..1), then its three axes (w the half
/// extent along each), as world.wgsl's `Occluder`.
pub type GlowBox = [[f32; 4]; 4];

/// Car boxes a frame takes at most (the nearest cars first; every
/// car, for the body mask, not only the ones with an underglow).
pub const MAX_GLOWS: usize = 8;

/// The cars of `occluders` (the nearest first, as the game lists them), in
/// the view space of `view` (the camera's right-handed view matrix), at most
/// [`MAX_GLOWS`]; centre.w their underglow's light (0: none). All of
/// them, the occlusion leaves every car's body alone (`glow_share`).
pub fn glow_boxes(view: glam::Mat4, occluders: &[crate::render::lamps::Occluder]) -> Vec<GlowBox> {
    let flip = |v: glam::Vec3| glam::Vec3::new(v.x, v.y, -v.z);
    occluders
        .iter()
        .take(MAX_GLOWS)
        .map(|o| {
            let g = o.gpu();
            let c = flip(view.transform_point3(glam::Vec3::from_slice(&g.centre[..3])));
            let axis =
                |a: [f32; 4]| flip(view.transform_vector3(glam::Vec3::from_slice(&a[..3]))).extend(a[3]).to_array();
            [c.extend(o.glow.clamp(0.0, 1.0)).to_array(), axis(g.x), axis(g.y), axis(g.z)]
        })
        .collect()
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    /// xy: the scene's size; zw: the occlusion target's.
    size: [f32; 4],
    /// near, far, reversed (0 / 1), strength.
    depth: [f32; 4],
    /// m00, m11, radius (m), the scene's texels per screen texel (SSAA /
    /// Render Scale: the reach's cap is the same part of the screen).
    proj: [f32; 4],
    /// FogValue x, y, z, fog on (0 / 1).
    fog: [f32; 4],
    /// x: how many of `glows` are set.
    glow_count: [f32; 4],
    glows: [GlowBox; MAX_GLOWS],
    /// Each box's [`glow_rect`] and their union (scene pixels).
    glow_rects: [[f32; 4]; MAX_GLOWS],
    glow_union: [f32; 4],
    /// The occlusion grid ([`Grid`]): xy the scene's texels per occlusion
    /// texel, zw where the depth is read in that footprint (the texel
    /// nearest its centre).
    grid: [f32; 4],
    /// xy: 1 / grid.xy; z: the reach's factor, scene-half texels to
    /// occlusion texels (2 / grid.y: 1 at native's half size).
    grid_inv: [f32; 4],
}

/// `glow_share`'s terms (ssao.wgsl), in the box's own axes (q, its half
/// extents h): the body is 0 where `outside` ≥ [`BODY_RIM`] or q.z ≤
/// −h.z + [`BODY_FLOOR`]; the underglow's footprint is 0 where its
/// |q.x| − [`FOOT`].0 · h.x or |q.y| − [`FOOT`].1 · h.y reaches
/// [`FOOT_REACH`] + [`FOOT_REACH_Y`] · h.y, or q.z ≥ 0, or q.z ≤ −h.z −
/// [`FOOT_BELOW`] (each smoothstep at its end, so the share is multiplied by
/// exactly 1 there).
const BODY_RIM: f64 = 0.08;
const BODY_FLOOR: f64 = 0.04;
const FOOT: (f64, f64) = (0.85, 0.8);
const FOOT_REACH: f64 = 0.35;
const FOOT_REACH_Y: f64 = 0.15;
const FOOT_BELOW: f64 = 0.9;

/// [`glow_rect`]'s margins: metres added round the region, and scene pixels
/// on each side. Both far above the f32 rounding of the shader's position
/// and box coordinates (well under a millimetre within the 4 m the loop
/// looks) and of this rectangle (a thousandth of a pixel).
const RECT_MARGIN_M: f64 = 0.05;
const RECT_MARGIN_PX: f64 = 1.0;

/// A rectangle that holds every pixel.
const EVERYWHERE: [f32; 4] = [f32::MIN, f32::MIN, f32::MAX, f32::MAX];

/// The scene pixels (min x, min y, max x, max y; `frag.xy` units) where
/// `glow_share` can take anything from box `g`: the screen's bounds of the
/// box-aligned region (in the box's axes, x within ±max(h.x + rim, foot
/// reach), y likewise, z from the footprint's or the body's floor to the
/// body's top) outside which both its terms are exactly 0, with
/// [`RECT_MARGIN_M`]. A pixel's position as `glow_share` reconstructs it
/// lies on the ray through the pixel (x = ndc.x / m00 · z, y = ndc.y / m11
/// · z), so a position inside the region is on a ray through the region,
/// whose pixel is inside its corners' projection. A region reaching the
/// eye's plane can be anywhere on screen. `scale` = (m00, m11), `size` the
/// scene's.
pub fn glow_rect(g: &GlowBox, scale: (f32, f32), size: (u32, u32)) -> [f32; 4] {
    use glam::{DMat3, DVec3};
    let v = |a: [f32; 4]| DVec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2]));
    let (c, ax, ay, az) = (v(g[0]), v(g[1]), v(g[2]), v(g[3]));
    let h = DVec3::new(f64::from(g[1][3]), f64::from(g[2][3]), f64::from(g[3][3]));
    // As the shader's `if o.centre.w <= 0.0 { continue; }` (a NaN takes the footprint).
    let glow = !(g[0][3] <= 0.0);
    let foot = |k: f64, hh: f64| if glow { k * hh + FOOT_REACH + FOOT_REACH_Y * h.y } else { 0.0 };
    let m = RECT_MARGIN_M;
    let ext_x = (h.x + BODY_RIM).max(foot(FOOT.0, h.x)) + m;
    let ext_y = (h.y + BODY_RIM).max(foot(FOOT.1, h.y)) + m;
    let z_top = h.z + BODY_RIM + m;
    let z_floor = if glow { -h.z - FOOT_BELOW } else { -h.z + BODY_FLOOR } - m;
    // q = A (p − c) with the axes as A's rows: p = c + A⁻¹ q.
    let a = DMat3::from_cols(ax, ay, az).transpose();
    if !(a.determinant().abs() > 1e-6) || !h.is_finite() || !c.is_finite() {
        return EVERYWHERE;
    }
    let back = a.inverse();
    let (m00, m11) = (f64::from(scale.0), f64::from(scale.1));
    let (w, hgt) = (f64::from(size.0), f64::from(size.1));
    let mut r = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for i in 0..8 {
        let q = DVec3::new(
            if i & 1 == 0 { -ext_x } else { ext_x },
            if i & 2 == 0 { -ext_y } else { ext_y },
            if i & 4 == 0 { z_floor } else { z_top },
        );
        let p = c + back * q;
        if !(p.z > 0.05) {
            return EVERYWHERE;
        }
        // ndc.x = m00 · x / z, pixel x = (ndc.x + 1) / 2 · w; ndc.y = m11 ·
        // y / z, pixel y = (1 − ndc.y) / 2 · h (the shader's mapping, inverted).
        let px = (m00 * p.x / p.z + 1.0) * 0.5 * w;
        let py = (1.0 - m11 * p.y / p.z) * 0.5 * hgt;
        r = [r[0].min(px), r[1].min(py), r[2].max(px), r[3].max(py)];
    }
    let e = RECT_MARGIN_PX;
    let rect = [r[0] - e, r[1] - e, r[2] + e, r[3] + e];
    if rect.iter().all(|x| x.is_finite()) { rect.map(|x| x as f32) } else { EVERYWHERE }
}

/// The union of `rects` (none: a rectangle no pixel is in).
fn union(rects: &[[f32; 4]]) -> [f32; 4] {
    rects.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |u, r| {
        [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]
    })
}

/// Off: every box's rectangle the whole screen (a check aid: the pass as
/// it was before the rectangles, `tests::first_use`).
static RECTS_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// [`RECTS_OFF`].
pub fn set_glow_rects_off(off: bool) {
    RECTS_OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}

/// Where the occlusion is computed for a scene `size` big shown on a
/// screen `texel_scale` times smaller (the scene's height over the
/// screen's): half the scene (2 × 2 scene texels per occlusion texel), or
/// under SSAA (scene larger than the screen) half the **screen**, the grid
/// the occlusion has without SSAA (docs/notes/graphics/ssaa.md "Effect and post passes at the screen's size under SSAA"). The apply
/// stays at the scene's size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// The occlusion targets' size.
    pub target: (u32, u32),
    /// The scene's texels per occlusion texel, per axis (exactly 2 at half
    /// the scene).
    pub ratio: [f32; 2],
}

impl Grid {
    pub fn of(size: (u32, u32), texel_scale: f32) -> Grid {
        let half = ((size.0 / 2).max(1), (size.1 / 2).max(1));
        if texel_scale <= 1.0 || !screen_grid_on() {
            return Grid { target: half, ratio: [2.0, 2.0] };
        }
        // The screen's half: the screen is the scene over `texel_scale`.
        let screen = |s: u32| ((s as f32 / texel_scale).round() as u32).max(2);
        let target = ((screen(size.0) / 2).max(1), (screen(size.1) / 2).max(1));
        if target.0 >= half.0 || target.1 >= half.1 {
            return Grid { target: half, ratio: [2.0, 2.0] };
        }
        Grid { target, ratio: [size.0 as f32 / target.0 as f32, size.1 as f32 / target.1 as f32] }
    }

    /// The shader's `grid` and `grid_inv` (exactly (2, 2, 1 − 2⁻¹⁰, …)
    /// and (0.5, 0.5, 1, 0) at half the scene: every expression then gives
    /// the half-size values bit for bit). The depth texel is the one
    /// holding the footprint's centre (`floor(centre − 2⁻¹⁰)`: at 2:1,
    /// where the centre is a texel corner, the top-left texel as before).
    fn uniforms(self) -> ([f32; 4], [f32; 4]) {
        let [rx, ry] = self.ratio;
        const BEFORE: f32 = 1.0 / 1024.0;
        ([rx, ry, rx * 0.5 - BEFORE, ry * 0.5 - BEFORE], [1.0 / rx, 1.0 / ry, 2.0 / ry, 0.0])
    }
}

/// The occlusion at the screen's half under SSAA (on unless
/// `NFSU2_SSAA_SCENE_SIZE` names `ssao`; for comparisons).
fn screen_grid_on() -> bool {
    !crate::render::debug_env::ssaa_scene_size("ssao")
}

pub struct Ssao {
    layouts: [wgpu::BindGroupLayout; 2],
    /// [single, multisampled] × [depth, occlusion, blur, apply].
    pipelines: [[wgpu::RenderPipeline; 4]; 2],
    uniform: wgpu::Buffer,
    /// The linear depth, the occlusion and its blur (half size; the
    /// latter two R the occlusion, G the linear depth; −1 for the sky).
    targets: Option<((u32, u32), [wgpu::TextureView; 3])>,
    /// The apply pass into a multisampled scene (its sample count): the
    /// multisampled-depth module and layout, built when first needed.
    apply_ms: Option<(u32, wgpu::RenderPipeline)>,
    ms_module: wgpu::ShaderModule,
    ms_layout: wgpu::PipelineLayout,
    scene_format: wgpu::TextureFormat,
    groups: Option<(wgpu::TextureView, [wgpu::BindGroup; 4])>,
    dummy: wgpu::TextureView,
    /// Nearest and clamped to the edge: the occlusion's taps and gathers
    /// read whole texels of the linear depth.
    nearest: wgpu::Sampler,
}

const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

impl Ssao {
    pub fn new(gpu: &Gpu, scene_format: wgpu::TextureFormat) -> Ssao {
        let device = &gpu.device;
        let layout = |multisampled: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("ssao"),
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            })
        };
        let layouts = [layout(false), layout(true)];
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssao.wgsl"));
        let modules = [false, true].map(|ms| {
            // The multisampled variant reads sample 0 of the depth.
            let src = if ms {
                source.replace("texture_depth_2d", "texture_depth_multisampled_2d")
            } else {
                source.to_string()
            };
            let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("ssao"),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("ssao"),
                bind_group_layouts: &[Some(&layouts[usize::from(ms)])],
                immediate_size: 0,
            });
            (shader, pl)
        });
        let pipelines = [false, true].map(|ms| {
            let (shader, pl) = &modules[usize::from(ms)];
            let make =
                |entry: &str, format: wgpu::TextureFormat, blend: Option<wgpu::BlendState>, mask: wgpu::ColorWrites| {
                    crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                        label: Some(entry),
                        layout: Some(pl),
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
                            targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: mask })],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                        multiview_mask: None,
                        cache: gpu.disk_pipeline_cache(),
                    })
                };
            // The last multiplies the scene by the occlusion (MULTIPLY).
            [
                make("fs_depth", DEPTH_FORMAT, None, wgpu::ColorWrites::ALL),
                make("fs_occlusion", AO_FORMAT, None, wgpu::ColorWrites::ALL),
                make("fs_blur", AO_FORMAT, None, wgpu::ColorWrites::ALL),
                make("fs_apply", scene_format, Some(MULTIPLY), wgpu::ColorWrites::COLOR),
            ]
        });
        let [_, (ms_module, ms_layout)] = modules;
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("ssao"),
                size: std::mem::size_of::<Uniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let dummy = half_target(gpu, (1, 1), AO_FORMAT);
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ssao"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Ssao {
            layouts,
            pipelines,
            uniform,
            targets: None,
            apply_ms: None,
            ms_module,
            ms_layout,
            scene_format,
            groups: None,
            dummy,
            nearest,
        }
    }

    /// The occlusion applied into a scene of `samples` (> 1) samples: its
    /// pipeline, made where missing.
    fn apply_ms_pipeline(&mut self, gpu: &Gpu, samples: u32) {
        if self.apply_ms.as_ref().is_none_or(|(n, _)| *n != samples) {
            self.apply_ms = Some((
                samples,
                crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                    label: Some("fs_apply (multisampled)"),
                    layout: Some(&self.ms_layout),
                    vertex: wgpu::VertexState {
                        module: &self.ms_module,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &self.ms_module,
                        entry_point: Some("fs_apply"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: self.scene_format,
                            blend: Some(MULTIPLY),
                            write_mask: wgpu::ColorWrites::COLOR,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                    multiview_mask: None,
                    cache: gpu.disk_pipeline_cache(),
                }),
            ));
        }
    }

    /// The pipelines a [`Ssao::run_into`] at `samples` adds to the ones
    /// made with the pass, made now (a warm-up's copy).
    pub fn warm(mut self, gpu: &Gpu, samples: u32) {
        if samples > 1 {
            self.apply_ms_pipeline(gpu, samples);
        }
    }

    /// The occlusion of `frame` multiplied into `scene` (`size` big).
    /// `texel_scale`: the scene's height over the screen's (1 at 1×).
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        size: (u32, u32),
        texel_scale: f32,
        frame: &Frame,
    ) {
        self.run_into(gpu, encoder, scene, 1, size, texel_scale, frame);
    }

    /// As [`Ssao::run`], into a scene of `samples` samples (the
    /// multisampled scene before its resolve: every sample of a pixel takes
    /// the pixel's factor, as the resolved pixel would).
    #[allow(clippy::too_many_arguments)]
    pub fn run_into(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        samples: u32,
        size: (u32, u32),
        texel_scale: f32,
        frame: &Frame,
    ) {
        if samples > 1 {
            self.apply_ms_pipeline(gpu, samples);
        }
        // The draws recorded since the previous mark (the characters, the
        // mirror) close here, so that the passes below are timed alone.
        crate::prof::gpu::mark(encoder, "graphics: the draws before the ambient occlusion");
        let grid = Grid::of(size, texel_scale);
        let half = grid.target;
        if self.targets.as_ref().is_none_or(|t| t.0 != half) {
            self.targets = Some((
                half,
                [
                    half_target(gpu, half, DEPTH_FORMAT),
                    half_target(gpu, half, AO_FORMAT),
                    half_target(gpu, half, AO_FORMAT),
                ],
            ));
            self.groups = None;
        }
        let ms = usize::from(frame.samples > 1);
        let [linear, occluded, blurred] = self.targets.as_ref().unwrap().1.clone();
        if self.groups.as_ref().is_none_or(|(d, _)| *d != frame.depth) {
            let group = |second: &wgpu::TextureView| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("ssao"),
                    layout: &self.layouts[ms],
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&frame.depth) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(second) },
                        wgpu::BindGroupEntry { binding: 2, resource: self.uniform.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.nearest) },
                    ],
                })
            };
            self.groups =
                Some((frame.depth.clone(), [group(&self.dummy), group(&linear), group(&occluded), group(&blurred)]));
        }
        let mut u = Uniform {
            size: [size.0 as f32, size.1 as f32, half.0 as f32, half.1 as f32],
            depth: [frame.near, frame.far, if frame.reversed { 1.0 } else { 0.0 }, frame.strength],
            proj: [frame.scale.0, frame.scale.1, RADIUS, texel_scale],
            fog: match frame.fog {
                Some(f) => [f[0], f[1], f[2], 1.0],
                None => [0.0, 0.0, 0.0, 0.0],
            },
            glow_count: [frame.glows.len().min(MAX_GLOWS) as f32, 0.0, 0.0, 0.0],
            glows: std::array::from_fn(|i| frame.glows.get(i).copied().unwrap_or_default()),
            glow_rects: [EVERYWHERE; MAX_GLOWS],
            glow_union: EVERYWHERE,
            grid: grid.uniforms().0,
            grid_inv: grid.uniforms().1,
        };
        if !RECTS_OFF.load(std::sync::atomic::Ordering::Relaxed) {
            let n = frame.glows.len().min(MAX_GLOWS);
            for (rect, g) in u.glow_rects.iter_mut().zip(&frame.glows[..n]) {
                *rect = glow_rect(g, frame.scale, size);
            }
            u.glow_union = union(&u.glow_rects[..n]);
        }
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let groups = &self.groups.as_ref().unwrap().1;
        let white = wgpu::LoadOp::Clear(wgpu::Color::WHITE);
        let passes = [(&linear, white), (&occluded, white), (&blurred, white), (scene, wgpu::LoadOp::Load)];
        for (i, (target, load)) in passes.into_iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(["ssao depth", "ssao", "ssao blur", "ssao apply"][i]),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            match (&self.apply_ms, i) {
                (Some((n, p)), 3) if samples > 1 && *n == samples => pass.set_pipeline(p),
                _ => pass.set_pipeline(&self.pipelines[ms][i]),
            }
            pass.set_bind_group(0, &groups[i], &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            crate::prof::gpu::mark(
                encoder,
                [
                    "graphics: ssao depth",
                    "graphics: ssao occlusion",
                    "graphics: ssao blur",
                    "graphics: ssao apply",
                ][i],
            );
        }
    }
}

/// The scene's colour times the occlusion's factor (alpha kept).
const MULTIPLY: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::Src,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

/// A half-size target.
fn half_target(gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("ssao"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// The taps' reach, metres.
const RADIUS: f32 = 1.2;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    fn car(at: Vec3, glow: f32) -> crate::render::lamps::Occluder {
        crate::render::lamps::Occluder {
            transform: Mat4::from_translation(at),
            min: Vec3::new(-2.0, -0.9, 0.0),
            max: Vec3::new(2.0, 0.9, 1.4),
            glow,
        }
    }

    /// Every car (with its underglow), at most MAX_GLOWS, in the
    /// pass's view space (x right, y up, z ahead, as `position()`).
    #[test]
    fn glow_boxes_are_in_the_occlusion_view_space() {
        // The camera at the origin looking along +Y (world z up).
        let view = Mat4::look_to_rh(Vec3::ZERO, Vec3::Y, Vec3::Z);
        let cars = [car(Vec3::new(1.0, 10.0, -1.0), 0.8), car(Vec3::new(0.0, 5.0, 0.0), 0.0)];
        let b = glow_boxes(view, &cars);
        assert_eq!(b.len(), 2);
        assert_eq!(b[1][0][3], 0.0, "no underglow");
        let c = b[0][0];
        // Box centre: 0.7 m up from the car's origin.
        assert!((c[0] - 1.0).abs() < 1e-5 && (c[1] - (-0.3)).abs() < 1e-5 && (c[2] - 10.0).abs() < 1e-5, "{c:?}");
        assert!((c[3] - 0.8).abs() < 1e-6);
        // The car's up axis is the view's up; its half height 0.7.
        assert!((b[0][3][1] - 1.0).abs() < 1e-5 && (b[0][3][3] - 0.7).abs() < 1e-5);
        let many: Vec<_> = (0..9).map(|i| car(Vec3::new(0.0, 5.0 + i as f32, 0.0), 1.0)).collect();
        assert_eq!(glow_boxes(view, &many).len(), MAX_GLOWS);
    }

    /// `glow_share`'s position of scene pixel `c` at linear depth `z`, as
    /// ssao.wgsl computes it (f32).
    fn shader_pos(c: (f32, f32), z: f32, scale: (f32, f32), size: (u32, u32)) -> Vec3 {
        let ndc = (c.0 / size.0 as f32 * 2.0 - 1.0, c.1 / size.1 as f32 * -2.0 + 1.0);
        Vec3::new(ndc.0 / scale.0 * z, ndc.1 / scale.1 * z, z)
    }

    /// WGSL's smoothstep (f32).
    fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// The factor one box multiplies a pixel's share by in `glow_share`
    /// (its loop body, f32, without the rectangle test).
    fn shader_factor(pos: Vec3, o: &GlowBox) -> f32 {
        let v = |a: [f32; 4]| Vec3::new(a[0], a[1], a[2]);
        let rel = pos - v(o[0]);
        if rel.dot(rel) > 16.0 {
            return 1.0;
        }
        let q = Vec3::new(rel.dot(v(o[1])), rel.dot(v(o[2])), rel.dot(v(o[3])));
        let h = Vec3::new(o[1][3], o[2][3], o[3][3]);
        let outside = (q.x.abs() - h.x).max(q.y.abs() - h.y).max(q.z - h.z);
        let body = (1.0 - smoothstep(0.0, 0.08, outside)) * smoothstep(-h.z + 0.04, -h.z + 0.14, q.z);
        let mut share = 1.0 - body;
        if o[0][3] <= 0.0 {
            return share;
        }
        let e = glam::Vec2::new((q.x.abs() - h.x * 0.85).max(0.0), (q.y.abs() - h.y * 0.8).max(0.0));
        let footprint = 1.0 - smoothstep(0.0, 0.35 + 0.15 * h.y, e.length());
        let above = q.z + h.z;
        let vertical = (1.0 - smoothstep(0.0, h.z, above)) * (1.0 - smoothstep(0.0, 0.6, -above - 0.3));
        share *= 1.0 - footprint * vertical;
        share
    }

    /// A car's box in the occlusion's view space: centre, yaw about the
    /// view's up axis, the street car's half extents, its underglow.
    fn view_box(centre: [f32; 3], yaw: f32, glow: f32) -> GlowBox {
        let (s, c) = yaw.sin_cos();
        [[centre[0], centre[1], centre[2], glow], [c, 0.0, s, 2.25], [-s, 0.0, c, 0.95], [0.0, 1.0, 0.0, 0.7]]
    }

    /// Outside a box's rectangle `glow_share`'s loop multiplies the share by
    /// exactly 1 at every depth along the pixel's ray (the shader's f32
    /// arithmetic, depths every 2 cm round the box), for cars with and
    /// without an underglow ahead (the chase camera's own car), beside, far
    /// off, turned and near the eye; a box reaching the eye's plane gets the
    /// whole screen; and the chase camera's car leaves most of the screen out
    /// (the skip does something).
    #[test]
    fn a_box_changes_no_pixel_outside_its_rectangle() {
        let size = (160u32, 90u32);
        let scale = (1.0 / (0.6f32 * 16.0 / 9.0), 1.0 / 0.6f32);
        let boxes = [
            view_box([0.0, -1.1, 6.0], std::f32::consts::FRAC_PI_2, 0.8),
            view_box([0.0, -1.1, 6.0], std::f32::consts::FRAC_PI_2, 0.0),
            view_box([3.0, -1.2, 9.0], 0.5, 1.0),
            view_box([-5.0, -0.9, 14.0], 1.4, 0.6),
            view_box([12.0, -2.0, 30.0], 2.5, 0.0),
            view_box([0.5, -1.5, 80.0], 0.2, 1.0),
            view_box([-2.0, -1.0, 4.0], 0.8, 0.5),
        ];
        let mut changed = 0;
        for g in &boxes {
            let r = glow_rect(g, scale, size);
            assert!(r[0] < r[2] && r[1] < r[3], "{g:?}: {r:?}");
            let cz = g[0][2];
            for py in 0..size.1 {
                for px in 0..size.0 {
                    let p = (px as f32 + 0.5, py as f32 + 0.5);
                    let inside = !(p.0 < r[0] || p.1 < r[1] || p.0 > r[2] || p.1 > r[3]);
                    for k in 0..=400 {
                        let z = cz - 4.0 + k as f32 * 0.02;
                        if z <= 0.0 {
                            continue;
                        }
                        let f = shader_factor(shader_pos(p, z, scale, size), g);
                        assert!(inside || f == 1.0, "{g:?}: pixel {p:?} at {z} takes {f} outside {r:?}");
                        changed += usize::from(f != 1.0);
                    }
                }
            }
        }
        assert!(changed > 1000, "the boxes reach pixels ({changed})");
        // The chase camera's car: its rectangle holds under half the screen.
        let r = glow_rect(&boxes[0], scale, size);
        let area = (r[2].min(size.0 as f32) - r[0].max(0.0)) * (r[3].min(size.1 as f32) - r[1].max(0.0));
        assert!(area < 0.5 * (size.0 * size.1) as f32, "{r:?}");
        // At the eye's plane: everywhere.
        assert_eq!(glow_rect(&view_box([0.0, -1.0, 1.5], std::f32::consts::FRAC_PI_2, 1.0), scale, size), EVERYWHERE);
        assert_eq!(glow_rect(&view_box([0.0, 0.0, -6.0], 0.0, 0.0), scale, size), EVERYWHERE);
        assert_eq!(union(&[]), [f32::MAX, f32::MAX, f32::MIN, f32::MIN]);
        assert_eq!(union(&[[1.0, 2.0, 3.0, 4.0], [0.0, 3.0, 2.0, 9.0]]), [0.0, 2.0, 3.0, 9.0]);
    }

    /// The rectangle's region is the shader's: the terms' constants as
    /// ssao.wgsl writes them, and the rectangles tested before the loop and
    /// before each box.
    #[test]
    fn the_rectangles_follow_the_shaders_terms() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssao.wgsl"));
        for line in [
            "let body = (1.0 - smoothstep(0.0, 0.08, outside)) * smoothstep(-h.z + 0.04, -h.z + 0.14, q.z);",
            "let e = max(abs(q.xy) - h.xy * vec2<f32>(0.85, 0.8), vec2<f32>(0.0));",
            "let footprint = 1.0 - smoothstep(0.0, 0.35 + 0.15 * h.y, length(e));",
            "let vertical = (1.0 - smoothstep(0.0, h.z, above)) * (1.0 - smoothstep(0.0, 0.6, -above - 0.3));",
            "if count == 0u || outside_rect(c, p.glow_union) {",
            "if outside_rect(c, p.glow_rects[i]) { continue; }",
        ] {
            assert!(src.contains(line), "ssao.wgsl: {line}");
        }
        assert_eq!((BODY_RIM, BODY_FLOOR, FOOT, FOOT_REACH, FOOT_REACH_Y, FOOT_BELOW), (0.08, 0.04, (0.85, 0.8), 0.35, 0.15, 0.9));
    }

    /// The occlusion's grid: half the scene, with the shader's values
    /// exactly native's 2:1 ones, at 1× (odd sizes too) and below 100 %;
    /// under SSAA half the screen (docs/notes/graphics/ssaa.md "Effect and post passes at the screen's size under SSAA").
    #[test]
    fn occlusion_grid_is_half_the_scene_without_ssaa_and_half_the_screen_with_it() {
        for (size, scale) in [((3840, 2160), 1.0), ((1281, 961), 1.0), ((1920, 1080), 0.5), ((640, 480), 1.0)] {
            let g = Grid::of(size, scale);
            assert_eq!(g, Grid { target: ((size.0 / 2).max(1), (size.1 / 2).max(1)), ratio: [2.0, 2.0] });
            let (grid, inv) = g.uniforms();
            assert_eq!((grid[..2].to_vec(), inv), (vec![2.0, 2.0], [0.5, 0.5, 1.0, 0.0]));
            // floor(2t + zw) = 2t, the top-left texel of each 2 × 2.
            for t in [0u32, 1, 2, 7, 1000, 4095] {
                assert_eq!((t as f32 * grid[0] + grid[2]).floor(), (2 * t) as f32);
            }
        }
        // SSAA 2x / 4x / 16x at 4K: the screen's half, 1920 × 1080.
        for (scene, label) in [((5431u32, 3055u32), "2x"), ((7680, 4320), "4x"), ((15360, 8640), "16x")] {
            let g = Grid::of(scene, scene.1 as f32 / 2160.0);
            assert_eq!(g.target, (1920, 1080), "{label}");
            assert!(
                (g.ratio[0] - scene.0 as f32 / 1920.0).abs() < 1e-6
                    && (g.ratio[1] - scene.1 as f32 / 1080.0).abs() < 1e-6
            );
            // The reach and its cap: the same part of the screen as at 1×.
            let (grid, inv) = g.uniforms();
            assert!((inv[2] * (scene.1 as f32 / 2160.0) - 1.0).abs() < 1e-4, "{label}: reach factor {}", inv[2]);
            // The depth texel: the one nearest the footprint's centre, for
            // every occlusion texel (the position the occlusion assumes is
            // that texel's own, so no pattern of the two grids' beat).
            for t in [0u32, 1, 2, 7, 100, 959] {
                let picked = (t as f32 * grid[0] + grid[2]).floor();
                let centre = (t as f32 + 0.5) * g.ratio[0];
                assert!(
                    (picked + 0.5 - centre).abs() <= 0.5 + 1e-3,
                    "{label}: texel {t} reads {picked}, centre {centre}"
                );
            }
        }
    }

    const N: u32 = 128;
    const NEAR: f32 = 0.5;
    const FAR: f32 = 1000.0;

    /// A synthetic view's depth (x right, y up, z ahead; m00 = m11 = 1),
    /// written by a pass: the ground 1.5 m below the eye, a box 1.6 m wide
    /// and 1 m high standing on it with its face 6 m ahead, the sky above.
    fn synthetic_depth(gpu: &Gpu) -> wgpu::TextureView {
        let depth = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: N, height: N, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::render::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = depth.create_view(&Default::default());
        let src = format!(
            "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
                let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
                return vec4<f32>(q * 2.0 - 1.0, 0.5, 1.0);
            }}
            @fragment fn fs(@builtin(position) p: vec4<f32>) -> @builtin(frag_depth) f32 {{
                let ndc = (p.xy / {n}.0) * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
                var z = 1e9;
                if ndc.y < 0.0 {{ z = 1.5 / -ndc.y; }}
                let x = ndc.x * 6.0;
                let y = ndc.y * 6.0;
                if abs(x) < 0.8 && y > -1.5 && y < -0.5 {{ z = 6.0; }}
                if z > 1e8 {{ return 1.0; }}
                return {f} * (z - {n0}) / (z * ({f} - {n0}));
            }}",
            n = N,
            f = FAR,
            n0 = NEAR
        );
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
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
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view,
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
        view
    }

    /// The occlusion of the synthetic view multiplied (twice: the second
    /// frame reuses the targets and groups) into a grey scene of `format`
    /// (alpha 0.5), the scene `texel_scale` times the screen (above 1: the
    /// occlusion on the screen's half, `Grid`): the scene read back.
    fn occluded_scene(gpu: &Gpu, depth: &wgpu::TextureView, format: wgpu::TextureFormat, texel_scale: f32) -> Vec<u8> {
        let scene = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: N, height: N, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texel = format.block_copy_size(None).unwrap();
        let grey: Vec<u8> = if texel == 8 {
            [200.0f32, 180.0, 160.0, 128.0].iter().flat_map(|v| half_bits(v / 255.0).to_le_bytes()).collect()
        } else {
            vec![200, 180, 160, 128]
        };
        gpu.queue.write_texture(
            scene.as_image_copy(),
            &grey.repeat((N * N) as usize),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(N * texel), rows_per_image: None },
            scene.size(),
        );
        let frame = Frame {
            depth: depth.clone(),
            samples: 1,
            near: NEAR,
            far: FAR,
            reversed: false,
            scale: (1.0, 1.0),
            fog: None,
            strength: 1.0,
            glows: Vec::new(),
        };
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut ssao = Ssao::new(gpu, format);
        for _ in 0..2 {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            ssao.run(gpu, &mut encoder, &scene.create_view(&Default::default()), (N, N), texel_scale, &frame);
            gpu.queue.submit([encoder.finish()]);
        }
        assert!(pollster::block_on(scope.pop()).is_none(), "{format:?}: a validation error");
        crate::render::read_back(gpu, &scene, N, N)
    }

    /// The f16 bits of `v` (round to nearest, normal range).
    fn half_bits(v: f32) -> u16 {
        let b = v.to_bits();
        let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
        if v == 0.0 || e <= 0 {
            return 0;
        }
        let m = (b & 0x7f_ffff) + 0x1000;
        let (e, m) = if m & 0x80_0000 != 0 { (e + 1, 0) } else { (e, m >> 13) };
        ((e as u32) << 10 | m) as u16
    }

    /// On a synthetic view (a box standing on open ground, the sky above),
    /// in an 8-bit and a 16-bit float scene, and under SSAA 2× (the
    /// occlusion on the screen's half, positions from the picked texels):
    /// the ground at the box's foot
    /// darkens; the sky and the open ground (every texel there unoccluded)
    /// are not blended and keep their colour to the bit, twice multiplied;
    /// alpha (the bleach bypass mask) is kept.
    #[test]
    fn only_occluded_pixels_are_blended() {
        let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
        let depth = synthetic_depth(&gpu);
        for (format, texel_scale) in
            [(wgpu::TextureFormat::Rgba8Unorm, 1.0), (crate::render::HDR_FORMAT, 1.0), (crate::render::HDR_FORMAT, 2.0)]
        {
            let out = occluded_scene(&gpu, &depth, format, texel_scale);
            let texel = format.block_copy_size(None).unwrap() as usize;
            let px = |x: u32, y: u32| out[(y * N + x) as usize * texel..(y * N + x + 1) as usize * texel].to_vec();
            let grey = px(0, 0);
            assert_eq!(px(64, 10), grey, "{format:?}: sky");
            assert_eq!(px(8, 120), grey, "{format:?}: open ground");
            // The ground just below the box's bottom edge (ndc y -0.25: row 80).
            let foot = px(64, 82);
            let alpha = texel - texel / 4;
            assert!(foot != grey && foot[alpha..] == grey[alpha..], "{format:?}: foot {foot:?}");
            let darkened = out.chunks_exact(texel).filter(|p| *p != grey.as_slice()).count();
            assert!(darkened > 20 && darkened < (N * N / 4) as usize, "{format:?}: darkened {darkened}");
        }
    }

    /// ssao.wgsl's SPIRAL is the spiral the taps used to compute per tap:
    /// tap i at k = (i + 0.5) / 12 along (cos, sin) of k·2π·φ² (φ² =
    /// 2.6180340), times k; turned per pixel by the noise's angle.
    #[test]
    fn the_spiral_table_is_the_taps_spiral() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssao.wgsl"));
        let table = &source[source.find("const SPIRAL").expect("the table")..];
        let table = &table[..table.find(");").unwrap()];
        let values: Vec<f64> = table
            .split("vec2<f32>(")
            .skip(1)
            .flat_map(|e| e.split(')').next().unwrap().split(',').map(|v| v.trim().parse::<f64>().unwrap()))
            .collect();
        assert_eq!(values.len(), 24);
        for i in 0..12 {
            let k = (i as f64 + 0.5) / 12.0;
            let a = k * 6.2831853 * 2.6180340;
            let (x, y) = (values[2 * i], values[2 * i + 1]);
            assert!((x - k * a.cos()).abs() < 1e-6 && (y - k * a.sin()).abs() < 1e-6, "tap {i}: {x} {y}");
        }
    }

    /// The apply pass weighs the occlusion by the underglow. (That
    /// the occlusion is multiplied into the scene before the additive
    /// layers, in the game's frame and the shots alike, is the order of the
    /// game's one world scene: `draw_world_scene`'s `WORLD_PASSES`, tested
    /// there.)
    #[test]
    fn occlusion_spares_the_underglow() {
        assert!(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssao.wgsl"))
                .contains("glow_share(frag.xy, z)")
        );
    }
}
