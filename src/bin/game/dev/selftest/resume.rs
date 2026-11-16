use crate::*;

/// Where Resume Career puts the player for each point a player may leave
/// the game at (a player report: the first car bought, the game left,
/// Resume put the car back at the airport). 0x495A30: with DDAY_EVENT_B
/// won the garage (UI_CareerCrib), whose Return to Explore Mode sets
/// career +0x8744 (0x4AF616); else, the intro movie played (0x86347C),
/// +0x8744 = 1 (0x495AD4) and the city start (0x5404B0 → 0x5406BE) puts
/// the car at the kept place +0x8750 / +0x8760; else the movie and the
/// airport start. The place is kept by entering a shop (0x54CB00,
/// 0x54CC0C: the car lot, the garage), a race engaged (0x541170), a
/// magazine / DVD shoot (0x500AB0), Jump to Garage (0x513B30) and Quit to
/// Main Menu (0x5139C0, in memory). Checked under Wine (2026-10-31): the
/// original's car-lot save resumes in the lot at (−722.38, −878.83); its
/// first-win save opens the garage and Return to Explore Mode stands the
/// car at the race's engage place (462.42, −46.16).
pub(crate) fn selftest_resume_points(game: &mut Game) -> Result<()> {
    use nfsu2::formats::native_save::{self as ns, NativeSave};
    fn press(game: &mut Game, code: KeyCode) -> Result<()> {
        game.key(code, &mut || {})?;
        game.advance_for(1.0 / 60.0)
    }
    // The game left and started again: only the file is kept.
    fn restart(game: &mut Game, name: &str) -> Result<()> {
        game.forget_session();
        profiles::load(name)?;
        game.mode = game.icon_menu(MenuKind::Main);
        Ok(())
    }
    fn resume(game: &mut Game) -> Result<()> {
        game.act(Action::Career, MenuKind::Main)?;
        game.act(Action::ResumeCareer, MenuKind::Career)?;
        for _ in 0..10 {
            game.advance_for(1.0 / 60.0)?;
        }
        Ok(())
    }
    // Messages opened over the city (urgent SMS) closed.
    fn settle(game: &mut Game) -> Result<()> {
        for _ in 0..8 {
            if matches!(game.mode, Mode::Explore) {
                return Ok(());
            }
            game.menu_key(Key::Back)?;
            game.advance_for(1.0 / 60.0)?;
        }
        anyhow::ensure!(matches!(game.mode, Mode::Explore), "free roam (mode {})", flow_mode(&game.mode));
        Ok(())
    }
    // The garage's Return to Explore Mode: No focused, Left to Yes.
    fn leave_garage(game: &mut Game, what: &str) -> Result<()> {
        anyhow::ensure!(
            matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))),
            "{what}: Resume opens the garage (0x495A50, mode {})",
            flow_mode(&game.mode)
        );
        game.act(Action::Explore, MenuKind::Crib)?;
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
        settle(game)
    }
    fn near(game: &Game, want: Vec2, what: &str) -> Result<()> {
        let at = game.here().with_context(|| format!("{what}: the car in free roam"))?.0;
        let d = at.truncate().distance(want);
        println!("resume {what}: car at ({:.2}, {:.2}), {d:.2} m from ({:.2}, {:.2})", at.x, at.y, want.x, want.y);
        anyhow::ensure!(
            d < 1.0,
            "{what}: the car at ({:.1}, {:.1}), not at ({:.1}, {:.1})",
            at.x,
            at.y,
            want.x,
            want.y
        );
        Ok(())
    }
    fn place_of(name: &str) -> Result<(u8, Vec2)> {
        let s = profiles::read(name)?;
        let f = |o: usize| f32::from_le_bytes(s.payload[o..o + 4].try_into().unwrap());
        Ok((s.u8(ns::PLACE_SET), Vec2::new(f(ns::PLACE_POS), f(ns::PLACE_POS + 4))))
    }
    // The original's own saves: the D-Day start (a Profile Manager save in
    // D-Day A), the car lot's prompt, the garage, the first win.
    let originals: [(&str, &str, bool, Vec2); 4] = [
        ("career_start", "tests/fixtures/native_saves/career_start.sav", false, Vec2::new(1695.25, -883.64)),
        ("car_lot", "tests/fixtures/native_saves/car_lot.sav", false, Vec2::new(-726.51, -875.20)),
        ("garage", "tests/fixtures/native_saves/garage.sav", true, Vec2::new(654.11, -97.64)),
        ("first win", "tests/fixtures/native_saves/stage1_win_streetx.sav", true, Vec2::new(462.42, -46.16)),
    ];
    let settings = game.settings.clone();
    for (what, file, garage, want) in originals {
        // The original's saves are the maintainer's (the private set).
        let Some(bytes) = crate::dev::private::read(file)? else {
            println!("{}", crate::dev::private::skip_line(&format!("resume, the original's {what} save")));
            continue;
        };
        profiles::write("NAME", &NativeSave::parse(&bytes)?)?;
        restart(game, "NAME")?;
        resume(game)?;
        if garage {
            leave_garage(game, what)?;
        } else {
            settle(game)?;
        }
        near(game, want, &format!("original {what}"))?;
        anyhow::ensure!(game.career_world.dday.state == 9 && game.scenes.shop.is_none(), "{what}: the D-Day script over, no scene");
    }
    let _ = profiles::delete("NAME");
    game.settings = settings;
    game.settings.autosave = false;
    game.settings.save_prompts = true;
    // The clone's own career, played. 1: a new career saved (MC_Main at
    // Start New Career, before the movie: 0x86347C clear), the game left:
    // Resume plays the movie again and starts at the airport.
    let mut p = Profile::new("RESUME");
    p.start_career(&game.game_data.career);
    p.save()?;
    restart(game, "RESUME")?;
    resume(game)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Movie(m)) if m.mode == 6),
        "before the car lot, game left: the intro movie again (mode {})",
        flow_mode(&game.mode)
    );
    for _ in 0..20 {
        game.advance_for(1.0 / 60.0)?;
    }
    press(game, KeyCode::Enter)?;
    for _ in 0..10 {
        game.advance_for(1.0 / 60.0)?;
    }
    let airport = career::free_roam_start(&game.game_data.root).context("ForwardStartingPosition")?.0.truncate();
    near(game, airport, "new career, game left before the lot")?;
    // Its start scene skipped, Rachel's instruction read: the car free.
    press(game, KeyCode::Enter)?;
    let mut frames = 0;
    while matches!(game.mode, Mode::Explore) && frames < 1200 {
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
    }
    press(game, KeyCode::Enter)?;
    let mut frames = 0;
    while game.career_world.dday.running() && frames < 1800 {
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
    }
    settle(game)?;
    // 2: Quit to Main Menu during D-Day A (0x5139C0: the place kept in
    // memory, no file written), Resume in the same session: back there.
    let shops = |g: &Game, name: &str| -> Result<(usize, Vec3)> {
        let shop = g.game_data.career.shops.iter().position(|s| s.name == name).with_context(|| format!("shop {name}"))?;
        let t = g.sim.route_triggers.find(g.game_data.career.shops[shop].trigger).with_context(|| format!("{name}'s trigger"))?;
        Ok((shop, ((t.min + t.max) * 0.5).extend(t.position.z)))
    };
    let put = |g: &mut Game, at: Vec3| {
        g.scenes.shop = None;
        if let Some(d) = g.sim.drive.as_mut() {
            d.state.position = at;
        }
        g.career_world.marker_trigger = None;
    };
    let quit_at = game.here().context("the car")?.0.truncate();
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "quit to the main menu");
    resume(game)?;
    settle(game)?;
    near(game, quit_at, "D-Day A, quit to the main menu")?;
    // 3: the car lot, a stock car taken and the lot's save accepted.
    let (lot, lot_at) = shops(game, "CC_CAR_LOT_1")?;
    put(game, lot_at);
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(game.engage(), "the car lot's Engage");
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::ShopEngage(ref e)) if e.shop == lot),
        "the car lot's engage dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    skip_shop_movie(game, "scene05")?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::CarLot(_))), "the car lot");
    game.menu_key(Key::Right)?;
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Accept)?;
    let mut asked = false;
    for _ in 0..12 {
        match &game.mode {
            Mode::Explore => break,
            // Yes on each question (the save, the overwrite), then OK.
            Mode::Menu(Menu::McMessage(m, _)) if m.buttons.len() == 2 => {
                asked |= m.text.starts_with("Are you sure you want to save");
                game.menu_key(Key::Left)?;
                game.menu_key(Key::Accept)?;
            }
            Mode::Menu(Menu::McMessage(..) | Menu::McMain(_)) => game.menu_key(Key::Accept)?,
            _ => game.advance_for(1.0 / 60.0)?,
        }
    }
    anyhow::ensure!(
        asked && matches!(game.mode, Mode::Explore),
        "the lot's save question answered Yes, then the city (mode {})",
        flow_mode(&game.mode)
    );
    let (set, at) = place_of("RESUME")?;
    println!("car lot save: place flag {set}, ({:.2}, {:.2})", at.x, at.y);
    anyhow::ensure!(
        set == 1 && at.distance(lot_at.truncate()) < 1.0,
        "the lot's save keeps the lot's place, flag set (0x54CC0C), as the original's car_lot.sav"
    );
    // Quit to Main Menu without a save, Resume: the place of the quit; the
    // game left instead: the lot.
    play_shop_scene(game)?;
    settle(game)?;
    let drove = game.here().context("the car")?.0.truncate();
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    resume(game)?;
    settle(game)?;
    near(game, drove, "car bought, quit to the main menu")?;
    restart(game, "RESUME")?;
    resume(game)?;
    settle(game)?;
    near(game, lot_at.truncate(), "car bought and saved, game left")?;
    anyhow::ensure!(
        game.career_world.dday.state == 9 && game.profile.as_ref().is_some_and(|p| p.intro == Some(career::DDAY_EVENT_B)),
        "D-Day B under way, the script over"
    );
    anyhow::ensure!(game.career_world.gps.as_ref().is_some_and(|g| g.target == career::DDAY_EVENT_B), "the GPS to the garage");
    // 4: the garage (D-Day B won), left, saved, the game left: Resume
    // opens the garage and its Return to Explore Mode stands the car at the
    // garage's trigger (the original's garage.sav keeps that place).
    let (crib, crib_at) = shops(game, "CRIB_1")?;
    put(game, crib_at);
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(game.engage(), "the garage's Engage");
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::ShopEngage(ref e)) if e.shop == crib),
        "the garage's engage dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    skip_shop_movie(game, "scene06")?;
    leave_garage(game, "the D-Day garage")?;
    play_shop_scene(game)?;
    settle(game)?;
    game.profile.as_ref().context("profile")?.save()?;
    restart(game, "RESUME")?;
    resume(game)?;
    leave_garage(game, "after the garage")?;
    near(game, crib_at.truncate(), "after the garage, saved")?;
    // 5: a race engaged (0x541170: flag clear, the engage place kept) and
    // its win saved (UI_PostRace's save point; the race is not run here):
    // the garage, then the engage place (the original's first-win save).
    let (e, start, _) = game.nearest_event().context("an open event")?;
    if let Some(d) = game.sim.drive.as_mut() {
        d.place(start, d.state.heading());
    }
    game.start_race(e, false, false)?;
    game.profile.as_ref().context("profile")?.save()?;
    let (set, at) = place_of("RESUME")?;
    anyhow::ensure!(
        set == 0 && at.distance(start.truncate()) < 1.0,
        "a race's save: flag clear, the engage place (as the original's first-win save)"
    );
    restart(game, "RESUME")?;
    resume(game)?;
    leave_garage(game, "after the first race")?;
    near(game, start.truncate(), "after the first race, saved")?;
    let _ = profiles::delete("RESUME");
    println!("resume points: as the original");
    Ok(())
}
