//! The programs a drive's frames need, made before the first frame that
//! needs them (docs/first-use-builds.md): at the drive's load (behind
//! its loading screen) and when a Graphics row changes while a drive is
//! in memory (behind the menu the row was changed in).

use crate::*;

/// What [`warm_passes`] makes the programs for: the settings in force
/// (frame-dependent fields left out) and the scene's sample count.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct WarmedFor {
    pub(crate) opts: nfsu2::render::RenderOptions,
    pub(crate) samples: u32,
    pub(crate) mirror: bool,
    /// Photo Mode's row (its grade and depth of field).
    pub(crate) photo: bool,
    /// The garage scenes' sample count (`display::front_msaa_samples`).
    pub(crate) front_samples: u32,
}

impl WarmedFor {
    /// `opts` without the per-frame fields (the temporal decision and the
    /// output size the last frame drew with).
    pub(crate) fn new(
        opts: &nfsu2::render::RenderOptions,
        samples: u32,
        mirror: bool,
        photo: bool,
        front_samples: u32,
    ) -> WarmedFor {
        let opts = nfsu2::render::RenderOptions { temporal_frame: false, output_size: None, ..*opts };
        WarmedFor { opts, samples, mirror, photo, front_samples }
    }
}

/// The drive's world and car renderers' sets, the post chain's and the
/// Enhanced passes' programs, the Animated Water's GPU halves and the
/// smoke's GPU half for `w`, side by
/// side on scoped threads. The objects are the ones the frames would make
/// (see the warm-ups called); nothing is drawn. `post`: the window's post
/// chain (taken out of the window while a loading screen draws).
#[allow(clippy::too_many_arguments)]
pub(crate) fn warm_passes(
    gpu: &Gpu,
    w: &WarmedFor,
    format: wgpu::TextureFormat,
    renderer: Option<&mut WorldRenderer>,
    car_renderer: Option<&mut CarRenderer>,
    post: Option<&mut PostFx>,
    world: &World,
    region: &str,
    effects: &mut sim::effects::Effects,
) {
    let (opts, samples, mirror) = (&w.opts, w.samples, w.mirror);
    let shape = nfsu2::render::enh::warm::Shape { format, samples };
    let mut renderer = renderer;
    // Wet Road Interaction's spray volume, a job of its own (it is
    // offered to the renderer once the jobs are done).
    let mut spray = None;
    let spray_wanted = renderer.as_ref().is_some_and(|r| r.wants_spray_volume(opts));
    // Ambient Detail's props (kept, made again for the drive's barriers
    // when made ahead or kept from the last drive, or made:
    // `AmbientWarm::run`), a job of their own beside the world renderer's
    // (put back once the jobs are done).
    let mut ambient = renderer.as_deref_mut().map(|r| r.take_ambient(samples, opts));
    let mut jobs: Vec<Box<dyn FnOnce() + Send + '_>> = Vec::new();
    if spray_wanted {
        let spray = &mut spray;
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: spray volume");
            *spray = WorldRenderer::spray_volume_ahead(gpu, opts);
        }));
    }
    if let Some(a) = ambient.as_mut() {
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: ambient detail");
            a.run(gpu, world, opts);
        }));
    }
    if let Some(r) = renderer.as_deref_mut() {
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: world renderer");
            r.warm_drive_passes(gpu, samples, opts);
            if mirror {
                r.warm_rear_view(gpu);
            }
        }));
    }
    // The shops' garage scenes over this drive (their worlds at the
    // front end's count).
    let front_samples = w.front_samples;
    jobs.push(Box::new(move || {
        let _t = nfsu2::prof::span("warm: garage scenes");
        nfsu2::render::native::world::record::warm_front_scene_programs(gpu, format, front_samples, opts)
    }));
    if let Some(c) = car_renderer {
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: car renderer");
            c.warm(gpu, samples, opts);
            if mirror {
                c.prepare_reflection(gpu);
            }
            // The shops' garage scenes' FE car draws its floor reflection
            // with the same set.
            nfsu2::render::car::warm_reflection_set(gpu);
        }));
    }
    if let Some(p) = post {
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: post chain");
            p.warm_frame(gpu, opts, samples)
        }));
    }
    jobs.push(Box::new(move || {
        let _t = nfsu2::prof::span("warm: screen passes");
        nfsu2::render::enh::warm::frame_passes(gpu, opts, shape)
    }));
    // Photo Mode (paused free roam or race): its grade into the window's
    // format and its depth of field (no motion blur in photo mode).
    if w.photo {
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: photo mode");
            nfsu2::render::photo_grade::PhotoGrade::new(gpu).warm(gpu, format);
            let scene = nfsu2::render::enh::warm::scene_format(opts, format);
            nfsu2::render::blur_dof::BlurDof::new(gpu).warm(gpu, scene, samples, false, true);
        }));
    }
    // Animated Water at its tier: the world's water mesh and the GPU
    // halves its first frame made, and their pipelines for the scene's
    // targets (`Water::build`, `Water::warm_tier`).
    let sim::effects::Effects { water, cinematic: smoke_cinematic, detail: smoke_detail, .. } = effects;
    if opts.animated_water {
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: water");
            let tier = opts.water_tier.max(1);
            water.build(gpu, world, tier);
            let scene = nfsu2::render::enh::warm::scene_format(opts, format);
            water.warm_tier(gpu, tier, scene, samples, opts.reversed_z);
        }));
    }
    // The smoke's GPU half for the Particles row (its first frame made
    // it), and its pipelines for the scene's targets (after the temporal
    // pass: single-sampled).
    jobs.push(Box::new(move || {
        let _t = nfsu2::prof::span("warm: smoke");
        let format = nfsu2::render::enh::warm::scene_format(opts, format);
        let targets = if opts.temporal { [(format, 1, 1)] } else { [(format, samples, samples)] };
        let row = opts.particles;
        let cinematic = nfsu2::render::smoke_cinematic::active(row);
        smoke_cinematic.prebuild(gpu, cinematic, &world.emitters, &world.textures, &targets);
        if !cinematic {
            smoke_detail.prebuild(gpu, row, &world.emitters, &world.textures, &targets);
        }
    }));
    // The traced scene's world half (Modern Lighting with RT, the traced
    // reflections): the region's first frame gathered it on the main thread.
    if nfsu2::render::enh::warm::traces(opts) {
        jobs.push(Box::new(move || {
            let _t = nfsu2::prof::span("warm: traced scene");
            nfsu2::render::rt::prepare(gpu, world, region)
        }));
    }
    nfsu2::render::enh::warm::run(jobs);
    if let Some(r) = renderer {
        r.offer_spray_volume(spray);
        if let Some(a) = ambient {
            r.put_ambient(a);
        }
    }
}

