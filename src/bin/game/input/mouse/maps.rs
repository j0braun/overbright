//! The front-end mouse of UI_CareerWorldMap, UI_InGame_WorldMap and the
//! status screens: one handler per screen, called by `Game::fe_mouse` (cut
//! from it; statement order unchanged).

use super::{ARROW_1, ARROW_2, LT_ARROW, MINI_MASTER, MouseCtx, PAGE, RT_ARROW};
use crate::{Game, Menu, Mode};
use nfsu2::formats::hash::fe_hash;
use nfsu2::frontend::screens::Key;

impl Game {
    // UI_CareerWorldMap (0x4E0F60): its ScrollBar arrows step the
    // stage (0x4E0E50); the "SideScrollBar" (+0x19C) or the wheel
    // step the events (0x4CC140), a track click three; the button
    // held over MINI_MASTER sends Select (the race question).
    pub(super) fn mouse_world_map(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let package = "UI_CareerWorldMap";
        if c.m.pressed && c.hit(package, ARROW_1) {
            return self.mouse_key(Key::Left);
        }
        if c.m.pressed && c.hit(package, ARROW_2) {
            return self.mouse_key(Key::Right);
        }
        let side = c.bar(package, "SideScrollBar");
        if self.bar_keys(c.wheel.or(side), Key::Up, Key::Down, PAGE)? {
            return Ok(());
        }
        if c.hit(package, MINI_MASTER) && c.held_once() {
            return self.mouse_key(Key::Accept);
        }
        Ok(())
    }

    // UI_InGame_WorldMap (0x4E08E0 → 0x4CBEE0) over its zones: the
    // markers (kind 0), the legend rows' ICON_BACKING_n (1) and the
    // zoom arrows (2). A click on a marker of a shown group selects
    // it and works the GPS button (0x4B10C0, as C); on a row it
    // focuses and toggles the row; on an arrow it zooms (0x4CBDA0).
    // A move into another zone than the last focuses the row or,
    // with the GPS off, selects the marker (0x4B1170); outside all
    // the row focus is dropped (the last zone kept).
    pub(super) fn mouse_ingame_map(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::InGameMap(map)) = &self.mode else { return Ok(()) };
        let package = "UI_InGame_WorldMap";
        let Some(pkg) = self.game_data.assets.package(package) else { return Ok(()) };
        let markers = map.mouse_markers(pkg);
        let rows = map.legend.len();
        let opened = map.opened;
        let mut zones: Vec<(u8, usize, u32)> = markers.iter().map(|&(i, h)| (0u8, i, h)).collect();
        zones.extend((0..rows).map(|r| (1u8, r, fe_hash(&format!("ICON_BACKING_{}", r + 1)))));
        zones.extend([(2u8, 0, LT_ARROW), (2u8, 1, RT_ARROW)]);
        let last = self.platform.mouse_state.map_zone.filter(|z| z.2 == opened).map(|z| (z.0, z.1));
        let over = zones.iter().find(|z| c.hit(package, z.2)).copied();
        if c.m.pressed
            && let Some((kind, i, _)) = over
        {
            match kind {
                0 => {
                    if let Mode::Menu(Menu::InGameMap(map)) = &mut self.mode
                        && map.click_marker(i)
                    {
                        return self.mouse_key(Key::Button1);
                    }
                }
                1 => {
                    if let Mode::Menu(Menu::InGameMap(map)) = &mut self.mode {
                        map.click_row(i);
                    }
                }
                _ => return self.mouse_key(if i == 0 { Key::LTrigger } else { Key::RTrigger }),
            }
        }
        match over {
            Some((kind, i, _)) if Some((kind, i)) != last => {
                self.platform.mouse_state.map_zone = Some((kind, i, opened));
                if let Mode::Menu(Menu::InGameMap(map)) = &mut self.mode {
                    match kind {
                        0 => map.hover_marker(i),
                        1 => map.hover_row(i),
                        _ => {}
                    }
                }
            }
            Some(_) => {}
            None => {
                if let Mode::Menu(Menu::InGameMap(map)) = &mut self.mode {
                    map.focus = None;
                }
            }
        }
        Ok(())
    }

    // UI_Status_Master (0x4B1F20): its ScrollBar arrows step the page
    // (0x4B1E60); UI_Status_Career (0x4B1B60): its SideScrollBar
    // arrows scroll the list (0x4B1A50). No wheel, no hover.
    pub(super) fn mouse_status(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Status(st)) = &self.mode else { return Ok(()) };
        let page = st.screen.page;
        if c.m.pressed {
            if c.hit("UI_Status_Master", ARROW_1) {
                return self.mouse_key(Key::Left);
            }
            if c.hit("UI_Status_Master", ARROW_2) {
                return self.mouse_key(Key::Right);
            }
            if page == 0 && c.hit("UI_Status_Career", 0x640F_0F22) {
                return self.mouse_key(Key::Up);
            }
            if page == 0 && c.hit("UI_Status_Career", 0x640F_0F23) {
                return self.mouse_key(Key::Down);
            }
        }
        Ok(())
    }
}
