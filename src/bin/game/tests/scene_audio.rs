use crate::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Real assets and production Game::step/Enter routing; offline mixer only.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn shop_pattern_native_envelope_and_skip_reentry_lifetime() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("SHOP_AUDIO_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.engine_sound(0.0);
    let shop = game.game_data.career.shops.iter().position(|s| s.name == "CC_BODYSHOP_1").unwrap();
    assert!(game.start_shop_scene(nfsu2::shop_scene::SceneKind::Enter, shop));
    let fixture =
        &testkit::private("docs/evidence/nis-audio-pattern/natural-physics.bin");
    // Vanilla: one audio update per 1/30 s scene frame (0x483B40 behind
    // the scene's frame cap); frame 35's event crosses on update 35 and
    // the scene's 140 frames end the pattern's 104 quarter steps + idle.
    // Stepped tick by tick (a shop scene runs race ticks, the
    // agreed uncapped deviation) and compared at each of the scene's own
    // audio updates (`audio.updates`, 1/30 s apart on its clock): the
    // 29/30/31-tick updates put zero or two audio updates in a 1/30 s
    // harness step, so the old per-step sample drifted off the capture.
    let mut matched = 0;
    let mut pcm = Vec::new();
    let mut tick = 0;
    for _ in 0..400 {
        game.step_for(1.0 / 60.0).unwrap();
        game.audio.as_ref().unwrap().mixer.pump(800, &mut pcm);
        let Some(run) = game.scenes.shop.as_ref() else { break };
        if run.audio.updates == tick {
            continue;
        }
        tick = run.audio.updates;
        let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
        if tick < 35 {
            assert_eq!((h.ctx.rpm, h.ctx.throttle), (1000.0, 0.0));
        } else {
            let row = &fixture[(tick - 35) * 244..][..244];
            assert_eq!(
                h.ctx.rpm.to_bits(),
                u32::from_le_bytes(row[0xec..0xf0].try_into().unwrap()),
                "native RPM at audio tick {tick}"
            );
            assert_eq!(
                h.ctx.throttle.to_bits(),
                u32::from_le_bytes(row[0xf0..0xf4].try_into().unwrap()),
                "native throttle at audio tick {tick}"
            );
            matched += 1;
        }
    }
    assert!(matched >= 104, "{matched}");
    assert!(pcm.iter().all(|s| s.is_finite()));
    let rms = (pcm.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt();
    assert!(rms > 1e-5, "real offline mix must produce audible samples: {rms}");
    if let Some(path) = std::env::var_os("NFSU2_PATTERN_PCM") {
        let bytes: Vec<u8> = pcm.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(path, bytes).unwrap();
    }
    for fps in [30, 144, 240] {
        game.mode = Mode::Explore;
        // 3 s leave the pattern running; each rate starts from a fresh
        // car context (the scene alone does not reset it).
        game.reset_car_sound_context();
        assert!(game.start_shop_scene(nfsu2::shop_scene::SceneKind::Enter, shop));
        let dt = 1.0 / fps as f32;
        for _ in 0..fps * 3 {
            // The window's own path at that display rate: the ticks
            // its frames hold; compared at the scene's audio update count.
            game.advance(dt).unwrap();
            let Some(run) = game.scenes.shop.as_ref() else { break };
            let tick = run.audio.updates;
            if tick == 0 {
                continue;
            }
            let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
            if tick < 35 {
                assert_eq!((h.ctx.rpm, h.ctx.throttle), (1000.0, 0.0), "{fps} FPS tick {tick}");
            } else {
                let row = &fixture[(tick - 35).min(107) * 244..][..244];
                assert_eq!(
                    h.ctx.rpm.to_bits(),
                    u32::from_le_bytes(row[0xec..0xf0].try_into().unwrap()),
                    "{fps} FPS/native tick{tick}"
                );
                assert_eq!(h.ctx.throttle.to_bits(), u32::from_le_bytes(row[0xf0..0xf4].try_into().unwrap()));
            }
        }
    }
    // Actual Enter path: the agreed immediate skip (docs/playing/native-deviations.md "Skipping scenes").
    for skip_tick in [0, 30, 90] {
        game.mode = Mode::Explore;
        assert!(game.start_shop_scene(nfsu2::shop_scene::SceneKind::Enter, shop));
        for _ in 0..skip_tick {
            game.step_for(1.0 / 60.0).unwrap();
        }
        game.key(KeyCode::Enter, &mut || panic!("unexpected exit")).unwrap();
        assert!(game.scenes.shop.is_none());
        assert!(matches!(game.mode, Mode::Menu(_)));
        game.engine_sound(1.0 / 60.0);
        assert!(!game.audio.as_ref().unwrap().scripted_engine);
        // Re-enter without replacing the mixer/engine asset cache.
        game.mode = Mode::Explore;
        assert!(game.start_shop_scene(nfsu2::shop_scene::SceneKind::Enter, shop));
        for _ in 0..60 {
            game.step_for(1.0 / 60.0).unwrap();
        }
        let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
        assert_eq!((h.ctx.rpm, h.ctx.throttle), (1000.0, 0.0), "no stale pattern after skip {skip_tick}");
    }
    for _ in 0..300 {
        game.step_for(1.0 / 60.0).unwrap();
    }
    assert!(game.scenes.shop.is_none() && matches!(game.mode, Mode::Menu(_)));
    assert!(!game.audio.as_ref().unwrap().scripted_engine, "natural completion tears down scripted state");
    println!(
        "shop audio:{matched} native context states,30/144/240 FPS,3 actual Enter skips/fresh reentries,natural completion; offline RMS={rms:.6}"
    );
}

/// Diagnostic only: real input/tank path; logs intermittent CPU flame gaps.
#[test]
#[ignore = "diagnostic: logs flame gaps, asserts nothing native"]
fn player_nitro_car_frame_coverage() {
    for name in ["COROLLA", "HUMMER", "LANCEREVO8", "MUSTANGGT", "RX7", "CORSA", "GTO", "SKYLINE", "SUPRA"] {
        for (label, dts) in [("10hz", vec![0.1]), ("transition", vec![1.0 / 240.0, 1.0 / 30.0, 0.1, 0.1])] {
            let mut game = Game::new().unwrap();
            let mut profile = Profile::new("NITRO_COVERAGE");
            profile.skip_intro(&game.game_data.career, &game.game_data.root, name, &mut game.rng);
            profile.perf.extend(
                game.game_data.career.packages.iter().filter(|p| p.category == 2).flat_map(|p| p.parts.iter().map(|x| x.id)),
            );
            game.profile = Some(profile);
            game.explore(None).unwrap();
            game.settings.custom[10] = 1;
            // Controlled flat course avoids the garage wall consuming the
            // tank in purge mode; original car/handling/effect assets remain.
            game.sim.encounters = None;
            let d = game.sim.drive.as_mut().unwrap();
            let emitters = d.world.emitters.clone();
            d.world = World::flat(10000.0);
            d.world.emitters = emitters;
            d.place(Vec3::ZERO, 0.0);
            game.platform.keys.insert(KeyCode::KeyW);
            let (mut frame, mut elapsed, mut active, mut empty, mut absent) = (0, 0.0, 0, 0, 0);
            while elapsed < 20.0 {
                // Three ordinary press/release periods; never refill the tank.
                if elapsed >= 3.0 && !(4.5..4.8).contains(&elapsed) && !(6.0..6.3).contains(&elapsed) {
                    game.platform.keys.insert(KeyCode::AltLeft);
                } else {
                    game.platform.keys.remove(&KeyCode::AltLeft);
                }
                let dt = dts[frame % dts.len()];
                // The window's own path at that display rate: the ticks
            // its frames hold.
            game.advance(dt).unwrap();
                let d = game.sim.drive.as_ref().unwrap();
                let stage = d.state.drivetrain.as_ref().unwrap().nitrous;
                if stage != 0 && !nfsu2::audio::carfx::purging(stage, d.state.speed(), d.state.gas()) {
                    active += 1;
                    let mut fx = d.effects.system.clone();
                    fx.particles.retain(|p| p.key == FLAME_KEY);
                    let axes = CameraAxes::look(d.camera.forward(), d.camera.up);
                    let q = fx.quads(
                        &d.world.emitters,
                        d.camera.position,
                        axes.right,
                        -axes.down,
                        axes.forward,
                        nfsu2::render::cull_scale(d.camera.game_fov.unwrap_or(d.camera.fov_y)),
                    );
                    empty += usize::from(fx.particles.is_empty());
                    absent += usize::from(q.is_empty());
                    if q.is_empty() {
                        eprintln!(
                            "GAP car={name} rate={label} frame={frame} t={elapsed:.6} dt={dt:.6} stage={stage} speed={} particles={} tank={:?}",
                            d.state.speed(),
                            fx.particles.len(),
                            d.state.nitrous
                        );
                    }
                }
                elapsed += dt;
                frame += 1;
            }
            eprintln!(
                "CASE car={name} rate={label} frames={frame} active={active} empty={empty} absent_quads={absent} markers={} tank={:?}",
                game.sim.drive.as_ref().unwrap().car.exhausts.len(),
                game.sim.drive.as_ref().unwrap().state.nitrous
            );
            assert!(active > 0, "{name} {label}: no ordinary NOS activation");
        }
    }
}

#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn shop_exit_pattern_and_shared_context_lifetime() {
    testkit::game();
    use nfsu2::shop_scene::SceneKind;
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("SHOP_EXIT_AUDIO_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.engine_sound(0.0);
    let shop = game.game_data.career.shops.iter().position(|s| s.name == "CC_BODYSHOP_1").unwrap();
    assert!(game.start_shop_scene(SceneKind::Exit, shop));
    let fixture =
        &testkit::private("docs/evidence/nis-audio-pattern4/pattern4-physics.bin");
    // One audio update per 1/30 s scene frame; the 120 frames hold 120
    // of the pattern's updates, the last in the frame whose clock passed
    // the length (as under Wine, `scene_audio_tail`).
    // Tick by tick, compared at each of the scene's audio updates
    // (see `shop_pattern_native_envelope_and_skip_reentry_lifetime`).
    let check = |game: &Game, tick: usize| {
        let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
        let row = &fixture[tick * 244..][..244];
        assert_eq!(
            h.ctx.rpm.to_bits(),
            u32::from_le_bytes(row[0xec..0xf0].try_into().unwrap()),
            "native exit RPM tick {}",
            tick + 1
        );
        assert_eq!(h.ctx.throttle.to_bits(), u32::from_le_bytes(row[0xf0..0xf4].try_into().unwrap()));
    };
    let mut seen = 0;
    for _ in 0..400 {
        game.step_for(1.0 / 60.0).unwrap();
        let Some(run) = game.scenes.shop.as_ref() else {
            // The last update ran in the tick that ended the scene.
            check(&game, seen);
            seen += 1;
            break;
        };
        if run.audio.updates == seen {
            continue;
        }
        seen = run.audio.updates;
        check(&game, seen - 1);
    }
    assert_eq!(seen, 120, "the exit scene's audio updates");
    // Immediate Exit skip is an adaptation: leave a nonterminal pattern
    // and verify the surviving context can resume it on subsequent entry.
    assert!(game.start_shop_scene(SceneKind::Exit, shop));
    for _ in 0..30 {
        game.step_for(1.0 / 60.0).unwrap();
    }
    let saved = game.audio.as_ref().unwrap().scripted_pattern.clone();
    let asset_entry = game.audio.as_ref().unwrap().engine.as_ref().unwrap().0;
    assert!(saved.rpm > 1000.0);
    game.key(KeyCode::Enter, &mut || panic!("unexpected exit")).unwrap();
    assert!(game.scenes.shop.is_none() && matches!(game.mode, Mode::Explore));
    for _ in 0..10 {
        game.step_for(1.0 / 60.0).unwrap();
    }
    assert_eq!(game.audio.as_ref().unwrap().scripted_pattern.rpm, saved.rpm);
    let held = {
        let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
        (h.ctx.rpm.to_bits(), h.ctx.throttle.to_bits(), h.phys.gear)
    };
    game.key(KeyCode::Escape, &mut || panic!("unexpected exit")).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))));
    for _ in 0..10 {
        game.step_for(1.0 / 60.0).unwrap();
    }
    assert!(game.audio.as_ref().unwrap().scripted_engine);
    assert_eq!(game.audio.as_ref().unwrap().scripted_pattern.rpm, saved.rpm);
    let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
    assert_eq!((h.ctx.rpm.to_bits(), h.ctx.throttle.to_bits(), h.phys.gear), held, "pause keeps sound context");
    game.key(KeyCode::Escape, &mut || panic!("unexpected exit")).unwrap();
    assert!(matches!(game.mode, Mode::Explore));
    let retained_gear = game.audio.as_ref().unwrap().engine.as_ref().unwrap().1.phys.gear;
    game.shop.visit = Some(game.game_data.career.shops[shop].hash);
    assert!(game.start_shop_scene(SceneKind::Enter, shop));
    let mut expected = saved;
    for tick in 1..35 {
        expected.step();
        game.step_for(1.0 / 30.0).unwrap();
        let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
        assert_eq!(h.ctx.rpm.to_bits(), expected.rpm.to_bits(), "retained pre-event pattern at {tick}");
        assert_eq!(h.phys.gear, retained_gear, "scripted updates retain ordinary-context gear");
    }
    game.step_for(1.0 / 30.0).unwrap();
    assert_eq!(game.audio.as_ref().unwrap().engine.as_ref().unwrap().1.ctx.rpm, 1282.5);
    // Enter completion destroys the native context. Exercise the actual
    // key route and rebuild through the production shop-exit path.
    game.key(KeyCode::Enter, &mut || panic!("unexpected exit")).unwrap();
    game.engine_sound(1.0 / 60.0);
    assert!(matches!(game.mode, Mode::Menu(_)));
    assert!(!game.audio.as_ref().unwrap().scripted_engine);
    assert_eq!(game.audio.as_ref().unwrap().scripted_pattern.rpm, 1000.0);
    assert_eq!(game.audio.as_ref().unwrap().engine.as_ref().unwrap().0, asset_entry, "FE retained asset identity");
    game.explore(None).unwrap();
    assert_eq!(game.scenes.shop.as_ref().unwrap().scene.kind, SceneKind::Exit);
    // Ticks to the exit scene's first audio update (1/30 s of race
    // ticks can hold none, 29 + 30 of 1/1800 s).
    for _ in 0..4 {
        game.step_for(1.0 / 60.0).unwrap();
        if game.scenes.shop.as_ref().is_some_and(|r| r.audio.updates > 0) {
            break;
        }
    }
    assert_eq!(game.audio.as_ref().unwrap().engine.as_ref().unwrap().1.ctx.rpm, 3689.25);
    for _ in 0..29 {
        game.step_for(1.0 / 60.0).unwrap();
    }
    assert!(game.audio.as_ref().unwrap().scripted_pattern.rpm > 1000.0);
    game.explore(None).unwrap(); // new car context, same engine entry/cache
    assert!(game.scenes.shop.is_none());
    assert!(!game.audio.as_ref().unwrap().scripted_engine);
    assert_eq!(game.audio.as_ref().unwrap().scripted_pattern.rpm, 1000.0);
    assert_eq!(game.audio.as_ref().unwrap().engine.as_ref().unwrap().0, asset_entry);
    game.engine_sound(1.0 / 60.0);
    let ordinary = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
    assert_ne!(ordinary.phys.gear, 0, "ordinary history initialized");
    assert!(!game.audio.as_ref().unwrap().scripted_engine);
    game.explore(None).unwrap(); // ordinary-only context is destroyed too
    let rebuilt = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
    let fresh = nfsu2::audio::hybrid::HybridMotor::new(asset_entry);
    assert_eq!(rebuilt.phys.gear, fresh.phys.gear);
    assert_eq!(rebuilt.ctx.rpm, fresh.ctx.rpm);
    assert_eq!(game.audio.as_ref().unwrap().engine.as_ref().unwrap().0, asset_entry);
}

