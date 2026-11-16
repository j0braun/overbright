use crate::frontend::fe_ids;
use crate::formats::hash::fe_hash;
use crate::frontend::career::stats::*;
use crate::frontend::screens::{Key, footer, scroll_bar_named};
use crate::frontend::widgets::rating::{rating_stars, slider};
use crate::frontend::{Assets, Quad, ScreenState, layout};

/// The rows UI_Status_Career shows at once (its seven row objects).
pub const VISIBLE: usize = 7;

/// UI_Status_Master and its page.
#[derive(Clone, Debug)]
pub struct StatusScreen {
    /// The page shown (+0x104, an index into [`PAGES`]).
    pub page: usize,
    /// The tabs (0x4CC5C0): one before stage 6, else three.
    pub pages: usize,
    /// UI_Status_Career's first row shown.
    pub top: usize,
    pub opened: f32,
    /// The player's car's visual rating (0x5234D0) for the stars, when
    /// there is a player (0x8900AC, 0x4E1526).
    pub rating: Option<f32>,
}

impl StatusScreen {
    pub fn new(stats: &CareerStats, now: f32) -> StatusScreen {
        StatusScreen { page: 0, pages: if stats.stage < 6 { 3 } else { 1 }, top: 0, opened: now, rating: None }
    }

    /// Left / Right step the page (0x4B1E60: not past the ends); Up /
    /// Down scroll UI_Status_Career's list. Returns whether the screen
    /// closes (Accept or Back: 0x4B1F20 plays out and the game resumes).
    pub fn key(&mut self, key: Key, rows: usize) -> bool {
        match key {
            Key::Left if self.page > 0 => self.page -= 1,
            Key::Right if self.page + 1 < self.pages => self.page += 1,
            Key::Up if self.page == 0 && self.top > 0 => self.top -= 1,
            Key::Down if self.page == 0 && self.top + VISIBLE < rows => self.top += 1,
            Key::Accept | Key::Back => return true,
            _ => {}
        }
        false
    }

    pub fn quads(&self, assets: &Assets, stats: &CareerStats, rows: &[Row], now: f32) -> Vec<Quad> {
        let t = now - self.opened;
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let mut quads = Vec::new();
        if let Some(master) = assets.package("UI_Status_Master") {
            let mut state = ScreenState::default();
            footer(&mut state, master, "Continue");
            let tab = match self.page {
                0 => label(0xA353_2050),
                1 if stats.stage == 0 => label(0xD56A_FE23),
                1 => {
                    let region = label(0xC08A_F0B2u32.wrapping_add(u32::from(stats.stage)));
                    label(0x94D3_715B).replacen("%d", &stats.stage.to_string(), 1).replacen("%s", &region, 1)
                }
                _ => label(0x74E4_99B0),
            };
            state.text.insert(0x3223_A234, tab);
            // 0x4B1DF0: the left arrow hides on the first page, the right
            // on the last.
            if self.page == 0 {
                state.hidden.insert(fe_ids::POST_RACE_STATS_LEFT);
            }
            if self.page + 1 >= self.pages {
                state.hidden.insert(fe_ids::POST_RACE_STATS_RIGHT);
            }
            // The page scroller (0x542CB0 "Scroll_Area" / "ScrollBar" in
            // the master's constructor 0x4E1A30): the handle at the page
            // prov: inferred frontend.status.quads
            // (0x538090 → 0x52E330; **inferred** the arguments 1, the
            // pages, the page from 1 — under Wine (menus112 s0–s2) the
            // dot sits left, centre and right on the three pages).
            crate::frontend::screens::scroll_bar_at(
                &mut state,
                master,
                "ScrollBar",
                true,
                1,
                self.pages,
                self.page + 1,
            );
            // Then 0x4B1DF0 (after each step) hides the arrows at the ends.
            for (end, arrow) in [(self.page == 0, fe_ids::POST_RACE_STATS_LEFT), (self.page + 1 >= self.pages, fe_ids::POST_RACE_STATS_RIGHT)] {
                if end {
                    state.shown.remove(&arrow);
                    state.hidden.insert(arrow);
                }
            }
            quads.extend(layout(assets, master, &state, t));
        }
        let Some(page) = assets.package(PAGES[self.page.min(2)]) else { return quads };
        let mut state = ScreenState::default();
        match self.page {
            0 => {
                career_page(&mut state, page, stats, rows, self.top);
                if let Some(r) = self.rating {
                    rating_stars(&mut state, page, r);
                }
            }
            1 => region_page(&mut state, page, stats, &label),
            _ => dvd_page(&mut state, page, stats, &label),
        }
        quads.extend(layout(assets, page, &state, t));
        quads
    }
}

