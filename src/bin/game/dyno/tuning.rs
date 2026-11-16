use crate::*;

impl Game {
    /// UI_PerformanceTuning_Master (screen 0x55D8C0 → 0x55AA50, options
    /// 0x557690): the seven tuning categories, each locked (0x522A10 +0x22)
    /// unless a fitted performance part opens one of its parameters
    /// (0x5225D0 with [`nfsu2::tuning`]).
    pub(crate) fn tuning_menu(&self, selected: usize) -> Mode {
        let fitted = |id: u8| self.profile.as_ref().is_some_and(|p| p.perf.contains(&id));
        let gears = self.tuned_gears();
        let options = nfsu2::tuning::CATEGORIES
            .iter()
            .enumerate()
            .map(|(i, &(icon, label, _))| {
                let open = nfsu2::tuning::category_open(&self.game_data.career.tuning, &fitted, gears, i);
                screens::IconOption {
                    icon,
                    label,
                    action: if open { Action::TuningCategory(i) } else { Action::Unavailable },
                }
            })
            .collect();
        let mut menu = IconMenu::new(MenuKind::Tuning.package(), None, options, self.now());
        menu.select_now(selected);
        Mode::Menu(Menu::Icons(menu, MenuKind::Tuning))
    }

    /// UI_PerformanceTuning_Master on `category`, as it stays under a
    /// category's screen.
    pub(crate) fn tuning_master_quads(&self, category: usize, now: f32) -> Vec<Quad> {
        match self.tuning_menu(category) {
            Mode::Menu(Menu::Icons(mut m, _)) => {
                m.opened = 0.0;
                m.select_now(category);
                m.quads(&self.game_data.assets, self.profile.as_ref(), now)
            }
            _ => Vec::new(),
        }
    }

    /// The settings the dyno edits: the tuning object's sliders. The dyno
    /// menu (0x55A470 → 0x50FE20) builds the player's car from the car
    /// object's settings 0 (vtable +0xC, 0x516AC0: settings n to car
    /// +0x398, the tuning object +0x14), so before a mode is chosen they
    /// start as Circuit's; a mode's accept loads its settings (0x50FC60).
    pub(crate) fn dyno_settings(&self) -> nfsu2::tuning::Settings {
        self.dyno.tuning_object.or_else(|| self.profile.as_ref().map(|p| p.tuning[0])).unwrap_or_default()
    }

    /// 0x50FAE0 (Quit Tuning, both answers of 0x71851654): the tuning
    /// object's sliders become the chosen mode's settings (none chosen:
    /// nothing, 0xFF); then 0x53CC80 drops the tuning object and the mode
    /// (0x83A540 = 0, 0x83A63C = 0xFF).
    /// The question Quit Tuning asks (0x564DE0): in Customize the
    /// "Are you sure you want to quit tuning?" one, else "Quit to Garage?".
    pub(crate) fn quit_tuning_question(&self) -> u32 {
        // 0x564E16: career "Quit to Performance Shop?" (0xB3FB59A9) with
        // 0x83A640 set, else "Quit to Garage?" (Wine, the finished-career save's dyno
        // from IND_PERF_SHOP_1).
        if self.save_flow.customize.is_some() {
            0x4081_3F63
        } else if self.dyno.from == 1 {
            0xB3FB_59A9
        } else {
            0xE5A0_D45E
        }
    }

    /// Where Quit Tuning's Yes goes (0x55A7B3 by 0x83A640): the garage
    /// (Customize's categories in Customize), or UI_ChoosePerformanceCategory
    /// (0x790478) for the shop's session, on its first entry (Wine: Engine).
    pub(crate) fn after_tuning(&self) -> Mode {
        if self.dyno.from == 1 && self.save_flow.customize.is_none() {
            self.perf_category_menu(0)
        } else {
            self.icon_menu(MenuKind::Crib)
        }
    }

    /// The tuning session's menus, drawn over its track.
    pub(crate) fn tuning_menu_over_world(&self) -> bool {
        self.dyno.tuning_session
            && self.sim.drive.is_some()
            && match &self.mode {
                Mode::Menu(Menu::Icons(_, MenuKind::Dyno | MenuKind::TuneMode | MenuKind::Tuning)) => true,
                Mode::Menu(Menu::TuneSliders(..) | Menu::TuneGraph(_) | Menu::DynoChart(_) | Menu::DynoResults(_)) => {
                    true
                }
                Mode::Menu(Menu::Dialog(..) | Menu::LotDeal(_) | Menu::McMessage(..)) => true,
                _ => false,
            }
    }

