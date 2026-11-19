use crate::*;

/// A replay re-simulates the race from its green-light snapshot with
/// the recorded inputs and ends where the race did: every car's
/// position and every racer's distance match.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn replay_reproduces_the_race() {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    let event = game.game_data.career.events.iter().position(|e| e.kind == career::EventKind::Circuit).unwrap();
    game.start_race(event, true, false).unwrap();
    // Past the race's start scene (if one plays) or the countdown, to
    // the green light.
    if game.scenes.shop.is_some() {
        while game.scenes.shop.is_some() && matches!(game.mode, Mode::Race(_)) {
            game.advance_for(1.0 / 60.0).unwrap();
        }
    } else {
        for _ in 0..(COUNTDOWN * 60.0) as u32 + 2 {
            game.advance_for(1.0 / 60.0).unwrap();
        }
    }
    // Weave and brake at uneven drawn frame times (1/45 s and 1/60 s, the
    // window's path: the world takes ticks only), with a shift and
    // a reset. One recorded frame per tick.
    let mut ticks = 0;
    for i in 0..900u32 {
        game.platform.keys.clear();
        game.platform.keys.insert(if i % 300 < 250 { KeyCode::ArrowUp } else { KeyCode::ArrowDown });
        match i % 120 {
            0..30 => drop(game.platform.keys.insert(KeyCode::ArrowLeft)),
            60..80 => drop(game.platform.keys.insert(KeyCode::ArrowRight)),
            _ => {}
        }
        if i == 400 {
            // As the R key does (the key handler needs an event loop).
            let (Mode::Race(r), Some(d)) = (&mut game.mode, game.sim.drive.as_mut()) else { panic!("racing") };
            reset_to_road(d, r);
            r.replay.recording.as_mut().unwrap().reset = true;
        }
        game.advance(if i % 3 == 0 { 1.0 / 45.0 } else { 1.0 / 60.0 }).unwrap();
        ticks += game.pacing.frame_updates;
    }
    game.platform.keys.clear();
    let ends = |game: &Game, race: &Race| {
        let mut v = vec![(game.sim.drive.as_ref().unwrap().state.position, race.racers.me.distance)];
        v.extend(race.racers.rivals.iter().map(|r| (r.state.position, r.progress.distance)));
        (v, race.timing.clock)
    };
    let Mode::Race(race) = &game.mode else { panic!("racing") };
    let recorded = ends(&game, race);
    let frames = race.replay.recording.as_ref().unwrap().frames.len();
    assert!(ticks > 900, "uneven frames took extra ticks: {ticks}");
    assert_eq!(frames, ticks);
    assert!(recorded.0[0].1 > 50.0 && recorded.0[1..].iter().any(|&(_, m)| m > 50.0), "the cars raced: {recorded:?}");
    // From the race's pause (UI_pause's Replay): THE_PRESENT taken and
    // a skip to the last marker; Back skips to THE_PRESENT and reopens
    // the pause on the race as it was.
    let Mode::Race(r) = std::mem::replace(&mut game.mode, Mode::Explore) else { panic!("racing") };
    game.sim.paused_race = Some(r);
    game.mode = game.icon_menu(MenuKind::RacePause);
    let Mode::Menu(Menu::Icons(m, _)) = &game.mode else { panic!("the pause") };
    assert!(m.options.iter().any(|o| matches!(o.action, Action::Replay)));
    game.act(Action::Replay, MenuKind::RacePause).unwrap();
    let Mode::Race(r) = &game.mode else { panic!("the replay") };
    let last = *r.replay.recording.as_ref().unwrap().markers().last().unwrap();
    assert!(
        r.replay.playback.as_ref().is_some_and(|p| p.pause && p.present.is_some() && p.skip == Some((SkipTo::Lap(last), 0.0)))
    );
    for _ in 0..90 {
        game.advance_for(1.0 / 60.0).unwrap();
    }
    game.key(KeyCode::Escape, &mut || {}).unwrap();
    for _ in 0..60 {
        if !matches!(game.mode, Mode::Race(_)) {
            break;
        }
        game.advance_for(1.0 / 60.0).unwrap();
    }
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::RacePause))));
    assert_eq!(ends(&game, game.sim.paused_race.as_ref().unwrap()), recorded);
    game.act(Action::ResumeRace, MenuKind::RacePause).unwrap();
    let menu = game.icon_menu(MenuKind::PostRace);
    game.keep_replay(menu);
    // Played to its last frame, the replay holds there, paused, until
    // Back, which (from the post-race menu, no THE_PRESENT) leaves at
    // once.
    let play_out = |game: &mut Game| {
        for _ in 0..2000 {
            let Mode::Race(r) = &game.mode else { panic!("the replay ended by itself") };
            let (p, rec) = (r.replay.playback.as_ref().unwrap(), r.replay.recording.as_ref().unwrap());
            if p.at >= rec.frames.len() && p.paused && p.skip.is_none() {
                break;
            }
            game.advance_for(1.0 / 60.0).unwrap();
        }
        game.advance_for(1.0 / 60.0).unwrap();
        let Mode::Race(r) = &game.mode else { panic!("the replay ended by itself") };
        let replayed = ends(game, r);
        game.key(KeyCode::Escape, &mut || {}).unwrap();
        assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))));
        replayed
    };
    // Replay twice: a replay leaves the snapshot as it was. From the
    // post-race menu it starts at the first marker (the green light).
    for _ in 0..2 {
        game.act(Action::Replay, MenuKind::PostRace).unwrap();
        assert!(
            matches!(&game.mode, Mode::Race(r) if r.replay.playback.as_ref().is_some_and(|p| p.skip == Some((SkipTo::Lap(0), 0.0)) && p.present.is_none()))
        );
        assert_eq!(play_out(&mut game), recorded);
    }
    // LAP_SNAPSHOTs at 0 and 15 s; the second is under 20 s from the
    // end. INSTANT_REPLAYs every 300 ticks (5 s), the two last kept
    // (10 and 15 s): the first of them is a marker, the 20 s rule
    // aside.
    let rec = game.sim.replay.as_ref().unwrap().replay.recording.as_ref().unwrap();
    assert_eq!(rec.laps.len(), 2);
    assert!(rec.laps[0].time == 0.0 && (15.0..15.1).contains(&rec.laps[1].time));
    assert_eq!(rec.instants.len(), 2);
    assert!(
        (10.0..10.2).contains(&rec.instants[0].time) && (15.0..15.2).contains(&rec.instants[1].time),
        "{} {}",
        rec.instants[0].time,
        rec.instants[1].time
    );
    assert_eq!(rec.markers(), vec![0, INSTANT_MARK]);
    // A skip to the 15 s snapshot, or to the instant replay, plays on
    // to the same end.
    for to in [1, INSTANT_MARK] {
        game.act(Action::Replay, MenuKind::PostRace).unwrap();
        if let Mode::Race(r) = &mut game.mode {
            r.replay.playback.as_mut().unwrap().skip = Some((SkipTo::Lap(to), 0.0));
        }
        assert_eq!(play_out(&mut game), recorded);
    }
}

