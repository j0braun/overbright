//! The streak buffer on ticks (docs/uncapped-fps.md, "Streak buffer on ticks"): the light-streak
//! buffer commits once per tick, so a frame drawn at α = 0 is the same
//! picture at any display rate. A real free roam (the game files under $NFSU2_ROOT,
//! Original preset: light trails and the overbright pass on), the car
//! carried along its heading, drawn offscreen at 30 / 60 / 120 / 200 Hz;
//! every α = 0 frame's image against the 60 Hz frame of the same tick.
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

const SIZE: (u32, u32) = (320, 240);

fn digest(pixels: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(pixels, &mut h);
    std::hash::Hasher::finish(&h)
}

/// One run at `hz` (0: uneven 1–4 ms frames): per tick whose frame was
/// drawn at α = 0, a digest of that frame's image; per committed slot (the
/// newest tick a commit stood after), the streak buffer as the next drawn
/// frame left it (a frame between ticks puts the committed one back).
#[allow(clippy::type_complexity)]
fn run(hz: f32, ticks: u64) -> (std::collections::BTreeMap<u64, u64>, std::collections::BTreeMap<u64, u64>) {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("STREAK_TICKS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    // The load's state drawn once at every rate (the harness ticked the
    // load without frames; a 120 Hz run's first frame would come before
    // the first tick and draw it, a 60 Hz one would not).
    game.draw().unwrap();
    let start = game.pacing.clocks.tick;
    let mut frames = crate::dev::ticks::Frames::new(hz);
    let mut out = std::collections::BTreeMap::new();
    let mut buffers = std::collections::BTreeMap::new();
    let (p0, dir) = {
        let d = game.sim.drive.as_ref().unwrap();
        (d.state.position, d.state.orientation * Vec3::X)
    };
    while game.pacing.clocks.tick < start + ticks {
        let k = (game.pacing.clocks.tick - start) as f32;
        let d = game.sim.drive.as_mut().unwrap();
        d.state.position = p0 + dir * (25.0 / 60.0) * k + Vec3::Z * 0.3;
        d.state.velocity = dir * 25.0;
        frames
            .tick_with(&mut game, &mut |game, f| {
                game.draw_between(f.alpha)?;
                let slot = game.render.streak.ticks.seen.map(|s| s - start);
                let w = game.platform.window.as_mut().unwrap();
                if f.alpha < 1e-4 && game.pacing.frame_updates > 0 {
                    w.settle();
                    let pixels = nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), SIZE.0, SIZE.1);
                    out.insert(game.pacing.clocks.tick - start, digest(&pixels));
                }
                if let Some(slot) = slot.filter(|s| !buffers.contains_key(s)) {
                    w.settle();
                    let Some(t) = w.post.as_ref().and_then(|p| p.dev_streak_texture()) else { return Ok(()) };
                    let (width, height) = (t.size().width, t.size().height);
                    buffers.insert(slot, digest(&nfsu2::render::read_back(&w.gpu, t, width, height)));
                }
                Ok(())
            })
            .unwrap();
    }
    (out, buffers)
}

/// α = 0 frames at 30 / 120 / 200 Hz are the 60 Hz frames of their tick,
/// bit for bit, with the light trails on (with per-frame commits every one differed;
/// with them off they were already equal).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn alpha_zero_frames_are_the_60_hz_frames_with_light_trails() {
    testkit::game();
    super::test_isolation::exclusive();
    let ticks = 300;
    assert!(Switches::from_custom(&Game::new().unwrap().settings.custom).streaks(), "light trails on by default");
    let (reference, committed) = run(60.0, ticks);
    assert!(reference.len() as u64 >= ticks - 2, "60 Hz draws every tick at α = 0");
    let distinct = reference.values().collect::<std::collections::BTreeSet<_>>().len();
    assert!(distinct > 100, "the picture moved ({distinct} distinct frames)");
    for hz in [30.0, 120.0, 200.0] {
        let (other, bufs) = run(hz, ticks);
        let off: Vec<u64> = bufs.iter().filter(|(k, v)| committed.get(k).is_some_and(|r| r != *v)).map(|(k, _)| *k).collect();
        assert!(off.is_empty(), "{hz} Hz: the committed buffer after slots {off:?} differs from 60 Hz");
        let mut common = 0;
        let mut differ = Vec::new();
        for (tick, v) in &other {
            if let Some(r) = reference.get(tick) {
                if v != r {
                    differ.push(*tick);
                }
                common += 1;
            }
        }
        assert!(differ.is_empty(), "{hz} Hz: the α = 0 frames of ticks {differ:?} differ from 60 Hz ({common} in common)");
        assert!(common as u64 >= ticks / 4, "{hz} Hz: {common} α = 0 frames in common");
        eprintln!("streak ticks: {hz} Hz identical to 60 Hz at {common} α = 0 frames");
    }
}

