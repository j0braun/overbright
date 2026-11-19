use crate::*;

/// Draws GarageMain's scene (0x4A7330 for the platform, the FE car at
/// 0x4A6670's place) under the menu, after stepping it to the request
/// (`garage_scene_tick`): the shots' still (a request at its own time).
/// The game steps the scene on its tick and draws with [`render_garage`].
/// True when it drew.
#[cfg(feature = "dev-harness")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_garage(
    request: Option<GarageRequest>,
    root: &std::path::Path,
    caches: &Caches,
    packages: &[nfsu2::performance::Package],
    garage: &mut Option<GarageScene>,
    gpu: &Gpu,
    format: wgpu::TextureFormat,
    view: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    size: (u32, u32),
    t: f32,
    display: Display,
    opts: &nfsu2::render::RenderOptions,
    samples: u32,
    post: Option<&mut PostFx>,
) -> bool {
    garage_scene_tick(request.as_ref(), root, caches, packages, garage, Some((gpu, format)), display);
    render_garage(request, root, caches, garage, gpu, format, view, depth, size, t, display, opts, samples, post, None)
}

/// GarageMain's scene stepped to the tick's request, before
/// anything draws it (0x4D82D0, GarageMain's update in the front end's
/// tick): the platform loaded, the FE car's builds on threads taken when
/// done (only here, on a tick), its part change (built in the tick when
/// the deterministic headless modes turn `GARAGE_ASYNC` off: a car asked
/// on tick m is shown from tick m at every rate), the cars built ahead,
/// the hydraulics (their sounds queued for the next tick's input), the
/// hood / trunk / doors and the scene's particles. `gpu`: the window's,
/// which the builds on threads need (None headless: every build is made
/// here). No request: the garage is down (its failed loads forgotten, the
/// cars built ahead freed).
pub(crate) fn garage_scene_tick(
    request: Option<&GarageRequest>,
    root: &std::path::Path,
    caches: &Caches,
    packages: &[nfsu2::performance::Package],
    garage: &mut Option<GarageScene>,
    gpu: Option<(&Gpu, wgpu::TextureFormat)>,
    display: Display,
) {
    let Some(request) = request else {
        garage_release(garage);
        return;
    };
    // A visit's first tick loads the platform and the FE car (built in
    // the tick: no car shown yet to build beside): side by side, the car
    // on a thread (the same car the tick's load gives).
    let fresh = garage.as_ref().is_none_or(|g| g.platform.name != request.platform);
    let first_car: Option<GarageCarKey> =
        request.car.as_ref().filter(|_| fresh).map(|n| (n.clone(), request.installed.clone(), request.perf.clone()));
    let (g, mut first_car) = std::thread::scope(|scope| {
        let job = first_car.map(|key| {
            scope.spawn(move || {
                let car = Car::load_upgraded(root, &key.0, &key.1, packages, &|id| key.2.contains(&id));
                (key, car)
            })
        });
        // The front end's light rigs (FrontB, read once a run), which the
        // first draw reads (`frontend_rig`).
        if fresh {
            scope.spawn(move || frontend_rig(caches, root, &request.lights));
        }
        let mut g = garage_load(request, root, garage);
        // The platform's renderer, made here beside the car's load rather
        // than in the visit's first draw (`render_garage`; the same object,
        // from the settings in force).
        if let (Some(g), Some((gpu, format))) = (g.as_deref_mut(), gpu)
            && g.renderer.is_none()
        {
            let _t = nfsu2::prof::span("garage: platform renderer");
            g.renderer = Some(WorldRenderer::new(gpu, &g.platform.world, format, &nfsu2::render::RenderOptions::in_force()));
        }
        (g, job.and_then(|j| j.join().ok()))
    });
    let Some(g) = g else { return };
    let asynchronous = garage_take_built(g, request, display) && gpu.is_some();
    let env_rate = display.env_rate;
    garage_car_change(g, request, root, packages, gpu, asynchronous, env_rate, &mut first_car);
    garage_outgoing_release(g, request, asynchronous, env_rate);
    // The FE car's renderer, made here rather than in the visit's first
    // draw (`garage_car_animate`; the same object).
    if let (Some((gpu, format)), Some((_, _, car, renderer @ None))) = (gpu, g.car.as_mut()) {
        let _t = nfsu2::prof::span("garage: car renderer");
        *renderer = Some(garage_car_renderer(gpu, car, format, env_rate, g.platform.envmap.as_deref()));
    }
    garage_preview_log(g, request);
    garage_prefetch(g, request, root, packages, gpu, asynchronous, env_rate);
    let garage_view = request.view.at(request.now);
    garage_car_tick(g, request, &garage_view.turntable());
    garage_particles_update(g, request, &garage_view.camera.camera());
}

