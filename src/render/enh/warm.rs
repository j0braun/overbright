//! The Enhanced screen passes' programs made before the first frame that
//! needs them (docs/first-use-builds.md): a drive's load, and the moment
//! a Graphics row is switched on in the menus.
//!
//! The passes make their shader modules and pipelines in their
//! constructors and first draws. Here a copy of each pass the settings in
//! force will run is made and dropped: its modules, layouts and pipelines
//! stay in the device's cache ([`crate::render::core::device_cache`]),
//! where the frame's own passes then find them. The objects are the ones
//! the frame would make (same descriptors), so nothing drawn changes; what
//! the copies allocate besides (a few small buffers and placeholders) is
//! freed with them.
//!
//! The passes are made side by side on scoped threads (wgpu's device
//! makes pipelines from several threads at once on Vulkan and DX12; the
//! device cache builds each key once).

use crate::render::{Gpu, RenderOptions};

/// What the frame's passes are made for: the window's format and the
/// scene's sample count (`display::msaa_samples` for a drive's frame).
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub format: wgpu::TextureFormat,
    pub samples: u32,
}

/// The scene's format for `opts`: High Precision Post's float targets,
/// else the window's (`PostFx::scene_format` once its targets are made).
pub fn scene_format(opts: &RenderOptions, format: wgpu::TextureFormat) -> wgpu::TextureFormat {
    if opts.hdr_active { crate::render::HDR_FORMAT } else { format }
}

/// The passes `opts` runs on a drive's frames, their programs made (see
/// the module). Returns once all are made.
pub fn frame_passes(gpu: &Gpu, opts: &RenderOptions, shape: Shape) {
    let scene = scene_format(opts, shape.format);
    let samples = shape.samples;
    let mut jobs: Vec<Box<dyn FnOnce() + Send + '_>> = Vec::new();
    if opts.ssao > 0.0 {
        jobs.push(Box::new(move || {
            let s = crate::render::ssao::Ssao::new(gpu, scene);
            s.warm(gpu, samples);
        }));
    }
    if opts.ssr != 0 {
        jobs.push(Box::new(move || {
            let mut ssr = crate::render::ssr::Ssr::new(gpu, scene);
            // Ray Traced: its trace's programs (the ray list's where the
            // device takes it), as the first traced frame makes them.
            if opts.rt_reflections() && crate::render::rt::available(gpu) {
                ssr.warm_rt(gpu, samples > 1);
            }
        }));
    }
    if opts.volumetric_fog != 0 {
        jobs.push(Box::new(move || crate::render::atmosphere::Atmosphere::new(gpu).warm(gpu, scene, samples > 1)));
    }
    if opts.car_lights_next_gen {
        jobs.push(Box::new(move || drop(crate::render::car_lights::CarLightsPass::new(gpu, scene))));
    }
    let motion = crate::render::blur_dof::MotionMode::of(opts) != crate::render::blur_dof::MotionMode::Original;
    let dof = crate::render::blur_dof::dof_on(opts);
    if motion || dof {
        jobs.push(Box::new(move || {
            let mut b = crate::render::blur_dof::BlurDof::new(gpu);
            b.warm(gpu, scene, samples, motion, dof);
        }));
    }
    if opts.temporal {
        jobs.push(Box::new(move || crate::render::temporal::Temporal::new(gpu).warm(gpu, scene)));
        if opts.fsr2 {
            jobs.push(Box::new(move || crate::render::temporal::Temporal::warm_fsr2(gpu, scene)));
        }
    }
    // The graphics passes after the post chain (FXAA, SMAA, Sharpen, the
    // resample to the screen: SSAA, Render Scale, FSR 1), into the window's
    // format.
    let probe = (1920, 1080);
    let scaled = opts.scene_size(probe, samples) != probe || opts.upscale != 0;
    if scaled || opts.fxaa || opts.smaa || opts.sharpen > 0.0 {
        let format = shape.format;
        jobs.push(Box::new(move || drop(crate::render::enhance::Enhance::new(gpu, format))));
        if opts.smaa {
            jobs.push(Box::new(move || drop(crate::render::smaa::Smaa::new(gpu, format))));
        }
        if opts.fsr {
            jobs.push(Box::new(move || crate::render::fsr::Fsr::new(gpu).warm(gpu, format)));
        }
    }
    // The draws among the scene's particles (after the temporal pass:
    // single-sampled): Sparks & Fire, the volume effects (Ambient Detail
    // Next-Gen's dust and steam, the spray's volume in rain).
    let targets = if opts.temporal { (scene, 1, 1) } else { (scene, samples, samples) };
    if opts.sparks_fire != 0 {
        jobs.push(Box::new(move || {
            crate::render::sparks_fire::Renderer::new(gpu).warm(gpu, targets.0, targets.1, targets.2)
        }));
    }
    let volumes = opts.ambient_detail >= 2 || opts.wet_interaction != 0;
    if volumes {
        jobs.push(Box::new(move || {
            crate::render::volume_fx::warm_programs(gpu, targets.0, targets.1, targets.2)
        }));
    }
    // The 3-D noise of Particles Cinematic and the volume effects (a CPU
    // table made once a run; usually made already, [`prefetch_tables`]).
    if needs_noise(opts) {
        jobs.push(Box::new(|| {
            let _ = crate::render::smoke_cinematic::noise_volume();
        }));
    }
    run(jobs);
}

