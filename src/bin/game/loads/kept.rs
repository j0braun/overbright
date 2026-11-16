//! A car change's next drive made ready in the menus (docs/first-use-builds.md
//! "Kept across a car change"). A drive dropped for a car change on the
//! same world (a part fitted in a shop, a car bought) leaves its world, the
//! world's renderer and its Animated Water for the next drive on that
//! world, which used to put them back to their state as loaded in its own
//! load: the world read again from its loaded state, every placement's
//! instance rewritten, the water's and the smoke's per-drive halves made
//! again. The menus the player is in meanwhile (the shop's) give the time:
//! that work runs here on a worker as soon as the menus are up, with the
//! same calls on the same objects, and the load takes the result (waiting
//! for it if it is still running). Nothing that reaches a frame changes:
//! the world's reset depends only on its loaded state, the instances only
//! on the reset world, the water's and smoke's new halves on the world and
//! the settings (kept only when the settings at the load are the ones they
//! were made for), and the loaded emitters (a process-wide table) are still
//! set by the load itself.
use crate::*;

/// The worker making a kept world ready for its next drive.
pub(crate) struct KeptAhead {
    pub(crate) region: String,
    job: std::thread::JoinHandle<Kept>,
}

/// What [`KeptAhead`]'s worker hands back.
pub(crate) struct Kept {
    /// The world back to its state as loaded (`World::reset_to_loaded`
    /// without the barrier switches kept), None when that failed.
    world: Option<World>,
    /// The renderer with its instances reset (`reset_instances`) when the
    /// world's reset succeeded, on the device it was made on.
    renderer: Option<(wgpu::Device, WorldRenderer)>,
    /// The world's emitters for the load to make the loaded ones.
    emitters: Vec<nfsu2::render::lamps::Lamp>,
    /// The water and the smoke's GPU halves built for `set`.
    set: Option<nfsu2::render::enh::warm::ProgramSet>,
    water: Option<nfsu2::render::water::Water>,
    cinematic: nfsu2::render::smoke_cinematic::Cinematic,
    detail: nfsu2::render::gpu_particles::Detail,
}

/// What the kept world's load takes besides the world and its renderer
/// (`drive_in_loaded`).
pub(crate) struct KeptExtras {
    /// The renderer's instances are reset; the emitters are left to set.
    pub(crate) emitters: Vec<nfsu2::render::lamps::Lamp>,
    /// The smoke's GPU halves, built for the settings in force at the load.
    pub(crate) smoke: Option<(nfsu2::render::smoke_cinematic::Cinematic, nfsu2::render::gpu_particles::Detail)>,
}

impl KeptAhead {
    /// Whether the work is made ahead (the loads ahead on,
    /// `cars_ahead::with_loads`).
    pub(crate) fn enabled() -> bool {
        Preload::enabled() && crate::render::cars_ahead::with_loads()
    }

    /// The worker has ended (the developer harness waits for it).
    #[cfg_attr(not(feature = "dev-harness"), allow(dead_code))]
    pub(crate) fn is_finished(&self) -> bool {
        self.job.is_finished()
    }
}

