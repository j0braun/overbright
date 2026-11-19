//! The rate gate's flows (docs/developing/checks.md "Rate gate"): `--flow menus`,
//! `--flow garage`, `--flow scenes`. Each is a fixed script of keys at
//! world-update (tick) indices, driven tick by tick (`ticks::Frames`) in
//! drawn frames on the rate run's schedule (`NFSU2_RATE`; without it 1/60 s
//! frames), every frame's draw-side state updates included
//! (`rate::drawn`). A step happens AT its tick, between ticks, also on
//! frames that hold two (a key does not wait for the next frame
//! boundary). The scripts check nothing about the game: the state
//! hashes (`NFSU2_STATE_HASH`) are what the rate gate compares. Not
//! native: a measuring aid.
use super::super::rate;
use crate::*;

/// One script step: a key tapped (pressed, held for one tick, released),
/// a mode set directly (a screen entered as the shots enter it), or a key
/// repeated (one tap every `gap` ticks) while a test holds, at most `max`
/// times.
enum Op {
    Tap(KeyCode),
    Set(fn(&mut Game) -> Result<()>),
    TapWhile(KeyCode, fn(&Game) -> bool, u32),
}

/// Runs `script` (ticks to wait before each step, the step), then `tail`
/// ticks more. Logs each step's tick and the mode reached.
fn run(game: &mut Game, name: &str, script: &[(u64, Op)], tail: u64) -> Result<()> {
    let start = rate::ticks();
    let mut frames = super::super::ticks::Frames::new(60.0);
    let mut wait = |game: &mut Game, n: u64| -> Result<()> {
        for _ in 0..n {
            frames.tick(game)?;
        }
        Ok(())
    };
    // A tap through the input queue (`Game::input_at`), pressed at
    // the start of the next tick and let go at the one after.
    let tap = |game: &mut Game, k: KeyCode| game.tap_next(k);
    for (i, (gap, op)) in script.iter().enumerate() {
        wait(game, *gap)?;
        match op {
            Op::Tap(k) => tap(game, *k),
            Op::Set(f) => f(game)?,
            Op::TapWhile(k, test, max) => {
                let mut n = 0;
                while test(game) && n < *max {
                    tap(game, *k);
                    wait(game, 12)?;
                    n += 1;
                }
            }
        }
        println!("{name} step {i} at tick {}: {}", rate::ticks() - start, flow_mode(&game.mode));
    }
    wait(game, tail)?;
    game.platform.keys.clear();
    println!("{name}: {} ticks, ends in {}", rate::ticks() - start, flow_mode(&game.mode));
    Ok(())
}

/// The icon menu's selected option's action, if one is up.
fn selected(game: &Game) -> Option<Action> {
    match &game.mode {
        Mode::Menu(Menu::Icons(m, _)) => m.options.get(m.selected).map(|o| o.action),
        _ => None,
    }
}

/// `--flow menus`: the front end without a world, keyed: the main menu,
/// Career and back, Options (Audio and Display opened and closed), then
/// Quick Race: Circuit, the track select (a step right and back: the map
/// widget's hand-off and zoom), the track, the car lot (Right three times,
/// the crib's swings), on into the race and its start scene.
pub(crate) fn flow_menus(game: &mut Game) -> Result<()> {
    let main: fn(&mut Game) -> Result<()> = |g| {
        g.mode = g.icon_menu(MenuKind::Main);
        Ok(())
    };
    let script = [
        (0, Op::Set(main)),
        (40, Op::Tap(KeyCode::ArrowLeft)),
        (20, Op::Tap(KeyCode::Enter)),
        (60, Op::Tap(KeyCode::Escape)),
        (40, Op::TapWhile(KeyCode::ArrowRight, |g| selected(g).is_some_and(|a| a != Action::Options), 8)),
        (20, Op::Tap(KeyCode::Enter)),
        (40, Op::Tap(KeyCode::Enter)),
        (60, Op::Tap(KeyCode::ArrowDown)),
        (20, Op::Tap(KeyCode::Escape)),
        (40, Op::Tap(KeyCode::ArrowRight)),
        (20, Op::Tap(KeyCode::Enter)),
        (60, Op::Tap(KeyCode::Escape)),
        (40, Op::Tap(KeyCode::Escape)),
        (40, Op::TapWhile(KeyCode::ArrowLeft, |g| selected(g).is_some_and(|a| a != Action::QuickRace), 8)),
        (20, Op::Tap(KeyCode::Enter)),
        (40, Op::Tap(KeyCode::Enter)),
        (30, Op::Tap(KeyCode::ArrowRight)),
        (40, Op::Tap(KeyCode::ArrowLeft)),
        (60, Op::Tap(KeyCode::Enter)),
        (60, Op::Tap(KeyCode::ArrowRight)),
        (30, Op::Tap(KeyCode::ArrowRight)),
        (30, Op::Tap(KeyCode::ArrowRight)),
        (60, Op::Tap(KeyCode::Enter)),
        (60, Op::TapWhile(KeyCode::Enter, |g| matches!(g.mode, Mode::Menu(_)), 6)),
    ];
    run(game, "menus", &script, 600)
}

