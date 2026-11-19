//! Reported issues: asset-backed reproductions run through the live entry points (`Game::step`,
//! `Game::key`). They load the original's assets: without NFSU2_ROOT they
//! are ignored with that reason (docs/developing/testing.md). The `#[ignore]`d ones
//! also need a GPU or private XDG homes and run only on request
//! (`cargo test -- --ignored NAME`).
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// What one first-drag run saw.
#[derive(Debug, Default)]
struct DragRun {
    scene: String,
    /// The least upright (car z axis · world z) of each rival over the run.
    min_up: Vec<f32>,
    /// The first rival tilted past 60° (z < 0.5): (rival, race clock,
    /// its drop-out status then, its hardest contact impulse so far).
    tipped: Option<(usize, f32, Option<u16>, f32)>,
    /// Rivals that dropped out (status 9 totaled / 8 blown) and when.
    dropouts: Vec<(usize, u16, f32)>,
    /// The player's finish time and the HUD's finish order then.
    finished: Option<f32>,
    hud_order: u32,
    /// 1 + the rivals home before the player (not dropped), from the
    /// rivals' own finish times.
    true_order: u32,
    rivals: Vec<(Option<f32>, Option<u16>)>,
    shifts: usize,
    nitrous_frames: usize,
}

/// Where the profile comes from.
#[derive(Clone, Copy, Debug)]
enum Start {
    Fresh(&'static str),
    Finished,
}

/// The first career drag (stage 1's first drag event) driven with the
/// player's keys: W held from the start scene on, Left Shift (Controls row
/// 4, Shift Up) tapped near the red line, Left Alt (row 7) held from
/// `nos_at` s after green; Enter tapped at `skip` of the scene's length
/// (None: the scene plays out).
fn drag_run(start: Start, hz: f32, skip: Option<f32>, nos_at: Option<f32>, seed: u32) -> DragRun {
    let mut game = Game::new().unwrap();
    match start {
        Start::Fresh(car) => {
            let mut profile = Profile::new("DRAG_REPORT_TEST");
            profile.skip_intro(&game.game_data.career, &game.game_data.root, car, &mut game.rng);
            game.profile = Some(profile);
        }
        Start::Finished => load_finished_career(&mut game).unwrap(),
    }
    game.rng = nfsu2::opponents::GameRng(seed);
    let event =
        game.game_data.career.stage_events(&game.game_data.root, 1).into_iter().find(|e| e.kind == career::EventKind::Drag).unwrap().index;
    game.start_race(event, false, false).unwrap();
    let mut run = DragRun {
        scene: game.scenes.shop.as_ref().map_or("none".into(), |s| s.scene.kind.name()),
        ..Default::default()
    };
    let length = game.scenes.shop.as_ref().map_or(0.0, |s| s.scene.length as f32);
    let mut skipped = skip.is_none();
    game.platform.keys.insert(KeyCode::KeyW);
    let redline = game.sim.drive.as_ref().unwrap().car.car_type.handling.engine.redline_rpm;
    let mut shift_wait = 0usize;
    let mut steer_hold = 0usize;
    let mut next_trace = 0.0f32;
    let trace = std::env::var_os("DRAG_TRACE").is_some();
    for _ in 0..(150.0 * hz) as usize {
        if !skipped && game.scenes.shop.as_ref().is_some_and(|s| s.frame >= skip.unwrap() * length) {
            game.platform.keys.insert(KeyCode::Enter);
            game.key(KeyCode::Enter, &mut || {}).unwrap();
            game.platform.keys.remove(&KeyCode::Enter);
            skipped = true;
        }
        let (clock, finished) = match &game.mode {
            Mode::Race(r) => (r.timing.clock, r.racers.me.finished),
            _ => break,
        };
        if finished.is_none() && clock >= 0.0 {
            if nos_at.is_some_and(|t| clock >= t) {
                game.platform.keys.insert(KeyCode::AltLeft);
            }
            let d = game.sim.drive.as_ref().unwrap();
            if shift_wait == 0 && d.state.rpm() >= 0.95 * redline {
                game.platform.keys.insert(KeyCode::ShiftLeft);
                game.key(KeyCode::ShiftLeft, &mut || {}).unwrap();
                game.platform.keys.remove(&KeyCode::ShiftLeft);
                run.shifts += 1;
                shift_wait = (0.5 * hz) as usize;
            }
            shift_wait = shift_wait.saturating_sub(1);
            // A player changes lanes round the traffic ahead (the steering
            // keys are the drag lane events): a car within 80 m ahead and
            // 2.5 m aside of the player's line, the key away from it held
            // 0.25 s.
            if steer_hold > 0 {
                steer_hold -= 1;
                if steer_hold == 0 {
                    game.platform.keys.remove(&KeyCode::ArrowLeft);
                    game.platform.keys.remove(&KeyCode::ArrowRight);
                }
            } else if let Mode::Race(r) = &game.mode {
                let d = game.sim.drive.as_ref().unwrap();
                let (at, fwd) = (d.state.position, d.state.forward());
                let left = Vec3::Z.cross(fwd);
                let blocking = r
                    .racers.traffic
                    .iter()
                    .flat_map(|t| t.cars.iter().filter(|c| c.lane.is_some()))
                    .map(|c| c.state.position - at)
                    .find(|rel| (0.0..80.0).contains(&rel.dot(fwd)) && rel.dot(left).abs() < 2.5);
                if let Some(rel) = blocking {
                    let key = if rel.dot(left) >= 0.0 { KeyCode::ArrowRight } else { KeyCode::ArrowLeft };
                    game.platform.keys.insert(key);
                    steer_hold = (0.25 * hz) as usize;
                }
            }
        }
        let tilting =
            matches!(&game.mode, Mode::Race(r) if r.racers.rivals.iter().any(|rv| (rv.state.orientation * Vec3::Z).z < 0.9));
        if trace && (tilting || matches!(&game.mode, Mode::Race(r) if r.timing.clock >= next_trace)) {
            let Mode::Race(r) = &game.mode else { unreachable!() };
            let d = game.sim.drive.as_ref().unwrap();
            let t = d.state.drivetrain.as_ref();
            eprintln!(
                "TRACE clock={:.2} speed={:.1} gear={:?} rpm={:.0} along={:.0} totaled={} rivals={:?}",
                r.timing.clock,
                d.state.speed(),
                t.map(|t| t.gear),
                d.state.rpm(),
                r.racers.me.along,
                d.state.totaled,
                r.racers.rivals
                    .iter()
                    .map(|rv| format!(
                        "along {:.1} v {:.1} up {:.3} fin {:?} drop {:?} pos {:.1?} vel {:.1?} spin {:.2?} contacts {}",
                        rv.progress.along,
                        rv.state.velocity.length(),
                        (rv.state.orientation * Vec3::Z).z,
                        rv.progress.finished,
                        rv.progress.dropped,
                        rv.state.position,
                        rv.state.velocity,
                        rv.state.spin,
                        rv.state.contacts.len()
                    ))
                    .collect::<Vec<_>>()
            );
            next_trace = r.timing.clock.max(0.0).floor() + 1.0;
        }
        // A drawn frame at that display rate, the window's path.
        game.advance(1.0 / hz).unwrap();
        let Mode::Race(r) = &game.mode else { break };
        if run.min_up.is_empty() {
            run.min_up = vec![1.0; r.racers.rivals.len()];
        }
        for (i, rv) in r.racers.rivals.iter().enumerate() {
            let up = (rv.state.orientation * Vec3::Z).z;
            run.min_up[i] = run.min_up[i].min(up);
            if up < 0.5 && run.tipped.is_none() {
                run.tipped = Some((i, r.timing.clock, rv.progress.dropped, rv.state.hardest_impulse));
            }
            if let (Some(status), Some(at)) = (rv.progress.dropped, rv.progress.finished)
                && !run.dropouts.iter().any(|d| d.0 == i)
            {
                run.dropouts.push((i, status, at));
            }
        }
        if game.sim.drive.as_ref().unwrap().state.drivetrain.as_ref().is_some_and(|t| t.nitrous != 0) {
            run.nitrous_frames += 1;
        }
        if let (Some(t), None) = (r.racers.me.finished, run.finished) {
            run.finished = Some(t);
            run.hud_order = r.racers.me.finish_order(r.racers.rivals.iter().map(|r| &r.progress));
            run.true_order = 1 + r
                .racers.rivals
                .iter()
                .filter(|rv| rv.progress.dropped.is_none() && rv.progress.finished.is_some_and(|f| f < t))
                .count() as u32;
        }
        if run.finished.is_some() && r.racers.rivals.iter().all(|rv| rv.progress.finished.is_some()) {
            break;
        }
    }
    if let Mode::Race(r) = &game.mode {
        run.rivals = r.racers.rivals.iter().map(|rv| (rv.progress.finished, rv.progress.dropped)).collect();
    }
    run
}

/// The first drag: opponents tipping, and the finish message saying
/// second. The matrix: fresh careers in stage 1 cars and the finished-career
/// save, 60 / 144 / 240 Hz, the start scene played out or skipped with
/// Enter at 0 / 10 / 50 / 95 % of it, with and without nitrous, three
/// grid seeds.
#[test]
#[ignore = "requires original assets and isolated XDG homes"]
fn first_drag_on_keys_matrix() {
    let starts: Vec<Start> = match std::env::var("DRAG_START").as_deref() {
        Ok("finished") => vec![Start::Finished],
        Ok("fresh") => vec![Start::Fresh("PEUGOT"), Start::Fresh("COROLLA"), Start::Fresh("CORSA")],
        Ok(car) if car.starts_with("fresh:") => vec![Start::Fresh(Box::leak(car[6..].to_owned().into_boxed_str()))],
        _ => vec![Start::Fresh("PEUGOT"), Start::Fresh("COROLLA"), Start::Fresh("CORSA"), Start::Finished],
    };
    let rates: Vec<f32> = std::env::var("DRAG_HZ")
        .ok()
        .map_or(vec![60.0, 144.0, 240.0], |s| s.split(',').filter_map(|x| x.parse().ok()).collect());
    let mut failures = Vec::new();
    let mut runs = 0;
    for &start in &starts {
        for &hz in &rates {
            let skips: Vec<Option<f32>> =
                std::env::var("DRAG_SKIPS").ok().map_or(vec![None, Some(0.0), Some(0.1), Some(0.5), Some(0.95)], |s| {
                    s.split(',').map(|x| x.parse().ok()).collect()
                });
            let seeds: Vec<u32> = std::env::var("DRAG_SEEDS")
                .ok()
                .map_or(vec![0, 4, 20261103], |s| s.split(',').filter_map(|x| x.parse().ok()).collect());
            for &skip in &skips {
                // Nitrous only where the car has a tank (the finished-career save's Lancer).
                let noses: &[Option<f32>] = if matches!(start, Start::Finished) { &[None, Some(1.5)] } else { &[None] };
                for &nos in noses {
                    for &seed in &seeds {
                        let run = drag_run(start, hz, skip, nos, seed);
                        runs += 1;
                        eprintln!(
                            "DRAG start={start:?} hz={hz} skip={skip:?} nos={nos:?} seed={seed} scene={} shifts={} nos_frames={} min_up={:?} tipped={:?} dropouts={:?} finished={:?} hud_order={} true_order={} rivals={:?}",
                            run.scene,
                            run.shifts,
                            run.nitrous_frames,
                            run.min_up,
                            run.tipped,
                            run.dropouts,
                            run.finished,
                            run.hud_order,
                            run.true_order,
                            run.rivals
                        );
                        // A rival rolling while still racing (not a totaled drop-out), the
                        // HUD order disagreeing, or no finish.
                        if run.tipped.is_some_and(|t| t.2.is_none())
                            || run.hud_order != run.true_order
                            || run.finished.is_none()
                        {
                            failures.push(format!(
                                "{start:?} {hz} {skip:?} {nos:?} {seed}: tipped {:?} order {}/{} finished {:?}",
                                run.tipped, run.hud_order, run.true_order, run.finished
                            ));
                        }
                    }
                }
            }
        }
    }
    eprintln!("first drag matrix: {runs} runs, {} failures", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// An offscreen window of `width` × `height` on the real draw path
/// (`Game::draw`), as `--bench` makes it.
fn offscreen_window(game: &mut Game, width: u32, height: u32) {
    // A window sets the process-wide output size (`qol::set_output_size`):
    // this test alone from here on (`test_isolation`).
    super::test_isolation::exclusive();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("nitro visibility target"),
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

/// The frame drawn as it is, then again without the player's flame
/// particles (the simulation untouched, the motion blur's level put back):
/// the pixels that differ are the flames' visible ones.
fn flame_pixels(game: &mut Game, width: u32, height: u32) -> usize {
    flame_pixels_at(game, width, height, 1.0)
}

/// [`flame_pixels`] for a frame drawn `alpha` of the way between the last
/// two world updates (the window loop's `draw_between`).
fn flame_pixels_at(game: &mut Game, width: u32, height: u32, alpha: f32) -> usize {
    let grab = |game: &mut Game| {
        let w = game.platform.window.as_mut().unwrap();
        w.settle();
        let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), width, height)
    };
    let motion = game.sim.drive.as_ref().unwrap().motion;
    game.draw_between(alpha).unwrap();
    let with = grab(game);
    let d = game.sim.drive.as_mut().unwrap();
    d.motion = motion;
    let kept = d.effects.system.particles.clone();
    d.effects.system.particles.retain(|p| p.key != FLAME_KEY);
    game.draw_between(alpha).unwrap();
    let without = grab(game);
    game.sim.drive.as_mut().unwrap().effects.system.particles = kept;
    with.chunks_exact(4)
        .zip(without.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
        .count()
}

/// A new career as a player starts it (Career, Start New Career, the
/// profile prompts, the movie and the airport scene skipped with Enter),
/// to free roam in Rachel's car (DDAY_PLAYER_CAR); with `read_sms` the
/// D-Day script's SMS screens read (Enter) until it is over. Each frame
/// drawn on the window path.
fn new_career_dday(game: &mut Game, read_sms: bool) {
    let frame = |game: &mut Game| {
        game.pacing.clock = game.pacing.clock.map(|c| c + 1.0 / 60.0);
        game.step_for(1.0 / 60.0).unwrap();
        game.draw().unwrap();
    };
    let tap = |game: &mut Game, code: KeyCode| {
        game.key(code, &mut || {}).unwrap();
        game.platform.keys.insert(code);
        frame(game);
        game.platform.keys.remove(&code);
        frame(game);
    };
    game.mode = game.icon_menu(MenuKind::Main);
    tap(game, KeyCode::ArrowLeft);
    tap(game, KeyCode::Enter);
    tap(game, KeyCode::Enter);
    if matches!(game.mode, Mode::Menu(Menu::McMain(_))) {
        tap(game, KeyCode::ArrowLeft);
    }
    for _ in 0..8 {
        if matches!(game.mode, Mode::Menu(Menu::Movie(_)) | Mode::Explore) {
            break;
        }
        if matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Confirm) {
            tap(game, KeyCode::ArrowLeft);
        }
        tap(game, KeyCode::Enter);
    }
    for _ in 0..60 {
        frame(game);
    }
    tap(game, KeyCode::Enter);
    for _ in 0..60 {
        if matches!(game.mode, Mode::Explore) {
            break;
        }
        frame(game);
    }
    for _ in 0..120 {
        frame(game);
    }
    if game.scenes.shop.is_some() {
        tap(game, KeyCode::Enter);
    }
    assert!(
        matches!(game.mode, Mode::Explore | Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))),
        "free roam after the scene: {}",
        mode_name(&game.mode)
    );
    let mut n = 0;
    while read_sms && game.career_world.dday.running() && n < 60 * 60 {
        if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) && n % 60 == 59 {
            tap(game, KeyCode::Enter);
        } else {
            frame(game);
        }
        n += 1;
    }
    assert_eq!(game.sim.drive.as_ref().unwrap().car.car_type.name, "350Z", "Rachel's car");
}

