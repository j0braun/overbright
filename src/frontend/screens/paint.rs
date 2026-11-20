use crate::frontend::fe_ids;
use super::*;

/// UI_Paint's paint slots (0x53BD60, mode 0): slot, icon, label.
pub const PAINT_SLOTS: [(u8, u32, u32); 9] = [
    (0x3F, fe_ids::BASE_PAINT_ICON, fe_ids::BASE_PAINT_LABEL), // Base Paint
    (0x45, fe_ids::SPOILER_ICON, fe_ids::SPOILER_LABEL), // Spoiler
    (0x4B, fe_ids::ROOF_SCOOP_ICON, fe_ids::ROOF_SCOOP_LABEL), // Roof Scoop
    (0x4C, fe_ids::MIRRORS_ICON, fe_ids::MIRRORS_LABEL), // Mirrors
    (0x49, fe_ids::RIMS_AND_SPINNERS_ICON, fe_ids::RIMS_LABEL), // Rims
    (0x4A, fe_ids::RIMS_AND_SPINNERS_ICON, fe_ids::PAINT_SPINNERS_LABEL), // Spinners
    (0x46, fe_ids::BRAKES_ICON, fe_ids::BRAKES_LABEL), // Brakes
    (0x44, fe_ids::ENGINE_ACCENTS_ICON, fe_ids::ENGINE_ACCENTS_LABEL), // Engine Accents
    (0x47, fe_ids::MUFFLER_TIP_ICON, fe_ids::MUFFLER_TIP_LABEL), // Muffler Tip
];

/// The paint finishes a slot starts on (0x50F5B0); the body-colour slots
/// cycle gloss, metal and pearl on the triggers (0x55D0E0).
pub fn paint_finish(slot: u8) -> u32 {
    match slot {
        0x3F | 0x45 | 0x4B | 0x4C => GLOSS,
        0x44 => 0x0016_BDCE,
        0x46 => 0xD664_0DFF,
        0x47 => fe_ids::MUFFLER,
        0x49 | 0x4A => 0x0000_DA27,
        _ => 0,
    }
}

/// Whether UI_Paint's finish arrows show with `entered` the slot entered
/// (None on the strip): only the finish slots 0x3F, 0x45, 0x4B, 0x4C
/// (0x55CC60's jump table 0x55CE88 by the byte table 0x55CE90).
pub fn finish_arrows_shown(entered: Option<u8>) -> bool {
    matches!(entered, Some(0x3F | 0x45 | 0x4B | 0x4C))
}

pub const GLOSS: u32 = fe_ids::GLOSS;

pub const METAL: u32 = fe_ids::METAL;

pub const PEARL: u32 = fe_ids::PEARL;

/// A paint as UI_Paint shows it: part hash, swatch colour (DISPRED /
/// DISPGREEN / DISPBLUE, else RED / GREEN / BLUE, 0–255), price, owned,
/// fitted.
pub type Swatch = (u32, [u8; 3], i32, bool, bool);

/// UI_Paint (screen 0x56C1C0 → 0x56AEC0): the paint slots across
/// OPTION_MASTER; Select enters a slot, whose paints of the current
/// finish (0x5449D0 with the finish as the brand, 0x548640) colour
/// PAINT_SWATCH_1…60 (20 a row; the rest hide) with U2_Paint_HighLight on
/// the selected one at its position + (152, 165) (screen +0x180 / +0x184).
pub struct PaintShop {
    pub slot: usize,
    pub finish: u32,
    pub swatches: Vec<Swatch>,
    /// In a slot: the swatch selected.
    pub selected: Option<usize>,
    pub opened: f32,
    /// Mode 1, a vinyl's remap colours (0x53BD60 with 0x8389A0 = 1): the
    /// layer and the vinyl's NUMREMAPCOLOURS; the strip is PAINT_ICON_1 …
    /// one a colour, each "Choose Vinyl Colors" (0xC034CE3D), over the
    /// layer's colour slots (0x50F440).
    pub vinyl: Option<(u8, u8)>,
    /// The graphics setup's cost in a career (Total Cost, 0x563C70 →
    /// 0x5453C0); hidden outside one (0x545470).
    pub total: Option<i32>,
    /// The swatch panel's last fade (group 0xDB285767, INIT → its
    /// zero-alpha script): "FadeIn" on entering a slot (0x55CD45), "FadeOut"
    /// on leaving it (0x55CF35); true for in, and when (ms).
    pub swatch_fade: Option<(bool, f32)>,
    /// The slot strip's arrows' blink (an IconScrollerMenu's 0x52FA80,
    // prov: inferred frontend.screens.paint_finish
    /// **inferred** from the class as its arrows are, [`ArrowBlink`]).
    pub arrows: ArrowBlink,
}

