//! `--shot` targets: The tuning session: tuning screens, dyno run, dyno chart.
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("dyno", dyno),
    Target::name("tunemode", tunemode),
    Target::name("tunemode_ask", tunemode_ask),
    Target::name("dynoenter", dynoenter),
    Target::name("dynoenter_end", dynoenter),
    Target::name("sliders", sliders),
    Target::name("graph", graph),
    Target::name("nos", graph),
    Target::name("tune_tip", tune_tip),
    Target::name("tune_drivetrain", tune_drivetrain),
    Target::name("tune_aero", tune_drivetrain),
    Target::name("tune_tires", tune_drivetrain),
    Target::name("tune_ecu", tune_drivetrain),
    Target::name("tune_turbo", tune_drivetrain),
    Target::name("tune_nos", tune_drivetrain),
    Target::name("tune_reset", tune_drivetrain),
    Target::name("dynochart_end", dynochart_end),
    Target::name("dynochart_zoom", dynochart_zoom),
    Target::name("dynochart", dynochart),
    Target::name("dynochart2", dynochart),
    Target::name("dynorun", dynorun),
    Target::name("dynorun_results", dynorun),
    Target::name("dynoresults", dynoresults),
    Target::name("tuning", tuning),
    Target::name("dyno_save_ask", dyno_save_ask),
    Target::name("dyno_save_mc", dyno_save_mc),
    Target::name("bodyshop_save_mc", dyno_save_mc),
    Target::name("testrun_pause", testrun_pause),
    Target::name("testrun_paused", testrun_paused),
    Target::name("testrun", testrun),
    Target::name("testdrag", testrun),
];

/// Customize's Performance Tuning / Dyno: the session on the dyno
/// track (Mode Select first, no mode yet; the dyno menu with one).
pub(super) fn dyno(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.dyno.tuning_mode = 0;
    // NFSU2_SHOT_SHOP=<a performance shop>: the shop's session.
    if game.visited_shop_kind() == Some(3) {
        game.dyno.from = 1;
    }
    Ok(game.enter_dyno())
}

pub(super) fn tunemode(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.enter_dyno())
}

