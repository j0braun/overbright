pub(crate) mod ai;
pub(crate) mod car_id;
pub(crate) mod drive;
pub(crate) mod effects;
pub(crate) mod encounters;
pub(crate) mod event_camera;
pub(crate) mod explore;
pub(crate) mod race;
pub(crate) mod replay;
pub(crate) mod rng;
pub(crate) mod session;
pub(crate) mod tick_settings;
pub(crate) mod traffic;

use crate::*;
pub(crate) use tick_settings::TickSettings;

/// The simulation's session: the drive, the AI's data and the game's
/// random numbers' users (traffic, encounters), the races kept for pause
/// and replay, the free-roam style, zone and triggers, and the game clock
/// (were `Game` fields).
pub(crate) struct SimSession {
    pub(crate) drive: Option<Drive>,
    /// A race paused by the pause menu.
    pub(crate) paused_race: Option<Race>,
    /// The last finished race with its recording, for the post-race
    /// menu's Replay.
    pub(crate) replay: Option<Race>,
    /// The race difficulty 0x89E7F4 and the AI racers' throttle level
    /// 0x89E804 as the last set-up left them (`nfsu2::difficulty`). The
    /// difficulty is read by the skill rows (0x4FED00, 0x5261F0), the
    /// Quick Race catch-up scale, the drift assist, the AI's drift rate,
    /// the drift board and the drag shift window.
    pub(crate) race_globals: nfsu2::difficulty::RaceGlobals,
    /// The game clock (0x865188) in ms: it runs in free roam only.
    pub(crate) clock_ms: f32,
    /// The region's route triggers (event markers, shops, photo spots).
    pub(crate) route_triggers: career::RouteTriggers,
    /// The player's zone (+0x9B0 the kind-0x11 trigger, +0x9B4 the last
    /// one seen, +0x9B8 its `NEIGHBOURHOOD_` hash; a new player object
    /// per world load, so reset with the free-roam style).
    pub(crate) zone: (Option<usize>, Option<usize>, u32),
    /// The free-roam racer's style manager, the total already turned into
    /// nitrous (racer +0x194) and its clock; dropped when a race starts
    /// (**inferred**: the free-roam racer is set up again after a race).
    pub(crate) roam_style: Option<(nfsu2::drift::StyleManager, f32, f32, nfsu2::style::Misc)>,
    /// The player's camera director's event cameras in free roam (race
    /// status 4 there, so Car_Crash's crash camera and the jump camera of
    /// 0x604750 run as in a race); a new director per free-roam start
    /// (**inferred**), dropped when a race starts.
    pub(crate) roam_camera: Option<RaceCamera>,
    /// The AI racers' data, and the game's random numbers (0x7EA624).
    pub(crate) ai: Option<AiData>,
    /// Explore mode's traffic, kept while the region stays loaded.
    pub(crate) traffic: Option<Traffic>,
    /// Career free roam's random encounters and outrun (race status
    /// +0xC40), rebuilt with free roam.
    pub(crate) encounters: Option<Encounters>,
    /// Photo-shoot spots found so far, by trigger name.
    pub(crate) spots: std::cell::RefCell<HashMap<u32, Option<career::Trigger>>>,
    /// View 1's lens drops as the start-up drew them (0x5C2FA0 →
    /// 0x61E160 → 0x6140A0; [`Game::boot_draws`]), handed to every world's
    /// weather object.
    pub(crate) lens: nfsu2::weather::LensSetup,
    /// The traffic models' paint counts (0x412840's draws).
    pub(crate) traffic_paints: Vec<u32>,
    /// The main menu's car render info draw is still to come
    /// (`Game::menu_car_draw`).
    pub(crate) menu_car_pending: bool,
    /// The contact sounds (impacts, scrape pushes) the physics
    /// posted, waiting for the audio's `impact_sounds` (were process-wide
    /// queues in `nfsu2::audio`); the tick's come in `TickEvents::contacts`.
    pub(crate) contact_sounds: nfsu2::drive::ContactSounds,
    /// The map editor's custom race being raced (its route in place of
    /// its base track's, `Game::map_race_route`); None outside one.
    pub(crate) map_race: Option<nfsu2::tools::map::CustomRace>,
}

impl SimSession {
    /// The values `Game::new` starts with (`route_triggers` and `ai` are
    /// read from the game files there).
    pub(crate) fn new(route_triggers: career::RouteTriggers, ai: Option<AiData>) -> SimSession {
        SimSession {
            drive: None,
            paused_race: None,
            replay: None,
            race_globals: Default::default(),
            clock_ms: 0.0,
            route_triggers,
            zone: (None, None, 0),
            roam_style: None,
            roam_camera: None,
            ai,
            traffic: None,
            encounters: None,
            spots: Default::default(),
            lens: Default::default(),
            traffic_paints: Vec::new(),
            menu_car_pending: false,
            contact_sounds: Default::default(),
            map_race: None,
        }
    }
}
