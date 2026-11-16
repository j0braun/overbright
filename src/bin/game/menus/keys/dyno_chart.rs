use crate::*;

impl Game {
    pub(super) fn dyno_chart_key(&mut self, key: Key, now: f32) -> Result<()> {
        if key == Key::Accept {
            self.record_dyno_run();
            // The pull spins down on behind the results.
            if let Mode::Menu(Menu::DynoChart(c)) = &mut self.mode {
                self.dyno.spin = c.pull.take().filter(|p| p.stage() != 0);
            }
            self.mode = Mode::Menu(Menu::DynoResults(screens::DynoResults {
                runs: self.dyno.runs,
                metric: self.settings.metric,
                opened: now,
                ratings: Some(self.dyno_ratings()),
            }));
        }
        Ok(())
    }

    pub(super) fn dyno_results_key(&mut self, key: Key, now: f32) -> Result<()> {
        match key {
            Key::Accept => self.mode = self.icon_menu(MenuKind::Dyno),
            Key::Back => {
                if let Mode::Menu(Menu::DynoChart(mut c)) = self.dyno_chart(now) {
                    c.pull = None;
                    c.finished = Some(0.0);
                    c.recorded = true;
                    self.mode = Mode::Menu(Menu::DynoChart(c));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
