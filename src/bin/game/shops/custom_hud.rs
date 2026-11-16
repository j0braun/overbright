use crate::*;

impl Game {
    /// A Custom Gauges part's HUDINDEX (0x311151F8; 0 for the stock one).
    pub(crate) fn gauge_hud_index(&self, hash: u32) -> u32 {
        let car = self.profile.as_ref().map_or("", |p| p.car.as_str());
        self.game_data.parts
            .find(car, 0xA3, Some(hash), None)
            .and_then(|p| self.game_data.parts.attribute(p, nfsu2::formats::hash::bin_hash("HUDINDEX")))
            .unwrap_or(0)
    }

    /// The career car's top rpm (the engine record's +0x1F8 that 0x549010
    /// reads; **inferred**: the stock car type's, its performance parts
    /// left out).
    pub(crate) fn car_max_rpm(&self) -> f32 {
        let car = self.profile.as_ref().map_or("", |p| p.car.as_str());
        self.game_data.car_types.iter().find(|t| t.name == car).map_or(0.0, |t| t.handling.engine.max_rpm)
    }

    /// A colour slot's fitted part (slots 0xA4..0xA6) and its RED / GREEN /
    /// BLUE (0x4F5100, 0x50F650).
    pub(crate) fn gauge_colour(&self, slot: u8) -> Option<(u32, [u8; 3])> {
        let p = self.profile.as_ref()?;
        let &hash = p.installed.get(&slot)?;
        let part = self.game_data.parts.find(&p.car, slot as usize, Some(hash), None)?;
        let c = |n: &str| self.game_data.parts.attribute(part, nfsu2::formats::hash::bin_hash(n)).unwrap_or(0).min(255) as u8;
        Some((hash, [c("RED"), c("GREEN"), c("BLUE")]))
    }

    /// UI_ChooseCustomHUD on the backing colour (0xFD7C68D8 → 0x549290(0)):
    /// each step's swatches from its slot's parts (0x5449D0), the picks
    /// starting from the fitted colour parts (0x50F650).
    pub(crate) fn choose_custom_hud(&self, category: usize, part: usize) -> Mode {
        use nfsu2::frontend::custom_hud::{ChooseCustomHud, STEPS, Swatch};
        let car = self.profile.as_ref().map_or(String::new(), |p| p.car.clone());
        let palette = |slot: u8| {
            let mut out = Vec::new();
            let mut after = None;
            while let Some(x) = self.game_data.parts.find_after(&car, slot as usize, after) {
                after = Some(x);
                let c =
                    |n: &str| self.game_data.parts.attribute(x, nfsu2::formats::hash::bin_hash(n)).unwrap_or(0).min(255) as u8;
                out.push(Swatch { hash: x.hash, rgb: [c("RED"), c("GREEN"), c("BLUE")] });
            }
            out
        };
        let palettes = std::array::from_fn(|k| palette(STEPS[k].palette));
        let fitted = std::array::from_fn(|k| self.gauge_colour(STEPS[k].slot).map(|c| c.0));
        let hash = self.specialties_gauge(category, part);
        let mut c = ChooseCustomHud::new(
            category,
            part,
            self.gauge_hud_index(hash),
            self.car_max_rpm(),
            self.settings.metric,
            palettes,
            fitted,
            self.now(),
        );
        // Lists Open on Installed (clone deviation, off = native swatch 1).
        if nfsu2::qol::lists_open_installed() {
            c.open_on_fitted();
        }
        Mode::Menu(Menu::ChooseHud(c))
    }

    /// The specialties category's nth part's hash.
    pub(crate) fn specialties_gauge(&self, category: usize, part: usize) -> u32 {
        match self.specialties_browser(category, Some(part)) {
            Mode::Menu(Menu::Parts(b)) => b.categories.get(category).and_then(|c| c.parts.get(part)).map_or(0, |p| p.0),
            _ => 0,
        }
    }

