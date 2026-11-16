use crate::*;

impl Game {
    /// A dialog's answer: the outcome picked (a screen, or a `LotDeal` to carry out);
    /// `outcomes` holds the ones not picked.
    pub(super) fn apply_dialog_outcome(&mut self, then: Mode, outcomes: &mut Vec<Mode>, quitting: bool) -> Result<()> {
        match then {
            Mode::Explore => {
                let at = self.career_world.pending_explore.take().or(self.here());
                self.explore(at)?;
            }
            Mode::Menu(Menu::LotDeal(LotDeal::DeleteTuned { record, at })) => self.delete_tuned_yes(record, at),
            Mode::Menu(Menu::LotDeal(LotDeal::QuitGame)) => {
                self.platform.quit_app = true;
                self.mode = outcomes.pop().unwrap_or_else(|| self.icon_menu(MenuKind::Main));
            }
            Mode::Menu(Menu::LotDeal(LotDeal::FitPart { package, sub, category, i, slot, hash, price, owned })) => {
                self.fit_browser_part(package, sub, category, i, slot, hash, price, owned);
            }
            Mode::Menu(Menu::LotDeal(LotDeal::FitGauge(c))) => self.fit_custom_gauge(&c),
            Mode::Menu(Menu::LotDeal(LotDeal::Convert { sub, category })) => self.carbon_convert(sub, category)?,
            // The garage's Yes: its save point when changed, then
            // 0x4AF607 (career +0x8744 = 1, 0x4AF616): the city
            // start at the kept place, the race's engage place
            // after a race's save (under Wine, the original's
            // first-win save: Resume, the garage, then that
            // place, not the airport).
            Mode::Menu(Menu::LotDeal(LotDeal::LeaveCrib)) => {
                let at = self.career_world.pending_explore.take().or(self.here());
                if self.changed_since_snapshot() {
                    self.save_point(false, AfterSave::LeaveGarage(at))?;
                } else {
                    self.go_on_garage(at)?;
                }
            }
            Mode::Menu(Menu::LotDeal(LotDeal::LeaveShop)) => {
                let at = self.career_world.pending_explore.take().or(self.here());
                if self.changed_since_snapshot() {
                    self.save_point(false, AfterSave::Explore(at))?;
                } else {
                    self.explore(at)?;
                }
            }
            Mode::Menu(Menu::LotDeal(LotDeal::Take { slot, car })) => self.take_car_yes(slot, car)?,
            Mode::Menu(Menu::LotDeal(LotDeal::Browse { car })) => self.mode = self.own_cars(Some(car)),
            Mode::Menu(Menu::LotDeal(LotDeal::Race { event })) => self.start_race(event, false, true)?,
            Mode::Menu(Menu::LotDeal(LotDeal::StoreTuning)) => {
                let m = self.dyno.tuning_mode as usize;
                if let (Some(t), Some(p)) = (self.dyno.tuning_object, self.profile.as_mut())
                    && m < p.tuning.len()
                {
                    p.tuning[m] = t;
                    let _ = p.commit();
                }
                self.mode = self.icon_menu(MenuKind::TuneMode);
            }
            Mode::Menu(Menu::LotDeal(LotDeal::SmsDelete { entry, mut mailbox })) => {
                let now = self.now();
                if let Some(p) = self.profile.as_mut() {
                    p.inbox.delete(entry);
                    mailbox.refresh(&p.inbox, &self.game_data.career.messages, None);
                    mailbox.opened = now;
                }
                if mailbox.rows.is_empty() {
                    self.sms_close();
                } else {
                    self.mode = Mode::Menu(Menu::SmsMailbox(mailbox));
                }
            }
            Mode::Menu(Menu::LotDeal(LotDeal::Gps { target, trigger })) => {
                self.career_world.gps = Some(dday::Gps::engage(target, &trigger, self.car_pose()));
                self.mode = Mode::Explore;
            }
            Mode::Menu(Menu::LotDeal(LotDeal::Resume(race))) => {
                self.resume_from_pause(race)?;
            }
            Mode::Menu(Menu::LotDeal(LotDeal::SaveShowcase(mut preview))) => {
                preview.save_ticks = Some(5);
                self.mode = Mode::Menu(Menu::Showcase(preview));
            }
            Mode::Menu(Menu::LotDeal(LotDeal::SaveCover { milestone, dvd })) => {
                self.mode = self.save_cover(milestone, dvd);
            }
            Mode::Menu(Menu::LotDeal(LotDeal::GpsOff(map))) => {
                self.career_world.gps = None;
                self.mode = Mode::Menu(Menu::InGameMap(*map));
            }
            Mode::Menu(Menu::LotDeal(LotDeal::Confirm(action, kind))) => {
                self.fe.confirmed = true;
                return self.act(action, kind);
            }
            // Quit Tuning's Yes (0x30ED2368 in 0x5647F0): with
            // 0x50FD10 and save prompts on (0x844AD8) the save
            // question (0x71851654 by 0x5587A0 over the dyno
            // menu's own package, No focused; Yes 0x24DD40CF,
            // No 0x1FAB5998, cancel 0x34DC1BCF) and StartDim
            // again (0x5649AC: the handler's DIM, current since
            // the quit question, restarts at t = 0, 0x538C10);
            // Back's 0x34DC1BCF is StopDim (0x56494E): the dyno
            // menu again, undimmed. Else the quit (0x53CC80).
            Mode::Menu(Menu::LotDeal(LotDeal::QuitTuning)) => self.quit_tuning_yes(),
            Mode::Menu(Menu::LotDeal(LotDeal::QuitTuningSave(save))) => self.quit_tuning_save_answer(save)?,
            Mode::Menu(Menu::LotDeal(LotDeal::TuningSave)) => {
                self.save_point(false, AfterSave::Menu(MenuKind::Dyno))?
            }
            Mode::Menu(Menu::LotDeal(LotDeal::Sign(i))) => {
                let sponsor =
                    self.profile.as_ref().and_then(|p| p.sponsor_offers(&self.game_data.career).get(i).map(|s| (*s).clone()));
                // 0x532D00 does not rebuild the open events: the
                // settle's does (under Wine the sponsor races
                // came after the events kept, then S2_URL_1).
                if let (Some(s), Some(p)) = (sponsor, self.profile.as_mut()) {
                    p.sign(&s);
                    let _ = p.commit();
                }
                // Then the post-race chain's next screen (0x4BFF80
                // on 0x8363DC), UI_PostRace — whose exits settle
                // the race (0x54C950). The clone went to the city
                // and left the win unsettled.
                self.mode = self.icon_menu(MenuKind::PostRace);
            }
            Mode::Menu(Menu::LotDeal(LotDeal::EnterDyno)) => self.customize_dyno_yes()?,
            Mode::Menu(Menu::LotDeal(LotDeal::TestRunRestart)) => {
                self.sim.paused_race = None;
                self.start_test_run()?;
            }
            // 0x526ADEFC (0x564A36): help bar off, 0x50FDF0,
            // 0x50F7E0(1), 0x83A644 = 1 and the session's
            // reload in dyno mode (0x5F4250(10)): the dyno
            // menu over the car on the rollers.
            Mode::Menu(Menu::LotDeal(LotDeal::ReturnToDyno)) => {
                self.sim.paused_race = None;
                self.mode = match self.load_dyno_track() {
                    Ok(()) => {
                        let menu = self.icon_menu(MenuKind::Dyno);
                        self.dyno_enter_scene(menu)
                    }
                    Err(e) => {
                        eprintln!("dyno track: {e:#}");
                        self.icon_menu(MenuKind::Dyno)
                    }
                };
            }
            Mode::Menu(Menu::LotDeal(LotDeal::Select { slot })) => {
                if let Some(p) = self.profile.as_mut() {
                    p.select_slot(slot);
                    p.refresh_open(&self.game_data.career, &mut self.rng);
                    let _ = p.commit();
                }
                self.drop_drive();
                self.mode = self.icon_menu(MenuKind::Crib);
            }
            other => self.dialog_other_outcome(other, quitting)?,
        }
        Ok(())
    }

