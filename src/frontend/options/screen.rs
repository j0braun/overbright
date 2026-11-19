//! The UI_Options screen.

use crate::frontend::fe_ids;
use super::*;

/// What a key on the options screen asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsKey {
    None,
    /// A value changed: save it.
    Changed,
    /// Back to UI_OptionsMain.
    Leave,
    /// A button row was accepted.
    Button(RowId),
}

/// UI_Options: the category's rows in the option list (0x552790 with 8
/// rows shown, UI_Options' +0x134), the selected row highlighted.
#[derive(Clone, Debug)]
pub struct OptionsScreen {
    pub category: Category,
    /// Selected row and the first row shown.
    pub selected: usize,
    pub top: usize,
    pub opened: f32,
    /// When the selection last moved, ms (the highlight's start).
    pub moved: f32,
    /// The row that lost the highlight at `moved` (its UnHighlight's
    /// start; the other rows sit at their scripts' end state).
    pub left: Option<usize>,
    /// The arrow that last blinked and when (0x51E6C0).
    pub blink: Option<(bool, f32)>,
    /// The player whose settings show (0x836364).
    pub player: usize,
    /// Save prompts and autosave when the screen opened (Gameplay keeps
    /// them at +0x14F / +0x150 for the leave questions).
    pub opened_save: (bool, bool),
    /// Opened from a pause (UI_PauseOptions; UI_PC_Customize_Options goes
    /// back to it, 0x49C57B).
    pub pause: Option<Pause>,
}

/// Rows shown at once (0x4CF750 sets the list's +0x134 to 8).
pub const VISIBLE: usize = 8;
/// Rows shown on the settings page (the eighth row's place holds
/// the detail line's third and fourth lines).
pub const PAGE_VISIBLE: usize = 7;

impl Category {
    /// Rows shown at once on this category's page.
    pub fn visible(self) -> usize {
        if matches!(self, Category::Enhancements | Category::Settings(_) | Category::Advanced) { PAGE_VISIBLE } else { VISIBLE }
    }
}

impl OptionsScreen {
    /// A fresh screen on its first enabled row (0x539660(0)).
    pub fn new(category: Category, s: &Settings, force_feedback: bool, now: f32) -> OptionsScreen {
        OptionsScreen::new_in(category, s, force_feedback, None, now)
    }

    /// A fresh screen, on UI_PauseOptions when `pause` is set.
    pub fn new_in(
        category: Category,
        s: &Settings,
        force_feedback: bool,
        pause: Option<Pause>,
        now: f32,
    ) -> OptionsScreen {
        let rows = rows_in(category, s, 0, force_feedback, pause);
        // The settings page starts on its first row, greyed or not
        // (its detail line says why).
        let page = category.visible() == PAGE_VISIBLE;
        let selected = rows.iter().position(|r| r.enabled || (page && r.id != RowId::UxNoMatch)).unwrap_or(0);
        OptionsScreen {
            category,
            selected,
            top: 0,
            opened: now,
            moved: now,
            left: None,
            blink: None,
            player: 0,
            opened_save: (s.save_prompts, s.autosave),
            pause,
        }
    }

    /// The sound the list base handler 0x556030 plays for `key` on the
    /// current row ([`crate::audio::fe_rules::list_screen`]): sliders are
    /// the 0x50D6F0 class, value rows the toggles, buttons the buttons.
    pub fn sound(&self, key: Key, s: &Settings, force_feedback: bool) -> Option<u8> {
        use crate::audio::fe_rules::{RowClass, list_screen};
        let rows = rows_in(self.category, s, self.player, force_feedback, self.pause);
        let row = rows.get(self.selected).map(|r| {
            let class = match r.kind {
                Kind::Slider { value, min, max } => {
                    RowClass::Slider { at_min: value <= min + 1e-4, at_max: value >= max - 1e-4 }
                }
                Kind::Cycle => RowClass::Toggle,
                Kind::Button => RowClass::Button,
            };
            (class, r.enabled)
        });
        list_screen(key, row)
    }

