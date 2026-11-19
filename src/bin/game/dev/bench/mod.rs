use crate::*;

pub(crate) mod boot;
pub(crate) mod env;
pub(crate) mod framelog;
pub(crate) mod latency;
pub(crate) mod menus;
pub(crate) mod rims;
pub(crate) mod scene;
pub(crate) mod transitions;

use env::BenchEnv;

/// Headless timing (`--bench <scenario> [frames]`): the load, then the
/// window's own frame (`Game::step`, `Game::draw`) on an offscreen
/// 1280×960 target at a fixed 60 Hz step and a fixed clock, without
/// sound. Prints the load time, then per frame the CPU update, the CPU
/// draw (command building and submission), the wait for the GPU and the
/// whole frame, the draw's CPU by part and, where the device has
/// timestamp queries ([`nfsu2::prof::gpu::FEATURES`]), the GPU's time by
/// part. The scenarios:
///
/// * `explore`: free roam after 20 s with the throttle held (traffic on
///   the road), then the timed frames from a standing start on a fixed
///   straight, the throttle held ([`EXPLORE_LINE`]);
/// * `race`: a stage 1 circuit from its start (its start scene or
///   countdown), the throttle held;
/// * `racing`: a career circuit with three rivals and traffic from the
///   green light, the player's car driven by the AI racer controller (as
///   past the finish line);
/// * `drive`: a career sprint with traffic driven the same way, a fast
///   drive through the city (sections and traffic come and go);
/// * `replay`: `racing` for `NFSU2_REPLAY_AFTER` frames (default 1800),
///   then its replay from the race's pause (`NFSU2_REPLAY_FROM=post`: from
///   the post-race menu), for side-by-sides with the original's replay;
/// * `boot`: the game's start up to its first frame and to the title
///   screen ([`boot::bench_boot`]); `transitions`: each kind of load from
///   free roam and from the menus ([`transitions::bench_transitions`]);
///   `menu`: the main menu's frame times while the loads and programs
///   made ahead run ([`menus::bench_menus`]).
///
/// `NFSU2_BENCH_EVENT=n` races event n instead. The frames repeat run to
/// run, so `NFSU2_BENCH_DUMP=dir` writes every `NFSU2_BENCH_DUMP_EVERY`-th
/// frame (default 60) there as PNG to compare two builds (the read-back
/// is in those frames' times). `NFSU2_BENCH_GPU=0` leaves the timestamps
/// out.
pub(crate) fn bench(game: &mut Game, what: &str, frames: usize) -> Result<()> {
    // `boot`: the process's age here (the loader and `Game::new` are behind it).
    let entered = (boot::process_age_ms(), Instant::now());
    let env = BenchEnv::from_env();
    // NFSU2_BENCH_SIZE=WxH: another target size (default 1280x960).
    let (width, height) = env.size.unwrap_or((1280, 960));
    headless_aspect(game);
    fake_hdr();
    println!("target {width}x{height}, aspect {:?}", nfsu2::render::display::aspect_mode());
    let timed = bench_window(game, &env, width, height)?;
    if what == "boot" {
        // No profile: the start has none before the title screen.
        game.pacing.clock = Some(0.0);
        return boot::bench_boot(game, bench_step, entered);
    }
    bench_profile(game, &env, width, height)?;
    // The fixed clock, stepped with the game.
    game.pacing.clock = Some(0.0);
    if what == "transitions" {
        return bench_transitions(game, bench_step);
    }
    if what == "rims" {
        return bench_rims(game, bench_step, frames);
    }
    if what == "menu" {
        return menus::bench_menus(game, bench_step, frames);
    }
    menu_first(game, &env)?;
    let started = Instant::now();
    load_scenario(game, what, &env)?;
    let load = started.elapsed().as_secs_f64() * 1000.0;
    if what == "scene" {
        println!("load {load:.1} ms");
        return bench_scene_pacing(game, frames);
    }
    // NFSU2_BENCH_INFLIGHT=n: frames the GPU may still be working on when
    // the next frame's simulation starts, as the window's swap chain allows
    // (latency 2); 0 waits for the GPU after every frame (serial: update,
    // draw and GPU added up, for the parts' attribution).
    let inflight: u64 = env.inflight.unwrap_or(2);
    if warm_up(game, what, frames, &env)? {
        return Ok(());
    }
    let (first, samples) = run_frames(game, what, frames, &env, (width, height), inflight)?;
    super::rate::bench_wav_write()?;
    report(game, what, &env, BenchRun { width, height, load, first, timed, inflight }, samples)
}

/// The fixed step: the clock and the game 1/60 s on.
const DT: f32 = 1.0 / 60.0;

/// One fixed step of the game, its clock with it.
fn bench_step(game: &mut Game) -> Result<()> {
    game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(DT));
    game.step(DT)
}

/// A career event of a kind, with three rivals and traffic (or NFSU2_BENCH_EVENT).
fn event_of(game: &Game, kind: career::EventKind, env: &BenchEnv) -> Option<usize> {
    if let Some(n) = env.event {
        return Some(n);
    }
    (1..=5).find_map(|stage| {
        game.game_data.career
            .stage_events(&game.game_data.root, stage)
            .iter()
            .find(|e| e.kind == kind && e.opponents.len() == 3 && e.traffic > 0 && e.races.len() == 1)
            .map(|e| e.index)
    })
}

/// A time since `t`, ms.
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// The whole frame done: its end submitted and the GPU idle.
fn bench_wait(game: &mut Game) {
    let w = game.platform.window.as_mut().unwrap();
    w.settle();
    let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
}

/// Pipelined: wait until frame `i` − `inflight` is done on the GPU.
/// The files under `dirs` dropped from the OS file cache (a measuring
/// aid: a load after it reads them from the disk, as after a reboot); the
/// MB dropped. Linux only (elsewhere nothing is dropped).
fn drop_pages(dirs: &[std::path::PathBuf]) -> f64 {
    let mut bytes = 0u64;
    let mut stack: Vec<std::path::PathBuf> = dirs.to_vec();
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t) if t.is_file() => bytes += drop_file_pages(&p),
                _ => {}
            }
        }
    }
    bytes as f64 / 1e6
}

#[cfg(target_os = "linux")]
fn drop_file_pages(path: &std::path::Path) -> u64 {
    use std::os::fd::AsRawFd;
    let Ok(f) = std::fs::File::open(path) else { return 0 };
    let len = f.metadata().map_or(0, |m| m.len());
    // SAFETY: a valid open descriptor; advice only, no memory touched.
    let r = unsafe { libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) };
    if r == 0 { len } else { 0 }
}

#[cfg(not(target_os = "linux"))]
fn drop_file_pages(_path: &std::path::Path) -> u64 {
    0
}

fn throttle(game: &mut Game, i: u64, inflight: u64) {
    let w = game.platform.window.as_mut().unwrap();
    while let Some((n, index)) = w.in_flight.front().cloned() {
        if n + inflight > i {
            break;
        }
        w.in_flight.pop_front();
        let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None });
    }
}

/// The whole process's CPU time (every thread: the pool's encoding too).
fn process_cpu() -> f64 {
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            let f: Vec<&str> = s.rsplit_once(')')?.1.split_whitespace().collect();
            Some((f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?) * 10.0)
        })
        .unwrap_or(0.0)
}

/// The GPU, the adapter's report and the offscreen window; whether the
/// GPU's timestamps run.
fn bench_window(game: &mut Game, env: &BenchEnv, width: u32, height: u32) -> Result<bool> {
    let timing = env.gpu_timing;
    nfsu2::prof::gpu::want(timing);
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    let timed = timing && nfsu2::prof::gpu::init(&gpu.device, &gpu.queue);
    {
        let info = gpu.adapter.get_info();
        game.platform.perf_overlay.pci = Some((info.vendor, info.device));
        let f = gpu.device.features();
        let has = |x: wgpu::Features| if f.contains(x) { "yes" } else { "no" };
        println!(
            "adapter: {} ({:?}, {} {}); BC {}, immediates {}, timestamps {}",
            info.name,
            info.backend,
            info.driver,
            info.driver_info,
            has(wgpu::Features::TEXTURE_COMPRESSION_BC),
            has(wgpu::Features::IMMEDIATES),
            has(nfsu2::prof::gpu::FEATURES),
        );
        // What the adapter offers, beside what the device was made with.
        // Multi-draw count and indirect first instance are offered but not
        // requested (`Gpu::open`): no draw is indirect (the world's and the
        // cars' draws are direct, culled on the CPU; docs/perf-integrated-gpu.md).
        println!(
            "  device: multiview cube {}, light-cluster loop per subgroup {}",
            has(wgpu::Features::MULTIVIEW),
            has(wgpu::Features::SUBGROUP)
        );
        let a = gpu.adapter.features();
        let offers = |x: wgpu::Features| if a.contains(x) { "yes" } else { "no" };
        println!(
            "  adapter offers: timestamps {}, multiview {} (views {}), multi-draw count {}, indirect first instance {}",
            offers(nfsu2::prof::gpu::FEATURES),
            offers(wgpu::Features::MULTIVIEW),
            gpu.adapter.limits().max_multiview_view_count,
            offers(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT),
            offers(wgpu::Features::INDIRECT_FIRST_INSTANCE)
        );
    }
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
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
    Ok(timed)
}

