use super::TickControls;
use crate::*;

impl Game {
    /// Rachel's front-end lines of the shop screens' INITs, then their
    /// save-point snapshot.
    pub(super) fn tick_fe_lines(&mut self) {
        // The shop screens' INIT (0x35F8620B) run Rachel's front-end lines
        // (0x5243E0, `Hints::fe_line`): the car lot 4 (0x4EDACB), the crib
        // 0x11 (0x4AF502), UI_ChoosePerformanceCategory 1 (0x55C68B),
        // UI_ChoosePaintCategory 0xF (0x562D18), UI_ChooseVinylLayer 0xE
        // (0x56AB6A), UI_DecalMain 5 (0x56B648), UI_ICEMAIN 0x12 with
        // 0x838998 = 0 (0x568995), UI_GenericParts_Browser 2 in the body
        // shop (0x838990 = 0, 0x566EEE); UI_PerformanceTuning_Setting 0x1B
        // on its first tick (+0x154, 0x55AA3C). Called each frame here, latched to the
        // screen's opening (`SaveFlow::fe_screen`), as the INIT does.
        let fe_kind = match &self.mode {
            Mode::Menu(Menu::CarLot(_)) => Some(4),
            Mode::Menu(Menu::Icons(_, MenuKind::Crib)) => Some(0x11),
            Mode::Menu(Menu::Icons(_, MenuKind::PerfCategory)) => Some(1),
            Mode::Menu(Menu::Icons(_, MenuKind::Graphics)) => Some(0xF),
            Mode::Menu(Menu::Icons(_, MenuKind::VinylLayer)) => Some(0xE),
            Mode::Menu(Menu::Icons(_, MenuKind::TuneMode)) => Some(0x1B),
            // UI_Paint's INIT (handler 0x563420 on 0x35F8620B, 0x56351A):
            // kind 8 when the category option 0x83899C is 2 (Paint; the
            // vinyls' colours run UI_Paint with 0).
            Mode::Menu(Menu::Paint(p)) if p.vinyl.is_none() => Some(8),
            Mode::Menu(Menu::Parts(b)) if b.package == "UI_DecalMain" => Some(5),
            Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.title == 0xDF86_227A => Some(0x12),
            Mode::Menu(Menu::Parts(b))
                if b.package == "UI_GenericParts_Browser" && self.visited_shop_kind() == Some(2) =>
            {
                Some(2)
            }
            _ => None,
        };
        // 0x5243E0 runs once per INIT (the screen opening), not every
        // frame: a second call on the crib's first visit stamps the kind's
        // time (+0x120 + 0x11 * 4), which the first call leaves alone
        // (0x5244F0's early return), after the snapshot — and the Yes asks
        // to save. A dialog or memory-card message over the
        // screen is a pushed package: no new INIT when it closes.
        let init = fe_kind.is_some() && self.save_flow.fe_screen != fe_kind;
        if fe_kind.is_some() {
            self.save_flow.fe_screen = fe_kind;
        } else if !matches!(
            self.mode,
            Mode::Menu(Menu::Dialog(..) | Menu::McMain(_) | Menu::McList(_) | Menu::McMessage(..))
        ) {
            self.save_flow.fe_screen = None;
        }
        let line = match &self.mode {
            _ if init => {
                let shop = self
                    .shop.visit
                    .and_then(|h| self.game_data.career.shops.iter().find(|s| s.hash == h))
                    .map(|s| (s.kind, s.hash));
                let career = self.save_flow.customize.is_none() && !self.race_flow.quick_free_run;
                self.profile.as_mut().filter(|_| career).and_then(|p| p.hints.fe_line(fe_kind.unwrap_or(0), shop))
            }
            // UI_ShowCase_Preview's INIT (0x4CF170 → 0x525860 at 0x4CF452).
            Mode::Menu(Menu::Showcase(_)) => self.profile.as_mut().and_then(|p| p.hints.showcase_line()),
            _ => None,
        };
        self.say(line);
        // Then those INITs take the save-point snapshot (0x52A470(0)): the
        // car lot (0x4EDADE), the crib (0x4AF515), the performance and
        // paint categories (0x55C697, 0x562D24), the parts browser
        // (0x566EFA) and UI_ICEMAIN (0x5689A1) — after the line, as there.
        if matches!(fe_kind, Some(4 | 0x11 | 1 | 0xF | 0x12 | 2)) && self.save_flow.customize.is_none() {
            self.take_snapshot(false);
        }
    }

