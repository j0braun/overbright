use crate::*;

/// `--flow resume`: a saved career (`NFSU2_FLOW_LOAD`, from
/// `NFSU2_FLOW_DATA`) resumed as a player does — Career, Resume Career, the
/// crib's "Return to Explore Mode" — then `NFSU2_FLOW_SECS` (default 120)
/// seconds of driving with the throttle held, logging every Rachel line
/// (run with `NFSU2_AUDIO_WAV` and `NFSU2_AUDIO_DEBUG` for the speech).
pub(crate) fn flow_resume(game: &mut Game) -> Result<()> {
    let tap = flow_tap;
    let name = std::env::var("NFSU2_FLOW_LOAD").context("--flow resume needs NFSU2_FLOW_LOAD")?;
    let secs: f32 = std::env::var("NFSU2_FLOW_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(120.0);
    let hints = |game: &Game, what: &str| {
        if let Some(p) = game.profile.as_ref() {
            println!(
                "[game {:7.2}] {what}: mode {} stage {} intro {:08X?} intro_played {} dday {} | hints slots {:?} zone {:08X} drive_sum {} | drive_time {:.1}",
                game.sim.clock_ms / 1000.0,
                flow_mode(&game.mode),
                p.stage,
                p.intro,
                p.intro_played,
                game.career_world.dday.state,
                p.hints.slots,
                p.hints.zone,
                p.hints.drive_sum,
                p.drive_time
            );
        }
    };
    game.mc_act(McAct::Load(name))?;
    game.menu_key(Key::Accept)?;
    hints(game, "loaded");
    game.mode = game.icon_menu(MenuKind::Main);
    tap(game, KeyCode::ArrowLeft)?;
    tap(game, KeyCode::Enter)?;
    hints(game, "career menu");
    tap(game, KeyCode::Enter)?;
    hints(game, "resume career");
    for _ in 0..120 {
        game.advance_for(1.0 / 60.0)?;
    }
    if matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))) {
        tap(game, KeyCode::Enter)?;
        // The question ("leave the garage?") opens on No.
        if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
            tap(game, KeyCode::ArrowLeft)?;
            tap(game, KeyCode::Enter)?;
        }
        for _ in 0..600 {
            if matches!(game.mode, Mode::Explore) {
                break;
            }
            game.advance_for(1.0 / 60.0)?;
        }
    }
    hints(game, "explore");
    // NFSU2_FLOW_AT=x,y,z,deg: the car put there once in free roam;
    // NFSU2_FLOW_IDLE=1: no throttle; NFSU2_FLOW_WEATHER=1: the weather
    // (target, amount, wetness, world clock, camera x / y) every second.
    if let Some(v) = std::env::var("NFSU2_FLOW_AT")
        .ok()
        .map(|s| s.split(',').filter_map(|f| f.trim().parse::<f32>().ok()).collect::<Vec<_>>())
        .filter(|v| v.len() == 4)
        && let Some(d) = game.sim.drive.as_mut()
    {
        d.place(Vec3::new(v[0], v[1], v[2]), v[3].to_radians());
    }
    let idle = std::env::var_os("NFSU2_FLOW_IDLE").is_some();
    let weather_log = std::env::var_os("NFSU2_FLOW_WEATHER").is_some();
    let mut last = String::new();
    let mut frames = 0u32;
    while (frames as f32) < secs * 60.0 {
        if weather_log && frames % 60 == 0 {
            if let Some(d) = game.sim.drive.as_ref() {
                println!(
                    "weather: t {:.0} mode {} clock {:.2} cycle {:.3} target {:.3} amount {:.3} wet {:.3} cam {:.1} {:.1} cover {} car {:.2} {:.2} {:.2} heading {:.1}",
                    frames as f32 / 60.0,
                    flow_mode(&game.mode),
                    d.clock,
                    nfsu2::weather::cycle(d.clock, d.camera.position.x),
                    d.weather.target,
                    d.weather.amount,
                    d.weather.wet,
                    d.camera.position.x,
                    d.camera.position.y,
                    d.weather.under_cover,
                    d.state.position.x,
                    d.state.position.y,
                    d.state.position.z,
                    d.state.heading().to_degrees()
                );
            } else {
                println!("weather: t {:.0} mode {} no drive", frames as f32 / 60.0, flow_mode(&game.mode));
            }
        }
        let state = flow_mode(&game.mode);
        if state != last {
            hints(game, "mode");
            last = state;
        }
        if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) && frames % 60 == 0 {
            tap(game, KeyCode::Enter)?;
        } else if !idle && matches!(game.mode, Mode::Explore) && !game.platform.keys.contains(&KeyCode::ArrowUp) {
            game.key(KeyCode::ArrowUp, &mut || {})?;
            game.platform.keys.insert(KeyCode::ArrowUp);
        }
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
        if frames % 600 == 0 {
            hints(game, "tick");
        }
    }
    game.platform.keys.clear();
    hints(game, "end");
    Ok(())
}