/// Draws GarageMain's scene as the tick left it (`garage_scene_tick`): the
/// camera, the car's angle and its steer are the request's view (the
/// constructor's record 0, 0x445500 with 0x7F6094, until a part view
/// blends them, `nfsu2::garage::View::at` the request's time), the platform
/// turned by a car change; otherwise GarageMain turns the car only on the
/// right stick (+0x248, 0x4A5D90) or in its idle camera tour (after 300 s,
/// 0x7F45D4), neither of which the clone has. 0x4D82D0 moves the camera to
/// record 1 for an SUV only when a car change's platform turn ends (not
/// ported). The FE car's hood, trunk and doors open as the request asks
/// (`nfsu2::part_anim`). Only the renderers (GPU caches) are written. True
/// when it drew. `drawn`: the frame's values between two ticks
/// (`Game::frame_garage`), drawn in place of the tick's view, car place,
/// wheel drops, part poses, camber, flares and particles; None: the tick's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_garage(
    request: Option<GarageRequest>,
    root: &std::path::Path,
    caches: &Caches,
    garage: &mut Option<GarageScene>,
    gpu: &Gpu,
    format: wgpu::TextureFormat,
    view: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    size: (u32, u32),
    t: f32,
    display: Display,
    opts: &nfsu2::render::RenderOptions,
    samples: u32,
    post: Option<&mut PostFx>,
    drawn: Option<&crate::garage::snapshot::GarageDrawn>,
) -> bool {
    let Some(request) = request else { return false };
    let Some(g) = garage.as_mut().filter(|g| g.platform.name == request.platform) else { return false };
    let renderer = g.renderer.get_or_insert_with(|| WorldRenderer::new(gpu, &g.platform.world, format, opts));
    // Full Screen Anti-Aliasing: the game's back buffer is multisampled
    // (0x5B9B3B), so the front end's scene is too.
    renderer.set_samples(gpu, samples, opts);
    let garage_view = drawn.map_or_else(|| request.view.at(request.now), |d| d.view);
    let shot = garage_view.camera;
    let table = garage_view.platform_turntable();
    for (slot, transform) in g.platform.poses(&table) {
        renderer.move_placement(gpu, slot, transform);
    }
    let camera = shot.camera();
    // NFS_FE_HW (a comparison aid, not the game): the projection's h / w
    // in place of the screen's. The Wine captures' front end uses the
    // window's client rectangle (0x5B9A70: GetWindowRect /
    // GetClientRect in windowed mode), ≈ 0.7206 there; see docs.
    let aspect = debug_env()
        .fe_hw
        .clone()
        .and_then(|v| v.parse::<f32>().ok())
        .map_or(size.0 as f32 / size.1 as f32, |hw| 1.0 / hw);
    let params = FrameParams {
        camera: &camera,
        aspect,
        visible_sections: None,
        far_sections: None,
        // The garage scenes have no fog maps: 0x5CAAE0's constants from
        // 0x614310's output 0x89E850 as the last game session left it (the
        // zero struct only before any, `Fog::frontend`).
        fog: Some(nfsu2::fog::Fog::frontend()),
        time: t,
        road_reflections: true,
        road_reflection_plane: None,
        sky: Default::default(),
        reversed: false,
        headlight_pools: &[],
        reflection_glows: &[],
        display,
        opts,
        rear_view: None,
        occluders: &[],
    };
    let frame = GarageFrame { gpu, format, view, depth, size, samples, display, t, camera, aspect };
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let (car_draw, open_fraction) = garage_car_animate(g, &request, root, caches, &frame, &garage_view, drawn);
    let outgoing = garage_outgoing_animate(g, &request, root, caches, &frame, &garage_view);
    let headlights = drawn.map_or(request.headlights, |d| d.headlights);
    garage_draw_car(g, headlights, &frame, &mut encoder, &params, car_draw, open_fraction, outgoing, post);
    garage_particles(g, &frame, &mut encoder, drawn, opts);
    gpu.queue.submit([encoder.finish()]);
    true
}

/// What `draw_garage`'s scene draws into, and its camera.
#[derive(Clone, Copy)]
struct GarageFrame<'a> {
    gpu: &'a Gpu,
    format: wgpu::TextureFormat,
    view: &'a wgpu::TextureView,
    depth: &'a wgpu::TextureView,
    size: (u32, u32),
    samples: u32,
    display: Display,
    t: f32,
    camera: Camera,
    aspect: f32,
}

/// `draw_garage` with no garage up: the failed loads forgotten, the cars
/// built ahead freed.
fn garage_release(garage: &mut Option<GarageScene>) {
    // No garage up: the next visit tries the loads that failed again
    // (the original loads the garage once per visit).
    garage_platform_failed(|f| f.forget());
    if let Some(g) = garage.as_mut() {
        g.car_failed.forget();
    }
    // No garage up: the cars built ahead are freed (off the
    // frame); the builds under way end on their own.
    if let Some(g) = garage.as_mut()
        && !(g.ready.is_empty() && g.prefetch.is_empty() && g.incoming.is_none() && g.outgoing.is_none())
    {
        let old = (std::mem::take(&mut g.ready), std::mem::take(&mut g.prefetch), g.incoming.take(), g.outgoing.take());
        std::thread::spawn(move || drop(old));
    }
}

/// `draw_garage`'s scene for the request's platform, loaded when it is
/// another (None when it fails to load).
fn garage_load<'a>(
    request: &GarageRequest,
    root: &std::path::Path,
    garage: &'a mut Option<GarageScene>,
) -> Option<&'a mut GarageScene> {
    if garage.as_ref().is_none_or(|g| g.platform.name != request.platform) {
        let _t = nfsu2::prof::span("garage: platform");
        *garage = garage_platform(root, &request.platform).map(GarageScene::new);
    }
    garage.as_mut()
}

/// The FE car `request` asks for.
fn request_key(request: &GarageRequest) -> Option<GarageCarKey> {
    request.car.as_ref().map(|n| (n.clone(), request.installed.clone(), request.perf.clone()))
}

/// The car shown now is `key`'s.
fn shows(g: &GarageScene, key: &GarageCarKey) -> bool {
    g.car.as_ref().is_some_and(|(n, i, ..)| *n == key.0 && *i == key.1) && g.car_perf == key.2
}

/// A built car not shown: kept with the cars built ahead when it has its
/// renderer (built on a thread), else freed off the frame.
fn keep_built(g: &mut GarageScene, key: GarageCarKey, car: Car, renderer: Option<CarRenderer>, env_rate: u8) {
    match renderer {
        Some(r) if GARAGE_ASYNC.load(std::sync::atomic::Ordering::Relaxed) => g.ready.insert(0, (key, car, r, env_rate)),
        r => drop(std::thread::spawn(move || drop((car, r)))),
    }
}

