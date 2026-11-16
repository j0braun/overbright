//! UI_ChooseCustomHUD (0x799C20; descriptor 0x7F87A0 {0x554DB0 →
//! constructor 0x553780}, vtable 0x79BEA8, handler 0x5644E0): the gauge
//! preview and colour picker the Custom Gauges overlay (UI_CustomHUDOverlay,
//! constructor 0x56C220, handler 0x563D00) embeds (0x555D10 mode 4).
//!
//! The screen's step is the global 0x838560: 1 while the overlay lists the
//! gauges (the preview alone), 2 / 3 / 4 for the backing, dial and needle
//! colours (the picker shown), 0 when the needle's is accepted (the
//! overlay's purchase). Each step keeps a 0x14-byte entry at +0x64 (+0x6C
//! the selected list node, +0x70 the count, +0x74 the 1-based index; the
//! constructor sets every index to 1). The picks go to 0x839C24 / 28 / 2C
//! (the colour parts) and 0x839BF4… (their colours), which the overlay
//! fits to the car in slots 0xA4 / 0xA6 / 0xA5 on purchase (0x563DD2…).

use glam::Vec2;

use super::screens::{Key, footer};
use super::{Assets, Quad, ScreenState, layout};
use crate::formats::hash::fe_hash;

pub const PACKAGE: &str = "UI_ChooseCustomHUD";

/// The picker's group (0xDB285767: the swatches, their bars and the
/// title): its "Hide" / "Show" scripts by 0x53C570 / 0x53C4B0.
const PICKER: u32 = 0xDB28_5767;
/// "Non_Interactive_Group" (0xED03D1D3): the gauge preview, hidden by the
/// constructor and shown once the custom textures are in (0x549010 /
/// 0x5644E0 on 0xC98356BA).
const PREVIEW: u32 = 0xED03_D1D3;
/// U2_Paint_HighLight (+0x60): over the selected swatch at its position
/// plus (21, 130) (+0xA0 / +0xA4, 0x548E50 → 0x51D4E0).
const HIGHLIGHT: u32 = 0x9BA6_D658;
const HIGHLIGHT_OFFSET: Vec2 = Vec2::new(21.0, 130.0);
/// The arrows (+0xAC left, +0xB0 right): "Blink" on a move that way.
const ARROWS: [u32; 2] = [0xD1CD_A594, 0x9345_E4A7];
/// The step's title (0x5504E0 sets 0xDC0EBA9C).
const TITLE: u32 = 0xDC0E_BA9C;
/// "Redline", hidden by the constructor (0x4954C0).
const REDLINE: u32 = 0x61D3_0442;
const SPEED_UNITS: u32 = 0xC338_3B63;
const MPH: u32 = 0x8569_AB44;
const KMH: u32 = 0x8569_A25F;
/// The swatches 0x549290 fills (PAINT_SWATCH_1…14; the package has 12).
const SWATCHES: usize = 14;

/// The preview's images and their textures (0x549010, `%02d` of the
/// gauge's HUDINDEX); the dial lines by the car's top rpm.
const TURBOFILL: u32 = 0x6D5F_E871;
const TURBODIAL: u32 = 0x6D5E_CE44;
const TURBOLINES: u32 = 0x19C9_9185;
const TACH_FILL: u32 = 0x05D1_9F25;
const RPMDIAL: u32 = 0xF025_0DAC;
const LINES: u32 = 0x1D90_51AA;

/// A colour step (0x838560 = 2 + index): the slot whose parts make its
/// swatches (0x549290: 0xA4 for the first, 0xA6 for the other two), the
/// slot its pick is fitted to (0x563D00: 0x839C24 → 0xA4, 0x839C28 →
/// 0xA6, 0x839C2C → 0xA5), the two preview images it tints (0x53C630) and
/// its title (0x5504E0).
pub struct Step {
    pub palette: u8,
    pub slot: u8,
    pub objects: [u32; 2],
    pub title: u32,
}

pub const STEPS: [Step; 3] = [
    // "Choose Backing Color"
    Step { palette: 0xA4, slot: 0xA4, objects: [TACH_FILL, TURBOFILL], title: 0x3A9C_CF4E },
    // "Choose Dial Color"
    Step { palette: 0xA6, slot: 0xA6, objects: [LINES, TURBOLINES], title: 0xDED7_CA79 },
    // "Choose Needle Color"
    Step { palette: 0xA6, slot: 0xA5, objects: [RPMDIAL, TURBODIAL], title: 0x6A0B_254C },
];

/// A colour part as a swatch: its hash and RED / GREEN / BLUE.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swatch {
    pub hash: u32,
    pub rgb: [u8; 3],
}

