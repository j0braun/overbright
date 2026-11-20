use crate::*;

/// Controller extras: a pad only, Controller Layout Modern, through the production
/// update: the triggers drive, Start pauses, B resumes, the stick held
/// in a menu repeats at the original's rate, the d-pad edits a profile
/// name, and unplugging the pad pauses with Pause on Disconnect. With
/// the layout native the same pad's triggers do nothing (the original).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn pad_only_modern_layout_end_to_end() {
    crate::tests::testkit::game();
    use nfsu2::pad::host::Btn;
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("PADTEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.pacing.clock = Some(100.0);
    let tick = |game: &mut Game| {
        game.pacing.clock = game.pacing.clock.map(|t| t + 1.0 / 60.0);
        game.advance_for(1.0 / 60.0).unwrap();
    };
    // Auto (the default) makes a standard pad Modern; no pad: nothing.
    assert_eq!(game.settings.enh.pad.layout, nfsu2::pad::extras::LAYOUT_AUTO);
    game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 0b12\n");
    tick(&mut game);
    assert_eq!(game.platform.pads.devices[0].layout, nfsu2::pad::host::Layout::Modern);
    // (The deviations list reads a process-wide flag that other tests'
    // games reset in parallel; `pad::extras` tests it.)
    assert!(game.platform.pads.any_modern());
    // Original: an Xbox pad's triggers are one axis (lZ) the native
    // defaults do not read.
    game.settings.enh.pad.layout = nfsu2::pad::extras::LAYOUT_ORIGINAL;
    tick(&mut game);
    assert_eq!(game.platform.pads.devices.len(), 1);
    game.platform.pads.devices[0].inputs.triggers = [0.0, 1.0];
    tick(&mut game);
    assert!(game.controls().throttle < 0.02, "native: the right trigger does nothing");
    // Modern: re-presented live.
    game.settings.enh.pad.layout = 1;
    tick(&mut game);
    assert_eq!(game.platform.pads.devices[0].layout, nfsu2::pad::host::Layout::Modern);
    game.platform.pads.devices[0].inputs.triggers = [0.0, 1.0];
    game.platform.pads.devices[0].inputs.sticks[0] = -1.0;
    tick(&mut game);
    let c = game.controls();
    assert_eq!((c.throttle, c.brake), (1.0, 0.0));
    assert!(c.steer < -0.99, "full left: {}", c.steer);
    game.platform.pads.devices[0].inputs = Default::default();
    tick(&mut game);
    // Start: the pause menu; B: back to free roam.
    game.platform.pads.devices[0].inputs.buttons[Btn::Start as usize] = true;
    tick(&mut game);
    game.platform.pads.devices[0].inputs.buttons[Btn::Start as usize] = false;
    tick(&mut game);
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))), "Start pauses");
    game.platform.pads.devices[0].inputs.buttons[Btn::East as usize] = true;
    tick(&mut game);
    game.platform.pads.devices[0].inputs.buttons[Btn::East as usize] = false;
    for _ in 0..30 {
        tick(&mut game);
    }
    assert!(matches!(game.mode, Mode::Explore), "B resumes");
    // The name editor with the d-pad.
    let mut m = screens::McMain::new(false, 0.0);
    m.selected = 0;
    m.key(Key::Accept, 0.0);
    assert_eq!(m.step, McStep::Name);
    game.mode = Mode::Menu(Menu::McMain(m));
    for b in [Btn::Left, Btn::Left, Btn::Left, Btn::Left, Btn::Right, Btn::Up, Btn::Up, Btn::Right, Btn::Down] {
        game.platform.pads.devices[0].inputs.buttons[b as usize] = true;
        tick(&mut game);
        game.platform.pads.devices[0].inputs.buttons[b as usize] = false;
        tick(&mut game);
    }
    assert!(matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.name == "C9"), "the d-pad edits the name");
    // Pause on Disconnect.
    game.explore(None).unwrap();
    game.settings.enh.pad.pause_on_disconnect = true;
    game.pacing.clock = Some(200.0);
    game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 0b12\n200.5 disconnect 1\n");
    tick(&mut game);
    assert_eq!(game.platform.pads.devices.len(), 1);
    assert!(matches!(game.mode, Mode::Explore));
    while game.pacing.clock.unwrap() < 200.6 {
        tick(&mut game);
    }
    assert!(game.platform.pads.devices.is_empty());
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))), "unplugged: paused");
}