    /// Up / Down move between enabled rows without wrapping (0x5390F0),
    /// the window following; Left / Right change the row (each item's
    /// handler); Accept presses a button row.
    pub fn key(&mut self, key: Key, s: &mut Settings, force_feedback: bool, has_profile: bool, now: f32) -> OptionsKey {
        let rows = rows_in(self.category, s, self.player, force_feedback, self.pause);
        // On the settings page a greyed row can be focused (its
        // detail line says why it is greyed); Left / Right do nothing there.
        let page = self.category.visible() == PAGE_VISIBLE;
        let focusable = |r: &Row| r.enabled || (page && r.id != RowId::UxNoMatch);
        match key {
            Key::Up | Key::Down => {
                let down = key == Key::Down;
                let mut i = self.selected;
                loop {
                    let next = if down { i + 1 } else { i.wrapping_sub(1) };
                    if next >= rows.len() {
                        break;
                    }
                    i = next;
                    if focusable(&rows[i]) {
                        break;
                    }
                }
                if rows.get(i).is_some_and(focusable) && i != self.selected {
                    self.left = Some(self.selected);
                    self.selected = i;
                    self.moved = now;
                }
                if self.selected < self.top {
                    self.top = self.selected;
                } else if self.selected >= self.top + self.category.visible() {
                    self.top = self.selected + 1 - self.category.visible();
                }
                OptionsKey::None
            }
            // The settings page's group pages step with the shoulder
            // keys; Button1 switches Compare anywhere on the page.
            Key::LTrigger | Key::RTrigger if matches!(self.category, Category::Settings(_)) => {
                let Category::Settings(g) = self.category else {
                    return OptionsKey::None;
                };
                OptionsKey::Button(RowId::UxGroup(g.step(key == Key::RTrigger)))
            }
            Key::Button1 if matches!(self.category, Category::Enhancements | Category::Settings(_) | Category::Advanced) => {
                crate::qol::set_compare(!crate::qol::comparing());
                OptionsKey::Changed
            }
            Key::Left | Key::Right if rows.get(self.selected).is_some_and(|r| r.id == RowId::UxCompare) => {
                crate::qol::set_compare(!crate::qol::comparing());
                self.blink = Some((key == Key::Right, now));
                OptionsKey::Changed
            }
            Key::Left | Key::Right if rows.get(self.selected).is_some_and(|r| r.id == RowId::UxFilter) => {
                let f = super::settings_page::filter();
                super::settings_page::set_filter(if key == Key::Right { f + 1 } else { f + 2 });
                self.blink = Some((key == Key::Right, now));
                OptionsKey::None
            }
            Key::Left | Key::Right => {
                let Some(row) = rows.get(self.selected) else {
                    return OptionsKey::None;
                };
                if row.kind == Kind::Button || !row.enabled {
                    return OptionsKey::None;
                }
                let before = s.clone();
                change(row.id, s, self.player, key == Key::Right, has_profile);
                self.blink = Some((key == Key::Right, now));
                // A removed row switched off leaves its group page.
                let n = rows_in(self.category, s, self.player, force_feedback, self.pause).len();
                if self.selected >= n {
                    self.selected = n.saturating_sub(1);
                    self.top = self.top.min(self.selected);
                }
                if *s != before { OptionsKey::Changed } else { OptionsKey::None }
            }
            // A focused greyed row (the settings page) takes no Accept.
            Key::Accept if rows.get(self.selected).is_some_and(|r| !r.enabled) => OptionsKey::None,
            Key::Accept => match rows.get(self.selected) {
                // Starts or cancels the background run here (the
                // game has nothing to switch to).
                Some(r) if r.id == RowId::GenerateTexturePack => {
                    crate::upscale_pack::toggle_job();
                    OptionsKey::None
                }
                // A group page's reset.
                Some(r) if matches!(r.id, RowId::UxReset(_)) => {
                    let RowId::UxReset(g) = r.id else {
                        return OptionsKey::None;
                    };
                    let before = s.clone();
                    super::settings_page::reset_group(g, s);
                    if *s != before { OptionsKey::Changed } else { OptionsKey::None }
                }
                Some(r) if r.kind == Kind::Button => OptionsKey::Button(r.id),
                // The Low/High and On/Off display rows toggle on Select too
                // (0x49BAB0…); Vsync does not (0x49B8A0).
                Some(r) if matches!(r.id, RowId::Custom(i) if i < 21 && CUSTOM_ROWS[i as usize].1 != CustomKind::Slider) =>
                {
                    let before = s.clone();
                    change(r.id, s, self.player, true, has_profile);
                    if *s != before { OptionsKey::Changed } else { OptionsKey::None }
                }
                _ => OptionsKey::None,
            },
            Key::Back => OptionsKey::Leave,
            _ => OptionsKey::None,
        }
    }

