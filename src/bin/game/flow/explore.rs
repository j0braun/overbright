use crate::*;

/// The encounter racers career free roam sets up (0x540790: 0x89E7C4 = 3 →
/// 0x53F010(3)), whose cars' render infos the world build makes after the
/// player's (**measured**: four before the traffic fill).
const ENCOUNTER_RACERS: usize = 3;

impl Game {
    pub(crate) fn explore(&mut self, at: Option<(Vec3, f32)>) -> Result<()> {
        // A snapshot no save point took is stale in the city.
        self.drop_snapshot();
        // 0x4AF60F: the garage left for the city (0x836324 = 0).
        self.fe.crib_from_start = false;
        // No race is raced again from the city: its rivals' cars go.
        self.loads.raced_cars.clear();
        // The free-roam set-up (0x526980 → 0x4FF160): Super Far back to Far.
        self.settings.race_camera(false);
        // Free roam's camera director (race status 4 there: the crash and
        // jump cameras run as in a race; observed under Wine), the Jump
        // Camera option copied (0x5F180B); none running at the start.
        self.sim.roam_camera = Some(RaceCamera::new(self.settings.jump_camera));
        if let Some(d) = self.sim.drive.as_mut() {
            d.state.crash_camera = false;
            d.state.crash_hit = false;
            d.event_mover = 0;
        }
        // The shops' ways back to the city lead to the customize
        // categories while customizing.
        if self.save_flow.customize.is_some() {
            self.mode = self.icon_menu(MenuKind::CustomizeCategory);
            return Ok(());
        }
        // Back in the city no shop is being visited: 0x541B22 and 0x500B40
        // prov: inferred game.explore
        // zero 0x863444 / 0x86344C (**inferred**: on the way out, after
        // the city start 0x5404B0 has read them).
        let leaving = self.shop.visit.take();
        if let Some(p) = self.profile.as_mut() {
            p.shop_ride = None;
        }
        // Any scene ends with its characters: leaving the post-race flow
        // (0x603EC0, every way out of state 7) runs the NIS manager's
        // unload 0x43B640, the scene's stop 0x439070 and its free 0x43B1F0,
        // so a race's ending crowd is never drawn again in the city.
        self.scenes.shop = None;
        self.scenes.crowd = None;
        // A city start ends any race: one paused and left for free roam
        // (Return to Explore Mode from its pause) must not be kept, or
        // every menu after it holds the song (`music_context_of`: Paused).
        self.sim.paused_race = None;
        // A custom race ends with it.
        self.sim.map_race = None;
        // A fresh start in the city builds the player's car anew: no marker
        // trigger yet (+0x9AC).
        if at.is_none() {
            (self.career_world.marker_trigger, self.career_world.marker) = (None, None);
        }
        // The city start (0x5404B0 / 0x540790): in a career with career
        // +0x8744 set the saved place (career +0x8750 / +0x8760 → racer
        // record +0x91C, placed by 0x606666), else the track's
        // ForwardStartingPosition (0x6065D6 → 0x5EB5E0); Quick Race's Free
        // Run always the latter.
        let saved = if self.race_flow.quick_free_run { None } else { self.profile.as_ref().and_then(Profile::explore_at) };
        let (at, heading) = at
            .or(saved)
            .or_else(|| career::free_roam_start(&self.game_data.root))
            .unwrap_or((Vec3::new(1695.0, -884.0, 53.0), 0.0));
        // The city start (0x540691) builds the car with settings 0. Free
        // roam is track 4000 with no race (0x540790; Free Run's record,
        // 0x4B2CF0); a career's (game mode 1) unless Quick Race's Free Run.
        let career = self.profile.is_some() && !self.race_flow.quick_free_run;
        let stage = self.profile.as_ref().map_or(0, |p| p.stage);
        let owns_car = self.profile.as_ref().is_some_and(|p| !p.car.is_empty());
        let barriers = BarrierSetup { track: 4000, free_roam: true, career, stage, reversed: false, owns_car };
        self.drive_in(EXPLORE_REGION, at, heading, RaceMode::FreeRoam, 0, barriers)?;
        // The city starts copy the career difficulty (0x540569 in 0x5404B0,
        // 0x5407CC in 0x540790) and leave 0x89E804 alone.
        self.sim.race_globals.city(self.settings.difficulty);
        self.car_difficulty();
        // The city start (0x5404B0): once the career's intro movie has
        // played (0x86347C, career +0x873C, saved with it) the D-Day script
        // is done (0x5404FE). The world's start (0x60A690) in a career's
        // free roam: the GPS to DDAY_EVENT_B's goal while it is current
        // (0x5412E0 → 0x532D90).
        if career {
            if self.profile.as_ref().is_some_and(|p| p.intro_played) {
                self.career_world.dday.state = 9;
            } else if self.career_world.dday.state == 0 && !self.career_world.career_scene_played && !owns_car {
                // The scene leads into the D-Day script, which flow event 1
                // starts only while the player has no car of their own
                // (0x513BC0); a career past the car lot never sees it
                // (**inferred** from that gate).
                self.career_world.career_scene_played = self.start_career_scene();
            }
            self.gps_to_intro(career::DDAY_EVENT_B);
        }
        // The encounter racers' set-up (0x540790 → 0x53F010(3)), then the
        // world build with the career's open events (0x5412E0) and the
        // traffic's models (docs/original/rng.md order).
        self.load_free_roam_population(at, career);
        self.mode = Mode::Explore;
        // Out of the running challenge's goal shop (state 4): its
        // unique-part screen (vtable 0x79CAA8, handler 0x561730) ends the
        // challenge whichever way it is left — a part bought (0xD05FC3A3,
        // `take_unique`) or Back confirmed ("If you don't choose a
        // unique…", 0x20D0D219, Yes 0x34DC1BEC, here) — by 0x5419D0.
        if self.career_world.challenge.is_some_and(|c| c.state == 4)
            && leaving.is_some_and(|h| self.challenge_record().is_some_and(|t| t.milestone == h))
        {
            self.finish_challenge();
            // In memory only: leaving the shop is the save point (its
            // prompt or autosave writes the file, "Save points").
            if let Some(p) = self.profile.as_ref() {
                let _ = p.commit();
            }
        }
        // Back from a shop: its drive-out scene (scene type 4, 0x43B000).
        if let Some(i) = leaving.and_then(|h| self.game_data.career.shops.iter().position(|s| s.hash == h)) {
            self.start_shop_scene(nfsu2::shop_scene::SceneKind::Exit, i);
        }
        Ok(())
    }

