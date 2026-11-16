//! The front-end mouse of UI_Options_PC_Controller and EA_Trax_Jukebox: one
//! handler per screen, called by `Game::fe_mouse` (cut from it;
//! statement order unchanged).

use super::{LT_ARROW, MouseCtx, RT_ARROW};
use crate::{Game, Menu, Mode};
use nfsu2::frontend::mouse as fm;
use nfsu2::frontend::screens::Key;

impl Game {
    // UI_Options_PC_Controller (0x4ED140): a move over a key cell
    // selects it (0x4CB6B0 / 0x4DEC40), over the header highlights
    // it instead; a click on the current cell starts the capture
    // (as Accept, 0x5C2110), one on the header's arrows +0xDC /
    // +0xE0 steps the device (0x4ACAA0). DONE_BUTTON's focus is
    // not ported.
    pub(super) fn mouse_controls(&mut self, cx: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Controls(c)) = &self.mode else { return Ok(()) };
        use nfsu2::frontend::screens::controls_visible;
        let (top, selected, column, header) = (c.top, c.selected, c.column, c.header);
        let package = "UI_Options_PC_Controller";
        let cell = (top..top + controls_visible())
            .flat_map(|i| [(i, 0), (i, 1)])
            .find(|&(i, col)| cx.drawn(package, fm::CONTROL_CELL + (i * 2 + col) as u32));
        if cx.hover {
            if let Some((i, col)) = cell {
                if (i, col) != (selected, column) || header {
                    if let Mode::Menu(Menu::Controls(c)) = &mut self.mode {
                        c.header = false;
                        c.selected = i;
                        c.column = col;
                    }
                }
            } else if cx.hit(package, 0x4E11_CF90)
                && !header
                && let Mode::Menu(Menu::Controls(c)) = &mut self.mode
            {
                c.header = true;
            }
        } else if cx.m.pressed {
            if !header && cell == Some((selected, column)) {
                return self.mouse_key(Key::Accept);
            }
            for (arrow, key) in [(0xD1CD_A594, Key::Left), (0x9345_E4A7, Key::Right)] {
                if cx.hit(package, arrow) {
                    if let Mode::Menu(Menu::Controls(c)) = &mut self.mode {
                        c.header = true;
                    }
                    self.mouse_key(key)?;
                    if let Mode::Menu(Menu::Controls(c)) = &mut self.mode {
                        c.header = header;
                    }
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    // EA_Trax_Jukebox (0x56B1D0): a click on a play-mode arrow
    // (+0x10C / +0x110) flips the mode (0x83AA58) with its trigger's
    // sound; its list is an icon scroller 0x54EB90 with +0xA4 /
    // +0xA8 = 0 (0x542CB0): a move over a row selects it, the wheel
    // steps it; then each row's arrows step the selected track's
    // state (0x53DEC0) and a click on a row plays it (Select). The
    // reorder band is not ported (the clone has no reorder mode).
    pub(super) fn mouse_jukebox(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Jukebox(j)) = &self.mode else { return Ok(()) };
        use nfsu2::frontend::screens::jukebox;
        let (top, selected) = (j.top, j.selected);
        let package = "EA_TRAX_Jukebox";
        if c.m.pressed && c.hit(package, LT_ARROW) {
            return self.mouse_key(Key::LTrigger);
        }
        if c.m.pressed && c.hit(package, RT_ARROW) {
            return self.mouse_key(Key::RTrigger);
        }
        let row = (0..jukebox::ROWS).find(|&r| c.drawn(package, jukebox::GROUPS[0] + r as u32));
        if let Some(r) = row {
            if c.wheel.is_some() {
                return self.mouse_key(if c.m.wheel < 0.0 { Key::Down } else { Key::Up });
            }
            if top + r != selected
                && let Mode::Menu(Menu::Jukebox(j)) = &mut self.mode
            {
                j.selected = top + r;
                j.moved = c.now;
            }
        }
        // Its bar (+0x34): ±1, a track click a page (one step a
        // shown row, 0x543560 / 0x5435A0).
        if self.bar_keys(c.bar(package, "ScrollBar"), Key::Up, Key::Down, jukebox::ROWS)? {
            return Ok(());
        }
        if c.m.pressed {
            for r in 0..jukebox::ROWS {
                if c.hit(package, jukebox::LEFT + r as u32) {
                    return self.mouse_key(Key::Left);
                }
                if c.hit(package, jukebox::RIGHT + r as u32) {
                    return self.mouse_key(Key::Right);
                }
                if c.drawn(package, jukebox::GROUPS[0] + r as u32) {
                    return self.mouse_key(Key::Accept);
                }
            }
        }
        Ok(())
    }
}