/// Where the shot is fired.
#[derive(Clone, Copy, Debug)]
enum Situation {
    FreeRoam,
    AfterShop,
    AfterRace,
    AfterPause,
    Race,
    /// Rachel's car (cars "DDAY"): free roam with the D-Day script still
    /// running (its SMS unread), right after the airport scene.
    Intro,
}

/// The player's nitrous flames sometimes missing. On the window's
/// draw path at the reported sizes and Enhancements: in each active,
/// non-purge NOS frame (sampled), the flames' CPU quads and their visible
/// pixels. Needs a GPU. `NITRO_SIZES=1920x1080,…`, `NITRO_ENH="k v;k v|…"`
/// (configurations apart by `|`), `NITRO_CARS`, `NITRO_SITUATIONS`.
#[test]
#[ignore = "requires original assets, a GPU and isolated XDG homes"]
fn player_nitro_flames_visible_on_window_path() {
    let sizes: Vec<(u32, u32)> =
        std::env::var("NITRO_SIZES").ok().map_or(vec![(1920, 1080), (2560, 1440), (3840, 2160)], |s| {
            s.split(',')
                .filter_map(|x| {
                    let (a, b) = x.split_once('x')?;
                    Some((a.parse().ok()?, b.parse().ok()?))
                })
                .collect()
        });
    let enhs: Vec<String> = std::env::var("NITRO_ENH").map_or(vec![
        String::new(),
        // A reported options.txt: MSAA 8x, scale 125 %, reversed-Z, SSAO.
        "enh.render_scale 125;enh.post_aa 5;enh.sharpen 100;enh.road_reflection 2;enh.car_reflection 1024;enh.detail_distance 300;enh.car_detail 300;enh.lod_fade 1;enh.keep_loaded 1;enh.anisotropy 16;enh.reversed_z 1;enh.shadows 2;enh.reflection_rate 1;enh.ssao 100".into(),
        "enh.post_aa 7;enh.enhanced_reflections 1".into(),
        "enh.post_aa 4;enh.aa_filter 4;enh.render_scale 150".into(),
    ], |s| s.split('|').map(str::to_owned).collect());
    let cars: Vec<String> = std::env::var("NITRO_CARS").map_or(
        vec!["DDAY".into(), "PEUGOT".into(), "FINISHED".into(), "MUSTANGGT".into(), "SKYLINE".into(), "HUMMER".into()],
        |s| s.split(',').map(str::to_owned).collect(),
    );
    let situations: Vec<Situation> = match std::env::var("NITRO_SITUATIONS").as_deref() {
        Ok(s) => s
            .split(',')
            .filter_map(|x| {
                Some(match x {
                    "free" => Situation::FreeRoam,
                    "shop" => Situation::AfterShop,
                    "race_free" => Situation::AfterRace,
                    "pause" => Situation::AfterPause,
                    "race" => Situation::Race,
                    "intro" => Situation::Intro,
                    _ => return None,
                })
            })
            .collect(),
        Err(_) => vec![
            Situation::Intro,
            Situation::FreeRoam,
            Situation::AfterShop,
            Situation::AfterRace,
            Situation::AfterPause,
            Situation::Race,
        ],
    };
    let levels: Vec<u8> = std::env::var("NITRO_LEVELS")
        .ok()
        .map_or(vec![1, 3], |s| s.split(',').filter_map(|x| x.parse().ok()).collect());
    let mut misses = Vec::new();
    let mut cases = 0;
    for &(width, height) in &sizes {
        for enh in &enhs {
            for car in &cars {
                for &level in &levels {
                    if car == "FINISHED" && level != levels[0] {
                        continue; // the save's own parts
                    }
                    for &situation in &situations {
                        let mut game = Game::new().unwrap();
                        if car == "FINISHED" {
                            load_finished_career(&mut game).unwrap();
                        } else if car == "DDAY" {
                            // A new career: the profile is made in the flow.
                        } else {
                            let mut profile = Profile::new("NITRO_WINDOW_TEST");
                            profile.skip_intro(&game.game_data.career, &game.game_data.root, car, &mut game.rng);
                            profile.perf.extend(
                                game.game_data.career
                                    .packages
                                    .iter()
                                    .filter(|p| p.category == 2 && p.level == level)
                                    .flat_map(|p| p.parts.iter().map(|x| x.id)),
                            );
                            game.profile = Some(profile);
                        }
                        super::test_isolation::exclusive();
                        for line in enh.split(';').filter(|l| !l.trim().is_empty()) {
                            let (k, v) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
                            game.settings.enh.parse(k, v);
                        }
                        game.settings.apply_enhancements();
                        offscreen_window(&mut game, width, height);
                        game.pacing.clock = Some(0.0);
                        let dt = 1.0 / 60.0;
                        let step = |game: &mut Game| {
                            game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(dt));
                            game.step_for(dt).unwrap();
                        };
                        match situation {
                            _ if car == "DDAY" => {
                                new_career_dday(&mut game, !matches!(situation, Situation::Intro));
                                if matches!(situation, Situation::AfterPause) {
                                    game.key(KeyCode::Escape, &mut || {}).unwrap();
                                    for _ in 0..30 {
                                        step(&mut game);
                                        game.draw().unwrap();
                                    }
                                    game.key(KeyCode::Escape, &mut || {}).unwrap();
                                }
                            }
                            Situation::Intro => unreachable!(),
                            Situation::FreeRoam => game.explore(None).unwrap(),
                            Situation::AfterShop => {
                                game.explore(None).unwrap();
                                let shop = game.game_data.career.shops.iter().position(|s| s.name == "CC_BODYSHOP_1").unwrap();
                                assert!(game.start_shop_scene(nfsu2::shop_scene::SceneKind::Enter, shop));
                                for _ in 0..30 {
                                    step(&mut game);
                                    game.draw().unwrap();
                                }
                                game.key(KeyCode::Enter, &mut || {}).unwrap();
                                for _ in 0..10 {
                                    step(&mut game);
                                    game.draw().unwrap();
                                }
                                game.explore(None).unwrap();
                                for _ in 0..600 {
                                    if game.scenes.shop.is_none() {
                                        break;
                                    }
                                    step(&mut game);
                                    game.draw().unwrap();
                                }
                            }
                            Situation::AfterRace => {
                                let e = game
                                    .game_data.career
                                    .stage_events(&game.game_data.root, 1)
                                    .into_iter()
                                    .find(|e| e.kind == career::EventKind::Circuit)
                                    .unwrap()
                                    .index;
                                game.start_race(e, false, false).unwrap();
                                game.key(KeyCode::Enter, &mut || {}).unwrap();
                                for _ in 0..300 {
                                    step(&mut game);
                                    game.draw().unwrap();
                                }
                                game.explore(None).unwrap();
                            }
                            Situation::AfterPause => {
                                game.explore(None).unwrap();
                                for _ in 0..30 {
                                    step(&mut game);
                                    game.draw().unwrap();
                                }
                                game.key(KeyCode::Escape, &mut || {}).unwrap();
                                for _ in 0..30 {
                                    step(&mut game);
                                    game.draw().unwrap();
                                }
                                game.key(KeyCode::Escape, &mut || {}).unwrap();
                            }
                            Situation::Race => {
                                let e = game
                                    .game_data.career
                                    .stage_events(&game.game_data.root, 1)
                                    .into_iter()
                                    .find(|e| e.kind == career::EventKind::Sprint)
                                    .unwrap()
                                    .index;
                                game.start_race(e, false, false).unwrap();
                                for _ in 0..3600 {
                                    if game.scenes.shop.is_some() {
                                        game.key(KeyCode::Enter, &mut || {}).unwrap();
                                    }
                                    step(&mut game);
                                    if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.0) {
                                        break;
                                    }
                                }
                            }
                        }
                        game.settings.custom[10] = 1;
                        game.platform.keys.insert(KeyCode::KeyW);
                        let (mut active, mut sampled, mut no_quads, mut invisible) = (0, 0, 0, 0);
                        let mut first_miss = None;
                        let mut pixel_counts = Vec::new();
                        for frame in 0..360 {
                            if frame == 90 {
                                game.platform.keys.insert(KeyCode::AltLeft);
                            }
                            step(&mut game);
                            // A menu over the drive (the D-Day script's SMS
                            // screens) holds the drive; the key stays held.
                            if !matches!(game.mode, Mode::Explore | Mode::Race(_)) {
                                game.draw().unwrap();
                                continue;
                            }
                            let d = game.sim.drive.as_ref().unwrap();
                            let stage = d.state.drivetrain.as_ref().map_or(0, |t| t.nitrous);
                            if stage == 0
                                || nfsu2::audio::carfx::purging(stage, d.state.speed(), d.state.gas())
                                || frame % 6 != 0
                            {
                                game.draw().unwrap();
                                continue;
                            }
                            active += 1;
                            let mut flames = d.effects.system.clone();
                            flames.particles.retain(|p| p.key == FLAME_KEY);
                            let axes = CameraAxes::look(d.camera.forward(), d.camera.up);
                            let quads = flames.quads(
                                &d.world.emitters,
                                d.camera.position,
                                axes.right,
                                -axes.down,
                                axes.forward,
                                nfsu2::render::cull_scale(d.camera.game_fov.unwrap_or(d.camera.fov_y)),
                            );
                            let pixels = flame_pixels(&mut game, width, height);
                            sampled += 1;
                            pixel_counts.push(pixels);
                            no_quads += usize::from(quads.is_empty());
                            if pixels == 0 {
                                invisible += 1;
                                first_miss.get_or_insert((frame, quads.len()));
                            }
                        }
                        cases += 1;
                        let tank = game.sim.drive.as_ref().and_then(|d| d.state.nitrous);
                        pixel_counts.sort_unstable();
                        let median = pixel_counts.get(pixel_counts.len() / 2).copied().unwrap_or(0);
                        eprintln!(
                            "NITRO size={width}x{height} enh=[{enh}] car={car} level={level} situation={situation:?} pixels_min={:?} pixels_median={median} sampled={sampled} active={active} no_quads={no_quads} invisible={invisible} first_miss={first_miss:?} tank={tank:?}",
                            pixel_counts.first()
                        );
                        if sampled == 0 || invisible > 0 {
                            misses.push(format!("{width}x{height} [{enh}] {car} L{level} {situation:?}: sampled {sampled} invisible {invisible} {first_miss:?}"));
                        }
                    }
                }
            }
        }
    }
    eprintln!("nitro window matrix: {cases} cases, {} with misses", misses.len());
    assert!(misses.is_empty(), "{misses:#?}");
}