    /// 0x50FD10: the tuning object differs from its mode's settings
    /// (0x50FCA0) or any working settings from the car object's (the
    /// dyno's entry, `tuning_entry`).
    pub(crate) fn tuning_changed(&self) -> bool {
        let Some(p) = self.profile.as_ref() else { return false };
        let m = self.dyno.tuning_mode as usize;
        self.dyno.tuning_object.is_some_and(|t| m < p.tuning.len() && p.tuning[m] != t) || p.tuning != self.dyno.tuning_entry
    }

    pub(crate) fn quit_tuning(&mut self) {
        // The session ends (0x526980 clears 0x89E7E6) and its track goes:
        // the garage / Customize scene is the front end's again.
        if std::mem::take(&mut self.dyno.tuning_session) {
            self.drop_drive();
            self.sim.paused_race = None;
        }
        let m = std::mem::replace(&mut self.dyno.tuning_mode, 0xFF);
        if let (Some(t), Some(p)) = (self.dyno.tuning_object.take(), self.profile.as_mut())
            && (m as usize) < p.tuning.len()
        {
            p.tuning[m as usize] = t;
            let _ = p.commit();
        }
    }

    /// The curve widget's inputs for the career car (`tuning::curve_graph`
    /// on the upgraded record, the dyno settings, the Units option); None
    /// for the nitrous widget or when the records cannot be read.
    pub(crate) fn curve_graph(&self, param: u8) -> Option<nfsu2::tuning::CurveGraph> {
        if !matches!(param, 23 | 24) {
            return None;
        }
        let p = self.profile.as_ref()?;
        let (stock, up) = self.tuning_records()?;
        let fitted = |id: u8| p.perf.contains(&id);
        Some(nfsu2::tuning::curve_graph(
            &up,
            &stock,
            &self.game_data.career.tuning,
            &fitted,
            &self.dyno_settings(),
            param,
            self.settings.metric,
        ))
    }

    /// The graph screen of a category with its curve's inputs.
    pub(crate) fn tune_graph(&self, category: usize, now: f32) -> screens::TuneGraph {
        let mut graph = screens::TuneGraph::new(category, now);
        graph.metric = self.settings.metric;
        graph.curve = self.curve_graph(graph.param);
        if graph.param == 16 {
            graph.views = self.slider_views(&[16, 17]);
        }
        graph
    }

    /// The slider rows' numbers for the dyno car (`tuning::slider_view`:
    /// the upgraded record tuned with the dyno settings, the Units option
    /// as the car's +4, **inferred**).
    pub(crate) fn slider_views(&self, params: &[u8]) -> Vec<(u8, nfsu2::tuning::SliderView)> {
        let Some(p) = self.profile.as_ref() else { return Vec::new() };
        let Some((stock, up)) = self.tuning_records() else { return Vec::new() };
        let fitted = |id: u8| p.perf.contains(&id);
        let settings = self.dyno_settings();
        params
            .iter()
            .map(|&param| {
                (
                    param,
                    nfsu2::tuning::slider_view(
                        &up,
                        &stock,
                        &self.game_data.career.tuning,
                        &fitted,
                        &settings,
                        param,
                        0,
                        self.settings.metric,
                    ),
                )
            })
            .collect()
    }

    /// The open tuning screen's numbers again (after a change).
    pub(crate) fn refresh_tuning_views(&mut self) {
        let params: Vec<u8> = match &self.mode {
            Mode::Menu(Menu::TuneSliders(t)) => t.rows.iter().map(|r| r.0).collect(),
            Mode::Menu(Menu::TuneGraph(g)) if g.param == 16 => vec![16, 17],
            _ => return,
        };
        let views = self.slider_views(&params);
        match &mut self.mode {
            Mode::Menu(Menu::TuneSliders(t)) => t.views = views,
            Mode::Menu(Menu::TuneGraph(g)) => g.views = views,
            _ => {}
        }
    }

