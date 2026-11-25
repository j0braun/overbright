use crate::*;

impl Game {
    pub(super) fn quick_race_key(&mut self, key: Key, now: f32) -> Result<()> {
        let real = self.real_now();
        let Mode::Menu(Menu::QuickRace(qr)) = &mut self.mode else { unreachable!() };
        if key == Key::Back {
            self.mode = self.icon_menu(MenuKind::QrMode);
            return Ok(());
        }
        // The track, then UI_QRModeOptions (0x498240, every type).
        let ty = nfsu2::frontend::quick_race::race_type(qr.mode);
        if qr.key(key, &mut self.settings.qr[ty], now, real).is_some() {
            self.quick.tracks = Some(qr.clone());
            let mode = qr.mode;
            let _ = self.settings.save();
            self.mode = self.quick_options_screen(mode, now);
        }
        Ok(())
    }

    pub(super) fn qr_options_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::QrOptions(o)) = &mut self.mode else { unreachable!() };
        let ty = nfsu2::frontend::quick_race::race_type(o.mode);
        let profile = self.profile.as_ref();
        let data = &self.game_data.career;
        let open = nfsu2::frontend::quick_race::unlocked(data, self.settings.qr[ty].track, &|e| {
            profile.is_some_and(|p| p.won(data, e.unlock))
        });
        let pressed = o.key(key, &self.game_data.career, &mut self.settings.qr[ty], open, now);
        let o = o.clone();
        match pressed {
            nfsu2::frontend::quick_race::QrOptionsKey::Back => {
                let _ = self.settings.save();
                self.mode = match self.quick.tracks.clone() {
                    Some(mut qr) => {
                        qr.opened = now;
                        Mode::Menu(Menu::QuickRace(qr))
                    }
                    None => self.icon_menu(MenuKind::QrMode),
                };
            }
            nfsu2::frontend::quick_race::QrOptionsKey::Continue => {
                let _ = self.settings.save();
                let mode = o.mode;
                self.quick.options = Some(o);
                let e = self.quick_event(mode);
                self.mode = self.quick_car_select(Some(e));
            }
            nfsu2::frontend::quick_race::QrOptionsKey::None => {}
        }
        Ok(())
    }

    pub(super) fn engage_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Engage(e)) = &mut self.mode else { unreachable!() };
        match key {
            Key::Left | Key::Right => e.focus.0 = Some((key == Key::Left, now)),
            Key::Accept => match e.focus.accept_focused(&self.game_data.assets, "UI_EngageRaceDialog", e.opened, now) {
                Some(true) => {
                    let event = e.event;
                    self.engage_accepted(event)?;
                }
                Some(false) => {
                    let event = e.event;
                    self.race_declined(event)?;
                }
                None => {}
            },
            Key::Back => {
                let event = e.event;
                self.race_declined(event)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn sponsor_popup_key(&mut self, key: Key) -> Result<()> {
        match key {
            // 0x4C12E0: Accept (0x0C407210 / 0x406415E3) → 0x4BFF80, the
            // sponsorship screen; Back (0x911AB364, 0x4C12F9 as 0x4C0020)
            // the reward before it, so the choice cannot be skipped.
            Key::Accept => {
                self.race_flow.chain.push(ChainStep::Popup);
                self.mode = self.icon_menu(MenuKind::Sponsor);
            }
            Key::Back => self.chain_back(self.now()),
            _ => {}
        }
        Ok(())
    }
}
