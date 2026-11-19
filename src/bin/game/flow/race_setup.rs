use crate::*;

impl Game {
    /// Load `track`'s route and region and put the player's car on grid
    /// slot 0.
    /// `career`: a career race (game mode 1), for the barriers.
    pub(crate) fn drive_to_grid(
        &mut self,
        track: &career::Track,
        variant: u8,
        mode: RaceMode,
        tuning: usize,
        career: bool,
    ) -> Result<(career::Route, Vec3, f32)> {
        // A race loads its own track, so the next free roam loads track
        // 4000 again, and the track loader's state 6 (0x57F2E0 → 0x412BE0)
        // gives its traffic models anew for the zone the player is in then.
        // Drop free roam's pool, so `explore` builds it again.
        self.sim.traffic = None;
        // A custom race of the map editor runs its own route on the track.
        let route = match self.map_race_route(track) {
            Some(r) => r?,
            None => career::load_route(&self.game_data.root, track, variant & 1 != 0)?,
        };
        let (at, heading) = route.grid.first().copied().unwrap_or_else(|| {
            let a = route.checkpoints[0];
            let b = route.checkpoints[1];
            (a.extend(0.0), (b - a).y.atan2((b - a).x))
        });
        // The race's barriers: its track record's number (+0x8A, i16) and
        // direction (variant bit 0, 0x89E7A4).
        let stage = self.profile.as_ref().map_or(0, |p| p.stage);
        let owns_car = self.profile.as_ref().is_some_and(|p| !p.car.is_empty());
        let barriers = BarrierSetup {
            track: track.number as i16 as i32,
            free_roam: false,
            career,
            stage,
            reversed: variant & 1 != 0,
            owns_car,
        };
        self.drive_in(&track.region, at, heading, mode, tuning, barriers)?;
        // The grid slot's ground (0x5EB5E0 → 0x5E72F0): some markers lie
        // under the map.
        if let Some(d) = self.sim.drive.as_mut() {
            let z = d.world.grid_height(at);
            d.place(Vec3::new(at.x, at.y, z), heading);
            // Held on the grid until the green light (0x410430 with 1).
            d.state.held = true;
        }
        Ok((route, at, heading))
    }

    /// Test Run (0x53C8D0 → 0x526980(0)): the tuning session's drive for
    /// the dyno's mode (0x83A63C) on its own track with no opponents
    /// (0x89E7C0 = 1): Circuit and URL Race 0x4FEF80 (track 4705, 10
    /// laps), Drag 0x4FEFF0 (4709, the drag flag, 1 lap), Drift 0x4FF070
    /// (4708, the drift flag, 10), Street X 0x4FF0E0 (4708, the street X
    /// flag, 10); the car with that mode's settings and the dyno's sliders
    /// (0x83A611). With no mode chosen the session reloads the dyno
    /// (0x4FF160, track 4706): the clone stays in the dyno menu.
    pub(crate) fn start_test_run(&mut self) -> Result<()> {
        let m = self.dyno.tuning_mode as usize;
        let Some(&(number, mode, laps)) = TEST_RUNS.get(m) else {
            self.mode = self.icon_menu(MenuKind::Dyno);
            return Ok(());
        };
        let track = self.game_data.career.tracks.get(&number).context("test run track")?.clone();
        let (route, at, _) = self.drive_to_grid(&track, 0, mode, DYNO_SLIDERS, false)?;
        let laps = if route.loops { laps } else { 1 };
        // Every local human racer has a style manager (0x5FC3A6), a drift
        // race also the drift manager (0x5F7A10).
        let style =
            Some(nfsu2::drift::StyleManager::new(self.game_data.tricks.clone(), nfsu2::drift::mode_mask(mode, !route.loops)));
        let drift = mode.drift().then(|| {
            let class = self.sim.drive.as_ref().map_or(3, |d| d.car.car_type.handling.performance_class);
            nfsu2::drift::DriftManager::new(class, 1.0)
        });
        self.sim.roam_style = None;
        self.sim.roam_camera = None;
        self.sim.zone = (None, None, 0);
        let mut race =
            Race::new(route, laps, mode, true, style, drift, self.settings.jump_camera);
        // The test run's world build, the in-game audio load around it and
        // the player car's render info (as a race's, **inferred**: not
        // measured for a test run).
        let early = self.audio_load_early(mix_kind(mode), false);
        race.racers.cars.rng = self.world_build();
        self.audio_load_late(early, false);
        self.render_infos(1);
        race.order = vec![0];
        race.test = Some((number, mode));
        // Not a race Restart sets up again (`start_race_from`).
        self.race_flow.restart = None;
        race.racers.me.place_on_road(&race.route, at.truncate());
        race.reference = race.lap_reference();
        self.mode = Mode::Race(race);
        self.hud_state.hud = Hud::default();
        self.sim.replay = None;
        Ok(())
    }

    pub(crate) fn start_race(&mut self, index: usize, quick: bool, from_map: bool) -> Result<()> {
        self.drop_snapshot();
        self.start_race_from(index, quick, from_map, true)
    }

    /// [`Self::start_race`]; `entered` counts a career event started
    /// (0x541170: career +0x872C, "Races Entered:"; Restart does not pass
    /// there).
    pub(crate) fn start_race_from(&mut self, index: usize, quick: bool, from_map: bool, entered: bool) -> Result<()> {
        // The endings' files read while the race runs (`endings_ahead`).
        self.endings_ahead_start();
        self.race_flow.next_round = None;
        self.race_flow.map_start = from_map && entered;
        // 0x541170: career +0x872C ("Races Entered:") += 1, the event's
        // visit entry +6 = 2 unless won.
        self.race_flow.event = (!quick).then_some(index);
        if entered
            && !quick
            && let Some(p) = self.profile.as_mut()
        {
            p.races_entered += 1;
            if let Some(e) = self.game_data.career.events.get(index) {
                p.race_started(e);
            }
        }
        // (A world challenge's engage, 0x498FAC → 0x541130, clears the
        // GPS.) A race's start (0x541170 → 0x5411B0) clears the router
        // only: the follower drops its route and the GPS stays engaged
        // but reads off (state 4), so the World Map offers Activate GPS.
        if let Some(g) = self.career_world.gps.as_mut() {
            g.drop_route();
        }
        // A career race engaged in the city (the race dialog 0x499B27, the
        // World Map 0x4E1284 → 0x541170): career +0x8744 = 0 and the car's
        // place kept at +0x8750 / +0x8760, where "Return to Explore Mode"
        // brings it back (0x513950). Restart and Next Race do not pass
        // there (here() is None in the race's region).
        if !quick
            && let Some(at) = self.here()
            && let Some(p) = self.profile.as_mut()
        {
            p.explore_set = false;
            p.explore_pos = Some(at);
        }
        // Restart (0x5F41F0(9), not `entered`) runs the event's set-up
        // without its line-up, looks and grid (0x581830 → 0x5322F0(event,
        // 0); a Quick Race's none) on the event's current race (its record
        // 0x85EF48, which only Next Race moves on, 0x4BEC90 → 0x54CAF0):
        // the round the race was and the field it was set up with.
        let (round, field) = match self.race_flow.restart.take() {
            Some(r) if !entered && r.event == index && r.quick == quick => (r.round, r.field),
            _ => (0, Field::default()),
        };
        self.start_round(index, quick, from_map, round, field)
    }

