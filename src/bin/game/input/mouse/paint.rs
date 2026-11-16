//! The front-end mouse of UI_Paint and UI_ChooseCustomHUD: one handler per
//! screen, called by `Game::fe_mouse` (cut from it; statement order
//! unchanged).

use super::{LT_ARROW, MouseCtx, RT_ARROW};
use crate::{Game, Menu, Mode};
use nfsu2::formats::hash::fe_hash;
use nfsu2::frontend::mouse as fm;
use nfsu2::frontend::screens::Key;

impl Game {
    // UI_Paint (0x563420): on the slots the strip (0x543D50); in a
    // slot 0x55D420: a move over a swatch (0x5308C0) selects and
    // previews it, a click on the current one asks to add it, the
    // finish arrows (+0x150 / +0x154) step the finish
    // (0x55D0E0(PAD_LTRIGGER / PAD_RTRIGGER)).
    pub(super) fn mouse_paint(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Paint(p)) = &self.mode else { return Ok(()) };
        let (selected, n) = (p.selected, p.swatches.len());
        match selected {
            None => {
                if self.bar_keys(c.bar("UI_Paint", "ScrollBar"), Key::Left, Key::Right, 1)? {
                    return Ok(());
                }
                if c.m.pressed && c.drawn("UI_Paint", fm::STRIP_SELECTED) {
                    return self.mouse_key(Key::Accept);
                }
            }
            Some(i) => {
                if !c.m.pressed && c.m.moved() {
                    let over = (0..n).find(|&k| c.drawn("UI_Paint", fe_hash(&format!("PAINT_SWATCH_{}", k + 1))));
                    if let Some(k) = over.filter(|&k| k != i)
                        && let Mode::Menu(Menu::Paint(p)) = &mut self.mode
                    {
                        p.selected = Some(k);
                    }
                } else if c.m.pressed {
                    if c.drawn("UI_Paint", fe_hash(&format!("PAINT_SWATCH_{}", i + 1))) {
                        return self.mouse_key(Key::Accept);
                    }
                    if c.hit("UI_Paint", LT_ARROW) {
                        return self.mouse_key(Key::LTrigger);
                    }
                    if c.hit("UI_Paint", RT_ARROW) {
                        return self.mouse_key(Key::RTrigger);
                    }
                }
            }
        }
        Ok(())
    }

    // UI_ChooseCustomHUD (0x5644E0): its arrows +0xAC / +0xB0 step
    // the colour (0x548E50(∓1), as Left / Right).
    pub(super) fn mouse_choose_hud(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        if c.m.pressed && c.hit("UI_ChooseCustomHUD", 0xD1CD_A594) {
            return self.mouse_key(Key::Left);
        }
        if c.m.pressed && c.hit("UI_ChooseCustomHUD", 0x9345_E4A7) {
            return self.mouse_key(Key::Right);
        }
        Ok(())
    }
}
