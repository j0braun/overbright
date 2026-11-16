//! The front-end mouse of MC_Main's questions and messages and the dialogs:
//! one handler per screen, called by `Game::fe_mouse` (cut from it;
//! statement order unchanged).

use super::MouseCtx;
use crate::{Game, Menu, Mode};
use nfsu2::frontend::screens::{self, Key};

impl Game {
    // MC_Main's Yes / No (the profile questions): as the dialogs
    // (**observed** under Wine: a move over a button focuses it, a
    // click answers it). MC_Main's handler 0x4FB8F0 has no mouse
    // prov: open game.mouse.fe_mouse-5
    // case of its own; where the original takes it is **open**.
    pub(super) fn mouse_mc_main(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::McMain(mc)) = &self.mode else { return Ok(()) };
        let selected = mc.selected;
        let over = screens::MC_MAIN_GROUPS[..2].iter().position(|&g| c.drawn("MC_Main", g));
        if let Some(i) = over {
            if i != selected
                && let Mode::Menu(Menu::McMain(mc)) = &mut self.mode
            {
                mc.selected = i;
            }
            if c.m.pressed {
                return self.mouse_key(Key::Accept);
            }
        }
        Ok(())
    }

    // MC_Main's messages (one button in the middle slot, two the
    // left and right): likewise.
    pub(super) fn mouse_mc_message(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::McMessage(mc, _)) = &self.mode else { return Ok(()) };
        let selected = mc.selected;
        let slots: &[usize] = if mc.buttons.len() == 1 { &[2] } else { &[0, 1] };
        let over = slots.iter().position(|&k| c.drawn("MC_Main", screens::MC_MAIN_GROUPS[k]));
        if let Some(i) = over {
            if i != selected
                && let Mode::Menu(Menu::McMessage(mc, _)) = &mut self.mode
            {
                mc.selected = i;
            }
            if c.m.pressed {
                return self.mouse_key(Key::Accept);
            }
        }
        Ok(())
    }

    // The dialogs: a move over a button focuses it, a click accepts
    // it (**observed** under Wine on the profile and quit questions;
    // the dialog class's own handler is not traced).
    pub(super) fn mouse_dialog(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Dialog(d, _)) = &self.mode else { return Ok(()) };
        let (package, groups, selected) = (d.package, d.button_groups(), d.selected);
        let over = groups.iter().position(|&g| c.drawn(package, g));
        if let Some(i) = over {
            if i != selected
                && let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode
            {
                d.selected = i;
            }
            if c.m.pressed {
                return self.mouse_key(Key::Accept);
            }
        }
        Ok(())
    }
}
