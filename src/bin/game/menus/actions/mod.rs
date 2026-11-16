mod in_game;
mod main_options;
mod rewards;
mod shops;
mod tuning;

use crate::*;

impl Game {
    /// The option actions (each option's vtable action; see screens).
    pub(crate) fn act(&mut self, action: Action, kind: MenuKind) -> Result<()> {
        // The pause menu's Photo Mode (opt-in), over the paused world.
        if action == Action::PhotoMode {
            self.photo_open();
            return Ok(());
        }
        let now = self.now();
        let confirmed = std::mem::take(&mut self.fe.confirmed);
        // A mode's option in UI_PerformanceTuning_Setting (0x564EB0) asks
        // first: "Tune settings for this race mode?" (0x6BB7037E; package
        // by length, 0x5587A0 with ""), Yes 0xD05FC3A3 (the accept below),
        // No 0x34DC1BCF (0x538CB0: back to the modes), No focused (seen
        // under Wine, menus112 d4).
        if !confirmed && let Action::TuneMode(_) = action {
            let back = std::mem::replace(&mut self.mode, Mode::Explore);
            let mut ask = self.ask(0x6BB7_037E, Mode::Menu(Menu::LotDeal(LotDeal::Confirm(action, kind))), back);
            if let Mode::Menu(Menu::Dialog(d, _)) = &mut ask {
                d.selected = 1;
            }
            // Over the mode screen, StartDim by 0x96DDA9E6 (0x564F06): its
            // handler's DIM has no event, so it stays as it was (menus112 d4).
            self.mode = dialog_over(ask, true, None);
            return Ok(());
        }
        if !confirmed && let Some(question) = self.in_game_question(action, kind) {
            // Cancel leaves the menu as it was (the dialog is pushed over it).
            let back = match std::mem::replace(&mut self.mode, Mode::Explore) {
                m @ Mode::Menu(Menu::Icons(_, k)) if k == kind => m,
                other => {
                    self.mode = other;
                    self.icon_menu(kind)
                }
            };
            self.mode = self.in_game_dialog(question, Mode::Menu(Menu::LotDeal(LotDeal::Confirm(action, kind))), back);
            return Ok(());
        }
        if lines_purged_by(action, kind)
            && let Some(a) = self.audio.as_mut()
        {
            a.streams.purge_lines(&a.mixer);
        }
        let next = match action {
            // Handled at the top.
            Action::PhotoMode => return Ok(()),
            Action::Unavailable | Action::Inert => return Ok(()),
            Action::Career
            | Action::QuickRace
            | Action::Online
            | Action::Lan
            | Action::ProfileManager
            | Action::Options
            | Action::PauseSetting(_)
            | Action::Setting(_)
            | Action::QrMode(_) => self.act_main_options(action, kind, now)?,
            Action::NewCareer
            | Action::Sign(_)
            | Action::ResumeCareer
            | Action::LoadCareer
            | Action::ProfileCreate
            | Action::Save
            | Action::ProfileSave
            | Action::ProfileLoad
            | Action::ProfileDelete
            | Action::Explore
            | Action::WorldMap
            | Action::CarSelect => self.act_profile(action, kind, now)?,
            Action::Customize
            | Action::CustomizeCategory(_)
            | Action::ChangeParts
            | Action::RimBrand(_)
            | Action::Paint
            | Action::Vinyls
            | Action::Decals
            | Action::PurchaseSetup
            | Action::VinylLayer(_)
            | Action::PerfCategory(_)
            | Action::PerfPackage(_)
            | Action::PerfStock
            | Action::PerfParts => self.act_shops(action)?,
            Action::TestRunRestart
            | Action::ReturnToDyno
            | Action::Tuning
            | Action::TuneSettings
            | Action::TestRun
            | Action::DynoRun => self.act_dyno(action, kind, now)?,
            Action::ModeSelect
            | Action::TuneMode(_)
            | Action::TuningCategory(_)
            | Action::TuningSave
            | Action::QuitTuning => self.act_tuning(action, now)?,
            Action::Rewards | Action::Magazines | Action::DvdCovers | Action::Sponsors | Action::Cover(_) => {
                self.act_rewards(action)?
            }
            Action::MainMenu
            | Action::QuitToMainMenu
            | Action::ReturnToExplore
            | Action::ReturnToFreeRun
            | Action::JumpToGarage
            | Action::ResumeRace
            | Action::Restart
            | Action::ReturnToWorldMap
            | Action::RaceStats
            | Action::NextRace => self.act_in_game(action, kind, now)?,
            Action::Replay => self.act_replay(action, kind)?,
        };
        // An arm that set the mode itself (or left it) returns `None`.
        if let Some(mode) = next {
            self.mode = mode;
        }
        Ok(())
    }
}

/// Whether a menu action purges the NIS and speech lines (0x481210 with 3
/// / 6 / 9 → 0x46C9D0 on the speech object's +8 and +4; **verified** in
/// the code, the post-race exits **observed** under Wine). UI_PostRace's
/// destructor (0x4E9540) calls 0x481210(3), so every way out of the
/// post-race menu does — not Race Stats or Replay, whose screens follow
// prov: open game.lines_purged_by
/// it (whether they close it is **open**). The pause (flow state 6,
/// 0x606740) leaves by message 8 with UI_Pause's reason (0x4C2170 →
/// 0x5F4250): Restart (0xE1A57D51) 3; Quit to Main Menu (0x4A7CF0), Back
/// to World Map (0x4A7CC0) and Jump to Garage (0x4A7C70) 6; Resume (1)
/// lets the held lines go on. Return to Explore Mode (0x451E768E) is
/// message 0xB, 0x481210(9).
pub(crate) fn lines_purged_by(action: Action, kind: MenuKind) -> bool {
    match kind {
        MenuKind::PostRace => matches!(
            action,
            Action::ReturnToExplore
                | Action::ReturnToFreeRun
                | Action::Restart
                | Action::ReturnToWorldMap
                | Action::QuitToMainMenu
                | Action::MainMenu
                | Action::NextRace
        ),
        MenuKind::RacePause | MenuKind::ExplorePause => {
            matches!(
                action,
                Action::Restart
                    | Action::QuitToMainMenu
                    | Action::MainMenu
                    | Action::ReturnToWorldMap
                    | Action::JumpToGarage
                    | Action::ReturnToExplore
                    | Action::ReturnToFreeRun
            )
        }
        _ => false,
    }
}
