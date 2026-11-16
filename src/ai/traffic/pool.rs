use super::*;

/// The traffic level career free roam sets (0x540790: 0x89E7A8 = 3). A
/// career race takes its event's byte +0x36 (0x5322F0); Quick Race
/// (0x526730, 0x526840, Free Run 0x53F590) its mode record's level
/// (0x83AAC8 + type × 20, the Mode Options' "MOTrafficLevel", which
/// 0x5029C0 cycles 0–3).
pub const FREE_ROAM_LEVEL: usize = 3;

/// The Quick Race mode records' traffic level before the Mode Options
/// change it. On a new save on path A (docs/formats.md) the records are the start-up defaults: the
/// profile constructor 0x53ED30 → 0x5344B0 → 0x52A230 sets every type's
/// level byte to 1 (docs/formats.md, "Traffic level"). A loaded save
/// overwrites them through the block loader 0x5344E0, and a new save made
/// after a load keeps those levels (open, docs/formats.md). A new profile
/// takes path B (the reset 0x515AA0 → 0x515800: types 3 and 10 at 2 and 0;
/// **verified** in memory, menus15) — the clone's records come from
/// `quick_race::new_profile_records` and a Quick Race reads its record's
/// level; this constant is only the test races' (`--race`) level.
pub const QUICK_RACE_LEVEL: usize = 1;

/// The preload's cap on traffic cars (0x4FEA51).
pub const MAX_CARS: usize = 16;

/// Distinct traffic models the model pass hands out before it reuses them
/// (0x412BE0).
pub const MAX_MODELS: usize = 7;

/// Traffic cars in the pool (0x4FE9F0): the track record's byte +0xEC +
/// 2·level + reversed (track 4000, free roam, level 3: 16), at most
/// `MAX_CARS`; none at level 0.
pub fn pool_size(track_traffic: &[i8; 8], level: usize, reversed: bool) -> usize {
    if level == 0 {
        return 0;
    }
    (track_traffic.get(2 * level + usize::from(reversed)).copied().unwrap_or(0).max(0) as usize).min(MAX_CARS)
}

/// Traffic Density (opt-in, docs/assets-distance.md; free roam only):
/// the native pool ([`pool_size`]) for 0, ×1.5 for 1 (More), ×2 for 2
/// (Dense), at most [`MAX_DENSE_CARS`]. The placer, its 1 s cadence, the
/// despawn ring and the jam rule stay native; only the pool grows.
pub fn dense_pool(native: usize, density: u8) -> usize {
    match density {
        0 => native,
        1 => (native * 3).div_ceil(2).min(MAX_DENSE_CARS),
        _ => (native * 2).min(MAX_DENSE_CARS),
    }
}

/// The largest pool Traffic Density makes (free roam's 16 doubled).
pub const MAX_DENSE_CARS: usize = 32;

/// 0x412BE0: a model for each traffic car from the traffic car types and
/// their caps for the zone (car type +0x854 + zone). While fewer than
/// `MAX_MODELS` models are in use, a random one (0x43C1C0 over those still
/// under their cap) whose use count then rises; after that the first in
/// use still under its cap, else the first in use, else the first.
/// Returns indices into `caps`.
pub fn assign_models(caps: &[i8], cars: usize, rng: &mut GameRng) -> Vec<usize> {
    let mut uses = vec![0i32; caps.len()];
    let mut open = caps.iter().filter(|&&c| c > 0).count() as u32;
    let mut distinct = 0;
    let mut out = Vec::with_capacity(cars);
    for _ in 0..cars {
        let pick = if distinct < MAX_MODELS {
            if open == 0 {
                out.push(0);
                continue;
            }
            let r = rng.below(open) as usize;
            let Some(m) = (0..caps.len()).filter(|&m| uses[m] < i32::from(caps[m])).nth(r) else {
                out.push(0);
                continue;
            };
            if uses[m] == 0 {
                distinct += 1;
            }
            if uses[m] + 1 == i32::from(caps[m]) {
                open -= 1;
            }
            m
        } else {
            (0..caps.len())
                .find(|&m| uses[m] > 0 && uses[m] < i32::from(caps[m]))
                .or_else(|| (0..caps.len()).find(|&m| uses[m] > 0))
                .unwrap_or(0)
        };
        uses[pick] += 1;
        out.push(pick);
    }
    out
}
