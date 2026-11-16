use crate::*;

impl Game {
    pub(crate) fn menu_quads(&self, now: f32) -> Vec<Quad> {
        let mut quads = self.menu_screen_quads(now);
        if !self.help_under_dialog() {
            quads.extend(self.help_bar_quads(now));
        }
        // The EA Trax song caption over everything (`audio::music::chyron`),
        // a package on the package clock from the song's start (F10).
        if let Some(c) = self.platform.chyron.as_ref().filter(|_| !matches!(self.mode, Mode::Menu(Menu::Movie(_)))) {
            // Chyron_FE in the front end (game state 3), else Chyron_IG
            // (0x4AC9F0): the dyno's menus are in game (state 6).
            let fe = matches!(self.mode, Mode::Menu(_)) && !self.dyno_music_session();
            quads.extend(nfsu2::audio::music::chyron_quads(&self.game_data.assets, c, now, fe));
        }
        quads
    }

    pub(crate) fn menu_screen_quads(&self, now: f32) -> Vec<Quad> {
        let mut quads = self.menu_screen_quads_only(now);
        quads.extend(self.shop_rating_quads(now));
        quads
    }

    pub(crate) fn menu_screen_quads_only(&self, now: f32) -> Vec<Quad> {
        let Mode::Menu(menu) = &self.mode else { return Vec::new() };
        // A dialog over this screen closed: its DIM, then UNDIM from the
        // answer (until then, as the dialog left it).
        if let Some(u) = self.fe.undim
            && !matches!(menu, Menu::Dialog(..))
            && menu_package(menu) == Some(u.package)
        {
            let undim_ms = now - u.undim_at.unwrap_or(now);
            return nfsu2::frontend::with_dim_scripts(
                u.package,
                u.dim,
                u.undim,
                u.scripts,
                now - u.dim_at,
                Some(undim_ms),
                || self.menu_of_quads(menu, now),
            );
        }
        self.menu_of_quads(menu, now)
    }

    /// Follow a dimmed dialog to its answer (see [`Undim`]).
    pub(crate) fn track_undim(&mut self) {
        let now = self.now();
        // A screen change pops the widget an overlay step pushed (0x55BCA0;
        // a dialog over the screen keeps it).
        if let Some(p) = self.fe.overlay_rating
            && !matches!(&self.mode, Mode::Menu(Menu::Parts(b)) if b.package == p)
            && !matches!(&self.mode, Mode::Menu(Menu::Dialog(..)))
        {
            self.fe.overlay_rating = None;
        }
        if !matches!(self.mode, Mode::Menu(Menu::McMain(_) | Menu::McList(_) | Menu::McMessage(..))) {
            self.fe.mc_under = None;
        }
        match &self.mode {
            Mode::Menu(Menu::Dialog(d, _)) => {
                let scripts = d.dim_scripts.unwrap_or((nfsu2::frontend::DIM_SCRIPT, nfsu2::frontend::UNDIM_SCRIPT));
                self.fe.undim = d.dim_under.map(|(package, dim)| Undim {
                    package,
                    dim,
                    undim: d.undim_under.unwrap_or(dim),
                    dim_at: d.opened,
                    undim_at: None,
                    scripts,
                });
            }
            mode => {
                let package = mode_package(mode);
                if let Some(u) = &mut self.fe.undim {
                    if package != Some(u.package) {
                        self.fe.undim = None;
                    } else if u.undim_at.is_none() {
                        u.undim_at = Some(now);
                    }
                }
            }
        }
    }

