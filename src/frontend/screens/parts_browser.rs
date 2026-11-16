use crate::frontend::fe_ids;
use super::*;

/// The body shop's categories (0x545B60): slot, icon (VISUAL_PART_*) and
/// label, in the screen's order; rims (0x1D) and carbon fibre (0x5E) are
/// their own kinds of option (0x53A630, 0x545890).
pub const BODY_SHOP_CATEGORIES: [(u8, u32, u32); 13] = [
    (0x01, fe_ids::FRONT_BUMPERS, fe_ids::FRONT_BUMPERS), // Front Bumpers
    (0x02, fe_ids::REAR_BUMPERS, fe_ids::REAR_BUMPERS), // Rear Bumpers
    (0x0B, fe_ids::SIDE_SKIRTS, fe_ids::SIDE_SKIRTS), // Side Skirts
    (0x0C, fe_ids::SPOILERS, fe_ids::SPOILERS), // Spoilers
    (0x09, fe_ids::HOODS, fe_ids::HOODS), // Hoods
    (0x07, fe_ids::ROOF_SCOOPS, fe_ids::ROOF_SCOOPS), // Roof Scoops
    (0x0E, fe_ids::HEADLIGHTS_ICON, fe_ids::HEADLIGHTS_LABEL), // Headlights
    (0x0F, fe_ids::TAILLIGHTS_ICON, fe_ids::TAILLIGHTS_LABEL), // Taillights
    (0x20, fe_ids::SIDE_MIRRORS_ICON, fe_ids::SIDE_MIRRORS_LABEL), // Side Mirrors
    (0x10, fe_ids::EXHAUST_TIPS_ICON, fe_ids::EXHAUST_TIPS_LABEL), // Exhaust Tips
    (0x1D, fe_ids::RIMS, fe_ids::RIMS), // Rims
    (0x5E, fe_ids::CARBON_FIBER, fe_ids::CARBON_FIBER), // Carbon Fiber
    (0x06, fe_ids::WIDE_BODY_KITS, fe_ids::WIDE_BODY_KITS), // Wide Body Kits
];

/// A part a browser category lists: hash, name, price, owned, installed.
pub type BrowserPart = (u32, String, i32, bool, bool);

