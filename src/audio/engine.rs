//! Which engine recording a car plays: the car type's sound index
//! (0x459240), its engine level from the fitted performance parts, the
//! engine entry for both (0x7F2230), and the entry's GIN (0x7F0DE0,
//! 0x4C-byte entries: the EE and SPU banks' names, then the GIN's).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub use super::car::tables::{GINS, LEVELS};

/// The car type's sound index (0x459240's switch; 28 and unknown types
/// take the default).
pub fn sound_index(car_type: usize) -> usize {
    static MAP: crate::exe_tables::Table<[u8; 31]> = crate::exe_tables::Table::new(|t| &t.sound_index);
    MAP.get(car_type).copied().unwrap_or(0x12) as usize
}

/// The engine level: 3 with part 0x18 fitted, 2 with 0x13, 1 with 0x0F,
/// else 0 (the upgrade state's fitted bytes, racer +0x300 + 0x50 + id).
pub fn level(perf: &BTreeSet<u8>) -> usize {
    [(0x18, 3), (0x13, 2), (0x0F, 1)].iter().find(|(p, _)| perf.contains(p)).map_or(0, |&(_, l)| l)
}

/// The engine entry a car type plays at its fitted parts.
pub fn entry(car_type: usize, perf: &BTreeSet<u8>) -> usize {
    LEVELS.get(sound_index(car_type)).map_or(0, |l| l[level(perf)] as usize)
}

/// The GIN file for an engine entry (names match the files ignoring
/// case, as on Windows: "Eclipse_5zigen" is GIN_Eclipse_5Zigen.gin).
pub fn gin_path(root: &Path, entry: usize) -> Option<PathBuf> {
    gin_file(root, &format!("GIN_{}.gin", GINS.get(entry)?))
}

/// A GIN file of SOUND/ENGINE by name, ignoring case.
pub fn gin_file(root: &Path, file: &str) -> Option<PathBuf> {
    let dir = crate::game_fs::path(root, "SOUND/ENGINE");
    let exact = dir.join(file);
    if exact.exists() {
        return Some(exact);
    }
    std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(file)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn engine_entries_follow_type_and_parts() {
        let mut perf = BTreeSet::new();
        // Type 0 → sound 0x16 → [23, 36, 37, 37].
        assert_eq!(entry(0, &perf), 23);
        perf.insert(0x0F);
        assert_eq!(entry(0, &perf), 36);
        perf.insert(0x18);
        assert_eq!(entry(0, &perf), 37);
        assert_eq!(sound_index(28), 0x12);
        assert_eq!(GINS[entry(0, &BTreeSet::new())], "Mazda_Miata");
    }

    /// The car type index is the GLOBALB order: the stock recordings of
    /// the types that have their own name in the GIN list match.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn globalb_order_picks_matching_recordings() {
        let none = BTreeSet::new();
        for (ty, gin) in [(1, "FordFocus_Road"), (3, "Nissan_240SX"), (4, "Mazda_Miata"), (8, "Hummer"), (19, "MitsEclipse_2000"), (26, "MustSal_Ramp_V2_CD"), (27, "Nissan_Skyline"), (29, "Mazda_RX7"), (30, "WRXSTi_Pull_09")] {
            assert_eq!(GINS[entry(ty, &none)], gin, "car type {ty}");
        }
    }
}
