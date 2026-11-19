//! Weather effects (clone deviations, opt-in; docs/notes/graphics/weather.md):
//! Rain Detail (fine rain streaks lit by the street lamps and the cars'
//! lights, ground splashes, the wet-air glow around lamps) and Spray &
//! Splashes (wheel spray and mist behind cars on a wet road). Nothing here
//! is native; what it follows is: the rain's amount (weather +0x1F0, the
//! density), the road's wetness from the rain (`Weather::road_wet`, the
//! spray), the cover zones (+0x1E4: no rain or splashes under a cover, as
//! the native drops are hidden there), the world clock 0x7FB718 (the
//! animation) and the light packs' street lamps.
//!
//! Drawn by `WorldRenderer` after the native particles (`render_particles`
//! / `draw_particles`, so also after the temporal pass), premultiplied over
//! the scene's colour only (the alpha, NFSU2's bleach mask, untouched),
//! depth-aware by reading the scene's depth. With every weather row off the
//! renderer never makes this (no pass, no buffer).

use glam::{Mat4, Vec3};

use super::spray::{CarSample, SPRAY_MAX};
use crate::render::Gpu;
use crate::render::camera::Camera;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    cam_vel: [f32; 4],
    depth: [f32; 4],
    ambient: [f32; 4],
    wind: [f32; 4],
    size: [f32; 4],
    lamps: [[f32; 4]; 2 * MAX_LAMPS],
    spots: [[f32; 4]; 4 * MAX_CARS],
    /// Next-Gen (weather_fx.wgsl `ng_*`; zeros below Next-Gen).
    ng_wind: [f32; 4],
    ng_view: [f32; 4],
    ng_air: [f32; 4],
    /// The haze's look (x veil, y mist ambient, z sky share of the
    /// in-scatter, w 0).
    ng_haze: [f32; 4],
}

const MAX_LAMPS: usize = 32;
const MAX_CARS: usize = 8;
/// Rain streaks and splashes at full rain (On, High).
pub const RAIN_COUNT: [u32; 2] = [5000, 10000];
pub const SPLASH_COUNT: [u32; 2] = [1200, 2500];
/// Next-Gen: streaks in the three layers and world-anchored splash
/// spots at full rain (most spots fall outside the view or on hidden
/// ground and draw nothing).
pub const NG_RAIN_COUNT: u32 = 26000;
pub const NG_SPLASH_COUNT: u32 = 7000;
/// Next-Gen rain haze (retuned to docs/notes/graphics/look-direction.md, "no double fog" and "count the glow twice"):
/// the lamps' and headlights' in-scatter at full rain (1/m), the rain mist's
/// extinction (1/m), the share of that extinction veiling the scene again
/// (`veil`: the native fog owns the attenuation, ≤ 0.25), the mist's own
/// light as a share of the native fog colour (`ambient`: ≤ 0.1) and the
/// share of the in-scatter over the sky (the native fog already veils the
/// dome). **Inferred**, by the look metrics (rain street / harbour).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HazeLook {
    pub scatter: f32,
    pub extinction: f32,
    pub veil: f32,
    pub ambient: f32,
    pub sky: f32,
    pub reach: f32,
}

/// The first version had scatter 0.0022, extinction 0.0022, veil 1, ambient 0.9, sky 1, reach 1: the mist put the
/// fog colour back over the scene (a double fog: rain street +7.6, harbour +13.4 mean luma over
/// Original) and the lamps' wide in-scatter whitened the native pink glow sprites. Now the mist
/// veils at 0.2 with 0.1 of the fog colour as its light and not over the sky, and the lamps scatter
/// over half their reach (tighter halos, dark gaps kept): +0.45 / +1.54.
pub const HAZE_LOOK: HazeLook =
    HazeLook { scatter: 0.0022, extinction: 0.0022, veil: 0.2, ambient: 0.1, sky: 0.3, reach: 0.5 };

