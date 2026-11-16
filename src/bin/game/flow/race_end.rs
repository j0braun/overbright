/// A career race's outcome kept from its end to the post-race exit
/// (0x54C950, **verified**): Return to Explore Mode (0x4F39A0 0x3411395C
/// or 0x35E413DD → 0x4A3E30 → 0x54C9E0), Back to World Map (0x33C42E03 →
/// 0x4A3DC0 → 0x54CA50) and Quit to Main Menu (0x24DD40CF / 0xD78D7BF1 →
/// 0x4A3E80 → 0x54CA50) run it; Restart (0xE1A57D51 → 0x5F41F0(9)) and a
/// tournament left early (0x451E768E → 0x4A3DF0) do not. It runs once
/// (career +0xD28 set on a win) and not at all for an event already won
/// when the race began (0x532EB0: its visit entry +6 = 1 sets +0xD28 and
/// clears +0xD26).
pub(crate) struct RaceResult {
    pub(crate) event: usize,
    pub(crate) won: bool,
    pub(crate) time: f32,
    /// The bank and reputation (0x541430 pays them only with career +0xD26
    /// set and +0xD27 clear: not for a race started from the World Map's
    /// event list, 0x5325F0).
    pub(crate) reward: (u32, u32),
    pub(crate) pays: bool,
}

/// UI_PostRaceReward's values (0x4A4240): reputation (the event's purse,
/// 0x532E60) and bank (the winning-margin bonus, 0x5330E0).
pub(crate) struct Reward {
    pub(crate) reputation: u32,
    pub(crate) bank: u32,
    pub(crate) name: String,
    pub(crate) opened: f32,
}

/// The race's end as `update_world`'s race phase hands it to
/// `Game::finish_race`: the player's finish time.
pub(crate) struct RaceEnd {
    pub(crate) time: f32,
}
