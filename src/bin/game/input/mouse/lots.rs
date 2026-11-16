//! The front-end mouse of the car selects, Quick Race's track select, the
//! post-race stats and UI_PerformanceBrandSelect: one handler per screen,
//! called by `Game::fe_mouse` (cut from it; statement order
//! unchanged).

use super::{ARROW_1, ARROW_2, LT_ARROW, MouseCtx, PAGE, RT_ARROW, in_box};
use crate::{Game, Menu, Mode};
use nfsu2::frontend::mouse as fm;
use nfsu2::frontend::screens::Key;

impl Game {
    // The car selects (UI_CareerCarSelect 0x4DFC50, UI_CareerCarLot
    // 0x4ED9E0, UI_QRCarSelect 0x4FC1F0): the list's scroll bar
    // (±1 Left / Right, a track click three); Quick Race's filter
    // arrows +0xD0 / +0xD4 first (PAD_LTRIGGER / PAD_RTRIGGER,
    // 0x4EED10; their "Blink" is not ported).
    pub(super) fn mouse_car_lot(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::CarLot(lot)) = &self.mode else { return Ok(()) };
        use nfsu2::frontend::screens::LotState;
        let (package, prefix) = match lot.state {
            LotState::QuickRace(_) | LotState::Customize => ("UI_QRCarSelect", "ScrollBar"),
            LotState::CribSelect => ("UI_CareerCarSelect", "Scroll_Bar"),
            _ => ("UI_CareerCarLot", "Scroll_Bar"),
        };
        if package == "UI_QRCarSelect" && c.m.pressed {
            if c.hit(package, LT_ARROW) {
                self.mouse_key(Key::LTrigger)?;
            } else if c.hit(package, RT_ARROW) {
                self.mouse_key(Key::RTrigger)?;
            }
        }
        if matches!(self.mode, Mode::Menu(Menu::CarLot(_))) {
            self.bar_keys(c.bar(package, prefix), Key::Left, Key::Right, PAGE)?;
        }
        Ok(())
    }

    // UI_QRTrackSelect (0x4EFA10): the scroll bar (+0xC0; ±1, a
    // track click three, 0x4E3270).
    pub(super) fn mouse_quick_race(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        self.bar_keys(c.bar("UI_QRTrackSelect", "ScrollBar"), Key::Left, Key::Right, PAGE)?;
        Ok(())
    }

    // UI_PostRaceStats (0x4FBE20): its ScrollBar arrows step the
    // racer (0x4F3DE0); the bar itself is passive.
    pub(super) fn mouse_stats(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        if c.m.pressed && c.hit("UI_PostRaceStats", ARROW_1) {
            return self.mouse_key(Key::Left);
        }
        if c.m.pressed && c.hit("UI_PostRaceStats", ARROW_2) {
            return self.mouse_key(Key::Right);
        }
        Ok(())
    }

    // UI_PerformanceBrandSelect (the option list 0x552790 through
    // 0x556030): a click on the highlighted row's arrows steps its
    // brand (Wine 2026-11-04, the button held a frame: Neuspeed →
    // Jackson Racing and back); a move over a row selects it and
    // prov: inferred game.mouse.fe_mouse-3
    // the bar steps the rows (**inferred** from the class, as
    // UI_Options).
    pub(super) fn mouse_brand_select(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let rows = fm::slider_rows();
        let canvas = c.canvas();
        let Mode::Menu(Menu::BrandSelect(b, _)) = &mut self.mode else { return Ok(()) };
        if c.hover {
            if let Some(r) =
                rows.iter().find(|r| in_box(canvas, r.1, r.2)).filter(|r| r.0 != b.selected && r.0 < b.rows.len())
            {
                b.selected = r.0;
                b.moved = c.now;
            }
        } else if c.m.pressed
            && let Some(r) = rows.iter().find(|r| r.0 == b.selected)
        {
            if in_box(canvas, r.3.0, r.3.1) {
                return self.mouse_key(Key::Left);
            }
            if in_box(canvas, r.4.0, r.4.1) {
                return self.mouse_key(Key::Right);
            }
        }
        self.bar_keys(c.bar("UI_PerformanceBrandSelect", "ScrollBar"), Key::Up, Key::Down, 0)?;
        Ok(())
    }
}
