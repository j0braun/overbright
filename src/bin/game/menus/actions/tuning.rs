use crate::*;

impl Game {
    /// `act`'s dyno group: entry, test run, dyno run (as `act_main_options`).
    pub(super) fn act_dyno(&mut self, action: Action, kind: MenuKind, now: f32) -> Result<Option<Mode>> {
        Ok(Some(match action {
            // Restart (0x564C90): UI_InGameDialog "…" (0x0972A44B), OK
            // 0x0639 → 0xE1A57D51, Cancel 0x541389E5 focused.
            Action::TestRunRestart => self.in_game_dialog(
                0x0972_A44B,
                Mode::Menu(Menu::LotDeal(LotDeal::TestRunRestart)),
                self.icon_menu(MenuKind::Dyno),
            ),
            // Return to Dyno (0x564D00): 0x455C3F80 with Yes 0xF770 → 0x526ADEFC,
            // No 0x061C focused, over the menu (0x96DDA9E6).
            Action::ReturnToDyno => {
                let text = self.label(0x455C_3F80, "?");
                let buttons = [self.label(0xF770, "Yes"), self.label(0x061C, "No")];
                let mut d = Dialog::new(
                    screens::dialog_package(&self.game_data.assets, &text),
                    &text,
                    &[&buttons[0], &buttons[1]],
                    self.now(),
                );
                d.selected = 1;
                dialog_over(
                    Mode::Menu(Menu::Dialog(
                        d,
                        vec![Mode::Menu(Menu::LotDeal(LotDeal::ReturnToDyno)), self.icon_menu(MenuKind::Dyno)],
                    )),
                    true,
                    None,
                )
            }
            // "Do you wish to Performance Tune / Dyno your car?" (0x4AF780);
            // yes (0x34DC1BEC → 0x495AF0 → 0x5309F0) into the dyno.
            // The crib's option (0x4AF780) asks 0xB10AE56F; Customize's
            // (0x55E7A0) "Do you want to enter the Performance Tuning /
            // Dyno track?" (0x54B649F1; 0x843F5D58 online), its Yes
            // (0x34DC1BEC in 0x55E530) going on to 0x5309F0 on the car.
            // No stays (icon_menu(Crib) is the categories in Customize).
            Action::Tuning => {
                let yes = Mode::Menu(Menu::LotDeal(LotDeal::EnterDyno));
                // GenericDialog_MED (0x78F2F8) from Customize, the crib's
                // GenericDialog_Small (0x790D74); No focused (0x5587A0's 1,
                // seen under Wine, menus112 d0).
                // The career performance shop's entry asks Customize's
                // question too (Wine, IND_PERF_SHOP_1), No back on it.
                let shop = kind == MenuKind::PerfCategory;
                let (q, package) = if self.save_flow.customize.is_some() || shop {
                    (0x54B6_49F1, "GenericDialog_MED")
                } else {
                    (0xB10A_E56F, "GenericDialog_SMALL")
                };
                let back = if shop {
                    self.perf_category_menu(nfsu2::performance::CATEGORIES.len())
                } else {
                    self.icon_menu(MenuKind::Crib)
                };
                self.dyno.from = if shop {
                    1
                } else if self.save_flow.customize.is_some() {
                    2
                } else {
                    0
                };
                let mut ask = self.ask_in(package, q, yes, back);
                if let Mode::Menu(Menu::Dialog(d, _)) = &mut ask {
                    d.selected = 1;
                }
                // The crib's StartDim (0x96DDA9E6 at 0x4AF790), Customize's
                // (0x55E819); No (0x1FAB5998 → 0x4AF4BB) UNDIMs.
                dialog_over(ask, true, None)
            }
            // Tune Settings (0x53C8A0): UI_PerformanceTuning_Master.
            Action::TuneSettings => self.icon_menu(MenuKind::Tuning),
            // Test Run (0x53C8D0) drives the tuned car on a test track;
            // Mode Select (0x522720's option): not traced yet.
            Action::TestRun => {
                // On the dyno, DynoExit drives the car off first
                // (`exit_dyno_scene`); its end starts the test run.
                if self.exit_dyno_scene() {
                    self.mode = Mode::Explore;
                } else {
                    self.start_test_run()?;
                }
                return Ok(None);
            }
            // Dyno Run (0x53C840): 0x83A7A4 = 1, so the session's reload
            // (0x581830 → 0x526980(1)) puts the car back on the dyno track
            // after a test run.
            Action::DynoRun => {
                if self.dyno.tuning_session && !self.on_dyno_track() {
                    self.sim.paused_race = None;
                    if let Err(e) = self.load_dyno_track() {
                        eprintln!("dyno track: {e:#}");
                    }
                }
                self.dyno_chart(now)
            }
            _ => unreachable!("act_dyno: {action:?}"),
        }))
    }