    /// The customized-car delete question answered Yes: the record freed, the car select again.
    fn delete_tuned_yes(&mut self, record: usize, at: usize) {
        // 0x4EF090: the record cleared (0x52A6D0), All
        // Cars (0x83AAC0 = 3), the list rebuilt (0x4EEC90,
        // 0x4B2310) on the next car, or the previous when
        // it was the last (0x4E2590 with 0xB5971BF1 /
        // 0x9120409E before the node goes).
        if let Some(p) = self.profile.as_mut() {
            if let Some(m) = p.my_cars.get_mut(record) {
                *m = None;
            }
            let _ = p.commit();
        }
        let cars = self.qr_cars(3);
        let selected = at.min(cars.len().saturating_sub(1));
        self.mode = Mode::Menu(Menu::CarLot(CarLot {
            title: String::new(),
            cars,
            selected,
            opened: self.now(),
            state: LotState::Customize,
            stock: None,
            filter: 3,
            arrows: Default::default(),
        }));
    }

    /// The car lot's take answered Yes: the car in the slot, then the city (through the save point).
    fn take_car_yes(&mut self, slot: usize, car: String) -> Result<()> {
        let at = self.here();
        if let Some(p) = self.profile.as_mut() {
            p.take_stock_car(slot, &car);
            p.refresh_open(&self.game_data.career, &mut self.rng);
            let _ = p.commit();
        }
        self.drop_drive();
        // The lot's state 6 (0x4ED8A0, 0x4ED945): the save
        // point (MC_Main mode 0x141) when the profile
        // changed since the lot opened (0x4EDADE's
        // snapshot, 0x52A4D0), else 0x9A3B571D; then the
        // city (0x5404B0).
        // Under Wine: "Are you sure you want to save the
        // profile named NAME?" right after the car.
        if self.changed_since_snapshot() {
            self.save_point(false, AfterSave::Explore(at))?;
        } else {
            self.explore(at)?;
        }
        Ok(())
    }

