//! The front-end mouse on the help bar (UI_PC_Help_Bar): its hover and its
//! buttons, which `Game::fe_mouse` runs before the screen's own handler
//! (cut from it; statement order unchanged).

use super::{HelpMsg, MouseCtx, help_message};
use crate::{Game, Menu, MenuKind, Mode};
use nfsu2::formats::hash::fe_hash;

impl Game {
    /// The help bar's hover, then a click on it: `Some` with the frame's
    /// result when the click ended the frame's handling.
    pub(super) fn mouse_help_bar(&mut self, c: &MouseCtx) -> Option<anyhow::Result<()>> {
        // The help bar's hover (0x52D500): the slot whose icon is under
        // the cursor; its pulse starts when the cursor enters it.
        if let Some((mask, _)) = self.mouse_help() {
            let slots = help_slots(mask).len();
            let over = (0..slots).find(|k| c.drawn("UI_PC_Help_Bar", fe_hash(&format!("icon_{}", k + 1))));
            let was = self.fe.help_hover;
            self.fe.help_hover = match (over, was) {
                (Some(k), Some((j, since))) if j == k => Some((k, since)),
                (Some(k), _) => Some((k, c.now)),
                (None, _) => None,
            };
        } else {
            self.fe.help_hover = None;
        }
        // The help bar's buttons (UI_PC_Help_Bar, 0x55DE10): a click on an
        // enabled slot (button%d or icon_%d) posts its bit's message.
        if c.m.pressed
            && let Some((mask, _)) = self.mouse_help()
        {
            let bits: Vec<u32> = help_slots(mask);
            for (k, &bit) in bits.iter().enumerate() {
                let (text, icon) = (fe_hash(&format!("button{}", k + 1)), fe_hash(&format!("icon_{}", k + 1)));
                if !(c.drawn("UI_PC_Help_Bar", text) || c.drawn("UI_PC_Help_Bar", icon)) {
                    continue;
                }
                let paint = matches!(self.mode, Mode::Menu(Menu::Icons(_, MenuKind::Graphics)));
                let tuning = matches!(
                    self.mode,
                    Mode::Menu(Menu::TuneSliders(_) | Menu::TuneGraph(_) | Menu::Icons(_, MenuKind::Tuning))
                );
                match help_message(bit, paint, tuning) {
                    Some(HelpMsg::Key(key)) => return Some(self.mouse_key(key)),
                    Some(HelpMsg::Tip) => {
                        self.tuning_tip();
                        return Some(Ok(()));
                    }
                    Some(HelpMsg::Quit) => {
                        self.quit_question();
                        return Some(Ok(()));
                    }
                    Some(HelpMsg::Logoff) => {
                        // 0x55DE10: sound 0x4B (docs/notes/audio/sound-audit.md),
                        // then the log-off question (online: not ported).
                        self.fe_id(0x4B);
                        return Some(Ok(()));
                    }
                    None => return Some(Ok(())),
                }
            }
        }
        None
    }
}

/// The help bar's slots: the help mask's bits in order, without 0x8000,
/// 0x1_0000, 0x2_0000 and 0x8000_0000, at most six (the hover and the
/// click took the same list, written out twice before).
fn help_slots(mask: u32) -> Vec<u32> {
    (0..32)
        .map(|b| 1u32 << b)
        .filter(|&b| mask & b != 0 && !matches!(b, 0x8000 | 0x1_0000 | 0x2_0000 | 0x8000_0000))
        .take(6)
        .collect()
}
