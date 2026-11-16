use crate::*;

/// The pause's Options: UI_PauseOptionsMain, each UI_PauseOptions
/// category, UI_PC_Customize_Options from Display and back, then back to
/// the pause; and UI_Wheel_Options from the front end's Player options.
pub(crate) fn selftest_pause_options(game: &mut Game) -> Result<()> {
    game.act(Action::Options, MenuKind::ExplorePause)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PauseOptions(false)))),
        "UI_PauseOptionsMain"
    );
    for n in 0..4 {
        game.act(Action::PauseSetting(n), MenuKind::PauseOptions(false))?;
        let Mode::Menu(Menu::Options(o)) = &game.mode else { anyhow::bail!("UI_PauseOptions {n}") };
        anyhow::ensure!(o.pause.is_some(), "a pause's options");
        let rows = options::rows_in(o.category, &game.settings, 0, FORCE_FEEDBACK, o.pause);
        println!("pause options {:?}: {} rows", o.category, rows.len());
        if n == 0 {
            // Sound round 7: the list base 0x556030 on the first volume
            // slider (row 0); Right at the maximum is silent, Left plays
            // [LEFT][SLIDER] (8), Up at the top still plays [UP][7] (0).
            let at_max =
                matches!(rows.first().map(|r| r.kind), Some(options::Kind::Slider { value, max, .. }) if value >= max);
            let sounds = (
                game.native_menu_sound(Key::Right),
                game.native_menu_sound(Key::Left),
                game.native_menu_sound(Key::Up),
                game.native_menu_sound(Key::LTrigger),
            );
            println!(
                "fe sounds on {:?} (slider at max {at_max}): Right {:?}, Left {:?}, Up {:?}, LTrigger {:?}",
                o.category, sounds.0, sounds.1, sounds.2, sounds.3
            );
            anyhow::ensure!(
                sounds.1 == Some(Some(8)) && sounds.2 == Some(Some(0)) && sounds.3 == Some(None),
                "the options list's sounds"
            );
            anyhow::ensure!(!at_max || sounds.0 == Some(None), "Right on a slider at its maximum is silent");
        }
        if n == 1 {
            // Display: Customize Display Settings (row 2) and back.
            game.menu_key(Key::Down)?;
            game.menu_key(Key::Down)?;
            game.menu_key(Key::Accept)?;
            anyhow::ensure!(
                matches!(&game.mode, Mode::Menu(Menu::Options(c)) if c.category == options::Category::CustomDisplay && c.pause.is_some()),
                "UI_PC_Customize_Options"
            );
            game.menu_key(Key::Back)?;
            anyhow::ensure!(
                matches!(&game.mode, Mode::Menu(Menu::Options(c)) if c.category == options::Category::Display && c.pause.is_some()),
                "back on the pause's Display"
            );
        }
        game.menu_key(Key::Back)?;
        // Gameplay may ask about the save settings; none changed here.
        anyhow::ensure!(
            matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PauseOptions(false)))),
            "back to UI_PauseOptionsMain"
        );
    }
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))), "back to the pause");
    // PAD_START (comma) on the pause's options resumes the game; not on
    // UI_PC_Customize_Options.
    game.act(Action::Options, MenuKind::ExplorePause)?;
    game.act(Action::PauseSetting(1), MenuKind::PauseOptions(false))?;
    game.menu_key(Key::Down)?;
    game.menu_key(Key::Down)?;
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Start)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Options(c)) if c.category == options::Category::CustomDisplay),
        "Start stays on Customize Display"
    );
    game.menu_key(Key::Back)?;
    game.menu_key(Key::Start)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "Start on UI_PauseOptions resumes free roam");
    game.act(Action::Options, MenuKind::ExplorePause)?;
    game.menu_key(Key::Start)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "Start on UI_PauseOptionsMain resumes free roam");
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    // UI_Options Player → Wheel Options → back.
    let mut o = options::OptionsScreen::new(options::Category::Player, &game.settings, FORCE_FEEDBACK, 0.0);
    o.selected = 9;
    o.top = 2;
    let saved = std::mem::replace(&mut game.mode, Mode::Menu(Menu::Options(o)));
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Options(c)) if c.category == options::Category::Wheel),
        "UI_Wheel_Options"
    );
    game.menu_key(Key::Back)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Options(c)) if c.category == options::Category::Player),
        "back on Player"
    );
    game.mode = saved;
    println!("pause options, customize display and wheel options: ok");
    Ok(())
}

