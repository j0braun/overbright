//! docs/uncapped-fps.md, "Draw clocks": the draw's clocks are
//! the tick snapshots' blended by α: S_{n−1}'s at α = 0 and S_n's at α = 1
//! bit for bit, monotonic over the drawn frames of any schedule, and the
//! same at 30 / 60 / 120 / 200 Hz at equal (tick, α). On a real free roam
//! (the player's game files).
use super::*;
use crate::render::clocks::DrawClocks;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Free roam in a moving car, the game's clocks unpinned.
fn explore() -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("CLOCKS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.pacing.clock = None;
    game
}

fn drive_on(game: &mut Game) {
    let d = game.sim.drive.as_mut().unwrap();
    d.state.velocity = d.state.orientation * Vec3::X * 25.0;
}

fn bits(c: &DrawClocks) -> (u32, u64, Option<u32>) {
    (c.seconds.to_bits(), c.blur.to_bits(), c.world.map(f32::to_bits))
}

/// α = 0: S_{n−1}'s clocks; α = 1: S_n's, bit for bit; the frame values
/// carry them (`FramePoses::clocks`).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn draw_clocks_are_the_ticks_at_the_ends() {
    testkit::game();
    let mut game = explore();
    for _ in 0..5 {
        drive_on(&mut game);
        game.advance(1.0 / 60.0).unwrap();
    }
    let (p, c) = (game.interp.prev.clocks, game.interp.cur.clocks);
    assert!(p.seconds < c.seconds && p.world_time < c.world_time, "the clocks move between the ticks");
    assert!(p.world.unwrap() < c.world.unwrap());
    let at = |a: f32| bits(&game.draw_clocks(a));
    assert_eq!(at(0.0), ((p.seconds as f32).to_bits(), p.world_time.to_bits(), p.world.map(|w| (w as f32).to_bits())));
    assert_eq!(at(1.0), ((c.seconds as f32).to_bits(), c.world_time.to_bits(), c.world.map(|w| (w as f32).to_bits())));
    // S_n is what the game stands at (use, then step): α = 1 is today's
    // 60 Hz value, the live clocks.
    assert_eq!(at(1.0), (game.seconds().to_bits(), game.pacing.world_time.to_bits(), Some(game.sim.drive.as_ref().unwrap().clock.to_bits())));
    assert_eq!(bits(&game.frame_poses(0.25).clocks), at(0.25));
    assert_eq!(bits(&game.frame_poses(1.0).clocks), at(1.0));
    // The harness's pinned clock is the shaders' as it stands.
    game.pacing.clock = Some(7.5);
    assert_eq!(game.draw_clocks(0.0).seconds, 7.5);
    game.pacing.clock = None;
    // A world clock set after the tick (a load's): drawn as set.
    game.sim.drive.as_mut().unwrap().clock = 1.0;
    assert_eq!(game.draw_clocks(0.0).world, Some(1.0));
}

/// The drawn frames' clocks never run back, at any schedule (across the
/// tick boundaries too), and above 60 Hz every frame moves on.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn draw_clocks_are_monotonic_over_the_frames() {
    testkit::game();
    for hz in [30.0f32, 60.0, 120.0, 144.0, 200.0, 0.0] {
        let mut game = explore();
        let mut last: Option<DrawClocks> = None;
        for i in 0..240u32 {
            drive_on(&mut game);
            // 0: a jittered schedule (4–40 ms).
            let interval = if hz > 0.0 { 1.0 / hz } else { 0.004 + ((i.wrapping_mul(2_654_435_761) >> 9) % 36) as f32 * 0.001 };
            let alpha = game.advance(interval).unwrap();
            let c = game.draw_clocks(alpha);
            if let Some(l) = last {
                assert!(c.seconds >= l.seconds, "{hz} Hz frame {i}: seconds {} < {}", c.seconds, l.seconds);
                // Smooth: above 60 Hz every drawn frame moves on, ticked
                // or not (the shaders' clock does not step per tick);
                // from the second tick on (the first has no S_{n−1}).
                if hz > 60.0 && game.interp.prev.tick > 1 {
                    assert!(c.seconds > l.seconds && c.blur > l.blur, "{hz} Hz frame {i}: the clocks stood still");
                }
                assert!(c.blur >= l.blur, "{hz} Hz frame {i}: blur {} < {}", c.blur, l.blur);
                assert!(c.world.unwrap() >= l.world.unwrap(), "{hz} Hz frame {i}: world");
            }
            last = Some(c);
        }
    }
}

/// At equal (tick, α) the clocks are the same at 30 / 60 / 120 / 200 Hz
/// (every tick pair a schedule's frames stand on, compared with 60 Hz's).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn draw_clocks_are_the_same_at_every_rate() {
    testkit::game();
    const ALPHAS: [f32; 5] = [0.0, 0.25, 0.5, 0.999, 1.0];
    type Row = [(u32, u64, Option<u32>); 5];
    let run = |hz: f32| -> std::collections::BTreeMap<u64, Row> {
        let mut game = explore();
        let mut out = std::collections::BTreeMap::new();
        let start = game.interp.cur.tick;
        while game.interp.cur.tick - start < 180 {
            drive_on(&mut game);
            game.advance(1.0 / hz).unwrap();
            out.insert(game.interp.cur.tick - start, ALPHAS.map(|a| bits(&game.draw_clocks(a))));
        }
        out
    };
    let reference = run(60.0);
    for hz in [30.0, 120.0, 200.0] {
        let r = run(hz);
        let common: Vec<u64> = r.keys().filter(|t| reference.contains_key(t)).copied().collect();
        assert!(common.len() >= 80, "{hz} Hz: {} ticks in common", common.len());
        for t in common {
            assert_eq!(r[&t], reference[&t], "{hz} Hz at tick {t}");
        }
    }
}
