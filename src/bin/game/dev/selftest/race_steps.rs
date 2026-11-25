//! The selftest's race steps, from the World Map race to the particles
//! in the order `selftest` runs them.
use super::*;

/// [`world_map_race`]'s race for the checks after it.
pub(super) struct WorldMapRace {
    /// Where Jump to Garage left the car.
    pub(super) jumped: (Vec3, f32),
    pub(super) event: usize,
    /// The profile's cash and reputation before the race.
    pub(super) cash: u32,
    pub(super) rep: u32,
}

/// Jump to Garage, then a World Map race: its start scene, a pause, its traffic, raced out to Results.
pub(super) fn world_map_race(game: &mut Game) -> Result<WorldMapRace> {
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    let jumped = game.here().context("in free roam before the garage")?;
    game.act(Action::JumpToGarage, MenuKind::ExplorePause)?;
    // UI_InGameDialog "Jump to Garage?" (0x4C2730), Cancel focused: OK.
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.package == "UI_InGameDialog" && d.selected == 1 && d.text.contains("Garage")),
        "Jump to Garage? on UI_InGameDialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    println!("garage: {:?}", menu(game));
    game.act(Action::WorldMap, MenuKind::Crib)?;
    let Mode::Menu(Menu::WorldMap(map)) = &game.mode else { anyhow::bail!("world map") };
    // The first open circuit (in the open events' order, 0x527D40).
    let open = game.profile.as_ref().context("profile")?.open_events(&game.game_data.career);
    let first = open.iter().find(|e| e.kind == career::EventKind::Circuit).map(|e| e.index);
    let pick = map.events.iter().position(|&i| Some(i) == first).unwrap_or(0);
    // Down from the region entry the list opens on to the first event.
    for _ in 0..=pick {
        game.menu_key(Key::Down)?;
    }
    // Accept asks "Would you like to race this event?" (0xEC9488BB).
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.contains("race this event")),
        "the race question"
    );
    answer_yes(game)?;
    let Mode::Race(race) = &game.mode else { anyhow::bail!("race started") };
    let event = race.event;
    println!("racing {} from the World Map", game.game_data.career.track_title(game.game_data.career.events[event].track));
    // The race's start scene (Crowds on, a scene for the track's start
    // kind): its camera with no HUD, then the chase view; no countdown.
    let scene = game.scenes.shop.as_ref().map(|r| (r.scene.kind, r.scene.marks));
    println!("race start scene: {scene:?}, crowds {}", game.settings.custom[5]);
    if let Some((_, [green, _])) = scene {
        anyhow::ensure!(
            matches!(&game.mode, Mode::Race(r) if r.scene_start && r.timing.clock < 0.0),
            "the scene's race starts with no countdown"
        );
        let bars = game.overlay_quads(game.now());
        anyhow::ensure!(
            bars.iter().all(|q| q.texture.is_none() && q.color == [0.0, 0.0, 0.0, 1.0]),
            "no HUD under the scene's camera"
        );
        println!("race start scene letterbox: {} bars", bars.len());
        for _ in 0..(green as f32 / 30.0 * 60.0) as u32 + 2 {
            game.advance_for(1.0 / 60.0)?;
        }
        anyhow::ensure!(
            game.scenes.shop.is_none() && matches!(&game.mode, Mode::Race(r) if r.timing.clock >= 0.0),
            "green at the scene's first mark"
        );
    }
    for _ in 0..(COUNTDOWN * 60.0) as u32 + 2 {
        game.advance_for(1.0 / 60.0)?;
    }
    // Pause and resume.
    let Mode::Race(race) = std::mem::replace(&mut game.mode, Mode::Explore) else { unreachable!() };
    game.sim.paused_race = Some(race);
    game.mode = game.icon_menu(MenuKind::RacePause);
    println!("race pause: {:?}", menu(game));
    game.act(Action::ResumeRace, MenuKind::RacePause)?;
    anyhow::ensure!(matches!(game.mode, Mode::Race(_)), "resumed");
    // The race's track load dropped free roam's pool (its models are given
    // again on the next free roam load, 0x57F2E0 state 6).
    anyhow::ensure!(game.sim.traffic.is_none(), "free roam traffic dropped for the race");
    // The race's traffic at its event's level (byte +0x36, 0x5322F0):
    // the placer puts a car a second on the race's route file.
    for _ in 0..300 {
        game.advance_for(1.0 / 30.0)?;
    }
    let Mode::Race(race) = &game.mode else { anyhow::bail!("still racing") };
    let level = game.game_data.career.events[event].traffic;
    let placed = race.racers.traffic.as_ref().map_or(0, |t| t.cars.iter().filter(|c| c.lane.is_some()).count());
    println!("race traffic (level {level}) after 10 s: {placed} cars");
    anyhow::ensure!(level <= 0 || placed > 0, "race traffic placed");
    let (cash, rep) = (game.profile.as_ref().unwrap().cash, game.profile.as_ref().unwrap().reputation);
    // The player is moved along the road; the opponents race it out
    // (the race ends when every racer has finished).
    let mut reported = false;
    for _ in 0..60000 {
        let Mode::Race(race) = &game.mode else { break };
        if race.racers.me.finished.is_none() {
            let target = race.route.road.point_at(race.racers.me.along + 100.0);
            let d = game.sim.drive.as_mut().unwrap();
            d.state.position = target.extend(d.state.position.z);
            // Half a second in frames the game makes (one world step per
            // frame, 0x5811D0; a single 0.5 s frame leaves the teleported
            // car in the air at its end, so no top speed is recorded).
            for _ in 0..15 {
                game.advance_for(1.0 / 30.0)?;
            }
        } else {
            if !reported {
                reported = true;
                let rivals: Vec<_> = race
                    .racers.rivals
                    .iter()
                    .map(|r| {
                        (
                            r.name.clone(),
                            r.car.car_type.name.clone(),
                            (r.progress.progress() * 100.0) as i32,
                            (r.state.speed() * 2.237) as i32,
                        )
                    })
                    .collect();
                println!("player done at {:.1} s; rivals (name, car, m, mph): {rivals:?}", race.timing.clock);
                game.advance_for(1.0 / 30.0)?;
                // Past the line an AI racer controller drives the player's
                // car (0x606CC0 → 0x5EBA40(3)).
                if let Mode::Race(race) = &game.mode {
                    anyhow::ensure!(
                        race.autopilot.is_some(),
                        "the player's car handed to an AI racer controller at the finish"
                    );
                }
                continue;
            }
            game.advance_for(1.0 / 30.0)?;
            if std::env::var_os("AI_TRACE").is_some()
                && let Mode::Race(race) = &game.mode
                && (race.timing.clock * 30.0) as i64 % 150 == 0
            {
                let v: Vec<_> = race
                    .racers.rivals
                    .iter()
                    .map(|r| {
                        format!(
                            "{} laps={:.2} v={:.0} tgt={:.0} pos=({:.0},{:.0}) path={} crash={}",
                            r.name,
                            r.progress.progress(),
                            r.state.speed(),
                            r.route.as_ref().map_or(-1.0, |x| x.speed_at(x.distance())),
                            r.state.position.x,
                            r.state.position.y,
                            r.route.as_ref().map_or(0, |x| x.path(race.racers.ai_routes.as_ref().unwrap()).len()),
                            r.state.crash
                        )
                    })
                    .collect();
                println!("t={:.0} {v:?}", race.timing.clock);
            }
        }
    }
    if let Mode::Race(race) = &game.mode {
        let rivals: Vec<_> = race
            .racers.rivals
            .iter()
            .map(|r| (r.name.clone(), r.progress.progress(), r.progress.lap(), r.progress.finished))
            .collect();
        anyhow::bail!("rivals did not finish by {:.0} s: {rivals:?}", race.timing.clock);
    }
    let Mode::Menu(Menu::Results(results, _)) = &game.mode else { anyhow::bail!("results screen") };
    println!("results: {:?}", results.rows);
    game.menu_key(Key::Accept)?;
    Ok(WorldMapRace { jumped, event, cash, rep })
}