/// Exercise the same Game::step entry point used by live redraws, with real
/// key/tank activation. Requires assets; no renderer/window is constructed.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn player_nitro_live_step_variable_frames() {
    testkit::game();
    for (label, steps) in [
        ("30hz", vec![1.0 / 30.0]),
        ("60hz", vec![1.0 / 60.0]),
        ("144hz", vec![1.0 / 144.0]),
        ("variable", vec![1.0 / 144.0, 1.0 / 30.0, 0.1, 1.0 / 60.0]),
    ] {
        let mut game = Game::new().unwrap();
        let mut profile = Profile::new("PLAYER_FLAME_TEST");
        profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        profile.perf.extend(
            game.game_data.career
                .packages
                .iter()
                .filter(|p| p.category == 2 && p.level == 1)
                .flat_map(|p| p.parts.iter().map(|x| x.id)),
        );
        game.profile = Some(profile);
        game.explore(None).unwrap();
        game.settings.custom[10] = 1;
        game.platform.keys.insert(KeyCode::KeyW);
        let (mut elapsed, mut frame, mut active, mut no_particles, mut no_quads) = (0.0, 0, 0, 0, 0);
        while elapsed < 3.0 {
            if elapsed >= 1.0 {
                game.platform.keys.insert(KeyCode::AltLeft);
            }
            let dt = steps[frame % steps.len()];
            // The window's own path at that display rate: the ticks
            // its frames hold.
            game.advance(dt).unwrap();
            let d = game.sim.drive.as_ref().unwrap();
            let stage = d.state.drivetrain.as_ref().map_or(0, |t| t.nitrous);
            if stage != 0 && !nfsu2::audio::carfx::purging(stage, d.state.speed(), d.state.gas()) {
                active += 1;
                let mut flames = d.effects.system.clone();
                flames.particles.retain(|p| p.key == FLAME_KEY);
                no_particles += usize::from(flames.particles.is_empty());
                let axes = CameraAxes::look(d.camera.forward(), d.camera.up);
                let quads = flames.quads(
                    &d.world.emitters,
                    d.camera.position,
                    axes.right,
                    -axes.down,
                    axes.forward,
                    nfsu2::render::cull_scale(d.camera.game_fov.unwrap_or(d.camera.fov_y)),
                );
                no_quads += usize::from(quads.is_empty());
            }
            elapsed += dt;
            frame += 1;
        }
        let d = game.sim.drive.as_ref().unwrap();
        eprintln!(
            "player nitro {label}: frames {frame}, active {active}, no_particles {no_particles}, no_quads {no_quads}, speed_kmh {:.2}, tank {:?}",
            d.state.speed() * 3.6,
            d.state.nitrous
        );
        assert!(active > 0, "{label}: key never activated nitro");
        assert_eq!(no_particles, 0, "{label}: live step lost player's flame particles");
        assert_eq!(no_quads, 0, "{label}: active frame lost flame quads");
    }
}

