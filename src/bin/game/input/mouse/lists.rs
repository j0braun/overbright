//! The front-end mouse of MC_List and UI_SMS_Mailbox: one handler per screen,
//! called by `Game::fe_mouse` (cut from it; statement order
//! unchanged).

use super::{LT_ARROW, MouseCtx, RT_ARROW};
use crate::{Game, Menu, Mode};
use nfsu2::formats::hash::fe_hash;
use nfsu2::frontend::screens::Key;

impl Game {
    // MC_List (0x4E8FC0 → 0x54EB90, +0xA4 / +0xA8 = 0): a move over
    // a row selects it, the wheel steps; a click does nothing.
    pub(super) fn mouse_mc_list(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::McList(l)) = &self.mode else { return Ok(()) };
        use nfsu2::frontend::screens::mc_list;
        let top = l.top;
        let n = l.names.len();
        let row = (0..mc_list::ROWS)
            .filter(|&r| top + r < n)
            .find(|&r| c.drawn("MC_List", mc_list::BACKING + r as u32) || c.drawn("MC_List", mc_list::NAME + r as u32));
        if let Some(r) = row {
            if c.wheel.is_some() {
                return self.mouse_key(if c.m.wheel < 0.0 { Key::Down } else { Key::Up });
            }
            if let Mode::Menu(Menu::McList(l)) = &mut self.mode {
                l.selected = top + r;
            }
        }
        Ok(())
    }

    // UI_SMS_Mailbox (0x4CCC10): its rows are an icon scroller with
    // +0xA4 = Select, +0xA8 = 1: a click on another row selects
    // it, on the selected one opens it; the wheel steps inside the
    // list's box. The mailbox arrows (PC_LTrigger_Icon /
    // PC_RTrigger_Icon) step the mailbox (**observed** under Wine;
    // prov: open game.mouse.fe_mouse-4
    // the code that does it is **open**).
    pub(super) fn mouse_sms(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::SmsMailbox(mb)) = &self.mode else { return Ok(()) };
        use nfsu2::frontend::sms::mailbox_objects;
        let (top, selected, n) = (mb.top, mb.selected, mb.rows.len());
        let package = "UI_SMS_Mailbox";
        if c.m.pressed {
            if c.hit(package, LT_ARROW) {
                return self.mouse_key(Key::LTrigger);
            }
            if c.hit(package, RT_ARROW) {
                return self.mouse_key(Key::RTrigger);
            }
        }
        if c.wheel.is_some() && c.hit(package, fe_hash("scrollerina_area")) {
            return self.mouse_key(if c.m.wheel < 0.0 { Key::Down } else { Key::Up });
        }
        if c.m.pressed {
            let row = (0..mailbox_objects::ROWS)
                .filter(|&r| top + r < n)
                .find(|&r| c.drawn(package, fe_hash(&format!("HIGH_LIGHT_{}", r + 1))));
            if let Some(r) = row {
                if top + r == selected {
                    return self.mouse_key(Key::Accept);
                }
                if let Mode::Menu(Menu::SmsMailbox(mb)) = &mut self.mode {
                    mb.selected = top + r;
                    mb.moved = c.now;
                }
            }
        }
        Ok(())
    }
}