/// The profile (a saved one, a native save's or a new one) and the run's
/// settings: underglow, detail, enhancements, the offline mixer.
fn bench_profile(game: &mut Game, env: &BenchEnv, width: u32, height: u32) -> Result<()> {
    // NFSU2_FLOW_LOAD=name: that saved profile (as `--flow` loads it).
    if let Some(name) = env.flow_load.clone() {
        game.mc_act(McAct::Load(name))?;
        game.menu_key(Key::Accept)?;
    }
    // NFSU2_SHOT_SAVE=file (as the shots'): that native save's profile
    // and car (e.g. the finished-career save's Skyline with its underglow).
    if game.profile.is_none()
        && let (Some(bytes), Some(ctx)) =
            (env.shot_save.as_ref().and_then(|f| std::fs::read(f).ok()), career::NativeContext::get())
    {
        let s = nfsu2::formats::native_save::NativeSave::parse(&bytes)?;
        game.settings.apply_native(&s);
        let p = Profile::from_native(&s, ctx);
        profiles::set_current(&p.name, s.clone());
        game.profile = Some(p);
    }
    if game.profile.is_none() {
        let mut p = Profile::new("Player");
        // NFSU2_BENCH_CAR: another starting car (default the Peugeot).
        let car = env.car.clone().unwrap_or_else(|| "PEUGOT".into());
        p.skip_intro(&game.game_data.career, &game.game_data.root, &car, &mut game.rng);
        game.profile = Some(p);
    }
    // NFSU2_NEON=<part name>: that underglow (slot 0x9A) fitted for the run
    // (as `--shot neon_car`; frame strips of the glow while driving,
    // docs/neon-quality.md).
    if env.neon {
        install_shot_neon(game);
    }
    // NFSU2_BENCH_DETAIL=1..5: that World Detail preset's custom values
    // (default the options' own, detail 5), to time the low settings.
    if let Some(d) = env.detail {
        game.settings.detail = d;
        game.settings.custom = nfsu2::frontend::options::custom_presets(d);
        println!("  detail {d}: custom {:?}", game.settings.custom);
    }
    // NFSU2_BENCH_PRESET=Original|Balanced|High|Ultra|Extreme|Remaster: a
    // Graphics page preset (its name on the page), before NFSU2_BENCH_ENH.
    if let Ok(name) = std::env::var("NFSU2_BENCH_PRESET") {
        let preset = nfsu2::qol::PRESETS
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(&name))
            .with_context(|| format!("no preset {name}"))?;
        preset.apply(&mut game.settings.enh);
        game.settings.apply_enhancements();
        println!("  preset {}: {:?}", preset.name(), game.settings.enh.deviations());
    }
    // NFSU2_BENCH_LEVEL=Low|Medium|High: an Enhanced level (the Graphics
    // page's Enhanced mode), before NFSU2_BENCH_ENH.
    if let Ok(name) = std::env::var("NFSU2_BENCH_LEVEL") {
        let level = nfsu2::qol::LEVELS
            .into_iter()
            .find(|l| l.name().eq_ignore_ascii_case(&name))
            .with_context(|| format!("no level {name}"))?;
        level.apply(&mut game.settings.enh);
        game.settings.apply_enhancements();
        println!("  level {}: {:?}", level.name(), game.settings.enh.deviations());
    }
    // NFSU2_BENCH_ENH="enh.render_scale 150;enh.post_aa 1": Enhancements /
    // Graphics page values (options.txt lines) for the run, to time them.
    if let Some(lines) = env.enh.as_ref() {
        for line in lines.split(';') {
            let (k, v) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
            // `custom.N V`: a World Detail custom value (options.txt's
            // line), e.g. `custom.7 0` the road reflection off.
            if let Some(n) = k.strip_prefix("custom.").and_then(|n| n.parse::<usize>().ok())
                && let (Some(slot), Ok(v)) = (game.settings.custom.get_mut(n), v.parse())
            {
                *slot = v;
                continue;
            }
            game.settings.enh.parse(k, v);
        }
        game.settings.apply_enhancements();
        println!(
            "  enhancements: {:?}, scene {:?} without MSAA",
            game.settings.enh.deviations(),
            nfsu2::qol::scene_size((width, height), 1)
        );
    }
    // NFSU2_BENCH_AUDIO=1: the game's sounds on an offline mixer, a
    // frame's worth (800 frames at 48 kHz) rendered after each step and
    // timed, as the device's callback would render them.
    if env.audio {
        game.audio = Some(game.new_audio(Mixer::offline(audio_capture_rate())));
    }
    Ok(())
}

/// NFSU2_BENCH_MENU_FIRST's main-menu frames and NFSU2_VIEW's camera.
fn menu_first(game: &mut Game, env: &BenchEnv) -> Result<()> {
    // NFSU2_BENCH_MENU_FIRST=1: NFSU2_BENCH_MENU_FRAMES (default 200)
    // main-menu frames first, in which the menus' preload runs (or not,
    // with NFSU2_NO_PRELOAD=1: the two runs' dumps must be the same).
    if env.menu_first {
        let n: usize = env.menu_frames.unwrap_or(200);
        game.mode = game.icon_menu(MenuKind::Main);
        for _ in 0..n {
            bench_step(game)?;
            game.draw()?;
        }
        if let Some(p) = game.loads.preload.as_ref() {
            println!(
                "  (after {n} menu frames the preload is {})",
                if p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished())
                {
                    "done"
                } else {
                    "still running"
                }
            );
        }
    }
    // NFSU2_VIEW=k: the camera setting (0 bumper .. 4 high), as the
    // qrstart shot takes it (bumper and hood draw the rear-view mirror).
    if let Some(v) = env.view {
        game.settings.players[0].camera = v;
    }
    Ok(())
}

/// The scenario's load (timed by the caller): its race or free roam.
fn load_scenario(game: &mut Game, what: &str, env: &BenchEnv) -> Result<()> {
    match what {
        // NFSU2_QR_MODE=circuit|sprint|drag|drift|streetx|url (race, racing): that Quick
        // Race mode's first track instead (as the original's defaults), to
        // line its start up with the original's frame by frame.
        "race" | "scene" | "racing" if std::env::var_os("NFSU2_QR_MODE").is_some() => {
            let mode = qr_mode_env();
            // NFSU2_QR_KO=1: a circuit with Lap KO (UI_QRModeOptions).
            game.quick.tracks = game.quick_track_select(mode, 0.0);
            if std::env::var_os("NFSU2_QR_KO").is_some() {
                game.settings.qr[0].lap_ko = true;
            }
            // NFSU2_QR_DIFFICULTY=0..2, NFSU2_QR_CATCHUP=0|1,
            // NFSU2_QR_TRAFFIC=0..3: the mode record's Difficulty, Catch Up
            // and Traffic Level rows.
            let ty = nfsu2::frontend::quick_race::race_type(mode);
            let num = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u8>().ok());
            if let Some(d) = num("NFSU2_QR_DIFFICULTY") {
                game.settings.qr[ty].difficulty = d.min(2);
            }
            if let Some(c) = num("NFSU2_QR_CATCHUP") {
                game.settings.qr[ty].catch_up = c != 0;
            }
            if let Some(t) = num("NFSU2_QR_TRAFFIC") {
                game.settings.qr[ty].traffic = t.min(3);
            }
            // Bench aid: NFSU2_BENCH_SEED=n varies the random numbers (the
            // grid, the world's draws) for repeated measurements.
            if let Some(seed) = env.seed {
                game.rng.0 ^= seed.wrapping_mul(0x9E37_79B9);
            }
            let e = game.quick_event(mode);
            game.quick.car = Some(std::env::var("NFSU2_QR_CAR").unwrap_or_else(|_| "PEUGOT".into()));
            println!(
                "quick race {mode:?}: {} ({})",
                game.game_data.career.track_title(game.game_data.career.events[e].track),
                game.game_data.career.events[e].track
            );
            game.start_race(e, true, false)?;
            if let (Mode::Race(r), Some(d)) = (&game.mode, game.sim.drive.as_ref()) {
                println!("  knockout {}, laps {}", r.scoring.knockout, r.laps);
                println!(
                    "  grid slot {}, at {:.1?} heading {:.1}°, scene {:?}",
                    r.grid_slot,
                    d.state.position,
                    d.state.heading().to_degrees(),
                    game.scenes.shop.as_ref().map(|s| (s.scene.kind, s.scene.marks, s.scene.length))
                );
            }
        }
        "race" | "scene" => {
            let e = game
                .game_data.career
                .stage_events(&game.game_data.root, 1)
                .iter()
                .find(|e| e.kind == career::EventKind::Circuit)
                .map(|e| e.index)
                .context("a circuit")?;
            game.start_race(e, false, false)?;
        }
        "racing" | "drive" | "replay" | "latency" => {
            let kind = if what == "drive" { career::EventKind::Sprint } else { career::EventKind::Circuit };
            // NFSU2_BENCH_KIND=drift|drag (measuring aid): the first
            // event of that kind instead (its cameras).
            let wanted = match env.kind.as_deref() {
                Some("drift") => Some(career::EventKind::Drift),
                Some("drag") => Some(career::EventKind::Drag),
                _ => None,
            };
            let e = match wanted {
                Some(k) => (1..=5)
                    .find_map(|stage| {
                        game.game_data.career.stage_events(&game.game_data.root, stage).iter().find(|e| e.kind == k).map(|e| e.index)
                    })
                    .context("an event of that kind")?,
                None => event_of(game, kind, env).context("an event with three rivals and traffic")?,
            };
            // NFSU2_BENCH_ENGAGE=n: from free roam, through n frames of the
            // event's Engage dialog (its rivals load ahead then, unless
            // NFSU2_NO_PRELOAD=1: the two runs' dumps must be the same).
            if let Some(n) = env.engage {
                game.explore(None)?;
                game.mode = Mode::Menu(Menu::Engage(Engage {
                    event: e,
                    message: 3,
                    opened: 0.0,
                    focus: Default::default(),
                    purse: game.game_data.career.events[e].reward,
                }));
                for _ in 0..n {
                    bench_step(game)?;
                    game.draw()?;
                }
            }
            game.start_race(e, false, false)?;
        }
        _ => {
            // Free roam proper: with the intro played (the profile's past
            // it), the career's start scene (a cut-scene with no HUD and no
            // traffic) does not run.
            game.explore(None)?
        }
    }
    Ok(())
}

