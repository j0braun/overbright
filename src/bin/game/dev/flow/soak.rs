use crate::*;

/// `--flow soak`: a long headless session as a player plays it, for the
/// audio (run with `NFSU2_AUDIO_WAV=- NFSU2_AUDIO_METER=10`). The phases
/// come from `NFSU2_SOAK` (comma-separated): `menu:s` the main menu for s
/// seconds (a key every 7 s), `career` the career menu's first option and
/// the D-Day intro as far as the car is free (Enter on the SMS screens),
/// `career_esc` the same up to the start scene, then Esc and Quit to Main
/// Menu, `drive:s` the throttle held and the steering swung, `race` a
/// stage-1 circuit run 3 s and finished (its ending scene and the
/// post-race screens), `quit` Esc and Quit to Main Menu (or from the
/// post-race screens), `profile` a fresh profile past the intro, `explore` free
/// roam from there, `master:x` the master volume from then on.
pub(crate) fn soak(game: &mut Game) -> Result<()> {
    use super::common::flow_tap as tap;
    // `NFSU2_SOAK_FPS=n`: the frames' rate (default 60; a check aid for
    // the display's rate, e.g. 1000 as an uncapped run gives). `each` still
    // sees 60 steps a second (i counts 1/60 s).
    fn run(game: &mut Game, secs: f32, mut each: impl FnMut(&mut Game, u32) -> Result<()>) -> Result<()> {
        let fps: f32 = std::env::var("NFSU2_SOAK_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(60.0);
        let per = (fps / 60.0).round().max(1.0) as u32;
        for i in 0..(secs * 60.0) as u32 {
            each(game, i)?;
            for _ in 0..per {
                game.advance_for(1.0 / (60.0 * per as f32))?;
                if game.platform.window.is_some() {
                    game.preload_frame();
                    // The tick's front-end steps (`step` runs them).
                    game.fe_tick_before_sounds();
                    game.fe_tick();
                    game.draw()?;
                }
            }
        }
        Ok(())
    }
    fn to_main(game: &mut Game) -> Result<()> {
        // Esc to the pause (explore / race), or the post-race screens; then
        // Quit to Main Menu and OK.
        if matches!(game.mode, Mode::Explore | Mode::Race(_)) {
            tap(game, KeyCode::Escape)?;
        }
        let kind = match &game.mode {
            Mode::Menu(Menu::Icons(_, k)) => *k,
            _ => MenuKind::ExplorePause,
        };
        game.act(Action::QuitToMainMenu, kind)?;
        tap(game, KeyCode::ArrowLeft)?;
        tap(game, KeyCode::Enter)
    }
    fn career(game: &mut Game, esc_in_scene: bool) -> Result<()> {
        if !matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
            game.mode = game.icon_menu(MenuKind::Main);
        }
        tap(game, KeyCode::ArrowLeft)?;
        tap(game, KeyCode::Enter)?;
        tap(game, KeyCode::Enter)?;
        for _ in 0..8 {
            match &game.mode {
                Mode::Menu(Menu::McMain(m)) if m.step == McStep::Ask || m.step == McStep::Confirm => {
                    tap(game, KeyCode::ArrowLeft)?;
                    tap(game, KeyCode::Enter)?;
                }
                Mode::Menu(Menu::McMain(_) | Menu::McMessage(..) | Menu::Dialog(..)) => tap(game, KeyCode::Enter)?,
                _ => break,
            }
        }
        if matches!(game.mode, Mode::Menu(Menu::Movie(_))) {
            run(game, 1.0, |_, _| Ok(()))?;
            tap(game, KeyCode::Enter)?;
            run(game, 0.5, |_, _| Ok(()))?;
        }
        if game.scenes.shop.is_some() {
            run(game, 2.0, |_, _| Ok(()))?;
            if esc_in_scene {
                return to_main(game);
            }
            tap(game, KeyCode::Enter)?;
        }
        // The D-Day script: Enter on its SMS screens, up to 30 s.
        let mut t = 0;
        while game.career_world.dday.running() && t < 30 * 60 {
            if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) && t % 60 == 59 {
                tap(game, KeyCode::Enter)?;
            }
            game.advance_for(1.0 / 60.0)?;
            t += 1;
        }
        println!("soak: career in {} (dday {})", flow_mode(&game.mode), game.career_world.dday.state);
        Ok(())
    }
    // NFSU2_SOAK_WINDOW=1: an offscreen 640x480 window, the frames drawn
    // and the menus' preload run as a windowed game's (a check aid: the
    // preloaded free roam, as a player gets it).
    if std::env::var_os("NFSU2_SOAK_WINDOW").is_some() {
        let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
        let (width, height, format) = (640, 480, wgpu::TextureFormat::Bgra8Unorm);
        let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("soak target"),
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
    let phases = std::env::var("NFSU2_SOAK").unwrap_or_else(|_| "menu:420,career,drive:420,quit,menu:300".into());
    if let Ok(name) = std::env::var("NFSU2_FLOW_LOAD") {
        game.mc_act(McAct::Load(name))?;
        game.menu_key(Key::Accept)?;
    }
    game.mode = game.icon_menu(MenuKind::Main);
    let mut master_set = None;
    for phase in phases.split(',') {
        let (name, arg) = phase.split_once(':').unwrap_or((phase, "0"));
        let secs: f32 = arg.parse().unwrap_or(0.0);
        soak_volumes(game)?;
        if name == "master" {
            master_set = Some(secs.clamp(0.0, 1.0));
        }
        if let Some(m) = master_set {
            game.settings.master = m;
        }
        println!("soak: {phase} (from {})", flow_mode(&game.mode));
        match name {
            "menu" => {
                if !matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
                    game.mode = game.icon_menu(MenuKind::Main);
                }
                run(game, secs, |g, i| {
                    if i % 420 == 419 {
                        g.menu_key(if (i / 420) % 2 == 0 { Key::Right } else { Key::Left })?;
                    }
                    Ok(())
                })?
            }
            "career" => career(game, false)?,
            "career_esc" => career(game, true)?,
            "drive" => {
                game.platform.keys.insert(KeyCode::ArrowUp);
                run(game, secs, |g, i| {
                    if matches!(g.mode, Mode::Menu(_)) && i % 60 == 59 {
                        g.menu_key(Key::Accept)?;
                    }
                    let steer =
                        [None, Some(KeyCode::ArrowLeft), None, Some(KeyCode::ArrowRight)][(i / 120 % 4) as usize];
                    g.platform.keys.remove(&KeyCode::ArrowLeft);
                    g.platform.keys.remove(&KeyCode::ArrowRight);
                    if let Some(k) = steer {
                        g.platform.keys.insert(k);
                    }
                    Ok(())
                })?;
                game.platform.keys.clear();
            }
            "race" => {
                let e = game
                    .game_data.career
                    .stage_events(&game.game_data.root, 1)
                    .iter()
                    .find(|e| e.kind == career::EventKind::Circuit)
                    .map(|e| e.index)
                    .context("a circuit")?;
                game.start_race(e, false, false)?;
                run(game, 12.0, |g, _| {
                    g.platform.keys.insert(KeyCode::ArrowUp);
                    Ok(())
                })?;
                if let Mode::Race(r) = &mut game.mode {
                    r.racers.me.finished = Some(r.timing.clock.max(0.0));
                }
                game.platform.keys.clear();
                run(game, 10.0, |_, _| Ok(()))?;
            }
            "quit" => to_main(game)?,
            // A check aid for sound comparisons: `NFSU2_SOAK_KEYS` =
            // "t key[+key..] [hold];..." (seconds from the phase start; key
            // names as `soak_key`; hold in seconds, else a tap), run for
            // `secs`; each action is logged with the capture's time.
            "keys" => {
                let script = std::env::var("NFSU2_SOAK_KEYS").unwrap_or_default();
                let mut steps: Vec<(f32, Vec<KeyCode>, f32)> = Vec::new();
                for s in script.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                    let f: Vec<&str> = s.split_whitespace().collect();
                    let t: f32 = f.first().and_then(|v| v.parse().ok()).context("soak keys: a time")?;
                    let keys = f.get(1).map_or(Vec::new(), |k| k.split('+').filter_map(soak_key).collect());
                    let hold: f32 = f.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    steps.push((t, keys, hold));
                }
                let mut held: Vec<(f32, Vec<KeyCode>)> = Vec::new();
                let mut next = 0;
                run(game, secs, |g, i| {
                    let now = i as f32 / 60.0;
                    if i % 15 == 0 {
                        if let Some(d) = g.sim.drive.as_ref() {
                            let cap = AUDIO_CAPTURE
                                .lock()
                                .unwrap()
                                .as_ref()
                                .map_or(0.0, |c| c.0.len() as f64 / 2.0 / f64::from(audio_capture_rate()));
                            let gear = d.state.drivetrain.as_ref().map_or(0, |t| t.gear);
                            let (map, out) = g
                                .audio
                                .as_ref()
                                .and_then(|a| a.engine.as_ref())
                                .map_or((0.0, [0; 4]), |(_, h, _)| (h.rpm_map, h.out));
                            let (lv0, sp) =
                                g.audio.as_ref().map_or((0, [0; 2]), |a| (a.levels.level(0, 0), a.speech_fields));
                            println!(
                                "soak car {now:.2} cap {cap:.3} rpm {:.0} gear {gear} speed {:.2} map {map:.0} lv0 {lv0} speech {sp:?} out {out:?}",
                                d.state.rpm(),
                                d.state.speed()
                            );
                        }
                    }
                    held.retain(|(until, keys)| {
                        if now >= *until {
                            keys.iter().for_each(|k| {
                                g.platform.keys.remove(k);
                            });
                            false
                        } else {
                            true
                        }
                    });
                    while next < steps.len() && steps[next].0 <= now {
                        let (t, keys, hold) = steps[next].clone();
                        let cap = AUDIO_CAPTURE
                            .lock()
                            .unwrap()
                            .as_ref()
                            .map_or(0.0, |c| c.0.len() as f64 / 2.0 / f64::from(audio_capture_rate()));
                        println!("soak key {t:.3} cap {cap:.3} {:?} hold {hold} in {}", keys, flow_mode(&g.mode));
                        for k in &keys {
                            g.key(*k, &mut || {})?;
                            g.platform.keys.insert(*k);
                        }
                        held.push((now + hold.max(1.0 / 60.0), keys));
                        next += 1;
                    }
                    Ok(())
                })?;
                game.platform.keys.clear();
            }
            // A stage-1 circuit paused 3 s in, then Return to Explore Mode.
            "race_explore" => {
                let e = game
                    .game_data.career
                    .stage_events(&game.game_data.root, 1)
                    .iter()
                    .find(|e| e.kind == career::EventKind::Circuit)
                    .map(|e| e.index)
                    .context("a circuit")?;
                game.start_race(e, false, false)?;
                run(game, 3.0, |_, _| Ok(()))?;
                tap(game, KeyCode::Escape)?;
                game.act(Action::ReturnToExplore, MenuKind::RacePause)?;
                if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
                    tap(game, KeyCode::ArrowLeft)?;
                    tap(game, KeyCode::Enter)?;
                }
            }
            // Free roam from where the career stands (`Game::explore`), as
            // `--flow engine` enters it; a check aid for sound comparisons.
            "explore" => game.explore(None)?,
            // The master volume changed mid-run (as the options' slider
            // does), kept by the next phases; a check aid.
            "master" => {}
            // From free roam's pause, Jump to Garage, s seconds there.
            "garage" => {
                tap(game, KeyCode::Escape)?;
                game.act(Action::JumpToGarage, MenuKind::ExplorePause)?;
                if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
                    tap(game, KeyCode::ArrowLeft)?;
                    tap(game, KeyCode::Enter)?;
                }
                run(game, secs, |_, _| Ok(()))?;
            }
            "profile" => {
                let mut p = Profile::new("SOAK");
                p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
                game.profile = Some(p);
            }
            _ => anyhow::bail!("unknown soak phase {phase}"),
        }
        println!("soak: {phase} done in {}", flow_mode(&game.mode));
    }
    Ok(())
}

