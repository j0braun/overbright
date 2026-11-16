use crate::*;

impl Game {
    /// UI_Paint on paint slot `slot` (PAINT_SLOTS) and `finish` (the
    /// slot's own when None), in it on `selected` when given. A slot's
    /// paints are those of its finish (0x5449D0 with the finish as the
    /// brand) that the shop lists (stock, owned or unlocked).
    pub(crate) fn paint_shop(&self, slot: usize, finish: Option<u32>, selected: Option<usize>) -> Mode {
        self.paint_shop_in(None, slot, finish, selected)
    }

    /// UI_Paint in paint mode (`vinyl` None) or a vinyl's colours (mode 1:
    /// the layer and its NUMREMAPCOLOURS). The swatches are the slot's
    /// paints of the finish (the vinyl colours': brand VINYL, 0x50F5B0);
    /// the one the setup or the car has is marked fitted.
    pub(crate) fn paint_shop_in(
        &self,
        vinyl: Option<(u8, u8)>,
        slot: usize,
        finish: Option<u32>,
        selected: Option<usize>,
    ) -> Mode {
        use nfsu2::formats::hash::bin_hash;
        let mut shop = screens::PaintShop {
            slot,
            finish: 0,
            swatches: Vec::new(),
            selected,
            opened: self.now(),
            vinyl,
            total: self.graphics_total(),
            swatch_fade: None,
            arrows: Default::default(),
        };
        let slots = shop.slots();
        let (part_slot, ..) = slots[slot.min(slots.len() - 1)];
        let finish =
            finish.unwrap_or(if vinyl.is_some() { graphics::VINYL_BRAND } else { screens::paint_finish(part_slot) });
        let channel = |p: &nfsu2::formats::parts::Part, disp: &str, plain: &str| {
            self.game_data.parts
                .attribute(p, bin_hash(disp))
                .or_else(|| self.game_data.parts.attribute(p, bin_hash(plain)))
                .unwrap_or(0)
                .min(255) as u8
        };
        // A vinyl colour slot the car leaves unset holds the stock ride's
        // colour (0x637050, `car::VINYL_STOCK_COLOURS`; under Wine the
        // swatches opened on its grey).
        let fitted = self.effective(part_slot).or_else(|| {
            let k = usize::from(part_slot.checked_sub(0x4D)? % 4);
            (vinyl.is_some() && (0x4D..=0x5C).contains(&part_slot))
                .then(|| nfsu2::formats::hash::bin_hash(nfsu2::car::VINYL_STOCK_COLOURS[k]))
        });
        shop.finish = finish;
        shop.swatches = self
            .slot_parts(part_slot, false)
            .into_iter()
            .filter(|p| self.game_data.parts.attribute(p, 0xEBB0_3E66) == Some(finish))
            .take(60)
            .map(|p| {
                let rgb =
                    [channel(p, "DISPRED", "RED"), channel(p, "DISPGREEN", "GREEN"), channel(p, "DISPBLUE", "BLUE")];
                let (_, _, price, owned, _) = self.browser_part(part_slot, p);
                (p.hash, rgb, price, owned, fitted == Some(p.hash))
            })
            .collect();
        Mode::Menu(Menu::Paint(shop))
    }

    /// The graphics shop's Total Cost in a career: the setup's cost
    /// (0x521540); none outside one.
    pub(crate) fn graphics_total(&self) -> Option<i32> {
        // Shown only in a career (0x850078 = 1 at 0x546E92 / 0x5484AD /
        // 0x54FB17): main-menu Customize hides it (0x545470).
        if self.profile.as_ref()?.scratch {
            return None;
        }
        Some(self.shop.setup.cost(&|slot, hash| self.setup_price(slot, hash)))
    }

    /// The career car's part in a slot by hash.
    pub(crate) fn car_part(&self, slot: u8, hash: u32) -> Option<&nfsu2::formats::parts::Part> {
        let car = &self.profile.as_ref()?.car;
        self.game_data.parts.find(car, slot as usize, Some(hash), None)
    }

    /// A part's price in the graphics shop (0x514280: the cost table by
    /// the part's category and level; level 0 free but for decals).
    pub(crate) fn setup_price(&self, slot: u8, hash: u32) -> i32 {
        self.car_part(slot, hash).map_or(0, |p| self.game_data.career.costs.price(self.game_data.parts.category(slot as usize, p), p.level))
    }

    pub(crate) fn part_attr(&self, slot: u8, hash: u32, key: u32) -> Option<u32> {
        self.car_part(slot, hash).and_then(|p| self.game_data.parts.attribute(p, key))
    }

