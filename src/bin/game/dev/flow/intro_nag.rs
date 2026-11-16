use crate::*;

/// `--flow intro_nag`: Rachel's complaints after stage 0's races, as a
/// player meets them (checked against the original under Wine).
/// A new career through the D-Day script, idle until Rachel
/// reveals stage 0's races (kind 2, 15 s of free roam); then each of
/// STAGE_0_CIRCUIT_0, STAGE_0_CIRCUIT_1 and STAGE_0_SPRINT_0 started,
/// finished first and left by UI_PostRace's Return to Explore Mode (the
/// settle 0x54C950 → 0x5242D0 posts kind 1 in 4000 ticks), then free roam
/// idle. Fails unless: each complaint comes a second of driving after the
/// exit (Time (4, 1 / 4 / 8)); SMS_LOC_GO_TO_CAR_LOT_NOW arrives with the
/// second win (condition 12, stage 0, 2 wins); the third closes the races.
/// With audio (`NFSU2_AUDIO_WAV=-`) also: the exit stops the ending
/// scene's NIS line (0x481210(9)), and an Esc pause 0.5 s into the first
/// complaint holds the line and the driving time, Esc again lets both go
/// on (0x4810F0 / 0x481210). `NFSU2_FLOW_SECS` (default 6): the idle after
/// each race.
pub(crate) fn flow_intro_nag(game: &mut Game) -> Result<()> {
    use super::common::flow_tap as tap;
    fn run(game: &mut Game, frames: u32) -> Result<()> {
        for _ in 0..frames {
            game.advance_for(1.0 / 60.0)?;
        }
        Ok(())
    }
    fn log(game: &Game, t: f32, what: &str) {
        let Some(p) = game.profile.as_ref() else { return };
        let speech = game.audio.as_ref().and_then(|a| a.streams.speech_pos(&a.mixer));
        let nis = game.audio.as_ref().is_some_and(|a| a.streams.nis_busy(&a.mixer));
        println!(
            "[{t:6.2} s] {what}: mode {} | drive {:.2} | slots {:?} complaints {} | speech {speech:?} nis {nis} free {} | open {:08X?} closed {} | unread {}",
            flow_mode(&game.mode),
            p.drive_time,
            p.hints.slots.iter().filter(|s| s.1 != 0).collect::<Vec<_>>(),
            p.hints.complaints,
            game.speech_free(),
            p.open,
            p.intro_closed,
            p.inbox.unread()
        );
    }
    fn close_sms(game: &mut Game) -> Result<()> {
        for _ in 0..4 {
            if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) {
                tap(game, KeyCode::Escape)?;
            }
        }
        Ok(())
    }
    let secs: f32 = std::env::var("NFSU2_FLOW_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
    let audio = game.audio.is_some();
    game.mode = game.icon_menu(MenuKind::Main);
    tap(game, KeyCode::ArrowLeft)?;
    tap(game, KeyCode::Enter)?;
    tap(game, KeyCode::Enter)?;
    for _ in 0..8 {
        match &game.mode {
            Mode::Menu(Menu::McMain(m)) if m.step == McStep::Ask || m.step == McStep::Confirm => {
                tap(game, KeyCode::ArrowLeft)?;
                tap(game, KeyCode::Enter)?;
            }
            Mode::Menu(Menu::McMain(_) | Menu::McMessage(..) | Menu::Dialog(..)) => tap(game, KeyCode::Enter)?,
            _ => break,
        }
    }
    if matches!(game.mode, Mode::Menu(Menu::Movie(_))) {
        run(game, 60)?;
        tap(game, KeyCode::Enter)?;
        run(game, 30)?;
    }
    play_career_scene(game)?;
    let mut t = 0;
    while game.career_world.dday.running() && t < 40 * 60 {
        if matches!(game.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) && t % 60 == 59 {
            tap(game, KeyCode::Enter)?;
        }
        game.advance_for(1.0 / 60.0)?;
        t += 1;
    }
    anyhow::ensure!(
        !game.career_world.dday.running() && matches!(game.mode, Mode::Explore),
        "the D-Day script over ({})",
        flow_mode(&game.mode)
    );
    log(game, 0.0, "script over");
    // Rachel's reveal (kind 2, 15 s of free roam; with speech it waits
    // for her script lines to end).
    let mut t = 0;
    while !game.profile.as_ref().is_some_and(|p| p.intro_races) && t < 90 * 60 {
        close_sms(game)?;
        game.advance_for(1.0 / 60.0)?;
        t += 1;
    }
    anyhow::ensure!(game.profile.as_ref().is_some_and(|p| p.intro_races), "stage 0's races revealed");
    log(game, t as f32 / 60.0, "races revealed");
    let car_lot_now = nfsu2::formats::hash::bin_hash("SMS_LOC_GO_TO_CAR_LOT_NOW");
    for (n, event) in [dday::STAGE0_RACES[0], dday::STAGE0_RACES[1], dday::STAGE0_RACES[2]].into_iter().enumerate() {
        let n = n as u8 + 1;
        // Rachel's lines done, the SMS screens shut.
        let mut t = 0;
        while (!game.speech_free() || !matches!(game.mode, Mode::Explore)) && t < 60 * 60 {
            close_sms(game)?;
            game.advance_for(1.0 / 60.0)?;
            t += 1;
        }
        let e = game.game_data.career.events.iter().position(|e| e.unlock == event).context("a stage-0 race")?;
        game.start_race(e, false, false)?;
        for _ in 0..12 * 60 {
            game.platform.keys.insert(KeyCode::ArrowUp);
            game.advance_for(1.0 / 60.0)?;
        }
        if let Mode::Race(r) = &mut game.mode {
            r.racers.me.finished = Some(r.timing.clock.max(0.0));
        }
        game.platform.keys.clear();
        for _ in 0..60 * 60 {
            if matches!(game.mode, Mode::Menu(Menu::Results(..))) {
                break;
            }
            game.advance_for(1.0 / 60.0)?;
        }
        anyhow::ensure!(
            matches!(game.mode, Mode::Menu(Menu::Results(..))),
            "race {n}: the results ({})",
            flow_mode(&game.mode)
        );
        for _ in 0..4 {
            if matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))) {
                break;
            }
            game.menu_key(Key::Accept)?;
            run(game, 30)?;
        }
        log(game, 0.0, &format!("race {n}: post race"));
        let nis_before = game.audio.as_ref().is_some_and(|a| a.streams.nis_busy(&a.mixer));
        game.act(Action::ReturnToExplore, MenuKind::PostRace)?;
        for _ in 0..20 {
            match &game.mode {
                Mode::Explore => break,
                // The post-race save question (No focused): No.
                Mode::Menu(Menu::McMessage(..) | Menu::Dialog(..) | Menu::McMain(_)) => tap(game, KeyCode::Enter)?,
                _ => game.advance_for(1.0 / 60.0)?,
            }
        }
        anyhow::ensure!(matches!(game.mode, Mode::Explore), "race {n}: back in free roam ({})", flow_mode(&game.mode));
        let p = game.profile.as_ref().context("the profile")?;
        anyhow::ensure!(
            p.hints.complaints == n - 1 && p.hints.slots.iter().any(|s| s.1 == 1),
            "race {n}: the complaint posted at the exit (0x5242D0)"
        );
        anyhow::ensure!(
            p.inbox.contains(car_lot_now) == (n >= 2),
            "race {n}: SMS_LOC_GO_TO_CAR_LOT_NOW with the second win"
        );
        let nis_after = game.audio.as_ref().is_some_and(|a| a.streams.nis_busy(&a.mixer));
        log(game, 0.0, &format!("race {n}: explore (NIS line before the exit {nis_before})"));
        anyhow::ensure!(!nis_after, "race {n}: the exit stops the NIS line (0x481210(9))");
        let drive0 = game.profile.as_ref().map_or(0.0, |p| p.drive_time);
        let mut fired = None;
        let mut frames = 0u32;
        while (frames as f32) < secs * 60.0 {
            let t = frames as f32 / 60.0;
            let p = game.profile.as_ref().context("the profile")?;
            if fired.is_none() && p.hints.complaints == n {
                fired = Some(p.drive_time - drive0);
                log(game, t, &format!("race {n}: complaint {n}"));
                // The first: Esc half a second into the line, 3 s held.
                if n == 1 && audio {
                    run(game, 30)?;
                    tap(game, KeyCode::Escape)?;
                    anyhow::ensure!(
                        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))),
                        "the explore pause"
                    );
                    let at = |g: &Game| {
                        (
                            g.audio.as_ref().and_then(|a| a.streams.speech_pos(&a.mixer)),
                            g.profile.as_ref().map_or(0.0, |p| p.drive_time),
                        )
                    };
                    let (held, drive) = at(game);
                    log(game, t, "Esc");
                    run(game, 180)?;
                    let (still, drive2) = at(game);
                    anyhow::ensure!(
                        held.is_some_and(|(_, h)| h) && held == still && drive == drive2,
                        "the pause holds Rachel's line and the driving time ({held:?} → {still:?}, {drive} → {drive2})"
                    );
                    tap(game, KeyCode::Escape)?;
                    run(game, 30)?;
                    let (after, _) = at(game);
                    log(game, t, "Esc again");
                    anyhow::ensure!(
                        matches!(game.mode, Mode::Explore)
                            && after.is_some_and(|(pos, h)| !h && pos > held.map_or(0.0, |x| x.0)),
                        "the resume lets the line go on ({after:?})"
                    );
                }
            }
            close_sms(game)?;
            game.advance_for(1.0 / 60.0)?;
            frames += 1;
        }
        let p = game.profile.as_ref().context("the profile")?;
        log(game, secs, &format!("race {n}: after {secs} s"));
        let fired = fired.with_context(|| format!("race {n}: no complaint in {secs} s"))?;
        anyhow::ensure!(
            (0.9..1.2).contains(&fired),
            "race {n}: the complaint a second of driving after the exit ({fired:.2} s)"
        );
        anyhow::ensure!(
            p.intro_closed == (n == 3) && p.open.is_empty() == (n == 3),
            "race {n}: the third complaint closes stage 0's races"
        );
    }
    Ok(())
}
