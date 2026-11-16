//! Race totals, style scores and the best-time / best-score records.

use super::*;

/// The profile's race totals (profile +0xA00 = 0x8454DC, 0x50 bytes, kept
/// raw so the fields not ported stay as loaded). Under Wine (stage-2
/// playthrough, 2026-10-31) +0xA04 and the kind's first dword went up at
/// the race's load, the rest at its end.
/// * At a race's load (World_Start 0x60A690 → 0x541A60 when g_RaceType
///   0x89E7B0 is set and not a dyno run 0x89E7E6; this = 0x83A9D0):
///   +0xA04 += 1 and, for a known kind, +0xA10 + 12·kind += 1 (in split
///   screen also +0xA40 and player 1's; not in the clone).
/// * After it (0x551440 → 0x515990): +0xA00 += the racer's distance
///   (result +0xF8 = car +0x410, an f32: dt × |v| while 0 < |v| < 1000
///   in race status 4, 0x604874), +0xA08 += place 1, +0xA0C += place ≠ 1,
///   +0xA14 + 12·kind += place 1, +0xA18 + 12·kind += the style score
///   ([`style_score`], 0x4FE170 on the result's +0x104 / +0x108),
///   +0xA4C += 1 on kind 1 (drag) when the result's status (+8, racer
///   +0x10) is 9 "Totaled", +0xA44 / +0xA48 in split screen (not in the
///   clone).
/// The kind is the track's +0x94 bits ([`record_kind`]).
#[derive(Clone, Debug, PartialEq)]
pub struct RaceTotals(pub [u8; 0x50]);

impl Default for RaceTotals {
    fn default() -> Self {
        RaceTotals([0; 0x50])
    }
}

impl RaceTotals {
    fn get(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.0[o..o + 4].try_into().unwrap())
    }
    fn bump(&mut self, o: usize, by: u32) {
        let v = self.get(o).wrapping_add(by);
        self.0[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    /// +0xA00: the distance raced, metres.
    pub fn distance(&self) -> f32 {
        f32::from_bits(self.get(0))
    }
    /// +0xA04: races loaded.
    pub fn started(&self) -> u32 {
        self.get(4)
    }
    /// +0xA08 / +0xA0C: races won / not won.
    pub fn firsts(&self) -> u32 {
        self.get(8)
    }
    pub fn others(&self) -> u32 {
        self.get(0xC)
    }
    /// +0xA10 + 12·kind: (loaded, won, style score) of a kind.
    pub fn kind(&self, kind: usize) -> [u32; 3] {
        [self.get(0x10 + 12 * kind), self.get(0x14 + 12 * kind), self.get(0x18 + 12 * kind)]
    }
    /// 0x541A60: a race loaded.
    pub fn race_loaded(&mut self, kind: i32) {
        self.bump(4, 1);
        if (0..4).contains(&kind) {
            self.bump(0x10 + 12 * kind as usize, 1);
        }
    }
    /// +0xA4C: drag races (kind 1) the player ended totaled (status 9).
    pub fn totaled(&self) -> u32 {
        self.get(0x4C)
    }
    /// 0x515990: a race finished in `place` (1 = won) after `distance`
    /// metres (car +0x410), with the racer's `status` (results +8) and
    /// style `score` ([`style_score`]).
    pub fn race_done(&mut self, kind: i32, place: usize, distance: f32, status: u16, score: i32) {
        let won = (place == 1) as u32;
        if (0..4).contains(&kind) {
            let k = kind as usize;
            self.bump(0x18 + 12 * k, score as u32);
            self.bump(0x14 + 12 * k, won);
            if kind == 1 && status == 9 {
                self.bump(0x4C, 1);
            }
        }
        let d = self.distance() + distance;
        self.0[0..4].copy_from_slice(&d.to_bits().to_le_bytes());
        self.bump(8, won);
        self.bump(0xC, 1 - won);
    }
}

/// The style score 0x515990 adds to the race totals: 0x4FE170 on the
/// results record, _ftol(floor(+0x108) × _ftol(+0x104)) (_ftol 0x75BED4
/// truncates; the floor is its correction at 0x4FE19C). 0x515DB0 sets
/// +0x104 = the racer's style manager total (0x56C7E0(0), +0x9C) and
/// +0x108 = max(owner +0x14 → +0x904, 1.0) (0x515F5C..0x515F7B), or both
// prov: inferred frontend.career.style_score
/// 0 without a style manager. **Inferred**: owner +0x14 → +0x904 is at
/// most 1 for the single player, so the factor is 1 (its writer is not
/// traced).
pub fn style_score(total: f32, factor: f32) -> i32 {
    let factor = if factor > 1.0 { factor } else { 1.0 };
    (factor.floor() * total.trunc()) as i32
}

/// One entry of the records table (profile +0xA0FC; **verified**: 0x502A50
/// finds an entry, 0x502DF0 adds or betters one, 0x515990 calls it after
/// each race from 0x551440 with the race options' track 0x89E7A0, run
/// backward 0x89E7A4 → `+3`, laps 0x89E7BC → `+2`, and the record kind
/// from the track's +0x94 bits (1 → 3, 2 → 1, 4 → 0, 8 → 2, else −1);
/// kind 2, drift, keeps the score, the others the best lap, in ticks of
/// 1/4000 s, of the player's results record (+0x10; UI_PostRaceResults'
/// BEST LAP TIME 0x4BFB90 reads the same field).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub track: i16,
    /// `+2` (0x89E7BC, the race's laps flag).
    pub lapped: i8,
    /// `+3` (0x89E7A4, run backward).
    pub backward: i8,
    pub value: i32,
}