    pub(crate) fn menu_of_quads(&self, menu: &Menu, now: f32) -> Vec<Quad> {
        let profile = self.profile.as_ref();
        match menu {
            Menu::Movie(m) => self.movie_quads(m, now),
            Menu::Icons(m, kind) => {
                let mut quads = m.quads(&self.game_data.assets, if *kind == MenuKind::Main { None } else { profile }, now);
                if *kind == MenuKind::PerfPackage
                    && m.selected == 0
                    && let Some(list) = &self.shop.perf_parts
                {
                    quads.extend(list.quads(&self.game_data.assets, now, m.opened));
                }
                quads
            }
            Menu::LotDeal(_) => Vec::new(),
            Menu::CarLot(lot) => self.car_lot_quads(lot, now),
            Menu::InGameMap(map) => map.quads(&self.game_data.assets, now),
            Menu::Status(st) => st.screen.quads(&self.game_data.assets, &st.stats, &st.rows, now),
            Menu::WorldMap(map) => self.world_map_quads(map, now),
            Menu::QuickRace(qr) => self.quick_race_quads(qr, now),
            Menu::QrOptions(o) => {
                let ty = nfsu2::frontend::quick_race::race_type(o.mode);
                let r = &self.settings.qr[ty];
                o.quads(&self.game_data.assets, &self.game_data.career, r, self.quick_track_open(r.track), now)
            }
            Menu::Dialog(d, outcomes) => self.dialog_quads(d, outcomes, now),
            Menu::McMain(_) | Menu::McList(_) | Menu::McMessage(..) => self.mc_quads(menu, now),
            Menu::Engage(e) => e.quads(&self.game_data.assets, &self.game_data.career, self.settings.metric, now),
            Menu::ShopEngage(e) => e.quads(
                &self.game_data.assets,
                &self.game_data.career,
                self.profile
                    .as_ref()
                    .is_some_and(|p| self.game_data.career.shops.get(e.shop).is_some_and(|s| p.entered.contains(&s.hash))),
                now,
            ),
            Menu::Challenge(e) => e.quads(&self.game_data.assets, &self.game_data.career, now),
            Menu::Parts(b) => self.parts_quads(b, now),
            Menu::ChooseHud(c) => {
                let mut quads = self.fe.hud_overlay.as_deref().map_or_else(Vec::new, |m| self.menu_of_quads(m, now));
                quads.extend(c.quads(&self.game_data.assets, now));
                quads
            }
            Menu::Paint(p) => p.quads(&self.game_data.assets, profile, now),
            Menu::Results(r, _) => r.quads(&self.game_data.assets, now),
            Menu::Stats(s) => s.quads(&self.game_data.assets, self.settings.metric, now),
            Menu::Options(o) => o.quads(&self.game_data.assets, &self.settings, profile.map(|p| p.cash), FORCE_FEEDBACK, now),
            Menu::Cover(v) => v.quads_hover(&self.game_data.assets, now, self.fe.fe_hover),
            Menu::Showcase(v) => v.quads_hover(&self.game_data.assets, now, self.fe.fe_hover),
            Menu::SmsMailbox(m) => {
                profile.map_or_else(Vec::new, |p| m.quads(&self.game_data.assets, &p.inbox, &self.game_data.career.messages, now))
            }
            Menu::SmsViewer(v) => {
                profile.map_or_else(Vec::new, |p| v.quads(&self.game_data.assets, &p.inbox, &self.game_data.career.messages, now))
            }
            Menu::DynoChart(c) => c.quads(&self.game_data.assets, now),
            Menu::DynoResults(r) => r.quads(&self.game_data.assets, now),
            // UI_PerformanceTuning_Master stays up under the category's
            // screen (0x55DDB0 pushes it over the master; Back runs the
            // master's BUTTON_MASTER FadeIn, 0x5650D0), as the frames under
            // Wine show its header and strip above the panel.
            Menu::TuneGraph(g) => {
                let mut quads = self.tuning_master_quads(g.category, now);
                quads.extend(g.quads(&self.game_data.assets, now));
                quads
            }
            Menu::TuneSliders(s) => {
                let mut quads = self.tuning_master_quads(s.category, now);
                quads.extend(s.quads(&self.game_data.assets, &|h| self.label(h, ""), now));
                quads
            }
            Menu::SponsorPopup(opened) => screens::sponsor_popup_quads(&self.game_data.assets, *opened, now),
            Menu::Splash(splash) => splash.quads(&self.game_data.assets, now),
            Menu::IspConnect(opened) => screens::isp_connect_quads(&self.game_data.assets, *opened, now),
            Menu::LanServers(opened, searching) => {
                let mut quads = screens::lan_servers_quads(&self.game_data.assets, *opened, now);
                if *searching {
                    // 0x5585F0: "Looking for LAN servers ..." (0x43003B5B)
                    // with Cancel (0x541389E5).
                    let text = self.label(0x4300_3B5B, "Looking for LAN servers ...");
                    // No package named (0x4FB5E8 → 0x5585F0): 0x526C50.
                    quads.extend(
                        Dialog::new(
                            screens::dialog_package(&self.game_data.assets, &text),
                            &text,
                            &[&self.label(0x5413_89E5, "Cancel")],
                            *opened,
                        )
                        .quads(&self.game_data.assets, now),
                    );
                }
                quads
            }
            Menu::EaLogin(login, connecting) => {
                let mut quads = login.quads(&self.game_data.assets, now);
                if let Some((text, since)) = connecting {
                    let text = self.label(*text, "");
                    let opened = if since.is_nan() { now } else { *since };
                    quads.extend(
                        Dialog::new(
                            screens::dialog_package(&self.game_data.assets, &text),
                            &text,
                            &[&self.label(0x5413_89E5, "Cancel")],
                            opened,
                        )
                        .quads(&self.game_data.assets, now),
                    );
                }
                quads
            }
            Menu::Credits(c) => c.quads(&self.game_data.assets, now),
            Menu::Controls(c) => {
                let (cells, page) = self.controls_cells(c);
                c.quads(&self.game_data.assets, &cells, page, 1 + self.platform.pads.devices.len(), now)
            }
            Menu::Jukebox(j) => j.quads(&self.game_data.assets, &self.settings.trax, self.settings.trax_random, now),
            Menu::BrandSelect(b, _) => b.quads(&self.game_data.assets, &self.game_data.career.brands, now),
            Menu::Reward(r) => screens::reward_quads(&self.game_data.assets, &r.name, r.reputation, r.bank, r.opened, now),
        }
    }

