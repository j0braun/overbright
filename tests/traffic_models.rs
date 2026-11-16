//! The traffic model pass (0x412BE0, docs/formats.md "Traffic" → Models)
//! on the shipped car types: per caps zone, no model past its cap (car type
//! byte +0x854 + zone; past it only the first model in use, the pass's
//! last resort), none with cap 0 (AMBULANCE, BUS, FIRETRUCK), at
//! most 7 distinct models.

mod common;

use nfsu2::ai::traffic::{MAX_CARS, MAX_MODELS, assign_models};
use nfsu2::opponents::GameRng;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn traffic_models_follow_the_zone_caps() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let types: Vec<_> = nfsu2::car::parse_car_types(&globalb).unwrap().into_iter().filter(|t| t.traffic).collect();
    // The manager's entries in record order (0x41AAF0): TAXI first.
    assert_eq!(types[0].name, "TAXI");
    for zone in 0..5 {
        let caps: Vec<i8> = types.iter().map(|t| t.traffic_caps[zone]).collect();
        for seed in 0..50 {
            let picks = assign_models(&caps, MAX_CARS, &mut GameRng(seed));
            assert_eq!(picks.len(), MAX_CARS);
            let mut seen = picks.clone();
            seen.sort();
            seen.dedup();
            assert!(seen.len() <= MAX_MODELS, "zone {zone}: {} models", seen.len());
            for &m in &seen {
                let n = picks.iter().filter(|&&x| x == m).count();
                assert!(caps[m] > 0, "zone {zone}: {} has cap 0", types[m].name);
                // Past its cap only through the last resort, the first
                // model in use (zone 4's caps of 2 run out with 7 models).
                assert!(n <= caps[m] as usize || m == seen[0], "zone {zone}: {} × {n} over cap {}", types[m].name, caps[m]);
                assert!(!matches!(types[m].name.as_str(), "AMBULANCE" | "BUS" | "FIRETRUCK"));
            }
        }
    }
}
