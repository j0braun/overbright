use crate::*;

/// The garage scene as loaded: the platform, its renderer and environment
/// cube, and the shown car.
pub(crate) struct GarageScene {
    pub(crate) platform: nfsu2::garage::Platform,
    pub(crate) renderer: Option<WorldRenderer>,
    pub(crate) car: Option<(String, std::collections::BTreeMap<u8, u32>, Car, Option<CarRenderer>)>,
    /// The performance parts the FE car was loaded with.
    pub(crate) car_perf: std::collections::BTreeSet<u8>,
    /// The FE car's part animation channels and the open flags last
    /// applied to them.
    pub(crate) channels: nfsu2::part_anim::Channels,
    pub(crate) applied: [bool; 4],
    /// The channels' poses at the tick's request time (settled on
    /// the tick, `garage_car_tick`; drawn by `garage_car_animate`).
    pub(crate) car_poses: Option<[[Option<nfsu2::part_anim::ChannelPose>; 2]; 4]>,
    /// The scene's particles (GarageMain's update 0x4D82D0 → 0x492E10):
    /// the system, the placed emitters and the front-end time last run.
    pub(crate) fx: Option<(nfsu2::particles::System, Vec<nfsu2::particles::Emitter>, f32)>,
    /// The FE car's hydraulics (0x4A6030) and the front-end time last run;
    /// the FE sounds its corners asked for since the game last took them.
    pub(crate) hydro: Option<(nfsu2::hydraulics::Garage, f32)>,
    pub(crate) hydro_sounds: Vec<u8>,
    /// The FE car's purge smoke emitters (0x61FBB0's kinds 13 / 14) and
    /// its transform as the last tick placed it.
    pub(crate) purge: PurgeSmoke,
    pub(crate) car_world: Option<glam::Mat4>,
    /// The FE car being built on a thread for a part change
    /// ([`GarageCarJob`]); the car shown stays drawn meanwhile.
    pub(crate) car_job: Option<GarageCarJob>,
    /// FE cars built ahead or shown before, newest first, with the
    /// Car Reflection Update Rate they were built for; and the builds ahead
    /// under way.
    pub(crate) ready: Vec<(GarageCarKey, Car, CarRenderer, u8)>,
    pub(crate) prefetch: Vec<GarageCarJob>,
    /// NFSU2_PREVIEW_LOG: the car asked and since when, until it shows
    /// (the part change's latency in the log).
    pub(crate) asked: Option<(GarageCarKey, std::time::Instant)>,
    pub(crate) asked_last: Option<GarageCarKey>,
    /// The FE car keys whose load failed this visit (shown, part change or
    /// built ahead): not built again until the next visit.
    pub(crate) car_failed: nfsu2::failed_load::FailedLoads<GarageCarKey>,
    /// GarageMain state 1 (0x4D85F8): the car a held request asks for
    /// (`GarageRequest::hold`), built but not shown yet, and the front-end
    /// time it was in; the game starts the platform's turn from then on.
    pub(crate) incoming: Option<(GarageCarKey, Car, Option<CarRenderer>, f32)>,
    /// GarageMain's other FE car (+0x4C / +0x50 by +0x54): the car a car
    /// change's turn carries out on its own disc, shown until the turn
    /// ends (+0xB64 = 0 at 0x4D84CF).
    pub(crate) outgoing: Option<OutgoingCar>,
    /// The developer harness's build delay (`dev_build_delay`): a car
    /// built in the tick, held back until this front-end time.
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) delayed: Option<(GarageCarKey, Option<Car>, f32)>,
}

/// The car a car change's turn carries out (`GarageScene::outgoing`), with
/// its hydraulics' corners and wheel drops, its hood / trunk / doors and
/// its camber as they stood when the turn began.
pub(crate) struct OutgoingCar {
    pub(crate) key: GarageCarKey,
    pub(crate) car: Car,
    pub(crate) renderer: Option<CarRenderer>,
    pub(crate) corner: [f32; 4],
    pub(crate) drops: [f32; 4],
    pub(crate) poses: [[Option<nfsu2::part_anim::ChannelPose>; 2]; 4],
    pub(crate) camber: f32,
}

