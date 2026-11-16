//! First use (docs/first-use-builds.md): after a drive's load, no drawn
//! frame makes a pipeline or a shader module; their programs are made by
//! the load's warm-up, and when a Graphics row changes, by the settings
//! warm-up before the frame that draws with them. A real free roam and a
//! race (the game files under $NFSU2_ROOT) at every preset, drawn
//! offscreen; then, in free roam, every preset switched to in turn (the
//! Graphics page's live change) and Photo Mode opened.
//!
//! The Animated Water at each of its tiers: a free roam started at the
//! casino square's fountains, and the tiers switched to in turn there.
//!
//! The main menu's garage scene after the boot screens: nothing made in its frames.
//!
//! Not reached here: rain that starts mid-drive (its pipelines, the spray's volume and the lens
//! drops' pass are made at the load whatever the weather, `WorldRenderer::warm_drive`,
//! `PostFx::warm_frame`, but no frame here rains); the shops' garage scenes.
use super::*;

const SIZE: (u32, u32) = (320, 240);

/// What the drawn frames made on the game's device.
fn made_by_frames(game: &mut Game, frames: usize) -> Vec<String> {
    let tag = game.platform.window.as_ref().unwrap().gpu.build_tag();
    nfsu2::render::record_builds(Some(tag));
    for _ in 0..frames {
        game.step_for(1.0 / 60.0).unwrap();
        game.draw().unwrap();
    }
    nfsu2::render::record_builds(None)
}

/// The frame just drawn, read back from the offscreen target once its end
/// is submitted: the frame's last command buffers go to the queue from a
/// worker (`FrameEnd`), so a read-back queued before `settle` can copy the
/// target before them and return the frame before (the copy raced the
/// worker on a busy machine; the windowed game presents from that same
/// worker, after the frame's own buffers).
fn picture(game: &mut Game) -> Vec<u8> {
    let w = game.platform.window.as_mut().unwrap();
    w.settle();
    nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), SIZE.0, SIZE.1)
}

/// The casino square, the car facing its round fountains (their arcing
/// jets in view from about 25 m).
const FOUNTAINS: (f32, f32, f32, f32) = (-300.0, -547.0, 20.0, std::f32::consts::PI);

fn at_the_fountains(game: &mut Game) {
    let (x, y, z, heading) = FOUNTAINS;
    game.explore(Some((Vec3::new(x, y, z), heading))).unwrap();
}

fn game_at(preset: nfsu2::qol::Preset) -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("FIRST_USE_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    // The window first: the loads below warm up for it.
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    preset.apply(&mut game.settings.enh);
    game.settings.enh.photo_mode = true;
    game.settings.apply_enhancements();
    game
}

/// Free roam, then a city circuit, at every preset: nothing made in
/// their frames.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_loaded_drive_draws_without_making_programs() {
    // The Graphics rows are process-wide: no other game test beside this one.
    super::test_isolation::exclusive();
    for preset in nfsu2::qol::PRESETS {
        let mut game = game_at(preset);
        game.explore(None).unwrap();
        let made = made_by_frames(&mut game, 20);
        assert!(made.is_empty(), "{} free roam: made in its frames: {made:?}", preset.name());
        let race = game
            .game_data
            .career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .expect("a city circuit");
        game.start_race(race, false, false).unwrap();
        let made = made_by_frames(&mut game, 10);
        assert!(made.is_empty(), "{} race: made in its frames: {made:?}", preset.name());
    }
}

/// The lamps' light-cluster loop in its two forms (docs/lamps-shadows.md
/// "The cluster loop"): a device with subgroup operations takes the
/// subgroup form, one made without them (`NFSU2_GPU_DROP=subgroup`, as DX12
/// through FXC is) the plain loop. At the High and Ultra presets
/// (Dynamic Lamps, Lamp Shadows Low and High) free roam and a city circuit
/// are drawn the same in both, and neither form's load leaves a program to
/// its frames. Free roam without Dynamic Lamps differs (the lamps are in view).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_cluster_loop_forms_draw_the_same_and_are_ready() {
    super::test_isolation::exclusive();
    nfsu2::surface_maps::set_blocking(true);
    let run = |preset: nfsu2::qol::Preset, subgroups: bool, lamps: bool| -> Vec<Vec<u8>> {
        // The device is made in `game_at` (the offscreen window).
        if subgroups {
            unsafe { std::env::remove_var("NFSU2_GPU_DROP") };
        } else {
            unsafe { std::env::set_var("NFSU2_GPU_DROP", "subgroup") };
        }
        let mut game = game_at(preset);
        unsafe { std::env::remove_var("NFSU2_GPU_DROP") };
        let gpu = &game.platform.window.as_ref().unwrap().gpu;
        let has = gpu.device.features().contains(wgpu::Features::SUBGROUP);
        let offered = gpu.adapter.features().contains(wgpu::Features::SUBGROUP);
        assert_eq!(has, subgroups && offered, "the device's subgroups as asked");
        if !lamps {
            game.settings.enh.dynamic_lamps = false;
            game.settings.apply_enhancements();
        }
        let mut pictures = Vec::new();
        game.explore(None).unwrap();
        let made = made_by_frames(&mut game, 10);
        assert!(made.is_empty(), "{} (subgroups {has}) free roam: made in its frames: {made:?}", preset.name());
        for _ in 0..3 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            pictures.push(picture(&mut game));
        }
        let race = game
            .game_data
            .career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .expect("a city circuit");
        game.start_race(race, false, false).unwrap();
        let made = made_by_frames(&mut game, 5);
        assert!(made.is_empty(), "{} (subgroups {has}) race: made in its frames: {made:?}", preset.name());
        for _ in 0..3 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            pictures.push(picture(&mut game));
        }
        pictures
    };
    for preset in [nfsu2::qol::Preset::Modern, nfsu2::qol::Preset::Ultra] {
        let scalar = run(preset, true, true);
        let plain = run(preset, false, true);
        for (i, (a, b)) in scalar.iter().zip(&plain).enumerate() {
            assert!(a == b, "{}: frame {i} differs between the cluster loop's forms", preset.name());
        }
        if preset == nfsu2::qol::Preset::Modern {
            let unlit = run(preset, true, false);
            assert!(unlit[..3] != scalar[..3], "free roam's lamps are in view");
        }
    }
}

