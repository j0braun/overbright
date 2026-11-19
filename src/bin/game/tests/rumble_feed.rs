use crate::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The Rumble option (High) in a race, driven at full throttle for 20 s
/// of game time with one pad attached: the motors run (the gear changes,
/// the bumps and the road texture), and the same at 30, 60, 120 and 200
/// frames per second (the model steps on the 1/60 s world tick).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rumble_runs_in_a_race_at_any_frame_rate() {
    testkit::game();
    let mut seen = Vec::new();
    for fps in [30.0f32, 60.0, 120.0, 200.0] {
        let mut game = Game::new().unwrap();
        game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
        let event = game.quick_event(screens::QrMode::Circuit);
        game.quick.car = Some("240SX".into());
        game.start_race(event, true, false).unwrap();
        game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 028e\n");
        game.platform.pads.poll(0.0);
        game.settings.enh.pad.rumble = 100;
        game.platform.pads.set_options(game.settings.enh.pad);
        // Past the start scene and the countdown.
        for _ in 0..(40.0 * fps) as usize {
            if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.5) {
                break;
            }
            game.step_for(1.0 / fps).unwrap();
        }
        assert!(matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.0), "{fps}: racing");
        game.platform.keys.insert(KeyCode::ArrowUp);
        let (mut strong, mut weak, mut peak) = (0.0f32, 0.0f32, (0.0f32, 0.0f32));
        for _ in 0..(20.0 * fps) as usize {
            game.step_for(1.0 / fps).unwrap();
            let (s, w) = game.platform.pads.rumble_out.first().copied().unwrap_or_default();
            strong += s / fps;
            weak += w / fps;
            peak = (peak.0.max(s), peak.1.max(w));
        }
        eprintln!("{fps} fps: strong {strong:.3} s·level, weak {weak:.3} s·level, peak {peak:?}");
        assert!(weak > 0.0 || strong > 0.0, "{fps}: the motors ran");
        seen.push((strong, weak));
    }
    for &(s, w) in &seen[1..] {
        assert!((s - seen[0].0).abs() <= 0.05 + 0.1 * seen[0].0 && (w - seen[0].1).abs() <= 0.05 + 0.1 * seen[0].1, "{seen:?}");
    }
}