/// UI_Paint's swatch panel (PAINT_SWATCH_n, the finish selector, the
/// highlight).
const SWATCH_PANEL: u32 = fe_ids::SWATCH_PANEL;

pub enum PaintKey {
    None,
    Leave,
    /// Select on a slot (0x406415E3): its swatches when it can be painted.
    Enter,
    /// The slot or the finish changed: refill the swatches.
    Refill,
    Install(usize),
}

impl PaintShop {
    /// The help bar's mask: in a slot's swatches 0x281 (Cancel Changes,
    /// Install paint, Quit Game; 0x55CE78), else the screen's.
    pub fn help_mask(&self) -> Option<u32> {
        // Both modes enter a slot through 0x55CC60 (the vinyl colours at
        // 0x563CB1), which sets 0x281; under Wine the vinyl colours show
        // Cancel Changes / Install paint / Quit Game.
        if self.selected.is_some() {
            return Some(0x281);
        }
        help_mask("UI_Paint")
    }

    /// The swatch the highlight is on: the selected one while the finish
    /// lists paints; none with no paints (0x55D28F hides U2_Paint_HighLight
    /// 0x9BA6D658 and shows the lock).
    pub fn highlighted(&self) -> Option<usize> {
        self.selected.filter(|&i| i < self.swatches.len())
    }

    /// The strip's slots: slot, icon, label.
    pub fn slots(&self) -> Vec<(u8, u32, u32)> {
        match self.vinyl {
            Some((layer, n)) => (1..=n.clamp(1, 4))
                .map(|i| {
                    (
                        crate::frontend::graphics::vinyl_colour_slot(layer, i),
                        fe_ids::PAINT_ICON_1 + u32::from(i - 1),
                        fe_ids::TXT_CHOOSE_VINYL_COLORS,
                    )
                })
                .collect(),
            None => PAINT_SLOTS.to_vec(),
        }
    }

    pub fn key(&mut self, key: Key) -> PaintKey {
        let n = self.swatches.len();
        let slots = self.slots();
        match (self.selected, key) {
            (None, Key::Left) if self.slot > 0 => {
                self.slot -= 1;
                self.finish = paint_finish(slots[self.slot].0);
                return PaintKey::Refill;
            }
            (None, Key::Right) if self.slot + 1 < slots.len() => {
                self.slot += 1;
                self.finish = paint_finish(slots[self.slot].0);
                return PaintKey::Refill;
            }
            (None, Key::Accept) => return PaintKey::Enter,
            (None, Key::Back) => return PaintKey::Leave,
            (Some(i), Key::Left) if i > 0 => self.selected = Some(i - 1),
            (Some(i), Key::Right) if i + 1 < n => self.selected = Some(i + 1),
            (Some(i), Key::Up) if i >= 20 => self.selected = Some(i - 20),
            (Some(i), Key::Down) if i + 20 < n => self.selected = Some(i + 20),
            // Accept with no paints to list (a finish whose paints are all
            // locked: the lock shows) does nothing (0x563707: the swatch
            // list +0x158 empty → return, no sound, no question).
            (Some(i), Key::Accept) if i < n => return PaintKey::Install(i),
            // Back from the swatches: the strip, except for a vinyl of
            // one colour, which leaves (0x5638C7).
            (Some(_), Key::Back) if matches!(self.vinyl, Some((_, n)) if n < 2) => return PaintKey::Leave,
            (Some(_), Key::Back) => self.selected = None,
            (_, Key::LTrigger | Key::RTrigger) if matches!(slots[self.slot].0, 0x3F | 0x45 | 0x4B | 0x4C) => {
                let up = key == Key::RTrigger;
                self.finish = match (self.finish, up) {
                    (GLOSS, true) => METAL,
                    (METAL, true) => PEARL,
                    (PEARL, true) => GLOSS,
                    (GLOSS, false) => PEARL,
                    (PEARL, false) => METAL,
                    _ => GLOSS,
                };
                self.selected = self.selected.map(|_| 0);
                return PaintKey::Refill;
            }
            _ => {}
        }
        PaintKey::None
    }