/// Where the explore bench's timed frames start (the road south of the free
/// roam start, heading along it) and the heading, degrees.
const EXPLORE_LINE: (Vec3, f32) = (Vec3::new(1710.5, -904.9, 24.0), -99.8);

/// The scenario up to its timed frames: free roam's 20 s of driving, the
/// race to the green light and on the AI controller, the replay. True when
/// the bench ended there (`latency`).
fn warm_up(game: &mut Game, what: &str, frames: usize, env: &BenchEnv) -> Result<bool> {
    match what {
        // Free roam: 20 s of driving first, so the traffic placer (a car a
        // second) has filled the road.
        "explore" => {
            // NFSU2_BENCH_AT=x,y,z,deg (measuring aid, docs/shadow-softness.md):
            // the car placed there (as NFSU2_SHOT_AT), standing; with
            // NFSU2_BENCH_FREEZE=1 a still view for temporal variance.
            // NFSU2_BENCH_AT_DRIVE=n: n frames of driving there first.
            if let Some(v) = env.at {
                if let Some(d) = game.sim.drive.as_mut() {
                    d.place(Vec3::new(v[0], v[1], v[2]), v[3].to_radians());
                }
                // NFSU2_FX_SPEED=km/h with NFSU2_FX_GEAR=n (measuring
                // aid, as the explore shot's): the placed car rolling at
                // that speed in that gear, the gas held throughout (no
                // standing frames), e.g. the reported 217 km/h rain chase.
                let fx_speed: Option<f32> = env.fx_speed;
                let fx_gear: Option<usize> = env.fx_gear;
                let set_speed = |game: &mut Game, kmh: f32| {
                    if let Some(d) = game.sim.drive.as_mut() {
                        let v = kmh / 3.6;
                        d.state.velocity = glam::Mat3::from_quat(d.state.orientation) * Vec3::X * v;
                        for (w, spec) in d.state.wheels.iter_mut().zip(d.car.car_type.wheels.iter()) {
                            w.spin = v / spec.radius.max(0.1);
                        }
                    }
                };
                let set_gear = |game: &mut Game| {
                    if let (Some(g), Some(t)) = (fx_gear, game.sim.drive.as_mut().and_then(|d| d.state.drivetrain.as_mut()))
                    {
                        t.gear = g + nfsu2::handling::NEUTRAL;
                        t.target_gear = t.gear;
                    }
                };
                let drive: usize = env.at_drive.unwrap_or(0);
                if let Some(kmh) = fx_speed {
                    set_speed(game, kmh);
                    for i in 0..drive {
                        game.platform.keys.insert(KeyCode::KeyW);
                        if i < 3 {
                            set_gear(game);
                        }
                        bench_step(game)?;
                    }
                }
                for i in 0..if fx_speed.is_some() { 0 } else { 120 + drive } {
                    if i < drive {
                        game.platform.keys.insert(KeyCode::KeyW);
                    } else {
                        game.platform.keys.clear();
                    }
                    bench_step(game)?;
                }
            } else {
                for _ in 0..1200 {
                    game.platform.keys.insert(KeyCode::KeyW);
                    bench_step(game)?;
                }
                // The timed frames drive a fixed line: the car is put at
                // the start of the straight south of the start (standing,
                // lane-centred, along the road) and the throttle stays held.
                // The 20 s run itself goes head-on into the wall east of the
                // start, where the car stays pressed for seconds and then
                // pivots off one way or the other, as the original's does
                // (docs/prop-hits.md §7c): where it ends turns on that, so it
                // cannot be what the frames measure.
                if let Some(d) = game.sim.drive.as_mut() {
                    d.place(EXPLORE_LINE.0, EXPLORE_LINE.1.to_radians());
                }
            }
        }
        // To the green light (past the start scene, or the countdown),
        // then the player's car on the AI racer controller.
        "racing" | "drive" | "replay" | "latency" => {
            if game.scenes.shop.is_some() {
                for _ in 0..60 * 60 {
                    if game.scenes.shop.is_none() || !matches!(game.mode, Mode::Race(_)) {
                        break;
                    }
                    bench_step(game)?;
                }
            }
            for _ in 0..(COUNTDOWN * 60.0) as u32 + 2 {
                if matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 0.0) {
                    break;
                }
                bench_step(game)?;
            }
            // The controller directly, not by marking the player finished:
            // a finish now ends the race (and plays its ending) 3 s later.
            if let (Mode::Race(r), Some(d)) = (&mut game.mode, game.sim.drive.as_ref()) {
                if let Some(routes) = r.racers.ai_routes.as_ref() {
                    r.autopilot = Some(Autopilot::new(d, routes, d.car.car_type.handling.race.drift()));
                }
            }
            let Mode::Race(r) = &game.mode else { anyhow::bail!("the race ended at the start") };
            let track = r.route.road.length;
            println!("event {}: {} m of road, {} laps", r.event, track as u32, r.laps);
            if what == "latency" {
                return bench_latency(game, frames).map(|()| true);
            }
            // `replay`: NFSU2_REPLAY_AFTER frames (default 1800, 30 s) of
            // the race on the autopilot, then its replay, timed and dumped:
            // from the race's pause (UI_pause's Replay), or with
            // NFSU2_REPLAY_FROM=post from the post-race menu.
            if what == "replay" {
                let after: usize = env.replay_after.unwrap_or(1800);
                for _ in 0..after {
                    bench_step(game)?;
                }
                let Mode::Race(r) = std::mem::replace(&mut game.mode, Mode::Explore) else {
                    anyhow::bail!("the race ended before its replay")
                };
                if env.replay_from_post {
                    game.mode = Mode::Race(r);
                    let menu = game.icon_menu(MenuKind::PostRace);
                    game.keep_replay(menu);
                    game.act(Action::Replay, MenuKind::PostRace)?;
                } else {
                    game.sim.paused_race = Some(r);
                    game.mode = game.icon_menu(MenuKind::RacePause);
                    game.act(Action::Replay, MenuKind::RacePause)?;
                }
                anyhow::ensure!(matches!(&game.mode, Mode::Race(r) if r.replay.playback.is_some()), "no replay");
                // Measuring aids: NFSU2_REPLAY_CAMERA=n the replay
                // camera index (6.. the extras with NFSU2_BENCH_ENH
                // "enh.replay_cameras 1"), NFSU2_REPLAY_SLOW=1 the native
                // slow motion / 0.5 / 0.25 the smooth one,
                // NFSU2_REPLAY_SMOOTH_PROBE=hz: the window loop's frames at
                // that rate for 2 s, printing the player's drawn step per
                // frame (the smoothness of slow motion), then the bench.
                if let Mode::Race(r) = &mut game.mode
                    && let Some(p) = r.replay.playback.as_mut()
                {
                    if let Some(n) = env.replay_camera {
                        while p.camera.index != n {
                            p.camera.cycle();
                            if p.camera.index == 0 {
                                break;
                            }
                        }
                    }
                    match env.replay_slow {
                        Some(v) if v >= 1.0 => p.slow = true,
                        Some(v) if v > 0.0 => p.smooth = v,
                        _ => {}
                    }
                }
                if let Some(hz) = env.replay_smooth_probe {
                    // Past the skip's wait first.
                    for _ in 0..60 {
                        game.advance(1.0 / 60.0)?;
                    }
                    let mut last: Option<Vec3> = None;
                    let mut steps = Vec::new();
                    for _ in 0..(hz * 2.0) as usize {
                        let alpha = game.advance(1.0 / hz)?;
                        let now = game.sim.drive.as_ref().map(|d| nfsu2::interpolate::CarPose::of(&d.state));
                        let drawn = match (game.interp.prev.poses.first(), now) {
                            (Some(a), Some(b)) if alpha < 1.0 => {
                                nfsu2::interpolate::CarPose::blend(a, &b, alpha, game.pacing.last_update_dt)
                                    .map_or(b.position, |c| c.position)
                            }
                            (_, Some(b)) => b.position,
                            _ => Vec3::ZERO,
                        };
                        if let Some(l) = last {
                            steps.push(drawn.distance(l));
                        }
                        last = Some(drawn);
                    }
                    let mean = steps.iter().sum::<f32>() / steps.len().max(1) as f32;
                    let still = steps.iter().filter(|&&s| s < mean * 0.1).count();
                    let max = steps.iter().copied().fold(0.0, f32::max);
                    let sd = (steps.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / steps.len().max(1) as f32).sqrt();
                    println!(
                        "replay smooth probe {hz} Hz: drawn step mean {:.4} m, sd {:.4} m ({:.0} %), max {:.4} m, {still} of {} frames standing",
                        mean,
                        sd,
                        sd / mean.max(1e-6) * 100.0,
                        max,
                        steps.len()
                    );
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

/// [`run_frames`]' measurements for [`report`].
struct Samples {
    ran: usize,
    start_at: Option<Vec3>,
    update: Vec<f64>,
    draw: Vec<f64>,
    gpu: Vec<f64>,
    whole: Vec<f64>,
    update_cpus: Vec<f64>,
    draw_cpus: Vec<f64>,
    update_cpu: u64,
    draw_cpu: u64,
    travelled: f32,
    low_eye: Vec<usize>,
    sections_seen: std::collections::HashSet<u32>,
    parts: Vec<(&'static str, u64, u64)>,
    process_start: f64,
    bench_json: Option<std::path::PathBuf>,
    threads_start: Vec<(u64, String, u64)>,
    gpu_mem_peak: (u64, u64),
    gpu_engine_start: u64,
    audio_ms: Vec<f64>,
    audio_cpu: Vec<f64>,
    audio_voices: usize,
    audio_sq: f64,
    audio_n: usize,
}

/// The run's figures besides the samples.
struct BenchRun {
    width: u32,
    height: u32,
    load: f64,
    first: f64,
    timed: bool,
    inflight: u64,
}

/// The timed frames: the first frame (its own time), then `frames` of the
/// window's frame, each timed by part, dumped, steered by the aids.
fn run_frames(
    game: &mut Game,
    what: &str,
    frames: usize,
    env: &BenchEnv,
    size: (u32, u32),
    inflight: u64,
) -> Result<(f64, Samples)> {
    let (width, height) = size;
    // NFSU2_BENCH_CAMTRACK=file (comparison aid): each frame's camera and
    // car pose from a line of the file (see `TrackPose`), e.g. the
    // original's view 1 logged under Wine, to render the same path.
    let cam_track: Vec<TrackPose> = match env.camtrack.as_ref() {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("reading {path}"))?
            .lines()
            .filter_map(TrackPose::parse)
            .collect(),
        None => Vec::new(),
    };
    let freeze_at: Option<usize> = env.freeze;
    let mut frozen: Option<(Vec3, Quat, nfsu2::render::camera::Camera)> = None;
    // A rate run (`NFSU2_RATE`): the frames on its schedule, as at
    // NFSU2_BENCH_HZ (the window's path, `rate::frame`).
    let bench_hz: Option<f32> = env.hz.or(super::rate::active().then_some(60.0));
    // NFSU2_BENCH_HZ outside a rate run: the frames on the
    // exact integer schedule of that rate (`rate::Schedule`, 1/36 000 s
    // units; before, an f32 1/hz each frame and an f64 clock that drifted,
    // a tick short at 200 Hz after 10 s), the fixed clock its exact time.
    let mut hz_schedule = env.hz.filter(|_| !super::rate::active()).and_then(|hz| super::rate::Schedule::parse(&hz.to_string()));
    let mut hz_clock_base: Option<f64> = None;
    let dump = env.dump.clone();
    let every: usize = env.dump_every.unwrap_or(60).max(1);
    let dump_frames: Option<Vec<(usize, usize)>> = env.dump_frames.as_ref().map(|v| {
        v.split(',')
            .filter_map(|r| {
                let (a, b) = r.split_once('-').unwrap_or((r, r));
                Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
            })
            .collect()
    });
    if let Some(dir) = &dump {
        std::fs::create_dir_all(dir)?;
    }
    let save = |game: &mut Game, i: usize| -> Result<()> {
        let Some(dir) = &dump else { return Ok(()) };
        let w = game.platform.window.as_mut().unwrap();
        w.settle();
        let mut pixels = nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), width, height);
        for p in pixels.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        image::save_buffer(dir.join(format!("{what}-{i:05}.png")), &pixels, width, height, image::ColorType::Rgba8)?;
        // The race clock of each dumped frame, to line dumps up with the
        // original's frames by game clock.
        if let Mode::Race(r) = &game.mode {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("clock.tsv"))?;
            // The racers' places (the player first), to line the grid
            // and the start scene's cars up with the original's memory.
            let mut cars = game
                .sim.drive
                .as_ref()
                .map(|d| format!("{:.2},{:.2}", d.state.position.x, d.state.position.y))
                .unwrap_or_default();
            for rv in &r.racers.rivals {
                cars.push_str(&format!("\t{}:{:.2},{:.2}", rv.slot + 1, rv.state.position.x, rv.state.position.y));
            }
            writeln!(f, "{i}\t{:.4}\t{}\t{cars}", r.timing.clock, game.scenes.shop.is_some())?;
        } else if let Some(dir) = &dump {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("clock.tsv"))?;
            let mode = match &game.mode {
                Mode::Menu(Menu::Results(..)) => "results".into(),
                Mode::Menu(Menu::Icons(_, k)) => format!("icons {k:?}"),
                Mode::Menu(Menu::Reward(_)) => "reward".into(),
                Mode::Menu(_) => "menu".into(),
                _ => "other".into(),
            };
            writeln!(f, "{i}\t-\t{}\t{mode}", game.scenes.shop.is_some())?;
        }
        Ok(())
    };
    // The first frame builds the renderers.
    let hold = |game: &mut Game| {
        if what == "explore" || what == "race" {
            game.platform.keys.insert(KeyCode::KeyW);
        }
    };
    if !env.no_gas {
        hold(game);
    }
    let pipelines_at_load = nfsu2::render::pipelines_built();
    let builds_at_load = Builds::now();
    let t = Instant::now();
    bench_step(game)?;
    let first_update = ms(t);
    nfsu2::prof::acc::enable(true);
    let _ = nfsu2::prof::acc::take();
    let t_draw = Instant::now();
    game.draw()?;
    let first_draw = ms(t_draw);
    nfsu2::prof::acc::enable(false);
    let t_wait = Instant::now();
    bench_wait(game);
    let first_wait = ms(t_wait);
    let first = ms(t);
    {
        let mut parts: Vec<_> = nfsu2::prof::acc::take().into_iter().filter(|p| p.1 >= 1_000_000).collect();
        parts.sort_by_key(|p| std::cmp::Reverse(p.1));
        let parts: Vec<String> = parts.iter().map(|(l, ns, _)| format!("{} {:.1}", l.trim(), *ns as f64 / 1e6)).collect();
        println!(
            "first frame: update {first_update:.1} ms, draw {first_draw:.1} ms, GPU wait {first_wait:.1} ms; draw CPU parts over 1 ms: {}",
            parts.join(", ")
        );
    }
    // Pipelines built by the first frame, then each later frame that
    // built one (a first-use hitch).
    let mut pipelines_seen = nfsu2::render::pipelines_built();
    println!("pipelines: {pipelines_at_load} by the load, {} in its first frame", pipelines_seen - pipelines_at_load);
    println!("first frame builds: {}", Builds::now().since(&builds_at_load));
    let mut pipeline_hitches = 0usize;
    let photo_at: Option<usize> = std::env::var("NFSU2_BENCH_PHOTO_AT").ok().and_then(|v| v.parse().ok());
    let first_frames: Option<usize> = std::env::var("NFSU2_BENCH_FIRST_FRAMES").ok().and_then(|v| v.parse().ok());
    // The frame a NFSU2_BENCH_ENH_SWITCH applied at (its draw reported).
    let mut switched: Option<usize> = None;
    save(game, 0)?;
    // The first frame's timing left out.
    nfsu2::prof::gpu::finish(&game.platform.window.as_ref().unwrap().gpu.device);
    let _ = (nfsu2::prof::gpu::take(), nfsu2::prof::gpu::take_frames());
    let start_at = game.sim.drive.as_ref().map(|d| d.state.position);
    let (mut update, mut draw, mut gpu, mut whole) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut update_cpus, mut draw_cpus) = (Vec::new(), Vec::new());
    let cpu = nfsu2::prof::thread_cpu_ns;
    let (mut update_cpu, mut draw_cpu) = (0, 0);
    // A frame's wall time: from its simulation's start to the next's.
    let mut frame_start = Instant::now();
    let mut travelled = 0.0f32;
    let mut low_eye: Vec<usize> = Vec::new();
    let mut sections_seen = std::collections::HashSet::new();
    // The draw's parts' CPU time (`prof::acc` spans in `draw`): the
    // totals, and each part's worst frame.
    nfsu2::prof::acc::enable(true);
    let _ = nfsu2::prof::acc::take();
    let mut parts: Vec<(&'static str, u64, u64)> = Vec::new();
    let mut ran = 0;
    let mut framelog = framelog::FrameLog::open();
    // The whole process's CPU time (every thread: the pool's encoding too).
    let process_start = process_cpu();
    // NFSU2_BENCH_JSON=file (the performance harness, docs/perf-gates.md): the run's
    // numbers as JSON — every frame's times, the parts, per-thread CPU,
    // the process's own VRAM/GTT peak (sampled every 30 frames), memory.
    // NFSU2_TRACE=file.json: each frame's update / draw / throttle as
    // trace spans too. Both off: nothing of this runs.
    let bench_json = env.json.clone();
    let trace_frames = nfsu2::prof::trace::on();
    let threads_start = if bench_json.is_some() { nfsu2::prof::threads() } else { Vec::new() };
    let mut gpu_mem_peak = (0u64, 0u64);
    let gpu_engine_start = bench_json.as_ref().and_then(|_| nfsu2::prof::gpu_memory()).map_or(0, |m| m.2);
    let (mut audio_buf, mut audio_ms, mut audio_cpu, mut audio_voices) = (Vec::new(), Vec::new(), Vec::new(), 0usize);
    let (mut audio_sq, mut audio_n) = (0.0f64, 0usize);
    // A rate run with NFSU2_AUDIO_WAV: the mix rendered per tick, after the
    // tick's sounds (`audio_tick_end`: each tick's own frames, 800 a RACE
    // tick, 1600 a SCENE tick at 48 kHz), into the capture the WAV is
    // written from; the per-frame pump below stands aside.
    if env.audio && super::rate::active() && std::env::var_os("NFSU2_AUDIO_WAV").is_some() {
        *AUDIO_CAPTURE.lock().unwrap() = Some((Vec::new(), 0.0));
    }
    // NFSU2_BENCH_FINISH=s: the player's racer finished s seconds after
    // the green light (as the original's racer +0x09 written to the race's
    // laps: 0x608C11 finishes it that frame), to compare the finish, the
    // race-over message, the ending scene and the results with the
    // original's. NFSU2_BENCH_THROUGH=1: run on through the post-race
    // screens (the results' cycling columns) instead of stopping at the
    // race's end.
    let finish_at: Option<f32> = env.finish;
    // NFSU2_BENCH_LAPS=n@s: the player's lap count and most laps set to n
    // s seconds after the green (the original's racer +0x08 / +0x09
    // written), e.g. to stand first in a Lap KO without driving.
    let mut laps_at: Option<(i32, f32)> = env.laps.as_ref().and_then(|v| {
        let (n, t) = v.split_once('@')?;
        Some((n.parse().ok()?, t.parse().ok()?))
    });
    let no_gas = env.no_gas;
    let through = env.through;
    let mut overlay_last: Option<Instant> = None;
    // The settings combination matrix (scripts/checks/matrix.sh,
    // docs/settings-model.md): NFSU2_BENCH_MATRIX="orig,max,pairs,rules"
    // draws NFSU2_BENCH_MATRIX_FRAMES (default 3) frames with each setting
    // of `settings_model::matrix_configs`, switched live as the Graphics
    // page does; a panic or a wgpu validation error ends the run.
    let matrix = env
        .matrix
        .as_ref()
        .map(|v| nfsu2::frontend::settings_model::matrix_configs(&game.settings, v))
        .unwrap_or_default();
    let matrix_k: usize = env.matrix_frames.unwrap_or(3).max(1);
    let frames = if matrix.is_empty() { frames } else { frames.max(matrix.len() * matrix_k + 2) };
    if !matrix.is_empty() {
        println!("  matrix: {} settings x {matrix_k} frames", matrix.len());
    }
    let mut matrix_done = 0usize;
    for i in 1..=frames {
        // A rate run with a tick budget (`NFSU2_RATE_TICKS`) ends on its
        // last tick, whatever the frame count.
        if super::rate::budget_spent() {
            println!("  (stopped after {ran} frames: the rate run's ticks are taken)");
            break;
        }
        if (i - 1) % matrix_k == 0
            && let Some((name, s)) = matrix.get((i - 1) / matrix_k)
        {
            game.settings = s.clone();
            game.settings.apply_enhancements();
            matrix_done = (i - 1) / matrix_k + 1;
            println!("  matrix {matrix_done}/{}: {name}", matrix.len());
        }
        if !through && !matches!(game.mode, Mode::Explore | Mode::Race(_)) {
            println!(
                "  (stopped after {ran} frames: the {} ended)",
                if what == "explore" { "free roam" } else { "race" }
            );
            break;
        }
        if let (Some(at), Mode::Race(r)) = (finish_at, &mut game.mode)
            && r.timing.clock >= at
            && r.racers.me.finished.is_none()
            && r.replay.playback.is_none()
        {
            println!("  finished at {:.2} s (frame {i})", r.timing.clock);
            // As the original's +0x08 / +0x09 = the race's laps: the
            // progress (+0x20 = lap + fraction) goes up with them.
            let n = race_laps(&r.route, r.laps) as i32;
            if let Some(l) = r.racers.me.laps.as_mut() {
                l.progress += (n - l.lap) as f32;
                (l.lap, l.most) = (n, n);
            }
            r.racers.me.finished = Some(r.timing.clock);
        }
        if let (Some((n, at)), Mode::Race(r)) = (laps_at, &mut game.mode)
            && r.timing.clock >= at
            && let Some(l) = r.racers.me.laps.as_mut()
        {
            l.progress += (n - l.lap) as f32;
            (l.lap, l.most) = (n, n);
            println!("  laps set to {n} at {:.2} s (frame {i})", r.timing.clock);
            laps_at = None;
        }
        if !no_gas {
            hold(game);
        }
        // NFSU2_BENCH_SPEEDS=1: the player's and every rival's speed (m/s)
        // each quarter second of race clock once the player has finished.
        if nfsu2::env_set!("NFSU2_BENCH_SPEEDS")
            && let (Mode::Race(r), Some(d)) = (&game.mode, game.sim.drive.as_ref())
            && let Some(f) = r.racers.me.finished
            && ((r.timing.clock - f) * 4.0).floor() != ((r.timing.clock - f - 1.0 / 60.0) * 4.0).floor()
        {
            let rivals: Vec<String> = r
                .racers.rivals
                .iter()
                .map(|x| {
                    format!("{:.1}{}", x.state.velocity.length(), if x.progress.finished.is_some() { "*" } else { "" })
                })
                .collect();
            println!("  speeds +{:.2} s: me {:.1} rivals {}", r.timing.clock - f, d.state.velocity.length(), rivals.join(" "));
        }
        // NFSU2_BENCH_RIVALS=file: every rival's laps (lap + fraction) and
        // speed each half second of race clock, and its lap times, for
        // comparisons with the original's racers.
        if let (Some(path), Mode::Race(r)) = (nfsu2::env_value_os!("NFSU2_BENCH_RIVALS"), &game.mode)
            && r.timing.clock >= 0.0
            && (r.timing.clock / 0.5).floor() != ((r.timing.clock - DT) / 0.5).floor()
        {
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
            let cols: Vec<String> = r
                .racers.rivals
                .iter()
                .map(|v| {
                    format!(
                        "{:.3}/{:.1}/r{}",
                        v.progress.laps.as_ref().map_or(0.0, |l| l.progress),
                        v.state.velocity.length(),
                        v.speed.skill()
                    )
                })
                .collect();
            writeln!(
                f,
                "{:.2}\tdiff {}\tthrottle {}\t{}",
                r.timing.clock,
                game.sim.race_globals.difficulty,
                game.sim.race_globals.throttle_level,
                cols.join("\t")
            )?;
        }
        // NFSU2_BENCH_AILOG=file (bench aid): each rival every 0.1 s of
        // race clock: row, car, laps, speed, gear, rpm, nitrous, tank
        // seconds, gas, brake, target speed, position, mover, heading (deg),
        // forward speed, GetUnstuck running, steer (deg), contact records,
        // the driver's look-ahead point.
        if let (Some(path), Mode::Race(r)) = (nfsu2::env_value_os!("NFSU2_BENCH_AILOG"), &game.mode)
            && r.timing.clock >= 0.0
            && (r.timing.clock / 0.1).floor() != ((r.timing.clock - DT) / 0.1).floor()
        {
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
            for (k, v) in r.racers.rivals.iter().enumerate() {
                let (gear, rpm) = match v.state.melo.as_ref() {
                    Some(m) => (m.gear, m.rpm),
                    None => v.state.drivetrain.as_ref().map_or((0, 0.0), |d| (d.gear, d.rpm())),
                };
                let (on, left) =
                    v.state.nitrous.as_ref().map_or((false, 0.0), |t| (t.running.is_some(), t.remaining()));
                writeln!(
                    f,
                    "{:.2}\t{k}\t{}\t{:.4}\t{:.2}\t{gear}\t{rpm:.0}\t{}\t{left:.2}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.1}\t{:.1}\tm{}\t{:.1}\t{:.2}\tu{}\t{:.1}\tc{}\t{:.1}\t{:.1}",
                    r.timing.clock,
                    v.speed.skill(),
                    v.progress.laps.as_ref().map_or(0.0, |l| l.progress),
                    v.state.velocity.length(),
                    on as u8,
                    v.car.car_type.name,
                    v.controls.gas,
                    v.controls.brake,
                    v.driver.prev_target,
                    v.state.position.x,
                    v.state.position.y,
                    if v.belt.belt.is_some() {
                        4
                    } else if v.state.melo.is_some() {
                        3
                    } else {
                        1
                    },
                    v.state.heading().to_degrees(),
                    v.state.speed(),
                    v.unstuck.since.is_some() as u8,
                    v.controls.steer_degrees,
                    v.state.contacts.len(),
                    v.driver.target.x,
                    v.driver.target.y
                )?;
            }
        }
        let before = game.sim.drive.as_ref().map(|d| d.state.position);
        let (t, c) = (Instant::now(), cpu());
        if i > 1 {
            whole.push((t - frame_start).as_secs_f64() * 1000.0);
        }
        // NFSU2_STREAK_FORM=auto: the light streaks' form chosen from the
        // bench's own frames, as the window's (a measuring aid).
        if let Some(hz) = bench_hz {
            game.streak_bench_frame((i > 1).then(|| (t - frame_start).as_secs_f32()), hz);
        }
        frame_start = t;
        // NFSU2_BENCH_HZ=144 (measuring aid, docs/chrome-flicker.md): the
        // frames drawn at that display rate as the windowed loop does (the
        // world at its 60 fps cadence, the frames between interpolated),
        // on the fixed clock; default one 1/60 s update per frame.
        // The audio tick clock's frames before this drawn frame (
        // each tick's real-time length at the mix rate): at NFSU2_BENCH_HZ
        // outside a rate run the frame's pump renders exactly its ticks'.
        let tick_frames_before = game.audio.as_ref().map_or(0, |a| a.tick_frames.0);
        let alpha = match bench_hz {
            Some(hz) => {
                let dt = match super::rate::frame(game) {
                    Some(dt) => dt,
                    None => match hz_schedule.as_mut() {
                        Some(sch) => {
                            let base = *hz_clock_base.get_or_insert(game.pacing.clock.unwrap_or(0.0));
                            let dt = sch.next_dt();
                            game.pacing.clock = Some(base + sch.seconds());
                            dt
                        }
                        None => {
                            game.pacing.clock = game.pacing.clock.map(|c| c + 1.0 / f64::from(hz));
                            1.0 / hz
                        }
                    },
                };
                game.advance(dt)?
            }
            None => {
                // NFSU2_BENCH_DRAWS=n (measuring aid): n drawn frames per world
                // update, the first n − 1 interpolated (`draw_between`), as a
                // display at n × 60 Hz draws them; the dump is the last.
                let draws: usize = nfsu2::env_parsed!("NFSU2_BENCH_DRAWS", usize).unwrap_or(1).max(1);
                if draws > 1 {
                    game.pacing.last_update_dt = DT;
                    game.refresh_snapshot();
                }
                bench_step(game)?;
                // The update's tick snapshot (the window's ticks take it
                // in `advance`).
                if draws > 1 {
                    game.capture_tick(DT);
                }
                for k in 1..draws {
                    game.draw_between(k as f32 / draws as f32)?;
                }
                1.0
            }
        };
        let u = ms(t);
        let uc = cpu() - c;
        // A rate run renders its mix per tick through the capture
        // (`audio_tick_end`), or not at all without NFSU2_AUDIO_WAV;
        // outside one the mix is pumped here per drawn frame: the fixed
        // step's 1/60 s, at NFSU2_BENCH_HZ the frames of the ticks the drawn
        // frame ran (none on a frame without a tick, 800 / 1600 a RACE /
        // SCENE tick at 48 kHz), so no tick is rendered twice or not at all.
        if let Some(a) = game.audio.as_ref().filter(|_| env.audio && !super::rate::active()) {
            let (t, c) = (Instant::now(), cpu());
            audio_buf.clear();
            let n = match bench_hz {
                Some(_) => (a.tick_frames.0 - tick_frames_before) as usize,
                None => audio_capture_rate() as usize / 60,
            };
            a.mixer.pump(n, &mut audio_buf);
            audio_ms.push(ms(t));
            audio_sq += audio_buf.iter().map(|&x| f64::from(x) * f64::from(x)).sum::<f64>();
            audio_n += audio_buf.len();
            audio_cpu.push((cpu() - c) as f64 / 1e6);
            audio_voices = audio_voices.max(a.mixer.voices());
        }
        update_cpu += uc;
        // NFSU2_BENCH_ENH_SWITCH="frame:enh.car_reflection 512;…": values
        // changed mid-run, as the Graphics page does (its live changes).
        // Several switches separated by '|' ("30:…|60:…", the
        // combination matrix, scripts/checks/matrix.sh); a switch starting
        // with '!' first resets every graphics value to native (the
        // Original preset); `custom.N V` sets a World Detail custom value.
        if let Some(v) = nfsu2::env_value!("NFSU2_BENCH_ENH_SWITCH") {
            for sw in v.split('|') {
                let Some((at, lines)) = sw.split_once(':') else { continue };
                let (reset, at) = match at.trim().strip_prefix('!') {
                    Some(a) => (true, a),
                    None => (false, at.trim()),
                };
                if at.parse::<usize>().ok() != Some(i) {
                    continue;
                }
                if reset {
                    nfsu2::qol::Preset::Original.apply(&mut game.settings.enh);
                }
                for line in lines.split(';') {
                    let (k, v) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
                    if let Some(n) = k.strip_prefix("custom.").and_then(|n| n.parse::<usize>().ok())
                        && let (Some(slot), Ok(v)) = (game.settings.custom.get_mut(n), v.parse())
                    {
                        *slot = v;
                        continue;
                    }
                    game.settings.enh.parse(k, v);
                }
                game.settings.apply_enhancements();
                println!("  frame {i}: enhancements {:?}", game.settings.enh.deviations());
                // What the options screen's next frame does first (the
                // programs for the new rows, `Game::warm_for_settings`),
                // timed apart from the frame that then draws with them.
                let before = Builds::now();
                let t = Instant::now();
                game.warm_for_settings();
                println!(
                    "  frame {i}: warm for the settings {:.1} ms ({})",
                    ms(t),
                    Builds::now().since(&before)
                );
                pipelines_seen = nfsu2::render::pipelines_built();
                switched = Some(i);
            }
        }
        // NFSU2_BENCH_PHOTO_AT=n: frame n opens Photo Mode from the pause
        // menu (with its row on), as a player does; that frame reported.
        if photo_at == Some(i) && game.sim.drive.is_some() {
            game.mode = game.icon_menu(MenuKind::ExplorePause);
            game.act(Action::PhotoMode, MenuKind::ExplorePause)?;
            println!("  frame {i}: photo mode {}", if game.photo.is_some() { "opened" } else { "not opened" });
            pipelines_seen = nfsu2::render::pipelines_built();
            switched = Some(i);
        }
        if let Some(pose) = cam_track.get(i - 1)
            && let Some(d) = game.sim.drive.as_mut()
        {
            pose.apply(d);
        }
        // NFSU2_BENCH_FREEZE=n (measuring aid, docs/chrome-flicker.md): from
        // frame n the player's car and the camera held where they were then
        // (the world, traffic and reflections go on), for a still view.
        if let Some(d) = game.sim.drive.as_mut()
            && let Some(at) = freeze_at.filter(|&n| i >= n)
        {
            let mut held = *frozen.get_or_insert((d.state.position, d.state.orientation, d.camera));
            // NFSU2_BENCH_FREEZE_MOVE=dx,dy,dz (measuring aid): the held
            // car and camera moved that much a frame (a steady crawl).
            if let Some(v) = env.freeze_move {
                let step = Vec3::new(v[0], v[1], v[2]) * (i - at) as f32;
                held.0 += step;
                held.2.position += step;
            }
            d.state.position = held.0;
            d.state.orientation = held.1;
            d.state.velocity = Vec3::ZERO;
            d.state.spin = Vec3::ZERO;
            d.state.angular_momentum = Vec3::ZERO;
            d.camera = held.2;
        }
        let (t2, c) = (Instant::now(), cpu());
        if bench_hz.is_some() {
            game.draw_between(alpha)?;
        } else {
            game.draw()?;
        }
        // NFSU2_RATE_DUMP=dir: the α = 0 frames at ticks ≡ 0 (mod 6).
        if let Some(n) = super::rate::dump_tick(game.pacing.frame_updates, alpha) {
            super::rate::dump(game, n, alpha, width, height)?;
        }
        let d = ms(t2);
        let dc = cpu() - c;
        draw_cpu += dc;
        update_cpus.push(uc as f64 / 1e6);
        draw_cpus.push(dc as f64 / 1e6);
        if nfsu2::env_value!("NFSU2_BENCH_TRACE").is_some()
            && let Some(dr) = game.sim.drive.as_ref()
        {
            let m = dr.state.transform();
            println!(
                "trace {i}: car {:?} up {:?} cam {:?} fwd {:?} up {:?}",
                dr.state.position,
                m.z_axis.truncate(),
                dr.camera.position,
                dr.camera.forward(),
                dr.camera.up
            );
        }
        let t3 = Instant::now();
        if inflight == 0 {
            bench_wait(game);
        } else {
            throttle(game, i as u64, inflight);
        }
        let g = ms(t3);
        if trace_frames {
            nfsu2::prof::trace::complete("frame: update", t, std::time::Duration::from_secs_f64(u / 1000.0), Some(uc));
            nfsu2::prof::trace::complete("frame: draw", t2, std::time::Duration::from_secs_f64(d / 1000.0), Some(dc));
            nfsu2::prof::trace::complete("frame: throttle", t3, std::time::Duration::from_secs_f64(g / 1000.0), None);
        }
        if bench_json.is_some()
            && i % 30 == 0
            && let Some(m) = nfsu2::prof::gpu_memory()
        {
            gpu_mem_peak = (gpu_mem_peak.0.max(m.0), gpu_mem_peak.1.max(m.1));
        }
        update.push(u);
        draw.push(d);
        gpu.push(g);
        // The performance overlay's frame figures (NFSU2_OVERLAY=full
        // times the overlay's own cost; docs/perf-overlay.md).
        if game.platform.perf_overlay.active(&game.settings.overlay) {
            let now = Instant::now();
            let interval = overlay_last.map_or(1.0 / 60.0, |l: Instant| now.duration_since(l).as_secs_f32());
            overlay_last = Some(now);
            let record = nfsu2::overlay::stats::FrameRecord {
                interval,
                sim: (u / 1000.0) as f32,
                // The default bench steps one 1/60 s tick a frame.
                ticks: if bench_hz.is_some() { u32::try_from(game.pacing.frame_updates).unwrap_or(u32::MAX) } else { 1 },
                dropped: if bench_hz.is_some() { game.pacing.frame_dropped } else { 0.0 },
                draw: (d / 1000.0) as f32,
                probes: game.render.streak.probes,
                ..Default::default()
            };
            game.platform.perf_overlay.push(record, &game.settings.overlay);
        }
        // NFSU2_BENCH_FIRST_FRAMES=n: the first n frames after the first,
        // each (a load's second after; GPU: the wait for the frame's work).
        if first_frames.is_some_and(|n| i <= n) {
            println!("  frame {i}: update {u:.1} ms, draw {d:.1} ms, GPU {g:.1} ms");
        }
        let built = nfsu2::render::pipelines_built();
        if built != pipelines_seen || switched == Some(i) {
            println!("  frame {i}: {} pipelines built (update {u:.1} ms, draw {d:.1} ms, GPU {g:.1} ms)", built - pipelines_seen);
            if built != pipelines_seen {
                pipeline_hitches += 1;
            }
            pipelines_seen = built;
        }
        let frame_parts = nfsu2::prof::acc::take();
        if let Some(log) = framelog.as_mut() {
            log.line(
                game,
                framelog::FrameRow {
                    frame: i,
                    update_ms: u,
                    update_cpu_ns: uc,
                    draw_ms: d,
                    draw_cpu_ns: dc,
                    throttle_ms: g,
                    ticks: if bench_hz.is_some() { game.pacing.frame_updates } else { 1 },
                    parts: &frame_parts,
                },
            );
        }
        for (label, ns, _) in frame_parts {
            match parts.iter_mut().find(|e| e.0 == label) {
                Some(e) => {
                    e.1 += ns;
                    e.2 = e.2.max(ns);
                }
                None => parts.push((label, ns, ns)),
            }
        }
        if let (Some(a), Some(dr)) = (before, game.sim.drive.as_ref()) {
            travelled += a.distance(dr.state.position);
            if let Some((s, _)) = dr.world.visible_sections_at(dr.camera.position) {
                sections_seen.insert(s);
            }
            // The chase view's eye under the car's centre of mass: the
            // world push flipped through the car (a Corolla drove with the
            // view under the road, looking back up at it).
            let com = dr.state.position + dr.state.orientation * Vec3::from(dr.car.car_type.handling.centre_of_mass);
            if matches!(dr.chase.kind(), 0 | 1 | 5) && dr.camera.position.z < com.z - 0.1 {
                low_eye.push(i);
            }
        }
        // NFSU2_BENCH_DUMP_FRAMES="a-b,c-d" (comparison aid): only the
        // frames in those ranges, each.
        let listed = dump_frames.as_ref().map(|r| r.iter().any(|&(a, b)| (a..=b).contains(&i)));
        if listed.unwrap_or(i % every == 0) {
            save(game, i)?;
        }
        ran = i;
    }
    if !matrix.is_empty() {
        println!("matrix: {matrix_done} of {} settings drawn", matrix.len());
    }
    println!("pipelines built after the first frame: {pipeline_hitches} frames");
    // The last frame's end, in the frame times too.
    bench_wait(game);
    whole.push(frame_start.elapsed().as_secs_f64() * 1000.0);
    nfsu2::prof::gpu::finish(&game.platform.window.as_ref().unwrap().gpu.device);
    // The allocator's live textures/buffers by label (measuring aid).
    if let Some(path) = env.vram_report.as_ref()
        && let Some(report) = nfsu2::prof::gpu::allocator_report(&game.platform.window.as_ref().unwrap().gpu.device)
    {
        let _ = std::fs::write(path, report);
    }
    let samples = Samples {
        ran, start_at, update, draw, gpu, whole, update_cpus, draw_cpus, update_cpu, draw_cpu, travelled, low_eye,
        sections_seen, parts, process_start, bench_json, threads_start, gpu_mem_peak, gpu_engine_start, audio_ms,
        audio_cpu, audio_voices, audio_sq, audio_n,
    };
    Ok((first, samples))
}