/// `draw_garage`'s builds done on threads put in place; whether the FE
/// car is built asynchronously. While the request holds its car back
/// (state 1) a build of that car becomes the incoming one, not the shown.
fn garage_take_built(g: &mut GarageScene, request: &GarageRequest, display: Display) -> bool {
    if g.car_job.as_ref().is_some_and(|j| j.handle.is_finished()) && request.hold {
        let job = g.car_job.take().unwrap();
        match job.handle.join().ok().flatten() {
            Some((car, r)) if request_key(request).as_ref() == Some(&job.key) => {
                if let Some((k, c, r, _)) = g.incoming.take() {
                    keep_built(g, k, c, r, display.env_rate);
                }
                g.incoming = Some((job.key, car, Some(r), request.now));
            }
            Some((car, r)) => keep_built(g, job.key, car, Some(r), display.env_rate),
            None => g.car_failed.remember(job.key, "garage car", "the build failed"),
        }
    }
    // A part change built on a thread: put in place once done (a stale
    // one too, as a step on the way), then the latest change started.
    if g.car_job.as_ref().is_some_and(|j| j.handle.is_finished()) {
        let job = g.car_job.take().unwrap();
        let _t = nfsu2::prof::span("garage: car swap");
        let old = match job.handle.join().ok().flatten() {
            Some((car, r)) => {
                let old_perf = std::mem::replace(&mut g.car_perf, job.key.2);
                // A new FE car starts with everything shut.
                (g.channels, g.applied) = (Default::default(), [false; 4]);
                // The car it replaces is kept for a step back.
                match g.car.replace((job.key.0, job.key.1, car, Some(r))) {
                    Some((n, installed, old, Some(old_r))) => {
                        g.ready.insert(0, ((n, installed, old_perf), old, old_r, display.env_rate));
                        None
                    }
                    old => old,
                }
            }
            None => {
                g.car_failed.remember(job.key, "garage car", "the build failed");
                g.car.take()
            }
        };
        // The old car's memory is freed off the frame.
        std::thread::spawn(move || drop(old));
    }
    let asynchronous =
        GARAGE_ASYNC.load(std::sync::atomic::Ordering::Relaxed) && g.car.as_ref().is_some_and(|c| c.3.is_some());
    // The cars built ahead (`GarageRequest::prefetch`) as they are
    // done, kept with the cars shown before ([`GARAGE_READY`] of them).
    let env_rate = display.env_rate;
    if asynchronous {
        let mut i = 0;
        while i < g.prefetch.len() {
            if g.prefetch[i].handle.is_finished() {
                let job = g.prefetch.swap_remove(i);
                match job.handle.join().ok().flatten() {
                    Some((car, r)) => g.ready.insert(0, (job.key, car, r, env_rate)),
                    None => g.car_failed.remember(job.key, "garage car (ahead)", "the build failed"),
                }
            } else {
                i += 1;
            }
        }
    }
    asynchronous
}

/// `garage_scene_tick`'s FE car brought to the request: from the cars
/// built ahead, as a build on a thread (`asynchronous`: `gpu` is there),
/// or loaded here.
#[allow(clippy::too_many_arguments)]
fn garage_car_change(
    g: &mut GarageScene,
    request: &GarageRequest,
    root: &std::path::Path,
    packages: &[nfsu2::performance::Package],
    gpu: Option<(&Gpu, wgpu::TextureFormat)>,
    asynchronous: bool,
    env_rate: u8,
    first_car: &mut Option<(GarageCarKey, anyhow::Result<Car>)>,
) {
    let key = request_key(request);
    // An incoming car no longer asked for is kept as built ahead.
    if g.incoming.as_ref().is_some_and(|i| Some(&i.0) != key.as_ref()) {
        let (k, c, r, _) = g.incoming.take().unwrap();
        keep_built(g, k, c, r, env_rate);
    }
    if request.hold {
        if let Some(key) = key {
            garage_car_hold(g, request, key, root, packages, gpu, asynchronous, env_rate, first_car);
        }
        return;
    }
    // The turn has begun (state 2): the incoming car takes the next disc,
    // the shown one rides out on its own (0x4D873A…0x4D87DF).
    if let Some((k, car, r, _)) = g.incoming.take() {
        garage_car_swap_in(g, request, k, car, r, env_rate);
        return;
    }
    match &request.car {
        // A change to a car built ahead or shown before: in place at
        // once (the same car a build would give: the build is
        // deterministic), the shown one kept for a step back.
        Some(name)
            if asynchronous
                && (g.car.as_ref().is_some_and(|(n, installed, ..)| n != name || *installed != request.installed)
                    || g.car_perf != request.perf)
                && g.ready.iter().any(|(k, .., rate)| {
                    k.0 == *name && k.1 == request.installed && k.2 == request.perf && *rate == env_rate
                }) =>
        {
            let _t = nfsu2::prof::span("garage: car swap (built ahead)");
            let at = g
                .ready
                .iter()
                .position(|(k, .., rate)| {
                    k.0 == *name && k.1 == request.installed && k.2 == request.perf && *rate == env_rate
                })
                .unwrap();
            let (key, car, r, _) = g.ready.remove(at);
            if debug_env().preview_log {
                log::info!("preview: {name} {:x?}", request.installed);
            }
            g.car_perf = key.2.clone();
            (g.channels, g.applied) = (Default::default(), [false; 4]);
            if let Some((n, installed, old, Some(old_r))) = g.car.replace((key.0, key.1, car, Some(r))) {
                g.ready.insert(0, ((n, installed, request.perf.clone()), old, old_r, env_rate));
            }
        }
        Some(name)
            if asynchronous
                && (g.car.as_ref().is_some_and(|(n, installed, ..)| n != name || *installed != request.installed)
                    || g.car_perf != request.perf) =>
        {
            // A car being built ahead for this change becomes the
            // change's job.
            if g.car_job.is_none()
                && let Some(at) = g
                    .prefetch
                    .iter()
                    .position(|j| j.key.0 == *name && j.key.1 == request.installed && j.key.2 == request.perf)
            {
                if debug_env().preview_log {
                    log::info!("preview: {name} {:x?}", request.installed);
                }
                g.car_job = Some(g.prefetch.swap_remove(at));
            }
            if g.car_job.is_none() {
                // NFSU2_PREVIEW_LOG (a test aid, the maintainer's preview_xtest.sh):
                // the FE car as loaded for the draw.
                if debug_env().preview_log {
                    log::info!("preview: {name} {:x?}", request.installed);
                }
                let key: GarageCarKey = (name.clone(), request.installed.clone(), request.perf.clone());
                let (gpu, format) = gpu.expect("an asynchronous build has the window's GPU");
                g.car_job = Some(garage_car_job(
                    key,
                    root,
                    packages,
                    gpu,
                    format,
                    env_rate,
                    g.platform.envmap.clone(),
                    "garage car",
                ));
            }
        }
        Some(name)
            if (g.car.as_ref().is_none_or(|(n, installed, ..)| n != name || *installed != request.installed)
                || g.car_perf != request.perf)
                && !g.car_failed.remembers(&(name.clone(), request.installed.clone(), request.perf.clone())) =>
        {
            let _t = nfsu2::prof::span("garage: car");
            g.car_job = None;
            let perf = &request.perf;
            // NFSU2_PREVIEW_LOG (a test aid, the maintainer's preview_xtest.sh): the
            // FE car as loaded for the draw.
            if debug_env().preview_log {
                log::info!("preview: {name} {:x?}", request.installed);
            }
            let key: GarageCarKey = (name.clone(), request.installed.clone(), perf.clone());
            let Some(built) = dev_build_delay(g, &key, request.now, |g| match first_car.take() {
                Some((k, car)) if k == key => g.car_failed.load(key.clone(), "garage car", || car),
                _ => load_garage_car(&mut g.car_failed, root, packages, key.clone()),
            }) else {
                return;
            };
            g.car = built.map(|c| (name.clone(), request.installed.clone(), c, None));
            g.car_perf = request.perf.clone();
            // A new FE car starts with everything shut.
            (g.channels, g.applied) = (Default::default(), [false; 4]);
        }
        Some(_) => {}
        None => (g.car, g.car_job) = (None, None),
    }
}

