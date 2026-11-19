//! docs/original/uncapped-fps.md, "Enhanced effects on ticks": the
//! Enhanced effects that simulate or accumulate (Wet Car, the spray, the
//! lens drops, the wet road's trail map, the smoke feeders, Sparks & Fire,
//! Ambient Detail, the Next-Gen sky's rows) step on ticks, so the same
//! drive drawn at 30, 60, 120 and 200 Hz and on an uneven display leaves
//! the same effect state at every tick both reach. On the real draw path
//! (an offscreen window), free roam in the rain with the effects on.
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The Enhancements lines the test runs with: every tick-stepped Enhanced effect.
const ROWS: &[(&str, &str)] = &[
    ("enh.wet_car", "1"),
    ("enh.spray", "1"),
    ("enh.lens_drops", "1"),
    ("enh.wet_interaction", "2"),
    ("enh.rain_detail", "2"),
    ("enh.particles", "3"),
    ("enh.sparks_fire", "1"),
    ("enh.ambient_detail", "2"),
    ("enh.sky", "1"),
];

/// An offscreen window on the real draw path (as `--bench` makes it).
fn offscreen_window(game: &mut Game, width: u32, height: u32) {
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("enh ticks test target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        color_space: Default::default(),
    };
    let depth = create_depth(&gpu, width, height);
    let ui = UiRenderer::new(&gpu, format);
    game.platform.window = Some(WindowState::new(None, None, Some(offscreen), config, gpu, depth, ui));
}

/// One run: free roam in the rain, the gas held, `secs` of frames at `hz`
/// (0: an uneven display, 4–40 ms frames and a 120 ms stall), each frame
/// drawn between its ticks. -> the effects' digests per tick reached at a
/// frame's end.
fn run(hz: f32, secs: f32) -> std::collections::BTreeMap<u64, [u64; 8]> {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ENH_TICKS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    for (k, v) in ROWS {
        game.settings.enh.parse(k, v);
    }
    game.settings.apply_enhancements();
    // The window first: the load makes the world's renderer (as the
    // game's and the bench's loads do), so the effects step from the
    // first tick at every rate.
    offscreen_window(&mut game, 320, 180);
    game.explore(None).unwrap();
    assert!(game.render.world.is_some(), "the load made the world's renderer");
    game.platform.keys.insert(KeyCode::KeyW);
    let mut out = std::collections::BTreeMap::new();
    let mut t = 0.0f32;
    let mut k = 0u64;
    while t < secs {
        let interval = if hz > 0.0 {
            1.0 / hz
        } else if k % 50 == 49 {
            0.120
        } else {
            let x = (k + 7).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
            0.004 + (x % 3601) as f32 * 1e-5
        };
        t += interval;
        k += 1;
        let alpha = game.advance(interval).unwrap();
        if game.pacing.frame_updates > 0 {
            out.insert(game.interp.cur.tick, game.enh_digest());
        }
        game.draw_between(alpha).unwrap();
        game.frame_drawn();
    }
    out
}

/// The same drive at 30 / 60 / 120 / 200 Hz and on an uneven display: the
/// effects' state is the same at every tick two rates both end a frame
/// on (before, every effect stepped in the draw read the drawn frame's
/// poses, its camera and its count).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn enhanced_effects_are_the_same_at_every_rate() {
    testkit::game();
    super::test_isolation::exclusive();
    crate::dev::hooks::SHOT_RAIN_CLOCK.store(162f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
    let secs = 3.0;
    let reference = run(60.0, secs);
    let live = reference.values().filter(|d| d[0] != 0 && d[2] != 0).count();
    let mut report = Vec::new();
    for hz in [30.0f32, 120.0, 200.0, 0.0] {
        let other = run(hz, secs);
        let mut common = 0;
        for (tick, d) in &other {
            let Some(r) = reference.get(tick) else { continue };
            common += 1;
            assert_eq!(d, r, "{hz} Hz: the effects differ from 60 Hz at tick {tick}");
        }
        assert!(common > 20, "{hz} Hz: only {common} ticks in common");
        report.push((hz, common));
    }
    crate::dev::hooks::SHOT_RAIN_CLOCK.store(0, std::sync::atomic::Ordering::Relaxed);
    eprintln!("enh ticks: {} ticks at 60 Hz ({live} with spray and ambient state); common: {report:?}", reference.len());
}
