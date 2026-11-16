use crate::*;

impl Game {
    /// What UI_PauseOptions' builders test (0x4C38F0, 0x4C4740): a career's
    /// free roam (0x500B60), a drag race (0x89E7D8 / 0x890108), the dyno's
    /// test run (0x89E7E6).
    pub(crate) fn pause_context(&self, race: bool) -> options::Pause {
        let r = if race { self.sim.paused_race.as_ref() } else { None };
        options::Pause {
            race,
            career_roam: !race && self.profile.is_some() && !self.race_flow.quick_free_run,
            drag: r.is_some_and(|r| r.drag),
            test_run: r.is_some_and(|r| r.test.is_some()),
        }
    }

    pub(crate) fn icon_menu(&self, kind: MenuKind) -> Mode {
        let now = self.now();
        // The saved profile (a whole native decode) only for the menus that
        // read it.
        let saved = Profile::load;
        let race = match &self.mode {
            Mode::Race(r) => Some(r),
            _ => self.sim.paused_race.as_ref(),
        };
        let from_map = race.is_some_and(|r| r.from_map) || self.race_flow.last_race.as_ref().is_some_and(|r| r.1);
        let career = !race.is_some_and(|r| r.quick) && !self.race_flow.last_race.as_ref().is_some_and(|r| r.2);
        let profile_title = self.label(0xBCB1_8F38, "Profile Manager");
        let (title, options) = match kind {
            MenuKind::Main => (None, screens::MAIN_MENU.to_vec()),
            // UI_StartCareer's title is set to "Career" (0x4AF12E).
            MenuKind::Career => {
                let saved = saved();
                (Some("Career"), screens::career_menu(saved.is_some(), saved.as_ref().is_some_and(|p| p.intro_played)))
            }
            // World Map shows when 0x862838 > 1: the career's current
            // stage is past the first.
            // The shops' ways back to the garage lead to the customize
            // categories while customizing.
            MenuKind::Crib if self.save_flow.customize.is_some() => return self.icon_menu(MenuKind::CustomizeCategory),
            MenuKind::Crib => (None, screens::crib_menu(self.profile.as_ref().is_some_and(|p| p.stage > 1))),
            MenuKind::CustomizeCategory if self.shop.garage_parts && self.save_flow.customize.is_none() => {
                let options: Vec<_> = screens::CUSTOMIZE_MENU
                    .iter()
                    .copied()
                    .filter(|o| matches!(o.action, Action::CustomizeCategory(0 | 1 | 3)))
                    .collect();
                let title = self.label(0xE149_7A54, "Parts Storage");
                let m = IconMenu::new(kind.package(), Some(&title), options, now);
                return Mode::Menu(Menu::Icons(m, kind));
            }
            MenuKind::CustomizeCategory => (None, screens::CUSTOMIZE_MENU.to_vec()),
            MenuKind::QrMode => (None, screens::QR_MODE_MENU.to_vec()),
            MenuKind::Options => (None, screens::OPTIONS_MENU.to_vec()),
            // 0x4C3000 also sets Header_text "PAUSE OPTIONS", Back_button
            // and Select_button.
            MenuKind::PauseOptions(_) => {
                let label = |h: u32| self.game_data.assets.strings.get(h).unwrap_or("").to_string();
                let texts = vec![
                    (nfsu2::formats::hash::fe_hash("Header_text"), label(0x1D7B_B6C9)),
                    (nfsu2::formats::hash::fe_hash("Back_button"), label(0x933D_D695)),
                    (nfsu2::formats::hash::fe_hash("Select_button"), label(0x8202_C364)),
                ];
                let mut m = IconMenu::new(kind.package(), None, screens::PAUSE_OPTIONS_MENU.to_vec(), now);
                m.extra = vec![texts; screens::PAUSE_OPTIONS_MENU.len()];
                return Mode::Menu(Menu::Icons(m, kind));
            }
            MenuKind::Rewards => {
                let (magazines, dvds) = (!self.earned_covers(false).is_empty(), !self.earned_covers(true).is_empty());
                let sponsors = self.profile.as_ref().is_some_and(|p| !p.sponsors.is_empty());
                (None, screens::reward_menu(magazines, dvds, sponsors))
            }
            MenuKind::Covers(dvd) => return self.covers_menu(dvd, 0),
            MenuKind::SignedSponsors => return self.signed_sponsors_menu(),
            // UI_PerformanceDyno_MAIN (screen 0x55D800 → 0x55A470, options
            // 0x5227F0): Tune Settings, Test Run, Dyno Run, Mode Select,
            // Save Profile (when 0x50FD10, a profile-saving test the clone
            // does not have: left out), Quit Tuning.
            // Paused in a test run (0x83A644 = 0: 0x557360 → 0x53C9F0):
            // Resume Test Run (0x53C7E0: 0x5F4250(1)), Restart (0x564C90),
            // Tune Settings, Return to Dyno (0x564D00), Options (0x53C870:
            // UI_PauseOptionsMain), Quit Tuning (**verified**, and seen
            // under Wine 2026-11-01 as "Dyno Shop" over the paused run).
            MenuKind::Dyno if self.sim.paused_race.as_ref().is_some_and(|r| r.test.is_some()) => (
                None,
                vec![
                    screens::IconOption { icon: 0x12BB_5EA2, label: 0x1051_49CB, action: Action::ResumeRace },
                    screens::IconOption { icon: 0xB295_A6B6, label: 0x8083_3FC2, action: Action::TestRunRestart },
                    screens::IconOption { icon: 0xD406_0FC4, label: 0xA34E_13BD, action: Action::TuneSettings },
                    screens::IconOption { icon: 0xFE1E_723E, label: 0x18D5_A136, action: Action::ReturnToDyno },
                    screens::IconOption { icon: 0x520D_E4E3, label: 0xB349_93C9, action: Action::Options },
                    screens::IconOption { icon: 0x4C9E_34E6, label: 0xDE5C_DFB2, action: Action::QuitTuning },
                ],
            ),
            MenuKind::Dyno => {
                let mut o = vec![
                    screens::IconOption { icon: 0xD406_0FC4, label: 0xA34E_13BD, action: Action::TuneSettings },
                    screens::IconOption { icon: 0xF188_098C, label: 0xF622_34EF, action: Action::TestRun },
                    screens::IconOption { icon: 0xFE1E_723E, label: 0x436C_91D4, action: Action::DynoRun },
                    screens::IconOption { icon: 0xC2B2_829C, label: 0xFFE0_8459, action: Action::ModeSelect },
                ];
                // Save Profile (0x5226D0: icon 0x73FE99B7, label
                // 0x410258C3) when 0x50FD10 holds as the menu is built
                // (0x522961).
                if self.tuning_changed() {
                    o.push(screens::IconOption { icon: 0x73FE_99B7, label: 0x4102_58C3, action: Action::TuningSave });
                }
                o.push(screens::IconOption { icon: 0x4C9E_34E6, label: 0xDE5C_DFB2, action: Action::QuitTuning });
                (None, o)
            }
            MenuKind::Tuning => return self.tuning_menu(0),
            // UI_PerformanceTuning_Setting (screen 0x55D860 → 0x55A630,
            // options 0x557420): Circuit, Drag, Drift, Street X, URL Race.
            MenuKind::TuneMode => (
                None,
                vec![
                    screens::IconOption { icon: 0xE963_8D3E, label: 0x93FD_EE74, action: Action::TuneMode(0) },
                    screens::IconOption { icon: 0xAAAB_31E9, label: 0x3510_2EDF, action: Action::TuneMode(1) },
                    screens::IconOption { icon: 0x0011_9144, label: 0xD716_2CFA, action: Action::TuneMode(2) },
                    screens::IconOption { icon: 0x13A4_CEEB, label: 0x30DA_61E1, action: Action::TuneMode(3) },
                    screens::IconOption { icon: 0x3F64_95C5, label: 0x75F9_03F4, action: Action::TuneMode(4) },
                ],
            ),
            // UI_ProfileManager's constructor (0x4EB700) sets its title
            // (0x42ADB44C) to string 0xBCB18F38 (0x4EB740): the package's
            // own label there reads "Options".
            MenuKind::Profile => (Some(profile_title.as_str()), screens::profile_menu(saved().is_some())),
            // "Jump to Garage?" needs the byte 0x8635A8 (career +0x8868,
            // saved by 0x514530). No instruction stores to it directly
            // prov: open game.icon_menu
            // (the setter is **open**), but the original's own saves hold 1
            // from the first stage-1 save on (0 at the car lot's,
            // under Wine) and its explore pause then offers the option
            // second (seen under Wine, variants round 2026-10-31, a stage-2
            // save): offered once D-Day's garage (DDAY_EVENT_B) is won —
            // prov: inferred game.icon_menu-2
            // when the clone's writer sets the byte (**inferred** setter).
            MenuKind::ExplorePause => {
                let garage = self.profile.as_ref().is_some_and(|p| p.won(&self.game_data.career, career::DDAY_EVENT_B));
                (None, screens::explore_pause_menu(self.profile.is_some() && !self.race_flow.quick_free_run, garage))
            }
            // Replay greyed until the recording holds an INSTANT_REPLAY
            // (the first is taken on the green light's frame).
            MenuKind::RacePause => {
                let replay = (!matches!(self.race_flow.game_mode, 4 | 8 | 0x200))
                    .then(|| race.and_then(|r| r.replay.recording.as_ref()).is_some_and(|r| !r.instants.is_empty()));
                (None, screens::race_pause_menu(career, from_map, replay))
            }
            // "Next Race" when 0x513FA0 holds: a following race record of
            // the event (a tournament under way) and the player finished.
            MenuKind::PostRace => {
                (None, screens::post_race_menu(career, self.race_flow.next_round.is_some(), from_map, self.sim.replay.is_some()))
            }
            MenuKind::Sponsor => return self.sponsor_menu(),
            MenuKind::RimBrand => return self.rim_brand_menu(self.fitted_rim_brand()),
            MenuKind::PerfCategory => return self.perf_category_menu(0),
            MenuKind::Graphics => return self.graphics_menu(0),
            MenuKind::VinylLayer => return self.vinyl_layer_menu(0),
            MenuKind::PerfPackage => return self.perf_package_menu(1),
        };
        let mut m = IconMenu::new(kind.package(), title, options, now);
        // The pause strips do not fade at their ends (see `edge_fade`).
        m.edge_fade = !matches!(kind, MenuKind::ExplorePause | MenuKind::RacePause);
        // UI_Main_Options (0x4AEE30) opens on the record's remembered
        // option (0x50B8A0), set to 2 (Quick Race) when none (0x50B8C0).
        if kind == MenuKind::Main {
            m.select_now(if self.fe.main_selection == 0 { 2 } else { self.fe.main_selection } - 1);
        }
        if kind == MenuKind::PostRace && (1..=m.options.len()).contains(&self.race_flow.post_race_selection) {
            m.select_now(self.race_flow.post_race_selection - 1);
        }
        Mode::Menu(Menu::Icons(m, kind))
    }
}
