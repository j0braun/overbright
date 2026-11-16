use crate::*;

impl Game {
    /// The garage scene the menu on screen stands on, if any. UI_Main's
    /// constructor (0x4DF140) pushes GarageMain.fng under it when it is not
    /// up, sets game mode 2, so GarageMain's light setup is SHOWROOM
    /// (0x4EAE37), on the platform the front end loads with UI_Main as its
    /// first screen, SHOWROOM.BIN (0x57FEFA). Its car (FE car 0x8371B0,
    /// 0x413440 at 0x4DF264): the preset DDAY_PLAYER_CAR (0x637280) while
    /// 0x86347D (career +0x873D) is clear, else (0x515D00(0) in game mode
    /// 2) the car named by 0x83AAB8, the last one customized from the Quick
    /// Race car select ([`Profile::menu_car`]). +0x873D is set once the
    /// career's car slot is current (0x500BD0 / 0x541550), which the clone
    /// reads as the profile having a car. With +0x873D set and no name
    /// there, 0x503510 finds no car and the FE car keeps type −1
    /// (0x610280(-1, …) at 0x4DF1D2); what the garage then shows is
    // prov: open game.main_menu_car
    /// **open**, and the clone shows the career's car.
    pub(crate) fn main_menu_car(&self) -> (Option<String>, std::collections::BTreeMap<u8, u32>) {
        match self.profile.as_ref() {
            Some(p) if !p.car.is_empty() && p.menu_car.is_some() => {
                p.menu_car.clone().map_or((None, Default::default()), |(c, i)| (Some(c), i))
            }
            Some(p) if !p.car.is_empty() => (Some(p.car.clone()), p.installed.clone()),
            _ => match self.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR) {
                Some(x) => (Some(x.car.clone()), x.installed()),
                None => (None, Default::default()),
            },
        }
    }

    /// The platform scene the front end stands on (0x57FE85 at its load,
    /// 0x4A7180 later), which also names the FE cars' light setup
    /// (GarageMain 0x4EAE1E → 0x48A2D0; 0x4A7180 the same for its scene):
    /// SHOWROOM outside the career (the main menu, Quick Race and the
    /// main-menu Customize; game mode 0x850078 ≠ 1); in the career the
    /// visited shop's scene file (record +0x40 up to its first '.') while
    /// 0x86344C is set, else CRIB01 (0x55A7D8 / 0x53CCD0 set 0x8635B9 and
    /// UI_CareerCrib 0x4AF820 swaps SHOWROOM for CRIB01, 0x4A7180(1, 4)).
    pub(crate) fn frontend_scene(&self) -> String {
        let career = self.profile.is_some() && self.save_flow.customize.is_none() && !self.race_flow.quick_free_run;
        // The main menu's own screens (UI_StartCareer, the options, the
        // profile manager, Quick Race) run in game mode 2 (UI_Main's
        // constructor) and Customize in 0x20: SHOWROOM, as under Wine; so
        // do the online (8) and LAN (0x200) screens (observed under Wine).
        let outside = match &self.mode {
            _ if matches!(self.race_flow.game_mode, 2 | 0x20 | 8 | 0x200)
                && !matches!(self.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))) =>
            {
                true
            }
            Mode::Menu(Menu::Icons(_, MenuKind::Main | MenuKind::QrMode)) => true,
            Mode::Menu(Menu::CarLot(lot)) => matches!(lot.state, LotState::QuickRace(_) | LotState::Customize),
            _ => !career,
        };
        if outside {
            return nfsu2::garage::PLATFORMS[1].into();
        }
        match self.shop.visit.and_then(|h| self.game_data.career.shops.iter().find(|s| s.hash == h)) {
            Some(shop) => shop.scene_file.split('.').next().unwrap_or_default().to_string(),
            None => nfsu2::garage::PLATFORMS[4].into(),
        }
    }

    /// What GarageMain's view is asked for under the current menu, or None
    /// when the menu leaves it alone (no call traced).
    /// * A parts list (UI_GenericParts_Browser in a category: the list
    ///   screen 0x553320 → 0x53BA10 → 0x4C2100(slot, 0, 0, 0)) and UI_Paint
    ///   in a slot (0x55CAD0 / 0x55D670 on each swatch change, the slot's
    ///   +0x48): the slot's view. UI_Rims_Browser (0x55F350 and its scroll
    ///   0x55F900): 0x1D, Front Wheel, for each style shown.
    /// * The returns to the base view are the screens' own events
    ///   (`GarageViewPhase`, `garage_view_returns`), not a menu's state:
    ///   the category strips leave the view as it is.
    pub(crate) fn garage_view_request(&self) -> Option<GarageViewRequest> {
        match &self.mode {
            Mode::Menu(Menu::Parts(b)) if matches!(b.package, "UI_Rims_Browser" | "UI_ChooseSpinner") => {
                Some(GarageViewRequest::Wheel(b.category))
            }
            Mode::Menu(Menu::Parts(b)) if b.package == "UI_GenericParts_Browser" => {
                match (b.part, b.categories.get(b.category)) {
                    // The list's update (0x567A60 → 0x50E580(slot, part) →
                    // 0x4C2100 → 0x4C1D40(part, slot) → 0x4A55E0 / 0x4A5510)
                    // passes the highlighted part, whose kind (+5 & 0x1F)
                    // picks a vinyl slot's view by its group.
                    (Some(i), Some(c)) => {
                        let group = (0x40..0x44)
                            .contains(&c.slot)
                            .then(|| c.parts.get(i).and_then(|p| self.car_part(c.slot, p.0)).map(|p| p.kind))
                            .flatten();
                        Some(GarageViewRequest::Slot(u32::from(c.slot), group))
                    }
                    _ => None,
                }
            }
            // A decal slot's list (0x568540 at 0x5685EF): 0x4C2100 on the
            // slot's location (0x50E5A0: 0x61–0x98 by eights → 0x34–0x3A,
            // else 0xA9, no record); under Wine Windshield's list blends
            // to Front Window Decal.
            Mode::Menu(Menu::Parts(b)) if b.package == "UI_DecalMain" => match (b.part, b.categories.get(b.category)) {
                (Some(_), Some(c)) if (0x61..=0x98).contains(&c.slot) => {
                    Some(GarageViewRequest::Slot(0x34 + u32::from(c.slot - 0x61) / 8, None))
                }
                _ => None,
            },
            Mode::Menu(Menu::Paint(p)) if p.vinyl.is_none() && p.selected.is_some() => {
                screens::PAINT_SLOTS.get(p.slot).map(|s| GarageViewRequest::Slot(u32::from(s.0), None))
            }
            // The neon and specialties lists: NeonOverlay_Step (0x55CAD0)
            // and UI_IcePartsOverlay's entry (0x5606F0) and step
            // (0x560930) call 0x4C2100(category slot +0x48) with the
            // preview; a slot with no camera record (the neon slots) leaves
            // prov: inferred game.garage_view_request
            // the view. **Inferred**: the neon list's entry moves it too
            // (its entry function is not traced); Custom Gauges (0xA3) has
            // its own overlay and no such call.
            Mode::Menu(Menu::Parts(b)) if matches!(b.package, "UI_CustomNeonMain" | "UI_ICEMAIN") => {
                match (b.part, b.categories.get(b.category)) {
                    (Some(_), Some(c)) if c.slot != 0xA3 && c.action == screens::CategoryAction::Parts => {
                        Some(GarageViewRequest::Slot(u32::from(c.slot), None))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The screen GarageMain stands under now (`GarageViewPhase`).
    pub(crate) fn garage_view_phase(&self) -> GarageViewPhase {
        match &self.mode {
            Mode::Menu(Menu::Parts(b)) => {
                GarageViewPhase::Browser { package: b.package, list: b.part.is_some(), sub: b.sub.is_some() }
            }
            Mode::Menu(Menu::Icons(_, MenuKind::VinylLayer)) => GarageViewPhase::VinylLayer,
            _ => GarageViewPhase::Other,
        }
    }

    /// Whether a screen returned GarageMain to its base view since the
    /// last frame (0x4C2140; the callers are listed at
    /// `GarageViewPhase`), from the screen it stood under then and now.
    pub(crate) fn garage_view_returns(&mut self) -> bool {
        let (was, is) = (self.garage_fe.phase, self.garage_view_phase());
        self.garage_fe.phase = is;
        // The crib's and Customize's 0x83898C ≠ 0 (a career shop's is 0).
        let customize = !self.career_shop();
        match (was, is) {
            // A list closed back to its strip: 0x55EF90, 0x55FCF0,
            // 0x5605C0 / 0x568A44, 0x56A903 / 0x56A814.
            (
                GarageViewPhase::Browser { package: p, list: true, .. },
                GarageViewPhase::Browser { package: q, list: false, .. },
            ) if p == q
                && matches!(p, "UI_GenericParts_Browser" | "UI_Rims_Browser" | "UI_ICEMAIN" | "UI_CustomNeonMain") =>
            {
                true
            }
            // UI_GenericParts_Browser left (0x56785F) in customize mode
            // or from a group of its own; UI_ICEMAIN left in customize
            // mode (0x568D0D).
            // (The clone's carbon fibre browser is the same package with
            // `sub`: its Back to the body categories is that screen's
            // exit, +0x1D5.)
            (
                GarageViewPhase::Browser { package: "UI_GenericParts_Browser", sub: true, .. },
                GarageViewPhase::Browser { package: "UI_GenericParts_Browser", sub: false, .. },
            ) => true,
            (GarageViewPhase::Browser { package: "UI_GenericParts_Browser", sub, .. }, _)
                if !matches!(is, GarageViewPhase::Browser { package: "UI_GenericParts_Browser", .. }) =>
            {
                customize || sub
            }
            (GarageViewPhase::Browser { package: "UI_ICEMAIN", .. }, _)
                if !matches!(is, GarageViewPhase::Browser { package: "UI_ICEMAIN", .. }) =>
            {
                customize
            }
            // UI_ChooseVinylLayer's constructor (0x554CFC).
            (_, GarageViewPhase::VinylLayer) => was != GarageViewPhase::VinylLayer,
            // UI_DecalMain's Back from the top node and UI_CustomNeonMain's
            // first tick and Back (0x5683C2, 0x553547, 0x56AA4B).
            _ => crate::menus::garage_view::screen_base_view_returns(was, is),
        }
    }

    /// The garage under the current menu, its view brought up to date.
    /// GarageMain is up under UI_Main (0x4DF140), UI_QRModeSelect
    /// (0x4EF280), the crib (0x4AF820) and the screens that reach it
    /// through 0x4A7830: the parts, paint and rims browsers (0x4C2100);
    /// the car lots and the shops' category menus in between are drawn
    /// over it too. Once pushed it stays: of the 121 uses of its name
    /// (0x78FCD8) the only pops (0x55DDB0) are MU_QuickRaceCarSelect's
    /// constructor 0x4D8F20 and destructor 0x4D9050 (split screen), the
    /// pushes (0x555F20(name, 0xFF, 0)) 0x49D9D5, 0x4B9405, 0x55B720 and
    /// 0x55E06C, the rest 0x52CF70 "is it up" checks. So the main menu's
    /// own screens stand on it too: UI_StartCareer, UI_OptionsMain and
    /// its pages (UI_Options, the controls, EA Trax, the credits),
    /// UI_ProfileManager and its memory card screens, and the Quick
    /// Race's track select and options (the original under Wine,
    /// 2026-10-29: all of these show the showroom and its car).
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn garage_request(&mut self) -> Option<GarageRequest> {
        self.garage_update();
        self.garage_fe.request.clone()
    }

    /// The garage's update for the tick — the lot swing, the
    /// view's tick, the FE car's swing, the part, turn and swing sounds and
    /// `garage_fe.last` — leaving the tick's request in
    /// `garage_fe.request` (`garage_request` returns it; the draw reads it).
    /// Called by the tick's `garage_tick` (not the frame's
    /// `prepare_frame`).
    pub(crate) fn garage_update(&mut self) {
        self.garage_fe.request = self.garage_update_request();
    }

    /// [`Self::garage_update`]'s request.
    fn garage_update_request(&mut self) -> Option<GarageRequest> {
        let shows = self.garage_shows();
        // A dialog is a screen pushed over the one that opened it: the
        // front end's scene (GarageMain, its own object, not a screen's)
        // stays under it as it was — the car lot's car, the garage's view.
        // Free roam is not drawn behind a shop's dialogs.
        if matches!(self.mode, Mode::Menu(Menu::Dialog(..) | Menu::LotDeal(_))) && self.garage_fe.view.is_some() {
            let now = self.now() / 1000.0;
            return self.garage_fe.last.clone().map(|r| GarageRequest { now, ..r });
        }
        if !shows {
            self.garage_fe.view = None;
            self.garage_fe.last = None;
            self.garage_fe.phase = GarageViewPhase::Other;
            self.garage_fe.parts_sounded = [false; 4];
            // No FE car: its turn loop ends with it (**inferred**).
            if let Some(a) = self.audio.as_mut() {
                a.fe.turn_stop();
                a.car_turn.1 = false;
            }
            return None;
        }
        let platform = self.frontend_scene();
        // A fresh GarageMain (or another scene) starts on the
        // constructor's view (0x4EA900: camera record +0x154 = 0).
        if self.garage_fe.view.as_ref().is_none_or(|(p, ..)| *p != platform) {
            self.garage_fe.view = Some((platform.clone(), None, nfsu2::garage::View::new(0)));
            self.garage_fe.left = false;
            // A fresh GarageMain's loader has no car yet (0x4C17DE: +0x2CE0
            // = 1 until its first car is in).
            self.garage_fe.type_shown = None;
        }
        let request = self.garage_view_request();
        // The Open/Close toggle lasts while its list is up on that part: a
        // step shuts it (Wine, the finished-career save's 350Z: the hood opened on
        // COPPERHEAD by BUTTON1 is shut on COBRA; 0x560930 → 0x438030).
        // The FE car is built again then (the preview ride's 0x55C180, and
        // on leaving the list), so the parts are shut at once, without a
        // closing animation or its sound (Wine, the finished-career save's 350Z:
        // STOCK doors opened, then Back or Down: shut in the next frame).
        if let Some((slot, part, t)) = self.garage_fe.toggled
            && !matches!(&self.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.part == part && b.categories.get(b.category).is_some_and(|c| c.slot == slot))
        {
            self.garage_fe.toggled = None;
            if let Some(g) = self.render.garage.as_mut() {
                (g.channels, g.applied) = (Default::default(), [false; 4]);
            }
            for (c, open) in t.into_iter().enumerate() {
                if open {
                    self.garage_fe.parts_sounded[c] = false;
                }
            }
        }
        let returns = self.garage_view_returns();
        let open = self.garage_open_parts();
        let (now, dt) = (self.now() / 1000.0, self.pacing.frame_dt);
        self.garage_sounds(open);
        self.lot_swing(now);
        let garage_left = self.garage_fe.left;
        let (swing, view) = self.garage_view_tick(request, returns, now, dt)?;
        self.car_turn_sound(view.at(now).car_angle);
        // The swing's sound (0x4D82D0: 0x51DE40(5 / 4, 0x17)) as it starts.
        if swing && let Some(id) = nfsu2::audio::fe_rules::garage_swing(garage_left) {
            self.fe_id(id);
        }
        let (car, installed, perf) = self.fe_car_ride();
        let hydraulics = self.garage_hydraulics(car.as_deref(), &installed, &perf);
        let camber = nfsu2::car::camber_angle(&|id| perf.contains(&id));
        // The Lights list (slot 0x9F) lights the FE car's lamps: entering a
        // UI_IcePartsOverlay list runs 0x54FB80(item, 1), which for slot
        // 0x9F sets 0x8A1C68; leaving (0x560642, 0x5569EE, 0x568A21)
        // clears it. 0x61ACF0 draws the FE cars' light flares (0x615640)
        // only with it set and g_LightGlowEnable (0x870CE4) on.
        let headlights = Switches::from_custom(&self.settings.custom).light_glow
            && matches!(&self.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.sub.is_none() && b.part.is_some() && b.categories.get(b.category).is_some_and(|c| c.slot == 0x9F));
        let view = self.fe_car_swing(swing, &car, view, now);
        let purge = self.shop.purge_test && self.purge_list_open();
        let prefetch = self.fe_prefetch(&car, &installed, &perf);
        let request = GarageRequest {
            lights: platform.clone(),
            platform,
            car,
            installed,
            view,
            open,
            asked: now,
            now,
            hydraulics,
            camber,
            perf,
            headlights,
            purge,
            prefetch,
        };
        self.garage_fe.last = Some(request.clone());
        Some(request)
    }

    /// Whether the current menu shows the front end's garage scene (see
    /// `garage_request`).
    pub(crate) fn garage_shows(&self) -> bool {
        match &self.mode {
            Mode::Menu(Menu::Icons(_, kind)) => matches!(
                kind,
                MenuKind::Main
                    | MenuKind::Career
                    | MenuKind::Options
                    | MenuKind::Profile
                    | MenuKind::QrMode
                    | MenuKind::Crib
                    | MenuKind::CustomizeCategory
                    | MenuKind::RimBrand
                    | MenuKind::Graphics
                    | MenuKind::VinylLayer
                    | MenuKind::PerfCategory
                    | MenuKind::PerfPackage
                    // The crib's Reward Options and its lists (reached
                    // only from the crib, so GarageMain stays up; Wine
                    // menus113 v0 / v2).
                    | MenuKind::Rewards
                    | MenuKind::Covers(_)
                    | MenuKind::SignedSponsors
            ),
            Mode::Menu(
                Menu::CarLot(_) | Menu::Parts(_) | Menu::Paint(_) | Menu::BrandSelect(..) | Menu::ChooseHud(_),
            ) => true,
            Mode::Menu(Menu::Options(o)) => o.pause.is_none(),
            Mode::Menu(
                Menu::Controls(_) | Menu::Jukebox(_) | Menu::Credits(_) | Menu::QuickRace(_) | Menu::QrOptions(..),
            ) => true,
            // The crib's World Map (UI_CareerWorldMap, reached only from
            // the crib, so GarageMain stays up): the garage and its car
            // under the map (Wine).
            Mode::Menu(Menu::WorldMap(_)) => true,
            // UI_Main's LAN and Online screens are pushed over it too
            // (GarageMain stays up, as the original under Wine
            // shows).
            Mode::Menu(Menu::LanServers(..) | Menu::IspConnect(_) | Menu::EaLogin(..)) => true,
            // The memory card screens: over the profile manager's garage;
            // at boot (no garage up yet) over the title.
            Mode::Menu(Menu::McMain(_) | Menu::McList(_) | Menu::McMessage(..)) => self.garage_fe.view.is_some(),
            _ => false,
        }
    }

    /// CarPart_RequestAnim's sound in the front end as the menu opens or
    /// shuts a part (`garage_request`).
    fn garage_sounds(&mut self, open: [bool; 4]) {
        // CarPart_RequestAnim's sound in the front end (0x61F830: 0x50D180
        // with [0x82B884]+0x90 = 1) as the menu opens or shuts a part (a
        // shut only when not shut, 0x4C1860). The doors' pair is 0x4C /
        // 0x4F when the FE car's door style part has ANIMSTYLE 1 (scissor,
        // also as a wide body's WIDE<kind>), else 0x50 / 0x4E (0x61F8F6).
        let animstyle_one = self
            .render.garage
            .as_ref()
            .and_then(|g| g.car.as_ref())
            .is_some_and(|(_, _, car, _)| matches!(car.part_styles.doors, 1 | 3..=6));
        for c in 0..4 {
            if open[c] != self.garage_fe.parts_sounded[c] {
                self.garage_fe.parts_sounded[c] = open[c];
                if let Some(id) = nfsu2::audio::fe_rules::part_sound_fe(c, open[c], animstyle_one) {
                    self.fe_id(id);
                }
            }
        }
    }

    /// A car lot's step and its delayed ride (`garage_request`).
    fn lot_swing(&mut self, now: f32) {
        // A car lot's Left / Right (0x4A7900 → GarageMain +0x148: Right
        // turns up) changes the car on the platform (0x4D82D0 state 2).
        let lot = match &self.mode {
            Mode::Menu(Menu::CarLot(lot)) => {
                lot.cars.get(lot.selected).map(|c| (lot.selected, lot.cars.len(), c.clone()))
            }
            _ => None,
        };
        // UI_QRCarSelect's step (0x4E2590) and the crib's select
        // (0x4A7900…) set +0x148; UI_CareerCarLot leaves it as it was.
        let sets_direction = matches!(&self.mode, Mode::Menu(Menu::CarLot(l)) if matches!(l.state, LotState::CribSelect | LotState::QuickRace(_) | LotState::Customize));
        let change = match (&self.garage_fe.lot, &lot) {
            (Some((was, _, before)), Some((is, n, now_car))) if was != is || before != now_car => {
                Some((*is > *was, *n > 1 && was != is))
            }
            _ => None,
        };
        // A lot just opened shows its car at once (the screen's
        // constructor hands it to GarageMain).
        if lot.is_none() || self.garage_fe.lot.is_none() {
            self.garage_fe.crib = None;
            self.garage_fe.lot_queued = None;
            self.garage_fe.crib_shown = lot.as_ref().map(|l| l.2.clone());
        }
        if let Some((up, stepped)) = change {
            if sets_direction && stepped {
                self.garage_fe.left = !up;
            }
            self.garage_fe.crib = Some(now);
            self.garage_fe.crib_real = self.real_now();
        }
        self.garage_fe.lot = lot.clone();
        // Every lot asks for its car 0.3 s after its last Left / Right:
        // the step stores the tick clock 0x8651A8 (the crib's 0x4B04C0 at
        // +0xB8, UI_CareerCarLot's 0x4AFFA0 at +0xE0, UI_QRCarSelect's
        // 0x4E2590 at +0xC4) and the screen's tick (0x4DFFEC, 0x4EE00D,
        // 0x4FCD1D) waits until (now − it) · 0.00025 ≥ 0.3, then hands the
        // ride (0x511E00) to the loader (0x4D8EC0). The old car stays
        // until then (Wine, UI_QRCarSelect: five Lefts 0.14 s apart, one
        // swing 0.53 s after the last). "Now" there is 0x8651A8
        // (0x4E0005, 0x4EE026, 0x4FCD2E: `fcomp 0x78758C; test ah, 1`),
        // ≥ 1200 ticks; before, 0.3 s of the package clock (19 ticks, not
        // 18).
        if self.garage_fe.crib.is_some()
            && nfsu2::game_clock::real_at_least(
                self.real_now(),
                self.garage_fe.crib_real,
                nfsu2::audio::fe_rules::CRIB_SWING_DELAY,
            )
        {
            self.garage_fe.crib = None;
            self.garage_fe.lot_queued = lot.as_ref().map(|l| l.2.clone());
        }
    }

    /// GarageMain's view this frame: a queued car's swing, a screen's
    /// return, the menu's request and the idle tour (`garage_request`);
    /// whether the platform swings, and the view as it then stands.
    fn garage_view_tick(
        &mut self,
        request: Option<GarageViewRequest>,
        returns: bool,
        now: f32,
        dt: f32,
    ) -> Option<(bool, nfsu2::garage::View)> {
        let garage_left = self.garage_fe.left;
        let (_, last, view) = self.garage_fe.view.as_mut()?;
        // GarageMain (0x4D82D0): a car asked for during a swing waits for
        // its end (0x4D8030 queues it while the old car is held, state 2's
        // end loads it: state 1), so the platform always ends a whole
        // third round, the asked car on the front disc (Wine: Rights 0.45 s
        // apart, three swings back to back, 0x8363F0 at 0 / 120 / 240).
        // A car of another type swings (state 1 → 2, +0x2CE0 set); the
        // same type with other parts is swapped in place (state 3); the
        // crib's empty and locked slots always swing (0x4A7930 / 0x4A7960
        // → +0x161 / +0x162).
        let mut swing = false;
        if !view.changing(now)
            && let Some(asked) = self.garage_fe.lot_queued.take()
            && self.garage_fe.crib_shown.as_ref() != Some(&asked)
        {
            let same_type = self.garage_fe.crib_shown.as_ref().is_some_and(|s| {
                s.placeholder.is_none() && asked.placeholder.is_none() && s.name.eq_ignore_ascii_case(&asked.name)
            });
            swing = !same_type;
            self.garage_fe.crib_shown = Some(asked);
        }
        if swing {
            view.change_car(!garage_left, now);
        }
        // A screen's return to the base view (0x4C2140): the camera blends
        // back, the car keeps its angle and steer. The menu's own request
        // as it then stands counts as asked (UI_Rims_Browser's 0x55FCF0
        // asks 0x1D just before it), so only its next change moves the
        // view again.
        if returns {
            view.reset(0, now, dt);
            *last = request;
        }
        if request.is_some() && request != *last {
            match request {
                // Camera class +0x154 and +0x158 as the constructor leaves
                // them (0; see docs/formats.md, open).
                Some(GarageViewRequest::Slot(slot, group)) => view.set_slot(slot, false, false, group, now, dt),
                Some(GarageViewRequest::Wheel(_)) => view.set_slot(0x1D, false, false, None, now, dt),
                None => {}
            }
            *last = request;
        }
        // The idle tour (0x4D82D0; car class 0 as above).
        view.tick(now, dt, false);
        let view = *view;
        Some((swing, view))
    }

    /// The FE car the menu shows: its name, installed parts (the unique
    /// part previewed, the performance parts' visuals) and performance
    /// parts (`garage_request`).
    fn fe_car_ride(&self) -> (Option<String>, std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>) {
        self.fe_car_ride_in(&self.mode)
    }

    /// The FE car with its parts and performance parts as the main menu's
    /// garage scene asks for it ([`Game::fe_car_ride`] under UI_Main).
    pub(crate) fn main_menu_ride(&self) -> (Option<String>, std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>) {
        let menu = IconMenu::new("UI_Main", None, screens::MAIN_MENU.to_vec(), 0.0);
        self.fe_car_ride_in(&Mode::Menu(Menu::Icons(menu, MenuKind::Main)))
    }

    /// [`Game::fe_car_ride`] for the screen `mode`.
    fn fe_car_ride_in(&self, mode: &Mode) -> (Option<String>, std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>) {
        let (car, mut installed) = match mode {
            // A career slot's car (the crib's Car Select, a trade's list)
            // shows as its record has it (0x511E00 builds the entry's ride
            // from the slot's car record, 0x413440 copies it into the FE
            // car); a stock lot car is stock.
            Mode::Menu(Menu::CarLot(_)) => match self.garage_fe.crib_shown.as_ref() {
                // A sponsor car shows its preset (under Wine: the Chingy
                // Navigator with its vinyl and neon).
                Some(c) if c.preset.is_some() => (
                    Some(c.name.clone()),
                    c.preset.as_deref().and_then(|n| self.preset_ride(n)).map(|r| r.0).unwrap_or_default(),
                ),
                Some(c) if c.placeholder.is_none() => match c.slot.and_then(|i| self.profile.as_ref()?.slot_ride(i)) {
                    Some((car, installed, _)) if car == c.name => (Some(car), installed),
                    _ => (Some(c.name.clone()), Default::default()),
                },
                _ => (None, Default::default()),
            },
            Mode::Menu(Menu::Icons(_, MenuKind::Main)) => self.main_menu_car(),
            // The main menu's screens leave UI_Main's FE car (0x8371B0) as
            // it was: nothing on the way to them sets it again.
            _ if self.race_flow.game_mode == 2 => self.main_menu_car(),
            _ => match self.profile.as_ref() {
                Some(p) if !p.car.is_empty() => (Some(p.car.clone()), self.preview_ride(&p.installed)),
                _ => match self.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR) {
                    Some(x) => (Some(x.car.clone()), x.installed()),
                    None => (None, Default::default()),
                },
            },
        };
        // UI_ChooseUniquePart's refresh (0x5618C0) previews the selected
        // part on the FE car (0x55C180(slot, part) on the copy +0x50 of the
        // car +0x940: slot 0x1D also takes 0x1E); a performance unique
        // only sets the copy's flag (0x521490: +0x50 + 0x318 + 4i), which
        // the car does not show.
        if let Mode::Menu(Menu::Parts(b)) = mode
            && b.package == "UI_ChooseUniquePart"
            && let Some(c) = b.categories.get(b.category)
            && c.slot != 0xFF
            && let Some(&(hash, ..)) = c.parts.first()
        {
            installed.insert(c.slot, hash);
            if c.slot == 0x1D {
                installed.insert(0x1E, hash);
            }
            // 0x55C180 → 0x631D30 → 0x61BCE0: the derived slots follow.
            if let Some(car) = car.as_deref() {
                installed = nfsu2::car::with_derived_slots(&installed, &self.game_data.parts, car);
            }
        }
        // The performance parts' brakes and engine (0x6399E0 by 0x639CF0):
        // the profile's car with its fitted parts, the D-Day preset's
        // with its level; a stock lot car has none, a slot car its slot's.
        let perf: std::collections::BTreeSet<u8> = match (&car, self.profile.as_ref()) {
            (_, _) if matches!(mode, Mode::Menu(Menu::CarLot(_))) => match mode {
                Mode::Menu(Menu::CarLot(_)) => match self.garage_fe.crib_shown.as_ref() {
                    Some(c) if c.preset.is_some() => {
                        c.preset.as_deref().and_then(|n| self.preset_ride(n)).map(|r| r.1).unwrap_or_default()
                    }
                    _ => self
                        .garage_fe.crib_shown
                        .as_ref()
                        .and_then(|c| c.slot)
                        .and_then(|i| self.profile.as_ref()?.slot_ride(i))
                        .filter(|r| Some(&r.0) == car.as_ref())
                        .map(|r| r.2)
                        .unwrap_or_default(),
                },
                _ => Default::default(),
            },
            (Some(c), Some(p)) if !p.car.is_empty() && *c == p.car => p.perf.clone(),
            (Some(c), _) => self
                .game_data.presets
                .iter()
                .find(|x| {
                    x.name == career::DDAY_PLAYER_CAR
                        && x.car == *c
                        && self.profile.as_ref().is_none_or(|p| p.car.is_empty())
                })
                .map(|x| x.perf(&self.game_data.career.packages))
                .unwrap_or_default(),
            _ => Default::default(),
        };
        if let Some(c) = &car {
            installed = nfsu2::car::perf_visual_parts(&installed, &self.game_data.parts, c, &self.game_data.career.packages, &|id| {
                perf.contains(&id)
            });
        }
        (car, installed, perf)
    }

    /// GarageMain state 1 (0x4D82D0) as the loader's car is in: a car
    /// of another type than the one loaded before (0x4D80C3: +0x2CE0 =
    /// the type differs), or the first car of a fresh loader (+0x2CE0 =
    /// 1 from 0x4C17DE / 0x4D80B7), swings in (state 2: the platform
    /// turns 120° over 0.5 s, 0x51DE40(5 / 4, 0x17) by +0x148) — the
    /// lots' steps above take the same path. Wine (the finished-career save): FE
    /// sound 69 (FE_MB 0x2E) and the turn on the garage after the dyno,
    /// on Resume Career (the main menu's RX-7 → the garage's Lancer)
    /// and on the main menu after the garage.
    fn fe_car_swing(
        &mut self,
        swing: bool,
        car: &Option<String>,
        view: nfsu2::garage::View,
        now: f32,
    ) -> nfsu2::garage::View {
        let mut view = view;
        if swing {
            self.garage_fe.type_shown = Some(car.clone());
        } else if car.is_some()
            && self.garage_fe.type_shown.as_ref().is_none_or(|t| !t.as_deref().zip(car.as_deref()).is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)))
            // A car asked during a swing waits for its end (0x4D8030 queues
            // it while the old car is held, +0x23D0; state 2's end loads
            // it), so every swing turns a whole third.
            && !self.garage_fe.view.as_ref().is_some_and(|v| v.2.changing(now))
        {
            self.garage_fe.type_shown = Some(car.clone());
            // A lot's steps swing above; a lot just opened hands its first
            // car to the loader from its constructor, which swings it in
            // like any other when its type differs (Wine:
            // UI_QRCarSelect's 206 after the main menu's 350Z, 0x8363F0
            // 120 → 240).
            let left = self.garage_fe.left;
            if let Some((_, _, v)) = self.garage_fe.view.as_mut() {
                v.change_car(!left, now);
                view = v.at(now);
            }
            if let Some(id) = nfsu2::audio::fe_rules::garage_swing(left) {
                self.fe_id(id);
            }
        }
        view
    }

    /// The rides a step in the shop's list would ask for next, as
    /// this one was made (the profile's car through `preview_ride`,
    /// then its performance parts' visuals), for `draw_garage` to build
    /// ahead; the step then shows its part in the next frame.
    fn fe_prefetch(
        &self,
        car: &Option<String>,
        installed: &std::collections::BTreeMap<u8, u32>,
        perf: &std::collections::BTreeSet<u8>,
    ) -> Vec<std::collections::BTreeMap<u8, u32>> {
        let shop_list = self.race_flow.game_mode != 2
            && match &self.mode {
                Mode::Menu(Menu::Parts(b)) => b.package != "UI_ChooseUniquePart",
                Mode::Menu(Menu::Paint(_)) => true,
                _ => false,
            };
        match (&car, self.profile.as_ref()) {
            (Some(c), Some(p)) if shop_list && !p.car.is_empty() && *c == p.car => self
                .shop_preview_neighbours()
                .into_iter()
                .map(|item| {
                    nfsu2::car::perf_visual_parts(
                        &self.preview_ride_with(&p.installed, Some(item)),
                        &self.game_data.parts,
                        c,
                        &self.game_data.career.packages,
                        &|id| perf.contains(&id),
                    )
                })
                .filter(|r| *r != *installed)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Which of the FE car's hood, trunk and doors the current menu holds
    /// open (0x4C1860(channel, open, 1.0) → 0x61F830; a close only when the
    /// channel is not shut, 0x4A5B20):
    /// * the hood for paint slot 0x44, Engine Accents (UI_Paint's set-up
    ///   0x55CC60 opens it, 0x55CE85 closes it; the garage camera opens it
    ///   for the slot 0x44 view and shuts it on the next, 0x4C1D40 /
    ///   0x4C1F27);
    /// * in UI_CustomNeonMain's parts lists, the hood for Engine neon
    ///   (0x9B) and the trunk for Trunk neon (0x9D) (0x50F290(category,
    // prov: inferred game.garage_open_parts
    ///   open): 0x553580 and 0x5571A2 open, 0x562B8C closes; **inferred**:
    ///   0x562B8C as leaving the list);
    /// * the trunk on the TRUNK AUDIO screens (0x5593F0 modes 2–5, at
    ///   0x559489, 0x55954C, 0x559659); the specialties menu itself (mode
    ///   0) closes the hood (0x559728).
    ///
    /// The doors: 0x521050(channel, open) works both for 2 (then 3 too),
    /// but its one caller (0x56946B) only closes; which screen opens them
    /// is not traced, nor 0x523CC0 (closes all four), 0x563BC6 (hood) and
    /// 0x56A5D1 / 0x56A7AC (trunk and hood).
    pub(crate) fn garage_open_parts(&self) -> [bool; 4] {
        let mut open = [false; 4];
        match &self.mode {
            Mode::Menu(Menu::Paint(p))
                if p.vinyl.is_none() && screens::PAINT_SLOTS.get(p.slot).is_some_and(|s| s.0 == 0x44) =>
            {
                open[0] = true
            }
            Mode::Menu(Menu::Parts(b)) if b.package == "UI_CustomNeonMain" && b.part.is_some() => {
                match b.categories.get(b.category).map(|c| c.slot) {
                    Some(0x9B) => open[0] = true,
                    Some(0x9D) => open[1] = true,
                    _ => {}
                }
            }
            Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.title == 0x02AA_DAC5 => open[1] = true,
            // The Split Hoods / Doors list's Open/Close (BUTTON1), while
            // that list is up on that part (leaving the list or a step
            // shuts them at once, see `garage_request`).
            Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.part.is_some() => {
                if let Some((slot, part, t)) = self.garage_fe.toggled
                    && part == b.part
                    && b.categories.get(b.category).is_some_and(|c| c.slot == slot)
                {
                    open = t;
                }
            }
            _ => {}
        }
        open
    }
}

/// What the garage under a menu shows: the platform scene's file name, the
/// cars' light setup (render info +0x904) and the car with its parts.
#[derive(Clone)]
pub(crate) struct GarageRequest {
    pub(crate) platform: String,
    pub(crate) lights: String,
    pub(crate) car: Option<String>,
    pub(crate) installed: std::collections::BTreeMap<u8, u32>,
    pub(crate) view: nfsu2::garage::View,
    /// The hood, trunk and doors the menu holds open
    /// (`Game::garage_open_parts`), asked at `asked` (seconds).
    pub(crate) open: [bool; 4],
    pub(crate) asked: f32,
    /// The front end's time the scene is drawn at (seconds).
    pub(crate) now: f32,
    /// The FE car's hydraulics (0x4A6030): the fitted slot 0xA1 part's
    /// level, and the suspension's lowering of the front and rear corners.
    pub(crate) hydraulics: (u32, [f32; 2]),
    /// Test N2O Purge on (UI_IcePartsOverlay +0x116): each FE tick its
    /// handler (0xC98356BA in 0x568D40) fires the FE car's purge,
    /// 0x4C1920(13, 1.0, 0) → 0x61FBB0 → 0x615CD0 (kinds 13 and 14).
    pub(crate) purge: bool,
    /// The front wheels' x-turn from the performance parts
    /// (`nfsu2::car::camber_angle`).
    pub(crate) camber: f32,
    /// The FE car's performance parts: its car type is the upgraded one
    /// (0x4A6670 reads the wheels from its own physics object, FE car
    /// +0x20 → 0x5B5360(1) → 0x5B4FD0 → 0x5B4DD0, so the suspension's
    /// lowering raises the wheel centres in the body, as 0x5F6830's frames
    /// do: the main menu's brake origin z 0.18557 by apitrace).
    pub(crate) perf: std::collections::BTreeSet<u8>,
    /// The FE car's light flares on (0x8A1C68, the Lights list).
    pub(crate) headlights: bool,
    /// The parts a step in the shop's list would ask for next (the
    /// same car and performance parts), built ahead by `draw_garage`.
    pub(crate) prefetch: Vec<std::collections::BTreeMap<u8, u32>>,
}

/// A menu's call on GarageMain's view: a part slot's (0x4C2100) or back
/// to the base view (0x4C2140).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GarageViewRequest {
    /// A slot's view, with a vinyl's group (part +5 & 0x1F) for 0x40–0x43.
    Slot(u32, Option<u8>),
    /// UI_Rims_Browser's / UI_ChooseSpinner's Front Wheel view (0x1D),
    /// asked again for each style the strip shows (0x55F900; the style's
    /// index), so a step after a return to the base view asks it again.
    Wheel(usize),
}

/// Which screen GarageMain stood under, for the returns to the base view
/// (0x4C2140(0, 1) → 0x4C1FA0 → 0x453320: the camera blends back, the
/// car keeps its angle and steer). Only these screens call it:
/// * UI_GenericParts_Browser leaving a category's list (0x55EF90 at
///   0x55F06D), and on its out animation's end (0xE1FDE1D1, 0x56785F)
///   when it leaves with the customize mode 0x83898C ≠ 0 (the crib, the
///   main menu's Customize) or from a group of its own (carbon fibre
///   +0x1D5, the graphics shop's vinyls +0x1D7, +0x1D4, +0x1D3);
/// * UI_Rims_Browser leaving the sizes (0x55FCF0 at 0x55FE8D, after its
///   own 0x4C2100(0x1D));
/// * UI_ICEMAIN leaving a list (0x5605C0 at 0x5606C1; 0x8ACAA8DF at
///   0x568A44) and leaving the screen with 0x83898C or 0x838998 set
// prov: inferred game.frontend_rig_hash
///   (0x568D0D; **inferred**: the flag 0x86461A it also checks is the
///   screen's way out);
/// * UI_CustomNeonMain leaving its list (0x8ACAA8DF at 0x56A903; the list
///   0x56A310 at 0x56A814);
/// * UI_ChooseVinylLayer's constructor (0x554CB0, vtable 0x79CC3C, at
///   0x554CFC).
/// UI_Paint, the car selects, the category menus and the performance
/// screens never call it, so a paint view stays up to the car select
/// (Wine: Customize → paint → back ×4: the paint camera and steer
/// 16 at Customize Car Select; rims → back: the base camera with the car
// prov: open game.frontend_rig_hash-2
/// still side-on, steer −24). **Open**: UI_DecalMain's 0x56B8D0
/// (0xE1FDE1D1 with +0x170) and UI_ChooseVinylLayer's 0x56AAD5 (the tick
/// after 0xE1FDE1D1 with +0x15B), whose flags are not traced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GarageViewPhase {
    Browser { package: &'static str, list: bool, sub: bool },
    VinylLayer,
    Other,
}
