//! Quick Race (game mode 2): the mode records the profile keeps, the
//! track list of UI_QRTrackSelect and UI_QRModeOptions ("Quick Race Game
//! Options").
//!
//! **Verified** against SPEED2.EXE:
//! * The race record (0x83AAB4, class constructor 0x52A230): +0x00 the race
//!   type (g_RaceOptions), then twelve 20-byte mode records from +0x10
//!   (type 12 at +0x100): +0x00 players 1, +0x01 opponents 3, +0x02
//!   difficulty 1, +0x03 1, +0x04 traffic level 1, +0x05 catch-up 1, +0x08
//!   the track (u16, 0 = none yet), +0x0A laps 3 (u16), +0x0C the direction
//!   (u32, 0 forward), +0x10 Lap KO 0 (accessors 0x5026F0 … 0x5029C0,
//!   0x502910 the record's track block).
//! * The flow (0x4E2900): UI_QRModeSelect → UI_QRTrackSelect (0x4B2A80) →
//!   UI_QRModeOptions (0x498240: for every type) → UI_QRCarSelect
//!   (0x498220); Free Run goes from the mode straight to the cars.

#[cfg(test)]
use crate::frontend::options::RowId;

pub mod race_setup;

pub use super::screens::qr_options::*;
pub use race_setup::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_profile_values() {
        assert_eq!(
            QrRecord::new_profile(10),
            QrRecord { track: 4601, difficulty: 0, traffic: 0, laps: 10, ..QrRecord::default() }
        );
        assert_eq!(QrRecord::new_profile(1).laps, 1);
        assert_eq!(QrRecord::new_profile(3).traffic, 2);
        assert_eq!(QrRecord::new_profile(4).opponents, 5);
        assert_eq!(QrRecord::new_profile(9), QrRecord { opponents: 0, laps: 0, ..QrRecord::default() });
        assert_eq!(QrRecord::new_profile(6), QrRecord::default());
    }

    #[test]
    fn lap_ko_sets_three_laps_and_opponents() {
        let mut r = QrRecord { laps: 7, opponents: 1, ..Default::default() };
        change(RowId::QrLapKo, 0, &mut r, true);
        assert!(r.lap_ko);
        assert_eq!((r.laps, r.opponents), (3, 3));
    }

    #[test]
    fn values_wrap() {
        let mut r = QrRecord::default();
        r.laps = 10;
        change(RowId::QrLaps, 0, &mut r, true);
        assert_eq!(r.laps, 1);
        change(RowId::QrLaps, 0, &mut r, false);
        assert_eq!(r.laps, 10);
        r.opponents = 0;
        change(RowId::QrOpponents, 0, &mut r, false);
        assert_eq!(r.opponents, 3);
        change(RowId::QrOpponents, 4, &mut r, true);
        assert_eq!(r.opponents, 4);
        r.track = 4402;
        r.opponents = 2;
        change(RowId::QrOpponents, 2, &mut r, true);
        assert_eq!(r.opponents, 0);
    }

    /// 0x4B37F0, 0x4B3750: Left from above the
    /// highest steps down, only Right wraps; building the options screen
    /// clamps only a drag on 4402 to 2.
    #[test]
    fn opponents_above_the_highest_and_drag_4402_on_open() {
        let mut r = QrRecord { opponents: 5, ..QrRecord::default() };
        change(RowId::QrOpponents, 0, &mut r, false);
        assert_eq!(r.opponents, 4, "Left: one down, no wrap");
        r.opponents = 5;
        change(RowId::QrOpponents, 0, &mut r, true);
        assert_eq!(r.opponents, 0, "Right: wraps to the lowest");
        let mut d = QrRecord { track: 4402, opponents: 3, ..QrRecord::default() };
        clamp_opponents_on_open(2, &mut d);
        assert_eq!(d.opponents, 2);
        for (ty, track) in [(2, 4401), (0, 4402), (1, 4402)] {
            let mut o = QrRecord { track, opponents: 3, ..QrRecord::default() };
            clamp_opponents_on_open(ty, &mut o);
            assert_eq!(o.opponents, 3, "{ty} {track}: no general clamp");
        }
    }

    #[test]
    fn new_profile_records_match_the_reset() {
        let r = new_profile_records();
        assert_eq!((r[0].track, r[0].laps, r[0].opponents, r[0].traffic), (4021, 3, 3, 1));
        assert_eq!((r[3].traffic, r[4].opponents, r[9].opponents, r[9].laps), (2, 5, 0, 0));
        assert_eq!((r[10].difficulty, r[10].traffic, r[10].track, r[10].laps), (0, 0, 4601, 10));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn names_are_distinct() {
        let mut rng = crate::opponents::GameRng::default();
        let names = draw_names(&mut rng, 5);
        let set: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(set.len(), 5);
    }
}
