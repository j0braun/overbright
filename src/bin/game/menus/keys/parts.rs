use crate::*;

impl Game {
    pub(super) fn parts_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Parts(b)) = &mut self.mode else { unreachable!() };
        let (sub, category, b_package) = (b.sub, b.category, b.package);
        let vinyls = b_package == "UI_GenericParts_Browser" && sub.is_some_and(|s| (0x40..=0x43).contains(&s));
        if self.parts_buttons(key, sub, category, b_package)? {
            return Ok(());
        }
        let Some(i) = self.parts_step(key, now, sub, category, b_package, vinyls)? else {
            return Ok(());
        };
        self.parts_install(i, sub, category, b_package, vinyls)
    }

    /// The browser's keys before its own (`PartsBrowser::key`): the hood and door
    /// toggles, Remove All, the purge test, the unique part, and the strips' accept
    /// checks. True when one of them took the key.
    fn parts_buttons(&mut self, key: Key, sub: Option<u8>, category: usize, b_package: &'static str) -> Result<bool> {
        let Mode::Menu(Menu::Parts(b)) = &mut self.mode else { unreachable!() };
        // UI_DecalMain's Remove All (0xC519BFC0 on a door, panel or
        // layout, the decals not open): "Do you wish to remove all
        // decals in this layout?" (0x30F7037F).
        // UI_IcePartsOverlay's BUTTON1 (0xC519BFC0 in 0x568D40) in
        // the Split Hoods (slot 9) or Doors (0xA0) list: the
        // category's channel (item +0x54: the hood 0, the doors 2)
        // opens when shut and shuts when open (0x4C18B0 = 0x433CD0
        // on the FE car → 0x521050 → 0x4C1860(channel, open, 1.0),
        // channel 2 also 3); while it moves, nothing. **Not
        // ported**: the doors' branch for GarageMain +0x13C = 1
        // (0x4C2030, a deferred toggle in 0x838559 / 0x83855A).
        if b_package == "UI_ICEMAIN"
            && key == Key::Button1
            && b.part.is_some()
            && let Some(slot) = b.categories.get(category).map(|c| c.slot).filter(|s| matches!(s, 0x09 | 0xA0))
        {
            let channel = if slot == 9 { 0 } else { 2 };
            let state = self.render.garage.as_ref().filter(|g| g.car.is_some()).map(|g| g.channels.state(channel));
            let part = b.part;
            let mut open = self.garage_fe.toggled.filter(|t| t.0 == slot && t.1 == part).map_or([false; 4], |t| t.2);
            match state {
                // No FE car built yet (only a headless shot's keys
                // come before the first draw): as shut.
                Some(nfsu2::part_anim::State::Shut) | None => open[channel] = true,
                Some(nfsu2::part_anim::State::Open) => open[channel] = false,
                _ => return Ok(true),
            }
            if channel == 2 {
                open[3] = open[2];
            }
            self.garage_fe.toggled = Some((slot, part, open));
            return Ok(true);
        }
        if b_package == "UI_DecalMain" && key == Key::Button1 && b.part.is_none() {
            let wide = self.fitted(6).and_then(|h| self.car_part(6, h)).is_some_and(|p| p.level != 0);
            let options = graphics::decal_options(self.shop.decal_node, wide);
            if let Some(graphics::DecalOption { item: DecalItem::Node(n), .. }) = options.get(category)
                && graphics::decal_node_slots(*n).is_some()
            {
                let n = *n;
                let (yes, no) = self.yes_no();
                let (a, b) = (
                    self.decal_shop(self.shop.decal_node, category, None),
                    self.decal_shop(self.shop.decal_node, category, None),
                );
                self.mode = self.setup_dialog_in(
                    Some("GenericDialog_SMALL"),
                    0x30F7_037F,
                    vec![(yes, Some(SetupStep::RemoveDecals(n)), a), (no, None, b)],
                );
                // StartDim after the ask (0x56B7AC).
                if let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode {
                    d.dim_under = Some(("UI_DecalMain", screens::DIM_HANDLER));
                }
            }
            return Ok(true);
        }
        // UI_IcePartsOverlay's 0xC519BFC0 (BUTTON1, the C key, or
        // the help bar's Test N2O Purge: 0xC539A4B0, which
        // UI_ICEMAIN's handler forwards as 0xC519BFC0, 0x568BC2)
        // in the Nitrous Purge list (0x568D40): +0x116 toggles,
        // on → FX_PURGE (0x458B30), off → its stop (0x458C40).
        // The split hoods' and doors' Open/Close (0x521050 on the
        // item's channel) are **not ported**.
        if b_package == "UI_ICEMAIN"
            && key == Key::Button1
            && sub.is_none()
            && b.part.is_some()
            && b.categories.get(category).is_some_and(|c| c.slot == 0xA2)
        {
            self.purge_test_set(!self.shop.purge_test);
            return Ok(true);
        }
        // UI_ChooseUniquePart's strip: Accept on an option answers
        // 0xD05FC3A3 (0x561730).
        if b_package == "UI_ChooseUniquePart" && key == Key::Accept {
            let item = self.unique_reward().get(category).copied();
            if item.is_some() {
                self.take_unique(item)?;
                return Ok(true);
            }
            return Ok(true);
        }
        self.parts_accept_checks(key, sub, category, b_package)
    }

    /// The ICE and neon strips' accept checks (nitrous for the purge, hood decals for a
    /// custom hood, the neon's required upgrades). True when one of them took the key.
    fn parts_accept_checks(
        &mut self,
        key: Key,
        sub: Option<u8>,
        category: usize,
        b_package: &'static str,
    ) -> Result<bool> {
        let Mode::Menu(Menu::Parts(b)) = &mut self.mode else { unreachable!() };
        // UI_ICEMAIN's accept on its strip (0x0C407210 in 0x568750,
        // after the locked item's WRONG):
        // (`at`: the part the list opens on, as `PartsBrowser::key`.)
        let ice_slot = (b_package == "UI_ICEMAIN" && sub.is_none() && b.part.is_none() && key == Key::Accept)
            .then(|| {
                b.categories
                    .get(category)
                    .filter(|c| !c.locked)
                    .map(|c| (c.slot, c.parts.iter().position(|p| p.4).unwrap_or(0)))
            })
            .flatten();
        // (Field borrows: the browser stays borrowed.)
        let nitrous = self.profile.as_ref().is_some_and(|p| [12u8, 13, 14].iter().any(|id| p.perf.contains(id)));
        let hood_decal = self.profile.as_ref().is_some_and(|p| (0x61..=0x68).any(|s| p.installed.contains_key(&s)));
        if let Some((slot, at)) = ice_slot {
            // Nitrous Purge (slot 0xA2) without a nitrous part
            // fitted (0x50F180: perf ids 12–14, 0x599620) only
            // tells "Dude, you gotta install nitrous…"
            // (0x20B30CD8, GenericDialog_SMALL by 0x558440) and
            // StartDims (0x568894); its OK 0x34DC1BEC is StopDim.
            // Split Hoods (slot 9) with a hood decal on (slots
            // 0x61–0x68, 0x610300) warns "Installing a custom
            // hood will remove any hood decals." (0x903854F0, ""
            // sized by length), StartDims (0x568927) and still
            // opens the list (0x556940): the list under it.
            if slot == 0xA2 && !nitrous {
                let text = self.label(0x20B3_0CD8, "");
                let again = self.specialties_browser(category, None);
                let m = Mode::Menu(Menu::Dialog(
                    Dialog::new("GenericDialog_SMALL", &text, &[&self.label(0x0639, "OK")], self.now()),
                    vec![again],
                ));
                self.mode = dialog_over(m, true, None);
                return Ok(true);
            }
            if slot == 9 && hood_decal {
                let text = self.label(0x9038_54F0, "");
                let list = self.specialties_browser(category, Some(at));
                self.mode = dialog_over(self.dialog(&text, list), true, None);
                return Ok(true);
            }
        }
        // UI_CustomNeonMain's accept on an open category (0x56A890,
        // 0x0C407210 with item +0x22 clear): the car as fitted
        // (0x8392C0, 0x610300) without a level-3 engine (slot 0x0D,
        // part +5 >> 5) for Engine Neon tells "You need to install
        // all level 3 engine, ECU and turbo upgrades…" (0xD649BE08),
        // without a level-3 trunk layout (slot 0x22) for Trunk Neon
        // "You must upgrade your trunk to CUSTOM LAYOUT…"
        // (0xA9E966D7), GenericDialog_SMALL by 0x558440 (no
        // StartDim), its OK 0x34DC1BEC back on the strip; else the
        // list opens (0x557140). Under Wine: both, on the finished-career
        // save's stock RX-7 and its 350Z.
        if b_package == "UI_CustomNeonMain"
            && b.part.is_none()
            && key == Key::Accept
            && let Some(c) = b.categories.get(category).filter(|c| !c.locked && matches!(c.slot, 0x9B | 0x9D))
            && let Some(p) = self.profile.as_ref()
        {
            let slot = c.slot;
            let fitted = nfsu2::car::perf_visual_parts(&p.installed, &self.game_data.parts, &p.car, &self.game_data.career.packages, &|id| {
                p.perf.contains(&id)
            });
            let level = |s: u8| fitted.get(&s).and_then(|&h| self.car_part(s, h)).map_or(0, |x| x.level);
            let text = match slot {
                0x9B if level(0x0D) < 3 => Some(self.label(0xD649_BE08, "")),
                0x9D if level(0x22) < 3 => Some(self.label(0xA9E9_66D7, "")),
                _ => None,
            };
            if let Some(text) = text {
                let again = self.neon_browser(category, None);
                let m = Mode::Menu(Menu::Dialog(
                    Dialog::new("GenericDialog_SMALL", &text, &[&self.label(0x0639, "OK")], self.now()),
                    vec![again],
                ));
                self.mode = dialog_over(m, false, None);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The item picked: a vinyl, a decal, or a part to fit.
    fn parts_install(
        &mut self,
        i: usize,
        sub: Option<u8>,
        category: usize,
        b_package: &'static str,
        vinyls: bool,
    ) -> Result<()> {
        let Mode::Menu(Menu::Parts(b)) = &mut self.mode else { unreachable!() };
        let c = &b.categories[category];
        // A vinyl (0x5664A0): into the setup on the layer
        // (0x54FDC0); one that takes colours goes on to them
        // (UI_Paint), else back to the layers.
        // Select asks first (0x566D80 → 0x559250; Yes 0x51123592
        // → 0x566F12 → 0x5670F9): in a career the vinyl's price
        // (0x514280) plus the setup's (0x521540) must be within
        // the bank (0x861E74), else "You do not have enough bank…"
        // (0xD1AC60BE, 0x558440); then "Would you like to add this
        // vinyl to your setup?" (0x31FBC167), or for a unique vinyl
        // (NUMCOLOURS 0x564B8CB6 = 0x40) while a unique one is
        // fitted or in the setup (0x5215E0) or the setup holds a
        // vinyl on layers 1–3 (0x40–0x42) the unique warning
        // 0xEC8616FD; Yes / No (0xF770 / 0x061C), No focused
        // (0x5587A0's 1), No back to the list (0x34DC1BCF).
        if vinyls {
            let (layer, hash) = (c.slot, c.parts[i].0);
            let back = self.vinyl_browser(layer, Some(category), Some(i));
            let cost = self.setup_price(layer, hash) + self.graphics_total().unwrap_or(0);
            if let Some(p) = self.profile.as_ref()
                && !p.scratch
                && (p.cash as i32) < cost
            {
                let text = self.label(0xD1AC_60BE, "");
                // Over the browser, no StartDim (0x5592D6 returns):
                // the browser stays as under the vinyl question.
                self.mode = dialog_over(self.dialog(&text, back), false, None);
                return Ok(());
            }
            let others = self.fitted_unique()
                || self.setup_unique()
                || self.shop.setup.items.iter().any(|&(s, h)| (0x40..0x43).contains(&s) && h != 0);
            let question = if self.unique_vinyl(hash) && others { 0xEC86_16FD } else { 0x31FB_C167 };
            let (yes, no) = self.yes_no();
            let again = self.vinyl_browser(layer, Some(category), Some(i));
            // GenericDialog_MED.fng (0x78F2F8 at 0x5593B3).
            self.mode = self.setup_dialog_in(
                Some("GenericDialog_MED"),
                question,
                vec![(yes, Some(SetupStep::Vinyl { layer, hash }), again), (no, None, back)],
            );
            if let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode {
                d.selected = 1;
            }
            return Ok(());
        }
        // A decal (UI_DecalsOverlay): into the setup, "No Decal"
        // taking the slot's off.
        if b_package == "UI_DecalMain" {
            let (slot, hash) = (c.slot, c.parts[i].0);
            if hash == 0 {
                let fitted = self.fitted(slot).is_some();
                self.shop.setup.remove(slot, fitted);
            } else {
                self.shop.setup.put(slot, hash);
            }
            self.mode = self.decal_shop(self.shop.decal_node, category, Some(i));
            return Ok(());
        }
        {
            let (slot, (hash, _, price, owned, installed)) = (c.slot, c.parts[i].clone());
            // The Custom Gauges overlay's accept (0x563D00 at
            // 0x563D6C): a gauge with a HUDINDEX (0x61B510 of
            // 0x311151F8) sends 0xFD7C68D8 to UI_ChooseCustomHUD,
            // which takes the colours before the purchase. The stock
            // one (HUDINDEX 0) does nothing: the overlay's constructor
            // 0x56C220 sets the step 0x838560 = 1 (0x56C283) and only
            // the picker's needle Accept sets 0 (0x56455C), so at
            // 0x564159–0x564192 a HUDINDEX of 0 with the step > 0
            // returns (0x5644D9) without a purchase or a dialog.
            if b_package == "UI_ICEMAIN" && sub.is_none() && slot == 0xA3 {
                if self.gauge_hud_index(hash) != 0 {
                    let picker = self.choose_custom_hud(category, i);
                    if let Mode::Menu(m) = std::mem::replace(&mut self.mode, picker) {
                        self.fe.hud_overlay = Some(Box::new(m));
                    }
                }
                return Ok(());
            }
            // UI_Rims_Browser's and UI_ChooseSpinner's size accept
            // (0x559AC0) asks first (`rim_question`).
            if matches!(b_package, "UI_Rims_Browser" | "UI_ChooseSpinner") {
                self.mode = self.rim_question(b_package, sub, category, i, slot, hash, price, owned, installed);
                return Ok(());
            }
            // UI_GenericParts_Browser asks first (`body_part_question`).
            if b_package == "UI_GenericParts_Browser" {
                self.mode = self.body_part_question(sub, category, i, slot, hash, price, owned, installed);
                return Ok(());
            }
            // The specialties' and the neon's lists ask too (
            // `ice_part_question`).
            if matches!(b_package, "UI_ICEMAIN" | "UI_CustomNeonMain") {
                self.mode = self.ice_part_question(b_package, sub, category, i, slot, hash, price, owned, installed);
                return Ok(());
            }
            self.fit_browser_part(b_package, sub, category, i, slot, hash, price, owned);
        }
        Ok(())
    }
}