/// What a drive's programs are made for: the settings in force without
/// their per-frame fields (the temporal decision, the output size the
/// last frame drew with), the window's format, the scene's sample count,
/// whether the first frame shows the rear-view mirror, Photo Mode's row.
/// Nothing of a track or a car: the same set serves every drive.
#[derive(Clone, Copy, PartialEq)]
pub struct ProgramSet {
    pub opts: RenderOptions,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub mirror: bool,
    pub photo: bool,
}

impl ProgramSet {
    pub fn new(opts: &RenderOptions, format: wgpu::TextureFormat, samples: u32, mirror: bool, photo: bool) -> ProgramSet {
        let opts = RenderOptions { temporal_frame: false, output_size: None, ..*opts };
        ProgramSet { opts, format, samples, mirror, photo }
    }
}

/// Every program of `set` that needs no world and no car, made side by
/// side on scoped threads: the world's scene sets
/// ([`crate::render::native::world::record::warm_scene_programs`]), the
/// cars' ([`crate::render::car::warm_programs`]), the Enhanced screen
/// passes' ([`frame_passes`]), the post chain's additions and Photo Mode's. They stay in the device's
/// shared sets and cache; a drive's own warm-up and frames ask for the
/// same descriptors and find them made. Run from the menus on a thread of
/// its own, and by the prepare mode; safe beside a load (each key is built
/// once, a second asker waits for it). Returns once all are made.
pub fn drive_programs(gpu: &Gpu, set: &ProgramSet) {
    let _t = crate::prof::span("programs ahead");
    let ProgramSet { opts, format, samples, photo, .. } = set;
    let (format, samples) = (*format, *samples);
    let mut jobs: Vec<Box<dyn FnOnce() + Send + '_>> = Vec::new();
    jobs.push(Box::new(move || crate::render::native::world::record::warm_scene_programs(gpu, format, samples, opts)));
    jobs.push(Box::new(move || crate::render::car::warm_programs(gpu, format, samples, opts)));
    jobs.push(Box::new(move || frame_passes(gpu, opts, Shape { format, samples })));
    // The post chain's additions for a drive's frames, on a copy of the
    // chain (the window's own asks for the same and finds them made).
    jobs.push(Box::new(move || crate::render::postfx::PostFx::new(gpu, format).warm_frame(gpu, opts, samples)));
    if *photo {
        jobs.push(Box::new(move || {
            crate::render::photo_grade::PhotoGrade::new(gpu).warm(gpu, format);
            crate::render::blur_dof::BlurDof::new(gpu).warm(gpu, scene_format(opts, format), samples, false, true);
        }));
    }
    run(jobs);
}