/// Ambient Occlusion's apply pass skips the cars' glow loop outside their
/// screen rectangles (`ssao::reach_rect`): the race start of a city circuit
/// (the rivals lined up beside the player, in view) and its first frames
/// after the start are drawn the same with the rectangles and with every
/// rectangle the whole screen, at the High (8× MSAA: the multisampled
/// apply), Ultra (2× SSAA) and Extreme presets and Ultra with the strength at
/// 25.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_occlusion_glow_rectangles_draw_the_same() {
    super::test_isolation::exclusive();
    nfsu2::surface_maps::set_blocking(true);
    let run = |preset: nfsu2::qol::Preset, strength: Option<u8>, rects: bool| -> Vec<Vec<u8>> {
        nfsu2::render::ssao::set_glow_rects_off(!rects);
        let mut game = game_at(preset);
        if let Some(s) = strength {
            game.settings.enh.ssao = s;
            game.settings.apply_enhancements();
        }
        assert!(nfsu2::render::RenderOptions::in_force().ssao > 0.0, "the occlusion on");
        let race = game
            .game_data
            .career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .expect("a city circuit");
        game.start_race(race, false, false).unwrap();
        let mut pictures = Vec::new();
        for i in 0..40 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            if i % 8 == 0 {
                pictures.push(picture(&mut game));
            }
        }
        nfsu2::render::ssao::set_glow_rects_off(false);
        pictures
    };
    for (preset, strength) in [
        (nfsu2::qol::Preset::Modern, None),
        (nfsu2::qol::Preset::Ultra, None),
        (nfsu2::qol::Preset::Extreme, None),
        (nfsu2::qol::Preset::Ultra, Some(25)),
    ] {
        let with = run(preset, strength, true);
        let without = run(preset, strength, false);
        for (i, (a, b)) in with.iter().zip(&without).enumerate() {
            assert!(a == b, "{} (strength {strength:?}): picture {i} differs with the glow rectangles", preset.name());
        }
    }
}

/// In free roam from the Original preset, each preset switched to in turn
/// (the settings warm-up runs first, as the options screen's next frame
/// does): nothing made in the frames drawn with it; then Photo Mode.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_row_switched_on_is_ready_before_its_first_frame() {
    super::test_isolation::exclusive();
    let mut game = game_at(nfsu2::qol::Preset::Original);
    game.explore(None).unwrap();
    assert!(made_by_frames(&mut game, 5).is_empty());
    for preset in nfsu2::qol::PRESETS.into_iter().skip(1) {
        preset.apply(&mut game.settings.enh);
        game.settings.enh.photo_mode = true;
        game.settings.apply_enhancements();
        game.warm_for_settings();
        let made = made_by_frames(&mut game, 10);
        assert!(made.is_empty(), "switched to {}: made in its frames: {made:?}", preset.name());
    }
    // Photo Mode from the pause menu (the last preset's rows in force).
    let tag = game.platform.window.as_ref().unwrap().gpu.build_tag();
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    nfsu2::render::record_builds(Some(tag));
    game.act(Action::PhotoMode, MenuKind::ExplorePause).unwrap();
    assert!(game.photo.is_some(), "photo mode opened");
    for _ in 0..5 {
        game.draw().unwrap();
    }
    let made = nfsu2::render::record_builds(None);
    assert!(made.is_empty(), "photo mode: made in its frames: {made:?}");
}

/// Ray tracing on (Modern + RT and the Ray Traced reflections, on a
/// device made with ray queries for them, as a start with the rows saved
/// on) at Extreme with SSAA 2x: free roam and a city circuit make no
/// program in their frames; switched off, a new free roam, and on again
/// there (the settings warm-up first), neither; then with the native Full
/// Screen Anti-Aliasing instead (the scene's depth multisampled), a new
/// free roam neither. The reflections' trace takes the path the device
/// takes (its rays from a list in a compute pass here); either path's
/// programs are the warm-ups' (`Ssr::warm_rt`). One device for all (the
/// traced scene and Modern + RT's placeholders are one device's).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_ray_traced_rows_are_ready_before_their_first_frame() {
    super::test_isolation::exclusive();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("FIRST_USE_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    nfsu2::qol::Preset::Extreme.apply(&mut game.settings.enh);
    nfsu2::qol::set_ray_traced(&mut game.settings.enh, true);
    // SSAA 2x: the scene's depth single-sampled.
    game.settings.enh.post_aa = 6;
    game.settings.apply_enhancements();
    // The device after the rows: made with ray queries for them.
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    let gpu = &game.platform.window.as_ref().unwrap().gpu;
    assert!(
        nfsu2::render::rt::available(gpu) && nfsu2::qol::rt_reflections(),
        "this test needs a GPU that traces rays (Vulkan ray queries on a discrete GPU)"
    );
    let list = nfsu2::render::rt::list_path(gpu);
    game.explore(None).unwrap();
    let made = made_by_frames(&mut game, 20);
    assert!(made.is_empty(), "ray traced (list {list}) free roam: made in its frames: {made:?}");
    let race = game
        .game_data
        .career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| {
            e.kind == career::EventKind::Circuit
                && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
        })
        .map(|e| e.index)
        .expect("a city circuit");
    game.start_race(race, false, false).unwrap();
    let made = made_by_frames(&mut game, 10);
    assert!(made.is_empty(), "ray traced (list {list}) race: made in its frames: {made:?}");
    // Off, a new free roam; then on again in it.
    nfsu2::qol::set_ray_traced(&mut game.settings.enh, false);
    game.settings.apply_enhancements();
    game.explore(None).unwrap();
    assert!(made_by_frames(&mut game, 5).is_empty());
    nfsu2::qol::set_ray_traced(&mut game.settings.enh, true);
    game.settings.apply_enhancements();
    game.warm_for_settings();
    let made = made_by_frames(&mut game, 10);
    assert!(made.is_empty(), "ray tracing switched on (list {list}): made in its frames: {made:?}");
    // The native FSAA (MSAA 8x) instead: the depth multisampled.
    game.settings.enh.post_aa = 0;
    game.settings.custom[14] = 3;
    game.settings.apply_enhancements();
    game.explore(None).unwrap();
    let made = made_by_frames(&mut game, 10);
    assert!(made.is_empty(), "ray traced with the native FSAA (list {list}) free roam: made in its frames: {made:?}");
}