/// What a key did.
#[derive(Debug, PartialEq)]
pub enum ChooseKey {
    None,
    /// Back from the backing colour: the gauge list again (step 1).
    Back,
    /// The needle colour accepted (step 0): the overlay buys the gauge.
    Done,
}

#[derive(Clone)]
pub struct ChooseCustomHud {
    /// The gauge list to return to (the specialties category and part).
    pub category: usize,
    pub part: usize,
    /// The gauge's HUDINDEX (0x839BF0) and the car's top rpm.
    pub hud_index: u32,
    pub max_rpm: f32,
    pub metric: bool,
    /// 0..3: the steps 2, 3, 4.
    pub step: usize,
    pub palettes: [Vec<Swatch>; 3],
    /// Each step's 1-based index (+0x74, +0x88, +0x9C).
    pub index: [usize; 3],
    /// The picks (0x839C24, 0x839C28, 0x839C2C): the fitted parts when the
    /// overlay opened (0x50F650), then each move's (0x53C630).
    pub chosen: [Option<u32>; 3],
    /// The colours the preview's images got (0x53C630; white again when a
    /// step is backed out of).
    pub preview: [Option<[u8; 3]>; 3],
    /// The last move: its time (ms) and the swatch it left.
    pub moved: Option<(f32, usize, bool)>,
    pub opened: f32,
    /// The picker hidden (**traced**): the needle's Accept (4 → 0,
    /// 0x56455C) sets the package colours 0x537960 UI_ICEMAIN 0, the
    /// overlay 0xFF, the chooser 0, so under the purchase's dialogs only
    /// UI_CustomHUDOverlay shows (dimmed); the dialogs' No / OK
    /// (0xB4623F67 → 0x5640BD, step 4) shows the picker again.
    pub hidden: bool,
}

impl ChooseCustomHud {
    /// 0xFD7C68D8 (the overlay's accept on a custom gauge): the picker on
    /// the backing colour.
    pub fn new(category: usize, part: usize, hud_index: u32, max_rpm: f32, metric: bool, palettes: [Vec<Swatch>; 3], fitted: [Option<u32>; 3], now: f32) -> Self {
        ChooseCustomHud { category, part, hud_index, max_rpm, metric, step: 0, palettes, index: [1; 3], chosen: fitted, preview: [None; 3], moved: None, opened: now, hidden: false }
    }

    /// Lists Open on Installed (clone deviation; native 0x549290 drops the
    /// fitted colour part and opens on swatch 1): each step opens on its
    /// fitted colour's swatch, shown in the preview, when the palette
    /// holds it.
    pub fn open_on_fitted(&mut self) {
        for k in 0..3 {
            let Some(hash) = self.chosen[k] else { continue };
            if let Some(i) = self.palettes[k].iter().position(|s| s.hash == hash) {
                self.index[k] = i + 1;
                self.preview[k] = Some(self.palettes[k][i].rgb);
            }
        }
    }

    /// The selected swatch of the step (0x549290: the stored index while it
    /// is within the list, else the first).
    pub fn current(&self) -> usize {
        let i = self.index[self.step];
        if (1..=self.palettes[self.step].len()).contains(&i) { i } else { 1 }
    }

    pub fn key(&mut self, key: Key, now: f32) -> ChooseKey {
        match key {
            // 0x9120409E / 0xB5971BF1 → 0x548E50(∓1).
            Key::Left => self.step_by(-1, now),
            Key::Right => self.step_by(1, now),
            // 0x406415E3: 2 → 3 → 4 → 0 (the jump table 0x5647DC).
            Key::Accept => {
                if self.step == 2 {
                    return ChooseKey::Done;
                }
                self.step += 1;
                self.moved = None;
                self.opened = now;
            }
            // 0x911AB364: the step's preview white (0x53C630 with
            // 0xFFFFFFFF, not kept), its index 1, the step before (from
            // the backing colour, the gauge list).
            Key::Back => {
                self.preview[self.step] = Some([0xFF; 3]);
                self.index[self.step] = 1;
                self.moved = None;
                if self.step == 0 {
                    return ChooseKey::Back;
                }
                self.step -= 1;
                self.opened = now;
            }
            _ => {}
        }
        ChooseKey::None
    }

