//! `--shot` targets: The career's own screens: crib, sponsors, rewards, world map, race results.
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("crib", crib),
    Target::name("crib_tour", crib_tour),
    Target::name("crib_explore", crib_explore),
    Target::name("crib_exit", crib_exit),
    Target::name("sponsor", sponsor),
    Target::name("sponsor_popup", sponsor_popup),
    Target::name("rewards", rewards),
    Target::name("magazines", rewards),
    Target::name("dvds", rewards),
    Target::name("signedsponsors", rewards),
    Target::name("cover", rewards),
    Target::name("magreward", rewards),
    Target::name("dvdreward", rewards),
    Target::name("magview", rewards),
    Target::name("race_stats", race_stats),
    Target::name("results", results),
    Target::name("drift_results", drift_results),
    Target::name("tournament", tournament),
    Target::name("standings", standings),
    Target::name("worldmap", worldmap),
    Target::name("winchain", winchain),
    Target::name("sponsor_world", winchain),
];

pub(super) fn crib(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::Crib))
}

/// The garage's idle tour (0x4D82D0; Wine menus115 tour_t30): idle
/// since −310 s, so the tour starts at −10 s on its first record,
/// "Animate Camera1" (record 42), settled by the shot.
pub(super) fn crib_tour(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let mode = game.icon_menu(MenuKind::Crib);
    game.mode = mode;
    let mut view = nfsu2::garage::View::new(0);
    let mut t = -310.0;
    while t < -1.0 {
        view.tick(t, 1.0 / 60.0, false);
        t += 1.0 / 60.0;
    }
    game.garage_fe.view = Some((game.frontend_scene(), game.garage_view_request(), view));
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The garage's "Return to Explore Mode" question over its scene.
pub(super) fn crib_explore(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.icon_menu(MenuKind::Crib);
    game.garage_request();
    game.act(Action::Explore, MenuKind::Crib)?;
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// Yes on "Return to Explore Mode?" and the drive-out, then
/// NFSU2_SHOT_MS (default 3000) of free roam at the garage marker
/// (with NFSU2_SHOT_SAVE: that profile's garage).
pub(super) fn crib_exit(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, env, .. } = cx;
    game.mode = game.icon_menu(MenuKind::Crib);
    game.garage_request();
    game.act(Action::Explore, MenuKind::Crib)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    let ms: f32 = env.ms.unwrap_or(3000.0);
    for _ in 0..(ms / 1000.0 * 60.0) as usize {
        // The particles too (the window's `step`): the garage
        // marker's ground rings.
        game.advance_for_each(1.0 / 60.0, |g, dt| {
            g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
            Ok(())
        })?;
    }
    let d = game.sim.drive.as_mut().context("free roam")?;
    game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &nfsu2::render::RenderOptions::in_force()));
    game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &nfsu2::render::RenderOptions::in_force()));
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn sponsor(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
    // A loaded profile past stage 1 keeps its stage; NFSU2_SHOT_REP
    // sets the stage's reputation (default 300).
    let stage = if p.stage >= 2 { p.stage } else { 2 };
    p.stage = stage;
    let rep: f32 = std::env::var("NFSU2_SHOT_REP").ok().and_then(|v| v.parse().ok()).unwrap_or(300.0);
    // One normal win: every sponsor on offer.
    let win = game
        .game_data.career
        .events
        .iter()
        .find(|e| e.stage == stage && e.category == career::Category::Normal && !e.suv)
        .context("a race of the stage")?;
    p.completed.insert(win.index);
    p.stage_rep.insert(stage, (rep, 1));
    Ok(game.icon_menu(MenuKind::Sponsor))
}

/// UI_SponsorPopup ("SELECT A SPONSOR!") once its INIT has played.
pub(super) fn sponsor_popup(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::SponsorPopup(-2.0)))
}

