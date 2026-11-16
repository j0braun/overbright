//! The HUD's texture list read at the load
//! (the HD UI pack's first HUD frame).
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The list `drive_in` prefetches covers every texture the HUD's first
/// frame draws (so no HD UI art is made in that frame) and the track map.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_hud_texture_list_covers_the_first_hud_frame() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("GS2_HUD");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let list = game.hud_textures(RaceMode::FreeRoam, 4000);
    assert!(list.windows(2).all(|w| w[0] < w[1]), "sorted, once each");
    assert!(list.contains(&nfsu2::formats::hash::bin_hash("TRACKMAP4000")), "the free-roam map");
    let now = game.now();
    let drawn: std::collections::BTreeSet<u32> = game.hud(now).iter().filter_map(|q| q.texture).collect();
    assert!(!drawn.is_empty());
    let missing: Vec<&str> = drawn
        .iter()
        .filter(|h| list.binary_search(h).is_err())
        .map(|h| game.game_data.assets.textures.get(h).map_or("?", |t| t.name.as_str()))
        .collect();
    assert!(missing.is_empty(), "first HUD frame textures not in the list: {missing:?}");
}