    /// The screen: the category title, the rows laid out from TITLE_MASTER
    /// and DATA_MASTER (0x51FF90 / 0x54EF40: each row the title master's
    /// height lower), names left on the title master's left edge, values
    /// centred on the data master (0x52D810), the arrows outside the data
    /// master's edges (0x52DAD0), sliders on the data master (0x52DF00 /
    /// 0x540D30) and each row's backing group at the name + (276, 10).
    pub fn quads(&self, assets: &Assets, s: &Settings, cash: Option<u32>, force_feedback: bool, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package(self.category.package(self.pause.is_some())) else {
            return Vec::new();
        };
        let mut state = ScreenState::default();
        super::screens::footer(&mut state, package, "Select");
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        if self.pause.is_some() && !matches!(self.category, Category::CustomDisplay | Category::Wheel) {
            // 0x4C3370…: "PAUSE OPTIONS", the category, Back and Select.
            state.text.insert(fe_hash("Header_text"), label(0x1D7B_B6C9));
            state.text.insert(fe_hash("Header_text2"), label(self.category.pause_header()));
            state.text.insert(fe_hash("Back_button"), label(0x933D_D695));
            state.text.insert(fe_hash("Select_button"), label(0x8202_C364));
        } else if self.category == Category::Wheel {
            // 0x4CFBFA: PLAYER reads "Player 1" / "Player 2" (0xD87133BC +
            // (0x836364 ≠ 0)).
            state.text.insert(fe_hash("PLAYER"), label(0xD871_33BC + u32::from(self.player != 0)));
        } else {
            state.text.insert(
                fe_ids::UI_MAIN_TITLE,
                assets
                    .strings
                    .get(self.category.title())
                    .or_else(|| clone_label(self.category.title()))
                    .unwrap_or("")
                    .to_string(),
            );
        }
        // 0x4B8470 hides these; the Player rows show the player arrows and
        // "Player Select" (0x4B7CD0).
        for h in [
            0x010C_BA23,
            0x97B7_1DDE,
            fe_ids::SCREEN_PLAYER,
            fe_hash("ColorCal_Logo"),
            fe_hash("ColorCal_Blurb"),
            fe_hash("Colour_Calibration_Backing"),
        ] {
            state.hidden.insert(h);
        }
        if self.category == Category::Player {
            state.shown.insert(fe_ids::OBJ_C90DC4AB);
        } else {
            state.hidden.insert(fe_ids::OBJ_C90DC4AB);
        }
        // 0x4B8470 hides the scroll bar's arrows (ScrollBar_Arrow_1 / _2,
        // 0x444969FD / FE) and shows them again only for the Player rows,
        // but then ends in the list's 0x539660(0), whose last step (list
        // +0x146 set) is the scroll bar's 0x538090(1, +0x12C − 1, the
        // selected row's position): with more than one row that shows
        // the bar, both arrows and the handle and places the handle
        // (0x52E330). Under Wine (2026-10-29) every UI_Options page (Audio,
        // Display, Gameplay, Player) shows both arrows and the handle at
        // the top on the first row.
        // The list's masters only place the rows.
        state.hidden.insert(TITLE_MASTER);
        state.hidden.insert(DATA_MASTER);
        match cash {
            Some(c) => {
                state.text.insert(fe_ids::UI_MAIN_CASH, crate::frontend::grouped(c as i64));
                state.hidden.insert(fe_ids::PLAYER_NAME);
            }
            None => {
                state.hidden.insert(fe_ids::PLAYER_NAME);
                state.hidden.insert(fe_ids::UI_MAIN_CASH);
            }
        }

        let rows = rows_in(self.category, s, self.player, force_feedback, self.pause);
        crate::frontend::screens::scroll_bar_at(
            &mut state,
            package,
            "ScrollBar",
            false,
            1,
            rows.len(),
            self.selected + 1,
        );
        let visible = self.category.visible();
        draw_rows_n(
            assets,
            package,
            &mut state,
            &rows,
            self.top,
            self.selected,
            self.moved - self.opened,
            self.left,
            self.blink.map(|(r, at)| (r, at - self.opened)),
            visible,
        );
        // The clone pages' detail line: the selected row's help
        // (the settings page: four lines from the eighth row's place).
        if let Some(text) = rows.get(self.selected).and_then(|r| detail(self.category, r.id, s)) {
            detail_lines(package, &mut state, &text, visible, if visible == PAGE_VISIBLE { 4 } else { 2 });
        }
        // Compare's state in the title while it shows the original.
        if matches!(self.category, Category::Enhancements | Category::Settings(_) | Category::Advanced) && crate::qol::comparing() {
            let title = state.text.get(&0x42AD_B44C).cloned().unwrap_or_default();
            state.text.insert(
                fe_ids::UI_MAIN_TITLE,
                format!("{title} - {}", super::settings_page::text(super::settings_page::Text::CompareOn)),
            );
        }
        layout(assets, package, &state, now - self.opened)
    }
}

