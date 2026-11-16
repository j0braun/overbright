use crate::*;

/// The clone's DynoEnter mix for a waveform comparison with the
/// original (`docs/evidence/s3-audio/waveform.md`): with
/// NFSU2_WAVE_SAVE=<a native save> and NFSU2_PATTERN_PCM=<out.f32>, the
/// save's car runs DynoEnter at one step per 1/30 s through the offline
/// mixer (48 kHz stereo f32) and the PCM is written. Run on request (`--ignored`).
#[test]
#[ignore = "capture tool: reads the game, NFSU2_WAVE_SAVE and NFSU2_PATTERN_PCM; run with --ignored"]
fn dyno_enter_waveform_capture() {
    crate::tests::testkit::game();
    let save = std::env::var_os("NFSU2_WAVE_SAVE").expect("NFSU2_WAVE_SAVE: the native save to capture");
    let out = std::env::var_os("NFSU2_PATTERN_PCM").expect("NFSU2_PATTERN_PCM: the PCM file to write");
    let save = nfsu2::formats::native_save::NativeSave::parse(&std::fs::read(save).unwrap()).unwrap();
    let mut game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    game.settings.apply_native(&save);
    game.profile = Some(Profile::from_native(&save, ctx));
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.engine_sound(0.0);
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno_scene();
    assert!(game.scenes.shop.is_some(), "no DynoEnter");
    let mut pcm = Vec::new();
    for _ in 0..330 {
        game.step_for(1.0 / 30.0).unwrap();
        game.audio.as_ref().unwrap().mixer.pump(1600, &mut pcm);
        if std::env::var_os("NFSU2_WAVE_TRACE").is_some() {
            let (a, d) = (game.audio.as_ref().unwrap(), game.sim.drive.as_ref().unwrap());
            let h = &a.engine.as_ref().unwrap().1;
            let l0 = a.levels.level(0, 0);
            eprintln!(
                "wave: level0 {} cam {:.1} m ctx {} {} voices {:?} p4 {} reverb {:?} engine players {:?}",
                l0,
                d.camera.position.distance(d.state.position),
                h.ctx.rpm,
                h.ctx.throttle,
                h.voices(l0, a.engine.as_ref().unwrap().2).1,
                h.abk_block(l0)[4],
                a.reverb.out,
                a.car_sfx
                    .engine_players()
                    .iter()
                    .filter(|p| p.0 != 0)
                    .map(|p| (p.1, p.2.iter().find(|x| x.0 == 2).map_or(-1, |x| x.1)))
                    .collect::<Vec<_>>()
            );
        }
    }
    std::fs::write(out, pcm.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
}

/// The dyno's music dip (docs/l1-leftovers.md §L3.4): under Wine
/// (the finished-career save, master / game music 1.0) the song's level (+0xBC >> 8) holds
/// 17–18 into DynoEnter, falls to 10 as SFXCTL_Tunnel's f2 turns 1
/// (the car in L4RG's kind-0x14 zone, ~7 s in) and rises to 19 when
/// the scene ends (SFXObj_NISStream f5 = race status 2 off) with f2
/// still 1 on the dyno screen.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn dyno_enter_music_dips_in_the_dyno_zone() {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("DYNO_DIP");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "LANCEREVO8", &mut game.rng);
    game.profile = Some(profile);
    let st = &mut game.settings;
    (st.master, st.engine, st.effects, st.tires, st.speech, st.game_music, st.menu_music) =
        (1.0, 0.65, 1.0, 0.65, 1.0, 1.0, 1.0);
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    for _ in 0..30 {
        game.step_for(1.0 / 30.0).unwrap();
    }
    assert_eq!(game.audio.as_ref().unwrap().driveby.tunnel.fields(), [0; 4], "free roam at the start: no zone");
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno_scene();
    assert!(game.scenes.shop.is_some(), "no DynoEnter");
    let mut rows = Vec::new();
    for i in 0..600 {
        game.step_for(1.0 / 30.0).unwrap();
        let a = game.audio.as_ref().unwrap();
        let lvl = a.music_levels.map_or(-1, |m| m[1]);
        rows.push((i, game.scenes.shop.is_some(), a.driveby.tunnel.fields()[2], lvl));
        if std::env::var_os("NFSU2_DIP_TRACE").is_some() {
            eprintln!(
                "dip: {i} scene {} f {:?} lvl {} ({})",
                game.scenes.shop.is_some(),
                a.driveby.tunnel.fields(),
                lvl,
                lvl >> 8
            );
        }
    }
    let first_in = rows.iter().find(|r| r.2 == 1).expect("the car never enters the dyno zone");
    assert!(first_in.1, "the zone is reached during the scene");
    assert!((90..330).contains(&first_in.0), "zone entered at frame {} (native ≈ 7 s = 210)", first_in.0);
    let before = rows.iter().filter(|r| r.1 && r.0 + 15 < first_in.0 && r.0 > 30).map(|r| r.3).max().unwrap();
    let during = rows.iter().filter(|r| r.1 && r.0 > first_in.0 + 30).map(|r| r.3).min().unwrap();
    let ratio = during as f32 / before as f32;
    // −510 mB: 10^(−5.1 / 20) = 0.556 (native 18 → 10).
    assert!((0.45..0.65).contains(&ratio), "music in the zone {during} vs {before} before ({ratio:.2})");
    let after = rows.last().unwrap();
    assert!(!after.1 && after.2 == 1, "on the dyno screen the car stays in the zone");
    // The levels against the original's (>> 8): 17–18 before the
    // zone, 10 in it, 19 on the dyno screen.
    assert!((16..=19).contains(&(before >> 8)), "before the zone {} (native 17–18)", before >> 8);
    assert!((9..=11).contains(&(during >> 8)), "in the zone {} (native 10)", during >> 8);
    assert!((18..=20).contains(&(after.3 >> 8)), "dyno screen {} (native 19)", after.3 >> 8);
}

