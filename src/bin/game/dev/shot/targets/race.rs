//! `--shot` targets: Races: drag, drift, street X, endings, replays, crash camera.
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("drift", drift),
    Target::name("drag", drift),
    Target::name("streetx", drift),
    Target::name("dragat", dragat),
    Target::name("draghud", draghud),
    Target::name("dragline", draghud),
    Target::name("dragblown", draghud),
    Target::name("drag_results", draghud),
    Target::name("drag_stats", draghud),
    Target::name("drag_postrace", draghud),
    Target::name("ending", ending),
    Target::name("ending_hold", ending),
    Target::name("finish", finish),
    Target::name("replay", replay),
    Target::name("qrstart", qrstart),
    Target::name("race_pause", qrstart),
    Target::name("lapko", lapko),
    Target::name("crashcam", crashcam),
];

/// A drift / drag / street X event's race after a few seconds of
/// driving (a drift: a hard turn on the handbrake at the end).
pub(super) fn drift(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    let kind = match screen {
        "drift" => career::EventKind::Drift,
        "drag" => career::EventKind::Drag,
        _ => career::EventKind::StreetX,
    };
    let e = (1..=5)
        .find_map(|stage| game.game_data.career.stage_events(&game.game_data.root, stage).iter().find(|e| e.kind == kind).map(|e| e.index))
        .context("an event of that kind")?;
    game.start_race(e, false, false)?;
    for i in 0..240 {
        game.platform.keys.insert(KeyCode::KeyW);
        if screen == "drift" && i >= 200 {
            game.platform.keys.insert(KeyCode::KeyD);
            game.platform.keys.insert(KeyCode::Space);
        }
        game.advance_for(1.0 / 30.0)?;
        if screen == "drag" {
            let now = game.now();
            game.hud(now);
        }
    }
    game.platform.keys.clear();
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// A drag race's HUD mid-run: the gas held from the countdown (the
/// revs on the line), up-shifts at the gear's shift light, the shot
/// a little past the second shift.
/// `dragline`: the same race on the line, the gas held through the
/// countdown (the revs on the line, the held gearbox).
/// `dragblown`: no shifts, held at the limiter until the engine blows
/// (racer status 8), a second later.
/// `dragat`: a drag race (NFSU2_SHOT_EVENT=<event name>, else the
/// first career drag; NFSU2_SHOT_SAVE for the profile) stepped at 1/60 s
/// with the gas held, each up-shift 50 rpm under its point, shot at the
/// race clock NFSU2_SHOT_DRAG_CLOCK (s; negative: the countdown).
/// NFSU2_SHOT_DRAG_NITRO="t0,t1" holds nitrous from t0 to t1;
/// NFSU2_SHOT_DRAG_LANE="t,left;t,right" taps a lane change (0.1 s).
pub(super) fn dragat(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let named = std::env::var("NFSU2_SHOT_EVENT")
        .ok()
        .and_then(|n| game.game_data.career.events.iter().find(|e| e.name == n).map(|e| e.index));
    let e = match named {
        Some(e) => e,
        None => (1..=5)
            .find_map(|stage| {
                game.game_data.career
                    .stage_events(&game.game_data.root, stage)
                    .iter()
                    .find(|e| e.kind == career::EventKind::Drag)
                    .map(|e| e.index)
            })
            .context("a drag event")?,
    };
    let at: f32 = std::env::var("NFSU2_SHOT_DRAG_CLOCK").ok().and_then(|v| v.parse().ok()).unwrap_or(4.0);
    let nitro: Option<(f32, f32)> = std::env::var("NFSU2_SHOT_DRAG_NITRO")
        .ok()
        .and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?))));
    let lanes: Vec<(f32, KeyCode)> = std::env::var("NFSU2_SHOT_DRAG_LANE")
        .unwrap_or_default()
        .split(';')
        .filter_map(|t| {
            let (t, k) = t.split_once(',')?;
            Some((t.trim().parse().ok()?, if k.trim() == "left" { KeyCode::ArrowLeft } else { KeyCode::ArrowRight }))
        })
        .collect();
    game.start_race(e, false, false)?;
    // No shift in the first 0.5 s: the engine comes off the limiter
    // over the launch (the light is red then).
    let mut last_shift = 0.0f32;
    for _ in 0..60 * 200 {
        let clock = match &game.mode {
            Mode::Race(r) => r.timing.clock,
            _ => break,
        };
        if clock >= at {
            break;
        }
        game.platform.keys.insert(KeyCode::KeyW);
        if nitro.is_some_and(|(a, b)| (a..b).contains(&clock)) {
            game.platform.keys.insert(KeyCode::AltLeft);
        } else {
            game.platform.keys.remove(&KeyCode::AltLeft);
        }
        for &(t, k) in &lanes {
            if (t..t + 0.1).contains(&clock) {
                game.platform.keys.insert(k);
            } else {
                game.platform.keys.remove(&k);
            }
        }
        let d = game.sim.drive.as_ref().unwrap();
        let (gear, up) = d.state.drivetrain.as_ref().map_or((0, f32::MAX), |t| (t.gear, t.shift_point_rpm(t.gear)));
        if clock >= 0.0 && clock - last_shift > 0.5 && gear >= 2 && d.state.rpm() >= up - 50.0 {
            game.platform.keys.insert(KeyCode::ShiftLeft);
            game.key(KeyCode::ShiftLeft, &mut || {})?;
            game.platform.keys.remove(&KeyCode::ShiftLeft);
            last_shift = clock;
        }
        game.advance_for(1.0 / 60.0)?;
        let now = game.now();
        game.hud(now);
    }
    game.platform.keys.clear();
    let d = game.sim.drive.as_mut().unwrap();
    let clock = match &game.mode {
        Mode::Race(r) => r.timing.clock,
        _ => f32::NAN,
    };
    println!(
        "dragat: {} clock {clock:.2}, gear {:?}, rpm {:.0}, {:.1} mph, nitrous {:?}",
        d.car.car_type.name,
        d.state.drivetrain.as_ref().map(|t| t.gear),
        d.state.rpm(),
        d.state.velocity.length() * 2.237,
        d.state.nitrous.as_ref().map(|n| n.left)
    );
    println!(
        "dragat: world clock {:.2}, sunrise weight {:.3}, position {:?}",
        d.clock,
        d.time_of_day.fog_weight(),
        d.state.position
    );
    if std::env::var_os("NFSU2_SHOT_DRAG_DEBUG").is_some() {
        let at = d.camera.position.truncate();
        let tint = nfsu2::screen_tint::ScreenTintState::default().update(&d.world, at, d.time_of_day.fog_weight());
        let fog = nfsu2::fog::FogState::default().update_weighted(&d.world, at, d.time_of_day.fog_weight());
        println!("dragat: camera {at:?} tint {tint:?} fog {fog:?}");
    }
    game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &nfsu2::render::RenderOptions::in_force()));
    game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &nfsu2::render::RenderOptions::in_force()));
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn draghud(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    Ok('arm: {
        let e = (1..=5)
            .find_map(|stage| {
                game.game_data.career
                    .stage_events(&game.game_data.root, stage)
                    .iter()
                    .find(|e| e.kind == career::EventKind::Drag)
                    .map(|e| e.index)
            })
            .context("a drag event")?;
        game.start_race(e, false, false)?;
        let mut shifts = 0;
        for _ in 0..600 {
            game.platform.keys.insert(KeyCode::KeyW);
            game.advance_for(1.0 / 30.0)?;
            // The HUD each frame, so its scripts (pulses, zooms) run.
            let now = game.now();
            game.hud(now);
            let d = game.sim.drive.as_mut().unwrap();
            let (gear, up) = d.state.drivetrain.as_ref().map_or((0, f32::MAX), |t| (t.gear, t.shift_point_rpm(t.gear)));
            if screen == "draghud"
                && matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 0.0)
                && gear >= 2
                && d.state.rpm() >= up - 50.0
            {
                d.state.shift(&d.car, true);
                shifts += 1;
            }
            if shifts >= 2 && matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 4.0) {
                break;
            }
            if screen == "dragline" && matches!(&game.mode, Mode::Race(r) if r.timing.clock >= -0.3) {
                break;
            }
            if screen == "dragblown"
                && matches!(&game.mode, Mode::Race(r) if r.racers.me.finished.is_some_and(|f| r.timing.clock >= f + 1.0))
            {
                break;
            }
            if screen.starts_with("drag_") && matches!(&game.mode, Mode::Race(r) if r.racers.me.finished.is_some()) {
                break;
            }
        }
        game.platform.keys.clear();
        // The blown engine's race run on to its end: the results (as
        // under Wine, Quick Race drag with the engine blown), or the
        // race stats opened from the post-race menu.
        if screen.starts_with("drag_") {
            for _ in 0..30 * 400 {
                game.advance_for(1.0 / 30.0)?;
                if matches!(&game.mode, Mode::Menu(Menu::Results(..))) {
                    break;
                }
            }
            anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Results(..))), "drag race not over");
            finish_scene(game, gpu, format);
            // Event Results' Accept: UI_PostRace (menus110 s15 / s16).
            if screen == "drag_postrace" {
                game.menu_key(Key::Accept)?;
                for _ in 0..30 {
                    game.advance_for(1.0 / 30.0)?;
                }
                break 'arm std::mem::replace(&mut game.mode, Mode::Explore);
            }
            if screen == "drag_stats" {
                let mut stats = game.race_flow.stats.clone().context("race stats")?;
                stats.index = stats.player;
                break 'arm Mode::Menu(Menu::Stats(stats));
            }
            break 'arm std::mem::replace(&mut game.mode, Mode::Explore);
        }
        let d = game.sim.drive.as_mut().unwrap();
        println!(
            "draghud: gear {:?}, rpm {:.0}, {:.1} mph, heat {:?}",
            d.state.drivetrain.as_ref().map(|t| t.gear),
            d.state.rpm(),
            d.state.velocity.length() * 2.237,
            d.state.drivetrain.as_ref().map(|t| t.heat)
        );
        game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &nfsu2::render::RenderOptions::in_force()));
        game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &nfsu2::render::RenderOptions::in_force()));
        std::mem::replace(&mut game.mode, Mode::Explore)
    })
}

