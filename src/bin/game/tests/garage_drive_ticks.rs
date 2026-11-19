//! The garage's scene over a loaded drive (a shop or the crib opened from
//! free roam): the game publishes the drive's world ticks for the
//! tick-stepped enhancement effects (docs/original/uncapped-fps.md, "Enhancement
//! effects on ticks"), whose camera and drawn sections belong to the
//! drive, not to the platform. The garage's renderers step per draw as
//! without them: with LOD Fade on, the drive's ticks published and none
//! published draw the same picture (they faded the whole platform out,
//! leaving the car on the clear colour).
use super::*;
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

use nfsu2::render::enh::tick_views::{self, TickFrame, TickOptions, TickView};

/// A fresh career's car in the body shop's parts browser.
fn garage_game() -> Game {
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("GARAGE_TICKS");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.mode = game.parts_browser(0, None);
    game
}

/// A drive's tick: its camera far out in the city, no section of the
/// platform's drawn from it.
fn drive_tick(tick: u64, time: f32) -> TickFrame {
    let camera = nfsu2::render::Camera::new(glam::Vec3::new(1500.0, -800.0, 20.0), 0.3, 0.0);
    let opts = TickOptions { lod_fade: true, ..Default::default() };
    let view = TickView { tick, time, clock: time, camera, sections: Some(Vec::new().into()), far: None, opts };
    TickFrame { views: vec![view], at: tick as f64, lag: 0.0 }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_ignores_a_loaded_drives_ticks() {
    testkit::game();
    super::test_isolation::exclusive();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let (w, h) = (320u32, 240u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("garage ticks test"),
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
    let settings = nfsu2::render::FrameSettings::new(&game.settings.custom, nfsu2::render::RenderOptions::in_force());
    let opts = nfsu2::render::RenderOptions { lod_fade: true, ..settings.opts };
    let t = game.draw_clocks(1.0).seconds;
    let mut draw = |frame: Option<TickFrame>| -> Vec<u8> {
        tick_views::publish(frame);
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
    let alone = draw(None);
    let mut over_drive = Vec::new();
    for n in 0..4u64 {
        over_drive = draw(Some(drive_tick(1_000 + n, t + n as f32)));
    }
    // The drive's frame is still published after the garage's draw.
    assert!(tick_views::frame().is_some_and(|f| f.views[0].tick == 1_003), "the drive's ticks were not put back");
    let alone_again = draw(None);
    tick_views::publish(None);
    tick_views::rewind();
    assert!(alone == alone_again, "the garage's picture is not repeatable");
    assert!(over_drive == alone, "the garage stepped its effects with a loaded drive's ticks");
}