/// A drive's programs made ahead (docs/first-use-builds.md "Ahead of the
/// load"): from the first frame the window draws (the boot screens, the
/// title screen, the menus), on a thread of its own, every program the
/// settings in force need for a drive that depends on those settings and
/// the window's format alone (`enh::warm::drive_programs`). They go into
/// the device's shared sets and cache; a load's warm-up asks for the same
/// descriptors, takes what is made, waits for what is being made and
/// builds only what is missing. A settings change in the menus starts
/// another pass for the new set (only its difference is built).
/// `NFSU2_NO_PROGRAMS_AHEAD=1` and `NFSU2_NO_PRELOAD=1` turn it off.
pub(crate) struct ProgramsAhead {
    /// What the pass in `job` makes them for.
    pub(crate) set: nfsu2::render::enh::warm::ProgramSet,
    /// (Read by the developer harness and the tests: whether it has ended.)
    #[cfg_attr(not(feature = "dev-harness"), allow(dead_code))]
    pub(crate) job: std::thread::JoinHandle<()>,
}

impl ProgramsAhead {
    pub(crate) fn enabled() -> bool {
        Preload::enabled() && !debug_env().no_programs_ahead
    }
}

impl Game {
    /// The set of programs a drive started now would take
    /// ([`Game::warm_target`] with the window's format).
    pub(crate) fn program_set(&self) -> Option<nfsu2::render::enh::warm::ProgramSet> {
        let w = self.platform.window.as_ref()?;
        let t = self.warm_target(&w.gpu);
        Some(nfsu2::render::enh::warm::ProgramSet::new(&t.opts, w.config.format, t.samples, t.mirror, t.photo, t.front_samples))
    }

