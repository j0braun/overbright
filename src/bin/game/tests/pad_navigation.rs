//! #305: a controller reaches what the keyboard reaches in driving scenes.

use super::*;
use nfsu2::pad::host::Btn;

fn pad_game() -> Game {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    game.settings.custom[5] = 1;
    let mut profile = Profile::new("PADNAV");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 0b12\n");
    game.advance_for(1.0 / 60.0).unwrap();
    assert_eq!(game.platform.pads.devices.len(), 1);
    game
}

fn tick(game: &mut Game) {
    game.pacing.clock = game.pacing.clock.map(|t| t + 1.0 / 60.0);
    game.advance_for(1.0 / 60.0).unwrap();
}

/// The race start scene is skipped by the Engage action, as the original's
/// handler takes it (Modern layout: d-pad up), not by the Enter key alone.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn pad_engage_skips_the_race_start_scene() {
    let frame_after = |press: bool| {
        let mut game = pad_game();
        game.rng = nfsu2::opponents::GameRng(4);
        let event = game
            .game_data
            .career
            .stage_events(&game.game_data.root, 1)
            .into_iter()
            .find(|e| e.kind == career::EventKind::Drag)
            .unwrap()
            .index;
        game.start_race(event, false, false).unwrap();
        let cut = game.scenes.shop.as_ref().expect("a start scene").scene.marks[1] as f32;
        for _ in 0..3 {
            tick(&mut game);
        }
        assert!(game.scenes.shop.as_ref().unwrap().frame < cut * 0.5, "still early in the scene");
        if press {
            game.platform.pads.devices[0].inputs.buttons[Btn::Up as usize] = true;
        }
        tick(&mut game);
        game.platform.pads.devices[0].inputs.buttons[Btn::Up as usize] = false;
        (game.scenes.shop.as_ref().map_or(f32::MAX, |s| s.frame), cut)
    };
    let (idle, cut) = frame_after(false);
    assert!(idle < cut * 0.5, "no press: the scene plays on");
    let (skipped, cut) = frame_after(true);
    assert!(skipped >= cut - 0.01, "d-pad up (Engage) skips to the cut: {skipped} of {cut}");
}
