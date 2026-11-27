use crate::frontend::fe_ids;
use super::*;

/// A car for the car lot.
#[derive(Clone, Debug, PartialEq)]
pub struct CarChoice {
    /// The car type's name, e.g. "PEUGOT".
    pub name: String,
    /// Make and model from the language table ("Peugeot", "206").
    pub title: String,
    /// The career slot it is in, for the player's own cars.
    pub slot: Option<usize>,
    /// A crib Car Select placeholder (0x4E0160): a locked slot (+0x930 and
    /// +0x931 set) or an open one without a car (+0x931 only).
    pub placeholder: Option<Placeholder>,
    /// A "My Cars" record (its index), with its parts.
    pub my_car: Option<usize>,
    /// A sponsor car's preset ride (GLOBALB 0x30220, "CHINGY"): the
    /// garage record's +0x18 (0x5163C0), whose parts it shows and races
    /// with.
    pub preset: Option<String>,
}

/// UICareerCribCarSelect's slots without a car of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placeholder {
    /// The slot is locked (slot record +0x7F4).
    Locked,
    /// The slot is open but has no car (+0x7F5 clear).
    Open,
}

/// UI_CareerCarSelect's objects (0x4B0140).
mod crib_select {
    /// "SLOT_TITLE": "%s %d" with "Slot" (0x93475966).
    pub const SLOT_TITLE: u32 = crate::frontend::fe_ids::SLOT_TITLE;
    /// "SPONSOR_LOCKED_TEXT": "OPEN FOR SPONSOR CAR" / "LOCKED".
    pub const LOCKED_TEXT: u32 = crate::frontend::fe_ids::LOCKED_TEXT;
    /// "BOTTOM_GROUP": the rating bars.
    pub const BOTTOM_GROUP: u32 = crate::frontend::fe_ids::BOTTOM_GROUP;
}

impl CarChoice {
    pub fn new(assets: &Assets, name: &str, slot: Option<usize>) -> CarChoice {
        let make = assets.strings.get(bin_hash(name)).unwrap_or(name);
        let model = assets.strings.get(bin_hash(&format!("CAR_NAME_{name}"))).unwrap_or(name);
        let title = if make == model { model.to_string() } else { format!("{make} {model}") };
        CarChoice { name: name.into(), title, slot, placeholder: None, my_car: None, preset: None }
    }

    /// A placeholder for career slot `slot`.
    pub fn placeholder(slot: usize, kind: Placeholder) -> CarChoice {
        CarChoice {
            name: String::new(),
            title: String::new(),
            slot: Some(slot),
            placeholder: Some(kind),
            my_car: None,
            preset: None,
        }
    }
}

/// UI_CareerCarLot's state (UICareerCarLot, constructor 0x4F9930, vtable
/// 0x795CF4, handler 0x4ED9E0; 0x495F00 picks the next, 0x4ED8A0 sets it
/// up). On opening: 2 when an open slot has no car (take this stock
/// car), else 3 with one car (trade it in), else 4 (trade it in or choose
/// another of the player's cars); 5 lists the player's cars to trade;
/// 6 saves (0x52A4D0, the memory card flow) and goes back to the city.
/// The crib's car select (UICareerCribCarSelect) uses state 0 here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LotState {
    Take,
    TradeIn,
    TradeOrBrowse,
    /// Choosing which car to trade for this stock one.
    Browse,
    /// The crib's car select: the player's cars to drive.
    CribSelect,
    /// Quick Race's UI_QRCarSelect before the event's race (None: Free
    /// Run).
    QuickRace(Option<usize>),
    /// Main-menu Customize's UI_QRCarSelect (game mode 0x20).
    Customize,
}

/// UI_CareerCarLot: the cars on offer, or the player's own.
#[derive(Clone)]
pub struct CarLot {
    pub title: String,
    pub cars: Vec<CarChoice>,
    pub selected: usize,
    pub opened: f32,
    pub state: LotState,
    /// The stock car to be had while browsing the player's cars (+0x64).
    pub stock: Option<String>,
    /// UI_QRCarSelect's filter (0x7F444C, kept in the profile at
    /// 0x83AAC0): bits 1 Stock, 2 My Cars, 4 Career, 8 Sponsor, 0x10
    /// Ranked; 3 is All Cars.
    pub filter: u32,
    /// The logo box's arrows' blink (0x4E2590, [`ArrowBlink`]).
    pub arrows: ArrowBlink,
}