/// A race start scene's CAR_AUDIO_PATTERN events drive every racer's
/// sound context (0x47C490) for the whole scene and the player's
/// (0x484C50) up to the camera cut, at one audio update per 1/30 s
/// scene frame (60 a second after the cut); slot n is the racer on grid
/// slot n. The reference is the native rule the Wine captures follow
/// (`docs/evidence/nis-audio-patterns`). Real assets, no GPU.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_start_scene_patterns_drive_racers_and_player() {
    testkit::game();
    use nfsu2::audio::scripted::{FREE_PERIOD, Pattern, SCENE_PERIOD};
    let mut game = Game::new().unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.settings.custom[5] = 1;
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let event = game.quick_event(mode);
    game.quick.car = Some("350Z".into());
    game.start_race(event, true, false).unwrap();
    let run = game.scenes.shop.as_ref().expect("a start scene");
    assert!(matches!(run.scene.kind, nfsu2::shop_scene::SceneKind::Intro(_)));
    let (events, cut, length) =
        (run.scene.audio_events.clone(), run.scene.marks[1] as f32 / 30.0, run.scene.length / 30.0);
    // The scene's frame limiter as it starts (the world's carry).
    let mut frames = run.audio.frames;
    let Mode::Race(race) = &game.mode else { panic!("race") };
    let player = race.grid_slot + 1;
    let rivals: Vec<usize> = race.racers.rivals.iter().map(|r| r.slot + 1).collect();
    // The reference: the native frames' ticks summed in f32
    // (`shop_scene::audio::native_update`), strict crossing, slot's context.
    let (mut clock, mut cursor, mut seconds) = (0.0f32, 0usize, 0.0f64);
    let mut sorted = events.clone();
    sorted.sort_by(|a, b| a.frame.total_cmp(&b.frame));
    let (mut mine, mut theirs) = (Pattern::default(), vec![Pattern::default(); rivals.len()]);
    let (mut player_updates, mut racer_updates, mut started) = (0, 0, 0);
    // One tick per pass (1/30 s while the scene holds the cars,
    // 1/60 s after the cut), the reference stepped and compared at each of
    // the scene's own audio updates (`audio.updates`): a tick can hold
    // none or two of them.
    let mut seen = game.scenes.shop.as_ref().unwrap().audio.updates;
    while game.scenes.shop.is_some() && seconds < f64::from(length) - 0.05 {
        let capped = game.scenes.shop.as_ref().unwrap().holds_cars();
        let period = if capped { SCENE_PERIOD } else { FREE_PERIOD };
        // One tick (a 1/30 s one while the scene holds the cars).
        game.step_for(period).unwrap();
        if game.scenes.shop.is_none() {
            break; // the scene ended within this frame: racing contexts
        }
        let holds = game.scenes.shop.as_ref().is_some_and(|r| r.holds_cars());
        seconds += f64::from(period);
        let now = game.scenes.shop.as_ref().unwrap().audio.updates;
        let n = now - seen;
        seen = now;
        for i in 0..n {
            let last = i + 1 == n;
            clock = nfsu2::shop_scene::audio::native_update(&mut frames, clock, period);
            while sorted.get(cursor).is_some_and(|e| e.frame * 0.03333333507180214_f32 < clock) {
                let e = sorted[cursor];
                cursor += 1;
                if e.slot == player && holds {
                    mine.start_id(e.pattern);
                    started += 1;
                }
                for (p, _) in theirs.iter_mut().zip(&rivals).filter(|(_, s)| **s == e.slot) {
                    p.start_id(e.pattern);
                    started += 1;
                }
            }
            if holds {
                mine.step();
                if last {
                    let h = &game.audio.as_ref().unwrap().engine.as_ref().unwrap().1;
                    assert_eq!(
                        (h.ctx.rpm.to_bits(), h.ctx.throttle.to_bits()),
                        (mine.rpm.to_bits(), mine.throttle.to_bits()),
                        "player at {clock}"
                    );
                }
                player_updates += 1;
            }
            let (_, (ai, _, _, _)) = game.world_sfx_input().unwrap();
            for (k, p) in theirs.iter_mut().enumerate() {
                p.step();
                if last {
                    assert_eq!(
                        (ai[k].ctx_rpm.to_bits(), ai[k].throttle.to_bits()),
                        (p.rpm.to_bits(), (p.throttle / 100.0).to_bits()),
                        "racer {k} at {clock}"
                    );
                }
            }
            racer_updates += 1;
        }
    }
    assert!(player_updates as f32 >= cut * 30.0 - 1.0, "{player_updates} player updates before the cut at {cut} s");
    assert!(racer_updates > player_updates && started >= 4, "{racer_updates} racer updates, {started} patterns");
    eprintln!(
        "start scene: player slot {player}, racers {rivals:?}, {player_updates} / {racer_updates} updates, {started} patterns started"
    );
}

