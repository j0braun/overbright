use crate::*;

impl Game {
    /// UI_QRCarSelect's Accept in game mode 0x20, or Customize in Quick
    /// Race (game mode 2, BUTTON1; `qr` the car select to go back to):
    /// a free My Cars record is needed (0x5165E0), else the "over the
    /// limit" dialog (0x558580 with 0x91921A8D) over the car select; then
    /// the save-point snapshot (0x552CD0: with save prompts on, unless the
    /// session latch 0x838540 is set) and 0x552CD0(3 / 2, …): a stock car
    /// gets a new record, TUNED_CAR_n in the first free one (0x52A720 →
    /// 0x516B90), a My Cars record is customized itself; player 0's car
    /// becomes it (0x502A40). The car is customized on an unsaved profile
    /// of its own, which lists parts by the Quick Race rule (0x50F0D0 flag
    /// 1, `Profile::part_listed`) and shows no prices.
    pub(crate) fn customize_car_select(&mut self, choice: CarChoice, again: Mode, qr: Option<CarLot>) {
        let free = self.profile.as_ref().and_then(|p| p.my_cars_free());
        // Quick Race's Customize tests for a free record only on a stock
        // car (0x4FCAFC: 0x516360, then 0x5165E0); a My Cars record is
        // customized itself. Customize's Accept always tests (0x4FC308).
        let tested = qr.is_none() || choice.my_car.is_none();
        let Some(free) = free.or((self.profile.is_none() || !tested).then_some(0)) else {
            let text = self.label(0x9192_1A8D, "You have reached the limit of tuned cars.");
            self.mode = self.dialog(&text, again);
            return;
        };
        if !self.save_flow.customize_changed {
            self.take_snapshot(false);
        }
        let (k, made) = match choice.my_car {
            Some(k) => (k, false),
            None => (free, true),
        };
        let mine = choice.my_car.and_then(|k| self.profile.as_ref()?.my_cars.get(k)?.clone());
        if let Some(p) = self.profile.as_mut() {
            if made {
                p.my_cars[k] =
                    Some(career::MyCar { id: career::tuned_car_id(k), car: choice.name.clone(), ..Default::default() });
            }
            p.menu_car_id = Some(career::tuned_car_id(k));
        }
        self.save_flow.customize_car = self.profile.is_some().then_some((k, made));
        self.quick.customize = qr;
        // The help bar keeps the loaded profile's name (Wine, the
        // finished-career save's tuned 350Z in main-menu Customize: its name).
        let mut p = Profile::new(self.profile.as_ref().map_or("Player", |c| c.name.as_str()));
        p.scratch = true;
        // The unlock tests read the global career (0x515360, 0x500B80):
        // the loaded profile's progress.
        if let Some(c) = &self.profile {
            p.stage = c.stage;
            p.completed = c.completed.clone();
            p.milestones = c.milestones.clone();
        }
        p.car = choice.name.clone();
        p.slots[0].car = Some(p.car.clone());
        if let Some(m) = mine {
            p.installed = m.installed;
            p.perf = m.perf;
        }
        // No profile: the memory state's snapshot, the session car as it
        // starts (0x552CD0 with save prompts on).
        self.save_flow.customize_entry = (self.profile.is_none() && self.settings.save_prompts && !self.save_flow.customize_changed)
            .then(|| (p.installed.clone(), p.perf.clone()));
        self.save_flow.pending_car = None;
        self.save_flow.customize = Some(self.profile.replace(p));
        self.drop_drive();
        self.mode = self.icon_menu(MenuKind::CustomizeCategory);
    }

    /// Main-menu Customize's car select (UI_QRCarSelect in game mode
    /// 0x20, 0x4AED30), on player 0's car (the constructor 0x4F9D90 keeps
    /// 0x83AAB8 at +0x1A8 and the list starts on it; under Wine it opened
    /// on the profile's My Cars record).
    pub(crate) fn customize_select(&self) -> Mode {
        let cars = self.qr_cars(3);
        let id = self.profile.as_ref().and_then(|p| p.menu_car_id);
        let selected = id
            .and_then(|id| {
                cars.iter().position(|c| match c.my_car {
                    Some(k) => career::tuned_car_id(k) == id,
                    None => career::stock_id(&c.name) == id,
                })
            })
            .unwrap_or(0);
        Mode::Menu(Menu::CarLot(CarLot {
            title: String::new(),
            cars,
            selected,
            opened: self.now(),
            state: LotState::Customize,
            stock: None,
            filter: 3,
            arrows: Default::default(),
        }))
    }

