//! The front-end mouse of IconScrollerMenu screens (and
//! UI_BuyPerformanceParts over them): one handler per screen, called by
//! `Game::fe_mouse` (cut from it; statement order unchanged).

use super::{LT_ARROW, MouseCtx, RT_ARROW, in_box};
use crate::{Game, Menu, MenuKind, Mode};
use nfsu2::frontend::mouse as fm;
use nfsu2::frontend::screens::Key;

impl Game {
    // IconScrollerMenu (0x543D50 → its strip's 0x538B90): the
    // strip's scroll bar only (+0x38; 1 or −10 one on, −1 or 10
    // one back; no wheel, no hover, verified). A click on the
    // selected icon accepts it (**observed** under Wine; not in
    // prov: open game.mouse.fe_mouse
    // 0x538B90, the path is **open**). UI_ChooseVinylLayer
    // (0x56AB10) adds its reorder arrows 0xCF6F0439 / 0xABA93D3F
    // (0x5632A0 with PAD_LTRIGGER / PAD_RTRIGGER).
    pub(super) fn mouse_icons(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Icons(menu, kind)) = &self.mode else { return Ok(()) };
        let (package, kind) = (menu.package.clone(), *kind);
        // UI_BuyPerformanceParts over Choose Individual Parts (an
        // option list, 0x552790): a pure move selects the row under
        // the cursor (Wine 2026-11-04: the highlight and the bar's
        // handle follow, the readouts stay; Enter then buys that
        // row; a click on a row does nothing); its bar steps the
        // prov: inferred game.mouse.fe_mouse-2
        // list (the option lists' 0x5444A0, **inferred** by class).
        if kind == MenuKind::PerfPackage && menu.selected == 0 {
            let canvas = c.canvas();
            if c.hover
                && let Some(r) = fm::slider_rows().iter().find(|r| in_box(canvas, r.1, r.2))
                && let Some(list) = self.shop.perf_parts.as_mut()
                && list.selected != r.0
                && r.0 < list.rows.len()
            {
                list.selected = r.0;
                list.keep_in_view();
                return Ok(());
            }
            if self.bar_keys(c.bar("UI_BuyPerformanceParts", "scrollbar"), Key::Up, Key::Down, 0)? {
                return Ok(());
            }
        }
        if self.bar_keys(c.bar(&package, "ScrollBar"), Key::Left, Key::Right, 1)? {
            return Ok(());
        }
        if c.m.pressed && c.drawn(&package, fm::STRIP_SELECTED) {
            return self.mouse_key(Key::Accept);
        }
        if kind == MenuKind::VinylLayer && c.m.pressed {
            if c.hit(&package, LT_ARROW) {
                return self.mouse_key(Key::LTrigger);
            }
            if c.hit(&package, RT_ARROW) {
                return self.mouse_key(Key::RTrigger);
            }
        }
        Ok(())
    }
}
