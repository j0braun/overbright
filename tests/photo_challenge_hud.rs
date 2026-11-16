mod common;

use nfsu2::frontend::{Assets, drawn_texts, hud::{Hud, Input}};
use nfsu2::formats::hash::fe_hash;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn photo_challenge_timer_emits_visible_glyphs_across_updates() {
    let root = common::game_root();
    let assets = Assets::load(&root).expect("installed game assets");
    for remaining in [120.0, 9.5] {
        let mut hud = Hud::default();
        let input = Input { career: true, challenge: Some(remaining), race: nfsu2::race::RaceMode::FreeRoam, ..Default::default() };
        hud.quads(&assets, &input, 0.0);
        for now in [250.0, 1200.0] {
            let drawn = drawn_texts(|| { hud.quads(&assets, &input, now); });
            assert!(drawn.iter().any(|(h, _)| *h == fe_hash("CurrentLapTime_2")),
                "timer {remaining}s must draw visible glyphs at {now}ms, not just store text");
        }
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn photo_marker_keeps_its_icon_after_the_initial_prompt_phase() {
    let root = common::game_root();
    let assets = Assets::load(&root).expect("installed game assets");
    for kind in [0x11, 0x12] {
        let mut hud = Hud::default();
        let input = Input { career: true, engage: Some(kind), race: nfsu2::race::RaceMode::FreeRoam, ..Default::default() };
        let icon = nfsu2::frontend::hud::engage_texture(kind).unwrap();
        let icon = assets.in_game.get(&icon).copied().unwrap_or(icon);
        let mut visible = [0; 2];
        for frame in 0..60 {
            let now = frame as f32 * 50.0;
            let quads = hud.quads(&assets, &input, now);
            if quads.iter().any(|q| q.texture == Some(icon) && q.color[3] > 0.0) {
                visible[usize::from(now > 1000.0)] += 1;
            }
        }
        assert!(visible[0] > 0, "marker {kind:x} initial icon must render");
        assert!(visible[1] > 0, "marker {kind:x} icon must survive the initial prompt phase");
    }
}