/// GarageMain state 1 (0x4D85F8) on the scene's side: the asked car
/// (`key`, of another type than the shown one) built while the shown car
/// stays on the front disc — from the cars built ahead, as a build on a
/// thread, or in the tick — and kept as `GarageScene::incoming` until the
/// game starts the turn.
#[allow(clippy::too_many_arguments)]
fn garage_car_hold(
    g: &mut GarageScene,
    request: &GarageRequest,
    key: GarageCarKey,
    root: &std::path::Path,
    packages: &[nfsu2::performance::Package],
    gpu: Option<(&Gpu, wgpu::TextureFormat)>,
    asynchronous: bool,
    env_rate: u8,
    first_car: &mut Option<(GarageCarKey, anyhow::Result<Car>)>,
) {
    if shows(g, &key) || g.incoming.is_some() || g.car_failed.remembers(&key) {
        return;
    }
    if debug_env().preview_log && g.car_job.as_ref().is_none_or(|j| j.key != key) {
        log::info!("preview: {} {:x?} (held)", key.0, key.1);
    }
    if asynchronous {
        if let Some(at) = g.ready.iter().position(|(k, .., rate)| *k == key && *rate == env_rate) {
            let (k, car, r, _) = g.ready.remove(at);
            g.incoming = Some((k, car, Some(r), request.now));
        } else if g.car_job.is_none() {
            g.car_job = match g.prefetch.iter().position(|j| j.key == key) {
                Some(at) => Some(g.prefetch.swap_remove(at)),
                None => {
                    let (gpu, format) = gpu.expect("an asynchronous build has the window's GPU");
                    Some(garage_car_job(key, root, packages, gpu, format, env_rate, g.platform.envmap.clone(), "garage car"))
                }
            };
        }
        // Another build under way: this one starts once it lands.
        return;
    }
    let Some(built) = dev_build_delay(g, &key, request.now, |g| match first_car.take() {
        Some((k, car)) if k == key => g.car_failed.load(key.clone(), "garage car", || car),
        _ => load_garage_car(&mut g.car_failed, root, packages, key.clone()),
    }) else {
        return;
    };
    if let Some(car) = built {
        g.incoming = Some((key, car, None, request.now));
    }
}

/// The incoming car put on show as the turn begins; the car shown so far
/// becomes the outgoing one (`GarageScene::outgoing`) with its corners,
/// drops, part poses and camber as they stand.
fn garage_car_swap_in(
    g: &mut GarageScene,
    request: &GarageRequest,
    key: GarageCarKey,
    car: Car,
    renderer: Option<CarRenderer>,
    env_rate: u8,
) {
    let _t = nfsu2::prof::span("garage: car swap (turn)");
    if let Some(o) = g.outgoing.take() {
        keep_built(g, o.key, o.car, o.renderer, env_rate);
    }
    let old_perf = std::mem::replace(&mut g.car_perf, key.2.clone());
    let corner = g.hydro.as_ref().map_or([0.0; 4], |(h, _)| h.corner);
    let drops = g.hydro.as_ref().map_or([0.0; 4], |(h, _)| std::array::from_fn(|i| h.wheel_drop(i)));
    let poses = g.car_poses;
    (g.channels, g.applied) = (Default::default(), [false; 4]);
    let Some((n, installed, old, old_r)) = g.car.replace((key.0, key.1, car, renderer)) else { return };
    let old_key = (n, installed, old_perf);
    if request.view.at(request.now).outgoing().is_some() {
        let poses = poses.unwrap_or_else(|| nfsu2::part_anim::Channels::default().poses(old.animations.as_ref(), request.now));
        let camber = nfsu2::car::camber_angle(&|id| old_key.2.contains(&id));
        g.outgoing = Some(OutgoingCar { key: old_key, car: old, renderer: old_r, corner, drops, poses, camber });
    } else {
        keep_built(g, old_key, old, old_r, env_rate);
    }
}

/// The outgoing car hidden once the turn has ended (0x4D84CF: its +0xB64
/// = 0), kept as built ahead.
fn garage_outgoing_release(g: &mut GarageScene, request: &GarageRequest, asynchronous: bool, env_rate: u8) {
    if g.outgoing.is_some() && request.view.at(request.now).outgoing().is_none() {
        let o = g.outgoing.take().unwrap();
        let r = o.renderer.filter(|_| asynchronous);
        keep_built(g, o.key, o.car, r, env_rate);
    }
}

