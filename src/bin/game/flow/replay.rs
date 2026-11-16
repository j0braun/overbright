use crate::*;

impl Game {
    /// The race is over: on to `next`, keeping the race for its replay
    /// (without a recording, its rivals' cars alone, for a Restart or the
    /// Next Race that races the field again: `keep_raced_rivals`).
    pub(crate) fn keep_replay(&mut self, next: Mode) {
        if let Mode::Race(race) = std::mem::replace(&mut self.mode, next) {
            if race.replay.recording.is_some() {
                self.sim.replay = Some(race);
            } else {
                self.sim.replay = None;
                self.keep_raced_rivals(race);
            }
        }
    }

    /// A replay left (0x4A50E0): from the race's pause, back to a new
    /// UI_pause (state 6, 0x5F43E0(6)) on the race as THE_PRESENT left it
    /// (**observed** under Wine: the pause reopens on "Resume Race");
    /// from the post-race menu, back to it (0x537990 to the screen below,
    /// still on Replay) with the world where the replay stood, paused
    /// (manager +0x14 = 1), in the player's camera.
    pub(crate) fn end_replay(&mut self) {
        let Mode::Race(mut race) = std::mem::replace(&mut self.mode, Mode::Explore) else { return };
        let pause = race.replay.playback.take().is_some_and(|p| p.pause);
        if pause {
            self.sim.paused_race = Some(race);
            self.mode = self.icon_menu(MenuKind::RacePause);
            return;
        }
        self.sim.replay = Some(race);
        if let Some(d) = self.sim.drive.as_mut() {
            d.chase = ChaseCamera::new(&d.car, VIEW_ORDER[self.settings.players[0].camera.min(4) as usize]);
        }
        // The menu below comes back as it was, on Replay (**observed**).
        let mut menu = self.icon_menu(MenuKind::PostRace);
        if let Mode::Menu(Menu::Icons(m, _)) = &mut menu
            && let Some(i) = m.options.iter().position(|o| matches!(o.action, Action::Replay))
        {
            m.select_now(i);
        }
        self.mode = menu;
    }
}