pub(super) fn rewards(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    let p = game.profile.get_or_insert_with(|| Profile::new("Player"));
    p.stage = 3;
    for m in game.game_data.career.milestones.iter().filter(|m| m.stage <= 3) {
        p.milestones.insert(m.hash);
    }
    let sponsors: Vec<u32> =
        game.game_data.career.stages.iter().find(|d| d.stage == 2).map(|d| d.sponsors.clone()).unwrap_or_default();
    if let Some(&s) = sponsors.first() {
        p.sponsors.insert(2, s);
    }
    Ok(match screen {
        "rewards" => game.icon_menu(MenuKind::Rewards),
        "magazines" => game.covers_menu(false, 1),
        "dvds" => game.covers_menu(true, 0),
        "signedsponsors" => game.signed_sponsors_menu(),
        // UI_MagazineReward after the first kind-3 shoot.
        "magreward" => {
            let i = game.game_data.career.milestones.iter().position(|m| m.kind == 3).unwrap_or(0);
            game.magazine_reward(i)
        }
        // STAGE_3_DVD_1 ("Four on the Floor", kind 4; Wine menus113
        // w14 / w15).
        "dvdreward" => {
            let i = game.game_data.career.milestones.iter().position(|m| m.kind == 4).unwrap_or(0);
            game.magazine_reward(i)
        }
        // UI_MagazineView on a saved cover (`Game::open_cover`):
        // NFSU2_SHOT_COVER_FILE, an original's or the clone's cover
        // file, else one the shot writes from a flat picture; the
        // view after "Load complete.".
        "magview" => {
            let i = game.earned_covers(false)[0];
            let name = game.profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
            let file = profiles::cover_name(&name, false, game.cover_number(i, false));
            let bytes =
                std::env::var_os("NFSU2_SHOT_COVER_FILE").and_then(|f| std::fs::read(f).ok()).unwrap_or_else(|| {
                    nfsu2::formats::native_save::picture(&[90, 140, 200, 255].repeat(320 * 432), 320, 432)
                });
            profiles::write_cover(&file, &bytes)?;
            game.mode = game.open_cover(i, false);
            game.menu_key(Key::Accept)?;
            std::mem::replace(&mut game.mode, Mode::Explore)
        }
        _ => {
            let i = game.earned_covers(false)[0];
            game.cover_view(i, false, 0.0)
        }
    })
}

pub(super) fn race_stats(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Stats(screens::PostRaceStats {
        racers: vec![screens::RacerStats {
            name: "Player".into(),
            car: "FOCUS".into(),
            time: 83.42,
            top_speed: 61.3,
            average: 38.2,
            best_lap: 27.1,
            laps: vec![(28.4, 1), (27.9, 1), (27.1, 1)],
            status: None,
            shifts: (0, 0),
            drift: None,
        }],
        index: 0,
        player: 0,
        circuit: true,
        drag: false,
        drift: false,
        opened: 0.0,
        arrows: Default::default(),
    })))
}

pub(super) fn results(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Results(
        PostRaceResults {
            kind: ResultsKind::Race,
            rows: vec![("Player".into(), "FOCUS".into(), 83.42, 0)],
            opened: 0.0,
            scores: false,
            player: Some(0),
        },
        None,
    )))
}

/// A career drift's Event Results with the rows the original showed
/// for S3_DRIFT_6 (Wine: Barb / Henry / Arnel and the player 4th).
pub(super) fn drift_results(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Results(
        PostRaceResults {
            kind: ResultsKind::Race,
            rows: vec![
                ("Barb".into(), "CELICA".into(), 54_017.0, 0),
                ("Henry".into(), "SUPRA".into(), 39_252.0, 0),
                ("Arnel".into(), "G35".into(), 25_675.0, 0),
                ("NAME".into(), "PEUGOT".into(), 7_288.0, 0),
            ],
            opened: 0.0,
            scores: true,
            player: Some(3),
        },
        None,
    )))
}