    /// A tuning category's slider screen (UI_PerformanceTuning_Sliders,
    /// the drivetrain's own package): its tunable rows (0x549860).
    pub(crate) fn tune_sliders(&self, category: usize, now: f32) -> Mode {
        let fitted = |id: u8| self.profile.as_ref().is_some_and(|p| p.perf.contains(&id));
        let gears = self.tuned_gears();
        let rows = screens::tuning_rows(category)
            .iter()
            .copied()
            .filter(|r| nfsu2::tuning::tunable(&self.game_data.career.tuning, &fitted, gears, r.0))
            .collect();
        let package = if category == 1 { "UI_PerformanceTuning_Drivetrain" } else { "UI_PerformanceTuning_Sliders" };
        let mut t = screens::TuneSliders {
            package,
            rows,
            selected: 0,
            top: 0,
            category,
            metric: self.settings.metric,
            opened: now,
            views: Vec::new(),
        };
        t.views = self.slider_views(&t.rows.iter().map(|r| r.0).collect::<Vec<_>>());
        Mode::Menu(Menu::TuneSliders(t))
    }

    /// One press on a tuning screen (`dir` ±1): the screens' 0x5B4680 on
    /// the dyno car tuned with the settings (`tuning::nudge`: the step,
    /// the clamps storing the sliders back, the gears pushed, the nitrous
    /// partner, a curve point's spreading).
    pub(crate) fn nudge_slider(&mut self, param: u8, point: usize, dir: f32) {
        let records = self.tuning_records();
        let current = self.dyno_settings();
        let Some(p) = self.profile.as_ref() else { return };
        let Some((stock, up)) = records else { return };
        let fitted = |id: u8| p.perf.contains(&id);
        self.dyno.tuning_object =
            Some(nfsu2::tuning::nudge(&up, &stock, &self.game_data.career.tuning, &fitted, &current, param, point, dir));
    }

    /// Settings as the car's set-up leaves them on its tuning object: each
    /// slider clamped into its range and stored back (0x5B4790 →
    /// 0x5B3EF0 → 0x59AF90). A later set-up from these bytes can differ from
    /// one from the raw bytes by the byte rounding (a gear clamped at 2.128
    /// is stored as t = −0.44 and rebuilt as 2.136; under Wine the dyno's
    /// second run's 0-60 / 0-100 took that).
    pub(crate) fn clamped_tuning(&self, s: nfsu2::tuning::Settings) -> nfsu2::tuning::Settings {
        let Some(p) = self.profile.as_ref() else { return s };
        let Some((stock, mut up)) = self.tuning_records() else { return s };
        let fitted = |id: u8| p.perf.contains(&id);
        nfsu2::tuning::apply(&mut up, &stock, &self.game_data.career.tuning, &fitted, &s)
    }

    /// PAD_START (0xB5AF2461) on a tuning screen (0x5650D0 / 0x5654E0,
    /// **verified**): not in the dyno chart (0x83A7A4) and not twice in a
    /// tick, the screen marks itself done (+0x14C), refits the player's car
    /// with the sliders (0x5F6C30(1) → 0x639CF0), hides the help bar and
    /// resumes the game (0x5F4250(1) = 0x5F41F0(8, 1), the call Resume Test
    /// Run makes, 0x53C7E0). So in a paused test run Start goes back to the
    /// run with the edits made; on the dyno (no run paused) the flow event
    // prov: inferred game.tuning_start
    /// has nothing to resume (**inferred**: nothing happens there). The
    /// clone's refit rebuilds the car's handling the way the run's load
    // prov: inferred game.tuning_start-2
    /// does (the dyno settings, the run's race set-up; **inferred**
    /// equivalent of 0x639CF0).
    pub(crate) fn tuning_start(&mut self) {
        if !self.sim.paused_race.as_ref().is_some_and(|r| r.test.is_some()) {
            return;
        }
        if let (Some(mut h), Some(d)) = (self.dyno_handling(), self.sim.drive.as_mut()) {
            let race = d.car.car_type.handling.race;
            nfsu2::handling::setup_performance(&mut h, race);
            d.car.car_type.handling = h;
        }
        if let Some(race) = self.sim.paused_race.take() {
            self.mode = Mode::Race(race);
        }
    }

    /// The tuning screens' Default (BUTTON1, 0x50F9B0): the parameter's
    /// sliders in the tuning object back to the middle.
    pub(crate) fn restore_tuning(&mut self, param: u8) {
        let mut s = self.dyno_settings();
        s.restore(param);
        self.dyno.tuning_object = Some(s);
    }
}