/// SDATA/sdat.viv: the career scene's NIS line and the first seconds of
/// EA Trax song 0 decode, and both chyron packages are there.
pub(crate) fn selftest_streams(game: &mut Game) -> Result<()> {
    use nfsu2::audio::{mixer, music, speech};
    let viv = nfsu2::formats::viv::Viv::mount_game(&game.game_data.root).context("SDATA/sdat.viv")?;
    let bank = speech::Bank::load(&viv, "nisaudio").context("the NIS bank")?;
    let r = *bank.select(speech::nis::EVENT_END, &speech::nis::CAREER).first().context("CareerNis1's line")?;
    let pcm = bank.decode(&viv, 0x4024, r).context("decoding CareerNis1's line")?;
    let secs = pcm.samples.len() as f32 / pcm.channels.max(1) as f32 / pcm.rate as f32;
    anyhow::ensure!((secs - 13.24).abs() < 0.05, "CareerNis1's line lasts {secs} s");
    let mpf = music::Mpf::parse(&viv.read("pfdata/MusicSFx.mpf").context("the music map")?)
        .context("parsing the music map")?;
    let buf: mixer::TrackBuf = std::sync::Arc::new(std::sync::Mutex::new(mixer::TrackPcm {
        channels: 2,
        rate: 0,
        samples: Vec::new(),
        done: false,
    }));
    let keep = buf.clone();
    music::decode_song(&viv, &mpf, 0, &buf).context("decoding EA Trax song 0")?;
    let n = keep.lock().unwrap().samples.len();
    anyhow::ensure!(n > 32000 * 2 * 190, "EA Trax song 0 decodes to {n} samples");
    for p in ["Chyron_FE", "Chyron_IG"] {
        anyhow::ensure!(game.game_data.assets.package(p).is_some(), "the {p} package");
    }
    // Rachel's garage hint (0x53E0C0 kind 4: Career (0x2000, 0x20)): the
    // in-game bank's fitting record decoded (MicroTalk).
    let ug2 = speech::Bank::load(&viv, "UG2_Speech").context("the speech bank")?;
    let (entry, recs) = ug2.fit(&speech::lines::career(0x2000, 0x20)).context("the garage line fits")?;
    let line = ug2.decode(&viv, entry, recs[0]).context("decoding the garage line")?;
    let rachel = line.samples.len() as f32 / line.channels.max(1) as f32 / line.rate.max(1) as f32;
    anyhow::ensure!(rachel > 0.5 && line.samples.iter().any(|&x| x.abs() > 0.01), "the garage line lasts {rachel} s");
    println!(
        "streams: CareerNis1 line {secs:.2} s, Rachel's garage line {rachel:.2} s, song 0 {:.1} s",
        n as f32 / 64000.0
    );
    Ok(())
}

/// The boot screens: each movie screen opens its movie (THX through the
/// English file), the intro skips on Accept and the others do not, the
/// splash's prompt comes 4 s after it opens, Enter then goes to UI_Main,
/// and 30 s idle brings the intro back.
pub(crate) fn selftest_boot(game: &mut Game) -> Result<()> {
    for step in BootStep::ALL {
        let mode = game.boot_screen(step);
        match (&mode, step.movie()) {
            (Mode::Menu(Menu::Movie(m)), Some((_, _, name))) => {
                anyhow::ensure!(m.player.is_some() && m.boot == Some(step), "boot movie {name} opens")
            }
            (Mode::Menu(Menu::Splash(_)), None) => {}
            _ => anyhow::bail!("boot screen {step:?}"),
        }
    }
    game.mode = game.boot_screen(BootStep::EaLogo);
    game.menu_key(Key::Accept)?;
    // Enter skips every boot movie (agreed deviation, docs/native-deviations.md "Skipping scenes"; natively the EA logo
    // does not skip).
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(
        !matches!(&game.mode, Mode::Menu(Menu::Movie(m)) if m.boot == Some(BootStep::EaLogo) && !m.done),
        "the EA logo skips on Enter"
    );
    game.mode = game.boot_screen(BootStep::Intro);
    game.menu_key(Key::Accept)?;
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Splash(s)) if s.ready.is_none()),
        "the intro skips to the splash"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Splash(_))), "no Enter before the prompt");
    if let Mode::Menu(Menu::Splash(s)) = &mut game.mode {
        s.age(5000.0);
    }
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(
        game.fe.splash_seen && matches!(&game.mode, Mode::Menu(Menu::Splash(s)) if s.ready.is_some()),
        "the prompt after 4 s"
    );
    if let Mode::Menu(Menu::Splash(s)) = &mut game.mode {
        s.age(30_000.0);
    }
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Movie(m)) if m.boot == Some(BootStep::Intro)),
        "30 s idle: the intro again"
    );
    game.mode = game.boot_screen(BootStep::Splash);
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Splash(s)) if s.ready.is_some()),
        "the prompt at once the second time"
    );
    game.menu_key(Key::Accept)?;
    // No profile on disk: "Would you like to create a new profile?" (MC_Main
    // operation 0x60, No focused); No goes on to UI_Main.
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if !m.career && !m.replacing && m.selected == 1),
        "Enter without a profile: create one?"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "No: UI_Main");
    println!("boot screens: ok");
    selftest_profiles(game)
}