/// Regression for the first-drag report (ignored without NFSU2_ROOT):
/// a fresh career's first drag on the player's keys at 60 and 240 Hz, the
/// start scene skipped half way: no rival rolls while still racing, and
/// the race-over message's order is the order of the finish times.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn first_drag_on_keys_keeps_rivals_upright_and_orders_the_finish() {
    testkit::game();
    for hz in [60.0, 240.0] {
        let run = drag_run(Start::Fresh("PEUGOT"), hz, Some(0.5), None, 4);
        assert!(run.finished.is_some(), "{hz} Hz: the player finishes: {run:?}");
        assert!(run.tipped.is_none_or(|t| t.2.is_some()), "{hz} Hz: a racing rival rolled: {run:?}");
        assert_eq!(run.hud_order, run.true_order, "{hz} Hz: {run:?}");
    }
}

/// 0x615CD0 (verified): a flame burst sits at the exhaust marker's
/// position with the CARFX_NITRO template's own axes, whatever the
/// marker's; with the gas off (0x5ECDC0) it does not fire at all. Rachel's
/// car (DDAY_PLAYER_CAR, the reported car), with its wide
/// body's LEFT_EXHAUST / RIGHT_EXHAUST markers. Runs with NFSU2_ROOT.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rachels_flames_burst_at_exhaust_positions_with_template_axes() {
    let root = testkit::game_root();
    let world = World::load(&root, "L4RA").unwrap();
    let globalb = nfsu2::game_fs::read(&root, "GLOBAL/GLOBALB.BUN").unwrap();
    let preset = nfsu2::car::parse_presets(&globalb).into_iter().find(|p| p.name == career::DDAY_PLAYER_CAR).unwrap();
    let car = Car::load_with(&root, &preset.car, &preset.installed()).unwrap();
    assert_eq!(car.exhausts.len(), 2, "the wide body's two exhaust markers");
    let group = nfsu2::formats::hash::bin_hash("CARFX_NITRO");
    let templates = world.emitters.group_templates(group);
    assert_eq!(templates.len(), 1);
    let (d, own) = templates[0];
    assert_eq!(world.emitters.descriptions[d].marker_group, 10, "render +0x548 + 8·10 = +0x598, the exhaust list");
    for gas in [1.0, 0.0] {
        let mut state = CarState::new(Vec3::new(0.0, 0.0, 1.0), 0.7);
        state.velocity = state.forward() * 20.0;
        state.publish_forward_speed();
        let mut drivetrain = nfsu2::handling::Drivetrain::new(&car.car_type.handling);
        drivetrain.nitrous = 1;
        state.drivetrain = Some(drivetrain);
        state.set_gas(gas);
        let mut fx = Effects::default();
        fx.step(&world, state.position - state.forward() * 6.0, &[Some((&car, &state))], &[], 1.0 / 60.0, &crate::sim::TickSettings::take(&[0; 22]));
        if gas == 0.0 {
            assert!(fx.flames.is_empty(), "off the gas the shot purges (0x5ECDC0)");
            continue;
        }
        let body = glam::Mat4::from_rotation_translation(state.orientation, state.position);
        for (mi, m) in car.exhausts.iter().enumerate() {
            let e = &fx.flames[&(0, d, mi)];
            let want = body
                * glam::Mat4::from_cols(
                    own.x_axis.truncate().extend(0.0),
                    own.y_axis.truncate().extend(0.0),
                    own.z_axis.truncate().extend(0.0),
                    m.w_axis,
                );
            assert!(e.transform.abs_diff_eq(want, 1e-5), "marker {mi}: {:?} vs {want:?}", e.transform);
        }
        assert!(
            fx.system.particles.iter().filter(|p| p.key == FLAME_KEY).count() >= 2 * 8 - 2,
            "about rate 8 a marker"
        );
    }
}