/// The summary: the JSON's series, the stats (mean, median, p95, p99, max),
/// the pops, the CPU and GPU lines.
fn report(game: &mut Game, what: &str, env: &BenchEnv, run: BenchRun, s: Samples) -> Result<()> {
    let BenchRun { width, height, load, first, timed, inflight } = run;
    let Samples {
        ran, start_at, mut update, mut draw, mut gpu, mut whole, mut update_cpus, mut draw_cpus, update_cpu, draw_cpu,
        travelled, low_eye, sections_seen, parts, process_start, bench_json, threads_start, mut gpu_mem_peak,
        gpu_engine_start, mut audio_ms, mut audio_cpu, audio_voices, audio_sq, audio_n,
    } = s;
    let mut gpu_frames = nfsu2::prof::gpu::take_frames();
    // The JSON's per-frame series, before the stats below sort them.
    let json: Option<String> = bench_json.as_ref().map(|_| {
        let series = |name: &str, v: &[f64]| format!("\"{name}\":[{}]", v.iter().map(|x| format!("{x:.4}")).collect::<Vec<_>>().join(","));
        let info = game.platform.window.as_ref().map(|w| w.gpu.adapter.get_info());
        let mem = nfsu2::prof::gpu_memory();
        if let Some(m) = mem {
            gpu_mem_peak = (gpu_mem_peak.0.max(m.0), gpu_mem_peak.1.max(m.1));
        }
        let threads_end = nfsu2::prof::threads();
        // Per thread name (the pool's workers share theirs): CPU ms/frame.
        let mut by_name: Vec<(String, f64, usize)> = Vec::new();
        // The main thread (its id is the process's) apart from the
        // helpers that inherit its name.
        let main = u64::from(std::process::id());
        for (tid, name, ns) in &threads_end {
            let name = if *tid == main { "main (game)".to_string() } else { name.clone() };
            let name = &name;
            let before = threads_start.iter().find(|t| t.0 == *tid).map_or(0, |t| t.2);
            let ms = ns.saturating_sub(before) as f64 / 1e6 / ran.max(1) as f64;
            match by_name.iter_mut().find(|e| &e.0 == name) {
                Some(e) => {
                    e.1 += ms;
                    e.2 += 1;
                }
                None => by_name.push((name.clone(), ms, 1)),
            }
        }
        by_name.sort_by(|a, b| b.1.total_cmp(&a.1));
        let threads = by_name.iter().map(|(n, ms, k)| format!("{{\"name\":\"{}\",\"ms_per_frame\":{ms:.4},\"threads\":{k}}}", nfsu2::prof::trace::escape(n))).collect::<Vec<_>>().join(",");
        let hwm = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmHWM")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())).unwrap_or(0);
        let engine = mem.map_or(0, |m| m.2).saturating_sub(gpu_engine_start);
        format!(
            "{{\"what\":\"{what}\",\"width\":{width},\"height\":{height},\"frames\":{ran},\"adapter\":\"{}\",\"driver\":\"{}\",\"enhancements\":\"{}\",\"load_ms\":{load:.2},\"first_frame_ms\":{first:.2},\"process_cpu_ms_per_frame\":{:.4},\"update_cpu_ms_per_frame\":{:.4},\"draw_cpu_ms_per_frame\":{:.4},\"vm_hwm_kib\":{hwm},\"vram_peak_kib\":{},\"gtt_peak_kib\":{},\"gfx_engine_ms_per_frame\":{:.4},\"inflight\":{inflight},\"travelled_m\":{travelled:.1},\"threads\":[{threads}],{},{},{},{},{},{},{}",
            nfsu2::prof::trace::escape(&info.as_ref().map_or(String::new(), |i| i.name.clone())),
            nfsu2::prof::trace::escape(&info.as_ref().map_or(String::new(), |i| format!("{} {}", i.driver, i.driver_info))),
            nfsu2::prof::trace::escape(&env.enh.clone().unwrap_or_default()),
            (process_cpu() - process_start) / ran.max(1) as f64,
            update_cpu as f64 / 1e6 / ran.max(1) as f64,
            draw_cpu as f64 / 1e6 / ran.max(1) as f64,
            gpu_mem_peak.0,
            gpu_mem_peak.1,
            engine as f64 / 1e6 / ran.max(1) as f64,
            series("update_ms", &update),
            series("update_cpu_ms", &update_cpus),
            series("draw_ms", &draw),
            series("draw_cpu_ms", &draw_cpus),
            series("throttle_ms", &gpu),
            series("frame_ms", &whole),
            series("gpu_busy_ms", &gpu_frames),
        )
    });
    let stats = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
        let at = |p: f64| v.get(((v.len().max(1) - 1) as f64 * p) as usize).copied().unwrap_or(0.0);
        format!(
            "mean {mean:6.2}  median {:6.2}  p95 {:6.2}  p99 {:6.2}  max {:6.2}",
            at(0.5),
            at(0.95),
            at(0.99),
            at(1.0)
        )
    };
    let traffic = match &game.mode {
        Mode::Race(r) => r.racers.traffic.as_ref().map_or(0, |t| t.cars.iter().filter(|c| c.lane.is_some()).count()),
        _ => game.sim.traffic.as_ref().map_or(0, |t| t.cars.iter().filter(|c| c.lane.is_some()).count()),
    };
    let rivals = if let Mode::Race(r) = &game.mode { r.racers.rivals.len() } else { 0 };
    let moved = start_at.zip(game.sim.drive.as_ref().map(|d| d.state.position)).map_or(0.0, |(a, b)| a.distance(b));
    println!(
        "bench {what}: load {load:.0} ms, first frame {first:.1} ms, {ran} frames, {traffic} traffic cars on the road, {rivals} rivals; the car drove {travelled:.0} m ({moved:.0} m from the start) through {} sections",
        sections_seen.len()
    );
    if let Some(d) = game.sim.drive.as_ref().filter(|d| d.weather.amount > 0.0) {
        println!("  rain {:.2} at clock {:.0} s", d.weather.amount, d.clock);
    }
    // NFSU2_POP_STATS=1 (measuring aid for LOD Fade / Keep All Loaded):
    // the main view's pops, and with NFSU2_POP_LOG=file each frame's.
    if nfsu2::render::pop_stats_enabled() {
        let p = nfsu2::render::take_pop_stats();
        let pops = p.appeared + p.vanished;
        println!(
            "  pops: {} frames sized, {} with a pop ({:.1}%); scenery appeared {} vanished {} at once ({:.2} a frame, worst {} in one, size sum {:.0}); sections entering the view {}; car LOD switches at once {}, cross-faded {}",
            p.frames,
            p.pop_frames,
            100.0 * p.pop_frames as f64 / p.frames.max(1) as f64,
            p.appeared,
            p.vanished,
            pops as f64 / p.frames.max(1) as f64,
            p.worst,
            p.popped_size,
            p.sections,
            p.car_pops,
            p.car_fades
        );
        if !p.block_distances.is_empty() {
            let mut d = p.block_distances.clone();
            d.sort_by(f32::total_cmp);
            println!(
                "  pops: blocks' nearest group in the view {:.0} m (min), {:.0} m (median), {} within 100 m",
                d[0],
                d[d.len() / 2],
                d.iter().filter(|&&x| x < 100.0).count()
            );
        }
        if let Some((lines, tsv)) = nfsu2::render::pop_meter::take_report() {
            for l in lines {
                println!("  {l}");
            }
            if let Some(path) = env.pop_meter_log.as_ref() {
                std::fs::write(&path, tsv)?;
            }
        }
        if let Some(path) = env.pop_log.as_ref() {
            let mut out = String::from("frame\tappeared\tvanished\tsections\tsize\tcar_pops\n");
            for (i, f) in p.per_frame.iter().enumerate() {
                out += &format!("{}\t{}\t{}\t{}\t{:.0}\t{}\n", i, f.0, f.1, f.2, f.3, f.4);
            }
            std::fs::write(&path, out)?;
        }
    }
    if !low_eye.is_empty() {
        println!(
            "  CAMERA: the chase eye under the car in {} frames (first {:?})",
            low_eye.len(),
            &low_eye[..low_eye.len().min(8)]
        );
    }
    let per_frame = |ns: u64| ns as f64 / 1e6 / ran.max(1) as f64;
    let over = |v: &[f64], limit: f64| v.iter().filter(|&&x| x > limit).count();
    println!("  process cpu (all threads) ms/frame: {:.2}", (process_cpu() - process_start) / ran.max(1) as f64);
    if let Some(post) = game.platform.window.as_ref().and_then(|w| w.post.as_ref()) {
        println!("  post passes: {}", post.fused_status().unwrap_or("(self-check still running)"));
    }
    println!(
        "  mode: {}",
        if inflight == 0 {
            "serial (the GPU waited for after every frame)".to_string()
        } else {
            format!(
                "pipelined, {inflight} frames in flight{}",
                if overlap_frames() { ", the frame's end overlapped with the next simulation" } else { "" }
            )
        }
    );
    println!("  update   ms/frame: {}  (cpu {:.2})", stats(&mut update), per_frame(update_cpu));
    println!("  update   cpu ms:   {}", stats(&mut update_cpus));
    println!("  draw     ms/frame: {}  (cpu {:.2})", stats(&mut draw), per_frame(draw_cpu));
    println!("  draw     cpu ms:   {}", stats(&mut draw_cpus));
    if !audio_ms.is_empty() {
        println!(
            "  audio    ms/frame: {}  (800 frames at 48 kHz a frame; up to {audio_voices} voices)",
            stats(&mut audio_ms)
        );
        println!("  audio    rms {:.5}", (audio_sq / audio_n.max(1) as f64).sqrt());
        println!("  audio    cpu ms:   {}", stats(&mut audio_cpu));
    }
    println!("  {} ms/frame: {}", if inflight == 0 { "gpu wait" } else { "throttle" }, stats(&mut gpu));
    if !gpu_frames.is_empty() {
        println!("  gpu busy ms/frame: {}  (timestamps, {} frames)", stats(&mut gpu_frames), gpu_frames.len());
    }
    if let Some(line) = game.streak_bench_summary() {
        println!("  {line}");
    }
    let (o16, o33) = (over(&whole, 1000.0 / 60.0), over(&whole, 1000.0 / 30.0));
    println!("  frame    ms/frame: {}  ({o16} over 16.7 ms, {o33} over 33.3 ms)", stats(&mut whole));
    report_parts(json, parts, ran, timed, bench_json)
}