/// A world's own effect objects made ahead of a drive on it
/// ([`world_programs`]), for the drive to take as its own: what its
/// load's warm-up would build on fresh ones (`Water::build`,
/// `Cinematic::prebuild`, `Detail::prebuild` on the same world then leave
/// them as they are).
#[derive(Default)]
pub struct WorldAhead {
    /// Animated Water at its tier, with the row on.
    pub water: Option<crate::render::water::Water>,
    /// Particles Cinematic's GPU half.
    pub cinematic: Option<crate::render::smoke_cinematic::Cinematic>,
    /// The detailed smoke's GPU half (the other Particles rows).
    pub detail: Option<crate::render::gpu_particles::Detail>,
}

/// The programs of `set` that are made through a world's own objects, for
/// `world` (as a drive's load makes them, `loads::warm`): Animated Water
/// at its tier, built and warmed (its mesh, GPU halves and pipelines), and
/// the smoke's GPU half with its pipelines for the scene's targets, both
/// returned for the drive on this world to take ([`WorldAhead`]); and,
/// with a row that traces, the traced scene's world half
/// ([`crate::render::rt::prepare`]). Run beside a world loaded ahead in
/// the menus, and by the prepare mode.
pub fn world_programs(gpu: &Gpu, set: &ProgramSet, world: &crate::world::World, region: &str) -> WorldAhead {
    let _t = crate::prof::span("programs ahead: world");
    let (opts, samples) = (&set.opts, set.samples);
    let scene = scene_format(opts, set.format);
    let row = opts.particles;
    let is_cinematic = crate::render::smoke_cinematic::active(row);
    let mut ahead = WorldAhead {
        water: opts.animated_water.then(crate::render::water::Water::default),
        cinematic: Some(Default::default()),
        detail: (!is_cinematic).then(Default::default),
    };
    let WorldAhead { water, cinematic, detail } = &mut ahead;
    let mut jobs: Vec<Box<dyn FnOnce() + Send + '_>> = Vec::new();
    if let Some(water) = water.as_mut() {
        jobs.push(Box::new(move || {
            let tier = opts.water_tier.max(1);
            water.build(gpu, world, tier);
            water.warm_tier(gpu, tier, scene, samples, opts.reversed_z);
        }));
    }
    jobs.push(Box::new(move || {
        let targets = if opts.temporal { [(scene, 1, 1)] } else { [(scene, samples, samples)] };
        if let Some(c) = cinematic.as_mut() {
            c.prebuild(gpu, is_cinematic, &world.emitters, &world.textures, &targets);
        }
        if let Some(d) = detail.as_mut() {
            d.prebuild(gpu, row, &world.emitters, &world.textures, &targets);
        }
    }));
    if traces(opts) {
        jobs.push(Box::new(move || crate::render::rt::prepare(gpu, world, region)));
    }
    run(jobs);
    ahead
}

/// Whether `opts`' frames read the traced scene (Modern Lighting with RT,
/// or the traced reflections): the rows whose first frame in a region
/// made it.
pub fn traces(opts: &RenderOptions) -> bool {
    crate::render::modern_light_bold::wants_rt(opts) || opts.rt_reflections()
}

/// `jobs` side by side, each on its own scoped thread (one inline).
pub fn run(mut jobs: Vec<Box<dyn FnOnce() + Send + '_>>) {
    let Some(first) = jobs.pop() else { return };
    std::thread::scope(|scope| {
        for job in jobs {
            std::thread::Builder::new()
                .name("warm-up".into())
                .spawn_scoped(scope, job)
                .expect("a warm-up thread");
        }
        first();
    });
}

/// Whether `opts` draws with the 3-D noise volume (Particles Cinematic,
/// Ambient Detail Next-Gen, the spray's volume).
fn needs_noise(opts: &RenderOptions) -> bool {
    crate::render::smoke_cinematic::active(opts.particles) || opts.ambient_detail >= 2 || opts.wet_interaction != 0
}

/// The CPU tables the passes `opts` turns on read (the 3-D noise volume,
/// ~0.25 s of one core), made once a run on a thread of their own as soon
/// as a row needs them (in the menus, typically), so neither a drive's
/// load nor its first frame waits for them. The same bytes whoever makes
/// them first.
pub fn prefetch_tables(opts: &RenderOptions) {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if needs_noise(opts) && !STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let _ = std::thread::Builder::new().name("noise table".into()).spawn(|| {
            let _ = crate::render::smoke_cinematic::noise_volume();
        });
    }
}