/// The World Map race's results: no reward, unpaid, filed; its save point.
pub(super) fn race_results(game: &mut Game, race: &WorldMapRace) -> Result<()> {
    let &WorldMapRace { event, cash, rep, .. } = race;
    // A race started from the World Map's list (career +0xD27, 0x5325F0)
    // shows no reward (0x4F3EA0: 0x500DB0) and pays nothing (0x541430);
    // the win is filed (0x54CA70) and handled on leaving UI_PostRace.
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))),
        "no reward screen after a World Map race"
    );
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(
        p.cash == cash && p.reputation == rep && p.filed.contains(&event) && !p.completed.contains(&event),
        "filed, unpaid"
    );
    anyhow::ensure!(game.race_flow.result.as_ref().is_some_and(|r| r.won && !r.pays), "the outcome waits for the exit");
    println!("post race: {:?}", menu(game));
    game.act(Action::RaceStats, MenuKind::PostRace)?;
    let Mode::Menu(Menu::Stats(stats)) = &game.mode else { anyhow::bail!("race stats") };
    let r = &stats.racers[0];
    println!("race stats: top {:.1} mph, average {:.1} mph, laps {:?}", r.top_speed * 2.237, r.average * 2.237, r.laps);
    // The selftest teleports the car, so the distance driven (Σ|Δpos|) runs high.
    anyhow::ensure!(r.time > 0.0 && r.average > 0.0 && r.top_speed > 0.0, "race stats filled");
    game.menu_key(Key::Back)?;
    // The race's record (0x502DF0): its best lap kept in the profile.
    anyhow::ensure!(
        game.profile.as_ref().is_some_and(|p| p.records.iter().any(|x| x.track != 0 && x.value > 0)),
        "the race's record kept"
    );
    game.act(Action::ReturnToExplore, MenuKind::PostRace)?;
    // A won career race: settled on leaving UI_PostRace (0x54C950; this
    // one, from the World Map's list, unpaid), then its save point with
    // Prompt (0x4F39A0, mode 0x441): the question (No focused); Yes, the
    // overwrite question when the file exists, "Save complete.", the city.
    anyhow::ensure!(
        game.race_flow.result.is_none() && game.profile.as_ref().is_some_and(|p| p.cash == cash && p.reputation == rep),
        "settled at the exit, unpaid"
    );
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMessage(m, _)) if m.text.starts_with("Are you sure you want to save")),
        "the post-race save question"
    );
    while let Mode::Menu(Menu::McMessage(m, _)) = &game.mode {
        println!("post-race save: {:?}", m.text);
        if m.buttons.len() == 2 {
            game.menu_key(Key::Left)?;
        }
        game.menu_key(Key::Accept)?;
    }
    anyhow::ensure!(Profile::load().context("saved")?.completed.contains(&event), "saved");
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "back to explore");
    Ok(())
}