impl OptionsScreen {
    /// HDR Output's calibration page (the Display group: the settings
    /// page's and the graphics group page's): the display transform draws
    /// its test pattern while it is open (docs/hdr.md "Calibration").
    pub fn calibrating(&self) -> bool {
        matches!(
            self.category,
            Category::GraphicsGroup(GraphicsGroup::Display)
                | Category::Settings(super::settings_model::UxGroup::Display)
        )
    }

    /// Where the calibration pattern goes on this page: the list's free row
    /// slots under the last row shown and above the detail lines, across
    /// the rows' columns (the names' left edge to the right arrows' right
    /// edge), on the 640×480 canvas (top left origin, centre-placed like the
    /// rows, so it scales and moves with the menu at every size and aspect).
    /// None off the calibration pages and when the page has no room for it.
    pub fn calibration_area(&self, assets: &Assets, s: &Settings, force_feedback: bool) -> Option<(Vec2, Vec2)> {
        if !self.calibrating() {
            return None;
        }
        let package = assets.package(self.category.package(self.pause.is_some()))?;
        let rows = rows_in(self.category, s, self.player, force_feedback, self.pause);
        let visible = self.category.visible();
        let shown = rows.len().saturating_sub(self.top).min(visible);
        let (rw, _) = object_size(package, RowParts::new(1).right).unwrap_or((24.0, 24.0));
        let (lo, hi) = calibration_band(
            master_box(package, TITLE_MASTER)?,
            master_box(package, DATA_MASTER)?,
            rw,
            shown,
            visible,
            if visible == PAGE_VISIBLE { 4 } else { 2 },
        )?;
        let centre = crate::frontend::CANVAS * 0.5;
        Some((lo + centre, hi + centre))
    }
}

/// The calibration pattern's band in the list's coordinates (the masters'
/// boxes, centre origin): from a quarter row under the `shown`th row to a
/// quarter row above the detail lines (as [`detail_lines`] places them),
/// from the names' left edge to the right arrow's right edge (`arrow_w`
/// wide). None when less than one row's height is left.
pub fn calibration_band(
    (tl, ts): (Vec2, Vec2),
    (dl, ds): (Vec2, Vec2),
    arrow_w: f32,
    shown: usize,
    visible: usize,
    detail_rows: usize,
) -> Option<(Vec2, Vec2)> {
    let margin = ts.y * 0.25;
    let detail_top = if detail_rows == 4 { ts.y * (visible as f32 - 0.2) } else { ts.y * visible as f32 };
    let top = tl.y + ts.y * shown as f32 + margin;
    let bottom = tl.y + detail_top - margin;
    let left = tl.x;
    let right = (dl.x + ds.x + arrow_w).max(tl.x + ts.x);
    (bottom - top >= ts.y && right > left).then(|| (Vec2::new(left, top), Vec2::new(right, bottom)))
}

/// Characters per detail line at [`DETAIL_SCALE`].
const DETAIL_CHARS: usize = 100;
const DETAIL_SCALE: f32 = 0.5;