/// A race's ending scene scripts the player's context behind the
/// post-race screens: on a win the player is slot 1 and takes the
/// scene's pattern (EndingNis02: 14), replayed against the 300 native
/// updates captured under Wine; on a loss it is slot 2, scripted, idle.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn ending_pattern_drives_the_winning_player_behind_the_results() {
    testkit::game();
    use nfsu2::shop_scene::{IntroTable, SceneKind};
    let native: Vec<(u32, u32)> =
        testkit::private_str("docs/evidence/nis-audio-patterns/native-updates.txt")
            .split("scene EndingNis02")
            .nth(1)
            .unwrap()
            .lines()
            .skip(3)
            .take(300)
            .map(|l| {
                let w: Vec<_> = l.split_whitespace().collect();
                (u32::from_str_radix(w[1], 16).unwrap(), u32::from_str_radix(w[2], 16).unwrap())
            })
            .collect();
    assert_eq!(native.len(), 300);
    for won in [true, false] {
        let mut game = Game::new().unwrap();
        game.audio = Some(game.new_audio(Mixer::offline(48000)));
        game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
        let event = game.quick_event(screens::QrMode::Circuit);
        game.quick.car = Some("240SX".into());
        game.start_race(event, true, false).unwrap();
        let winner = game.sim.drive.as_ref().unwrap().car.car_type.handling.clone();
        let table = IntroTable::load(&game.game_data.root).unwrap();
        let seed = (0..1000)
            .find(|seed| {
                let mut rng = nfsu2::opponents::GameRng(*seed);
                table
                    .pick_ending(4014, &winner, true, won, &mut rng)
                    .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(2))
            })
            .unwrap();
        game.rng = nfsu2::opponents::GameRng(seed);
        game.play_race_ending(4014, Vec3::ZERO, 0.0, &winner, won, None);
        game.keep_replay(Mode::Menu(Menu::Options(options::OptionsScreen::new(
            options::Category::Display,
            &game.settings,
            false,
            0.0,
        ))));
        let run = game.scenes.shop.as_ref().unwrap();
        assert_eq!((run.scene.kind, run.car_slot), (SceneKind::Ending(2), if won { 1 } else { 2 }));
        for (i, &(rpm, throttle)) in native.iter().enumerate() {
            game.step_for(1.0 / 30.0).unwrap();
            assert!(game.scenes.shop.is_some());
            let a = game.audio.as_ref().unwrap();
            assert!(a.scripted_engine);
            let h = &a.engine.as_ref().unwrap().1;
            if won {
                assert_eq!((h.ctx.rpm.to_bits(), h.ctx.throttle.to_bits()), (rpm, throttle), "update {}", i + 1);
            } else {
                assert_eq!((h.ctx.rpm, h.ctx.throttle), (1000.0, 0.0), "update {}", i + 1);
            }
        }
    }
}

