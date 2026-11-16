use crate::*;

/// What a car load for a drive is keyed on: the car, its parts, its
/// performance packages and its tuning settings (`drive_in_loaded`'s
/// `Car::load_tuned` arguments).
pub(crate) type CarKey =
    (String, std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>, nfsu2::tuning::Settings);

/// What a rival's car load is keyed on: `line_up_rivals`'
/// `Car::load_upgraded` arguments (the car, its parts, its performance
/// packages).
pub(crate) type RivalKey = (String, std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>);

/// A career race's rivals loading ahead while its UI_EngageRaceDialog is
/// up (background loading, as [`Preload`]): the line-up and the looks drawn
/// on a copy of the random numbers as the race's start will draw them, so
/// their cars are the loads `line_up_rivals` will ask for; it takes a load
/// only when its key is the one it asks for (same call, same arguments),
/// and loads any other itself, so what the race gets is the same either
/// way.
pub(crate) struct RivalsAhead {
    pub(crate) event: usize,
    /// A Quick Race's car picked (`QuickRaceState::car` and `parts`) the
    /// line-up was predicted for; None for a career race (the drive's car).
    pub(crate) car: Option<QuickCar>,
    /// The random state the line-up was predicted from.
    pub(crate) rng: u32,
    /// Each car's load, and its renderer built on the same thread after it
    /// ([`CarRendererAhead`], as the race's first frame would build it).
    pub(crate) loads: Vec<(RivalKey, std::thread::JoinHandle<Result<Car>>, Option<CarRendererAhead>)>,
}

/// A Quick Race's car as UI_QRCarSelect picked it: its name and, for a My
/// Cars record, its parts and performance parts.
pub(crate) type QuickCar =
    (Option<String>, Option<(std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>)>);

/// Career free roam's encounter racers' cars loading ahead in the menus
/// over the city (background loading, as [`Preload`]): their draws made on
/// a copy of the random numbers as free roam's set-up will make them, so
/// the loads are the ones `Encounters::new` will ask for; it takes a load
/// only for the same arguments.
pub(crate) struct EncountersAhead {
    /// What the draws were made from.
    pub(crate) inputs: EncounterInputs,
    /// The predictions made on this run of menus (at most
    /// [`ENCOUNTER_PREDICTIONS`]).
    pub(crate) predictions: u32,
    pub(crate) loads: Vec<(RivalKey, std::thread::JoinHandle<Result<Car>>)>,
}

/// What free roam's encounter set-up (`Encounters::picks`) reads, as the
/// menus stand: the shared generator's state, the player's car type (its
/// name, rating and SUV pool), the career difficulty, and the car key
/// (the profile's car, parts and performance the racers are matched to).
#[derive(Clone, PartialEq)]
pub(crate) struct EncounterInputs {
    pub(crate) rng: u32,
    pub(crate) car: (String, u32, bool),
    pub(crate) difficulty: u8,
    pub(crate) key: CarKey,
}

/// A screen on the way to free roam is up this long (s) before the
/// encounter racers are predicted and loaded behind it.
pub(crate) const ENCOUNTERS_AFTER: f32 = 0.5;

/// The predictions one run of menus makes at most (each a set-up's draws
/// on a copy, about 2 ms of the main thread, and up to three car loads).
pub(crate) const ENCOUNTER_PREDICTIONS: u32 = 8;

/// A region's world as `drive_in` loads it, and its renderer (built for
/// the device and colour format given) with its uploads done.
pub(crate) type PreloadedWorld = (Result<World>, Option<PreloadedRenderer>);

/// A preloaded world's renderer with the device and colour format it was
/// built for, and the world's effect objects made beside it (the water,
/// the smoke's GPU half; with the programs made ahead:
/// `enh::warm::world_programs`).
pub(crate) type PreloadedRenderer =
    (wgpu::Device, wgpu::TextureFormat, WorldRenderer, nfsu2::render::enh::warm::WorldAhead);

/// Loads done ahead, on threads of their own, while the player is in the
/// menus (docs/native-deviations.md §4: the clone's loads hidden behind the front
/// end; the game loads only on the track loader's steps). They are the
/// same calls `drive_in` makes, so what it takes from here is what it
/// would have loaded itself; it waits for one still running rather than
/// load twice. `NFSU2_NO_PRELOAD=1` turns it off.
pub(crate) struct Preload {
    pub(crate) region: String,
    pub(crate) world: Option<std::thread::JoinHandle<PreloadedWorld>>,
    pub(crate) car: Option<(CarKey, CarJob)>,
    /// The car's renderer, built on the car's thread once it is loaded
    /// ([`CarRendererAhead`]), for the drive that takes the car.
    pub(crate) car_renderer: Option<CarRendererAhead>,
}

/// A preloaded car's renderer (`CarRenderer::new`, as `drive_in` builds
/// the player's), built on the car's thread after its load, with the key
/// of what it was built for (`render::cars_ahead::BuildKey`); the load
/// takes it when the key is its own, else builds it as before. A renderer
/// that has not drawn holds no state.
pub(crate) type CarRendererAhead =
    std::sync::Arc<std::sync::Mutex<Option<(crate::render::cars_ahead::BuildKey, CarRenderer)>>>;

