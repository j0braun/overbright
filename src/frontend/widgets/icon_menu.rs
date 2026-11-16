//! The horizontal icon menu (IconScrollerMenu).

use glam::Vec2;

use crate::formats::hash::fe_hash;
use crate::frontend::career::Profile;
use crate::frontend::screens::*;
use crate::frontend::{Assets, Quad, ScreenState, layout};

/// A horizontal icon menu (the game's IconScrollerMenu, vtable 0x79C0FC).
pub struct IconMenu {
    /// The package it lives in (UI_Main, UI_StartCareer, UI_CareerCrib, …).
    pub package: String,
    /// Title set by the screen's code (UI_StartCareer sets "Career",
    /// 0x4AF12E); None keeps the package's own.
    pub title: Option<String>,
    pub options: Vec<IconOption>,
    pub selected: usize,
    pub opened: f32,
    /// The scroll offset's ease (0x574E40): start time (ms), start value
    /// and speed, target.
    scroll: (f32, f32, f32, f32),
    /// Text the screen's code sets per option (object, text), by option.
    pub extra: Vec<Vec<(u32, String)>>,
    /// Objects the screen hides.
    pub hide: Vec<u32>,
    /// Objects hidden while a given option is selected (by option).
    pub hide_option: Vec<Vec<u32>>,
    /// One slot instead of the icon strip: the SideScroll screens
    /// (UI_Sponsorship_new, 0x4DF736) set only the slot's texture and
    /// keep their option templates off screen.
    pub single: bool,
    /// A scrolling text list (0x544700): its row objects, each option's
    /// wrapped lines, and the first line shown (Up/Down move it).
    pub list: Option<(Vec<u32>, Vec<Vec<String>>)>,
    top: usize,
    /// Stat bars the screen lays out, with each option's current and
    /// preview values (UI_ChoosePerformancePackage, 0x54FFA0).
    pub bars: Vec<(StatBar, Vec<(f32, f32)>)>,
    /// What the screen's code hides, shows and runs per option, by option.
    pub option_state: Vec<OptionState>,
    /// Whether the icons fade out near the strip's ends (0x52F8D0 runs
    /// 0x50D8D0 → 0x51FD70 only with the scroller's +0xE4 set, which
    /// 0x538B40 sets from IconScrollerMenu::StopDim 0x538CB0 and three
    /// other callers). **Observed** under Wine (1280 × 960): the main,
    /// options, career and quick-race strips fade (the clone matched them
    /// to the level with the fade), the free-roam and race pause strips
    /// (UI_pause) do not (their right icon at full alpha: 143 / 157
    /// against the faded 132 / 145); which screens reach StopDim is
    /// **inferred** from that, not traced.
    pub edge_fade: bool,
    /// The strip's arrows' blink (0x52FA80, [`ArrowBlink`]).
    pub arrows: ArrowBlink,
}

/// Objects a screen's code hides or shows, and scripts it runs, while an
/// option is highlighted.
#[derive(Clone, Debug, Default)]
pub struct OptionState {
    pub hidden: Vec<u32>,
    pub shown: Vec<u32>,
    pub scripts: Vec<(u32, &'static str)>,
    /// Image objects given another texture (object, texture hash).
    pub images: Vec<(u32, u32)>,
}

/// The scroller's constants (constructor 0x543AA0 with 0x543850): the gap
/// between icons, the strip width (the constructor's 480), the ease
/// length (0.3 s) and the size of icons not selected (0x52F8D0: × 0.8).
pub(crate) const ICON_GAP: f32 = 40.0;
pub(crate) const STRIP_WIDTH: f32 = 480.0;
pub(crate) const SCROLL_TIME: f32 = 300.0;
pub(crate) const UNSELECTED_SCALE: f32 = 0.8;

impl IconMenu {
    pub fn new(package: &str, title: Option<&str>, options: Vec<IconOption>, now: f32) -> IconMenu {
        IconMenu {
            package: package.into(),
            title: title.map(str::to_string),
            options,
            selected: 0,
            opened: now,
            scroll: (now, 0.0, 0.0, 0.0),
            extra: Vec::new(),
            hide: Vec::new(),
            hide_option: Vec::new(),
            single: false,
            list: None,
            top: 0,
            bars: Vec::new(),
            option_state: Vec::new(),
            edge_fade: true,
            arrows: ArrowBlink::default(),
        }
    }