/// CareerNis1 has no audio events, yet the player's context is scripted
/// through it (observed under Wine 2026-11-04: 403 updates, 1000 rpm /
/// 0 throttle, `docs/evidence/s3-audio`), not the ordinary engine.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn career_scene_scripts_the_player_idle() {
    testkit::game();
    let native: Vec<(u32, u32)> =
        testkit::private_str("docs/evidence/nis-audio-patterns/native-updates.txt")
            .split("scene CareerNis1")
            .nth(1)
            .unwrap()
            .lines()
            .skip(2)
            .filter(|l| !l.is_empty())
            .map(|l| {
                let w: Vec<_> = l.split_whitespace().collect();
                (u32::from_str_radix(w[1], 16).unwrap(), u32::from_str_radix(w[2], 16).unwrap())
            })
            .collect();
    assert_eq!(native.len(), 403);
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("CAREER_NIS_AUDIO");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.engine_sound(0.0);
    game.scenes.shop = None;
    assert!(game.start_career_scene());
    for (i, want) in native.iter().take(390).enumerate() {
        game.step_for(1.0 / 30.0).unwrap();
        assert!(game.scenes.shop.is_some(), "update {}", i + 1);
        let a = game.audio.as_ref().unwrap();
        assert!(a.scripted_engine);
        let h = &a.engine.as_ref().unwrap().1;
        assert_eq!((h.ctx.rpm.to_bits(), h.ctx.throttle.to_bits()), *want, "update {}", i + 1);
    }
}