/// The replay camera's cycle (0x5FAFE0) and its automatic picks
/// (0x5FB060).
#[test]
fn replay_camera_cycles_and_picks() {
    // Reads the process-wide extended-cameras switch its neighbour sets:
    // not while that test runs.
    super::test_isolation::exclusive();
    let mut rng = nfsu2::opponents::GameRng::default();
    let mut cam = ReplayCamera::default();
    cam.direct(&mut rng, 0.0, 0.0, false);
    let first = cam.view.unwrap();
    assert!(VIEW_ORDER.contains(&first));
    assert_eq!(cam.since, Some(0.0));
    // Held for 4 s, and after that only at speed.
    cam.direct(&mut rng, 50.0, 3.9, false);
    assert_eq!(cam.view, Some(first));
    for _ in 0..50 {
        cam.direct(&mut rng, 5.0, 10.0, false);
    }
    assert_eq!(cam.view, Some(first));
    // Reversing (forward speed below 0) never draws either.
    for _ in 0..50 {
        cam.direct(&mut rng, -20.0, 10.0, false);
    }
    assert_eq!(cam.since, Some(0.0));
    // Automatic showing view 2: the cycle skips index 1.
    cam.view = Some(2);
    cam.cycle();
    assert_eq!((cam.index, cam.view), (2, Some(4)));
    // The cycle leaves +0x214 alone.
    assert_eq!(cam.since, Some(0.0));
    for _ in 0..4 {
        // A director frame on each index: a chosen one holds.
        cam.direct(&mut rng, 50.0, 15.0, false);
        cam.cycle();
    }
    assert_eq!(cam.index, 0);
    // Back to automatic: a pick at once, passing over the view showing
    // (unless the draw is past the last weight: then the last view, as
    // the game's loop ends on it).
    cam.direct(&mut rng, 0.0, 20.0, false);
    assert_eq!(cam.since, Some(20.0));
    // A track shot that ends with none after it: a chase view drawn at
    // once and stamped, whatever the speed.
    cam.direct(&mut rng, 0.0, 21.0, true);
    assert_eq!(cam.since, Some(21.0));
}

