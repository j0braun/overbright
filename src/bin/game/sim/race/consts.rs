/// The region explore mode drives in.
pub(crate) const EXPLORE_REGION: &str = "L4RA";

/// The performance shop whose packages cost 80% (shop hash CC_MEGALOW,
/// tested by 0x5415F0).
pub(crate) const MEGALOW: u32 = 0x7FC8_D0D0;

/// How close to an event's start (m) Enter starts it.
/// How long after the last racer finishes the post-race screens come
/// (0x60B3B0 case 3: every racer finished, 0x5F85C0, and the tick counter
/// 0x7FB720, 60 a second, past the last finisher's finish tick +0x50 plus
/// its hold +0x54). The hold is set at the finish (0x5EA540): 3 s, 6 s
/// when the racer's +0x58 is 6 or 7, none in race types 7 (outside mode
/// 4) and 8.
pub(crate) const RACE_OVER: f32 = 3.0;

/// A drift race's time-out after its first finisher (race status +0x20 =
/// race parameters +0x3C, 0x89E7DC, 30.0 from RaceParams_Reset 0x5ED600
/// and written nowhere else).
pub(crate) const HURRY_TIME: f32 = 30.0;

/// The finish's hold while a crash / jump camera (event kind 6 / 7, racer
/// +0x58) runs (0x5EA540 → 0x7A54A4).
pub(crate) const RACE_OVER_EVENT: f32 = 6.0;

/// The start countdown: 3, 2, 1 for a step each (race status +0xC, 0.9 s,
/// set at 0x5FC740; 0x5EEA50).
pub const COUNTDOWN_STEP: f32 = 0.9;

pub(crate) const COUNTDOWN: f32 = 3.0 * COUNTDOWN_STEP;