/// Rain wetting the road mid-drive (Wet Roads on: the road's draws leave
/// their dry-road pipelines, `DepthPrepass::equal_dry`, for the full ones)
/// makes nothing in its frames, at every preset with Wet Roads, and Modern
/// + RT where the device traces: both sets are the load's. And the dry
/// frames before it make nothing either.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rain_starting_mid_drive_makes_no_programs() {
    super::test_isolation::exclusive();
    let mut wet_presets = 0;
    for preset in nfsu2::qol::PRESETS {
        for rt in [false, true] {
            let mut game = game_at(preset);
            if !game.settings.enh.wet_roads {
                continue;
            }
            wet_presets += usize::from(!rt);
            if rt {
                if !nfsu2::qol::rt_supported() {
                    continue;
                }
                game.settings.enh.modern_lighting = 3;
                game.settings.apply_enhancements();
            }
            game.explore(None).unwrap();
            let made = made_by_frames(&mut game, 10);
            assert!(made.is_empty(), "{} (RT {rt}) dry: made in its frames: {made:?}", preset.name());
            let d = game.sim.drive.as_mut().unwrap();
            d.weather.amount = 1.0;
            d.weather.road_wet = 0.5;
            let made = made_by_frames(&mut game, 10);
            assert!(made.is_empty(), "{} (RT {rt}) the road wet: made in its frames: {made:?}", preset.name());
        }
    }
    assert!(wet_presets > 0, "no preset with Wet Roads");
}

/// The temporal rows and the upscalers (TAA; Upscaling Quality, Balanced
/// and Performance; FSR 1's three tiers; FSR 2's five) at the Enhanced
/// levels Low, Medium and High and on the Extreme preset, each on a device
/// of its own, with the native Full Screen Anti-Aliasing on (the count a
/// menu's frames draw with, not a temporal frame's): free roam loaded
/// straight away makes no program in its first ten frames (the load's
/// warm-up made them), and so does one loaded after the menus' programs
/// made ahead (`ProgramsAhead`, the set `game --prepare` makes), whose
/// load makes none either. Their world's, cars', horizon fog's, blur's,
/// rain's and FSR's programs are the frames' own, keyed as the frames key
/// them (the LOD bias, single-sampled targets, the output formats).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_temporal_rows_are_ready_before_their_first_frame() {
    super::test_isolation::exclusive();
    let rows: Vec<(String, u8, u8)> = std::iter::once(("TAA".to_string(), nfsu2::qol::TAA, 0))
        .chain((1..=nfsu2::qol::UPSCALE_MAX).map(|u| (format!("Upscaling {u}"), 0, u)))
        .collect();
    let levels = [Some(nfsu2::qol::Level::Low), Some(nfsu2::qol::Level::Medium), Some(nfsu2::qol::Level::High), None];
    for level in levels {
        let level_name = level.map_or("Extreme", |l| l.name());
        for (name, aa, up) in &rows {
            for after_menus in [false, true] {
                let mut game = game_at(nfsu2::qol::Preset::Original);
                match level {
                    Some(l) => l.apply(&mut game.settings.enh),
                    None => nfsu2::qol::Preset::Extreme.apply(&mut game.settings.enh),
                }
                if *aa != 0 {
                    game.settings.enh.post_aa = *aa;
                }
                game.settings.enh.upscale = *up;
                game.settings.apply_enhancements();
                assert!(game.settings.custom[14] != 0, "the native FSAA on");
                if after_menus {
                    menus_until_made(&mut game);
                }
                let tag = game.platform.window.as_ref().unwrap().gpu.build_tag();
                nfsu2::render::record_builds(Some(tag));
                game.explore(None).unwrap();
                let made = nfsu2::render::record_builds(None);
                if after_menus {
                    assert!(made.is_empty(), "{level_name} {name}: the load after the menus made: {made:?}");
                }
                let made = made_by_frames(&mut game, 10);
                assert!(made.is_empty(), "{level_name} {name} (after the menus: {after_menus}): made in its frames: {made:?}");
            }
        }
    }
}

/// Free roam loaded at the fountains with Animated Water at each tier
/// (On, Realistic, Next-Gen): the water's mesh, its GPU halves and their
/// programs are made by the load, nothing in the frames.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn water_at_each_tier_is_ready_before_its_first_frame() {
    super::test_isolation::exclusive();
    for tier in 1..=3u8 {
        let mut game = game_at(nfsu2::qol::Preset::Original);
        game.settings.enh.water = tier;
        game.settings.apply_enhancements();
        at_the_fountains(&mut game);
        let made = made_by_frames(&mut game, 20);
        assert!(made.is_empty(), "water tier {tier} loaded: made in its frames: {made:?}");
        assert!(game.sim.drive.as_ref().unwrap().effects.water.active(), "water tier {tier}: the fountains in view");
    }
    // The tiers switched to in turn at the fountains (the settings warm-up
    // first, as the options screen's next frame does), then Extreme's rows
    // with Next-Gen water (the scene's other passes and samples beside it).
    let mut game = game_at(nfsu2::qol::Preset::Original);
    at_the_fountains(&mut game);
    assert!(made_by_frames(&mut game, 5).is_empty());
    for tier in [1u8, 2, 3, 2, 1] {
        game.settings.enh.water = tier;
        game.settings.apply_enhancements();
        game.warm_for_settings();
        let made = made_by_frames(&mut game, 10);
        assert!(made.is_empty(), "switched to water tier {tier}: made in its frames: {made:?}");
    }
    nfsu2::qol::Preset::Extreme.apply(&mut game.settings.enh);
    game.settings.enh.water = 3;
    game.settings.apply_enhancements();
    game.warm_for_settings();
    let made = made_by_frames(&mut game, 10);
    assert!(made.is_empty(), "Extreme with Next-Gen water: made in its frames: {made:?}");
}

