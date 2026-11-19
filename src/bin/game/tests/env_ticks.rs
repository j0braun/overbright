//! docs/original/uncapped-fps.md, "Environment cube on ticks": the cars'
//! environment cube follows the ticks. On a real free roam (the player's
//! game files), through the real draw path on an offscreen window: the
//! cube a frame samples (a digest of its six faces read back) at a tick
//! is the same at 30 / 60 / 120 / 200 Hz, and a frame without a new tick
//! renders no face.
use super::*;
use crate::dev::ticks::Frames;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Free roam in a car moving at 25 m/s (three 60 Hz ticks on), Car
/// Reflection Update Rate `rate`, an offscreen window.
fn moving_explore(rate: u8) -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ENV_TICKS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.settings.custom[0] = rate;
    game.explore(None).unwrap();
    for _ in 0..3 {
        let d = game.sim.drive.as_mut().unwrap();
        d.state.velocity = d.state.orientation * Vec3::X * 25.0;
        game.advance_ticks(1).unwrap();
    }
    super::snapshots::offscreen_window(&mut game, 160, 120);
    game
}

/// The cube's digest after the α = 0 frames of the ticks ≡ 0 (mod 6) from the start
/// from 24 ticks on (every face rendered since the first draw), over
/// `ticks` ticks of frames at `hz`; and the faces rendered by frames
/// without a new tick (must be none).
fn cube_at_ticks(rate: u8, hz: f32, ticks: u64) -> (Vec<(u64, u64)>, usize) {
    let mut game = moving_explore(rate);
    let start = game.pacing.clocks.tick;
    let mut frames = Frames::new(hz);
    let mut out = Vec::new();
    let mut idle_faces = 0usize;
    while game.pacing.clocks.tick < start + ticks {
        frames
            .tick_with(&mut game, &mut |g, d| {
                let before = g.render.world.as_ref().and_then(|r| r.env_tick);
                let updates = g.pacing.frame_updates;
                g.draw_between(d.alpha)?;
                let after = g.render.world.as_ref().and_then(|r| r.env_tick);
                if updates == 0 && before.is_some() && before != after {
                    idle_faces += 1;
                }
                let n = g.pacing.clocks.tick;
                if d.alpha < 1e-4 && n >= start + 24 && (n - start) % 6 == 0 {
                    let w = g.platform.window.as_mut().unwrap();
                    w.settle();
                    let digest = g.render.world.as_ref().unwrap().environment_digest(&w.gpu);
                    out.push((n, digest));
                }
                Ok(())
            })
            .unwrap();
    }
    (out, idle_faces)
}

/// 30 Hz frame k and 120 / 200 Hz frames at α = 0 sample the cube of
/// 60 Hz frame 2k / the same tick: the faces' digests agree at every
/// compared tick, at rates 1 (one face a tick) and 3 (all six); frames
/// between ticks render none.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn cube_is_the_same_at_every_display_rate() {
    testkit::game();
    for rate in [1u8, 3] {
        let (reference, _) = cube_at_ticks(rate, 60.0, 240);
        assert!(reference.len() >= 10, "rate {rate}: {} compared ticks", reference.len());
        let distinct = reference.iter().map(|r| r.1).collect::<std::collections::HashSet<_>>().len();
        assert!(distinct > 1, "rate {rate}: the cube changes as the car moves");
        for hz in [30.0f32, 120.0, 200.0] {
            let (got, idle) = cube_at_ticks(rate, hz, 240);
            assert_eq!(idle, 0, "rate {rate} {hz} Hz: a frame without a tick rendered faces");
            for (n, d) in &got {
                if let Some((_, r)) = reference.iter().find(|(m, _)| m == n) {
                    assert_eq!(d, r, "rate {rate} {hz} Hz tick {n}: the cube differs from 60 Hz");
                }
            }
            let compared = got.iter().filter(|(n, _)| reference.iter().any(|(m, _)| m == n)).count();
            eprintln!("rate {rate} {hz} Hz: {compared} ticks compared, {} at 60 Hz", reference.len());
            assert!(compared >= 10, "rate {rate} {hz} Hz: {compared} ticks compared");
        }
    }
}
