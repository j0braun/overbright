use crate::*;

impl Game {
    /// The game's audio for a debug line: the mode, the music context and
    /// song, the volumes and what the mix holds.
    #[cfg(feature = "dev-harness")]
    pub(crate) fn audio_state(&self) -> String {
        let st = &self.settings;
        let mix =
            self.audio.as_ref().map_or(String::new(), |a| format!("song {:?} | {}", a.streams.song(), a.mixer.state()));
        format!(
            "{} {:?} scene {} | master {} menu {} game {} | {mix}",
            flow_mode(&self.mode),
            self.music_context(),
            self.scenes.shop.is_some(),
            st.master,
            st.menu_music,
            st.game_music
        )
    }

    /// Whether the game's pause holds Rachel's line and the NIS line
    /// (0x4810F0 → 0x46C8D0, released by 0x481210 → 0x46C930): the pause
    /// menus over a game (UI_Pause, its options, a question over them, any
    /// menu over a paused race), a scene's line too. Not the SMS screens
    /// (flow mode 0xD), the World Map (0x12) or the status screen (0x13),
    /// which 0x4810F0 leaves out, nor a movie or a race's countdown.
    pub(crate) fn speech_held(&self) -> bool {
        fn pause_menu(mode: &Mode) -> bool {
            match mode {
                Mode::Menu(Menu::Icons(
                    _,
                    MenuKind::ExplorePause | MenuKind::RacePause | MenuKind::PauseOptions(_),
                )) => true,
                Mode::Menu(Menu::Options(o)) => o.pause.is_some(),
                // A dialog over the pause (Quit to Main Menu?) keeps it.
                Mode::Menu(Menu::Dialog(_, back)) => back.last().is_some_and(pause_menu),
                _ => false,
            }
        }
        self.sim.drive.is_some()
            && (pause_menu(&self.mode)
                || (self.sim.paused_race.is_some()
                    && matches!(&self.mode, Mode::Menu(m) if !matches!(m, Menu::Movie(_) | Menu::InGameMap(_)))))
    }

    /// The music manager's context (0x482350 / 0x482180 / 0x481210):
    /// a movie or an in-engine scene, the flow's pause and countdown, the
    /// front end or the game, and in game whether EA Trax plays (game
    /// music and master on, not a drag or drift race: 0x4018F0 /
    /// 0x401910 on the race flags 0x89E7D8 / 0x89E7D9).
    pub(crate) fn music_context(&self) -> MusicContext {
        self.music_context_of(&self.mode)
    }

    /// The flow's PAUSE state (6, entered through 0x4810F0 by UI_Pause and
    /// the in-game map, a dialog over them included): the NIS and speech
    /// lines are held there (`Streams::hold_speech`; observed under Wine:
    /// both streams' +0x2C set while the race's pause is up, cleared on
    /// Resume).
    #[cfg(feature = "dev-harness")]
    pub(crate) fn lines_held(&self) -> bool {
        // One rule with the speech hold (`speech_held`, applied by
        // `Streams::hold_speech`). **Open**: the crowd trace counted the
        // in-game map as the flow's PAUSE; the SMS trace has 0x4810F0
        // leave out flow mode 0x12. The SMS trace's rule is kept.
        self.speech_held()
    }

    /// The tuning session on the dyno track (race type 8): the music
    /// manager's in-game pass with the front end's pool (`MusicContext::
    /// Dyno`).
    pub(crate) fn dyno_music_session(&self) -> bool {
        self.dyno.tuning_session && self.on_dyno_track()
    }

    /// 0x482180 in the dyno: EA Trax with the game music and the master
    /// on, else the score.
    fn dyno_music(&self) -> MusicContext {
        let st = &self.settings;
        if st.game_music <= 0.0 || st.master <= 0.0 { MusicContext::Score } else { MusicContext::Dyno }
    }