/// The main menu drawn until the loads and the programs made ahead are
/// done (`Preload`, `ProgramsAhead`).
fn menus_until_made(game: &mut Game) {
    game.mode = game.icon_menu(MenuKind::Main);
    for _ in 0..3000 {
        game.step_for(1.0 / 60.0).unwrap();
        game.draw().unwrap();
        let loaded = game.loads.preload.as_ref().is_some_and(|p| {
            p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished())
        });
        if loaded && game.loads.programs.as_ref().is_some_and(|p| p.job.is_finished()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("the menus' loads and programs were not done after 3000 frames");
}

/// After the menus have been up (the world loaded and the programs made
/// ahead), a free-roam load makes no pipeline and no shader module, and
/// neither do its frames: at every preset, and again after a preset is
/// switched to in the menus (the programs follow the settings).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_load_after_the_menus_makes_no_program() {
    super::test_isolation::exclusive();
    for preset in nfsu2::qol::PRESETS {
        let mut game = game_at(preset);
        menus_until_made(&mut game);
        let tag = game.platform.window.as_ref().unwrap().gpu.build_tag();
        nfsu2::render::record_builds(Some(tag));
        game.explore(None).unwrap();
        let made = nfsu2::render::record_builds(None);
        assert!(made.is_empty(), "{}: the load after the menus made: {made:?}", preset.name());
        let made = made_by_frames(&mut game, 10);
        assert!(made.is_empty(), "{}: its frames made: {made:?}", preset.name());
    }
    // A preset switched to in the menus: the next pass makes its programs.
    let mut game = game_at(nfsu2::qol::Preset::Original);
    menus_until_made(&mut game);
    nfsu2::qol::Preset::Extreme.apply(&mut game.settings.enh);
    game.settings.apply_enhancements();
    game.loads.preload = None;
    menus_until_made(&mut game);
    let tag = game.platform.window.as_ref().unwrap().gpu.build_tag();
    nfsu2::render::record_builds(Some(tag));
    game.explore(None).unwrap();
    let made = nfsu2::render::record_builds(None);
    assert!(made.is_empty(), "Extreme after Original in the menus: the load made: {made:?}");
}

/// Quick Race's car select loads its track's region ahead when that is
/// not free roam's, and the other menus go back to free roam's.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn quick_race_loads_its_region_ahead() {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("QR_AHEAD_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    let mode = screens::QrMode::Drag;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let e = game.quick_event(mode);
    let event = &game.game_data.career.events[e];
    let track = event.races.first().map_or(event.track, |r| r.0);
    let region = game.game_data.career.tracks[&track].region.clone();
    assert_ne!(region, EXPLORE_REGION, "the first drag strip is a region of its own");
    game.mode = game.quick_car_select(Some(e));
    game.step_for(1.0 / 60.0).unwrap();
    assert_eq!(game.loads.preload.as_ref().map(|p| p.region.as_str()), Some(region.as_str()));
    assert!(game.loads.preload.as_ref().is_some_and(|p| p.world.is_some() && p.car.is_none()));
    // The start takes it: no second load of the region's world.
    let world = game.loads.preload.as_mut().unwrap().world.take().unwrap().join().unwrap();
    assert!(world.0.is_ok() && world.1.is_some(), "the region's world and its renderer");
    game.loads.preload = None;
    game.mode = game.icon_menu(MenuKind::Main);
    game.step_for(1.0 / 60.0).unwrap();
    assert_eq!(game.loads.preload.as_ref().map(|p| p.region.as_str()), Some(EXPLORE_REGION));
}

/// The boot screens drawn until the work ahead is done (free roam's world
/// loaded ahead and the programs made, as behind the boot movies and the
/// title), then the main menu: its garage scene (the platform's renderer,
/// the FE car's and its floor reflection) makes no pipeline and no shader
/// module in its frames, at every preset.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_menus_garage_scene_after_the_boot_screens_makes_no_program() {
    super::test_isolation::exclusive();
    for preset in nfsu2::qol::PRESETS {
        let mut game = game_at(preset);
        game.mode = game.boot_screen(BootStep::Blank);
        for _ in 0..3000 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            assert!(matches!(game.mode, Mode::Menu(Menu::Movie(_) | Menu::Splash(_))), "still on the boot screens");
            assert!(game.render.garage.is_none());
            if ahead_done(&game) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(ahead_done(&game), "the work ahead ended");
        game.mode = game.icon_menu(MenuKind::Main);
        let made = made_by_frames(&mut game, 10);
        assert!(
            game.render
                .garage
                .as_ref()
                .is_some_and(|g| g.renderer.is_some() && g.car.as_ref().is_some_and(|c| c.3.is_some())),
            "the garage drawn"
        );
        assert!(made.is_empty(), "{}: the main menu's garage scene made: {made:?}", preset.name());
    }
}

/// The loads and the programs made ahead are done.
fn ahead_done(game: &Game) -> bool {
    let loaded = game.loads.preload.as_ref().is_some_and(|p| {
        p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished())
    });
    loaded && game.loads.programs.as_ref().is_some_and(|p| p.job.is_finished())
}