impl GarageScene {
    /// A visit's scene on `platform`, before its first tick: no renderer,
    /// no car yet.
    pub(crate) fn new(platform: nfsu2::garage::Platform) -> GarageScene {
        GarageScene {
            platform,
            renderer: None,
            car: None,
            car_perf: Default::default(),
            channels: Default::default(),
            applied: [false; 4],
            car_poses: None,
            fx: None,
            hydro: None,
            hydro_sounds: Vec::new(),
            purge: Default::default(),
            car_world: None,
            car_job: None,
            ready: Vec::new(),
            prefetch: Vec::new(),
            asked: None,
            asked_last: None,
            car_failed: nfsu2::failed_load::FailedLoads::new(GARAGE_CARS_FAILED),
            incoming: None,
            outgoing: None,
            #[cfg(any(test, feature = "dev-harness"))]
            delayed: None,
        }
    }
}

/// The garage's FE car, its parts and performance parts.
pub(crate) type GarageCarKey = (String, std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>);

/// A part change's FE car built off the main thread (clone deviation: the
/// frame rate dropped sharply while picking different rims): the car
/// load (`Car::load_upgraded`) and its renderer (textures and buffers
/// uploaded) for the key, which `draw_garage` puts in place of the shown
/// car once done. Off with [`GARAGE_ASYNC`] (the headless shots and
/// checks): the change is built in the frame, as before.
pub(crate) struct GarageCarJob {
    pub(crate) key: GarageCarKey,
    pub(crate) handle: std::thread::JoinHandle<Option<(Car, CarRenderer)>>,
}

/// The FE cars built ahead at most at a time (a step up and down the
/// list) and the built cars kept beside the shown one (those two and the
/// car shown before), each its own car and renderer (size not measured).
pub(crate) const GARAGE_PREFETCH: usize = 2;

pub(crate) const GARAGE_READY: usize = 3;

