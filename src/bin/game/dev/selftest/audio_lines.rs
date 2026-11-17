use crate::*;

/// The race's lines end with it (the spectators' shouting could still
/// play after the race). The start scene's line
/// (the starter, the crowd, NIS kind 2) and the ending scene's (the crowd
/// at the line, kind 6, ~50 s long) are NIS stream lines; the original
/// purges them (and the speech line) as the post-race menu or the pause
/// is left by Restart, Quit to Main Menu or Return to Explore Mode
/// (0x481210 with 3 / 6 / 9, `lines_purged_by`), and holds them while
/// paused. Each case runs on an offline mixer: the race's line is checked
/// playing, then gone 3 s after the exit.
pub(crate) fn selftest_race_lines_end(game: &mut Game) -> Result<()> {
    fn run(game: &mut Game, secs: f32) -> Result<()> {
        for _ in 0..(secs * 60.0) as u32 {
            game.advance_for(1.0 / 60.0)?;
        }
        Ok(())
    }
    fn line(game: &Game) -> Option<(u32, u8)> {
        game.audio.as_ref().and_then(|a| a.streams.nis_line().filter(|l| a.mixer.track_playing(l.0)))
    }
    fn playing(game: &Game, id: u32) -> bool {
        game.audio.as_ref().is_some_and(|a| a.mixer.track_playing(id))
    }
    fn confirm(game: &mut Game) -> Result<()> {
        for _ in 0..4 {
            if !matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
                break;
            }
            game.menu_key(Key::Left)?;
            game.menu_key(Key::Accept)?;
        }
        Ok(())
    }
    // A stage-1 circuit with its start scene; its line (kind 2) plays.
    fn start(game: &mut Game, e: usize) -> Result<u32> {
        game.start_race(e, false, false)?;
        run(game, 1.0)?;
        let (id, kind) = line(game).context("the start scene's line plays")?;
        anyhow::ensure!(kind == 2, "the start scene's line is NIS kind 2 ({kind})");
        Ok(id)
    }
    // The race finished at once (as `--flow soak`'s race), up to Post Race;
    // the ending scene's line (kind 6) plays through the results.
    fn finish(game: &mut Game) -> Result<u32> {
        for _ in 0..40 * 60 {
            if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 1.0) {
                break;
            }
            game.advance_for(1.0 / 60.0)?;
        }
        if let Mode::Race(r) = &mut game.mode {
            r.racers.me.finished = Some(r.timing.clock.max(0.0));
        }
        for _ in 0..60 * 60 {
            game.advance_for(1.0 / 60.0)?;
            if matches!(game.mode, Mode::Menu(Menu::Results(..))) {
                break;
            }
        }
        anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Results(..))), "the results ({})", flow_mode(&game.mode));
        let (id, kind) = line(game).context("the ending scene's line plays over the results")?;
        anyhow::ensure!(kind == 6, "the ending's line is NIS kind 6 ({kind})");
        for _ in 0..8 {
            if matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))) {
                break;
            }
            game.menu_key(Key::Accept)?;
            run(game, 0.5)?;
        }
        anyhow::ensure!(
            matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))),
            "Post Race ({})",
            flow_mode(&game.mode)
        );
        anyhow::ensure!(playing(game, id), "the ending's line plays on over Post Race (as under Wine)");
        Ok(id)
    }
    let prev = game.audio.take();
    game.audio = Some(game.new_audio(Mixer::offline(audio_capture_rate())));
    let capture = session::AudioCapture::start();
    let r = (|| -> Result<()> {
        // The reported case first: a new career's first race (stage 0's
        // first, in Rachel's car), won, then Enter as a player presses it
        // through Post Race (its first option, Return to Explore Mode) to
        // free roam; then Esc and Resume three times. The ending's line
        // ends on leaving Post Race (UI_PostRace's destructor, 0x4E9540 →
        // 0x481210(3)) and no pause or resume brings it back.
        let saved = game.profile.clone();
        let intro = (|| -> Result<()> {
            let mut p = Profile::new("INTRO");
            p.start_career(&game.game_data.career);
            p.intro_played = true;
            game.profile = Some(p);
            game.explore(None)?;
            let e = game
                .game_data.career
                .events
                .iter()
                .find(|e| e.unlock == dday::STAGE0_RACES[0])
                .map(|e| e.index)
                .context("stage 0's first race")?;
            start(game, e)?;
            let id = finish(game)?;
            for _ in 0..10 {
                if matches!(game.mode, Mode::Explore) && game.scenes.shop.is_none() {
                    break;
                }
                game.key(KeyCode::Enter, &mut || {})?;
                run(game, 1.0)?;
            }
            anyhow::ensure!(
                matches!(game.mode, Mode::Explore),
                "intro: free roam after its first race ({})",
                flow_mode(&game.mode)
            );
            for k in 0..3 {
                run(game, 2.0)?;
                anyhow::ensure!(
                    !playing(game, id) && line(game).is_none(),
                    "intro: the ending's line in free roam ({k}: {:?})",
                    line(game)
                );
                game.key(KeyCode::Escape, &mut || {})?;
                run(game, 1.0)?;
                anyhow::ensure!(game.lines_held() && line(game).is_none(), "intro: the pause ({k})");
                game.act(Action::ResumeRace, MenuKind::ExplorePause)?;
                if matches!(game.mode, Mode::Menu(_)) {
                    game.key(KeyCode::Escape, &mut || {})?;
                }
                run(game, 1.0)?;
                anyhow::ensure!(
                    matches!(game.mode, Mode::Explore) && !playing(game, id) && line(game).is_none(),
                    "intro: the ending's line after resume {k} ({:?})",
                    line(game)
                );
            }
            Ok(())
        })();
        game.profile = saved;
        intro?;
        let e = game
            .game_data.career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| e.kind == career::EventKind::Circuit)
            .map(|e| e.index)
            .context("a circuit")?;
        // The pause holds the start line; Restart purges it.
        let id = start(game, e)?;
        game.key(KeyCode::Escape, &mut || {})?;
        anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::RacePause))), "the race's pause");
        run(game, 0.5)?;
        anyhow::ensure!(game.lines_held() && playing(game, id), "the start line held in the pause");
        game.act(Action::Restart, MenuKind::RacePause)?;
        confirm(game)?;
        run(game, 3.0)?;
        anyhow::ensure!(!playing(game, id), "the start line 3 s after the pause's Restart");
        // Quit to Main Menu from the pause.
        let id = line(game).map(|l| l.0).context("the restarted race's start line")?;
        game.key(KeyCode::Escape, &mut || {})?;
        game.act(Action::QuitToMainMenu, MenuKind::RacePause)?;
        confirm(game)?;
        run(game, 3.0)?;
        anyhow::ensure!(!playing(game, id), "the start line 3 s after the pause's Quit to Main Menu");
        // Return to Explore Mode from the pause.
        let id = start(game, e)?;
        game.key(KeyCode::Escape, &mut || {})?;
        game.act(Action::ReturnToExplore, MenuKind::RacePause)?;
        confirm(game)?;
        run(game, 3.0)?;
        anyhow::ensure!(
            matches!(game.mode, Mode::Explore) && !playing(game, id),
            "free roam, the start line gone 3 s after the pause's Return to Explore"
        );
        // Finished: Post Race → free roam (a save question answered).
        start(game, e)?;
        let id = finish(game)?;
        game.act(Action::ReturnToExplore, MenuKind::PostRace)?;
        for _ in 0..8 {
            if matches!(game.mode, Mode::Explore) {
                break;
            }
            if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
                confirm(game)?;
            } else {
                game.menu_key(Key::Accept)?;
            }
            run(game, 0.5)?;
        }
        run(game, 3.0)?;
        anyhow::ensure!(matches!(game.mode, Mode::Explore), "free roam after Post Race ({})", flow_mode(&game.mode));
        anyhow::ensure!(
            !playing(game, id) && line(game).is_none(),
            "no race line 3 s back in free roam ({:?})",
            line(game)
        );
        // Finished: Post Race → Restart, and → Quit to Main Menu.
        start(game, e)?;
        let id = finish(game)?;
        game.act(Action::Restart, MenuKind::PostRace)?;
        confirm(game)?;
        run(game, 3.0)?;
        anyhow::ensure!(!playing(game, id), "the ending's line 3 s after Post Race's Restart");
        let id = finish(game)?;
        game.act(Action::QuitToMainMenu, MenuKind::PostRace)?;
        confirm(game)?;
        run(game, 3.0)?;
        anyhow::ensure!(
            !playing(game, id) && line(game).is_none(),
            "no race line 3 s after Post Race's Quit to Main Menu ({:?})",
            line(game)
        );
        println!(
            "race lines: held in the pause; gone after Restart / Quit / Return to Explore from the pause and Post Race"
        );
        Ok(())
    })();
    drop(capture);
    game.audio = prev;
    r
}

