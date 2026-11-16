//! The garage scenes draw no weather (docs/formats/front-end.md, "Garage
//! scenes: no weather"): the original updates and draws the rain only in
//! game mode 6 (0x5CB40D before the weather update 0x61F130, which draws
//! the drops 0x613A80 and applies the rain tint; 0x5CB7EE for the lens drops
//! 0x5C7AC0), so the front end's garage (mode 3) has none. The weather a
//! drive left behind (a shower in free roam, then a shop or the main menu)
//! must not reach the garage's renderer: with every weather row at its
//! highest, a heavy shower published and none published draw the same
//! picture.
use super::*;
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// A fresh career's car in the body shop's parts browser.
fn garage_game() -> Game {
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("GARAGE_WEATHER");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.mode = game.parts_browser(0, None);
    game
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_draws_no_rain_left_by_a_drive() {
    testkit::game();
    super::test_isolation::exclusive();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let (w, h) = (320u32, 240u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("garage weather test"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let depth = create_depth(&gpu, w, h);
    let mut game = garage_game();
    for _ in 0..30 {
        game.advance(1.0 / 60.0).unwrap();
    }
    // Every weather row at its highest: Rain Detail Next-Gen (the drops, the
    // splashes, the haze), the spray, Wet Road Interaction, the wet roads and
    // the wet cars.
    let settings = nfsu2::render::FrameSettings::new(&game.settings.custom, nfsu2::render::RenderOptions::in_force());
    let opts = nfsu2::render::RenderOptions {
        weather_fx: true,
        rain_detail: nfsu2::render::RenderOptions::RAIN_NEXT_GEN,
        spray: true,
        wet_interaction: 2,
        wet_roads: true,
        wet_car: true,
        lens_drops: true,
        ..settings.opts
    };
    let t = game.draw_clocks(1.0).seconds;
    let mut draw = |scene: Option<nfsu2::render::wet_car::Scene>| -> Vec<u8> {
        nfsu2::render::wet_car::publish(scene);
        assert!(render_garage(
            game.garage_fe.request.clone(),
            &game.game_data.root,
            &game.game_data.caches,
            &mut game.render.garage,
            &gpu,
            format,
            &view,
            &depth,
            (w, h),
            t,
            settings.display,
            &opts,
            1,
            None,
            None,
        ));
        nfsu2::render::read_back(&gpu, &target, w, h)
    };
    let dry = draw(None);
    let shower = nfsu2::render::wet_car::Scene { rain: 1.0, road_wet: 1.0, car_wet: 1.0, covered: false, clock: 150.0 };
    let rain = draw(Some(shower));
    let dry_again = draw(None);
    nfsu2::render::wet_car::publish(None);
    assert!(dry == dry_again, "the garage's dry picture is not repeatable");
    assert!(rain == dry, "the garage drew the weather a drive left behind");
}