/// A circuit 8 s in with the player put past the line
/// (`Progress::finished`), then NFSU2_SHOT_MS (default 2000) of the
/// race over: the AI racer controller driving the player's car on.
/// A race's end: a Quick Race circuit with the start scene cut, the
/// player's car past the line (its finish forced) and the race over
/// 3 s later (`RACE_OVER`): Event Results over the ending scene,
/// NFSU2_SHOT_MS on (default 2000; NFSU2_ENDING_NIS picks the
/// scene). `ending_hold`: the race-over message, 1 s after the line.
pub(super) fn ending(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, env, .. } = cx;
    // NFSU2_ENDING_KIND=streetx / sprint: the stage's first event of
    // that kind instead of a circuit.
    let kind = match std::env::var("NFSU2_ENDING_KIND").as_deref() {
        Ok("streetx") => career::EventKind::StreetX,
        Ok("sprint") => career::EventKind::Sprint,
        _ => career::EventKind::Circuit,
    };
    // NFSU2_SHOT_EVENT=<name>: that career event (e.g. the final
    // race, S5_CIRCUIT_36).
    let named = std::env::var("NFSU2_SHOT_EVENT").ok().and_then(|n| game.game_data.career.events.iter().position(|e| e.name == n));
    let e = match named {
        Some(e) => e,
        None => (1..=5)
            .find_map(|s| game.game_data.career.stage_events(&game.game_data.root, s).iter().find(|e| e.kind == kind).map(|e| e.index))
            .context("an event of that kind")?,
    };
    game.start_race(e, false, false)?;
    game.scenes.shop = None;
    game.scenes.crowd = None;
    if let Mode::Race(r) = &mut game.mode {
        r.timing.clock = 0.0;
    }
    for _ in 0..90 {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
    }
    game.platform.keys.clear();
    if let Mode::Race(r) = &mut game.mode {
        r.racers.me.finished = Some(r.timing.clock);
    }
    let ms: f32 = if screen == "ending_hold" {
        1000.0
    } else {
        3000.0 + env.ms.unwrap_or(2000.0)
    };
    for _ in 0..(ms / 1000.0 * 30.0) as usize {
        // NFSU2_SHOT_STEP: the particles too (the burnout's smoke), each
        // tick's.
        let fx = std::env::var_os("NFSU2_SHOT_STEP").is_some();
        game.advance_for_each(1.0 / 30.0, |g, dt| {
            if fx {
                g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
            }
            Ok(())
        })?;
    }
    eprintln!(
        "ending: mode {} scene {:?}",
        match &game.mode {
            Mode::Menu(_) => "menu",
            Mode::Race(_) => "race",
            _ => "other",
        },
        game.scenes.shop.as_ref().map(|r| (r.scene.kind.name(), r.frame, r.camera.is_some()))
    );
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// NFSU2_FINISH_KIND=sprint / streetx / drift / drag / url / ko: the
/// first career event of that kind (url: a URL event; ko: a circuit
/// with the player knocked out, racer status 3) instead of a
/// circuit, so each race type's race-over message and the camera
/// past the line render in race mode.
pub(super) fn finish(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, env, .. } = cx;
    let want = std::env::var("NFSU2_FINISH_KIND").unwrap_or_default();
    let pick = |e: &&career::Event| match want.as_str() {
        "sprint" => e.kind == career::EventKind::Sprint,
        "streetx" => e.kind == career::EventKind::StreetX,
        "drift" => e.kind == career::EventKind::Drift,
        "drag" => e.kind == career::EventKind::Drag,
        "url" => e.category == career::Category::Url,
        _ => e.kind == career::EventKind::Circuit && e.category != career::Category::Url,
    };
    let e = (1..=5)
        .find_map(|st| game.game_data.career.stage_events(&game.game_data.root, st).into_iter().find(pick).map(|e| e.index))
        .context("an event of that kind")?;
    game.start_race(e, false, false)?;
    // Past the start scene, from the green light (as "ending").
    game.scenes.shop = None;
    game.scenes.crowd = None;
    if let Mode::Race(r) = &mut game.mode {
        r.timing.clock = 0.0;
        // The shot's drag runs without its traffic, so that the
        // straight run reaches the line (a hit totals the car).
        if want == "drag" {
            r.racers.traffic = None;
        }
    }
    // A drag (manual, 0x5EB440) is driven to its real finish: the
    // gas held, an up-shift at each gear's shift point (held at the
    // limiter its engine blows, racer status 8, as `dragblown`).
    let frames = if want == "drag" { 30 * 120 } else { 240 };
    for _ in 0..frames {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
        if want == "drag" {
            if matches!(&game.mode, Mode::Race(r) if r.racers.me.finished.is_some()) {
                break;
            }
            let d = game.sim.drive.as_mut().unwrap();
            let (gear, up) = d.state.drivetrain.as_ref().map_or((0, f32::MAX), |t| (t.gear, t.shift_point_rpm(t.gear)));
            if matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 0.0) && gear >= 2 && d.state.rpm() >= up - 50.0 {
                d.state.shift(&d.car, true);
            }
        }
    }
    game.platform.keys.clear();
    if let Mode::Race(r) = &mut game.mode {
        eprintln!(
            "finish: clock {:.2} me {:?} dropped {:?} rivals {:?}",
            r.timing.clock,
            r.racers.me.finished,
            r.racers.me.dropped,
            r.racers.rivals.iter().map(|x| (x.progress.finished, x.progress.progress())).collect::<Vec<_>>()
        );
        r.racers.me.finished.get_or_insert(r.timing.clock);
        if want == "ko" {
            r.scoring.knockout = true;
            r.racers.me.dropped = Some(3);
        }
    }
    // The HUD runs along from the finish (its RACEOVER scripts
    // start at the first frame past the line), to the shot's own
    // HUD time, so the race-over message has faded in.
    let ms: f32 = env.ms.unwrap_or(2000.0);
    let frames = (ms / 1000.0 * 30.0) as usize;
    for k in 0..frames {
        game.advance_for(1.0 / 30.0)?;
        let _ = game.hud(ms - (frames - 1 - k) as f32 * 1000.0 / 30.0);
    }
    if let (Mode::Race(r), Some(d)) = (&game.mode, game.sim.drive.as_ref()) {
        eprintln!(
            "finish: autopilot {} speed {:.1} m/s gas {:.2} brake {:.2}",
            r.autopilot.is_some(),
            d.state.velocity.length(),
            r.autopilot.as_ref().map_or(0.0, |a| a.controls.gas),
            r.autopilot.as_ref().map_or(0.0, |a| a.controls.brake)
        );
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// A race's replay paused halfway: UI_ReplayControl, no HUD.
pub(super) fn replay(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let e = game
        .game_data.career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| e.kind == career::EventKind::Circuit)
        .map(|e| e.index)
        .context("a circuit")?;
    game.start_race(e, false, false)?;
    // Past the start scene, from the green light (as "finish"):
    // 8 s from the load ended inside the scene, before any
    // recording, so no bar showed (round 3A).
    game.scenes.shop = None;
    game.scenes.crowd = None;
    if let Mode::Race(r) = &mut game.mode {
        r.timing.clock = 0.0;
    }
    for _ in 0..240 {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
    }
    game.platform.keys.clear();
    if let Mode::Race(r) = &mut game.mode {
        let at = r.replay.recording.as_ref().map_or(0, |rec| rec.frames.len() / 2);
        let markers = r.replay.recording.as_ref().map_or_else(Vec::new, Recording::markers);
        let region = game.sim.drive.as_ref().map_or_else(String::new, |d| d.region.clone());
        r.replay.playback = Some(Playback {
            at,
            paused: true,
            markers,
            tracks: track_cameras(&game.game_data.root, &region),
            typed: typed_cameras(&game.game_data.caches, &game.game_data.root),
            ..Playback::default()
        });
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// A Quick Race circuit (the first track, as the Quick Race track
/// select opens on it) in the 206, for side-by-sides with the
/// original's race start: `NFSU2_FRAMES=n` frames at 30 fps
/// (default 0: the grid as it loads), `NFSU2_THROTTLE=1` holding the
/// throttle, `NFSU2_LOOKBACK=1` the look-back key, `NFSU2_VIEW=k`
/// the camera setting (0 bumper .. 4 high, VIEW_ORDER).
/// `NFSU2_INTRO_NIS=n` picks the start scene IntroNis<n> and
/// `NFSU2_SCENE_FRAME=f` runs on to its frame f (the scene's own
/// frames, `shop_scene::FPS`), to compare the flag girl and her
/// flares with a frame of the original's.
/// race_pause: the same Quick Race, then its pause menu (UI_pause
/// in a race, 0x4C2BD0) over the frozen race (Wine menus110 q0).
pub(super) fn qrstart(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    // NFSU2_QR_MODE=drift|sprint|drag|circuit: that mode's first
    // track (default Circuit).
    let mode = qr_mode_env();
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let e = game.quick_event(mode);
    game.quick.car = Some(std::env::var("NFSU2_QR_CAR").unwrap_or_else(|_| "PEUGOT".into()));
    if let Some(v) = std::env::var("NFSU2_VIEW").ok().and_then(|v| v.parse::<u8>().ok()) {
        game.settings.players[0].camera = v;
    }
    game.start_race(e, true, false)?;
    let frames = std::env::var("NFSU2_FRAMES").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    for _ in 0..frames {
        if std::env::var_os("NFSU2_THROTTLE").is_some() {
            game.platform.keys.insert(KeyCode::KeyW);
        }
        if std::env::var_os("NFSU2_LOOKBACK").is_some() {
            game.platform.keys.insert(KeyCode::KeyB);
        }
        // NFSU2_SHOT_STEP: the whole frame step (sounds, particles:
        // tyre smoke at the line), not only the simulation.
        if std::env::var_os("NFSU2_SHOT_STEP").is_some() {
            game.step_for(1.0 / 30.0)?;
        } else {
            game.advance_for(1.0 / 30.0)?;
        }
    }
    // INTRO_KEYS (as the intro shot): from the green light, the
    // player's keys and the log, to compare with the original.
    if std::env::var_os("INTRO_KEYS").is_some() {
        for _ in 0..30 * 60 {
            if matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 0.0) {
                break;
            }
            game.advance_for(1.0 / 30.0)?;
        }
        let d = game.sim.drive.as_ref().unwrap();
        let f = d.state.forward();
        let cm = d.state.position
            + glam::Mat3::from_quat(d.state.orientation) * Vec3::from(d.car.car_type.handling.centre_of_mass);
        eprintln!("qrstart: green, car at {cm:?} heading {:.2}", f.y.atan2(f.x).to_degrees());
        intro_drive(game)?;
    }
    if let Some(f) = std::env::var("NFSU2_SCENE_FRAME").ok().and_then(|v| v.parse::<f32>().ok()) {
        let mut n = 0;
        while game.scenes.shop.as_ref().is_some_and(|r| r.frame < f) && n < 6000 {
            // The particles too (the window's `step`), for the
            // flares' sparks and smoke.
            game.advance_for_each(1.0 / 60.0, |g, dt| {
                g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
                Ok(())
            })?;
            n += 1;
        }
        eprintln!("qrstart: scene {:?}", game.scenes.shop.as_ref().map(|r| (r.scene.kind.name(), r.frame)));
    }
    if screen == "race_pause" {
        // Past the start (scene / countdown), then Escape's pause.
        for _ in 0..30 * 60 {
            if game.scenes.shop.is_none() && matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 1.0) {
                break;
            }
            game.advance_for(1.0 / 30.0)?;
        }
        let Mode::Race(race) = std::mem::replace(&mut game.mode, Mode::Explore) else { anyhow::bail!("racing") };
        game.sim.paused_race = Some(race);
        game.mode = game.icon_menu(MenuKind::RacePause);
        game.pacing.started = Instant::now();
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `lapko`: a Quick Race circuit with Lap KO on (the mode
/// record's row: 3 laps, 3 opponents; race.knockout), full throttle
/// until 3 s after the green light.
pub(super) fn lapko(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let r = &mut game.settings.qr[nfsu2::frontend::quick_race::race_type(mode)];
    (r.lap_ko, r.laps, r.opponents) = (true, 3, 3);
    let e = game.quick_event(mode);
    game.quick.car = Some(std::env::var("NFSU2_QR_CAR").unwrap_or_else(|_| "PEUGOT".into()));
    game.start_race(e, true, false)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Race(r) if r.scoring.knockout), "a knockout race");
    for _ in 0..30 * 90 {
        if game.scenes.shop.is_none() && matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 3.0) {
            break;
        }
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
    }
    game.platform.keys.clear();
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `crashcam` (test harness): a Quick Race circuit (Peugeot 206),
/// full throttle until 4 s after the green light, then Car_Crash's
/// camera test as a car-against-car push of 30 or more leaves it
/// (`CarState::crash_hit`, crash mode), and NFSU2_CRASH_FRAMES
/// (default 20) 1/30 s frames more. NFSU2_CRASH_HOVER=1 holds the
/// car 2.5 m up in crash mode, so kind 7 (the second shot) follows
/// after 3 s of simulation (as the hover capture under Wine).
pub(super) fn crashcam(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some(std::env::var("NFSU2_QR_CAR").unwrap_or_else(|_| "PEUGOT".into()));
    game.start_race(e, true, false)?;
    if let (Mode::Race(r), Some(d)) = (&game.mode, game.sim.drive.as_ref())
        && let Some((nodes, far)) = crash_route(d, r.racers.ai_routes.as_ref())
    {
        eprintln!(
            "crashcam: track {:?} grid car {:?} far {far} nodes {:?}",
            r.racers.ai_routes.as_ref().map(|a| (a.track, a.backward)),
            d.state.position,
            nodes.iter().map(|n| (n.s - nodes[0].s, n.edge[0].x, n.edge[0].y)).collect::<Vec<_>>()
        );
    }
    for _ in 0..30 * 90 {
        if game.scenes.shop.is_none() && matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 4.0) {
            break;
        }
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
    }
    game.platform.keys.clear();
    // A rival put 20 m ahead, facing the player, both at
    // NFSU2_CRASH_SPEED m/s (default 28): a head-on push through
    // the clone's own contacts (as the teleport capture under Wine,
    // `crash.sh`). NFSU2_CRASH_FORCE=1 instead marks the
    // crash directly (Car_Crash's test).
    let speed: f32 = std::env::var("NFSU2_CRASH_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(28.0);
    // NFSU2_CRASH_AT=x,y,z,fx,fy,vx,vy,vz: the original's car +0x60
    // (taken as the clone's centre of mass), forward and velocity
    // when its crash camera started (`at.py` on a Wine log), and the
    // crash marked there: the shot's placement compared directly.
    let at: Option<Vec<f32>> = std::env::var("NFSU2_CRASH_AT")
        .ok()
        .map(|s| s.split(',').filter_map(|f| f.trim().parse().ok()).collect())
        .filter(|v: &Vec<f32>| v.len() == 8);
    if let Some(v) = at {
        let d = game.sim.drive.as_mut().context("a car")?;
        d.state.orientation = glam::Quat::from_rotation_z(v[4].atan2(v[3]));
        let com = d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
        d.state.position = Vec3::new(v[0], v[1], v[2]) - com;
        d.state.velocity = Vec3::new(v[5], v[6], v[7]);
        // Explicit synthetic/replay motion initialization; no native placement offset.
        d.state.publish_forward_speed();
        d.state.crash = true;
        d.state.crash_clear = None;
        d.state.crash_hit = true;
    } else if std::env::var_os("NFSU2_CRASH_FORCE").is_some() {
        let d = game.sim.drive.as_mut().context("a car")?;
        d.state.crash = true;
        d.state.crash_clear = None;
        d.state.crash_hit = true;
    } else if let (Mode::Race(r), Some(d)) = (&mut game.mode, game.sim.drive.as_mut()) {
        let f = d.state.forward();
        let f = Vec3::new(f.x, f.y, 0.0).normalize_or(Vec3::X);
        let rival = r.racers.rivals.first_mut().context("a rival")?;
        rival.state.position = d.state.position + f * 20.0 + Vec3::Z * 0.2;
        rival.state.orientation = glam::Quat::from_rotation_z(std::f32::consts::PI) * d.state.orientation;
        rival.state.velocity = -f * speed;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        rival.state.publish_forward_speed();
        rival.state.angular_momentum = Vec3::ZERO;
        rival.state.spin = Vec3::ZERO;
        d.state.velocity = f * speed;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        d.state.publish_forward_speed();
    }
    if let (Mode::Race(r), Some(d)) = (&game.mode, game.sim.drive.as_ref()) {
        eprintln!(
            "crashcam: option {} clock {:.2} placed {:?}",
            r.camera.events.enabled,
            r.timing.clock,
            crash_route(d, r.racers.ai_routes.as_ref()).map(|(n, far)| (n.len(), far, n[far].s - n[0].s))
        );
    }
    let hover = std::env::var_os("NFSU2_CRASH_HOVER").is_some();
    let lift = game.sim.drive.as_ref().map_or(0.0, |d| d.state.position.z) + 2.5;
    let frames = std::env::var("NFSU2_CRASH_FRAMES").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(20);
    for i in 0..frames {
        if hover
            && game.sim.drive.as_ref().is_some_and(|d| d.state.crash)
            && let Some(d) = game.sim.drive.as_mut()
        {
            d.state.position.z = lift;
            d.state.velocity = Vec3::ZERO;
            // Explicit synthetic/replay motion initialization; no native placement offset.
            d.state.publish_forward_speed();
            d.state.crash = true;
        }
        game.advance_for(1.0 / 30.0)?;
        if let (Mode::Race(r), Some(d)) = (&game.mode, game.sim.drive.as_ref()) {
            if i == 0
                && let Some(s) = r.camera.shot.as_ref()
            {
                eprintln!("crashcam: shot eye {:?} second {:?} side {} visible {}", s.eye, s.second, s.side, s.visible);
            }
            eprintln!(
                "crashcam {i} kind {} rate {:.2} left {:.2} shot {:?} eye {:?} fov {:?} car {:?}",
                r.camera.events.kind,
                r.camera.events.slow.rate,
                r.camera.events.slow.remaining,
                r.camera.shot.as_ref().map(|s| s.visible),
                d.camera.position,
                d.camera.game_fov.map(f32::to_degrees),
                d.state.position
            );
        }
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}