    /// 0x548E50: the index moves by `by` within 1..count, the arrow that
    /// way blinks, and the swatch's part and colour become the step's pick.
    fn step_by(&mut self, by: i32, now: f32) {
        let count = self.palettes[self.step].len();
        if count == 0 {
            return;
        }
        let old = self.current();
        let new = (old as i32 + by).clamp(1, count as i32) as usize;
        self.index[self.step] = new;
        let s = self.palettes[self.step][new - 1];
        self.chosen[self.step] = Some(s.hash);
        self.preview[self.step] = Some(s.rgb);
        self.moved = Some((now, old, new < old));
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        if self.hidden {
            return Vec::new();
        }
        let Some(package) = assets.package(PACKAGE) else { return Vec::new() };
        let mut s = preview_state(assets, package, self.hud_index, self.max_rpm, self.metric);
        // 0x53C4B0: the picker group's "Show", the highlight and arrows.
        s.run(PICKER, "SHOW", 0.0);
        s.hidden.remove(&HIGHLIGHT);
        for a in ARROWS {
            s.hidden.remove(&a);
        }
        let step = &STEPS[self.step];
        s.text.insert(TITLE, assets.strings.get(step.title).unwrap_or_default().to_string());
        // 0x549290: a swatch per part in the step's palette (its RED /
        // GREEN / BLUE, alpha 0xFF), the rest hidden.
        for n in 1..=SWATCHES {
            let h = fe_hash(&format!("PAINT_SWATCH_{n}"));
            match self.palettes[self.step].get(n - 1) {
                Some(sw) => {
                    s.color.insert(h, [sw.rgb[0] as f32 / 255.0, sw.rgb[1] as f32 / 255.0, sw.rgb[2] as f32 / 255.0, 1.0]);
                }
                None => {
                    s.hidden.insert(h);
                }
            }
        }
        let current = self.current();
        let swatch = fe_hash(&format!("PAINT_SWATCH_{current}"));
        if let Some(o) = package.object(swatch) {
            let p = o.attributes.position();
            s.position.insert(HIGHLIGHT, Vec2::new(p.x, p.y) + HIGHLIGHT_OFFSET);
        }
        if let Some((at, old, left)) = self.moved {
            let t = at - self.opened;
            s.run(fe_hash(&format!("PAINT_SWATCH_{old}")), "UNHIGHLIGHT", t);
            s.run(swatch, "HIGHLIGHT", t);
            if old != current {
                s.run(ARROWS[usize::from(!left)], "BLINK", t);
            }
        }
        // 0x53C630: the preview's images in each step's colour, their own
        // alpha kept.
        for (k, st) in STEPS.iter().enumerate() {
            if let Some(rgb) = self.preview[k] {
                for &h in &st.objects {
                    let a = package.object(h).map_or(1.0, |o| o.attributes.color()[3]);
                    s.color.insert(h, [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, a]);
                }
            }
        }
        layout(assets, package, &s, now - self.opened)
    }
}

/// The preview's state (step 1, 0x5644E0 on 0xC98356BA → 0x549010): the
/// preview group shown with the gauge's textures, the picker, highlight
/// and arrows hidden (0x53C570), Redline hidden, the speed's units.
fn preview_state(assets: &Assets, package: &super::feng::Package, hud_index: u32, max_rpm: f32, metric: bool) -> ScreenState {
    let mut s = ScreenState::default();
    footer(&mut s, package, "Install");
    s.run(PREVIEW, "SHOW", 0.0);
    s.hidden.insert(HIGHLIGHT);
    s.hidden.extend(ARROWS);
    s.hidden.insert(REDLINE);
    // 0x549010: dial lines by the car's top rpm (+0x1F8): 10500 from
    // 10000, 9500 from 9000, 8500 from 8000, else 7500.
    let lines = if max_rpm >= 10_000.0 {
        10_500
    } else if max_rpm >= 9000.0 {
        9500
    } else if max_rpm >= 8000.0 {
        8500
    } else {
        7500
    };
    let i = hud_index;
    for (object, texture) in [
        (TURBOFILL, format!("3RDPERSON_TURBOFILL_CUSTOM_{i:02}")),
        (TURBODIAL, format!("3RDPERSON_TURBODIAL_CUSTOM_{i:02}")),
        (TURBOLINES, format!("3RDPERSON_TURBOLINES_CUSTOM_{i:02}")),
        (TACH_FILL, format!("TACH_FILL_CUSTOM_{i:02}")),
        (RPMDIAL, format!("3RDPERSON_RPMDIAL_CUSTOM_{i:02}")),
        (LINES, format!("3RDPERSON_{lines}LINES_CUSTOM_{i:02}")),
    ] {
        if let Some(t) = assets.texture_for(&texture) {
            s.image.insert(object, t);
        }
    }
    s.text.insert(SPEED_UNITS, assets.strings.get(if metric { KMH } else { MPH }).unwrap_or_default().to_string());
    s
}

/// The screen while the overlay lists the gauges (step 1): the preview of
/// the selected gauge alone.
pub fn preview_quads(assets: &Assets, hud_index: u32, max_rpm: f32, metric: bool, now: f32) -> Vec<Quad> {
    let Some(package) = assets.package(PACKAGE) else { return Vec::new() };
    let mut s = preview_state(assets, package, hud_index, max_rpm, metric);
    s.run(PICKER, "HIDE", 0.0);
    layout(assets, package, &s, now)
}

