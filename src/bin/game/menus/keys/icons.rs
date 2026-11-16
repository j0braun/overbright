use crate::*;

impl Game {
    pub(super) fn icons_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Icons(m, kind)) = &mut self.mode else { unreachable!() };
        let kind = *kind;
        let at = m.selected;
        if key == Key::Back {
            return self.icons_back(kind, at);
        }
        if kind == MenuKind::Graphics && key == Key::Button1 {
            return self.graphics_cancel_key();
        }
        if kind == MenuKind::VinylLayer && matches!(key, Key::LTrigger | Key::RTrigger | Key::Button1) {
            return self.vinyl_layer_key(key);
        }
        let action = m.key(key, now);
        if kind == MenuKind::Main {
            self.fe.main_selection = m.selected + 1;
        }
        if kind == MenuKind::PostRace {
            self.race_flow.post_race_selection = m.selected + 1;
        }
        if let Some(action) = action {
            self.act(action, kind)?;
        }
        Ok(())
    }

    /// Back on an icon strip: the screen it goes back to, by kind.
    fn icons_back(&mut self, kind: MenuKind, at: usize) -> Result<()> {
        self.mode = match kind {
            MenuKind::Main => return Ok(()),
            // UI_OptionsMain's Back (0x4B6010): with save
            // prompts on, the save point (0x4A3100 mode 0x2042)
            // when the profile changed since the Options opened
            // or the prompts option did; the snapshot dropped.
            // **Verified** under Wine (a volume changed: "Are you
            // sure you want to save the profile named NAME?").
            // With no profile the same save point's operation 0x40
            // asks "Would you like to create a new profile?"
            // (0x4F33A0 → 0x4E8D30(2)); mode 0x2042's low nibble 2
            // switches to UI_Main whatever the answer (0x4A31D0 →
            // 0x537990). **Observed** under Wine (L2, No Profile,
            // a volume changed): the question; No → the main menu;
            // Yes → the name → "Are you sure you want to save the
            // profile named NAME?" → No → the main menu, NAME.
            MenuKind::Options => {
                let prompts = self.settings.save_prompts;
                let entry = self.fe.options_settings.take();
                let changed = prompts
                    && (self.changed_since_snapshot()
                        || (self.profile.is_none() && entry.is_some_and(|e| e != self.settings)));
                self.drop_snapshot();
                if prompts && (changed || self.fe.options_prompts != prompts) {
                    if self.profile.is_none() {
                        self.save_point_create(AfterSave::Menu(MenuKind::Main));
                    } else {
                        self.save_point(false, AfterSave::Menu(MenuKind::Main))?;
                    }
                    return Ok(());
                }
                self.icon_menu(MenuKind::Main)
            }
            MenuKind::Career | MenuKind::QrMode | MenuKind::Profile => self.icon_menu(MenuKind::Main),
            MenuKind::Rewards => self.icon_menu(MenuKind::Crib),
            MenuKind::Covers(_) | MenuKind::SignedSponsors => self.icon_menu(MenuKind::Rewards),
            MenuKind::Dyno => {
                let yes = Mode::Menu(Menu::LotDeal(LotDeal::QuitTuning));
                let q = self.quit_tuning_question();
                dialog_over(self.ask(q, yes, self.icon_menu(MenuKind::Dyno)), true, None)
            }
            MenuKind::Tuning => self.icon_menu(MenuKind::Dyno),
            // 0x911AB364 in 0x55A750: with no mode chosen
            // "Quit to Garage?", else back to the dyno menu.
            MenuKind::TuneMode if self.dyno.tuning_mode == 0xFF => {
                let yes = Mode::Menu(Menu::LotDeal(LotDeal::QuitTuning));
                let q = self.quit_tuning_question();
                // StartDim at 0x55A90F (no event: unchanged).
                dialog_over(self.ask(q, yes, self.icon_menu(MenuKind::TuneMode)), true, None)
            }
            MenuKind::TuneMode => self.icon_menu(MenuKind::Dyno),
            // Leaving Customize: the set-aside profile back,
            // and the car customized becomes the main menu's
            // (0x55BE00 → 0x521110: 0x83AAB8[player] = its
            // name; the block is saved with the profile).
            MenuKind::CustomizeCategory if self.shop.garage_parts && self.save_flow.customize.is_none() => {
                self.shop.garage_parts = false;
                self.crib_menu_on(Action::ChangeParts)
            }
            MenuKind::CustomizeCategory => match self.customize_category_back()? {
                Some(mode) => mode,
                None => return Ok(()),
            },
            MenuKind::Crib | MenuKind::ExplorePause => return self.garage_back(kind),
            // 0x4A7D60 on Back (0x911AB364 → 0xE1FDE1D1):
            // UI_Pause.fng (UI_PerformanceDyno_MAIN in a test
            // run, 0x89E7E6).
            MenuKind::PauseOptions(true) if self.sim.paused_race.as_ref().is_some_and(|r| r.test.is_some()) => {
                self.icon_menu(MenuKind::Dyno)
            }
            MenuKind::PauseOptions(race) => {
                self.icon_menu(if race { MenuKind::RacePause } else { MenuKind::ExplorePause })
            }
            MenuKind::RacePause => match self.sim.paused_race.take() {
                Some(r) => Mode::Race(r),
                None => self.icon_menu(MenuKind::Main),
            },
            MenuKind::PostRace => return Ok(()),
            // The offer stays for the next race.
            MenuKind::Sponsor => self.icon_menu(MenuKind::PostRace),
            // Back to the body shop on rims (0x839BC0).
            // UI_ChoosePerformanceCategory's Back (0x911AB364
            // in 0x55C530): in a career shop (0x83898C = 0)
            // "Are you sure you wish to exit back to Explore
            // Mode?" first (`shop_exit_question`, 0x55C715).
            MenuKind::PerfCategory if self.shop.garage_parts => self.storage_menu(1),
            MenuKind::PerfCategory => {
                if self.career_shop() {
                    // The strip stays on its entry under the
                    // question (Wine: Turbo).
                    let again = self.perf_category_menu(at);
                    self.shop_exit_question(again)
                } else {
                    self.leave_shop()?;
                    return Ok(());
                }
            }
            MenuKind::Graphics => {
                self.leave_graphics()?;
                return Ok(());
            }
            MenuKind::VinylLayer => self.graphics_menu(1),
            MenuKind::PerfPackage => {
                let at = nfsu2::performance::CATEGORIES.iter().position(|c| c.0 == self.shop.perf_category).unwrap_or(0);
                self.perf_category_menu(at)
            }
            MenuKind::RimBrand => {
                let at = screens::BODY_SHOP_CATEGORIES.iter().position(|c| c.0 == 0x1D).unwrap_or(0);
                self.new_parts_browser(at)
            }
        };
        Ok(())
    }

    /// The graphics shop's Cancel Changes (Button1).
    fn graphics_cancel_key(&mut self) -> Result<()> {
        let Mode::Menu(Menu::Icons(m, _)) = &mut self.mode else { unreachable!() };
        // Cancel Changes (0xC519BFC0, not on the setup option):
        // "Would you like to cancel changes to your vinyl /
        // decal / paint setup?" (0x6EE64972, 0x449A9DD9,
        // 0x1A311489), Yes → 0x561A50 by the option's kind.
        let at = m.selected;
        let cancel = match at {
            0 => Some((2, 0x1A31_1489)),
            1 => Some((0, 0x6EE6_4972)),
            2 => Some((1, 0x449A_9DD9)),
            _ => None,
        };
        if let Some((kind_of, question)) = cancel {
            let (yes, no) = self.yes_no();
            let (a, b) = (self.graphics_menu(at), self.graphics_menu(at));
            self.mode = self.setup_dialog(question, vec![(yes, Some(SetupStep::Cancel(kind_of)), a), (no, None, b)]);
        }
        Ok(())
    }

    /// The vinyl layers' Change Color (Button1) and Reorder Layers (triggers).
    fn vinyl_layer_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::Icons(m, _)) = &mut self.mode else { unreachable!() };
        let at = m.selected;
        let layer = 0x40 + at as u8;
        let unique = self.fitted_unique() || self.setup_unique();
        if key == Key::Button1 {
            // Change Color (0xC519BFC0): a vinyl that takes
            // colours, no unique one on.
            let vinyl =
                self.effective(layer).filter(|&v| self.part_attr(0x40, v, graphics::REMAP).is_some_and(|r| r != 0));
            if let (Some(v), false) = (vinyl, unique) {
                self.shop.vinyl_layer = layer;
                self.shop.vinyl = Some(v);
                self.shop.vinyl_pending.clear();
                self.mode = self.vinyl_colours(0);
            }
            return Ok(());
        }
        // Reorder Layers (0x9803F6E2 → 0x5632A0): the layer's
        // vinyl swaps with the one below (left trigger) or
        // above, never with a unique one on; the selection
        // follows it.
        let other =
            if key == Key::RTrigger { (layer < 0x43).then(|| layer + 1) } else { (layer > 0x40).then(|| layer - 1) };
        let movable = self.fitted(layer).is_some() || self.shop.setup.get(layer).is_some_and(|h| h != 0);
        if let (Some(o), true, false) = (other, movable, unique) {
            let installed = self.profile.as_ref().map(|p| p.installed.clone()).unwrap_or_default();
            self.shop.setup.swap_layers(layer, o, &|s| installed.get(&s).copied());
            self.mode = self.vinyl_layer_menu((o - 0x40) as usize);
        }
        Ok(())
    }

    /// Back from Customize's categories: the set-aside profile back and the car kept;
    /// the screen to show, or None when a save point or the car select took over.
    fn customize_category_back(&mut self) -> Result<Option<Mode>> {
        let mut no_profile_car = None;
        if let Some(p) = self.save_flow.customize.take() {
            let custom = std::mem::replace(&mut self.profile, p);
            if self.profile.is_none() {
                no_profile_car = custom.as_ref().map(|c| career::MyCar {
                    id: career::tuned_car_id(0),
                    car: c.car.clone(),
                    installed: c.installed.clone(),
                    perf: c.perf.clone(),
                });
            }
            if let (Some(c), Some(p)) = (custom, self.profile.as_mut()) {
                // The record takes the session's car; 0x521110:
                // a record the session made that still has
                // every stock part (0x50E900 compares the 0xAA
                // slots) is freed and player 0's car is the
                // stock one again.
                match self.save_flow.customize_car.take() {
                    Some((k, true)) if c.installed.is_empty() => {
                        p.my_cars[k] = None;
                        p.menu_car_id = Some(career::stock_id(&c.car));
                    }
                    Some((k, _)) => {
                        if let Some(m) = p.my_cars.get_mut(k).and_then(Option::as_mut) {
                            m.installed = c.installed.clone();
                            m.perf = c.perf.clone();
                        }
                    }
                    None => {}
                }
                p.menu_car = Some((c.car, c.installed));
                p.commit()?;
            }
        }
        // 0x55BE00 back to UI_QRCarSelect: 0x52A4D0 sets
        // the session latch 0x838540; from Quick Race
        // (0x83898C = 2) a change asks first (0x4A3100
        // mode 0x2042); main-menu Customize goes back to
        // its car select (0x5377B0).
        // No profile: the memory state against the
        // session's start (the record 0x521110 frees
        // when every part is stock compares equal).
        let entry = self.save_flow.customize_entry.take();
        let changed = self.changed_since_snapshot()
            || no_profile_car
                .as_ref()
                .is_some_and(|c| entry.is_some_and(|e| e != (c.installed.clone(), c.perf.clone())));
        self.save_flow.customize_changed |= changed;
        if self.quick.customize.is_some() {
            // With no profile the save point (0x2042)
            // asks to create one; the answer goes on to
            // the car select either way (mode 2), and a
            // profile made keeps the car (**observed**
            // under Wine, L2: Quick Race → 206 →
            // Customize → a front bumper → Back → Yes →
            // NAME → No: the car select on the tuned 206
            // at the list's end, NAME current).
            if changed && self.profile.is_none() {
                self.save_flow.pending_car = no_profile_car;
                self.save_point_create(AfterSave::CarSelect);
            } else if changed {
                self.save_point(false, AfterSave::CarSelect)?;
            } else {
                self.go_on(AfterSave::CarSelect)?;
            }
            return Ok(None);
        }
        Ok(Some(self.customize_select()))
    }

    /// Back from the garage or the free-roam pause: the city (through the save point).
    fn garage_back(&mut self, kind: MenuKind) -> Result<()> {
        // Leaving the garage: 0x4AF360's save point
        // (mode 0x151) when changed, then 0x9A3B571D
        // (0x4AF607): career +0x8744 = 1 and the city
        // start (0x5404B0) at the place kept on the way
        // in (0x54CB00 / 0x513B30, the car's place) or
        // at the last one kept (`AfterSave::LeaveGarage`).
        // The pause sits over the running city: Back
        // resumes it (no track load); the garage is a
        // city start.
        if kind == MenuKind::ExplorePause {
            self.back_to_city()?;
        } else if self.changed_since_snapshot() {
            self.save_point(false, AfterSave::LeaveGarage(self.here()))?;
        } else {
            self.go_on_garage(self.here())?;
        }
        Ok(())
    }
}