    /// A browser part bought (in a career, unowned, by 0x514280's price)
    /// and fitted, then the browser again on it (the parts browsers'
    /// accept; the rims' and spinners' Yes, 0x09F00F07 / 0x51123592).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fit_browser_part(
        &mut self,
        b_package: &'static str,
        sub: Option<u8>,
        category: usize,
        i: usize,
        slot: u8,
        hash: u32,
        price: i32,
        owned: bool,
    ) {
        if let Some(p) = self.profile.as_mut() {
            // Buying (0x514280's price) then fitting it; outside
            // a career (0x566DD5) only fitting it (0x51123592).
            if !owned && !p.scratch {
                if (p.cash as i32) < price {
                    return;
                }
                p.cash -= price as u32;
                p.own_part(slot, hash);
                // 0x528A00: the current slot's spend (+0x7F0) grows by it.
                p.spent = p.spent.wrapping_add(price.max(0) as u32);
            }
            // 0x55C240 (the rims' 0x567BD3 and spinners' 0x569780 fit 0x1D
            // and 0x1E themselves): fitted, the ride stored.
            let before = p.installed.clone();
            p.fit_visual(&self.game_data.parts, slot, hash);
            p.store_visual(&self.game_data.parts, &self.game_data.career.packages, &before);
            self.drop_drive();
            // The body shop's Select (0x5664A0 → 0x5671A9), the
            // specialties' and neon lists' Yes (0x568DA5 / 0x568E0B,
            // 0x56A391 / 0x56A582): 0x521290 on the part fitted.
            // The rims and spinners are not among 0x521290's callers.
            if matches!(b_package, "UI_GenericParts_Browser" | "UI_ICEMAIN" | "UI_CustomNeonMain") {
                self.install_sound(slot);
            }
        }
        // The DVD covers the new rating reaches (0x5331D0).
        let rating = self.visual_rating();
        if let Some(p) = self.profile.as_mut() {
            p.offer_covers(&self.game_data.career, rating);
            let _ = p.commit();
        }
        // After a Yes every parts browser closes its list and shows its
        // categories again, on the category: UI_GenericParts_Browser
        // 0x5665AA → 0x55EF90(0) (+0x1D9 = 0, the strip back); the
        // specialties' and neon overlays set +0x114 / +0x115 (0x568E0B,
        // 0x56A3AB), their next tick removes them and sends 0x8ACAA8DF to
        // UI_ICEMAIN / UI_CustomNeonMain (0x5692BD, 0x56A7E5), which show
        // their strips again (0x568A04, 0x56A8E6); UI_Rims_Browser and
        // UI_ChooseSpinner leave the sizes for the styles (0x567C5B,
        // 0x567D1C, 0x5697C0, 0x56987B → 0x55FCF0(0), +0x214 = 0) — the
        // same step as their Back from the sizes (0x55FCF0(1)). The
        // garage's storage (0x83898C; the body shop's 0x51123592 with
        // +0x1D7 set leaves through 0x54F990 instead) is not this rule and
        // keeps its list here (**open**).
        let storage = self.shop.garage_parts && b_package == "UI_GenericParts_Browser";
        let back = |i: usize| if storage { Some(i) } else { None };
        self.mode = if b_package == "UI_ICEMAIN" && sub.is_some() {
            self.audio_browser(sub == Some(0x23), category, None)
        } else if b_package == "UI_CustomNeonMain" {
            self.neon_browser(category, None)
        } else if b_package == "UI_ICEMAIN" {
            self.specialties_browser(category, None)
        } else if b_package == "UI_Rims_Browser" {
            self.rims_browser(sub.unwrap_or(0) as usize, Some(category), None)
        } else if b_package == "UI_ChooseSpinner" {
            self.spinner_browser(Some(category), None)
        } else {
            self.parts_browser_in(sub, category, back(i))
        };
    }

    /// UI_Rims_Browser's / UI_ChooseSpinner's size accept (0x559AC0,
    /// **verified**): the fitted rim (0x610300(0x1D)) only says "The part
    /// you are trying to apply is already installed!" (0xDA15D7E5); in a
    /// career (0x850078 = 1) an unowned rim above level 0 (+5 & 0xE0) costs
    /// more than the bank: "You do not have enough bank…" (0xD1AC60BE),
    /// else "Would you like to purchase this part?" (0x44B7167F, Yes
    /// 0x09F00F07); owned, level 0 or outside a career "Would you like to
    /// install this part now?" (0xF5AC2B2A, Yes 0x51123592). All on
    /// GenericDialog_SMALL, No / Back 0x1FAB5998 (No focused), the strip's
    /// fade off (0x52EBA0(0)) and StartDim (0x559C10).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rim_question(
        &self,
        package: &'static str,
        sub: Option<u8>,
        category: usize,
        i: usize,
        slot: u8,
        hash: u32,
        price: i32,
        owned: bool,
        installed: bool,
    ) -> Mode {
        let again = if package == "UI_Rims_Browser" {
            self.rims_browser(sub.unwrap_or(0) as usize, Some(category), Some(i))
        } else {
            self.spinner_browser(Some(category), Some(i))
        };
        let career = self.profile.as_ref().is_some_and(|p| !p.scratch) && self.career_shop();
        if installed {
            let text = self.label(0xDA15_D7E5, "The part you are trying to apply is already installed!");
            let d = Dialog::new("GenericDialog_SMALL", &text, &[&self.label(0x0639, "OK")], self.now());
            return dialog_over(Mode::Menu(Menu::Dialog(d, vec![again])), true, None);
        }
        let buy = career && !owned;
        if buy && self.profile.as_ref().is_some_and(|p| (p.cash as i32) < price) {
            let text = self.label(0xD1AC_60BE, "You do not have enough bank to purchase this item.");
            let d = Dialog::new("GenericDialog_SMALL", &text, &[&self.label(0x0639, "OK")], self.now());
            return dialog_over(Mode::Menu(Menu::Dialog(d, vec![again])), true, None);
        }
        let question = if buy { 0x44B7_167F } else { 0xF5AC_2B2A };
        let yes = Mode::Menu(Menu::LotDeal(LotDeal::FitPart { package, sub, category, i, slot, hash, price, owned }));
        dialog_over(self.ask_in("GenericDialog_SMALL", question, yes, again), true, None)
    }

    /// UI_GenericParts_Browser's Select / Accept on a part in its list
    /// (0x5664A0 → 0x566D98, **verified**; the same block for 0x0C407210
    /// and 0x406415E3, so the mouse's MINI_MASTER Select asks as well):
    /// the fitted part (0x50F1B0) only says "The part you are trying to
    /// apply is already installed!" (0xDA15D7E5); in a career (0x850078 =
    /// 1) an unowned part (0x521BF0) above level 0 (+5 & 0xE0) costing more
    /// than the bank (0x514280 against 0x861E74, `jge`) "You do not have
    /// enough bank…" (0xD1AC60BE, GenericDialog_SMALL), else "Would you
    /// like to purchase this part?" (0x44B7167F, Yes 0x09F00F07); owned,
    /// level 0 or outside a career "Would you like to install this part
    /// now?" (0xF5AC2B2A, Yes 0x51123592). The notices by 0x558440 and the
    /// questions by 0x5587A0 (Yes / No, No focused) on the package sized
    /// by the text (0x783FE5 = ""), except the bank's; the browser's masks
    /// off (0x53A2C0), its strips' fade off (0x52EBA0(0)) and DIM on its
    /// Event Handler (0x566748 → 0x537C10(…, 0x47FF4E7C, "DIM")).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn body_part_question(
        &self,
        sub: Option<u8>,
        category: usize,
        i: usize,
        slot: u8,
        hash: u32,
        price: i32,
        owned: bool,
        installed: bool,
    ) -> Mode {
        let package = "UI_GenericParts_Browser";
        let again = || self.parts_browser_in(sub, category, Some(i));
        let sized = |text: &str| screens::dialog_package(&self.game_data.assets, text);
        let career = self.profile.as_ref().is_some_and(|p| !p.scratch) && self.career_shop();
        if installed {
            let text = self.label(0xDA15_D7E5, "The part you are trying to apply is already installed!");
            let d = Dialog::new(sized(&text), &text, &[&self.label(0x0639, "OK")], self.now());
            return dialog_over(Mode::Menu(Menu::Dialog(d, vec![again()])), true, None);
        }
        let level = self.car_part(slot, hash).map_or(0, |p| p.level);
        let buy = career && !owned && level != 0;
        if buy && self.profile.as_ref().is_some_and(|p| (p.cash as i32) < price) {
            let text = self.label(0xD1AC_60BE, "You do not have enough bank to purchase this item.");
            let d = Dialog::new("GenericDialog_SMALL", &text, &[&self.label(0x0639, "OK")], self.now());
            return dialog_over(Mode::Menu(Menu::Dialog(d, vec![again()])), true, None);
        }
        let question = if buy { 0x44B7_167F } else { 0xF5AC_2B2A };
        let yes = Mode::Menu(Menu::LotDeal(LotDeal::FitPart { package, sub, category, i, slot, hash, price, owned }));
        let text = self.label(question, "?");
        dialog_over(self.ask_in(sized(&text), question, yes, again()), true, None)
    }

    /// The specialties' and the neon's lists: Select / Accept on a part in
    /// UI_IcePartsOverlay (handler 0x568D40, the block at 0x568E2F for
    /// 0x0C407210 and 0x406415E3; UI_ICEMAIN forwards its Select there,
    /// 0x568952) or UI_NeonPartsOverlay (0x56A310, 0x56A3CB; **traced**):
    /// the fitted part (0x50F1B0) says "The part you are trying to
    /// apply is already installed!" (0xDA15D7E5); in a career (0x850078 =
    /// 1) an unowned part (0x521BF0) above level 0 (+5 & 0xE0) costing more
    /// than the bank (0x514280 against 0x861E74, `jge`) "You do not have
    /// enough bank…" (0xD1AC60BE), else the purchase question with Yes
    /// 0x09F00F07: 0x44B7167F, or for the trunk layout (slot 0x22) 0xF58C0CD9
    /// over a stock or empty one (0x50F150) and 0x4FAFB114 over another;
    /// otherwise the install question with Yes 0x51123592: 0xF5AC2B2A, or for
    /// slot 0x22 0x113EAB80 over a stock one, 0x266131CE for a stock layout
    /// over another, 0x8E4312B0 for any other. All three on
    /// GenericDialog_SMALL (0x78FA24 / the string at 0x568E9B) — not sized
    /// by the text as the body shop's — the questions by 0x5587A0 with Yes /
    /// No 0xF770 / 0x61C and No focused (its 1), No / Back 0xB4623F67;
    /// 0x838970 set and the overlay's own DIM (0x560B70 / 0x562950 on its
    /// 0x47FF4E7C). Yes buys (0x528A00, 0x09F00F07 only), fits (0x55C240)
    /// and plays the install sound (0x521290): `fit_browser_part`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ice_part_question(
        &self,
        package: &'static str,
        sub: Option<u8>,
        category: usize,
        i: usize,
        slot: u8,
        hash: u32,
        price: i32,
        owned: bool,
        installed: bool,
    ) -> Mode {
        let overlay = if package == "UI_CustomNeonMain" { "UI_NeonPartsOverlay" } else { "UI_IcePartsOverlay" };
        let again = || {
            if package == "UI_CustomNeonMain" {
                self.neon_browser(category, Some(i))
            } else if sub.is_some() {
                self.audio_browser(sub == Some(0x23), category, Some(i))
            } else {
                self.specialties_browser(category, Some(i))
            }
        };
        let over = |mut m: Mode| {
            if let Mode::Menu(Menu::Dialog(d, o)) = &mut m {
                d.under = Some(o.len() - 1);
                d.dim_under = Some((overlay, screens::DIM_HANDLER));
            }
            m
        };
        let career = self.profile.as_ref().is_some_and(|p| !p.scratch) && self.career_shop();
        if installed {
            let text = self.label(0xDA15_D7E5, "The part you are trying to apply is already installed!");
            let d = Dialog::new("GenericDialog_SMALL", &text, &[&self.label(0x0639, "OK")], self.now());
            return over(Mode::Menu(Menu::Dialog(d, vec![again()])));
        }
        let level = self.car_part(slot, hash).map_or(0, |p| p.level);
        let buy = career && !owned && level != 0;
        if buy && self.profile.as_ref().is_some_and(|p| (p.cash as i32) < price) {
            let text = self.label(0xD1AC_60BE, "You do not have enough bank to purchase this item.");
            let d = Dialog::new("GenericDialog_SMALL", &text, &[&self.label(0x0639, "OK")], self.now());
            return over(Mode::Menu(Menu::Dialog(d, vec![again()])));
        }
        // 0x50F150(0x22): the fitted trunk layout is none or stock.
        let stock_layout = self.trunk_layout_level() == 0;
        let question = match (buy, slot == 0x22) {
            (true, false) => 0x44B7_167F,
            (true, true) if stock_layout => 0xF58C_0CD9,
            (true, true) => 0x4FAF_B114,
            (false, false) => 0xF5AC_2B2A,
            (false, true) if stock_layout => 0x113E_AB80,
            (false, true) if hash != 0 && level == 0 => 0x2661_31CE,
            (false, true) => 0x8E43_12B0,
        };
        let yes = Mode::Menu(Menu::LotDeal(LotDeal::FitPart { package, sub, category, i, slot, hash, price, owned }));
        over(focus_no(self.ask_in("GenericDialog_SMALL", question, yes, again())))
    }

    /// Out of the garage with no save point (0x9A3B571D at once, 0x4AF607):
    /// career +0x8744 = 1, then the city start.
    pub(crate) fn go_on_garage(&mut self, at: Option<(Vec3, f32)>) -> Result<()> {
        if let Some(p) = self.profile.as_mut() {
            p.explore_set = true;
        }
        self.explore(at)
    }

    /// The loaded profile as main-menu Customize has made it so far: the
    /// set-aside profile with the session's car in its My Cars record (the
    /// original customizes the record itself; the clone merges it on Back,
    /// `MenuKind::CustomizeCategory`). None without a profile.
    pub(crate) fn customize_merged(&self) -> Option<Profile> {
        let mut real = self.save_flow.customize.clone().flatten()?;
        if let (Some(c), Some((k, _))) = (self.profile.as_ref(), self.save_flow.customize_car)
            && let Some(m) = real.my_cars.get_mut(k).and_then(Option::as_mut)
        {
            m.installed = c.installed.clone();
            m.perf = c.perf.clone();
        }
        Some(real)
    }

    /// Customize's "Do you want to enter the Performance Tuning / Dyno
    /// track?" Yes (0x34DC1BEC in UI_ChooseCustomizeCategory's handler
    /// 0x55E530; **verified**, observed under Wine): 0x52A4D0 (the
    /// profile against the snapshot 0x552CD0 took, which it frees); changed
    /// → the save point (0x4A3100 mode 0x1141), unchanged with a profile
    /// current (0x83AA00) → 0x461A18EE, the dyno, at once. MC's result goes
    /// on to the dyno either way (mode 0x1000). The crib's and the
    /// performance shop's dyno have no save point here.
    pub(crate) fn customize_dyno_yes(&mut self) -> Result<()> {
        if self.dyno.from != 2 || self.save_flow.customize.is_none() {
            self.mode = self.enter_dyno_scene();
            return Ok(());
        }
        let snapshot = self.save_flow.snapshot.borrow_mut().take();
        let Some(merged) = self.customize_merged() else {
            // No profile current (0x83AA00 clear): the save point (0x4A3100
            // mode 0x1141) starts MC_Main's operation 0x40, which with no
            // profile (0x502FE0) asks "Would you like to create a new
            // profile?" (0x4F33A0 → 0x4E8D30(2): 0x02B641C0, No focused).
            // Wine (L1, fresh copy, No Profile, Customize → Peugeot 206 →
            // Dyno Yes): the question; No → "You cannot enter Performance
            // Tuning / Dyno without an active profile." (0x55E71F); Yes →
            // the name → "Are you sure you want to save the profile named
            // NAME?" → No → the dyno with NAME current (unsaved).
            let now = self.now();
            self.mc_over(MenuKind::CustomizeCategory, now);
            let mut m = McMain::new(false, now);
            m.autosave = self.settings.autosave;
            m.career = false;
            self.save_flow.mc_create_then = Some(AfterSave::CustomizeDyno);
            self.mode = Mode::Menu(Menu::McMain(m));
            return Ok(());
        };
        let _ = merged.commit();
        let changed = snapshot.is_some_and(|snap| merged.native_payload().is_some_and(|now| now != snap));
        if !changed {
            self.mode = self.enter_dyno_scene();
            return Ok(());
        }
        // The save point shows and saves the loaded profile with the car.
        self.save_flow.customize = Some(None);
        self.dyno.scratch = self.profile.replace(merged);
        self.save_point(false, AfterSave::CustomizeDyno)
    }

    /// A career shop visit (0x83898C = 0 with game mode 0x850078 = 1):
    /// not Customize, not a Quick Race free run.
    pub(crate) fn career_shop(&self) -> bool {
        self.profile.is_some() && self.save_flow.customize.is_none() && !self.race_flow.quick_free_run && !self.shop.garage_parts
    }

    /// The garage's Parts Storage menu on option `i`.
    pub(crate) fn storage_menu(&self, i: usize) -> Mode {
        let mut m = self.icon_menu(MenuKind::CustomizeCategory);
        if let Mode::Menu(Menu::Icons(menu, _)) = &mut m {
            menu.select_now(i);
        }
        m
    }

    /// The garage's menu on the option doing `action`.
    pub(crate) fn crib_menu_on(&self, action: Action) -> Mode {
        let mut m = self.icon_menu(MenuKind::Crib);
        if let Mode::Menu(Menu::Icons(menu, _)) = &mut m
            && let Some(i) = menu.options.iter().position(|o| o.action == action)
        {
            menu.select_now(i);
        }
        m
    }

    /// The career shops' exit question (**verified**: the performance
    /// category 0x55C6E5 and UI_ICEMAIN 0x568B2C, as the car lot's
    /// 0x4EDE8B): 0x5587A0 over the screen's own package, "" (sized by
    /// length), Yes 0xF770 → 0xD05FC3A3, No 0x061C and Back 0x1FAB5998,
    /// text "Are you sure you wish to exit back to Explore Mode?"
    /// (0xD76E0B5A); then StartDim (0x55C715 / 0x568B66). No is StopDim
    /// (0x1FAB5998 → 0x538CB0: back on the screen, UNDIM); Yes leaves to
    /// the city (0x552C90: the save point when changed, `LeaveShop`).
    pub(crate) fn shop_exit_question(&mut self, again: Mode) -> Mode {
        self.career_world.pending_explore = self.here();
        let mut m =
            dialog_over(self.ask(0xD76E_0B5A, Mode::Menu(Menu::LotDeal(LotDeal::LeaveShop)), again), true, None);
        // No focused (0x5587A0's eighth argument 1; Wine menus13: the
        // performance shop's question, as the car lot's).
        if let Mode::Menu(Menu::Dialog(d, _)) = &mut m {
            d.selected = 1;
        }
        m
    }

    /// 0x552C90: leaving a career shop for the city — the save point when
    /// the profile changed since the shop's screen opened (0x4A3100 mode
    /// 0x41), then the city start.
    pub(crate) fn leave_shop(&mut self) -> Result<()> {
        let at = self.here();
        if self.changed_since_snapshot() {
            return self.save_point(false, AfterSave::Explore(at));
        }
        self.explore(at)
    }
}