    /// Race `round` of an event's races.
    pub(crate) fn start_round(
        &mut self,
        index: usize,
        quick: bool,
        from_map: bool,
        round: usize,
        field: Field,
    ) -> Result<()> {
        // A race builds the player's car anew: no marker trigger (+0x9AC).
        (self.career_world.marker_trigger, self.career_world.marker) = (None, None);
        dev_hooks::rng_pin(self, "race");
        // A Quick Race's opponents draw from the pool for the player's car
        // (0x53EEA0: 0x4FEAF0 with the player's car record, its SUV flag).
        let quick_race = quick && Some(index) == self.quick.event;
        if quick_race {
            let suv = self
                .quick.car
                .as_ref()
                .and_then(|c| self.game_data.car_types.iter().find(|t| t.name.eq_ignore_ascii_case(c)))
                .is_some_and(|t| t.suv);
            self.game_data.career.events[index].suv = suv;
        }
        let event = self.game_data.career.events[index].clone();
        // The set-up's 0x89E7F4 and 0x89E804 (Race_SetupFromEvent 0x5322F0:
        // the career difficulty and the event's +0x7D; the Quick Race
        // set-ups: the record's difficulty, and drag's 0x53F6A0 also the
        // throttle level).
        match self.quick.mode.filter(|_| quick_race) {
            Some(m) => self.sim.race_globals.quick_race(
                self.settings.qr[nfsu2::frontend::quick_race::race_type(m)].difficulty,
                m == screens::QrMode::Drag,
            ),
            None => self.sim.race_globals.career_race(self.settings.difficulty, event.throttle),
        }
        self.car_difficulty();
        let (track_no, variant, laps) =
            event.races.get(round).copied().unwrap_or((event.track, event.variant, event.laps));
        let track = self.game_data.career.tracks.get(&track_no).context("event track")?.clone();
        // The race's load (World_Start 0x60A690 → 0x541A60, a race type
        // set and no dyno run): profile +0xA04 and the kind's count (seen
        // under Wine at a sponsor circuit's load, 8 → 9 and 1 → 2). Each
        // prov: inferred game.start_round
        // round loads its track; **inferred** for Restart (the clone
        // reloads the race there too).
        if let Some(p) = self.profile.as_mut() {
            p.totals.race_loaded(career::record_kind(track.flags));
        }
        // The set-up's camera setting (Super Far in a drift race, else not).
        self.settings.race_camera(event.race_mode().drift());
        let t_grid = nfsu2::prof::span("start_round: drive_to_grid");
        let (route, at, _) =
            self.drive_to_grid(&track, variant, event.race_mode(), tuning_mode_for(event.kind), !quick)?;
        drop(t_grid);
        let laps = if route.loops { laps.max(1) as u32 } else { 1 };
        let laps = self.sim.map_race.as_ref().filter(|_| route.loops).map_or(laps, |r| r.laps);
        // A drift race scores the player's drifts (0x5F7A10: the style
        // manager for the drift mask, the drift manager on the car's class).
        let mode = event.race_mode();
        // The human's drivetrain +0xE4 (0x5B6130): the difficulty's shift
        // window scale in a drag race outside the career, else 1.0.
        let shift_scale = nfsu2::handling::drag_shift_scale(
            mode == nfsu2::race::RaceMode::Drag,
            self.race_flow.game_mode == 1,
            self.difficulty(),
        );
        if let Some(d) = self.sim.drive.as_mut() {
            d.car.car_type.handling.shift_window_scale = shift_scale;
        }
        self.car_difficulty();
        // Every local human racer has a style manager (0x5FC3A6), a drift
        // race also the drift manager (0x5F7A10).
        let style =
            Some(nfsu2::drift::StyleManager::new(self.game_data.tricks.clone(), nfsu2::drift::mode_mask(mode, !route.loops)));
        let drift = mode.drift().then(|| {
            let class = self.sim.drive.as_ref().map_or(3, |d| d.car.car_type.handling.performance_class);
            nfsu2::drift::DriftManager::new(class, 1.0)
        });
        self.sim.roam_style = None;
        self.sim.roam_camera = None;
        self.sim.zone = (None, None, 0);
        let mut race =
            Race::new(route, laps, mode, quick, style, drift, self.settings.jump_camera);
        race.event = index;
        race.reversed = variant & 1 != 0;
        race.from_map = from_map;
        race.round = round;
        race.field = field;
        race.start_gap = track.start_gap[usize::from(variant & 1 != 0)];
        race.racers.me.place_on_road(&race.route, at.truncate());
        // The opponents (0x5260C0) and the grid order (0x5264B0).
        let t_routes = nfsu2::prof::span("start_round: AiRoutes::load");
        race.racers.ai_routes = nfsu2::ai::route::AiRoutes::load(&self.game_data.root, &track, variant & 1 != 0)
            .ok()
            .map(|r| nfsu2::ai::route::AiRoutes { race_coords: true, ..r });
        drop(t_routes);
        // The grid order (0x5264B0's argument): a Quick Race's set-up's
        // (0x53FC30's switch: Circuit / Sprint 0x53F160 3 — 2 only with
        // the street X flag, which they leave clear —, Street X 0x53F380
        // 2, Drag 0x53F6A0 3, Drift 0x53F840 2, URL 0x53FA50 2), else the
        // career's by the event's type (0x53250F).
        let order = match self.quick.mode.filter(|_| quick_race) {
            Some(screens::QrMode::Circuit | screens::QrMode::Sprint | screens::QrMode::Drag) => {
                nfsu2::opponents::GridOrder::Shuffled
            }
            Some(screens::QrMode::FreeRun) => nfsu2::opponents::GridOrder::HumansFirst,
            Some(_) => nfsu2::opponents::GridOrder::AiFirst,
            None => nfsu2::opponents::GridOrder::career(event.kind),
        };
        let t_rivals = nfsu2::prof::span("start_round: line_up_rivals");
        self.line_up_rivals(&event, &mut race, mode, order)?;
        drop(t_rivals);
        // The log's waits: the rivals' cars, a part of the race's load
        // (docs/developing/waits-log.md).
        let ((taken, asked), kept) = (self.loads.rivals_taken, self.loads.rivals_kept);
        if let Some(l) = self.loads.waits.load_mut().filter(|_| asked > 0) {
            l.notes.push(match kept {
                0 => format!("rivals {taken} of {asked} loaded ahead"),
                _ => format!("rivals {taken} of {asked} loaded ahead, {kept} kept from the race before"),
            });
        }
        self.loads.waits.load_part("rivals");
        // The player's own starting position from here on.
        let at = race.route.grid.get(race.grid_slot).map_or(at, |g| g.0);
        if mode.drift() {
            race.scoring.board = self.drift_board(&race, track_no, &event);
        }
        // The load after the set-up (0x526980 → World_Start 0x60A690): the
        // world build's draws (`Game::world_build`), its generator the race
        // field's (the clone once drew the seed before the line-up). With
        // the sound, the in-game audio load comes first when the mix map's
        // kind changes, else after the rain set-up (`audio_load_early`).
        let early = self.audio_load_early(mix_kind(mode), true);
        dev_hooks::rng_pin(self, "race_world");
        race.racers.cars.rng = self.world_build();
        self.audio_load_late(early, true);
        // Traffic (0x4FE9F0 from the set-up, 0x42F910 each frame) at the
        // race's level: a career race its event's byte +0x36 (0x5322F0),
        // a Quick Race its mode record's (0x526730 / 0x526840). The
        // placer works on the race's own route file, whose records carry
        // the same traffic lanes as free roam's.
        let level =
            if quick && !quick_race { nfsu2::ai::traffic::QUICK_RACE_LEVEL } else { event.traffic.max(0) as usize };
        // The mode record's catch-up (0x53F160: 0x89E7F8 = +0x05) and, in
        // a circuit with Lap KO, the knockout race type (0x89E7B0 = 3).
        if quick_race && let Some(m) = self.quick.mode {
            let ty = nfsu2::frontend::quick_race::race_type(m);
            let r = self.settings.qr[ty];
            self.race_flow.catch_up_globals = (r.catch_up, Some(usize::from(r.difficulty)));
            race.scoring.knockout = ty == 0 && r.lap_ko;
        }
        // The catch-up (0x4172D0 / 0x4084A0, gated by 0x89E7F8) with the
        // traffic curve choice 0x89E7A8 ∈ {2, 3} (a career race: the
        // event's +0x36, 0x53236C; a Quick Race: its mode record's level).
        // 0x4172D0 clears its flag in a drag (0x89E7D8 / 0x890108) or a
        // drift race (0x89E7D9 / 0x890104), 0x417304..0x41732A: no
        // multiplier there (**verified**).
        if !quick || quick_race {
            let (on, difficulty) = self.race_flow.catch_up_globals;
            race.catch_up = (on && !mode.drag() && !mode.drift()).then_some(matches!(level, 2 | 3));
            race.catch_up_difficulty = difficulty;
        }
        if level > 0 {
            let files = (
                format!("Routes{}{}.bin", track.number, if race.reversed { "B" } else { "F" }),
                format!("Paths{}.bin", track.number),
            );
            // No seed of the traffic's own: its models draw on the
            // shared generator (0x412C9A), its placements on the world's.
            let _t = nfsu2::prof::span("start_round: Traffic::load");
            race.racers.traffic = Traffic::load(
                &self.game_data.root,
                &track.region,
                (&files.0, &files.1),
                &self.game_data.car_types,
                Some(&track),
                self.sim.ai.as_ref().map(|a| &a.tuning),
                self.sim.ai.as_ref().map(|a| &a.bounds),
                at,
                level.min(3),
                race.reversed,
                0,
            )
            .map_err(|e| eprintln!("race traffic: {e:#}"))
            .ok();
        }
        self.race_world_cars(&mut race);
        // The racing lines' speeds for the field (0x5E9E90): the field's
        // class ([`nfsu2::ai::route::field_class`]), an AI SUV, the track's
        // speed mode.
        if let (Some(routes), Some(curves)) =
            (race.racers.ai_routes.as_mut(), self.sim.ai.as_ref().and_then(|a| a.curves.as_ref()))
        {
            let class = nfsu2::ai::route::field_class(
                !race.quick,
                race.racers.rivals.iter().map(|r| r.car.car_type.handling.performance_class).min(),
            );
            let suv = race.racers.rivals.iter().any(|r| r.car.car_type.suv);
            // The race road is built (0x88F2BE): its lap (0x88F2B0) and
            // loop flag (0x88F2BC) feed the junction-type adjustment on a
            // track without +0x94 bit 1 (0x5E9328).
            let lap = (track.flags & 2 == 0)
                .then_some(nfsu2::ai::route::RaceLap { length: race.route.road.length, wrap: race.route.loops });
            let _t = nfsu2::prof::span("start_round: racing_speeds");
            routes.racing_speeds(curves, class, suv, nfsu2::ai::route::speed_mode(track.flags, track.number), lap);
            // Bench aid: NFSU2_BENCH_ROUTEDUMP=file writes the processed
            // network (each record's points' lane speeds, each junction's
            // flags) for comparisons with the original's memory.
            if let Some(path) = debug_env().bench_routedump.clone() {
                let mut out = format!("class {class} suv {suv}\n");
                for (i, r) in routes.file.records.iter().enumerate() {
                    for (k, c) in r.points.iter().enumerate() {
                        out +=
                            &format!("p {i} {} {k} {:?} {:.1} {:.1}\n", r.id, c.lane_speed, c.position.x, c.position.y);
                    }
                }
                for (k, j) in routes.file.junctions.iter().enumerate() {
                    out += &format!("j {k} {:08x}\n", j.flags);
                }
                let _ = std::fs::write(path, out);
            }
        }
        // The race's traffic reads the same route manager (0x88A320): the
        // network as the race load left it, which in a race is the file
        // itself — 0x5E9E90 prunes only with g_RouteCoordRace clear or
        // 0x88A330 = 1, and 0x88A330 is never set (its one writer, the
        // constructor 0x5E3FA0, clears it; observed 0 in a race, the
        // junction table's live count equal to its length) — plus the
        // racing speeds and junction types above.
        if let (Some(t), Some(routes)) = (race.racers.traffic.as_mut(), race.racers.ai_routes.as_ref()) {
            t.routes = routes.file.clone();
        }
        race.order = (0..=race.racers.rivals.len()).collect();
        race.reference = race.lap_reference();
        let t_scene = nfsu2::prof::span("start_round: race_start_scene");
        self.scenes.shop = self.race_start_scene(&track, &mut race);
        drop(t_scene);
        let _t_warm = nfsu2::prof::span("start_round: shadow textures");
        race.scene_start = self.scenes.shop.is_some();
        if let Some(run) = self.scenes.shop.as_ref() {
            let [green, cut] = run.scene.marks.map(|m| m as f32);
            race.timing.countdown_step = (green - cut) / nfsu2::shop_scene::FPS / 3.0;
            race.scene_end = (run.scene.length - green) / nfsu2::shop_scene::FPS;
            race.timing.origin = -green / nfsu2::shop_scene::FPS;
        }
        if let (Some(w), Some(r)) = (self.platform.window.as_ref(), self.sim.drive.as_ref().and(self.render.world.as_mut())) {
            let cars = race
                .racers.rivals
                .iter()
                .map(|rv| &rv.car)
                .chain(race.racers.traffic.iter().flat_map(|t| t.cars.iter().map(|c| &*c.car)));
            warm_shadow_textures(&w.gpu, r, cars, &nfsu2::render::RenderOptions::in_force());
        }
        // What a Restart races again (`start_race_from`).
        self.race_flow.restart = Some(crate::flow::RestartRace { event: index, quick, round, field: race.field.clone() });
        self.mode = Mode::Race(race);
        self.hud_state.hud = Hud::default();
        self.race_flow.last_race = Some((index, from_map, quick));
        self.sim.replay = None;
        Ok(())
    }