    /// The scroll offset and its speed at `now` (a cubic Hermite from the
    /// value and speed at the start to the target at rest, 0x574E40).
    fn offset(&self, now: f32) -> (f32, f32) {
        let (start, p0, v0, p1) = self.scroll;
        let t = ((now - start) / SCROLL_TIME).clamp(0.0, 1.0);
        let v0 = v0 * SCROLL_TIME;
        let a = v0 - 2.0 * (p1 - p0);
        let b = 3.0 * (p1 - p0) - 2.0 * v0;
        let value = ((a * t + b) * t + v0) * t + p0;
        let speed = ((3.0 * a * t + 2.0 * b) * t + v0) / SCROLL_TIME;
        if t >= 1.0 { (p1, 0.0) } else { (value, speed) }
    }

    /// Open on option `i`, already scrolled to it.
    pub fn select_now(&mut self, i: usize) {
        let i = i.min(self.options.len().saturating_sub(1));
        let target = -(i as f32) * (118.0 + ICON_GAP);
        self.selected = i;
        self.scroll = (self.opened, target, 0.0, target);
    }

    fn select(&mut self, i: usize, now: f32, spacing: f32) {
        let (p, v) = self.offset(now);
        self.selected = i;
        self.top = 0;
        self.scroll = (now, p, v, -(i as f32) * spacing);
    }

    /// The menu event whose sound a key plays on the strip
    /// (IconScrollerMenu_Handler 0x543D50; the event local starts at 0,
    /// no sound): Left (0x9120409E) / Right (0xB5971BF1) — handled only
    /// with the list's +0x79 set, the horizontal strips — give LEFT /
    /// RIGHT only when the current item is not the first (0x5276E0) /
    /// last (0x527710); Up / Down need +0x79 clear, so the strips ignore
    /// them; accept (0x0C407210) gives SELECT, or WRONG (0x13) on a locked
    /// item (+0x22); back (0x911AB364) gives BACK. Returns None for no
    /// sound. The text list of the SideScroll screens (`list`) keeps the
    // prov: open frontend.screens.sound_event
    /// generic Up / Down (**open**: its own handler's sound).
    pub fn sound_event(&self, key: Key) -> Option<u8> {
        use crate::audio::fe::event as e;
        let n = self.options.len();
        match key {
            Key::Left => (n > 0 && self.selected > 0).then_some(e::LEFT),
            Key::Right => (n > 0 && self.selected + 1 < n).then_some(e::RIGHT),
            Key::Up if self.list.is_some() => Some(e::UP),
            Key::Down if self.list.is_some() => Some(e::DOWN),
            Key::Accept if matches!(self.options.get(self.selected).map(|o| o.action), Some(Action::Unavailable)) => {
                Some(e::WRONG)
            }
            Key::Accept => Some(e::SELECT),
            Key::Back => Some(e::BACK),
            _ => None,
        }
    }

    pub fn key(&mut self, key: Key, now: f32) -> Option<Action> {
        let n = self.options.len();
        // The option pitch: the 118-wide template plus the gap.
        let spacing = 118.0 + ICON_GAP;
        match key {
            // The scroller stops at the ends (0x51FA00 walks without wrap).
            Key::Left if self.selected > 0 => {
                self.select(self.selected - 1, now, spacing);
                self.arrows.step(false, now, false);
            }
            Key::Right if self.selected + 1 < n => {
                self.select(self.selected + 1, now, spacing);
                self.arrows.step(true, now, false);
            }
            Key::Up => self.top = self.top.saturating_sub(1),
            Key::Down => {
                if let Some((rows, lines)) = &self.list {
                    let count = lines.get(self.selected).map_or(0, Vec::len);
                    self.top = (self.top + 1).min(count.saturating_sub(rows.len()));
                }
            }
            Key::Accept => return Some(self.options[self.selected].action),
            _ => {}
        }
        None
    }

