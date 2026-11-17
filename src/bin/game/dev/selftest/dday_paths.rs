use crate::*;

/// The career menu's ways into a career saved during the D-Day intro, as
/// a player takes them ("Start New Career" once left the car
/// held at the airport behind the SMS mailbox). Such a save (the intro
/// movie played, 0x86347C = career +0x873C; no car of its own; Rachel's
/// instruction read and other messages unread) offers "Resume Career"
/// beside a fresh "Start New Career" (0x4AF110). Resume goes to the city
/// with the script over (0x495A30 → 0x5404B0 → 0x5404FE) and the car
/// free; "Start New Career" is a new career, whose instruction opens in the
/// viewer (one unread, 0x4976B0) and whose car is free once it is read and
/// closed. Keys go through the window's handler; Enter skips the movie and
/// the start scene.
pub(crate) fn selftest_dday_paths(game: &mut Game) -> Result<()> {
    fn press(game: &mut Game, code: KeyCode) -> Result<()> {
        game.key(code, &mut || {})?;
        game.advance_for(1.0 / 60.0)
    }
    fn drive_off(game: &mut Game, what: &str) -> Result<()> {
        // Two seconds of throttle out of the bay (about 20 m: straight on,
        // the lot's far wall stops the car there, as in the original).
        let from = game.here().context("the car")?.0;
        game.platform.keys.insert(KeyCode::ArrowUp);
        for _ in 0..120 {
            game.advance_for(1.0 / 60.0)?;
        }
        game.platform.keys.clear();
        let moved = (game.here().context("the car")?.0 - from).truncate().length();
        println!("{what}: the car drove {moved:.1} m in 2 s");
        anyhow::ensure!(moved > 10.0, "{what}: the car takes the throttle ({moved:.1} m)");
        Ok(())
    }
    let labels = |game: &Game| match &game.mode {
        Mode::Menu(Menu::Icons(m, MenuKind::Career)) => m.options.iter().map(|o| o.action).collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    // A save like the reported one: D-Day A under way, the movie played, no car;
    // its volumes (master 1, menu music 0.7, game music 1).
    (game.settings.master, game.settings.menu_music, game.settings.game_music) = (1.0, 0.7, 1.0);
    // Its inbox as reported: ENGAGE_TIP_CAREER_PROG (urgent, shown),
    // SMS_INSTRUCTION and SMS_MINIMAP_INSTRUCTION read and filed, then
    // SMS_MINIMAP_LEGEND_BLURB, SMS_STEERING_CONTROLS and three engage tips
    // unread.
    let mut p = Profile::new("DDAY");
    p.start_career(&game.game_data.career);
    p.intro_played = true;
    for (k, m) in [
        0x5F04_C43A,
        dday::SMS_INSTRUCTION,
        0x1157_5F3D,
        0x8D78_AEC0,
        0x35B2_CBA5,
        0x80E5_BFCE,
        0xFC25_E222,
        0x4251_40E8,
    ]
    .into_iter()
    .enumerate()
    {
        p.inbox.add(m);
        if k < 3 {
            p.inbox.read(k);
        }
    }
    p.inbox.mark_shown(&game.game_data.career.messages);
    p.inbox.file();
    p.save()?;
    game.profile = Some(p);
    game.career_world.dday = Default::default();
    game.drop_drive();
    // UI_Main opens on Quick Race: Left to Career, Enter.
    game.mode = game.icon_menu(MenuKind::Main);
    press(game, KeyCode::ArrowLeft)?;
    press(game, KeyCode::Enter)?;
    let options = labels(game);
    println!("career menu of a D-Day save: {options:?}");
    anyhow::ensure!(
        options == [Action::ResumeCareer, Action::NewCareer, Action::LoadCareer],
        "Resume Career, Start New Career, Load Game (0x4AF110 with 0x86347C set)"
    );
    // Resume: the city at once, the script over, no SMS screens.
    press(game, KeyCode::Enter)?;
    for _ in 0..120 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(
        matches!(game.mode, Mode::Explore) && game.scenes.shop.is_none(),
        "Resume goes to the city, no movie or start scene (mode {}, scene {})",
        flow_mode(&game.mode),
        game.scenes.shop.is_some()
    );
    anyhow::ensure!(game.career_world.dday.state == 9 && !game.career_world.dday.holds_input(), "the D-Day script over (0x5404FE), the car free");
    drive_off(game, "resumed D-Day")?;
    // Quit (Esc, Quit to Main Menu, OK), then Start New Career: a new
    // career, its own intro. The first time Esc comes in its start scene
    // and the player quits from there: the scene goes with the track, so
    // the main menu plays its songs (it stopped them while it lingered,
    // the reported "silent menus").
    fn quit(game: &mut Game) -> Result<()> {
        press(game, KeyCode::Escape)?;
        anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))), "Esc pauses");
        game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "quit to the main menu");
        Ok(())
    }
    fn new_career(game: &mut Game, focus_new: bool) -> Result<()> {
        press(game, KeyCode::ArrowLeft)?;
        press(game, KeyCode::Enter)?;
        if focus_new {
            press(game, KeyCode::ArrowRight)?;
        }
        press(game, KeyCode::Enter)?;
        anyhow::ensure!(
            matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Ask),
            "Start New Career asks for a new profile"
        );
        // Yes; the name as offered (the editor takes its first Enter as
        // nothing, 0x5512A9); save: Yes (asked unless Autosave is on).
        for _ in 0..6 {
            match &game.mode {
                Mode::Menu(Menu::McMain(m)) if matches!(m.step, McStep::Ask | McStep::Confirm) => {
                    press(game, KeyCode::ArrowLeft)?;
                    press(game, KeyCode::Enter)?;
                }
                Mode::Menu(Menu::McMain(_)) => press(game, KeyCode::Enter)?,
                _ => break,
            }
        }
        anyhow::ensure!(
            matches!(game.mode, Mode::Menu(Menu::Movie(_))),
            "the intro movie (mode {})",
            flow_mode(&game.mode)
        );
        anyhow::ensure!(
            game.profile.as_ref().is_some_and(|p| !p.intro_played && p.inbox.entries.is_empty()),
            "a new career: no intro played, an empty inbox"
        );
        for _ in 0..40 {
            game.advance_for(1.0 / 60.0)?;
        }
        press(game, KeyCode::Enter)?;
        for _ in 0..10 {
            game.advance_for(1.0 / 60.0)?;
        }
        anyhow::ensure!(game.scenes.shop.is_some(), "the career's start scene");
        Ok(())
    }
    quit(game)?;
    new_career(game, true)?;
    quit(game)?;
    anyhow::ensure!(
        game.scenes.shop.is_none() && game.music_context() == MusicContext::FrontEnd,
        "the main menu after quitting in the start scene: no scene left, the front end's music ({:?})",
        game.music_context()
    );
    for _ in 0..30 {
        game.advance_for(1.0 / 60.0)?;
    }
    if let Some(a) = game.audio.as_ref() {
        anyhow::ensure!(a.streams.song().is_some(), "a menu song plays after quitting in the start scene");
    }
    // Its movie played, so the career menu offers Resume beside a fresh
    // career again; Start New Career, and Enter skips the start scene.
    new_career(game, true)?;
    press(game, KeyCode::Enter)?;
    let mut frames = 0;
    while matches!(game.mode, Mode::Explore) && frames < 1200 {
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
    }
    let instruction =
        game.profile.as_ref().and_then(|p| p.inbox.entries.iter().position(|e| e.message == dday::SMS_INSTRUCTION));
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::SmsViewer(v)) if Some(v.entry) == instruction),
        "the instruction SMS opens in the viewer, not the mailbox"
    );
    press(game, KeyCode::Enter)?;
    let mut frames = 0;
    while game.career_world.dday.running() && frames < 1800 {
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
    }
    anyhow::ensure!(
        game.career_world.dday.state == 9 && matches!(game.mode, Mode::Explore),
        "the script over once the instruction is read and closed"
    );
    drive_off(game, "new career")?;
    println!("D-Day saves: Resume and Start New Career ok");
    Ok(())
}