/// The detail line under the list (a clone addition): `text` wrapped
/// into at most two lines on the list's last two row slots (12, 13, never
/// shown as rows: [`VISIBLE`] is 8), left on the title master's column
/// below the eighth row, smaller than the rows and unhighlighted.
fn detail_lines(package: &super::feng::Package, state: &mut ScreenState, text: &str, visible: usize, max_lines: usize) {
    let Some((tl, ts)) = master_box(package, TITLE_MASTER) else {
        return;
    };
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let last = lines.last_mut().unwrap();
        if !last.is_empty()
            && last.len() + 1 + word.len() > if max_lines == 4 { DETAIL_CHARS - 6 } else { DETAIL_CHARS }
        {
            if lines.len() == max_lines {
                break;
            }
            lines.push(String::new());
        }
        let last = lines.last_mut().unwrap();
        if !last.is_empty() {
            last.push(' ');
        }
        last.push_str(word);
    }
    for (k, line) in lines.iter().enumerate() {
        // The list's spare rows: 12 and 13; the settings page's four lines take 10
        // to 13 (the package has no fourteenth row).
        let h = RowParts::new(14 - max_lines + k).name;
        state.hidden.remove(&h);
        state.shown.insert(h);
        state.text.insert(h, line.clone());
        // The settings page's four lines: a little higher and closer (inside the panel).
        let (y0, dy) = if max_lines == 4 {
            (ts.y * (visible as f32 - 0.2), ts.y * 0.45)
        } else {
            (ts.y * visible as f32, ts.y * 0.5)
        };
        state.position.insert(h, Vec2::new(tl.x, tl.y + y0 + dy * k as f32));
        state.size.insert(h, Vec2::splat(DETAIL_SCALE));
        state.run(h, "UNHIGHLIGHT", 0.0);
    }
}

#[cfg(test)]
mod calibration_tests {
    use super::*;

    /// Masters as the list reads them (top left, size; centre origin).
    const TITLE: (Vec2, Vec2) = (Vec2::new(-250.0, -150.0), Vec2::new(240.0, 30.0));
    const DATA: (Vec2, Vec2) = (Vec2::new(40.0, -150.0), Vec2::new(150.0, 30.0));

    #[test]
    fn the_band_takes_the_free_rows_between_the_last_row_and_the_help() {
        let (lo, hi) = calibration_band(TITLE, DATA, 24.0, 5, PAGE_VISIBLE, 4).unwrap();
        // Under the fifth row (its bottom at -150 + 5 × 30) with a margin.
        assert!(lo.y >= -150.0 + 5.0 * 30.0);
        // Above the help lines (from 6.8 rows down) with a margin.
        assert!(hi.y <= -150.0 + 6.8 * 30.0);
        assert!(hi.y - lo.y >= 30.0);
        // Across the rows' columns: names to the right arrow.
        assert_eq!(lo.x, -250.0);
        assert_eq!(hi.x, 40.0 + 150.0 + 24.0);
        // No room: a full page.
        assert!(calibration_band(TITLE, DATA, 24.0, PAGE_VISIBLE, PAGE_VISIBLE, 4).is_none());
        assert!(calibration_band(TITLE, DATA, 24.0, 6, PAGE_VISIBLE, 4).is_none());
        // The graphics group page (8 rows, two help lines): four free rows.
        let (lo, hi) = calibration_band(TITLE, DATA, 24.0, 4, VISIBLE, 2).unwrap();
        assert!(lo.y >= -150.0 + 4.0 * 30.0 && hi.y <= -150.0 + 8.0 * 30.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_calibration_page_has_room_for_the_pattern() {
        let root = crate::testkit::game_root();
        let assets = Assets::load(&root).unwrap();
        let mut s = Settings::default();
        s.enh.hdr_output = true;
        for category in [
            Category::Settings(super::super::settings_model::UxGroup::Display),
            Category::GraphicsGroup(GraphicsGroup::Display),
        ] {
            for pause in [None, Some(Pause::default())] {
                let o = OptionsScreen::new_in(category, &s, false, pause, 0.0);
                assert!(o.calibrating());
                let (lo, hi) = o.calibration_area(&assets, &s, false).unwrap_or_else(|| panic!("{category:?} {pause:?}"));
                let package = assets.package(category.package(pause.is_some())).unwrap();
                let (tl, ts) = master_box(package, TITLE_MASTER).unwrap();
                let rows = rows_in(category, &s, 0, false, pause).len().min(category.visible());
                let centre = crate::frontend::CANVAS * 0.5;
                // Under every row's box, inside the canvas, a row high at least.
                assert!(lo.y >= tl.y + centre.y + ts.y * rows as f32, "{category:?}: {lo} under {rows} rows");
                assert!(lo.x >= 0.0 && hi.x <= 640.0 && hi.y <= 480.0, "{category:?}: {lo} {hi}");
                assert!(hi.y - lo.y >= ts.y, "{category:?}: {lo} {hi}");
            }
        }
        // Off the calibration pages: none.
        let o = OptionsScreen::new(Category::Audio, &s, false, 0.0);
        assert!(o.calibration_area(&assets, &s, false).is_none());
    }
}
