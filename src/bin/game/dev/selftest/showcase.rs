use crate::*;

/// Uses the diagnostic's isolated data directory; never opens an existing profile.
pub(crate) fn selftest_showcase_picture(game: &mut Game) -> Result<()> {
    game.pacing.clock = Some(0.0);
    let mut profile = Profile::new("PHOTO_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    let first = game.game_data.career.milestones.iter().position(|m| m.kind == 1).context("kind-1 shoot")?;
    game.mode = Mode::Menu(Menu::Showcase(game.showcase_preview(first, false, 0.0)));
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(_, choices)) if !matches!(&choices[0], Mode::Explore) && matches!(&choices[1], Mode::Explore)),
        "showcase Yes must capture/save; only No may directly leave"
    );
    // Real world framebuffer, through the same draw and save path as the window.
    let (width, height) = env_size("NFSU2_BENCH_SIZE").unwrap_or((640, 480));
    headless_aspect(game);
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("showcase regression"),
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
    game.platform.window = Some(WindowState::new(None, None, Some(target), config, gpu, depth, ui));
    game.mode = Mode::Explore;
    game.explore(None)?;
    let artifact = std::env::var_os("NFSU2_SHOWCASE_EVIDENCE").map(std::path::PathBuf::from);
    if let Some(dir) = &artifact {
        std::fs::create_dir_all(dir)?;
    }
    let tick = |game: &mut Game| -> Result<()> {
        game.pacing.clock = Some(game.pacing.clock.unwrap_or(0.0) + 1.0 / 60.0);
        game.step_for(1.0 / 60.0)?;
        game.draw()
    };
    for kind in [1, 2] {
        let milestone = game.game_data.career.milestones.iter().position(|m| m.kind == kind).context("shoot kind")?;
        let dvd = kind == 2;
        let file = profiles::cover_name("PHOTO_TEST", dvd, game.cover_number(milestone, dvd));
        let preview = game.showcase_preview(milestone, dvd, game.now());
        game.mode = Mode::Menu(Menu::Showcase(preview.clone()));
        game.menu_key(Key::Accept)?; // shutter
        game.menu_key(Key::Back)?; // retake
        anyhow::ensure!(matches!(&game.mode,Mode::Menu(Menu::Showcase(v)) if !v.picture && v.save_ticks.is_none()));
        game.menu_key(Key::Accept)?;
        game.menu_key(Key::Accept)?; // question
        if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
            d.selected = 1;
        }
        game.menu_key(Key::Accept)?; // No
        anyhow::ensure!(
            matches!(game.mode, Mode::Explore) && profiles::read_cover(&file).is_err(),
            "No must not write a picture"
        );
        game.mode = Mode::Menu(Menu::Showcase(preview));
        game.menu_key(Key::Accept)?;
        // Let the real picture-state overlays become visible before accepting.
        for _ in 0..20 {
            tick(game)?;
        }
        game.menu_key(Key::Accept)?;
        if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
            d.selected = 0;
        }
        game.menu_key(Key::Accept)?;
        let position = game.sim.drive.as_ref().context("showcase world")?.state.position;
        game.menu_key(Key::Back)?;
        for _ in 0..4 {
            tick(game)?;
            anyhow::ensure!(
                matches!(&game.mode, Mode::Menu(Menu::Showcase(_))) && profiles::read_cover(&file).is_err(),
                "capture must wait for fifth update tick"
            );
        }
        tick(game)?;
        anyhow::ensure!(game.sim.drive.as_ref().unwrap().state.position == position, "capture wait must freeze driving");
        let expected_message = game.mc_text(profiles::mc_text::SAVE_DONE, "PHOTO_TEST");
        anyhow::ensure!(
            matches!(&game.mode,Mode::Menu(Menu::McMessage(m,_)) if m.text==expected_message),
            "save complete after actual framebuffer capture"
        );
        let saved = profiles::read_cover(&file)?;
        let w = game.platform.window.as_mut().unwrap();
        w.settle();
        let framebuffer = nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), width, height);
        let (crop, cw, ch) =
            screens::showcase::capture_rgba(&framebuffer, (width, height), true).context("framebuffer crop")?;
        let expected = nfsu2::formats::native_save::read_picture(&nfsu2::formats::native_save::picture(&crop, cw, ch))?;
        anyhow::ensure!(saved == expected, "saved picture must be the presented world+overlay crop");
        anyhow::ensure!(saved.chunks_exact(4).any(|p| p[..3] != saved[..3]), "real scene must not be a solid stand-in");
        if let Some(dir) = &artifact {
            image::save_buffer(
                dir.join(format!("kind{kind}-{width}x{height}-saved.png")),
                &saved,
                512,
                512,
                image::ColorType::Rgba8,
            )?;
            image::save_buffer(
                dir.join(format!("kind{kind}-{width}x{height}-crop.png")),
                &crop,
                cw,
                ch,
                image::ColorType::Rgba8,
            )?;
        }
        game.mode = game.open_cover(milestone, dvd);
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(
            matches!(&game.mode,Mode::Menu(Menu::Cover(v)) if v.picture && v.camera.is_none()),
            "saved-view reload must use file pixels"
        );
        game.draw()?;
        println!(
            "showcase kind{kind} {width}x{height}: No/retake, five ticks, real capture, native save and saved-view reload pass"
        );
    }
    // A write failure must be reported, not silently turned into free roam.
    // This blocker is under the diagnostic's unique temporary profile directory.
    let file = profiles::cover_name("PHOTO_TEST", false, game.cover_number(first, false));
    let path = profiles::file(&file);
    let retry_picture = profiles::read_cover(&file)?;
    std::fs::remove_file(&path)?;
    std::fs::create_dir(&path)?;
    let failed = game.save_picture(first, false, Some((&retry_picture, 512, 512)));
    let text = game.mc_text(profiles::mc_text::SAVE_FAILED, "PHOTO_TEST");
    anyhow::ensure!(matches!(failed,Mode::Menu(Menu::McMessage(m,_)) if m.text==text), "write failure must be visible");
    std::fs::remove_dir(&path)?;
    let retried = game.save_picture(first, false, Some((&retry_picture, 512, 512)));
    let text = game.mc_text(profiles::mc_text::SAVE_DONE, "PHOTO_TEST");
    anyhow::ensure!(
        matches!(retried,Mode::Menu(Menu::McMessage(m,_)) if m.text==text)
            && profiles::read_cover(&file)? == retry_picture,
        "retry same real picture after write failure"
    );
    // The existing kind 3/4 cover render and dialog writer remain separate.
    for kind in [3, 4] {
        let milestone = game.game_data.career.milestones.iter().position(|m| m.kind == kind).context("reward kind")?;
        game.mode = game.magazine_reward(milestone);
        let old_shadow = game.settings.custom[2];
        game.settings.custom[2] = 1;
        game.draw()?;
        let on = game.showcase.cover_car.as_ref().and_then(|c| c.pixels.clone()).context("reward shadow on")?;
        game.settings.custom[2] = 0;
        game.showcase.cover_car.as_mut().unwrap().drawn = false;
        game.draw()?;
        let off = game.showcase.cover_car.as_ref().and_then(|c| c.pixels.clone()).context("reward shadow off")?;
        let changed = on.0.chunks_exact(4).zip(off.0.chunks_exact(4)).filter(|(a, b)| a != b).count();
        anyhow::ensure!(changed > 0, "reward kind{kind} must include its FE shadow");
        game.settings.custom[2] = 1;
        game.showcase.cover_car.as_mut().unwrap().drawn = false;
        game.draw()?;
        anyhow::ensure!(
            game.showcase.cover_car.as_ref().and_then(|c| c.pixels.as_ref()) == Some(&on),
            "reward shadow restore must be exact"
        );
        game.settings.custom[2] = old_shadow;
        if let Some(dir) = &artifact {
            image::save_buffer(
                dir.join(format!("reward-kind{kind}-shadow-off.png")),
                &off.0,
                off.1,
                off.2,
                image::ColorType::Rgba8,
            )?;
        }
        println!("reward kind{kind}: shadow changes {changed} pixels; exact restoration");
        let shot = game.showcase.cover_car.as_ref().and_then(|c| c.pixels.as_ref()).context("existing reward cover capture")?;
        let expected =
            nfsu2::formats::native_save::read_picture(&nfsu2::formats::native_save::picture(&shot.0, shot.1, shot.2))?;
        if let Some(dir) = &artifact {
            image::save_buffer(
                dir.join(format!("reward-kind{kind}-crop.png")),
                &shot.0,
                shot.1,
                shot.2,
                image::ColorType::Rgba8,
            )?;
        }
        game.menu_key(Key::Accept)?;
        if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
            d.selected = 0;
        }
        game.menu_key(Key::Accept)?;
        let file = profiles::cover_name("PHOTO_TEST", kind == 4, game.cover_number(milestone, kind == 4));
        anyhow::ensure!(profiles::read_cover(&file)? == expected, "existing reward kind{kind} save changed");
    }
    println!("write failure/retry and existing kind3/4 reward saves pass");
    Ok(())
}