    /// Rachel's line of an option's Select (not an INIT): the
    /// performance category's item (vtable 0x79B5C4, its action 0x53B780
    /// on 0x0C407210: 0x838994 = the item's +0x48, the category, then
    /// 0x525050) and UI_PerformanceTuning_Master's (handler 0x55D920 on
    /// 0x0C407210 with the item's +0x22 clear: 0x83A648 = +0x48, then
    /// 0x525110 at 0x55D9CC). Only an open item gets there (0x543D50
    /// skips a locked one, +0x22). 0x5243E0 says nothing outside a career
    /// (0x850078 = 1) and stamps the kind's time (hints +0x120 + 4·kind),
    /// after the screen's snapshot: on a first visit the shop's leave
    /// then asks to save.
    pub(crate) fn option_line(&mut self, kind: u32) {
        let shop = self
            .shop.visit
            .and_then(|h| self.game_data.career.shops.iter().find(|s| s.hash == h))
            .map(|s| (s.kind, s.hash));
        let career = self.save_flow.customize.is_none() && !self.race_flow.quick_free_run;
        let line = self.profile.as_mut().filter(|_| career).and_then(|p| p.hints.fe_line(kind, shop));
        self.say(line);
    }

    /// UI_ShowCase_Preview's frame (or what its leave shut), then the
    /// car's part poses and glow.
    pub(super) fn tick_showcase(&mut self, dt: f32, input: &TickControls) {
        let TickControls { controls } = *input;
        // UI_ShowCase_Preview's frame; its leave (0x499FA0 → 0x499F10)
        // shuts what Show Off left open, and the car's part animations run
        // on wherever it drives.
        if let Mode::Menu(Menu::Showcase(v)) = &mut self.mode {
            v.save_tick();
            let (mode, camera) = (v.mode, v.picks.camera);
            let (milestone, dvd) = (v.milestone, v.dvd);
            // The picture is taken by `Game::draw` once the ticks are up.
            // Headless (no window: the selftest, the playthrough bot) no
            // frame is ever drawn, so the request resolves as a failed
            // save ("Save failed.", Continue) instead of waiting for good.
            if v.save_ticks == Some(0) && self.platform.window.is_none() {
                self.mode = self.save_picture(milestone, dvd, None);
            } else {
                self.showcase_frame(mode, camera, controls, dt, self.now() / 1000.0);
            }
        } else if self.showcase.shut.iter().any(|&o| o) && !matches!(self.mode, Mode::Menu(Menu::Dialog(..))) {
            let now = self.now() / 1000.0;
            let anims = self.sim.drive.as_ref().and_then(|d| d.car.animations.as_ref());
            let styles = self.sim.drive.as_ref().map(|d| d.car.part_styles).unwrap_or_default();
            for (flag, channels) in [(0, &[0usize][..]), (1, &[2, 3][..]), (2, &[1][..])] {
                if self.showcase.shut[flag] {
                    for &c in channels {
                        self.showcase.parts.request(anims, &styles, c, false, 1.0, now);
                        Self::part_stitch(self.audio.as_mut(), &mut self.rng, c, false);
                    }
                }
            }
            self.showcase.shut = [false; 3];
        }
        let anim_now = self.now() / 1000.0;
        if !matches!(self.mode, Mode::Menu(Menu::Showcase(_)))
            && let Some(d) = self.sim.drive.as_mut()
            && let Some(cr) = self.render.cars.get_mut(&d.car_id)
        {
            cr.set_part_poses(self.showcase.parts.poses(d.car.animations.as_ref(), anim_now));
            cr.set_part_glow(part_glow(&d.car, anim_now));
        }
    }
}