/// `render_garage`'s outgoing car posed on its disc for the drawn view (its
/// hydraulics, part poses and camber as the turn found them, the view's
/// steer); None when no turn carries one out.
fn garage_outgoing_animate(
    g: &mut GarageScene,
    request: &GarageRequest,
    root: &std::path::Path,
    caches: &Caches,
    f: &GarageFrame,
    garage_view: &nfsu2::garage::View,
) -> Option<CarDraw> {
    let GarageFrame { gpu, format, display, t, .. } = *f;
    let table = garage_view.outgoing()?;
    let envmap = g.platform.envmap.as_deref();
    let o = g.outgoing.as_mut()?;
    let world = table.car_transform_on(&o.car.car_type.wheels, o.corner);
    let car = &o.car;
    let renderer = o.renderer.get_or_insert_with(|| garage_car_renderer(gpu, car, format, display.env_rate, envmap));
    renderer.prepare_reflection(gpu);
    let steer = garage_view.steer.to_radians();
    let wheels: [nfsu2::car::WheelPose; 4] = std::array::from_fn(|i| nfsu2::car::WheelPose {
        steer: if i < 2 { steer } else { 0.0 },
        lift: -o.drops[i],
        tilt: o.camber,
        ..Default::default()
    });
    renderer.set_part_poses(o.poses);
    renderer.set_part_glow(part_glow(car, t));
    let rig = frontend_rig(caches, root, &request.lights);
    let open_fraction = [0, 1].map(|c| o.poses[c][0].map(|p| p.value));
    Some(CarDraw { world, wheels, rig, open_fraction })
}

/// One FE car's draw: its world, wheels, light rig and the hood's and
/// trunk's open fractions.
struct CarDraw {
    world: glam::Mat4,
    wheels: [nfsu2::car::WheelPose; 4],
    rig: [nfsu2::render::lighting::RigLight; 3],
    open_fraction: [Option<f32>; 2],
}