/// Menus round 13: the career shops' exit questions, UI_ICEMAIN's
/// nitrous notice, the rims' size question and the dyno's save question
/// with its Back (the dyno menu again).
pub(crate) fn selftest_menus13(game: &mut Game) -> Result<()> {
    let saved_profile = game.profile.clone();
    let saved_customize = game.save_flow.customize.take();
    let is_question = |game: &Game, hash: u32| matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1 && d.dim_under.is_some() && game.game_data.assets.strings.get(hash).is_some_and(|t| d.text == t.replace("%s", "")));
    // The performance shop's and UI_ICEMAIN's Back: the question, No
    // focused; No (Back) stays.
    game.mode = game.perf_category_menu(0);
    game.menu_key(Key::Back)?;
    anyhow::ensure!(is_question(game, 0xD76E_0B5A), "the performance shop asks before leaving");
    game.menu_key(Key::Back)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PerfCategory))),
        "No stays in the performance shop"
    );
    game.mode = game.specialties_browser(0, None);
    game.menu_key(Key::Back)?;
    anyhow::ensure!(is_question(game, 0xD76E_0B5A), "UI_ICEMAIN asks before leaving");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN"),
        "No stays in UI_ICEMAIN"
    );
    // Nitrous Purge without nitrous: the notice, the strip again after.
    if let Some(p) = game.profile.as_mut() {
        p.perf.retain(|id| !(12..=14).contains(id));
    }
    let at = screens::SPECIALTIES.iter().position(|c| c.0 == 0xA2).context("Nitrous Purge")?;
    game.mode = game.specialties_browser(at, None);
    if let Mode::Menu(Menu::Parts(b)) = &mut game.mode {
        b.categories[at].locked = false;
    }
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.buttons.len() == 1 && d.text.contains("nitrous") && d.dim_under.is_some()),
        "the nitrous notice"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.part.is_none()),
        "back on the strip"
    );
    // The rims' size accept asks (0x559AC0) before fitting.
    game.mode = game.rims_browser(0, Some(0), Some(0));
    if let Mode::Menu(Menu::Parts(b)) = &game.mode
        && b.categories.get(b.category).is_some_and(|c| !c.parts.is_empty())
    {
        let before = game.profile.as_ref().map(|p| p.installed.get(&0x1D).copied());
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(
            matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.package == "GenericDialog_SMALL" && d.dim_under.is_some()),
            "the rims' question"
        );
        game.menu_key(Key::Back)?;
        anyhow::ensure!(game.profile.as_ref().map(|p| p.installed.get(&0x1D).copied()) == before, "Back fits nothing");
    }
    // The dyno's save question: Back returns to the dyno menu.
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno();
    if let Some(p) = game.profile.as_mut() {
        p.scratch = false;
        let mut t = p.tuning[0];
        t.0[0] = t.0[0].wrapping_add(10);
        game.dyno.tuning_object = Some(t);
    }
    let prompts = std::mem::replace(&mut game.settings.save_prompts, true);
    game.act(Action::QuitTuning, MenuKind::Dyno)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1), "Quit Tuning focuses No");
    answer_yes(game)?;
    anyhow::ensure!(is_question(game, 0x7185_1654), "the save question over the dyno menu");
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Dyno))), "Back: the dyno menu again");
    game.settings.save_prompts = prompts;
    game.dyno.tuning_object = None;
    game.quit_tuning();
    game.profile = saved_profile;
    game.save_flow.customize = saved_customize;
    game.mode = game.icon_menu(MenuKind::Main);
    println!("menus13: shop exit questions, nitrous notice, rims question, dyno save question: ok");
    Ok(())
}