    fn music_context_of(&self, mode: &Mode) -> MusicContext {
        let st = &self.settings;
        // An in-engine scene in game (a race's intro: the flow's
        // CAMERA_FLYBY, state 7 via 0x481210(2)) stops the song; the front
        // end's pass (0x47B080 with audio +0x90 = 1) does not look at one.
        // Not a race's ending scene: it plays in the race's flow, whose
        // song goes on through it and the post-race screens over it
        // (state 7 keeps an EA Trax song, 0x47AD0F; observed: the same
        // song from "Winner!" through Event Results and Post Race).
        let scene =
            self.scenes.shop.as_ref().is_some_and(|r| !matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_)));
        match mode {
            // A movie: the front end stops its song (state 1 → 0xB, a new
            // one after it), the game holds it (state 5 → 6), 0x482350
            // on 0x8383AC.
            Mode::Menu(Menu::Movie(_)) => {
                if self.sim.drive.is_some() {
                    MusicContext::Paused
                } else {
                    MusicContext::Stopped
                }
            }
            // DynoEnter (a type-2 scene on the dyno track): the song goes on
            // (Wine L1: state 6 through the scene; the flow's CAMERA_FLYBY
            // stop 0x481210(2) is a race intro's).
            Mode::Race(_) | Mode::Explore if self.dyno_music_session() => self.dyno_music(),
            Mode::Race(_) | Mode::Explore if scene => MusicContext::Stopped,
            Mode::Race(r) => {
                if r.timing.clock < 0.0 {
                    // PRERACE (flow state 3, 0x481210(3)): held.
                    MusicContext::Paused
                } else if r.drag || r.scoring.drift.is_some() || st.game_music <= 0.0 || st.master <= 0.0 {
                    // 0x482180: a drag or drift race, or the game music
                    // or the master at 0 (0x482243), takes the score.
                    MusicContext::Score
                } else {
                    MusicContext::Game
                }
            }
            Mode::Explore => {
                if st.game_music <= 0.0 || st.master <= 0.0 {
                    MusicContext::Score
                } else {
                    MusicContext::Game
                }
            }
            Mode::Menu(m) => {
                let paused = self.sim.paused_race.is_some()
                    || matches!(
                        m,
                        Menu::Icons(_, MenuKind::ExplorePause | MenuKind::RacePause | MenuKind::PauseOptions(_))
                            | Menu::InGameMap(_)
                    );
                if self.dyno_music_session() {
                    // The tuning session's Quit question (0x55A8A9 /
                    // 0x564DE0 → 0x5F4240: the flow's message 7, the
                    // pause → 0x4810F0) holds the song (Wine L1: state 5
                    // while "Quit to Garage?" was up); its Save question
                    // prov: inferred game.music_context_of
                    // follows it under the same pause (**inferred**).
                    if let Menu::Dialog(_, back) = m
                        && back.iter().any(|b| {
                            matches!(b, Mode::Menu(Menu::LotDeal(LotDeal::QuitTuning | LotDeal::QuitTuningSave(_))))
                        })
                    {
                        return MusicContext::Paused;
                    }
                    if !matches!(m, Menu::Dialog(..)) {
                        return self.dyno_music();
                    }
                }
                // A dialog is pushed over the screen it asks from (its last
                // outcome, No / Cancel / OK): the music goes on as there, so
                // opening and answering it changes no song.
                if let Menu::Dialog(_, back) = m
                    && let Some(under) = back.last()
                {
                    return match under {
                        Mode::Menu(Menu::Dialog(..)) => MusicContext::FrontEnd,
                        _ => self.music_context_of(under),
                    };
                }
                // The screens over a loaded world run in the game's audio
                // mode (+0x90 ≠ 1: only the front end's load,
                // 0x5783E0 → 0x486B30, sets 1), so the game's song goes
                // on: the post-race chain (Event Results, UI_PostRaceReward,
                // UI_SponsorPopup and the sponsorship screen after it, all
                // in the race's flow) and the status screen (flow mode
                // 0x13, which 0x4810F0 does not hold).
                let over_game = matches!(
                    m,
                    Menu::Engage(_)
                        | Menu::ShopEngage(_)
                        | Menu::Challenge(_)
                        | Menu::Results(..)
                        | Menu::Stats(_)
                        | Menu::Reward(_)
                        | Menu::SponsorPopup(_)
                        | Menu::Status(_)
                        | Menu::SmsMailbox(_)
                        | Menu::SmsViewer(_)
                        | Menu::Icons(_, MenuKind::PostRace)
                ) || (matches!(m, Menu::Icons(_, MenuKind::Sponsor)) && self.sim.drive.is_some());
                if scene && (paused || over_game) {
                    MusicContext::Stopped
                } else if paused {
                    MusicContext::Paused
                } else if over_game {
                    if st.game_music <= 0.0 || st.master <= 0.0 { MusicContext::Score } else { MusicContext::Game }
                } else if st.menu_music <= 0.0 {
                    // 0x482180: the front end's music level 0 → the
                    // interactive score (at SFXObj_Ambience's level: the
                    // effects volume).
                    MusicContext::ScoreFrontEnd
                } else {
                    MusicContext::FrontEnd
                }
            }
        }
    }
}