    /// Free roam's set-up and world build, in the original's order of
    /// draws (**measured**, the sequence log, docs/original/rng.md): the career's
    /// encounter racers around `at` (0x540790 → 0x53F010(3)), with the
    /// sound the in-game audio load (speech re-seed, accel transitions),
    /// the world build (`Game::world_build`: the world generator's seed,
    /// the traffic slots, the rain), the world cars' render infos, the
    /// traffic fill (world generator, a render info per car placed), in a
    /// career the DVD covers and the open events (0x5412E0 → 0x5331D0,
    /// 0x533320 → 0x527D40: the slot-event picks), CARSFX_Stereo's rolls,
    /// the traffic's models (Traffic_AssignModels 0x412C9A), SfxCtlEngine's
    /// draws and the placed cars' render infos again. The world generator
    /// then sits in the world's car field: the traffic's while traffic is
    /// loaded, else the encounters'.
    /// The original builds the world, and so the traffic pool, on every
    /// free-roam entry; the clone does too (it kept the pool between
    /// entries before, so the models were not drawn again).
    fn load_free_roam_population(&mut self, at: Vec3, career: bool) {
        // Career free roam sets level 3 (0x540790); Quick Race's Free Run
        // its mode record's (0x53F590, `QUICK_RACE_LEVEL`).
        let level = if self.race_flow.quick_free_run {
            self.settings.qr[9].traffic as usize
        } else {
            nfsu2::ai::traffic::FREE_ROAM_LEVEL
        };
        let files = ("RoutesFreeRoam.bin", "PathsFreeRoam.bin");
        // The original loads track 4000 on every free-roam entry, its
        // routes pruned by the career stage's barriers (0x578070 →
        // 0x5E9E90).
        let stage = self.profile.as_ref().map_or(0, |p| u32::from(p.stage));
        self.sim.traffic = None;
        // Career free roam's set-up builds the encounter racers each time
        // (0x540790 → 0x53F010(3)) and a new race status, its encounter
        // manager in state NONE (0x41AC60); Quick Race's Free Run has none
        // (0x53F590 is not the career set-up).
        self.sim.encounters = None;
        // The racers' cars loaded ahead in the menus ([`EncountersAhead`]).
        let mut ahead = self.loads.encounters_ahead.take().map(|a| a.loads).unwrap_or_default();
        let offered = ahead.len();
        self.loads.encounters_taken = (0, 0);
        let mut encounters = None;
        dev_hooks::rng_pin(self, "entry");
        if !self.race_flow.quick_free_run
            && self.profile.is_some()
            && let (Some(ai), Some(d)) = (self.sim.ai.as_ref(), self.sim.drive.as_ref())
        {
            let difficulty = (self.sim.race_globals.difficulty, self.sim.race_globals.throttle_level);
            // The game draws them from its shared random state (0x43C1C0 /
            // 0x43C1F0 on 0x7EA624), so its draws move it on: the next
            // set-up (a garage jump, a shop, a race and back) draws
            // other cars (observed under Wine, a community save: three free-roam
            // loads in a row, three different line-ups,
            // docs/notes/parity/drag-drift-outrun-parity.md §0).
            let _t = nfsu2::prof::span("Encounters::new");
            let human = self.human_perf(ai);
            encounters = Encounters::new(
                &self.game_data.root,
                &self.game_data.caches,
                &self.game_data.car_types,
                &self.game_data.parts,
                (&self.game_data.career.packages, &self.game_data.car_records, human),
                ai,
                self.game_data.career.tracks.get(&4000),
                &d.car,
                difficulty,
                self.profile.as_ref().map_or(0, |p| u32::from(p.stage)),
                &mut self.rng,
                &mut ahead,
            )
            .map_err(|e| eprintln!("encounters: {e:#}"))
            .ok();
            let asked = encounters.as_ref().and_then(|e| e.pending.as_ref()).map_or(0, |p| p.1.len());
            self.loads.encounters_taken = (offered - ahead.len(), asked);
        }
        // The in-game audio load's re-seed of the speech library and its
        // objects' draws come before the world build here (sound on).
        let early = self.audio_load_early(0, false);
        dev_hooks::rng_pin(self, "world");
        let mut world = self.world_build();
        self.audio_load_late(early, false);
        // The world cars' render infos: the player's and the encounter
        // racers' (0x53F010(3)), then the traffic fill's.
        self.render_infos(1 + if encounters.is_some() { ENCOUNTER_RACERS } else { 0 });
        let mut traffic = {
            let _t = nfsu2::prof::span("explore Traffic::load");
            let mut traffic = Traffic::load(
                &self.game_data.root,
                EXPLORE_REGION,
                files,
                &self.game_data.car_types,
                self.game_data.career.tracks.get(&4000),
                self.sim.ai.as_ref().map(|a| &a.tuning),
                self.sim.ai.as_ref().map(|a| &a.bounds),
                at,
                level,
                false,
                nfsu2::qol::traffic_density(),
            )
            .map_err(|e| eprintln!("traffic: {e:#}"))
            .ok();
            // 0x5E9E90's free-roam post-load (the GPS and traffic read the
            // pruned network): barriers from PathsFreeRoam.bin by stage.
            // (The pruned network made once a run, `free_roam_routes`.)
            if let Some(t) = traffic.as_mut()
                && let Ok(file) = free_roam_routes(&self.game_data.caches, &self.game_data.root, stage)
            {
                t.routes = (*file).clone();
                t.stage = Some(stage);
            }
            traffic
        };
        // The world's set-up fill (0x42F7A0 → 0x42E2C0) on the world
        // generator, a render info for each car placed (**measured**, the
        // sequence log: 16 cars in 33 tries on the stage-1 save).
        let plan = traffic.as_ref().map(|t| {
            crate::sim::traffic::place::traffic_fill_plan(t, t.pool_len(), at.truncate(), &[], None, false, &mut world)
        });
        let placed = plan.as_ref().map_or(0, |p| p.iter().flatten().count());
        self.render_infos(placed);
        if career {
            // 0x5412E0 offers the stage's DVD covers the car's visual
            // rating reaches (0x5331D0) and rebuilds the open events and
            // the marker table (0x533320 → 0x527D40): every city start in a
            // career, after a race or a shop as after Resume. A settled win
            // leaves its event in the list until here.
            let rating = self.visual_rating();
            if let Some(p) = self.profile.as_mut() {
                p.offer_covers(&self.game_data.career, rating);
                p.refresh_open(&self.game_data.career, &mut self.rng);
            }
            self.refresh_event_triggers();
        }
        self.stereo_rolls();
        if let Some(t) = traffic.as_mut()
            && let Err(e) = t.populate(&self.game_data.root, &self.game_data.car_types, &mut self.rng)
        {
            eprintln!("traffic: {e:#}");
            traffic = None;
        }
        self.traffic_models_draws(placed);
        if let (Some(t), Some(plan), Some(d)) = (traffic.as_mut(), plan, self.sim.drive.as_ref()) {
            crate::sim::traffic::place::traffic_fill_apply(&d.world, t, plan, false, None);
        }
        self.placed_traffic_renderers(
            traffic.iter().flat_map(|t| t.cars.iter().filter(|c| c.lane.is_some()).map(|c| (&c.id, &*c.car))),
        );
        self.sim.traffic = traffic;
        // The world generator in the world's car field.
        match (self.sim.traffic.as_mut(), encounters.as_mut()) {
            (Some(t), _) => t.field.rng = world,
            (None, Some(e)) => e.field.rng = world,
            _ => {}
        }
        // The wait lives in the career object (+0x5D14), not the race
        // status, so it carries over from one free-roam load to the next.
        if let (Some(e), Some(p)) = (encounters.as_mut(), self.profile.as_ref()) {
            e.timer = p.outrun_timer;
        }
        self.sim.encounters = encounters;
        if let (Some(w), Some(r)) = (self.platform.window.as_ref(), self.sim.drive.as_ref().and(self.render.world.as_mut())) {
            warm_shadow_textures(&w.gpu, r, self.sim.traffic.iter().flat_map(|t| t.cars.iter().map(|c| &*c.car)), &nfsu2::render::RenderOptions::in_force());
        }
        // The racers' cars joined here, in the load, rather than in the
        // frame that places the first of them (`Encounters::ready`, a
        // stall of 25–60 ms in free roam's first frames otherwise; the
        // game loads them with the track). Nothing reads them before:
        // they stay parked as they are with none.
        if let Some(e) = self.sim.encounters.as_mut() {
            e.ready();
        }
    }

