use crate::*;

/// `game --bench transitions`: how long each kind of track load takes
/// with free roam loaded (the call, then its first frame drawn and the GPU
/// done): into a race, back to free roam, out of the garage and out of a
/// shop with and without a part fitted.
/// The programs made ahead in the menus are made (or are not made ahead).
fn programs_made(game: &Game) -> bool {
    game.loads.programs.as_ref().is_none_or(|p| p.job.is_finished())
}

/// What a car change leaves for the next drive is made ready (the kept
/// world and what goes with it).
fn kept_ready(game: &Game) -> bool {
    game.loads.kept_ahead.as_ref().is_none_or(|k| k.is_finished())
}

/// NFSU2_BENCH_DROP_PAGES=1: the cache's and the cars' files dropped from
/// the OS file cache (the next reads from the disk, as after a reboot).
fn drop_pages(game: &Game) {
    if nfsu2::env_value!("NFSU2_BENCH_DROP_PAGES").is_some_and(|v| v != "0") {
        let cars = nfsu2::game_fs::path(&game.game_data.root, "CARS");
        let mb = super::drop_pages(&[nfsu2::cache::dir().unwrap_or_default(), cars]);
        println!("  (dropped {mb:.0} MB from the OS file cache)");
    }
}

pub(crate) fn bench_transitions(game: &mut Game, step: impl Fn(&mut Game) -> Result<()>) -> Result<()> {
    let wait = |game: &Game| {
        let w = game.platform.window.as_ref().unwrap();
        let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    };
    let timed = |game: &mut Game, what: &str, f: &dyn Fn(&mut Game) -> Result<()>| -> Result<()> {
        let t = Instant::now();
        let cpu0 = nfsu2::prof::thread_cpu_ns();
        f(game)?;
        let load = t.elapsed().as_secs_f64() * 1000.0;
        let cpu1 = nfsu2::prof::thread_cpu_ns();
        let builds = super::Builds::now();
        // The first frame's parts (wall): its update, its draw (with
        // the draw's CPU parts, `prof::acc`), the GPU wait after it.
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        step(game)?;
        let stepped = ms(t1);
        let t2 = Instant::now();
        nfsu2::prof::acc::enable(true);
        let _ = nfsu2::prof::acc::take();
        game.draw()?;
        nfsu2::prof::acc::enable(false);
        let drawn = ms(t2);
        let cpu2 = nfsu2::prof::thread_cpu_ns();
        let t3 = Instant::now();
        wait(game);
        let waited = ms(t3);
        let all = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{what}: load {load:.0} ms, with its first frame {all:.0} ms; track loads {}, PC_Loading shown {}",
            game.loads.track_loads, game.loads.loads_shown
        );
        let mut parts: Vec<_> = nfsu2::prof::acc::take().into_iter().filter(|p| p.1 >= 2_000_000).collect();
        parts.sort_by_key(|p| std::cmp::Reverse(p.1));
        let parts: Vec<String> =
            parts.iter().map(|(l, ns, _)| format!("{} {:.0}", l.trim(), *ns as f64 / 1e6)).collect();
        println!(
            "  (its first frame: update {stepped:.1} ms, draw {drawn:.1} ms, GPU wait {waited:.1} ms; draw CPU parts over 2 ms: {})",
            parts.join(", ")
        );
        println!("  (its first frame's builds: {})", super::Builds::now().since(&builds));
        println!(
            "  (main thread CPU: the load {:.1} ms, its first frame {:.1} ms; rivals taken ahead {} of {}; encounter cars taken ahead {} of {})",
            cpu1.saturating_sub(cpu0) as f64 / 1e6,
            cpu2.saturating_sub(cpu1) as f64 / 1e6,
            game.loads.rivals_taken.0,
            game.loads.rivals_taken.1,
            game.loads.encounters_taken.0,
            game.loads.encounters_taken.1
        );
        (game.loads.rivals_taken, game.loads.encounters_taken) = ((0, 0), (0, 0));
        (game.loads.track_loads, game.loads.loads_shown) = (0, 0);
        // A second of the new place, each frame drawn (its longest frame:
        // work joined or loaded after the call, as the encounter racers').
        let mut worst = (0.0f64, 0usize, String::new());
        let after = super::Builds::now();
        nfsu2::prof::acc::enable(true);
        for i in 0..60 {
            let _ = nfsu2::prof::acc::take();
            let f = Instant::now();
            let t1 = Instant::now();
            step(game)?;
            let update = ms(t1);
            game.draw()?;
            let ms = f.elapsed().as_secs_f64() * 1000.0;
            if ms > worst.0 {
                // The frame's update and its draw's CPU parts over 1 ms.
                let mut parts: Vec<_> = nfsu2::prof::acc::take().into_iter().filter(|p| p.1 >= 1_000_000).collect();
                parts.sort_by_key(|p| std::cmp::Reverse(p.1));
                let parts: Vec<String> =
                    parts.iter().map(|(l, ns, _)| format!("{} {:.1}", l.trim(), *ns as f64 / 1e6)).collect();
                worst = (ms, i + 1, format!("update {update:.1} ms; {}", parts.join(", ")));
            }
        }
        nfsu2::prof::acc::enable(false);
        wait(game);
        println!(
            "  (the second after it: the longest frame {:.1} ms, frame {} ({}); builds {})",
            worst.0,
            worst.1,
            worst.2,
            super::Builds::now().since(&after)
        );
        Ok(())
    };
    // NFSU2_BENCH_MENU_FIRST=1: the first load after the main menu's
    // preload (as a player starting the game sees it).
    if std::env::var_os("NFSU2_BENCH_MENU_FIRST").is_some() {
        let t = Instant::now();
        let mut frames = 0;
        while frames < 600 {
            step(game)?;
            game.draw()?;
            frames += 1;
            if game.loads.preload.as_ref().is_some_and(|p| {
                p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished()) && programs_made(game)
            }) {
                break;
            }
        }
        println!("  (the main menu's preload: {frames} frames, {:.0} ms)", t.elapsed().as_secs_f64() * 1000.0);
    }
    timed(game, "free roam, first load", &|g| g.explore(None))?;
    // NFSU2_BENCH_TRANSITIONS_RESTART=n: a city circuit, then n restarts
    // from its pause, each after two seconds of the race (with
    // NFSU2_BENCH_DROP_PAGES=1 the cache's and the cars' files dropped from
    // the OS file cache before each, as after a reboot).
    if let Some(n) = nfsu2::env_value!("NFSU2_BENCH_TRANSITIONS_RESTART").and_then(|v| v.parse::<usize>().ok()) {
        let race = game
            .game_data.career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .context("a city circuit")?;
        timed(game, "free roam -> race", &|g| g.start_race(race, false, false))?;
        for _ in 0..n {
            for _ in 0..120 {
                step(game)?;
                game.draw()?;
            }
            wait(game);
            // The race's pause, as its key opens it.
            let Mode::Race(r) = std::mem::replace(&mut game.mode, Mode::Explore) else { anyhow::bail!("not racing") };
            game.sim.paused_race = Some(r);
            game.mode = game.icon_menu(MenuKind::RacePause);
            drop_pages(game);
            timed(game, "race -> restart", &|g| {
                g.fe.confirmed = true;
                g.act(Action::Restart, MenuKind::RacePause)
            })?;
        }
        return Ok(());
    }
    // NFSU2_BENCH_TRANSITIONS_KEPT=1: Quick Races one after the other in a
    // region, the world kept for the next (after free roam's first load), for
    // short measuring series: each pair's first race from its car select up
    // while the region loads ahead, then another track from its car select
    // (half a second up) with the last race's drive in memory, the same car
    // and another. Drift tracks and URL circuits (barriers that switch other
    // road pieces on: Ambient Detail's props placed again by the load), city
    // circuits (the same surfaces).
    if nfsu2::env_value!("NFSU2_BENCH_TRANSITIONS_KEPT").is_some_and(|v| v != "0") {
        let city: Vec<u16> = game
            .quick_track_select(screens::QrMode::Circuit, 0.0)
            .context("the circuits")?
            .tracks
            .iter()
            .map(|t| t.number)
            .filter(|n| game.game_data.career.tracks.get(n).is_some_and(|t| t.region == EXPLORE_REGION))
            .take(2)
            .collect();
        anyhow::ensure!(city.len() == 2, "two city circuits");
        let pairs = [
            (screens::QrMode::Drift, "drift", [4312, 4311, 4313]),
            (screens::QrMode::Url, "URL", [4701, 4712, 4713]),
            (screens::QrMode::Circuit, "city circuit", [city[0], city[1], city[0]]),
        ];
        for (mode, name, tracks) in pairs {
            for (k, (track, car)) in tracks.into_iter().zip(["PEUGOT", "PEUGOT", "COROLLA"]).enumerate() {
                if k == 0 {
                    game.drop_drive();
                    (game.loads.resident, game.loads.kept_ahead, game.render.resident) = (None, None, None);
                    game.loads.preload = None;
                }
                let ty = nfsu2::frontend::quick_race::race_type(mode);
                game.quick.tracks = game.quick_track_select(mode, 0.0);
                game.settings.qr[ty].track = track;
                game.settings.qr[ty].reversed = false;
                let e = game.quick_event(mode);
                game.quick.car = Some(car.into());
                game.mode = game.quick_car_select(Some(e));
                // The first race's car select until the loads ahead are
                // done (at most 10 s), the others' for half a second.
                for frame in 0..if k == 0 { 600 } else { 30 } {
                    step(game)?;
                    game.draw()?;
                    let ahead = game.loads.preload.as_ref().is_some_and(|p| {
                        p.world.as_ref().is_none_or(|h| h.is_finished())
                            && p.car.as_ref().is_none_or(|c| c.1.is_finished())
                    });
                    if k == 0 && frame > 0 && ahead && programs_made(game) {
                        break;
                    }
                }
                let what = match k {
                    0 => format!("Quick Race {name} from its car select"),
                    1 => format!("another Quick Race {name}, the world kept"),
                    _ => format!("another Quick Race {name}, another car, the world kept"),
                };
                timed(game, &what, &|g| g.start_race(e, true, false))?;
            }
        }
        return Ok(());
    }
    // NFSU2_BENCH_TRANSITIONS_SHOPS=1: the shops' transitions alone (after
    // free roam's first load), for short measuring series.
    let shops_only = nfsu2::env_value!("NFSU2_BENCH_TRANSITIONS_SHOPS").is_some_and(|v| v != "0");
    if !shops_only {
        let race = game
            .game_data.career
            .stage_events(&game.game_data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .context("a city circuit")?;
        timed(game, "free roam -> race", &|g| g.start_race(race, false, false))?;
        timed(game, "race -> free roam", &|g| g.explore(None))?;
        // The same race after its Engage dialog has been up while its rivals
        // load ahead ([`RivalsAhead`]).
        game.mode = Mode::Menu(Menu::Engage(Engage {
            event: race,
            message: 3,
            opened: 0.0,
            focus: Default::default(),
            purse: game.game_data.career.events[race].reward,
        }));
        {
            let t = Instant::now();
            let mut frames = 0;
            while frames < 600 {
                step(game)?;
                game.draw()?;
                frames += 1;
                if game.loads.rivals_ahead.as_ref().is_some_and(|a| a.loads.iter().all(|(_, h, _)| h.is_finished())) {
                    break;
                }
            }
            println!("  (the Engage dialog's rivals: {frames} frames, {:.0} ms)", t.elapsed().as_secs_f64() * 1000.0);
        }
        timed(game, "free roam -> race, rivals loaded ahead", &|g| g.start_race(race, false, false))?;
        // NFSU2_BENCH_TRANSITIONS_RACE=1: the race's transitions alone.
        if nfsu2::env_value!("NFSU2_BENCH_TRANSITIONS_RACE").is_some_and(|v| v != "0") {
            return Ok(());
        }
        // Back after the post-race menu has been up while free roam's
        // encounter racers load ahead ([`EncountersAhead`]); with
        // NFSU2_BENCH_DROP_PAGES=1 the cache's and the cars' files dropped
        // from the OS file cache first, as after a reboot.
        drop_pages(game);
        game.mode = game.icon_menu(MenuKind::PostRace);
        {
            let t = Instant::now();
            let mut frames = 0;
            while frames < 600 {
                step(game)?;
                game.draw()?;
                frames += 1;
                if game.loads.encounters_ahead.as_ref().is_some_and(|a| a.loads.iter().all(|(_, h)| h.is_finished())) {
                    break;
                }
            }
            println!(
                "  (the post-race menu's encounter racers: {frames} frames, {:.0} ms)",
                t.elapsed().as_secs_f64() * 1000.0
            );
        }
        timed(game, "that race -> free roam, encounter racers loaded ahead", &|g| g.explore(None))?;
    }
    // NFSU2_BENCH_TRANSITIONS_ROAM=1: the ways into free roam with its
    // encounter racers loaded ahead alone: the post-race menu's above, then
    // the main menu's as at a game's start (the region not kept, no
    // traffic yet, the menus' loads ahead in).
    if nfsu2::env_value!("NFSU2_BENCH_TRANSITIONS_ROAM").is_some_and(|v| v != "0") {
        game.drop_drive();
        game.sim.traffic = None;
        drop_pages(game);
        game.loads.resident = None;
        game.loads.kept_ahead = None;
        game.render.resident = None;
        game.loads.preload = None;
        game.mode = game.icon_menu(MenuKind::Main);
        let t = Instant::now();
        let mut frames = 0;
        while frames < 600 {
            step(game)?;
            game.draw()?;
            frames += 1;
            let preload = game.loads.preload.as_ref().is_some_and(|p| {
                p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_some_and(|c| c.1.is_finished())
            });
            let racers =
                game.loads.encounters_ahead.as_ref().is_some_and(|a| a.loads.iter().all(|(_, h)| h.is_finished()));
            if frames >= 60 && preload && racers && programs_made(game) {
                break;
            }
        }
        println!("  (the main menu's loads ahead: {frames} frames, {:.0} ms)", t.elapsed().as_secs_f64() * 1000.0);
        return timed(game, "menu -> free roam, encounter racers loaded ahead", &|g| g.explore(None));
    }
    let shop = |kind: u8| game.game_data.career.shops.iter().find(|s| s.kind == kind).map(|s| s.hash);
    let (garage, parts) = (shop(0).context("a garage")?, shop(2).context("a body shop")?);
    // Into the shops' front ends (their garage scenes built on the first
    // frame).
    for kind in [0u8, 2] {
        let i = game.game_data.career.shops.iter().position(|s| s.kind == kind).context("a shop")?;
        timed(game, if kind == 0 { "into the garage" } else { "into a shop" }, &|g| {
            // (Set by the shop's engage, as in the game.)
            g.shop.visit = Some(g.game_data.career.shops[i].hash);
            g.mode = g.shop_front_end(i);
            Ok(())
        })?;
    }
    timed(game, "out of the garage", &|g| {
        g.shop.visit = Some(garage);
        g.explore(g.here())
    })?;
    timed(game, "out of a shop", &|g| {
        g.shop.visit = Some(parts);
        g.explore(g.here())
    })?;
    timed(game, "out of a shop, a part fitted", &|g| {
        if let Some(p) = g.profile.as_mut() {
            let car = p.car.clone();
            if let Some(part) = (0..0x40usize)
                .find_map(|slot| g.game_data.parts.find_after(&car, slot, None).filter(|x| x.level > 0).map(|x| (slot, x.hash)))
            {
                p.installed.insert(part.0 as u8, part.1);
            }
        }
        let at = g.here();
        g.drop_drive();
        g.shop.visit = Some(parts);
        g.explore(at)
    })?;
    // The same exit as a player makes it: the part fitted in the shop's
    // menus (the drive dropped there), which stay up while the car and
    // what the next drive takes are made ahead, then the exit.
    {
        let fitted = game.profile.as_ref().map(|p| p.installed.clone()).unwrap_or_default();
        if let Some(p) = game.profile.as_mut() {
            let car = p.car.clone();
            if let Some(part) = (0..0x40usize).find_map(|slot| {
                game.game_data
                    .parts
                    .find_after(&car, slot, None)
                    .filter(|x| x.level > 0 && fitted.get(&(slot as u8)) != Some(&x.hash))
                    .map(|x| (slot, x.hash))
            }) {
                p.installed.insert(part.0 as u8, part.1);
            }
        }
        let at = game.here();
        let i = game.game_data.career.shops.iter().position(|s| s.hash == parts).context("a body shop")?;
        game.shop.visit = Some(parts);
        game.mode = game.shop_front_end(i);
        game.drop_drive();
        let t = Instant::now();
        let (mut frames, mut worst) = (0, 0.0f64);
        while frames < 600 {
            let f = Instant::now();
            let _t = nfsu2::prof::span("bench: a shop menu frame");
            step(game)?;
            game.draw()?;
            drop(_t);
            // Paced as the window's swap chain paces them (two frames in
            // flight), so the exit finds the GPU as far behind as a
            // player's would be, not the unpaced backlog of the loop.
            let ended = game.platform.window.as_ref().map_or(0, |w| w.frames_ended);
            super::throttle(game, ended, 2);
            if frames > 0 {
                worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
            }
            frames += 1;
            let car = game.loads.preload.as_ref().is_some_and(|p| p.car.as_ref().is_some_and(|c| c.1.is_finished()));
            let racers = game.loads.encounters_ahead.as_ref().is_some_and(|a| a.loads.iter().all(|(_, h)| h.is_finished()));
            if frames >= 60 && car && racers && kept_ready(game) {
                break;
            }
        }
        println!(
            "  (the shop's menus after the part: {frames} frames, {:.0} ms, the longest frame after the first {worst:.1} ms)",
            t.elapsed().as_secs_f64() * 1000.0
        );
        timed(game, "out of a shop, a part fitted, after the shop's menus", &|g| {
            g.shop.visit = Some(parts);
            g.explore(at)
        })?;
    }
    if shops_only {
        return Ok(());
    }
    // To the main menu (the region not kept), then free roam: at once, and
    // after the menus' preload ([`Preload`]) has had time to finish.
    let to_menu = |g: &mut Game| {
        g.drop_drive();
        g.loads.resident = None;
        g.loads.kept_ahead = None;
        g.render.resident = None;
        g.loads.preload = None;
        g.mode = g.icon_menu(MenuKind::Main);
    };
    to_menu(game);
    timed(game, "menu -> free roam, at once", &|g| g.explore(None))?;
    to_menu(game);
    let t = Instant::now();
    let (mut frames, mut worst) = (0, 0.0f64);
    while frames < 600 {
        let f = Instant::now();
        step(game)?;
        game.draw()?;
        if frames > 0 {
            worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
        }
        frames += 1;
        // NFSU2_BENCH_MENU_FRAMES=n: at least n menu frames (to compare
        // the menu's frames with NFSU2_NO_PRELOAD=1).
        let least: usize = std::env::var("NFSU2_BENCH_MENU_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        if frames >= least
            && game.loads.preload.as_ref().is_none_or(|p| {
                p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished()) && programs_made(game)
            })
        {
            break;
        }
    }
    println!(
        "  (the menus' preload: {frames} menu frames, {:.0} ms, the longest frame after the first {worst:.1} ms)",
        t.elapsed().as_secs_f64() * 1000.0
    );
    timed(game, "menu -> free roam, preloaded", &|g| g.explore(None))?;
    // A race on a track of its own region: from free roam at once, and
    // after its Engage dialog has been up while the race preloads; then back
    // to free roam at once, and after the post-race menu's preload.
    let wait_preload = |game: &mut Game, what: &str| -> Result<()> {
        let t = Instant::now();
        let (mut frames, mut worst) = (0, 0.0f64);
        while frames < 600 {
            let f = Instant::now();
            step(game)?;
            game.draw()?;
            if frames > 0 {
                worst = worst.max(f.elapsed().as_secs_f64() * 1000.0);
            }
            frames += 1;
            if game.loads.preload.as_ref().is_some_and(|p| {
                p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished()) && programs_made(game)
            }) {
                break;
            }
        }
        println!(
            "  ({what}: {frames} frames, {:.0} ms, the longest frame after the first {worst:.1} ms)",
            t.elapsed().as_secs_f64() * 1000.0
        );
        Ok(())
    };
    let away = game
        .game_data.career
        .events
        .iter()
        .find(|e| {
            e.kind == career::EventKind::Circuit
                && e.races.first().and_then(|r| game.game_data.career.tracks.get(&r.0)).is_some_and(|t| t.region != EXPLORE_REGION)
        })
        .map(|e| e.index);
    if let Some(e) = away {
        timed(game, "free roam -> another region's race, at once", &|g| g.start_race(e, false, false))?;
        timed(game, "that race -> free roam, at once", &|g| g.explore(None))?;
        game.mode = Mode::Menu(Menu::Engage(Engage {
            event: e,
            message: 3,
            opened: 0.0,
            focus: Default::default(),
            purse: game.game_data.career.events[e].reward,
        }));
        wait_preload(game, "the Engage dialog's preload")?;
        timed(game, "free roam -> another region's race, preloaded", &|g| g.start_race(e, false, false))?;
        game.mode = game.icon_menu(MenuKind::PostRace);
        wait_preload(game, "the post-race menu's preload")?;
        timed(game, "that race -> free roam, preloaded", &|g| g.explore(None))?;
    }
    // Quick Race starts from the main menu (the region not kept, as a
    // player coming from the menus has it), each mode's first track.
    for (mode, what) in
        [(screens::QrMode::Circuit, "menu -> Quick Race circuit"), (screens::QrMode::Drag, "menu -> Quick Race drag")]
    {
        to_menu(game);
        game.quick.tracks = game.quick_track_select(mode, 0.0);
        let e = game.quick_event(mode);
        game.quick.car = Some("PEUGOT".into());
        timed(game, what, &|g| g.start_race(e, true, false))?;
    }
    // The same starts as a player makes them: from the Quick Race car
    // select, which has been up while the track's region loads ahead.
    for (mode, what) in [
        (screens::QrMode::Circuit, "Quick Race circuit from its car select"),
        (screens::QrMode::Drag, "Quick Race drag from its car select"),
    ] {
        to_menu(game);
        game.quick.tracks = game.quick_track_select(mode, 0.0);
        let e = game.quick_event(mode);
        game.quick.car = Some("PEUGOT".into());
        game.mode = game.quick_car_select(Some(e));
        wait_preload(game, "the car select's load ahead")?;
        timed(game, what, &|g| g.start_race(e, true, false))?;
    }
    Ok(())
}
