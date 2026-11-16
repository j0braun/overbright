use crate::*;

impl Game {
    /// The rim brands for the car (0x546530): SUVs (car type +0x88A) have
    /// their own.
    pub(crate) fn rim_brands(&self) -> &'static [(u32, u32, u32)] {
        if self.career_car_type().is_some_and(|c| c.suv) { &screens::SUV_RIM_BRANDS } else { &screens::RIM_BRANDS }
    }

    /// A brand's rims (0x539B80): the stock brand's are the level-0 rims,
    /// the others their own of the car's rim size (car type +0xDC).
    pub(crate) fn brand_rims(&self, brand: u32) -> Vec<&nfsu2::formats::parts::Part> {
        let size = self.career_car_type().map_or(0, |c| c.rim_size);
        self.slot_parts(0x1D, false)
            .into_iter()
            .filter(|p| {
                if brand == screens::STOCK_RIMS {
                    p.level == 0
                } else {
                    self.game_data.parts.attribute(p, 0xEBB0_3E66) == Some(brand)
                        && self.game_data.parts.attribute(p, 0xCE7D_8DB5) == Some(u32::from(size))
                }
            })
            .collect()
    }

    /// UI_ChooseRimBrand (screen 0x554A70; 0x5463C0 adds each brand): a
    /// brand without rims is locked, stock never.
    /// UI_ChooseRimBrand's opening entry (0x546530): the strip's remembered
    /// brand, seeded with the fitted rim's brand (slot 0x1D, attribute
    /// 0xEBB03E66) when it has none (Wine, the finished-career save's O.Z. rims: the
    /// strip opened on O.Z.); the first entry without one.
    pub(crate) fn fitted_rim_brand(&self) -> usize {
        let Some(p) = self.profile.as_ref() else { return 0 };
        let Some(h) = self.effective(0x1D) else { return 0 };
        let Some(part) = self.game_data.parts.find(&p.car, 0x1D, Some(h), None) else { return 0 };
        let Some(brand) = self.game_data.parts.attribute(part, 0xEBB0_3E66) else { return 0 };
        self.rim_brands().iter().position(|b| b.0 == brand).unwrap_or(0)
    }

    pub(crate) fn rim_brand_menu(&self, selected: usize) -> Mode {
        let options = self
            .rim_brands()
            .iter()
            .enumerate()
            .map(|(i, &(brand, icon, label))| {
                let locked = brand != screens::STOCK_RIMS && self.brand_rims(brand).is_empty();
                screens::IconOption {
                    icon,
                    label,
                    action: if locked { Action::Unavailable } else { Action::RimBrand(i) },
                }
            })
            .collect();
        let mut menu = IconMenu::new(MenuKind::RimBrand.package(), None, options, self.now());
        menu.select_now(selected);
        Mode::Menu(Menu::Icons(menu, MenuKind::RimBrand))
    }

    /// UI_Rims_Browser for a brand (0x55F350): its rims grouped by name
    /// into styles on the strip (icon 0x0294D2A3, label the name's hash),
    /// each style's sizes down the column; it opens on the fitted rim's
    /// style when it is the brand's.
    pub(crate) fn rims_browser(&self, brand: usize, style: Option<usize>, part: Option<usize>) -> Mode {
        let (hash, _, _) = self.rim_brands()[brand];
        self.styles_browser("UI_Rims_Browser", hash, Some(brand as u8), style, part)
    }

    /// UI_ChooseSpinner (factory 0x56C100 → 0x569510, fill 0x560D40,
    /// messages 0x569680), from the Spinners item of UI_ICEMAIN (slot 0x1D:
    /// 0x568750 sets 0x838998 = 8 and the next screen, 0x50E3D0, is this
    /// one): the rims browser's fill for the brand "SPINNER" (0x539B80 with
    /// bin_hash("SPINNER"), the car's slot-0x1D rims of its group, 0x60FEF0)
    /// — styles by name on OPTION_MASTER (icon 0x0294D2A3), each style's
    /// sizes down MINI_MASTER, opening on the fitted rim's style when it is a
    /// spinner. Back returns to UI_ICEMAIN (0x520A90, 0x838998 = 0).
    pub(crate) fn spinner_browser(&self, style: Option<usize>, part: Option<usize>) -> Mode {
        self.styles_browser("UI_ChooseSpinner", nfsu2::formats::hash::bin_hash("SPINNER"), None, style, part)
    }

    pub(crate) fn styles_browser(
        &self,
        package: &'static str,
        hash: u32,
        sub: Option<u8>,
        style: Option<usize>,
        part: Option<usize>,
    ) -> Mode {
        use screens::{BrowserCategory, CategoryAction};
        let mut categories: Vec<BrowserCategory> = Vec::new();
        for p in self.brand_rims(hash) {
            let name = self.game_data.parts.string(p.name as u32).unwrap_or_default();
            let mut entry = self.browser_part(0x1D, p);
            // UI_Rims_Browser's part name (0x5468B0 on 0xD28B9316): "%s %d\""
            // prov: open game.styles_browser
            // (0x79C1CC) of the part's name (0x610010; **open**: the
            // attribute 0x4732DA07 taking its place when present) and its
            // size, the attribute 0xEB0101E2's low byte, signed.
            if package == "UI_Rims_Browser"
                && let Some(size) = self.game_data.parts.attribute(p, 0xEB01_01E2)
            {
                entry.1 = format!("{name} {}\"", size as u8 as i8);
            }
            match categories.last_mut() {
                Some(c) if c.text.as_deref() == Some(name.as_str()) => c.parts.push(entry),
                _ => categories.push(BrowserCategory {
                    slot: 0x1D,
                    icon: 0x0294_D2A3,
                    label: nfsu2::formats::hash::bin_hash(&name),
                    parts: vec![entry],
                    locked: false,
                    action: CategoryAction::Parts,
                    text: Some(name),
                }),
            }
        }
        // UI_Rims_Browser's title (0x5468B0 on +0x134): "%s %s" (0x794FD8)
        // of the brand's name and the style's. 0x50E960(brand 0x839BBC):
        // the brand record's name (+0, 0x4FFF40 searching the table
        // 0x838400 by +0x40), else (a switch of the brand hashes) its
        // label's language string.
        if package == "UI_Rims_Browser"
            && let Some(&(hash, _, label)) = sub.and_then(|b| self.rim_brands().get(usize::from(b)))
            && let Some(brand) = self
                .game_data.career
                .brands
                .iter()
                .find(|b| b.hash == hash)
                .map(|b| b.name.as_str())
                .or_else(|| self.game_data.assets.strings.get(label))
        {
            for c in &mut categories {
                c.text = c.text.take().map(|name| format!("{brand} {name}"));
            }
        }
        let style = style.or_else(|| categories.iter().position(|c| c.parts.iter().any(|p| p.4))).unwrap_or(0);
        Mode::Menu(Menu::Parts(PartsBrowser {
            package,
            title: 0,
            sub,
            categories,
            category: style,
            part,
            opened: self.now(),
            total: None,
            arrows: Default::default(),
        }))
    }
}