/// Diagnostic beside the original (Wine, Quick Race drag, first track,
/// defaults: traffic Minimum, 3 opponents; the player standing on the
/// grid; racer status words read from race status [0x890118] +0xBD0 →
/// racer +0x10): the rivals' finish statuses after 90 s. `QR_SEEDS`.
#[test]
#[ignore = "requires original assets; diagnostic"]
fn quick_drag_standing_rival_statuses() {
    let seeds: Vec<u32> = std::env::var("QR_SEEDS")
        .ok()
        .map_or((0..12).collect(), |s| s.split(',').filter_map(|x| x.parse().ok()).collect());
    let (mut runs, mut totaled, mut finished) = (0, 0, 0);
    for seed in seeds {
        let mut game = Game::new().unwrap();
        game.rng = nfsu2::opponents::GameRng(seed);
        game.quick.tracks = game.quick_track_select(screens::QrMode::Drag, 0.0);
        let event = game.quick_event(screens::QrMode::Drag);
        game.quick.car = Some("PEUGOT".into());
        game.start_race(event, true, false).unwrap();
        game.key(KeyCode::Enter, &mut || {}).unwrap();
        for _ in 0..60 * 100 {
            game.step_for(1.0 / 60.0).unwrap();
            if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 90.0) {
                break;
            }
        }
        let Mode::Race(r) = &game.mode else { panic!("race over") };
        let st: Vec<_> = r
            .racers.rivals
            .iter()
            .map(|rv| (rv.progress.finished.map(|t| (t * 10.0).round() / 10.0), rv.progress.dropped))
            .collect();
        runs += 1;
        totaled += st.iter().filter(|s| s.1 == Some(9)).count();
        finished += st.iter().filter(|s| s.0.is_some() && s.1.is_none()).count();
        eprintln!(
            "QRDRAG seed={seed} track={} traffic={:?} rivals={st:?}",
            game.game_data.career.events[r.event].track,
            r.racers.traffic.as_ref().map(|t| t.cars.len())
        );
    }
    eprintln!("QRDRAG runs={runs} rivals finished={finished} totaled={totaled}");
}

/// The nitro tests' order for the drive's effects (the step and the
/// window's draw), set again around each update: a drive the update
/// rebuilds starts on `EffectsOrder::NATIVE`.
fn set_effects_order(game: &mut Game, order: EffectsOrder) {
    if let Some(d) = game.sim.drive.as_mut() {
        d.effects.order = order;
    }
}