    /// A movie screen (IG_PlayMovie or the boot movies' package).
    fn movie_quads(&self, m: &MovieScreen, now: f32) -> Vec<Quad> {
        let (name, object) =
            m.boot.and_then(BootStep::movie).map_or(("IG_PlayMovie", MOVIE_OBJECT), |(p, o, _)| (p, o));
        let Some(package) = self.game_data.assets.package(name) else { return Vec::new() };
        // IG_PlayMovie's and LS_PSAMovie's subtitle group (0xD3F7189E:
        // the line 0x599B8442 over two 900×300 backing images
        // 0x8BD49BCC): its zero-length INIT chains to HIDE
        // (0x16A259, alpha 0), and only the subtitle start 0x57DAB0
        // would run SHOW / HIDE on it — never in this EXE (region
        // 0x7FA73C is the static 6, 0x50D350 false). So the group
        // stays hidden by its own chain; the other objects' INITs
        // have no chain.
        let state = ScreenState { chain_init: true, ..ScreenState::default() };
        let mut state = state;
        if let Some(p) = m.player.as_ref().filter(|p| !p.rgba.is_empty()) {
            state.image.insert(object, MOVIE_TEXTURE);
            // 0x536860: v runs from height / 2^k − 0.001 down to
            // 0.001 of the texture rounded up to 2^k rows (the
            // flip, +0xC, set for a ".vp6" name by 0x566030: the
            // frames are stored bottom up); u from 0 to 1. Here
            // the texture is the frame itself, so v is rescaled.
            let h = p.height().max(2) as f32;
            let k = h / (p.height().max(2).next_power_of_two() as f32);
            state.uv.insert(object, [0.0, (k - 0.001) / k, 1.0, 0.001 / k]);
        }
        nfsu2::frontend::layout(&self.game_data.assets, package, &state, now)
    }