/// `draw_garage`'s NFSU2_PREVIEW_LOG latency line.
fn garage_preview_log(g: &mut GarageScene, request: &GarageRequest) {
    // NFSU2_PREVIEW_LOG: a part change's latency, from the frame
    // that asked for it to the frame that shows it.
    if debug_env().preview_log {
        let want = request.car.as_ref().map(|n| (n.clone(), request.installed.clone(), request.perf.clone()));
        let shown = g.car.as_ref().map(|(n, i, ..)| (n.clone(), i.clone(), g.car_perf.clone()));
        if g.asked_last != want {
            g.asked_last = want.clone();
            g.asked = want.clone().map(|k| (k, std::time::Instant::now()));
        }
        if want == shown
            && let Some((_, t)) = g.asked.take()
        {
            log::info!("preview shown after {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        }
    }
}

/// `garage_scene_tick`'s builds ahead of the list's neighbours, and the
/// kept cars trimmed (`asynchronous`: `gpu` is there).
#[allow(clippy::too_many_arguments)]
fn garage_prefetch(
    g: &mut GarageScene,
    request: &GarageRequest,
    root: &std::path::Path,
    packages: &[nfsu2::performance::Package],
    gpu: Option<(&Gpu, wgpu::TextureFormat)>,
    asynchronous: bool,
    env_rate: u8,
) {
    // Build the list's neighbours ahead while no change is being
    // built (at most GARAGE_PREFETCH at a time), and keep the built cars
    // to GARAGE_READY, newest first (the rest freed off the frame).
    // NFSU2_GARAGE_NO_AHEAD=1: neither (an A/B aid: the change built when
    // asked, without building ahead).
    let ahead = !debug_env().garage_no_ahead;
    if asynchronous
        && let Some(name) = &request.car
        && let Some((gpu, format)) = gpu
    {
        if g.car_job.is_none() && ahead {
            for installed in &request.prefetch {
                if g.prefetch.len() >= GARAGE_PREFETCH {
                    break;
                }
                let key: GarageCarKey = (name.clone(), installed.clone(), request.perf.clone());
                let shown = g.car.as_ref().is_some_and(|(n, i, ..)| *n == key.0 && *i == key.1) && g.car_perf == key.2;
                if shown
                    || g.car_failed.remembers(&key)
                    || g.prefetch.iter().any(|j| j.key == key)
                    || g.ready.iter().any(|(k, .., rate)| *k == key && *rate == env_rate)
                {
                    continue;
                }
                g.prefetch.push(garage_car_job(
                    key,
                    root,
                    packages,
                    gpu,
                    format,
                    env_rate,
                    g.platform.envmap.clone(),
                    "garage car (ahead)",
                ));
            }
        }
        let keep = if ahead { GARAGE_READY } else { 0 };
        if g.ready.len() > keep {
            let old = g.ready.split_off(keep);
            std::thread::spawn(move || drop(old));
        }
    }
}

/// `garage_scene_tick`'s FE car stepped: the hydraulics
/// (0x4A6030), its place on the turntable from them, the hood / trunk /
/// doors asked as the request says and their poses settled at the
/// request's time (`Channels::poses` settles an open or shut channel, as
/// the game's draw does every frame); what `garage_car_animate` draws.
fn garage_car_tick(g: &mut GarageScene, request: &GarageRequest, table: &nfsu2::garage::Turntable) {
    let GarageScene { car, hydro, hydro_sounds, channels, applied, car_world, car_poses, .. } = g;
    *car_world = None;
    *car_poses = None;
    if let Some((_, _, car, _)) = car.as_ref() {
        let hy = garage_hydraulics_update(hydro, hydro_sounds, request);
        *car_world = Some(table.car_transform_on(&car.car_type.wheels, hy.corner));
        part_anim_update(channels, applied, car, request);
        *car_poses = Some(channels.poses(car.animations.as_ref(), request.now));
    }
}

/// `render_garage`'s FE car posed and animated as the tick left it
/// (hydraulics, steer, camber, hood / trunk / doors, glow): its world,
/// wheels and rig, and the hood's and trunk's open fractions.
fn garage_car_animate(
    g: &mut GarageScene,
    request: &GarageRequest,
    root: &std::path::Path,
    caches: &Caches,
    f: &GarageFrame,
    garage_view: &nfsu2::garage::View,
    drawn: Option<&crate::garage::snapshot::GarageDrawn>,
) -> (Option<(glam::Mat4, [nfsu2::car::WheelPose; 4], [nfsu2::render::lighting::RigLight; 3])>, [Option<f32>; 2]) {
    let GarageFrame { gpu, format, display, t, .. } = *f;
    // The FE car's pose and lighting, for the floor reflection and the
    // main view.
    let mut car_draw = None;
    let mut open_fraction = [None; 2];
    let (world, poses, drops, camber) = match drawn {
        Some(d) => (d.car_world, d.poses, d.drops, d.camber),
        None => (g.car_world, g.car_poses, g.hydro.as_ref().map(|(hy, _)| std::array::from_fn(|i| hy.wheel_drop(i))), request.camber),
    };
    if let (Some((_, _, car, car_renderer)), Some(world), Some(poses), Some(drops)) = (g.car.as_mut(), world, poses, drops) {
        let car_renderer = car_renderer.get_or_insert_with(|| {
            let _t = nfsu2::prof::span("garage: car renderer");
            garage_car_renderer(gpu, car, format, display.env_rate, g.platform.envmap.as_deref())
        });
        car_renderer.prepare_reflection(gpu);
        // 0x4A6E8B: wheels 0 and 1 turned by 0x7F45A4 about +z, which the
        // constructor sets from the shot's +0x04 and a part view from its
        // record's +0x10.
        let steer = garage_view.steer.to_radians();
        // The front rims' x-turn (camber, `Car::camber`); the brakes' is
        // set per draw below (0x620AF0 turns their frames in place).
        let wheels: [nfsu2::car::WheelPose; 4] = std::array::from_fn(|i| nfsu2::car::WheelPose {
            steer: if i < 2 { steer } else { 0.0 },
            lift: -drops[i],
            tilt: camber,
            ..Default::default()
        });
        // The hood's and trunk's open fractions for their neon (0x615A20:
        // 0x433CD0 ≠ shut, then 0x434610(channel, 0)).
        open_fraction = [0, 1].map(|c| poses[c][0].map(|p| p.value));
        car_renderer.set_part_poses(poses);
        car_renderer.set_part_glow(part_glow(car, t));
        let rig = frontend_rig(caches, root, &request.lights);
        car_draw = Some((world, wheels, rig));
    }
    (car_draw, open_fraction)
}

/// F5 (phase4-rate.md): the FE car's hydraulics, 0x4A6030 each
/// frame (from 0x4D82D0), stepped in 1/60 s frames up to the request's
/// time (at most 10 s of it at once), their sounds queued (played by the
/// next tick's input phase). Called from `garage_car_tick` (on the tick).
fn garage_hydraulics_update<'a>(
    hydro: &'a mut Option<(nfsu2::hydraulics::Garage, f32)>,
    hydro_sounds: &mut Vec<u8>,
    request: &GarageRequest,
) -> &'a nfsu2::hydraulics::Garage {
    let (level, lowering) = request.hydraulics;
    let (hy, last) =
        hydro.get_or_insert_with(|| (Default::default(), request.now - (request.now - request.asked).clamp(0.0, 10.0)));
    let mut left = (request.now - *last).clamp(0.0, 10.0);
    while left > 1e-6 {
        let dt = left.min(1.0 / 60.0);
        hydro_sounds.extend(hy.update(level, lowering, dt));
        left -= dt;
    }
    *last = request.now;
    if debug_env().hydro_log {
        eprintln!(
            "garage hydraulics level {level} lowering {lowering:?} mode {} corners {:?} at {:.2} s",
            hy.mode,
            hy.corner,
            request.now - request.asked
        );
    }
    hy
}

/// The hood, trunk and doors (0x61F830 → 0x433E20 at the
/// menu's request, 0x434190 at the draw): the channels asked to open or
/// shut as the request says. Called from `garage_car_tick`.
fn part_anim_update(
    channels: &mut nfsu2::part_anim::Channels,
    applied: &mut [bool; 4],
    car: &Car,
    request: &GarageRequest,
) {
    for c in 0..4 {
        if request.open[c] != applied[c] {
            if request.open[c] || channels.state(c) != nfsu2::part_anim::State::Shut {
                channels.request(car.animations.as_ref(), &car.part_styles, c, request.open[c], 1.0, request.asked);
            }
            applied[c] = request.open[c];
        }
    }
}