    /// The overlay's purchase after the needle colour (0x9E382ECD →
    /// 0x563D6C with the step 0x838560 = 0 → 0x564145, **verified**): in a
    /// career (0x850078 = 1, a gauge with a HUDINDEX) the bank short of
    /// the price (0x514280 against 0x861E74, `jge`) shows "You do not have
    /// enough bank…" (0xD1AC60BE, OK 0x639, 0x5641FB–0x564246), else "Would
    /// you like to purchase this part?" (0x44B7167F, Yes 0x09F00F07); outside
    /// a career "Would you like to install this part now?" (0xF5AC2B2A, Yes
    /// 0x51123592; 0x564273). Both questions on GenericDialog_SMALL with
    /// Yes / No (0xF770 / 0x61C), No focused (0x5587A0's 1). Every one of
    /// them dims the overlay (0x5642A4: 0x838970 = 0xB4623F67, 0x560B70 =
    /// DIM on UI_CustomHUDOverlay's 0x47FF4E7C); No / OK (0xB4623F67,
    /// 0x5640BD) undims and puts the picker back on the needle colour
    /// (0x838560 = 4), the picks kept. Yes: `fit_custom_gauge`.
    pub(crate) fn buy_custom_gauge(&mut self, c: &nfsu2::frontend::custom_hud::ChooseCustomHud) {
        let hash = self.specialties_gauge(c.category, c.part);
        let Some(part) = self.profile.as_ref().and_then(|p| self.game_data.parts.find(&p.car, 0xA3, Some(hash), None)).copied()
        else {
            return;
        };
        let (_, _, price, _, _) = self.browser_part(0xA3, &part);
        let mut back = c.clone();
        back.opened = self.now();
        back.moved = None;
        // Step 0: the picker hidden under the dialogs (0x56455C), shown
        // again when they go back to step 4 (`menu_tick`).
        back.hidden = true;
        let back = Mode::Menu(Menu::ChooseHud(back));
        let career = self.profile.as_ref().is_some_and(|p| !p.scratch);
        let mut mode = if career && self.profile.as_ref().is_some_and(|p| nfsu2::frontend::career::short_of_bank(p.cash, price)) {
            let text = self.label(0xD1AC_60BE, "You do not have enough bank to purchase this item.");
            let ok = self.label(0x0639, "OK");
            Mode::Menu(Menu::Dialog(Dialog::new("GenericDialog_SMALL", &text, &[&ok], self.now()), vec![back]))
        } else {
            // 0x564248 / 0x564273: the question; Yes buys in a career
            // (0x9F00F07 → 0x563DA4: 0x528A00 then 0x55C240), else only
            // fits (0x51123592).
            let question = if career { 0x44B7_167F } else { 0xF5AC_2B2A };
            let yes = Mode::Menu(Menu::LotDeal(LotDeal::FitGauge(Box::new(c.clone()))));
            let mut m = self.ask_in("GenericDialog_SMALL", question, yes, back);
            if let Mode::Menu(Menu::Dialog(d, _)) = &mut m {
                d.under = Some(1);
            }
            m
        };
        // Over the picker, the overlay under it dimmed (0x560B70).
        if let Mode::Menu(Menu::Dialog(d, _)) = &mut mode {
            d.under.get_or_insert(0);
            d.dim_under = Some(("UI_CustomHUDOverlay", screens::DIM_HANDLER));
        }
        self.mode = mode;
    }

    /// The question's Yes (0x9F00F07 in a career, 0x51123592 outside one;
    /// 0x563DA4 / 0x563F01): the gauge bought (0x528A00) and fitted
    /// (0x55C240), then the picks fitted to slots 0xA4, 0xA6 and 0xA5
    /// (0x631D30 with 0x839C24 / 28 / 2C); back to the categories.
    pub(crate) fn fit_custom_gauge(&mut self, c: &nfsu2::frontend::custom_hud::ChooseCustomHud) {
        use nfsu2::frontend::custom_hud::STEPS;
        let hash = self.specialties_gauge(c.category, c.part);
        let Some(part) = self.profile.as_ref().and_then(|p| self.game_data.parts.find(&p.car, 0xA3, Some(hash), None)).copied()
        else {
            return;
        };
        let (_, _, price, owned, _) = self.browser_part(0xA3, &part);
        if let Some(p) = self.profile.as_mut() {
            if !owned && !p.scratch {
                p.cash -= price as u32;
                p.own_part(0xA3, hash);
                // 0x528A00: the spend (+0x7F0) too.
                p.spent = p.spent.wrapping_add(price.max(0) as u32);
            }
            let before = p.installed.clone();
            p.fit_visual(&self.game_data.parts, 0xA3, hash);
            for (k, step) in STEPS.iter().enumerate() {
                if let Some(h) = c.chosen[k] {
                    p.fit_visual(&self.game_data.parts, step.slot, h);
                }
            }
            // 0x563E0F … 0x563FBA: the ride stored after each fit.
            p.store_visual(&self.game_data.parts, &self.game_data.career.packages, &before);
            self.drop_drive();
            // 0x563EE7: 0x521290 once the picks are fitted.
            self.install_sound(0xA3);
        }
        let rating = self.visual_rating();
        if let Some(p) = self.profile.as_mut() {
            p.offer_covers(&self.game_data.career, rating);
            let _ = p.commit();
        }
        self.mode = self.specialties_browser(c.category, None);
    }
}
