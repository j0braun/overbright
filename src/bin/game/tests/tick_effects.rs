//! docs/uncapped-fps.md, "Enhancement effects on ticks": the
//! tick-stepped enhancement effects (LOD Fade's per-instance fades, the
//! lamps' section fades and shadow layers, the Next-Gen water's basins,
//! the traced scene's car cache) hold the same state at equal ticks
//! whatever the display rate. A real free roam (the player's game files),
//! driven by a held key keyed on the tick, drawn offscreen at 30, 60, 120
//! and 200 Hz and uncapped (uneven 1–4 ms frames).
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The enhancements whose state steps on ticks (World Detail Ultra's
/// keep-loaded makes LOD Fade's section cross-fades and the lamps' fades
/// run; Next-Gen water; Dynamic Lamps with High lamp shadows; probes).
const ROWS: &str = "enh.lod_fade 1;enh.keep_loaded 1;enh.detail_distance 200;enh.dynamic_lamps 1;enh.lamp_shadows 2;enh.water 3;enh.light_probes 1";

pub(super) fn offscreen_window(game: &mut Game, width: u32, height: u32) {
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tick effects test target"),
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

/// One run: free roam with the rows on, the throttle held from the first
/// tick, `ticks` ticks drawn at `hz` (0: uncapped); per tick a frame stepped to,
/// the effects' digest after that frame.
fn run(hz: f32, ticks: u64) -> std::collections::BTreeMap<u64, (u64, u64)> {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("TICK_EFFECTS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    for line in ROWS.split(';') {
        let (k, v) = line.split_once(' ').unwrap();
        game.settings.enh.parse(k, v);
    }
    game.settings.apply_enhancements();
    game.explore(None).unwrap();
    offscreen_window(&mut game, 320, 240);
    // The car put on its way before each tick (the same at every rate),
    // the frames drawn as each ends (`ticks::Frames`: one tick a call).
    let start = game.pacing.clocks.tick;
    let mut frames = crate::dev::ticks::Frames::new(hz);
    let mut out = std::collections::BTreeMap::new();
    let mut seen = start;
    let (p0, dir) = {
        let d = game.sim.drive.as_ref().unwrap();
        (d.state.position, d.state.orientation * Vec3::X)
    };
    while game.pacing.clocks.tick < start + ticks {
        // Carried along the car's heading, 25 m/s (wherever the free roam
        // starts it; the chase camera follows).
        let k = (game.pacing.clocks.tick - start) as f32;
        let d = game.sim.drive.as_mut().unwrap();
        d.state.position = p0 + dir * (25.0 / 60.0) * k + Vec3::Z * 0.3;
        d.state.velocity = dir * 25.0;
        frames
            .tick_with(&mut game, &mut |game, f| {
                game.draw_between(f.alpha)?;
                let tick = game.pacing.clocks.tick;
                if tick > seen {
                    seen = tick;
                    let world = game.render.world.as_ref().map_or(0, nfsu2::render::enh::tick_views::digest);
                    let water = game.sim.drive.as_ref().unwrap().effects.water.tick_digest();
                    out.insert(tick - start, (world, water));
                }
                Ok(())
            })
            .unwrap();
    }
    let d = game.sim.drive.as_ref().unwrap();
    eprintln!("tick effects: {hz} Hz ({}): player at {:?} speed {:.1}, {} states", mode_name(&game.mode), d.state.position, d.state.velocity.length(), out.values().collect::<std::collections::BTreeSet<_>>().len());
    out
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn tick_stepped_effects_are_the_same_at_every_rate() {
    testkit::game();
    super::test_isolation::exclusive();
    let ticks = 420;
    let reference = run(60.0, ticks);
    assert!(reference.len() as u64 >= ticks, "60 Hz draws every tick");
    let changes = reference.values().collect::<std::collections::BTreeSet<_>>().len();
    assert!(changes > 10, "the effects moved ({changes} distinct states)");
    for hz in [30.0, 120.0, 200.0, 0.0] {
        let other = run(hz, ticks);
        let mut common = 0;
        for (tick, v) in &other {
            if let Some(r) = reference.get(tick) {
                assert_eq!(v, r, "{hz} Hz: the effects' state differs from 60 Hz at tick {tick}");
                common += 1;
            }
        }
        assert!(common as u64 >= ticks / 3, "{hz} Hz: {common} ticks in common");
        eprintln!("tick effects: {hz} Hz identical to 60 Hz at {common} ticks");
    }
}
