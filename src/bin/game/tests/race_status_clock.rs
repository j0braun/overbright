//! The race status's clock +0x14 and the NIS sound
//! object's backup line cleanups that read it (0x475330 → 0x4753E5).
//! Real assets, offline mixer, production `Game::step` and Enter routing.

use crate::*;

fn quick_circuit() -> Game {
    let mut game = Game::new().unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.settings.custom[5] = 1;
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let event = game.quick_event(mode);
    game.quick.car = Some("350Z".into());
    game.start_race(event, true, false).unwrap();
    assert!(game.scenes.shop.as_ref().is_some_and(|r| matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Intro(_))));
    game
}

fn status(game: &Game) -> (i32, f32) {
    let Mode::Race(race) = &game.mode else { panic!("race") };
    race.status(game.scenes.shop.as_ref().is_some_and(|run| run.frame < run.scene.marks[1] as f32))
}

fn armed(game: &Game) -> bool {
    game.audio.as_ref().unwrap().streams.backup_armed()
}

/// Native (0x6076E0 / 0x5FC910(4) / 0x577410, measured on two
/// runs): +0x14 is 0 through the start scene (state 2) and the countdown
/// (state 3); at the green +0x10 = the tick rounded half up to 0.01 s, and
/// while racing +0x14 = tick − +0x10 rounded half up to 0.01 s (a multiple
/// of 40 ticks). The backup line armed as the start line starts (0x46CD20)
/// is cleaned up in state 3 within 15 s once the speech stream is silent
/// (0x4753FB..0x475475); skipping the scene (0x479200) only shortens the
/// scene, the clock's zero stays the green.
fn run(skip: bool) {
    let mut game = quick_circuit();
    assert!(armed(&game), "the race start's line arms the backup (0x46CD20)");
    let dt = 1.0 / 60.0;
    let (mut saw2, mut saw3, mut green) = (false, false, None::<i32>);
    let mut pressed = false;
    for _ in 0..60 * 40 {
        if skip && !pressed && game.scenes.shop.as_ref().is_some_and(|r| r.frame > 10.0) {
            // Through the input queue at the next tick.
            game.tap_next(KeyCode::Enter);
            pressed = true;
        }
        game.step_for(dt).unwrap();
        let (state, secs) = status(&game);
        let Mode::Race(race) = &game.mode else { panic!("race") };
        match state {
            2 => {
                saw2 = true;
                assert_eq!(secs, 0.0, "+0x14 is 0 in the scene");
            }
            3 => {
                saw3 = true;
                assert_eq!(secs, 0.0, "+0x14 is 0 in the countdown");
                // Rachel is silent: the first state-3 update cleans up.
                assert!(!armed(&game), "state 3, speech silent: the backup is cleaned up");
            }
            _ => {
                let tick = (game.sim.drive.as_ref().unwrap().clock as f64 * 4000.0).round() as i32;
                let g = *green.get_or_insert(race.timing.green_tick.expect("the green stamp"));
                assert_eq!(g % 40, 0);
                assert_eq!(race.timing.status_clock, nfsu2::standings::round_centi(tick - g));
                assert_eq!(race.timing.status_clock % 40, 0);
                assert!((secs - race.timing.clock).abs() <= dt + 0.005, "{secs} vs {}", race.timing.clock);
                if race.timing.clock > 2.0 {
                    break;
                }
            }
        }
    }
    assert!(saw2 && saw3 && green.is_some(), "scene {saw2} countdown {saw3} green {green:?}");
    assert_eq!(pressed, skip);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_clock_and_backup_cleanup_played() {
    run(false);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_clock_and_backup_cleanup_skipped() {
    run(true);
}

/// 0x475431..0x475475: past 15.0 s (0x787AF4) on +0x14 an armed backup is
/// cleaned up in any state; at 15 s or less only state 3 cleans up. The
/// game passes the race status (state, +0x14 × 0.00025) each audio update.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn backup_cleanup_after_fifteen_seconds() {
    let mut game = quick_circuit();
    assert!(armed(&game));
    let Mode::Race(race) = &mut game.mode else { panic!("race") };
    race.timing.clock = 14.99;
    race.timing.status_clock = 59_960;
    game.stream_sounds(1.0 / 60.0);
    assert_eq!(status(&game).0, 4);
    assert!(armed(&game), "state 4 at 14.99 s: kept");
    // 60 000 ticks × 0.00025 (f32, as the x87 at 24-bit precision rounds
    // it) is 15.000001 > 15.0: the first clock value past the window.
    let Mode::Race(race) = &mut game.mode else { panic!("race") };
    race.timing.status_clock = 60_000;
    game.stream_sounds(1.0 / 60.0);
    assert!(!armed(&game), "past 15 s: cleaned up");
}