/// Back in free roam where Jump to Garage left the car, with a new pool.
pub(super) fn back_at_engage_place(game: &mut Game, race: &WorldMapRace) -> Result<()> {
    let &WorldMapRace { jumped, .. } = race;
    // The race was engaged from the garage's World Map with the car where
    // Jump to Garage left it (0x541170 keeps that place, 0x513950 sets
    // career +0x8744): back there, not at the free-roam start.
    {
        let at = game.here().context("in free roam after the race")?.0;
        let start = career::free_roam_start(&game.game_data.root).context("free-roam start")?.0;
        println!(
            "after the race at ({:.0}, {:.0}); engaged at ({:.0}, {:.0}); free-roam start ({:.0}, {:.0})",
            at.x, at.y, jumped.0.x, jumped.0.y, start.x, start.y
        );
        anyhow::ensure!(at.truncate().distance(jumped.0.truncate()) < 1.0, "back where the race was engaged");
        anyhow::ensure!(at.truncate().distance(start.truncate()) > 50.0, "not at the free-roam start");
    }
    // Back in free roam: a new pool whose models all have a cap above 0 in
    // the traffic zone holding the player (0x430240 → 0x5D9C00 → 0x412BE0).
    {
        let t = game.sim.traffic.as_ref().context("free roam traffic after the race")?;
        let at = game.sim.drive.as_ref().map(|d| d.state.position.truncate()).unwrap_or_default();
        let zone = nfsu2::ai::traffic::TrafficZone::at(&t.zones, at).map_or(0, |z| z.caps);
        anyhow::ensure!(
            t.cars.iter().all(|c| c.car.car_type.traffic_caps[zone] > 0),
            "traffic models capped for zone {zone}"
        );
        println!("free roam traffic after the race: zone {zone}, {} cars", t.cars.len());
    }
    Ok(())
}