/// 0x5FB060's gate on the track draw (docs/notes/parity/replay-cameras.md): a drawn
/// chase view keeps the track cameras off for 1.5 s (unless the cameras
/// were just reset, +0x224) and holds outright from 1.5 to 4 s; a held
/// track shot is not gated.
#[test]
fn replay_chase_views_hold_before_a_track_camera() {
    let mut cam = ReplayCamera::default();
    // Never stamped (+0x214 = 0): a track may be drawn at once.
    assert_eq!(cam.gate(0.0, false), Some(true));
    // After the reset (replay start, every skip): the track draw is
    // allowed under 1.5 s, held 1.5–4 s, allowed after.
    cam.reset(10.0);
    assert_eq!(cam.gate(10.5, false), Some(true));
    assert_eq!(cam.gate(12.0, false), None);
    assert_eq!(cam.gate(14.5, false), Some(true));
    // A track started clears +0x224; a chase view drawn after it keeps
    // the draw off under 1.5 s.
    cam.track_started(15.0);
    assert_eq!(cam.since, Some(15.0));
    let mut rng = nfsu2::opponents::GameRng::default();
    cam.direct(&mut rng, 30.0, 20.0, true);
    assert_eq!(cam.gate(20.5, false), Some(false));
    assert_eq!(cam.gate(21.6, false), None);
    assert_eq!(cam.gate(24.1, false), Some(true));
    // On a track mover the chase holds do not apply.
    assert_eq!(cam.gate(21.6, true), Some(true));
    assert_eq!(cam.gate(20.5, true), Some(false));
}

/// Replay Cameras "Extended": C goes through the native six, then
/// the four extras, then back to automatic; native wraps at six. The
/// extras are movers of their own (cuts) with depth of field.
#[test]
fn replay_extended_cameras_follow_the_native_six() {
    super::test_isolation::exclusive();
    let mut e = nfsu2::qol::Enhancements::default();
    e.extras.replay_cameras = true;
    e.apply();
    let mut cam = ReplayCamera::default();
    let seq: Vec<usize> = (0..11)
        .map(|_| {
            cam.cycle();
            cam.index
        })
        .collect();
    assert_eq!(seq, [1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1]);
    // Leaving an extra keeps the chase view it had.
    assert!(cam.view.is_some());
    assert!(nfsu2::render::blur_dof::dof_scene(0x200 + 6));
    nfsu2::qol::Enhancements::default().apply();
    let mut cam = ReplayCamera::default();
    let seq: Vec<usize> = (0..7)
        .map(|_| {
            cam.cycle();
            cam.index
        })
        .collect();
    assert_eq!(seq, [1, 2, 3, 4, 5, 0, 1]);
}