/// The name of Quick Race's car filter (0x4B2746): 3 "All Cars", else the
/// highest bit's — 1 "Stock Cars", 2 "My Cars", 4 "Career", 8 "SPONSOR
/// CARS", 0x10 "Ranked Cars".
pub fn qr_filter_label(filter: u32) -> u32 {
    if filter == 3 {
        return 0x6B68_362C;
    }
    let mut label = fe_ids::OBJ_E798A2F7;
    for (bit, hash) in [(1, fe_ids::OBJ_E798A2F7), (2, 0xE7AB_43D3), (4, 0xB404_5365), (8, 0x26FE_CEE7), (0x10, 0x1A23_D5E6)] {
        if filter & bit != 0 {
            label = hash;
        }
    }
    label
}

/// The filter the shoulder triggers switch to in Quick Race (0x4EED10 in
/// game modes 2 and 4): the right trigger (PAD_RTRIGGER 0xD9FEEC59) steps
/// All → Stock → My Cars → Career → Sponsor → All, the left
/// (PAD_LTRIGGER 0x5073EF13) the other way, skipping a filter with no
/// car (`has`, 0x497EE0 → 0x534860: records of the filter that Quick
/// Race has unlocked); Stock is never skipped.
pub fn next_qr_filter(filter: u32, forward: bool, has: impl Fn(u32) -> bool) -> u32 {
    // The filters after `filter` in the stepping order, first one with cars.
    let order: [u32; 5] = if forward { [3, 1, 2, 4, 8] } else { [3, 8, 4, 2, 1] };
    let Some(at) = order.iter().position(|&f| f == filter) else { return filter };
    for step in 1..order.len() {
        let f = order[(at + step) % order.len()];
        if f == 3 || f == 1 || has(f) {
            return f;
        }
    }
    filter
}

impl CarLot {
    /// The help-bar mask 0x4B2310 hands 0x54E6F0: the record's +0xC
    /// (UI_QRCarSelect 0x1405: Quit Game, Continue, Back, Customize) less
    /// Customize (0x1000) in Customize (game mode 0x20), for the Career and
    /// Sponsor filters (4, 8) and for a car in a career slot (0x516340).
    /// In a career (game mode 1) none of 0x4B2310's mode branches run and
    /// the screen's own record mask goes to 0x54E6F0 unchanged:
    /// UI_CareerCarLot / UI_CareerCarSelect 0x405 (Back, Continue, Quit
    /// Game, under Wine at the D-Day car lot, career playthrough 2026-10-31).
    pub fn help_mask(&self) -> Option<u32> {
        let quick = matches!(self.state, LotState::QuickRace(_));
        if !quick && self.state != LotState::Customize {
            return help_mask(if self.state == LotState::CribSelect {
                "UI_CareerCarSelect"
            } else {
                "UI_CareerCarLot"
            });
        }
        let mut mask = help_mask("UI_QRCarSelect")?;
        if self.state == LotState::Customize
            || matches!(self.filter, 4 | 8)
            || self.cars.get(self.selected).is_some_and(|c| c.slot.is_some())
        {
            mask &= !0x1000;
        }
        Some(mask)
    }

    pub fn key(&mut self, key: Key, now: f32) -> Option<usize> {
        // Left / Right step the car (0x4AFFA0, 0x4B04C0, 0x4E2590), not
        // past the first or last, and blink that way's arrow from 0
        // (0x5426D0 on "ScrollBar_Arrow_1" / "_2"); Up / Down are not
        // handled.
        match key {
            Key::Left if self.selected > 0 => {
                self.selected -= 1;
                self.arrows.step(false, now, true);
            }
            Key::Right if self.selected + 1 < self.cars.len() => {
                self.selected += 1;
                self.arrows.step(true, now, true);
            }
            Key::Accept if !self.cars.is_empty() => return Some(self.selected),
            _ => {}
        }
        None
    }