/// The engine's level against the original's (`docs/engine-level.md`,
/// read under Wine with the finished-career save's volumes: master 0.5, engine
/// 0.65, effects 1, music 0). Free roam idle: the mix map's level[0]
/// 4181 (from the CAR event's p4 −31491 at HM +0x7C 10000) and the
/// mixer's buses at 1 (the option volumes are in the levels only).
/// DynoEnter: the NIS view mode 4 gives level[0] ≈ 1530 at the
/// camera's 40 m; SFXCTL_Tunnel keeps the reverb's dry B 0x7FFF (the
/// abk layers' kind-8 volume); the engine is heard every second
/// (−45…−28 dBFS natively; the clone was digitally silent). The dyno
/// screen after it: the car still idles at level[0] 5518 (p4 −31083).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn engine_level_matches_native_free_roam_and_dyno() {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ENGINE_LEVEL");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "LANCEREVO8", &mut game.rng);
    game.profile = Some(profile);
    let st = &mut game.settings;
    (st.master, st.engine, st.effects, st.tires, st.speech, st.game_music, st.menu_music) =
        (0.5, 0.65, 1.0, 0.65, 1.0, 0.0, 0.0);
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    let db = |s: &[f32]| {
        10.0 * ((s.iter().map(|&x| f64::from(x) * f64::from(x)).sum::<f64>() / s.len().max(1) as f64).max(1e-30))
            .log10()
    };
    let mut pcm = Vec::new();
    for _ in 0..4 * 60 {
        game.step_for(1.0 / 60.0).unwrap();
        game.audio.as_ref().unwrap().mixer.pump(800, &mut pcm);
    }
    let a = game.audio.as_ref().unwrap();
    let level0 = a.levels.level(0, 0);
    assert!((level0 - 4181).abs() <= 21, "free roam idle level[0] {level0} (native 4181)");
    let buses = a.mixer.levels();
    assert_eq!((buses.effects, buses.engine), (1.0, 1.0), "the option volumes are applied once, in the mix map");
    let idle = db(&pcm[pcm.len() - 2 * 96000..]);
    assert!(idle > -50.0, "free roam idle {idle:.1} dBFS (native −43 with the world's beds)");
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno_scene();
    assert!(game.scenes.shop.is_some(), "no DynoEnter");
    let (mut pcm, mut first, mut dry) = (Vec::new(), None, i32::MAX);
    for i in 0..300 {
        game.step_for(1.0 / 30.0).unwrap();
        let a = game.audio.as_ref().unwrap();
        a.mixer.pump(1600, &mut pcm);
        // The level where the camera is 40 m from the car (the scene
        // opens 43.7 m away in the original's capture; the car now
        // drives in on its mover as there).
        let d = game.sim.drive.as_ref().unwrap();
        if game.scenes.shop.is_some() && d.camera.position.distance(d.state.position) <= 40.0 {
            first.get_or_insert(a.levels.level(0, 0));
        }
        if i > 0 && game.scenes.shop.is_some() {
            dry = dry.min(a.reverb.out[3]);
        }
    }
    let first = first.unwrap();
    assert!((1300..1800).contains(&first), "DynoEnter's level[0] {first} at 40 m (native ≈ 1530)");
    assert_eq!(dry, 0x7FFF, "the reverb's dry B through the scene");
    for s in 1..9 {
        let v = db(&pcm[s * 96000..(s + 1) * 96000]);
        assert!(v > -60.0, "DynoEnter second {s}: {v:.1} dBFS (native −45…−28)");
    }
    assert!(game.scenes.shop.is_none() && game.tuning_menu_over_world(), "on the dyno screen");
    let a = game.audio.as_ref().unwrap();
    let level0 = a.levels.level(0, 0);
    assert!(
        a.car_sfx.engine_playing() && (5000..5800).contains(&level0),
        "the dyno screen's idle: level[0] {level0} (native 5518)"
    );
    let screen = db(&pcm[pcm.len() - 96000..]);
    assert!(screen > -50.0, "the dyno screen's idle {screen:.1} dBFS (native −40)");
}