/// The music after a race paused and left for free roam (its pause's
/// Return to Explore Mode): the race is over, so the garage plays the
/// front end's songs. A race kept as paused held the song in every menu
/// after it (`Paused`), for good: the reported silent menus.
pub(crate) fn selftest_music_after_race_pause(game: &mut Game) -> Result<()> {
    fn confirm(game: &mut Game) -> Result<()> {
        if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
            game.menu_key(Key::Left)?;
            game.menu_key(Key::Accept)?;
        }
        Ok(())
    }
    (game.settings.master, game.settings.menu_music, game.settings.game_music) = (1.0, 0.7, 1.0);
    let e = game
        .game_data.career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| e.kind == career::EventKind::Circuit)
        .map(|e| e.index)
        .context("a circuit")?;
    game.start_race(e, false, false)?;
    for _ in 0..60 {
        game.advance_for(1.0 / 60.0)?;
    }
    game.key(KeyCode::Escape, &mut || {})?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::RacePause))) && game.sim.paused_race.is_some(),
        "the race's pause"
    );
    game.act(Action::ReturnToExplore, MenuKind::RacePause)?;
    confirm(game)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore) && game.sim.paused_race.is_none(), "free roam, the race over");
    game.key(KeyCode::Escape, &mut || {})?;
    game.act(Action::JumpToGarage, MenuKind::ExplorePause)?;
    confirm(game)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "the garage");
    anyhow::ensure!(
        game.music_context() == MusicContext::FrontEnd,
        "the garage plays the front end's songs after a race left from its pause ({:?})",
        game.music_context()
    );
    println!("music after a race left from its pause: {:?}", game.music_context());
    Ok(())
}