    /// A race load's world cars after the world build (**measured**, the
    /// sequence log, docs/original/rng.md): the render infos of the player and the
    /// rivals, the set-up's traffic fill (0x6065C0 → 0x42F7A0, on the
    /// world generator) with a render info for each car placed,
    /// CARSFX_Stereo's rolls, the traffic's models (Traffic_AssignModels
    /// 0x412C9A), SfxCtlEngine's draws and the placed cars' render infos
    /// again; the cars the fill could not place are dropped.
    pub(crate) fn race_world_cars(&mut self, race: &mut Race) {
        self.render_infos(1 + race.racers.rivals.len());
        let drag = race.drag;
        let plan = race.racers.traffic.as_ref().map(|t| {
            let racers: Vec<(Vec2, Vec2)> =
                race.racers.rivals.iter().map(|r| (r.state.position.truncate(), r.state.velocity.truncate())).collect();
            let place = crate::sim::traffic::place::DragPlace {
                coord: race.racers.me.along,
                lane: None,
                clock: race.timing.clock.min(-1e-3),
                start_line: race.route.start_at,
                start_gap: race.start_gap,
            };
            let me = self.sim.drive.as_ref().map_or(Vec2::ZERO, |d| d.state.position.truncate());
            crate::sim::traffic::place::traffic_fill_plan(
                t,
                t.pool_len(),
                me,
                &racers,
                Some((&race.route.road, place)),
                drag,
                &mut race.racers.cars.rng,
            )
        });
        let placed = plan.as_ref().map_or(0, |p| p.iter().flatten().count());
        self.render_infos(placed);
        self.stereo_rolls();
        if let Some(t) = race.racers.traffic.as_mut()
            && let Err(e) = t.populate(&self.game_data.root, &self.game_data.car_types, &mut self.rng)
        {
            eprintln!("race traffic: {e:#}");
            race.racers.traffic = None;
        }
        self.traffic_models_draws(placed);
        if let (Some(t), Some(plan), Some(d)) = (race.racers.traffic.as_mut(), plan, self.sim.drive.as_ref()) {
            crate::sim::traffic::place::traffic_fill_apply(&d.world, t, plan, true);
        }
        self.placed_traffic_renderers(race.racers.traffic.iter().flat_map(|t| t.cars.iter().map(|c| (&c.id, &*c.car))));
    }

