//! Motion blur (the Graphics page's "Motion Blur" Camera / Per-Object
//! tiers) and depth of field (its "Depth Of Field" Cinematic tier): see
//! docs/motionblur-dof.md. **Clone deviations**, opt-in; nothing here is
//! in SPEED2.EXE. The original's own speed blur (0x5CC470,
//! `crate::motion_blur`, the "On" tier) and its depth of field switch
//! (0x870CF0, whose pass 0x5CCCE0 never runs behind the chase camera and
//! is not ported) are untouched.
//!
//! Both run on the lit scene once the world, the cars, their shadows and
//! the race start's characters are drawn, before the additive layers
//! (particles, glow sprites, horizon fog) and the post chain (streaks,
//! overbright glow, rain, the temporal pass) and the HUD, so none of
//! those ghosts or smears: the scene is resolved (or copied) once, the
//! passes read the copy and write the result back over every sample of
//! the (multisampled) scene.
//!
//! * **Motion**: per pixel, the camera's motion from the depth (camera-
//!   relative reprojection with the last drawn view, as the TAA
//!   pass), a drawn car's rigid motion inside its box (Per-Object; the
//!   player's car is masked sharp in both tiers), and a wheel's spin as
//!   an arc about its axle (Per-Object). Tile max, neighbour max and a
//!   depth-aware gather (McGuire et al. 2012).
//! * **Shutter**: the blur is the motion over [`SHUTTER`] seconds (180° at
//!   60 fps), whatever the display rate: the motion between two drawn
//!   frames is scaled by `SHUTTER / frame time` ([`shutter_scale`]).
//! * **Depth of field**: only in scenes that are watched, not driven
//!   ([`dof_scene`]: the scene cameras (NIS / shop scenes), replays, the
//!   garage and showroom, photo mode); in focus the focused car's whole
//!   depth range ([`focus_band`]); elsewhere a golden-angle disc gather at
//!   half size on the half-size scene (each tap a 4 × 4 footprint), blended
//!   in by the pixel's own circle of confusion or a blurred foreground's
//!   cover.

use crate::render::Gpu;
use crate::render::camera::Camera;
use crate::render::temporal;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// The shutter (s): half a 60 fps frame.
pub const SHUTTER: f32 = 0.5 / 60.0;
/// The longest streak, a share of the scene's height.
pub const MAX_BLUR: f32 = 0.04;
/// Camera tier: no blur from the camera's motion below this speed (m/s),
/// all of it from [`RAMP_TO`] on (smoothstep between).
pub const RAMP_FROM: f32 = 8.0;
pub const RAMP_TO: f32 = 40.0;
/// A drawn frame longer than this (s) has no motion worth blurring (a
/// stall, a load): no blur, history restarts.
pub const MAX_FRAME: f64 = 0.1;
/// A wheel's arc over the shutter is held to this (radians): past it the
/// rim is a uniform blur anyway.
pub const MAX_ARC: f32 = 2.5;
/// A car's box starts this far (m) over its tyres' lowest point: the
/// road just around it keeps the camera's motion.
pub const FLOOR: f32 = 0.15;
/// Taps of the motion gather and of the depth of field's disc (at half
/// size).
pub const MOTION_TAPS: u32 = 15;
pub const DOF_TAPS: u32 = 40;
/// The largest circle of confusion, a share of the scene's height.
pub const MAX_COC: f32 = 0.012;

const AUX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The Motion Blur row's clone tiers (`qol::motion_blur`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionMode {
    /// The original's speed blur (or none): this module does nothing.
    Original,
    /// The camera's motion only, stronger with speed; the player's car,
    /// its wheels and the HUD sharp.
    Camera,
    /// Camera + each car's own motion and its wheels' spin; the player's
    /// car body sharp.
    PerObject,
}

impl MotionMode {
    pub fn from_tier(v: u8) -> MotionMode {
        match v {
            1 => MotionMode::Camera,
            2 => MotionMode::PerObject,
            _ => MotionMode::Original,
        }
    }

    /// The mode in force (the Graphics page's value, from the frame's
    /// settings).
    pub fn of(opts: &crate::render::RenderOptions) -> MotionMode {
        MotionMode::from_tier(opts.motion_blur)
    }
}

/// `SHUTTER / dt` for a drawn frame `dt` seconds after the last: the
/// factor that turns the motion between two drawn frames into the motion
/// over the shutter. None when the frame is empty (nothing moved: a pause)
/// or too long ([`MAX_FRAME`]).
pub fn shutter_scale(dt: f64) -> Option<f32> {
    (dt > 1e-5 && dt <= MAX_FRAME).then(|| (f64::from(SHUTTER) / dt) as f32)
}

/// The Camera tier's share of the camera's motion at `speed` (m/s).
pub fn speed_ramp(speed: f32) -> f32 {
    let t = ((speed - RAMP_FROM) / (RAMP_TO - RAMP_FROM)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

static PHOTO: AtomicBool = AtomicBool::new(false);

/// Photo mode is up: depth of field runs (whatever the camera).
pub fn set_photo_mode(on: bool) {
    PHOTO.store(on, Ordering::Relaxed);
}

static PHOTO_FOCUS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static PHOTO_APERTURE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Photo mode's depth of field (docs/photo-mode.md): focus distance
/// (m, 0 = the car looked at) and f-number (0 = none). While photo mode is
/// up these alone decide the depth of field (the Graphics row's Cinematic
/// not needed).
pub fn set_photo_dof(focus: f32, aperture: f32) {
    PHOTO_FOCUS.store(focus.to_bits(), Ordering::Relaxed);
    PHOTO_APERTURE.store(aperture.to_bits(), Ordering::Relaxed);
}

/// Photo mode's (focus, f-number) when photo mode is up.
pub fn photo_dof() -> Option<(f32, f32)> {
    PHOTO.load(Ordering::Relaxed).then(|| {
        (f32::from_bits(PHOTO_FOCUS.load(Ordering::Relaxed)), f32::from_bits(PHOTO_APERTURE.load(Ordering::Relaxed)))
    })
}

/// Whether the depth of field runs for camera mover `mover`: photo mode's
/// aperture while it is up, else the Graphics row's Cinematic in a watched
/// scene ([`dof_scene`]).
pub fn dof_wanted(mover: u32, opts: &crate::render::RenderOptions) -> bool {
    match photo_dof() {
        Some((_, n)) => n > 0.0,
        None => dof_on(opts) && dof_scene(mover),
    }
}

/// The focus band for photo mode (the car's own `band` when its focus is
/// Auto; widened by the f-number either way), or `band` outside it.
pub fn photo_band(band: (f32, f32)) -> (f32, f32) {
    match photo_dof() {
        Some((focus, n)) if n > 0.0 => {
            let at = if focus > 0.0 { focus } else { (band.0 + band.1) * 0.5 };
            let b = crate::photo::focus_band(at, n);
            if focus > 0.0 { b } else { (b.0.min(band.0), b.1.max(band.1)) }
        }
        _ => band,
    }
}

/// Whether the world view drawn by camera mover `mover` (`Game::camera_mover`:
/// 1 the chase camera, 0x0F/0x10 crash/jump, 0x12 the replay camera, 0x14
/// the showcase, 0x100 + shot a scene's camera track) is watched rather
/// than driven: depth of field runs. Never behind the chase, crash or jump
/// cameras; not the showcase (the magazine picture is the original's).
pub fn dof_scene(mover: u32) -> bool {
    mover >= 0x100 || mover == 0x12 || PHOTO.load(Ordering::Relaxed) || dof_always()
}

/// `NFSU2_DOF_ALWAYS=1` (measuring aid, not an option): depth of field
/// behind every camera, to time it in the driving benches.
fn dof_always() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| crate::render::debug_env::nfsu2_dof_always())
}

