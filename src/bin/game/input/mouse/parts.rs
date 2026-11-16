//! The front-end mouse of the parts browsers and their overlays: one handler
//! per screen, called by `Game::fe_mouse` (cut from it; statement
//! order unchanged).

use super::{BarHit, MINI_MASTER, MouseCtx, PAGE};
use crate::{Game, Menu, Mode};
use nfsu2::formats::hash::fe_hash;
use nfsu2::frontend::mouse as fm;
use nfsu2::frontend::screens::Key;

impl Game {
    pub(super) fn mouse_parts(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Parts(b)) = &self.mode else { return Ok(()) };
        let (package, in_list, overlay) = (b.package, b.part.is_some(), b.overlay());
        let locked = b.categories.get(b.category).is_some_and(|c| c.locked);
        match package {
            // UI_GenericParts_Browser (0x5664A0): the strip's
            // "ScrollBar" (+0x114: ±1 Left / Right, a track click
            // three, 0x54FA00), the column's "SideScrollBar"
            // (+0x16C, the wheel over it: Up / Down 0x55EBB0, a
            // track click three), and the button held over
            // MINI_MASTER (+0x1E0, 0x52D0A0) sends Select.
            "UI_GenericParts_Browser" => {
                if self.bar_keys(c.bar(package, "ScrollBar"), Key::Left, Key::Right, PAGE)? {
                    return Ok(());
                }
                let side = c.bar(package, "SideScrollBar");
                if self.bar_keys(c.wheel.or(side), Key::Up, Key::Down, PAGE)? {
                    return Ok(());
                }
                if c.hit(package, MINI_MASTER) && c.held_once() {
                    return self.mouse_key(Key::Accept);
                }
            }
            // UI_Rims_Browser (0x567A60) / UI_ChooseSpinner
            // (0x569680): on the styles the strip (0x543D50) and a
            // click on the current style enters its sizes; in the
            // sizes a click on the strip's bar does nothing, one on
            // the current size or MINI_MASTER accepts it (0x559AC0);
            // locked: nothing. Then the "SideScrollBar" (+0x1B4,
            // ±1; the rims' only in the sizes) or the wheel steps
            // the size (0x55F750 / 0x561150); no track pages.
            "UI_Rims_Browser" | "UI_ChooseSpinner" => {
                if !in_list {
                    if self.bar_keys(c.bar(package, "ScrollBar"), Key::Left, Key::Right, 1)? {
                        return Ok(());
                    }
                    if c.m.pressed && c.drawn(package, fm::STRIP_SELECTED) {
                        return if locked { Ok(()) } else { self.mouse_key(Key::Accept) };
                    }
                } else {
                    let strip = ["ScrollBar_Arrow_1", "ScrollBar_Arrow_2", "ScrollBar_Backing", "ScrollBar_Handle"];
                    if c.m.pressed && strip.iter().any(|n| c.hit(package, fe_hash(n))) {
                        return Ok(());
                    }
                    if c.m.pressed && (c.drawn(package, fm::COLUMN_SELECTED) || c.hit(package, MINI_MASTER)) {
                        return if locked { Ok(()) } else { self.mouse_key(Key::Accept) };
                    }
                }
                let side = c.bar(package, "SideScrollBar").filter(|_| in_list || package == "UI_ChooseSpinner");
                let step = match side {
                    Some(h @ (BarHit::Arrow(_) | BarHit::Step(_))) => Some(h),
                    _ => c.wheel.filter(|_| in_list),
                };
                if self.bar_keys(step, Key::Up, Key::Down, 0)? {
                    return Ok(());
                }
            }
            // UI_ICEMAIN / UI_CustomNeonMain / UI_DecalMain: their
            // strips as an IconScrollerMenu's (**inferred** from
            // the class); a slot's list is its own screen over them.
            _ if !in_list && overlay.is_some() => {
                if self.bar_keys(c.bar(package, "ScrollBar"), Key::Left, Key::Right, 1)? {
                    return Ok(());
                }
                if c.m.pressed && c.drawn(package, fm::STRIP_SELECTED) {
                    return self.mouse_key(Key::Accept);
                }
            }
            _ => match overlay {
                // UI_IcePartsOverlay (0x568D40) and the neon's
                // UI_PartsList (0x56A310): the "SideScrollBar" (+0xB4)
                // or the wheel Up / Down, a track click three; the
                // button held on MINI_MASTER sends Select.
                Some(o @ ("UI_IcePartsOverlay" | "UI_NeonPartsOverlay")) => {
                    let side = c.bar(o, "SideScrollBar");
                    if self.bar_keys(c.wheel.or(side), Key::Up, Key::Down, PAGE)? {
                        return Ok(());
                    }
                    if c.hit(o, MINI_MASTER) && c.held_once() {
                        return self.mouse_key(Key::Accept);
                    }
                }
                // UI_CustomHUDOverlay (0x563D00): ±1 or the wheel
                // (0x55D670), no pages; MINI_MASTER held sends Accept
                // 0x406415E3.
                Some(o @ "UI_CustomHUDOverlay") => {
                    let side = match c.bar(o, "SideScrollBar") {
                        Some(h @ (BarHit::Arrow(_) | BarHit::Step(_))) => Some(h),
                        _ => c.wheel,
                    };
                    if self.bar_keys(side, Key::Up, Key::Down, 0)? {
                        return Ok(());
                    }
                    if c.hit(o, MINI_MASTER) && c.held_once() {
                        return self.mouse_key(Key::Accept);
                    }
                }
                // UI_DecalsOverlay (0x56B970): the wheel, else the
                // bar's ±1 (0x560110); no pages, no click.
                Some(o @ "UI_DecalsOverlay") => {
                    let side = match c.bar(o, "SideScrollBar") {
                        Some(h @ (BarHit::Arrow(_) | BarHit::Step(_))) => Some(h),
                        _ => None,
                    };
                    if self.bar_keys(c.wheel.or(side), Key::Up, Key::Down, 0)? {
                        return Ok(());
                    }
                }
                _ => {}
            },
        }
        Ok(())
    }
}