/// A city circuit with traffic: the race's traffic, paused for the start,
/// gets its renderers built ahead on a worker, and the frames after the
/// start take them instead of building them (`render::cars_ahead`).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_race_traffic_takes_renderers_built_ahead() {
    super::test_isolation::exclusive();
    let mut game = game_at(nfsu2::qol::Preset::Original);
    let e = game
        .game_data
        .career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| e.kind == career::EventKind::Circuit)
        .map(|e| e.index)
        .unwrap();
    game.start_race(e, false, false).unwrap();
    let traffic = |game: &Game| match &game.mode {
        Mode::Race(r) => r.racers.traffic.as_ref().map(|t| t.cars.iter().map(|c| c.id.key()).collect::<Vec<_>>()),
        _ => None,
    };
    let ids = traffic(&game).expect("the circuit has traffic");
    for _ in 0..900 {
        game.step_for(1.0 / 60.0).unwrap();
        game.draw().unwrap();
        if ids.iter().all(|&id| game.render.cars.has_key(id)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(ids.iter().all(|&id| game.render.cars.has_key(id)), "every traffic car drawn");
    assert!(game.render.cars_ahead.taken > 0, "renderers built ahead were taken");
    assert_eq!(game.render.cars_ahead.ready_count(), 0, "none left over");
}

/// A city circuit's rivals: their renderers are built with their cars'
/// loads, in the race's set-up (`line_up_rivals`) or behind the race's
/// Engage dialog (`RivalsAhead`), and taken by the frames that draw them;
/// the race's first frames are the ones drawn with the renderers built in
/// those frames (the same start with the work switched off), at Original
/// and the Modern preset, from free roam and from the Engage dialog.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_race_rivals_take_renderers_built_with_their_loads() {
    super::test_isolation::exclusive();
    nfsu2::surface_maps::set_blocking(true);
    let run = |preset: nfsu2::qol::Preset, engage: bool, ahead: bool| -> Vec<Vec<u8>> {
        crate::render::cars_ahead::set_with_loads_off(!ahead);
        let mut game = game_at(preset);
        let e = game
            .game_data
            .career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .unwrap();
        if engage {
            game.explore(None).unwrap();
            game.mode = Mode::Menu(Menu::Engage(Engage {
                event: e,
                message: 3,
                opened: 0.0,
                focus: Default::default(),
                purse: game.game_data.career.events[e].reward,
            }));
            for _ in 0..20 {
                game.step_for(1.0 / 60.0).unwrap();
                game.draw().unwrap();
            }
            let t = std::time::Instant::now();
            let done = |g: &Game| g.loads.rivals_ahead.as_ref().is_some_and(|a| a.loads.iter().all(|(_, h, _)| h.is_finished()));
            while !done(&game) && t.elapsed().as_secs() < 60 {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(done(&game), "the rivals loaded ahead");
            let built = game.loads.rivals_ahead.as_ref().unwrap().loads.iter().filter(|l| l.2.as_ref().is_some_and(|s| s.lock().unwrap().is_some())).count();
            assert_eq!(built > 0, ahead, "their renderers built with their loads");
        }
        game.start_race(e, false, false).unwrap();
        if engage {
            let (taken, made) = game.loads.rivals_taken;
            assert!(made > 0 && taken == made, "the race took {taken} of its {made} rivals loaded ahead");
        }
        let rivals: Vec<u64> = match &game.mode {
            Mode::Race(r) => r.racers.rivals.iter().map(|r| r.id.key()).collect(),
            _ => Vec::new(),
        };
        assert!(!rivals.is_empty(), "the circuit has rivals");
        let taken = game.render.cars_ahead.taken;
        let mut pictures = Vec::new();
        for _ in 0..5 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            pictures.push(picture(&mut game));
        }
        // Each rival drawn so far took the renderer built with its load.
        let drawn = rivals.iter().filter(|&&id| game.render.cars.has_key(id)).count() as u64;
        assert!(drawn > 0, "a rival drawn");
        if ahead {
            assert!(game.render.cars_ahead.taken >= taken + drawn, "the rivals' renderers built ahead were taken");
        }
        pictures
    };
    for preset in [nfsu2::qol::Preset::Original, nfsu2::qol::Preset::Modern] {
        for engage in [false, true] {
            let ahead = run(preset, engage, true);
            let in_place = run(preset, engage, false);
            crate::render::cars_ahead::set_with_loads_off(false);
            for (i, (a, b)) in ahead.iter().zip(&in_place).enumerate() {
                assert!(a == b, "{} (from the Engage dialog: {engage}): the race's frame {i} differs", preset.name());
            }
        }
    }
}

/// The shop exit with a part fitted (a new car on the kept world): the
/// drive's Animated Water kept across the car change (`Water::for_new_drive`)
/// is the water a new drive builds. Two games do the same exit at the casino
/// square's fountains, one taking the kept water and one with it dropped
/// (built again by the load's warm-up): the same water state after the load,
/// and the same pictures in the first frames, at High, Ultra and Extreme
/// (the plain tier and the higher ones, whose halves are made again).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_water_kept_across_a_car_change_is_a_new_one() {
    super::test_isolation::exclusive();
    // Surface Detail's maps before the first frame, as the headless modes
    // make them (in the background their timing would differ run to run).
    nfsu2::surface_maps::set_blocking(true);
    let run = |preset: nfsu2::qol::Preset, keep: bool| -> (u64, Vec<Vec<u8>>) {
        let mut game = game_at(preset);
        at_the_fountains(&mut game);
        for _ in 0..30 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
        }
        if let Some(p) = game.profile.as_mut() {
            let car = p.car.clone();
            let part = (0..0x40usize).find_map(|slot| {
                game.game_data.parts.find_after(&car, slot, None).filter(|x| x.level > 0).map(|x| (slot, x.hash))
            });
            let (slot, hash) = part.expect("a part to fit");
            p.installed.insert(slot as u8, hash);
        }
        let at = game.here();
        game.drop_drive();
        assert!(game.render.resident_water.is_some(), "the water kept with the world");
        if !keep {
            game.render.resident_water = None;
        }
        game.explore(at).unwrap();
        assert!(game.render.resident_water.is_none(), "taken by the load");
        let digest = game.sim.drive.as_ref().unwrap().effects.water.new_drive_digest();
        let mut pictures = Vec::new();
        for _ in 0..5 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            pictures.push(picture(&mut game));
        }
        (digest, pictures)
    };
    let mut tiers = Vec::new();
    for preset in [nfsu2::qol::Preset::Modern, nfsu2::qol::Preset::Ultra, nfsu2::qol::Preset::Extreme] {
        let (kept, kept_pictures) = run(preset, true);
        let opts = nfsu2::render::RenderOptions::in_force();
        tiers.push(if opts.animated_water { opts.water_tier.max(1) } else { 0 });
        let (new, new_pictures) = run(preset, false);
        assert_eq!(kept, new, "{}: the kept water's state is a new one's", preset.name());
        for (i, (a, b)) in kept_pictures.iter().zip(&new_pictures).enumerate() {
            assert!(a == b, "{}: frame {i} after the exit differs", preset.name());
        }
    }
    assert!(
        tiers.iter().any(|&t| t == 1) && tiers.iter().any(|&t| t >= 2),
        "the plain tier and a higher one: {tiers:?}"
    );
}