/// Depth of field in force (the Graphics page's Cinematic, from the
/// frame's settings).
pub fn dof_on(opts: &crate::render::RenderOptions) -> bool {
    opts.depth_of_field != 0
}

/// A drawn car for the motion pass.
#[derive(Clone, Copy, Debug)]
pub struct BlurCar {
    /// Key, drawn pose, box and wheels (as the TAA pass's).
    pub car: temporal::Car,
    /// The view's own car: its body stays sharp.
    pub player: bool,
    /// Each wheel's spin, rad/s.
    pub spin: [f32; 4],
}

/// The last drawn frame (or, for one still, the frame [`SHUTTER`] ago made
/// from the velocities).
#[derive(Clone, Debug)]
pub struct Last {
    pub key: u64,
    pub camera: Camera,
    pub aspect: f32,
    pub cars: HashMap<u64, Mat4>,
    /// Drawn time, s.
    pub time: f64,
}

/// What the motion pass needs of a drawn frame.
#[derive(Clone, Debug)]
pub struct MotionFrame {
    pub mode: MotionMode,
    /// The main view's camera as drawn (with the TAA jitter, if any).
    pub camera: Camera,
    pub aspect: f32,
    pub reversed: bool,
    pub cars: Vec<BlurCar>,
    /// What draws the view (the TAA pass's `temporal_key`): a change restarts.
    pub key: u64,
    /// The drawn time (s): the world's time at the drawn poses.
    pub time: f64,
    /// A still with no last frame: this one stands in (made from the
    /// velocities, at an earlier `time`).
    pub assumed_last: Option<Last>,
    /// The motion from the ticks (None: from the last drawn frame).
    pub tick: Option<TickMotion>,
}

/// A frame's motion taken from the ticks instead of the last drawn frame,
/// so the blur of a frame on a tick is the same at every display rate:
/// `last` is the drawn state one tick before the frame's (None: no blur
/// this frame — a cut, a new view, no tick before), `phase` the noise
/// phase, the number of ticks the blur drew on so far (what the drawn-frame
/// counter is at a steady 60 Hz).
#[derive(Clone, Debug)]
pub struct TickMotion {
    pub last: Option<Last>,
    pub phase: u32,
}

/// Whether the 60 Hz frame of a tick drew the motion blur, its tick's
/// state `cur` and the tick's before `prev` (the drawn-frame path's test:
/// the same view, no cut, a time step the shutter takes).
pub fn tick_draws(prev: &Last, cur: &Last) -> bool {
    prev.key == cur.key && !temporal::is_cut(&prev.camera, &cur.camera) && shutter_scale(cur.time - prev.time).is_some()
}

/// What the depth of field needs of a frame.
#[derive(Clone, Copy, Debug)]
pub struct DofFrame {
    pub camera: Camera,
    pub aspect: f32,
    pub reversed: bool,
    /// The depth range kept sharp (distance from the eye, m).
    pub focus: (f32, f32),
}

/// The distance range (m) from `eye` over which a car with box `lo..hi`
/// (car space) drawn at `transform` lies: all of it is in focus. A margin
/// of 0.3 m before and 0.6 m behind; never nearer than 0.3 m.
pub fn focus_band(eye: Vec3, transform: Mat4, lo: Vec3, hi: Vec3) -> (f32, f32) {
    let mut near = f32::INFINITY;
    let mut far = 0.0f32;
    for i in 0..8 {
        let c = Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        );
        let d = transform.transform_point3(c).distance(eye);
        near = near.min(d);
        far = far.max(d);
    }
    // inside the box (a bonnet camera): from the eye
    let local = transform.inverse().transform_point3(eye);
    if local.cmpge(lo).all() && local.cmple(hi).all() {
        near = 0.0;
    }
    ((near - 0.3).max(0.3), far + 0.6)
}

/// The car the scene's camera looks at: of `cars` (transform, box) the one
/// nearest the view's axis (by angle) within 80 m and in front, else the
/// first (the player's).
pub fn focus_car(camera: &Camera, cars: &[(Mat4, Vec3, Vec3)]) -> Option<usize> {
    let f = camera.forward();
    let mut best = None;
    let mut best_cos = 0.8f32;
    for (i, (t, lo, hi)) in cars.iter().enumerate() {
        let c = t.transform_point3((*lo + *hi) * 0.5) - camera.position;
        let d = c.length();
        if !(0.5..=80.0).contains(&d) {
            continue;
        }
        let cos = c.dot(f) / d;
        if cos > best_cos {
            best_cos = cos;
            best = Some(i);
        }
    }
    best.or((!cars.is_empty()).then_some(0))
}

/// The matrices of a drawn frame against the last: clip (as drawn) →
/// camera-relative world, camera-relative world → the last frame's clip
/// (with this frame's jitter, so a sub-pixel jitter is no motion), and
/// camera-relative world → this frame's clip.
pub fn matrices(cur: &Camera, aspect: f32, reversed: bool, last: &Camera, last_aspect: f32) -> (Mat4, Mat4, Mat4) {
    let rot = |c: &Camera| Mat4::look_to_rh(Vec3::ZERO, c.forward(), c.up);
    let rel_to_clip = cur.projection_depth(aspect, reversed) * rot(cur);
    let mut prev = *last;
    prev.jitter = cur.jitter;
    let prev_rel_to_clip = prev.projection_depth(last_aspect, reversed)
        * rot(&prev)
        * Mat4::from_translation(cur.position - prev.position);
    (rel_to_clip.inverse(), prev_rel_to_clip, rel_to_clip)
}

/// A car's motion: camera-relative point now → where it was (relative to
/// this frame's eye), from its last pose `prev` and this one `now`.
pub fn car_motion(eye: Vec3, now: Mat4, prev: Mat4) -> Mat4 {
    let to_rel = Mat4::from_translation(-eye);
    to_rel * prev * (to_rel * now).inverse()
}