    /// A unique vinyl: NUMCOLOURS 0x40 (0x50EE90).
    pub(crate) fn unique_vinyl(&self, hash: u32) -> bool {
        self.part_attr(0x40, hash, graphics::NUMCOLOURS) == Some(graphics::UNIQUE_COLOURS)
    }

    /// The part the car has in a slot (0x610300 on the shop's car).
    pub(crate) fn fitted(&self, slot: u8) -> Option<u32> {
        self.profile.as_ref()?.installed.get(&slot).copied()
    }

    /// The part a slot shows: the setup's, else the car's.
    pub(crate) fn effective(&self, slot: u8) -> Option<u32> {
        match self.shop.setup.get(slot) {
            Some(0) => None,
            Some(h) => Some(h),
            None => self.fitted(slot),
        }
    }

    /// A unique vinyl on the car (0x50EF10).
    pub(crate) fn fitted_unique(&self) -> bool {
        graphics::VINYL_LAYERS.iter().any(|&l| self.fitted(l).is_some_and(|h| self.unique_vinyl(h)))
    }

    /// A unique vinyl in the setup (0x521590).
    pub(crate) fn setup_unique(&self) -> bool {
        self.shop.setup.items.iter().any(|&(s, h)| (0x40..=0x43).contains(&s) && h != 0 && self.unique_vinyl(h))
    }

    /// Whether the layer has a vinyl to change or remove (0x563110: one
    /// fitted the setup does not touch, or one in the setup).
    pub(crate) fn layer_has_vinyl(&self, layer: u8) -> bool {
        match self.shop.setup.get(layer) {
            Some(h) => h != 0,
            None => self.fitted(layer).is_some(),
        }
    }

    /// Decals open in a career (0x529F10): some decal (slot 0x61) of the
    /// car has a brand that 0x529E90 passes: one no performance part
    /// carries (0x502310 over the packages), or one in a career car
    /// slot's brand list (+0x6E0 = car record +0x434 + 0x2AC, slots with
    /// their own car): 0x503E10 appends there the brand picked on
    /// UI_PerformanceBrandSelect (0x521F00 → 0x8389B4) for each
    /// performance part bought (0x521B10, 0x54FE60), up to 0x44, never
    /// removed (`Profile::brands`, `Profile::slot_brands`).
    pub(crate) fn decals_unlocked(&self) -> bool {
        let Some(p) = self.profile.as_ref() else { return true };
        let open = self.decal_brand_test();
        let mut after = None;
        while let Some(d) = self.game_data.parts.find_after(&p.car, 0x61, after) {
            after = Some(d);
            if open(self.game_data.parts.attribute(d, 0xEBB0_3E66).unwrap_or(0)) {
                return true;
            }
        }
        false
    }

    /// 0x529E90's brand test: a brand no performance part carries
    /// (0x502310), or one in a career car slot's brand list.
    pub(crate) fn decal_brand_test(&self) -> impl Fn(u32) -> bool + use<> {
        let perf_brands: HashSet<u32> =
            self.game_data.career.packages.iter().flat_map(|k| k.parts.iter()).flat_map(|x| x.brands.iter().copied()).collect();
        let fitted_brands = self.profile.as_ref().map(|p| p.slot_brands()).unwrap_or_default();
        move |brand| !perf_brands.contains(&brand) || fitted_brands.contains(&brand)
    }