/// A car change made ready in the shop's menus (`loads::kept`: the kept
/// world's reset, its renderer's instances, the water's and the smoke's
/// per-drive halves; the car's renderer built on the preloaded car's
/// thread) gives the exit the drive the exit's own work gives: two games
/// fit the same part at the fountains, stay the same frames in the body
/// shop's menus, one with the work made ahead and one without, then leave:
/// the same world (every field of the cache's dump), the same water state
/// and the same pictures of the first five frames at the Modern, Ultra
/// and Extreme presets.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_car_change_made_ready_in_the_menus_is_the_one_made_at_the_exit() {
    super::test_isolation::exclusive();
    nfsu2::surface_maps::set_blocking(true);
    type Sections = Vec<(String, nfsu2::cache::Bytes)>;
    let run = |preset: nfsu2::qol::Preset, ahead: bool| -> (Sections, u64, Vec<Vec<u8>>) {
        crate::render::cars_ahead::set_with_loads_off(!ahead);
        let mut game = game_at(preset);
        at_the_fountains(&mut game);
        for _ in 0..30 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
        }
        if let Some(p) = game.profile.as_mut() {
            let car = p.car.clone();
            let part = (0..0x40usize).find_map(|slot| {
                game.game_data.parts.find_after(&car, slot, None).filter(|x| x.level > 0).map(|x| (slot, x.hash))
            });
            let (slot, hash) = part.expect("a part to fit");
            p.installed.insert(slot as u8, hash);
        }
        let at = game.here();
        let shop = game.game_data.career.shops.iter().position(|s| s.kind == 2).expect("a body shop");
        game.shop.visit = Some(game.game_data.career.shops[shop].hash);
        game.mode = game.shop_front_end(shop);
        game.drop_drive();
        for _ in 0..40 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
        }
        // The work ahead ended (without frames: both games draw the same).
        let car_done = |g: &Game| g.loads.preload.as_ref().is_none_or(|p| p.car.as_ref().is_none_or(|c| c.1.is_finished()));
        let kept_done = |g: &Game| g.loads.kept_ahead.as_ref().is_none_or(|k| k.is_finished());
        let t = std::time::Instant::now();
        while !(car_done(&game) && kept_done(&game)) && t.elapsed().as_secs() < 60 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(game.loads.kept_ahead.is_some(), ahead, "the kept world made ready in the menus");
        let renderer_ahead =
            game.loads.preload.as_ref().and_then(|p| p.car_renderer.as_ref()).is_some_and(|s| s.lock().unwrap().is_some());
        assert_eq!(renderer_ahead, ahead, "the car's renderer built on its thread");
        game.explore(at).unwrap();
        assert!(game.loads.kept_ahead.is_none() && game.render.kept_extras.is_none(), "taken by the load");
        let drive = game.sim.drive.as_ref().unwrap();
        let (world, digest) = (drive.world.to_sections(), drive.effects.water.new_drive_digest());
        let mut pictures = Vec::new();
        for _ in 0..5 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            pictures.push(picture(&mut game));
        }
        (world, digest, pictures)
    };
    for preset in [nfsu2::qol::Preset::Modern, nfsu2::qol::Preset::Ultra, nfsu2::qol::Preset::Extreme] {
        let (world, water, pictures) = run(preset, true);
        let (world_new, water_new, pictures_new) = run(preset, false);
        crate::render::cars_ahead::set_with_loads_off(false);
        assert!(
            world.len() == world_new.len() && world.iter().zip(&world_new).all(|(a, b)| a == b),
            "{}: the world made ready ahead differs from the exit's",
            preset.name()
        );
        assert_eq!(water, water_new, "{}: the water's state", preset.name());
        for (i, (a, b)) in pictures.iter().zip(&pictures_new).enumerate() {
            assert!(a == b, "{}: frame {i} after the exit differs", preset.name());
        }
    }
}

/// The shops' garage scenes (each kind's front end over free roam: its
/// platform's renderer and the FE car's) after a free-roam load: their
/// frames make no pipeline and no shader module, at every preset.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_shops_garage_scenes_after_a_load_make_no_program() {
    super::test_isolation::exclusive();
    for preset in nfsu2::qol::PRESETS {
        let mut game = game_at(preset);
        game.explore(None).unwrap();
        for _ in 0..5 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
        }
        let mut kinds: Vec<u8> = game.game_data.career.shops.iter().map(|s| s.kind).collect();
        kinds.sort();
        kinds.dedup();
        for kind in kinds {
            let i = game.game_data.career.shops.iter().position(|s| s.kind == kind).unwrap();
            game.shop.visit = Some(game.game_data.career.shops[i].hash);
            game.mode = game.shop_front_end(i);
            let made = made_by_frames(&mut game, 10);
            let drawn = game.render.garage.as_ref().map(|g| g.platform.name.clone());
            assert!(made.is_empty(), "{}: shop kind {kind} ({drawn:?}) made: {made:?}", preset.name());
        }
    }
}