/// UI_Status_Career (0x4EE8B0 → 0x4E13C0): seven row objects (label
/// 0x566BF789 + k, value 0xA148528A + k, backing 0x28887216 + k, the
/// sliders Slider_*_<k + 1>, hidden by 0x512240(0) unless the row is a
/// slider row); the arrows 0x640F0F22 / 23 hide at the list's ends
/// (0x496F50).
fn career_page(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    _stats: &CareerStats,
    rows: &[Row],
    top: usize,
) {
    for k in 0..VISIBLE {
        let n = k + 1;
        let parts = [
            format!("Slider_base_{n}"),
            format!("Slider_slider_{n}"),
            format!("Slider_handle_{n}"),
            format!("Slider_Beg_{n}"),
            format!("Slider_End_{n}"),
        ]
        .map(|s| fe_hash(&s));
        let (name, value) = (0x566B_F789 + k as u32, 0xA148_528A + k as u32);
        for h in parts {
            state.hidden.insert(h);
        }
        // The row fill 0x4B1780: a heading (a cell with style bit 2, set
        // by 0x4CC4A0's 0x4AEA60(text, 0, 2)) colours the label
        // 0xFFCAF48A and the backing its own RGB at alpha 0xDC; any
        // other row the label 0xFFFFFFFF and the backing at alpha 0x7D
        // (0x51D800; seen under Wine, menus112 s0).
        let backing = 0x2888_7216 + k as u32;
        if let Some(row) = rows.get(top + k) {
            let heading = matches!(row, Row::Header(_));
            state.color.insert(name, crate::frontend::argb(if heading { 0xFFCA_F48A } else { 0xFFFF_FFFF }));
            if let Some(o) = package.object(backing) {
                let mut c = o.attributes.color();
                c[3] = if heading { 0xDC as f32 / 255.0 } else { 0x7D as f32 / 255.0 };
                state.color.insert(backing, c);
            }
        }
        match rows.get(top + k) {
            None => {
                state.hidden.extend([name, value, backing]);
            }
            Some(Row::Header(text)) => {
                state.text.insert(name, text.clone());
                state.text.insert(value, String::new());
            }
            Some(Row::Value(text, v)) => {
                state.text.insert(name, text.clone());
                state.text.insert(value, v.clone());
            }
            Some(Row::Slider(text, f)) => {
                state.text.insert(name, text.clone());
                // 0x4B17FF: a slider row's value reads "%$d %%"
                // (0x790F54) of trunc(fraction × 100) (0x784258, _ftol).
                state.text.insert(value, slider_text(*f));
                for h in parts {
                    state.hidden.remove(&h);
                }
                slider(state, package, parts, *f);
                // 0x4B181C's 0x512240(1) shows the slider's +0xC…+0x18
                // (base, knob, an unset slot, handle; 0x540C20's
                // descriptor from 0x4EE9DE), not +0x1C / +0x20 (Beg, End),
                // which the constructor's 0x512240(0) hid: no end caps
                // (seen under Wine, menus112 s0).
                state.shown.remove(&parts[3]);
                state.shown.remove(&parts[4]);
                state.hidden.extend([parts[3], parts[4]]);
            }
        }
    }
    if top + VISIBLE >= rows.len() {
        state.hidden.insert(fe_ids::OBJ_640F0F23);
    }
    if top == 0 {
        state.hidden.insert(fe_ids::SIDESCROLLBAR_ARROW_1);
    }
    scroll_bar_named(state, package, "SideScrollBar", VISIBLE, rows.len(), top);
}

