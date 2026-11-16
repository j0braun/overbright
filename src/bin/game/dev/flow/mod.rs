use crate::*;

pub(crate) mod common;
pub(crate) mod engine;
pub(crate) mod intro_nag;
pub(crate) mod outrun;
pub(crate) mod rain;
pub(crate) mod rate_flows;
pub(crate) mod resume;
pub(crate) mod soak;

/// A check aid (`--flow newcareer`): a player's path from the main menu
/// into a new career and the D-Day intro, headless and keyed as a player
/// keys it (the window's key handler), logging each step. Its save folder
/// is a fresh one, or `NFSU2_FLOW_DATA=dir` (a copy of the profiles,
/// never the player's own; `NFSU2_FLOW_LOAD=name` loads one first); `NFSU2_AUDIO_WAV` runs the mixer offline;
/// `NFSU2_FLOW_SMS=esc` leaves the SMS screens with Esc instead of Enter.
pub(crate) fn flow(game: &mut Game, what: &str) -> Result<()> {
    fn press(game: &mut Game, code: KeyCode) -> Result<()> {
        game.key(code, &mut || {})?;
        game.platform.keys.insert(code);
        Ok(())
    }
    use common::flow_tap as tap;
    if what == "parity" {
        let plan = std::env::args().nth(3).context("--flow parity <plan> (tools/parity/parity.py writes it)")?;
        return parity::flow_parity(game, &plan);
    }
    anyhow::ensure!(
        matches!(
            what,
            "newcareer"
                | "soak"
                | "dday"
                | "resume"
                | "engine"
                | "racelines"
                | "intro_nag"
                | "rain"
                | "outrun"
                | "firstrace"
                | "firstdrag"
                | "postrace"
                | "menus"
                | "garage"
                | "scenes"
        ),
        "flows: newcareer, soak, dday, resume, engine, racelines, intro_nag, rain, outrun, firstrace, firstdrag, postrace, menus, garage, scenes"
    );
    if let Some(r) = super::rate::name() {
        println!("rate run: schedule {r}");
    }
    super::rate::start(game);
    match what {
        "menus" => return rate_flows::flow_menus(game),
        "garage" => return rate_flows::flow_garage(game),
        "scenes" => return rate_flows::flow_scenes(game),
        _ => {}
    }
    if what == "postrace" {
        return post_race::flow_post_race(game);
    }
    if what == "firstdrag" {
        return first_drag::flow_first_drag(game);
    }
    if what == "firstrace" {
        return first_race::flow_first_race(game);
    }
    if what == "outrun" {
        return flow_outrun(game);
    }
    if what == "rain" {
        return flow_rain(game);
    }
    if what == "intro_nag" {
        return flow_intro_nag(game);
    }
    // The selftest's race-lines case on its own (a fresh career past the
    // intro).
    if what == "racelines" {
        let mut p = Profile::new("LINES");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        game.profile = Some(p);
        game.explore(None)?;
        return selftest_race_lines_end(game);
    }
    if what == "soak" {
        return soak(game);
    }
    if what == "resume" {
        return flow_resume(game);
    }
    if what == "engine" {
        return flow_engine(game);
    }
    // The selftest's D-Day save paths on their own.
    if what == "dday" {
        return selftest_dday_paths(game);
    }
    let t0 = game.now();
    let log = |game: &Game, what: &str| {
        let p = game.profile.as_ref();
        let car = game.here().map(|(at, _)| at);
        println!(
            "[{:7.2} s game {:6.2}] {what}: mode {} | world clock {:.2} sky {:?} | dday {} read {} closed {} | unread {} inbox {:08X?} | car {:?} speed {:.2} | speech free {}",
            (game.now() - t0) / 1000.0,
            game.sim.clock_ms / 1000.0,
            flow_mode(&game.mode),
            game.sim.drive.as_ref().map_or(-1.0, |d| d.clock),
            game.sim.drive.as_ref().map(|d| d.time_of_day.sky()),
            game.career_world.dday.state,
            game.career_world.dday.read,
            game.career_world.dday.closed,
            p.map_or(0, |p| p.inbox.unread()),
            p.map_or(Vec::new(), |p| p.inbox.entries.iter().map(|e| e.message).collect::<Vec<_>>()),
            car,
            game.sim.drive.as_ref().map_or(0.0, |d| d.state.speed()),
            game.speech_free(),
        );
    };
    log(game, "start");
    if let Ok(name) = std::env::var("NFSU2_FLOW_LOAD") {
        game.mc_act(McAct::Load(name))?;
        log(game, "loaded");
        game.menu_key(Key::Accept)?;
    }
    game.mode = game.icon_menu(MenuKind::Main);
    // The main menu opens on Quick Race: Left to Career, Enter; then the
    // career menu's first option.
    tap(game, KeyCode::ArrowLeft)?;
    tap(game, KeyCode::Enter)?;
    if let Mode::Menu(Menu::Icons(m, _)) = &game.mode {
        let labels: Vec<String> = m
            .options
            .iter()
            .map(|o| format!("{} ({:?})", game.game_data.assets.strings.get(o.label).unwrap_or("?"), o.action))
            .collect();
        println!("career menu options: {labels:?}");
    }
    log(game, "career menu");
    tap(game, KeyCode::Enter)?;
    log(game, "first career option");
    // MC_Main: Yes, the name as offered, save: Yes (and any question after).
    if matches!(game.mode, Mode::Menu(Menu::McMain(_))) {
        tap(game, KeyCode::ArrowLeft)?;
    }
    for _ in 0..8 {
        if matches!(game.mode, Mode::Menu(Menu::Movie(_)) | Mode::Explore) {
            break;
        }
        if matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Confirm) {
            tap(game, KeyCode::ArrowLeft)?;
        }
        tap(game, KeyCode::Enter)?;
        log(game, "enter");
    }
    // The movie: Enter after a second.
    for _ in 0..60 {
        game.advance_for(1.0 / 60.0)?;
    }
    tap(game, KeyCode::Enter)?;
    for _ in 0..30 {
        if matches!(game.mode, Mode::Explore) {
            break;
        }
        game.advance_for(1.0 / 60.0)?;
    }
    log(game, "after the movie");
    // The start scene: Enter after `NFSU2_FLOW_SCENE` seconds (default 2).
    let wait: f32 = std::env::var("NFSU2_FLOW_SCENE").ok().and_then(|v| v.parse().ok()).unwrap_or(2.0);
    let mut t = 0.0;
    while game.scenes.shop.is_some() && t < wait {
        game.advance_for(1.0 / 60.0)?;
        t += 1.0 / 60.0;
    }
    if game.scenes.shop.is_some() {
        tap(game, KeyCode::Enter)?;
    }
    log(game, "scene over");
    let esc = std::env::var("NFSU2_FLOW_SMS").is_ok_and(|v| v == "esc");
    let mut last = String::new();
    let mut frames = 0u32;
    let mut sms_frames = 0u32;
    let mut throttle = false;
    while frames < 60 * 40 {
        let state = format!("{} {} {} {}", flow_mode(&game.mode), game.career_world.dday.state, game.career_world.dday.read, game.career_world.dday.closed);
        if state != last {
            log(game, "change");
            last = state;
        }
        if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) {
            sms_frames += 1;
            // The player reads for a second, then Enter (or Esc).
            if sms_frames % 60 == 0 {
                tap(game, if esc { KeyCode::Escape } else { KeyCode::Enter })?;
                continue;
            }
        } else if !game.career_world.dday.running() && !throttle && matches!(game.mode, Mode::Explore) {
            press(game, KeyCode::ArrowUp)?;
            throttle = true;
            log(game, "throttle down");
        }
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
        if frames % 300 == 0 {
            log(game, "tick");
        }
    }
    game.platform.keys.clear();
    log(game, "end");
    Ok(())
}