/// A category on the browser's strip (0x545710): its slot, icon and label,
/// the parts it lists (0x5449D0), whether it is locked (item +0x22) and
/// what Select does with it.
#[derive(Clone, Debug)]
pub struct BrowserCategory {
    pub slot: u8,
    pub icon: u32,
    pub label: u32,
    pub parts: Vec<BrowserPart>,
    pub locked: bool,
    pub action: CategoryAction,
    /// The label's text when it is not a language string (a rim style's
    /// name, 0x43DB50).
    pub text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CategoryAction {
    /// Browse its parts.
    Parts,
    /// Open its own categories (item +0x23): rims by brand (0x53A630),
    /// carbon fibre (0x545890).
    Open,
    /// Fit this part at once (the carbon trunk and kits, 0x545D40).
    Toggle(u32),
}

/// The rim brands (0x546530; 0x5463C0 adds each): brand (the rims'
/// attribute 0xEBB03E66), icon, label; SUVs (car type +0x88A) have their
/// own. The last, stock, is never locked.
pub const RIM_BRANDS: [(u32, u32, u32); 16] = [
    (fe_ids::RIM_N_5ZIGEN_COL0, fe_ids::RIM_N_5ZIGEN_COL1, fe_ids::RIM_N_5ZIGEN_COL2), // 5Zigen
    (fe_ids::AVUS, fe_ids::RIM_AVUS_COL1, fe_ids::RIM_AVUS_COL2), // Avus
    (fe_ids::RIM_BBS_COL0, fe_ids::RIM_BBS_COL1, fe_ids::RIM_BBS_COL2), // BBS
    (fe_ids::ENKEI, fe_ids::RIM_ENKEI_COL1, fe_ids::RIM_ENKEI_COL2), // Enkei
    (fe_ids::GIANELLE, fe_ids::RIM_GIANELLE_COL1, fe_ids::RIM_GIANELLE_COL2), // Gianelle
    (fe_ids::KONIG, fe_ids::RIM_KONIG_COL1, fe_ids::RIM_KONIG_COL2), // Konig
    (fe_ids::LEXANI, fe_ids::RIM_LEXANI_COL1, fe_ids::RIM_LEXANI_COL2), // Lexani
    (fe_ids::LOWENHART, fe_ids::RIM_LOWENHART_COL1, fe_ids::RIM_LOWENHART_COL2), // Lowenhart
    (fe_ids::MOMO, fe_ids::RIM_MOMO_COL1, fe_ids::RIM_MOMO_COL2), // Momo
    (fe_ids::RIM_O_Z_COL0, fe_ids::RIM_O_Z_COL1, fe_ids::RIM_O_Z_COL2), // O.Z.
    (fe_ids::NFSU, fe_ids::RIM_NFSU_2_COL1, fe_ids::RIM_NFSU_2_COL2), // NFSU 2
    (fe_ids::RIM_RACING_HART_COL0, fe_ids::RIM_RACING_HART_COL1, fe_ids::RIM_RACING_HART_COL2), // Racing Hart
    (fe_ids::ROTA, fe_ids::RIM_ROTA_COL1, fe_ids::RIM_ROTA_COL2), // Rota
    (fe_ids::VOLK, fe_ids::RIM_VOLK_COL1, fe_ids::RIM_VOLK_COL2), // Volk
    (fe_ids::WORK, fe_ids::RIM_WORK_COL1, fe_ids::RIM_WORK_COL2), // Work
    (STOCK_RIMS, fe_ids::RIM_STOCK_COL0, fe_ids::RIM_STOCK_COL1),  // Stock
];

pub const SUV_RIM_BRANDS: [(u32, u32, u32); 10] = [
    (fe_ids::DONZ, fe_ids::RIM_DONZ_COL1, fe_ids::RIM_DONZ_COL2), // Donz
    (fe_ids::FOXX, fe_ids::RIM_FOXX_COL1, fe_ids::RIM_FOXX_COL2), // Foxx
    (fe_ids::GIANELLE, fe_ids::RIM_GIANELLE_COL1, fe_ids::RIM_GIANELLE_COL2), // Gianelle
    (fe_ids::GIOVANNA, fe_ids::RIM_GIOVANNA_COL1, fe_ids::RIM_GIOVANNA_COL2), // Giovanna
    (fe_ids::KAIZER, fe_ids::RIM_KAIZER_COL1, fe_ids::RIM_KAIZER_COL2), // Kaizer
    (fe_ids::RIM_O_Z_COL0, fe_ids::RIM_O_Z_COL1, fe_ids::RIM_O_Z_COL2), // O.Z.
    (fe_ids::NFSU, fe_ids::RIM_NFSU_2_COL1, fe_ids::RIM_NFSU_2_COL2), // NFSU 2
    (fe_ids::RIM_STREETSPIN_COL0, fe_ids::RIM_STREETSPIN_COL1, fe_ids::RIM_STREETSPIN_COL2), // StreetSpin
    (fe_ids::RIM_WELD_COL0, fe_ids::RIM_WELD_COL1, fe_ids::RIM_WELD_COL2), // Weld
    (STOCK_RIMS, fe_ids::RIM_STOCK_COL0, fe_ids::RIM_STOCK_COL1),  // Stock
];

/// The stock "brand": the car's level-0 rims (0x539B80).
pub const STOCK_RIMS: u32 = fe_ids::STOCK_RIMS;

/// A rim brand's name as 0x50E960 gives it (UI_ChooseUniquePart's rim
/// name, UI_Rims_Browser's title): first the career brand table
/// (0x838400, chunk 0x34A14; 0x4FFF40 searches it by +0x40) — its +0 name
/// string as is, which of the rim brands only 5Zigen (0x352D08D1) is in,
/// so it reads "5ZIGEN" (runtime read) — else the
/// fixed switch over the brand hashes, the language string of the
/// brand's label (the third column of [`RIM_BRANDS`] /
/// [`SUV_RIM_BRANDS`], equal to that switch for every listed brand).
pub fn rim_brand_name(
    brands: &[crate::frontend::career::Brand],
    strings: &crate::frontend::feng::strings::Strings,
    brand: u32,
) -> String {
    if let Some(b) = brands.iter().find(|b| b.hash == brand) {
        return b.name.clone();
    }
    RIM_BRANDS
        .iter()
        .chain(SUV_RIM_BRANDS.iter())
        .find(|r| r.0 == brand)
        .and_then(|r| strings.get(r.2))
        .unwrap_or("")
        .to_string()
}

/// UI_GenericParts_Browser (screen 0x566440, constructor 0x55E830): one
/// screen in two modes. Its categories (0x54FAF0; the body shop's
/// 0x545B60) are an IconScrollerMenu across CATEGORY_MASTER, each a
/// CATEGORY_ICON clone (gap 40, the strip 480 wide, fading at its ends);
/// LEFT / RIGHT change category and LOCK_MASTER shows on a locked one
/// (0x51DF10). Select enters the category (0x55EDE0): the strip hides and
/// its parts (0x53A4C0) go down MINI_MASTER, each a PARTS_ICON clone with
/// texture 0x73C4EC2D (gap 20, the column 200 tall), starting on the
/// installed part; UP / DOWN change part; the selected part's name
/// (0xD28B9316) and price (0x0380B632, 0x514280) show and Select buys and
/// fits it, then shows the categories again (0x5665AA → 0x55EF90(0)); Back
/// returns to the categories (0x55EF90(1)). The title is the
/// shop's ("Body Shop", or "Body Parts Storage" in the garage) and the
/// option title the category's label (0x545520).
pub struct PartsBrowser {
    /// UI_GenericParts_Browser, or UI_Rims_Browser (0x56B360 → 0x5678F0)
    /// for a rim brand's styles: their strip is OPTION_MASTER's.
    pub package: &'static str,
    pub title: u32,
    /// Rims (0x1D) or carbon fibre (0x5E) when in their own categories.
    pub sub: Option<u8>,
    pub categories: Vec<BrowserCategory>,
    pub category: usize,
    /// In a category: the part selected.
    pub part: Option<usize>,
    pub opened: f32,
    /// The graphics shop's Total Cost in a career (0x54FAF0 → 0x5453C0);
    /// None hides it (0x545470).
    pub total: Option<i32>,
    /// The category strip's arrows' blink (the IconScrollerMenu's
    /// 0x52FA80, [`ArrowBlink`]).
    pub arrows: ArrowBlink,
}

pub const TITLE: u32 = fe_ids::UI_MAIN_TITLE;
pub const OPTION_TITLE: u32 = fe_ids::PARTS_BROWSER_OPTION_TITLE;
pub const CASH: u32 = fe_ids::UI_MAIN_CASH;
pub const PLAYER_NAME: u32 = fe_ids::PLAYER_NAME;
pub const PART_NAME: u32 = fe_ids::PART_NAME;
pub const PART_PRICE: u32 = fe_ids::PART_PRICE;
pub const TOTAL_COST: u32 = fe_ids::TOTAL_COST;
pub const CATEGORY_MASTER: u32 = fe_ids::CATEGORY_MASTER;
pub const OPTION_MASTER: u32 = fe_ids::OPTION_MASTER;
pub const OPTION_TEMPLATE: u32 = fe_ids::OPTION_TEMPLATE;
pub const RIM_OPTION_TITLE: u32 = fe_ids::UI_MAIN_OPTION_TITLE;
pub const CATEGORY_ICON: u32 = fe_ids::CATEGORY_ICON;
pub const LOCK_MASTER: u32 = fe_ids::LOCK_MASTER;
pub const MINI_MASTER: u32 = fe_ids::MINI_MASTER;
pub const PARTS_ICON: u32 = fe_ids::PARTS_ICON;
pub const PART_TEXTURE: u32 = fe_ids::PART_TEXTURE;
/// Shown in category mode, hidden in a category (0x55EF90 / 0x55EDE0).
pub const CATEGORY_ONLY: u32 = fe_ids::CATEGORY_ONLY;
/// "Install" and its button: shown in a category.
pub const INSTALL: [u32; 2] = [fe_ids::CAR_LOT_CUSTOMIZE, fe_ids::INSTALL_1];

/// What a browser key did.
pub enum BrowserKey {
    None,
    /// Back out of the browser.
    Leave,
    /// Buy and fit the category's nth part.
    Install(usize),
    /// Open the selected category's own categories.
    Open,
    /// Fit the selected category's toggle part.
    Toggle,
}

impl PartsBrowser {
    /// The help bar's mask: the screen's, and in UI_GenericParts_Browser's
    /// categories 0x55EDE0's 0x401 | 8 (Install Part), | 0x10 (Install
    /// Vinyl) instead in the graphics shop's vinyls (0x838990 = 2 with the
    /// category option 0x83899C = 0); leaving one (0x55EF90) puts the
    /// screen's back (+0x20).
    pub fn help_mask(&self) -> Option<u32> {
        if self.package == "UI_GenericParts_Browser" && self.part.is_some() {
            let vinyl = self.sub.is_some_and(|l| crate::frontend::graphics::VINYL_LAYERS.contains(&l));
            return Some(0x401 | if vinyl { 0x10 } else { 0x8 });
        }
        // A decal slot's list (UI_DecalsOverlay, 0x556880 for slots
        // 0x34–0x3E and 0x61–0x98): 0x800421, Decal Color, Back, Install
        // Decal, Quit Game (0x54E6F0 at 0x556929; so under Wine).
        if self.package == "UI_DecalMain" && self.part.is_some() {
            return Some(0x80_0421);
        }
        // A neon list: UI_NeonPartsOverlay's 0x409 (Install Part; under
        // Wine, menus103). The Custom Gauges list keeps Continue there.
        if self.package == "UI_CustomNeonMain" && self.part.is_some() {
            return help_mask("UI_NeonPartsOverlay");
        }
        // A specialties list in UI_IcePartsOverlay (every category but
        // Custom Gauges, whose list is UI_CustomHUDOverlay): its 0x409,
        // Install Part (Wine, the finished-career save's Window Tint list: Back,
        // Install Part, Quit Game; the clone said Continue).
        // The trunk audio's lists are the same overlay (0x5593F0 modes
        // 2–5: Install Part under Wine, the finished-career save's layout and
        // component lists).
        if self.package == "UI_ICEMAIN"
            && self.part.is_some()
            && self.categories.get(self.category).is_some_and(|c| c.slot != 0xA3)
        {
            // 0x54FB80 on entering a list: Nitrous Purge 0x80409 (Test N2O
            // Purge, 0xBA0E1020), the split hoods 0x100409 (Open/Close Hood,
            // 0xEAA1D5DF), the doors 0x200409 (Open/Close Doors,
            // 0x3E9431DC) (Wine, the finished-career save's purge list: Test N2O
            // Purge, Back, Install Part, Quit Game).
            match self.categories.get(self.category).map(|c| c.slot) {
                Some(0xA2) => return Some(0x8_0409),
                Some(0x09) => return Some(0x10_0409),
                Some(0xA0) => return Some(0x20_0409),
                _ => {}
            }
            return help_mask("UI_IcePartsOverlay");
        }
        help_mask(self.package)
    }

