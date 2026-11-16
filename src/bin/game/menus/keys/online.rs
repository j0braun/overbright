use crate::*;

/// How long UI_OLISPConnect stays before UI_OLEALogin, ms: none — the
/// original goes on at its next tick (Wine: EA Login
/// fades in ≈ 0.13 s after the key, no ISP screen or error in any frame,
/// 0x8500D0 staying 2; 0x49DF20 never returns −1 / −2, so 0x4B97D0's
/// error dialog is unreachable).
pub(crate) const ISP_TRY_MS: f32 = 0.0;

/// UI_PC_LAN_ServerSelect's search timer (**traced**): the
/// constructor (0x4FB60E..0x4FB634) sets +0x900 = _ftol((0x8651A8 ×
/// 0.00025 + 5.0) × 4000 + 0.5) (0x784268, 0x7843E8, 0x784264, 0x784260),
/// on the real-time clock, not the package clock; the tick (0x4FB8AE..
/// 0x4FB8C9) closes the dialog once 0x8651A8 × 0.00025 > +0x900 × 0.00025,
/// i.e. 0x8651A8 > +0x900: 20 000 ticks (300 RACE ticks) after the open,
/// one more past 0x8651A8 ≈ 1.06e7 (the x87 product's last bit). Before:
/// 5000 ms of the package clock, 305 ticks.
pub(crate) fn lan_search_until(real: u64) -> u64 {
    let k = f64::from(0.00025f32);
    ((real as f64 * k + f64::from(5.0f32)) * f64::from(4000.0f32) + 0.5) as u64
}

impl Game {
    pub(super) fn splash_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::Splash(splash)) = &mut self.mode else { unreachable!() };
        if matches!(key, Key::Accept | Key::Start) && splash.ready.is_some() {
            self.fe_sound(fe::event::START, fe::control::GENERIC);
            self.mode = self.boot_profile_step();
        }
        Ok(())
    }

    pub(super) fn isp_connect_key(&mut self, key: Key) -> Result<()> {
        if matches!(key, Key::Accept | Key::Back) {
            self.mode = self.icon_menu(MenuKind::Main);
        }
        Ok(())
    }

    pub(super) fn ea_login_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::EaLogin(login, connecting)) = &mut self.mode else { unreachable!() };
        let (login, connecting) = (login.clone(), connecting.is_some());
        let under = Mode::Menu(Menu::EaLogin(login.clone(), None));
        let yes_no = [self.label(0xF770, "Yes"), self.label(0x061C, "No")];
        let question = |game: &Self, text: u32, no: Mode| {
            let text = game.label(text, "?");
            let mut d =
                Dialog::new(screens::dialog_package(&game.game_data.assets, &text), &text, &[&yes_no[0], &yes_no[1]], game.now());
            d.selected = 1;
            d.under = Some(2);
            (d, vec![game.icon_menu(MenuKind::Main), no])
        };
        match (connecting, key) {
            // The dialog's Cancel (0xCA4C311B → 0x4B8F40): No
            // (0xCA4C3140) brings "Connecting to lobby..." up.
            (true, Key::Accept) => {
                let (mut d, mut outcomes) =
                    question(self, 0xD857_CE14, Mode::Menu(Menu::EaLogin(login, Some((0x0023_E142, f32::NAN)))));
                outcomes.push(under);
                d.back = true;
                self.mode = Mode::Menu(Menu::Dialog(d, outcomes));
            }
            (true, _) => {}
            (false, Key::Accept) => {
                self.fe_sound(fe::event::START, fe::control::GENERIC);
                if let Mode::Menu(Menu::EaLogin(_, c)) = &mut self.mode {
                    *c = Some((0x1448_49C1, now));
                }
            }
            // Back: 0x558830 (Back on it ignored), No does nothing.
            (false, Key::Back) => {
                let (mut d, mut outcomes) = question(self, 0xAFFF_7C7A, Mode::Menu(Menu::EaLogin(login.clone(), None)));
                outcomes.push(under);
                d.back = false;
                self.mode = Mode::Menu(Menu::Dialog(d, outcomes));
            }
            (false, k) => {
                if let Mode::Menu(Menu::EaLogin(l, _)) = &mut self.mode {
                    l.key(k);
                }
            }
        }
        Ok(())
    }

    pub(super) fn lan_servers_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::LanServers(_, searching)) = &mut self.mode else { unreachable!() };
        match key {
            Key::Accept | Key::Back if *searching => *searching = false,
            Key::Back => self.mode = self.icon_menu(MenuKind::Main),
            _ => {}
        }
        Ok(())
    }
}
