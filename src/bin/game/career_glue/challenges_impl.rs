use crate::*;

impl Game {
    /// The world challenges on offer (career +0x7148, 0x527D40).
    pub(crate) fn challenges_on_offer(&self) -> Vec<usize> {
        self.profile.as_ref().map_or_else(Vec::new, |p| p.challenges_on_offer(&self.game_data.career, self.career_world.challenge.is_some()))
    }

    /// The running challenge's record.
    pub(crate) fn challenge_record(&self) -> Option<&nfsu2::frontend::sms::MagTrigger> {
        self.career_world.challenge.and_then(|c| self.game_data.career.mag_triggers.get(c.index))
    }

    /// The marker table's world-challenge entries (0x533320's last loop,
    /// type 15, 0x500DE0): each challenge on offer at the route trigger its
    /// +0x02 names (0x5D9C80 over kind 0xE000), none while one runs.
    pub(crate) fn challenge_markers(&self) -> Vec<(usize, career::Trigger)> {
        self.challenges_on_offer()
            .into_iter()
            .filter_map(|i| self.sim.route_triggers.find(self.game_data.career.mag_triggers[i].marker).map(|t| (i, t)))
            .collect()
    }

    /// While a challenge runs the marker table holds its goal alone
    /// (0x533320 with career +0xC08 → 0x515060 → 0x500EA0): its shop
    /// (0x4FFE20 by the goal hash) at the shop's trigger (+0x3C, 0x4FFC50),
    /// else its milestone (0x4FFF80) at the photo spot (+0x2C).
    pub(crate) fn challenge_goal_marker(&self) -> Option<(MarkerTarget, career::Trigger)> {
        let t = self.challenge_record()?;
        if let Some(s) = self.game_data.career.shops.iter().position(|s| s.hash == t.milestone) {
            return self.sim.route_triggers.find(self.game_data.career.shops[s].trigger).map(|tr| (MarkerTarget::Shop(s), tr));
        }
        let m = self.game_data.career.milestones.iter().find(|m| m.hash == t.milestone)?;
        self.sim.route_triggers.find(m.spot).map(|tr| (MarkerTarget::Shoot(m.hash), tr))
    }

    /// Whether a marker is the running challenge's goal (0x532A60: a shop
    /// whose +0x38, or a milestone whose +0x20, is the record's +0xC).
    pub(crate) fn challenge_goal(&self, m: MarkerTarget) -> bool {
        let Some(t) = self.challenge_record() else { return false };
        match m {
            MarkerTarget::Shop(s) => self.game_data.career.shops[s].hash == t.milestone,
            MarkerTarget::Shoot(h) => h == t.milestone,
            _ => false,
        }
    }

    /// A world challenge starts (the dialog's Accept, 0x498F00 → 0x541130):
    /// the GPS off (0x4131F0, 0x838590 = 0), then 0x533BF0: the prompt
    /// removed (0x4DB820), the car's heading kept (+0x118), the failed flag
    /// clear, the record at +8, state 1, its name's flag +6 = 2, the clock
    /// at the record's +0x10 seconds, its goal milestone offered (state +4
    /// = 2: the magazine offer), the marker table rebuilt (0x533320: only
    /// the goal's marker while it runs, 0x515060).
    pub(crate) fn start_challenge(&mut self, index: usize) {
        let Some(t) = self.game_data.career.mag_triggers.get(index).cloned() else { return };
        self.career_world.gps = None;
        self.career_world.marker = None;
        let heading = self.sim.drive.as_ref().map_or(0.0, |d| d.state.heading());
        self.career_world.challenge = Some(WorldChallenge { index, time: t.time as f32, state: 1, failed: false, heading });
        if let Some(p) = self.profile.as_mut() {
            p.start_challenge(&self.game_data.career, &t);
            // Career +0xC08 set, then 0x533320 → 0x527D40: the open
            // events emptied (Wine, the original's career at stage 2: 16 → 0 on
            // STAGE_2_SHOW_1's Accept), their markers and triggers gone.
            p.challenge_running = true;
            p.refresh_open(&self.game_data.career, &mut self.rng);
        }
        self.refresh_event_triggers();
    }