/// [`HAZE_LOOK`], or `NFSU2_RAIN_HAZE=scatter=…,extinction=…,veil=…,ambient=…,sky=…` (a tuning aid).
pub fn haze_look() -> HazeLook {
    static L: std::sync::OnceLock<HazeLook> = std::sync::OnceLock::new();
    *L.get_or_init(|| haze_look_from(&crate::render::debug_env::nfsu2_rain_haze()))
}

fn haze_look_from(spec: &str) -> HazeLook {
    let mut l = HAZE_LOOK;
    for kv in spec.split(',') {
        let Some((k, v)) = kv.split_once('=') else { continue };
        let Ok(v) = v.trim().parse::<f32>() else { continue };
        match k.trim() {
            "scatter" => l.scatter = v,
            "extinction" => l.extinction = v,
            "veil" => l.veil = v,
            "ambient" => l.ambient = v,
            "sky" => l.sky = v,
            "reach" => l.reach = v,
            _ => {}
        }
    }
    l
}
/// The half-resolution haze's format (in-scatter rgb, opacity).
const HAZE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The haze's in-scatter is worked out at 1/HAZE_DIV of the resolution
/// each way (a glow; weather_fx.wgsl `fs_haze_half`).
const HAZE_DIV: u32 = 4;

/// Next-Gen's wind at world clock `t` (s): a steady breeze with gusts and
/// a slowly turning direction (m/s), and its displacement integrated from
/// 0 (m, the drops' drift without jumps when the wind changes).
/// **Inferred**, by eye (nothing native: the native rain has no wind).
pub fn wind_ng(t: f32) -> (glam::Vec2, glam::Vec2) {
    // w(t) = base + g·(sin(a t), cos(b t)); its integral closed form.
    const BASE: glam::Vec2 = glam::Vec2::new(1.1, 0.5);
    const G: f32 = 1.4;
    const A: f32 = 0.31;
    const B: f32 = 0.17;
    let t = f64::from(t);
    let w = glam::Vec2::new(BASE.x + G * (A as f64 * t).sin() as f32, BASE.y + G * (B as f64 * t).cos() as f32);
    let d = glam::DVec2::new(
        f64::from(BASE.x) * t + f64::from(G) * (1.0 - (A as f64 * t).cos()) / A as f64,
        f64::from(BASE.y) * t + f64::from(G) * (B as f64 * t).sin() / B as f64,
    );
    (w, d.as_vec2())
}
/// Headlight colour (inferred: the cars' pool colour is not carried here),
/// reach (m) and cone (cos outer / inner).
const HEAD_COLOUR: Vec3 = Vec3::new(0.85, 0.82, 0.72);
const HEAD_RANGE: f32 = 30.0;
const HEAD_COS: [f32; 2] = [0.86, 0.96];

/// This frame's counts, by what [`WeatherFx::prepare`] found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    pub rain: u32,
    pub splashes: u32,
    pub halos: u32,
    pub spray: u32,
    /// Next-Gen: streaks, splash spots, the haze pass (0 / 1).
    pub rain_ng: u32,
    pub splash_ng: u32,
    pub haze: u32,
}

/// Whether the Next-Gen haze puts anything on screen with these air values (`FrameUniform::ng_air`:
/// x the puddles' level, y the in-scatter, z the extinction): its light is the lamps' sum × y, its
/// alpha 1 − e^(−z · len); both zero, `fs_haze_up` discards every pixel.
pub fn haze_draws(ng_air: [f32; 4]) -> bool {
    ng_air[1] != 0.0 || ng_air[2] != 0.0
}

static PLAIN_HAZE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The haze's passes run even when [`haze_draws`] says they draw nothing (true): for the test that
/// compares the frames with and without them.
pub fn set_plain_haze(on: bool) {
    PLAIN_HAZE.store(on, std::sync::atomic::Ordering::Relaxed);
}

