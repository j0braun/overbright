use crate::frontend::fe_ids;
use crate::frontend::career::CareerData;
use crate::frontend::options::{Row, draw_rows};
use crate::frontend::quick_race::race_setup::*;
use crate::frontend::screens::{Key, QrMode};
use crate::frontend::{Assets, Quad, ScreenState, layout};

/// What a key on UI_QRModeOptions asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QrOptionsKey {
    None,
    /// Continue: UI_QRCarSelect (0x498220).
    Continue,
    /// Back: UI_QRTrackSelect (0x4B2AF0).
    Back,
}

/// UI_QRModeOptions (class 0x4EF3F0 on the option list 0x552790, vtable
/// 0x79801C, factory 0x4FA550): titled "Quick Race Game Options"
/// (0x47597606) in game mode 2, 0x4FD28546 hidden; the type's rows.
#[derive(Clone, Debug)]
pub struct QrOptions {
    pub mode: QrMode,
    pub selected: usize,
    pub top: usize,
    pub opened: f32,
    pub moved: f32,
    pub blink: Option<(bool, f32)>,
}

/// Rows shown at once (the option list's default).
const VISIBLE: usize = 8;

impl QrOptions {
    /// On the first enabled row (0x539660(0)).
    pub fn new(mode: QrMode, rows: &[Row], now: f32) -> QrOptions {
        let selected = rows.iter().position(|r| r.enabled).unwrap_or(0);
        QrOptions { mode, selected, top: 0, opened: now, moved: now, blink: None }
    }

    /// Up / Down move between enabled rows without wrapping (0x5390F0);
    /// Left / Right change the row; Accept continues, Back goes back.
    pub fn key(&mut self, key: Key, data: &CareerData, r: &mut QrRecord, open: bool, now: f32) -> QrOptionsKey {
        let ty = race_type(self.mode);
        let rows = rows(ty, data.tracks.get(&r.track), open, r);
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
                    if rows[i].enabled {
                        break;
                    }
                }
                if rows.get(i).is_some_and(|r| r.enabled) && i != self.selected {
                    self.selected = i;
                    self.moved = now;
                }
                if self.selected < self.top {
                    self.top = self.selected;
                } else if self.selected >= self.top + VISIBLE {
                    self.top = self.selected + 1 - VISIBLE;
                }
                QrOptionsKey::None
            }
            Key::Left | Key::Right => {
                if let Some(row) = rows.get(self.selected).filter(|r| r.enabled) {
                    change(row.id, ty, r, key == Key::Right);
                    self.blink = Some((key == Key::Right, now));
                }
                QrOptionsKey::None
            }
            Key::Accept => QrOptionsKey::Continue,
            Key::Back => QrOptionsKey::Back,
            _ => QrOptionsKey::None,
        }
    }

    pub fn quads(&self, assets: &Assets, data: &CareerData, r: &QrRecord, open: bool, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_QRModeOptions") else { return Vec::new() };
        let mut state = ScreenState::default();
        crate::frontend::screens::footer(&mut state, package, "Select");
        for hidden in ["EA Messenger", "$JOY_EVENT_FENG_BUTTON1$", "PLAYER NAME", "$999999999"] {
            for o in package.objects.iter().filter(|o| o.text == hidden) {
                state.hidden.insert(o.hash);
            }
        }
        state
            .text
            .insert(fe_ids::UI_MAIN_TITLE, assets.strings.get(0x4759_7606).unwrap_or("Quick Race Game Options").to_string());
        state.hidden.insert(0x4FD2_8546);
        // The list's masters only place the rows.
        state.hidden.insert(fe_ids::TITLE_MASTER);
        state.hidden.insert(fe_ids::DATA_MASTER);
        let rows = rows(race_type(self.mode), data.tracks.get(&r.track), open, r);
        draw_rows(
            assets,
            package,
            &mut state,
            &rows,
            self.top,
            self.selected,
            self.moved - self.opened,
            self.blink.map(|(r, at)| (r, at - self.opened)),
        );
        // The list's scroll bar (option list +0x6C, 0x539337 …):
        // 0x538090(1, the row count, 0x520030's 1-based position), as
        // UI_Options'. Wine (menus15, Circuit's 7 rows): the handle at the
        // top on row 1, 5/6 of the way on row 6, at the end on row 7 (the
        // clone took count − 1 and sat at the end from row 6).
        crate::frontend::screens::scroll_bar_at(
            &mut state,
            package,
            "ScrollBar",
            false,
            1,
            rows.len(),
            self.selected + 1,
        );
        layout(assets, package, &state, now - self.opened)
    }
}