/// A won ending's hold (EndingNis01, observed under Wine 2026-11-06,
/// the maintainer's private captures): the player's wind, road and
/// skid objects keep running behind the post-race screens through the
/// scene's 550 frames, FX_SKID's slip levels rise in the drift and the
/// wind's pitch follows the car, and both stop at the scene's end. Real
/// assets, no GPU.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn won_ending_keeps_the_player_car_sounds_until_the_scene_ends() {
    testkit::game();
    use nfsu2::shop_scene::{IntroTable, SceneKind};
    let mut game = Game::new().unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let event = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some("RX7".into());
    game.start_race(event, true, false).unwrap();
    let winner = game.sim.drive.as_ref().unwrap().car.car_type.handling.clone();
    let table = IntroTable::load(&game.game_data.root).unwrap();
    let Some(seed) = (0..2000).find(|seed| {
        let mut rng = nfsu2::opponents::GameRng(*seed);
        table
            .pick_ending(4014, &winner, true, true, &mut rng)
            .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(1))
    }) else {
        panic!("no EndingNis01 pick for the RX-7");
    };
    game.rng = nfsu2::opponents::GameRng(seed);
    // On the circuit's start line, as the race's finish gives it.
    let (at, heading) = match &game.mode {
        Mode::Race(r) => r.route.start_line.zip(r.route.start_heading).expect("the start line"),
        _ => panic!("race"),
    };
    game.play_race_ending(4014, at, heading, &winner, true, None);
    game.keep_replay(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Display,
        &game.settings,
        false,
        0.0,
    ))));
    let run = game.scenes.shop.as_ref().unwrap();
    assert_eq!((run.scene.kind, run.scene.length), (SceneKind::Ending(1), 550.0));
    let (mut skid, mut wind_frames, mut top) = (0, 0, 0.0f32);
    for i in 0..600 {
        nfsu2::audio::aems::log::set_log_time(f64::from(i) / 30.0);
        game.step_for(1.0 / 30.0).unwrap();
        let frame = game.scenes.shop.as_ref().unwrap().frame;
        let a = game.audio.as_ref().unwrap();
        if std::env::var_os("NFSU2_TEST_PROBE").is_some() && i % 30 == 0 {
            let d = game.sim.drive.as_ref().unwrap();
            eprintln!(
                "frame {frame} cam {:.1} m speed {:.2} skid {} levels 5: {:?} 7: {:?}",
                d.camera.position.distance(d.state.position),
                d.state.speed(),
                a.car_sfx.skid_level(),
                (0..3).map(|s| a.levels.level(5, s)).collect::<Vec<_>>(),
                (0..2).map(|s| a.levels.level(7, s)).collect::<Vec<_>>()
            );
        }
        if frame < 549.0 {
            assert!(a.env.wind_on(), "frame {i}: the wind runs behind the results");
            wind_frames += 1;
            skid = skid.max(a.car_sfx.skid_level());
            top = top.max(game.sim.drive.as_ref().unwrap().state.speed());
        } else if i > 552 {
            assert!(!a.env.wind_on() && a.car_sfx.skid_level() == 0, "frame {i}: silent after the scene");
        }
    }
    assert!(wind_frames > 500);
    assert!(skid > 0, "the drift's slip is heard (native FX_SKID p4 / p5 up to 409)");
    eprintln!("won ending: {wind_frames} wind frames, skid level up to {skid}, top speed {top:.2} m/s");
    assert!(top > 2.0, "the scene drives the car ({top} m/s)");
}

/// Road noise and wind read car +0x430, the body's |velocity| (Wine,
/// EndingNis03's slide on the NIS mover read +0x42C 4.95 and
/// +0x430 5.46 = |body +0xA0|), not the forward speed: a car sliding
/// sideways keeps its wind pitch.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn road_and_wind_read_the_cars_whole_speed() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.explore(None).unwrap();
    let d = game.sim.drive.as_mut().unwrap();
    let side = d.state.orientation * Vec3::Y;
    d.state.velocity = side * 5.0 + d.state.forward() * 2.0;
    let (env, _) = game.world_sfx_input().unwrap();
    assert!((env.speed - 29.0f32.sqrt()).abs() < 1e-4, "{}", env.speed);
}

/// An ending lost to an AI (EndingNis01, observed under Wine 2026-11-04,
/// `docs/evidence/s3-audio`): every racer's context idles scripted at
/// 1000 rpm / 0 throttle — the winner's frame-0 pattern 13 reaches its
/// context before it is scripted and is lost — and the racers' engines
/// keep sounding behind the post-race screens. Real assets, no GPU.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn ending_lost_to_an_ai_idles_and_voices_the_racers() {
    testkit::game();
    use nfsu2::shop_scene::{IntroTable, SceneKind};
    let native: Vec<Vec<(u32, u32)>> = {
        let updates = testkit::private_str("docs/evidence/nis-audio-patterns/native-updates.txt");
        let text =
            updates
                .split("scene EndingNis01")
                .nth(1)
                .unwrap()
                .split("scene ")
                .next()
                .unwrap();
        text.split("context ")
            .skip(1)
            .map(|c| {
                c.lines()
                    .skip(1)
                    .filter(|l| !l.is_empty())
                    .map(|l| {
                        let w: Vec<_> = l.split_whitespace().collect();
                        (u32::from_str_radix(w[1], 16).unwrap(), u32::from_str_radix(w[2], 16).unwrap())
                    })
                    .collect()
            })
            .collect()
    };
    assert_eq!(native.iter().map(Vec::len).collect::<Vec<_>>(), [512, 513, 512, 512]);
    let mut game = Game::new().unwrap();
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let event = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some("RX7".into());
    game.start_race(event, true, false).unwrap();
    let table = IntroTable::load(&game.game_data.root).unwrap();
    // A rival the native pick can give EndingNis01 for (its rear split).
    let pick = {
        let Mode::Race(race) = &game.mode else { panic!("race") };
        race.racers.rivals.iter().enumerate().find_map(|(k, r)| {
            let h = &r.car.car_type.handling;
            (0..2000)
                .find(|seed| {
                    let mut rng = nfsu2::opponents::GameRng(*seed);
                    table
                        .pick_ending(4014, h, true, false, &mut rng)
                        .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(1))
                })
                .map(|seed| (k, seed, SceneCar { rival: k, ..SceneCar::from_rival(r) }, h.clone()))
        })
    };
    let Some((k, seed, companion, winner)) = pick else {
        eprintln!("no RWD rival in this line-up");
        return;
    };
    game.rng = nfsu2::opponents::GameRng(seed);
    game.play_race_ending(4014, Vec3::ZERO, 0.0, &winner, false, Some(companion));
    game.keep_replay(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Display,
        &game.settings,
        false,
        0.0,
    ))));
    let run = game.scenes.shop.as_ref().unwrap();
    assert_eq!((run.scene.kind, run.car_slot), (SceneKind::Ending(1), 2));
    for i in 0..300 {
        game.step_for(1.0 / 30.0).unwrap();
        let a = game.audio.as_ref().unwrap();
        let h = &a.engine.as_ref().unwrap().1;
        assert_eq!((h.ctx.rpm.to_bits(), h.ctx.throttle.to_bits()), native[1][i], "player update {}", i + 1);
        let p = &a.ai_patterns[k];
        if i > 0 {
            assert_eq!((p.rpm.to_bits(), p.throttle.to_bits()), native[0][i - 1], "winner update {}", i + 1);
        }
        assert!(a.others.ai_voices() >= 1, "the winner's engine plays behind the results");
    }
}