static HAZE_LEFT_OUT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The frames whose haze passes were left out so far (the comparison test's check that it happened).
pub fn haze_left_out() -> u64 {
    HAZE_LEFT_OUT.load(std::sync::atomic::Ordering::Relaxed)
}

/// What to draw for the rows in force, the rain's amount and the cover.
pub fn counts(rain_detail: u8, spray_on: bool, rain: f32, covered: bool, _lamps: usize, spray: usize) -> Counts {
    let tier = usize::from(rain_detail.clamp(1, 2) - 1);
    let falling = if covered || rain_detail == 0 { 0.0 } else { rain.clamp(0.0, 1.0) };
    if rain_detail >= crate::render::RenderOptions::RAIN_NEXT_GEN {
        // Next-Gen: its own streaks, splashes and the haze (in place of the
        // halos); the haze stays under a cover (the air beyond is wet).
        return Counts {
            rain_ng: (NG_RAIN_COUNT as f32 * falling).round() as u32,
            splash_ng: (NG_SPLASH_COUNT as f32 * falling).round() as u32,
            haze: u32::from(rain > 0.01),
            spray: if spray_on { spray.min(SPRAY_MAX) as u32 } else { 0 },
            ..Counts::default()
        };
    }
    Counts {
        rain: (RAIN_COUNT[tier] as f32 * falling).round() as u32,
        splashes: (SPLASH_COUNT[tier] as f32 * falling).round() as u32,
        // One volume (no second glow): no wet-air halo billboards round the lamps
        // (`vs_halo`, kept unused): the native glow sprite is the lamp's halo, and the wet air's light is
        // the shared medium's (Volumetric Fog). Was `lamps.min(MAX_LAMPS)` while it rained.
        halos: 0,
        spray: if spray_on { spray.min(SPRAY_MAX) as u32 } else { 0 },
        ..Counts::default()
    }
}

pub struct WeatherFx {
    device: wgpu::Device,
    /// The device's program cache: the pipelines a warm-up's copy made
    /// (a drive's, the menus' garage scenes', `WeatherFx::warm`) are the
    /// ones this one's draws take.
    programs: crate::render::core::device_cache::Programs,
    /// The device's pipeline cache (`Gpu::pipeline_cache`).
    cache: Option<wgpu::PipelineCache>,
    frame: wgpu::Buffer,
    puffs: wgpu::Buffer,
    layouts: [wgpu::BindGroupLayout; 2],
    modules: [wgpu::ShaderModule; 2],
    pipelines: std::sync::Mutex<
        std::collections::HashMap<(wgpu::TextureFormat, u32, bool, &'static str), wgpu::RenderPipeline>,
    >,
    counts: Counts,
    /// Rain Detail at Next-Gen in the last [`WeatherFx::prepare`]'s
    /// frame settings (the draw's pass list).
    next_gen: bool,
    /// Next-Gen's half-resolution haze (size, texture, view) and its
    /// upsample's layout (group 1).
    haze: std::sync::Mutex<Option<(wgpu::Extent3d, wgpu::Texture, wgpu::TextureView)>>,
    haze_layout: wgpu::BindGroupLayout,
    haze_sampler: wgpu::Sampler,
}

/// The frame's view as the world's main view drew it.
#[derive(Clone, Copy)]
pub struct View {
    pub camera: Camera,
    pub aspect: f32,
    pub reversed: bool,
    /// The fog colour (0..1).
    pub fog: [f32; 3],
}

impl WeatherFx {
    pub fn new(gpu: &Gpu) -> WeatherFx {
        let device = &gpu.device;
        let frame = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("weather fx frame"),
                size: std::mem::size_of::<FrameUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let puffs = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("weather fx spray"),
                size: (SPRAY_MAX * 48) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let layout = |multisampled: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("weather fx"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            })
        };
        let src =
            crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::WeatherFx);
        let module = |label, s: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(s.into()),
            })
        };
        WeatherFx {
            device: device.clone(),
            programs: crate::render::core::device_cache::Programs::of(gpu),
            cache: gpu.disk_pipeline_cache().cloned(),
            frame,
            puffs,
            layouts: [layout(false), layout(true)],
            modules: [
                module("weather fx", src.to_string()),
                module("weather fx ms", src.replace("texture_depth_2d", "texture_depth_multisampled_2d")),
            ],
            pipelines: Default::default(),
            counts: Counts::default(),
            next_gen: false,
            haze: Default::default(),
            haze_layout: crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("weather fx haze"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            }),
            haze_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("weather fx haze"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    pub fn counts(&self) -> Counts {
        self.counts
    }

    /// Before the draw: set the frame's view, lights and counts from the
    /// spray as the last tick left it (`sim`, stepped on the tick
    /// by [`crate::render::WorldRenderer::enh_tick`]; "stepped" class).
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        view: &View,
        scene: &crate::render::enh::materials::car::wet::Scene,
        lamps: &[(Vec3, f32, Vec3)],
        cars: &[CarSample],
        sim: &super::host::WeatherSim,
        opts: &crate::render::RenderOptions,
    ) {
        let cam = &view.camera;
        let eye = cam.position;
        self.next_gen = opts.rain_next_gen();
        let spray_on = opts.spray || opts.wet_interaction != 0;
        let spray = &sim.spray;
        let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
        let view_proj = cam.projection_depth(view.aspect, view.reversed) * rot;
        let mut u = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            eye: eye.extend(scene.clock).to_array(),
            cam_vel: sim.cam_vel.extend(scene.rain).to_array(),
            depth: [cam.near, cam.far, if view.reversed { 1.0 } else { 0.0 }, 0.0],
            ambient: [view.fog[0], view.fog[1], view.fog[2], scene.wetness().film],
            wind: [0.6, 0.3, 0.0, f32::from(opts.rain_detail.max(1))],
            // The droplets' motion streaks (Wet Road Interaction).
            size: [0.0, 0.0, 0.0, if spray.tier > 0 { 1.0 } else { 0.0 }],
            lamps: [[0.0; 4]; 2 * MAX_LAMPS],
            spots: [[0.0; 4]; 4 * MAX_CARS],
            ng_wind: [0.0; 4],
            ng_view: [0.0; 4],
            ng_air: [0.0; 4],
            ng_haze: [0.0; 4],
        };
        if self.next_gen {
            let (w, d) = wind_ng(scene.clock);
            u.ng_wind = [w.x, w.y, d.x, d.y];
            let fwd = cam.forward();
            let ahead = glam::Vec2::new(fwd.x, fwd.y).normalize_or_zero() * 16.0;
            // The ground under the eye: the nearest car's wheel contacts
            // (the chase camera's own car), else 1.5 m below the eye.
            let ground = cars
                .iter()
                .min_by(|a, b| {
                    a.world
                        .w_axis
                        .truncate()
                        .distance_squared(eye)
                        .total_cmp(&b.world.w_axis.truncate().distance_squared(eye))
                })
                .filter(|c| c.world.w_axis.truncate().distance(eye) < 15.0)
                .map_or(-1.5, |c| c.contacts.iter().map(|p| p.z).sum::<f32>() / 4.0 - eye.z);
            let (_, m11) = cam.projection_scale(view.aspect);
            u.ng_view = [ahead.x, ahead.y, ground, m11];
            let rain = scene.rain.clamp(0.0, 1.0);
            // The puddles (rain rings) only where Wet Roads draws them.
            let level = if opts.wet_roads { scene.wetness().film } else { 0.0 };
            let look = haze_look();
            // One medium (`volumes::medium`): the wet air is the rain's share of the shared medium;
            // while the froxel medium runs (any Volumetric Fog tier) it holds the rain's air, so the haze
            // adds none of its own (two hazes of one rain before).
            let share = if opts.volumetric_fog >= 1 {
                0.0
            } else {
                crate::render::enh::volumes::medium::medium(rain).rain_share()
            };
            u.ng_air = [level, look.scatter * share, look.extinction * share, f32::from(u8::from(scene.covered))];
            u.ng_haze = [look.veil, look.ambient, look.sky, look.reach];
        }
        // One volume, each light in the air once: the street lamps' wet-air light is Volumetric Fog's
        // while any of its tiers runs (the froxel medium), the cars' headlight cones the
        // Next-Gen car lights' (their march or the medium); the haze keeps the rest and its mist. (The
        // drops and streaks keep every lamp and cone: that is lit rain, not air.) haze.wgsl: the lamps'
        // reach scale 0 leaves them out, a negative one the cones.
        if self.next_gen {
            let lamps_air = if opts.volumetric_fog >= 1 { 0.0 } else { u.ng_haze[3] };
            u.ng_haze[3] = if opts.car_lights_next_gen { -lamps_air - f32::MIN_POSITIVE } else { lamps_air };
        }
        let mut n = 0;
        for (p, r, c) in lamps.iter().take(MAX_LAMPS) {
            u.lamps[n * 2] = (*p - eye).extend(*r).to_array();
            u.lamps[n * 2 + 1] = c.extend(0.0).to_array();
            n += 1;
        }
        u.depth[3] = n as f32;
        let mut near: Vec<&CarSample> =
            cars.iter().filter(|c| c.world.w_axis.truncate().distance(eye) < 90.0).collect();
        near.sort_by(|a, b| {
            a.world.w_axis.truncate().distance(eye).total_cmp(&b.world.w_axis.truncate().distance(eye))
        });
        let mut s = 0;
        for c in near.into_iter().take(MAX_CARS) {
            let m = c.world;
            let at = m.w_axis.truncate();
            let forward = m.x_axis.truncate().normalize_or(Vec3::X);
            let up = m.z_axis.truncate().normalize_or(Vec3::Z);
            let head = at + forward * 2.0 + up * 0.7;
            let dir = (forward - Vec3::Z * 0.08).normalize_or(forward);
            u.spots[s * 4] = (head - eye).extend(HEAD_RANGE).to_array();
            u.spots[s * 4 + 1] = dir.extend(HEAD_COS[0]).to_array();
            u.spots[s * 4 + 2] = HEAD_COLOUR.extend(HEAD_COS[1]).to_array();
            u.spots[s * 4 + 3] = (m.transform_point3(Vec3::new(-2.2, 0.0, 0.8)) - eye).extend(0.0).to_array();
            s += 1;
        }
        u.wind[2] = s as f32;
        gpu.queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&u));
        self.counts = counts(opts.rain_detail, spray_on, scene.rain, scene.covered, n, spray.len());
        // The haze with neither in-scatter nor extinction (any Volumetric Fog tier holds the rain's air)
        // draws nothing: `fs_haze_half` writes zero light and `fs_haze_up` discards every pixel (its
        // alpha and light are 0). Neither pass runs then.
        if self.next_gen
            && self.counts.haze > 0
            && !haze_draws(u.ng_air)
            && !PLAIN_HAZE.load(std::sync::atomic::Ordering::Relaxed)
        {
            self.counts.haze = 0;
            HAZE_LEFT_OUT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        if crate::env_set!("NFSU2_WEATHER_DEBUG") {
            eprintln!("weather fx: {:?} scene {scene:?} cars {} cam_vel {:?}", self.counts, cars.len(), sim.cam_vel);
        }
        if self.counts.spray > 0 {
            // With Wet Road Interaction the mist is the volume's
            // (wet_spray), not a billboard; the droplets are streaked by
            // their motion over the frame (`vel`, the camera's taken off).
            let volume = spray.tier > 0;
            let data: Vec<[f32; 8]> = spray
                .particles
                .iter()
                .map(|p| {
                    let t = p.age / p.life;
                    let size = p.size.0 + (p.size.1 - p.size.0) * t.sqrt();
                    // Mist fades in over the first fifth and out after.
                    let a = if p.mist { p.alpha * (t / 0.2).min(1.0) * (1.0 - t) } else { p.alpha * (1.0 - t * t) };
                    let a = if volume && p.mist { 0.0 } else { a };
                    let r = p.pos - eye;
                    [r.x, r.y, r.z, size, t, if p.mist { 1.0 } else { 0.0 }, p.seed, a]
                })
                .collect();
            gpu.queue.write_buffer(&self.puffs, 0, bytemuck::cast_slice(&data));
            if volume {
                let vel: Vec<[f32; 4]> =
                    spray.particles.iter().map(|p| (p.vel - sim.cam_vel).extend(0.0).to_array()).collect();
                gpu.queue.write_buffer(&self.puffs, (SPRAY_MAX * 32) as u64, bytemuck::cast_slice(&vel));
            }
        }
    }

    /// Draw into `target` (`format`, `samples`), reading `depth` (the same
    /// sample count), with the temporal pass's viewport when given.
    /// Every pipeline a [`WeatherFx::draw`] into `format` × `samples`
    /// (either look, its haze too) may use, made now (a drive's warm-up),
    /// so the first rain is no pipeline build. Nothing is drawn.
    pub fn warm(&self, format: wgpu::TextureFormat, samples: u32) {
        let ms = samples > 1;
        for entry in ["haze_up", "vs_spray", "vs_ring_ng", "vs_splash_ng", "vs_rain_ng", "vs_halo", "vs_splash", "vs_rain"] {
            self.pipeline(entry, format, samples, ms);
        }
        self.pipeline("haze_half", HAZE_FORMAT, 1, ms);
    }

    /// The pipeline drawing `entry` (and its fragment shader) into
    /// `format` × `samples`, reading a depth texture that is multisampled
    /// when `depth_ms`; blended premultiplied over unless it is the
    /// half-resolution haze (written as it is).
    fn pipeline(
        &self,
        entry: &'static str,
        format: wgpu::TextureFormat,
        samples: u32,
        depth_ms: bool,
    ) -> wgpu::RenderPipeline {
        let key = (format, samples, depth_ms, entry);
        let mut all = self.pipelines.lock().unwrap();
        all.entry(key)
            .or_insert_with(|| {
                let (vs, fs) = match entry {
                    "vs_splash_ng" | "vs_ring_ng" => (entry, "fs_ng"),
                    "haze_half" => ("vs_haze", "fs_haze_half"),
                    "haze_up" => ("vs_haze", "fs_haze_up"),
                    _ => (entry, "fs_main"),
                };
                let groups = [Some(&self.layouts[usize::from(depth_ms)]), Some(&self.haze_layout)];
                let layout = self.programs.pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("weather fx"),
                    bind_group_layouts: if entry == "haze_up" { &groups[..] } else { &groups[..1] },
                    immediate_size: 0,
                });
                let module = &self.modules[usize::from(depth_ms)];
                let blend = (entry != "haze_half").then_some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::REPLACE,
                });
                let write_mask = if entry == "haze_half" { wgpu::ColorWrites::ALL } else { wgpu::ColorWrites::COLOR };
                self.programs.render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module,
                        entry_point: Some(vs),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module,
                        entry_point: Some(fs),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                    multiview_mask: None,
                    cache: self.cache.as_ref(),
                })
            })
            .clone()
    }

    /// Draw into `target` (`format`, `samples`), reading `depth` (the same
    /// sample count), with the temporal pass's viewport when given.
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        viewport: Option<[f32; 4]>,
    ) {
        let c = self.counts;
        if c == Counts::default() {
            return;
        }
        let ms = samples > 1;
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weather fx"),
            layout: &self.layouts[usize::from(ms)],
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.frame.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 2, resource: self.puffs.as_entire_binding() },
            ],
        });
        let next_gen = self.next_gen;
        let draws: &[(&'static str, u32, u32)] = if next_gen {
            // Next-Gen: the haze under everything (its half-resolution
            // pass first, then the full-screen upsample), the spray, the rain
            // rings, splashes, then the streaks.
            &[
                ("haze_up", 3, c.haze),
                ("vs_spray", 4, c.spray),
                ("vs_ring_ng", 4, c.splash_ng),
                ("vs_splash_ng", 4, c.splash_ng),
                ("vs_rain_ng", 4, c.rain_ng),
            ]
        } else {
            &[("vs_halo", 4, c.halos), ("vs_spray", 4, c.spray), ("vs_splash", 4, c.splashes), ("vs_rain", 4, c.rain)]
        };
        // Tuning aid: NFSU2_RAIN_NG_SKIP=haze_up,vs_rain_ng,… leaves those
        // draws out (their cost share).
        let skip = crate::render::debug_env::nfsu2_rain_ng_skip();
        // All the pipelines made the first frame any is drawn (one
        // kind starting later, e.g. the first splash, is then no
        // first-use build); the draws as before.
        let pipelines: Vec<_> = draws
            .iter()
            .filter(|d| d.2 > 0 && !skip.split(',').any(|s| s == d.0))
            .map(|d| (d.0, self.pipeline(d.0, format, samples, ms), d.1, d.2))
            .collect();
        let haze_group = pipelines.iter().any(|p| p.0 == "haze_up").then(|| self.haze_pass(encoder, &group, depth, ms));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("weather fx"),
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
        pass.set_bind_group(0, &group, &[]);
        for (entry, p, v, n) in &pipelines {
            pass.set_pipeline(p);
            if *entry == "haze_up" {
                pass.set_bind_group(1, haze_group.as_ref().unwrap(), &[]);
            }
            pass.draw(0..*v, 0..*n);
        }
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: weather fx");
    }

    /// Next-Gen's haze in-scatter at 1/HAZE_DIV of the scene's resolution (`HAZE_FORMAT`, kept
    /// between frames, remade with the size); the upsample's group.
    fn haze_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        group: &wgpu::BindGroup,
        depth: &wgpu::TextureView,
        ms: bool,
    ) -> wgpu::BindGroup {
        let full = depth.texture().size();
        let size = wgpu::Extent3d {
            width: full.width.div_ceil(HAZE_DIV).max(1),
            height: full.height.div_ceil(HAZE_DIV).max(1),
            depth_or_array_layers: 1,
        };
        let mut haze = self.haze.lock().unwrap();
        if haze.as_ref().is_none_or(|h| h.0 != size) {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("weather fx haze"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HAZE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            *haze = Some((size, tex, view));
        }
        let view = &haze.as_ref().unwrap().2;
        let pipeline = self.pipeline("haze_half", HAZE_FORMAT, 1, ms);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weather fx haze"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            if !crate::render::debug_env::nfsu2_rain_ng_skip().split(',').any(|s| s == "haze_half") {
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weather fx haze"),
            layout: &self.haze_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.haze_sampler) },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The density follows the native rain amount; nothing falls under a
    /// cover zone or with the row off.
    #[test]
    fn counts_follow_the_rain() {
        assert_eq!(counts(1, false, 0.0, false, 10, 0), Counts::default());
        let full = counts(1, false, 1.0, false, 10, 0);
        assert_eq!((full.rain, full.splashes, full.halos), (RAIN_COUNT[0], SPLASH_COUNT[0], 0));
        assert_eq!(counts(2, false, 0.5, false, 40, 0).rain, RAIN_COUNT[1] / 2);
        assert_eq!(counts(2, false, 1.0, true, 40, 0).rain, 0);
        assert_eq!(counts(0, false, 1.0, false, 40, 0), Counts::default());
        assert_eq!(counts(0, true, 0.0, false, 0, 50).spray, 50);
    }

    /// The Next-Gen haze keeps docs/notes/graphics/look-direction.md's "no double fog" (no second
    /// fog: veil ≤ 0.25, the mist's light ≤ 0.1 of the fog colour, less over
    /// the sky) and its lamps' in-scatter tighter than their reach; the
    /// tuning aid parses.
    #[test]
    fn rain_haze_stays_in_the_look_budget() {
        let l = HAZE_LOOK;
        assert!(l.veil <= 0.25 && l.ambient <= 0.1 && l.sky < 1.0 && l.reach < 1.0, "{l:?}");
        assert!(l.scatter > 0.0 && l.extinction > 0.0);
        let t = haze_look_from("veil=1,ambient=0.9,sky=1,reach=1,scatter=0.003,bogus=2,extinction");
        assert_eq!(
            (t.veil, t.ambient, t.sky, t.reach, t.scatter, t.extinction),
            (1.0, 0.9, 1.0, 1.0, 0.003, l.extinction)
        );
        assert_eq!(std::mem::size_of::<FrameUniform>() % 16, 0);
    }

    /// Next-Gen: its own streaks, splash spots and haze by the rain;
    /// none of On/High's (and those tiers unchanged); nothing falls under a
    /// cover, the haze stays.
    #[test]
    fn next_gen_counts_follow_the_rain() {
        let ng = crate::render::RenderOptions::RAIN_NEXT_GEN;
        let full = counts(ng, false, 1.0, false, 10, 0);
        assert_eq!((full.rain, full.splashes, full.halos), (0, 0, 0));
        assert_eq!((full.rain_ng, full.splash_ng, full.haze), (NG_RAIN_COUNT, NG_SPLASH_COUNT, 1));
        assert_eq!(counts(ng, false, 0.5, false, 10, 0).rain_ng, NG_RAIN_COUNT / 2);
        let covered = counts(ng, false, 1.0, true, 10, 0);
        assert_eq!((covered.rain_ng, covered.splash_ng, covered.haze), (0, 0, 1));
        assert_eq!(counts(ng, false, 0.0, false, 10, 0), Counts::default());
        assert_eq!(counts(ng, true, 0.0, false, 0, 50).spray, 50);
        for t in [1u8, 2] {
            let c = counts(t, false, 1.0, false, 10, 0);
            assert_eq!((c.rain_ng, c.splash_ng, c.haze), (0, 0, 0));
        }
    }

    /// The gusting wind's displacement is its integral (the drops drift
    /// continuously, no jump as the wind changes), and the gusts stay
    /// within a breeze.
    #[test]
    fn wind_drift_is_continuous() {
        let mut last = wind_ng(100.0);
        for i in 1..=6000 {
            let t = 100.0 + i as f32 / 60.0;
            let now = wind_ng(t);
            let step = (now.1 - last.1) * 60.0;
            assert!((step - now.0).length() < 0.05, "{t}: {step} vs {}", now.0);
            assert!(now.0.length() < 3.5 && now.0.length() > 0.0);
            last = now;
        }
    }

    /// The Next-Gen fields are at the end of the uniform (weather_fx.wgsl
    /// `Frame`: 3 vec4 after the spots, the haze look last).
    #[test]
    fn frame_uniform_layout() {
        assert_eq!(std::mem::size_of::<FrameUniform>(), 16 * (8 + 6 + 2 * MAX_LAMPS + 4 * MAX_CARS + 4));
        let src =
            crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::WeatherFx);
        assert!(src.contains("ng_wind: vec4<f32>,\n") || src.contains("ng_wind: vec4<f32>,"));
        for e in ["vs_rain_ng", "vs_splash_ng", "vs_ring_ng", "fs_ng", "vs_haze", "fs_haze_half", "fs_haze_up"] {
            assert!(src.contains(&format!("fn {e}(")), "{e}");
        }
    }
}
