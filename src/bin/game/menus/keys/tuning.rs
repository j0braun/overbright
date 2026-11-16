use crate::*;

impl Game {
    pub(super) fn tune_sliders_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::TuneSliders(sliders)) = &mut self.mode else { unreachable!() };
        if key == Key::Back {
            let category = sliders.category;
            self.mode = self.tuning_menu(category);
            return Ok(());
        }
        // BUTTON1 (0xC519BFC0, 0x5650D0; help bar 0x800 "Default"):
        // the selected row's parameter back to its middle (0x50F9B0;
        // the rows re-read their values, 0x530DA0).
        if key == Key::Button1 {
            if let Some(param) = sliders.selected_param() {
                self.restore_tuning(param);
            }
            self.refresh_tuning_views();
            return Ok(());
        }
        if let Some((param, dir)) = sliders.key(key) {
            self.nudge_slider(param, 0, dir);
            self.refresh_tuning_views();
        }
        Ok(())
    }

    pub(super) fn tune_graph_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::TuneGraph(graph)) = &mut self.mode else { unreachable!() };
        if key == Key::Back {
            let category = graph.category;
            self.mode = self.tuning_menu(category);
            return Ok(());
        }
        // BUTTON1 (0xC519BFC0 in 0x5654E0): the widget's parameter
        // (+0x50 → +0x4: 23, 24 or 16) back to its middle (0x50F9B0),
        // the car refitted (0x50FDA0 / 0x50FDD0) and the widget set
        // up again (vtable +0x8 / +0xC).
        let restore = key == Key::Button1;
        let press = if restore { Some((graph.param, 0, 0.0)) } else { graph.key(key) };
        if let Some((param, point, dir)) = press {
            if restore {
                self.restore_tuning(param);
            } else {
                self.nudge_slider(param, point, dir);
            }
            let curve = self.curve_graph(param);
            if let Mode::Menu(Menu::TuneGraph(g)) = &mut self.mode {
                g.curve = curve;
            }
            self.refresh_tuning_views();
        }
        Ok(())
    }
}