/// The player's flames went missing at
/// higher frame rates. The window loop (`Game::advance` + `draw_between`,
/// the 1/60 s world cadence with interpolated drawing) at 60 / 120 / 144 /
/// 240 / 360 Hz frame intervals: every drawn frame of an active, non-purge
/// shot must show flame pixels. Needs a GPU. `NITRO_HZ`, `NITRO_CARS`
/// (PEUGOT, DDAY = Rachel's 350Z after the intro).
#[test]
#[ignore = "requires original assets, a GPU and isolated XDG homes"]
fn player_nitro_flames_visible_at_display_rates() {
    let rates: Vec<f32> = std::env::var("NITRO_HZ")
        .ok()
        .map_or(vec![60.0, 120.0, 144.0, 240.0, 360.0], |s| s.split(',').filter_map(|x| x.parse().ok()).collect());
    let cars: Vec<String> =
        std::env::var("NITRO_CARS").map_or(vec!["PEUGOT".into()], |s| s.split(',').map(str::to_owned).collect());
    let (width, height) = env_size("NITRO_SIZE").unwrap_or((1920, 1080));
    let mut misses = Vec::new();
    // NITRO_OLD=1: the behaviour before the fixes (flames emitted before
    // the particle update, carried particles not blended), to compare.
    let old = std::env::var_os("NITRO_OLD").is_some();
    let order = if old { EffectsOrder::BEFORE_FIXES } else { EffectsOrder::NATIVE };
    for car in &cars {
        for &hz in &rates {
            let mut game = Game::new().unwrap();
            offscreen_window(&mut game, width, height);
            game.pacing.clock = Some(0.0);
            if car == "DDAY" {
                new_career_dday(&mut game, true);
            } else {
                let mut profile = Profile::new("NITRO_RATE_TEST");
                profile.skip_intro(&game.game_data.career, &game.game_data.root, car, &mut game.rng);
                profile.perf.extend(
                    game.game_data.career
                        .packages
                        .iter()
                        .filter(|p| p.category == 2 && p.level == 1)
                        .flat_map(|p| p.parts.iter().map(|x| x.id)),
                );
                game.profile = Some(profile);
                game.explore(None).unwrap();
            }
            game.settings.custom[10] = 1;
            game.platform.keys.insert(KeyCode::KeyW);
            let (mut drawn, mut active, mut invisible, mut pixels) = (0usize, 0usize, 0usize, Vec::new());
            let mut by_alpha: Vec<(f32, usize)> = Vec::new();
            let mut t = 0.0f32;
            while t < 5.0 {
                if t >= 1.5 {
                    game.platform.keys.insert(KeyCode::AltLeft);
                }
                let interval = 1.0 / hz;
                game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(interval));
                set_effects_order(&mut game, order);
                let alpha = game.advance(interval).unwrap();
                set_effects_order(&mut game, order);
                t += interval;
                drawn += 1;
                let d = game.sim.drive.as_ref().unwrap();
                let stage = d.state.drivetrain.as_ref().map_or(0, |x| x.nitrous);
                if stage == 0
                    || nfsu2::audio::carfx::purging(stage, d.state.speed(), d.state.gas())
                    || !matches!(game.mode, Mode::Explore)
                {
                    game.draw_between(alpha).unwrap();
                    continue;
                }
                active += 1;
                let n = flame_pixels_at(&mut game, width, height, alpha);
                pixels.push(n);
                by_alpha.push((alpha, n));
                if n == 0 {
                    invisible += 1;
                }
            }
            pixels.sort_unstable();
            let low_alpha: Vec<_> = by_alpha.iter().filter(|x| x.0 < 0.5).map(|x| x.1).collect();
            let high_alpha: Vec<_> = by_alpha.iter().filter(|x| x.0 >= 0.5).map(|x| x.1).collect();
            let mean = |v: &[usize]| if v.is_empty() { 0.0 } else { v.iter().sum::<usize>() as f32 / v.len() as f32 };
            eprintln!(
                "RATE car={car} hz={hz} drawn={drawn} active={active} invisible={invisible} min={:?} median={:?} mean_alpha_lt_half={:.0} mean_alpha_ge_half={:.0}",
                pixels.first(),
                pixels.get(pixels.len() / 2),
                mean(&low_alpha),
                mean(&high_alpha)
            );
            if active == 0 || invisible > 0 {
                misses.push(format!("{car} {hz} Hz: {invisible} of {active} drawn active frames without flame pixels"));
            }
        }
    }
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The player's flames went missing at
/// high speed. The first career drag on the finished-career save's Lancer (NOS fitted) on
/// the player's keys (W, Left Shift near the red line, Left Alt from 1.5 s
/// after the green), drawn through the window loop at `NITRO_LOOP` (60 /
/// 144 Hz frame intervals through `advance` + `draw_between`, or "step":
/// fixed 1/60 s `Game::step` + plain draws): per drawn active frame the
/// flames' visible pixels, by speed band. `NITRO_OLD=1`: before the fixes.
/// `NITRO_LOOP` also takes 30 / 120 / 200 and
/// "jitter" (2–34 ms frames), `NITRO_SETTINGS=<options.txt>` a play setup's
/// World Detail / Enhancements lines.
#[test]
#[ignore = "requires original assets, a GPU and isolated XDG homes"]
fn player_nitro_flames_visible_at_speed() {
    let old = std::env::var_os("NITRO_OLD").is_some();
    let order = if old { EffectsOrder::BEFORE_FIXES } else { EffectsOrder::NATIVE };
    let (width, height) = env_size("NITRO_SIZE").unwrap_or((1920, 1080));
    let loops: Vec<String> = std::env::var("NITRO_LOOP")
        .map_or(vec!["step".into(), "60".into(), "144".into()], |s| s.split(',').map(str::to_owned).collect());
    let mut misses = Vec::new();
    for lp in &loops {
        let mut game = Game::new().unwrap();
        offscreen_window(&mut game, width, height);
        load_finished_career(&mut game).unwrap();
        game.rng = nfsu2::opponents::GameRng(4);
        let event = game
            .game_data.career
            .stage_events(&game.game_data.root, 1)
            .into_iter()
            .find(|e| e.kind == career::EventKind::Drag)
            .unwrap()
            .index;
        game.start_race(event, false, false).unwrap();
        game.key(KeyCode::Enter, &mut || {}).unwrap();
        game.settings.custom[10] = 1;
        // NITRO_SETTINGS=<options.txt>: its World Detail custom values and
        // Enhancements lines for the run (a play setup, e.g. a
        // saved options.txt); default Original.
        if let Some(text) = std::env::var_os("NITRO_SETTINGS").and_then(|f| std::fs::read_to_string(f).ok()) {
            for line in text.lines() {
                let (k, v) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
                if let Some(n) = k.strip_prefix("custom.").and_then(|n| n.parse::<usize>().ok())
                    && let (Some(slot), Ok(v)) = (game.settings.custom.get_mut(n), v.parse())
                {
                    *slot = v;
                } else if k.starts_with("enh.") {
                    game.settings.enh.parse(k, v);
                }
            }
            game.settings.apply_enhancements();
        }
        game.pacing.clock = Some(0.0);
        game.platform.keys.insert(KeyCode::KeyW);
        let redline = game.sim.drive.as_ref().unwrap().car.car_type.handling.engine.redline_rpm;
        // "jitter": frame intervals drawn from 2–34 ms (a fixed LCG).
        let jitter = lp == "jitter";
        let mut lcg = 0x1234_5678u32;
        let base = if lp == "step" || jitter { 1.0 / 60.0 } else { 1.0 / lp.parse::<f32>().unwrap() };
        let mut interval = base;
        let mut bands: std::collections::BTreeMap<u32, (usize, usize, usize)> = Default::default();
        let mut shift_wait = 0.0f32;
        let mut elapsed = 0.0f32;
        while elapsed < 40.0 {
            if jitter {
                lcg = lcg.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                interval = 0.002 + 0.032 * (lcg >> 8) as f32 / (1u32 << 24) as f32;
            }
            elapsed += interval;
            let (clock, done) = match &game.mode {
                Mode::Race(r) => (r.timing.clock, r.racers.me.finished.is_some()),
                _ => break,
            };
            if done {
                break;
            }
            if clock >= 1.5 {
                game.platform.keys.insert(KeyCode::AltLeft);
            }
            shift_wait -= interval;
            if clock >= 0.0 && shift_wait <= 0.0 && game.sim.drive.as_ref().unwrap().state.rpm() >= 0.95 * redline {
                game.key(KeyCode::ShiftLeft, &mut || {}).unwrap();
                shift_wait = 0.5;
            }
            game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(interval));
            set_effects_order(&mut game, order);
            let alpha = if lp == "step" {
                game.step_for(interval).unwrap();
                1.0
            } else {
                game.advance(interval).unwrap()
            };
            set_effects_order(&mut game, order);
            let d = game.sim.drive.as_ref().unwrap();
            let stage = d.state.drivetrain.as_ref().map_or(0, |x| x.nitrous);
            let kmh = d.state.speed() * 3.6;
            if stage == 0
                || nfsu2::audio::carfx::purging(stage, d.state.speed(), d.state.gas())
                || game.scenes.shop.is_some()
            {
                game.draw_between(alpha).unwrap();
                continue;
            }
            let n = flame_pixels_at(&mut game, width, height, alpha);
            let b = bands.entry((kmh / 50.0) as u32 * 50).or_default();
            b.0 += 1;
            b.1 += usize::from(n == 0);
            b.2 += n;
        }
        for (band, (frames, invisible, pixels)) in &bands {
            eprintln!(
                "SPEED loop={lp} old={old} band={band}-{} km/h frames={frames} invisible={invisible} mean_pixels={}",
                band + 50,
                pixels / (*frames).max(1)
            );
            if *invisible > 0 {
                misses.push(format!("{lp} {band} km/h: {invisible} of {frames}"));
            }
        }
        assert!(!bands.is_empty(), "{lp}: no active nitrous frame");
    }
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The flame at speed (0x609CF0: the particle update 0x492E10 before the
/// cars' updates 0x5F5740 that emit): a burst at 50 m/s sits at the
/// exhaust, not a frame's travel (0.83 m) ahead of it inside the body. A
/// scene's CARFX_NITRO intensity 2 (SEQ_CAR_NITRO) bursts twice the
/// particles of a shot. Rachel's 350Z; runs with NFSU2_ROOT.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn flames_stay_at_the_exhaust_at_speed_and_scene_flames_fire() {
    let root = testkit::game_root();
    let world = World::load(&root, "L4RA").unwrap();
    let globalb = nfsu2::game_fs::read(&root, "GLOBAL/GLOBALB.BUN").unwrap();
    let preset = nfsu2::car::parse_presets(&globalb).into_iter().find(|p| p.name == career::DDAY_PLAYER_CAR).unwrap();
    let car = Car::load_with(&root, &preset.car, &preset.installed()).unwrap();
    let mut state = CarState::new(Vec3::new(0.0, 0.0, 1.0), 0.3);
    state.velocity = state.forward() * 50.0;
    state.publish_forward_speed();
    let mut drivetrain = nfsu2::handling::Drivetrain::new(&car.car_type.handling);
    drivetrain.nitrous = 1;
    state.drivetrain = Some(drivetrain);
    state.set_gas(1.0);
    let body = glam::Mat4::from_rotation_translation(state.orientation, state.position);
    let exhausts: Vec<Vec3> = car.exhausts.iter().map(|m| body.transform_point3(m.w_axis.truncate())).collect();
    let ahead = |fx: &Effects| {
        fx.system
            .particles
            .iter()
            .filter(|p| p.key == FLAME_KEY)
            .map(|p| exhausts.iter().map(|e| (p.position - *e).dot(state.forward())).fold(f32::INFINITY, f32::min))
            .fold(f32::NEG_INFINITY, f32::max)
    };
    let mut fx = Effects::default();
    fx.step(&world, state.position - state.forward() * 6.0, &[Some((&car, &state))], &[], 1.0 / 60.0, &crate::sim::TickSettings::take(&[0; 22]));
    let shot = fx.system.particles.iter().filter(|p| p.key == FLAME_KEY).count();
    assert!(shot >= 2 * 8 - 2, "a burst at each exhaust: {shot}");
    assert!(ahead(&fx) < 0.1, "newest flame {} m ahead of the exhaust", ahead(&fx));
    // The order before the fix, for contrast: a frame's travel ahead.
    let mut old = Effects::default();
    old.step_in_order(
        &world,
        state.position - state.forward() * 6.0,
        &[Some((&car, &state))],
        &[],
        EffectsOrder { emit_before_update: true, ..EffectsOrder::NATIVE },
        1.0 / 60.0,
        &crate::sim::TickSettings::take(&[0; 22]),
    );
    assert!(ahead(&old) > 0.6, "emit-then-move put it {} m ahead", ahead(&old));
    // A standing scene car with SEQ_CAR_NITRO's intensity 2 and no shot.
    let mut still = CarState::new(Vec3::new(0.0, 0.0, 1.0), 0.3);
    still.publish_forward_speed();
    let mut scene = Effects::default();
    scene.step(&world, still.position - still.forward() * 6.0, &[Some((&car, &still))], &[2.0], 1.0 / 30.0, &crate::sim::TickSettings::take(&[0; 22]));
    let n = scene.system.particles.iter().filter(|p| p.key == FLAME_KEY).count();
    assert!(n >= 2 * 16 - 2, "intensity 2 bursts 16 a marker: {n}");
    let mut none = Effects::default();
    none.step(&world, still.position - still.forward() * 6.0, &[Some((&car, &still))], &[0.0], 1.0 / 30.0, &crate::sim::TickSettings::take(&[0; 22]));
    assert!(none.system.particles.iter().all(|p| p.key != FLAME_KEY));
    // The shipped scenes carry the records (IntroNis25: car 4 at 0 → 2..5).
    let track = nfsu2::game_fs::read_to_string(&root, "NIS/Scene_IntroNis25_EvtTrk.txt").unwrap();
    let ev = nfsu2::shop_scene::effects::parse(&track);
    assert_eq!(nfsu2::shop_scene::effects::intensity(&ev, 4, nfsu2::shop_scene::effects::NITRO, 3.0), 2.0);
    assert_eq!(nfsu2::shop_scene::effects::intensity(&ev, 4, nfsu2::shop_scene::effects::NITRO, 5.5), 0.0);
}

