use crate::*;

impl Game {
    /// Load `region` (keeping the current one when it matches) with the
    /// profile's car, and put the car at `at`.
    /// `tuning` picks which of the car's five settings it drives with
    /// (0x528610 → the car object's 0x516AC0).
    ///
    /// The track's barriers are set up for `barriers` as the track loader
    /// does (0x57A660 → 0x578070, after the last race's or free roam's end
    /// switched its scenery off): the race's on; the player's car takes its
    /// own list (0x604270 → 0x5F54B0). The game loads the region again
    /// (0x57A590 → 0x5D8360: every barrier group off) only for another
    /// region or a test run (0x581972); the clone's reload for another car
    /// keeps the switches.
    pub(crate) fn drive_in(
        &mut self,
        region: &str,
        at: Vec3,
        heading: f32,
        race: RaceMode,
        tuning: usize,
        barriers: BarrierSetup,
    ) -> Result<()> {
        // Every race and free roam start is a track load in the game: the
        // track loader's first step opens PC_Loading (0x57F333 → 0x4C57C0)
        // and the flow's LOADING state closes it on leaving (0x6038F0
        // message 2 → 0x4A9310). The clone's load is synchronous, and
        // the screen goes up only once the load has taken
        // [`LOADING_DELAY_MS`] (agreed deviation, docs/native-deviations.md "Loading screens": the original always
        // shows it, but a load kept in memory or cached is near instant).
        self.begin_loading();
        self.loads.waits.load_begin(region);
        // The reader ahead gives way to the load (docs/read-ahead.md).
        let _hold = nfsu2::cache::read_ahead::hold();
        let _t = nfsu2::prof::span("drive_in");
        let result = self.drive_in_loaded(region, at, heading, race, tuning, barriers);
        self.loads.loading = None;
        self.loads.waits.load_end(result.is_ok());
        result
    }

    /// A track load starts: PC_Loading is due (drawn by
    /// [`LoadingGate`] once the load takes long enough).
    pub(crate) fn begin_loading(&mut self) {
        let now = self.now();
        let loading = screens::Loading { package: screens::Loading::PC, opened: now };
        self.loads.loading = Some(loading);
        self.loads.track_loads += 1;
        // The flow's LOADING stops the song (0x481210(1) → state 7); the
        // next one starts once the race runs (0x47B080, not while
        // PC_Loading is up, 0x4A9330).
        if let Some(a) = self.audio.as_mut() {
            a.streams.music_frame(
                &a.mixer,
                MusicContext::Stopped,
                &[],
                false,
                nfsu2::audio::music::SCORE_FRONT_END,
                None,
                None,
                0.0,
            );
        }
    }

    /// The drive is over (its car changed, a new career, …): its world and
    /// renderer stay at hand for the next load of the region.
    pub(crate) fn drop_drive(&mut self) {
        self.reset_car_sound_context();
        // A race over with the drive: its rivals' cars kept to race again go.
        self.loads.raced_cars.clear();
        // The track goes and any in-engine scene running in it with it (a
        // race's ending held behind the post-race screens, a start or shop
        // scene the pause interrupted): none may outlive it into the front
        // end, whose music it would stop (`music_context_of`).
        self.scenes.shop = None;
        self.scenes.crowd = None;
        (self.dyno.rest, self.dyno.rollers, self.dyno.spin) = (None, None, None);
        if let Some(mut d) = self.sim.drive.take() {
            let device = self.platform.window.as_ref().map(|w| w.gpu.device.clone());
            self.render.resident_water = Some(std::mem::take(&mut d.effects.water).for_new_drive());
            self.loads.resident = Some((d.region, d.world));
            self.loads.kept_ahead = None;
            self.render.resident = device.zip(self.render.world.take());
            // World teardown (0x5FF7CD): the native clocks' reset.
            self.pacing.clocks.world_teardown();
            // The car's renderer goes with the drive.
            self.render.cars.remove(&d.car_id);
        }
    }

    pub(crate) fn loading_quads(&self, now: f32) -> Vec<Quad> {
        self.loads.loading.map(|l| l.quads(&self.game_data.assets, now)).unwrap_or_default()
    }

