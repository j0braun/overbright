//! The fog's and the rain's faster passes draw the frames their plain forms draw (docs/atmosphere.md "Cost",
//! docs/weather.md "Cost"): free roam at night in the rain with Volumetric Fog Next-Gen, Rain Detail
//! Next-Gen, the lens drops and High Precision Post, on the real draw path (an offscreen window), each pass's
//! plain form switched on in one run and off in the other: the same frames, byte for byte where the pass does the
//! same arithmetic (the tint's fold and the march's specialisation: to the rounding, their tests say how far).
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The rows the runs take: the Next-Gen fog and rain, the lens drops (with the native rain splatter that
/// carries them), High Precision Post, the spray and its volume, the smoke's tier.
const ROWS: &[(&str, &str)] = &[
    ("enh.volumetric_fog", "3"),
    ("enh.rain_detail", "3"),
    ("enh.lens_drops", "1"),
    ("enh.hdr_post", "2"),
    ("enh.spray", "1"),
    ("enh.wet_interaction", "2"),
    ("enh.wet_roads", "1"),
    ("enh.particles", "2"),
    ("enh.dynamic_lamps", "1"),
    ("enh.lamp_shadows", "1"),
];

const SIZE: (u32, u32) = (480, 270);
const FRAMES: usize = 120;

/// One run with `plain` set by `switch` (on, then back off) and `extra` rows over [`ROWS`]: free roam in the
/// rain, the gas held, [`FRAMES`] frames at 60 Hz. -> each frame's pixels.
fn frames(switch: fn(bool), plain: bool, extra: &[(&str, &str)]) -> Vec<Vec<u8>> {
    frames_with(switch, plain, extra, true)
}

/// [`frames`] with the native Rain Splatter on or off (`splatter`).
fn frames_with(switch: fn(bool), plain: bool, extra: &[(&str, &str)], splatter: bool) -> Vec<Vec<u8>> {
    frames_custom(switch, plain, extra, &[(10, u8::from(splatter)), (19, u8::from(splatter))])
}

/// [`frames`] with the native rows `custom` (index, value) over the Rain Splatter's (custom 10 and 19, on).
fn frames_custom(switch: fn(bool), plain: bool, extra: &[(&str, &str)], custom: &[(usize, u8)]) -> Vec<Vec<u8>> {
    switch(plain);
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("FOG_RAIN_PATHS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    for (k, v) in ROWS.iter().chain(extra) {
        game.settings.enh.parse(k, v);
    }
    // The native Rain Splatter (custom 10 and 19): the lens pass the drops are drawn by.
    game.settings.custom[10] = 1;
    game.settings.custom[19] = 1;
    for &(i, v) in custom {
        game.settings.custom[i] = v;
    }
    game.settings.apply_enhancements();
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    game.explore(None).unwrap();
    game.platform.keys.insert(KeyCode::KeyW);
    let mut out = Vec::new();
    for _ in 0..FRAMES {
        game.step_for(1.0 / 60.0).unwrap();
        game.draw().unwrap();
        let w = game.platform.window.as_mut().unwrap();
        w.settle();
        out.push(nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), SIZE.0, SIZE.1));
    }
    switch(false);
    out
}

/// The plain and the fast run of `switch`: frame by frame the same bytes, the frames not all alike (the
/// drive moves). -> the fast run's frames.
fn same_frames(name: &str, switch: fn(bool)) -> Vec<Vec<u8>> {
    let plain = frames(switch, true, &[]);
    let fast = frames(switch, false, &[]);
    assert_eq!(plain.len(), FRAMES);
    assert!(plain.first() != plain.last(), "{name}: the drive's frames change");
    for (i, (a, b)) in plain.iter().zip(&fast).enumerate() {
        assert!(a == b, "{name}: frame {i} differs from the plain pass's");
    }
    fast
}

/// In the rain at night (clock 162), the rest of the test's globals as they were after it.
fn in_the_rain(test: impl FnOnce()) {
    testkit::game();
    super::test_isolation::exclusive();
    crate::dev::hooks::SHOT_RAIN_CLOCK.store(162f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
    test();
    crate::dev::hooks::SHOT_RAIN_CLOCK.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// The lens drops drawn onto the frame over copies of only the texels they sample draw the frames of the
/// drops drawn over a whole copy of the frame; and the fast run drew them so.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn lens_drops_in_place_draw_the_plain_frames() {
    in_the_rain(|| {
        let before = nfsu2::render::rain::in_place_frames();
        same_frames("the lens drops in place", nfsu2::render::rain::set_plain_copy);
        let drawn = nfsu2::render::rain::in_place_frames() - before;
        assert!(drawn as usize >= FRAMES / 2, "the drops drawn in place in {drawn} of {FRAMES} frames");
    });
}

/// The rain's haze, with any Volumetric Fog tier holding the rain's air, puts nothing on screen: the
/// frames without its two passes are the frames with them; and the fast run left them out.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rain_haze_left_out_under_the_fog_draws_the_plain_frames() {
    in_the_rain(|| {
        let before = nfsu2::render::weather_fx::haze_left_out();
        same_frames("the haze left out", nfsu2::render::weather_fx::set_plain_haze);
        let left_out = nfsu2::render::weather_fx::haze_left_out() - before;
        assert!(left_out as usize >= FRAMES / 2, "the haze left out in {left_out} of {FRAMES} frames");
    });
}

