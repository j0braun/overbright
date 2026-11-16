use crate::*;

impl Game {
    /// The screens that move on by themselves.
    pub(crate) fn menu_tick(&mut self) {
        let now = self.now();
        let real = self.real_now();
        if self.fe.mc_background
            && !matches!(
                self.mode,
                Mode::Menu(Menu::McMain(_) | Menu::McList(_) | Menu::McMessage(..) | Menu::Dialog(..))
            )
        {
            self.fe.mc_background = false;
        }
        if matches!(self.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
            self.race_flow.game_mode = 2;
        }
        // 0x54E6F0 → 0x537E70 for a mask with bit 1 and not bit 2 starts
        // the cycle once in the career (0x51DDA0 while +0x3C is 0).
        // Its tick (0x537F60) counts the cycle down by the tick's quantised
        // dt; started on this tick, it counts from the next.
        if self.race_flow.game_mode != 1 {
            self.fe.help_cycle = None;
        } else if let Some(c) = self.fe.help_cycle.as_mut() {
            c.step(self.pacing.clocks.dt, now);
        } else if self.help_screen().is_some_and(|(m, _)| m & 3 == 1) {
            self.fe.help_cycle = Some(screens::HelpCycle::new(now));
        }
        // The dialogs' No / OK back on the needle colour (0xB4623F67 →
        // 0x5640BD, step 4) shows the custom gauge picker again.
        if let Mode::Menu(Menu::ChooseHud(c)) = &mut self.mode {
            c.hidden = false;
        }
        // The track-map widgets (`MapWidget`): the screen's hand-off 0.3 s
        // of the real-time clock after a change (World Map 0x4E12DF,
        // track select 0x4EFC30), else the widget's tick (0x4CDD40: the
        // track shown, the zoom counted down by the tick's dt).
        let dt = self.pacing.clocks.dt;
        let track = match &self.mode {
            Mode::Menu(Menu::WorldMap(map)) => self.world_map_track(map),
            Mode::Menu(Menu::QuickRace(qr)) => qr.track().map(|t| t.number),
            _ => None,
        };
        match &mut self.mode {
            Mode::Menu(Menu::WorldMap(map)) => map.widget.step(real, dt, track),
            Mode::Menu(Menu::QuickRace(qr)) => qr.widget.step(real, dt, track),
            _ => {}
        }
        match &mut self.mode {
            // UI_OLISPConnect's tick (0xC98356BA → 0x4B97D0 → 0x49DF20):
            // 0x49DF20 returns only 0 or 1 (0x49DF80 / 0x49DF9F / 0x49DFB4),
            // so the error dialog for the lobby's code (−1 / −2) is never
            // reached. Under Wine offline the
            // original shows EA Login at once (≈ 0.13 s after the key,
            // 0x8500D0 staying 2): the next tick leaves for UI_OLEALogin
            // (ISP_TRY_MS = 0). Which handler moves it while 0x8500D0 = 2
            // is not traced; the clone has no network code.
            Mode::Menu(Menu::IspConnect(opened)) if now - *opened >= ISP_TRY_MS => {
                self.mode = Mode::Menu(Menu::EaLogin(screens::EaLogin::new(now), None));
            }
            // The connect helper's dialog: a reopened one starts now.
            Mode::Menu(Menu::EaLogin(_, Some((_, since)))) if since.is_nan() => *since = now,
            // The connection fails (no EA server): 0x49F780's notice with
            // OK (0x2C0E485A → 0x4A0140 → UI_Main) over EA Login.
            Mode::Menu(Menu::EaLogin(login, Some((_, since)))) if now - *since >= screens::EaLogin::CONNECT_MS => {
                let login = login.clone();
                let text = self.label(0x3F92_0248, "");
                let ok = self.label(0x0639, "OK");
                let under = Mode::Menu(Menu::EaLogin(login, None));
                let mut d = Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&ok], now);
                d.under = Some(1);
                d.back = false;
                self.mode = Mode::Menu(Menu::Dialog(d, vec![self.icon_menu(MenuKind::Main), under]));
            }
            // The search dialog goes 5 s of the real-time clock after the
            // open (`lan_search_until`, 0x4FB8AE); no server answers offline.
            Mode::Menu(Menu::LanServers(_, searching)) if *searching && real > self.fe.lan_search_until => {
                *searching = false
            }
            Mode::Menu(Menu::Splash(splash)) => {
                if splash.tick(&self.game_data.assets, now, real) {
                    self.mode = self.boot_screen(BootStep::Intro);
                } else if splash.ready.is_some() {
                    self.fe.splash_seen = true;
                }
            }
            _ => {}
        }
    }
}