    /// `act`'s tuning group: modes, categories, save, quit (as `act_main_options`).
    pub(super) fn act_tuning(&mut self, action: Action, now: f32) -> Result<Option<Mode>> {
        Ok(Some(match action {
            // Mode Select (0x5648FA): the working settings go back to their
            // mode (0x50FC20), then UI_PerformanceTuning_Setting.
            // 0x55A590 first: with a mode chosen and the tuning object no
            // longer its working settings (0x59AEA0), "Your current
            // custom-tuned setup has changed. Would you like to store it?"
            // (0xA2323032): Yes 0x2B5DFEF6 stores them (0x50FC20), No
            // 0xB4EDEB6D does not; both open the mode screen.
            Action::ModeSelect => {
                let m = self.dyno.tuning_mode as usize;
                let changed = self
                    .dyno.tuning_object
                    .zip(self.profile.as_ref())
                    .is_some_and(|(t, p)| m < p.tuning.len() && p.tuning[m] != t);
                if changed {
                    self.ask(
                        0xA232_3032,
                        Mode::Menu(Menu::LotDeal(LotDeal::StoreTuning)),
                        self.icon_menu(MenuKind::TuneMode),
                    )
                } else {
                    self.icon_menu(MenuKind::TuneMode)
                }
            }
            // A mode's accept (0xD05FC3A3 in 0x55A750): it becomes the
            // mode (0x83A63C) and its settings the tuning object's
            // (0x50FC60); the first choice goes on to the dyno chart, a
            // later one back to the dyno menu.
            Action::TuneMode(m) => {
                let first = self.dyno.tuning_mode == 0xFF;
                self.dyno.tuning_mode = m;
                // The tuning object's car is set up with them before the
                // sliders are edited, so they are held clamped (**verified**
                // under Wine: the finished-career save's Lancer with raw Circuit
                // bytes c8 00 / c8 64 00 held b6 38 / 96 32 30 after the
                // accept and the first dyno run; when in that sequence the
                // set-up runs is **inferred**).
                self.dyno.tuning_object = self.profile.as_ref().map(|p| self.clamped_tuning(p.tuning[m as usize]));
                if first { self.dyno_chart(now) } else { self.icon_menu(MenuKind::Dyno) }
            }
            // 0x55D9CC → 0x525110(i): 0x13, 0x14, 0x18, 0x19, 0x15, 0x16,
            // 0x17 by the category (table 0x525188), before its screen.
            Action::TuningCategory(i) => {
                if let Some(&kind) = [0x13, 0x14, 0x18, 0x19, 0x15, 0x16, 0x17].get(i) {
                    self.option_line(kind);
                }
                if i < 4 { self.tune_sliders(i, now) } else { Mode::Menu(Menu::TuneGraph(self.tune_graph(i, now))) }
            }
            // Quit Tuning (0x564DE0) asks by game mode: career "Quit to
            // Garage?" (0xE5A0D45E; "Quit to Performance Shop?" 0xB3FB59A9
            // with 0x83A640 set), Car Lot 0xFC820867, otherwise — Customize
            // — "Are you sure you want to quit tuning?" (0x40813F63); Yes
            // (0x34DC1BCF) leaves the screen (0x538CB0), to the categories.
            // Save Profile (react 0x564D70): "Are you sure you want to save
            // your profile?" (0x0D7DE8AB); Yes 0x51123592 → 0x4A3100(0x51).
            // Both over the dyno menu, dimmed (0x96DDA9E6 at 0x564D60 /
            // 0x564DD0); No's 0x251BF83E (0x564AE4) brings it back.
            // Both with No focused (0x5587A0's eighth argument 1 at
            // 0x564D84 / 0x564E4D, as the car lot's and the shops').
            Action::TuningSave => focus_no(dialog_over(
                self.ask(0x0D7D_E8AB, Mode::Menu(Menu::LotDeal(LotDeal::TuningSave)), self.icon_menu(MenuKind::Dyno)),
                true,
                None,
            )),
            Action::QuitTuning => {
                let yes = Mode::Menu(Menu::LotDeal(LotDeal::QuitTuning));
                let q = self.quit_tuning_question();
                focus_no(dialog_over(self.ask(q, yes, self.icon_menu(MenuKind::Dyno)), true, None))
            }
            _ => unreachable!("act_tuning: {action:?}"),
        }))
    }
}