/// Controller extra (Quit with Both Sticks) through the production update:
/// off (the default) L3 + R3 on the main menu do nothing; on, one stick
/// does nothing, both together open the quit question, sticks already held
/// when the main menu appeared do nothing until let go, and nowhere else.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn both_sticks_quit_from_the_main_menu() {
    crate::tests::testkit::game();
    use nfsu2::pad::host::Btn;
    let mut game = Game::new().unwrap();
    game.pacing.clock = Some(100.0);
    let tick = |game: &mut Game| {
        game.pacing.clock = game.pacing.clock.map(|t| t + 1.0 / 60.0);
        game.advance_for(1.0 / 60.0).unwrap();
    };
    let set = |game: &mut Game, l3: bool, r3: bool| {
        let b = &mut game.platform.pads.devices[0].inputs.buttons;
        b[Btn::L3 as usize] = l3;
        b[Btn::R3 as usize] = r3;
    };
    let main = |g: &Game| matches!(g.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main)));
    let asked = |g: &Game| matches!(&g.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1);
    for layout in [nfsu2::pad::extras::LAYOUT_ORIGINAL, nfsu2::pad::extras::LAYOUT_MODERN] {
        game.settings.enh.pad = Default::default();
        game.settings.enh.pad.layout = layout;
        game.mode = game.icon_menu(MenuKind::Main);
        game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 0b12\n");
        tick(&mut game);
        assert_eq!(game.platform.pads.devices.len(), 1);
        // Off (native): both sticks do nothing.
        assert!(!game.settings.enh.pad.sticks_quit);
        set(&mut game, true, true);
        for _ in 0..10 {
            tick(&mut game);
        }
        assert!(main(&game), "off: nothing");
        // On while both are held: nothing until they are let go.
        game.settings.enh.pad.sticks_quit = true;
        for _ in 0..10 {
            tick(&mut game);
        }
        assert!(main(&game), "held before: nothing");
        set(&mut game, false, false);
        tick(&mut game);
        // One stick: nothing, either one.
        for (l3, r3) in [(true, false), (false, true)] {
            set(&mut game, l3, r3);
            for _ in 0..10 {
                tick(&mut game);
            }
            set(&mut game, false, false);
            tick(&mut game);
            assert!(main(&game), "one stick: nothing ({l3}, {r3})");
        }
        // Both: the quit question, Cancel focused; B cancels it.
        set(&mut game, true, true);
        tick(&mut game);
        assert!(asked(&game), "both sticks: the quit question (layout {layout})");
        set(&mut game, false, false);
        game.menu_key(Key::Back).unwrap();
        for _ in 0..5 {
            tick(&mut game);
        }
        assert!(main(&game), "Cancel: back on the main menu");
        // Not on another menu.
        game.mode = game.icon_menu(MenuKind::Options);
        tick(&mut game);
        set(&mut game, true, true);
        for _ in 0..10 {
            tick(&mut game);
        }
        assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Options))), "only the main menu");
        // Back to the main menu with both still held: nothing.
        game.mode = game.icon_menu(MenuKind::Main);
        for _ in 0..10 {
            tick(&mut game);
        }
        assert!(main(&game), "held when the menu appeared: nothing");
        set(&mut game, false, false);
    }
    assert!(!game.platform.exit_requested, "the question asks; it never quits by itself");
}

