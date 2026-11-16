//! docs/uncapped-fps.md, "Native visuals on ticks": the original's
//! per-frame visuals (overbright level, nitrous blur, screen tint, fog,
//! horizon fog, flare blink) step once per tick, so the values are the
//! same at every display rate at equal ticks, and the 60 Hz values are
//! the old per-frame path's, bit for bit (the tint one tick later: the
//! original samples it after the screen passes, 0x5CBAEC). On a real free
//! roam (the player's game files).
use super::*;
use crate::render::visuals::FrameVis;
use nfsu2::fog::FogState;
use nfsu2::horizon_fog::HorizonFog;
use nfsu2::motion_blur::MotionBlur;
use nfsu2::render::postfx::OverbrightLevel;
use nfsu2::screen_tint::ScreenTintState;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Free roam with every visual's state fresh, the gas held.
fn explore() -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("VISUALS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    d.fog = FogState::default();
    d.tint = ScreenTintState::default();
    d.horizon_fog = HorizonFog::default();
    d.motion = MotionBlur::default();
    d.vis = Default::default();
    d.flare_pools_clock = d.clock;
    game.visuals = Default::default();
    game.platform.keys.insert(KeyCode::KeyW);
    game
}

/// A frame's visuals as bits (the scalars a draw uses).
fn bits(v: &FrameVis) -> Vec<u32> {
    let mut b = vec![u32::from(v.tint.enabled), v.tint.bleach.to_bits(), v.tint.grey.to_bits()];
    b.extend(v.tint.color.map(f32::to_bits));
    match v.fog {
        Some(f) => b.extend(f.value.into_iter().chain(f.color).map(f32::to_bits)),
        None => b.push(u32::MAX),
    }
    b.extend(v.rain_color.map(f32::to_bits));
    b.push(v.horizon.map_or(u32::MAX, f32::to_bits));
    b.push(v.blur.level.to_bits());
    b.extend(v.blur.velocity.to_array().map(f32::to_bits));
    b.push(u32::from(v.blink));
    b.push(v.overbright.0.to_bits());
    b
}

/// 600 ticks at 60 Hz equal the old 600 drawn frames (each one step of the
/// per-frame entry points at the frame's camera, then drawn at once), bit
/// for bit; the tint is the old frame's of the tick before.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn ticks_equal_the_old_frames_at_60() {
    testkit::game();
    let mut game = explore();
    let (mut fog, mut tint, mut horizon, mut motion) =
        (FogState::default(), ScreenTintState::default(), HorizonFog::default(), MotionBlur::default());
    let mut pools = nfsu2::glow::Pools::default();
    let mut level = OverbrightLevel::default();
    let mut clock = game.sim.drive.as_ref().unwrap().clock;
    let mut old_tint = None;
    let (mut blurred, mut tinted) = (false, false);
    let switches = Switches::from_custom(&game.settings.custom);
    for n in 0..600 {
        game.advance(1.0 / 60.0).unwrap();
        let v = game.frame_vis(1.0);
        let d = game.sim.drive.as_ref().unwrap();
        let (cam, w) = (d.camera, d.time_of_day.fog_weight());
        let at = cam.position.truncate();
        // The old frame, from the same state.
        let dt = (d.clock - clock).max(0.0);
        clock = d.clock;
        pools.reset(dt);
        let t = tint.update(&d.world, at, w);
        let f = fog.update_rain(&d.world, at, w, d.weather.amount);
        motion.camera(cam.position, game.pacing.last_update_dt);
        let axes = CameraAxes::look(cam.forward(), cam.up);
        let nitrous = crate::render::visuals::nitrous_blur(&d.state);
        let blur = if native_speed_blur(switches, nfsu2::qol::motion_blur) {
            motion.frame(nitrous, (axes.right, axes.down, axes.forward))
        } else {
            None
        };
        let hv = horizon.step(&d.world, at);
        let ob = level.frame(false);
        assert_eq!(v.blink, pools.blink, "tick {n}: blink");
        assert_eq!(
            v.fog.map(|f| (f.value.map(f32::to_bits), f.color.map(f32::to_bits))),
            f.map(|f| (f.value.map(f32::to_bits), f.color.map(f32::to_bits))),
            "tick {n}: fog"
        );
        assert_eq!(v.rain_color.map(f32::to_bits), fog.rain_color.map(f32::to_bits), "tick {n}: rain colour");
        if switches.horizon_fog {
            assert_eq!(v.horizon.map(f32::to_bits), Some(hv.to_bits()), "tick {n}: horizon fog");
        }
        if native_speed_blur(switches, nfsu2::qol::motion_blur) {
            assert_eq!(crate::render::native::motion_blur::motion_blur_frame(&v.blur, &cam), blur, "tick {n}: blur");
        }
        assert_eq!(v.overbright.0.to_bits(), ob.0.to_bits(), "tick {n}: overbright");
        let want = old_tint.unwrap_or(nfsu2::screen_tint::TintSample::drawn(
            &nfsu2::screen_tint::TintSample::UNSAMPLED,
            &nfsu2::screen_tint::TintSample::UNSAMPLED,
            0.0,
        ));
        assert_eq!(v.tint, want, "tick {n}: tint (the old frame's of the tick before)");
        blurred |= v.blur.level > 0.0;
        tinted |= v.tint.enabled;
        old_tint = Some(t);
    }
    assert!(switches.horizon_fog && native_speed_blur(switches, nfsu2::qol::motion_blur), "the default switches");
    // (The car of this segment has no nitrous fitted: the level stays 0;
    // its steps are the library's tests, `motion_blur` tick_tests.)
    eprintln!("blur level above 0 on some tick: {blurred}; tint enabled on some tick: {tinted}");
}