/// A lost race's ending companion (the AI winner riding along in the
/// ending, `SceneCar::from_rival`): its renderer is built during the
/// race-over hold once the player has finished behind the winner
/// (`CompanionAhead`), offered for the companion and taken by the ending's
/// first frame that draws it; with the work off none is offered (the
/// frame builds it). The simulation is the same either way (the state
/// hashes of the ending's frames). Each of a stage-1 drag and sprint is
/// driven by its rivals until the first has finished; the player is put
/// over the line half a second behind it. The ending's pictures are not
/// compared: two identical runs in one process already differ in some of
/// the ending's frames (the whole frame's brightness), whatever this
/// change does (docs/first-use-builds.md "Car renderers ahead").
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_lost_race_ending_takes_the_companion_built_ahead() {
    super::test_isolation::exclusive();
    nfsu2::surface_maps::set_blocking(true);
    let run = |preset: nfsu2::qol::Preset, kind: career::EventKind, ahead: bool| -> (Vec<[u64; 5]>, bool) {
        crate::render::cars_ahead::set_with_loads_off(!ahead);
        let mut game = game_at(preset);
        let e = game
            .game_data
            .career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| e.kind == kind && e.races.len() == 1 && e.opponents.len() == 3)
            .map(|e| e.index)
            .unwrap();
        game.explore(None).unwrap();
        game.start_race(e, false, false).unwrap();
        // The rivals race; the player stands until one has finished.
        let finished = |g: &Game| match &g.mode {
            Mode::Race(r) => r.racers.rivals.iter().any(|r| r.progress.finished.is_some() && r.progress.dropped.is_none()),
            _ => false,
        };
        for _ in 0..60 * 600 {
            game.step_for(1.0 / 60.0).unwrap();
            if finished(&game) {
                break;
            }
        }
        assert!(finished(&game), "a rival finished");
        game.step_for(0.5).unwrap();
        let Mode::Race(r) = &mut game.mode else { panic!("still racing") };
        r.racers.me.finished = Some(r.timing.clock);
        // The race-over hold, then the ending.
        let (mut states, mut offered) = (Vec::new(), None);
        for _ in 0..60 * 10 {
            game.step_for(1.0 / 60.0).unwrap();
            let companion = game.scenes.shop.as_ref().and_then(|s| s.companion.as_ref()).map(|c| c.id.key());
            if let Some(id) = companion {
                offered.get_or_insert(game.render.cars_ahead.has_ready(id));
            }
            game.draw().unwrap();
            if let Some(id) = companion {
                assert!(!game.render.cars_ahead.has_ready(id), "taken by the frame that drew it");
                states.push(crate::dev::state_hash::components(&game));
                if states.len() == 5 {
                    break;
                }
            }
        }
        assert_eq!(states.len(), 5, "an ending with the companion played");
        (states, offered == Some(true))
    };
    for preset in [nfsu2::qol::Preset::Original, nfsu2::qol::Preset::Modern] {
        for kind in [career::EventKind::Drag, career::EventKind::Sprint] {
            let (ahead, offered) = run(preset, kind, true);
            let (in_place, offered_in_place) = run(preset, kind, false);
            crate::render::cars_ahead::set_with_loads_off(false);
            assert!(offered, "{} {kind:?}: the companion's renderer built ahead, for its first frame", preset.name());
            assert!(!offered_in_place, "{} {kind:?}: none built ahead with the work off", preset.name());
            assert_eq!(ahead, in_place, "{} {kind:?}: the same ending", preset.name());
        }
    }
}

/// Ambient Detail's props placed by the menus' work ahead (for the world as
/// loaded) are the ones a drive's first frame draws. In the city, free
/// roam's and a race's barriers hold none of their surfaces, so neither
/// the load nor the frames place them again; on a drift track the race's
/// barriers switch road pieces on, so the load (its warm-up) places them
/// again for the race and the frames draw those. Either way they are the
/// props placed for the world as the first frame has it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn ambient_props_made_ahead_reach_the_first_frame() {
    super::test_isolation::exclusive();
    use nfsu2::render::ambient_detail::{AmbientDetail, placements_made};
    let mut game = game_at(nfsu2::qol::Preset::Original);
    game.settings.enh.ambient_detail = 1;
    game.settings.apply_enhancements();
    let gpu = game.platform.window.as_ref().unwrap().gpu.clone();
    // The props drawn equal those placed now for the drive's world.
    let drawn_are_placed_now = |game: &Game, what: &str| {
        let kept = game.render.world.as_ref().and_then(|r| r.ambient()).expect("Ambient Detail's props");
        let now = AmbientDetail::new(&gpu, &game.sim.drive.as_ref().unwrap().world);
        let bytes = |a: &AmbientDetail| {
            a.props().iter().flat_map(|p| bytemuck::bytes_of(&p.inst).to_vec()).collect::<Vec<u8>>()
        };
        assert!(!kept.props().is_empty(), "{what}: no props");
        assert!(bytes(kept) == bytes(&now), "{what}: the props drawn are not those of the drive's world");
    };
    // The region loaded ahead in a menu (`wait`), then the drive: the
    // props placed by its load and by its first frames.
    let drive = |game: &mut Game, what: &str, go: &dyn Fn(&mut Game)| -> (usize, usize) {
        for _ in 0..3000 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            if ahead_done(game) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(ahead_done(game), "{what}: the work ahead ended");
        let before = placements_made();
        go(game);
        let by_load = placements_made() - before;
        for _ in 0..3 {
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
        }
        (by_load, placements_made() - before - by_load)
    };
    let to_menu = |game: &mut Game| {
        game.drop_drive();
        game.loads.resident = None;
        game.loads.kept_ahead = None;
        game.render.resident = None;
        game.loads.preload = None;
    };
    // Free roam from the main menu.
    game.mode = game.icon_menu(MenuKind::Main);
    let placed = drive(&mut game, "free roam", &|g| g.explore(None).unwrap());
    assert_eq!(placed, (0, 0), "free roam: props placed by its load, by its frames");
    drawn_are_placed_now(&game, "free roam");
    // Quick Race from its car select (the track's region loaded ahead): a
    // city circuit, then a drift track.
    for (mode, what, by_load) in
        [(screens::QrMode::Circuit, "a city circuit", 0), (screens::QrMode::Drift, "a drift track", 1)]
    {
        to_menu(&mut game);
        game.quick.tracks = game.quick_track_select(mode, 0.0);
        let e = game.quick_event(mode);
        game.quick.car = Some("PEUGOT".into());
        game.mode = game.quick_car_select(Some(e));
        let placed = drive(&mut game, what, &|g| g.start_race(e, true, false).unwrap());
        assert_eq!(placed, (by_load, 0), "{what}: props placed by its load, by its frames");
        drawn_are_placed_now(&game, what);
    }
}

/// The SSR mask from the main view (docs/wet-roads-ssr.md "The mask from the main view"): the Enhanced level
/// High (Screen-Space Reflections, the lamps' depth pre-pass) at one sample (the native Full Screen
/// Anti-Aliasing off), in the rain from the first frame: the main view's road pipelines with the mask's
/// targets and the mask pass's road are made by the load, nothing in the frames; and the frames took them.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_mask_from_the_main_view_is_ready_before_its_first_frame() {
    super::test_isolation::exclusive();
    let mut game = game_at(nfsu2::qol::Preset::Original);
    nfsu2::qol::Level::High.apply(&mut game.settings.enh);
    game.settings.custom[14] = 0;
    game.settings.apply_enhancements();
    crate::dev::hooks::SHOT_RAIN_CLOCK.store(162f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
    game.explore(None).unwrap();
    let before = nfsu2::render::native::world::plan::ssr_mask_from_main();
    let made = made_by_frames(&mut game, 10);
    let from_main = nfsu2::render::native::world::plan::ssr_mask_from_main() - before;
    crate::dev::hooks::SHOT_RAIN_CLOCK.store(0, std::sync::atomic::Ordering::Relaxed);
    assert!(made.is_empty(), "made in its frames: {made:?}");
    let device = nfsu2::render::native::world::plan::MASK_FROM_MAIN_DEVICE.load(std::sync::atomic::Ordering::Relaxed);
    assert!(device != 1 || from_main >= 5, "the mask from the main view in {from_main} of 10 frames");
}

