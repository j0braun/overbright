//! The shared generator (0x7EA624) and the world generator (world
//! +0xF8) in the original's draw sites and order (docs/rng.md).
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

fn steps(from: u32, n: usize) -> u32 {
    let mut r = nfsu2::opponents::GameRng(from);
    for _ in 0..n {
        r.below(1);
    }
    r.0
}

/// The start-up (0x57ED20's chain after the seed 0x57ED5C): 120 lens
/// draws (0x57ED7C → 0x5C2FA0), the input binding's (0x5795BB), the
/// traffic list's first build (0x412840 with no world, 15 × 2): 151 shared
/// draws, as measured on the original with the sound off (c1 `boot`), then
/// the main menu's car render info (0x6383A5; the sequence log): 152.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_start_up_makes_the_originals_151_shared_draws_and_the_menu_cars() {
    testkit::game();
    let game = Game::new().unwrap();
    assert_eq!(game.sim.traffic_paints, vec![270; 15], "15 traffic types, 270 paints each (0x631A30)");
    assert_eq!(game.rng.0, steps(0x1234_5678, 152));
}

/// A world build (0x5FF4A0): the world seed (0x5FF557, the shared draw is
/// the world's state), the slot reset with the world (0x412840: 15 ×
/// {paint, shared, world}), the rain set-up of both views (0x61E100,
/// 3260): 3291 shared draws and 15 world draws.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_world_build_draws_seed_slots_and_rain_in_order() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.explore(None).unwrap();
    let before = game.rng.0;
    let world = game.world_build();
    assert_eq!(game.rng.0, steps(before, 1 + 30 + 3260));
    assert_eq!(world.0, steps(before % 0x7FFF_FFFF, 15), "the seed is the draw, then the 15 slots' world draws");
}