/// The visuals at every tick of a scripted drive, drawn at `hz` (each
/// frame drawn between its ticks), by tick.
fn run(hz: f32, ticks: u64) -> std::collections::BTreeMap<u64, Vec<u32>> {
    let mut game = explore();
    let mut out = std::collections::BTreeMap::new();
    while game.interp.cur.tick < ticks {
        let alpha = game.advance(1.0 / hz).unwrap();
        // Frames between ticks: within S_{n−1}, S_n and S_{n−1} at α = 0.
        let (p, c) = (game.frame_vis(0.0), game.frame_vis(1.0));
        if game.interp.prev.tick > 0 {
            let prev = game.interp.prev.vis.unwrap();
            assert_eq!(bits(&p), bits(&FrameVis::live(Some(&prev), game.interp.prev.overbright)));
        }
        let m = game.frame_vis(alpha);
        assert!(
            nfsu2::render::native::fx::tick::within(m.overbright.0, p.overbright.0, c.overbright.0)
                && nfsu2::render::native::fx::tick::within(m.blur.level, p.blur.level, c.blur.level)
                && nfsu2::render::native::fx::tick::within(m.tint.grey, p.tint.grey, c.tint.grey),
            "{hz} Hz, tick {}: α {alpha} outside the two ticks",
            game.interp.cur.tick
        );
        game.draw_between(alpha).unwrap();
        out.insert(game.interp.cur.tick, bits(&c));
    }
    out
}

/// At 30, 120 and 200 Hz the visuals equal 60 Hz's at every tick both
/// runs have (30 Hz: every second one).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn every_rate_steps_the_same_visuals() {
    testkit::game();
    let reference = run(60.0, 600);
    for hz in [30.0, 120.0, 200.0] {
        let other = run(hz, 600);
        let mut common = 0;
        for (tick, v) in &other {
            if let Some(r) = reference.get(tick) {
                assert_eq!(v, r, "{hz} Hz differs from 60 Hz at tick {tick}");
                common += 1;
            }
        }
        assert!(common >= 290, "{hz} Hz: {common} ticks compared");
    }
}