/// The engage dialog at a marker, Quit to the main menu, Resume at the saved place.
pub(super) fn quit_and_resume(game: &mut Game) -> Result<()> {
    // The engage dialog at an event's marker in Bayview.
    if let Some((e, start, _)) = game.nearest_event() {
        let d = game.sim.drive.as_mut().unwrap();
        d.place(start, d.state.heading());
        let message = 1;
        game.mode = Mode::Menu(Menu::Engage(Engage {
            event: e,
            message,
            opened: game.now(),
            focus: Default::default(),
            purse: game.game_data.career.events[e].reward,
        }));
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(matches!(game.mode, Mode::Race(_)), "engage accepted starts the race");
        let p = game.profile.as_ref().context("profile")?;
        anyhow::ensure!(
            !p.explore_set && p.explore_pos.is_some_and(|(at, _)| at.truncate().distance(start.truncate()) < 1.0),
            "the race start keeps the engage place, flag clear (0x541170)"
        );
        game.mode = Mode::Explore;
    }
    let engaged = game.profile.as_ref().and_then(|p| p.explore_pos);
    // Quit to the main menu: Yes/No.
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.package == "UI_InGameDialog"), "asked");
    // Cancel (focused) keeps the pause; OK quits.
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))),
        "Cancel back to the pause"
    );
    game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "main menu");
    game.act(Action::Career, MenuKind::Main)?;
    println!("career menu with a save: {:?}", menu(game));
    game.act(Action::ResumeCareer, MenuKind::Career)?;
    // 0x495A30: past the intro, Resume opens the garage.
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "resume opens the garage");
    // Out of the garage (0x4AF607: career +0x8744 = 1, 0x5404B0): at the
    // saved place, here the race's engage point.
    if let Some((engaged, _)) = engaged {
        // Back there asks to quit to the main menu (0x4AF59F, 0x836324
        // set by the career start); No (focused) stays.
        game.menu_key(Key::Back)?;
        anyhow::ensure!(
            matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.contains("quit to the main menu")),
            "Back in the resumed garage asks to quit"
        );
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "No: the garage");
        game.act(Action::Explore, MenuKind::Crib)?;
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
        let at = game.here().context("free roam out of the garage")?.0;
        println!("out of the garage after Resume at ({:.0}, {:.0})", at.x, at.y);
        anyhow::ensure!(
            at.truncate().distance(engaged.truncate()) < 1.0,
            "the loaded career starts at its saved place"
        );
    }
    Ok(())
}

/// Quick Race: a drag race in its own region.
pub(super) fn quick_race_drag(game: &mut Game) -> Result<()> {
    // Quick Race: a drag race in its own region.
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::QuickRace, MenuKind::Main)?;
    println!("quick race modes: {:?}", menu(game));
    game.act(Action::QrMode(screens::QrMode::Drag), MenuKind::QrMode)?;
    // The track list (0x4CDED0): open tracks first; Accept on the first.
    if let Mode::Menu(Menu::QuickRace(qr)) = &game.mode {
        let names: Vec<String> =
            qr.tracks.iter().map(|t| format!("{}{}", t.number, if t.locked { "*" } else { "" })).collect();
        println!("quick race drag tracks: {}", names.join(" "));
    }
    game.menu_key(Key::Accept)?;
    // UI_QRModeOptions, then Continue.
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::QrOptions(_))),
        "quick race game options ({})",
        mode_name(&game.mode)
    );
    game.menu_key(Key::Accept)?;
    // The track, then UI_QRCarSelect's first stock car.
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if matches!(l.state, LotState::QuickRace(Some(_)))),
        "quick race car select"
    );
    game.menu_key(Key::Accept)?;
    if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
        // The transmission question (0x497D50): keep the current setting.
        game.menu_key(Key::Accept)?;
    }
    let region = game.sim.drive.as_ref().map(|d| d.region.clone());
    println!("quick race in {:?}", region);
    // The drag's start scene first (0x436840 case 0, as every race type).
    for _ in 0..60 * 60 {
        if game.scenes.shop.is_none() || !matches!(game.mode, Mode::Race(_)) {
            break;
        }
        game.advance_for(1.0 / 60.0)?;
    }
    for _ in 0..(COUNTDOWN * 60.0) as u32 + 2 {
        game.advance_for(1.0 / 60.0)?;
    }
    // Drag lanes: the player's DragCarController and each rival on a
    // traffic lane (0x42DF10, 0x420ED0).
    game.advance_for(0.1)?;
    if let Mode::Race(race) = &game.mode {
        anyhow::ensure!(
            race.dragger.as_ref().is_some_and(|s| s.lane.lane.is_some()),
            "drag: the player steers on a lane"
        );
        anyhow::ensure!(race.racers.rivals.iter().all(|r| r.lane.lane.is_some()), "drag: the rivals drive traffic lanes");
    }
    // The rivals drive their own race (an AI that crashes takes its
    // GetUnstuck time), so allow up to 300 s.
    for _ in 0..600 {
        let Mode::Race(race) = &game.mode else { break };
        let target = race.route.road.point_at(race.racers.me.along + 100.0);
        let d = game.sim.drive.as_mut().unwrap();
        d.state.position = target.extend(d.state.position.z);
        game.advance_for(0.5)?;
    }
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Results(_, None))),
        "quick race finished: results, no reward ({})",
        mode_name(&game.mode)
    );
    game.menu_key(Key::Accept)?;
    println!("quick post race: {:?}", menu(game));
    Ok(())
}