/// `draw_garage`'s scene with the FE car's floor reflection, then the
/// car, its shadow, depth of field, neon and flares.
#[allow(clippy::too_many_arguments)]
fn garage_draw_car(
    g: &mut GarageScene,
    headlights: bool,
    f: &GarageFrame,
    encoder: &mut wgpu::CommandEncoder,
    params: &FrameParams,
    car_draw: Option<(glam::Mat4, [nfsu2::car::WheelPose; 4], [nfsu2::render::lighting::RigLight; 3])>,
    open_fraction: [Option<f32>; 2],
    outgoing: Option<CarDraw>,
    post: Option<&mut PostFx>,
) {
    let GarageFrame { gpu, view, depth, size, samples, display, t, camera, aspect, .. } = *f;
    // The car a turn carries out (`GarageScene::outgoing`), drawn as the
    // shown one: floor reflection, car, shadow and neon.
    let out_car = outgoing.as_ref().and_then(|d| g.outgoing.as_ref().and_then(|o| o.renderer.as_ref().map(|r| (&o.car, r, d))));
    let out_draws = std::cell::Cell::new(0u32);
    let opts = params.opts;
    let renderer = g.renderer.as_mut().expect("the platform's renderer");
    // Frontend view 4 reflects the base about TYRE_HEIGHT (native
    // 0x630EDB–0x630F17), then Car_Draw adds its copied-matrix mesh bias.
    // Same-frame WorldView uploads and live 0x7F45A8 establish z=0.15;
    // the former WVP comparison mixed main/reflection depth projections
    // and could not establish the plane. See evidence/fe-reflection-plane.
    // Lighting uses this reflected base; floor/discs sample the result.
    let car_renderer = g.car.as_ref().and_then(|(_, _, _, r)| r.as_ref());
    // The car's draws so far this frame: each 0x620AF0 call turns the
    // front brakes' frames once more, in place (0x62E141 / 0x62EBB7), so
    // the original's main menu shows them at 1× the camber in the floor
    // reflection and 2× in the car view (apitrace: 3.5° and 7.0°).
    let draws = std::cell::Cell::new(0u32);
    let brakes_turned = |wheels: &[nfsu2::car::WheelPose; 4], n: u32| {
        wheels.map(|w| nfsu2::car::WheelPose { brake_tilt: w.tilt * n as f32, ..w })
    };
    renderer.render_with_reflection(gpu, encoder, view, depth, size, params, &mut |encoder, color, depth| {
        if let (Some(cr), Some((world, wheels, rig))) = (car_renderer, car_draw.as_ref()) {
            let mirrored = nfsu2::garage::reflected_car_world(*world);
            draws.set(draws.get() + 1);
            let wheels = &brakes_turned(wheels, draws.get());
            cr.render_reflection(
                gpu,
                encoder,
                color,
                depth,
                camera.view_projection(aspect),
                camera.view(),
                &CarLighting::frontend(rig, &camera, mirrored),
                mirrored,
                wheels,
                opts,
            );
            // The car's uniforms are rewritten for the main view below.
            let done = std::mem::replace(encoder, gpu.device.create_command_encoder(&Default::default()));
            gpu.queue.submit([done.finish()]);
        }
        if let Some((_, cr, d)) = out_car {
            let mirrored = nfsu2::garage::reflected_car_world(d.world);
            out_draws.set(out_draws.get() + 1);
            let wheels = &brakes_turned(&d.wheels, out_draws.get());
            cr.render_reflection(
                gpu,
                encoder,
                color,
                depth,
                camera.view_projection(aspect),
                camera.view(),
                &CarLighting::frontend(&d.rig, &camera, mirrored),
                mirrored,
                wheels,
                opts,
            );
            let done = std::mem::replace(encoder, gpu.device.create_command_encoder(&Default::default()));
            gpu.queue.submit([done.finish()]);
        }
    });
    let mut shadows = Vec::new();
    let mut quads = Vec::new();
    if let (Some(d), Some(OutgoingCar { car, renderer: Some(cr), .. })) = (outgoing.as_ref(), g.outgoing.as_mut()) {
        let wheels = brakes_turned(&d.wheels, out_draws.get() + 1);
        cr.render_sampled(
            gpu,
            samples,
            encoder,
            view,
            depth,
            camera.view_projection_depth(aspect, opts.reversed_z),
            camera.view(),
            &CarLighting::frontend(&d.rig, &camera, d.world),
            d.world,
            &wheels,
            opts,
        );
        let positions = car.car_type.wheels.map(|w| w.position);
        if let (true, Some(s)) = (display.shadows, nfsu2::shadow::car_shadow(d.world, car.shadow_box, positions, None)) {
            shadows.extend(car_shadow_passes(
                gpu,
                renderer,
                car,
                s.toward_eye(camera.position, d.world.w_axis.truncate(), nfsu2::shadow::shadow_pull(None)),
                "SHADOWFE",
                t,
                opts,
            ));
        }
        neon_tubes(&tube_view(&camera, aspect), car, d.world, t, d.open_fraction, &mut quads);
    }
    if let (Some((_, _, car, Some(car_renderer))), Some((world, wheels, rig))) = (g.car.as_mut(), car_draw) {
        let wheels = brakes_turned(&wheels, draws.get() + 1);
        car_renderer.render_sampled(
            gpu,
            samples,
            encoder,
            view,
            depth,
            camera.view_projection_depth(aspect, opts.reversed_z),
            camera.view(),
            &CarLighting::frontend(&rig, &camera, world),
            world,
            &wheels,
            opts,
        );
        // The front end's shadow (0x6162F0 with 0x8654A4 = 3): `%s_SHADOWFE`
        // laid flat at 0.152 m. Depth-tested, it hides under SHOWROOM's
        // disc tops (0.159 m), as the game's would.
        let wheels = car.car_type.wheels.map(|w| w.position);
        // g_CarShadowEnable (0x616D40).
        if let (true, Some(s)) = (display.shadows, nfsu2::shadow::car_shadow(world, car.shadow_box, wheels, None)) {
            // The neon's clock in the front end: 0x8651A8 × 0.00025, here
            // the garage's time.
            shadows.extend(car_shadow_passes(
                gpu,
                renderer,
                car,
                s.toward_eye(camera.position, world.w_axis.truncate(), nfsu2::shadow::shadow_pull(None)),
                "SHADOWFE",
                t,
                opts,
            ));
        }
        // Both cars' shadows in one pass (one vertex upload).
        if !shadows.is_empty() {
            renderer.render_shadows(gpu, encoder, view, depth, &shadows);
            shadows.clear();
        }
        // Depth of field (docs/motionblur-dof.md): Cinematic depth of field on the
        // garage's turntable, the car in focus, before its glows.
        if let Some(post) = post.filter(|_| nfsu2::render::blur_dof::dof_on(opts)) {
            let wheels = std::array::from_fn(|i| {
                (car.car_type.wheels[i].position, car.car_type.wheels[i].radius, car.car_type.wheels[i].width * 0.5)
            });
            let b = nfsu2::render::temporal::Car::new(0, world, car.shadow_box, wheels);
            let focus = nfsu2::render::blur_dof::focus_band(camera.position, world, b.lo, b.hi);
            let post = post.stage();
            let format = post.scene_format();
            let frame = nfsu2::render::blur_dof::DofFrame { camera, aspect, reversed: opts.reversed_z, focus };
            post.blur_dof
                .get_or_insert_with(|| nfsu2::render::blur_dof::BlurDof::new(gpu))
                .dof(gpu, encoder, view, depth, samples, size, format, &frame);
        }
        // The neon tubes (0x61ACA0 → 0x615A20 → 0x60D800): in the front
        // end drawn whatever the glow option (0x5CB633: 0x8654A4 ≠ 6 draws
        // 0x61ACA0 alone with the glow off), depth-tested, not written.
        // prov: inferred game.draw_garage
        // **Inferred**: into the scene itself (with the glow and streaks on
        // the game draws them in the glow pass, 0x5CB5BC / 0x5CBC93).
        neon_tubes(&tube_view(&camera, aspect), car, world, t, open_fraction, &mut quads);
        // The Lights list's flares (0x61ACF0 → 0x615640 for the FE car,
        // whose render info has no car object: the headlights at 1.0 in
        // the render info's headlight colour, +0x380, from the lights part
        // the list fits; the brake lights at 0.85, the centre one at 0).
        // prov: inferred game.draw_garage-2
        // **Inferred**: drawn with the neon into the scene.
        if headlights {
            let axes = CameraAxes::look(camera.forward(), camera.up);
            let view = nfsu2::glow::View {
                eye: camera.position,
                right: axes.right,
                down: axes.down,
                forward: axes.forward,
                view_projection: camera.view_projection(aspect),
                scale: nfsu2::render::cull_scale(camera.game_fov.unwrap_or(camera.fov_y)),
                front_end: true,
            };
            nfsu2::glow::car_sprites(&view, &car.light_flares, world, car.headlight_pool_color, false, &mut quads);
        }
    }
    // No shown car (an empty slot coming in): the outgoing car's shadow
    // and neon alone.
    if !shadows.is_empty() {
        renderer.render_shadows(gpu, encoder, view, depth, &shadows);
    }
    if !quads.is_empty() {
        renderer.upload_glows(gpu, &quads);
        renderer.draw_glows(encoder, view, depth, false);
    }
}

