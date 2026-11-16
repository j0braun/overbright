use crate::photo::car_centre;
use crate::*;
use nfsu2::photo::{self as ph, Look, Row};

/// `--photo OUTDIR [explore|race]` (check, headless): free roam (or a
/// stage 1 circuit) driven for a while on an offscreen window
/// (NFSU2_PHOTO_SIZE, default 1280x960), paused, photo mode opened, the
/// camera moved by held keys and the mouse wheel, pictures taken at 2x, 4x
/// and 1x with depth of field, filters and exposure, the frames with the
/// help bar dumped, photo mode closed, the game resumed and driven on.
/// `NFSU2_PHOTO_CONTROL=1` runs the same steps and draws without photo
/// mode: with `NFSU2_STATE_HASH` set the two hash files must be identical
/// (the pause / resume leaves the simulation as it was).
/// `still`: the help-bar frame written there and the check ended (the
/// `--shot photo` target). `NFSU2_PHOTO_FORMAT` draws into another window
/// format (see below).
pub(crate) fn photo_check(game: &mut Game, out: &str, scene: &str, still: Option<&str>) -> Result<()> {
    let control = std::env::var("NFSU2_PHOTO_CONTROL").is_ok_and(|v| v == "1");
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out)?;
    let (width, height) = env_size("NFSU2_PHOTO_SIZE").unwrap_or((1280, 960));
    headless_aspect(game);
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    // NFSU2_PHOTO_FORMAT=rgba8|rgb10a2|rgba16float: the window's format
    // (a swap chain other than the usual 8-bit BGRA); default bgra8.
    let format = match std::env::var("NFSU2_PHOTO_FORMAT").as_deref() {
        Ok("rgba8") => wgpu::TextureFormat::Rgba8Unorm,
        Ok("rgb10a2") => wgpu::TextureFormat::Rgb10a2Unorm,
        Ok("rgba16float") => wgpu::TextureFormat::Rgba16Float,
        Ok("bgra8") | Err(_) => wgpu::TextureFormat::Bgra8Unorm,
        Ok(other) => anyhow::bail!("NFSU2_PHOTO_FORMAT={other}: rgba8, rgb10a2, rgba16float or bgra8"),
    };
    let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("photo check target"),
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
    if game.profile.is_none() {
        let mut p = Profile::new("Player");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        game.profile = Some(p);
    }
    // NFSU2_PHOTO_ENH: Enhancements lines for the run (as NFSU2_BENCH_ENH).
    game.settings.enh.photo_mode = true;
    if let Ok(lines) = std::env::var("NFSU2_PHOTO_ENH") {
        for line in lines.split(';') {
            let (k, v) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
            game.settings.enh.parse(k, v);
        }
    }
    game.settings.apply_enhancements();
    game.pacing.clock = Some(0.0);
    const DT: f32 = 1.0 / 60.0;
    let step = |game: &mut Game| -> Result<()> {
        game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(DT));
        game.step_for(DT)
    };
    let dump = |game: &mut Game, name: &str| -> Result<()> {
        let w = game.platform.window.as_mut().unwrap();
        w.settle();
        let mut px = nfsu2::render::read_back_rgba8(&w.gpu, w.offscreen.as_ref().unwrap())?;
        for p in px.chunks_exact_mut(4) {
            p[3] = 255;
        }
        image::save_buffer(out.join(name), &px, width, height, image::ColorType::Rgba8)?;
        Ok(())
    };
    let race = scene == "race";
    if race {
        let e = game
            .game_data.career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| e.kind == career::EventKind::Circuit)
            .map(|e| e.index)
            .context("a circuit")?;
        game.start_race(e, false, false)?;
    } else {
        game.explore(None)?;
    }
    game.platform.keys.insert(KeyCode::KeyW);
    for i in 0..900 {
        step(game)?;
        if i % 30 == 0 || i >= 890 {
            game.draw()?;
        }
    }
    game.platform.keys.clear();
    // The pause (UI_pause over the world).
    let kind = if race { MenuKind::RacePause } else { MenuKind::ExplorePause };
    if race {
        let Mode::Race(r) = std::mem::replace(&mut game.mode, Mode::Explore) else { anyhow::bail!("the race ended") };
        game.sim.paused_race = Some(r);
    }
    game.mode = game.icon_menu(kind);
    let listed =
        matches!(&game.mode, Mode::Menu(Menu::Icons(m, _)) if m.options.iter().any(|o| o.action == Action::PhotoMode));
    anyhow::ensure!(listed, "the pause menu lists Photo Mode with the row on");
    for _ in 0..10 {
        step(game)?;
        game.draw()?;
    }
    dump(game, &format!("{scene}-pause.png"))?;
    if !control {
        game.act(Action::PhotoMode, kind)?;
        anyhow::ensure!(game.photo.is_some(), "photo mode opened");
    }
    let before = state_hash::components(game);
    // Held keys: forward, orbit right, up; the wheel nearer.
    for (keys, n) in [
        (&[KeyCode::KeyJ][..], 50),
        (&[KeyCode::KeyK][..], 6),
        (&[KeyCode::BracketRight][..], 20),
        (&[KeyCode::KeyD][..], 5),
    ] {
        for _ in 0..n {
            for k in keys {
                game.platform.keys.insert(*k);
            }
            step(game)?;
            game.draw()?;
        }
        game.platform.keys.clear();
    }
    game.platform.mouse.wheel = 1.0;
    step(game)?;
    game.draw()?;
    let mut report = Vec::new();
    if !control {
        let key = |game: &mut Game, code: KeyCode, n: usize| -> Result<()> {
            for _ in 0..n {
                game.photo_key(code)?;
            }
            Ok(())
        };
        // The strip: Aperture f/2.8, focus Auto.
        let p = game.photo.as_mut().unwrap();
        p.selected = Row::ALL.iter().position(|r| *r == Row::Aperture).unwrap();
        key(game, KeyCode::ArrowRight, 3)?;
        step(game)?;
        game.draw()?;
        dump(game, &format!("{scene}-photo-help.png"))?;
        if let Some(still) = still {
            std::fs::copy(out.join(format!("{scene}-photo-help.png")), still)?;
            return Ok(());
        }
        let p = game.photo.as_ref().unwrap();
        let (c, d) = (game.photo_camera().unwrap(), game.sim.drive.as_ref().unwrap());
        report.push(format!(
            "eye {:.2?} car {:.2?} distance {:.2} m (limit {})",
            c.position,
            d.state.position,
            c.position.distance(car_centre(d)),
            ph::MAX_RADIUS
        ));
        report.push(format!("look {:?}", p.look));
        let shot = |game: &mut Game, report: &mut Vec<String>| -> Result<()> {
            let size = game.photo_capture()?.context("a picture")?;
            report.push(format!("capture {}x{} look {:?}", size.0, size.1, game.photo.as_ref().unwrap().look));
            let k = game.photo.as_ref().unwrap().look.capture;
            anyhow::ensure!(size == (width * k, height * k), "capture size {size:?} for {k}x");
            Ok(())
        };
        shot(game, &mut report)?; // 2x, f/2.8
        let p = game.photo.as_mut().unwrap();
        p.look.filter = ph::Filter::Sepia;
        p.look.exposure = 0.5;
        p.look.bleach = 40;
        p.look.capture = 4;
        p.look.focus = 3.0;
        p.look.aperture = 1.4;
        shot(game, &mut report)?;
        let p = game.photo.as_mut().unwrap();
        p.look = Look {
            filter: ph::Filter::BlackWhite,
            capture: 1,
            car_shadow: false,
            light_glow: false,
            speed_blur: false,
            ..Look::default()
        };
        p.rig.roll = 15.0;
        p.rig.fov = 35.0;
        shot(game, &mut report)?;
        // Dollied far out and pushed down: held by the radius and the ground.
        let p = game.photo.as_mut().unwrap();
        p.rig.distance = ph::MAX_DISTANCE;
        p.rig.pitch = -ph::MAX_PITCH;
        p.rig.pan = Vec3::new(30.0, 0.0, -5.0);
        let (c, d) = (game.photo_camera().unwrap(), game.sim.drive.as_ref().unwrap());
        let ground = d.world.collision().ground_below(c.position + Vec3::Z * 0.01).map(|g| g.0);
        report.push(format!(
            "held: eye {:.2?}, {:.2} m from the car, ground below {:?}",
            c.position,
            c.position.distance(car_centre(d)),
            ground
        ));
        anyhow::ensure!(c.position.distance(car_centre(d)) <= ph::MAX_RADIUS + 0.01, "the radius");
        step(game)?;
        game.draw()?;
        dump(game, &format!("{scene}-photo-held.png"))?;
        game.photo_flush();
        let mut written: Vec<std::path::PathBuf> =
            std::fs::read_dir(&game.photo.as_ref().unwrap().dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        written.sort();
        for path in written {
            let name = path.file_name().unwrap().to_owned();
            let (i, size) = (report.len(), image::image_dimensions(&path)?);
            let to = out.join(format!("{scene}-capture-{i}-{}x{}.png", size.0, size.1));
            std::fs::copy(&path, &to)?;
            report.push(format!("wrote {} → {}", name.to_string_lossy(), to.display()));
        }
        game.photo_key(KeyCode::Escape)?;
        anyhow::ensure!(game.photo.is_none(), "Esc closes photo mode");
    } else {
        // the same steps and draws as the photo run
        for _ in 0..2 {
            step(game)?;
            game.draw()?;
        }
    }
    let after = state_hash::components(game);
    report.push(format!("state before photo {before:016x?}"));
    report.push(format!("state after photo  {after:016x?}"));
    step(game)?;
    game.draw()?;
    dump(game, &format!("{scene}-pause-after.png"))?;
    // Resume (Back on UI_pause) and drive on.
    game.menu_key(Key::Back)?;
    report.push(format!(
        "resumed: {}",
        match &game.mode {
            Mode::Explore => "explore",
            Mode::Race(_) => "race",
            Mode::Menu(_) => "menu",
        }
    ));
    game.platform.keys.insert(KeyCode::KeyW);
    for _ in 0..300 {
        step(game)?;
    }
    report.push(format!("state after 300 steps {:016x?}", state_hash::components(game)));
    for l in &report {
        println!("{l}");
    }
    std::fs::write(
        out.join(format!("{scene}-{}.txt", if control { "control" } else { "photo" })),
        report.join("\n") + "\n",
    )?;
    Ok(())
}