    pub fn key(&mut self, key: Key) -> BrowserKey {
        match self.part {
            None => match key {
                Key::Left if self.category > 0 => self.category -= 1,
                Key::Right if self.category + 1 < self.categories.len() => self.category += 1,
                Key::Accept => match self.categories.get(self.category).filter(|c| !c.locked) {
                    Some(c) if c.action == CategoryAction::Open => return BrowserKey::Open,
                    Some(c) if matches!(c.action, CategoryAction::Toggle(_)) => return BrowserKey::Toggle,
                    Some(c) if !c.parts.is_empty() => self.part = self.opening_part(crate::qol::lists_open_installed()),
                    _ => {}
                },
                Key::Back => return BrowserKey::Leave,
                _ => {}
            },
            Some(i) => {
                let n = self.categories.get(self.category).map_or(0, |c| c.parts.len());
                match key {
                    Key::Up if i > 0 => self.part = Some(i - 1),
                    Key::Down if i + 1 < n => self.part = Some(i + 1),
                    Key::Accept if i < n => return BrowserKey::Install(i),
                    Key::Back => self.part = None,
                    _ => {}
                }
            }
        }
        BrowserKey::None
    }

    /// The row the current category's list opens on: the installed part
    /// when listed (0x5606F0 …), else the head. UI_CustomHUDOverlay's
    /// constructor (0x56C220) fills its list again after the base's
    /// (0x56BF80 → 0x5606F0, on the fitted part) with 0x53C200, which
    /// selects the first item (0x5386B0): Custom Gauges opens on STOCK,
    /// unless `on_installed` (Lists Open on Installed, a clone deviation).
    pub fn opening_part(&self, on_installed: bool) -> Option<usize> {
        let c = self.categories.get(self.category).filter(|c| !c.parts.is_empty())?;
        if !on_installed && self.package == "UI_ICEMAIN" && c.slot == 0xA3 && self.sub.is_none() {
            return Some(0);
        }
        Some(c.parts.iter().position(|p| p.4).unwrap_or(0))
    }