    /// Put the event's opponents on the grid (0x5260C0): the line-up
    /// (skills and cars, [`nfsu2::opponents::line_up`]) kept over a
    /// tournament's races, each car upgraded to its skill (0x5B6AE0,
    /// sources 0 and 3), on the grid slots `order` gives (0x5264B0; marker
    /// +0x0C = slot − 1; the player moves there too), with its AI route,
    /// target speed and driver.
    /// The race difficulty 0x89E7F4.
    pub(crate) fn difficulty(&self) -> usize {
        self.sim.race_globals.difficulty
    }

    /// Hands the difficulty to the human's car: the drift assist (0x5AC317)
    /// reads the global for every car; the clone keeps it on the handling.
    pub(crate) fn car_difficulty(&mut self) {
        let d = self.difficulty();
        if let Some(drive) = self.sim.drive.as_mut() {
            drive.car.car_type.handling.difficulty = d;
        }
    }

    /// A drift race's standings at the start (0x56D860, from the first
    /// 0x5F7A10 once the list is empty; verified): a node for the human
    /// (flag set) and, off the downhill tracks (0x104E–0x1053), one per AI
    /// racer (score 0, flag set). On a downhill track (offline, one human)
    /// no AI racer is listed; instead three virtual rivals: names from
    /// 0x4FE970 (4 drawn, 3 used), a RANDOM car each (0x4FEAF0 with racer
    /// numbers 100–102 and the human's car record), the fixed scores of
    /// [`nfsu2::drift::virtual_scores`] at the human's class and the
    /// difficulty, flag clear.
    pub(crate) fn drift_board(
        &mut self,
        race: &Race,
        track: u16,
        _event: &career::Event,
    ) -> Vec<nfsu2::drift::BoardEntry> {
        use nfsu2::drift::BoardEntry;
        let name = self.profile.as_ref().map_or_else(|| "Player".to_string(), |p| p.name.clone());
        let car = self.sim.drive.as_ref().map_or_else(String::new, |d| d.car.car_type.name.clone());
        let mut board = vec![BoardEntry { racer: Some(0), name, car, score: 0.0, flag: true }];
        if !nfsu2::drift::downhill(track) {
            board.extend(race.racers.rivals.iter().enumerate().map(|(i, r)| BoardEntry {
                racer: Some(i + 1),
                name: r.name.clone(),
                car: r.car.car_type.name.clone(),
                score: 0.0,
                flag: true,
            }));
            return board;
        }
        let difficulty = self.difficulty();
        let Some(d) = self.sim.drive.as_ref() else { return board };
        let class = d.car.car_type.handling.performance_class;
        let Some(scores) = nfsu2::drift::virtual_scores(track, class, difficulty) else { return board };
        let player_car =
            self.game_data.car_types.iter().position(|t| t.name.eq_ignore_ascii_case(&d.car.car_type.name)).unwrap_or(0);
        let rating =
            |c: usize| self.sim.ai.as_ref().map_or(0.0, |a| a.bounds.scale(self.game_data.car_types[c].handling.performance_rating));
        let player_rating = rating(player_car);
        let names = nfsu2::frontend::quick_race::draw_names(&mut self.rng, 4);
        for (score, name) in scores.into_iter().zip(names) {
            let pool = nfsu2::opponents::car_pool(d.car.car_type.suv);
            let car = nfsu2::opponents::pick_car(&mut self.rng, pool, player_car, true, player_rating, &|c| {
                if c < self.game_data.car_types.len() { rating(c) } else { 0.0 }
            });
            let car = car.and_then(|c| self.game_data.car_types.get(c as usize)).map_or_else(String::new, |t| t.name.clone());
            board.push(BoardEntry { racer: None, name, car, score, flag: false });
        }
        // The list is sorted each frame (0x56DDE0); the virtual rivals'
        // scores lead from the start.
        nfsu2::drift::sort_board(&mut board);
        board
    }

    /// The player's car as the line-up reads it: its car type's index and
    /// its handling's performance rating.
    pub(crate) fn lineup_player(&self, car: &Car) -> (usize, f32) {
        let index =
            self.game_data.car_types.iter().position(|t| t.name.eq_ignore_ascii_case(&car.car_type.name)).unwrap_or(0);
        (index, car.car_type.handling.performance_rating)
    }

    /// The event's line-up (0x5260C0's first part,
    /// [`nfsu2::opponents::line_up`]) for the player's car (`lineup_player`),
    /// drawn from `rng`.
    pub(crate) fn new_lineup(
        &self,
        event: &career::Event,
        ai: &AiData,
        (player_car, rating): (usize, f32),
        rng: &mut nfsu2::opponents::GameRng,
    ) -> Vec<nfsu2::opponents::Entrant> {
        let ratings: Vec<f32> = self.game_data.car_types.iter().map(|t| ai.bounds.scale(t.handling.performance_rating)).collect();
        let player_rating = ai.bounds.scale(rating);
        let stage = self.profile.as_ref().and_then(|p| {
            let def = self.game_data.career.stages.iter().find(|s| s.stage == p.stage)?;
            Some(nfsu2::opponents::StageSkill {
                events: def.skill_wins,
                low: def.skill.0,
                high: def.skill.1,
                done: p.stage_win_count(&self.game_data.career),
            })
        });
        // A named car is a preset (0x637280 by its name's hash), of
        // the preset's car type.
        let preset_car = |h: u32| {
            let p = self.game_data.presets.iter().find(|p| nfsu2::formats::hash::bin_hash(&p.name) == h)?;
            self.game_data.car_types.iter().position(|t| t.name.eq_ignore_ascii_case(&p.car))
        };
        nfsu2::opponents::line_up(event, rng, stage, player_car, player_rating, &ratings, &preset_car)
    }

