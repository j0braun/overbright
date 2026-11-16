use crate::*;

impl Game {
    /// The player's car as `drive_in` loads it with tuning `tuning`.
    pub(crate) fn car_key(&self, tuning: usize) -> CarKey {
        let (car_name, installed, perf) = self.player_car();
        let perf = self.with_uniques(&perf);
        let settings = if tuning == DYNO_SLIDERS {
            self.dyno_settings()
        } else {
            self.profile.as_ref().map(|p| p.tuning[tuning.min(4)]).unwrap_or_default()
        };
        (car_name, installed, perf, settings)
    }

    /// A load of the player's car (as `drive_in` makes it) on a thread.
    pub(crate) fn spawn_car_load(&self, key: &CarKey) -> std::thread::JoinHandle<Result<Car>> {
        let (root, packages, unlocks, key) =
            (self.game_data.root.clone(), self.game_data.career.packages.clone(), self.game_data.career.tuning.clone(), key.clone());
        std::thread::Builder::new()
            .name("preload car".into())
            .spawn(move || {
                let (name, installed, perf, settings) = key;
                Preload::pool().install(|| {
                    Car::load_tuned(
                        &root,
                        &name,
                        &installed,
                        &packages,
                        &|id| perf.contains(&id),
                        Some((&unlocks, &settings)),
                    )
                })
            })
            .expect("the preload thread")
    }

    /// [`Game::spawn_car_load`], and with a window the car's renderer built
    /// on the same thread after it ([`CarRendererAhead`]).
    pub(crate) fn spawn_car_load_drawn(&self, key: &CarKey) -> (std::thread::JoinHandle<Result<Car>>, Option<CarRendererAhead>) {
        let w = self.platform.window.as_ref().filter(|_| crate::loads::kept::KeptAhead::enabled());
        let Some(draw) = w.map(|w| (w.gpu.clone(), w.config.format, nfsu2::render::RenderOptions::in_force())) else {
            return (self.spawn_car_load(key), None);
        };
        let slot = CarRendererAhead::default();
        let (root, packages, unlocks, key) =
            (self.game_data.root.clone(), self.game_data.career.packages.clone(), self.game_data.career.tuning.clone(), key.clone());
        let to = slot.clone();
        let job = std::thread::Builder::new()
            .name("preload car".into())
            .spawn(move || {
                let (name, installed, perf, settings) = key;
                let car = Preload::pool().install(|| {
                    Car::load_tuned(
                        &root,
                        &name,
                        &installed,
                        &packages,
                        &|id| perf.contains(&id),
                        Some((&unlocks, &settings)),
                    )
                });
                if let Ok(car) = &car {
                    let (gpu, format, opts) = draw;
                    let _t = nfsu2::prof::span("preload car: renderer");
                    *to.lock().unwrap() = crate::render::cars_ahead::build(&gpu, car, format, &opts);
                }
                car
            })
            .expect("the preload thread");
        (job, Some(slot))
    }

    /// A rival's car load (as `line_up_rivals` makes it) on a thread.
    pub(crate) fn spawn_rival_load(&self, key: &RivalKey) -> std::thread::JoinHandle<Result<Car>> {
        let (root, packages, key) = (self.game_data.root.clone(), self.game_data.career.packages.clone(), key.clone());
        std::thread::Builder::new()
            .name("preload rival".into())
            .spawn(move || {
                let (name, installed, fitted) = key;
                Preload::pool()
                    .install(|| Car::load_upgraded(&root, &name, &installed, &packages, &|id| fitted.contains(&id)))
            })
            .expect("the preload thread")
    }

    /// [`Game::spawn_rival_load`], and with a window the car's renderer
    /// built on the same thread after it ([`CarRendererAhead`]).
    pub(crate) fn spawn_rival_load_drawn(&self, key: &RivalKey) -> (std::thread::JoinHandle<Result<Car>>, Option<CarRendererAhead>) {
        let w = self.platform.window.as_ref().filter(|_| crate::render::cars_ahead::with_loads());
        let Some(draw) = w.map(|w| (w.gpu.clone(), w.config.format, nfsu2::render::RenderOptions::in_force())) else {
            return (self.spawn_rival_load(key), None);
        };
        let slot = CarRendererAhead::default();
        let (root, packages, key) = (self.game_data.root.clone(), self.game_data.career.packages.clone(), key.clone());
        let to = slot.clone();
        let job = std::thread::Builder::new()
            .name("preload rival".into())
            .spawn(move || {
                let (name, installed, fitted) = key;
                let car = Preload::pool()
                    .install(|| Car::load_upgraded(&root, &name, &installed, &packages, &|id| fitted.contains(&id)));
                if let Ok(car) = &car {
                    let (gpu, format, opts) = draw;
                    *to.lock().unwrap() = crate::render::cars_ahead::build(&gpu, car, format, &opts);
                }
                car
            })
            .expect("the preload thread");
        (job, Some(slot))
    }