    /// The list's own screen over the strip (UI_ICEMAIN's, the neon's and
    /// the decals' parts), when the browser has one.
    pub fn overlay(&self) -> Option<&'static str> {
        let name = match self.package {
            "UI_CustomNeonMain" => "UI_NeonPartsOverlay",
            // A decal slot's decals (0x556880 opens it over the shop).
            "UI_DecalMain" => "UI_DecalsOverlay",
            _ if self.categories.get(self.category).is_some_and(|c| c.slot == 0xA3) => "UI_CustomHUDOverlay",
            _ => "UI_IcePartsOverlay",
        };
        matches!(self.package, "UI_ICEMAIN" | "UI_CustomNeonMain" | "UI_DecalMain").then_some(name)
    }

    pub fn quads(&self, assets: &Assets, profile: Option<&Profile>, now: f32) -> Vec<Quad> {
        use parts_browser::*;
        let Some(package) = assets.package(self.package) else { return Vec::new() };
        // The rim styles and the car specialties strip on OPTION_MASTER;
        // the specialties' parts go down UI_IcePartsOverlay's column.
        let rims = matches!(
            self.package,
            "UI_Rims_Browser"
                | "UI_ChooseSpinner"
                | "UI_ChooseUniquePart"
                | "UI_ICEMAIN"
                | "UI_CustomNeonMain"
                | "UI_DecalMain"
        );
        let overlay = self.overlay().and_then(|n| assets.package(n));
        let mut state = ScreenState::default();
        footer(&mut state, package, "Install");
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        if self.title != 0 {
            state.text.insert(TITLE, label(self.title));
        }
        let category = self.categories.get(self.category);
        if let Some(c) = category {
            let text = c.text.clone().unwrap_or_else(|| label(c.label));
            state.text.insert(if rims { RIM_OPTION_TITLE } else { OPTION_TITLE }, text);
        }
        if let Some(p) = profile {
            state.text.insert(CASH, crate::frontend::grouped(p.cash as i64));
        }
        state.hidden.insert(PLAYER_NAME);
        // The body shop has no total (0x545470 hides it); the graphics
        // shop's shows in a career (0x5453C0).
        let _ = TOTAL_COST;
        total_cost(&mut state, self.total);
        state.hidden.insert(CATEGORY_ICON);
        state.hidden.insert(PARTS_ICON);
        let part = self.part.and_then(|i| category.and_then(|c| c.parts.get(i)));
        match part {
            Some((_, name, price, owned, _)) => {
                state.text.insert(PART_NAME, name.clone());
                // The price shows only in a career (0x546FC0: 0x850078 = 1,
                // else hidden by 0x4954E0), not in main-menu Customize.
                if profile.is_some_and(|p| p.scratch) {
                    state.hidden.insert(PART_PRICE);
                } else {
                    // `%$d` (0x78F744: the number grouped by thousands, no
                    // currency sign; 0x5456DB …) of the part's cost
                    // (0x514280), owned or fitted alike (Wine, the finished-career
                    // save's fitted SPYDER bumper: "1,000"). Two browsers
                    // print `%d` (0x5469D3, 0x54744A): the same below 1,000.
                    let _ = owned;
                    state.text.insert(PART_PRICE, crate::frontend::grouped(i64::from(*price)));
                }
                state.hidden.insert(CATEGORY_ONLY);
            }
            None => {
                state.hidden.insert(PART_NAME);
                state.hidden.insert(PART_PRICE);
                state.hidden.extend(INSTALL);
            }
        }
        // LOCK_MASTER INITs to HIDE; SHOW while the selected category is
        // locked.
        if self.part.is_none() && category.is_some_and(|c| c.locked) {
            state.shown.insert(LOCK_MASTER);
        } else {
            state.hidden.insert(LOCK_MASTER);
        }
        // The strip's master is where the selected category sits; it draws
        // nothing itself.
        let (master, template) = if rims { (OPTION_MASTER, OPTION_TEMPLATE) } else { (CATEGORY_MASTER, CATEGORY_ICON) };
        state.hidden.insert(master);
        state.hidden.insert(template);
        if overlay.is_some() {
            // UI_ICEMAIN's own Install / select rows (0x5593F0 hides
            // them until a category is entered).
            if self.part.is_none() {
                state.hidden.insert(fe_ids::INSTALL);
                state.hidden.insert(0xA228_5688);
            }
            state.hidden.insert(fe_ids::CAR_LOT_CUSTOMIZE);
            state.hidden.insert(fe_ids::INSTALL_1);
        }
        // UI_DecalMain's, UI_ICEMAIN's and UI_CustomNeonMain's strips the
        // same (under Wine, menus103: the handle at the start and no left
        // arrow on Windshield, Custom Gauges, Underglow Neon; the same
        // IconScrollerMenu class **inferred** from the look).
        // UI_ChooseSpinner the same as UI_Rims_Browser (Wine, the finished-career
        // save's Spinners: no left arrow on the first style, no column bar
        // until a style is entered; the clone showed both).
        if !rims
            || matches!(
                self.package,
                "UI_Rims_Browser" | "UI_ChooseSpinner" | "UI_DecalMain" | "UI_ICEMAIN" | "UI_CustomNeonMain"
            )
        {
            // The category strip is an IconScrollerMenu (0x545B60): its
            // "ScrollBar" (0x538090(1, count, position), 0x52E330) with
            // Arrow_1 hidden on the first category and Arrow_2 on the
            // last (0x52FA80). Under Wine (main-menu Customize's Body
            // Shop, 2026-10-29) the handle sits at the start on Front
            // Bumpers and the left arrow is off; the clone left the
            // package's handle at its authored x 216.
            let n = self.categories.len();
            scroll_bar_at(&mut state, package, "ScrollBar", true, 1, n, if n == 0 { 0 } else { self.category + 1 });
            if self.category == 0 {
                state.hidden.insert(fe_ids::POST_RACE_STATS_LEFT);
            } else if self.category + 1 == n {
                state.hidden.insert(fe_ids::POST_RACE_STATS_RIGHT);
            }
            self.arrows.apply(&mut state, SCROLLBAR_ARROWS, self.opened);
            // UI_Rims_Browser's refresh 0x5468B0 runs its sizes column's
            // bar the same way (+0x1B4: 0x538090(1, +0x210, position)); under
            // Wine its styles strip shows the handle at the start and no
            // left arrow on the first style, and no column until a style
            // is entered, as here.
            // The refresh 0x545520 runs both bars in either mode: the
            // strip's (+0x114) and the parts column's "SideScrollBar"
            // (+0x16C, 0x542AD0 with flag 1 in the constructor at
            // 0x55E8F3; 0x538090(1, column count +0x1C8, position)). On
            // entering a category (0x55EDE0) nothing hides the strip:
            // under Wine (main-menu Customize, Front Bumpers' parts) its
            // icons, handle and arrows stay as on the categories.
            match self.part.zip(category) {
                None => {
                    // The parts column's scroller (group 0x4A1316E9, its
                    // master 0x38E39AD7 / frame 0xD82B4A08, arrows
                    // 0x640F0F22 / 23 = SideScrollBar_Arrow_1 / 2, bar
                    // 0x884FBA76 = SideScrollBar_Backing, handle
                    // 0xF31E8ED3 = SideScrollBar_Handle; all flagged 4):
                    // INIT → HIDE until a category's parts are listed.
                    state.hidden.extend([
                        0x4A13_16E9,
                        fe_ids::MINI_MASTER,
                        fe_ids::U2_MINI_ICON_BACKING,
                        fe_ids::SIDESCROLLBAR_ARROW_1,
                        fe_ids::OBJ_640F0F23,
                        fe_ids::SIDESCROLLBAR_BACKING,
                        fe_ids::SIDESCROLLBAR_HANDLE,
                    ]);
                }
                Some((i, c)) => {
                    let n = c.parts.len();
                    scroll_bar_at(&mut state, package, "SideScrollBar", false, 1, n, if n == 0 { 0 } else { i + 1 });
                    // The arrows by the column's ends, as the strip's
                    // (the IconScrollerMenu's 0x52FA80 on its +0x80 /
                    // prov: inferred frontend.screens.quads
                    // +0x84; **inferred** that the column's are the
                    // SideScrollBar arrows). Under Wine: no up arrow on
                    // the first part, both on the second.
                    if i == 0 {
                        state.hidden.insert(fe_ids::SIDESCROLLBAR_ARROW_1);
                    }
                    if i + 1 >= n {
                        state.hidden.insert(fe_ids::OBJ_640F0F23);
                    }
                }
            }
        }
        let mut quads = layout(assets, package, &state, now - self.opened);
        // A decal, specialties or neon list keeps the strip's icons (under
        // Wine, menus103).
        // The exit question hides the strip's items at once (0x567266:
        // +0x2F = 0, 0x52EBA0(0)) before the handler's MENUFADEOUT; No
        // shows them again (0x567505, 0x52EBA0(1)).
        let strip_off = crate::frontend::dim_script(self.package) == Some(crate::frontend::MENUFADEOUT_SCRIPT);
        if !strip_off
            && (self.part.is_none()
                || !rims
                || matches!(self.package, "UI_Rims_Browser" | "UI_DecalMain" | "UI_ICEMAIN" | "UI_CustomNeonMain"))
        {
            let icons: Vec<u32> = self.categories.iter().map(|c| c.icon).collect();
            quads.extend(scroller_quads(package, master, template, &icons, self.category, true, 40.0, 480.0));
        }
        match (self.part, category) {
            (None, _) => {}
            (Some(i), Some(c)) => {
                let icons = vec![PART_TEXTURE; c.parts.len()];
                let column = match overlay {
                    Some(o) => {
                        let mut os = ScreenState::default();
                        footer(&mut os, o, "Install");
                        os.hidden.insert(PARTS_ICON);
                        if let Some((_, name, price, owned, _)) = c.parts.get(i) {
                            os.text.insert(PART_NAME, name.clone());
                            // Hidden outside a career (0x546FC0 in the decal
                            // list's fill 0x546F50; under Wine no price in
                            // Customize).
                            // The part's cost (0x514280 on the part in the
                            // shop's list 0x5214E0 or on the car 0x610300; "0"
                            // with neither), owned or not (Wine, the finished-career
                            // save's DARK BLACK tint: "200", XENON 8000k
                            // lights: "550"; the clone printed 0).
                            let _ = owned;
                            // "No Decal" (hash 0): 0x546F50 hides the price
                            // (0x4954E0 on 0x380B632) with no part (Wine:
                            // blank beside "No Decal"; the clone printed 0).
                            let no_part = self.package == "UI_DecalMain" && c.parts.get(i).is_some_and(|p| p.0 == 0);
                            if no_part || profile.is_some_and(|p| p.scratch) {
                                os.hidden.insert(PART_PRICE);
                            } else {
                                os.text.insert(PART_PRICE, crate::frontend::grouped(i64::from(*price)));
                            }
                        }
                        // The overlay's column bar and end arrows as the
                        // parts browser's (same SideScrollBar objects;
                        // under Wine: handle at the top, no up arrow on
                        // "No Decal").
                        let n = c.parts.len();
                        scroll_bar_at(&mut os, o, "SideScrollBar", false, 1, n, if n == 0 { 0 } else { i + 1 });
                        if i == 0 {
                            os.hidden.insert(fe_ids::SIDESCROLLBAR_ARROW_1);
                        }
                        if i + 1 >= n {
                            os.hidden.insert(fe_ids::OBJ_640F0F23);
                        }
                        quads.extend(layout(assets, o, &os, now - self.opened));
                        o
                    }
                    None => package,
                };
                quads.extend(scroller_quads(column, MINI_MASTER, PARTS_ICON, &icons, i, false, 20.0, 200.0));
            }
            _ => {}
        }
        quads
    }
}