    /// The event's line-up drawn and dressed as the race set-up 0x5260C0
    /// does it, racer by racer: each entrant's car pick (0x4FEC15, or a
    /// preset's car) and then its looks (0x6399E0(1, 1) → 0x6374C0 and
    /// 0x632ED0), before the next entrant's pick (**measured**, the
    /// sequence log: a circuit's, a sprint's and a drag's set-up). Only the
    /// first `limit` entrants are dressed (the grid's room). A Quick Race
    /// (`quick`) picks every racer first and then dresses them (0x53EEA0,
    /// then 0x532020 for every AI racer, as free roam's encounter set-up,
    /// which the log shows picking all before dressing; **inferred** for
    /// Quick Race, not logged).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lineup_dressed(
        &self,
        event: &career::Event,
        ai: &AiData,
        player: (usize, f32),
        quick: bool,
        drag: bool,
        limit: usize,
        rng: &mut nfsu2::opponents::GameRng,
    ) -> (Vec<nfsu2::opponents::Entrant>, Vec<RivalKey>) {
        let (mut lineup, mut fits) = (Vec::new(), Vec::new());
        let bench = debug_env().bench_rival_cars.clone();
        for o in &event.opponents {
            let mut one = event.clone();
            one.opponents = vec![o.clone()];
            for mut e in self.new_lineup(&one, ai, player, rng) {
                // Bench aid (not native): NFSU2_BENCH_RIVAL_CARS=NAME,NAME
                // forces the rivals' car types (for side-by-side runs with
                // the original).
                if let Some(name) = bench.as_deref().and_then(|v| v.split(',').nth(lineup.len()))
                    && let Some(i) =
                        name.trim().parse::<usize>().ok().filter(|&i| i < self.game_data.car_types.len()).or_else(
                            || self.game_data.car_types.iter().position(|t| t.name.eq_ignore_ascii_case(name.trim())),
                        )
                {
                    e.car = i;
                }
                if !quick && lineup.len() < limit {
                    fits.push(self.rival_fit(&e, quick, drag, ai, player.0, rng));
                }
                lineup.push(e);
            }
        }
        if quick {
            let n = lineup.len().min(limit);
            fits = self.rival_fits(&lineup[..n], quick, drag, ai, player.0, rng);
        }
        (lineup, fits)
    }

    /// The entries' cars as `line_up_rivals` loads them: their looks and
    /// performance (the dressing draws from `rng`, in entry order).
    pub(crate) fn rival_fits(
        &self,
        lineup: &[nfsu2::opponents::Entrant],
        quick: bool,
        drag: bool,
        ai: &AiData,
        player_car: usize,
        rng: &mut nfsu2::opponents::GameRng,
    ) -> Vec<RivalKey> {
        lineup.iter().map(|e| self.rival_fit(e, quick, drag, ai, player_car, rng)).collect()
    }

    /// One entrant's car: its performance and its looks (a preset's parts,
    /// else dressed by that performance, drawing from `rng`).
    pub(crate) fn rival_fit(
        &self,
        e: &nfsu2::opponents::Entrant,
        quick: bool,
        drag: bool,
        ai: &AiData,
        player_car: usize,
        rng: &mut nfsu2::opponents::GameRng,
    ) -> RivalKey {
        let human = self.human_perf(ai);
        let player_perf = self.player_car().2;
        let player_nitrous =
            nfsu2::performance::state_float(&self.game_data.career.packages, &|id| player_perf.contains(&id), 2)
                * 0.333_333_34;
        let preset =
            e.preset.and_then(|h| self.game_data.presets.iter().find(|p| nfsu2::formats::hash::bin_hash(&p.name) == h));
        let record = &self.game_data.car_records[e.car].1;
        let packages = &self.game_data.career.packages;
        // A preset's own performance (+0x48 / +0x2F4) first;
        // **inferred** that the skill source's fit follows it.
        let mut fitted = preset.map(|p| p.perf(packages)).unwrap_or_default();
        match (quick, e.source, human.as_ref()) {
            (true, _, Some(h)) | (false, 1, Some(h)) => {
                fitted = nfsu2::ai::route::match_to_human(&ai.presets, &ai.bounds, e.car, h, record, packages)
            }
            (false, 0 | 3, _) => {
                fitted = nfsu2::ai::route::fit_performance(
                    &ai.presets,
                    &ai.bounds,
                    e.car,
                    player_car,
                    e.skill,
                    record,
                    packages,
                    drag,
                );
                nfsu2::performance::refit(&mut fitted, packages, player_nitrous, &[2]);
                nfsu2::performance::ai_refit(&mut fitted, packages);
            }
            _ => nfsu2::performance::ai_refit(&mut fitted, packages),
        }
        let ty = &self.game_data.car_types[e.car];
        let installed = match preset {
            Some(p) => p.installed(),
            None => {
                let mut ride = nfsu2::ai_looks::Ride::stock(&self.game_data.parts, ty);
                ride.dress(rng, nfsu2::performance::state_levels(packages, &|id| fitted.contains(&id)));
                ride.installed
            }
        };
        (ty.name.clone(), installed, fitted)
    }

    pub(crate) fn predict_encounters(&self, car: &Car) -> Option<Vec<RivalKey>> {
        if self.race_flow.quick_free_run || self.profile.is_none() {
            return None;
        }
        let ai = self.sim.ai.as_ref()?;
        let mut rng = nfsu2::opponents::GameRng(self.rng.0);
        // The city start copies the career difficulty (0x5404B0 / 0x540790).
        let difficulty = usize::from(self.settings.difficulty.min(2));
        let human = self.human_perf(ai);
        Some(
            Encounters::picks(
                &self.game_data.car_types,
                &self.game_data.parts,
                (&self.game_data.career.packages, &self.game_data.car_records, human),
                ai,
                car,
                difficulty,
                &mut rng,
            )
            .1,
        )
    }

    /// The rivals' cars of career event `index` as its start will load
    /// them (an event of several races: its first, where the line-up is
    /// drawn), predicted on a copy of the random numbers as they stand:
    /// nothing draws on the shared generator between the dialog's Engage
    /// and the line-up (`start_round`: the line-up and the looks come
    /// before the world build, whose seed draw is after them), or None for
    /// an event not lined up this way.
    pub(crate) fn predict_rivals(&self, index: usize) -> Option<Vec<RivalKey>> {
        let (ai, d) = (self.sim.ai.as_ref()?, self.sim.drive.as_ref()?);
        let event = self.game_data.career.events.get(index)?;
        if self.quick.event == Some(index) || self.race_flow.game_mode == 8 {
            return None;
        }
        let mut rng = nfsu2::opponents::GameRng(self.rng.0);
        let player = self.lineup_player(&d.car);
        Some(self.lineup_dressed(event, ai, player, false, event.race_mode().drag(), usize::MAX, &mut rng).1)
    }

    /// The rivals' cars of the Quick Race set up in event slot `index`
    /// (`quick_event`, the car picked: `QuickRaceState::car`) as its start
    /// will load them, predicted on a copy of the random numbers as they
    /// stand, as [`Self::predict_rivals`]: the line-up and the looks are
    /// drawn after the track's load (`start_round`), from the event with
    /// the car's SUV pool and the player's car as `drive_in` makes it (its
    /// type, and the rating its race set-up gives it, `Car::set_race`);
    /// nothing draws on the shared generator between the car's pick and
    /// the line-up.
    pub(crate) fn predict_quick_rivals(&self, index: usize) -> Option<Vec<RivalKey>> {
        let ai = self.sim.ai.as_ref()?;
        let name = self.quick.car.as_ref().filter(|_| self.quick.event == Some(index))?;
        let ty = self.game_data.car_types.iter().find(|t| t.name.eq_ignore_ascii_case(name))?;
        let mut event = self.game_data.career.events.get(index)?.clone();
        event.suv = ty.suv;
        let mode = event.race_mode();
        let (car, _, perf, settings) = self.car_key(tuning_mode_for(event.kind));
        let g = nfsu2::car::shared_file(&nfsu2::game_fs::path(&self.game_data.root, "GLOBAL/GLOBALB.BUN")).ok()?;
        let tune = Some((&self.game_data.career.tuning[..], &settings));
        let car_type = nfsu2::car::parse_car_type_tuned(&g, &car, &self.game_data.career.packages, &|id| perf.contains(&id), tune).ok()?;
        let mut handling = car_type.stock.clone();
        nfsu2::handling::setup_performance(&mut handling, mode);
        let index = self.game_data.car_types.iter().position(|t| t.name.eq_ignore_ascii_case(&car)).unwrap_or(0);
        let player = (index, handling.performance_rating);
        let mut rng = nfsu2::opponents::GameRng(self.rng.0);
        Some(self.lineup_dressed(&event, ai, player, true, mode.drag(), usize::MAX, &mut rng).1)
    }

    /// A race over (paused for a Restart, or ended): its rivals' cars, each
    /// with the key of the load that made it (`Field::looks`, entry order),
    /// for the next set-up (`line_up_rivals`), which takes a car only for
    /// the same key and loads any other as before (a Restart or a Next Race
    /// races the same field again).
    pub(crate) fn keep_raced_rivals(&mut self, race: Race) {
        if race.test.is_some() || race.racers.rivals.len() != race.field.looks.len() {
            return;
        }
        let keys = race.field.looks.clone();
        self.loads.raced_cars = keys.into_iter().zip(race.racers.rivals.into_iter().map(|r| r.car)).collect();
    }

    pub(crate) fn line_up_rivals(
        &mut self,
        event: &career::Event,
        race: &mut Race,
        mode: RaceMode,
        order: nfsu2::opponents::GridOrder,
    ) -> Result<()> {
        let (Some(ai), Some(d)) = (self.sim.ai.as_ref(), self.sim.drive.as_ref()) else { return Ok(()) };
        let Some(routes) = race.racers.ai_routes.as_ref() else { return Ok(()) };
        let player = self.lineup_player(&d.car);
        let player_car = player.0;
        let difficulty = self.difficulty();
        let throttle = self.sim.race_globals.throttle_level;
        // A new field: each entrant picked and dressed in turn
        // (`lineup_dressed`); a field kept from a race before is dressed
        // below when its looks are not kept.
        let quick = self.quick.event == Some(race.event);
        let mut dressed = None;
        if race.field.lineup.is_empty() {
            let mut rng = nfsu2::opponents::GameRng(self.rng.0);
            let limit = race.route.grid.len().saturating_sub(1);
            let (lineup, fits) = self.lineup_dressed(event, ai, player, quick, race.drag, limit, &mut rng);
            race.field.lineup = lineup;
            dressed = Some(fits);
            self.rng = rng;
        }
        // The cars load side by side (as the traffic's do); the rest goes
        // in grid order, so the random draws keep their order.
        let entries = race.field.lineup.len().min(race.route.grid.len().saturating_sub(1));
        // Their looks (`nfsu2::ai_looks`): a preset's parts (0x637280), or a
        // RANDOM car dressed by its performance (0x5260C0 → 0x6399E0(1, 1)
        // → 0x6374C0; Quick Race's 0x53EEA0 → 0x532020 the same), in
        // entry order from the shared random numbers.
        // The performance: Quick Race matches every racer to the player
        // (0x532020 → 0x5B7580); a career entry by its skill source
        // (0x5260C0): 1 the same match, 0 and 3 the skill's preset
        // (0x5B6AE0) with nitrous refitted to the player's (0x5A6890(the
        // player's nitrous level / 3, {2})), 2 none; each then
        // 0x6399E0(1, ·)'s category-9 refit.
        // A field that has raced (Restart, Next Race) keeps its looks and
        // grid slots (`Field::slots`): no draw.
        let kept = race.field.looks.len() == entries
            && race.field.slots.len() == entries + 1
            && race.field.slots.iter().all(|&s| s < race.route.grid.len());
        let (fits, slots) = if kept {
            (race.field.looks.clone(), race.field.slots.clone())
        } else {
            let fits = match dressed {
                Some(fits) => fits,
                None => {
                    let mut rng = nfsu2::opponents::GameRng(self.rng.0);
                    let fits =
                        self.rival_fits(&race.field.lineup[..entries], quick, race.drag, ai, player_car, &mut rng);
                    self.rng = rng;
                    fits
                }
            };
            // The grid slots (0x5264B0, after the line-up's and looks'
            // draws): the player moves off slot 0 when the order puts it
            // elsewhere.
            let mut slots = nfsu2::opponents::grid_slots(order, entries, &mut self.rng);
            if let Some(at) = dev_hooks::parity_grid(self)
                && let Some(want) = (0..race.route.grid.len().min(slots.len())).min_by(|&a, &b| {
                    race.route.grid[a].0.truncate().distance(at).total_cmp(&race.route.grid[b].0.truncate().distance(at))
                })
                && let Some(k) = slots.iter().position(|&x| x == want)
            {
                // The racer that drew the original's player slot gives it up.
                slots.swap(0, k);
            }
            race.field.looks = fits.clone();
            race.field.slots = slots.clone();
            (fits, slots)
        };
        race.grid_slot = slots[0];
        if slots[0] != 0
            && let Some(&(at, heading)) = race.route.grid.get(slots[0])
            && let Some(d) = self.sim.drive.as_mut()
        {
            let z = d.world.grid_height(at);
            d.place(Vec3::new(at.x, at.y, z), heading);
            d.state.held = true;
            race.racers.me = Progress::default();
            race.racers.me.place_on_road(&race.route, at.truncate());
        }
        let Some(d) = self.sim.drive.as_ref() else { return Ok(()) };
        let (root, packages) = (&self.game_data.root, &self.game_data.career.packages);
        // A load done ahead for the same arguments (`RivalsAhead`) is taken
        // as it is; any other loads here.
        let mut ahead = self.loads.rivals_ahead.take().filter(|a| a.event == race.event).map(|a| a.loads).unwrap_or_default();
        let taken: Vec<_> =
            fits.iter().map(|key| ahead.iter().position(|(k, ..)| k == key).map(|i| ahead.swap_remove(i)).map(|(_, h, r)| (h, r))).collect();
        // The cars of the race raced again (`keep_raced_rivals`) for the
        // keys asked for again: the same load's result (the set-up below
        // makes the race's handling again from the car's stock record:
        // `set_race`, the difficulty, the drag shift set-up); the others go.
        let mut raced = std::mem::take(&mut self.loads.raced_cars);
        let kept: Vec<Option<Car>> = fits
            .iter()
            .zip(&taken)
            .map(|(key, t)| match t {
                Some(_) => None,
                None => raced.iter().position(|(k, _)| k == key).map(|i| raced.swap_remove(i).1),
            })
            .collect();
        drop(raced);
        self.loads.rivals_taken = (taken.iter().flatten().count(), fits.len());
        self.loads.rivals_kept = kept.iter().flatten().count();
        self.loads.race_rivals = fits.clone();
        self.loads.companion_ahead = None;
        if nfsu2::prof::enabled() {
            eprintln!(
                "[time] rivals loaded ahead: {} of {}; kept from the race raced again: {}",
                self.loads.rivals_taken.0,
                fits.len(),
                self.loads.rivals_kept
            );
        }
        // Check aid (not native): NFSU2_DUMP_RIVAL_RECORDS=DIR writes each
        // rival's upgraded car record (entry order) for the parity harness.
        if let Some(dir) = debug_env().dump_rival_records.clone()
            && let Ok(globalb) = nfsu2::game_fs::read(&root, "GLOBAL/GLOBALB.BUN")
        {
            for (i, (name, _, fitted)) in fits.iter().enumerate() {
                if let Ok(r) = nfsu2::car::tuned_record(&globalb, name, packages, &|id| fitted.contains(&id)) {
                    std::fs::write(std::path::Path::new(&dir).join(format!("rival{}-{name}.bin", i + 1)), r).ok();
                }
                let mut ids: Vec<u8> = fitted.iter().copied().collect();
                ids.sort_unstable();
                std::fs::write(
                    std::path::Path::new(&dir).join(format!("rival{}-{name}.fitted", i + 1)),
                    format!("{ids:02x?}\n"),
                )
                .ok();
            }
        }
        // Each car's renderer (`CarRenderer::new`, which the race's first
        // frame made) built on the car's thread once it is loaded, for that
        // frame to take (`render::cars_ahead`; it reads the car's parts,
        // solids, textures and light materials, which nothing below
        // changes); without a window or with the work ahead off, none.
        let draw = self
            .platform
            .window
            .as_ref()
            .filter(|_| crate::render::cars_ahead::with_loads())
            .map(|w| (w.gpu.clone(), w.config.format, nfsu2::render::RenderOptions::in_force()));
        let draw = draw.as_ref();
        // A renderer built with a load ahead is taken when built for what
        // this one would be built for.
        let built = move |car: Result<Car>, ahead: Option<crate::loads::preload::CarRendererAhead>| {
            let r = match (&car, draw) {
                (Ok(c), Some((gpu, format, opts))) => {
                    let key = crate::render::cars_ahead::BuildKey::now(gpu, *format, opts);
                    ahead
                        .and_then(|s| s.lock().unwrap().take())
                        .filter(|(k, _)| *k == key)
                        .or_else(|| crate::render::cars_ahead::build(gpu, c, *format, opts))
                }
                _ => None,
            };
            (car, r)
        };
        let cars: Vec<(Result<Car>, _)> = std::thread::scope(|s| {
            let loads: Vec<_> = fits
                .iter()
                .zip(taken)
                .zip(kept)
                .map(|(((name, installed, fitted), done), kept)| match (done, kept) {
                    (Some((h, ahead)), _) => s.spawn(move || {
                        let car = h.join().unwrap_or_else(|_| Err(anyhow::anyhow!("a rival's load ahead panicked")));
                        built(car, ahead)
                    }),
                    (None, Some(car)) => s.spawn(move || built(Ok(car), None)),
                    (None, None) => s.spawn(move || {
                        built(Car::load_upgraded(root, name, installed, packages, &|id| fitted.contains(&id)), None)
                    }),
                })
                .collect();
            loads.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let first = race.racers.rivals.len();
        let mut renderers = Vec::new();
        for ((i, e), (car, renderer)) in race.field.lineup.iter().enumerate().zip(cars) {
            renderers.push(renderer);
            let &(at, heading) = &race.route.grid[slots[i + 1]];
            let mut car = car?;
            car.set_race(mode);
            // The drift assist reads the race difficulty (0x5AC317).
            car.car_type.handling.difficulty = difficulty;
            // The racer set-up (0x5B6130) of an AI racer in a drag race:
            // the clutch factor outside the career and the skill row's
            // shift offset ranges (the row as below).
            let skill_row = event.opponents.get(i).map_or(0, |o| o.difficulty[difficulty] as usize);
            nfsu2::handling::drag_ai_shift_setup(
                &mut car.car_type.handling,
                self.race_flow.game_mode == 1,
                ai.tuning.shift_ranges(skill_row),
            );
            // The grid slot's ground (0x5EB5E0), then the racer set-up's
            // placement on the four wheels' ground (0x5F4A00).
            let road = d.world.grid_height(at);
            let mut state = CarState::placed(&car.car_type, &d.world, Vec3::new(at.x, at.y, road), heading);
            // An AI racer (owner kind 2) takes the race's barrier groups
            // too (0x604270 → 0x5F54B0).
            state.ground.barriers = d.world.racer_barriers(&d.barriers);
            // The race start (0x403C20) gives every AI racer player 1's
            // Stability Control byte (car +0x464).
            state.stability_off = self.settings.players[0].stability_off;
            // Each racer's shift offsets (0x594720) from its own sequence
            // (fixed per grid entry; no other random state is drawn).
            state.drivetrain = Some(nfsu2::handling::Drivetrain::seeded(
                &car.car_type.handling,
                0x1234_5678 ^ (i as u32 + 1).wrapping_mul(0x9E37_79B9),
            ));
            let mut progress = Progress::default();
            progress.place_on_road(&race.route, at.truncate());
            let h = &car.car_type.handling;
            // The driver tables' skill row: the entry's byte for the
            // difficulty (+0x08 + 0x89E7F4).
            let row = event.opponents.get(i).map_or(0, |o| o.difficulty[difficulty] as usize);
            // Racer number 100 + i (car +0x0A): its start hold (0x5EF000).
            let route =
                nfsu2::ai::route::RacerRoute::start_in(
                    routes,
                    state.position,
                    Vec2::new(heading.cos(), heading.sin()),
                    car.car_type.handling.race.drag(),
                )
                    .map(|mut r| {
                        r.branch_hold = nfsu2::ai::route::start_hold(100 + i as i32);
                        r
                    });
            let driver =
                nfsu2::ai::driver::AiDriver::new(h, car.car_type.suv, nfsu2::ai::driver::THROTTLE_CAP[throttle]);
            let accel: Box<dyn Fn(f32) -> f32> = Box::new(nfsu2::handling::acceleration(h));
            // No draw: the racers draw on the world generator; this
            // own generator is never read by a think.
            let rng = nfsu2::opponents::GameRng::default();
            // A drift race: the DriftDriver (0x4260B0) and the racer's own
            // style and drift managers (0x5F7A10; 0x56D7E0: the AI's rate
            // 0x79DAF0[0x89E7F4]).
            let drifting = mode.drift();
            let class = h.performance_class;
            let style = drifting.then(|| {
                nfsu2::drift::StyleManager::new(self.game_data.tricks.clone(), nfsu2::drift::mode_mask(mode, !race.route.loops))
            });
            let scorer =
                drifting.then(|| nfsu2::drift::DriftManager::new(class, nfsu2::drift::AI_DIFFICULTY[difficulty]));
            race.racers.rivals.push(Rival {
                name: e.name.clone(),
                car,
                id: CarId::new(),
                state,
                progress,
                block_row: event.opponents.get(i).map_or(1, |o| o.block.clamp(0, 2) as usize),
                route,
                speed: nfsu2::ai::route::SpeedTarget::new(row),
                driver,
                controls: Default::default(),
                rng,
                accel,
                unstuck: Default::default(),
                actions: Default::default(),
                lane: Default::default(),
                // Held from the race-state notices before the green
                // (0x429B70: vt+0x1C(state ≠ 4) on the driver, the
                // DriftDriver's +0x23C, 0x405B10: gas and brake 1, the
                // wheels straight, neutral), so a drift rival stands from
                // the scene's cut (state 3) to the green (state 4).
                drift: drifting.then(|| {
                    let mut d = nfsu2::ai::driver::DriftDriver::default();
                    d.hold = true;
                    d
                }),
                style,
                scorer,
                refill: (0.0, 0.0),
                hold: HOLD_TICKS,
                slot: slots[i + 1],
                belt: BeltState::default(),
                think: Default::default(),
                retained: None,
            });
        }
        for (rival, renderer) in race.racers.rivals[first..].iter().zip(renderers) {
            if let Some((key, r)) = renderer {
                self.render.cars_ahead.offer(&rival.id, key, r);
            }
        }
        Ok(())
    }
}

/// The mix map's kind (MixMap_Select 0x4869A0): drag 2, drift 1, else 0.
pub(crate) fn mix_kind(mode: RaceMode) -> u8 {
    if mode.drag() {
        2
    } else if mode.drift() {
        1
    } else {
        0
    }
}

impl Game {
    /// The desktop map editor's test run (`shell::cli::MapTest`): the
    /// loaded career save's car (its parts too), else the Quick Race car
    /// select's first car, and then the save let go, so nothing in the run
    /// writes it; then the custom race on its base event's set-up (as a
    /// race outside the career: no progress, no reward) or free roam at the
    /// spot. The mod was chosen at the start (`tools::map::set_only`).
    pub(crate) fn start_map_test(&mut self, t: &crate::shell::cli::MapTest) -> Result<()> {
        match self.profile.take() {
            Some(p) if !p.car.is_empty() => {
                let installed = nfsu2::car::with_derived_slots(&p.installed, &self.game_data.parts, &p.car);
                self.quick.parts = Some((installed, p.perf.clone()));
                self.quick.car = Some(p.car);
            }
            _ => {
                self.quick.parts = None;
                self.quick.car = Some(
                    self.qr_cars(self.quick.filter).first().map(|c| c.name.clone()).context("no Quick Race car")?,
                );
            }
        }
        log::info!("map test: car {}", self.quick.car.as_deref().unwrap_or("?"));
        if let Some(id) = &t.race {
            let race = nfsu2::tools::map::find_race(&t.folder, id)
                .with_context(|| format!("map mod {} has no race {id}", t.folder))?;
            let checked = nfsu2::tools::map::race::road_net(
                &self.game_data.root,
                self.game_data.career.tracks.get(&race.track).context("the race's base track")?,
            )
            .and_then(|net| nfsu2::tools::map::race::check(&net, &race).map_err(|(leg, why)| anyhow::anyhow!("leg {leg}: {why}")));
            checked.with_context(|| format!("race {id} cannot be raced"))?;
            let event = map_race_event(&self.game_data.career, &race)
                .with_context(|| format!("no career event of this mode on track {}", race.track))?;
            log::info!("map test: racing {id} on track {} (event {event})", race.track);
            self.sim.map_race = Some(race);
            let r = self.start_race(event, true, false);
            if r.is_err() {
                self.sim.map_race = None;
            }
            return r;
        }
        let (at, heading) = t.at.context("a race or a spot")?;
        self.explore(Some((at, heading.to_radians())))
    }
}

/// The career event a custom race borrows its set-up from: one on its
/// base track and direction of the same race mode (circuit and sprint
/// share theirs), an event of the same kind first.
pub(crate) fn map_race_event(career: &career::CareerData, race: &nfsu2::tools::map::CustomRace) -> Option<usize> {
    use nfsu2::tools::map::RaceKind;
    let want = match race.kind {
        RaceKind::Circuit | RaceKind::Sprint => RaceMode::FreeRoam,
        RaceKind::StreetX => RaceMode::StreetX,
        RaceKind::Drift => RaceMode::Drift,
    };
    // The race's first round is the one raced (`start_round`).
    let fits = |e: &&career::Event| {
        let (track, variant) = e.races.first().map_or((e.track, e.variant), |r| (r.0, r.1));
        track == race.track && (variant & 1 != 0) == race.backward && e.race_mode() == want
    };
    let same_kind = |e: &&career::Event| match race.kind {
        RaceKind::Circuit => e.kind == career::EventKind::Circuit,
        RaceKind::Sprint => e.kind == career::EventKind::Sprint,
        _ => true,
    };
    career.events.iter().filter(fits).find(same_kind).or_else(|| career.events.iter().find(fits)).map(|e| e.index)
}

#[cfg(test)]
mod map_test_tests {
    use crate::*;
    use nfsu2::tools::map::{CustomRace, MapMod, RaceKind, store};

    /// A mod in this test's data folder holding a short sprint over the
    /// first three checkpoints of a career sprint's track. The data folder
    /// is the test thread's sandbox (`test_isolation`, only with
    /// `dev-harness`): without it these tests would write into the
    /// player's maps folder, so they are ignored then.
    fn sprint_mod(game: &Game, folder: &str) -> (CustomRace, career::Route) {
        let event = game
            .game_data
            .career
            .events
            .iter()
            .find(|e| e.kind == career::EventKind::Sprint && e.races.first().is_none_or(|r| r.1 & 1 == 0))
            .expect("a sprint")
            .clone();
        let number = event.races.first().map_or(event.track, |r| r.0);
        let track = game.game_data.career.tracks[&number].clone();
        let native = career::load_route(&game.game_data.root, &track, false).unwrap();
        let mut race = CustomRace::new("short", number, RaceKind::Sprint);
        race.checkpoints = native.checkpoints[..3].to_vec();
        let m = MapMod { name: "Test".into(), races: vec![race.clone()], ..Default::default() };
        let dir = store::maps_dir();
        assert!(dir.starts_with(std::env::temp_dir()), "not the test's own maps folder: {}", dir.display());
        store::save(&dir, folder, &m).unwrap();
        store::set_enabled(&dir, folder, true).unwrap();
        nfsu2::tools::map::reload_active();
        (race, native)
    }

    #[test]
    #[cfg_attr(
        any(not(ob_game), not(feature = "dev-harness")),
        ignore = "needs NFSU2_ROOT (the player's own game files) and the dev-harness test sandbox (it writes a map mod)"
    )]
    fn a_test_run_races_the_custom_race_without_the_save() {
        let mut game = Game::new().unwrap();
        let (race, native) = sprint_mod(&game, "test-run");
        let mut profile = Profile::new("MAP_TEST_RUN");
        profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        let car = profile.car.clone();
        game.profile = Some(profile);
        let t = crate::shell::cli::MapTest { folder: "test-run".into(), race: Some("short".into()), ..Default::default() };
        game.start_map_test(&t).unwrap();
        assert!(game.profile.is_none(), "the save let go: nothing in the run writes it");
        assert_eq!(game.quick.car.as_deref(), Some(car.as_str()), "the save's car");
        let Mode::Race(r) = &game.mode else { panic!("on the grid") };
        assert_eq!(r.route.checkpoints, race.checkpoints);
        assert!(r.route.road.length < native.road.length);
        assert!(r.quick, "a race outside the career");
        assert!(matches!(game.test_view(), crate::shell::cli::TestView::Racing { laps: 1 }));
        for _ in 0..60 {
            game.advance(1.0 / 60.0).unwrap();
        }
        // An unknown race is refused, not raced.
        let mut game = Game::new().unwrap();
        let bad = crate::shell::cli::MapTest { race: Some("nope".into()), ..t };
        assert!(game.start_map_test(&bad).is_err());
    }

    #[test]
    #[cfg_attr(
        any(not(ob_game), not(feature = "dev-harness")),
        ignore = "needs NFSU2_ROOT (the player's own game files) and the dev-harness test sandbox (it writes a map mod)"
    )]
    fn a_test_run_drives_from_the_spot() {
        let mut game = Game::new().unwrap();
        let (_, route) = sprint_mod(&game, "test-roam");
        game.profile = None;
        // A spot on a city sprint's grid (on the road), heading 90°.
        let at = route.grid.first().map(|g| g.0).unwrap_or(route.checkpoints[0].extend(0.0));
        let t = crate::shell::cli::MapTest { folder: "test-roam".into(), at: Some((at, 90.0)), ..Default::default() };
        game.start_map_test(&t).unwrap();
        assert!(matches!(game.mode, Mode::Explore));
        assert!(game.quick.car.is_some(), "a stock car without a save");
        let d = game.sim.drive.as_ref().unwrap();
        assert!(d.state.position.truncate().distance(at.truncate()) < 5.0, "{} vs {at}", d.state.position);
        assert!((d.state.heading().to_degrees() - 90.0).abs() < 5.0, "{}", d.state.heading().to_degrees());
    }
}