/// `--flow soak`'s NFSU2_SOAK_VOL="menu_music 0;game_music 0": option
/// volumes for the run, set again at each phase (a profile made or loaded
/// brings its own); a check aid for sound comparisons.
pub(crate) fn soak_volumes(game: &mut Game) -> Result<()> {
    if let Ok(v) = std::env::var("NFSU2_SOAK_VOL") {
        for kv in v.split(';') {
            let (k, x) = kv.trim().split_once(' ').unwrap_or((kv.trim(), "1"));
            let x: f32 = x.parse().unwrap_or(1.0);
            let s = &mut game.settings;
            match k {
                "master" => s.master = x,
                "speech" => s.speech = x,
                "menu_music" => s.menu_music = x,
                "game_music" => s.game_music = x,
                "effects" => s.effects = x,
                "engine" => s.engine = x,
                "tires" => s.tires = x,
                _ => anyhow::bail!("NFSU2_SOAK_VOL: unknown {k}"),
            }
        }
    }
    Ok(())
}

/// `--flow soak`'s key names (the `keys` phase).
pub(crate) fn soak_key(k: &str) -> Option<KeyCode> {
    Some(match k {
        "up" => KeyCode::ArrowUp,
        "down" => KeyCode::ArrowDown,
        "left" => KeyCode::ArrowLeft,
        "right" => KeyCode::ArrowRight,
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Escape,
        "space" => KeyCode::Space,
        "nos" => KeyCode::AltLeft,
        "h" => KeyCode::KeyH,
        "p" => KeyCode::KeyP,
        "m" => KeyCode::KeyM,
        _ => return None,
    })
}