    /// UI_CareerCarLot as 0x4F9D90 / 0x4B2310 fill it in a career (game
    /// mode 1: the package's own "Car Lot" title and "Select Car" button):
    /// the selected car's manufacturer logo (0x511E60:
    /// CARSELECT_MANUFACTURER_<record +0xC0>) and model badge
    /// (SECONDARY_LOGO_<car>, else GENERICPLACEHOLDER), and the three
    /// rating bars (0x497CD0).
    pub fn quads(
        &self,
        assets: &Assets,
        profile: Option<&Profile>,
        make: Option<&str>,
        handling: Option<[f32; 3]>,
        now: f32,
    ) -> Vec<Quad> {
        let customize = self.state == LotState::Customize;
        let quick = customize || matches!(self.state, LotState::QuickRace(_));
        // The crib's Car Select is UI_CareerCarSelect.fng (factory 0x4FC110,
        // table 0x7F7F6C).
        let crib = self.state == LotState::CribSelect;
        let name = if quick {
            "UI_QRCarSelect"
        } else if crib {
            "UI_CareerCarSelect"
        } else {
            "UI_CareerCarLot"
        };
        let Some(package) = assets.package(name) else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select Car");
        // Quick Race (game mode 2, 0x4F9D90 / 0x4B2310): the package's
        // "Quick Race Car Select" title, the filter's name on both
        // "Filter type goes here" strings (0x4B2746, `qr_filter_label`), the
        // race type after "Race Mode:"; Customize (0xA936C3A2) and
        // 0x89782734 hidden (0x4B2310; shown for filters 4 and 8 only).
        if quick {
            for h in [fe_ids::CAR_LOT_CUSTOMIZE, fe_ids::INSTALL_1] {
                state.hidden.insert(h);
            }
            let filter = assets.strings.get(qr_filter_label(self.filter)).unwrap_or("All Cars").to_string();
            for h in [0x3E81_DE59, fe_ids::FILTERTYPE] {
                state.text.insert(h, filter.clone());
            }
            state.text.insert(fe_ids::OBJ_A9205BD8, self.title.clone());
            state.hidden.insert(ui_main::CASH);
        }
        // Customize (0x4B2310 in game mode 0x20): "Customize Car Select"
        // (0x61032CE5); a stock car shows "Customize New Car" (0xC8054CE0)
        // on 0xA936C3A2 with 0x89782734. The race mode row has no race.
        if customize {
            let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
            state.text.insert(ui_main::TITLE, label(0x6103_2CE5));
            state.text.insert(fe_ids::CAR_LOT_CUSTOMIZE, label(fe_ids::TXT_CUSTOMIZE_NEW_CAR));
            for h in [fe_ids::CAR_LOT_CUSTOMIZE, fe_ids::INSTALL_1] {
                state.hidden.remove(&h);
                state.shown.insert(h);
            }
            for h in [0x6578_FB3F, fe_ids::OBJ_A9205BD8] {
                state.hidden.insert(h);
            }
        }
        if let (Some(p), false) = (profile, quick) {
            state.text.insert(ui_main::CASH, crate::frontend::grouped(p.cash as i64));
        }
        // 0x4ED8A0 shows FILTERTYPE as "Career Cars" in states 4 and 5;
        // UI_CareerCarLot has no such object, so nothing changes.
        state.hidden.insert(ui_main::PLAYER_NAME);
        // UICareerCribCarSelect's refresh (0x4B0140): SLOT_TITLE "Slot %d";
        // a car shows CARBADGE and BOTTOM_GROUP and hides
        // SPONSOR_LOCKED_TEXT; a placeholder the reverse, with "OPEN FOR
        // SPONSOR CAR" (0xCA89507E) and image 0xDA665928, or "LOCKED"
        // (0xC9F7F7E5) and image 0xF05704AC, on the logo.
        // UICareerCribCarSelect's set-up (0x4EE210, from the constructor
        // 0x4F9AD0): MANUFACTURERLOGO shown; FILTERTYPE, CUSTOMIZE,
        // JOY_EVENT(141) / (142) and Trade_Group (the "Trade - In Cost/
        // Reimburse:" bar, 0xD42F677B) hidden.
        if crib {
            state
                .hidden
                .extend(["FILTERTYPE", "CUSTOMIZE", "JOY_EVENT(141)", "JOY_EVENT(142)", "Trade_Group"].map(fe_hash));
        }
        if crib && let Some(car) = self.cars.get(self.selected) {
            let label = |h: u32, fallback: &str| assets.strings.get(h).unwrap_or(fallback).to_string();
            // The number is the screen's +0x5C ("%s %d", 0x790EF4): 1 on
            // the list's head (0x4F9B17; 0x4E03A6 puts +0x54 there), +1 / −1
            // with each step (0x4B052F / 0x4B0587): the list position.
            state.text.insert(crib_select::SLOT_TITLE, format!("{} {}", label(fe_ids::TXT_SLOT, "Slot"), self.selected + 1));
            match car.placeholder {
                None => {
                    state.hidden.insert(crib_select::LOCKED_TEXT);
                    state.shown.insert(crib_select::BOTTOM_GROUP);
                }
                Some(kind) => {
                    let (text, image) = match kind {
                        Placeholder::Open => (label(0xCA89_507E, "OPEN FOR SPONSOR CAR"), 0xDA66_5928),
                        Placeholder::Locked => (label(0xC9F7_F7E5, "LOCKED"), 0xF057_04AC),
                    };
                    state.text.insert(crib_select::LOCKED_TEXT, text);
                    state.shown.insert(crib_select::LOCKED_TEXT);
                    state.hidden.insert(crib_select::BOTTOM_GROUP);
                    state.hidden.insert(car_lot::BADGE);
                    if assets.textures.contains_key(&image) {
                        state.image.insert(car_lot::LOGO, image);
                        state.shown.insert(car_lot::LOGO);
                    }
                }
            }
        }
        if let Some(car) = self.cars.get(self.selected).filter(|c| c.placeholder.is_none()) {
            let placeholder = assets.texture_for("GENERICPLACEHOLDER");
            let (make, badge) = region_logos(&car.name, make);
            let logo = make.and_then(|m| assets.texture_for(&format!("CARSELECT_MANUFACTURER_{m}"))).or(placeholder);
            let badge = assets.texture_for(&format!("SECONDARY_LOGO_{badge}")).or(placeholder);
            for (object, texture) in [(car_lot::LOGO, logo), (car_lot::BADGE, badge)] {
                if let Some(t) = texture {
                    state.image.insert(object, t);
                    state.shown.insert(object);
                }
            }
        }
        // 0x4B2310's list scroll bar (0x538090(1, count +0x60, position
        // +0x64)) and its arrows ScrollBar_Arrow_1 / _2 (the logo box's
        // left / right): the first car hides the left one, the last the
        // right one (0x4F9D90 sets both from the list's ends; 0x4B2310
        // updates them). The crib's select has its own refresh (0x4B0140).
        // The bar's prefix (0x542AD0) is "ScrollBar" (0x79648C, 0x4F9DFC)
        // in UI_QRCarSelect and "Scroll_Bar" (0x798F38, 0x4F9B2B in the
        // constructor 0x4F9AD0) in UI_CareerCarLot, whose objects carry
        // that name (Scroll_Bar_Handle 0x95043B2D, _Arrow_1 / _2
        // 0x42AA46BC / BD): the first car hides the left arrow and puts
        // the handle at the bar's left (Wine, menus107 c13 / c24).
        // The crib's refresh 0x4B0140 sets it too (0x538090(1, +0x58,
        // +0x5C): the list's count, which 0x4E0160 counts up per entry,
        // and the slot number).
        {
            let n = self.cars.len();
            let prefix =
                if package.object(fe_hash("Scroll_Bar_Handle")).is_some() { "Scroll_Bar" } else { "ScrollBar" };
            scroll_bar_at(&mut state, package, prefix, true, 1, n, if n == 0 { 0 } else { self.selected + 1 });
            // The crib's select keeps both arrows (Wine, finished-career save: the
            // left one shown on slot 1 of 5; 0x4B0140's 0x50D820 calls do
            // not reach the package's Scroll_Bar_Arrow_1 / _2).
            if n > 1 && !crib {
                let (left, right) = (fe_hash(&format!("{prefix}_Arrow_1")), fe_hash(&format!("{prefix}_Arrow_2")));
                for h in [left, right] {
                    state.hidden.remove(&h);
                }
                if self.selected == 0 {
                    state.hidden.insert(left);
                } else if self.selected + 1 == n {
                    state.hidden.insert(right);
                }
            }
        }
        // The steps' blink names ScrollBar_Arrow_1 / _2 in every lot: the
        // career lot's and the crib's packages have none (see
        // [`ArrowBlink`]).
        self.arrows.apply(&mut state, SCROLLBAR_ARROWS, self.opened);
        // OL_CarMode_Group (the "Race Mode:" panel) shows only in
        // UI_OLCarLot.fng (0x4F9D90 hides it in every other package; the
        // online modes 8 / 0x100 with the Ranked filter show it).
        state.hidden.insert(fe_hash("OL_CarMode_Group"));
        // The three rating bars (0x497BF0 builds them with 0x540C20: range
        // 0..1, end pads 46; 0x497CD0 sets each to the car's rating, 0x4FF550
        // clamping it to the range, and lays it out with 0x540D30).
        if let Some(ratings) = handling {
            for (prefix, value) in ["accel", "top_speed", "handling"].into_iter().zip(ratings) {
                lot_bar(&mut state, package, prefix, value);
            }
        }
        layout(assets, package, &state, now - self.opened)
    }
}