/// Quick Race: a circuit from the Game Options (Reverse, Lap KO).
pub(super) fn quick_race_circuit(game: &mut Game) -> Result<()> {
    // A circuit from the Game Options: Track Direction Reverse, Lap KO on
    // (3 laps, 3 opponents), raced as set (0x53F160).
    game.mode = game.icon_menu(MenuKind::QrMode);
    game.act(Action::QrMode(screens::QrMode::Circuit), MenuKind::QrMode)?;
    // Resort Loop runs one way; Outer Ring (4001) both.
    while let Mode::Menu(Menu::QuickRace(qr)) = &game.mode {
        if qr.track().is_some_and(|t| game.game_data.career.tracks.get(&t.number).is_some_and(|t| t.reversible))
            || qr.selected + 1 >= qr.tracks.len()
        {
            break;
        }
        game.menu_key(Key::Right)?;
    }
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::QrOptions(_))), "circuit game options");
    game.menu_key(Key::Right)?;
    game.menu_key(Key::Down)?;
    game.menu_key(Key::Right)?;
    let r = game.settings.qr[0];
    println!(
        "circuit record: track {} reversed {} lap KO {} laps {} opponents {}",
        r.track, r.reversed, r.lap_ko, r.laps, r.opponents
    );
    anyhow::ensure!(r.reversed && r.lap_ko && r.laps == 3 && r.opponents == 3, "the options set the record");
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Accept)?;
    if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
        game.menu_key(Key::Accept)?;
    }
    if let Mode::Race(race) = &game.mode {
        println!(
            "circuit quick race: {} rivals {:?}, reversed {}, knockout {}, catch-up {:?}",
            race.racers.rivals.len(),
            race.racers.rivals.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            race.reversed,
            race.scoring.knockout,
            race.catch_up
        );
        anyhow::ensure!(
            race.reversed && race.scoring.knockout && race.racers.rivals.len() == 3 && race.catch_up.is_some(),
            "the race takes the record"
        );
    } else {
        anyhow::bail!("circuit quick race did not start ({})", mode_name(&game.mode));
    }
    Ok(())
}

/// Quick Race's Free Run, quit to the main menu; the saved profile then (checked at the end).
pub(super) fn free_run(game: &mut Game) -> Result<Profile> {
    // Quick Race's Free Run (0x4B2CF0, track 4000): free roam in the city
    // without the career; quitting to the main menu ends it.
    game.mode = game.icon_menu(MenuKind::QrMode);
    game.act(Action::QrMode(screens::QrMode::FreeRun), MenuKind::QrMode)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if l.state == LotState::QuickRace(None)),
        "free run car select"
    );
    game.menu_key(Key::Accept)?;
    if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
        // The transmission question (0x497D50): keep the current setting.
        game.menu_key(Key::Accept)?;
    }
    anyhow::ensure!(matches!(game.mode, Mode::Explore) && game.race_flow.quick_free_run, "free run explores");
    anyhow::ensure!(game.sim.drive.as_ref().is_some_and(|d| d.region == EXPLORE_REGION), "free run in the city");
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    println!("free run pause: {:?}", menu(game));
    game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(!game.race_flow.quick_free_run, "free run over");
    let saved = Profile::load().unwrap();
    Ok(saved)
}

