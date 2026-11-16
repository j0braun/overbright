use crate::*;

impl Game {
    pub(super) fn credits_key(&mut self, key: Key) -> Result<()> {
        if key == Key::Back {
            self.mode = self.icon_menu(MenuKind::Options);
        }
        Ok(())
    }

    pub(super) fn controls_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Controls(c)) = &mut self.mode else { unreachable!() };
        match key {
            Key::Back => self.mode = self.icon_menu(MenuKind::Options),
            // BUTTON1 (0xC519BFBF, 0x4F9751): "Restore defaults?"
            // (0x2BBCCA6B, GenericDialog_SMALL, OK 0x0639 / Cancel
            // 0x541389E5); OK (0xEFEDCBA0) → 0x4ACD90.
            Key::Button1 => {
                let back = Mode::Menu(Menu::Controls(c.clone()));
                let restored = Mode::Menu(Menu::Controls(screens::ControlsScreen { restore: true, ..c.clone() }));
                let text = self.label(0x2BBC_CA6B, "?");
                let package = screens::dialog_package(&self.game_data.assets, &text);
                let buttons = [self.label(0x0000_0639, "OK"), self.label(0x5413_89E5, "Cancel")];
                self.mode = Mode::Menu(Menu::Dialog(
                    Dialog::new(package, &text, &[&buttons[0], &buttons[1]], self.now()),
                    vec![restored, back],
                ));
            }
            k => {
                let devices = 1 + self.platform.pads.devices.len();
                c.key(k, now, devices);
            }
        }
        Ok(())
    }

    pub(super) fn brand_select_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::BrandSelect(b, pending)) = &mut self.mode else { unreachable!() };
        match key {
            Key::Accept => {
                let (brands, pending) = (b.picked(), *pending);
                for (slot, brand) in self.shop.perf_brand_slots.iter_mut().zip(brands) {
                    *slot = brand;
                }
                self.mode = self.perf_confirm(pending, true);
            }
            Key::Back => {}
            k => b.key(k, now),
        }
        Ok(())
    }

    pub(super) fn jukebox_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Jukebox(j)) = &mut self.mode else { unreachable!() };
        match key {
            // Leaving builds the playlist again (0x56B1D0 on
            // 0x911AB364 → 0x479220): the pools refill.
            Key::Back => {
                if let Some(a) = self.audio.as_mut() {
                    a.streams.reset_playlist();
                }
                self.mode = self.icon_menu(MenuKind::Options);
            }
            // 0x56B1D0: Accept plays the selected track (0x480C10 →
            // 0x47AF90 with the item's +0x18).
            Key::Accept => {
                let pick = j.selected;
                if let Some(a) = self.audio.as_mut()
                    && a.streams.play_song(&a.mixer, pick).is_some()
                {
                    self.platform.chyron = Some(nfsu2::audio::music::Chyron::new(pick, self.now(), self.real_now()));
                }
            }
            // The play mode (settings +0x54): 0x565DA0, the jukebox's
            // update, flips it on either of its two events (+0x10C /
            // +0x110; under Wine '[' and ']' each flip it, the keys the
            // clone reads as the triggers). Its sound (0x458520 with
            // 0x8381AB / 0x8381C6: [LTRIGGER][0] for +0x10C,
            // [RTRIGGER][0] for +0x110) is the menu key sound.
            Key::LTrigger | Key::RTrigger => {
                self.settings.trax_random = !self.settings.trax_random;
                let _ = self.settings.save();
            }
            k => {
                if j.key(k, &mut self.settings.trax, now) {
                    let _ = self.settings.save();
                }
            }
        }
        Ok(())
    }
}