    /// Once a frame: in the menus with no drive (or one in another region
    /// than free roam's), free roam's world (unless
    /// it is resident) and the player's car as free roam loads it go on
    /// loading ahead ([`Preload`]); a car changed in the menus is loaded
    /// again once the last load is done.
    ///
    /// UI_EngageRaceDialog open at an event's marker: the race's region,
    /// when it is not the one driven, and the car as the race loads it,
    /// go on loading while the dialog is up. Driving (or racing) again,
    /// whatever is left is let go.
    pub(crate) fn preload_frame(&mut self) {
        if !Preload::enabled() || self.loads.loading.is_some() || self.platform.window.is_none() {
            return;
        }
        self.kept_ahead_frame();
        self.preload_owned_bits();
        self.preload_rivals();
        self.preload_companion();
        // The player's car key (`car_key(0)`, a ride built from
        // the record) made at most once a frame, and only where it is read.
        let mut key = None;
        self.preload_encounters(&mut key);
        self.preload_drive_loads(key);
    }

    /// `preload_frame`'s part map of the profile's cars
    /// (`NativeContext::owned_bits`, which every save of the profile reads,
    /// a race's end first): when the cars change (a car bought or taken, a
    /// new career) the map of a car it was never made for is made on a
    /// thread, so the save finds it made. A save before it is done makes it
    /// itself, as before (the same function of the part database, kept once
    /// made).
    pub(crate) fn preload_owned_bits(&mut self) {
        let Some(p) = self.profile.as_ref() else { return };
        let cars = p.slot_cars();
        if self.loads.owned_bits.as_ref().is_some_and(|(c, _)| *c == cars) {
            return;
        }
        let Some(ctx) = career::NativeContext::get() else { return };
        let missing: Vec<String> = cars.iter().filter(|c| !ctx.owned_bits_ready(c)).cloned().collect();
        let job = (!missing.is_empty()).then(|| {
            std::thread::Builder::new()
                .name("part map ahead".into())
                .spawn(move || {
                    for car in &missing {
                        let _ = ctx.owned_bits(car);
                    }
                })
                .expect("the part map's thread")
        });
        self.loads.owned_bits = Some((cars, job));
    }