/// "Tune settings for this race mode?" over the mode screen, which
/// stays as it was (its DIM has no event; menus112 d4).
pub(super) fn tunemode_ask(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.enter_dyno();
    game.act(Action::TuneMode(0), MenuKind::TuneMode)?;
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The DynoEnter scene NFSU2_SCENE_MS in (default 6000; Wine
/// menus113 e1–e8), then the car on the rollers after its end
/// (`dynoenter_end`).
pub(super) fn dynoenter(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno_scene();
    anyhow::ensure!(game.scenes.shop.is_some(), "no DynoEnter scene");
    let ms: f32 = if screen == "dynoenter_end" {
        12000.0
    } else {
        std::env::var("NFSU2_SCENE_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(6000.0)
    };
    for _ in 0..(ms / 1000.0 * 30.0) as usize {
        game.advance_for(1.0 / 30.0)?;
    }
    eprintln!("dynoenter: scene {:?}", game.scenes.shop.as_ref().map(|r| r.frame));
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn sliders(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    // Every suspension part: all nine rows, a few moved.
    let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
    p.car = "PEUGOT".into();
    p.perf.extend([35u8, 37]);
    for _ in 0..15 {
        game.nudge_slider(3, 0, -1.0);
    }
    for _ in 0..30 {
        game.nudge_slider(0, 0, 1.0);
    }
    Ok(game.tune_sliders(0, 0.0))
}

pub(super) fn graph(_game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    Ok(Mode::Menu(Menu::TuneGraph(screens::TuneGraph::new(if screen == "graph" { 4 } else { 6 }, 0.0))))
}

/// Each tuning category's screen with the level-3 part that opens
/// it (career chunk 0x34A1A) and a few presses: Drivetrain (part
/// 11: the gears and the final drive), Aerodynamics (58 / 67),
/// Tires & Brakes (55, 44), ECU (51), Turbo (28), Nitrous (14).
/// `tune_reset`: the suspension moved, then BUTTON1 (Default) on
/// the first row (0x5650D0 → 0x50F9B0).
/// The mouse on the tyres' sliders (`mouse::fe_mouse`): a move over
/// the second row selects it, a click on its Tip mark opens the hint
/// (0x565060 → 0xC519BFC3 → 0x558440) over the screen.
pub(super) fn tune_tip(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
    p.car = "PEUGOT".into();
    p.perf.extend([55u8, 44]);
    game.mode = game.tune_sliders(3, 0.0);
    let record = |game: &mut Game| {
        nfsu2::frontend::mouse::begin_frame(true);
        let _ = game.menu_quads(0.0);
        nfsu2::frontend::mouse::begin_frame(true);
    };
    record(game);
    let rows = nfsu2::frontend::mouse::slider_rows();
    let row = rows.get(1).context("a second slider row")?;
    let frame = game.ui_frame().context("a frame")?;
    let px = |c: Vec2| {
        Vec2::new(frame.origin.0 + (c.x + 320.0) * frame.scale.0, frame.origin.1 + (c.y + 240.0) * frame.scale.1)
    };
    game.platform.mouse.at = Some(px(row.1 + row.2 * 0.5));
    game.fe_mouse()?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::TuneSliders(t)) if t.selected == 1),
        "a move selects the row under the cursor"
    );
    record(game);
    let tip = nfsu2::frontend::mouse::slider_rows().get(1).and_then(|r| r.5).context("the second row's Tip")?;
    game.platform.mouse.at = Some(px(tip + Vec2::new(0.0, 5.0)));
    game.platform.mouse.last = game.platform.mouse.at;
    game.platform.mouse.pressed = true;
    game.fe_mouse()?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Dialog(..))), "a click on the Tip mark opens the hint");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn tune_drivetrain(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    let (category, parts, presses): (usize, &[u8], &[(u8, usize, f32, usize)]) = match screen {
        "tune_drivetrain" => (1, &[11], &[(10, 0, 1.0, 12), (13, 0, -1.0, 8), (9, 0, 1.0, 20)]),
        "tune_aero" => (2, &[58, 67], &[(21, 0, 1.0, 20), (22, 0, -1.0, 10)]),
        "tune_tires" => (3, &[55, 44], &[(19, 0, 1.0, 15), (18, 0, -1.0, 30)]),
        "tune_ecu" => (4, &[51], &[(23, 6, 1.0, 10), (23, 2, -1.0, 6)]),
        "tune_turbo" => (5, &[28], &[(24, 7, 1.0, 10), (24, 1, -1.0, 6)]),
        "tune_nos" => (6, &[14], &[(16, 0, 1.0, 6)]),
        _ => (0, &[35, 37], &[(3, 0, -1.0, 15), (0, 0, 1.0, 30)]),
    };
    let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
    p.car = "PEUGOT".into();
    p.perf.extend(parts.iter().copied());
    for &(param, point, dir, n) in presses {
        for _ in 0..n {
            game.nudge_slider(param, point, dir);
        }
    }
    Ok(if screen == "tune_reset" {
        game.mode = game.tune_sliders(0, 0.0);
        if let Mode::Menu(Menu::TuneSliders(t)) = &mut game.mode {
            t.selected = t.rows.iter().position(|r| r.0 == 0).unwrap_or(0);
        }
        let before = game.dyno_settings();
        game.menu_key(Key::Button1)?;
        let after = game.dyno_settings();
        anyhow::ensure!(
            before.t(0, 0) > 0.0 && after.t(0, 0) == 0.0 && after.t(3, 0) == before.t(3, 0),
            "Default resets the selected row only"
        );
        std::mem::replace(&mut game.mode, Mode::Explore)
    } else if category < 4 {
        game.tune_sliders(category, 0.0)
    } else {
        let mut g = game.tune_graph(category, 0.0);
        g.point = presses[0].1;
        anyhow::ensure!(category == 6 || g.curve.is_some(), "the curve's inputs");
        Mode::Menu(Menu::TuneGraph(g))
    })
}