    /// An engage dialog dismissed: the dialog was pushed over the running
    /// city, so popping it resumes free roam where it stood — no track
    /// load and no PC_Loading (the original shows none here). Only with no
    /// free roam loaded does it start the city anew.
    pub(crate) fn back_to_city(&mut self) -> Result<()> {
        if self.here().is_some() {
            self.mode = Mode::Explore;
            Ok(())
        } else {
            self.explore(None)
        }
    }

    /// Where the player is in explore mode, to come back to: the car of
    /// free roam (race state 0x89E7B0 = 0), not a race's car in the same
    /// region (city races run on L4RA too).
    /// The car's place as the game keeps it (0x500AB0, 0x513B30, 0x5139C0
    /// read car +0x60 / +0x64 / +0x68: the centre of mass, not the body's
    /// origin +0x50), which the placement (`CarState::placed`) puts back
    /// on the same point.
    pub(crate) fn here(&self) -> Option<(Vec3, f32)> {
        self.sim.drive.as_ref().filter(|d| d.region == EXPLORE_REGION && d.barriers.free_roam).map(|d| {
            (
                d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass),
                d.state.heading(),
            )
        })
    }

    /// Return to Explore Mode (0x4A3E30 → 0x54C9E0: the career race
    /// settled; 0x4A3DF0 → 0x513950): career +0x8744 = 1, so the city
    /// start puts the car where the race was engaged (an outrun, race
    /// state 7, keeps the car's place instead; the clone's outruns run in
    /// free roam). From explore's pause, back to it.
    pub(crate) fn return_to_explore(&mut self) -> Result<()> {
        // 0x54C9E0: the event's stage (+0x37) read, the outcome settled,
        // then 0x513A50 on the stage as it now is: a movie (0x53D7B0, mode
        // 4) whose end sends 0x5F41F0(0xB), else 0xB at once.
        let event = self.race_flow.event.take();
        let stage_before = event.and_then(|i| self.game_data.career.events.get(i)).map(|e| e.stage);
        self.settle_race();
        if self.here().is_none()
            && let Some(names) = event.zip(stage_before).and_then(|(i, st)| self.explore_movie(i, st))
        {
            let names: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            self.mode = Mode::Menu(Menu::Movie(MovieScreen::new(&self.game_data.root, 4, &names)));
            if let Some(a) = self.audio.as_ref() {
                a.mixer.set_stream(None);
            }
            return Ok(());
        }
        self.explore_after_race()
    }

    /// The race dialog's Accept (0x499B50): an event with a movie (+0x02,
    /// 0x4FFDD0) queues it (0x53D640: mode 1, "DriftTut" after "Scene20")
    /// on IG_PlayMovie, whose end starts the race (0x54AF90 case 1 →
    /// 0x499B10); else the race at once.
    pub(crate) fn engage_accepted(&mut self, event: usize) -> Result<()> {
        let Some(movie) = self.game_data.career.events.get(event).map(|e| e.movie.clone()).filter(|m| !m.is_empty()) else {
            return self.start_race(event, false, false);
        };
        let mut names = vec![movie.as_str()];
        if movie.eq_ignore_ascii_case("Scene20") {
            names.push("DriftTut");
        }
        let mut m = MovieScreen::new(&self.game_data.root, 1, &names);
        m.event = Some(event);
        self.mode = Mode::Menu(Menu::Movie(m));
        if let Some(a) = self.audio.as_ref() {
            a.mixer.set_stream(None);
        }
        Ok(())
    }

    /// 0x513A50 (`stage` the event's +0x37 before the outcome settled):
    /// none for a race from the World Map's list (+0xD27); the event's
    /// +0x04 movie once it is won (0x500B80); "Scene07" when the stage
    /// changed to 2; for the last race (0x04CCE3E1) "Scene15" won, else
    /// "Scene16". 0x53D7B0 adds "UpperClass" after "Scene07".
    pub(crate) fn explore_movie(&self, event: usize, stage: u8) -> Option<Vec<String>> {
        if self.race_flow.map_start {
            return None;
        }
        let e = self.game_data.career.events.get(event)?;
        let p = self.profile.as_ref()?;
        let won = p.won(&self.game_data.career, e.unlock);
        let name = if !e.won_movie.is_empty() && won {
            e.won_movie.clone()
        } else if stage != p.stage && p.stage == 2 {
            "Scene07".to_string()
        } else if e.unlock == 0x04CC_E3E1 {
            if won { "Scene15" } else { "Scene16" }.to_string()
        } else {
            return None;
        };
        let mut names = vec![name.clone()];
        if name.eq_ignore_ascii_case("Scene07") {
            names.push("UpperClass".into());
        }
        Some(names)
    }

    /// Return to Explore Mode past the settle and its movie (0x5F41F0(0xB)).
    pub(crate) fn explore_after_race(&mut self) -> Result<()> {
        // 0x606740 case 0xB → 0x481210(9): Rachel's line and the NIS line
        // (a race's ending scene's) are stopped as a race is left; the
        // explore pause's Return to Explore Mode only resumes.
        if self.here().is_none()
            && let Some(a) = self.audio.as_mut()
        {
            a.streams.purge_speech(&a.mixer);
        }
        if self.here().is_none()
            && !self.race_flow.quick_free_run
            && let Some(p) = self.profile.as_mut()
        {
            p.explore_set = true;
        }
        // From explore's own pause or map the city is still running:
        // back to it without a track load (no PC_Loading).
        if self.here().is_some() {
            self.back_to_city()?;
        } else {
            self.explore(self.here())?;
        }
        Ok(())
    }

    /// The end of Quit to Main Menu in game (0x4A3E80): the career race
    /// settled (0x54CA50), then 0x5139C0: career +0x8744 = 1 and, out of a
    /// race, the car's place kept (in memory, the save points write).
    pub(crate) fn after_quit(&mut self) -> Result<()> {
        // Game_QuitToMainMenu (0x4A3E80) and the pause's OK (0x4A7CF0)
        // both run 0x5139C0: a running world challenge is dropped.
        self.drop_challenge();
        self.settle_race();
        self.race_flow.event = None;
        let quick = self.race_flow.quick_free_run
            || self.sim.paused_race.as_ref().is_some_and(|r| r.quick)
            || (self.here().is_none() && self.race_flow.last_race.is_some_and(|r| r.2));
        let here = self.here();
        if !quick && let Some(p) = self.profile.as_mut() {
            match here {
                Some(at) => p.keep_explore_at(at),
                None => p.explore_set = true,
            }
            let _ = p.commit();
        }
        self.race_flow.quick_free_run = false;
        self.quick.car = None;
        self.quick.parts = None;
        self.drop_drive();
        self.sim.paused_race = None;
        Ok(())
    }

    /// The race dialog declined (0x499C00: Back 0x911AB364 or Cancel →
    /// 0x499BB0): the marker's event's visit entry +6 = 3 (0x5138F0),
    /// then back to the city.
    pub(crate) fn race_declined(&mut self, event: usize) -> Result<()> {
        if let (Some(p), Some(e)) = (self.profile.as_mut(), self.game_data.career.events.get(event)) {
            p.race_declined(e);
        }
        self.back_to_city()
    }

    /// The driving controls from the keyboard map (options' `keys`).
    /// PAD_START (0xB5AF2461, the FE map's comma) on the pause's option
    /// screens resumes the game. UI_PauseOptionsMain (0x4A7D60): play out,
    /// then (+0x150 set on the out animation 0xE1FDE1D1) 0xC98356BA →
    /// 0x5F4250(1). UI_PauseOptions (0x4A8344): on Gameplay the save
    /// questions first (0x4A7F70: "turn off save prompts?" / "turn on
    /// Autosave?"; Yes, 0x33333333 → 0x4A8164, sets +0x158 and resumes,
    /// No stays), then the same resume (+0x158). UI_PC_Customize_Options
    /// (0x49C540) does not take it. Returns whether the key was taken.
    pub(crate) fn pause_start(&mut self) -> Result<bool> {
        let race = match &self.mode {
            Mode::Menu(Menu::Icons(_, MenuKind::PauseOptions(race))) => *race,
            Mode::Menu(Menu::Options(o))
                if !matches!(
                    o.category,
                    options::Category::CustomDisplay
                        | options::Category::Enhancements
                        | options::Category::Graphics
                        | options::Category::GraphicsGroup(_)
                        | options::Category::Settings(_)
                        | options::Category::Advanced
                        | options::Category::HdUi
                ) && o.pause.is_some() =>
            {
                let o = o.clone();
                let race = o.pause.is_some_and(|p| p.race);
                let (prompts, autosave) = o.opened_save;
                if o.category == options::Category::Gameplay {
                    let question = if prompts && !self.settings.save_prompts {
                        Some(0x22BC_94AF)
                    } else if !autosave && self.settings.autosave {
                        Some(0xF356_8B86)
                    } else {
                        None
                    };
                    if let Some(q) = question {
                        let _ = self.settings.save();
                        let resume = if race { MenuKind::RacePause } else { MenuKind::ExplorePause };
                        self.mode = self.ask(
                            q,
                            Mode::Menu(Menu::LotDeal(LotDeal::Resume(resume == MenuKind::RacePause))),
                            Mode::Menu(Menu::Options(o)),
                        );
                        return Ok(true);
                    }
                }
                let _ = self.settings.save();
                race
            }
            _ => return Ok(false),
        };
        self.resume_from_pause(race)?;
        Ok(true)
    }

    /// Back to the paused race or free roam (0x5F4250(1)).
    pub(crate) fn resume_from_pause(&mut self, race: bool) -> Result<()> {
        if race {
            self.mode = match self.sim.paused_race.take() {
                Some(r) => Mode::Race(r),
                None => self.icon_menu(MenuKind::Main),
            };
        } else {
            self.explore(self.here())?;
        }
        Ok(())
    }
}
