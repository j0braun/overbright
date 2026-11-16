use crate::*;

impl Game {
    /// A one-button notice on the game's dialog.
    /// The package by 0x526C50's size rule (`screens::dialog_package`):
    /// the request names none (0x783FE5), in the front end and in game.
    pub(crate) fn dialog(&self, text: &str, then: Mode) -> Mode {
        let package = screens::dialog_package(&self.game_data.assets, text);
        Mode::Menu(Menu::Dialog(Dialog::new(package, text, &[&self.label(0x0639, "OK")], self.now()), vec![then]))
    }

    /// The question an in-game option asks first on UI_InGameDialog
    /// (**verified**, 2026-10-29: 0x5587A0 with "UI_InGameDialog.fng",
    /// OK 0x0639 / Cancel 0x541389E5, the focus on Cancel; OK sends the
    /// option's message, Cancel 0xB4623F67). UI_pause's handlers: Back to
    /// World Map 0x4C2560, Restart 0x4C25D0, Jump to Garage 0x4C2730,
    /// Return to Explore Mode in a race 0x4C2780, Quit to Main Menu
    /// 0x4C2820; each picks the "All unsaved race progress will be lost"
    /// text while the race camera (0x8900D8) is in state 6 with +0x24 = 4,
    /// which holds in free roam too (the original under Wine asks "Quit to
    /// Main Menu?^All unsaved race progress will be lost." from the explore
    /// pause). UI_postrace's: Return to Explore Mode 0x4BEC20 (only while
    /// the tournament has a race left, 0x503080; else straight on), Restart
    /// 0x4BECC0, Quit to Main Menu 0x4BED40, Back to World Map 0x4BEDC0.
    /// The explore pause's Return to Explore Mode resumes at once.
    pub(crate) fn in_game_question(&self, action: Action, kind: MenuKind) -> Option<u32> {
        let pause = matches!(kind, MenuKind::ExplorePause | MenuKind::RacePause);
        let post = kind == MenuKind::PostRace;
        if !pause && !post {
            return None;
        }
        Some(match action {
            Action::ReturnToWorldMap => {
                if post {
                    0x33C4_2E03
                } else {
                    0xE761_C8B8
                }
            }
            Action::Restart => 0xAA50_AA02,
            Action::JumpToGarage => 0xC9BA_9E3E,
            Action::ReturnToExplore if kind == MenuKind::RacePause => 0x07CD_17B0,
            Action::ReturnToExplore if post && self.race_flow.next_round.is_some() => 0x6C32_8734,
            // Quit to Main Menu: its own branch in `act` (the quit's save).
            _ => return None,
        })
    }

    /// UI_InGameDialog: the question with OK / Cancel, Cancel focused.
    pub(crate) fn in_game_dialog(&self, question: u32, ok: Mode, cancel: Mode) -> Mode {
        let text = self.label(question, "?");
        let buttons = [self.label(0x0639, "OK"), self.label(0x5413_89E5, "Cancel")];
        let mut d = Dialog::new("UI_InGameDialog", &text, &[&buttons[0], &buttons[1]], self.now());
        d.selected = 1;
        // Over the pause / Post Race, which dims (0x96DDA9E6 after the ask:
        // 0x4C25BE … 0x4C28E0, 0x4BEC69 … 0x4BEDFB); Cancel's 0x251BF83E
        // (0x4C230D) brings it back.
        dialog_over(Mode::Menu(Menu::Dialog(d, vec![ok, cancel])), true, None)
    }

    /// A Yes/No question on the game's dialog (0x5587A0 with "Yes"
    /// 0x88579775 and "No" 0xBE503981).
    /// A Yes / No question sized by 0x526C50 (the request names no
    /// package, as at 0x55A1EB, 0x4E0FE0, 0x4EDE8B, 0x55A5DD, 0x49C8C2).
    pub(crate) fn ask(&self, question: u32, yes: Mode, no: Mode) -> Mode {
        let text = self.label(question, "?");
        let package = screens::dialog_package(&self.game_data.assets, &text);
        self.ask_in(package, question, yes, no)
    }

    /// A Yes / No question on a package the request names
    /// (GenericDialog_SMALL.fng, 0x78FA24: 0x4AF63F, 0x4AF7DF, 0x49B9A1).
    /// The focused button is the request's (`ask_focus`).
    pub(crate) fn ask_in(&self, package: &'static str, question: u32, yes: Mode, no: Mode) -> Mode {
        let text = self.label(question, "?");
        let buttons = [self.label(0x8857_9775, "Yes"), self.label(0xBE50_3981, "No")];
        let mut d = Dialog::new(package, &text, &[&buttons[0], &buttons[1]], self.now());
        d.selected = ask_focus(question);
        Mode::Menu(Menu::Dialog(d, vec![yes, no]))
    }
}