/// A slider row's value text (0x4B1835): the fraction × 100 in the x87's
/// precision, truncated (0x75BED4), as "%$d %%".
pub fn slider_text(f: f32) -> String {
    format!("{} %", crate::frontend::grouped((f64::from(f) * 100.0).trunc() as i64))
}

/// UI_Status_Region (0x4B1C00 → 0x497040).
fn region_page(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    s: &CareerStats,
    label: &dyn Fn(u32) -> String,
) {
    let slider_parts = [fe_ids::SLIDER_BASE, 0x8DAF_A5E4, 0x7342_982D, fe_ids::SLIDER_BEG, fe_ids::SLIDER_END];
    let counts = |(a, b): (u32, u32)| {
        format!("{}/{}", crate::frontend::grouped(i64::from(a)), crate::frontend::grouped(i64::from(b)))
    };
    if s.stage == 0 {
        // The intro: "Go to Car Lot" / "Go To Garage" (0x50C900) on the
        // first two labels, the percentage, the first count and the
        // slider hidden.
        state.text.insert(fe_ids::OBJ_4D2F5C61, label(0x2EEE_AA14));
        state.text.insert(fe_ids::OBJ_4D2F5C62, label(0xFD12_3B59));
        state.hidden.extend([fe_ids::OBJ_317AD6F2, fe_ids::OBJ_317AD6F3]);
        state.hidden.extend(slider_parts);
    } else {
        slider(state, package, slider_parts, s.region);
        state.text.insert(fe_ids::OBJ_317AD6F2, format!("{} %", crate::frontend::screens::fixed2(s.region * 100.0)));
        state.text.insert(fe_ids::OBJ_317AD6F3, counts(s.normal));
        if s.unlocked[1] {
            state.text.insert(fe_ids::OBJ_317AD6F4, counts(s.sponsor));
            state.text.insert(fe_ids::OBJ_317AD6F5, counts(s.url));
            state.text.insert(fe_ids::OBJ_317AD6F6, counts(s.stage_dvds));
        }
    }
    if !s.unlocked[1] {
        for k in 0..3u32 {
            state.hidden.extend([fe_ids::OBJ_4D2F5C63 + k + 0x476F_ECE6, fe_ids::OBJ_4D2F5C63 + k]);
        }
        state.hidden.extend([fe_ids::OBJ_317AD6F4, fe_ids::OBJ_317AD6F5, fe_ids::OBJ_317AD6F6]);
    }
}

/// UI_Status_DVD (0x4B1CF0 → 0x497330): the current stage's covers.
fn dvd_page(
    state: &mut ScreenState,
    _package: &crate::frontend::feng::Package,
    s: &CareerStats,
    label: &dyn Fn(u32) -> String,
) {
    let n = s.dvd_list.len();
    for (k, &(rating, done)) in s.dvd_list.iter().enumerate() {
        let k32 = k as u32;
        state.shown.insert(fe_ids::OBJ_02A9AFED + k32);
        // The labels' "%$.1f" (0x43E950): half up, not Rust's half even.
        let r = crate::frontend::grouped_fixed(f64::from(rating), 1);
        state.text.insert(
            0x2734_FB0E + k32,
            label(0x35CC_C129).replacen("%d", &(k + 1).to_string(), 1).replacen("%$.1f", &r, 1),
        );
        state.text.insert(0xB627_67F2 + k32, label(0x0303_53CD).replacen("%$.1f", &r, 1));
        // 0x50C990: the tick's texture, won or not.
        state.image.insert(0x449A_7D70 + k32, if done { fe_ids::BOX_ON } else { fe_ids::BOX_OFF });
    }
    if n > 0 {
        state.hidden.insert(fe_ids::OBJ_42E73D71);
    } else {
        state.shown.insert(fe_ids::OBJ_42E73D71);
    }
    for k in n..crate::frontend::career::stats::DVD_SLOTS {
        state.hidden.insert(fe_ids::OBJ_02A9AFED + k as u32);
    }
}