    pub fn quads(&self, assets: &Assets, profile: Option<&Profile>, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package(&self.package) else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        if let Some(t) = &self.title {
            state.text.insert(ui_main::TITLE, t.clone());
        }
        let hash = self.options[self.selected].label;
        // A clone item's label ("Photo Mode") is not in the game's
        // string table.
        let label =
            assets.strings.get(hash).or_else(|| crate::frontend::options::clone_label(hash)).unwrap_or("").to_string();
        state.text.insert(ui_main::OPTION_TITLE, label);
        state.image.insert(ui_main::SLOT, self.options[self.selected].icon);
        match profile {
            Some(p) => {
                state.text.insert(ui_main::CASH, crate::frontend::grouped(p.cash as i64));
                state.hidden.insert(ui_main::PLAYER_NAME);
            }
            None => {
                state.hidden.insert(ui_main::PLAYER_NAME);
                state.hidden.insert(ui_main::CASH);
            }
        }
        if let Some(extra) = self.extra.get(self.selected) {
            for (object, text) in extra {
                state.text.insert(*object, text.clone());
            }
        }
        state.hidden.extend(self.hide.iter().copied());
        for (bar, values) in &self.bars {
            if let Some(&(current, preview)) = values.get(self.selected) {
                stat_bar(&mut state, package, bar, current, preview);
            }
        }
        if let Some(o) = self.option_state.get(self.selected) {
            state.hidden.extend(o.hidden.iter().copied());
            state.shown.extend(o.shown.iter().copied());
            for &(object, script) in &o.scripts {
                state.run(object, script, 0.0);
            }
            state.image.extend(o.images.iter().copied());
        }
        if let Some(hide) = self.hide_option.get(self.selected) {
            state.hidden.extend(hide.iter().copied());
        }
        // LOCK_MASTER INITs to HIDE; 0x51DF10 runs SHOW on it while the
        // selected option is locked (item +0x22).
        if matches!(self.options[self.selected].action, Action::Unavailable) {
            state.shown.insert(parts_browser::LOCK_MASTER);
        } else {
            state.hidden.insert(parts_browser::LOCK_MASTER);
        }
        if let Some((rows, lines)) = &self.list {
            let lines = lines.get(self.selected).map_or(&[][..], Vec::as_slice);
            for (i, &row) in rows.iter().enumerate() {
                if let Some(line) = lines.get(self.top + i) {
                    state.text.insert(row, line.clone());
                } else {
                    state.hidden.insert(row);
                }
            }
            scroll_bar(&mut state, package, rows.len(), lines.len(), self.top);
        }
        // The strip's own "ScrollBar" (0x542AD0 at 0x543AF9, scroller
        // +0x38): 0x538090(1, +0x28 − 1, +0xC8) at 0x538993 / 0x538B28, the
        // handle at the selected option of all of them (measured against
        // the original: UI_Main's Quick Race, 2 of 7, sits 1/6 along).
        // The single-slot screens are the same class (0x552340 → the
        // scroller 0x543AA0; Wine menus112 pr04 / pr05: UI_Sponsorship_new
        // with five sponsors, the handle at the left on the first and 1/4
        // along on the second, the left arrow off on the first).
        {
            scroll_bar_at(&mut state, package, "ScrollBar", true, 1, self.options.len(), self.selected + 1);
            // Then 0x52FA80 (from the scroller's update 0x538950): the
            // left arrow (scroll bar +0x48, ScrollBar_Arrow_1) hides on the
            // first option, the right (+0x4C) on the last.
            if self.selected == 0 {
                state.hidden.insert(fe_hash("ScrollBar_Arrow_1"));
            } else if self.selected + 1 == self.options.len() {
                state.hidden.insert(fe_hash("ScrollBar_Arrow_2"));
            }
            self.arrows.apply(&mut state, SCROLLBAR_ARROWS, self.opened);
        }
        let mut quads = layout(assets, package, &state, now - self.opened);
        if self.single {
            return quads;
        }
        // OPTION_MASTER (the slot) gives the strip's centre and the icon
        // size; every option is placed from it (0x52F8D0).
        let Some(slot) = quads.iter().position(|q| q.texture == Some(self.options[self.selected].icon)) else {
            return quads;
        };
        let master = quads.remove(slot);
        let centre = (master.rect[0] + master.rect[1]) * 0.5;
        let size = master.rect[1] - master.rect[0];
        let spacing = size.x + ICON_GAP;
        let (offset, _) = self.offset(now);
        // The options are clones of OPTION_N, so they keep its z (100 in
        // UI_Main / UI_OptionsMain) and draw in that bucket: under the
        // selection's glow (CATEGORY_ONLY 0xDF422345, z 84), not on top of
        // everything (measured against the original, Wine 1280 × 960: the
        // selected additive icon shows through the glow's alpha, Audio's
        // centre 148,169,116, not 174,195,142).
        let template_z =
            package.object(fe_hash("OPTION_1")).map(|t| t.attributes.position().z.trunc().clamp(0.0, 255.0));
        // A dialog's DIM over the screen (StartDim 0x538C10): the handler's
        // DIM sends 0xA301C015 and the options, clones of OPTION_N with its
        // scripts, run its DIM (UI_PerformanceDyno_MAIN: alpha to 0;
        // UI_ProfileManager: to 60); 0x51FD20 turned the fade by place off.
        let dimmed = package.object(fe_hash("OPTION_1")).and_then(|t| crate::frontend::dim_alpha_of(package, t));
        let at = template_z.map_or(quads.len(), |tz| quads.iter().position(|q| q.z <= tz).unwrap_or(quads.len()));
        let tail = quads.split_off(at);
        for (i, o) in self.options.iter().enumerate() {
            let x = centre.x + offset + i as f32 * spacing;
            // Outside the strip: not drawn; near its ends: faded (0x50D8D0
            // over 0.8 × the icon width).
            let (lo, hi) = (centre.x - STRIP_WIDTH * 0.5, centre.x + STRIP_WIDTH * 0.5);
            if x < lo || x > hi {
                continue;
            }
            let ramp = size.x * UNSELECTED_SCALE;
            let alpha = if !self.edge_fade {
                1.0
            } else if x < lo + ramp {
                (x - lo) / ramp
            } else if x > hi - ramp {
                (hi - x) / ramp
            } else {
                1.0
            };
            let half = if i == self.selected { size * 0.5 } else { size * UNSELECTED_SCALE * 0.5 };
            let c = Vec2::new(x, centre.y);
            // The options are clones of the white option_N templates: only
            // the master's alpha carries over (0x51FD70 sets the alpha).
            let color = [1.0, 1.0, 1.0, dimmed.unwrap_or(master.color[3] * alpha)];
            quads.push(Quad {
                texture: Some(o.icon),
                rect: [c - half, c + half],
                uv: master.uv,
                color,
                z: template_z.unwrap_or(master.z),
                angle: 0.0,
                place: master.place,
            });
            if i == self.selected {
                crate::frontend::mouse::record_rect(
                    &self.package,
                    crate::frontend::mouse::STRIP_SELECTED,
                    c - half,
                    c + half,
                    master.place,
                );
            }
        }
        // The strip's own box for the wheel (0x54EB90's +0x8C / +0x94; its
        // prov: inferred frontend.screens.quads-6
        // extent here is the strip's, **inferred**).
        let half = Vec2::new(STRIP_WIDTH * 0.5, size.y * 0.5);
        crate::frontend::mouse::record_rect(
            &self.package,
            crate::frontend::mouse::STRIP,
            centre - half,
            centre + half,
            master.place,
        );
        quads.extend(tail);
        quads
    }
}