/// UI_Slider_Layout (0x540D30) for a 0..1 bar with 46-pixel end pads
/// (0x540C20 from 0x497BF0): `<prefix>_backing` stays; with f the value,
/// (bx, by) the backing's top left (0x51D080: position − size / 2), bw ×
/// bh its size and hw × hh the handle's, the track is bw − 92 wide; the
/// handle's top left goes to (f·track − hw/2 + 46 + bx, bh/2 + by − hh/2 −
/// 1) and `<prefix>_cur` is f·track wide (0x495590) with its top left at
/// (bx + 46, bh/2 + by − hh/2 + 1) (0x51D390: top left + size / 2 for an
/// image).
pub(super) fn lot_bar(state: &mut ScreenState, package: &crate::frontend::feng::Package, prefix: &str, value: f32) {
    const PAD: f32 = 46.0;
    let get = |name: &str| package.object(fe_hash(&format!("{prefix}_{name}")));
    let (Some(backing), Some(handle), Some(cur)) = (get("backing"), get("handle"), get("cur")) else { return };
    let (bp, bs) = (backing.attributes.position(), backing.attributes.size());
    let (bx, by) = (bp.x - bs.x * 0.5, bp.y - bs.y * 0.5);
    let hs = handle.attributes.size();
    let f = value.clamp(0.0, 1.0);
    let track = bs.x - 2.0 * PAD;
    let y = bs.y * 0.5 + by - hs.y * 0.5 + 1.0;
    let hx = f * track - hs.x * 0.5 + PAD + bx;
    state.position.insert(handle.hash, Vec2::new(hx + hs.x * 0.5, y - 2.0 + hs.y * 0.5));
    let (width, ch) = (f * track, cur.attributes.size().y);
    state.size.insert(cur.hash, Vec2::new(width, ch));
    state.position.insert(cur.hash, Vec2::new(bx + PAD + width * 0.5, y + ch * 0.5));
}

