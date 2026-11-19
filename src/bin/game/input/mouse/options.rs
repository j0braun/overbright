//! The front-end mouse of the option lists (UI_Options, Quick Race's options,
//! the tuning sliders) and the tuning curve: one handler per screen, called
//! by `Game::fe_mouse` (cut from it; statement order unchanged).

use super::{MouseCtx, in_box};
use crate::{Game, Menu, Mode};
use glam::Vec2;
use nfsu2::formats::hash::fe_hash;
use nfsu2::frontend::mouse as fm;
use nfsu2::frontend::screens::Key;

impl Game {
    // The option lists (0x52FC50 from 0x556030: UI_Options, Quick
    // Race's Game Options, the tuning sliders): a pure move selects
    // the enabled row under the cursor (0x51FEB0 over the rows'
    // title boxes, 0x50DB10); a click goes to the selected row's
    // arrows (Left / Right, 0x52DA00) or a tuning row's Tip mark
    // (0x565060 → 0xC519BFC3). The list's scroll bar runs on the
    // tick (0x5444A0): ±1 moves the selection, a track click does
    // nothing. Over DONE_BUTTON (+0x68) outside every row the
    // selection is dropped (not ported: the clone's lists always
    // have one).
    pub(super) fn mouse_option_list(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let rows = fm::slider_rows();
        let canvas = c.canvas();
        let selected = match &self.mode {
            Mode::Menu(Menu::TuneSliders(t)) => t.selected,
            Mode::Menu(Menu::Options(o)) => o.selected,
            Mode::Menu(Menu::QrOptions(o)) => o.selected,
            _ => return Ok(()),
        };
        if c.hover {
            if let Some(r) = rows.iter().find(|r| in_box(canvas, r.1, r.2)).filter(|r| r.0 != selected) {
                match &mut self.mode {
                    Mode::Menu(Menu::TuneSliders(t)) => t.selected = r.0,
                    Mode::Menu(Menu::Options(o)) => {
                        o.left = Some(o.selected);
                        o.selected = r.0;
                        o.moved = c.now;
                    }
                    Mode::Menu(Menu::QrOptions(o)) => {
                        o.left = Some(o.selected);
                        o.selected = r.0;
                        o.moved = c.now;
                    }
                    _ => {}
                }
            }
        } else if c.m.pressed
            && let Some(r) = rows.iter().find(|r| r.0 == selected)
        {
            if in_box(canvas, r.3.0, r.3.1) {
                return self.mouse_key(Key::Left);
            }
            if in_box(canvas, r.4.0, r.4.1) {
                return self.mouse_key(Key::Right);
            }
            // Settings page (docs/developing/settings-page.md, clone page): a click on the
            // settings page's selected row opens a group / presses a
            // reset (Enter); the original's lists keep theirs.
            if matches!(&self.mode, Mode::Menu(Menu::Options(o)) if o.category.visible() == nfsu2::frontend::options::PAGE_VISIBLE)
                && in_box(canvas, r.1, r.2)
            {
                return self.mouse_key(Key::Accept);
            }
            // The tip mark's box: its string's extent as drawn.
            if let Some(at) = r.5 {
                let k = rows.iter().position(|x| x.0 == selected).unwrap_or(0);
                let tip = fe_hash(&format!("TIP_TEXT_{}", k + 1));
                if c.drawn("UI_PerformanceTuning_Sliders", tip)
                    || in_box(canvas, at - Vec2::new(12.0, 0.0), Vec2::new(24.0, 12.0))
                {
                    self.tuning_tip();
                    return Ok(());
                }
            }
        }
        let package = match &self.mode {
            Mode::Menu(Menu::Options(o)) => Some(o.category.package(o.pause.is_some())),
            Mode::Menu(Menu::QrOptions(_)) => Some("UI_QRModeOptions"),
            _ => None,
        };
        if let Some(package) = package {
            self.bar_keys(c.bar(package, "ScrollBar"), Key::Up, Key::Down, 0)?;
        }
        Ok(())
    }

    // The curve widget (0x522FA0): a move over a point's column
    // selects it; the wheel moves it (+ up). The nitrous widget
    // (0x531500): a click on its arrows (0xD1CDA594 / 0x9345E4A7)
    // or the wheel steps it.
    pub(super) fn mouse_tune_graph(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        let Mode::Menu(Menu::TuneGraph(g)) = &self.mode else { return Ok(()) };
        let (package, nos, point) = (g.package, g.param == 16, g.point);
        if nos {
            if c.m.pressed && c.drawn(package, 0xD1CD_A594) {
                return self.mouse_key(Key::Left);
            }
            if c.m.pressed && c.drawn(package, 0x9345_E4A7) {
                return self.mouse_key(Key::Right);
            }
            if c.m.wheel != 0.0 {
                return self.mouse_key(if c.m.wheel < 0.0 { Key::Left } else { Key::Right });
            }
        } else {
            if c.m.moved()
                && let Some(k) = (0..9).find(|&k| c.drawn(package, fm::CURVE_COLUMN + k as u32))
                && k != point
                && let Mode::Menu(Menu::TuneGraph(g)) = &mut self.mode
            {
                g.point = k;
            }
            if c.m.wheel != 0.0 {
                return self.mouse_key(if c.m.wheel > 0.0 { Key::Up } else { Key::Down });
            }
        }
        Ok(())
    }
}