/// The front end's screens that were stubs: Online and LAN offline,
/// Controls / EA Trax / Credits, Customize, and Replay offered only
/// with a recording.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn front_end_screens() {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    let main = |g: &Game| matches!(g.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main)));
    // Online: UI_OLISPConnect, UI_OLEALogin, its connect dialog, the
    // lost-connection notice, OK to the main menu.
    game.act(Action::Online, MenuKind::Main).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::IspConnect(_))));
    game.mode = Mode::Menu(Menu::IspConnect(game.now() - ISP_TRY_MS));
    game.menu_tick();
    assert!(matches!(game.mode, Mode::Menu(Menu::EaLogin(_, None))));
    game.menu_key(Key::Down).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::EaLogin(ref l, Some((0x1448_49C1, _)))) if l.selected == 1));
    // Cancel → the question; No → "Connecting to lobby...".
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1));
    game.menu_key(Key::Accept).unwrap();
    game.menu_tick();
    assert!(matches!(game.mode, Mode::Menu(Menu::EaLogin(_, Some((0x0023_E142, t)))) if !t.is_nan()));
    if let Mode::Menu(Menu::EaLogin(_, Some((_, t)))) = &mut game.mode {
        *t -= nfsu2::frontend::screens::EaLogin::CONNECT_MS;
    }
    game.menu_tick();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.buttons.len() == 1));
    game.menu_key(Key::Accept).unwrap();
    assert!(main(&game));
    // Back on EA Login asks; Yes leaves for the main menu.
    game.mode = Mode::Menu(Menu::EaLogin(nfsu2::frontend::screens::EaLogin::new(game.now()), None));
    game.menu_key(Key::Back).unwrap();
    game.menu_key(Key::Left).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(main(&game));
    // LAN: the search dialog, cancelled, then Back.
    game.act(Action::Lan, MenuKind::Main).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::LanServers(_, true))));
    game.menu_key(Key::Back).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::LanServers(_, false))));
    game.menu_key(Key::Back).unwrap();
    assert!(main(&game));
    // Options: Controls, EA GAMES TRAX, Credits; Back to the options.
    for n in [4, 5, 6] {
        game.act(Action::Setting(n), MenuKind::Options).unwrap();
        assert!(
            matches!(game.mode, Mode::Menu(Menu::Controls(_) | Menu::Jukebox(_) | Menu::Credits(_))),
            "setting {n}"
        );
        game.menu_key(Key::Down).unwrap();
        game.menu_key(Key::Back).unwrap();
        assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Options))));
    }
    let credits = screens::Credits::load(&game.game_data.root, 0.0);
    assert!(credits.lines.iter().any(|l| l.contains("BLACK BOX")));
    // Customize: the car, the categories on an unsaved profile, Back
    // restores the profile.
    let before = game.profile.as_ref().map(|p| (p.name.clone(), p.car.clone()));
    game.act(Action::Customize, MenuKind::Main).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if l.state == LotState::Customize && !l.cars.is_empty()));
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))));
    assert!(game.profile.as_ref().is_some_and(|p| p.scratch));
    game.act(Action::CustomizeCategory(0), MenuKind::CustomizeCategory).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Parts(_))));
    game.mode = game.icon_menu(MenuKind::Crib);
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))));
    // 0x83898C = 3 (0x552CD0): Performance Tuning / Dyno asks
    // 0x54B649F1 (GenericDialog_MED, No focused), Yes the tuning
    // session (`enter_dyno`); its Quit asks 0x40813F63, Yes back to
    // the categories.
    game.act(Action::Tuning, MenuKind::CustomizeCategory).unwrap();
    let Mode::Menu(Menu::Dialog(d, outcomes)) = std::mem::replace(&mut game.mode, Mode::Explore) else {
        panic!("no dialog")
    };
    assert!(d.selected == 1 && d.package == "GenericDialog_MED");
    assert!(matches!(outcomes[0], Mode::Menu(Menu::LotDeal(LotDeal::EnterDyno))));
    assert!(matches!(outcomes[1], Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))));
    game.act(Action::QuitTuning, MenuKind::Dyno).unwrap();
    let Mode::Menu(Menu::Dialog(_, outcomes)) = std::mem::replace(&mut game.mode, Mode::Explore) else {
        panic!("no dialog")
    };
    assert!(matches!(outcomes[0], Mode::Menu(Menu::LotDeal(LotDeal::QuitTuning))));
    game.mode = game.icon_menu(MenuKind::CustomizeCategory);
    let custom = game.profile.as_ref().map(|p| p.car.clone());
    // Customize's Back: its car select again (0x55BE00 → 0x5377B0),
    // whose Back is UI_Main (through the save point when changed).
    game.menu_key(Key::Back).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if l.state == LotState::Customize));
    game.menu_key(Key::Back).unwrap();
    if matches!(game.mode, Mode::Menu(Menu::McMessage(..))) {
        game.menu_key(Key::Accept).unwrap();
    }
    assert!(main(&game));
    assert_eq!(game.profile.as_ref().map(|p| (p.name.clone(), p.car.clone())), before);
    // The customized car becomes the main menu's (0x521110 → 0x83AAB8)
    // once the career has a car of its own.
    if let Some(p) = game.profile.as_ref() {
        assert_eq!(p.menu_car.as_ref().map(|m| m.0.clone()), custom);
        if !p.car.is_empty() {
            assert_eq!(game.garage_request().and_then(|r| r.car), custom);
        }
    }
    // UI_PerformanceBrandSelect: Right steps the first row's brand
    // back (0x521EA0: to the list's last), Select buys the package
    // with it (on an unsaved profile).
    let mut p = Profile::new("Brands");
    p.scratch = true;
    p.car = "240SX".into();
    p.cash = 1_000_000;
    let saved = game.profile.replace(p);
    let pending = PerfPending::Package(3, true);
    let b = game.brand_select(pending, 0.0).expect("a branded package");
    let first = &b.rows[0].1;
    let want = first[first.len() - 1];
    game.mode = Mode::Menu(Menu::BrandSelect(b, pending));
    game.menu_key(Key::Right).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(!matches!(game.mode, Mode::Menu(Menu::BrandSelect(..))));
    assert!(game.profile.as_ref().is_some_and(|p| p.brands.contains(&want) && p.slot_brands().contains(&want)));
    game.profile = saved;
    // UI_postrace lists Replay only with a recording.
    let replay = |v: Vec<screens::IconOption>| v.iter().any(|o| matches!(o.action, Action::Replay));
    assert!(!replay(screens::post_race_menu(true, false, false, false)));
    assert!(replay(screens::post_race_menu(true, false, false, true)));
}
