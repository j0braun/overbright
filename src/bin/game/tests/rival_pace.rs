//! The rivals' pace in a career race with the player's car held at the
//! grid (diagnostic recorder; compared against the original's memory
//! reads of the same event).
use super::*;
use super::testkit;

/// Diagnostic: per rival every 0.5 s of race clock, from the green light:
/// "t rival along_m x y speed target catch_up gap_m mover gear rpm gas brake
/// nitrous streamed skill" lines to `RIVAL_PACE_OUT`, for the career event
/// `RIVAL_PACE_EVENT` started from the native save `RIVAL_PACE_SAVE` (its
/// options and difficulty applied), `RIVAL_PACE_SECS` seconds (default 60).
/// `RIVAL_PACE_DIFFICULTY` = 0/1/2 overrides the save's difficulty;
/// `RIVAL_PACE_SEED` seeds the shared generator; `RIVAL_PACE_POINTS` =
/// seconds: each rival's route points (record:cp, s, lane-0 speed) then,
/// on stderr;
/// `RIVAL_PACE_DRIVE` = seconds: the player's car drives on the AI racer's
/// controller (`Autopilot`) from the green for that long, then stops.
#[test]
#[ignore = "diagnostic: records the rivals' pace of a career race"]
fn record_rival_pace() {
    testkit::game();
    let name = std::env::var("RIVAL_PACE_EVENT").unwrap();
    let out = std::env::var("RIVAL_PACE_OUT").unwrap();
    let secs: f32 = std::env::var("RIVAL_PACE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let mut game = Game::new().unwrap();
    // RIVAL_PACE_SEED: another state of the shared generator (the grid draw
    // and the rivals' picks), for a second run.
    if let Some(seed) = std::env::var("RIVAL_PACE_SEED").ok().and_then(|s| s.parse().ok()) {
        game.rng = nfsu2::opponents::GameRng(seed);
    }
    if let Ok(path) = std::env::var("RIVAL_PACE_SAVE") {
        let ctx = career::NativeContext::get().unwrap();
        let s = nfsu2::formats::native_save::NativeSave::parse(&std::fs::read(path).unwrap()).unwrap();
        game.settings.apply_native(&s);
        let p = Profile::from_native(&s, ctx);
        profiles::set_current(&p.name, s.clone());
        game.profile = Some(p);
    } else {
        let mut p = Profile::new("PACE");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        game.profile = Some(p);
    }
    if let Some(d) = std::env::var("RIVAL_PACE_DIFFICULTY").ok().and_then(|v| v.parse().ok()) {
        game.settings.difficulty = d;
    }
    let event = game.game_data.career.events.iter().position(|e| e.name == name).unwrap();
    if std::env::var_os("RIVAL_PACE_NO_TRAFFIC").is_some() {
        game.game_data.career.events[event].traffic = 0;
    }
    // RIVAL_PACE_QUICK=1: the harness's start without the career set-up's
    // catch-up (as the earlier trail recorder started it).
    let quick = std::env::var_os("RIVAL_PACE_QUICK").is_some();
    game.start_race(event, quick, false).unwrap();
    while game.scenes.shop.is_some() && matches!(game.mode, Mode::Race(_)) {
        game.advance_for(1.0 / 60.0).unwrap();
    }
    let mut text = String::new();
    let mut header = false;
    let mut next = 0.0f32;
    let drive: f32 = std::env::var("RIVAL_PACE_DRIVE").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let mut driving = false;
    loop {
        let Mode::Race(race) = &mut game.mode else { break };
        let clock = race.timing.clock;
        if drive > 0.0 && clock >= 0.0 && !driving && clock < drive {
            if let (Some(routes), Some(d)) = (race.racers.ai_routes.as_ref(), game.sim.drive.as_ref()) {
                race.autopilot = Some(Autopilot::new(d, routes, false));
                driving = true;
            }
        }
        if driving && clock >= drive {
            race.autopilot = None;
            driving = false;
        }
        if clock >= secs {
            break;
        }
        if !header {
            header = true;
            text += &format!(
                "# event {name} difficulty {} laps {} length {:.0} traffic {} catch_up {:?}\n",
                game.sim.race_globals.difficulty,
                race.laps,
                race.route.road.length,
                race.racers.traffic.is_some(),
                race.catch_up
            );
            for (i, r) in race.racers.rivals.iter().enumerate() {
                text += &format!("# rival {} {} skill {} slot {}\n", i + 1, r.car.car_type.name, r.speed.skill(), r.slot);
            }
        }
        let length = race.route.road.length;
        if let Some(at) = std::env::var("RIVAL_PACE_POINTS").ok().and_then(|s| s.parse::<f32>().ok())
            && clock >= at
            && clock < at + 1.0 / 60.0
        {
            for r in &race.racers.rivals {
                if let Some(route) = r.route.as_ref() {
                    eprintln!("points v {:.1} {}", r.state.velocity.truncate().length(), route.debug());
                }
            }
        }
        if clock >= next {
            next += 0.5;
            let me = race.racers.me.progress();
            if let Some(d) = game.sim.drive.as_ref() {
                let p = d.state.position;
                text += &format!(
                    "{clock:.2} 0 {:.1} {:.1} {:.1} {:.2} 0 1 0 0 0 0 0 0 0 0 0 Human 0 0\n",
                    me * length,
                    p.x,
                    p.y,
                    d.state.velocity.truncate().length()
                );
            }
            for (i, r) in race.racers.rivals.iter().enumerate() {
                let p = r.state.position;
                let mover = if r.belt.belt.is_some() {
                    4
                } else if r.state.melo.is_some() {
                    3
                } else {
                    1
                };
                let (gear, rpm) = r.state.drivetrain.as_ref().map_or((0, 0.0), |t| (t.gear, r.state.rpm()));
                text += &format!(
                    "{clock:.2} {} {:.1} {:.1} {:.1} {:.2} {:.2} {:.3} {:.1} {mover} {gear} {rpm:.0} {:.2} {:.2} {} {} {} {:?} {} {:.2}\n",
                    i + 1,
                    r.progress.progress() * length,
                    p.x,
                    p.y,
                    r.state.velocity.truncate().length(),
                    r.driver.target_speed,
                    r.speed.catch_up(),
                    (r.progress.progress() - me) * length,
                    r.controls.gas,
                    r.controls.brake,
                    u8::from(r.state.nitrous.as_ref().is_some_and(|t| t.running.is_some())),
                    u8::from(r.state.streamed),
                    r.speed.skill(),
                    r.actions.kind,
                    u8::from(r.unstuck.since.is_some()),
                    r.belt.block,
                );
            }
        }
        game.advance_for(1.0 / 60.0).unwrap();
    }
    std::fs::write(out, text).unwrap();
}

/// Diagnostic: the URL events a native save opens (`RIVAL_PACE_SCAN` = save
/// paths separated by ':').
#[test]
#[ignore = "diagnostic: lists the URL events each save opens"]
fn scan_url_events() {
    testkit::game();
    let game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    let data = &game.game_data.career;
    for path in std::env::var("RIVAL_PACE_SCAN").unwrap().split(':') {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let Ok(s) = nfsu2::formats::native_save::NativeSave::parse(&bytes) else {
            println!("{path}: not a save");
            continue;
        };
        let mut p = Profile::from_native(&s, ctx);
        let mut rng = nfsu2::opponents::GameRng(1);
        p.refresh_open(data, &mut rng);
        let open: Vec<String> = p
            .open_events(data)
            .iter()
            .filter(|e| e.category == career::Category::Url)
            .map(|e| format!("{} ({:?} track {} laps {})", e.name, e.kind, e.track, e.laps))
            .collect();
        println!("{path}: stage {} car {} url {:?}", p.stage, p.car, open);
    }
}