/// A tournament: each race's results lead to the next, the last to the reward.
pub(super) fn tournament(game: &mut Game) -> Result<()> {
    // A tournament: each race's results lead to the next; the last to
    // the reward.
    let tour = game.game_data.career.events.iter().position(|e| e.races.len() == 2).context("a two-race event")?;
    game.start_race(tour, false, false)?;
    for round in 0..2 {
        // As in the first race: the player is moved along, then the
        // rivals race it out in real frames.
        // The player's place is kept here, not read back from its locator:
        // this event's road passes over itself, where a 100 m jump can
        // land nearer the road it left.
        let mut along = None;
        for _ in 0..60000 {
            let Mode::Race(race) = &game.mode else { break };
            if race.racers.me.finished.is_none() {
                let s = along.unwrap_or(race.racers.me.along) + 100.0;
                along = Some(s);
                let target = race.route.road.point_at(s);
                let d = game.sim.drive.as_mut().unwrap();
                d.state.position = target.extend(d.state.position.z);
                game.advance_for(0.5)?;
            } else {
                game.advance_for(1.0 / 30.0)?;
            }
        }
        if let Mode::Race(race) = &game.mode {
            let rivals: Vec<_> = race
                .racers.rivals
                .iter()
                .map(|r| {
                    (
                        r.name.clone(),
                        r.progress.progress(),
                        r.progress.lap(),
                        r.progress.finished,
                        (r.state.position.x as i32, r.state.position.y as i32),
                        r.state.speed() as i32,
                        r.controls,
                        r.state.drivetrain.as_ref().map(|d| (d.gear, d.rpm() as i32)),
                        r.state.crash,
                        r.state.since_collision,
                        format!(
                            "{} top {} torque {:?}",
                            r.car.car_type.name,
                            r.car.car_type.handling.top_speed_mph,
                            r.car.car_type.handling.engine.torque.values
                        ),
                    )
                })
                .collect();
            anyhow::bail!("results after round {round}: rivals at {:.0} s {rivals:?}", race.timing.clock);
        }
        let Mode::Menu(Menu::Results(results, reward)) = &game.mode else {
            anyhow::bail!("results after round {round}")
        };
        anyhow::ensure!(
            results.kind == ResultsKind::Tournament && results.rows[0].3 == 8,
            "the race's tournament results"
        );
        anyhow::ensure!(
            reward.is_some() == (round == 1),
            "the reward only after the last race (round {round}, rows {:?}, manual {:?})",
            results.rows,
            game.sim.drive.as_ref().map(|d| d.state.manual)
        );
        game.menu_key(Key::Accept)?;
        let Mode::Menu(Menu::Results(standings, _)) = &game.mode else {
            anyhow::bail!("standings after round {round}")
        };
        anyhow::ensure!(standings.kind == (ResultsKind::Standings { next_race: round == 0 }), "the standings");
        anyhow::ensure!(standings.rows[0].3 == 8 * (round as i32 + 1), "points summed");
        game.menu_key(Key::Accept)?;
        if round == 0 {
            // UI_PostRace with "Next Race" first (0x4BEE10, 0x513FA0).
            let first = menu(game).and_then(|(k, o)| (k == MenuKind::PostRace).then(|| o.first().cloned()).flatten());
            anyhow::ensure!(first.as_deref() == Some("Next Race"), "Next Race offered: {:?}", menu(game));
            game.act(Action::NextRace, MenuKind::PostRace)?;
            anyhow::ensure!(matches!(game.mode, Mode::Race(ref r) if r.round == 1), "the second race");
        }
    }
    println!("tournament {} raced through", game.game_data.career.events[tour].name);
    Ok(())
}

/// The particle effects of a solid-wall scrape.
pub(super) fn particles(game: &mut Game) -> Result<()> {
    // The particle effects: a solid-wall scrape bursts glow and sparks
    // (XECS_SOLID_WALL), which fall and die within their lives.
    if let Some(d) = game.sim.drive.as_mut() {
        let at = d.state.position;
        nfsu2::particles::drain();
        nfsu2::particles::post_contact(20, at, 30.0, 50.0, Vec3::new(20.0, 0.0, 0.0), Vec3::Y);
        d.effects.step(&d.world, at, &[], &[], 1.0 / 60.0, &crate::sim::TickSettings::take(&[0; 22]));
        let born = d.effects.system.particles.len();
        anyhow::ensure!(born >= 10, "wall burst particles: {born}");
        for _ in 0..90 {
            d.effects.step(&d.world, at, &[], &[], 1.0 / 60.0, &crate::sim::TickSettings::take(&[0; 22]));
        }
        anyhow::ensure!(
            d.effects.system.particles.is_empty(),
            "particles outlive 1.5 s: {}",
            d.effects.system.particles.len()
        );
        println!("particles: a wall burst spawned {born}");
    }
    Ok(())
}