/// A pose `dt` seconds before one moving at `velocity` (m/s) and turning
/// at `spin` (rad/s, world): a still's stand-in for the last frame.
pub fn pose_before(now: Mat4, velocity: Vec3, spin: Vec3, dt: f32) -> Mat4 {
    let (scale, rot, pos) = now.to_scale_rotation_translation();
    let turn = if spin.length_squared() > 1e-12 {
        glam::Quat::from_axis_angle(spin.normalize(), -spin.length() * dt)
    } else {
        glam::Quat::IDENTITY
    };
    Mat4::from_scale_rotation_translation(scale, turn * rot, pos - velocity * dt)
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuCar {
    inv: [[f32; 4]; 4],
    fwd: [[f32; 4]; 4],
    motion: [[f32; 4]; 4],
    lo: [f32; 4],
    hi: [f32; 4],
    wheel: [[f32; 4]; 4],
    wheel_hw: [f32; 4],
    spin: [f32; 4],
    sphere: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    clip_to_rel: [[f32; 4]; 4],
    prev_rel_to_clip: [[f32; 4]; 4],
    rel_to_clip: [[f32; 4]; 4],
    size: [f32; 4],
    blur: [f32; 4],
    tiles: [f32; 4],
    dof: [f32; 4],
    misc: [f32; 4],
}

/// The per-car values the shader gets for `frame` against `last`, nearest
/// first, at most [`temporal::MAX_CARS`]: (GPU car, the mask kind) — for the
/// tests too. `car_scale`: the shutter factor of a car's own motion (0 in
/// the Camera tier: the camera's motion only).
fn gpu_cars(frame: &MotionFrame, last: &Last, car_scale: f32) -> Vec<GpuCar> {
    let eye = frame.camera.position;
    let forward = frame.camera.forward();
    let to_rel = Mat4::from_translation(-eye);
    let per_object = frame.mode == MotionMode::PerObject;
    let mut out = Vec::new();
    for b in &frame.cars {
        if out.len() == temporal::MAX_CARS {
            break;
        }
        let c = &b.car;
        let centre = c.transform.transform_point3((c.lo + c.hi) * 0.5) - eye;
        let radius = (c.hi - c.lo).length() * 0.5;
        if centre.dot(forward) < -radius {
            continue;
        }
        let fwd = to_rel * c.transform;
        // a car with no last pose (new, or jumped) moves with the camera
        let (motion, scale) = match last
            .cars
            .get(&c.key)
            .filter(|p| p.w_axis.truncate().distance(c.transform.w_axis.truncate()) < temporal::CUT_DISTANCE)
        {
            Some(prev) if per_object => (car_motion(eye, c.transform, *prev), car_scale),
            _ => (Mat4::IDENTITY, 0.0),
        };
        // the player's wheels: masked in the Camera tier, spun in Per-Object
        let spin = if per_object { b.spin.map(|w| (w * SHUTTER).clamp(-MAX_ARC, MAX_ARC)) } else { [0.0; 4] };
        out.push(GpuCar {
            inv: fwd.inverse().to_cols_array_2d(),
            fwd: fwd.to_cols_array_2d(),
            motion: motion.to_cols_array_2d(),
            lo: [c.lo.x, c.lo.y, c.lo.z, if b.player { 1.0 } else { 0.0 }],
            hi: [c.hi.x, c.hi.y, c.hi.z, scale],
            wheel: c.wheels.map(|(p, r, _)| [p.x, p.y, p.z, r]),
            wheel_hw: c.wheels.map(|(_, _, hw)| hw),
            spin,
            sphere: [centre.x, centre.y, centre.z, radius * radius],
        });
    }
    out
}

/// Pipelines, targets and the motion history.
pub struct BlurDof {
    layouts: [wgpu::BindGroupLayout; 2],
    modules: [(wgpu::ShaderModule, wgpu::PipelineLayout); 2],
    pipelines: HashMap<(&'static str, wgpu::TextureFormat, u32, bool), wgpu::RenderPipeline>,
    copy_layout: wgpu::BindGroupLayout,
    copy_module: (wgpu::ShaderModule, wgpu::PipelineLayout),
    copies: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    motion_uniforms: wgpu::Buffer,
    dof_uniforms: wgpu::Buffer,
    cars: wgpu::Buffer,
    sampler: wgpu::Sampler,
    dummy: wgpu::TextureView,
    /// size, scene format, tile: the scene's copy, velocity (or distance and
    /// circle of confusion), tiles, neighbours, the half-size gather, the
    /// half-size scene, the half-size distance and circle of confusion.
    /// The scene copy and the velocity / circle of confusion are the
    /// pool's [`crate::render::pool::Slot::SceneCopy`] / [`crate::render::pool::Slot::SceneAux`]
    /// (both cleared and rewritten in full before each read), held by the
    /// last element.
    #[allow(clippy::type_complexity)]
    targets: Option<(
        (u32, u32),
        wgpu::TextureFormat,
        u32,
        [wgpu::TextureView; 8],
        [std::sync::Arc<crate::render::pool::Target>; 2],
    )>,
    last: Option<Last>,
    frame: u32,
    /// The (format, samples, motion, dof) [`BlurDof::warm`] last built
    /// the pipelines for.
    warmed: Option<(wgpu::TextureFormat, u32, bool, bool)>,
}

const COPY_SHADER: &str = "
@group(0) @binding(0) var src: texture_2d<f32>;
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fs_copy(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(frag.xy), 0);
}
";

/// `NFSU2_BLUR_DEBUG=1`: the motion pass shows its velocity (R, G |x|, |y|
/// / 32 px), masked pixels red, wheels green; the depth of field its
/// circle of confusion (R) and the near field (G). A tuning aid.
fn debug_view() -> f32 {
    static V: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *V.get_or_init(|| crate::render::debug_env::nfsu2_blur_debug().unwrap_or(0.0))
}

impl BlurDof {
    pub fn new(gpu: &Gpu) -> BlurDof {
        let device = &gpu.device;
        let layout = |multisampled: bool| {
            let tex = |binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            };
            let buffer = |binding, size: usize| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size as u64),
                },
                count: None,
            };
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("blur / dof"),
                entries: &[
                    tex(0),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled,
                        },
                        count: None,
                    },
                    tex(2),
                    tex(3),
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    buffer(5, std::mem::size_of::<Uniforms>()),
                    buffer(6, std::mem::size_of::<GpuCar>() * temporal::MAX_CARS),
                ],
            })
        };
        let layouts = [layout(false), layout(true)];
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/blur_dof.wgsl"));
        let modules = [false, true].map(|ms| {
            // The multisampled variant reads sample 0 of the depth.
            let src = if ms {
                source.replace("texture_depth_2d", "texture_depth_multisampled_2d")
            } else {
                source.to_string()
            };
            let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("blur / dof"),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("blur / dof"),
                bind_group_layouts: &[Some(&layouts[usize::from(ms)])],
                immediate_size: 0,
            });
            (shader, pl)
        });
        let copy_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("blur / dof copy"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let copy_module = (
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("blur / dof copy"),
                source: wgpu::ShaderSource::Wgsl(COPY_SHADER.into()),
            }),
            crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("blur / dof copy"),
                bind_group_layouts: &[Some(&copy_layout)],
                immediate_size: 0,
            }),
        );
        let uniform = |label| {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor {
                    label: Some(label),
                    size: std::mem::size_of::<Uniforms>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            )
        };
        let cars = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("blur cars"),
                size: (std::mem::size_of::<GpuCar>() * temporal::MAX_CARS) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blur / dof"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let dummy = target(gpu, "blur / dof dummy", (1, 1), AUX_FORMAT);
        BlurDof {
            layouts,
            modules,
            pipelines: HashMap::new(),
            copy_layout,
            copy_module,
            copies: HashMap::new(),
            motion_uniforms: uniform("blur"),
            dof_uniforms: uniform("dof"),
            cars,
            sampler,
            dummy,
            targets: None,
            last: None,
            frame: 0,
            warmed: None,
        }
    }

    /// Forget the last frame (a frame drawn without the motion pass).
    pub fn reset(&mut self) {
        self.last = None;
        set_sprite_dof(None);
    }

    /// Every pipeline the motion pass (`motion`) and the depth of
    /// field (`dof`) use for a `format` scene of `samples` samples, built
    /// now (the frame the passes are first wanted, before the motion pass's
    /// first draw, which needs a frame before it) rather than on the frame
    /// that first draws them. The same pipelines either way; nothing drawn
    /// changes.
    pub fn warm(&mut self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, motion: bool, dof: bool) {
        let key = (format, samples, motion, dof);
        if self.warmed == Some(key) {
            return;
        }
        self.warmed = Some(key);
        let ms = samples > 1;
        let mut used: Vec<(&'static str, wgpu::TextureFormat, u32)> = Vec::new();
        if motion {
            used.extend([
                ("fs_velocity", AUX_FORMAT, 1),
                ("fs_tile_rows", AUX_FORMAT, 1),
                ("fs_tile", AUX_FORMAT, 1),
                ("fs_neighbour", AUX_FORMAT, 1),
                ("fs_gather", format, samples),
            ]);
        }
        if dof {
            used.extend([
                ("fs_coc", AUX_FORMAT, 1),
                ("fs_down", AUX_FORMAT, 1),
                ("fs_coc_half", AUX_FORMAT, 1),
                ("fs_dof_half", AUX_FORMAT, 1),
                ("fs_dof", format, samples),
            ]);
        }
        for (entry, f, n) in used {
            self.pipeline(gpu, entry, f, n, ms);
        }
        if (motion || dof) && !ms {
            self.copy_pipeline(gpu, format);
        }
    }

    /// The scene copy's pipeline into `format` (single-sampled scenes).
    fn copy_pipeline(&mut self, gpu: &Gpu, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        let (shader, pl) = &self.copy_module;
        self.copies.entry(format).or_insert_with(|| {
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some("blur / dof copy"),
                layout: Some(pl),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_copy"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        })
    }

    fn pipeline(
        &mut self,
        gpu: &Gpu,
        entry: &'static str,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        let (shader, pl) = &self.modules[usize::from(ms)];
        self.pipelines
            .entry((entry, format, samples, ms))
            .or_insert_with(|| {
                // The single-sampled gather skips the pixels it leaves.
                let skip: &[(&str, f64)] =
                    if entry == "fs_gather" && samples == 1 { &[("SKIP_UNCHANGED", 1.0)] } else { &[] };
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
                        compilation_options: wgpu::PipelineCompilationOptions { constants: skip, ..Default::default() },
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                    multiview_mask: None,
                    cache: gpu.disk_pipeline_cache(),
                })
            })
            .clone()
    }

    /// The scene (`samples` samples) resolved or copied into the copy the
    /// gathers read; the targets made for `size` / `format`.
    fn prepare(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        samples: u32,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> [wgpu::TextureView; 8] {
        let tile = tile_size(size);
        if self.targets.as_ref().is_none_or(|(s, f, t, _, _)| *s != size || *f != format || *t != tile) {
            let tiles = tile_count(size, tile);
            // Drop the old ones first (a pooled target lives while held).
            self.targets = None;
            let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
            let copy = gpu.pool.get(
                &gpu.device,
                crate::render::pool::Slot::SceneCopy,
                "blur / dof scene copy",
                size,
                format,
                1,
                usage,
            );
            let velocity = gpu.pool.get(
                &gpu.device,
                crate::render::pool::Slot::SceneAux,
                "blur velocity",
                size,
                AUX_FORMAT,
                1,
                usage,
            );
            self.targets = Some((
                size,
                format,
                tile,
                [
                    copy.view.clone(),
                    velocity.view.clone(),
                    target(gpu, "blur tiles", tiles, AUX_FORMAT),
                    target(gpu, "blur neighbours", tiles, AUX_FORMAT),
                    target(gpu, "dof half", half(size), AUX_FORMAT),
                    target(gpu, "dof half scene", half(size), AUX_FORMAT),
                    target(gpu, "dof half coc", half(size), AUX_FORMAT),
                    target(gpu, "blur tile rows", tile_rows(tiles, tile), AUX_FORMAT),
                ],
                [copy, velocity],
            ));
        }
        let views = self.targets.as_ref().unwrap().3.clone();
        if samples > 1 {
            // An empty pass that resolves the multisampled scene.
            let p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur / dof resolve"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene,
                    depth_slice: None,
                    resolve_target: Some(&views[0]),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            drop(p);
        } else {
            let pipeline = self.copy_pipeline(gpu, format).clone();
            let pipeline = &pipeline;
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("blur / dof copy"),
                layout: &self.copy_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(scene) }],
            });
            pass(
                encoder,
                "blur / dof copy",
                &views[0],
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                pipeline,
                &group,
            );
        }
        views
    }

    fn group(
        &self,
        gpu: &Gpu,
        ms: bool,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        a: &wgpu::TextureView,
        b: &wgpu::TextureView,
        uniforms: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur / dof"),
            layout: &self.layouts[usize::from(ms)],
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(color) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(a) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(b) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: self.cars.as_entire_binding() },
            ],
        })
    }

    /// The motion blur of `frame` over `scene` (`samples` samples, `size`
    /// big, `format`; `depth` its depth). Returns whether it drew (no
    /// last frame, a cut, a pause: nothing).
    #[allow(clippy::too_many_arguments)]
    pub fn motion(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        samples: u32,
        size: (u32, u32),
        format: wgpu::TextureFormat,
        frame: &MotionFrame,
    ) -> bool {
        // The depth of field (if it runs this frame) comes after and sets it again.
        set_sprite_dof(None);
        let this = Last {
            key: frame.key,
            camera: frame.camera.unjittered(),
            aspect: frame.aspect,
            cars: frame.cars.iter().map(|c| (c.car.key, c.car.transform)).collect(),
            time: frame.time,
        };
        if frame.mode == MotionMode::Original {
            self.last = None;
            return false;
        }
        let stored = std::mem::replace(&mut self.last, Some(this));
        let last = match &frame.tick {
            Some(t) if frame.assumed_last.is_none() => t.last.clone(),
            _ => frame.assumed_last.clone().or(stored),
        };
        let last = match last {
            Some(l) if l.key == frame.key && !temporal::is_cut(&l.camera, &frame.camera) => l,
            _ => return false,
        };
        let Some(scale) = shutter_scale(frame.time - last.time) else { return false };
        let dt = (frame.time - last.time) as f32;
        let speed = frame.camera.position.distance(last.camera.position) / dt;
        let camera_scale = scale * speed_ramp(speed);
        let cars = gpu_cars(frame, &last, scale);
        let (clip_to_rel, prev_rel_to_clip, rel_to_clip) =
            matrices(&frame.camera, frame.aspect, frame.reversed, &last.camera, last.aspect);
        let tile = tile_size(size);
        let tiles = tile_count(size, tile);
        self.frame = self.frame.wrapping_add(1);
        let noise = frame.tick.as_ref().map_or(self.frame, |t| t.phase);
        let u = Uniforms {
            clip_to_rel: clip_to_rel.to_cols_array_2d(),
            prev_rel_to_clip: prev_rel_to_clip.to_cols_array_2d(),
            rel_to_clip: rel_to_clip.to_cols_array_2d(),
            size: [size.0 as f32, size.1 as f32, 1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32],
            blur: [
                camera_scale,
                max_blur(size),
                cars.len() as f32,
                if frame.mode == MotionMode::PerObject { 2.0 } else { 1.0 },
            ],
            tiles: [tile as f32, tiles.0 as f32, tiles.1 as f32, MOTION_TAPS as f32],
            dof: [0.0; 4],
            misc: [if frame.reversed { 1.0 } else { 0.0 }, (noise % 64) as f32, debug_view(), 0.0],
        };
        gpu.queue.write_buffer(&self.motion_uniforms, 0, bytemuck::bytes_of(&u));
        let mut all: Vec<GpuCar> = vec![bytemuck::Zeroable::zeroed(); temporal::MAX_CARS];
        all[..cars.len()].copy_from_slice(&cars);
        gpu.queue.write_buffer(&self.cars, 0, bytemuck::cast_slice::<GpuCar, u8>(&all));
        let [copy, velocity, tile_max, neighbours, _, _, _, rows] =
            self.prepare(gpu, encoder, scene, samples, size, format);
        crate::prof::gpu::mark(encoder, "graphics: motion blur: resolve");
        let ms = samples > 1;
        let dummy = self.dummy.clone();
        let p_velocity = self.pipeline(gpu, "fs_velocity", AUX_FORMAT, 1, ms);
        let p_rows = self.pipeline(gpu, "fs_tile_rows", AUX_FORMAT, 1, ms);
        let p_tile = self.pipeline(gpu, "fs_tile", AUX_FORMAT, 1, ms);
        let p_neighbour = self.pipeline(gpu, "fs_neighbour", AUX_FORMAT, 1, ms);
        let p_gather = self.pipeline(gpu, "fs_gather", format, samples, ms);
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let g = self.group(gpu, ms, &copy, depth, &dummy, &dummy, &self.motion_uniforms);
        pass(encoder, "blur velocity", &velocity, clear, &p_velocity, &g);
        crate::prof::gpu::mark(encoder, "graphics: motion blur: velocity");
        // The tile max in two steps (each sampled row's, then the
        // rows'), the same pick as one loop over the tile.
        let g = self.group(gpu, ms, &copy, depth, &velocity, &dummy, &self.motion_uniforms);
        pass(encoder, "blur tile rows", &rows, clear, &p_rows, &g);
        let g = self.group(gpu, ms, &copy, depth, &rows, &dummy, &self.motion_uniforms);
        pass(encoder, "blur tile max", &tile_max, clear, &p_tile, &g);
        let g = self.group(gpu, ms, &copy, depth, &tile_max, &dummy, &self.motion_uniforms);
        pass(encoder, "blur neighbour max", &neighbours, clear, &p_neighbour, &g);
        crate::prof::gpu::mark(encoder, "graphics: motion blur: tiles");
        let g = self.group(gpu, ms, &copy, depth, &velocity, &neighbours, &self.motion_uniforms);
        pass(encoder, "blur gather", scene, wgpu::LoadOp::Load, &p_gather, &g);
        crate::prof::gpu::mark(encoder, "graphics: motion blur: gather");
        true
    }

    /// The depth of field of `frame` over `scene` (as [`BlurDof::motion`]).
    #[allow(clippy::too_many_arguments)]
    pub fn dof(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        samples: u32,
        size: (u32, u32),
        format: wgpu::TextureFormat,
        frame: &DofFrame,
    ) {
        let rot = Mat4::look_to_rh(Vec3::ZERO, frame.camera.forward(), frame.camera.up);
        let proj = frame.camera.projection_depth(frame.aspect, frame.reversed);
        let rel_to_clip = proj * rot;
        self.frame = self.frame.wrapping_add(1);
        let max_coc = (MAX_COC * size.1 as f32 * photo_dof().map_or(1.0, |(_, n)| crate::photo::coc_scale(n))).max(2.0);
        // The glow sprites drawn after this pass take the same blur (`defocus_quads`).
        set_sprite_dof(Some(SpriteDof {
            eye: frame.camera.position,
            focus: frame.focus,
            max_coc,
            px_per_m: proj.y_axis.y * size.1 as f32 * 0.5,
        }));
        let u = Uniforms {
            clip_to_rel: rel_to_clip.inverse().to_cols_array_2d(),
            prev_rel_to_clip: rel_to_clip.to_cols_array_2d(),
            rel_to_clip: rel_to_clip.to_cols_array_2d(),
            size: [size.0 as f32, size.1 as f32, 1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32],
            blur: [0.0; 4],
            tiles: [1.0, 1.0, 1.0, 0.0],
            dof: [
                frame.focus.0,
                frame.focus.1,
                max_coc,
                DOF_TAPS as f32,
            ],
            misc: [if frame.reversed { 1.0 } else { 0.0 }, (self.frame % 64) as f32, debug_view(), 0.0],
        };
        gpu.queue.write_buffer(&self.dof_uniforms, 0, bytemuck::bytes_of(&u));
        let [copy, coc, _, _, gathered, small, small_coc, _] = self.prepare(gpu, encoder, scene, samples, size, format);
        crate::prof::gpu::mark(encoder, "graphics: depth of field: resolve");
        let ms = samples > 1;
        let dummy = self.dummy.clone();
        let p_coc = self.pipeline(gpu, "fs_coc", AUX_FORMAT, 1, ms);
        let p_down = self.pipeline(gpu, "fs_down", AUX_FORMAT, 1, ms);
        let p_coc_half = self.pipeline(gpu, "fs_coc_half", AUX_FORMAT, 1, ms);
        let p_half = self.pipeline(gpu, "fs_dof_half", AUX_FORMAT, 1, ms);
        let p = self.pipeline(gpu, "fs_dof", format, samples, ms);
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let g = self.group(gpu, ms, &copy, depth, &dummy, &dummy, &self.dof_uniforms);
        pass(encoder, "depth of field: circle of confusion", &coc, clear, &p_coc, &g);
        pass(encoder, "depth of field: half-size scene", &small, clear, &p_down, &g);
        let g = self.group(gpu, ms, &copy, depth, &coc, &dummy, &self.dof_uniforms);
        pass(encoder, "depth of field: half-size circle of confusion", &small_coc, clear, &p_coc_half, &g);
        crate::prof::gpu::mark(encoder, "graphics: depth of field: coc, half scene");
        let g = self.group(gpu, ms, &small, depth, &small_coc, &dummy, &self.dof_uniforms);
        pass(encoder, "depth of field: gather (half size)", &gathered, clear, &p_half, &g);
        crate::prof::gpu::mark(encoder, "graphics: depth of field: gather");
        let g = self.group(gpu, ms, &copy, depth, &coc, &gathered, &self.dof_uniforms);
        pass(encoder, "depth of field", scene, wgpu::LoadOp::Load, &p, &g);
        crate::prof::gpu::mark(encoder, "graphics: depth of field: composite");
    }
}