impl Game {
    /// Once a frame in the menus: a world kept by a car change starts
    /// being made ready for its next drive ([`KeptAhead`]).
    pub(crate) fn kept_ahead_frame(&mut self) {
        if !KeptAhead::enabled() || self.loads.kept_ahead.is_some() || !matches!(self.mode, Mode::Menu(_)) {
            return;
        }
        let Some(w) = self.platform.window.as_ref() else { return };
        let Some((region, mut world)) = self.loads.resident.take() else { return };
        let (gpu, set) = (w.gpu.clone(), self.program_set());
        let mut renderer = self.render.resident.take();
        let mut water = self.render.resident_water.take();
        let job = std::thread::Builder::new()
            .name("kept world".into())
            .spawn(move || {
                let _t = nfsu2::prof::span("kept world ahead");
                let reset = {
                    let _t = nfsu2::prof::span("kept world ahead: reset");
                    world.reset_to_loaded(false)
                };
                if !reset {
                    return Kept {
                        world: None,
                        renderer: None,
                        emitters: Vec::new(),
                        set: None,
                        water: None,
                        cinematic: Default::default(),
                        detail: Default::default(),
                    };
                }
                let same_device = renderer.as_ref().is_some_and(|(d, _)| *d == gpu.device);
                if let Some((_, r)) = renderer.as_mut().filter(|_| same_device) {
                    let _t = nfsu2::prof::span("kept world ahead: instances");
                    r.reset_instances(&gpu, &world);
                }
                let emitters = {
                    let _t = nfsu2::prof::span("kept world ahead: emitters");
                    nfsu2::render::modern_light_bold::emitters(&world)
                };
                // The per-drive halves a load's warm-up makes (`warm_passes`):
                // the water at its tier and the smoke's GPU half, for the
                // settings in force now; a load for other settings drops
                // them and makes its own.
                let (mut cinematic, mut detail) = (Default::default(), Default::default());
                let set = set.filter(|_| same_device);
                if let Some(set) = &set {
                    let _t = nfsu2::prof::span("kept world ahead: water, smoke");
                    let opts = &set.opts;
                    let scene = nfsu2::render::enh::warm::scene_format(opts, set.format);
                    if let Some(water) = water.as_mut().filter(|_| opts.animated_water) {
                        let tier = opts.water_tier.max(1);
                        water.build(&gpu, &world, tier);
                        water.warm_tier(&gpu, tier, scene, set.samples, opts.reversed_z);
                    }
                    let targets = if opts.temporal { [(scene, 1, 1)] } else { [(scene, set.samples, set.samples)] };
                    let row = opts.particles;
                    let is_cinematic = nfsu2::render::smoke_cinematic::active(row);
                    let c: &mut nfsu2::render::smoke_cinematic::Cinematic = &mut cinematic;
                    c.prebuild(&gpu, is_cinematic, &world.emitters, &world.textures, &targets);
                    if !is_cinematic {
                        let d: &mut nfsu2::render::gpu_particles::Detail = &mut detail;
                        d.prebuild(&gpu, row, &world.emitters, &world.textures, &targets);
                    }
                }
                Kept { world: Some(world), renderer, emitters, set, water, cinematic, detail }
            })
            .expect("the kept world's thread");
        self.loads.kept_ahead = Some(KeptAhead { region, job });
    }

    /// The kept world made ready for `region` ([`KeptAhead`], waited for if
    /// still running): the world and its renderer (when on `device`), as
    /// `take_kept_world` gives them, the renderer's and the effects' part
    /// in [`KeptExtras`] and the water in `render.resident_water`. None
    /// (everything dropped) for another region or when the world's reset
    /// failed, as the load's own reset.
    pub(crate) fn take_kept_ahead(
        &mut self,
        region: &str,
        device: &Option<wgpu::Device>,
    ) -> Option<(World, Option<WorldRenderer>)> {
        self.render.kept_extras = None;
        let ahead = self.loads.kept_ahead.take()?;
        let kept = {
            let _t = nfsu2::prof::span("drive_in: kept world joined");
            ahead.job.join().expect("the kept world's thread")
        };
        if ahead.region != region {
            std::thread::spawn(move || drop(kept));
            return None;
        }
        let Kept { world, renderer, emitters, set, water, cinematic, detail } = kept;
        let world = world?;
        let renderer = renderer.filter(|(d, _)| Some(d) == device.as_ref()).map(|r| r.1);
        // Built for the settings in force now, or the halves dropped (the
        // load's warm-up makes them as for a new drive).
        let same = set.is_some() && set == self.program_set();
        self.render.resident_water =
            water.filter(|_| renderer.is_some()).map(|w| if same { w } else { w.for_new_drive() });
        self.render.kept_extras =
            renderer.is_some().then(|| KeptExtras { emitters, smoke: same.then_some((cinematic, detail)) });
        Some((world, renderer))
    }
}