/// The opponents' flames on the start line. A
/// race start scene with SEQ_CAR_NITRO (`NFSU2_INTRO_NIS`, default 25:
/// car 4 at frames 0, 4, 18, 30, 68, 100, car 2 at 87) on the window path:
/// per drawn update the scene cars' flame particles and their visible
/// pixels. Needs a GPU.
#[test]
#[ignore = "requires original assets, a GPU, isolated XDG homes and NFSU2_INTRO_NIS"]
fn start_scene_flames_visible() {
    let (width, height) = env_size("NITRO_SIZE").unwrap_or((1920, 1080));
    let mut game = Game::new().unwrap();
    offscreen_window(&mut game, width, height);
    game.settings.custom[10] = 1;
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let event = game.quick_event(mode);
    game.quick.car = Some("PEUGOT".into());
    game.start_race(event, true, false).unwrap();
    let scene = game.scenes.shop.as_ref().map(|s| s.scene.kind.name()).unwrap_or_default();
    let (mut with_particles, mut visible, mut frames) = (0, 0, 0);
    let mut log = Vec::new();
    while game.scenes.shop.is_some() && frames < 400 {
        game.step_for(1.0 / 30.0).unwrap();
        frames += 1;
        let d = game.sim.drive.as_ref().unwrap();
        let n = d.effects.system.particles.iter().filter(|p| p.key >= FLAME_KEY && p.key < FLAME_KEY + 8).count();
        if n == 0 {
            game.draw().unwrap();
            continue;
        }
        with_particles += 1;
        // The flame pixels: the frame with and without every car's flame.
        let grab = |game: &mut Game| {
            let w = game.platform.window.as_mut().unwrap();
            w.settle();
            let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), width, height)
        };
        let motion = game.sim.drive.as_ref().unwrap().motion;
        game.draw().unwrap();
        let a = grab(&mut game);
        let d = game.sim.drive.as_mut().unwrap();
        d.motion = motion;
        let kept = d.effects.system.particles.clone();
        d.effects.system.particles.retain(|p| !(p.key >= FLAME_KEY && p.key < FLAME_KEY + 8));
        game.draw().unwrap();
        let b = grab(&mut game);
        game.sim.drive.as_mut().unwrap().effects.system.particles = kept;
        let px = a
            .chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 8))
            .count();
        visible += usize::from(px > 0);
        log.push((game.scenes.shop.as_ref().map_or(-1.0, |s| s.frame), n, px));
    }
    eprintln!(
        "SCENE {scene}: {frames} updates, {with_particles} with flame particles, {visible} with visible flame pixels; (frame, particles, pixels) {log:?}"
    );
    assert!(with_particles > 0, "{scene}: no scene flame fired");
    assert!(visible > 0, "{scene}: no flame visible");
}

