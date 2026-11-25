use crate::*;

impl Game {
    /// n of the cover's file: the milestones of the kind (1 / 3 magazines,
    /// 2 / 4 DVDs) up to and including this one in record order (0x510B80).
    pub(crate) fn cover_number(&self, milestone: usize, dvd: bool) -> usize {
        self.game_data.career
            .milestones
            .iter()
            .take(milestone + 1)
            .filter(|m| matches!(m.kind, 1..=4) && m.is_dvd() == dvd)
            .count()
            .max(1)
    }

    /// UI_MagazineView on a cover: the library loads the picture the shoot
    /// saved (`profiles::read_cover`) and tells "Load complete." (Continue:
    /// the view) or "Unable to load <NAME> Magazine <n>." (Continue: the
    /// list again); **observed** under Wine.
    pub(crate) fn open_cover(&mut self, milestone: usize, dvd: bool) -> Mode {
        let name = self.profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
        let file = profiles::cover_name(&name, dvd, self.cover_number(milestone, dvd));
        self.showcase.cover_car = None;
        match profiles::read_cover(&file) {
            Ok(rgba) => {
                self.showcase.cover_picture = Some(rgba);
                let text = self.mc_text(profiles::mc_text::LOAD_DONE, &file);
                self.mc_tell(&text, McAct::Cover(milestone, dvd, true))
            }
            Err(e) => {
                log::info!("cover {file}: {e:#}");
                let text = self.mc_text(profiles::mc_text::LOAD_FAILED, &file);
                self.mc_tell(&text, McAct::Cover(milestone, dvd, false))
            }
        }
    }

    /// The reward's cover saved (0x523F00 → 0x4A95B0 op 2 with the
    /// capture): `<NAME> DVD <n>` / `<NAME> Magazine <n>` beside the
    /// profile (`profiles::cover_name`, `native_save::picture`), then
    /// "Save complete." (or "Save failed.") with Continue to free roam.
    pub(crate) fn save_cover(&mut self, milestone: usize, dvd: bool) -> Mode {
        let pixels =
            self.showcase.cover_car.as_ref().and_then(|c| c.pixels.as_ref()).map(|(rgba, w, h)| (rgba.as_slice(), *w, *h));
        self.save_picture(milestone, dvd, pixels)
    }

    pub(crate) fn save_picture(&self, milestone: usize, dvd: bool, pixels: Option<(&[u8], u32, u32)>) -> Mode {
        let name = self.profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
        // n: the milestones of the kind (1 / 3 magazines, 2 / 4 DVDs) up to
        // and including this one in record order (0x510B80).
        let file = profiles::cover_name(&name, dvd, self.cover_number(milestone, dvd));
        let written = match pixels {
            Some((rgba, w, h)) => profiles::write_cover(&file, &nfsu2::formats::native_save::picture(rgba, w, h)),
            None => Err(anyhow::anyhow!("no cover shot")),
        };
        let text = match written {
            Ok(path) => {
                log::info!("cover saved: {}", path.display());
                self.mc_text(profiles::mc_text::SAVE_DONE, &name)
            }
            Err(e) => {
                eprintln!("save {file}: {e:#}");
                self.mc_text(profiles::mc_text::SAVE_FAILED, &name)
            }
        };
        self.mc_tell(&text, McAct::Continue(AfterSave::Roam))
    }

