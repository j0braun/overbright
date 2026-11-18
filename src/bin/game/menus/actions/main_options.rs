use crate::*;

impl Game {
    /// `act`'s main menu and options group: the screen's next mode, or `None` when the arm set it (or left it) itself.
    pub(super) fn act_main_options(&mut self, action: Action, kind: MenuKind, now: f32) -> Result<Option<Mode>> {
        Ok(Some(match action {
            // UI_Main → UI_StartCareer (0x4AED80).
            Action::Career => {
                // Leaving Quick Race's Free Run for the career by any way
                // (its pause's Quit clears it too): the markers are the
                // career's again (0x500B60: game mode 1).
                self.quick.car = None;
                self.quick.parts = None;
                self.race_flow.quick_free_run = false;
                self.icon_menu(MenuKind::Career)
            }
            // Quick Race → UI_QRModeSelect (0x4FC050).
            Action::QuickRace => self.icon_menu(MenuKind::QrMode),
            // Online (0x4AECB0 → UI_Main's 0xD05FC3A3, 0x4F98B0): game
            // mode 8 and the online flow's first screen, UI_OLISPConnect.
            Action::Online => {
                self.race_flow.game_mode = 8;
                Mode::Menu(Menu::IspConnect(now))
            }
            // LAN (0x4AEDE0): game mode 0x200, UI_PC_LAN_ServerSelect, whose
            // constructor (0x4FB4C0) opens the network and searches.
            Action::Lan => {
                self.race_flow.game_mode = 0x200;
                self.fe.lan_search_until = crate::menus::keys::online::lan_search_until(self.real_now());
                Mode::Menu(Menu::LanServers(now, true))
            }
            // → UI_ProfileManager (0x4AEDB0), UI_OptionsMain (0x4AECE0).
            Action::ProfileManager => self.icon_menu(MenuKind::Profile),
            // The pause's Options (0x4C2700): UI_PauseOptionsMain.
            Action::Options if matches!(kind, MenuKind::ExplorePause | MenuKind::RacePause) => {
                self.icon_menu(MenuKind::PauseOptions(kind == MenuKind::RacePause))
            }
            // The test run's Options (0x53C870 → UI_PauseOptionsMain).
            Action::Options if kind == MenuKind::Dyno => self.icon_menu(MenuKind::PauseOptions(true)),
            // UI_Main's Options (0x4AECE0): the save-point snapshot
            // (0x52A470(0)) and the save-prompts option kept (0x836360).
            Action::Options => {
                if kind == MenuKind::Main {
                    self.take_snapshot(false);
                    self.fe.options_prompts = self.settings.save_prompts;
                    // No profile: the snapshot is of the memory state, so
                    // the options alone here.
                    self.fe.options_settings =
                        (self.profile.is_none() && self.settings.save_prompts).then(|| self.settings.clone());
                }
                self.icon_menu(MenuKind::Options)
            }
            // UI_PauseOptionsMain's four: UI_PauseOptions on the category.
            Action::PauseSetting(n) => match options::Category::from_setting(n) {
                Some(c) => {
                    let pause = self.pause_context(matches!(kind, MenuKind::PauseOptions(true)));
                    Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                        c,
                        &self.settings,
                        FORCE_FEEDBACK,
                        Some(pause),
                        now,
                    )))
                }
                None => return Ok(None),
            },
            // UI_OptionsMain's Audio, Display, Gameplay and Player open
            // UI_Options on their category (0x4B6080).
            Action::Setting(n) => match options::Category::from_setting(n) {
                Some(c) => {
                    Mode::Menu(Menu::Options(options::OptionsScreen::new(c, &self.settings, FORCE_FEEDBACK, now)))
                }
                // Controls (4), EA GAMES TRAX (5), Credits (0x4B6080).
                None => match n {
                    4 => Mode::Menu(Menu::Controls(screens::ControlsScreen::new(now))),
                    5 => Mode::Menu(Menu::Jukebox(screens::Jukebox::new(now))),
                    _ => Mode::Menu(Menu::Credits(screens::Credits::load(&self.game_data.root, now))),
                },
            },
            // Free Run (race type 9; 0x4B2CF0 sets the type record's
            // track to 4000, the city): the next screen (0x4B2A80) skips
            // UI_QRTrackSelect for UI_QRCarSelect, then free roam without
            // the career.
            Action::QrMode(screens::QrMode::FreeRun) => self.quick_car_select(None),
            // Quick Race modes → the track list of that kind.
            Action::QrMode(m) => match self.quick_track_select(m, now) {
                Some(qr) => Mode::Menu(Menu::QuickRace(qr)),
                None => self.icon_menu(kind),
            },
            _ => unreachable!("act_main_options: {action:?}"),
        }))
    }

    /// `act`'s career / profile / crib group (as `act_main_options`).
    pub(super) fn act_profile(&mut self, action: Action, kind: MenuKind, now: f32) -> Result<Option<Mode>> {
        Ok(Some(match action {
            // Start New Career (0x4AF090) opens MC_Main (0x4A3100 mode
            // 0x21961, `McMain`): the profile's name, then, on the
            // screen's 0x461A18EE, the career (0x495A30): the D-Day intro
            // in Rachel's car.
            Action::NewCareer => {
                let mut m = McMain::new(self.profile.is_some(), now);
                m.autosave = self.settings.autosave;
                Mode::Menu(Menu::McMain(m))
            }
            // Resume (0x4AF0C0 → 0x495A30): the career in game mode 1.
            // A sponsor's react (0x4C14C0): with none signed in the stage
            // (stage record +0xE8), "Want to sign on with %s?" (0x39DF9562,
            // IG_GenericDialog_SMALL.fng, Yes 0x88579775 / No 0xBE503981, No
            // focused; seen under Wine); Yes sends 0xD05FC3A3, the sign.
            Action::Sign(i) => {
                let Some(name) =
                    self.profile.as_ref().and_then(|p| p.sponsor_offers(&self.game_data.career).get(i).map(|s| s.name.clone()))
                else {
                    return Ok(None);
                };
                let shown = self.label(nfsu2::formats::hash::bin_hash(&format!("SPONSOR_{name}")), &name);
                let text = self.label(0x39DF_9562, "Want to sign on with %s?").replace("%s", &shown);
                let buttons = [self.label(0x8857_9775, "Yes"), self.label(0xBE50_3981, "No")];
                let mut d = Dialog::new("GenericDialog_SMALL", &text, &[&buttons[0], &buttons[1]], now);
                d.selected = 1;
                // Over the screen (0x5587A0's arg 1, the caller's package;
                // Wine menus112 pr06): No / Back (0x34DC1BCF) stays on it.
                d.under = Some(1);
                let here = std::mem::replace(&mut self.mode, Mode::Explore);
                self.mode = Mode::Menu(Menu::Dialog(d, vec![Mode::Menu(Menu::LotDeal(LotDeal::Sign(i))), here]));
                return Ok(None);
            }
            // Resume (0x495A30): past the D-Day intro, the garage
            // (UI_CareerCrib); else the intro goes on in the city.
            Action::ResumeCareer | Action::LoadCareer => {
                // 0x495A30 / 0x4C5033: game mode 1.
                self.race_flow.game_mode = 1;
                self.quick.car = None;
                self.quick.parts = None;
                self.race_flow.quick_free_run = false;
                self.profile = Profile::load();
                let Some(p) = self.profile.as_mut() else { return Ok(None) };
                p.upgrade_save(&self.game_data.career);
                // The open events are not in the save (career +0x71C8 holds
                // event-record pointers) and the load picks none: the
                // original makes no slot-event draw at a profile load or at
                // Resume, its first after the load is the city start's
                // 0x527D40 (**measured**, the sequence log, docs/rng.md).
                p.start_career(&self.game_data.career);
                if let Some(mut d) = self.sim.drive.take() {
                    let device = self.platform.window.as_ref().map(|w| w.gpu.device.clone());
                    self.render.resident_water = Some(std::mem::take(&mut d.effects.water).for_new_drive());
                    self.loads.resident = Some((d.region, d.world));
                    self.loads.kept_ahead = None;
                    self.render.resident = device.zip(self.render.world.take());
                    self.pacing.clocks.world_teardown();
                    // The car's renderer goes with the drive.
                    self.render.cars.remove(&d.car_id);
                }
                if p.intro.is_some() {
                    self.career_intro()?;
                    return Ok(None);
                }
                self.icon_menu(MenuKind::Crib)
            }
            // UI_ProfileManager's Create (0x4C5880: MC_Main mode 0x861,
            // operation 0x60): MC_Main's create flow, then back here.
            Action::ProfileCreate => {
                self.mc_over(kind, now);
                let mut m = McMain::new(self.profile.is_some(), now);
                m.autosave = self.settings.autosave;
                m.career = false;
                Mode::Menu(Menu::McMain(m))
            }
            // Save (0x4C57F0: mode 0x4A51, operation 0x50, 0x4F35FC): with a
            // profile "Are you sure you want to save the profile named %s?"
            // (state 5); without one MC_Main leaves at once (0x461A18EE).
            // The crib's Save (0x4AF740 on 0x0C407210): the snapshot dropped
            // (0x5032C0), then the save point with mode 0xC41 (no 0x200):
            // autosave on skips "Are you sure…" and asks only the
            // overwrite question (Wine, the finished-career save: "Do you want
            // to overwrite previously saved game?" at once). As the crib's
            // other accepts, the menu has faded out under it (FORWARD).
            Action::Save if kind == MenuKind::Crib => {
                self.drop_snapshot();
                self.save_point(false, AfterSave::Menu(MenuKind::Crib))?;
                return Ok(None);
            }
            Action::ProfileSave | Action::Save => match self.profile.as_ref().map(|p| p.name.clone()) {
                Some(name) => {
                    self.mc_over(kind, now);
                    self.mc_save_question(&name, kind)
                }
                None => self.icon_menu(kind),
            },
            // Load (0x4C5820, mode 0xC11) and Delete (0x4C5850, mode
            // 0x831): the profiles listed (0x4E8CE0 → MC_List).
            Action::ProfileLoad => {
                self.mc_over(kind, now);
                self.mc_list(McOp::Load)
            }
            Action::ProfileDelete => {
                self.mc_over(kind, now);
                self.mc_list(McOp::Delete)
            }
            // The garage (0x4AF630 etc.).
            Action::Explore => {
                let at = self.here();
                let yes = Mode::Menu(Menu::LotDeal(LotDeal::LeaveCrib));
                self.career_world.pending_explore = at;
                // No focused (0x5587A0's 1 at 0x4AF644; seen under Wine,
                // career playthrough 2026-10-31). The garage's accept
                // (0x0C407210 in its handler 0x4AF3A0) first plays the
                // Event Handler's FORWARD (0xDE6EFF34: the objects'
                // FADEOUT, then 0xE1FDE1D1 at 140 ms, on which the option's
                // action 0x4AF630 asks and sends StartDim): under Wine
                // (stage-2 playthrough, frames 33 ms apart) the strip, title,
                // logo and help bar fade out in about 100 ms and the
                // question then stands over the garage scene alone, the
                // package not drawn at all (not dimmed); No brings it back.
                // So, as the other icon menus' questions, no screen under it.
                let mut ask = self.ask_in("GenericDialog_SMALL", 0x60B4_2AFB, yes, self.icon_menu(MenuKind::Crib));
                if let Mode::Menu(Menu::Dialog(d, _)) = &mut ask {
                    d.selected = 1;
                }
                ask
            }
            Action::WorldMap => self.world_map(),
            Action::CarSelect => self.own_cars(None),
            _ => unreachable!("act_profile: {action:?}"),
        }))
    }
}
