use crate::*;

/// `--flow engine`: the player's engine is heard (a player's "no engine
/// sound" report) in free roam with a fresh career, with the
/// fixture (tests/fixtures/native_saves/career_complete.sav: a finished
/// career, the Lancer) and in a Quick Race: on an offline mixer, 2 s at
/// idle then 3 s with the throttle held, music and speech off. Each must
/// have the engine's "CAR" event playing at a level above silence (p4 >
/// −32767), its GIN voice on the throttle and the output's RMS above
/// 0.002 (≈ −54 dBFS), at idle as on the throttle. Measured: fresh
/// 0.047 / 0.096; finished career 0.008 / 0.016 (its own options: master 0.5,
/// engine 0.65 → level 0 3735 against 18236, as the original's verified
/// option curves give).
pub(crate) fn flow_engine(game: &mut Game) -> Result<()> {
    fn measure(game: &mut Game, what: &str) -> Result<()> {
        game.settings.game_music = 0.0;
        game.settings.menu_music = 0.0;
        game.settings.speech = 0.0;
        let rms = |game: &mut Game, secs: f32, throttle: bool| -> Result<(f64, i32, u8)> {
            if throttle {
                game.platform.keys.insert(KeyCode::ArrowUp);
            }
            let start = AUDIO_CAPTURE.lock().unwrap().as_ref().map_or(0, |c| c.0.len());
            let (mut p4, mut byte) = (-0x7FFF, 0u8);
            for _ in 0..(secs * 60.0) as u32 {
                game.advance_for(1.0 / 60.0)?;
                if let Some(a) = game.audio.as_ref()
                    && let Some((_, h, min)) = a.engine.as_ref()
                {
                    let level0 = a.levels.level(0, 0);
                    p4 = p4.max(h.abk_block_fx(level0, a.reverb.out[2], a.reverb.out[3])[4]);
                    byte = byte.max(h.voices(level0, *min).1[0]);
                }
            }
            game.platform.keys.remove(&KeyCode::ArrowUp);
            let c = AUDIO_CAPTURE.lock().unwrap();
            let s = c.as_ref().map_or(&[][..], |c| &c.0[start..]);
            let rms = (s.iter().map(|&x| f64::from(x) * f64::from(x)).sum::<f64>() / s.len().max(1) as f64).sqrt();
            Ok((rms, p4, byte))
        };
        // Past the start (a scene, a countdown) and the first second.
        rms(game, 4.0, false)?;
        let idle = rms(game, 2.0, false)?;
        let gas = rms(game, 3.0, true)?;
        let car_event = game.audio.as_ref().is_some_and(|a| a.car_sfx.engine_playing());
        let car = game.sim.drive.as_ref().map(|d| d.car.car_type.name.clone()).unwrap_or_default();
        let entry = game.audio.as_ref().and_then(|a| a.engine.as_ref()).map(|e| e.0);
        let st = &game.settings;
        println!(
            "engine {what}: volumes master {} engine {} effects {} tires {}; level0 {:?}",
            st.master,
            st.engine,
            st.effects,
            st.tires,
            game.audio.as_ref().map(|a| (a.levels.level(0, 0), a.engine.as_ref().map(|e| e.1.out)))
        );
        println!(
            "engine {what}: {car}, entry {entry:?}, CAR event {car_event}; idle rms {:.4} p4 {}; throttle rms {:.4} p4 {} GIN byte {}",
            idle.0, idle.1, gas.0, gas.1, gas.2
        );
        anyhow::ensure!(entry.is_some() && car_event, "{what}: the engine's CAR event plays");
        anyhow::ensure!(
            idle.1 > -0x7FFF && gas.1 > -0x7FFF && gas.2 > 0,
            "{what}: the engine's levels are above silence"
        );
        anyhow::ensure!(
            idle.0 > 0.002 && gas.0 > 0.002,
            "{what}: the output is heard (rms {:.4} / {:.4})",
            idle.0,
            gas.0
        );
        Ok(())
    }
    game.audio = Some(game.new_audio(Mixer::offline(audio_capture_rate())));
    *AUDIO_CAPTURE.lock().unwrap() = Some((Vec::new(), 0.0));
    // A fresh career past the intro (the Peugeot), in free roam.
    let mut p = Profile::new("ENGINE");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.explore(None)?;
    measure(game, "fresh career")?;
    // The finished-career save loaded as the player loads it, in free roam
    // (skipped without the private set).
    if let Some(save) = crate::dev::private::read(crate::dev::private::FINISHED_CAREER)? {
        let data = std::env::var_os("XDG_DATA_HOME").context("the flow's data folder")?;
        let name = crate::dev::private::profile_name(&nfsu2::formats::native_save::NativeSave::parse(&save)?);
        let dir = std::path::Path::new(&data).join("NFS Underground 2").join(&name);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(&name), save)?;
        game.mode = game.icon_menu(MenuKind::Main);
        game.mc_act(McAct::Load(name.clone()))?;
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(game.profile.as_ref().is_some_and(|p| p.name.eq_ignore_ascii_case(&name)), "the finished-career profile {name} loaded");
        game.explore(None)?;
        measure(game, "finished career")?;
    } else {
        println!("{}", crate::dev::private::skip_line("--flow engine, the finished-career step"));
    }
    // A Quick Race (the first circuit, the Peugeot).
    game.profile = None;
    game.mode = game.icon_menu(MenuKind::Main);
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some("PEUGOT".into());
    game.start_race(e, true, false)?;
    measure(game, "quick race")?;
    *AUDIO_CAPTURE.lock().unwrap() = None;
    Ok(())
}
