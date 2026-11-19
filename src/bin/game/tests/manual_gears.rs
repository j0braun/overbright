//! The manual gearbox through the player's real input paths: a bound key's
//! press queued as the window queues it (`Game::input_at` → `Game::key`)
//! and a controller button read by `Game::pad_frame`, in free roam. The
//! original's shift handlers belong to the human input (0x407FC0 /
//! 0x4080B0, registered by its constructor at 0x41E19F…0x41E1F2 for events
//! 0x9B / 0x9D and 0x9C / 0x9E), so they shift wherever the player drives;
//! the automatic gearbox ignores them (mode 0 / 2 never reaches the
//! request).
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

use nfsu2::handling::NEUTRAL;
use nfsu2::pad::host::Btn;

const FIRST: usize = NEUTRAL + 1;

/// Free roam in a fresh career, the Transmission option as given.
fn free_roam(manual: bool) -> Game {
    let mut game = Game::new().unwrap();
    game.settings.players[0].manual = manual;
    let mut profile = Profile::new("MANUAL_GEARS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    assert!(matches!(game.mode, Mode::Explore));
    game
}

fn gear(game: &Game) -> usize {
    game.sim.drive.as_ref().and_then(|d| d.state.drivetrain.as_ref()).map_or(usize::MAX, |t| t.gear)
}

fn ticks(game: &mut Game, n: usize) {
    for _ in 0..n {
        game.advance(1.0 / 60.0).unwrap();
    }
}

/// A key pressed and released as the window delivers it, then a second
/// for the gearbox to finish the change.
fn tap(game: &mut Game, code: KeyCode) {
    let tick = game.pacing.clocks.tick;
    game.input_at(tick, InputEvent::press(code));
    game.input_at(tick + 3, InputEvent::KeyUp(code));
    ticks(game, 60);
}

/// A controller button pressed for three ticks, then a second.
fn press_button(game: &mut Game, b: Btn) {
    game.platform.pads.devices[0].inputs.buttons[b as usize] = true;
    ticks(game, 3);
    game.platform.pads.devices[0].inputs.buttons[b as usize] = false;
    ticks(game, 57);
}

/// Transmission Manual in free roam: the car stands in first gear, the
/// default Shift Up / Shift Down keys (Left Shift / Left Ctrl) and keys
/// rebound on the Controls page step the gear as the original's handlers
/// do: first → neutral, never past neutral into reverse (observed under
/// Wine: 1st → N → N → 1st).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn free_roam_manual_shifts_on_the_keyboard() {
    testkit::game();
    let mut game = free_roam(true);
    assert!(game.sim.drive.as_ref().unwrap().state.manual, "the Transmission option reaches the car");
    ticks(&mut game, 30);
    assert_eq!(gear(&game), FIRST, "a manual car starts free roam in first gear");
    let steps = [
        (KeyCode::ShiftLeft, FIRST + 1),
        (KeyCode::ShiftLeft, FIRST + 2),
        (KeyCode::ControlLeft, FIRST + 1),
        (KeyCode::ControlLeft, FIRST),
        (KeyCode::ControlLeft, NEUTRAL),
        (KeyCode::ControlLeft, NEUTRAL),
        (KeyCode::ShiftLeft, FIRST),
    ];
    for (n, (key, want)) in steps.into_iter().enumerate() {
        tap(&mut game, key);
        assert_eq!(gear(&game), want, "step {n}: {key:?}");
    }
    // Rebound on the Controls page (Shift Up E, Shift Down Q).
    game.settings.bind(options::action::SHIFT_UP, 0, "E");
    game.settings.bind(options::action::SHIFT_DOWN, 0, "Q");
    tap(&mut game, KeyCode::KeyE);
    assert_eq!(gear(&game), FIRST + 1, "the rebound Shift Up key");
    tap(&mut game, KeyCode::KeyQ);
    assert_eq!(gear(&game), FIRST, "the rebound Shift Down key");
}

/// The same through a controller (Modern layout: R1 Shift Up, L1 Shift
/// Down), read by the pad frame.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn free_roam_manual_shifts_on_a_controller() {
    testkit::game();
    let mut game = free_roam(true);
    game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 0b12\n");
    ticks(&mut game, 30);
    assert_eq!(game.platform.pads.devices.len(), 1);
    assert_eq!(gear(&game), FIRST);
    press_button(&mut game, Btn::R1);
    assert_eq!(gear(&game), FIRST + 1, "R1 shifts up");
    press_button(&mut game, Btn::R1);
    assert_eq!(gear(&game), FIRST + 2, "R1 shifts up");
    press_button(&mut game, Btn::L1);
    assert_eq!(gear(&game), FIRST + 1, "L1 shifts down");
}

/// Transmission Automatic: the shift keys change nothing; the run with
/// them pressed equals the run without, gear, target gear and engine
/// speed, tick by tick, under the gas.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn free_roam_automatic_ignores_the_shift_keys() {
    testkit::game();
    let run = |shifts: bool| {
        let mut game = free_roam(false);
        assert!(!game.sim.drive.as_ref().unwrap().state.manual);
        game.platform.keys.insert(KeyCode::ArrowUp);
        let mut trace = Vec::new();
        for n in 0..360 {
            if shifts && n % 40 == 10 {
                let code = if n % 80 == 10 { KeyCode::ControlLeft } else { KeyCode::ShiftLeft };
                let tick = game.pacing.clocks.tick;
                game.input_at(tick, InputEvent::press(code));
                game.input_at(tick + 2, InputEvent::KeyUp(code));
            }
            game.advance(1.0 / 60.0).unwrap();
            let t = game
                .sim
                .drive
                .as_ref()
                .unwrap()
                .state
                .drivetrain
                .as_ref()
                .map(|t| (t.gear, t.target_gear, t.rpm().to_bits()));
            trace.push(t);
        }
        trace
    };
    let (plain, shifted) = (run(false), run(true));
    assert!(plain.iter().flatten().all(|t| t.0 >= NEUTRAL), "the automatic gearbox drives forward");
    assert_eq!(plain, shifted);
}
