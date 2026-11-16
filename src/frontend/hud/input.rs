use super::*;

/// The minimap's view of the world (0x4CA840).
#[derive(Clone, Debug, Default)]
pub struct MapView {
    /// TRACKMAP<n>, the track's map texture.
    pub texture: Option<u32>,
    /// The track record's frame: left x, top y, world size, zoom.
    pub left: f32,
    pub top: f32,
    pub size: f32,
    pub zoom: f32,
    /// The player: world x, y and heading (radians about +z, 0 = +x).
    pub player: glam::Vec2,
    pub heading: f32,
    /// Markers: the indicator name ("CIRCUITEVENTINDICATOR_", …), the
    /// world position and, for a car (AIRACERINDICATOR_, item type 3), its
    /// heading (radians about +z, 0 = +x); each name's objects are used in
    /// order. The race's start and finish items (types 1 and 2, flag 1 at
    /// 0x4EC950) never show: 0x4CA6D0's table 0x4CA7E4 sends types 0–2 to
    /// "hidden", so the clone lists none.
    pub icons: Vec<(&'static str, glam::Vec2, Option<f32>)>,
    /// Markers this many windows away or further are dropped: 1, or 10
    /// in free roam with the GPS on, in stage 0 or during a world
    /// challenge (0x4CA840).
    pub range: f32,
    /// Map mode 2: the map turns with the car (0x4CA840).
    pub rotate: bool,
    /// The engaged GPS's map point (0x838590 +0x20 / +0x24): the group
    /// 0x697D2BCA goes there (0x4C8410).
    pub gps: Option<glam::Vec2>,
    /// 0x4C8410: GPS_Search_Group (+0x24) shows (states 0–2, the
    /// player's follower holding no route).
    pub gps_searching: bool,
    /// GPS_SEARCH_ERROR (+0x2C) shows with this text (states 1, 2).
    pub gps_error: Option<String>,
}

/// The player's HUD options (the options block, see
/// [`super::super::options::Settings`]): which custom-HUD elements show, the
/// minimap (the race or explore map mode is not Off) and the units.
#[derive(Clone, Copy, Debug)]
pub struct HudOptions {
    pub metric: bool,
    pub map: bool,
    pub gauges: bool,
    pub position: bool,
    pub lap_info: bool,
    /// Split Time is not Off.
    pub split: bool,
    pub score: bool,
    pub leaderboard: bool,
}

impl Default for HudOptions {
    /// The profile defaults (0x5116E0): everything on, miles per hour.
    fn default() -> HudOptions {
        HudOptions {
            metric: false,
            map: true,
            gauges: true,
            position: true,
            lap_info: true,
            split: true,
            score: true,
            leaderboard: true,
        }
    }
}

/// What the HUD shows, from the race and the car.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// The player's HUD options.
    pub options: HudOptions,
    /// HUD_FEATURE_RVM's test (0x60048C..0x6004B7): the Rearview Mirror
    /// option (0x83AA18) on, the look-back key not held (player +0x249)
    /// and the camera setting (0x83AA74) bumper (0) or hood (1).
    pub rear_view: bool,
    /// A race is on (0x89E7B0 ≠ 0); otherwise free roam.
    pub racing: bool,
    /// Career free roam (0x850078 = 1): the bank and reward elements exist.
    pub career: bool,
    /// The race is over (0x5FFF60): race time, s.
    pub finished: Option<f32>,
    /// The player's racer status at the race over (racer +0x10: 1
    /// finished, 3 knocked out, 8 blown engine, 9 totaled, 11 time's up;
    /// 0 taken as finished).
    pub status: u16,
    /// Cars in the race (the element masks hide the position with one).
    pub racers: u32,
    /// The race's racers, humans and AI (0x89E7C0 + 0x89E7C4 + 0x89E7C8 +
    /// 0x89E7CC; a drift race's virtual rivals are not racers).
    pub field: u32,
    /// Forward speed, m/s (car +0x430).
    pub speed: f32,
    pub rpm: f32,
    /// Engine redline and maximum rpm (the HUD's engine block +4, +8).
    pub redline: f32,
    pub max_rpm: f32,
    /// Gear: 0 reverse, 1 neutral, 2 first….
    pub gear: usize,
    /// The clutch locked (car +0x448 = transmission state +0x44 is 3,
    /// set each frame by the mover 0x5A593B).
    pub clutch_locked: bool,
    pub shift_light: bool,
    /// Laps: the one under way (0-based) and the race's count.
    pub lap: u32,
    pub laps: u32,
    /// Point to point: the lap counter shows the distance done (0..1).
    pub sprint: Option<f32>,
    /// Race clock, s (negative in the countdown).
    /// Seconds of race (sim) clock since the race's HUD was set up (its
    /// load): the leader board element's constructor (0x4F63E7) starts its
    /// name / gap phase (+0x1BC) there.
    pub race_age: f32,
    pub clock: f32,
    /// When the lap under way started, s on the race clock.
    pub lap_start: f32,
    /// The last lap's time and when it ended, s.
    pub last_lap: Option<(f32, f32)>,
    pub best_lap: Option<f32>,
    /// Laps completed so far.
    pub laps_done: u32,
    pub wrong_way: bool,
    pub cash: i64,
    /// The car's HUD style (its HUDINDEX attribute; 0 for a stock car).
    pub hud_index: u32,
    /// The fitted custom gauge colours (RED / GREEN / BLUE of the parts in
    /// slots 0xA4, 0xA5, 0xA6; None for an empty slot, white).
    pub gauge_colors: [Option<[u8; 3]>; 3],
    pub map: Option<MapView>,
    /// The player's name (the leader board's row).
    pub name: String,
    /// The race type (its HUD package and features).
    pub race: crate::race::RaceMode,
    /// The drift element's values in a drift race.
    pub drift: Option<DriftHud>,
    /// The player's place (racer status +0x0A) and the race time the
    /// position element shows, s.
    pub place: u32,
    /// The player's finish order (racer status +0x0C, from 0x5EEF80 at
    /// its finish), which the race-over place line reads (0x4AB4B0 at
    /// 0x4AB4E5); 0 before the finish (the place is used then).
    pub finish_order: u32,
    /// Lap KO's knocked-out racers (race status +0x28), which
    /// NumberRacers leaves out (0x4EBB10, race types 3 and 7).
    pub knocked_out: u32,
    pub race_time: f32,
    /// A drift race's hurry counter (race status +0x20 ≥ 0 once armed,
    /// see the game's `Race::time_out`): the time left, s.
    pub hurry: Option<f32>,
    /// The leader board's rows in race order (0x4C6F30).
    pub leaders: Vec<LeaderRow>,
    /// The drag tachometer's values in a drag race.
    pub drag: Option<DragHud>,
    /// The nitrous tank's fraction (0x5925C0) when the car has a tank
    /// (0x401860: its size over 0).
    pub nitrous: Option<f32>,
    /// The car has boost (0x5ECED0: the engine's +0x98 above 0, the
    /// largest of the turbo curve's nine points, physics +0x334, taken by
    /// 0x457B00 when the engine is built, 0x5AA6D4): the turbo gauge
    /// (0x4C6B80) shows its objects only then, hides them otherwise.
    pub turbo: bool,
    /// The turbo's boost (engine +0x9C, 0x5ECED0; `handling::Turbo`).
    pub boost: f32,
    /// The race's start scene started it: no 3-2-1 and no "GO!" (race
    /// status +8 stays 0, 0x5EEA50).
    pub no_countdown: bool,
    /// A replay plays (race camera state RCSTATE_INSTANT_REPLAY, 5): the
    /// countdown element alone (0x600186 → 0x60052A, then 0x60052C's
    /// tail), so "GO!" for a step after the green light.
    pub replay: bool,
    /// The player's camera director runs event kind 1, 6 or 7 (+0x58: the
    /// replay key camera, the crash / jump camera and its second shot):
    /// 0x6000A0 gives no features at all (0x6000E8..0x6000F4), whichever
    /// mover the factory built (the chase camera too when the sight test
    /// failed). Observed under Wine: the whole HUD is gone for the
    /// crash camera's span, chase or not.
    pub event_camera: bool,
    /// The countdown's step, race status +0xC, s (0 means 0.9, 0x5FC740's;
    /// with Crowds off the picked start scene's, 0x436840).
    pub countdown_step: f32,
    /// The engage prompt's kind (0x4C9170): set while the car's marker is
    /// current (see [`engage_texture`]).
    pub engage: Option<u8>,
    /// An outrun under way (race state 0x89E7B0 = 7): its meter's reading.
    pub outrun: Option<OutrunHud>,
    /// An outrun's result, shown with `finished` (0x4C7F00 cases 6 and 7):
    /// won, and in a career win the stage's bank.
    pub outrun_result: Option<(bool, Option<i32>)>,
    /// The random encounter's icon is up (0x413030 → 0x500D20 → 0x4DB890(1)
    /// → 0x4C9170(9, 0) on the free-roam events element).
    pub encounter: bool,
    /// The SMS element (HUD +0x7C, built by the HUD constructor at
    /// 0x4F913E in a career, not online): the inbox's unread count
    /// (0x514D30(1)) and the kind whose icon a new message shows (0x4ABAF0).
    pub sms: Option<SmsHud>,
    /// The D-Day script's HUD features while it runs (0x60031E; see
    /// [`super::super::dday::Script::hud_features`]); it also holds the SMS
    /// slot's flashing (0x4C8AC0 → 0x502420).
    pub script: Option<(u32, u32)>,
    /// The script's prompt (kind 0x14, value 2): ms since it began
    /// flashing.
    pub phone: Option<f32>,
    /// A world challenge's clock running (state 1, 0x501680): its time
    /// left, s (0x85B944).
    pub challenge: Option<f32>,
    /// A photo shoot's cash popup coming due this frame (0x4AC170 armed
    /// it; 0x4A9FF0 shows it after four updates): the milestone's kind
    /// and the cash paid (+0x26 plus the rating's share).
    pub cash_reward: Option<(u8, i32)>,
    /// The tricks the player's style manager reported this frame
    /// (0x56FD20 → 0x4DA6F0 → 0x4C88E0), see [`crate::drift::TrickReport`].
    pub tricks: Vec<crate::drift::TrickReport>,
    /// The style element's +0x79 (0x4F7AD0 at 0x4F8034): the player's car
    /// has a tank (car +0x454 neither 0 nor 0x7FFFFFFF, its +0x414 over 0)
    /// and the Race Breaker flag 0x89E7E7 is set: the popup's points show.
    pub style_points: bool,
}

/// What the SMS element reads of the inbox.
#[derive(Clone, Copy, Debug, Default)]
pub struct SmsHud {
    pub unread: u32,
    pub kind: u8,
}

/// One slot of the engage prompt's queue (0x14 bytes at element +0x60):
/// +0 the state (0 free, 1 queued, 2 and 3 flashing, 4 resting, 6
/// ending), +1 the kind (1–0x12 the markers' and the encounter's, 0x13 the
/// SMS, 0x14 the D-Day script's), +4 when state 2 began (ms; g_SimTicks
/// in the game) and +0x10 the value (the unread count; 0x14's seconds in
/// state 2).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Slot {
    pub(super) state: u8,
    pub(super) kind: u8,
    pub(super) since: f32,
    pub(super) value: u32,
}