    /// A decal slot's list (UI_DecalsOverlay's fill 0x560260, **verified**):
    /// "No Decal" first, then the slot's parts in the database walk
    /// (0x5204F0 for slots 0x61–0x98: 0x61BA40 / 0x61BA60, no level or
    /// unlock test) whose NAME attribute (0x79AC30 hashed, 0x43DB50) differs
    /// from their BRAND (0xEBB03E66) — or, when the decal on the slot (the
    /// setup's, else the car's, 0x568521 → 0x610300) has NAME = BRAND,
    /// those that have it (the constructor 0x568460's +0x112): one of each
    /// brand's two textures. Each brand passes 0x53B510 (ECX 0x838980):
    /// 0x529E90 in a career or outside one, everything with the
    /// unlock-all byte 0x838464 (game mode 8's 0x534120 not ported). The
    /// list opens on the slot's decal when listed (0x51F7B0), else on No
    /// Decal.
    pub(crate) fn decal_list(&self, slot: u8) -> Vec<&nfsu2::formats::parts::Part> {
        let Some(p) = self.profile.as_ref() else { return Vec::new() };
        if !(0x61..0x99).contains(&slot) {
            return self.slot_parts(slot, false);
        }
        let name = nfsu2::formats::hash::bin_hash("NAME");
        let same = |x: &nfsu2::formats::parts::Part| {
            self.game_data.parts.attribute(x, name).unwrap_or(0) == self.game_data.parts.attribute(x, 0xEBB0_3E66).unwrap_or(0)
        };
        let on = match self.shop.setup.get(slot) {
            Some(h) if h != 0 => Some(h),
            _ => self.fitted(slot),
        };
        let flag = on.and_then(|h| self.game_data.parts.find(&p.car, slot as usize, Some(h), None)).is_some_and(same);
        let open = self.decal_brand_test();
        let all = nfsu2::frontend::career::UNLOCK_ALL.load(std::sync::atomic::Ordering::Relaxed);
        let mut out = Vec::new();
        let mut after = None;
        while let Some(x) = self.game_data.parts.find_after(&p.car, slot as usize, after) {
            after = Some(x);
            if same(x) == flag && (all || open(self.game_data.parts.attribute(x, 0xEBB0_3E66).unwrap_or(0))) {
                out.push(x);
            }
        }
        out
    }

    /// UI_ChoosePaintCategory (0x548210): Paint, Vinyls, Decals (in a
    /// career locked while no decal is open, 0x529F10), then "Purchase
    /// Setup" in a career, else "Install Setup" (icon 0x911A4EDE).
    pub(crate) fn graphics_menu(&self, selected: usize) -> Mode {
        let career = self.career_shop();
        let decals = if self.decals_unlocked() { Action::Decals } else { Action::Unavailable };
        let options = vec![
            screens::IconOption { icon: 0x5506_81A4, label: 0x6A4C_46A6, action: Action::Paint },
            screens::IconOption { icon: 0x5577_8E5A, label: 0x78DE_EA1C, action: Action::Vinyls },
            screens::IconOption { icon: 0xDA1D_AE54, label: 0x4E93_3E83, action: decals },
            screens::IconOption {
                icon: 0x911A_4EDE,
                label: if career { 0x920D_BB02 } else { 0xD132_A6FE },
                action: Action::PurchaseSetup,
            },
        ];
        self.graphics_icon_menu(MenuKind::Graphics, options, Vec::new(), selected)
    }

    /// An icon menu of the graphics shop: Total Cost in a career (0x5453C0,
    /// "%$d" of the setup's cost), hidden outside one (0x545470).
    pub(crate) fn graphics_icon_menu(
        &self,
        kind: MenuKind,
        options: Vec<screens::IconOption>,
        hide_option: Vec<Vec<u32>>,
        selected: usize,
    ) -> Mode {
        let n = options.len();
        let mut menu = IconMenu::new(kind.package(), None, options, self.now());
        match self.graphics_total() {
            Some(t) => menu.extra = vec![vec![(0x2F5E_7DA3, nfsu2::frontend::grouped(i64::from(t)))]; n],
            None => menu.hide.push(0x9FF8_4A88),
        }
        menu.hide_option = hide_option;
        menu.select_now(selected);
        Mode::Menu(Menu::Icons(menu, kind))
    }

    /// UI_ChooseVinylLayer (0x554CB0, options 0x548460 each 0x522140):
    /// the four layers, labelled and by default iconed VINYL_LAYER_1 … _4;
    /// a layer with a vinyl (the setup's, else the car's) shows its group's
    /// icon (0x50E190), every layer the Unique icon while a unique vinyl
    /// is on or in the setup. "Reorder Layers" and its trigger arrows
    /// (0xD999B6E7, 0xCF6F0439, 0xABA93D3F) show on a layer with a vinyl
    /// (0x5535E0).
    pub(crate) fn vinyl_layer_menu(&self, selected: usize) -> Mode {
        let unique = self.setup_unique() || self.fitted_unique();
        let mut hide_option = Vec::new();
        let options = graphics::VINYL_LAYERS
            .iter()
            .zip(graphics::VINYL_LAYER_ICONS)
            .map(|(&layer, default)| {
                let vinyl = if unique { None } else { self.effective(layer) };
                let icon = if unique {
                    graphics::UNIQUE_ICON
                } else {
                    vinyl
                        .and_then(|h| self.car_part(layer, h))
                        .and_then(|p| graphics::vinyl_group_icon(p.kind))
                        .unwrap_or(default)
                };
                hide_option.push(if self.layer_has_vinyl(layer) && !unique {
                    Vec::new()
                } else {
                    vec![0xD999_B6E7, 0xCF6F_0439, 0xABA9_3D3F]
                });
                screens::IconOption { icon, label: default, action: Action::VinylLayer(layer) }
            })
            .collect();
        // A layer with a vinyl SHOWs the three (0x553718 → 0x495520): the
        // label "Reorder Layers" starts hidden (FNG flag 0x40), so it
        // needs the show; under Wine it is drawn on Flames' layer.
        let shown: Vec<bool> = hide_option.iter().map(Vec::is_empty).collect();
        let mut mode = self.graphics_icon_menu(MenuKind::VinylLayer, options, hide_option, selected);
        if let Mode::Menu(Menu::Icons(menu, _)) = &mut mode {
            menu.option_state = shown
                .into_iter()
                .map(|s| screens::OptionState {
                    shown: if s { vec![0xD999_B6E7, 0xCF6F_0439, 0xABA9_3D3F] } else { Vec::new() },
                    ..Default::default()
                })
                .collect();
        }
        mode
    }

