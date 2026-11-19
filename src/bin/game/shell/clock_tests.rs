//! The front end on the tick clock and game time from ticks, through
//! the window's cadence (`Game::advance`) at 30 / 60 / 120 / 200 Hz.
use crate::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Exact display frames of `hz` (1/36 000 s units, as the rate gate's).
fn frame(game: &mut Game, hz: f32) {
    game.advance(1.0 / hz).unwrap();
}

/// A menu timer fires on the same tick at every display rate: LAN's search
/// dialog (5 s, 0x4FB610), opened on a tick, goes on the same tick at 30,
/// 60, 120 and 200 Hz; the package clock and `now` there are the same too.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn menu_timer_fires_on_the_same_tick_at_every_rate() {
    testkit::game();
    let mut game = Game::new().unwrap();
    assert!(game.pacing.clock.is_none());
    // Per rate: (ticks since the screen opened, whether the search ended)
    // after each drawn frame.
    let mut seen: Vec<(f32, Vec<(u64, bool)>)> = Vec::new();
    for hz in [60.0f32, 30.0, 120.0, 200.0] {
        // Open on a tick ≡ 0 (mod 6) with nothing in the accumulator, so
        // the tick pattern's phase is the same for every rate.
        while game.pacing.cadence.acc != 0 {
            frame(&mut game, 200.0);
        }
        while game.pacing.clocks.tick % 6 != 0 {
            frame(&mut game, 60.0);
        }
        game.act(Action::Lan, MenuKind::Main).unwrap();
        assert!(matches!(game.mode, Mode::Menu(Menu::LanServers(_, true))));
        let t0 = game.pacing.clocks.tick;
        let mut obs = Vec::new();
        for _ in 0..(7.0 * hz) as usize {
            frame(&mut game, hz);
            obs.push((game.pacing.clocks.tick - t0, matches!(game.mode, Mode::Menu(Menu::LanServers(_, false)))));
        }
        seen.push((hz, obs));
        game.mode = game.icon_menu(MenuKind::Main);
    }
    // 60 Hz: one tick a frame, so the firing tick exactly. 1.2 × 5000 =
    // 6000 package ms at 59 per 3 ticks: 305 ticks in (the timer reads the
    // clock before its own tick's step).
    let k = seen[0].1.iter().find(|o| o.1).expect("the search ends at 60 Hz").0;
    assert!((300..=310).contains(&k), "fired {k} ticks in");
    for (hz, obs) in &seen {
        assert!(obs.iter().any(|o| o.1), "{hz} Hz: the search never ended");
        for &(n, fired) in obs {
            assert_eq!(fired, n >= k, "{hz} Hz: {n} ticks in, ended {fired}; at 60 Hz it ends {k} ticks in");
        }
    }
}

/// The package clock in the front end, a world and back at 60 Hz: 20, 19,
/// 20 per tick from one carry; the world counter 0 at world create, +1 per
/// tick, frozen under the pause (the package clock still stepping),
/// 100 000 after the world goes.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn clocks_across_menu_world_pause_menu() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut steps = Vec::new();
    let run = |game: &mut Game, frames: usize, steps: &mut Vec<u32>| {
        for _ in 0..frames {
            let before = game.pacing.clocks.package_ms;
            frame(game, 60.0);
            assert_eq!(game.pacing.frame_updates, 1, "one tick per 1/60 s frame");
            steps.push((game.pacing.clocks.package_ms - before) as u32);
        }
    };
    run(&mut game, 60, &mut steps);
    assert!(game.pacing.clocks.world_counter >= 100_060);
    let mut profile = Profile::new("CLOCK_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    assert_eq!((game.pacing.clocks.world_counter, game.pacing.clocks.sim_clock), (0, 4000));
    run(&mut game, 30, &mut steps);
    assert_eq!(game.pacing.clocks.world_counter, 30);
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    let (counter, sim, real) = (game.pacing.clocks.world_counter, game.pacing.clocks.sim_clock, game.pacing.clocks.real);
    run(&mut game, 30, &mut steps);
    assert_eq!((game.pacing.clocks.world_counter, game.pacing.clocks.sim_clock), (counter, sim), "frozen in the pause");
    assert_eq!(game.pacing.clocks.real - real, 2000, "the real-time clock runs: 30 ticks = 0.5 s");
    assert!(game.pacing.clocks.last_pieces().iter().all(|&p| p <= 20));
    game.after_quit().unwrap();
    assert_eq!(game.pacing.clocks.world_counter, 100_000);
    game.mode = game.icon_menu(MenuKind::Main);
    run(&mut game, 30, &mut steps);
    assert_eq!(game.pacing.clocks.world_counter, 100_030);
    // The tick pattern runs on through every switch: each step is 20 or 19
    // and every 3 consecutive steps sum to 59 (one carry from boot).
    let tail = &steps[6..];
    assert!(tail.iter().all(|&s| s == 19 || s == 20), "{steps:?}");
    for w in tail.windows(3) {
        assert_eq!(w.iter().sum::<u32>(), 59, "{steps:?}");
    }
    // `now` is the package clock over 1.2.
    assert!((game.now() * 1.2 - game.pacing.clocks.package_ms as f32).abs() < 0.01);
}