/// The title screen's song (UG_LS_Splash, 0x4A90F0 → 0x480C30(−2) →
/// 0x47AC50): the boot's movies, then the title plays song 0, Riders on the
/// Storm, in the default Random mode whatever the boot seed (0x57ECC0);
/// observed under Wine on every cold start, with no profile, a fresh one
/// and an existing one (the profile is applied after the title). The
/// attract movie and the title again start it over.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_title_plays_song_0_for_any_boot_seed() {
    testkit::game();
    for ticks in [0u64, 7, 0x8284_E4, 987_654_321] {
        let mut game = Game::new().unwrap();
        game.rng = nfsu2::opponents::GameRng::boot(ticks);
        assert!(game.settings.trax_random, "the profile reset's play mode is Random");
        game.audio = Some(game.new_audio(Mixer::offline(48000)));
        let mut pcm = Vec::new();
        let mut play = |game: &mut Game| {
            for _ in 0..10 {
                game.step_for(1.0 / 60.0).unwrap();
                game.audio.as_ref().unwrap().mixer.pump(800, &mut pcm);
            }
        };
        game.mode = game.boot_screen(crate::flow::movies::BootStep::Intro);
        play(&mut game);
        assert_eq!(game.audio.as_ref().unwrap().streams.song(), None, "no song over the boot movie");
        game.mode = game.boot_screen(crate::flow::movies::BootStep::Splash);
        play(&mut game);
        assert_eq!(game.audio.as_ref().unwrap().streams.song(), Some(0), "boot ticks {ticks}");
        // Over the attract movie and back on the title: song 0 again.
        game.mode = game.boot_screen(crate::flow::movies::BootStep::Intro);
        play(&mut game);
        game.mode = game.boot_screen(crate::flow::movies::BootStep::Splash);
        play(&mut game);
        assert_eq!(game.audio.as_ref().unwrap().streams.song(), Some(0), "the title again, boot ticks {ticks}");
    }
}

/// A profile with its own EA Trax list (song 0 Off, song 0 Racing, and the
/// default list): the boot plays with the profile reset's list (table
/// order, songs 0–5 Menu, Random; 0x515AA0 at start-up), so the title still
/// plays song 0 for any boot seed; the profile's list takes effect only
/// when the profile is applied after the title (0x5344E0 → 0x479220,
/// observed at "Load complete." → Continue): the masks follow the
/// profile's list, both pools refill to them, the last positions go back
/// and the title's song goes on.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_custom_trax_list_applies_after_the_title() {
    use nfsu2::frontend::{options::Settings, profiles};
    testkit::game();
    for song0 in [0u8, 2, 1] {
        for ticks in [0u64, 7, 0x8284_E4, 987_654_321, 0xFFFF_FFFF] {
            // A new process: no profile current yet (the thread's sandbox
            // keeps the last iteration's).
            profiles::clear_current();
            let mut game = Game::new().unwrap();
            // The profile on disk, loaded by the boot after its options
            // (`Game::new`'s order: `Settings::load`, then `profiles::load`).
            let name = "TRAX_LIST";
            let mut custom = Settings::default();
            custom.trax[0] = song0;
            let mut save = nfsu2::frontend::career::fresh_save(name);
            custom.write_native(&mut save);
            profiles::write(name, &save).unwrap();
            profiles::load(name).unwrap();
            game.fe.boot_profiles = Some(profiles::Boot::One(name.to_string()));
            assert_eq!(game.settings.trax, Settings::default().trax, "the boot's list is the reset's");
            assert!(game.settings.trax_random);
            game.rng = nfsu2::opponents::GameRng::boot(ticks);
            game.audio = Some(game.new_audio(Mixer::offline(48000)));
            let mut pcm = Vec::new();
            let mut play = |game: &mut Game| {
                for _ in 0..10 {
                    game.step_for(1.0 / 60.0).unwrap();
                    game.audio.as_ref().unwrap().mixer.pump(800, &mut pcm);
                }
            };
            game.mode = game.boot_screen(crate::flow::movies::BootStep::Splash);
            play(&mut game);
            let a = game.audio.as_ref().unwrap();
            assert_eq!(a.streams.song(), Some(0), "song 0 state {song0}, boot ticks {ticks}");
            assert_eq!(a.streams.playlist().unwrap().order[0], (0, 1), "the title's list is the reset's");
            // The profile applied: its list in force, the pools full.
            game.mode = game.boot_profile_step();
            play(&mut game);
            assert_eq!(game.settings.trax[0], song0);
            let a = game.audio.as_ref().unwrap();
            let want = nfsu2::audio::music::Playlist::new(&game.settings.trax_playlist(), game.settings.trax_random);
            assert_eq!(a.streams.playlist(), Some(&want), "the pools after the load, song 0 state {song0}");
            assert_eq!(a.streams.song(), Some(0), "the title's song goes on");
        }
    }
}

