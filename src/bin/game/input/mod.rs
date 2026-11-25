use crate::*;

mod dispatch;
mod hydraulics;
mod keys;
pub(crate) mod mouse;
mod pads;
mod queue;
mod replay_keys;

pub(crate) use keys::*;
pub(crate) use queue::{InputEvent, InputQueue};

/// Whether a force-feedback device is attached (0x5B8250 asks the input
/// layer) for the developer shots and the selftest, which run without
/// one: the Feedback option is disabled. The game asks its pads
/// (`Host::force_feedback`).
pub(crate) const FORCE_FEEDBACK: bool = false;

/// The Controls row of Look Back (DIACTION 9; keys B / K, 0x7FCAC0).
pub(crate) const LOOK_BACK_ROW: usize = 9;

impl Game {
    /// The player's driving inputs: the actions held on the keyboard and
    /// on the controllers, through the human input's JOY handlers
    /// (`nfsu2::pad::driving`: a digital steer wins over the stick, a
    /// digital pedal over the analog ones).
    pub(crate) fn controls(&self) -> Controls {
        if let Some(c) = dev_hooks::replay_controls(self) {
            return c;
        }
        let held = self.held_actions();
        // The car's gear +0x434 = 1 (neutral) for 0x416DE0's throttle.
        let neutral = self.sim.drive.as_ref().and_then(|d| d.state.drivetrain.as_ref()).is_some_and(|t| t.gear == 1);
        let d = nfsu2::pad::driving(held, self.platform.pads.analog(), neutral);
        Controls {
            throttle: d.throttle,
            brake: d.brake,
            handbrake: d.ebrake,
            steer: -d.steer,
            nitrous: held & (1 << nfsu2::pad::act::NITROUS) != 0,
            // A wheel drives: 0x4061C0 with the player's Wheel Range.
            wheel: self.platform.pads.wheel_drives().then_some(self.settings.players[0].wheel_range),
            ..Default::default()
        }
    }

    /// A key let go: C (BUTTON1, its release 0xD871B0A4) ends the purge
    /// test in the Nitrous Purge list (0x568D40: +0x116 = 0, 0x458C40).
    pub(crate) fn key_released(&mut self, code: KeyCode) {
        if code == KeyCode::KeyC && self.purge_list_open() {
            self.purge_test_set(false);
        }
    }

    /// The actions held on the keyboard and the controllers.
    fn held_actions(&self) -> u64 {
        let mut held = self.platform.pads.held();
        for (row, &(_, a)) in screens::CONTROL_ROWS.iter().enumerate() {
            if self.platform.keys.iter().any(|&k| self.is_action(k, row)) {
                held |= 1u64 << a;
            }
        }
        held
    }

    /// "Are you sure you want to quit?" (0x9DE6B485; Quit Game 0x1C101778
    /// → 0x919B1DD9, Cancel 0x541389E5 → 0x42CA53FC, also Back) over the
    /// screen, Cancel focused: the 'Q' key's question (0x558B80) and the
    /// help bar's Quit Game slot's (0x55DE10, mask bit 1).
    pub(crate) fn quit_question(&mut self) {
        let text = self.label(0x9DE6_B485, "Are you sure you want to quit?");
        let buttons = [self.label(0x1C10_1778, "Quit Game"), self.label(0x5413_89E5, "Cancel")];
        let mut d =
            Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&buttons[0], &buttons[1]], self.now());
        d.selected = 1;
        d.under = Some(1);
        let here = std::mem::replace(&mut self.mode, Mode::Explore);
        self.mode = Mode::Menu(Menu::Dialog(d, vec![Mode::Menu(Menu::LotDeal(LotDeal::QuitGame)), here]));
    }

    /// The key is bound to this Controls row, primary or secondary.
    pub(crate) fn is_action(&self, code: KeyCode, row: usize) -> bool {
        key_name(code).is_some_and(|n| self.settings.keys[row].iter().any(|k| k == n))
    }
}