/// A car loading ahead on a thread, or its load joined once it was done
/// (so the menus can read the car before `drive_in` takes it).
pub(crate) enum CarJob {
    Loading(std::thread::JoinHandle<Result<Car>>),
    Done(Result<Car>),
}

impl CarJob {
    pub(crate) fn is_finished(&self) -> bool {
        match self {
            CarJob::Loading(h) => h.is_finished(),
            CarJob::Done(_) => true,
        }
    }

    /// The load's result, waited for if still running.
    pub(crate) fn join(self) -> Result<Car> {
        match self {
            CarJob::Loading(h) => h.join().expect("the car's preload"),
            CarJob::Done(r) => r,
        }
    }

    /// The car, once its load is done (joined then; never waits).
    pub(crate) fn car(&mut self) -> Option<&Car> {
        if let CarJob::Loading(h) = self {
            if !h.is_finished() {
                return None;
            }
            let CarJob::Loading(h) = std::mem::replace(self, CarJob::Done(Err(anyhow::anyhow!("taken")))) else {
                unreachable!()
            };
            *self = CarJob::Done(h.join().expect("the car's preload"));
        }
        match self {
            CarJob::Done(Ok(car)) => Some(car),
            _ => None,
        }
    }
}

impl Preload {
    /// The loads' parallel parts run on a pool of their own (half the
    /// cores), not rayon's global one, which the frame's own parallel work
    /// would otherwise queue behind (menu frames of 10–19 ms instead of
    /// 3–4 measured with the global pool).
    pub(crate) fn pool() -> &'static rayon::ThreadPool {
        static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
        POOL.get_or_init(|| {
            let n = std::thread::available_parallelism().map_or(2, |n| n.get() / 2).max(2);
            rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .thread_name(|i| format!("preload {i}"))
                .build()
                .expect("the preload pool")
        })
    }

    pub(crate) fn enabled() -> bool {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| debug_env().no_preload.as_ref().is_none_or(|v| v == "0"))
    }

    /// `region`'s world and renderer (`gpu`: the window's device and
    /// colour format) on a thread; with `warm`, the career stage's free-roam
    /// routes (and the encounter racers' racing speeds for each class) too.
    /// With `programs` (the set a drive started now would take), the
    /// renderer's own warm-up for it (`WorldRenderer::warm_drive`: what a
    /// load's warm-up makes on this renderer) and the programs made through
    /// a world's objects (`enh::warm::world_programs`): the load then finds
    /// them made, or makes them itself for another set.
    pub(crate) fn world(
        caches: Arc<Caches>,
        root: PathBuf,
        region: String,
        gpu: Option<(Arc<Gpu>, wgpu::TextureFormat)>,
        warm: Option<(u32, Option<nfsu2::ai::route::ClassCurves>, Option<usize>, bool)>,
        programs: Option<nfsu2::render::enh::warm::ProgramSet>,
    ) -> std::thread::JoinHandle<PreloadedWorld> {
        std::thread::Builder::new()
            .name("preload".into())
            .spawn(move || {
                Self::pool().install(move || {
                    let _t = nfsu2::prof::span("preload: world");
                    let world = World::load(&root, &region);
                    // Its collision too, which the drive's first placing
                    // made in the region's first frame (the same object).
                    if let Ok(w) = &world {
                        w.collision_ahead();
                    }
                    let renderer = match (&world, gpu) {
                        (Ok(w), Some((gpu, format))) => {
                            let mut r = WorldRenderer::new(&gpu, w, format, &nfsu2::render::RenderOptions::in_force());
                            let index = gpu.queue.submit([]);
                            let _ =
                                gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None });
                            let mut ahead = nfsu2::render::enh::warm::WorldAhead::default();
                            if let Some(set) = programs.filter(|s| s.format == format) {
                                let _t = nfsu2::prof::span("preload: programs");
                                r.warm_drive(&gpu, w, set.samples, &set.opts);
                                if r.wants_spray_volume(&set.opts) {
                                    r.offer_spray_volume(WorldRenderer::spray_volume_ahead(&gpu, &set.opts));
                                }
                                if set.mirror {
                                    r.warm_rear_view(&gpu);
                                }
                                ahead = nfsu2::render::enh::warm::world_programs(&gpu, &set, w, &region);
                            }
                            Some((gpu.device.clone(), format, r, ahead))
                        }
                        _ => None,
                    };
                    if let Some((stage, curves, mode, suv)) = warm {
                        let _t = nfsu2::prof::span("preload: routes");
                        let _ = free_roam_routes(&caches, &root, stage);
                        std::thread::scope(|s| {
                            for class in 0..4 {
                                let (caches, root, curves) = (&*caches, &root, curves.as_ref());
                                s.spawn(move || {
                                    encounter_routes(caches, root, stage, curves, mode.map(|m| (class, suv, m)))
                                });
                            }
                        });
                    }
                    (world, renderer)
                })
            })
            .expect("the preload thread")
    }
}