pub(super) fn tournament(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Results(
        PostRaceResults {
            kind: ResultsKind::Tournament,
            rows: vec![("Player".into(), "FOCUS".into(), 83.42, 8)],
            opened: 0.0,
            scores: false,
            player: Some(0),
        },
        None,
    )))
}

pub(super) fn standings(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Results(
        PostRaceResults {
            kind: ResultsKind::Standings { next_race: true },
            rows: vec![("Player".into(), "FOCUS".into(), 0.0, 16)],
            opened: 0.0,
            scores: false,
            player: Some(0),
        },
        None,
    )))
}

pub(super) fn worldmap(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.world_map())
}

/// The winning race's whole post-race chain, driven by the keys as
/// in play (Wine menus112 pr01–pr08): a stage-2 career sprint won
/// (the player's finish forced 2 s in), then Accept through Event
/// Results → Race Reward → "SELECT A SPONSOR!" → Sponsorships → the
/// sign question (Yes) → Post Race. NFSU2_CHAIN_STEP=n stops after
/// n keys (0 the results; default 5, Post Race). Each screen
/// reached is checked and logged ("winchain: …"). `sponsor_world`:
/// the chain to Sponsorships (step 3), over the race's world as in
/// the original (docs/notes/front-end/fe-backdrops.md).
pub(super) fn winchain(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    let p = game.profile.as_mut().context("profile")?;
    p.stage = 2;
    let e = game
        .game_data.career
        .stage_events(&game.game_data.root, 2)
        .iter()
        .find(|e| e.kind == career::EventKind::Sprint && e.category == career::Category::Normal && !e.suv)
        .map(|e| e.index)
        .context("a stage-2 sprint")?;
    game.start_race(e, false, false)?;
    game.scenes.shop = None;
    game.scenes.crowd = None;
    if let Mode::Race(r) = &mut game.mode {
        r.timing.clock = 0.0;
    }
    for _ in 0..60 {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0)?;
    }
    game.platform.keys.clear();
    if let Mode::Race(r) = &mut game.mode {
        r.racers.me.finished = Some(r.timing.clock);
    }
    for _ in 0..30 * 30 {
        game.advance_for(1.0 / 30.0)?;
        if matches!(&game.mode, Mode::Menu(Menu::Results(..))) {
            break;
        }
    }
    let name = |m: &Mode| match m {
        Mode::Menu(Menu::Results(_, Some(_))) => "results (won)",
        Mode::Menu(Menu::Results(_, None)) => "results",
        Mode::Menu(Menu::Reward(_)) => "reward",
        Mode::Menu(Menu::SponsorPopup(_)) => "sponsor popup",
        Mode::Menu(Menu::Icons(_, MenuKind::Sponsor)) => "sponsorships",
        Mode::Menu(Menu::Dialog(..)) => "sign question",
        Mode::Menu(Menu::Icons(_, MenuKind::PostRace)) => "post race",
        Mode::Menu(_) => "another menu",
        Mode::Race(_) => "race",
        _ => "other",
    };
    let want = ["results (won)", "reward", "sponsor popup", "sponsorships", "sign question", "post race"];
    let steps: usize = if screen == "sponsor_world" {
        3
    } else {
        std::env::var("NFSU2_CHAIN_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(5).min(5)
    };
    for (i, w) in want.iter().enumerate().take(steps + 1) {
        if i > 0 {
            // The screens' INITs play out before the key.
            for _ in 0..45 {
                game.advance_for(1.0 / 30.0)?;
            }
            // The sign question opens on No (Wine pr06): Yes is
            // to its left.
            if i == 5 {
                game.menu_key(Key::Left)?;
            }
            game.menu_key(Key::Accept)?;
        }
        eprintln!("winchain: {}", name(&game.mode));
        anyhow::ensure!(name(&game.mode) == *w, "winchain step {i}: {} (wanted {w})", name(&game.mode));
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}