    /// Quit to Garage answered Yes: the changed-setups save question, or out of tuning.
    fn quit_tuning_yes(&mut self) {
        if self.tuning_changed() && self.settings.save_prompts && self.profile.as_ref().is_some_and(|p| !p.scratch) {
            let text = self.label(0x7185_1654, "");
            let buttons = [self.label(0xF770, "Yes"), self.label(0x061C, "No")];
            let mut d = Dialog::new(
                screens::dialog_package(&self.game_data.assets, &text),
                &text,
                &[&buttons[0], &buttons[1]],
                self.now(),
            );
            d.selected = 1;
            d.cancel = Some(2);
            let menu = self.icon_menu(MenuKind::Dyno);
            self.mode = dialog_over(
                Mode::Menu(Menu::Dialog(
                    d,
                    vec![
                        Mode::Menu(Menu::LotDeal(LotDeal::QuitTuningSave(true))),
                        Mode::Menu(Menu::LotDeal(LotDeal::QuitTuningSave(false))),
                        menu,
                    ],
                )),
                true,
                None,
            );
        } else {
            self.quit_tuning();
            self.mode = self.after_tuning();
        }
    }

    /// Any other outcome: the screen it names (after the quit and its save point when quitting).
    fn dialog_other_outcome(&mut self, other: Mode, quitting: bool) -> Result<()> {
        if quitting {
            // UI_PostRace's Quit to Main Menu (OK
            // 0x30ED2368 → 0x24DD40CF): after a career
            // race won the outcome is settled (0x54C950)
            // and the same save point runs (mode 0x441),
            // then the quit (0xD78D7BF1 → 0x4A3E80).
            let from_post = std::mem::take(&mut self.save_flow.quit_post_race);
            if from_post {
                // 0x4A3E80 → 0x50B8C0(UI_postrace, 0).
                self.race_flow.post_race_selection = 0;
            }
            let save = from_post && self.post_race_save_due();
            self.after_quit()?;
            if save {
                self.save_point(false, AfterSave::Menu(MenuKind::Main))?;
                return Ok(());
            }
        }
        self.mode = other;
        Ok(())
    }

    /// The changed-setups question answered: Yes stores the setup and saves, No leaves tuning.
    fn quit_tuning_save_answer(&mut self, save: bool) -> Result<()> {
        if save {
            // Commit, save over the dyno, then quit.
            let m = self.dyno.tuning_mode as usize;
            if let (Some(t), Some(p)) = (self.dyno.tuning_object, self.profile.as_mut())
                && m < p.tuning.len()
            {
                p.tuning[m] = t;
                let _ = p.commit();
            }
            self.save_point(false, AfterSave::QuitTuning)?;
        } else {
            self.quit_tuning();
            self.mode = self.after_tuning();
        }
        Ok(())
    }
}