/// `render_garage`'s scene particles (and the purge test's smoke), as the
/// tick left them (`garage_particles_update`).
fn garage_particles(
    g: &mut GarageScene,
    f: &GarageFrame,
    encoder: &mut wgpu::CommandEncoder,
    drawn: Option<&crate::garage::snapshot::GarageDrawn>,
    opts: &nfsu2::render::RenderOptions,
) {
    let GarageFrame { gpu, view, depth, camera, .. } = *f;
    let renderer = g.renderer.as_mut().expect("the platform's renderer");
    let library = &g.platform.world.emitters;
    let Some(fx) = g.fx.as_ref() else { return };
    let axes = CameraAxes::look(camera.forward(), camera.up);
    let pixels_per_metre = nfsu2::render::cull_scale(camera.game_fov.unwrap_or(camera.fov_y));
    let quads = match drawn {
        Some(d) => fx.0.quads_scaled_where(&d.particles, &d.scale, library, camera.position, axes.right, -axes.down, axes.forward, pixels_per_metre, |_| true),
        None => fx.0.quads(library, camera.position, axes.right, -axes.down, axes.forward, pixels_per_metre),
    };
    renderer.render_particles(gpu, encoder, view, depth, &quads, opts);
}

/// The scene's particles, stepped on the tick
/// (`garage_scene_tick`; the generator's draws with them, docs/native-fixes/N-RNG.tsv): the
/// placed emitters bound and started at the load (0x48BC00, 0x48BC50), run
/// with the front end's time (0x4D82D0 → Particles_Update 0x492E10) and
/// drawn over the scene by `garage_particles` (0x48BDF0). A still starts
/// as the scene would stand `now` seconds after the menu asked for it (at
/// most 10 s, in 1/30 s steps).
fn garage_particles_update(g: &mut GarageScene, request: &GarageRequest, camera: &Camera) {
    {
        let library = &g.platform.world.emitters;
        let fx = g.fx.get_or_insert_with(|| {
            let mut rng = nfsu2::particles::Rng(0x1234_5679);
            let mut placed = Vec::new();
            for p in g.platform.world.placed_emitters.iter().filter(|p| p.state <= 1 && p.switch == 0) {
                let Some(d) = library.by_hash(p.description) else { continue };
                let mut e = nfsu2::particles::Emitter::new(d, p.transform, (rng.next() * 4.0e9) as u32);
                e.on = p.state == 1;
                e.start(library);
                placed.push(e);
            }
            (nfsu2::particles::System::default(), placed, request.now - (request.now - request.asked).clamp(0.0, 10.0))
        });
        fx.0.views = vec![camera.position];
        let mut left = (request.now - fx.2).clamp(0.0, 10.0);
        // The purge test fires once per FE tick; the clone ticks it every
        // 1/120 s of front-end time (the original's frame cap, at which
        // the reference captures ran; **inferred** rate).
        let purge = request.purge.then(|| g.car.as_ref().and_then(|c| c.2.purge.clone()).zip(g.car_world)).flatten();
        if purge.is_none() {
            g.purge.groups = [0; 2];
        }
        while left > 0.0 {
            let dt = left.min(if purge.is_some() { 1.0 / 120.0 } else { 1.0 / 30.0 });
            if let Some((p, world)) = purge.as_ref() {
                let mut rng = nfsu2::particles::Rng(0x1234_5679 ^ (fx.0.particles.len() as u32));
                g.purge.burst(&mut fx.0, library, p, *world, Vec3::ZERO, 0, &mut rng);
            }
            for e in &mut fx.1 {
                fx.0.run(library, e, dt);
            }
            fx.0.update(library, dt, |_| None);
            left -= dt;
        }
        fx.2 = request.now;
    }
}