/// Uneven frames (1–4 ms, the harness's "uncapped"): next to no frame lands
/// on a tick, so nearly every slot is committed by a probe; the committed
/// buffer after each slot is the 60 Hz one.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn committed_buffer_is_the_60_hz_one_on_uneven_frames() {
    testkit::game();
    super::test_isolation::exclusive();
    let ticks = 240;
    let (_, reference) = run(60.0, ticks);
    let (_, uneven) = run(0.0, ticks);
    let mut common = 0;
    let mut differ = Vec::new();
    for (slot, v) in &uneven {
        if let Some(r) = reference.get(slot) {
            if v != r {
                differ.push(*slot);
            }
            common += 1;
        }
    }
    assert!(differ.is_empty(), "uneven frames: the buffer after slots {differ:?} differs from 60 Hz ({common} in common)");
    assert!(common as u64 >= ticks - 4, "{common} slots in common");
    eprintln!("streak ticks: uneven frames' buffer identical to 60 Hz at {common} slots");
}

/// Every drawn frame from `from` to `to` (ticks after the start) at 144 Hz,
/// the form switched to the cheap one at tick `switch` (None: exact all
/// along): (tick, the newest committed slot, the image).
#[allow(clippy::type_complexity)]
fn frames_across(switch: Option<u64>, from: u64, to: u64) -> Vec<(u64, Option<u64>, Vec<u8>)> {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("STREAK_TICKS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    game.draw().unwrap();
    let start = game.pacing.clocks.tick;
    let mut frames = crate::dev::ticks::Frames::new(144.0);
    let mut out = Vec::new();
    let (p0, dir) = {
        let d = game.sim.drive.as_ref().unwrap();
        (d.state.position, d.state.orientation * Vec3::X)
    };
    while game.pacing.clocks.tick < start + to {
        let k = game.pacing.clocks.tick - start;
        if switch == Some(k) {
            // The developer knob, read again as a new game would.
            unsafe { std::env::set_var("NFSU2_STREAK_FORM", "cheap") };
            crate::shell::debug_env::reload_debug_env();
        }
        let d = game.sim.drive.as_mut().unwrap();
        d.state.position = p0 + dir * (25.0 / 60.0) * k as f32 + Vec3::Z * 0.3;
        d.state.velocity = dir * 25.0;
        frames
            .tick_with(&mut game, &mut |game, f| {
                game.draw_between(f.alpha)?;
                let tick = game.pacing.clocks.tick - start;
                if tick >= from {
                    let slot = game.render.streak.ticks.seen.map(|s| s - start);
                    let w = game.platform.window.as_mut().unwrap();
                    w.settle();
                    out.push((tick, slot, nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), SIZE.0, SIZE.1)));
                }
                Ok(())
            })
            .unwrap();
    }
    unsafe { std::env::remove_var("NFSU2_STREAK_FORM") };
    crate::shell::debug_env::reload_debug_env();
    out
}

/// A switch from the exact to the cheap form mid-drive (144 Hz): the
/// frames before it are the exact run's; after it the commits per tick are
/// the same (one per tick, the same slots). The cheap form's pictures
/// differ from the exact run's near the moving lights (the commits'
/// sub-tick phase), but the switch is no jump: the change from one frame
/// to the next across it stays within what the exact run's frames change
/// by anyway.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_switch_to_the_cheap_form_is_not_visible() {
    testkit::game();
    super::test_isolation::exclusive();
    let (switch, from, to) = (120, 110, 160);
    let exact = frames_across(None, from, to);
    let cheap = frames_across(Some(switch), from, to);
    assert_eq!(exact.len(), cheap.len(), "the same frames");
    // Mean |Δ| per channel (of 255) and pixels differing between two images.
    let delta = |a: &[u8], b: &[u8]| {
        let (mut sum, mut n) = (0u64, 0usize);
        for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            let d: Vec<u8> = pa[..3].iter().zip(&pb[..3]).map(|(x, y)| x.abs_diff(*y)).collect();
            sum += d.iter().map(|&v| u64::from(v)).sum::<u64>();
            n += usize::from(d.iter().any(|&v| v > 0));
        }
        (sum as f64 / (a.len() / 4 * 3) as f64, n)
    };
    let (mut exact_step, mut cheap_step) = (0.0f64, 0.0f64);
    for i in 0..exact.len() {
        let ((t, sa, a), (tb, sb, b)) = (&exact[i], &cheap[i]);
        assert_eq!((t, sa), (tb, sb), "the same ticks and committed slots");
        let (vs, n) = delta(a, b);
        if *t < switch {
            assert_eq!(n, 0, "tick {t}: before the switch, the exact run's frame");
        }
        let (de, dc) = if i > 0 { (delta(&exact[i - 1].2, a).0, delta(&cheap[i - 1].2, b).0) } else { (0.0, 0.0) };
        eprintln!("streak switch: tick {t}: frame-to-frame |Δ| exact {de:.3} cheap {dc:.3}; cheap vs exact {vs:.3} ({n} px)");
        exact_step = exact_step.max(de);
        cheap_step = cheap_step.max(dc);
    }
    assert!(cheap_step <= 1.25 * exact_step, "a jump at the switch: {cheap_step:.3} against {exact_step:.3}");
}