    pub(crate) fn drive_in_loaded(
        &mut self,
        region: &str,
        at: Vec3,
        heading: f32,
        race: RaceMode,
        tuning: usize,
        barriers: BarrierSetup,
    ) -> Result<()> {
        self.reset_car_sound_context();
        // The world sound managers' set-up (0x476E00 picks the plane
        // table by the race; 0x476F00 the fountains by the track number).
        if let Some(a) = self.audio.as_mut() {
            a.enviro_setup =
                nfsu2::audio::enviro::Setup { track: barriers.track, drag: race == RaceMode::Drag, url: race.url() };
            // A new car's SFXCTL_Physics: +0x6C clear.
            a.soundfx.blown_asked = false;
        }
        // The renderers below write to the GPU: the last frame first.
        if let Some(w) = self.platform.window.as_mut() {
            w.settle();
        }
        let (car_name, installed, perf) = self.player_car();
        let perf = self.with_uniques(&perf);
        let reuse = self
            .sim.drive
            .as_ref()
            .is_some_and(|d| d.region == region && d.car.car_type.name.eq_ignore_ascii_case(&car_name));
        // A test run (0x89E7E6, the one drive with the dyno's sliders)
        // loads its region again.
        let region_kept = self.sim.drive.as_ref().is_some_and(|d| d.region == region) && tuning != DYNO_SLIDERS;
        if reuse && !region_kept {
            self.sim.drive.as_mut().unwrap().world.reset_groups();
        }
        // The load's PC_Loading gate, kept over the warm-up at the end (its
        // clock is the load's: the 200 ms rule counts the warm-up too).
        let mut load_gate = None;
        if !reuse {
            // The dyno's tuning goes on after the upgrades (0x5B4DD0 → 0x5B4790).
            // The test run's car: the mode's settings with the dyno's
            // sliders (0x83A611) copied over.
            let settings = if tuning == DYNO_SLIDERS {
                self.dyno_settings()
            } else {
                self.profile.as_ref().map(|p| p.tuning[tuning.min(4)]).unwrap_or_default()
            };
            // The game's loaders are steps on the main loop (TrackLoader::
            // LoadHandler 0x57F2E0: each step queues its files with a
            // completion callback and returns), so the front end keeps
            // running and PC_Loading animates (its dots) over the load
            // (verified under Wine 2026-10-29). The clone loads the world
            // and the car side by side on workers and draws the package
            // meanwhile.
            // The region's world when it is in memory: the drive's own (a
            // load of the same region for another car) or the one a car
            // change left. It goes back to its state as loaded (the
            // barrier switches kept when the region is kept, as below), so
            // it is the world a fresh load would give.
            let device = self.platform.window.as_ref().map(|w| w.gpu.device.clone());
            let kept = self.take_kept_world(region, region_kept, &device);
            // What the menus loaded ahead ([`Preload`]): the same loads,
            // taken (or waited for) instead of made again.
            let key: CarKey = (car_name.clone(), installed.clone(), perf.clone(), settings);
            let (preloaded_car, car_renderer_ahead) = self.take_preloaded_car(&key).unzip();
            if let Some(l) = self.loads.waits.load_mut() {
                l.car = match &preloaded_car {
                    Some(job) if job.is_finished() => "loaded ahead",
                    Some(_) => "loading ahead (waited for)",
                    None => "loaded",
                };
            }
            let (world, car, kept_renderer, mut fresh_renderer, mut gate) =
                self.load_world_and_car(region, region_kept, kept, &key, preloaded_car, &device)?;
            self.loads.waits.load_part("world and car");
            // What the kept world's renderer and effects were made ready
            // with in the menus ([`KeptAhead`]).
            let extras = self.render.kept_extras.take().filter(|_| kept_renderer.is_some());
            let mut drive = self.build_drive(region, at, heading, world, car);
            // The effect objects made ahead for this world with its
            // preloaded renderer (the water, the smoke's GPU half): the
            // drive's own, which the warm-up below would build on fresh
            // ones; it then finds them built.
            if let Some((_, ahead)) = fresh_renderer.as_mut() {
                let ahead = std::mem::take(ahead);
                if let Some(water) = ahead.water {
                    drive.effects.water = water;
                }
                if let Some(cinematic) = ahead.cinematic {
                    drive.effects.cinematic = cinematic;
                }
                if let Some(detail) = ahead.detail {
                    drive.effects.detail = detail;
                }
            }
            let mut extras = extras.map(|mut e| {
                if let Some((cinematic, detail)) = e.smoke.take() {
                    drive.effects.cinematic = cinematic;
                    drive.effects.detail = detail;
                }
                e.emitters
            });
            // The kept world's water (`Water::for_new_drive`): what the
            // warm-up would build again for the same world.
            if let Some(water) = self.render.resident_water.take().filter(|_| kept_renderer.is_some()) {
                drive.effects.water = water;
            }
            let mut fresh_renderer = fresh_renderer.map(|r| r.0);
            // A new world's renderer is the longest step left (on this
            // thread): PC_Loading goes up before it only if the load has
            // already run [`LOADING_DELAY_MS`] (agreed deviation, "Loading screens": no
            // flash of the loading screen for a quick switch).
            if kept_renderer.is_none() && fresh_renderer.is_none() {
                gate.tick(&mut self.platform.window, &self.game_data.assets, now_ms(self.pacing.started));
            }
            // The new drive's world renderer, in place once the drive
            // is (below); the old drive's stays drawn until then.
            let mut world_renderer = None;
            if let Some(w) = &self.platform.window {
                world_renderer = Some(match kept_renderer {
                    Some(mut r) => {
                        let _t = nfsu2::prof::span("drive_in: reset placements");
                        match extras.take() {
                            Some(emitters) => nfsu2::render::modern_light_bold::set_loaded_emitters(emitters),
                            None => r.reset_placements(&w.gpu, &drive.world),
                        }
                        r
                    }
                    None => fresh_renderer
                        .take()
                        .unwrap_or_else(|| WorldRenderer::new(&w.gpu, &drive.world, w.config.format, &nfsu2::render::RenderOptions::in_force())),
                });
                let _t = nfsu2::prof::span("drive_in: car renderer");
                let opts = nfsu2::render::RenderOptions::in_force();
                // Built on the preloaded car's thread (its load joined
                // above, so the build has ended) for what this one is for.
                let key = crate::render::cars_ahead::BuildKey::now(&w.gpu, w.config.format, &opts);
                let built = car_renderer_ahead.flatten().and_then(|s| s.lock().unwrap().take()).filter(|(k, _)| *k == key);
                let renderer = built.map(|(_, r)| r).unwrap_or_else(|| CarRenderer::new(&w.gpu, &drive.car, w.config.format, &opts));
                self.render.cars.set(&drive.car_id, renderer);
                drop(_t);
                // The uploads done behind the loading screen (their staging
                // memory freed), not in the first frame's submission.
                let _t = nfsu2::prof::span("drive_in: uploads");
                let index = w.gpu.queue.submit([]);
                let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None });
                // The pipelines the load built, kept for the next run.
                w.gpu.save_pipeline_cache();
            }
            self.loads.waits.load_part("renderers and uploads");
            nfsu2::cache::release_mapped_pages();
            // Car +0x464 from the player's Stability Control option
            // (0x5F1802 → 0x5F3E20).
            drive.state.stability_off = self.settings.players[0].stability_off;
            // The old drive (a region left: `take_kept_world` put it
            // back) goes here, its world's renderer then its car's.
            let old_world = std::mem::replace(&mut self.render.world, world_renderer);
            let old = self.sim.drive.replace(drive);
            drop(old_world);
            if let Some(old) = old {
                self.render.cars.remove(&old.car_id);
            }
            load_gate = Some(gate);
        }
        // World create (0x5FF54D): the native clocks' reset .
        self.pacing.clocks.world_create();
        // The HUD's textures, their HD UI art made with the load (the
        // HUD's first frame stalled 50-100 ms on it). Off without the pack.
        let hud_textures = if self.platform.window.is_some() && nfsu2::hd_ui_pack::active() {
            self.hud_textures(race, barriers.track)
        } else {
            Vec::new()
        };
        // What the warm-up below makes the drive's programs for (read
        // here on this thread: the worker read the globals while this
        // thread ticked the load and drew its frames).
        let warm_for = self.platform.window.as_ref().map(|w| (w.gpu.clone(), self.warm_target(&w.gpu)));
        let drive = self.sim.drive.as_mut().unwrap();
        if reuse {
            drive.restart_clock();
        }
        drive.barriers = barriers;
        drive.world.set_up_barriers(&barriers);
        // The track's world animations as 0x43A4A0 creates them for the
        // track number (the windowed ones only where the
        // track admits them); their placements go to the renderer below
        // with the barriers' (a preloaded or kept renderer included).
        drive.world.set_track(barriers.track as u32);
        let changed = drive.world.take_changed();
        if let (Some(r), Some(w)) = (self.render.world.as_mut(), &self.platform.window) {
            for p in changed {
                let placement = &drive.world.placements[p];
                r.update_placement(&w.gpu, p, placement.transform, placement.flags);
            }
            // Ambient Detail's props of the last drive on this world (a
            // world kept for this one) are checked against this drive's
            // barriers by the warm-up below, and made again if they differ.
            r.new_drive_ambient();
            // Light Probes baked under the last drive's set-up: those that
            // see a placement this one switched are baked again.
            r.new_drive_probes(&drive.world, nfsu2::render::RenderOptions::in_force().light_probes);
        }
        drive.car.set_race(race);
        drive.state.ground.barriers = drive.world.racer_barriers(&barriers);
        drive.place(at, heading);
        // The racer is made again on every drive-in: no saved places yet.
        drive.roam = Progress::default();
        drive.state.start_race(race);
        // Racer info +0x20 (0x5F17F9): the player's Transmission option;
        // a drag race forces manual (0x5EB440).
        drive.state.manual |= self.settings.players[0].manual;
        // The log's waits: the old drive gone, the HUD's textures, the
        // barriers and placements (docs/waits-log.md).
        self.loads.waits.load_part("set-up");
        // What the drive's first frame makes on first use (its
        // pipelines at the frame's samples, Light Probes' baker, Ambient
        // Detail's props), made here, behind the loading screen. On a
        // worker while this thread ticks the load's gate, so PC_Loading
        // goes up once the whole load (warm-up included) passes
        // [`LOADING_DELAY_MS`] (docs/perf-loading-pacing.md §5).
        if let Some((gpu, target)) = warm_for {
            let _t = nfsu2::prof::span("drive_in: warm");
            // The post chain and the Enhanced passes the drive's frames run
            // (docs/first-use-builds.md): the chain (made now if the menus
            // have not: as the first frame would make it) is taken out of
            // the window while the loading frames draw (they do not use it).
            let format = {
                let w = self.platform.window.as_mut().unwrap();
                let format = w.config.format;
                w.post.get_or_insert_with(|| PostFx::new(&w.gpu, format));
                format
            };
            let mut post = self.platform.window.as_mut().and_then(|w| w.post.take());
            let (renderer, car_renderer) = (self.render.world.as_mut(), self.render.cars.get_mut(&drive.car_id));
            let (world, region, effects) = (&drive.world, drive.region.as_str(), &mut drive.effects);
            let warm = || {
                super::warm::warm_passes(
                    &gpu,
                    &target,
                    format,
                    renderer,
                    car_renderer,
                    post.as_mut(),
                    world,
                    region,
                    effects,
                );
                post
            };
            let post = match load_gate.as_mut() {
                Some(gate) => std::thread::scope(|scope| {
                    let job = scope.spawn(warm);
                    // Ticked at least once: a load already past the delay
                    // when the warm-up starts shows it. The HUD's textures
                    // meanwhile, one between two ticks.
                    let mut textures = hud_textures.iter();
                    loop {
                        gate.tick(&mut self.platform.window, &self.game_data.assets, now_ms(self.pacing.started));
                        if let Some(&h) = textures.next() {
                            prefetch_ui_textures(&mut self.platform.window, &self.game_data.assets, &[h]);
                            continue;
                        }
                        if job.is_finished() {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    job.join().expect("the drive's warm-up")
                }),
                None => {
                    let post = warm();
                    prefetch_ui_textures(&mut self.platform.window, &self.game_data.assets, &hud_textures);
                    post
                }
            };
            if let Some(w) = self.platform.window.as_mut() {
                w.post = post;
            }
            self.render.warmed = Some(target);
            gpu.save_pipeline_cache();
            self.loads.waits.load_part("programs");
        }
        if let Some(gate) = load_gate {
            if let Some(l) = self.loads.waits.load_mut() {
                l.shown = gate.shown;
            }
            self.loads.loads_shown += gate.shown as u32;
            self.loads.loading = gate.loading;
        }
        Ok(())
    }

    /// The region's world when it is in memory (`drive_in_loaded`): the
    /// drive's own or the one a car change left, back to its state as
    /// loaded; the drive is put back when its region is another.
    fn take_kept_world(
        &mut self,
        region: &str,
        region_kept: bool,
        device: &Option<wgpu::Device>,
    ) -> Option<(World, Option<WorldRenderer>)> {
        let kept = match self.sim.drive.take() {
            Some(mut d) if d.region == region => {
                // The world's renderer kept (its water with it), the
                // car's dropped with the drive.
                self.render.resident_water = Some(std::mem::take(&mut d.effects.water).for_new_drive());
                let kept = Some((d.world, self.render.world.take()));
                self.render.cars.remove(&d.car_id);
                kept
            }
            Some(d) => {
                self.sim.drive = Some(d);
                None
            }
            None => None,
        };
        // Made ready in the menus ([`KeptAhead`]): taken as it is (the
        // kept world's own reset is the one below, without the barriers
        // kept, which a region kept by the drive alone asks for).
        if kept.is_none() && self.loads.kept_ahead.is_some() {
            debug_assert!(!region_kept, "a region kept by its drive is that drive's world");
            self.loads.resident = None;
            self.render.resident = None;
            return self.take_kept_ahead(region, device);
        }
        self.loads.kept_ahead = None;
        let kept = kept.or_else(|| {
            let r = self.render.resident.take();
            self.loads.resident
                .take()
                .filter(|r| r.0 == region)
                .map(|(_, world)| (world, r.filter(|(d, _)| Some(d) == device.as_ref()).map(|r| r.1)))
        });
        // The water goes with the kept renderer (the same device).
        let water = self.render.resident_water.take().filter(|_| kept.as_ref().is_some_and(|k| k.1.is_some()));
        self.loads.resident = None;
        self.render.resident = None;
        let kept = kept.and_then(|(mut world, renderer)| {
            let _t = nfsu2::prof::span("drive_in: world kept");
            world.reset_to_loaded(region_kept).then_some((world, renderer))
        });
        self.render.resident_water = water.filter(|_| kept.is_some());
        kept
    }

    /// The world (kept, preloaded or loaded) and the car (preloaded or
    /// loaded) side by side, PC_Loading ticked meanwhile
    /// (`drive_in_loaded`): the world, the car, the kept world's renderer,
    /// the preloaded one, and the loading gate.
    fn load_world_and_car(
        &mut self,
        region: &str,
        region_kept: bool,
        kept: Option<(World, Option<WorldRenderer>)>,
        key: &CarKey,
        preloaded_car: Option<CarJob>,
        device: &Option<wgpu::Device>,
    ) -> Result<(
        World,
        Car,
        Option<WorldRenderer>,
        Option<(WorldRenderer, nfsu2::render::enh::warm::WorldAhead)>,
        LoadingGate,
    )> {
        let (car_name, installed, perf, settings) = (&key.0, &key.1, &key.2, key.3);
        if let Some(l) = self.loads.waits.load_mut() {
            let ahead = self.loads.preload.as_ref().filter(|p| p.region == region).and_then(|p| p.world.as_ref());
            l.world = match ahead {
                _ if kept.is_some() => "kept",
                Some(job) if job.is_finished() => "loaded ahead",
                Some(_) => "loading ahead (waited for)",
                None => "loaded",
            };
        }
        let preloaded_world = if kept.is_none() {
            match self.loads.preload.take() {
                Some(mut p) if p.region == region => {
                    let world = p.world.take();
                    if p.car.is_some() {
                        self.loads.preload = Some(p);
                    }
                    world
                }
                other => {
                    self.loads.preload = other;
                    None
                }
            }
        } else {
            None
        };
        let format = self.platform.window.as_ref().map(|w| w.config.format);
        let (root, data, window, assets, loading, started) =
            (&self.game_data.root, &self.game_data.career, &mut self.platform.window, &self.game_data.assets, self.loads.loading, self.pacing.started);
        let mut gate = LoadingGate::new(loading);
        let load_car = || {
            Car::load_tuned(
                root,
                &car_name,
                &installed,
                &data.packages,
                &|id| perf.contains(&id),
                Some((&data.tuning, &settings)),
            )
        };
        // A preloaded car's load joined (or, failed, made here).
        let car_of = |h: Option<CarJob>| -> Result<Car> {
            // The drive's world sound banks, read beside the car.
            nfsu2::audio::carfx::prewarm_drive_banks(root);
            match h.map(CarJob::join) {
                Some(Ok(car)) => Ok(car),
                _ => load_car(),
            }
        };
        let mut fresh_renderer = None;
        let (world, car, kept_renderer) = match kept {
            Some((world, renderer)) => {
                let car = std::thread::scope(|scope| {
                    let car = scope.spawn(|| car_of(preloaded_car));
                    while !car.is_finished() {
                        gate.tick(window, assets, now_ms(started));
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    car.join().expect("the car's load")
                });
                (world, car?, renderer)
            }
            None => {
                let (world, car) = std::thread::scope(|scope| {
                    let car = scope.spawn(|| car_of(preloaded_car));
                    let world = scope.spawn(|| match preloaded_world {
                        Some(h) => {
                            let (world, renderer) = h.join().expect("the world's preload");
                            (world, renderer)
                        }
                        None => {
                            // Its collision beside the car's load (the
                            // drive's placing made it afterwards).
                            let world = World::load(root, region);
                            if let Ok(w) = &world {
                                w.collision_ahead();
                            }
                            (world, None)
                        }
                    });
                    while !(car.is_finished() && world.is_finished()) {
                        gate.tick(window, assets, now_ms(started));
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    (world.join().expect("the world's load"), car.join().expect("the car's load"))
                });
                let ((world, renderer), car) = (world, car?);
                let world = world?;
                // The preloaded renderer was built from the world as
                // loaded, for this device and format.
                fresh_renderer =
                    renderer.filter(|(d, f, ..)| device.as_ref() == Some(d) && format == Some(*f)).map(|r| (r.2, r.3));
                if region_kept && let Some(old) = &self.sim.drive {
                    world.copy_barriers_from(&old.world);
                    fresh_renderer = None;
                }
                (world, car, None)
            }
        };
        Ok((world, car, kept_renderer, fresh_renderer, gate))
    }

    /// The new drive on `world` with `car` at `at` (`drive_in_loaded`):
    /// its chase camera, state, rain cover zones and props.
    fn build_drive(&self, region: &str, at: Vec3, heading: f32, world: World, car: Car) -> Drive {
        // The camera the player's Camera option names (0x5F2FB0 →
        // 0x502500).
        let chase = ChaseCamera::new(&car, VIEW_ORDER[self.settings.players[0].camera.min(4) as usize]);
        let state = CarState::new(at, heading);
        let fov_offset = nfsu2::qol::fov_offset();
        let camera = chase.clone().update(&state, 0.0, fov_offset);
        let mut drive = Drive {
            region: region.into(),
            world,
            car,
            car_id: CarId::new(),
            state,
            chase,
            fov_offset,
            camera,
            fog: FogState::default(),
            tint: ScreenTintState::default(),
            screen_flash: Default::default(),
            horizon_fog: Default::default(),
            motion: Default::default(),
            time_of_day: rain_clock().map_or_else(TimeOfDay::default, |c| TimeOfDay::default().after(c - 1.0)),
            clock: rain_clock().unwrap_or(1.0),
            world_clock: Default::default(),
            weather: nfsu2::weather::Weather::with_lens(&self.sim.lens),
            human_road: Default::default(),
            event_mover: 0,
            node_counts: (0, 0),
            props: Props::default(),
            posed: Default::default(),
            effects: Effects::default(),
            barriers: BarrierSetup::default(),
            flare_mirrors: Default::default(),
            flare_pools: Default::default(),
            flare_pools_clock: 0.0,
            vis: Default::default(),
            flare_lights: Vec::new(),
            hydro: Default::default(),
            placed_stamp: None,
            roam: Default::default(),
            impact: Default::default(),
        };
        // The rain's cover zones (0x61E710: route triggers of kinds 5
        // and 6; the clone reads them from L4RA's PathsFreeRoam.bin, as
        // the drive-by zones do).
        drive.weather.set_covers([5, 6].map(|k| career::RouteTriggers::load_kind(&self.game_data.root, k)));
        let _t = nfsu2::prof::span("drive_in: props");
        drive.props = Props::new(&drive.world);
        drop(_t);
        drive
    }
}