/// A race's song goes on through its end: "Winner!", the post-race
/// flow's load of the ending scene (state 7 keeps an EA Trax song with
/// the game music on, 0x47AD0F), the ending scene and the post-race
/// screens over it (the game's audio mode). Observed under Wine (Quick
/// Race, a finish written into the race record): the same song from the
/// race through Event Results and Post Race. The clone stopped it at the
/// load and started another on Event Results.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_race_song_plays_on_through_the_results() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.settings.master = 1.0;
    game.settings.game_music = 1.0;
    game.settings.menu_music = 1.0;
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let event = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some("240SX".into());
    game.start_race(event, true, false).unwrap();
    let mut pcm = Vec::new();
    let mut play = |game: &mut Game, secs: f32| {
        for _ in 0..(secs * 60.0) as usize {
            game.step_for(1.0 / 60.0).unwrap();
            game.audio.as_ref().unwrap().mixer.pump(800, &mut pcm);
        }
    };
    // Past the start scene and the countdown: the race's song.
    for _ in 0..40 {
        if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 1.0) {
            break;
        }
        play(&mut game, 0.5);
    }
    assert_eq!(game.music_context(), MusicContext::Game);
    let song = game.audio.as_ref().unwrap().streams.song();
    assert!(song.is_some(), "a song in the race");
    let _ = game.finish_race(crate::flow::race_end::RaceEnd { time: 60.0 });
    assert!(
        game.scenes.shop.as_ref().is_some_and(|r| matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_))),
        "the ending scene behind the results"
    );
    assert!(matches!(game.mode, Mode::Menu(Menu::Results(..))));
    assert_eq!(game.music_context(), MusicContext::Game, "Event Results over the ending scene");
    play(&mut game, 2.0);
    assert_eq!(game.audio.as_ref().unwrap().streams.song(), song, "the same song on Event Results");
    // The screens after it, still in the race's flow.
    while matches!(game.mode, Mode::Menu(Menu::Results(..) | Menu::Reward(_))) {
        game.menu_key(Key::Accept).unwrap();
        play(&mut game, 0.5);
    }
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))));
    assert_eq!(game.audio.as_ref().unwrap().streams.song(), song, "the same song on Post Race");
    let reward = crate::flow::race_end::Reward { reputation: 0, bank: 0, name: String::new(), opened: 0.0 };
    for m in [Menu::SponsorPopup(0.0), Menu::Reward(reward)] {
        game.mode = Mode::Menu(m);
        assert_eq!(game.music_context(), MusicContext::Game, "the post-race chain is in the game's audio mode");
    }
}

/// A race's pause and its options (UI_PauseOptionsMain, UI_PauseOptions on
/// each category) hold the race's song; Resume lets the same song go on.
/// No front-end song starts over the paused race.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_race_pause_options_hold_the_race_song() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.settings.master = 1.0;
    game.settings.game_music = 1.0;
    game.settings.menu_music = 1.0;
    game.audio = Some(game.new_audio(Mixer::offline(48000)));
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let event = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some("240SX".into());
    game.start_race(event, true, false).unwrap();
    let mut pcm = Vec::new();
    let mut play = |game: &mut Game, secs: f32| {
        for _ in 0..(secs * 60.0) as usize {
            game.step_for(1.0 / 60.0).unwrap();
            game.audio.as_ref().unwrap().mixer.pump(800, &mut pcm);
        }
    };
    for _ in 0..40 {
        if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 1.0) {
            break;
        }
        play(&mut game, 0.5);
    }
    assert_eq!(game.music_context(), MusicContext::Game);
    let song = game.audio.as_ref().unwrap().streams.song();
    assert!(song.is_some(), "a song in the race");
    game.key(KeyCode::Escape, &mut || {}).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::RacePause))), "{}", flow_mode(&game.mode));
    play(&mut game, 0.5);
    assert_eq!(game.music_context(), MusicContext::Paused);
    game.act(Action::Options, MenuKind::RacePause).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PauseOptions(true)))), "{}", flow_mode(&game.mode));
    for n in 0..4 {
        game.mode = game.icon_menu(MenuKind::PauseOptions(true));
        game.act(Action::PauseSetting(n), MenuKind::PauseOptions(true)).unwrap();
        assert!(matches!(&game.mode, Mode::Menu(Menu::Options(o)) if o.pause.is_some()), "{n}: {}", flow_mode(&game.mode));
        play(&mut game, 1.0);
        assert_eq!(game.music_context(), MusicContext::Paused, "pause options category {n}");
        assert_eq!(game.audio.as_ref().unwrap().streams.song(), song, "pause options category {n}: no new song");
    }
    // Back out to the pause and resume: the same song goes on.
    game.mode = game.icon_menu(MenuKind::RacePause);
    game.act(Action::ResumeRace, MenuKind::RacePause).unwrap();
    assert!(matches!(game.mode, Mode::Race(_)), "{}", flow_mode(&game.mode));
    play(&mut game, 1.0);
    assert_eq!(game.music_context(), MusicContext::Game);
    assert_eq!(game.audio.as_ref().unwrap().streams.song(), song, "the race's song after Resume");
}