/// "ManufacturerLogo" and "CarBadge" (0x4EEB90).
pub const LOGO: u32 = fe_ids::LOGO;
pub const BADGE: u32 = fe_ids::BADGE;

/// The car lot's make and badge names in the game's region
/// ([`crate::region`]; see [`region_logos_in`]).
pub fn region_logos<'a>(car: &'a str, make: Option<&'a str>) -> (Option<&'a str>, &'a str) {
    region_logos_in(crate::region::get(), car, make)
}

/// The car lot's make and badge names by region (0x511E60). The clone runs
/// in English (language 0x7F70D0 = 0). In a European region (0x570A50;
/// the European release's 0x7FA73C is 6) the Corsa (car type 7) shows
/// VAUXHALL and the CORSA badge, the Miata (4) MX5 and the Tiburon (11)
/// COUPE; every other car its own make and name. Region 0 keeps every
/// car's own (MIATA, TIBURON). Japan, Korea and the other languages have
/// their own and are not handled.
pub fn region_logos_in<'a>(region: u32, car: &'a str, make: Option<&'a str>) -> (Option<&'a str>, &'a str) {
    if region == 0 {
        return (make, car);
    }
    match car {
        "CORSA" => (Some("VAUXHALL"), "CORSA"),
        "MIATA" => (make, "MX5"),
        "TIBURON" => (make, "COUPE"),
        _ => (make, car),
    }
}

#[cfg(test)]
mod region_tests {
    use super::region_logos_in;

    /// 0x511E60 in English: a European region renames the Corsa, Miata and
    /// Tiburon; region 0 keeps every car's own make and name.
    #[test]
    fn logos_by_region() {
        assert_eq!(region_logos_in(6, "CORSA", Some("OPEL")), (Some("VAUXHALL"), "CORSA"));
        assert_eq!(region_logos_in(6, "MIATA", Some("MAZDA")), (Some("MAZDA"), "MX5"));
        assert_eq!(region_logos_in(6, "TIBURON", Some("HYUNDAI")), (Some("HYUNDAI"), "COUPE"));
        for car in ["CORSA", "MIATA", "TIBURON", "CIVIC"] {
            assert_eq!(region_logos_in(0, car, Some("MAKE")), (Some("MAKE"), car));
        }
    }
}