/// Outrun rivals did not switch into race mode and were much easier than
/// in the original. Native: while the manager tries to engage (state 3) every
/// encounter car runs goal 9 (0x42A860 → 0x428630) whose slot +0x4C
/// (0x40DC00, table 0x81E378) caps the driver's speed — 40 mph beyond
/// 100 m of a standing player, 60 mph within (observed under Wine: 17.881
/// / 26.822 m/s); at the challenge (0x607190) the challenged car gets goal
/// 6 and race state 7's target (0x41C5E0: the route's speed × 0.84 while
/// leading, slot +0x4C = 0x40DDD0 = 1e9, no cap). So the rival visibly
/// changes gear from a cruise to race pace. Before the fix
/// the clone had no cap: the car cruised at 0.75 × its race target and the
/// challenge raised that by 0.84 / 0.75 = 1.12 only — the issue. Reverting
/// the cap (`goal9_think`, `f.state == 3`) fails the ratio below.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn outrun_rival_switches_from_pursuit_cruise_to_race_pace() {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("OutrunRaceMode");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let e = game.sim.encounters.as_mut().unwrap();
    e.ready();
    assert!(!e.cars.is_empty(), "an encounter racer");
    let routes = e.routes.clone();
    let d = game.sim.drive.as_ref().unwrap();
    assert!(d.state.velocity.length() < 0.5, "the player stands");
    let player = d.state.position.truncate();
    // A straight racing-line point (flag 0x800) over 150 m from the player.
    let (position, direction) = routes
        .file
        .records
        .iter()
        .enumerate()
        .flat_map(|(i, record)| {
            record.points.iter().enumerate().filter(|(_, p)| p.flags & 0x800 != 0).map(move |(cp, _)| (i, cp))
        })
        .map(|(rec, cp)| (routes.file.records[rec].points[cp].position, routes.file.direction(rec, cp, false)))
        .find(|(p, _)| p.truncate().distance(player) > 150.0)
        .unwrap();
    let r = &mut e.cars[0];
    r.active = true;
    r.state = CarState::new(position, direction.y.atan2(direction.x));
    r.state.velocity = direction.extend(0.0) * 20.0;
    r.state.publish_forward_speed();
    r.route = nfsu2::ai::route::RacerRoute::start(&routes, position, direction);
    r.controller = Default::default();
    let ai = game.sim.ai.as_ref().unwrap();
    let node = e.player_node(&d.state);
    e.state = 3;
    for _ in 0..6 {
        e.think(ai, 1.0 / 60.0, d, node, &[], None);
    }
    let cruise = e.cars[0].driver_plan.1;
    let distance = e.cars[0].state.position.truncate().distance(player);
    let cap = nfsu2::outrun::pursuit_cap(distance, 0.0);
    assert!(cruise <= cap + 1e-3, "goal 9 cruise {cruise} m/s above the 0x40DC00 cap {cap} at {distance:.0} m");
    // The challenge: goal 6 for the challenged car, which leads.
    e.state = 4;
    e.outrun = Some(Outrun { opponent: 0, clock: 0.0, start_tick: 4000, player: 10.0, rival: 0.0, player_leads: false });
    for _ in 0..6 {
        e.think(ai, 1.0 / 60.0, d, node, &[], None);
    }
    let race = e.cars[0].driver_plan.1;
    eprintln!("outrun race mode: cruise {cruise:.3} m/s (cap {cap:.3} at {distance:.0} m), race {race:.3} m/s");
    assert!(race >= 1.5 * cruise, "the challenge must switch to race pace: cruise {cruise:.2} → race {race:.2} m/s");
}

/// A career race on the free-roam drive (the same region and car: the drive
/// is kept) hides the marker effects: the original's per-frame gate
/// (0x605790: race type 0x89E7B0 non-zero → 0x605C37 turns all 12 slots
/// off) takes the event, game-tip (SMS) and cash markers off together.
/// Back in free roam the marker update places them again.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_on_the_free_roam_drive_hides_the_markers() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("RACE_MARKERS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let tip = MARKER_EFFECTS.iter().position(|e| e.2 == "XEE_GAME_TIP").unwrap();
    let t = {
        let p = game.profile.as_ref().unwrap();
        *game.sim.route_triggers.tips().find(|t| p.tip_pending(&game.game_data.career, t.name)).expect("an unsent engage tip")
    };
    let near = |g: &mut Game| {
        let d = g.sim.drive.as_mut().unwrap();
        d.state.position = (t.position.truncate() + glam::Vec2::new(30.0, 0.0)).extend(d.state.position.z);
        g.marker_effects();
    };
    let lit = |g: &Game| g.sim.drive.as_ref().unwrap().effects.markers.iter().filter(|m| m.is_some()).count();
    near(&mut game);
    assert!(game.sim.drive.as_ref().unwrap().effects.markers[tip].is_some(), "free roam: the tip's marker");
    let event = game
        .game_data
        .career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| {
            e.kind == career::EventKind::Sprint
                && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
        })
        .map(|e| e.index)
        .expect("a city sprint");
    game.start_race(event, false, false).unwrap();
    assert!(matches!(game.mode, Mode::Race(_)), "racing");
    assert!(lit(&game) > 0, "the race kept the free-roam drive (the path this covers)");
    game.particle_effects(1.0 / 60.0, &crate::sim::TickSettings::take(&game.settings.custom));
    assert_eq!(lit(&game), 0, "no marker effect in the race's first world update");
    game.particle_effects(1.0 / 60.0, &crate::sim::TickSettings::take(&game.settings.custom));
    assert_eq!(lit(&game), 0, "nor later");
    game.mode = Mode::Explore;
    near(&mut game);
    assert!(game.sim.drive.as_ref().unwrap().effects.markers[tip].is_some(), "free roam again: the tip's marker back");
}

/// The first intro race (stage 0's first circuit in Rachel's 350Z,
/// DDAY_PLAYER_CAR), driven as the window does, with the nitrous key held
/// from the start scene's second 0.5 on: as in the original (observed
/// under Wine), nothing fires under the scene's camera and the key held
/// on into the countdown starts nothing either; let go and pressed again
/// in the countdown, a shot starts (at a standstill the purge). The run
/// without the key keeps its tank too. Needs a GPU.
#[test]
#[ignore = "requires original assets, a GPU and isolated XDG homes"]
fn race_start_nitro_waits_for_the_countdown() {
    for nitro in [false, true] {
        let mut game = Game::new().unwrap();
        super::test_isolation::exclusive();
        offscreen_window(&mut game, 1280, 960);
        game.pacing.clock = Some(0.0);
        new_career_dday(&mut game, true);
        let e = game
            .game_data
            .career
            .events
            .iter()
            .position(|e| e.unlock == dday::STAGE0_RACES[0])
            .expect("stage 0's first race");
        game.start_race(e, false, false).unwrap();
        let dt = 1.0 / 60.0;
        let (mut full, mut after_cut, mut pressed_again, mut fired) = (None, 0, None, false);
        for f in 0..60 * 40 {
            let tank = |game: &Game| game.sim.drive.as_ref().unwrap().state.nitrous;
            let cut = game.scenes.shop.as_ref().is_none_or(|run| run.frame >= run.scene.marks[1] as f32);
            if cut {
                after_cut += 1;
            }
            // Held from frame 30; let go 30 frames into the countdown and
            // pressed again 10 frames later.
            let down = nitro && f >= 30 && !(30..40).contains(&after_cut);
            if down {
                game.platform.keys.insert(KeyCode::AltLeft);
            } else {
                game.platform.keys.remove(&KeyCode::AltLeft);
            }
            if after_cut == 40 && nitro {
                pressed_again = Some(f);
            }
            game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(dt));
            let alpha = game.advance(dt).unwrap();
            game.draw_between(alpha).unwrap();
            let Mode::Race(r) = &game.mode else { panic!("left the race") };
            let t = tank(&game);
            if full.is_none() {
                full = t.map(|t| t.remaining());
            }
            let running = t.is_some_and(|t| t.running.is_some());
            if pressed_again.is_none() {
                assert!(!running, "nitro {nitro}: a shot at frame {f} (scene cut {cut}, {after_cut} frames past it)");
            } else {
                fired |= running;
            }
            if r.timing.clock >= 0.0 {
                break;
            }
        }
        game.platform.keys.clear();
        assert!(full.is_some_and(|t| t > 0.0), "Rachel's car has a tank");
        assert!(after_cut > 40, "the countdown ran ({after_cut} frames)");
        assert_eq!(fired, nitro, "a press in the countdown starts a shot");
    }
}

/// 0x606CC0 → 0x5EA540 (verified): the local player's finish ends a
/// running nitrous shot (0x5EA63E jumps to 0x592510 on the car's tank)
/// before the driving keys are removed (0x605C80), so a Nitrous key still
/// held over the line does not burn the rest of the tank. The first career
/// drag on the finished-career save's car, the key held from the green; the
/// race ends by a drop-out (status 9), which takes the same finish path.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn finishing_on_nitrous_ends_the_shot() {
    testkit::game();
    let mut game = Game::new().unwrap();
    load_finished_career(&mut game).unwrap();
    game.rng = nfsu2::opponents::GameRng(4);
    let event =
        game.game_data.career.stage_events(&game.game_data.root, 1).into_iter().find(|e| e.kind == career::EventKind::Drag).unwrap().index;
    game.start_race(event, false, false).unwrap();
    game.platform.keys.insert(KeyCode::Enter);
    game.key(KeyCode::Enter, &mut || {}).unwrap();
    game.platform.keys.remove(&KeyCode::Enter);
    game.platform.keys.insert(KeyCode::KeyW);
    let dt = 1.0 / 60.0;
    let mut at_finish = None;
    for _ in 0..60 * 30 {
        game.step_for(dt).unwrap();
        let Mode::Race(r) = &game.mode else { panic!("the race ended early") };
        if r.timing.clock < 0.5 {
            continue;
        }
        game.platform.keys.insert(KeyCode::AltLeft);
        let d = game.sim.drive.as_mut().unwrap();
        let tank = d.state.nitrous.expect("the finished-career car has nitrous");
        match at_finish {
            None if r.racers.me.finished.is_some() => {
                assert!(tank.running.is_none(), "the finish ended the shot: {tank:?}");
                assert!(tank.remaining() > 0.0, "nitrous left in the tank: {tank:?}");
                at_finish = Some((tank.remaining(), r.timing.clock));
            }
            None if tank.running.is_some_and(|t| t > 0.25) => d.state.totaled = true,
            None => {}
            Some((left, clock)) => {
                assert_eq!(tank.remaining(), left, "no shot after the finish: {tank:?}");
                if r.timing.clock > clock + 1.0 {
                    return;
                }
            }
        }
    }
    panic!("the run never got past its finish: {at_finish:?}");
}

