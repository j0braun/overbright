pub(crate) mod challenges;
pub(crate) mod uniques;
pub(crate) mod rating;
pub(crate) mod markers;
pub(crate) mod challenges_impl;
pub(crate) mod uniques_impl;
pub(crate) mod sponsors;
pub(crate) mod covers;
pub(crate) mod cover_rewards;
pub(crate) mod sms;
pub(crate) mod world_map;

use crate::*;

/// The career's free-roam state: the D-Day script and GPS, the map's
/// markers and the world challenge (were `Game` fields). Every field starts
/// empty (`Default`).
#[derive(Default)]
pub(crate) struct CareerWorld {
    /// Where "Return to Explore Mode" goes.
    pub(crate) pending_explore: Option<(Vec3, f32)>,
    /// The career start scene has played this session: a later city start
    /// (a pause left, a dialog closed) does not play it again.
    pub(crate) career_scene_played: bool,
    /// The D-Day script (0x861E7C, 0x5342A0), Rachel's hint queue
    /// (0x8635BC) and the GPS (0x838590); see `nfsu2::frontend::dday`.
    pub(crate) dday: dday::Script,
    pub(crate) gps: Option<dday::Gps>,
    /// When the script's prompt (kind 0x14) began flashing (clock_ms).
    pub(crate) dday_prompt: Option<f32>,
    /// The explore region's event starts for the minimap, for the open
    /// events they were found for.
    pub(crate) event_starts: Option<(Vec<u32>, Vec<(usize, career::Trigger)>)>,
    /// The route trigger the car is in that stands for a marker (player
    /// +0x9AC, 0x5EB050), by its place among the kind 13–15 triggers.
    pub(crate) marker_trigger: Option<usize>,
    /// The in-game map's legend hide flags (career 0x8635A9.., 0x496390),
    /// kept between openings.
    pub(crate) map_hidden: [bool; 14],
    /// The marker it held when entered, if it was valid then (career
    /// +0x8700…+0x8728, 0x532B20): what the Engage key engages.
    pub(crate) marker: Option<MarkerTarget>,
    /// The world challenge under way (career +0xC08 = 0x85B948 set).
    pub(crate) challenge: Option<WorldChallenge>,
}