    /// [`ProgramsAhead`], once a frame: with no drive in memory (a drive's
    /// own programs follow the settings through `warm_for_settings`), a
    /// pass for the set in force is started unless the last one was for it.
    pub(crate) fn programs_frame(&mut self) {
        if !ProgramsAhead::enabled() || self.sim.drive.is_some() || self.loads.loading.is_some() {
            return;
        }
        // Not beside the window's very first frame, which makes the menus'
        // own programs: from the frame after it.
        if self.platform.window.as_ref().is_none_or(|w| w.frames_ended == 0) {
            return;
        }
        let Some(set) = self.program_set() else { return };
        if self.loads.programs.as_ref().is_some_and(|p| p.set == set) {
            return;
        }
        let gpu = self.platform.window.as_ref().unwrap().gpu.clone();
        let job = std::thread::Builder::new()
            .name("programs".into())
            .spawn(move || nfsu2::render::enh::warm::drive_programs(&gpu, &set))
            .expect("the programs thread");
        self.loads.programs = Some(ProgramsAhead { set, job });
    }

    /// What the warm-up is made for now: the settings in force, the
    /// scene's samples, whether the first frame shows the rear-view mirror
    /// (the HUD's test, `rear_view`: the option, a bumper or hood camera).
    pub(crate) fn warm_target(&self, gpu: &Gpu) -> WarmedFor {
        let opts = nfsu2::render::RenderOptions::in_force();
        // The count a drive's frames take, not the last frame's (a menu's,
        // a garage's or the loading screen's).
        let samples = nfsu2::render::display::drive_msaa_samples(&self.settings.custom, gpu, &opts);
        let mirror = self.settings.rearview && self.settings.players[0].camera <= 1;
        let front = nfsu2::render::display::front_msaa_samples(&self.settings.custom, gpu, &opts);
        WarmedFor::new(&opts, samples, mirror, self.settings.enh.photo_mode, front)
    }

    /// A Graphics row changed with a drive in memory (the options in the
    /// pause menu, or in the main menus with the region kept): the
    /// programs the drive's frames now need, made here, before the frame
    /// that draws with them (the menu's own frame when the world shows
    /// under it), side by side.
    pub(crate) fn warm_for_settings(&mut self) {
        if self.sim.drive.is_none() {
            // The CPU tables the rows in force read, made on a thread of
            // their own ahead of the drive (`enh::warm::prefetch_tables`).
            if self.platform.window.is_some() {
                nfsu2::render::enh::warm::prefetch_tables(&nfsu2::render::RenderOptions::in_force());
            }
            return;
        }
        let Some(gpu) = self.platform.window.as_ref().map(|w| w.gpu.clone()) else { return };
        let target = self.warm_target(&gpu);
        if self.render.warmed.as_ref() == Some(&target) {
            return;
        }
        let (Some(w), Some(d)) = (self.platform.window.as_mut(), self.sim.drive.as_mut()) else { return };
        let _t = nfsu2::prof::span("warm for the settings");
        // The last frame's end first: its render threads read the renderers'
        // sets this replaces.
        w.settle();
        let format = w.config.format;
        warm_passes(
            &gpu,
            &target,
            format,
            self.render.world.as_mut(),
            self.render.cars.get_mut(&d.car_id),
            w.post.as_mut(),
            &d.world,
            &d.region,
            &mut d.effects,
        );
        self.render.warmed = Some(target);
    }
}