/// A world kept for the next drive (the last race's region) draws the
/// Ambient Detail props of that drive's barriers, placed by its load:
/// Quick Races one after the other on a region's tracks whose barriers
/// switch other road pieces on (drift tracks), keeping the world each time
/// another way — the drive kept for the same car, its world and renderer
/// kept for another car, the world kept beside the menus after a car change
/// (made ready there, or taken at once) — and URL circuits (no props there,
/// but the ground the debris lands on). Where two drives' barriers give the
/// same surfaces (two city circuits), the props kept are drawn and none is
/// placed. Each time the drawn props, vents and ground are those placed then
/// for the drive's world.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_kept_world_draws_the_next_drives_props() {
    super::test_isolation::exclusive();
    use nfsu2::render::ambient_detail::{AmbientDetail, placements_made};
    use screens::QrMode;
    let mut game = game_at(nfsu2::qol::Preset::Original);
    game.settings.enh.ambient_detail = 1;
    game.settings.apply_enhancements();
    let gpu = game.platform.window.as_ref().unwrap().gpu.clone();
    let frame = |game: &mut Game| {
        game.step_for(1.0 / 60.0).unwrap();
        game.draw().unwrap();
    };
    // The props drawn, and those placed now for the drive's world.
    let drawn = |game: &Game| {
        game.render.world.as_ref().and_then(|r| r.ambient()).expect("Ambient Detail's props").placement_digest()
    };
    let placed_now = |game: &Game| AmbientDetail::new(&gpu, &game.sim.drive.as_ref().unwrap().world).placement_digest();
    // A Quick Race of `mode` on `track` with `car`, from its car select
    // drawn until the work ahead has ended (`menu`; else at once): the
    // props placed by its load and by its first three frames (the work
    // ahead places props on its own threads too), and whether a world was
    // made ready in the menus for it.
    let race = |game: &mut Game, mode: QrMode, track: u16, car: &str, menu: bool| -> ((usize, usize), bool) {
        let ty = nfsu2::frontend::quick_race::race_type(mode);
        game.quick.tracks = game.quick_track_select(mode, 0.0);
        game.settings.qr[ty].track = track;
        game.settings.qr[ty].reversed = false;
        let e = game.quick_event(mode);
        game.quick.car = Some(car.into());
        game.mode = game.quick_car_select(Some(e));
        for k in 0..if menu { 3000 } else { 0 } {
            frame(game);
            let loads = &game.loads;
            let preload = loads.preload.as_ref().is_none_or(|p| {
                p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished())
            });
            let kept = loads.kept_ahead.as_ref().is_none_or(|k| k.is_finished());
            let programs = loads.programs.as_ref().is_none_or(|p| p.job.is_finished());
            if k >= 5 && preload && kept && programs {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let ahead = game.loads.kept_ahead.is_some();
        let before = placements_made();
        game.start_race(e, true, false).unwrap();
        let by_load = placements_made() - before;
        for _ in 0..3 {
            frame(game);
        }
        ((by_load, placements_made() - before - by_load), ahead)
    };
    // The drives in turn: (what, mode, track, car, how the world is kept,
    // whether its barriers give other surfaces than the last drive's).
    enum Kept {
        /// A new region (nothing kept).
        No,
        /// The drive itself (the same car).
        Drive,
        /// A car change in the menus: the world kept beside them, made
        /// ready there (`KeptAhead`) or taken at once.
        Menus { ahead: bool },
    }
    let city: Vec<u16> = game
        .quick_track_select(QrMode::Circuit, 0.0)
        .expect("the circuits")
        .tracks
        .iter()
        .map(|t| t.number)
        .filter(|n| game.game_data.career.tracks.get(n).is_some_and(|t| t.region == EXPLORE_REGION))
        .take(2)
        .collect();
    assert_eq!(city.len(), 2, "two city circuits");
    let drives = [
        ("a drift track", QrMode::Drift, 4312, "PEUGOT", Kept::No, true),
        ("another drift track, the same car", QrMode::Drift, 4311, "PEUGOT", Kept::Drive, true),
        ("another drift track, another car", QrMode::Drift, 4313, "COROLLA", Kept::Drive, true),
        ("a drift track after a car change", QrMode::Drift, 4314, "PEUGOT", Kept::Menus { ahead: true }, true),
        ("after a car change, at once", QrMode::Drift, 4315, "COROLLA", Kept::Menus { ahead: false }, true),
        ("a URL circuit", QrMode::Url, 4701, "PEUGOT", Kept::No, true),
        ("another URL circuit", QrMode::Url, 4712, "PEUGOT", Kept::Drive, true),
        ("a city circuit", QrMode::Circuit, city[0], "PEUGOT", Kept::No, true),
        ("another city circuit", QrMode::Circuit, city[1], "PEUGOT", Kept::Drive, false),
    ];
    let mut last = None;
    for (what, mode, track, car, kept, other) in drives {
        let menu = match kept {
            Kept::No | Kept::Drive => true,
            Kept::Menus { ahead } => {
                game.drop_drive();
                ahead
            }
        };
        let (placed, made_ready) = race(&mut game, mode, track, car, menu);
        assert_eq!(made_ready, matches!(kept, Kept::Menus { ahead: true }), "{what}: a world made ready in the menus");
        let (drawn, now) = (drawn(&game), placed_now(&game));
        assert!(drawn == now, "{what}: the props drawn are not those of the drive's world");
        if !matches!(kept, Kept::No) {
            assert_eq!(last != Some(now), other, "{what}: other surfaces than the last drive's");
            assert_eq!(placed, (usize::from(other), 0), "{what}: props placed by its load, by its frames");
        }
        last = Some(now);
    }
}
