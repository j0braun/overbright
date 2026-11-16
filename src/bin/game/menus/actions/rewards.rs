use crate::*;

impl Game {
    /// `act`'s rewards group (as `act_main_options`).
    pub(super) fn act_rewards(&mut self, action: Action) -> Result<Option<Mode>> {
        Ok(Some(match action {
            Action::Rewards => self.icon_menu(MenuKind::Rewards),
            // UI_MagazineSelect (0x53DDC0 / 0x53DE00 set 0x838578 to 1 or
            // 2) and UI_RewardsSponsor (0x4AFB40).
            Action::Magazines => self.covers_menu(false, 0),
            Action::DvdCovers => self.covers_menu(true, 0),
            Action::Sponsors => self.signed_sponsors_menu(),
            // 0x53DE70: the cover (+0x28) to 0x837FD8, then UI_MagazineView.
            Action::Cover(i) => {
                let dvd = self.game_data.career.milestones.get(i).is_some_and(|m| m.is_dvd());
                self.open_cover(i, dvd)
            }
            _ => unreachable!("act_rewards: {action:?}"),
        }))
    }
}