    /// The car lots and car selects.
    fn car_lot_quads(&self, lot: &CarLot, now: f32) -> Vec<Quad> {
        let profile = self.profile.as_ref();
        // The three bars (0x497CD0): the shops' ratings of the car
        // as it comes (0x5AC500 with no parts). The crib's select
        // (0x4962F0) rates the list entry's car record (0x511E00:
        // the slot's car with its performance parts, uniques and
        // the body parts' aerodynamics).
        let crib = lot.state == screens::LotState::CribSelect;
        let ride = lot
            .cars
            .get(lot.selected)
            .filter(|c| crib && c.placeholder.is_none())
            .and_then(|c| profile?.slot_ride(c.slot?).map(|r| (c.slot.unwrap_or(0), r)));
        let stats = lot
            .cars
            .get(lot.selected)
            .and_then(|c| self.game_data.car_records.iter().find(|(n, _)| *n == c.name))
            .map(|(_, r)| {
                let mut r = r.clone();
                match (&ride, profile) {
                    (Some((slot, (car, installed, perf))), Some(p)) => {
                        let uniques =
                            if *slot == p.slot { &p.perf_uniques } else { &p.slots[*slot].perf_uniques };
                        let mut all = perf.clone();
                        all.extend(
                            uniques.iter().filter(|&&i| i < 10).map(|&i| nfsu2::performance::UNIQUE_BASE + i),
                        );
                        self.fit_aero(car, installed, &mut all);
                        nfsu2::performance::upgrade(&mut r, &self.game_data.career.packages, &|id| all.contains(&id));
                    }
                    _ => nfsu2::performance::upgrade(&mut r, &[], &|_| false),
                }
                let x = nfsu2::performance::ratings(&r, &self.game_data.career.stat_ranges);
                [x.acceleration, x.top_speed, x.handling]
            });
        let make = lot.cars.get(lot.selected).and_then(|c| self.game_data.career.makes.get(&c.name)).map(String::as_str);
        let mut quads = lot.quads(&self.game_data.assets, profile, make, stats, now);
        // The crib's select pushes the Visual Rating widget
        // (UI_Menu_Asset_Reputation, 0x557ED0) and rates the
        // entry's car (0x55B1B0 from the constructor 0x4F9AD0 and
        // each step, 0x4B05DE); a placeholder pops it (0x53D8D0).
        if let Some((_, (car, installed, _))) = &ride {
            let r = self.visual_rating_car(car, installed);
            quads.extend(nfsu2::frontend::status::shop_rating_quads(&self.game_data.assets, r, r, now));
        }
        quads
    }

    /// UI_CareerWorldMap.
    fn world_map_quads(&self, map: &WorldMap, now: f32) -> Vec<Quad> {
        let profile = self.profile.as_ref();
        // The distance row: the track's length (record +0x98, an
        // int of metres), as Quick Race's track select prints it
        // (**inferred** the row's source is the same field).
        let length = map
            .event()
            .and_then(|i| Some(self.game_data.career.tracks.get(&self.game_data.career.events[i].track)?.length as i32 as f32));
        // The widget as the ticks left it (`MapWidget::step` in
        // `menu_tick`): its track and zoom countdown.
        let view = map.widget.view().and_then(|(track, count)| {
            let (window, fixed) = match screens::world_map_region(track) {
                Some(w) => (w, false),
                None => {
                    let t = self.game_data.career.tracks.get(&track)?;
                    (t.map_window, t.map_window_fixed)
                }
            };
            let zoom = screens::map_zoom_count(window, fixed, count);
            Some(screens::MapView { track, uv: screens::map_window_uv(window, zoom) })
        });
        match profile {
            Some(p) => map.quads(&self.game_data.assets, &self.game_data.career, p, length, self.settings.metric, view, now),
            None => Vec::new(),
        }
    }

    /// Quick Race's track select.
    fn quick_race_quads(&self, qr: &QuickRace, now: f32) -> Vec<Quad> {
        // The profile's record of the track run forward (the list
        // node's direction +0x10 is 0): drift (race type 3) the best
        // score (0x502C50), else the best time (0x502AA0, the
        // track's default with none), in seconds.
        let fresh = Profile::new("Player");
        let p = self.profile.as_ref().unwrap_or(&fresh);
        let best = qr.track().and_then(|q| self.game_data.career.tracks.get(&q.number)).map(|t| {
            if nfsu2::frontend::quick_race::race_type(qr.mode) == 3 {
                p.best_score(t.number as i16, false) as f32
            } else {
                p.best_time(t, false) as f32 / 4000.0
            }
        });
        let view = qr.widget.view().and_then(|(track, count)| {
            let t = self.game_data.career.tracks.get(&track)?;
            let zoom = screens::map_zoom_count(t.map_window, t.map_window_fixed, count);
            Some(screens::MapView { track, uv: screens::map_window_uv(t.map_window, zoom) })
        });
        qr.quads(&self.game_data.assets, &self.game_data.career, self.settings.metric, best, view, now)
    }