/// The records table's size (0x140 entries).
pub const RECORD_SLOTS: usize = 0x140;

/// The record kind 0x551440 gives a race from its track's +0x94 bits.
pub fn record_kind(track_flags: u32) -> i32 {
    if track_flags & 1 != 0 {
        3
    } else if track_flags & 2 != 0 {
        1
    } else if track_flags & 4 != 0 {
        0
    } else if track_flags & 8 != 0 {
        2
    } else {
        -1
    }
}

impl Profile {
    /// 0x502DF0: add or better the record of (track, lapped, backward).
    /// `drift` (record kind 2) keeps a score (better when higher; a
    /// negative score is not kept), else a best lap in ticks (better when
    /// lower; 0 and 0x7FFFFFFF are no time). A new one takes the first
    /// free entry; with none free nothing is kept. True when kept.
    pub fn add_record(&mut self, drift: bool, track: i16, lapped: i8, backward: i8, value: i32) -> bool {
        let valid = if drift { value >= 0 } else { value != 0 && value != i32::MAX };
        if self.records.len() < RECORD_SLOTS {
            self.records.resize(RECORD_SLOTS, Record::default());
        }
        if let Some(r) = self
            .records
            .iter_mut()
            .find(|r| r.track != 0 && r.track == track && r.backward == backward && r.lapped == lapped)
        {
            // 0x502DA0: drift keeps a higher score, the rest a lower time.
            let better = if drift { r.value < value } else { valid && value < r.value };
            if !better {
                return false;
            }
            r.value = value;
            return true;
        }
        if !valid {
            return false;
        }
        let Some(r) = self.records.iter_mut().find(|r| r.track == 0) else {
            return false;
        };
        *r = Record { track, lapped, backward, value };
        true
    }

    /// 0x502AA0: the lowest time kept for (track, backward), any laps flag;
    /// none → the track's default (+0x9C forward / +0xA0 backward, ms, as
    /// ticks: ×0.001 × 4000 + 0.5).
    pub fn best_time(&self, track: &Track, backward: bool) -> i32 {
        let b = backward as i8;
        self.records
            .iter()
            .filter(|r| r.track != 0 && r.track == track.number as i16 && r.backward == b)
            .map(|r| r.value)
            .min()
            .unwrap_or_else(|| (track.default_record[backward as usize] * 0.001 * 4000.0 + 0.5) as i32)
    }

    /// 0x502C50: the highest score kept for (track, backward), else 0.
    pub fn best_score(&self, track: i16, backward: bool) -> i32 {
        let b = backward as i8;
        self.records
            .iter()
            .filter(|r| r.track != 0 && r.track == track && r.backward == b)
            .map(|r| r.value)
            .fold(0, i32::max)
    }
}
