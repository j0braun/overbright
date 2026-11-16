use crate::*;

impl Game {
    /// UI_PC_Help_Bar's mask and the screen's opening time for the screen
    /// up: UI_Main's constructor turns the bar on (0x54E6B0 at 0x4DF2ED;
    /// the career's screens' constructors too, e.g. the garage 0x4C2170,
    /// the car lot 0x4F9930, the SMS screens 0x497850 / 0x4E1BF0) and
    /// every screen's base constructor (0x551DF0) sets it to the screen
    /// record's mask (0x54E6F0). The in-game World Map draws its own
    /// (`InGameWorldMap::quads`).
    pub(crate) fn help_screen(&self) -> Option<(u32, f32)> {
        let Mode::Menu(menu) = &self.mode else { return None };
        let (mask, opened) = match menu {
            // UI_ChooseVinylLayer's refresh (0x5535E0): a layer with a
            // vinyl (setup or car, 0x521680 / 0x610300) that takes colours
            // (REMAP 0x39DD714), no unique one on (0x521590 / 0x5215E0),
            // adds 0x800000 (Repaint, 0x54E6F0 at 0x5536B4).
            Menu::Icons(m, MenuKind::VinylLayer) => {
                let layer = 0x40 + m.selected as u8;
                let unique = self.fitted_unique() || self.setup_unique();
                let remap = self
                    .effective(layer)
                    .filter(|&v| v != 0)
                    .and_then(|v| self.part_attr(0x40, v, graphics::REMAP))
                    .is_some_and(|r| r != 0);
                (
                    screens::help_mask("UI_ChooseVinylLayer").map(|k| if remap && !unique { k | 0x80_0000 } else { k }),
                    m.opened,
                )
            }
            // Choose Individual Parts pushes UI_BuyPerformanceParts (0x5620E0),
            // whose mask the bar shows: Install Part (under Wine).
            Menu::Icons(m, MenuKind::PerfPackage) if m.selected == 0 && self.shop.perf_parts.is_some() => {
                (screens::help_mask("UI_BuyPerformanceParts"), m.opened)
            }
            // The package screen's refresh (0x550187…0x5501DF) sets the
            // bar per option: 0x409 on the first, 0x405 (Continue) on Return
            // To Stock (label 0xED55C635), else 0x441 (Install Package)
            // (Wine, the finished-career save: "Continue" on Return To Stock).
            Menu::Icons(m, MenuKind::PerfPackage)
                if m.options.get(m.selected).is_some_and(|o| o.label == 0xED55_C635) =>
            {
                (Some(0x405), m.opened)
            }
            // UI_PerformanceDyno_MAIN's refresh (0x5573ED): 0x4 (Continue
            // alone) while 0x83A7A4 is set, the car on the dyno track (the
            // session's entry sets it, 0x60B52C; Dyno Run 0x53C840 again),
            // else 0x404 (Wine, the finished-career save's dyno from IND_PERF_SHOP_1:
            // prov: inferred game.help_screen
            // Continue only). **Inferred**: a paused test run is the state
            // with it clear.
            Menu::Icons(m, MenuKind::Dyno) if self.dyno.tuning_session && self.sim.paused_race.is_none() => {
                (Some(0x4), m.opened)
            }
            Menu::Icons(m, kind) => (screens::help_mask(kind.package()), m.opened),
            Menu::QuickRace(qr) => (screens::help_mask("UI_QRTrackSelect"), qr.opened),
            Menu::QrOptions(o) => (screens::help_mask("UI_QRModeOptions"), o.opened),
            Menu::CarLot(lot) => (lot.help_mask(), lot.opened),
            Menu::Parts(b) => (b.help_mask(), b.opened),
            Menu::Paint(p) => (p.help_mask(), p.opened),
            Menu::ChooseHud(c) => (screens::help_mask("UI_ChooseCustomHUD"), c.opened),
            Menu::Options(o) => {
                (screens::help_mask(if o.pause.is_some() { "UI_PauseOptions" } else { "UI_Options" }), o.opened)
            }
            Menu::Results(r, _) => (
                screens::help_mask(match r.kind {
                    screens::ResultsKind::Race => "UI_PostRaceResults",
                    screens::ResultsKind::Tournament => "UI_PostRace_TournResults",
                    screens::ResultsKind::Standings { .. } => "UI_PostRace_TournStandings",
                }),
                r.opened,
            ),
            Menu::Reward(r) => (screens::help_mask("UI_PostRaceReward"), r.opened),
            Menu::Stats(st) => (screens::help_mask("UI_PostRaceStats"), st.opened),
            Menu::SponsorPopup(opened) => (screens::help_mask("UI_SponsorPopup"), *opened),
            Menu::SmsViewer(v) => (screens::help_mask("UI_EngageMessageDialog"), v.opened),
            Menu::Controls(c) => (screens::help_mask("UI_Options_PC_Controller"), c.opened),
            Menu::Jukebox(j) => (screens::help_mask("EA_Trax_Jukebox"), j.opened),
            Menu::Credits(c) => (screens::help_mask("Credits"), c.opened),
            Menu::BrandSelect(b, _) => (screens::help_mask("UI_PerformanceBrandSelect"), b.opened),
            Menu::DynoChart(d) => (screens::help_mask("UI_PerformanceDyno_Chart"), d.opened),
            Menu::DynoResults(d) => (screens::help_mask("UI_PerformanceDyno_Results"), d.opened),
            Menu::TuneSliders(t) => (screens::help_mask(t.package), t.opened),
            Menu::TuneGraph(t) => (screens::help_mask(t.package), t.opened),
            Menu::WorldMap(w) => (w.help_mask(), w.opened),
            Menu::Status(st) => (screens::help_mask("UI_Status_Master"), st.screen.opened),
            Menu::SmsMailbox(m) => (screens::help_mask("UI_SMS_Mailbox"), m.opened),
            // The search dialog (GenericDialog, pushed by the constructor)
            // stands over the bar (Wine).
            Menu::LanServers(opened, false) => (screens::help_mask("UI_PC_LAN_ServerSelect"), *opened),
            // In game mode 8 with the network up (0x8654A4 = 3) 0x54E6F0
            // turns bit 0x1 (Quit Game) into 0x2 (Logoff): Back, Continue,
            // Logoff under Wine. The connect dialog hides the bar; the
            // lost-connection notice leaves it (Wine).
            Menu::EaLogin(l, None) => (screens::help_mask("UI_OLEALogin").map(|m| (m & !1) | 2), l.opened),
            Menu::Dialog(d, outcomes) if d.buttons.len() == 1 => match d.under.and_then(|i| outcomes.get(i)) {
                Some(Mode::Menu(Menu::EaLogin(l, None))) => {
                    (screens::help_mask("UI_OLEALogin").map(|m| (m & !1) | 2), l.opened)
                }
                _ => (None, 0.0),
            },
            _ => (None, 0.0),
        };
        Some((mask?, opened))
    }