    /// A dialog over the screen it was pushed over.
    fn dialog_quads(&self, d: &Dialog, outcomes: &[Mode], now: f32) -> Vec<Quad> {
        // The screen it was pushed over, drawn first (behind).
        // Its DIM (when the asking code started one) runs in its
        // package's scripts from the dialog's opening.
        let mut quads = match d.under.and_then(|i| outcomes.get(i)) {
            Some(Mode::Menu(Menu::InGameMap(map))) => map.quads_with_bar(&self.game_data.assets, now, false),
            Some(Mode::Menu(m)) if !matches!(m, Menu::Dialog(..)) => {
                let mut q = match d.dim_under {
                    Some((package, objects)) => match d.dim_scripts {
                        Some(scripts) => nfsu2::frontend::with_dim_scripts(
                            package,
                            objects,
                            objects,
                            scripts,
                            now - d.opened,
                            None,
                            || self.menu_of_quads(m, now),
                        ),
                        None => nfsu2::frontend::with_dim(package, objects, now - d.opened, || {
                            self.menu_of_quads(m, now)
                        }),
                    },
                    None => self.menu_of_quads(m, now),
                };
                q.extend(self.shop_rating_quads_of(m, now));
                q
            }
            _ => Vec::new(),
        };
        quads.retain(|q| q.color[3] > 0.0);
        if self.help_under_dialog() {
            quads.extend(self.help_bar_quads(now));
        }
        quads.extend(d.quads(&self.game_data.assets, now));
        quads
    }

    /// MC_Main's questions, MC_List and MC_Main's messages over their background.
    fn mc_quads(&self, menu: &Menu, now: f32) -> Vec<Quad> {
        let mut quads = if self.fe.mc_background {
            screens::mc_background_quads(&self.game_data.assets, now)
        } else if let Some((Mode::Menu(m), package, at, scripts)) =
            self.fe.mc_under.as_ref().map(|(m, p, a, s)| (m.as_ref(), *p, *a, *s))
        {
            let mut q = nfsu2::frontend::with_dim_scripts(
                package,
                screens::DIM_HANDLER,
                screens::DIM_HANDLER,
                scripts,
                now - at,
                None,
                || self.menu_of_quads(m, now),
            );
            q.extend(self.shop_rating_quads_of(m, now));
            q.retain(|q| q.color[3] > 0.0);
            q
        } else {
            Vec::new()
        };
        quads.extend(match menu {
            Menu::McMain(m) => m.quads(&self.game_data.assets, now),
            Menu::McList(l) => l.quads(&self.game_data.assets, now),
            Menu::McMessage(m, _) => m.quads(&self.game_data.assets, now),
            _ => unreachable!(),
        });
        quads
    }

    /// The parts browsers (and UI_ICEMAIN's custom gauge preview).
    fn parts_quads(&self, b: &PartsBrowser, now: f32) -> Vec<Quad> {
        let profile = self.profile.as_ref();
        let mut quads = b.quads(&self.game_data.assets, profile, now);
        // The Custom Gauges overlay's UI_ChooseCustomHUD: the
        // selected gauge's preview (0x548D30 sets 0x839BF0 to its
        // HUDINDEX; 0x5644E0 on 0xC98356BA → 0x549010).
        if b.package == "UI_ICEMAIN"
            && !matches!(self.mode, Mode::Menu(Menu::ChooseHud(_)))
            && b.sub.is_none()
            && let (Some(i), Some(c)) = (b.part, b.categories.get(b.category))
            && c.slot == 0xA3
            && let Some(part) = c.parts.get(i)
        {
            quads.extend(nfsu2::frontend::custom_hud::preview_quads(
                &self.game_data.assets,
                self.gauge_hud_index(part.0),
                self.car_max_rpm(),
                self.settings.metric,
                now - b.opened,
            ));
        }
        quads
    }
}