pub(crate) fn selftest_dialogs_loading(game: &mut Game) -> Result<()> {
    // 0x526C50: strlen × CONDUITMDITC_TT21I's line height.
    let font =
        game.game_data.assets.fonts.get(&nfsu2::formats::hash::bin_hash("CONDUITMDITC_TT21I")).context("CONDUITMDITC_TT21I")?;
    let h = font.line_height;
    let small = (2561.0 / h).ceil() as usize - 1;
    let large = (5122.0 / h).ceil() as usize;
    println!("dialog sizes: line height {h}, SMALL up to {small} characters, LARGE from {large}");
    for (len, want) in [
        (small, "GenericDialog_SMALL"),
        (small + 1, "GenericDialog_MED"),
        (large - 1, "GenericDialog_MED"),
        (large, "GenericDialog_LARGE"),
    ] {
        let got = screens::dialog_package(&game.game_data.assets, &"x".repeat(len));
        anyhow::ensure!(got == want, "{len} characters: {got}, not {want}");
        anyhow::ensure!(game.game_data.assets.package(got).is_some(), "{got} loaded");
    }
    // 'Q' on UI_Main (0x558B80): the quit question, Cancel focused; Back
    // and Cancel return to the menu, Quit Game ends the game.
    game.mode = game.icon_menu(MenuKind::Main);
    game.key(winit::keyboard::KeyCode::KeyQ, &mut || {})?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.buttons.len() == 2 && d.selected == 1),
        "Q asks to quit, Cancel focused"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        !game.platform.quit_app && matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))),
        "Cancel returns to UI_Main"
    );
    game.key(winit::keyboard::KeyCode::KeyQ, &mut || {})?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(std::mem::take(&mut game.platform.quit_app), "Quit Game ends the game");
    game.mode = game.icon_menu(MenuKind::Main);
    // Escape on UI_Main does nothing (native); elsewhere it is Back.
    let mut exited = false;
    game.key(winit::keyboard::KeyCode::Escape, &mut || exited = true)?;
    anyhow::ensure!(
        !exited && !game.platform.quit_app && matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))),
        "Escape on UI_Main does nothing"
    );
    game.mode = game.icon_menu(MenuKind::Options);
    game.key(winit::keyboard::KeyCode::Escape, &mut || exited = true)?;
    anyhow::ensure!(
        !exited && !matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Options))),
        "Escape on Options goes back"
    );
    game.mode = game.icon_menu(MenuKind::Main);
    // The career's drive in (a track load) showed PC_Loading and took it
    // down after.
    anyhow::ensure!(
        game.sim.drive.is_some() && game.loads.track_loads > 0 && game.loads.loading.is_none(),
        "a track load for free roam, PC_Loading down after it"
    );
    let quads = screens::Loading { package: screens::Loading::PC, opened: 0.0 }.quads(&game.game_data.assets, 0.0);
    anyhow::ensure!(!quads.is_empty(), "PC_Loading drawn");
    // An engage dialog dismissed pops back to the running city: no track
    // load, no PC_Loading.
    if game.here().is_some() {
        let loads = game.loads.track_loads;
        game.mode = Mode::Menu(Menu::ShopEngage(ShopEngage {
            shop: 0,
            opened: now_ms(game.pacing.started),
            focus: Default::default(),
        }));
        game.menu_key(Key::Back)?;
        anyhow::ensure!(
            matches!(game.mode, Mode::Explore) && game.loads.track_loads == loads,
            "the shop dialog's Back resumes free roam without a load"
        );
        // Back to the city from the garage with a car of our own, in a new
        // session (the start scene not yet played): no career start scene.
        if game.profile.as_ref().is_some_and(|p| !p.car.is_empty()) {
            let played = std::mem::replace(&mut game.career_world.career_scene_played, false);
            game.mode = game.icon_menu(MenuKind::Crib);
            game.menu_key(Key::Back)?;
            anyhow::ensure!(
                game.scenes.shop.as_ref().is_none_or(|r| r.scene.kind != nfsu2::shop_scene::SceneKind::Career),
                "no career start scene out of the garage with a car"
            );
            game.career_world.career_scene_played = played;
        }
        let loads = game.loads.track_loads;
        // "Return to Explore Mode" from explore's pause: no load either.
        game.mode = game.icon_menu(MenuKind::ExplorePause);
        game.act(Action::ReturnToExplore, MenuKind::ExplorePause)?;
        anyhow::ensure!(
            matches!(game.mode, Mode::Explore) && game.loads.track_loads == loads,
            "Return to Explore from the pause resumes free roam without a load"
        );
        // Pausing and leaving the pause, twice: the city runs on, no load
        // and no career start scene again.
        let scene = game.scenes.shop.as_ref().map(|r| r.scene.kind);
        for _ in 0..2 {
            game.mode = game.icon_menu(MenuKind::ExplorePause);
            game.menu_key(Key::Back)?;
            anyhow::ensure!(
                matches!(game.mode, Mode::Explore)
                    && game.loads.track_loads == loads
                    && game.scenes.shop.as_ref().map(|r| r.scene.kind) == scene,
                "Back from the pause resumes free roam without a load or a scene (explore {}, loads {} vs {loads}, scene {})",
                matches!(game.mode, Mode::Explore),
                game.loads.track_loads,
                game.scenes.shop.is_some()
            );
        }
    }
    Ok(())
}
