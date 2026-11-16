use crate::*;

impl Game {
    /// The browser's own key and what its step does short of fitting a part (leaving,
    /// opening, toggling); the item to fit, if any.
    pub(super) fn parts_step(
        &mut self,
        key: Key,
        now: f32,
        sub: Option<u8>,
        category: usize,
        b_package: &'static str,
        vinyls: bool,
    ) -> Result<Option<usize>> {
        let Mode::Menu(Menu::Parts(b)) = &mut self.mode else { return Ok(None) };
        let part_before = b.part;
        let category_before = b.category;
        let step = b.key(key);
        // The category strip's arrow that way blinks (0x52FA80).
        if b.part.is_none() && part_before.is_none() && b.category != category_before {
            b.arrows.step(b.category > category_before, now, false);
        }
        // A step in the neon list (NeonOverlay_Step 0x55CAD0) fits
        // the highlighted item on the preview ride (0x55C180), whose
        // 0x55B1B0 pushes the Visual Rating widget (0x557ED0) when
        // it is not up; it stays until a screen change pops it
        // (0x55BCA0 → 0x53D8D0). Under Wine (menus103): none on
        // entering the neon list ("NO NEON"), 0.14 on RED. (The
        // UI_ICEMAIN lists: see `shop_rating_quads`.)
        if let Mode::Menu(Menu::Parts(b)) = &self.mode
            && part_before.is_some()
            && b.part != part_before
            && b.part.is_some()
            && b_package == "UI_CustomNeonMain"
        {
            self.fe.overlay_rating = Some(b_package);
        }
        // Carbon fibre's Hood opened (0x5664A0 at 0x566786, +0x1D5 set,
        // slot 9, the list not open): with a vinyl of kind 0x14 on a layer
        // the warning first, over the list it opens (`carbon_hood_warning`).
        if let Mode::Menu(Menu::Parts(b)) = &self.mode
            && sub == Some(0x5E)
            && part_before.is_none()
            && b.part.is_some()
            && b.categories.get(b.category).is_some_and(|c| c.slot == 9)
            && let Some(text) = self.carbon_hood_warning()
        {
            let text = self.label(text, "");
            let ok = self.label(0x0639, "OK");
            let mut d = Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&ok], self.now());
            d.dim_scripts = Some((nfsu2::frontend::MENUFADEOUT_SCRIPT, nfsu2::frontend::UNDIM_SCRIPT));
            let under = std::mem::replace(&mut self.mode, Mode::Explore);
            self.mode = dialog_over(Mode::Menu(Menu::Dialog(d, vec![under])), true, None);
            return Ok(None);
        }
        let Mode::Menu(Menu::Parts(b)) = &mut self.mode else { return Ok(None) };
        let i = match step {
            screens::BrowserKey::None => return Ok(None),
            // Back from the vinyls: the layers (0x50E430).
            screens::BrowserKey::Leave if vinyls => {
                let layer = sub.unwrap_or(0x40);
                self.mode = self.vinyl_layer_menu((layer - 0x40) as usize);
                return Ok(None);
            }
            // Back up UI_DecalMain's tree (0x560030 with -1), from
            // its top to the graphics shop.
            screens::BrowserKey::Leave if b_package == "UI_DecalMain" => {
                self.mode = match graphics::decal_parent(self.shop.decal_node) {
                    Some(parent) => {
                        let at = graphics::decal_options(parent, false)
                            .iter()
                            .position(|o| o.item == DecalItem::Node(self.shop.decal_node))
                            .unwrap_or(0);
                        self.shop.decal_node = parent;
                        self.decal_shop(parent, at, None)
                    }
                    None => self.graphics_menu(2),
                };
                return Ok(None);
            }
            // A node (0x56B3C0 → 0x5682B0): a layout asks first
            // when it changes the layout and decals are on the old
            // one ("Choosing a new layout will remove decals from
            // the previous layout. Continue?", 0x10B83327); the
            // hood only takes decals over the stock hood
            // (0xD104AD36).
            screens::BrowserKey::Open if b_package == "UI_DecalMain" => {
                self.decal_open_key(category)?;
                return Ok(None);
            }
            // Back from rims or carbon fibre: the body categories
            // on the one left (0x839BC0 remembers rims).
            // Back from a rim brand's styles: the brands on it.
            screens::BrowserKey::Leave if b.package == "UI_CustomNeonMain" => {
                let at = screens::SPECIALTIES.iter().position(|c| c.0 == 0x9E).unwrap_or(0);
                self.mode = self.specialties_browser(at, None);
                return Ok(None);
            }
            screens::BrowserKey::Leave if b.package == "UI_ICEMAIN" && sub == Some(0x23) => {
                self.mode = self.audio_browser(false, 0, None);
                return Ok(None);
            }
            // Back from the trunk audio: mode 0 set up again
            // (0x5593F0 case 0), on its first category (Wine, the
            // finished-career save: Custom Gauges highlighted as the hatch
            // shuts).
            screens::BrowserKey::Leave if b.package == "UI_ICEMAIN" && sub == Some(0x22) => {
                self.mode = self.specialties_browser(0, None);
                return Ok(None);
            }
            // UI_ICEMAIN's Back (0x911AB364 in 0x568750) in a
            // career (0x850078 = 1) shop (0x83898C = 0) on its top
            // list (0x838998 = 0): the exit question (0x568B66).
            screens::BrowserKey::Leave if b.package == "UI_ICEMAIN" && self.shop.garage_parts && sub.is_none() => {
                self.mode = self.storage_menu(2);
                return Ok(None);
            }
            screens::BrowserKey::Leave if b.package == "UI_ICEMAIN" => {
                if self.career_shop() {
                    let again = self.specialties_browser(category, None);
                    self.mode = self.shop_exit_question(again);
                } else {
                    self.leave_shop()?;
                }
                return Ok(None);
            }
            screens::BrowserKey::Leave if b.package == "UI_ChooseUniquePart" => {
                let text = self.label(0x20D0_D219, "You sure you don't want a unique part?");
                let (yes, no) = self.yes_no();
                let again = self.unique_part_screen();
                // StartDim after the ask (0x561801).
                self.mode = dialog_over(
                    Mode::Menu(Menu::Dialog(
                        Dialog::new("GenericDialog_SMALL", &text, &[&yes, &no], self.now()),
                        vec![Mode::Explore, again],
                    )),
                    true,
                    None,
                );
                return Ok(None);
            }
            screens::BrowserKey::Leave if b.package == "UI_ChooseSpinner" => {
                let at = screens::SPECIALTIES.iter().position(|c| c.0 == 0x1D).unwrap_or(0);
                self.mode = self.specialties_browser(at, None);
                return Ok(None);
            }
            screens::BrowserKey::Leave if b.package == "UI_Rims_Browser" => {
                self.mode = self.rim_brand_menu(sub.unwrap_or(0) as usize);
                return Ok(None);
            }
            // Back from carbon fibre: the body categories on it.
            screens::BrowserKey::Leave if sub.is_some() => {
                let at = screens::BODY_SHOP_CATEGORIES.iter().position(|c| Some(c.0) == sub).unwrap_or(0);
                self.mode = self.new_parts_browser(at);
                return Ok(None);
            }
            screens::BrowserKey::Leave => {
                self.parts_leave_key(category, b_package)?;
                return Ok(None);
            }
            screens::BrowserKey::Open => {
                self.parts_open_key(category);
                return Ok(None);
            }
            screens::BrowserKey::Toggle => {
                self.parts_toggle_key(sub, category)?;
                return Ok(None);
            }
            screens::BrowserKey::Install(i) => i,
        };
        Ok(Some(i))
    }

    /// The decal shop's Open on a node.
    fn decal_open_key(&mut self, category: usize) -> Result<()> {
        let wide = self.fitted(6).and_then(|h| self.car_part(6, h)).is_some_and(|p| p.level != 0);
        let Some(DecalItem::Node(node)) = graphics::decal_options(self.shop.decal_node, wide).get(category).map(|o| o.item)
        else {
            return Ok(());
        };
        if node == 0x10005 && self.fitted(9).and_then(|h| self.car_part(9, h)).is_some_and(|p| p.level != 0) {
            let text = self.label(0xD104_AD36, "");
            // Over the decal shop, no StartDim (0x56B57C
            // returns).
            self.mode = dialog_over(self.dialog(&text, self.decal_shop(self.shop.decal_node, category, None)), false, None);
            return Ok(());
        }
        match graphics::decal_layout(node) {
            Some((location, first, count)) => {
                let changes = self.layout_count(location) != Some(count);
                let has_decals = (first..first + 8).any(|s| self.effective(s).is_some());
                if changes && has_decals {
                    let (yes, no) = self.yes_no();
                    let (a, b) = (
                        self.decal_shop(self.shop.decal_node, category, None),
                        self.decal_shop(self.shop.decal_node, category, None),
                    );
                    self.mode = self.setup_dialog_in(
                        Some("GenericDialog_SMALL"),
                        0x10B8_3327,
                        vec![(yes, Some(SetupStep::Layout(node)), a), (no, None, b)],
                    );
                    // StartDim after the ask (0x56B5F3).
                    if let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode {
                        d.dim_under = Some(("UI_DecalMain", screens::DIM_HANDLER));
                    }
                } else if changes {
                    self.setup_step(SetupStep::Layout(node))?;
                } else {
                    self.shop.decal_node = node;
                    self.mode = self.decal_shop(node, 0, None);
                }
            }
            None => {
                self.shop.decal_node = node;
                self.mode = self.decal_shop(node, 0, None);
            }
        }
        Ok(())
    }

    /// A toggle category's Accept (`CategoryAction::Toggle`): carbon
    /// fibre's Convert / Revert Trunk (slot 0x60) and Body Kits (0x5D) ask
    /// first (UI_GenericParts_Browser's handler 0x5664A0 on 0x406415E3 with
    /// +0x1D5 set, **verified**: 0x566686 and 0x566895). With the "none"
    /// part 0xC8D313AC fitted the question converts, else it reverts
    /// (`carbon_question`); Yes (0x51123592) converts (`carbon_convert`),
    /// No / Back (0xB4623F67) leaves the strip as it was.
    fn parts_toggle_key(&mut self, sub: Option<u8>, category: usize) -> Result<()> {
        let Mode::Menu(Menu::Parts(b)) = &self.mode else { unreachable!() };
        let c = &b.categories[category];
        let screens::CategoryAction::Toggle(_) = c.action else { return Ok(()) };
        let question = self.carbon_question(c.slot);
        let yes = Mode::Menu(Menu::LotDeal(LotDeal::Convert { sub, category }));
        let text = self.label(question, "?");
        let package = screens::dialog_package(&self.game_data.assets, &text);
        let again = self.parts_browser_in(sub, category, None);
        // 0x5587A0(…, Yes 0xF770, No 0x061C, 0x51123592, 0xB4623F67,
        // 0xB4623F67, 1: No focused, question), then StartDim on the
        // browser (0x537C10 "DIM").
        self.mode = dialog_over(focus_no(self.ask_in(package, question, yes, again)), true, None);
        Ok(())
    }

    /// The convert question for the trunk (0x60) or the kits (0x5D)
    /// (0x566686..0x566728, 0x566895..0x5669E3; **verified**). The fitted
    /// part 0x610300(slot) still the "none" part 0xC8D313AC:
    /// * trunk: "Do you want to convert your trunk to carbon fiber?"
    ///   (0x2B950C36), in a career (0x850078 = 1) with that part not owned
    ///   (0x521BF0 on the slot and the fitted part) the purchase question
    ///   0x3175F2B0;
    /// * kits: with no conflicting vinyl the same pair 0x9E274DAA /
    ///   0xB7C84DA4; with one, 0xEE2C85BB (purchase, "This will cause some
    ///   vinyls to be removed", whatever the career). A vinyl on a layer
    ///   (0x610300(0x40..0x43), kind = +5 & 0x1F) conflicts: with a wide
    ///   body fitted (slot 6, level +5 >> 5 above 0) every kind but 0x14;
    ///   without one the kinds 0x0E, 0x0F and 0x15..0x1A (the jump table
    ///   0x56787C / 0x567884 over kind − 0x0E, 0..0x0C).
    ///
    /// Otherwise the revert question: 0xA645A29E (trunk), 0x660BDF52 (kits).
    pub(crate) fn carbon_question(&self, slot: u8) -> u32 {
        const NONE: u32 = 0xC8D3_13AC;
        let fitted = self.profile.as_ref().and_then(|p| p.installed.get(&slot).copied()).unwrap_or(NONE);
        let unowned = self.profile.as_ref().is_some_and(|p| !p.scratch && !p.owned.contains(&(slot, fitted)));
        if fitted != NONE {
            return if slot == 0x60 { 0xA645_A29E } else { 0x660B_DF52 };
        }
        if slot == 0x60 {
            return if unowned { 0x3175_F2B0 } else { 0x2B95_0C36 };
        }
        let part = |s: u8| self.fitted(s).and_then(|h| self.car_part(s, h));
        let wide = part(6).is_some_and(|p| p.level > 0);
        let conflict = (0x40..0x44).filter_map(part).any(|p| {
            let kind = p.kind & 0x1F;
            if wide { kind != 0x14 } else { matches!(kind, 0x0E | 0x0F | 0x15..=0x1A) }
        });
        if conflict {
            0xEE2C_85BB
        } else if unowned {
            0xB7C8_4DA4
        } else {
            0x9E27_4DAA
        }
    }

    /// The carbon hood's warning (0x566786..0x566862, **verified**): a vinyl
    /// whose kind (+5 & 0x1F) is 0x14 on one of the layers 0x40..0x43
    /// (0x610300) gives "Warning! Installing a carbon fiber hood will cause
    /// any hood vinyls to be removed from your car." (0xB3BBB11D), or with
    /// a decal in one of the slots 0x61..0x68 the "… hood vinyls and decals
    /// …" one (0x9796DFB4): a one-button dialog (0x558440), its OK
    /// 0x34DC1BEC; the browser plays MENUFADEOUT (0x537C10, 0x79BE40) and
    /// the list opens under it (0x55EDE0); OK ends in UNDIM (0x5674D8 with
    /// the list open, 0x79031C). None without such a vinyl.
    pub(crate) fn carbon_hood_warning(&self) -> Option<u32> {
        let part = |s: u8| self.fitted(s).and_then(|h| self.car_part(s, h));
        let hood_vinyl = (0x40..0x44).filter_map(part).any(|p| p.kind & 0x1F == 0x14);
        if !hood_vinyl {
            return None;
        }
        let decal = (0x61..0x69).any(|s| self.fitted(s).is_some());
        Some(if decal { 0x9796_DFB4 } else { 0xB3BB_B11D })
    }

    /// The convert question's Yes (0x566F12 → 0x567098): the carbon (or
    /// back the "none") part for the category's slot, bought first in a
    /// career when not owned (0x528A00), fitted (0x55C240), the strip
    /// built again (0x545D40) and the install sound (0x521290).
    pub(crate) fn carbon_convert(&mut self, sub: Option<u8>, category: usize) -> Result<()> {
        let mode = self.parts_browser_in(sub, category, None);
        let Mode::Menu(Menu::Parts(b)) = &mode else { return Ok(()) };
        let c = &b.categories[category];
        let screens::CategoryAction::Toggle(fit) = c.action else { return Ok(()) };
        let slot = c.slot;
        if let Some(p) = self.profile.as_mut() {
            // The accept (0x566F12, 0x1D5 set): an unowned
            // part (0x521BF0) is bought by 0x528A00 — its
            // 0x514280 price (the level-3 one converting, 0
            // for the level-0 none part) off the bank, not
            // checked first and floored at 0 — then fitted
            // (0x55C240).
            let level = if fit == 0xC8D3_13AC { 0 } else { 3 };
            let price = self.game_data.career.costs.price(nfsu2::formats::parts::SLOT_CATEGORIES[slot as usize], level);
            // In a career only (0x567055).
            if !p.scratch && !p.owned.contains(&(slot, fit)) {
                p.cash = p.cash.saturating_sub(price.max(0) as u32);
                p.own_part(slot, fit);
                // 0x528A00: the spend (+0x7F0) too.
                p.spent = p.spent.wrapping_add(price.max(0) as u32);
            }
            let before = p.installed.clone();
            p.fit_visual(&self.game_data.parts, slot, fit);
            p.store_visual(&self.game_data.parts, &self.game_data.career.packages, &before);
            let _ = p.commit();
            self.drop_drive();
        }
        // 0x5670AC: 0x521290 on the part fitted.
        self.install_sound(slot);
        self.mode = self.parts_browser_in(sub, category, None);
        Ok(())
    }

    /// The generic Back on a parts list: storage, the body shop's exit question, or
    /// out of the shop.
    fn parts_leave_key(&mut self, category: usize, b_package: &'static str) -> Result<()> {
        if self.shop.garage_parts {
            self.mode = self.storage_menu(0);
        } else if b_package == "UI_GenericParts_Browser" && self.career_shop() {
            // The body shop's Back on its strip (0x5671EB:
            // 0x83898C = 0, not the graphics shop's groups
            // +0x1D7, not carbon fibre +0x1D5): the exit
            // question (0x56721E, No focused), the masks off
            // (0x53A2C0), the strip's items hidden
            // (0x52EBA0(0)) and MENUFADEOUT on the handler
            // (0x567272) instead of StartDim. No (0x1FAB5998,
            // 0x5674D8): masks 0xFF, strip shown, MENUFADE;
            // Yes (0xD05FC3A3, 0x5676FE): Shop_LeaveToCity.
            let again = self.parts_browser(category, None);
            self.mode = self.shop_exit_question(again);
            if let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode {
                d.dim_scripts = Some((nfsu2::frontend::MENUFADEOUT_SCRIPT, nfsu2::frontend::MENUFADE_SCRIPT));
            }
        } else {
            self.leave_shop()?;
        }
        Ok(())
    }

    /// Open on a category: its own list or screen.
    fn parts_open_key(&mut self, category: usize) {
        let Mode::Menu(Menu::Parts(b)) = &mut self.mode else { unreachable!() };
        let slot = b.categories[category].slot;
        let ice_main = b.package == "UI_ICEMAIN";
        self.mode = match slot {
            0x1D if ice_main => self.spinner_browser(None, None),
            0x1D => self.rim_brand_menu(self.fitted_rim_brand()),
            0x9E => self.neon_browser(0, None),
            0x22 => self.trunk_audio(),
            0x23 => self.audio_browser(true, 0, None),
            _ => self.parts_browser_in(Some(slot), 0, None),
        };
    }
}
