use crate::*;

impl Game {
    pub(super) fn dialog_gearbox_back_key(&mut self) -> Result<()> {
        let Mode::Menu(Menu::Dialog(_, outcomes)) = &mut self.mode else { unreachable!() };
        self.shop.ask_gearbox = None;
        self.mode = outcomes.swap_remove(0);
        Ok(())
    }

    pub(super) fn dialog_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode else { unreachable!() };
        let answer_sound = d.answer_sound;
        if let Some(button) = d.key(key) {
            if let Some((b, id)) = answer_sound
                && b == button
            {
                self.fe_id(id);
            }
            let Mode::Menu(Menu::Dialog(answered, mut outcomes)) = std::mem::replace(&mut self.mode, Mode::Explore)
            else {
                unreachable!()
            };
            // The transmission question: Manual (0x5F5E3886) sets
            // the player's Transmission byte 0x83AA79 to 1, Auto
            // (0x1A2826E1) to 0, and both start the race
            // (0xBA5B5FCE).
            if let Some(event) = self.shop.ask_gearbox.take() {
                // Both answers play PlayFESound 0x56 (0x4EB25B /
                // 0x4EB325).
                self.fe_id(0x56);
                // The byte is the profile's (0x83AA79; options.txt does not
                // hold it): the current profile takes it, with no file
                // written between the key and the race's load.
                self.settings.players[0].manual = button == 0;
                self.settings.save_to_profile();
                return match event {
                    Some(e) => self.start_race(e, true, false),
                    None => {
                        self.race_flow.quick_free_run = true;
                        self.explore(None)
                    }
                };
            }
            if let Some(pending) = self.shop.pending_perf.take()
                && button == 0
                && outcomes.len() > 1
            {
                self.mode = match self.brand_select(pending, now) {
                    Some(b) => Mode::Menu(Menu::BrandSelect(b, pending)),
                    None => self.perf_confirm(pending, false),
                };
                return Ok(());
            }
            // A graphics-shop button's step.
            let step = self.shop.dialog_steps.get_mut(button).and_then(Option::take);
            self.shop.dialog_steps.clear();
            if let Some(step) = step {
                return self.setup_step(step);
            }
            let picked = button.min(outcomes.len() - 1);
            let then = outcomes.swap_remove(picked);
            // A save point may follow: keep the screen the dialog
            // was over for its MC screens (`save_under`). The
            // IconScrollerMenu screens (the performance categories,
            // UI_ICEMAIN, the dyno: their handlers call 0x543D50)
            // StartDim again on 0x96DDA9E6 (DIM from t = 0); the
            // parts browser's handler (0x5664A0, vtable 0x79D768)
            // has no case for it and stays as its MENUFADEOUT left it.
            self.fe.save_under = None;
            if matches!(
                then,
                Mode::Menu(Menu::LotDeal(LotDeal::LeaveShop | LotDeal::QuitTuningSave(true) | LotDeal::TuningSave))
            ) && let Some(u) = answered.under.filter(|&u| u != picked)
            {
                let u = if u == outcomes.len() { picked } else { u };
                if let Some(package) = outcomes.get(u).and_then(mode_package) {
                    let screen = std::mem::replace(&mut outcomes[u], Mode::Explore);
                    let (at, scripts) = match answered.dim_scripts {
                        Some(scripts) => (answered.opened, scripts),
                        None => (now, (nfsu2::frontend::DIM_SCRIPT, nfsu2::frontend::UNDIM_SCRIPT)),
                    };
                    self.fe.save_under = Some((Box::new(screen), package, at, scripts));
                }
            }
            if std::mem::take(&mut self.fe.resetting_display) && button == 0 {
                self.settings.reset_display(self.card);
                let _ = self.settings.save();
            }
            let quitting = std::mem::take(&mut self.fe.quitting) && button == 0;
            self.apply_dialog_outcome(then, &mut outcomes, quitting)?;
        }
        Ok(())
    }
}