    /// UI_MagazineView on milestone `i`'s cover: the setup's photo-shoot
    /// camera (0x523D10: 0x510790 by the cover, 0x5105C0's picks with
    /// the game's generator 0x43C1C0 (`rng`); the game draws from it in
    /// some hundred places, the clone in fewer, so its picks are not the
    // prov: inferred game.cover_view
    /// game's, **inferred**) and the career car posed for the
    /// shot.
    pub(crate) fn cover_view(&mut self, i: usize, dvd: bool, now: f32) -> Mode {
        if self.game_data.magazines.is_none() {
            self.game_data.magazines = nfsu2::formats::magazines::Magazines::load(&self.game_data.root);
        }
        let cover = self.game_data.career.milestones.get(i).map_or(0, |m| m.cover);
        let mut picks = self.game_data.magazines.as_ref().and_then(|m| m.pick(cover, &mut self.rng));
        // NFSU2_COVER_CAMERA=k / NFSU2_COVER_LAYERS=b,m,c,x (comparison aids,
        // not the game): the setup's k-th camera and the given layer picks in
        // place of the random draws, to match a cover the original saved.
        if let (Some(p), Some(m)) = (picks.as_mut(), self.game_data.magazines.as_ref()) {
            let setup = m.setup(cover);
            if let Some(c) = debug_env()
                .cover_camera
                .clone()
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|k| setup?.cameras.get(k))
                .and_then(|h| m.cameras.iter().find(|c| c.hash == *h))
            {
                p.camera = *c;
            }
            if let Some(v) = debug_env().cover_layers.clone() {
                for (k, i) in v.split(',').enumerate().take(4) {
                    if let (Ok(i), Some(setup)) = (i.trim().parse::<usize>(), setup) {
                        p.layers[k] = setup.layers[k].get(i).copied().unwrap_or(0);
                    }
                }
            }
        }
        let camera = picks.map(|p| p.camera);
        let layers = picks.map_or([0; 4], |p| p.layers);
        self.showcase.cover_car = None;
        if camera.is_some() {
            let (name, installed, perf) = self.player_car();
            let perf = self.with_uniques(&perf);
            let settings = self.profile.as_ref().map(|p| p.tuning[0]).unwrap_or_default();
            if let Ok(car) = Car::load_tuned(
                &self.game_data.root,
                &name,
                &installed,
                &self.game_data.career.packages,
                &|id| perf.contains(&id),
                Some((&self.game_data.career.tuning, &settings)),
            ) {
                let streamed = nfsu2::formats::magazines::load_layers(&self.game_data.root, &layers);
                // 0x523D10: the picked environment map (0x510750) loaded
                // from FRONTEND\ENVMAPS and copied into the static cube
                // (0x5B9920).
                let environment = picks.and_then(|p| self.game_data.magazines.as_ref()?.envmap(p.envmap));
                // 0x523E55 selects the record's light hash before loading
                // its cube; two Showroom records deliberately use different rigs.
                let rig = environment
                    .map_or(*nfsu2::render::lighting::DEFAULT_RIG, |e| frontend_rig_hash(&self.game_data.caches, &self.game_data.root, e.light));
                let envmap = environment.and_then(|e| nfsu2::formats::magazines::load_envmap(&self.game_data.root, &e.name));
                self.showcase.cover_car =
                    Some(CoverShot { car, renderer: None, drawn: false, streamed, envmap, rig, pixels: None });
            }
        }
        let camera = camera.filter(|_| self.showcase.cover_car.is_some());
        Mode::Menu(Menu::Cover(screens::CoverView {
            milestone: i,
            dvd,
            opened: now,
            camera,
            layers,
            reward: false,
            picture: false,
        }))
    }

    /// UI_ShowCase_Preview on milestone `i` (0x4CE770): the marker
    /// effects off (`markers_off`), the camera set
    /// (0x499EC0 loads MAGAZINES_SHOWCASE.BIN) and 0x4B57B0(0, 0): the
    /// cover's set (the cover 0x837FD8 is the milestone's +0x28, set by
    /// 0x541940 just before) and 0x510680's picks with the game's
    /// generator (`rng`; as for the covers, the clone draws from it in
    /// fewer places, so the picks are not the game's, **inferred**).
    pub(crate) fn showcase_preview(&mut self, i: usize, dvd: bool, now: f32) -> screens::ShowcasePreview {
        // 0x4CE9C0: 0x501290(1), the map markers off for the showcase.
        self.markers_off();
        if self.showcase.cameras.is_none() {
            self.showcase.cameras = nfsu2::formats::magazines::Showcase::load(&self.game_data.root);
        }
        let mut v = screens::ShowcasePreview::new(i, dvd, now);
        let cover = self.game_data.career.milestones.get(i).map_or(0, |m| m.cover);
        if let Some(s) = self.showcase.cameras.as_ref() {
            v.setup = s.setup(cover);
            if let Some(set) = v.setup {
                v.picks = s.pick(set, &mut self.rng);
            }
        }
        v
    }

    /// What a showcase key asks of the game (see `screens::ShowcaseKey`).
    pub(crate) fn showcase_key(&mut self, v: &mut screens::ShowcasePreview, k: screens::ShowcaseKey) {
        let now = self.now() / 1000.0;
        // EngageState_Run 0x529590 and _Stop 0x5295C0 each play table
        // [BACK][GENERIC] (0x8380EE, id 5) as the world starts or stops.
        if matches!(k, screens::ShowcaseKey::Drive | screens::ShowcaseKey::Stop) {
            self.fe_sound(fe::event::BACK, fe::control::GENERIC);
        }
        let anims = self.sim.drive.as_ref().and_then(|d| d.car.animations.as_ref());
        let styles = self.sim.drive.as_ref().map(|d| d.car.part_styles).unwrap_or_default();
        match k {
            // 0x4B5C70 → 0x499F10: what Show Off opened shuts.
            screens::ShowcaseKey::Drive if v.mode == screens::ShowcaseMode::Control => {
                for (flag, channels) in [(0, &[0usize][..]), (1, &[2, 3][..]), (2, &[1][..])] {
                    if v.open[flag] {
                        v.open[flag] = false;
                        for &c in channels {
                            self.showcase.parts.request(anims, &styles, c, false, 1.0, now);
                            Self::part_stitch(self.audio.as_mut(), &mut self.rng, c, false);
                        }
                    }
                }
            }
            screens::ShowcaseKey::Parts(channels, open) => {
                for &c in channels {
                    self.showcase.parts.request(anims, &styles, c, open, 1.0, now);
                    Self::part_stitch(self.audio.as_mut(), &mut self.rng, c, open);
                }
            }
            screens::ShowcaseKey::Cycle => {
                if let (Some(s), Some(set)) = (self.showcase.cameras.as_ref(), v.setup) {
                    s.next_camera(set, &mut v.picks);
                }
            }
            // 0x4CF592: ShowcaseCamera_Shutter 0x501250 (the FX_Camera
            // shutter, `Scrapes::shutter`), then 0x499E60: the masthead
            // and extra from the LOW pack.
            screens::ShowcaseKey::Stream => {
                if let Some(a) = self.audio.as_mut() {
                    let played = a.scrapes.shutter(&mut a.mixer);
                    if debug_env().audio_debug {
                        eprintln!("audio: FX_Camera shutter {}", if played { "started" } else { "not started" });
                    }
                }
                let hashes: Vec<u32> = [v.picks.masthead, v.picks.extra].into_iter().filter(|&h| h != 0).collect();
                let streamed =
                    nfsu2::formats::magazines::load_layers_from(&self.game_data.root, "MAGAZINES_STREAMING_LOW.bin", &hashes);
                v.streamed = !streamed.is_empty();
                self.platform.ui_uploads.extend(streamed);
            }
            _ => {}
        }
    }

    /// CarPart_RequestAnim's sound in game (0x61F830 with [0x82B884]+0x90
    /// ≠ 1: the showcase runs in the world): 0x481500 on the FE_COMMON
    /// player (`fe_rules::part_stitch`). The stitch layer's start draws
    /// (0x45B280) are the game's shared generator's (`rng`), as every audio
    /// object's (docs/original/rng.md).
    pub(crate) fn part_stitch(audio: Option<&mut Audio>, rng: &mut nfsu2::opponents::GameRng, channel: usize, open: bool) {
        if let (Some(a), Some(n)) = (audio, nfsu2::audio::fe_rules::part_stitch(channel, open)) {
            let saved = nfsu2::audio::collision::enter_game_rng(rng.0);
            a.part_stitches.start(n);
            rng.0 = nfsu2::audio::collision::leave_game_rng(saved);
        }
    }

    /// The showcase's frame: under Control Car and Show Off the world runs
    /// (the player's controls, or none: 0x5EBA40(0)); the showcase camera
    /// (`chase::showcase_view` with the picked camera's parameters, 0x4B5730)
    /// then replaces the chase view, pushed out of the world as the chase
    /// view is (0x444530). `anim_now`: the part animations' clock, s.
    pub(crate) fn showcase_frame(
        &mut self,
        mode: screens::ShowcaseMode,
        camera: u32,
        controls: Controls,
        dt: f32,
        anim_now: f32,
    ) {
        let params = self.showcase.cameras.as_ref().and_then(|s| s.camera(camera)).map(|c| c.params);
        let Some(d) = self.sim.drive.as_mut() else { return };
        if matches!(mode, screens::ShowcaseMode::Control | screens::ShowcaseMode::ShowOff) {
            let input = if mode == screens::ShowcaseMode::Control { controls } else { Controls::default() };
            let traffic = self.sim.traffic.as_mut().filter(|t| t.region == d.region);
            step_explore(d, traffic, self.sim.encounters.as_mut(), input, dt, &mut self.rng, &mut self.sim.contact_sounds);
        }
        if let Some(params) = params {
            let mut q = nfsu2::ground::GroundQuery::default();
            q.barriers = d.state.ground.barriers.clone();
            let normal = d.world.ground_for(&mut q, d.state.position).map_or(Vec3::Z, |g| g.1);
            d.camera = nfsu2::chase::showcase_view(&params, d.state.position, d.state.orientation, normal);
            d.push_camera();
        }
        if let Some(cr) = self.render.cars.get_mut(&d.car_id) {
            cr.set_part_poses(self.showcase.parts.poses(d.car.animations.as_ref(), anim_now));
            cr.set_part_glow(part_glow(&d.car, anim_now));
        }
    }

    /// UI_MagazineReward on milestone `i`'s cover (see
    /// `screens::CoverView::reward_quads`); kind 4 is a DVD.
    pub(crate) fn magazine_reward(&mut self, i: usize) -> Mode {
        let dvd = self.game_data.career.milestones.get(i).is_some_and(|m| m.kind == 4);
        let mut mode = self.cover_view(i, dvd, self.now());
        if let Mode::Menu(Menu::Cover(v)) = &mut mode {
            v.reward = true;
        }
        mode
    }
}
