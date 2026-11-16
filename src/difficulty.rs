//! The race difficulty and the AI racers' throttle level: the two globals
//! a race's set-up writes and the race code reads (SPEED2.EXE).
//!
//! * **0x89E7F4, the difficulty** (0 Easy, 1 Medium, 2 Hard; 0 in the
//!   image). A career race copies the career difficulty (profile +0x250 =
//!   0x83AC30, the Options → Gameplay "Career Difficulty" row) at
//!   Race_SetupFromEvent 0x53231B; the city starts copy it too (0x540569
//!   in 0x5404B0, 0x5407CC in Career_FreeRoamSetup 0x540790); the Quick
//!   Race set-ups (0x53F160 circuit / sprint, 0x53F380, 0x53F590, 0x53F6A0
//!   drag, 0x53F840 drift, 0x53FA50 URL, 0x526730, 0x526840, 0x601E4B)
//!   copy their mode record's +0x02 (UI_QRModeOptions' Difficulty row); the
//!   race-type defaults 0x4FEF80 … 0x4FF160 (test runs, 0x526980) clear it.
//! * **0x89E804, the throttle level** (0 in the image; 3 already at a
//!   fresh boot's main menu under Wine, its writer there **not traced**):
//!   a career event's byte +0x7D (0x5323F5: 0 or 1 in the drag events, 3
//!   in every other), Quick Race drag's difficulty (0x53F6A0: 0, 1, 2);
//!   0x5400C5 (the 0x53FEC0 set-up, not reached offline: **not traced**)
//!   by the difficulty too. No other set-up writes it, so a Quick Race
//!   circuit keeps the last one's (read 3 under Wine in a Quick Race
//!   circuit on Easy). The CarDriver's throttle cap +0x22C is
//!   0x783A28[level] for a racer's controller (0x425B52).

/// The CarDriver's throttle cap by the throttle level (0x783A28).
pub use crate::ai::driver::THROTTLE_CAP;

/// The traffic controller's +0x799 by difficulty (0x42C6E3 in
/// Traffic_CreateOnPoint, 0x42C98E in Traffic_PlaceCar: Easy 9, Medium 4,
/// Hard 0; −1 from its constructor 0x426623 and for any other value). It
/// is the traffic controller's slot +0x78 (0x41E780), the slot the racers'
/// code reads as the skill row; which code reads it for a traffic car is
/// **not traced** (seen under Wine: 9 on every traffic car of an Easy
/// Quick Race), so the clone's traffic does not use it.
pub const TRAFFIC_SKILL: [i8; 3] = [9, 4, 0];

/// The two globals as a set-up leaves them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaceGlobals {
    /// 0x89E7F4, 0..=2.
    pub difficulty: usize,
    /// 0x89E804, 0..=3.
    pub throttle_level: usize,
}

impl Default for RaceGlobals {
    /// At the main menu of a fresh boot (read under Wine): difficulty 0,
    /// throttle level 3.
    fn default() -> RaceGlobals {
        RaceGlobals { difficulty: 0, throttle_level: 3 }
    }
}

impl RaceGlobals {
    /// Race_SetupFromEvent (0x5322F0): the career difficulty and the
    /// event's +0x7D.
    pub fn career_race(&mut self, career_difficulty: u8, event_throttle: i8) {
        self.difficulty = usize::from(career_difficulty.min(2));
        self.throttle_level = (event_throttle.max(0) as usize).min(3);
    }

    /// A Quick Race set-up: the mode record's difficulty; drag's (0x53F6A0)
    /// also sets the throttle level from it.
    pub fn quick_race(&mut self, record_difficulty: u8, drag: bool) {
        self.difficulty = usize::from(record_difficulty.min(2));
        if drag {
            self.throttle_level = self.difficulty;
        }
    }

    /// A city start (0x5404B0 / 0x540790): the career difficulty; the
    /// throttle level stays.
    pub fn city(&mut self, career_difficulty: u8) {
        self.difficulty = usize::from(career_difficulty.min(2));
    }

    /// The AI racers' throttle cap (CarDriver +0x22C).
    pub fn throttle_cap(&self) -> f32 {
        THROTTLE_CAP[self.throttle_level.min(3)]
    }

    /// The traffic controllers' +0x799.
    pub fn traffic_skill(&self) -> i8 {
        TRAFFIC_SKILL[self.difficulty.min(2)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn career_race_takes_the_career_difficulty_and_the_event_level() {
        let mut g = RaceGlobals::default();
        for d in 0..3u8 {
            g.career_race(d, 3);
            assert_eq!((g.difficulty, g.throttle_level, g.throttle_cap()), (usize::from(d), 3, 1.0));
        }
        g.career_race(2, 0);
        assert_eq!(g.throttle_cap(), 0.93);
        g.career_race(2, 1);
        assert_eq!(g.throttle_cap(), 0.97);
    }

    #[test]
    fn quick_race_keeps_the_level_outside_drag() {
        let mut g = RaceGlobals::default();
        g.career_race(1, 0);
        g.quick_race(2, false);
        assert_eq!((g.difficulty, g.throttle_level), (2, 0));
        for (d, cap) in [(0u8, 0.93), (1, 0.97), (2, 1.0)] {
            g.quick_race(d, true);
            assert_eq!((g.difficulty, g.throttle_level, g.throttle_cap()), (usize::from(d), usize::from(d), cap));
        }
    }

    #[test]
    fn city_start_takes_the_career_difficulty_only() {
        let mut g = RaceGlobals { difficulty: 0, throttle_level: 1 };
        g.city(2);
        assert_eq!(g, RaceGlobals { difficulty: 2, throttle_level: 1 });
        assert_eq!(g.traffic_skill(), 0);
        g.city(0);
        assert_eq!(g.traffic_skill(), 9);
    }
}