/// The clone's free-roam mix for a level comparison with the original
/// (`docs/engine-level.md`): with NFSU2_WAVE_SAVE=<a native save> and
/// NFSU2_PATTERN_PCM=<out.f32>, the save's car in free roam idles 8 s,
/// holds the throttle 4 s and idles 8 s (1/60 s steps, offline mixer,
/// 48 kHz stereo f32). Run on request (`--ignored`).
#[test]
#[ignore = "capture tool: reads the game, NFSU2_WAVE_SAVE and NFSU2_PATTERN_PCM; run with --ignored"]
fn free_roam_waveform_capture() {
    crate::tests::testkit::game();
    let save = std::env::var_os("NFSU2_WAVE_SAVE").expect("NFSU2_WAVE_SAVE: the native save to capture");
    let out = std::env::var_os("NFSU2_PATTERN_PCM").expect("NFSU2_PATTERN_PCM: the PCM file to write");
    let save = nfsu2::formats::native_save::NativeSave::parse(&std::fs::read(save).unwrap()).unwrap();
    let mut game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    game.settings.apply_native(&save);
    game.profile = Some(Profile::from_native(&save, ctx));
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    let mut pcm = Vec::new();
    for i in 0..20 * 60 {
        if i == 8 * 60 {
            game.platform.keys.insert(KeyCode::ArrowUp);
        } else if i == 12 * 60 {
            game.platform.keys.remove(&KeyCode::ArrowUp);
        }
        game.step_for(1.0 / 60.0).unwrap();
        game.audio.as_ref().unwrap().mixer.pump(800, &mut pcm);
        if std::env::var_os("NFSU2_WAVE_TRACE").is_some() && i % 60 == 0 {
            let a = game.audio.as_ref().unwrap();
            let (_, h, min) = a.engine.as_ref().unwrap();
            let l0 = a.levels.level(0, 0);
            eprintln!(
                "free: {} s level0 {l0} ctx {} {} out {:?} p4 {} bytes {:?}",
                i / 60,
                h.ctx.rpm,
                h.ctx.throttle,
                h.out,
                h.abk_block(l0)[4],
                h.voices(l0, *min).1
            );
        }
    }
    std::fs::write(out, pcm.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
}

/// The tuning screens' gear count (asked every frame through the master
/// list's options) is kept for its car and performance parts: the same
/// number as the handling's parse, worked out again when the car or the
/// parts change.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn tuning_gears_follow_the_car_and_its_parts() {
    super::testkit::game();
    let mut game = Game::new().unwrap();
    let fresh = |game: &Game| game.tuned_handling().map_or(0, |h| h.gearbox.ratios.len().saturating_sub(2) as u32);
    assert_eq!(game.tuned_gears(), 0, "no profile");
    let mut profile = Profile::new("GEARS");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "LANCEREVO8", &mut game.rng);
    game.profile = Some(profile);
    let stock = fresh(&game);
    assert!(stock >= 4, "{stock} forward gears");
    assert_eq!(game.tuned_gears(), stock);
    assert_eq!(game.tuned_gears(), stock, "kept");
    assert!(game.dyno.gears.lock().unwrap().as_ref().is_some_and(|(k, _)| k.0 == "LANCEREVO8"));
    // Each performance part on its own: the count the parse gives, kept
    // for the parts it was worked out with.
    for id in 0..=u8::MAX {
        game.profile.as_mut().unwrap().perf.insert(id);
        assert_eq!(game.tuned_gears(), fresh(&game), "with part {id}");
        assert!(game.dyno.gears.lock().unwrap().as_ref().is_some_and(|(k, _)| k.1.contains(&id)), "part {id}: kept for it");
        game.profile.as_mut().unwrap().perf.remove(&id);
        assert_eq!(game.tuned_gears(), stock, "part {id} taken off");
    }
    game.profile.as_mut().unwrap().car = "PEUGOT".into();
    assert_eq!(game.tuned_gears(), fresh(&game), "another car");
}