/// The race HUD's colours from the fitted colour parts (0x4F5100): slot
/// 0xA4 tints tach_fill and 3rdperson_Turbofill, 0xA5 3rdPersonNeedle and
/// 3rdperson_TurboDial, 0xA6 3rdPersonTachRPMNumbers and
/// 3rdperson_Turbolines; with no part in a slot, white (0xFF each).
pub const HUD_TINTS: [(usize, [&str; 2]); 3] = [
    (0, ["tach_fill", "3rdperson_Turbofill"]),
    (1, ["3rdPersonNeedle", "3rdperson_TurboDial"]),
    (2, ["3rdPersonTachRPMNumbers", "3rdperson_Turbolines"]),
];

/// The race HUD's colour for a slot's part (BLUE / GREEN / RED, alpha
/// 0xFF; 0x4F5100), as RGBA 0..1; white with none.
pub fn hud_color(rgb: Option<[u8; 3]>) -> [f32; 4] {
    let [r, g, b] = rgb.unwrap_or([0xFF; 3]);
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chooser() -> ChooseCustomHud {
        let pal = |n: u32| (0..n).map(|i| Swatch { hash: 100 + i, rgb: [i as u8, 0, 0] }).collect::<Vec<_>>();
        ChooseCustomHud::new(0, 1, 1, 9000.0, false, [pal(12), pal(12), pal(12)], [Some(7), Some(8), Some(9)], 0.0)
    }

    /// 0x549290 fetches the fitted colour part and drops it (0x54938B): every
    /// step opens on its first swatch (index 1) even with a fitted colour in
    /// the palette. Wine (the finished-career save, GRIFFIN with a GREEN backing fitted): the
    /// backing step reopened on WHITE.
    #[test]
    fn steps_open_on_the_first_swatch_not_the_fitted() {
        let pal = |n: u32| (0..n).map(|i| Swatch { hash: 100 + i, rgb: [i as u8, 0, 0] }).collect::<Vec<_>>();
        let mut c = ChooseCustomHud::new(0, 2, 2, 9000.0, false, [pal(12), pal(12), pal(12)], [Some(103), Some(105), Some(107)], 0.0);
        for step in 0..3 {
            assert_eq!((c.step, c.current()), (step, 1));
            c.key(Key::Accept, step as f32);
        }
    }

    /// Lists Open on Installed: each step on its fitted colour's swatch; a
    /// colour outside the palette (or none) keeps swatch 1.
    #[test]
    fn open_on_fitted_puts_each_step_on_its_colour() {
        let pal = |n: u32| (0..n).map(|i| Swatch { hash: 100 + i, rgb: [i as u8, 0, 0] }).collect::<Vec<_>>();
        let mut c = ChooseCustomHud::new(0, 2, 2, 9000.0, false, [pal(12), pal(12), pal(12)], [Some(103), Some(999), None], 0.0);
        c.open_on_fitted();
        assert_eq!(c.index, [4, 1, 1]);
        assert_eq!(c.preview, [Some([3, 0, 0]), None, None]);
        assert_eq!(c.chosen, [Some(103), Some(999), None]);
    }

    #[test]
    fn steps_and_picks() {
        let mut c = chooser();
        assert_eq!(c.current(), 1);
        // Accepting without a move keeps the fitted pick (0x839C24 is
        // only written by 0x53C630 on a move).
        assert_eq!(c.key(Key::Accept, 1.0), ChooseKey::None);
        assert_eq!(c.step, 1);
        c.key(Key::Right, 2.0);
        c.key(Key::Right, 3.0);
        assert_eq!((c.index[1], c.chosen[1], c.preview[1]), (3, Some(102), Some([2, 0, 0])));
        // Left at 1 stays at 1 (0x548E50 clamps).
        c.key(Key::Left, 4.0);
        c.key(Key::Left, 4.0);
        c.key(Key::Left, 4.0);
        assert_eq!((c.index[1], c.chosen[1]), (1, Some(100)));
        // Back: the step's preview white and index 1, the pick kept.
        c.key(Key::Right, 5.0);
        c.key(Key::Back, 6.0);
        assert_eq!((c.step, c.index[1], c.chosen[1], c.preview[1]), (0, 1, Some(101), Some([0xFF; 3])));
        c.key(Key::Accept, 7.0);
        c.key(Key::Accept, 8.0);
        assert_eq!(c.step, 2);
        assert_eq!(c.key(Key::Accept, 9.0), ChooseKey::Done);
        assert_eq!(c.chosen, [Some(7), Some(101), Some(9)]);
        let mut c = chooser();
        assert_eq!(c.key(Key::Back, 1.0), ChooseKey::Back);
        // Right past the end stops at the count.
        for _ in 0..20 {
            c.key(Key::Right, 2.0);
        }
        assert_eq!(c.current(), 12);
    }
}