/// The depth of field of the frame being drawn, for the light sprites drawn after its pass (without it
/// the native glow sprites come after the blur, in the scene or the light-streak buffer, so out-of-focus
/// lamps stayed sharp, hard-edged discs over the blurred background).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteDof {
    /// The eye (world).
    pub eye: Vec3,
    /// The depth range kept sharp (m) and the largest circle of confusion (px), as the pass's.
    pub focus: (f32, f32),
    pub max_coc: f32,
    /// Pixels per metre at 1 m from the eye (the scene's height / (2 tan(fov / 2))).
    pub px_per_m: f32,
}

static SPRITE_DOF: std::sync::Mutex<Option<SpriteDof>> = std::sync::Mutex::new(None);

fn set_sprite_dof(d: Option<SpriteDof>) {
    if let Ok(mut s) = SPRITE_DOF.lock() {
        *s = d;
    }
}

/// This frame's depth of field for the light sprites, taken (once: a frame without the pass has none).
pub fn take_sprite_dof() -> Option<SpriteDof> {
    SPRITE_DOF.lock().ok().and_then(|mut s| s.take())
}

/// blur_dof.wgsl's `coc`: the circle of confusion (px) at distance `z` (m).
pub fn coc_px(z: f32, focus: (f32, f32), max_coc: f32) -> f32 {
    let (near, far) = focus;
    if z > far {
        max_coc * ((z - far) / z.max(1e-3) * 1.6).clamp(0.0, 1.0)
    } else if z < near {
        max_coc * ((near - z) / z.max(0.05) * 1.2).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Taps of a light sprite's circle of confusion at most ([`defocus_quads`]).
pub const SPRITE_TAPS: usize = 12;

/// The light sprites (glow quads; additive) defocused by the circle of confusion at each one's
/// depth, as the pass blurs the scene there: an out-of-focus sprite is drawn as copies spread
/// evenly over its circle (a Vogel disc in the sprite's own plane), each with its share of the
/// light, so their sum is the sprite convolved with the disc: a soft disc of the same energy
/// (bokeh) instead of a sharp, hard-edged one. In-focus sprites (and circles under a pixel) stay
/// as they are, in order; the taps are shared out so the list stays within `cap` quads.
pub fn defocus_quads(quads: &[crate::particles::Quad], d: &SpriteDof, cap: usize) -> Vec<crate::particles::Quad> {
    let quads = &quads[..quads.len().min(cap)];
    // Each quad's circle (in the quad's plane, m) or none.
    let circles: Vec<Option<f32>> = quads
        .iter()
        .map(|q| {
            let centre = (q.corners[0] + q.corners[1] + q.corners[2] + q.corners[3]) * 0.25;
            let z = centre.distance(d.eye);
            let coc = coc_px(z, d.focus, d.max_coc);
            (coc >= 1.0 && z > 1e-3).then(|| coc * z / d.px_per_m.max(1e-3))
        })
        .collect();
    let blurred = circles.iter().filter(|c| c.is_some()).count();
    if blurred == 0 {
        return quads.to_vec();
    }
    let taps = ((cap - (quads.len() - blurred)) / blurred).clamp(1, SPRITE_TAPS);
    let mut out = Vec::with_capacity(quads.len() + blurred * (taps - 1));
    for (q, circle) in quads.iter().zip(&circles) {
        let Some(radius) = circle.filter(|_| taps > 1) else {
            out.push(*q);
            continue;
        };
        let ax = (q.corners[1] - q.corners[0]).normalize_or_zero();
        let ay = (q.corners[3] - q.corners[0]).normalize_or_zero();
        for i in 0..taps {
            // Vogel's disc: equal areas, golden-angle turns.
            let r = radius * ((i as f32 + 0.5) / taps as f32).sqrt();
            let a = i as f32 * 2.399_963;
            let off = (ax * a.cos() + ay * a.sin()) * r;
            let mut c = *q;
            for p in &mut c.corners {
                *p += off;
            }
            // The light shared out in whole steps (the first copies take the remainder).
            for (ch, v) in c.rgba[..3].iter_mut().zip(q.rgba) {
                let v = usize::from(v);
                *ch = (v / taps + usize::from(i < v % taps)) as u8;
            }
            out.push(c);
        }
    }
    out
}

/// The longest streak for a scene `size` big (px).
pub fn max_blur(size: (u32, u32)) -> f32 {
    (MAX_BLUR * size.1 as f32).max(4.0)
}

/// The tile size: at least half the longest streak (a streak reaches no
/// further than the next tile), at least 8.
pub fn tile_size(size: (u32, u32)) -> u32 {
    ((max_blur(size) * 0.5).ceil() as u32).max(8)
}

fn half(size: (u32, u32)) -> (u32, u32) {
    (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1))
}

fn tile_count(size: (u32, u32), tile: u32) -> (u32, u32) {
    (size.0.div_ceil(tile).max(1), size.1.div_ceil(tile).max(1))
}

/// The tile rows' target: per tile column, each tile's sampled rows
/// (every other one, `tile.div_ceil(2)` a tile) one under the other.
fn tile_rows(tiles: (u32, u32), tile: u32) -> (u32, u32) {
    (tiles.0, tiles.1 * tile.div_ceil(2))
}

fn target(gpu: &Gpu, label: &str, size: (u32, u32), format: wgpu::TextureFormat) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
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
    p.set_pipeline(pipeline);
    p.set_bind_group(0, group, &[]);
    p.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec2};

    fn camera(eye: Vec3, yaw: f32) -> Camera {
        let mut c = Camera::new(eye, yaw, -0.05);
        c.game_fov = Some(1.248_277);
        c
    }

    /// Pixel position of world point `p` seen by `c`.
    fn px(c: &Camera, aspect: f32, size: (u32, u32), p: Vec3) -> Vec2 {
        let clip = c.view_projection_depth(aspect, false) * p.extend(1.0);
        Vec2::new((clip.x / clip.w * 0.5 + 0.5) * size.0 as f32, (0.5 - clip.y / clip.w * 0.5) * size.1 as f32)
    }

    /// A light sprite out of focus is spread over the circle of confusion at its
    /// depth with its light kept (the copies' colours sum to the sprite's), its copies within the
    /// circle; in focus it is unchanged; the list stays within the cap; the CPU circle is the shader's.
    #[test]
    fn light_sprites_take_the_blur_of_their_depth() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/blur_dof.wgsl"));
        assert!(src.contains("return m * clamp((z - far) / max(z, 1e-3) * 1.6, 0.0, 1.0);"));
        assert!(src.contains("return m * clamp((near - z) / max(z, 0.05) * 1.2, 0.0, 1.0);"));
        // The gather's bright-tap weight: up to 1.3 within the display range, falling over white (Karis).
        assert!(src.contains("let b = w * (1.0 + 2.0 * clamp(l - 0.85, 0.0, 0.15)) / (1.0 + max(l - 1.0, 0.0));"));
        let d = SpriteDof { eye: Vec3::ZERO, focus: (8.0, 12.0), max_coc: 26.0, px_per_m: 1300.0 };
        let quad = |y: f32| crate::particles::Quad {
            texture: 1,
            corners: [
                Vec3::new(-0.3, y, -0.3),
                Vec3::new(0.3, y, -0.3),
                Vec3::new(0.3, y, 0.3),
                Vec3::new(-0.3, y, 0.3),
            ],
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            rgba: [200, 181, 7, 255],
        };
        let q = defocus_quads(&[quad(10.0), quad(40.0), quad(3.0)], &d, 1000);
        // In focus: as given, first.
        assert_eq!(q.len(), 1 + 2 * SPRITE_TAPS);
        assert_eq!(q[0].corners, quad(10.0).corners);
        assert_eq!(q[0].rgba, quad(10.0).rgba);
        for (k, y) in [(1usize, 40.0f32), (2, 3.0)] {
            let copies = &q[1 + (k - 1) * SPRITE_TAPS..1 + k * SPRITE_TAPS];
            for ch in 0..3 {
                let sum: u32 = copies.iter().map(|c| u32::from(c.rgba[ch])).sum();
                assert_eq!(sum, u32::from(quad(y).rgba[ch]), "light kept");
            }
            let radius = coc_px(y, d.focus, d.max_coc) * y / d.px_per_m;
            assert!(radius > 0.0);
            let mut far = 0.0f32;
            for c in copies {
                let off = c.corners[0] - quad(y).corners[0];
                assert!(off.y.abs() < 1e-5 && off.length() <= radius * 1.0001, "in the circle, in the sprite's plane");
                far = far.max(off.length());
            }
            assert!(far > radius * 0.8, "spread over it");
        }
        // The cap: the taps shared out, in-focus ones kept.
        let many: Vec<_> = (0..300).map(|i| quad(if i % 3 == 0 { 10.0 } else { 40.0 })).collect();
        let q = defocus_quads(&many, &d, 1000);
        assert!(q.len() <= 1000 && q.len() >= 300, "{}", q.len());
        // A frame's state is taken once.
        set_sprite_dof(Some(d));
        assert_eq!(take_sprite_dof(), Some(d));
        assert_eq!(take_sprite_dof(), None);
    }

    /// The streak is the motion over the shutter whatever the display
    /// rate: a camera driving at 40 m/s and turning at 0.5 rad/s, drawn at
    /// 60, 144 and 240 Hz, gives the same streak length (within 1 %) for
    /// points 30 to 160 m ahead, through the pass's own matrices.
    #[test]
    fn shutter_is_the_same_at_any_display_rate() {
        let size = (1920u32, 1080u32);
        let aspect = size.0 as f32 / size.1 as f32;
        let at = |t: f32| camera(Vec3::new(0.0, 40.0 * t, 1.5), 0.5 * t);
        let points = [Vec3::new(4.0, 70.0, 0.0), Vec3::new(-6.0, 90.0, 4.0), Vec3::new(20.0, 200.0, 10.0)];
        let mut lengths: Vec<Vec<f32>> = Vec::new();
        for hz in [60.0f32, 144.0, 240.0] {
            let dt = 1.0 / hz;
            let (last, cur) = (at(1.0 - dt), at(1.0));
            let (clip_to_rel, prev_rel_to_clip, _) = matrices(&cur, aspect, false, &last, aspect);
            let scale = shutter_scale(f64::from(dt)).unwrap();
            lengths.push(
                points
                    .iter()
                    .map(|&p| {
                        // the pass: this pixel's point from the depth, back to last frame's pixel
                        let now = px(&cur, aspect, size, p);
                        let clip = cur.view_projection_depth(aspect, false) * p.extend(1.0);
                        let ndc = (clip / clip.w).truncate();
                        let rel = clip_to_rel * ndc.extend(1.0);
                        let rel = rel.truncate() / rel.w;
                        let pc = prev_rel_to_clip * rel.extend(1.0);
                        let prev = Vec2::new(
                            (pc.x / pc.w * 0.5 + 0.5) * size.0 as f32,
                            (0.5 - pc.y / pc.w * 0.5) * size.1 as f32,
                        );
                        // the pass agrees with the last view itself
                        assert!((prev - px(&last, aspect, size, p)).length() < 0.05);
                        (now - prev).length() * scale
                    })
                    .collect(),
            );
        }
        for i in 0..points.len() {
            let l60 = lengths[0][i];
            assert!(l60 > 1.0, "{l60}");
            for l in &lengths[1..] {
                assert!((l[i] / l60 - 1.0).abs() < 0.01, "{} vs {l60}", l[i]);
            }
        }
        // 180° at 60 fps: half the frame's motion.
        assert!((shutter_scale(1.0 / 60.0).unwrap() - 0.5).abs() < 1e-6);
        // A pause (no time) and a stall draw no blur.
        assert_eq!(shutter_scale(0.0), None);
        assert_eq!(shutter_scale(0.5), None);
    }

    #[test]
    fn camera_tier_grows_with_speed() {
        assert_eq!(speed_ramp(0.0), 0.0);
        assert_eq!(speed_ramp(RAMP_FROM), 0.0);
        assert!((speed_ramp((RAMP_FROM + RAMP_TO) * 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(speed_ramp(RAMP_TO), 1.0);
        assert_eq!(speed_ramp(90.0), 1.0);
        assert!(speed_ramp(20.0) < speed_ramp(30.0));
    }

    fn blur_car(key: u64, at: Mat4, player: bool) -> BlurCar {
        let wheels = [
            (Vec3::new(1.3, 0.8, -0.2), 0.33, 0.12),
            (Vec3::new(1.3, -0.8, -0.2), 0.33, 0.12),
            (Vec3::new(-1.3, 0.8, -0.2), 0.33, 0.12),
            (Vec3::new(-1.3, -0.8, -0.2), 0.33, 0.12),
        ];
        BlurCar {
            car: temporal::Car::new(key, at, (Vec3::new(-2.0, -0.9, -0.5), Vec3::new(2.0, 0.9, 0.0)), wheels),
            player,
            spin: [60.0, 60.0, 400.0, -10.0],
        }
    }

    /// The masks: the player's car is flagged sharp in both tiers; in the
    /// Camera tier no car moves on its own and no wheel spins; in
    /// Per-Object a rival moves by its own motion (to where its last pose
    /// put it) and its wheels' arcs are the spin over the shutter (held
    /// to MAX_ARC); a car with no last pose moves with the camera.
    #[test]
    fn masks_and_per_object_motion() {
        let eye = Vec3::new(0.0, 0.0, 2.0);
        let cam = camera(eye, 0.0);
        let pose = |y: f32| Mat4::from_rotation_translation(Quat::from_rotation_z(0.1), Vec3::new(1.0, y, 0.5));
        let player = blur_car(0, pose(6.0), true);
        let rival = blur_car(100, pose(15.0), false);
        let fresh = blur_car(1000, pose(25.0), false);
        let last = Last {
            key: 1,
            camera: cam,
            aspect: 1.5,
            cars: [(0u64, pose(5.5)), (100u64, pose(14.0))].into(),
            time: 0.0,
        };
        let mut frame = MotionFrame {
            mode: MotionMode::Camera,
            camera: cam,
            aspect: 1.5,
            reversed: false,
            cars: vec![player, rival, fresh],
            key: 1,
            time: 1.0 / 60.0,
            assumed_last: None,
            tick: None,
        };
        let g = gpu_cars(&frame, &last, 0.5);
        assert_eq!(g.len(), 3);
        assert_eq!((g[0].lo[3], g[1].lo[3], g[2].lo[3]), (1.0, 0.0, 0.0));
        assert!(g.iter().all(|c| c.hi[3] == 0.0 && c.spin == [0.0; 4]), "camera tier: no own motion, no arcs");
        frame.mode = MotionMode::PerObject;
        let g = gpu_cars(&frame, &last, 0.5);
        assert_eq!(g[1].hi[3], 0.5);
        assert_eq!(g[2].hi[3], 0.0, "no last pose: the camera's motion");
        assert_eq!(g[0].spin[0], 60.0 * SHUTTER);
        assert_eq!(g[0].spin[2], MAX_ARC);
        // The rival's motion takes a point on it to where it was.
        let m = Mat4::from_cols_array_2d(&g[1].motion);
        let local = Vec3::new(1.0, 0.3, 0.2);
        let now = pose(15.0).transform_point3(local) - eye;
        let was = pose(14.0).transform_point3(local) - eye;
        assert!((m.transform_point3(now) - was).length() < 1e-4);
        // The box test the shader does: a point of the rival is inside its
        // box (car space through `inv`), the road under it is not.
        let inv = Mat4::from_cols_array_2d(&g[1].inv);
        let inside = |p: Vec3| {
            let l = inv.transform_point3(p);
            l.cmpge(Vec3::from_slice(&g[1].lo[..3])).all() && l.cmple(Vec3::from_slice(&g[1].hi[..3])).all()
        };
        assert!(inside(now));
        assert!(!inside(pose(15.0).transform_point3(Vec3::new(0.0, 0.0, -0.6)) - eye));
        // A car behind the eye is not sent.
        frame.cars.push(blur_car(1001, pose(-30.0), false));
        assert_eq!(gpu_cars(&frame, &last, 0.5).len(), 3);
    }

    /// A still's stand-in last frame: the pose a moment ago from the
    /// velocities.
    #[test]
    fn still_poses_come_from_the_velocities() {
        let now = Mat4::from_rotation_translation(Quat::from_rotation_z(0.3), Vec3::new(10.0, 5.0, 1.0));
        let before = pose_before(now, Vec3::new(30.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.2), 0.1);
        assert!((before.w_axis.truncate() - Vec3::new(7.0, 5.0, 1.0)).length() < 1e-4);
        let yaw = |m: Mat4| m.x_axis.y.atan2(m.x_axis.x);
        assert!((yaw(before) - (0.3 - 0.12)).abs() < 1e-4);
    }

    /// Depth of field: only in watched scenes (scene camera shots, the
    /// replay camera, photo mode), never behind the chase, crash or jump
    /// cameras or on the showcase; the whole focused car is in focus.
    #[test]
    fn dof_runs_in_scenes_and_keeps_the_car_sharp() {
        let _globals = crate::qol::test_globals();
        assert!(!dof_scene(1) && !dof_scene(0x0F) && !dof_scene(0x10) && !dof_scene(0x14));
        assert!(dof_scene(0x12) && dof_scene(0x100) && dof_scene(0x103));
        set_photo_mode(true);
        assert!(dof_scene(1));
        set_photo_mode(false);
        let eye = Vec3::new(0.0, -8.0, 1.5);
        let t = Mat4::from_rotation_translation(Quat::from_rotation_z(0.7), Vec3::new(0.0, 0.0, 0.5));
        let (lo, hi) = (Vec3::new(-2.2, -0.9, -0.5), Vec3::new(2.2, 0.9, 0.9));
        let (near, far) = focus_band(eye, t, lo, hi);
        for x in [-2.2, 0.0, 2.2] {
            for y in [-0.9, 0.9] {
                let d = t.transform_point3(Vec3::new(x, y, 0.2)).distance(eye);
                assert!(near < d && d < far, "{near} {d} {far}");
            }
        }
        assert!(near > 0.3 && far - near < 8.0);
        // A camera inside the box (bonnet): from the eye on.
        assert_eq!(focus_band(t.w_axis.truncate(), t, lo, hi).0, 0.3);
        // The car the camera looks at, else the first.
        let cam = Camera::looking_at(eye, Vec3::new(6.0, 4.0, 0.5));
        let cars = [
            (Mat4::from_translation(Vec3::new(0.0, 0.0, 0.5)), lo, hi),
            (Mat4::from_translation(Vec3::new(6.0, 4.0, 0.5)), lo, hi),
        ];
        assert_eq!(focus_car(&cam, &cars), Some(1));
        let away = Camera::looking_at(eye, Vec3::new(0.0, -30.0, 0.5));
        assert_eq!(focus_car(&away, &cars), Some(0));
    }

    /// The Original tier is the native path: this module draws nothing.
    /// (The game's side — `blur_and_dof` past its early return, the native
    /// speed blur under Customize Display's switch, the passes after the
    /// lit scene and before the additive layers in the game's frame and the
    /// shots alike — is tested on the game's one world scene,
    /// `draw_world_scene`.)
    #[test]
    fn original_is_untouched_and_defaults_are_native() {
        assert_eq!(MotionMode::from_tier(0), MotionMode::Original);
        assert_eq!(MotionMode::from_tier(1), MotionMode::Camera);
        assert_eq!(MotionMode::from_tier(2), MotionMode::PerObject);
        // Default values are native.
        let e = crate::qol::Enhancements::default();
        assert_eq!((e.motion_blur, e.depth_of_field), (0, 0));
    }

    /// The Graphics rows: Customize Display's values first, the clone's
    /// tiers above them (stored apart, the native value held on).
    #[test]
    fn graphics_rows_share_the_native_switches() {
        use crate::frontend::graphics_table::row;
        use crate::frontend::options::{RowId, Settings};
        let mut s = Settings::default();
        let r = row(RowId::MotionBlur).unwrap();
        assert_eq!(r.tiers.len(), 4);
        (r.set)(&mut s, 0);
        assert_eq!((s.custom[11], s.enh.motion_blur), (0, 0));
        (r.set)(&mut s, 1);
        assert_eq!((s.custom[11], s.enh.motion_blur, (r.get)(&s)), (1, 0, 1));
        (r.set)(&mut s, 3);
        assert_eq!((s.enh.motion_blur, (r.get)(&s)), (2, 3));
        (r.set)(&mut s, 2);
        assert_eq!((s.custom[11], s.enh.motion_blur, (r.get)(&s)), (1, 1, 2));
        let r = row(RowId::DepthOfField).unwrap();
        (r.set)(&mut s, 2);
        assert_eq!((s.enh.depth_of_field, (r.get)(&s)), (1, 2));
        (r.set)(&mut s, 0);
        assert_eq!((s.custom[13], s.enh.depth_of_field), (0, 0));
        // Tile and streak sizes scale with the scene.
        assert_eq!(max_blur((1920, 1080)), 43.2);
        assert!(tile_size((3840, 2160)) as f32 >= max_blur((3840, 2160)) * 0.5);
    }

    /// The two-step tile max (fs_tile_rows, then fs_tile over the
    /// rows) picks the same vector as one scan of the tile (fs_tile before
    /// the two-step split), ties (the first in scan order wins) and partial tiles at the
    /// scene's edge included. A CPU model of both shaders' loops over
    /// random fields with many equal lengths.
    #[test]
    fn two_step_tile_max_matches_one_scan() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let pick = |best: &mut ([f32; 2], f32), v: [f32; 2]| {
            let l = v[0] * v[0] + v[1] * v[1];
            if l > best.1 {
                *best = (v, l);
            }
        };
        for &(w, h, t) in &[(37usize, 23usize, 9usize), (64, 64, 8), (50, 31, 12), (13, 7, 61), (122, 69, 61)] {
            // Few distinct values: many equal lengths, zeros, sign flips.
            let field: Vec<[f32; 2]> =
                (0..w * h).map(|_| [((rnd() % 5) as f32 - 2.0) * 0.5, ((rnd() % 3) as f32 - 1.0) * 0.5]).collect();
            let at = |x: usize, y: usize| field[y * w + x];
            let (tx, ty) = (w.div_ceil(t), h.div_ceil(t));
            let half = t.div_ceil(2);
            for cy in 0..ty {
                for cx in 0..tx {
                    // One scan (the old fs_tile).
                    let mut one = ([0.0f32; 2], 0.0f32);
                    for y in (0..t).step_by(2) {
                        for x in (0..t).step_by(2) {
                            let (qx, qy) = (cx * t + x, cy * t + y);
                            if qx < w && qy < h {
                                pick(&mut one, at(qx, qy));
                            }
                        }
                    }
                    // Rows, then the rows.
                    let mut two = ([0.0f32; 2], 0.0f32);
                    for j in 0..half {
                        let qy = cy * t + j * 2;
                        let mut row = ([0.0f32; 2], 0.0f32);
                        if qy < h {
                            for x in (0..t).step_by(2) {
                                if cx * t + x < w {
                                    pick(&mut row, at(cx * t + x, qy));
                                }
                            }
                        }
                        pick(&mut two, row.0);
                    }
                    assert_eq!(one.0.map(f32::to_bits), two.0.map(f32::to_bits), "{w}x{h} tile {t} at {cx},{cy}");
                }
            }
        }
        assert_eq!(tile_rows((90, 51), 61), (90, 51 * 31));
    }
}