/// A [`GarageCarJob`] for `key` on a thread of its own.
#[allow(clippy::too_many_arguments)]
pub(crate) fn garage_car_job(
    key: GarageCarKey,
    root: &std::path::Path,
    packages: &[nfsu2::performance::Package],
    gpu: &Gpu,
    format: wgpu::TextureFormat,
    env_rate: u8,
    envmap: Option<Vec<nfsu2::formats::texture::Texture>>,
    thread: &str,
) -> GarageCarJob {
    let (root, packages, worker) = (root.to_path_buf(), packages.to_vec(), gpu.car_worker());
    let k = key.clone();
    let handle = std::thread::Builder::new()
        .name(thread.into())
        .spawn(move || {
            let _t = nfsu2::prof::span("garage: car (thread)");
            #[cfg(any(test, feature = "dev-harness"))]
            if let Some(ms) = dev_build_delay_ms() {
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
            let car = Car::load_upgraded(&root, &k.0, &k.1, &packages, &|id| k.2.contains(&id)).ok()?;
            let r = garage_car_renderer(&worker, &car, format, env_rate, envmap.as_deref());
            Some((car, r))
        })
        .expect("the garage car's thread");
    GarageCarJob { key, handle }
}

/// Whether the garage builds a part change's FE car on a thread
/// ([`GarageCarJob`]); the deterministic headless modes (shots, the
/// selftest, flows, the playthrough) and NFSU2_GARAGE_SYNC=1 build it in
/// the frame.
pub(crate) static GARAGE_ASYNC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// The garage's FE car renderer: the car's, with the platform's cube as
/// its environment (0x5B9920 copies the platform's XPOS … ZNEG into the
/// front end's cube 0x87088C, the FE cars' EnvMapTexture — only with
/// g_CarEnvironmentMapEnable set; at 0 there is no cube, black).
pub(crate) fn garage_car_renderer(
    gpu: &Gpu,
    car: &Car,
    format: wgpu::TextureFormat,
    env_rate: u8,
    envmap: Option<&[nfsu2::formats::texture::Texture]>,
) -> CarRenderer {
    let mut r = CarRenderer::new(gpu, car, format, &nfsu2::render::RenderOptions::in_force());
    if env_rate == 0 {
        r.set_environment(gpu, &nfsu2::render::car::black_envmap(gpu));
    } else if let Some((size, data)) = fe_cube_texels(envmap) {
        r.set_environment(gpu, &nfsu2::render::car::cube_from_texels(gpu, size, &data));
    }
    r
}

/// The front end's cube 0x87088C as the platforms left it:
/// one cube for the whole run, which 0x5B9920 overwrites with each
/// platform's faces. Process-wide like the original's.
static FE_CUBE: std::sync::Mutex<Option<(u32, Vec<u8>)>> = std::sync::Mutex::new(None);

/// The cube's texels after a platform with `faces` was copied in: its six
/// faces; without them the previous platform's cube with each face's first
/// 1024 bytes zeroed (`envmap_without_faces`); None while no platform ever
/// supplied faces (the renderer's start cube, black, stays).
pub(crate) fn fe_cube_texels(faces: Option<&[nfsu2::formats::texture::Texture]>) -> Option<(u32, Vec<u8>)> {
    fe_cube_copy(&mut FE_CUBE.lock().unwrap_or_else(|e| e.into_inner()), faces.and_then(nfsu2::render::car::envmap_texels))
}

/// [`fe_cube_texels`] on a given cube, with the platform's faces decoded.
pub(crate) fn fe_cube_copy(cube: &mut Option<(u32, Vec<u8>)>, faces: Option<(u32, Vec<u8>)>) -> Option<(u32, Vec<u8>)> {
    let next = match faces {
        Some(t) => t,
        None => {
            let (size, data) = cube.take()?;
            nfsu2::render::car::envmap_without_faces(size, data)
        }
    };
    *cube = Some(next.clone());
    Some(next)
}

/// The FE car keys a visit keeps as failed (the shown car, a part change
/// and the builds ahead, [`GARAGE_PREFETCH`]).
pub(crate) const GARAGE_CARS_FAILED: usize = GARAGE_PREFETCH + 2;

/// The garage platform whose load failed (one: the request's name), kept
/// until the name changes or the garage closes (`draw_garage`). Outside
/// [`GarageScene`]: a failed platform leaves no scene. Per thread: the
/// garage is drawn on the main thread, and each test thread has its own.
pub(crate) fn garage_platform_failed<T>(f: impl FnOnce(&mut nfsu2::failed_load::FailedLoads<String>) -> T) -> T {
    thread_local! {
        static FAILED: std::cell::RefCell<nfsu2::failed_load::FailedLoads<String>> =
            const { std::cell::RefCell::new(nfsu2::failed_load::FailedLoads::new(1)) };
    }
    FAILED.with_borrow_mut(f)
}

/// GarageMain's platform (0x4A7330 loads it once per visit and draws
/// nothing without it), unless the same name failed this visit: the frame
/// asks every frame, so a missing or broken platform is read and logged
/// once.
pub(crate) fn garage_platform(root: &std::path::Path, name: &str) -> Option<nfsu2::garage::Platform> {
    garage_platform_failed(|f| f.load(name.to_string(), "garage", || nfsu2::garage::Platform::load(root, name)))
}

/// NFSU2_DEV_CAR_BUILD_DELAY_MS=n (developer harness only): every FE car
/// build takes n ms longer, as on a slow disk or a cold cache. A build on
/// a thread sleeps; a build in the tick (the deterministic headless modes)
/// is held back n ms of front-end time (`dev_build_delay`), so a test or
/// a frame dump sees the same latency at every frame rate.
#[cfg(any(test, feature = "dev-harness"))]
pub(crate) fn dev_build_delay_ms() -> Option<u64> {
    #[cfg(test)]
    if let Some(ms) = TEST_BUILD_DELAY_MS.get() {
        return Some(ms).filter(|&ms| ms > 0);
    }
    std::env::var("NFSU2_DEV_CAR_BUILD_DELAY_MS").ok()?.parse().ok().filter(|&ms| ms > 0)
}

#[cfg(test)]
thread_local! {
    /// A test's own build delay (`dev_build_delay_ms`), for the builds in
    /// its thread's ticks.
    pub(crate) static TEST_BUILD_DELAY_MS: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// A car built in the tick for `key` at front-end time `now`, through the
/// developer harness's build delay: `load` runs once, its result handed
/// out only once the delay has passed (None until then, Some(result)
/// after). Without the delay (and outside the developer harness) at once.
pub(crate) fn dev_build_delay(
    g: &mut GarageScene,
    key: &GarageCarKey,
    now: f32,
    load: impl FnOnce(&mut GarageScene) -> Option<Car>,
) -> Option<Option<Car>> {
    #[cfg(any(test, feature = "dev-harness"))]
    if let Some(ms) = dev_build_delay_ms() {
        if g.delayed.as_ref().is_none_or(|(k, ..)| k != key) {
            let car = load(g);
            g.delayed = Some((key.clone(), car, now + ms as f32 / 1000.0));
        }
        let ready = g.delayed.as_ref().is_some_and(|(_, _, at)| now >= *at);
        return ready.then(|| g.delayed.take().and_then(|d| d.1));
    }
    let _ = (key, now);
    Some(load(g))
}

/// The FE car built in the frame (no thread), unless its key failed this
/// visit; a failure is kept and logged once.
pub(crate) fn load_garage_car(
    failed: &mut nfsu2::failed_load::FailedLoads<GarageCarKey>,
    root: &std::path::Path,
    packages: &[nfsu2::performance::Package],
    key: GarageCarKey,
) -> Option<Car> {
    let k = key.clone();
    failed.load(key, "garage car", || Car::load_upgraded(root, &k.0, &k.1, packages, &|id| k.2.contains(&id)))
}

#[cfg(test)]
mod failed_garage_tests {
    use super::*;

    /// A missing FE car is built once across the frames, and
    /// a changed key once; the next visit (`forget`) tries it again.
    #[test]
    fn a_missing_garage_car_is_loaded_once_per_key_and_visit() {
        let root = std::path::Path::new("/nonexistent/ob-failed-garage");
        let mut failed = nfsu2::failed_load::FailedLoads::new(GARAGE_CARS_FAILED);
        let key = |n: &str| -> GarageCarKey { (n.to_string(), Default::default(), Default::default()) };
        for _ in 0..120 {
            if !failed.remembers(&key("NOCAR")) {
                assert!(load_garage_car(&mut failed, root, &[], key("NOCAR")).is_none());
            }
        }
        assert_eq!(failed.attempts(), 1, "one build across 120 frames");
        assert!(load_garage_car(&mut failed, root, &[], key("NOCAR2")).is_none());
        assert_eq!(failed.attempts(), 2, "a new key built once");
        assert!(load_garage_car(&mut failed, root, &[], key("NOCAR")).is_none());
        assert_eq!(failed.attempts(), 2, "the first still kept this visit");
        failed.forget();
        assert!(load_garage_car(&mut failed, root, &[], key("NOCAR")).is_none());
        assert_eq!(failed.attempts(), 3, "the next visit tries it again");
    }

    /// A missing platform is read once across the frames; a
    /// new name, and the same name after the garage closed, again.
    #[test]
    fn a_missing_garage_platform_is_read_once_per_name_and_visit() {
        let root = std::path::Path::new("/nonexistent/ob-failed-garage");
        let start = garage_platform_failed(|f| f.attempts());
        for _ in 0..120 {
            assert!(garage_platform(root, "OB_NO_PLATFORM").is_none());
        }
        assert_eq!(garage_platform_failed(|f| f.attempts()) - start, 1, "one read across 120 frames");
        assert!(garage_platform(root, "OB_NO_PLATFORM_2").is_none());
        assert_eq!(garage_platform_failed(|f| f.attempts()) - start, 2, "a new name read once");
        garage_platform_failed(|f| f.forget());
        assert!(garage_platform(root, "OB_NO_PLATFORM_2").is_none());
        assert_eq!(garage_platform_failed(|f| f.attempts()) - start, 3, "the next visit reads it again");
    }
}