/// `--flow garage`: a fresh career's car in the front end's garage: the
/// body shop's parts (a list opened, stepped, closed), then Hydraulics
/// with a level-2 part fitted (the FE car pumping, a step), then the
/// Customize categories, then the crib's World Map (an event selected and
/// back to the region: the map widget's hand-offs and zoom).
pub(crate) fn flow_garage(game: &mut Game) -> Result<()> {
    let mut p = Profile::new("GARAGE");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    let parts: fn(&mut Game) -> Result<()> = |g| {
        g.mode = g.parts_browser(0, None);
        Ok(())
    };
    let hydro: fn(&mut Game) -> Result<()> = |g| {
        nfsu2::frontend::career::UNLOCK_ALL.store(true, std::sync::atomic::Ordering::Relaxed);
        install_shot_hydraulics(g);
        g.mode = g.specialties_browser(5, Some(1));
        Ok(())
    };
    let customize: fn(&mut Game) -> Result<()> = |g| {
        g.mode = g.icon_menu(MenuKind::CustomizeCategory);
        Ok(())
    };
    let world_map: fn(&mut Game) -> Result<()> = |g| {
        g.mode = g.world_map();
        Ok(())
    };
    let script = [
        (0, Op::Set(parts)),
        (60, Op::Tap(KeyCode::Enter)),
        (90, Op::Tap(KeyCode::ArrowRight)),
        (60, Op::Tap(KeyCode::ArrowRight)),
        (60, Op::Tap(KeyCode::Escape)),
        (60, Op::Tap(KeyCode::ArrowRight)),
        (60, Op::Tap(KeyCode::Enter)),
        (90, Op::Tap(KeyCode::Escape)),
        (60, Op::Set(hydro)),
        (240, Op::Tap(KeyCode::ArrowRight)),
        (180, Op::Tap(KeyCode::ArrowLeft)),
        (120, Op::Set(customize)),
        (60, Op::Tap(KeyCode::ArrowRight)),
        (40, Op::Tap(KeyCode::ArrowRight)),
        (60, Op::Set(world_map)),
        (60, Op::Tap(KeyCode::ArrowDown)),
        (10, Op::Tap(KeyCode::ArrowDown)),
        (70, Op::Tap(KeyCode::ArrowUp)),
        (40, Op::Tap(KeyCode::ArrowUp)),
    ];
    run(game, "garage", &script, 120)
}

/// `--flow scenes`: the scenes that hold the cars (1/30 s updates): the
/// career's start scene over the airport played through into free roam,
/// the dyno's DynoEnter scene into its menu, and stage 0's first race's
/// start scene through the green (the ending → post-race is `postrace`).
pub(crate) fn flow_scenes(game: &mut Game) -> Result<()> {
    let mut p = Profile::new("SCENES");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.explore(None)?;
    game.scenes.shop = None;
    anyhow::ensure!(game.start_career_scene(), "the career's start scene did not start");
    let career: fn(&mut Game) -> Result<()> = |_| Ok(());
    let dyno: fn(&mut Game) -> Result<()> = |g| {
        g.dyno.tuning_mode = 0;
        g.mode = g.enter_dyno_scene();
        Ok(())
    };
    let race: fn(&mut Game) -> Result<()> = |g| {
        g.explore(None)?;
        let e = g
            .game_data
            .career
            .events
            .iter()
            .find(|e| e.unlock == dday::STAGE0_RACES[0])
            .map(|e| e.index)
            .context("stage 0's first race")?;
        g.start_race(e, false, false)
    };
    let gas: fn(&mut Game) -> Result<()> = |g| {
        g.hold_next(KeyCode::ArrowUp, true);
        Ok(())
    };
    let script = [(0, Op::Set(career)), (480, Op::Set(dyno)), (360, Op::Set(race)), (420, Op::Set(gas))];
    run(game, "scenes", &script, 300)
}
