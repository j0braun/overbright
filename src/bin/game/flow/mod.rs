pub(crate) mod endings_ahead;
pub(crate) mod movies;
pub(crate) mod race_end;
pub(crate) mod explore;
pub(crate) mod race_end_impl;
pub(crate) mod race_setup;
pub(crate) mod replay;
pub(crate) mod scenes;
pub(crate) mod menu_tick;

use crate::*;

/// The races' flow around the race itself: restart, tournaments,
/// results and the set-up's globals (were `Game` fields).
pub(crate) struct RaceFlow {
    /// The last race (event, from the World Map, quick) for Restart.
    pub(crate) last_race: Option<(usize, bool, bool)>,
    /// Quick Race's Free Run (explore without a career).
    pub(crate) quick_free_run: bool,
    /// UI_postrace's remembered option, 1-based (its screen record's
    /// +0x10): written on each move like UI_Main's, read when the screen
    /// is built (0x4D71CB → 0x4BEE10, then 0x50B8A0 → the scroller's
    /// vt+0x20 0x538A40: n > 0 selects option n − 1), reset to 0 only by
    /// its Quit to Main Menu (0x4A3E80 → 0x50B8C0(0) on the calling
    /// screen). **Observed** under Wine (stage-3 playthrough): after a
    /// Restart the next Post Race opened on Restart.
    pub(crate) post_race_selection: usize,
    /// g_GameMode (0x850078): 2 from UI_Main's constructor (0x4DF18B), 1
    /// for the career (0x495A3D, 0x4C5033), 0x20 Customize (0x4AED3A), 8
    /// online, 0x200 LAN; 0 before UI_Main.
    pub(crate) game_mode: u32,
    /// The race catch-up globals as the last set-up left them: the option
    /// 0x89E7F8 and the Quick Race difficulty behind the human's bonus
    /// scale 0x89E7FC (`AiTuning::catch_up_scale`; None = 1.0). Only the
    /// Quick Race set-ups write them (0x53F160 …: the mode record's +0x05);
    /// the career's set-up 0x5322F0 leaves them, so a career race runs with
    /// what the boot or the last Quick Race left. **Observed** under Wine:
    /// 1 and 1.0 from the boot's first frames, and 1 in a career race
    /// after Resume Career (docs/race-ai-parity.md); the boot-time writer
    /// is not traced.
    pub(crate) catch_up_globals: (bool, Option<usize>),
    /// A career race's outcome not yet settled: the race's end files a
    /// win in its stage record only (0x54CA70 → 0x513390); the win or loss
    /// handler runs when the player leaves UI_PostRace (0x54C950).
    pub(crate) result: Option<RaceResult>,
    /// Career +0xD27: the race was started from the World Map's event list
    /// (0x5325F0 sets it after 0x5322F0); the flow's track switch for
    /// Restart (9), Next Race (10) and a race's start (16) runs 0x5322F0
    /// again (0x581830 → 0x58193A), which clears it.
    pub(crate) map_start: bool,
    /// Career +0x8730: the career event the race was started for (set by
    /// 0x541170, cleared by the exits 0x54C9E0 / 0x54CA50 / 0x513950),
    /// which Return to Explore Mode's movie test reads (0x513A50).
    pub(crate) event: Option<usize>,
    /// A tournament's next race: event, quick, from the map, round, the
    /// times so far.
    pub(crate) next_round: Option<(usize, bool, bool, usize, Field)>,
    /// The last race set up, as Restart races it again ([`RestartRace`]).
    pub(crate) restart: Option<RestartRace>,
    /// The tournament standings that follow its race results.
    pub(crate) standings: Option<PostRaceResults>,
    /// The last race's stats, for the post-race menu's Race Stats.
    pub(crate) stats: Option<screens::PostRaceStats>,
    /// The race endings' files read on a thread since the race started
    /// (`endings_ahead`).
    pub(crate) endings_ahead: Option<std::thread::JoinHandle<endings_ahead::Endings>>,
}

impl Default for RaceFlow {
    fn default() -> RaceFlow {
        RaceFlow {
            last_race: None,
            stats: None,
            quick_free_run: false,
            post_race_selection: 0,
            game_mode: 0,
            catch_up_globals: (true, None),
            result: None,
            map_start: false,
            event: None,
            next_round: None,
            restart: None,
            standings: None,
            endings_ahead: None,
        }
    }
}

/// A race as Restart (0x5F41F0(9)) sets it up again: the event's current
/// race (the round; its record 0x85EF48 stays where it is) with the field
/// it was set up with (`Field`: the line-up, its looks and grid slots, and
/// a tournament's points and times before it).
#[derive(Clone, Debug)]
pub(crate) struct RestartRace {
    pub(crate) event: usize,
    pub(crate) quick: bool,
    pub(crate) round: usize,
    pub(crate) field: Field,
}

impl Game {
    /// The race flow's state [[0x8900D8]] as the SoundFX streams read it
    /// (0x484E82; **verified** under Wine, per frame in
    /// two career circuits): 3 only during the start scene (race status 2,
    /// through the scene frame at mark 2 = header +0x30);
    /// 4 from the countdown's first frame, while racing and in free
    /// roam, and after the player's finish while the race-over hold runs
    /// (≈ 3 s, "You finished …" over the race); 7 once the hold is over
    /// (0x60B3B0 case 3 → 0x603EC0 message 1), staying through the results.
    /// Elsewhere 0 (no SoundFX stream runs there in the clone).
    pub(crate) fn race_flow_state(&self) -> u32 {
        match &self.mode {
            Mode::Race(r)
                if r.timing.clock < 0.0
                    && self.scenes.shop.as_ref().is_some_and(|s| s.frame <= s.scene.marks[1] as f32) =>
            {
                3
            }
            Mode::Race(r) if r.racers.me.finished.is_some() && r.timing.hold.is_some_and(|h| r.over >= h) => 7,
            Mode::Race(_) | Mode::Explore => 4,
            _ => 0,
        }
    }
}
