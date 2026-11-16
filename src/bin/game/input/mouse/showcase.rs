//! The front-end mouse of the magazine cover views and UI_ShowCase_Preview:
//! one handler per screen, called by `Game::fe_mouse` (cut from it;
//! statement order unchanged).

use super::MouseCtx;
use crate::{Game, Menu, Mode};
use nfsu2::frontend::screens::{self, Key};

impl Game {
    // UI_MagazineView (0x565B60): a click on BACK_BUTTON sends Back;
    // UI_MagazineReward (0x55B270): a click on its button 0x69BA0838
    // asks to save the cover (0x55B230). Hovering pulses them
    // (`CoverView::quads_hover`).
    pub(super) fn mouse_cover(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Cover(v)) = &self.mode else { return Ok(()) };
        let reward = v.reward;
        let (package, button) = if reward {
            ("UI_MagazineReward", screens::CoverView::REWARD_BUTTON)
        } else {
            ("UI_MagazineView", screens::CoverView::BACK_BUTTON)
        };
        self.set_fe_hover(c.hit(package, button).then_some(button), c.now);
        if c.m.pressed {
            if reward && c.hit("UI_MagazineReward", 0x69BA_0838) {
                return self.mouse_key(Key::Accept);
            }
            if !reward && c.hit("UI_MagazineView", 0x1C7F_CF8B) {
                return self.mouse_key(Key::Back);
            }
        }
        Ok(())
    }

    // UI_ShowCase_Preview (0x4CF170): a click on a shown corner
    // button posts the state's message for it (the menu: button 3
    // Accept; Control Car / Show Off: button 3 Back; Take Picture:
    // button 2 Back, 3 Accept); else the menu state (0x4CEA70): a
    // click on an option's box HIGHLIGHT_n runs it, a move over one
    // highlights it (0x49A050).
    pub(super) fn mouse_showcase(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::Showcase(v)) = &self.mode else { return Ok(()) };
        use nfsu2::frontend::screens::{ShowcaseMode, showcase};
        let (mode, option) = (v.mode, v.option);
        let package = "UI_ShowCase_Preview";
        let over = (0..3).find(|&k| c.drawn(package, showcase::BUTTONS[k]));
        self.set_fe_hover(over.map(|k| showcase::BUTTONS[k]), c.now);
        if c.m.pressed {
            let button = over;
            let key = match (mode, button) {
                (ShowcaseMode::Menu, Some(2)) => Some(Key::Accept),
                (ShowcaseMode::Control | ShowcaseMode::ShowOff, Some(2)) => Some(Key::Back),
                (ShowcaseMode::Picture, Some(1)) => Some(Key::Back),
                (ShowcaseMode::Picture, Some(2)) => Some(Key::Accept),
                _ => None,
            };
            if let Some(key) = key {
                return self.mouse_key(key);
            }
            if button.is_some() {
                return Ok(());
            }
        }
        if mode == ShowcaseMode::Menu {
            let over = (0..4).find(|&k| c.hit(package, showcase::BOXES[k]));
            if let Some(k) = over {
                if k != option
                    && let Mode::Menu(Menu::Showcase(v)) = &mut self.mode
                {
                    v.previous = Some(v.option);
                    v.option = k;
                    v.changed = c.now;
                }
                if c.m.pressed {
                    return self.mouse_key(Key::Accept);
                }
            }
        }
        Ok(())
    }
}