    /// The vinyl a layer's lists open on: the car's (0x610300 on the FE
    /// car 0x8392C0, which holds the bought parts, not the setup), else the
    /// setup's (0x521860) — UI_PartsBrowser_EnterCategory's column
    /// (0x53A4C0 with +0x1D7) and the group strip's seed (0x546040:
    /// 0x50EFD0, else 0x521860). Wine (the finished-career save, main-menu Customize): Tear Set
    /// item 3 bought, item 1 put in the setup, the layer reopened on item 3.
    pub(crate) fn vinyl_list_current(&self, layer: u8) -> Option<u32> {
        self.fitted(layer).filter(|&h| h != 0).or_else(|| self.shop.setup.get(layer).filter(|&h| h != 0))
    }

    /// The vinyl browser (UI_GenericParts_Browser in the graphics shop,
    /// 0x546040): the vinyl groups across the strip, each the car's
    /// slot-0x40 vinyls of its kind that the shop lists (locked without
    /// any); it opens on the layer's vinyl's group. Titled "Graphics Shop"
    /// (0xB6AAECF7); the price is 0x514280's for the layer.
    pub(crate) fn vinyl_browser(&self, layer: u8, category: Option<usize>, part: Option<usize>) -> Mode {
        let current = self.vinyl_list_current(layer);
        let listed = self.slot_parts(0x40, false);
        let categories: Vec<screens::BrowserCategory> = graphics::VINYL_GROUPS
            .iter()
            .map(|&(kind, icon, label)| {
                let parts: Vec<screens::BrowserPart> = listed
                    .iter()
                    // AD_BESTBUY (kind 0x16) is skipped while the region
                    // 0x570A40 is not 0 — the static 6 at 0x7FA73C.
                    .filter(|p| {
                        p.kind == kind && !(kind == 0x16 && p.hash == nfsu2::formats::hash::bin_hash("AD_BESTBUY"))
                    })
                    .map(|p| {
                        let (hash, name, _, owned, _) = self.browser_part(0x40, p);
                        (hash, name, self.setup_price(layer, hash), owned, current == Some(hash))
                    })
                    .collect();
                let locked = parts.is_empty();
                screens::BrowserCategory {
                    slot: layer,
                    icon,
                    label,
                    parts,
                    locked,
                    action: screens::CategoryAction::Parts,
                    text: None,
                }
            })
            .collect();
        let group = current
            .and_then(|h| self.car_part(0x40, h))
            .and_then(|p| graphics::VINYL_GROUPS.iter().position(|g| g.0 == p.kind));
        let category = category.or(group).unwrap_or(0);
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_GenericParts_Browser",
            title: 0xB6AA_ECF7,
            sub: Some(layer),
            categories,
            category,
            part,
            opened: self.now(),
            total: self.graphics_total(),
            arrows: Default::default(),
        }))
    }

    /// The vinyl's colours on UI_Paint (mode 1), starting on colour
    /// `colour`.
    ///
    /// With one colour UI_Paint's setup enters its slot at once (0x563C79:
    /// mode 1, the vinyl 0x839BB8's NUMREMAPCOLOURS below 2 → 0x548640,
    /// 0x55CC60): the swatches open on the slot's colour.
    pub(crate) fn vinyl_colours(&self, colour: usize) -> Mode {
        let n =
            self.shop.vinyl.and_then(|h| self.part_attr(0x40, h, graphics::NUMREMAPCOLOURS)).unwrap_or(1).clamp(1, 4) as u8;
        let mut mode = self.paint_shop_in(Some((self.shop.vinyl_layer, n)), colour.min(n as usize - 1), None, None);
        if n < 2
            && let Mode::Menu(Menu::Paint(p)) = &mut mode
            && !p.swatches.is_empty()
        {
            p.selected = Some(p.swatches.iter().position(|s| s.4).unwrap_or(0));
            p.swatch_fade = Some((true, p.opened));
        }
        mode
    }

    /// The decal a slot's list opens on (0x560260): the setup's entry for
    /// the slot when its part is non-zero, else — no entry, or "No Decal"
    /// put in the setup (part 0: 0x560260 leaves its setup loop) — the car's
    /// (0x610300 on 0x8392C0); "No Decal" when neither (0x51F7B0 finds
    /// nothing: the head).
    pub(crate) fn decal_list_current(&self, slot: u8) -> Option<u32> {
        self.shop.setup.get(slot).filter(|&h| h != 0).or_else(|| self.fitted(slot).filter(|&h| h != 0))
    }

    /// UI_DecalMain (0x553100, options 0x546A00) on a node of its tree;
    /// a decal slot's decals ("No Decal" 0x2B5ABCF4, then the slot's
    /// decals) list through UI_DecalsOverlay (0x546F50; the name is the
    /// decal's LANGUAGEHASH, else its name; the price 0x514280's).
    pub(crate) fn decal_shop(&self, node: u32, category: usize, part: Option<usize>) -> Mode {
        use screens::{BrowserCategory, CategoryAction};
        let wide = self.fitted(6).and_then(|h| self.car_part(6, h)).is_some_and(|p| p.level != 0);
        let profile = self.profile.as_ref();
        let categories: Vec<BrowserCategory> = graphics::decal_options(node, wide)
            .into_iter()
            .map(|o| {
                // The top level locks by the location part (0x52FF70).
                let locked = o.location.is_some_and(|loc| {
                    let first = profile.and_then(|p| self.game_data.parts.find_after(&p.car, loc as usize, None));
                    match (profile, first) {
                        (Some(p), Some(part)) => {
                            !(part.level == 0
                                || p.part_unlocked(&self.game_data.career, self.game_data.parts.category(loc as usize, part), part.level))
                        }
                        _ => false,
                    }
                });
                match o.item {
                    DecalItem::Node(_) => BrowserCategory {
                        slot: 0,
                        icon: o.icon,
                        label: o.label,
                        parts: Vec::new(),
                        locked,
                        action: CategoryAction::Open,
                        text: None,
                    },
                    DecalItem::Slot(slot) => {
                        let current = self.decal_list_current(slot);
                        let none = self.label(0x2B5A_BCF4, "No Decal");
                        let mut parts: Vec<screens::BrowserPart> = vec![(0, none, 0, true, current.is_none())];
                        parts.extend(self.decal_list(slot).into_iter().map(|p| {
                            let (hash, name, _, owned, _) = self.browser_part(slot, p);
                            (hash, name, self.setup_price(slot, hash), owned, current == Some(hash))
                        }));
                        BrowserCategory {
                            slot,
                            icon: o.icon,
                            label: o.label,
                            parts,
                            locked,
                            action: CategoryAction::Parts,
                            text: None,
                        }
                    }
                }
            })
            .collect();
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_DecalMain",
            title: 0,
            sub: None,
            categories,
            category,
            part,
            opened: self.now(),
            total: self.graphics_total(),
            arrows: Default::default(),
        }))
    }

    /// The paint slot's reason it cannot be painted (0x50F310 false →
    /// the dialog of 0x563420): stock parts (0x73BC702E), engine accents
    /// without the level-3 engine (0xD286BE3A), spinner paint on plain
    /// rims (0x6BCBCEEA), carbon or aluminium spoilers, roof scoops,
    /// mirrors (0xE7337E9D, 0xAD460262, 0xF152BEFA). None: paintable.
    pub(crate) fn unpaintable(&self, slot: u8) -> Option<u32> {
        let level = |s: u8| self.fitted(s).and_then(|h| self.car_part(s, h)).map_or(0, |p| p.level);
        let light = |s: u8| {
            self.fitted(s).is_some_and(|h| {
                self.part_attr(s, h, 0x721A_FF7C).is_some_and(|v| v != 0)
                    || self.part_attr(s, h, 0x6509_EC92).is_some_and(|v| v != 0)
            })
        };
        match slot {
            0x3F => None,
            0x44 => (level(0x0D) <= 2).then_some(0xD286_BE3A),
            0x45 => match (level(0x0C), light(0x0C)) {
                (0, _) => Some(0x73BC_702E),
                (_, true) => Some(0xE733_7E9D),
                _ => None,
            },
            0x46 => {
                let perf = self.profile.as_ref().map(|p| p.perf.clone()).unwrap_or_default();
                (![0x28, 0x29, 0x2B].iter().any(|id| perf.contains(id))).then_some(0x73BC_702E)
            }
            0x47 => (level(0x10) == 0).then_some(0x73BC_702E),
            0x49 => (level(0x1D) == 0).then_some(0x73BC_702E),
            0x4A => {
                if level(0x1D) == 0 {
                    Some(0x73BC_702E)
                } else if self.fitted(0x1D).and_then(|h| self.part_attr(0x1D, h, 0x931F_F82E)).is_none() {
                    Some(0x6BCB_CEEA)
                } else {
                    None
                }
            }
            0x4B => match (level(7), light(7)) {
                (0, _) => Some(0x73BC_702E),
                (_, true) => Some(0xAD46_0262),
                _ => None,
            },
            0x4C => match (level(0x20), light(0x20)) {
                (0, _) => Some(0x73BC_702E),
                (_, true) => Some(0xF152_BEFA),
                _ => None,
            },
            _ => Some(0x73BC_702E),
        }
    }

    /// A question whose buttons change the graphics setup: each button's
    /// label, its step (if any) and where it goes otherwise.
    pub(crate) fn setup_dialog(&mut self, question: u32, buttons: Vec<(String, Option<SetupStep>, Mode)>) -> Mode {
        self.setup_dialog_in(None, question, buttons)
    }

    /// `package`: the one the request names (GenericDialog_SMALL.fng at
    /// 0x56B772, 0x56B5B9); else "3button" or 0x526C50's size rule.
    pub(crate) fn setup_dialog_in(
        &mut self,
        package: Option<&'static str>,
        question: u32,
        buttons: Vec<(String, Option<SetupStep>, Mode)>,
    ) -> Mode {
        let text = self.label(question, "?");
        let package = match (package, buttons.len()) {
            (Some(p), _) => p,
            (None, 3) => "GenericDialog_ThreeButton",
            (None, _) => screens::dialog_package(&self.game_data.assets, &text),
        };
        let labels: Vec<String> = buttons.iter().map(|b| b.0.clone()).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut outcomes = Vec::new();
        self.shop.dialog_steps.clear();
        for (_, step, mode) in buttons {
            self.shop.dialog_steps.push(step);
            outcomes.push(mode);
        }
        // The shops ask from their handlers over their own package
        // (0x5587A0 arg 1 = the screen's package, no FORWARD), which stays
        // drawn: the "No" outcome is that screen (Wine: the graphics shop
        // under its install question).
        let mut d = Dialog::new(package, &text, &refs, self.now());
        d.under = outcomes.iter().rposition(|m| matches!(m, Mode::Menu(_)));
        // The shops' two-button questions all push 1 (No focused); the
        // three-button ones 0 (0x5631A0 / 0x56325F).
        if labels.len() == 2 {
            d.selected = ask_focus(question);
        }
        Mode::Menu(Menu::Dialog(d, outcomes))
    }

    pub(crate) fn yes_no(&self) -> (String, String) {
        (self.label(0x8857_9775, "Yes"), self.label(0xBE50_3981, "No"))
    }

    /// Buy the setup (0x547580): each item fits its part (a vinyl over a
    /// fitted unique one clears every layer first) or takes the slot's
    /// off; in a career each part costs its price (0x514280, the cash not
    /// below 0) and, but for the paints, is marked bought.
    pub(crate) fn purchase_setup(&mut self) {
        let setup = std::mem::take(&mut self.shop.setup);
        let uniques: HashSet<u32> = setup
            .items
            .iter()
            .map(|i| i.1)
            .chain(graphics::VINYL_LAYERS.iter().filter_map(|&l| self.fitted(l)))
            .filter(|&h| self.unique_vinyl(h))
            .collect();
        let prices: Vec<(u8, u32, i32)> =
            setup.items.iter().map(|&(s, h)| (s, h, if h == 0 { 0 } else { self.setup_price(s, h) })).collect();
        if let Some(p) = self.profile.as_mut() {
            let before = p.installed.clone();
            setup.apply(&mut p.installed, &|h| uniques.contains(&h));
            // 0x547791: the ride (each item fitted by 0x631D30) stored.
            p.store_visual(&self.game_data.parts, &self.game_data.career.packages, &before);
            // Charged only in a career (0x850078 = 1 at 0x54761B).
            for (slot, hash, price) in prices {
                if hash != 0 && !p.scratch {
                    p.cash = p.cash.saturating_sub(price.max(0) as u32);
                    // 0x547747: the current slot's spend (+0x7F0) grows by
                    // the same price.
                    p.spent = p.spent.wrapping_add(price.max(0) as u32);
                    if graphics::marks_owned(slot) {
                        p.own_part(slot, hash);
                    }
                }
            }
        }
        let rating = self.visual_rating();
        if let Some(p) = self.profile.as_mut() {
            p.offer_covers(&self.game_data.career, rating);
            let _ = p.commit();
        }
        self.drop_drive();
        // 0x547796: the purchase ends with the table's [INSTALL][PAINT]
        // sound (0x838149, id 38), whatever was bought.
        self.fe_sound(fe::event::INSTALL, fe::control::PAINT);
    }

    /// Carry out a dialog button's step.
    pub(crate) fn setup_step(&mut self, step: SetupStep) -> Result<()> {
        self.mode = match step {
            SetupStep::Paint { slot, finish, part_slot, hash, selected } => {
                self.shop.setup.put(part_slot, hash);
                self.paint_shop_in(None, slot, Some(finish), Some(selected))
            }
            SetupStep::VinylColour { colour, slot, hash } => {
                if let Some(v) = self.shop.vinyl {
                    let (unique, setup_unique) = (self.unique_vinyl(v), self.setup_unique());
                    if self.shop.setup.get(self.shop.vinyl_layer) != Some(v) {
                        self.shop.setup.put_vinyl(self.shop.vinyl_layer, v, unique, setup_unique);
                    }
                }
                let n = self
                    .shop.vinyl
                    .and_then(|h| self.part_attr(0x40, h, graphics::NUMREMAPCOLOURS))
                    .unwrap_or(1)
                    .clamp(1, 4) as usize;
                if n > 1 {
                    // More than one colour (0xD05FC3A3 → 0x563AE4): the
                    // choice waits in the screen's list (0x53C170, one
                    // per slot) and the strip comes back (0x55CEA0(0));
                    // Back puts them all in the setup.
                    self.shop.vinyl_pending.retain(|&(s, _)| s != slot);
                    self.shop.vinyl_pending.push((slot, hash));
                    self.vinyl_colours(colour)
                } else {
                    // One colour (0x563B24): the vinyl and its colour into
                    // the setup, on to the layers (0x55BD70).
                    self.shop.setup.put_colour(slot, hash);
                    self.shop.vinyl = None;
                    self.vinyl_layer_menu((self.shop.vinyl_layer - 0x40) as usize)
                }
            }
            SetupStep::Vinyl { layer, hash } => {
                // 0x5670F9: into the setup (0x54FDC0); with REMAP
                // (0x39DD714) on to its colours (UI_Paint mode 1,
                // 0x55BD70), else back to the layers (0x55BE00).
                let (unique, setup_unique) = (self.unique_vinyl(hash), self.setup_unique());
                self.shop.setup.put_vinyl(layer, hash, unique, setup_unique);
                self.shop.vinyl_layer = layer;
                if self.part_attr(0x40, hash, graphics::REMAP).is_some_and(|r| r != 0) {
                    self.shop.vinyl = Some(hash);
                    self.shop.vinyl_pending.clear();
                    self.vinyl_colours(0)
                } else {
                    self.vinyl_layer_menu((layer - 0x40) as usize)
                }
            }
            SetupStep::ChangeVinyl(layer) => {
                self.shop.vinyl_layer = layer;
                self.vinyl_browser(layer, None, None)
            }
            SetupStep::RemoveVinyl(layer) => {
                let (fitted_unique, setup_unique) = (self.fitted_unique(), self.setup_unique());
                let fitted: Vec<u8> =
                    graphics::VINYL_LAYERS.iter().copied().filter(|&l| self.fitted(l).is_some()).collect();
                self.shop.setup.remove_vinyl(layer, fitted_unique, setup_unique, &|l| fitted.contains(&l));
                self.vinyl_layer_menu((layer - 0x40) as usize)
            }
            SetupStep::Purchase { leave } => {
                self.purchase_setup();
                if leave {
                    self.leave_shop()?;
                    return Ok(());
                }
                self.graphics_menu(3)
            }
            SetupStep::Discard => {
                self.shop.setup = Setup::default();
                self.leave_shop()?;
                return Ok(());
            }
            SetupStep::Cancel(kind) => {
                self.shop.setup.cancel(kind);
                self.graphics_menu(match kind {
                    0 => 1,
                    1 => 2,
                    _ => 0,
                })
            }
            SetupStep::Layout(node) => {
                // 0x55FEA0: the layout's decal slots empty, its location
                // part goes in.
                if let Some((location, first, count)) = graphics::decal_layout(node) {
                    for slot in first..first + 8 {
                        let fitted = self.fitted(slot).is_some();
                        self.shop.setup.remove(slot, fitted);
                    }
                    if let Some(part) = self.location_part(location, count) {
                        self.shop.setup.put(location, part);
                    }
                }
                self.shop.decal_node = node;
                self.decal_shop(node, 0, None)
            }
            SetupStep::Exit => {
                if self.shop.setup.is_empty() {
                    self.leave_shop()?;
                    return Ok(());
                }
                let (yes, no) = self.yes_no();
                let question = if self.career_shop() { 0xA6C0_D334 } else { 0x511C_CA10 };
                // The shop's menu under the question as Back left it
                // (Wine menus103 graphics_exit: on Decals).
                let at = match &self.mode {
                    Mode::Menu(Menu::Icons(m, MenuKind::Graphics)) => m.selected,
                    _ => 0,
                };
                let (a, b) = (self.graphics_menu(at), self.graphics_menu(at));
                // StartDim after the ask (0x562F5D / 0x562FB2).
                // GenericDialog_SMALL.fng named at both sites (0x562DD6 /
                // 0x562F98: 0x78FA24), not 0x526C50's size rule (Wine
                // menus103 graphics_exit: the small box).
                dialog_over(
                    self.setup_dialog_in(
                        Some("GenericDialog_SMALL"),
                        question,
                        vec![(yes, Some(SetupStep::Purchase { leave: true }), a), (no, Some(SetupStep::Discard), b)],
                    ),
                    true,
                    None,
                )
            }
            SetupStep::RemoveDecals(node) => {
                if let Some(range) = graphics::decal_node_slots(node) {
                    for slot in range {
                        let fitted = self.fitted(slot).is_some();
                        self.shop.setup.remove(slot, fitted);
                    }
                }
                self.decal_shop(self.shop.decal_node, 0, None)
            }
        };
        Ok(())
    }

    /// Leaving the graphics shop (Back, 0x911AB364 in 0x562C60): in a
    /// career "Are you sure you wish to exit back to Explore Mode?"
    /// (0xD76E0B5A) first; then with a setup "The graphics setup for your
    /// car has changed. Would you like to purchase your new graphics
    /// setup?" (0xA6C0D334; outside a career 0x511CCA10, install): Yes
    /// buys it (0x51A33F36 → 0x547580), No drops it (0x562F5DAC).
    pub(crate) fn leave_graphics(&mut self) -> Result<()> {
        if self.career_shop() {
            let (yes, no) = self.yes_no();
            let at = match &self.mode {
                Mode::Menu(Menu::Icons(m, MenuKind::Graphics)) => m.selected,
                _ => 0,
            };
            let (a, b) = (self.graphics_menu(at), self.graphics_menu(at));
            // StartDim after the ask (0x562F0B; UI_ChoosePaintCategory's
            // has no event: unchanged under it).
            self.mode = dialog_over(
                self.setup_dialog(0xD76E_0B5A, vec![(yes, Some(SetupStep::Exit), a), (no, None, b)]),
                true,
                None,
            );
            Ok(())
        } else {
            self.setup_step(SetupStep::Exit)
        }
    }

    /// The location part with `count` NUM_DECALS (0x521950).
    pub(crate) fn location_part(&self, location: u8, count: u32) -> Option<u32> {
        let car = &self.profile.as_ref()?.car;
        let mut after = None;
        while let Some(p) = self.game_data.parts.find_after(car, location as usize, after) {
            after = Some(p);
            if self.game_data.parts.attribute(p, graphics::NUM_DECALS) == Some(count) {
                return Some(p.hash);
            }
        }
        None
    }

    /// The location's layout NUM_DECALS as the setup or the car has it
    /// (0x5218C0).
    pub(crate) fn layout_count(&self, location: u8) -> Option<u32> {
        let h = self.effective(location)?;
        self.part_attr(location, h, graphics::NUM_DECALS)
    }
}