/// A collected cash pickup's or engage tip's world icon goes at once, as in
/// the original (seen under Wine at 60 fps: the XEE_BANK bills and their
/// ground ring are drawn on one frame and gone on the next). Turning a
/// marker effect off (0x602230, from 0x605790 when its slot is empty or
/// 0x6054C0 when its spot moved) frees each emitter's particles with it
/// (0x494690: the particle groups whose owner +0x0C is the emitter), so
/// none of the 1 s icons or 2.5 s rings already spawned is left to fade.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn collected_world_icons_go_at_once() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("WORLD_ICONS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let home_z = game.sim.drive.as_ref().unwrap().state.position.z;
    let mid = |t: &career::Trigger| {
        if t.points > 0 {
            t.polygon[..t.points as usize].iter().copied().sum::<glam::Vec2>() / t.points as f32
        } else {
            (t.min + t.max) * 0.5
        }
    };
    let go = |g: &mut Game, at: glam::Vec2, secs: f32| {
        for _ in 0..(secs * 60.0).round() as usize {
            g.sim.drive.as_mut().unwrap().state.position = at.extend(home_z);
            g.advance_for_each(1.0 / 60.0, |g, dt| {
                g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
                Ok(())
            })
            .unwrap();
        }
    };
    // The live particles of one marker effect's group.
    let alive = |g: &Game, name: &str| {
        let d = g.sim.drive.as_ref().unwrap();
        let group = d.world.emitters.group(nfsu2::formats::hash::bin_hash(name));
        d.effects.system.particles.iter().filter(|p| group.contains(&(p.description as usize))).count()
    };
    let (t, b) = {
        let p = game.profile.as_ref().unwrap();
        game.sim.route_triggers
            .banks()
            .find_map(|t| {
                game.game_data.career.banks.iter().find(|b| b.trigger == t.name).filter(|b| p.bank_open(&game.game_data.career, b)).map(|b| (*t, *b))
            })
            .expect("an open cash pickup")
    };
    go(&mut game, mid(&t) + glam::Vec2::new(20.0, 0.0), 1.5);
    let before = alive(&game, "XEE_BANK");
    assert!(before > 0, "the pickup's icon and rings are up 20 m away");
    let cash = game.profile.as_ref().unwrap().cash;
    go(&mut game, mid(&t), 1.0 / 60.0);
    assert_eq!(game.profile.as_ref().unwrap().cash, cash + b.cash as u32, "the pickup paid");
    assert_eq!(alive(&game, "XEE_BANK"), 0, "its {before} particles went with the effect in the same update");
    // An engage tip: its icon goes when the world runs again after its SMS.
    let t = {
        let p = game.profile.as_ref().unwrap();
        *game.sim.route_triggers.tips().find(|t| p.tip_pending(&game.game_data.career, t.name)).expect("an unsent engage tip")
    };
    go(&mut game, mid(&t) + glam::Vec2::new(20.0, 0.0), 1.5);
    assert!(alive(&game, "XEE_GAME_TIP") > 0, "the tip's icon is up 20 m away");
    go(&mut game, mid(&t), 1.0 / 60.0);
    assert!(!game.profile.as_ref().unwrap().tip_pending(&game.game_data.career, t.name), "the tip's SMS came");
    game.mode = Mode::Explore;
    go(&mut game, mid(&t), 1.0 / 60.0);
    assert_eq!(alive(&game, "XEE_GAME_TIP"), 0, "the tip's particles went with the effect");
}

/// The player's car under a race's start scene camera is drawn whatever
/// the player's own view (as in the original, observed under Wine: in the
/// bumper view the car is in every shot of the scene; in the hood view,
/// Look Back tapped through the scene, it never blinks). The first intro
/// race (stage 0's first circuit in Rachel's 350Z), driven as the window
/// does at 30, 60, 120, 144 and 240 fps and at an uneven uncapped rate: in
/// the bumper view with no key, and in the hood view with Look Back and
/// the nitrous key tapped from the scene's second 0.5 on. While the scene's
/// camera is the view the car is never left out (`player_car_hidden`, the
/// draw's test); after the cut the view's own rule applies again (the
/// bumper view leaves the car out, as before). Needs a GPU.
#[test]
#[ignore = "requires original assets, a GPU and isolated XDG homes"]
fn race_start_scene_draws_the_player_car_in_every_view() {
    let (w, h) = (320u32, 240u32);
    let uncapped = [1.0 / 300.0, 1.0 / 90.0, 1.0 / 165.0, 1.0 / 240.0, 1.0 / 120.0];
    let rates: [(&str, &[f32]); 6] = [
        ("30", &[1.0 / 30.0]),
        ("60", &[1.0 / 60.0]),
        ("120", &[1.0 / 120.0]),
        ("144", &[1.0 / 144.0]),
        ("240", &[1.0 / 240.0]),
        ("uncapped", &uncapped),
    ];
    // (camera setting: 0 bumper, 1 hood; keys tapped)
    for (camera, keys) in [(0u8, &[][..]), (1, &[KeyCode::KeyB, KeyCode::AltLeft][..])] {
        for (rate, dts) in rates {
            let mut game = Game::new().unwrap();
            super::test_isolation::exclusive();
            offscreen_window(&mut game, w, h);
            game.settings.players[0].camera = camera;
            game.pacing.clock = Some(0.0);
            new_career_dday(&mut game, true);
            let e = game
                .game_data
                .career
                .events
                .iter()
                .position(|e| e.unlock == dday::STAGE0_RACES[0])
                .expect("stage 0's first race");
            game.start_race(e, false, false).unwrap();
            let (mut scene_frames, mut checked, mut after_cut) = (0, 0, 0);
            let mut hidden_after_cut = false;
            let mut t = 0.0f32;
            for f in 0..60 * 60 {
                let dt = dts[f % dts.len()];
                t += dt;
                // From the scene's second 0.5 on, each key down 0.25 s of
                // every 0.5 s (the keys in turn).
                let phase = (t / 0.25) as usize;
                for (i, k) in keys.iter().enumerate() {
                    if t >= 0.5 && phase % 2 == 0 && (phase / 2) % keys.len() == i {
                        game.platform.keys.insert(*k);
                    } else {
                        game.platform.keys.remove(k);
                    }
                }
                game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(dt));
                let alpha = game.advance(dt).unwrap();
                game.draw_between(alpha).unwrap();
                let Mode::Race(r) = &game.mode else { panic!("left the race") };
                let clock = r.timing.clock;
                let in_scene = game.scenes.shop.as_ref().is_some_and(|run| run.camera_holds_view());
                // Past the scene's first half second (its own shots).
                let shot = game.scenes.shop.as_ref().is_some_and(|run| run.frame >= 15.0);
                let hidden = player_car_hidden(&game.mode, game.sim.drive.as_ref().unwrap(), game.scenes.shop.as_ref());
                if in_scene {
                    scene_frames += 1;
                    assert!(
                        !hidden,
                        "camera {camera} at {rate} fps: the player's car left out under the scene's camera (frame {f})"
                    );
                    if shot {
                        checked += 1;
                    }
                } else if clock < 0.0 {
                    after_cut += 1;
                    hidden_after_cut |= hidden;
                }
                if clock >= 0.0 {
                    break;
                }
            }
            game.platform.keys.clear();
            eprintln!(
                "camera {camera} at {rate}: {scene_frames} scene frames ({checked} past its first half second), {after_cut} after the cut, left out there {hidden_after_cut}"
            );
            assert!(checked >= 10, "camera {camera} at {rate} fps: the scene ran ({scene_frames} frames)");
            assert!(after_cut > 0, "camera {camera} at {rate} fps: the countdown ran");
            if camera == 0 {
                assert!(hidden_after_cut, "the bumper view leaves the car out once the chase camera is the view");
            }
        }
    }
}