    /// Career +0xC08 cleared by a challenge's end (0x541970 / 0x5419D0,
    /// then 0x533320 at 0x533741 / 0x5419B7 / 0x541A0E) or give-up
    /// (0x533D00 state 7, 0x533D17): the open events rebuilt from an empty
    /// list, every slot drawn anew (Wine: S2_CIRCUIT_3 → 4; in another
    /// session S2_CIRCUIT_2 → 5, S2_DRIFT_3 → 2).
    fn challenge_over(&mut self) {
        if let Some(p) = self.profile.as_mut() {
            p.challenge_running = false;
            p.refresh_open(&self.game_data.career, &mut self.rng);
        }
        self.refresh_event_triggers();
    }

    /// The challenge's clock (0x5016A0, from the career's tick 0x551330):
    /// in state 1 the frame time comes off; at or under 0, 0x501600: the
    /// failed flag, state 6, flow event 0x11 → 0x498D00, which with the
    /// flag opens UI_EngageShopDialog's retry.
    pub(crate) fn tick_challenge(&mut self, dt: f32) {
        let Some(c) = self.career_world.challenge.as_mut().filter(|c| c.state == 1) else { return };
        c.time -= dt;
        if c.time <= 0.0 {
            c.failed = true;
            c.state = 6;
            self.career_world.marker = None;
            let opened = self.now();
            self.mode = Mode::Menu(Menu::Challenge(screens::ChallengeDialog {
                kind: screens::ChallengeDialogKind::Retry,
                opened,
                focus: Default::default(),
                from: String::new(),
            }));
        }
    }

    /// The retry dialog's answer (0x499690 with the failed flag; the
    /// dialog's destructor 0x4993E0 → 0x533D00). Retry (Accept, 0x5150D0):
    /// state 8, the car put back at the start marker's trigger (+4, +8,
    /// +0x14) facing the start's heading (0x5FBD70), then 0x533BF0 starts
    /// it again. Cancel or Back (0x501640): state 7 and the goal
    /// milestone's offer withdrawn (+4 = 0), then the record is cleared
    /// (the challenge stays on offer: it is not done).
    pub(crate) fn answer_retry(&mut self, retry: bool) -> Result<()> {
        let Some(c) = self.career_world.challenge else { return Ok(()) };
        let Some(t) = self.game_data.career.mag_triggers.get(c.index).cloned() else { return Ok(()) };
        if retry {
            let at = self.sim.route_triggers.find(t.marker).map(|tr| (tr.position, c.heading));
            self.explore(at.or(self.here()))?;
            self.start_challenge(c.index);
        } else {
            if let Some(p) = self.profile.as_mut() {
                p.abandon_challenge(&t);
            }
            self.career_world.challenge = None;
            self.challenge_over();
            self.explore(self.here())?;
        }
        Ok(())
    }

    /// 0x5139C0 / 0x513B30 (the quit to the main menu, Jump to Garage, the
    /// pause's Back to World Map): with a world challenge running (career
    /// +0xC08) its goal milestone's offer is withdrawn (0x5003B0 → +4 = 0)
    /// and the record and state (+0xD1C) are cleared — the challenge stays
    /// on offer, not done. **Verified** under Wine (variants round,
    /// 2026-10-31): STAGE_2_SHOW_1 taken (0x85B948 set, 0x85BA5C = 1), the
    /// pause's Quit to Main Menu left both 0.
    pub(crate) fn drop_challenge(&mut self) {
        let Some(c) = self.career_world.challenge.take() else { return };
        if let (Some(t), Some(p)) = (self.game_data.career.mag_triggers.get(c.index).cloned(), self.profile.as_mut()) {
            p.abandon_challenge(&t);
            // +0xC08 cleared; no rebuild here (0x5139C0 / 0x513B30 do
            // not call 0x533320): the list stays empty until the next
            // city start (0x5412E0) rebuilds it.
            p.challenge_running = false;
            let _ = p.commit();
        }
        self.career_world.marker = None;
    }

    /// The challenge is done (0x541970 in state 3, from the milestone's
    /// photo shoot, 0x498E70 / 0x4CE770; 0x5419D0 from the unique-part
    /// screen): state 5, its name's flag +6 = 1, state 0, the record
    /// cleared, the markers rebuilt (0x533320).
    pub(crate) fn finish_challenge(&mut self) {
        let Some(c) = self.career_world.challenge.take() else { return };
        if let (Some(t), Some(p)) = (self.game_data.career.mag_triggers.get(c.index), self.profile.as_mut()) {
            p.touch(t.name);
            p.challenges_done.insert(t.name);
        }
        self.challenge_over();
    }
}