pub(super) fn dynochart_end(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(match game.dyno_chart(-12000.0) {
        // The pull run to its end: the legend shown (0x54A630).
        Mode::Menu(Menu::DynoChart(mut c)) => {
            for _ in 0..20_000 {
                if c.progress() >= 1.0 {
                    break;
                }
                c.step(nfsu2::dyno::FRAME, -12000.0);
            }
            // The legend's Show from the chart's opening on.
            c.finished = Some(0.0);
            Mode::Menu(Menu::DynoChart(c))
        }
        m => m,
    })
}

/// U held for a second on the chart: the dyno camera zoomed in
/// (0x51 DEBUG_CAMERA_MOVE_CLOSER; Wine menus12 dyno_zoom).
pub(super) fn dynochart_zoom(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let m = game.dyno_chart(-3000.0);
    game.mode = m;
    game.platform.keys.insert(KeyCode::KeyU);
    for _ in 0..60 {
        game.dyno_zoom(1.0 / 60.0);
    }
    game.platform.keys.remove(&KeyCode::KeyU);
    anyhow::ensure!(game.dyno.distance < DYNO_DISTANCE.1 - 1.0, "U zooms the dyno camera in ({})", game.dyno.distance);
    // The pull stopped (the manager's state 0): O does nothing.
    let (zoomed, pull) = match &mut game.mode {
        Mode::Menu(Menu::DynoChart(c)) => (game.dyno.distance, c.pull.take()),
        _ => anyhow::bail!("the dyno chart"),
    };
    game.platform.keys.insert(KeyCode::KeyO);
    game.dyno_zoom(1.0);
    game.platform.keys.remove(&KeyCode::KeyO);
    anyhow::ensure!(game.dyno.distance == zoomed, "no zoom once the pull has stopped");
    if let Mode::Menu(Menu::DynoChart(c)) = &mut game.mode {
        c.pull = pull;
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn dynochart(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    if screen == "dynochart2" {
        game.dyno.first = game.dyno_handling().map(|h| h.engine);
    }
    game.mode = game.dyno_chart(-3000.0);
    // Partway through the pull (the manager's own pace), the car
    // on the rollers moving with it.
    while matches!(&game.mode, Mode::Menu(Menu::DynoChart(c)) if c.pull.as_ref().is_some_and(|p| p.progress() < 0.6 && p.stage() != 0))
    {
        game.dyno_tick(nfsu2::dyno::FRAME);
    }
    // A modified copy shows the first run apart.
    if let Mode::Menu(Menu::DynoChart(c)) = &mut game.mode
        && let Some(f) = c.first.as_mut()
    {
        f.torque.values.iter_mut().for_each(|v| *v *= 0.8);
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The Dyno Run as the window plays it: the chart opened with the
/// Dyno Run option and NFSU2_SHOT_MS (default 4000) of frames run
/// through the game's update (the car on the rollers, its wheels
/// and the rollers turning; the maintainer's `dyno_motion.sh` takes a row of
/// these). `dynorun_results`: Continue at the pull's top, then the
/// spin-down on behind UI_PerformanceDyno_Results.
pub(super) fn dynorun(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, env, .. } = cx;
    game.dyno.tuning_mode = 0;
    game.mode = game.icon_menu(MenuKind::Dyno);
    game.act(Action::DynoRun, MenuKind::Dyno)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::DynoChart(_))), "Dyno Run opens the chart");
    let ms: f32 = env.ms.unwrap_or(4000.0);
    let mut continued = false;
    if std::env::var_os("NFSU2_DYNO_TRACE").is_some() {
        let (name, _, perf) = game.player_car();
        eprintln!("dynotrace car {name} perf {:?} settings {:?}", game.with_uniques(&perf), game.dyno_settings().0);
    }
    for frame in 0..(ms / 1000.0 * 60.0) as usize {
        if screen == "dynorun_results"
            && !continued
            && matches!(&game.mode, Mode::Menu(Menu::DynoChart(c)) if c.progress() >= 1.0)
        {
            game.menu_key(Key::Accept)?;
            anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::DynoResults(_))), "Continue opens the results");
            continued = true;
        }
        game.advance_for(1.0 / 60.0)?;
        // NFSU2_DYNO_TRACE: the pull frame by frame (gear, rpm,
        // wheel spin), to set against the original's car +0x434 /
        // +0x440 / +0x330 (docs/formats.md, "Dyno chart").
        if std::env::var_os("NFSU2_DYNO_TRACE").is_some() {
            if let Mode::Menu(Menu::DynoChart(c)) = &game.mode {
                if let Some(p) = c.pull.as_ref() {
                    let st = p.car_state();
                    eprintln!(
                        "dynotrace {:.4} stage {} gear {} rpm {:.1} spin {:.2?}",
                        (frame + 1) as f32 / 60.0,
                        p.stage(),
                        p.gear(),
                        p.rpm(),
                        st.wheels.iter().map(|w| w.spin).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    let d = game.sim.drive.as_ref().context("the dyno track")?;
    let rest = game.dyno.rest.context("the car on the rollers")?;
    let f = d.state.orientation * Vec3::X;
    eprintln!(
        "dynorun {ms} ms: mode {} rpm {:.0} gear {:?} spin {:.1?} roll {:.2?} pitch {:.2} deg rise {:.4} rollers {:.2?}",
        flow_mode(&game.mode),
        d.state.rpm(),
        d.state.drivetrain.as_ref().map(|t| t.gear),
        d.state.wheels.iter().map(|w| w.spin).collect::<Vec<_>>(),
        d.state.wheels.iter().map(|w| w.roll).collect::<Vec<_>>(),
        f.z.asin().to_degrees(),
        (d.state.position - rest.0).z,
        game.dyno.rollers.as_ref().map(|r| r.angles),
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn dynoresults(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let mut h = game.dyno_handling().context("a career car")?;
    nfsu2::handling::setup_performance(&mut h, RaceMode::FreeRoam);
    let run = screens::DynoRun::of(&h);
    game.dyno.runs.record(run);
    game.dyno.runs.record(screens::DynoRun { top_speed: run.top_speed * 1.1, ..run });
    Ok(Mode::Menu(Menu::DynoResults(screens::DynoResults {
        runs: game.dyno.runs,
        metric: game.settings.metric,
        opened: -3000.0,
        ratings: Some(game.dyno_ratings()),
    })))
}

pub(super) fn tuning(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    // A street-level transmission and nitrous: Drivetrain and
    // Nitrous open.
    let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
    p.perf.insert(8);
    p.perf.insert(12);
    Ok(game.icon_menu(MenuKind::Tuning))
}

/// Quit Tuning's Yes with a setup changed: "save your profile now?"
/// over the dyno menu, dimmed again (0x5649AC).
pub(super) fn dyno_save_ask(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno();
    let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
    p.scratch = false;
    let mut t = p.tuning[0];
    t.0[0] = t.0[0].wrapping_add(10);
    game.dyno.tuning_object = Some(t);
    game.settings.save_prompts = true;
    game.act(Action::QuitTuning, MenuKind::Dyno)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o)) if d.dim_under.is_some() && d.under == Some(2) && o.len() == 3),
        "the save question over the dimmed dyno menu"
    );
    // Asked at the front end's time 0 (loading the track took a
    // while), so NFSU2_SHOT_MS is its time since the ask.
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
        d.opened = 0.0;
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The save points' MC screens over their caller (0x4A3100 mode
/// & 0xF = 1): the dyno's save question answered Yes (0x5649E1,
/// mode 0x51 over the dimmed dyno menu) and the body shop's exit
/// question answered Yes with the profile changed (0x552C90, mode
/// 0x41 over the browser its MENUFADEOUT faded out).
pub(super) fn dyno_save_mc(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    if screen == "dyno_save_mc" {
        game.dyno.tuning_mode = 0;
        game.mode = game.enter_dyno();
        let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
        p.scratch = false;
        let mut t = p.tuning[0];
        t.0[0] = t.0[0].wrapping_add(10);
        game.dyno.tuning_object = Some(t);
        game.settings.save_prompts = true;
        game.act(Action::QuitTuning, MenuKind::Dyno)?;
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
    } else {
        game.profile.get_or_insert_with(|| Profile::new("Player")).scratch = false;
        game.mode = game.parts_browser(0, None);
        game.menu_key(Key::Back)?;
    }
    *game.save_flow.snapshot.borrow_mut() = Some(Vec::new());
    game.settings.autosave = false;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::McMain(_) | Menu::McList(_) | Menu::McMessage(..)))
            && game.fe.mc_under.is_some(),
        "the save point's MC screen over its caller"
    );
    let (m, package, _, scripts) = game.fe.mc_under.take().unwrap();
    game.fe.mc_under = Some((m, package, 0.0, scripts));
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The dyno's test run in Circuit / Drag mode a few seconds in.
/// Esc in a test run: the test run's menu over the paused run
/// (0x53C9F0: six options), then Return to Dyno's question (No
/// focused) and its Yes: the dyno menu on the rollers.
pub(super) fn testrun_pause(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    game.dyno.tuning_mode = 0;
    game.act(Action::TestRun, MenuKind::Dyno)?;
    for _ in 0..90 {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
    }
    game.platform.keys.clear();
    game.key(KeyCode::Escape, &mut || {})?;
    anyhow::ensure!(game.sim.paused_race.as_ref().is_some_and(|r| r.test.is_some()), "the run is paused");
    let n = match &game.mode {
        Mode::Menu(Menu::Icons(m, MenuKind::Dyno)) => m.options.len(),
        _ => 0,
    };
    anyhow::ensure!(n == 6, "the test run's six options, got {n}");
    game.act(Action::ReturnToDyno, MenuKind::Dyno)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1),
        "Return to Dyno? with No focused"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(game.sim.paused_race.is_none() && game.on_dyno_track(), "back on the dyno");
    // The session's reload plays DynoEnter again (Wine).
    anyhow::ensure!(
        game.scenes.shop.as_ref().is_some_and(|r| r.scene.kind == nfsu2::shop_scene::SceneKind::DynoEnter),
        "DynoEnter after Return to Dyno"
    );
    while game.scenes.shop.is_some() {
        game.advance_for(1.0 / 30.0)?;
    }
    let n = match &game.mode {
        Mode::Menu(Menu::Icons(m, MenuKind::Dyno)) => m.options.len(),
        _ => 0,
    };
    anyhow::ensure!(n >= 5, "the dyno menu again");
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The test run paused (Escape after 3 s on the throttle): the
/// Dyno Shop strip with Resume Test Run first (Wine, scrfix).
pub(super) fn testrun_paused(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    // In a tuning session, as entered from a shop's dyno.
    game.dyno.tuning_session = true;
    game.dyno.tuning_mode = 0;
    game.dyno.tuning_object = game.profile.as_ref().map(|p| p.tuning[0]);
    game.act(Action::TestRun, MenuKind::Dyno)?;
    for _ in 0..90 {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
    }
    game.platform.keys.clear();
    game.key(KeyCode::Escape, &mut || {})?;
    anyhow::ensure!(game.sim.paused_race.as_ref().is_some_and(|r| r.test.is_some()), "the run is paused");
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn testrun(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    game.dyno.tuning_mode = if screen == "testdrag" { 1 } else { 0 };
    game.dyno.tuning_object = game.profile.as_ref().map(|p| p.tuning[game.dyno.tuning_mode as usize]);
    game.act(Action::TestRun, MenuKind::Dyno)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Race(r) if r.test.is_some()), "no test run");
    // NFSU2_LOG_SPEED: the speed (mph) every half second, to set
    // against the original's test run (Wine).
    let log = std::env::var_os("NFSU2_LOG_SPEED").is_some();
    for i in 0..240 {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
        if log && (i + 1) % 15 == 0 {
            let d = game.sim.drive.as_ref().unwrap();
            eprintln!(
                "testrun t {:.1} speed {:.1} mph gear {:?}",
                (i + 1) as f32 / 30.0,
                d.state.speed() * 2.237,
                d.state.drivetrain.as_ref().map(|t| t.gear)
            );
        }
    }
    game.platform.keys.clear();
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}
