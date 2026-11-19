//! The car lot's car change (GarageMain 0x4D82D0): the platform turns only
//! once the new car is loaded (state 1), the old car rides out on its own
//! disc and the new one rides in on the next (state 2, two FE cars), at
//! every display rate and however long the car takes to build.
use super::*;
use crate::InputEvent;
use crate::garage::scene::TEST_BUILD_DELAY_MS;
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// One tick's car lot as the scene stands.
#[derive(Clone, Debug, PartialEq)]
struct Tick {
    tick: u64,
    now: u32,
    turning: bool,
    platform: u32,
    shown: Option<String>,
    outgoing: Option<String>,
    asked: Option<String>,
    in_front: bool,
}

/// The career lot (its first car swung in), Right pressed at tick 60, run
/// at `hz` for 2.5 s with every car build from tick 40 on `delay_ms` long;
/// each tick's state from tick 50 on.
fn run(hz: u32, delay_ms: u64) -> Vec<Tick> {
    TEST_BUILD_DELAY_MS.set(None);
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("LOT_TURN");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.mode = game.career_lot();
    let Mode::Menu(Menu::CarLot(lot)) = &game.mode else { panic!("the career lot") };
    assert!(lot.cars.len() > 1 && !lot.cars[0].name.eq_ignore_ascii_case(&lot.cars[1].name), "two car types");
    let base = game.pacing.clocks.tick;
    game.input_at(base + 60, InputEvent::press(KeyCode::ArrowRight));
    game.input_at(base + 61, InputEvent::KeyUp(KeyCode::ArrowRight));
    let mut out: Vec<Tick> = Vec::new();
    for _ in 0..(5 * hz / 2 + u32::try_from(delay_ms).unwrap() * hz / 1000) {
        game.advance(1.0 / hz as f32).unwrap();
        let tick = game.pacing.clocks.tick - base;
        if tick >= 40 {
            TEST_BUILD_DELAY_MS.set(Some(delay_ms));
        }
        if tick < 50 || out.last().is_some_and(|t| t.tick == tick) {
            continue;
        }
        let (Some(r), Some(g)) = (game.garage_fe.request.as_ref(), game.render.garage.as_ref()) else { continue };
        let v = r.view.at(r.now);
        out.push(Tick {
            tick,
            now: r.now.to_bits(),
            turning: r.view.changing(r.now),
            platform: v.platform_angle.to_bits(),
            shown: g.car.as_ref().map(|c| c.0.clone()),
            outgoing: g.outgoing.as_ref().map(|o| o.key.0.clone()),
            asked: r.car.clone(),
            in_front: v.turntable().car_in_front(),
        });
    }
    TEST_BUILD_DELAY_MS.set(None);
    out
}

/// Every tick: at rest the shown car stands on the front disc; while the
/// platform turns the new car is on show from the turn's first tick and
/// the old one rides out beside it; one turn, at least the build's delay
/// after the step; the same at 30 / 60 / 120 / 200 Hz.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_lot_turns_once_the_new_car_is_in_and_the_old_rides_out() {
    testkit::game();
    for delay in [0u64, 200, 1000, 3000] {
        let reference = run(60, delay);
        let first = reference.iter().find(|t| t.turning).expect("the platform turns");
        let old = reference[0].shown.clone().expect("the first car on show");
        assert!(
            first.shown.is_some() && first.shown != Some(old.clone()),
            "{delay} ms: the new car in the turn's first tick: {first:?}"
        );
        assert!(reference.iter().filter(|t| t.turning).count() <= 31, "{delay} ms: one half-second turn");
        let step = reference.iter().find(|t| t.tick >= 60).map(|t| f32::from_bits(t.now)).unwrap();
        let start = f32::from_bits(first.now);
        assert!(
            start - step >= 0.3 + delay as f32 / 1000.0 - 0.05,
            "{delay} ms: the turn waits for the car ({step} → {start})"
        );
        for t in &reference {
            if t.turning {
                assert_eq!(t.shown, t.asked, "{delay} ms: the new car rides in: {t:?}");
                assert_eq!(t.outgoing.as_ref(), Some(&old), "{delay} ms: the old car rides out: {t:?}");
            } else {
                assert!(
                    t.in_front && t.shown.is_some() && t.outgoing.is_none(),
                    "{delay} ms: at rest on the front disc: {t:?}"
                );
            }
        }
        for hz in [30, 120, 200] {
            let got = run(hz, delay);
            let mut common = 0;
            for t in &got {
                if let Some(w) = reference.iter().find(|w| w.tick == t.tick) {
                    assert_eq!(t, w, "{hz} Hz, {delay} ms");
                    common += 1;
                }
            }
            assert!(common >= 40, "{hz} Hz, {delay} ms: only {common} ticks in common");
        }
    }
}

/// The lot's car change drawn frame by frame (`render_garage` with the
/// frames between ticks), for a look at the turn: LOT_TURN_DUMP=dir,
/// LOT_TURN_HZ (60), LOT_TURN_DELAY_MS (0); one PNG per drawn frame from
/// the step to 1.5 s after the turn's start.
#[test]
#[ignore = "a frame dump for a look at the turn (LOT_TURN_DUMP=dir); not a check"]
fn lot_turn_frame_dump() {
    testkit::game();
    super::test_isolation::exclusive();
    let dir = std::path::PathBuf::from(std::env::var("LOT_TURN_DUMP").expect("LOT_TURN_DUMP=dir"));
    let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let (hz, delay) = (env("LOT_TURN_HZ", 60) as u32, env("LOT_TURN_DELAY_MS", 0));
    std::fs::create_dir_all(&dir).unwrap();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let (w, h) = (640u32, 480u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lot turn dump"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let depth = create_depth(&gpu, w, h);
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("LOT_TURN");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.mode = game.career_lot();
    let base = game.pacing.clocks.tick;
    game.input_at(base + 60, InputEvent::press(KeyCode::ArrowRight));
    game.input_at(base + 61, InputEvent::KeyUp(KeyCode::ArrowRight));
    let (mut n, mut end) = (0, None);
    for _ in 0..(10 * hz) {
        let alpha = game.advance(1.0 / hz as f32).unwrap();
        let tick = game.pacing.clocks.tick - base;
        TEST_BUILD_DELAY_MS.set(Some(delay).filter(|_| tick >= 40));
        if tick < 58 {
            continue;
        }
        let turning = game.garage_fe.request.as_ref().is_some_and(|r| r.view.changing(r.now));
        if turning && end.is_none() {
            end = Some(tick + 90);
        }
        if end.is_some_and(|e| tick > e) {
            break;
        }
        let settings =
            nfsu2::render::FrameSettings::new(&game.settings.custom, nfsu2::render::RenderOptions::in_force());
        let t = game.draw_clocks(alpha).seconds;
        let drawn = game.frame_garage(alpha);
        let request = game.garage_fe.request.clone();
        assert!(render_garage(
            request,
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
            &settings.opts,
            1,
            None,
            drawn.as_ref(),
        ));
        let px = nfsu2::render::read_back(&gpu, &target, w, h);
        image::save_buffer(dir.join(format!("f{n:04}-t{tick:03}.png")), &px, w, h, image::ColorType::Rgba8).unwrap();
        n += 1;
    }
    TEST_BUILD_DELAY_MS.set(None);
}