    pub fn quads(&self, assets: &Assets, profile: Option<&Profile>, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_Paint") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Install");
        let slots = self.slots();
        let (_, _, label) = slots[self.slot.min(slots.len() - 1)];
        state.text.insert(ui_main::OPTION_TITLE, assets.strings.get(label).unwrap_or("").to_string());
        if let Some(p) = profile {
            state.text.insert(fe_ids::UI_MAIN_CASH, crate::frontend::grouped(p.cash as i64));
        }
        state.hidden.insert(fe_ids::PLAYER_NAME);
        for i in 0..60 {
            let h = fe_hash(&format!("PAINT_SWATCH_{}", i + 1));
            match self.swatches.get(i) {
                Some((_, rgb, ..)) => {
                    state.color.insert(h, [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0]);
                }
                None => {
                    state.hidden.insert(h);
                }
            }
        }
        const HIGHLIGHT: u32 = fe_ids::HIGHLIGHT;
        match self.highlighted().and_then(|i| package.object(fe_hash(&format!("PAINT_SWATCH_{}", i + 1)))) {
            Some(o) => {
                let p = o.attributes.position();
                state.position.insert(HIGHLIGHT, Vec2::new(p.x + 152.0, p.y + 165.0));
            }
            None => {
                state.hidden.insert(HIGHLIGHT);
            }
        }
        // The finish between the trigger arrows (Paint_text, 0x5484F0):
        // the label of the finish's name + "_PAINT" (0x548591: 0x43DB80
        // continues the finish's hash +0x174 with "_PAINT", 0x79C324):
        // GLOSS_PAINT "Gloss", METAL_PAINT "Metallic", PEARL_PAINT
        // "Pearlescent", the rims' RIM_PAINT "Powder Coat" (Wine, the
        // finished-career save's 350Z), "" for a finish without a label.
        let finish = assets.strings.get(crate::formats::hash::bin_hash_from(self.finish, "_PAINT"));
        state.text.insert(fe_ids::PAINT_TEXT, finish.unwrap_or("").to_string());
        // No paint slot is locked: LOCK_MASTER stays hidden (INIT → HIDE).
        state.hidden.insert(parts_browser::LOCK_MASTER);
        // The swatches' lock: SHOW with no paints to list, else HIDE
        // (0x548640).
        if self.swatches.is_empty() {
            state.shown.insert(fe_ids::OBJ_23A43844);
        } else {
            state.hidden.insert(fe_ids::OBJ_23A43844);
        }
        state.hidden.insert(ui_main::SLOT);
        state.hidden.insert(fe_ids::OPTION_TEMPLATE);
        total_cost(&mut state, self.total);
        // The highlighted swatch's price (CASH, "%$d" of 0x514280; 0x55CC60
        // / 0x55D2E0): in a career's paint swatches only.
        const CASH: u32 = fe_ids::PAINT_CASH;
        match (self.total, self.vinyl, self.selected.and_then(|i| self.swatches.get(i))) {
            (Some(_), None, Some((_, _, price, ..))) => {
                state.text.insert(CASH, crate::frontend::grouped(i64::from(*price)));
            }
            _ => {
                state.hidden.insert(CASH);
            }
        }
        // The finish's trigger hints show on the finish slots (0x5484F0:
        // 0x3F, 0x45, 0x4B, 0x4C); the vinyl colours hide the finish
        // selector (group 0x031A9C0A).
        if !matches!(slots[self.slot.min(slots.len() - 1)].0, 0x3F | 0x45 | 0x4B | 0x4C) {
            state.hidden.insert(0x9DE5_BA07);
            state.hidden.insert(0x7A1F_F30D);
        }
        if self.vinyl.is_some() {
            state.hidden.insert(0x031A_9C0A);
        }
        // The finish selector's arrows (top level, UI_Paint +0x150 / +0x154):
        // the ctor 0x56AEC0 hides both (0x56B04E / 0x56B05A); entering a
        // slot (0x55CC60) shows them (0x55CD95 / 0x55CDA8) only for slot
        // 0x3F, 0x45, 0x4B or 0x4C (byte table 0x55CE90); leaving the slot
        // (0x55CF0E) hides them again. So the vinyl colours (slots 0x4D…)
        // and every other slot never show them.
        if !finish_arrows_shown(self.selected.map(|_| slots[self.slot.min(slots.len() - 1)].0)) {
            state.hidden.insert(fe_ids::PAINT_FINISH_ARROW_1);
            state.hidden.insert(fe_ids::PAINT_FINISH_ARROW_2);
        }
        // Entering a slot (0x55CC60) hides the strip's highlight frame
        // 0xDF422345 (0x55CD2A), leaving it shows it again (0x55CF0E).
        if self.selected.is_some() {
            state.hidden.insert(parts_browser::CATEGORY_ONLY);
        }
        // The swatch panel stays at INIT's zero alpha on the slots (under
        // Wine: no swatches nor finish bar until a slot is entered); a
        // shop opened in a slot (the vinyl colours, a refill) shows it.
        match self.swatch_fade {
            Some((fade_in, at)) => {
                state.run(SWATCH_PANEL, if fade_in { "FadeIn" } else { "FadeOut" }, at - self.opened)
            }
            // The vinyl colours too while no colour slot is entered (Wine,
            // the finished-career save's two-colour Tear Set vinyl: no swatches on
            // "Choose Vinyl Colors" until colour 1 is entered).
            None if self.selected.is_none() => {
                state.hidden.insert(SWATCH_PANEL);
            }
            None => {}
        }
        // The slot strip is an IconScrollerMenu as the parts browser's
        // (0x538090 on its "ScrollBar", the arrows by its ends, 0x52FA80;
        // prov: inferred frontend.screens.quads-2
        // **inferred** for UI_Paint from the class): under Wine the handle
        // sits at the start and the left arrow is off on Base Paint.
        let n = slots.len();
        scroll_bar_at(&mut state, package, "ScrollBar", true, 1, n, if n == 0 { 0 } else { self.slot + 1 });
        if self.slot == 0 {
            state.hidden.insert(fe_ids::POST_RACE_STATS_LEFT);
        }
        if self.slot + 1 >= n {
            state.hidden.insert(fe_ids::POST_RACE_STATS_RIGHT);
        }
        if self.selected.is_none() {
            self.arrows.apply(&mut state, SCROLLBAR_ARROWS, self.opened);
        }
        let mut quads = layout(assets, package, &state, now - self.opened);
        let icons: Vec<u32> = slots.iter().map(|s| s.1).collect();
        quads.extend(scroller_quads(package, ui_main::SLOT, fe_ids::OPTION_TEMPLATE, &icons, self.slot, true, 40.0, 480.0));
        quads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shop(swatches: Vec<Swatch>) -> PaintShop {
        PaintShop {
            slot: 0,
            finish: METAL,
            swatches,
            selected: Some(0),
            opened: 0.0,
            vinyl: None,
            total: Some(0),
            swatch_fade: None,
            arrows: Default::default(),
        }
    }

    /// A finish with no paints to list (all locked): Accept asks nothing
    /// and no swatch is highlighted (0x563707, 0x55D28F).
    #[test]
    fn accept_with_no_paints_does_nothing() {
        let mut p = shop(Vec::new());
        assert!(matches!(p.key(Key::Accept), PaintKey::None));
        assert_eq!(p.highlighted(), None);
        let mut p = shop(vec![(1, [0; 3], 0, false, false)]);
        assert!(matches!(p.key(Key::Accept), PaintKey::Install(0)));
        assert_eq!(p.highlighted(), Some(0));
    }
}