    /// A dialog up with the help bar left on under it (the EA Login's
    /// lost-connection notice, `help_screen`): the bar is a package below
    /// the dialog's on the FE stack, so it is drawn before the dialog and
    /// its backdrop darkens it (Wine, round 3C ol_lost: the labels at
    /// 0.40 of white; the clone drew the bar over the dialog, undimmed).
    pub(crate) fn help_under_dialog(&self) -> bool {
        matches!(&self.mode, Mode::Menu(Menu::Dialog(..))) && self.help_screen().is_some()
    }

    /// UI_PC_Help_Bar under the screen up (`help_screen`), with the profile
    /// name in RealGroup; in the career (game mode 1) the name and the cash
    /// take turns (0x537E70, `screens::HelpCycle`).
    pub(crate) fn help_bar_quads(&self, now: f32) -> Vec<Quad> {
        let Some((mask, opened)) = self.help_screen() else { return Vec::new() };
        let name = self.profile.as_ref().map(|p| p.name.as_str()).or(self.fe.help_name.as_deref());
        let cash = self.profile.as_ref().map(|p| nfsu2::frontend::grouped(p.cash as i64)).unwrap_or_default();
        let career = self.fe.help_cycle.filter(|_| self.race_flow.game_mode == 1).map(|c| (cash.as_str(), c));
        let vinyl_layer = matches!(self.mode, Mode::Menu(Menu::Icons(_, MenuKind::VinylLayer)));
        // 0x52D340 reads g_IsDynoRun (0x89E7E6, the tuning session) for
        // 0x800's label: "Reset to default" there (Wine 2026-11-01, every
        // tuning screen), "Reset Keys To Default" elsewhere.
        screens::help_bar_in(
            &self.game_data.assets,
            &screens::help_labels_in(mask, self.dyno.tuning_session, vinyl_layer),
            mask,
            name,
            career,
            now,
            opened,
            self.fe.help_hover,
        )
    }
}