/// The fog's light injection with each workgroup's lights culled to those that reach its cells draws the
/// frames of the plain loop over every light; and the fog is in those frames (they differ without it).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn fog_light_cull_draws_the_plain_frames() {
    in_the_rain(|| {
        let fast = same_frames("the fog's light cull", nfsu2::render::atmosphere::set_plain_inject);
        let off = frames(nfsu2::render::atmosphere::set_plain_inject, false, &[("enh.volumetric_fog", "0")]);
        assert!(fast.iter().zip(&off).any(|(a, b)| a != b), "the fog draws into the frames");
    });
}

/// The volumes' resolve reading the bounds once a workgroup, clearing with plain loads and stores and
/// gathering the bricks there draws the frames of the voxel-by-voxel resolve (the wet spray's volume and the
/// smoke's in the frames).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn volume_resolve_per_workgroup_draws_the_plain_frames() {
    in_the_rain(|| {
        same_frames("the resolve per workgroup", nfsu2::render::volume_fx::set_plain_resolve);
    });
}

/// The volumes' march specialised to each renderer's look (the branches its look never takes left out of its
/// program) keeps the frames of the march with every branch in (the wet spray's and the smoke's volumes): the
/// same arithmetic, the compiler's rounding of the smaller program aside (measured: one level on at most seven
/// pixels of a frame, from the spray's first frame on).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn volume_march_specialised_keeps_the_frames() {
    in_the_rain(|| {
        let switch: fn(bool) = nfsu2::render::volume_fx::set_plain_march;
        let plain = frames(switch, true, &[]);
        let fast = frames(switch, false, &[]);
        assert!(plain.first() != plain.last(), "the drive's frames change");
        for (i, (a, b)) in plain.iter().zip(&fast).enumerate() {
            let (mut worst, mut off) = (0u8, 0usize);
            for (x, y) in a.iter().zip(b) {
                let d = x.abs_diff(*y);
                worst = worst.max(d);
                off += usize::from(d > 0);
            }
            assert!(worst <= 2, "frame {i}: a channel {worst} levels off the plain march's");
            assert!(off * 1000 <= a.len(), "frame {i}: {off} of {} channels off the plain march's", a.len());
        }
    });
}

/// The rain's tint folded into the Volumetric Fog's apply: the frames are the tint pass's to the half float's
/// rounding (one product for the two steps: at most a few levels on a few pixels), and the fold happened.
/// Volumetric Fog Low and High Precision Post as the Medium level has them, without the lens drops (World
/// Detail 3's rain splatter off), so the tint comes right after the medium.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rain_tint_folded_into_the_fog_keeps_the_frames() {
    in_the_rain(|| {
        let medium = [
            ("enh.volumetric_fog", "1"),
            ("enh.rain_detail", "1"),
            ("enh.lens_drops", "0"),
            ("enh.hdr_post", "1"),
            ("enh.wet_interaction", "0"),
        ];
        let switch: fn(bool) = nfsu2::render::postfx::PostFx::set_plain_tint;
        let plain = frames_with(switch, true, &medium, false);
        let before = nfsu2::render::postfx::PostFx::tint_folds();
        let folded = frames_with(switch, false, &medium, false);
        let folds = nfsu2::render::postfx::PostFx::tint_folds() - before;
        assert!(folds as usize >= FRAMES / 2, "the tint folded in {folds} of {FRAMES} frames");
        let (mut worst, mut sum, mut n) = (0u8, 0u64, 0u64);
        for (a, b) in plain.iter().zip(&folded) {
            for (x, y) in a.iter().zip(b) {
                let d = x.abs_diff(*y);
                worst = worst.max(d);
                sum += u64::from(d);
                n += 1;
            }
        }
        assert!(worst <= 3, "a channel {worst} levels off the tint pass's frames");
        assert!((sum as f64) < 0.05 * n as f64, "the mean difference {} levels", sum as f64 / n as f64);
    });
}

/// The SSR mask's term and surface written by the main view's road, the hidden road's by the mask pass
/// (docs/wet-roads-ssr.md "The mask from the main view"), draw the frames of the mask pass working them all
/// out itself, byte for byte; and, on a device that takes it, the frames took it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn ssr_mask_from_the_main_view_draws_the_plain_frames() {
    in_the_rain(|| {
        // (One sample: the native Full Screen Anti-Aliasing, custom 14, off.)
        let ssr = [("enh.ssr", "1"), ("enh.road_reflection", "2")];
        let switch: fn(bool) = nfsu2::render::native::world::plan::set_plain_ssr_mask;
        let plain = frames_custom(switch, true, &ssr, &[(14, 0)]);
        let before = nfsu2::render::native::world::plan::ssr_mask_from_main();
        let fast = frames_custom(switch, false, &ssr, &[(14, 0)]);
        let from_main = nfsu2::render::native::world::plan::ssr_mask_from_main() - before;
        // (A device that takes it: a discrete GPU.)
        let device =
            nfsu2::render::native::world::plan::MASK_FROM_MAIN_DEVICE.load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            device != 1 || from_main as usize >= FRAMES / 2,
            "the mask from the main view in {from_main} of {FRAMES} frames"
        );
        for (i, (a, b)) in plain.iter().zip(&fast).enumerate() {
            assert!(a == b, "frame {i} differs from the mask pass's");
        }
    });
}