    /// `preload_frame`'s free roam car key, made on first use this frame.
    fn free_roam_key<'a>(&self, key: &'a mut Option<CarKey>) -> &'a CarKey {
        key.get_or_insert_with(|| self.car_key(0))
    }

    /// `preload_frame`'s lost race's ending companion
    /// ([`crate::loads::CompanionAhead`]): once the player has finished and
    /// the winner, by the race end's finishing order, is a rival that has
    /// finished (a drift race, placed by its points, and a test run or a
    /// replay, which have no ending, are left out).
    fn preload_companion(&mut self) {
        let Mode::Race(race) = &self.mode else { return };
        if self.loads.companion_ahead.is_some()
            || race.racers.me.finished.is_none()
            || !race.scoring.board.is_empty()
            || race.test.is_some()
            || race.replay.playback.is_some()
            || !crate::render::cars_ahead::with_loads()
        {
            return;
        }
        let runs: Vec<crate::flow::race_end_impl::Run<'_>> = std::iter::once(&race.racers.me)
            .chain(race.racers.rivals.iter().map(|r| &r.progress))
            .map(|p| (String::new(), String::new(), p))
            .collect();
        let winner = crate::flow::race_end_impl::finish_order(&runs).first().copied().unwrap_or(0);
        let Some(rival) = winner.checked_sub(1) else { return };
        let decided = &race.racers.rivals[rival].progress;
        if decided.finished.is_none() || decided.dropped.is_some() || decided.ended.is_some() {
            return;
        }
        let Some(key) = self.loads.race_rivals.get(rival).cloned() else { return };
        let (job, renderer) = self.spawn_rival_load_drawn(&key);
        let Some(renderer) = renderer else { return };
        // The car itself is not kept: the ending copies the rival's.
        let job = std::thread::Builder::new()
            .name("companion ahead".into())
            .spawn(move || drop(job.join()))
            .expect("the companion thread");
        self.loads.companion_ahead = Some(crate::loads::CompanionAhead { rival, job, renderer });
    }

    /// `preload_frame`'s engaged race's rivals.
    fn preload_rivals(&mut self) {
        // The engaged race's rivals ([`RivalsAhead`]), and a Quick Race's
        // while its transmission question is up (the car picked; either
        // answer starts the race): predicted again when the random numbers
        // moved since (a few times at most).
        match (&self.mode, &self.sim.drive) {
            (Mode::Menu(Menu::Engage(e)), Some(_)) => {
                let index = e.event;
                self.rivals_ahead_for(index, None, Game::predict_rivals);
            }
            (Mode::Menu(Menu::Dialog(..)), _) if let Some(Some(index)) = self.shop.ask_gearbox => {
                let car = Some((self.quick.car.clone(), self.quick.parts.clone()));
                self.rivals_ahead_for(index, car, Game::predict_quick_rivals);
            }
            // Driving again (the race declined): let them go.
            (Mode::Explore, _) => self.loads.rivals_ahead = None,
            _ => {}
        }
    }

    /// The rivals of race `index` loading ahead ([`RivalsAhead`]) for the
    /// line-up `predict` gives from the random numbers as they stand (and
    /// a Quick Race's car picked, `car`).
    fn rivals_ahead_for(&mut self, index: usize, car: Option<QuickCar>, predict: fn(&Game, usize) -> Option<Vec<RivalKey>>) {
        let same = |a: &RivalsAhead| a.event == index && a.car == car;
        let fresh = self.loads.rivals_ahead.as_ref().is_none_or(|a| !same(a) || (a.rng != self.rng.0 && a.loads.len() < 16));
        if !fresh {
            return;
        }
        let Some(keys) = predict(self, index) else { return };
        let mut ahead = self.loads.rivals_ahead.take().filter(same).unwrap_or(RivalsAhead {
            event: index,
            car: car.clone(),
            rng: 0,
            loads: Vec::new(),
        });
        ahead.rng = self.rng.0;
        for key in keys {
            if !ahead.loads.iter().any(|(k, ..)| *k == key) {
                let (job, renderer) = self.spawn_rival_load_drawn(&key);
                ahead.loads.push((key, job, renderer));
            }
        }
        self.loads.rivals_ahead = Some(ahead);
    }

    /// `preload_frame`'s free roam encounter racers.
    fn preload_encounters(&mut self, key: &mut Option<CarKey>) {
        // Free roam's encounter racers ([`EncountersAhead`]) in the
        // menus that go on to free roam's set-up: the front end with no
        // drive (free roam's set-up once the career goes on), the menus
        // after a race (and a race's pause, Quit), a shop's and the garage
        // (Jump to Garage keeps the drive), and the screens that follow
        // them until a drive or a race runs (the post-race save point's
        // questions, the movie a won race plays).
        let listed = match &self.mode {
            Mode::Menu(_) if self.sim.drive.is_none() => true,
            Mode::Menu(
                Menu::Icons(_, MenuKind::PostRace | MenuKind::RacePause | MenuKind::Crib)
                | Menu::Results(..)
                | Menu::Stats(_)
                | Menu::Reward(_)
                | Menu::SponsorPopup(_),
            ) => true,
            Mode::Menu(_) => self.shop.visit.is_some(),
            _ => false,
        };
        let to_free_roam = listed || (self.loads.to_free_roam && matches!(self.mode, Mode::Menu(_)));
        self.loads.to_free_roam = to_free_roam;
        // Free roam's car: the drive's when free roam keeps it (`drive_in`'s
        // reuse), else the menus' preloaded one once its load is done.
        // The player's car (the ride built from the record, a part
        // search per slot: ~9 % of the main thread when it ran every frame)
        // only on the screens that go on to free roam: `kept` and `car` are
        // read only there.
        let kept = to_free_roam && {
            let player = self.player_car().0;
            self.sim.drive
                .as_ref()
                .is_some_and(|d| d.region == EXPLORE_REGION && d.car.car_type.name.eq_ignore_ascii_case(&player))
        };
        if to_free_roam && !kept && self.sim.drive.as_ref().is_none_or(|d| d.region != EXPLORE_REGION) {
            let key = self.free_roam_key(key);
            if let Some((_, job)) = self
                .loads.preload
                .as_mut()
                .filter(|p| p.region == EXPLORE_REGION)
                .and_then(|p| p.car.as_mut())
                .filter(|(k, _)| k == key)
            {
                job.car();
            }
        }
        let car = if kept {
            self.sim.drive.as_ref().map(|d| &d.car)
        } else if self.sim.drive.as_ref().is_none_or(|d| d.region != EXPLORE_REGION) {
            let key = self.free_roam_key(key);
            match self.loads.preload.as_ref().filter(|p| p.region == EXPLORE_REGION).and_then(|p| p.car.as_ref()) {
                Some((k, CarJob::Done(Ok(car)))) if k == key => Some(car),
                _ => None,
            }
        } else {
            None
        };
        // Not on a screen's first half second (its own set-up, as the
        // garage scene's car, goes first); a question over a screen
        // (UI_GenericDialog: the garage's Return to Explore Mode) sets
        // nothing up and goes on with the screen's time.
        let screen = match &self.mode {
            Mode::Menu(Menu::Dialog(..)) => self.loads.free_roam_menu,
            Mode::Menu(m) => Some(std::mem::discriminant(m)),
            _ => None,
        };
        let same = std::mem::replace(&mut self.loads.free_roam_menu, screen) == screen;
        self.loads.free_roam_menu_secs =
            if to_free_roam && same { self.loads.free_roam_menu_secs + self.pacing.frame_dt } else { 0.0 };
        let due = self.loads.free_roam_menu_secs >= ENCOUNTERS_AFTER;
        if !to_free_roam {
            // Driving or racing on: the random numbers move on.
            if matches!(self.mode, Mode::Explore | Mode::Race(_)) {
                self.loads.encounters_ahead = None;
            }
        } else if !due {
        } else if let Some(car) = car {
            // What the set-up's draws read besides the generator: the
            // player's car type, its rating and pool (`EncounterDraw`), the
            // difficulty, the performance matched to (`human_perf`: the
            // profile's car, parts and performance, the key's).
            let inputs = EncounterInputs {
                rng: self.rng.0,
                car: (car.car_type.name.clone(), car.car_type.handling.performance_rating.to_bits(), car.car_type.suv),
                difficulty: self.settings.difficulty,
                key: self.free_roam_key(key).clone(),
            };
            // Again on the first tick the inputs moved (a draw on the
            // shared generator, a car or part changed), once the last
            // loads are in.
            let fresh = self.loads.encounters_ahead.as_ref().is_none_or(|a| {
                a.inputs != inputs && a.predictions < ENCOUNTER_PREDICTIONS && a.loads.iter().all(|(_, h)| h.is_finished())
            });
            if fresh && let Some(keys) = self.predict_encounters(car) {
                let mut ahead = self.loads.encounters_ahead.take().unwrap_or(EncountersAhead {
                    inputs: inputs.clone(),
                    predictions: 0,
                    loads: Vec::new(),
                });
                (ahead.inputs, ahead.predictions) = (inputs, ahead.predictions + 1);
                // The cars no longer predicted are let go; the others kept.
                ahead.loads.retain(|(k, _)| keys.contains(k));
                for key in keys {
                    if !ahead.loads.iter().any(|(k, _)| *k == key) {
                        let job = self.spawn_rival_load(&key);
                        ahead.loads.push((key, job));
                    }
                }
                self.loads.encounters_ahead = Some(ahead);
            }
        }
    }

    /// `preload_frame`'s world and player's car: the engaged race's region,
    /// or free roam's.
    fn preload_drive_loads(&mut self, key: Option<CarKey>) {
        match (&self.mode, &self.sim.drive) {
            (Mode::Menu(Menu::Engage(e)), Some(d)) => {
                let event = &self.game_data.career.events[e.event];
                let track_no = event.races.first().map_or(event.track, |r| r.0);
                let Some(region) = self.game_data.career.tracks.get(&track_no).map(|t| t.region.clone()) else { return };
                if region == d.region || self.loads.preload.as_ref().is_some_and(|p| p.region == region) {
                    return;
                }
                let key = self.car_key(tuning_mode_for(event.kind));
                let gpu = self.platform.window.as_ref().map(|w| (w.gpu.clone(), w.config.format));
                let programs = self.program_set().filter(|_| crate::loads::warm::ProgramsAhead::enabled());
                let world = Preload::world(
                    self.game_data.caches.clone(),
                    self.game_data.root.clone(),
                    region.clone(),
                    gpu,
                    None,
                    programs,
                );
                let (car, car_renderer) = self.spawn_car_load_drawn(&key);
                self.loads.preload =
                    Some(Preload { region, world: Some(world), car: Some((key, CarJob::Loading(car))), car_renderer });
                return;
            }
            // A Quick Race about to start (its options, its car select):
            // its track's region when that is not free roam's, so the
            // start finds the world and its renderer loaded as free roam's
            // are from the other menus. The car is picked on these screens
            // and is loaded by the start.
            (Mode::Menu(m), None) => {
                let track = match m {
                    Menu::QrOptions(o) => Some(self.settings.qr[nfsu2::frontend::quick_race::race_type(o.mode)].track),
                    Menu::CarLot(l) => match l.state {
                        LotState::QuickRace(Some(e)) => self.game_data.career.events.get(e).map(|event| event.races.first().map_or(event.track, |r| r.0)),
                        _ => None,
                    },
                    _ => None,
                };
                let region = track.and_then(|t| self.game_data.career.tracks.get(&t)).map(|t| t.region.clone());
                match region {
                    Some(region) if region != EXPLORE_REGION => {
                        if self.loads.preload.as_ref().is_none_or(|p| p.region != region) {
                            let gpu = self.platform.window.as_ref().map(|w| (w.gpu.clone(), w.config.format));
                            let programs = self.program_set().filter(|_| crate::loads::warm::ProgramsAhead::enabled());
                            let world = Preload::world(
                                self.game_data.caches.clone(),
                                self.game_data.root.clone(),
                                region.clone(),
                                gpu,
                                None,
                                programs,
                            );
                            self.loads.preload = Some(Preload { region, world: Some(world), car: None, car_renderer: None });
                        }
                        return;
                    }
                    // Back in the other menus: free roam's again.
                    _ => {
                        if self.loads.preload.as_ref().is_some_and(|p| p.region != EXPLORE_REGION) {
                            self.loads.preload = None;
                        }
                    }
                }
            }
            // The menus after a race on a track of its own (the results,
            // the post-race menu): free roam's region as from the menus.
            (Mode::Menu(_), Some(d)) if d.region != EXPLORE_REGION => {}
            (Mode::Explore | Mode::Race(_), Some(_)) => {
                self.loads.preload = None;
                return;
            }
            _ => return,
        }
        if self.loads.preload.as_ref().is_some_and(|p| p.region != EXPLORE_REGION) {
            return;
        }
        let resident = self.loads.resident.as_ref().is_some_and(|r| r.0 == EXPLORE_REGION)
            || self.loads.kept_ahead.as_ref().is_some_and(|k| k.region == EXPLORE_REGION);
        let key = key.unwrap_or_else(|| self.car_key(0));
        let stage = self.profile.as_ref().map_or(0, |p| u32::from(p.stage));
        let programs = self.program_set().filter(|_| crate::loads::warm::ProgramsAhead::enabled());
        let p = self.loads.preload.get_or_insert_with(|| Preload { region: EXPLORE_REGION.into(), world: None, car: None, car_renderer: None });
        if !resident && p.world.is_none() {
            let gpu = self.platform.window.as_ref().map(|w| (w.gpu.clone(), w.config.format));
            // The encounter racers' speeds: free roam's track record's
            // mode, the player's pool (SUVs for an SUV).
            let mode = self.game_data.career.tracks.get(&4000).map(|t| nfsu2::ai::route::speed_mode(t.flags, t.number));
            let suv = self.game_data.car_types.iter().find(|t| t.name.eq_ignore_ascii_case(&key.0)).is_some_and(|t| t.suv);
            let curves = self.sim.ai.as_ref().and_then(|a| a.curves.clone());
            let warm = self.profile.is_some().then_some((stage, curves, mode, suv));
            p.world = Some(Preload::world(
                self.game_data.caches.clone(),
                self.game_data.root.clone(),
                EXPLORE_REGION.into(),
                gpu,
                warm,
                programs,
            ));
        }
        let stale = match &p.car {
            None => true,
            Some((k, h)) => *k != key && h.is_finished(),
        };
        if stale {
            let (job, car_renderer) = self.spawn_car_load_drawn(&key);
            let p = self.loads.preload.as_mut().unwrap();
            (p.car, p.car_renderer) = (Some((key, CarJob::Loading(job))), car_renderer);
        }
    }

    /// The preloaded car for `key` (None when there is none for it) and
    /// the renderer being built for it.
    pub(crate) fn take_preloaded_car(&mut self, key: &CarKey) -> Option<(CarJob, Option<CarRendererAhead>)> {
        let p = self.loads.preload.as_mut()?;
        if p.car.as_ref().is_some_and(|(k, _)| k == key) {
            return p.car.take().map(|(_, h)| (h, p.car_renderer.take()));
        }
        None
    }
}
