//! The player's brake pedal through the countdown and
//! from the green.

use crate::*;

/// Native (**measured**, the player's input +0x214 per frame):
/// through the countdown the start's hold gives brake 1.0; from the green
/// the race's input object's analog brake byte +0x2D9 = 1 (0x407EB0,
/// × 1/255 at 0x407F24) gives 1/255 while no brake key is down — not 1/255
/// through the countdown and not 0 after it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn countdown_brake_is_full_and_the_green_leaves_one_255th() {
    let mut game = Game::new().unwrap();
    game.settings.custom[5] = 1;
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let event = game.quick_event(mode);
    game.quick.car = Some("350Z".into());
    game.start_race(event, true, false).unwrap();
    let (mut countdown, mut racing) = (0, 0);
    for _ in 0..60 * 30 {
        game.step_for(1.0 / 60.0).unwrap();
        let Mode::Race(race) = &game.mode else { panic!("race") };
        let scene_camera = game.scenes.shop.as_ref().is_some_and(|r| r.frame < r.scene.marks[1] as f32);
        let brake = game.sim.drive.as_ref().unwrap().state.brake_input;
        if race.timing.clock < 0.0 && !scene_camera {
            countdown += 1;
            assert_eq!(brake, 1.0, "the countdown's hold");
        } else if race.timing.clock > 0.1 {
            racing += 1;
            assert_eq!(brake, 0.003921569, "from the green: +0x2D9 = 1, × 1/255");
            if racing == 30 {
                break;
            }
        }
    }
    assert!(countdown > 60 && racing == 30, "countdown {countdown} racing {racing}");
}