/// The CPU and GPU time by part, the peak memory, the JSON written.
fn report_parts(
    mut json: Option<String>,
    parts: Vec<(&'static str, u64, u64)>,
    ran: usize,
    timed: bool,
    bench_json: Option<std::path::PathBuf>,
) -> Result<()> {
    nfsu2::prof::acc::enable(false);
    println!("  cpu by part, ms/frame (and the worst frame):");
    if let Some(j) = json.as_mut() {
        let p = parts
            .iter()
            .map(|(l, ns, w)| {
                format!(
                    "{{\"name\":\"{}\",\"ms_per_frame\":{:.4},\"worst_ms\":{:.4}}}",
                    nfsu2::prof::trace::escape(l),
                    *ns as f64 / 1e6 / ran.max(1) as f64,
                    *w as f64 / 1e6
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        j.push_str(&format!(",\"cpu_parts\":[{p}]"));
    }
    for (label, ns, worst) in parts {
        println!("    {label:40} {:6.2}  ({:6.2})", ns as f64 / 1e6 / ran.max(1) as f64, worst as f64 / 1e6);
    }
    if timed {
        let parts = nfsu2::prof::gpu::take();
        let total: f64 = parts.iter().filter(|p| p.0 != nfsu2::prof::gpu::GAP).map(|p| p.1).sum();
        if let Some(j) = json.as_mut() {
            let p = parts
                .iter()
                .map(|(l, ms, n)| {
                    format!(
                        "{{\"name\":\"{}\",\"ms_per_frame\":{:.5},\"frames\":{n}}}",
                        nfsu2::prof::trace::escape(l),
                        ms / ran.max(1) as f64
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            j.push_str(&format!(",\"gpu_parts\":[{p}]"));
        }
        println!("  gpu time by part, ms/frame (timestamps; total {:.2}):", total / ran.max(1) as f64);
        for (label, ms, n) in parts {
            println!("    {label:40} {:6.2}  (in {n} frames)", ms / ran.max(1) as f64);
        }
    }
    if let Some(peak) = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| {
        s.lines().find(|l| l.starts_with("VmHWM")).map(|l| l.split_whitespace().nth(1).unwrap_or("0").to_string())
    }) {
        println!("  peak memory (VmHWM): {} MB", peak.parse::<u64>().unwrap_or(0) / 1024);
        #[cfg(feature = "mimalloc")]
        println!("  allocator: {}", nfsu2::allocator::describe());
    }
    if let (Some(path), Some(mut j)) = (bench_json, json) {
        // The peak at the end too (as the text's "peak memory" line).
        let hwm = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmHWM"))
                    .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
            })
            .unwrap_or(0);
        j.push_str(&format!(",\"vm_hwm_end_kib\":{hwm}}}"));
        std::fs::write(&path, j).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

/// The pipelines and shader modules made so far and the time spent making
/// them (`device_cache`'s count and clock): a frame's builds are the
/// difference of two.
#[derive(Clone, Copy)]
pub(crate) struct Builds {
    pub(crate) pipelines: u64,
    pub(crate) modules: u64,
    pub(crate) module_ms: f64,
    pub(crate) pipeline_ms: f64,
}

impl Builds {
    pub(crate) fn now() -> Builds {
        let (module_ms, pipeline_ms) = nfsu2::render::core::device_cache::build_ms();
        Builds {
            pipelines: nfsu2::render::pipelines_built(),
            modules: nfsu2::render::core::device_cache::modules_built(),
            module_ms,
            pipeline_ms,
        }
    }

    pub(crate) fn since(&self, before: &Builds) -> String {
        format!(
            "{} pipelines, {} modules ({:.1} ms making them through the device cache)",
            self.pipelines - before.pipelines,
            self.modules - before.modules,
            (self.module_ms - before.module_ms) + (self.pipeline_ms - before.pipeline_ms)
        )
    }
}
