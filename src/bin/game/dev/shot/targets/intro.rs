//! `--shot` targets: The career's start: D-Day intro, its SMS, route and the first car lot.
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("intro_tip", intro_tip),
    Target::name("intro_sms", intro_sms),
    Target::name("intro_route", intro_route),
    Target::name("carlot_first", carlot_first),
    Target::name("intro_nis", intro_nis),
    Target::name("intro", intro_or_car),
    Target::name("intro_prompt", intro_or_car),
    Target::name("intro_gps", intro_or_car),
    Target::name("intro_gps_error", intro_or_car),
    Target::name("introlot", introlot),
];

/// An open event's marker 20 m ahead (its effect), or the car in
/// its trigger (the HUD's engage prompt).
/// A fresh career: the D-Day intro in the city.
/// `intro_prompt`: a fresh career's first second in the city (the
/// D-Day script's prompt, the phone icon); `intro_gps`: the script
/// run through (the instruction read and closed), the GPS on the
/// car lot. `intro`: as the Wine capture of the original takes it
/// (the private capture script: Rachel's SMS dismissed, then
/// 20 s standing): the script through and 20 s more.
/// `intro_gps_error`: then the GPS engaged again from a point on no
/// road (no start node: "Unable to contact satellite").
/// `intro_tip`: the script run through, then the car put in the
/// engage-tip trigger nearest the start: what opens.
pub(super) fn intro_tip(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let mut p = Profile::new("Player");
    p.start_career(&game.game_data.career);
    game.profile = Some(p);
    game.explore(None)?;
    play_career_scene(game)?;
    for _ in 0..330 {
        if !matches!(game.mode, Mode::Explore) {
            game.sms_close();
        }
        game.advance_for(1.0 / 30.0)?;
    }
    let me = game.sim.drive.as_ref().unwrap().state.position.truncate();
    for t in game.sim.route_triggers.tips() {
        eprintln!("tip {:08X} box {:?}..{:?} ({:.0} m)", t.name, t.min, t.max, ((t.min + t.max) * 0.5).distance(me));
    }
    let t = game
        .sim.route_triggers
        .tips()
        .min_by(|a, b| ((a.min + a.max) * 0.5).distance(me).total_cmp(&((b.min + b.max) * 0.5).distance(me)))
        .context("a tip")?
        .clone();
    let (h, z) = game.sim.drive.as_ref().map(|d| (d.state.heading(), d.state.position.z)).unwrap();
    game.sim.drive.as_mut().unwrap().place(((t.min + t.max) * 0.5).extend(z + 1.0), h);
    for _ in 0..90 {
        if !matches!(game.mode, Mode::Explore) {
            break;
        }
        game.advance_for(1.0 / 30.0)?;
    }
    for _ in 0..30 {
        game.advance_for(1.0 / 30.0)?;
    }
    eprintln!("inbox {:?}", game.profile.as_ref().unwrap().inbox.entries);
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `intro_sms`: the SMS screens the script opens 2 s in.
pub(super) fn intro_sms(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let mut p = Profile::new("Player");
    p.start_career(&game.game_data.career);
    game.profile = Some(p);
    game.explore(None)?;
    play_career_scene(game)?;
    for _ in 0..300 {
        if !matches!(game.mode, Mode::Explore) {
            break;
        }
        game.advance_for(1.0 / 30.0)?;
    }
    for _ in 0..30 {
        game.advance_for(1.0 / 30.0)?;
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `intro_route`: a fresh career's intro driven from the airport to
/// the car lot on the GPS route (`drive_intro_route`); the shot is
/// the car lot's engage dialog.
pub(super) fn intro_route(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let mut p = Profile::new("Player");
    p.start_career(&game.game_data.career);
    game.profile = Some(p);
    game.explore(None)?;
    play_career_scene(game)?;
    let (secs, metres) = drive_intro_route(game, std::env::var_os("INTRO_VERBOSE").is_some())?;
    eprintln!("intro route: the car lot after {secs:.0} s, {metres:.0} m");
    finish_scene(game, gpu, format);
    if let Mode::Menu(Menu::ShopEngage(e)) = &mut game.mode {
        e.opened = 0.0;
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `carlot_first`: a new career's first entry into the car lot
/// (CC_CAR_LOT_1) by its engage dialog: the drive-in, the lot's
/// movie SCENE05 (NFSU2_MOVIE_S seconds of it, default 2, then
/// skipped), the car lot with Rachel's line, NFSU2_SHOT_MS (default
/// 6000) in. With NFSU2_AUDIO_WAV the movie's and Rachel's sound.
pub(super) fn carlot_first(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, env, .. } = cx;
    let mut p = Profile::new("Player");
    p.start_career(&game.game_data.career);
    game.profile = Some(p);
    game.explore(None)?;
    let shop = game.game_data.career.shops.iter().position(|s| s.name == "CC_CAR_LOT_1").context("the car lot")?;
    game.mode = Mode::Menu(Menu::ShopEngage(ShopEngage { shop, opened: 0.0, focus: Default::default() }));
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Movie(_))), "the car lot's movie");
    let movie: f32 = std::env::var("NFSU2_MOVIE_S").ok().and_then(|s| s.parse().ok()).unwrap_or(2.0);
    for _ in 0..(movie * 30.0) as usize {
        game.advance_for(1.0 / 30.0)?;
    }
    game.menu_key(Key::Accept)?;
    let ms: f32 = env.ms.unwrap_or(6000.0);
    for _ in 0..(ms / 1000.0 * 30.0) as usize {
        game.advance_for(1.0 / 30.0)?;
    }
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::CarLot(_))), "the car lot after its movie");
    anyhow::ensure!(
        game.profile.as_ref().is_some_and(|p| p.hints.garage && p.entered.contains(&game.game_data.career.shops[shop].hash)),
        "Rachel's line and the lot entered"
    );
    eprintln!("carlot_first: the movie, then the lot with Rachel's line");
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `intro_nis`: the career's start scene NFSU2_SHOT_MS (default
/// 5000) in.
pub(super) fn intro_nis(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, env, .. } = cx;
    let mut p = Profile::new("Player");
    p.start_career(&game.game_data.career);
    game.profile = Some(p);
    game.explore(None)?;
    let ms: f32 = env.ms.unwrap_or(5000.0);
    for _ in 0..(ms / 1000.0 * 30.0) as usize {
        game.advance_for(1.0 / 30.0)?;
    }
    anyhow::ensure!(game.scenes.shop.is_some(), "the scene still playing");
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn intro_car(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    // INTRO_CAR=name: Quick Race's Free Run in that stock car (as
    // the original's measurements under Wine), then INTRO_POSE /
    // INTRO_KEYS as below.
    game.quick.car = std::env::var("INTRO_CAR").ok();
    game.race_flow.quick_free_run = true;
    // INTRO_NOS: the level-1 nitrous package's parts fitted (Quick
    // Race Customize: Dry Shot of Nitrous Oxide).
    if std::env::var_os("INTRO_NOS").is_some() {
        let ids: std::collections::BTreeSet<u8> = game
            .game_data.career
            .packages
            .iter()
            .filter(|p| p.category == 2 && p.level == 1)
            .flat_map(|p| p.parts.iter().map(|x| x.id))
            .collect();
        eprintln!("intro: nitrous parts {ids:?}");
        game.quick.parts = Some((Default::default(), ids));
    }
    game.explore(None)?;
    for _ in 0..60 {
        game.advance_for(1.0 / 30.0)?;
    }
    intro_drive(game)?;
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `intro`, `intro_prompt`, `intro_gps`, `intro_gps_error`: with INTRO_CAR
/// set, `intro` is [`intro_car`], else each is [`intro`] (the old match's
/// guarded arm before the plain one).
pub(super) fn intro_or_car(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    if screen == "intro" && std::env::var_os("INTRO_CAR").is_some() { intro_car(game, cx) } else { intro(game, cx) }
}

pub(super) fn intro(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    let mut p = Profile::new("Player");
    p.start_career(&game.game_data.career);
    // As the career's D-Day start does (0x54AF90 → 0x5410F0 →
    // 0x524250): Rachel's reveal and nag queued.
    p.hints.dday_a();
    game.profile = Some(p);
    game.explore(None)?;
    play_career_scene(game)?;
    // The HUD's own clock for the frames below (the shot renders
    // at NFSU2_SHOT_MS; slots past their steps stay put).
    let mut hud_t = 0.0f32;
    let start = game.sim.drive.as_ref().map(|d| d.state.position);
    for _ in 0..30 {
        game.advance_for(1.0 / 30.0)?;
        // The HUD each frame, as the window does: its prompt
        // queue steps (the D-Day phone prompt rests at 4 s, Wine
        // round 3A: the cingular icon after the SMS).
        hud_t += 1000.0 / 30.0;
        let _ = game.hud(hud_t);
    }
    if screen != "intro_prompt" {
        // NFSU2_INTRO_WAIT=s: how long to wait for the GPS
        // (default 10 s; Rachel's lines hold the script when the
        // sounds play).
        let wait: f32 = std::env::var("NFSU2_INTRO_WAIT").ok().and_then(|s| s.parse().ok()).unwrap_or(10.0);
        for i in 0..(wait * 30.0) as usize {
            if !matches!(game.mode, Mode::Explore) {
                game.sms_close();
            }
            game.advance_for(1.0 / 30.0)?;
            hud_t += 1000.0 / 30.0;
            let _ = game.hud(hud_t);
            if game.career_world.gps.is_some() {
                eprintln!("intro: GPS engaged after {:.1} s", i as f32 / 30.0);
                break;
            }
        }
        anyhow::ensure!(game.career_world.gps.is_some(), "the GPS engaged");
        // INTRO_ROUTE=file: the GPS route to the car lot as "x y z"
        // lines once found (for driving the original there).
        if let Ok(path) = std::env::var("INTRO_ROUTE") {
            for _ in 0..600 {
                if game.career_world.gps.as_ref().is_some_and(|g| matches!(g.follower.state, 1 | 2) && g.aid.is_some()) {
                    break;
                }
                game.advance_for(1.0 / 30.0)?;
                hud_t += 1000.0 / 30.0;
                let _ = game.hud(hud_t);
            }
            let gps = game.career_world.gps.clone().context("the GPS engaged")?;
            let routes = &game.sim.traffic.as_ref().context("free roam routes")?.routes;
            let text: String = gps.route(routes).iter().map(|p| format!("{} {} {}\n", p.x, p.y, p.z)).collect();
            std::fs::write(&path, text)?;
            if let Some(d) = game.sim.drive.as_ref() {
                eprintln!("intro: aid {:?} car {:?} look {:?}", gps.aid, d.state.position, d.camera.forward());
            }
        }
    }
    if screen == "intro" {
        for _ in 0..600 {
            game.advance_for(1.0 / 30.0)?;
            hud_t += 1000.0 / 30.0;
            let _ = game.hud(hud_t);
        }
        // INTRO_GAS: hold the throttle for 5 s and log the speed.
        if std::env::var_os("INTRO_GAS").is_some() {
            game.platform.keys.insert(KeyCode::ArrowUp);
            for i in 0..150 {
                game.advance_for(1.0 / 30.0)?;
                hud_t += 1000.0 / 30.0;
                let _ = game.hud(hud_t);
                if i % 15 == 0 {
                    let d = game.sim.drive.as_ref().unwrap();
                    eprintln!(
                        "gas t {:.1} speed {:.2} m/s rest {} rpm {:.0} spin {:.1} mode explore {}",
                        i as f32 / 30.0,
                        d.state.velocity.length(),
                        d.state.rest,
                        d.state.rpm(),
                        d.state.wheels[2].spin,
                        matches!(game.mode, Mode::Explore)
                    );
                }
            }
            game.platform.keys.clear();
        }
        intro_drive(game)?;
    }
    if let (Some(a), Some(d)) = (start, game.sim.drive.as_ref()) {
        eprintln!(
            "intro: the car moved {:.2} m (speed {:.2} m/s, rest class {})",
            a.distance(d.state.position),
            d.state.velocity.length(),
            d.state.rest
        );
        let com = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
        eprintln!(
            "intro: centre of mass {:.3} {:.3} {:.3}, eye {:.3} {:.3} {:.3}",
            com.x, com.y, com.z, d.camera.position.x, d.camera.position.y, d.camera.position.z
        );
        eprintln!(
            "intro: origin {:.4?} rows {:.4?} com offset {:?} look {:.4?}",
            d.state.position,
            glam::Mat3::from_quat(d.state.orientation).transpose(),
            d.car.car_type.handling.centre_of_mass,
            d.camera.forward()
        );
    }
    if screen == "intro_gps_error" {
        let g = game.career_world.gps.clone().context("the GPS engaged")?;
        let t = career::Trigger {
            gps: g.at,
            position: g.at,
            name: 0,
            min: Vec2::ZERO,
            max: Vec2::ZERO,
            polygon: [Vec2::ZERO; 8],
            points: 0,
        };
        game.career_world.gps = Some(dday::Gps::engage(g.target, &t, Some((Vec3::new(0.0, 0.0, -500.0), Vec2::X))));
        // The one tick that finds no start node (a second tick's
        // follower would start the search again).
        game.advance_ticks(1)?;
        hud_t += 1000.0 / 60.0;
        let _ = game.hud(hud_t);
        anyhow::ensure!(game.career_world.gps.as_ref().is_some_and(|g| g.state() == 1), "unable to contact satellite");
    }
    if std::env::var_os("NFSU2_LIST_CARS").is_some() {
        if let Some(p) = game.profile.as_ref() {
            eprintln!(
                "intro races {} closed {} open {:08X?} sms unread {}",
                p.intro_races,
                p.intro_closed,
                p.open,
                p.inbox.unread()
            );
        }
        let me = game.sim.drive.as_ref().unwrap().state.position;
        eprintln!("player at {me:?}");
        for (i, c) in game.sim.traffic.iter().flat_map(|t| t.cars.iter().enumerate()).filter(|(_, c)| c.lane.is_some()) {
            eprintln!(
                "traffic {i} {} at {:?} ({:.0} m)",
                c.car.car_type.name,
                c.state.position,
                c.state.position.distance(me)
            );
        }
        for c in game.sim.encounters.iter().flat_map(|e| e.cars.iter()).filter(|c| c.active) {
            eprintln!(
                "encounter {} at {:?} ({:.0} m)",
                c.car.car_type.name,
                c.state.position,
                c.state.position.distance(me)
            );
        }
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// A fresh career driven into the intro's goal shop (the car lot):
/// its trigger engages it at once (0x514FD0).
pub(super) fn introlot(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let mut p = Profile::new("Player");
    p.start_career(&game.game_data.career);
    if std::env::var_os("INTRO_SAVE").is_some() {
        // As Resume does (0x495A30).
        p = Profile::load().context("the saved career")?;
        p.upgrade_save(&game.game_data.career);
        p.refresh_open(&game.game_data.career, &mut game.rng);
        p.start_career(&game.game_data.career);
        p.scratch = true;
    }
    game.profile = Some(p);
    game.explore(None)?;
    play_career_scene(game)?;
    let intro = game.profile.as_ref().and_then(|p| p.intro);
    let goal = intro.and_then(|h| game.game_data.career.events.iter().find(|e| e.unlock == h)).map(|e| e.goals.clone());
    eprintln!("intro {intro:?} goals {goal:?}");
    let lot = game
        .game_data.career
        .shops
        .iter()
        .position(|s| goal.as_ref().is_some_and(|g| g.iter().any(|&(k, h)| k == 4 && h == s.hash)))
        .context("the intro's goal shop")?;
    let shop = &game.game_data.career.shops[lot];
    eprintln!(
        "goal shop {} trigger {:08X} open {}",
        shop.name,
        shop.trigger,
        game.profile.as_ref().unwrap().shop_open(&game.game_data.career, shop)
    );
    let t = game.sim.route_triggers.find(shop.trigger).context("its trigger")?.clone();
    let at = ((t.min + t.max) * 0.5).extend(t.position.z);
    let start = game.sim.drive.as_ref().map(|d| d.state.position);
    eprintln!("trigger box {:?}..{:?} z {} start {start:?}", t.min, t.max, t.position.z);
    if let Some(d) = game.sim.drive.as_ref() {
        for e in d
            .world
            .placed_emitters
            .iter()
            .filter(|e| (e.transform.w_axis.truncate().truncate() - at.truncate()).length() < 80.0)
        {
            eprintln!("emitter {:08X} switch {:08X} at {:?}", e.description, e.switch, e.transform.w_axis);
        }
    }
    let before = std::env::var("INTRO_BACK").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
    if let Some(d) = game.sim.drive.as_mut() {
        d.place(at + Vec3::new(0.0, -before, 0.0), std::f32::consts::FRAC_PI_2);
    }
    // INTRO_DRIVE: hold the throttle from there and log the way in.
    let drive = std::env::var_os("INTRO_DRIVE").is_some();
    if drive {
        game.platform.keys.insert(KeyCode::ArrowUp);
    }
    for i in 0..if drive { 300 } else { 30 } {
        game.advance_for_each(1.0 / 30.0, |g, dt| {
            g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
            Ok(())
        })?;
        if drive && i % 10 == 0 {
            let p = game.sim.drive.as_ref().map(|d| d.state.position);
            let inside = p.map(|p| t.contains(p.truncate()));
            eprintln!(
                "t {:.1} pos {p:?} in trigger {inside:?} marker {:?} mode explore {}",
                i as f32 / 30.0,
                game.career_world.marker_trigger,
                matches!(game.mode, Mode::Explore)
            );
        }
        if !matches!(game.mode, Mode::Explore) {
            break;
        }
    }
    game.platform.keys.clear();
    // INTRO_STEPS: keys after the trigger ("a" Accept, "l" Left,
    // "w" a second's wait, "x" out of the shop), and the shot shows
    // the screen as it then stands; e.g. "lwawww" is 3 s into the
    // drive-in, "lwawwwwwxww" 2 s into the drive-out.
    for k in std::env::var("INTRO_STEPS").unwrap_or_default().chars() {
        if k == 'a' {
            game.menu_key(Key::Accept)?;
        }
        if k == 'l' {
            game.menu_key(Key::Left)?;
        }
        if k == 'b' {
            game.menu_key(Key::Back)?;
        }
        // "x" leaves the shop for the city, as its way out does.
        if k == 'x' {
            game.explore(game.here())?;
        }
        for _ in 0..if k == 'w' { 30 } else { 1 } {
            game.advance_for(1.0 / 30.0)?;
        }
    }
    eprintln!(
        "marker {:?} trigger {:?} mode {}",
        game.career_world.marker.map(|m| format!("{m:?}")),
        game.career_world.marker_trigger,
        match &game.mode {
            Mode::Explore => "explore",
            Mode::Menu(_) => "menu",
            Mode::Race(_) => "race",
        }
    );
    if let Some(d) = game.sim.drive.as_mut() {
        game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &nfsu2::render::RenderOptions::in_force()));
        game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &nfsu2::render::RenderOptions::in_force()));
    }
    // NFSU2_SHOT_MS counts from the dialog's opening.
    match &mut game.mode {
        Mode::Menu(Menu::ShopEngage(e)) => e.opened = 0.0,
        Mode::Menu(Menu::Engage(e)) => e.opened = 0.0,
        Mode::Menu(Menu::Challenge(e)) => e.opened = 0.0,
        _ => {}
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}
