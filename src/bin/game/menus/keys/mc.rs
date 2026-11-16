use crate::*;

impl Game {
    pub(super) fn mc_list_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::McList(l)) = &mut self.mode else { unreachable!() };
        match l.key(key) {
            Some(Some(name)) => {
                let op = l.op;
                self.mode = self.mc_pick(op, name);
            }
            Some(None) => {
                let to = if std::mem::take(&mut self.fe.from_boot) { MenuKind::Main } else { MenuKind::Profile };
                self.mode = self.icon_menu(to);
            }
            None => {}
        }
        Ok(())
    }

    pub(super) fn mc_message_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::McMessage(m, acts)) = &mut self.mode else { unreachable!() };
        if let Some(b) = m.key(key) {
            if acts.is_empty() {
                return Ok(());
            }
            let act = acts.swap_remove(b.min(acts.len() - 1));
            self.mc_act(act)?;
        }
        Ok(())
    }

    pub(super) fn mc_main_create_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::McMain(m)) = &mut self.mode else { unreachable!() };
        match m.key(key, now) {
            // A save point's create question (state 2): No (or Escape
            // back out of it) sends the caller 0x8867412D; Customize's
            // dyno entry answers with the OK message 0x60D24D65 (0x55E71F
            // → 0x558550; fallback 0xC01A6F63) over its categories.
            Some(McOutcome::Cancel) if !matches!(self.save_flow.mc_create_then, Some(AfterSave::CustomizeDyno)) => {
                // Mode 0x2042: on to the next screen (0x4A31D0).
                self.save_flow.pending_car = None;
                let then = self.save_flow.mc_create_then.take().unwrap_or(AfterSave::Menu(MenuKind::Main));
                self.go_on(then)?;
            }
            Some(McOutcome::Cancel) => {
                self.save_flow.mc_create_then = None;
                let text = self.label(0x60D2_4D65, "");
                let menu = self.icon_menu(MenuKind::CustomizeCategory);
                self.mode = dialog_over(self.dialog(&text, menu), true, None);
            }
            // The name given: the profile is current (0x502F40); Yes to
            // "save the profile named …?" saves it (state 6, the
            // library's overwrite question when the file exists), No
            // leaves with 0x8867412D, which mode 0x1000 with a named
            // profile turns into 0x461A18EE (0x4FD5EE): the caller goes
            // on either way.
            Some(McOutcome::Done { name, save }) => {
                let then = self.save_flow.mc_create_then.take().unwrap_or(AfterSave::CustomizeDyno);
                let name = profiles::sanitize(&name);
                profiles::set_current(&profiles::resolve(&name), nfsu2::formats::native_save::NativeSave::fresh(&name));
                let mut p = Profile::new(&name);
                p.cash = nfsu2::frontend::cheats::creation_cash(m.replacing, self.fe.boot_cash);
                let dyno = matches!(then, AfterSave::CustomizeDyno);
                // The car the customize session made with no profile
                // stays in the memory's My Cars (record 0, 0x52A720),
                // which the new profile is: it keeps it.
                let car = if dyno {
                    self.profile.as_ref().filter(|c| c.scratch).map(|c| career::MyCar {
                        id: career::tuned_car_id(0),
                        car: c.car.clone(),
                        installed: c.installed.clone(),
                        perf: c.perf.clone(),
                    })
                } else {
                    self.save_flow.pending_car.take()
                };
                if let Some(car) = car {
                    p.menu_car_id = Some(car.id);
                    p.menu_car = Some((car.car.clone(), car.installed.clone()));
                    p.my_cars[0] = Some(car);
                    if dyno {
                        // The session's Back updates that record.
                        self.save_flow.customize_car = Some((0, true));
                    }
                }
                if !dyno {
                    // Options / Customize Back (0x2042): the new
                    // profile is current, saved when asked to, then the
                    // next screen.
                    self.profile = Some(p);
                    if save {
                        self.mc_act(McAct::SpOverwrite(then))?;
                    } else {
                        self.go_on(then)?;
                    }
                    return Ok(());
                }
                // The session's profile stays the Customize scratch; the
                // new one is the profile set aside (`AfterSave::
                // CustomizeDyno` puts the scratch back).
                self.dyno.scratch = self.profile.replace(p);
                if save {
                    self.mc_act(McAct::SpOverwrite(then))?;
                } else {
                    self.go_on(then)?;
                }
            }
            None => {}
        }
        Ok(())
    }

    pub(super) fn mc_main_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::McMain(m)) = &mut self.mode else { unreachable!() };
        match m.key(key, now) {
            // 0x8867412D: back to UI_StartCareer (0x4A31D0 pops MC_Main
            // and sends the caller 0x251BF83E).
            Some(McOutcome::Cancel) => {
                let back = if m.career {
                    MenuKind::Career
                } else if std::mem::take(&mut self.fe.from_boot) {
                    MenuKind::Main
                } else {
                    MenuKind::Profile
                };
                self.mode = self.icon_menu(back);
            }
            // 0x461A18EE: UI_StartCareer starts the career (0x495A30).
            // The Profile Manager's / the boot's create: the new
            // profile current, saved when asked to, then the menu.
            Some(McOutcome::Done { name, save }) if !m.career => {
                let name = profiles::sanitize(&name);
                // The file the name opens: an existing folder of any case
                // (`profiles::resolve`); the payload keeps the typed name.
                profiles::set_current(&profiles::resolve(&name), nfsu2::formats::native_save::NativeSave::fresh(&name));
                let mut p = Profile::new(&name);
                p.cash = nfsu2::frontend::cheats::creation_cash(m.replacing, self.fe.boot_cash);
                self.profile = Some(p);
                self.settings = options::Settings::load();
                // The profile's EA Trax list in force (0x5344E0 / 0x515AA0 →
                // 0x479220): the pools refill, the song goes on.
                if let Some(a) = self.audio.as_mut() {
                    a.streams.reset_playlist();
                }
                self.apply_pad_maps();
                let back = if self.fe.from_boot { MenuKind::Main } else { MenuKind::Profile };
                self.mode = if save {
                    self.mc_act_mode(McAct::SaveAsk(name))
                } else {
                    self.fe.from_boot = false;
                    self.icon_menu(back)
                };
            }
            Some(McOutcome::Done { name, save }) => {
                self.quick.car = None;
                self.quick.parts = None;
                let mut p = Profile::new(&name);
                p.cash = nfsu2::frontend::cheats::creation_cash(m.replacing, self.fe.boot_cash);
                p.start_career(&self.game_data.career);
                if save {
                    let _ = p.commit();
                }
                self.profile = Some(p);
                self.drop_drive();
                self.career_intro()?;
            }
            None => {}
        }
        Ok(())
    }
}
