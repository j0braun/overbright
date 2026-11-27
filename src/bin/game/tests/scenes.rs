use crate::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// After winning the final race against Caleb (S5_CIRCUIT_36) the
/// car's ending scene once sank under the map. The
/// start line there rises ~2 m across the scene; the ending's cars run
/// on their NIS movers and must keep their wheels on the road in every
/// ending scene. Real assets, no GPU; run with isolated XDG homes.
#[test]
#[ignore = "requires original assets; sets NFSU2_ENDING_NIS for the whole process, so run it on its own with --ignored"]
fn final_race_ending_keeps_the_car_on_the_road() {
    for n in 1u8..=4 {
        // SAFETY: ignored test, run on its own.
        unsafe { std::env::set_var("NFSU2_ENDING_NIS", n.to_string()) };
        let mut game = Game::new().unwrap();
        let mut profile = Profile::new("FINAL_ENDING_TEST");
        profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        game.profile = Some(profile);
        let e = game.game_data.career.events.iter().position(|e| e.name == "S5_CIRCUIT_36").expect("the final race");
        game.start_race(e, false, false).unwrap();
        game.scenes.shop = None;
        game.scenes.crowd = None;
        if let Mode::Race(r) = &mut game.mode {
            r.timing.clock = 0.0;
        }
        for _ in 0..90 {
            game.platform.keys.insert(KeyCode::KeyW);
            game.advance_for(1.0 / 30.0).unwrap();
        }
        game.platform.keys.clear();
        if let Mode::Race(r) = &mut game.mode {
            r.racers.me.finished = Some(r.timing.clock);
        }
        let (mut frames, mut lowest) = (0, f32::INFINITY);
        for _ in 0..30 * 20 {
            game.advance_for(1.0 / 30.0).unwrap();
            let Some(run) = game.scenes.shop.as_ref() else { continue };
            if run.scene.kind != nfsu2::shop_scene::SceneKind::Ending(n) {
                continue;
            }
            let d = game.sim.drive.as_ref().unwrap();
            for w in &d.car.car_type.wheels {
                let hub = d.state.position + d.state.orientation * w.position;
                let road = d.world.road_height_below(hub + Vec3::Z * 2.0).expect("road under the scene car");
                lowest = lowest.min(hub.z - road);
            }
            frames += 1;
        }
        eprintln!("EndingNis{n:02}: {frames} frames, lowest wheel centre {lowest:.3} m above the road");
        assert!(frames > 100, "EndingNis{n:02} did not play");
        assert!(lowest > 0.1, "EndingNis{n:02}: a wheel centre {lowest} m from the road");
    }
    unsafe { std::env::remove_var("NFSU2_ENDING_NIS") };
}

/// The post-race burnout (it once had no smoke): EndingNis01's winner spins its
/// rears (brake 0.2, additive 2.5 from frame 0) under the post-race
/// menus, and the original smokes there (Wine, `native-finish-rwd`,
/// docs/notes/graphics/smoke.md "Scene smoke"). The particle step runs in
/// the ending scene and the sliding wheels emit the tyre smoke group.
#[test]
#[ignore = "requires original assets; sets NFSU2_ENDING_NIS for the whole process, so run it on its own with --ignored"]
fn ending_burnout_emits_tyre_smoke() {
    // SAFETY: ignored test, run on its own.
    unsafe { std::env::set_var("NFSU2_ENDING_NIS", "1") };
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ENDING_SMOKE_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    let e = (1..=5)
        .find_map(|s| {
            game.game_data.career.stage_events(&game.game_data.root, s).iter().find(|e| e.kind == career::EventKind::Circuit).map(|e| e.index)
        })
        .unwrap();
    game.start_race(e, false, false).unwrap();
    game.scenes.shop = None;
    game.scenes.crowd = None;
    if let Mode::Race(r) = &mut game.mode {
        r.timing.clock = 0.0;
    }
    for _ in 0..90 {
        game.platform.keys.insert(KeyCode::KeyW);
        game.advance_for(1.0 / 30.0).unwrap();
    }
    game.platform.keys.clear();
    if let Mode::Race(r) = &mut game.mode {
        r.racers.me.finished = Some(r.timing.clock);
    }
    let (mut frames, mut smoke) = (0, 0usize);
    for _ in 0..30 * 6 {
        game.advance_for_each(1.0 / 30.0, |g, dt| {
            g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
            Ok(())
        }).unwrap();
        if !game.scenes.shop.as_ref().is_some_and(|r| r.scene.kind == nfsu2::shop_scene::SceneKind::Ending(1)) {
            continue;
        }
        frames += 1;
        let d = game.sim.drive.as_ref().unwrap();
        let lib = &d.world.emitters;
        smoke = smoke.max(
            d.effects
                .system
                .particles
                .iter()
                .filter(|p| {
                    lib.descriptions
                        .get(p.description as usize)
                        .is_some_and(|d| d.name.starts_with("XETT_") && d.name.contains("Smk"))
                })
                .count(),
        );
    }
    unsafe { std::env::remove_var("NFSU2_ENDING_NIS") };
    eprintln!("EndingNis01: {frames} frames, at most {smoke} tyre smoke particles");
    assert!(matches!(game.mode, Mode::Menu(_)), "the ending plays under the post-race menus");
    assert!(frames > 60, "EndingNis01 did not play");
    assert!(smoke > 20, "tyre smoke particles in the burnout: {smoke}");
}

/// Uses the real scene/car assets, without a GPU (skipped without
/// NFSU2_ROOT). With NFSU2_INTRO_NIS=13 it also checks that scene's
/// Car1 burnout.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_start_wheel_phase_survives_scene_placement() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.settings.custom[5] = 1;
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let event = game.quick_event(mode);
    game.quick.car = Some("350Z".into());
    // A seed whose draw picks a scene with moving cars (IntroNis08).
    game.rng = nfsu2::opponents::GameRng(0);
    game.start_race(event, true, false).unwrap();
    // The intro the race draws (NFSU2_INTRO_NIS=n forces IntroNis<n>).
    let kind = game.scenes.shop.as_ref().unwrap().scene.kind;
    match std::env::var("NFSU2_INTRO_NIS").ok().and_then(|v| v.parse::<u8>().ok()) {
        Some(n) => assert_eq!(kind, nfsu2::shop_scene::SceneKind::Intro(n)),
        None => assert!(matches!(kind, nfsu2::shop_scene::SceneKind::Intro(_)), "{kind:?}"),
    }
    let mut spinning = 0;
    let mut moving = 0;
    for _ in 0..120 {
        let old: Vec<_> = {
            let Mode::Race(race) = &game.mode else { panic!("race") };
            std::iter::once(game.sim.drive.as_ref().unwrap().state.wheels)
                .chain(race.racers.rivals.iter().map(|r| r.state.wheels))
                .collect()
        };
        let from = game.scenes.shop.as_ref().unwrap().frame;
        game.advance_for(1.0 / 60.0).unwrap();
        let run = game.scenes.shop.as_ref().unwrap();
        let Mode::Race(race) = &game.mode else { panic!("race") };
        let d = game.sim.drive.as_ref().unwrap();
        let cars = std::iter::once((&d.car, &d.state, race.grid_slot + 1))
            .chain(race.racers.rivals.iter().map(|r| (&r.car, &r.state, r.slot + 1)));
        for ((car, state, slot), previous) in cars.zip(&old) {
            if run.scene.racer(slot, run.frame).is_none() {
                continue;
            }
            moving += usize::from(state.speed().abs() > 0.1);
            let (v, f) = (state.velocity, state.forward());
            assert_eq!(
                state.exported_forward_speed().to_bits(),
                ((v.z * f.z + v.y * f.y) + v.x * f.x).to_bits(),
                "scene placement publishes before handoff to ordinary physics"
            );
            let _ = from;
            for i in 0..4 {
                // The NIS mover integrates each wheel's roll per 1/70 s
                // substep from its overwritten spin (0x5AB890): the phase
                // continues from the last frame's, never reset. At 60 Hz
                // the mover steps every other frame (`nis::SCENE_FRAME`).
                let error = (0..3)
                    .map(|k| {
                        let delta = state.wheels[i].spin * k as f32 / 60.0;
                        let e = state.wheels[i].roll - previous[i].roll - delta;
                        e.sin().atan2(e.cos()).abs() / (1.0 + 0.05 * delta.abs())
                    })
                    .fold(f32::INFINITY, f32::min);
                assert!(error < 0.05, "slot {slot} wheel {i} reset its phase at {}", run.frame);
                let _ = car;
                if slot == 1 && run.frame >= 13.0 {
                    spinning += usize::from(state.wheels[i].spin.abs() > 20.0);
                }
            }
        }
    }
    assert!(moving > 0, "the scene exercised moving placements");
    // IntroNis13's Car1 burnout (NFSU2_INTRO_NIS=13); other scenes
    // check the phase hand-off alone.
    if kind == nfsu2::shop_scene::SceneKind::Intro(13) {
        assert!(spinning > 0, "the scripted Car1 burnout reached driven wheels");
    }
    let Mode::Race(race) = &game.mode else { panic!("race") };
    let run = game.scenes.shop.as_ref().unwrap();
    let mut companion = SceneCar::from_rival(&race.racers.rivals[0]);
    companion.slot = 1;
    companion.state.velocity = Vec3::splat(7.0);
    // Explicit synthetic/replay motion initialization; no native placement offset.
    companion.state.publish_forward_speed();
    let sample = companion.state.begin_world_update(1.0 / nfsu2::shop_scene::FPS);
    companion.frame(&run.scene, run.origin, 0.0, 1.0);
    let mut expected = companion.state.clone();
    expected.finish_world_update(sample);
    assert_eq!(
        companion.state.world_acceleration, expected.world_acceleration,
        "scene-owned companion publishes its outer motion interval"
    );
    let retained = companion.state.world_acceleration;
    companion.frame(&run.scene, run.origin, 1.0, 1.0);
    assert_eq!(companion.state.world_acceleration, retained, "zero scene dt preserves acceleration");
    companion.frame(&run.scene, run.origin, 1.0, 0.5);
    assert_eq!(companion.state.world_acceleration, retained, "negative scene dt preserves acceleration");
    let sample = companion.state.begin_world_update(2.0 / nfsu2::shop_scene::FPS);
    companion.frame(&run.scene, run.origin, 0.5, 2.5);
    let mut expected = companion.state.clone();
    expected.finish_world_update(sample);
    assert_eq!(companion.state.world_acceleration, expected.world_acceleration);

    // A scene overwrite must reuse the exact samples from race motion,
    // including its controller/mover decisions, rather than resample an
    // earlier pre-controller velocity.
    let Mode::Race(race) = &mut game.mode else { panic!("race") };
    let d = game.sim.drive.as_mut().unwrap();
    let samples = step_race(d, race, Controls::default(), game.sim.ai.as_ref(), 0.025, &mut Default::default());
    for (state, sample) in std::iter::once(&d.state)
        .chain(race.racers.rivals.iter().map(|r| &r.state))
        .zip(std::iter::once(samples.player).chain(samples.rivals.into_iter().map(Option::unwrap)))
    {
        let mut expected = state.clone();
        expected.finish_world_update(sample);
        assert_eq!(
            state.world_acceleration, expected.world_acceleration,
            "scene receives the same post-controller sample used by physics"
        );
    }
}

/// Real ending selection and shared scene updater; requires game assets.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn loss_ending_uses_player_slot_for_wheel_controls() {
    testkit::game();
    use nfsu2::shop_scene::{IntroTable, SceneKind};
    let mut game = Game::new().unwrap();
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let event = game.quick_event(mode);
    game.quick.car = Some("240SX".into());
    game.start_race(event, true, false).unwrap();
    let winner = game.sim.drive.as_ref().unwrap().car.car_type.handling.clone();
    assert!(winner.gearbox.rear_split > 0.8);
    let table = IntroTable::load(&game.game_data.root).unwrap();
    let seed = (0..1000)
        .find(|seed| {
            let mut rng = nfsu2::opponents::GameRng(*seed);
            table
                .pick_ending(4014, &winner, true, false, &mut rng)
                .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(1))
        })
        .expect("native loss variant for EndingNis01");
    game.rng = nfsu2::opponents::GameRng(seed);
    game.play_race_ending(4014, Vec3::ZERO, 0.0, &winner, false, None);
    let run = game.scenes.shop.as_ref().unwrap();
    assert_eq!(run.scene.kind, SceneKind::Ending(1));
    assert_eq!(run.car_slot, 2, "losing player must use entry slot +7");
    assert!(run.scene.wheel_events.iter().any(|e| e.slot == 1));
    assert!(!run.scene.wheel_events.iter().any(|e| e.slot == 2));
    // First update places the player; subsequent frames exercise the
    // live shared updater with winner-only burnout controls active.
    for _ in 0..4 {
        game.shop_scene_frame(1.0 / 60.0);
    }
    let d = game.sim.drive.as_ref().unwrap();
    let speed = if d.state.speed().abs() < 0.1 { 0.0 } else { d.state.speed() };
    for (wheel, spec) in d.state.wheels.iter().zip(&d.car.car_type.wheels) {
        assert!(
            (wheel.spin - speed / spec.radius).abs() < 1e-4,
            "slot1 winner burnout leaked to losing player slot2: spin {} rolling {}",
            wheel.spin,
            speed / spec.radius
        );
    }
}

/// The shop and dyno scenes' car on its NIS mover against the original
/// (Wine, the finished-career save's Lancer; `docs/evidence/shop-scene-mover/`):
/// ShopEnter1 and ShopExit1 at IND_BODYSHOP_1 and DynoEnter, the
/// body's centre of mass per scene count turned into the car's origin
/// (the clone's own centre of mass). The shops' car (on the 30 fps run's
/// steps, `nis::ScenePace`) stays within 1 cm of the original's in x / y
/// at every mover step; the dyno's origin within 0.3 m (the faster
/// drive, one count off) at every 1/30 s frame, and DynoEnter ends on the
/// rollers where the original's did (within 8 cm), which the channel
/// placement missed by 0.29 m. Real assets, no GPU.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn shop_and_dyno_scenes_drive_the_car_like_the_original() {
    testkit::game();
    use nfsu2::shop_scene::SceneKind;
    let native = |text: &str| -> Vec<(u32, Vec3, Vec3, Vec3)> {
        text.lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let w: Vec<&str> = l.split('\t').collect();
                (w[2] == "5").then(|| {
                    let f = |k: usize| {
                        Vec3::new(w[k].parse().unwrap(), w[k + 1].parse().unwrap(), w[k + 2].parse().unwrap())
                    };
                    (w[3].parse().unwrap(), f(4), f(7), f(10))
                })
            })
            .collect()
    };
    let save = nfsu2::formats::native_save::NativeSave::parse(&testkit::private("tests/fixtures/native_saves/career_complete.sav"))
    .unwrap();
    let run = |kind: SceneKind| -> (Vec<(u32, Vec3)>, Vec<(u32, Vec3)>, Vec3, Car, Vec3) {
        let mut game = Game::new().unwrap();
        let ctx = career::NativeContext::get().unwrap();
        game.settings.apply_native(&save);
        game.profile = Some(Profile::from_native(&save, ctx));
        game.explore(None).unwrap();
        if kind == SceneKind::DynoEnter {
            game.dyno.tuning_mode = 0;
            game.mode = game.enter_dyno_scene();
        } else {
            let shop = game.game_data.career.shops.iter().position(|s| s.name == "IND_BODYSHOP_1").unwrap();
            assert!(game.start_shop_scene(kind, shop));
        }
        let mut track = vec![(0, game.sim.drive.as_ref().unwrap().state.position)];
        // Indexed by the native frames stepped (1/30 s each: one scene tick
        // in the dyno, two race ticks in a shop's uncapped scene), as the
        // capture's frame counter counts them; on the tick cadence the scene's float
        // frame runs 1/60 ahead on each 61-tick frame, so it no longer
        // names the frame.
        let mut stepped = 0u32;
        // And by the mover's count (the capture's count column).
        let mut counts: Vec<(u32, Vec3)> = Vec::new();
        while game.scenes.shop.as_ref().is_some_and(|r| r.scene.kind == kind) {
            game.advance_for(1.0 / 30.0).unwrap();
            stepped += 1;
            let state = &game.sim.drive.as_ref().unwrap().state;
            track.push((stepped, state.position));
            if let Some(m) = state.nis {
                counts.retain(|c: &(u32, Vec3)| c.0 != m.count as u32);
                counts.push((m.count as u32, state.position));
            }
            assert!(track.len() < 400);
        }
        // ShopExit1 hands the car back on its physics: three more
        // frames (the original's stood still by its second poll).
        if kind == SceneKind::Exit {
            for _ in 0..3 {
                game.advance_for(1.0 / 30.0).unwrap();
            }
            assert_eq!(
                game.sim.drive.as_ref().unwrap().state.velocity,
                Vec3::ZERO,
                "ShopExit1: the car still moves after the scene"
            );
        }
        let d = game.sim.drive.as_ref().unwrap();
        let com = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
        (track, counts, d.state.position, d.car.clone(), com)
    };
    for (kind, text, limit) in [
        (
            SceneKind::Enter,
            &testkit::private_str("docs/evidence/shop-scene-mover/native-shopenter.tsv"),
            0.15,
        ),
        (
            SceneKind::Exit,
            &testkit::private_str("docs/evidence/shop-scene-mover/native-shopexit.tsv"),
            0.15,
        ),
        (
            SceneKind::DynoEnter,
            &testkit::private_str("docs/evidence/shop-scene-mover/native-dynoenter.tsv"),
            0.3,
        ),
    ] {
        let (track, counts, end, car, end_com) = run(kind);
        let com = Vec3::from(car.car_type.handling.centre_of_mass);
        let origin = |c: Vec3, f: Vec3, u: Vec3| c - (f * com.x + u.cross(f) * com.y + u * com.z);
        let nat = native(text);
        assert!(nat.len() > 100, "{kind:?}: native samples");
        let worst = [0i32, -1].map(|off| {
            nat.iter()
                .filter_map(|&(c, cm, f, u)| {
                    let at = track.iter().find(|t| t.0 as i32 == c as i32 + off)?.1;
                    Some((origin(cm, f, u) - at).truncate().length())
                })
                .fold(0.0f32, f32::max)
        });
        let worst = worst[0].min(worst[1]);
        // Per mover step: the original's row at count c shows the body
        // after c - 1 steps (its count goes up when the frame's target is
        // set, before the world's step integrates it), the clone's car
        // after c - 1 steps against it. A count the polls read twice
        // (once mid-frame: count 114 at 0.151 m) is left out.
        if kind == SceneKind::DynoEnter {
            // Under the 1/30 s ticks: one step a frame (`track` above).
        } else {
        let once = |c: u32| nat.iter().filter(|n| n.0 == c).count() == 1;
        let per_step: Vec<f32> = nat
            .iter()
            .filter(|n| once(n.0))
            .filter_map(|&(c, cm, f, u)| Some((origin(cm, f, u) - counts.iter().find(|t| t.0 + 1 == c)?.1).truncate().length()))
            .collect();
        let step_worst = per_step.iter().fold(0.0f32, |a, &b| a.max(b));
        let step_mean = per_step.iter().sum::<f32>() / per_step.len().max(1) as f32;
        eprintln!(
            "{kind:?}: per mover step ({} counts), worst x / y {step_worst:.4} m, mean {step_mean:.4} m",
            per_step.len()
        );
        // The shops' scenes take the 30 fps run's steps
        // (`nis::ScenePace`): within 1 cm of the original's at every step.
        assert!(per_step.len() > 100 && step_worst < 0.01, "{kind:?}: a step {step_worst} m from the original's");
        }
        // The height: the origin's z within 2 cm of the
        // original's on average (an earlier "8 cm under" compared the
        // finished-career save's Skyline with the original's Lancer).
        let dz: Vec<f32> = nat
            .iter()
            .filter_map(|&(c, cm, f, u)| Some(origin(cm, f, u).z - track.iter().find(|t| t.0 == c)?.1.z))
            .collect();
        let mean_dz = dz.iter().sum::<f32>() / dz.len().max(1) as f32;
        eprintln!("{kind:?}: {} clone frames, worst x / y {worst:.3} m, mean dz {mean_dz:.3} m", track.len());
        assert!(kind != SceneKind::DynoEnter || worst < limit, "{kind:?}: the clone's car {worst} m from the original's");
        assert!(dz.len() > 100 && mean_dz.abs() < 0.02, "{kind:?}: the origin {mean_dz} m off the original's height");
        if kind == SceneKind::Exit {
            // Where the original's car stood after the scene (mode 1,
            // its last row): the centre of mass within 2 cm.
            let rest = text
                .lines()
                .filter(|l| !l.starts_with('#'))
                .map(|l| l.split('\t').collect::<Vec<_>>())
                .filter(|w| w[2] == "1")
                .last()
                .unwrap();
            let cm = Vec3::new(rest[4].parse().unwrap(), rest[5].parse().unwrap(), rest[6].parse().unwrap());
            let gap = cm.distance(end_com);
            eprintln!("ShopExit1: the stopped car's centre of mass {end_com:?} {gap:.3} m from the original's {cm:?}");
            assert!(gap < 0.02, "ShopExit1: the stopped car {gap} m from the original's");
        }
        if kind == SceneKind::DynoEnter {
            let (_, cm, f, u) = *nat.last().unwrap();
            let gap = (origin(cm, f, u) - end).truncate().length();
            assert!(gap < 0.08, "DynoEnter ends {gap} m from the original's place on the rollers");
        }
    }
}

/// The shops' drive-in / drive-out scenes run at the display's rate
/// (docs/playing/native-deviations.md "High refresh rates"); their car keeps the
/// steps of the original's 30 fps run (`nis::ScenePace`): one mover step
/// per 1/30 s frame of 60, 60, 60, 60, 61 ticks of 1/1800 s, the same steps
/// bit for bit at 30, 60, 120 and 200 fps, and the car drawn between its
/// last two steps, so it moves on every drawn frame while the scene drives
/// it (before, it stood still for two of every three 60 Hz updates and
/// moved 20 times a second). Real assets, no GPU.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn shop_scene_car_takes_the_30_fps_steps_at_any_frame_rate() {
    testkit::game();
    use nfsu2::shop_scene::SceneKind;
    let bits = |v: Vec3| v.to_array().map(f32::to_bits);
    for kind in [SceneKind::Enter, SceneKind::Exit] {
        let mut runs = Vec::new();
        for fps in [30.0f32, 60.0, 120.0, 200.0] {
            let mut game = Game::new().unwrap();
            let mut profile = Profile::new("DRIVEIN_RATES");
            profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
            game.profile = Some(profile);
            game.explore(None).unwrap();
            let shop = game.game_data.career.shops.iter().position(|s| s.name == "IND_BODYSHOP_1").unwrap();
            assert!(game.start_shop_scene(kind, shop));
            // Per mover step: its count, the body's place and the step's
            // ticks.
            let mut steps: Vec<(i32, [u32; 3], i64)> = Vec::new();
            let (mut moving, mut still) = (0usize, 0usize);
            let mut last: Option<Vec3> = None;
            let mut frames = 0;
            while game.scenes.shop.as_ref().is_some_and(|r| r.scene.kind == kind) {
                let alpha = game.advance(1.0 / fps).unwrap();
                frames += 1;
                assert!(frames < 5000, "{kind:?} at {fps} fps: the scene does not end");
                let d = game.sim.drive.as_ref().unwrap();
                let Some(m) = d.state.nis else {
                    last = None;
                    continue;
                };
                let pace = m.pace.expect("a shop scene's car is on its 30 fps schedule");
                if steps.last().is_none_or(|s| s.0 != m.count) {
                    steps.push((m.count, bits(d.state.position), pace.span));
                }
                let drawn = game.frame_poses(alpha).player().position;
                // While the scene drives the car (its newest step moved it
                // over 1 cm: 0.3 m/s; at rest it settles by fractions of a
                // millimetre), every drawn frame shows it somewhere new.
                if let Some(l) = last
                    && pace.before.position.distance(d.state.position) > 0.01
                {
                    moving += 1;
                    still += usize::from(drawn == l);
                }
                last = Some(drawn);
            }
            eprintln!("{kind:?} at {fps} fps: {} steps, {frames} frames, {moving} moving, {still} still", steps.len());
            assert!(moving > 30, "{kind:?} at {fps} fps: the scene moved the car");
            assert_eq!(still, 0, "{kind:?} at {fps} fps: the drawn car stood still between moving frames");
            // The 30 fps limiter's frames: 60, 60, 60, 60, 61 ticks.
            for w in steps.windows(5).skip(1).filter(|_| fps >= 60.0) {
                assert_eq!(w.iter().filter(|s| s.2 == 61).count(), 1, "{kind:?}: a 30 fps run's frame times");
                assert!(w.iter().all(|s| s.2 == 60 || s.2 == 61));
            }
            runs.push((fps, steps));
        }
        // The world's ticks are the same at any rate (`frame_clock`), so
        // are the car's steps, bit for bit. A 30 fps frame (two ticks) may
        // hold two steps, so it sees only the second; from 60 fps on every
        // step is seen.
        let (_, all) = runs.last().unwrap();
        assert!(all.len() > 100, "{kind:?}: {} steps", all.len());
        assert!(all.windows(2).all(|w| w[1].0 == w[0].0 + 1), "{kind:?}: a step not seen at 200 fps");
        for (fps, steps) in &runs {
            assert!(*fps < 60.0 || steps.len() == all.len(), "{kind:?} at {fps} fps: {} steps, not {}", steps.len(), all.len());
            let seen: Vec<_> = all.iter().filter(|a| steps.iter().any(|s| s.0 == a.0)).cloned().collect();
            assert_eq!(seen.len(), steps.len(), "{kind:?} at {fps} fps: a step 200 fps did not take");
            assert_eq!(steps, &seen, "{kind:?}: the car's steps at {fps} fps differ from 200 fps");
        }
    }
}

/// The Dyno Shop's Test Run on the dyno plays DynoExit first (Wine,
/// the maintainer's private capture: the finished-career save's Lancer,
/// mode 5 for the scene's 180 frames off the rollers and down the
/// ramp, then the test run's track). The clone's car follows the
/// original's centre of mass and the test run starts at the end.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn test_run_drives_off_the_dyno_through_dyno_exit() {
    testkit::game();
    use nfsu2::shop_scene::SceneKind;
    let save = nfsu2::formats::native_save::NativeSave::parse(&testkit::private("tests/fixtures/native_saves/career_complete.sav"))
    .unwrap();
    let mut game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    game.settings.apply_native(&save);
    game.profile = Some(Profile::from_native(&save, ctx));
    game.explore(None).unwrap();
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno_scene();
    while game.scenes.shop.is_some() {
        game.advance_for(1.0 / 30.0).unwrap();
    }
    assert!(matches!(game.mode, Mode::Menu(_)), "the dyno menu after DynoEnter");
    game.act(Action::TestRun, MenuKind::Dyno).unwrap();
    assert_eq!(game.scenes.shop.as_ref().map(|r| r.scene.kind), Some(SceneKind::DynoExit));
    let mut track = Vec::new();
    while let Some(r) = game.scenes.shop.as_ref().filter(|r| r.scene.kind == SceneKind::DynoExit) {
        let frame = r.frame;
        let d = game.sim.drive.as_ref().unwrap();
        track
            .push((frame, d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass)));
        game.advance_for(1.0 / 30.0).unwrap();
        assert!(track.len() < 400);
    }
    assert!(matches!(&game.mode, Mode::Race(r) if r.test.is_some()), "the test run after DynoExit");
    // Return to Dyno from the paused run: the session's reload plays
    // DynoEnter again (Wine), then the Dyno Shop.
    for _ in 0..10 {
        game.advance_for(1.0 / 30.0).unwrap();
    }
    game.key(KeyCode::Escape, &mut || {}).unwrap();
    assert!(game.sim.paused_race.is_some(), "the run is paused");
    game.act(Action::ReturnToDyno, MenuKind::Dyno).unwrap();
    game.menu_key(Key::Left).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert_eq!(
        game.scenes.shop.as_ref().map(|r| r.scene.kind),
        Some(SceneKind::DynoEnter),
        "DynoEnter after Return to Dyno"
    );
    game.mode = Mode::Explore;
    game.scenes.shop = None;
    let rows: Vec<Vec<f32>> =
        testkit::private_str("docs/evidence/dyno-exit/native-dynoexit.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split('\t').map(|w| w.parse().unwrap()).collect())
            .collect();
    // The scene's clock from the last mode-1 row (the scene's start,
    // within a poll); the clone's pose at the same scene time,
    // interpolated between its frames.
    let t0 = rows.iter().take_while(|w| w[2] != 5.0).last().unwrap()[1];
    let at = |frame: f32| -> Option<Vec3> {
        let k = track.iter().position(|t| t.0 >= frame)?;
        if k == 0 {
            return Some(track[0].1);
        }
        let (a, b) = (track[k - 1], track[k]);
        Some(a.1.lerp(b.1, (frame - a.0) / (b.0 - a.0)))
    };
    let (mut worst, mut sum, mut n) = (0.0f32, 0.0f32, 0);
    for w in rows.iter().filter(|w| w[2] == 5.0) {
        let Some(c) = at((w[1] - t0) / (4000.0 / 30.0)) else { continue };
        let gap = (c - Vec3::new(w[3], w[4], w[5])).length();
        worst = worst.max(gap);
        sum += gap;
        n += 1;
    }
    eprintln!(
        "DynoExit: {} clone frames, {n} native samples, mean {:.3} m, worst {worst:.3} m",
        track.len(),
        sum / n as f32
    );
    // The poller read the tick and the body in two requests (a few ms
    // apart, up to 19 m/s at the end): the worst sample is the fastest.
    assert!(
        n > 150 && sum / (n as f32) < 0.2 && worst < 0.8,
        "DynoExit: the clone's car {worst} m from the original's"
    );
}

/// The dyno camera 0x15 through a Dyno Run under Wine (the finished-career save's
/// Lancer on the rollers after DynoEnter, `docs/evidence/shop-scene-mover/
/// native-dynorun-camera.txt`): the player's mover stayed vtable 0x788070
/// for the whole pull and the results, eye (−1675.7385, −1959.4895,
/// 46.1347) and forward (−0.8820, 0.2940, −0.3683) unchanged to the
/// float while the body moved, +0xC4 8000; the car's origin (+0x50)
/// (−1682.0702, −1957.3428, 43.4827), axes (+0xD0) forward (−0.5,
/// 0.8661, 0), left (−0.8661, −0.5, 0). The clone's orbit from the same
/// frame lands within 4 cm and 0.3° of it.
#[test]
fn dyno_camera_matches_the_dyno_run_under_wine() {
    let fwd = Vec3::new(-0.5, 0.8661, 0.0).normalize();
    let left = Vec3::new(-0.8661, -0.5, 0.0).normalize();
    let q = glam::Quat::from_mat3(&glam::Mat3::from_cols(fwd, left, fwd.cross(left)));
    let cam = dyno_camera(Vec3::new(-1682.0702, -1957.3428, 43.4827), q, DYNO_DISTANCE.1);
    let eye = Vec3::new(-1675.7385, -1959.4895, 46.1347);
    assert!(cam.position.distance(eye) < 0.05, "eye {:?}", cam.position);
    let native = Vec3::new(-0.8820, 0.2940, -0.3683).normalize();
    assert!(cam.forward().angle_between(native).to_degrees() < 0.3, "forward {:?}", cam.forward());
    assert_eq!(cam.game_fov, Some(8000.0 * std::f32::consts::TAU / 65536.0));
}

/// Scene ownership and native result bindings, without a GPU. An AI
/// winner must survive dropping a race that has no replay recording.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn ending_keeps_ai_winner_in_its_channel_without_mutating_replay() {
    testkit::game();
    use nfsu2::shop_scene::{IntroTable, SceneKind};
    let mut game = Game::new().unwrap();
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let event = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some("240SX".into());
    game.start_race(event, true, false).unwrap();
    let (companion, winner, original_position, original_roll) = {
        let Mode::Race(race) = &mut game.mode else { panic!("race") };
        let rival = race.racers.rivals.first().expect("a real race rival");
        (
            SceneCar::from_rival(rival),
            rival.car.car_type.handling.clone(),
            rival.state.position,
            rival.state.wheels[2].roll,
        )
    };
    let name = companion.car.name.clone();
    let table = IntroTable::load(&game.game_data.root).unwrap();
    let seed_for = |won| {
        (0..1000)
            .find(|seed| {
                let mut rng = nfsu2::opponents::GameRng(*seed);
                table
                    .pick_ending(4014, &winner, true, won, &mut rng)
                    .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(2))
            })
            .expect("native EndingNis02 variant")
    };
    game.rng = nfsu2::opponents::GameRng(seed_for(false));
    game.play_race_ending(4014, Vec3::ZERO, 0.0, &winner, false, Some(companion));
    for _ in 0..12 {
        game.shop_scene_frame(1.0 / 60.0);
    }
    let run = game.scenes.shop.as_ref().unwrap();
    assert_eq!(run.scene.kind, SceneKind::Ending(2));
    assert_eq!(run.car_slot, 2);
    let c = run.companion.as_ref().expect("AI winner retained for the rendering paths");
    assert_eq!((c.slot, c.car.name.as_str()), (1, name.as_str()));
    // Ending cars run on their NIS movers (0x4309D0 -> 0x5A40B0 ->
    // 0x5AB3D0): the channel gives the centre of mass's x / y and the
    // heading, the height stays the car's own (here no road under the
    // test's zero origin), so the planar pose is what is native.
    for (slot, car, state) in [
        (c.slot, &c.car, &c.state),
        (run.car_slot, &game.sim.drive.as_ref().unwrap().car, &game.sim.drive.as_ref().unwrap().state),
    ] {
        let (at, rot) = run.scene.racer(slot, run.frame).expect("distinct native bound channel");
        let expected = run.origin.transform_point3(at).truncate();
        let centre = (state.position + state.orientation * Vec3::from(car.car_type.handling.centre_of_mass)).truncate();
        let forward = run.origin.transform_vector3(rot * Vec3::X).truncate().normalize();
        let heading = (state.orientation * Vec3::X).truncate().normalize();
        let (track, turn) = (centre.distance(expected), heading.angle_to(forward).abs().to_degrees());
        eprintln!("ending slot {slot}: centre {track:.4} m, heading {turn:.3} deg off the channel");
        assert!(track < 0.05, "slot {slot} {track} m off its channel");
        assert!(turn < 1.0, "slot {slot} {turn} deg off its channel");
    }
    let Mode::Race(race) = &mut game.mode else { panic!("race") };
    assert_eq!(race.racers.rivals[0].state.position, original_position);
    assert_eq!(race.racers.rivals[0].state.wheels[2].roll, original_roll);
    // No recording: retaining the scene car cannot depend on self.replay.
    race.replay.recording = None;
    game.keep_replay(Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Display,
        &game.settings,
        false,
        0.0,
    ))));
    assert!(game.sim.replay.is_none());
    assert!(game.scenes.shop.as_ref().unwrap().companion.is_some());
    game.shop_scene_frame(1.0 / 60.0);
    let supplied = game.scenes.shop.as_mut().unwrap().companion.take();
    game.rng = nfsu2::opponents::GameRng(seed_for(true));
    game.play_race_ending(4014, Vec3::ZERO, 0.0, &winner, true, supplied);
    let run = game.scenes.shop.as_ref().unwrap();
    assert_eq!(run.car_slot, 1);
    assert!(run.companion.is_none(), "single-player wins do not invent a runner-up in player 1's slot");

    // Exercise the real race-results dispatcher too, with the second AI
    // winning so a mistaken first-rival snapshot cannot satisfy the test.
    game.start_race(event, true, false).unwrap();
    game.scenes.shop = None;
    let (winner_name, winner_handling) = {
        let Mode::Race(race) = &mut game.mode else { panic!("race") };
        assert!(race.racers.rivals.len() >= 2);
        race.timing.clock = 10.0;
        race.racers.me.finished = Some(10.0);
        race.over = 100.0;
        race.replay.recording = None;
        for r in &mut race.racers.rivals {
            r.progress.finished = Some(20.0);
        }
        race.racers.rivals[1].progress.finished = Some(5.0);
        (race.racers.rivals[1].car.name.clone(), race.racers.rivals[1].car.car_type.handling.clone())
    };
    game.rng = nfsu2::opponents::GameRng(
        (0..1000)
            .find(|seed| {
                let mut rng = nfsu2::opponents::GameRng(*seed);
                table
                    .pick_ending(4014, &winner_handling, true, false, &mut rng)
                    .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(2))
            })
            .unwrap(),
    );
    game.advance_for(1.0 / 60.0).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Results(..))));
    // The ordinary race update starts a recording automatically. Its
    // retained rival must also remain independent of the scene snapshot.
    let saved = game.sim.replay.as_ref().unwrap().racers.rivals[1].state.position;
    let run = game.scenes.shop.as_ref().expect("ending after real results transition");
    assert_eq!(run.car_slot, 2);
    assert_eq!(run.companion.as_ref().expect("AI winner visible in ending").car.name, winner_name);
    game.shop_scene_frame(1.0 / 60.0);
    assert_eq!(game.sim.replay.as_ref().unwrap().racers.rivals[1].state.position, saved);
}

/// The race endings read ahead (`flow::endings_ahead`): a finish that takes
/// them plays the ending a finish that loads it in place plays — the same
/// pick (the random draw included), the same scene, cameras and
/// characters — for each of the four scenes, won and lost.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn an_ending_read_ahead_is_the_one_loaded_at_the_finish() {
    testkit::game();
    super::test_isolation::exclusive();
    use nfsu2::shop_scene::IntroTable;
    let start = |ahead: bool| {
        let mut game = Game::new().unwrap();
        let mode = screens::QrMode::Circuit;
        game.quick.tracks = game.quick_track_select(mode, 0.0);
        let event = game.quick_event(mode);
        game.quick.car = Some("240SX".into());
        game.start_race(event, true, false).unwrap();
        if ahead {
            let job = game.race_flow.endings_ahead.as_ref().expect("read ahead at the start");
            while !job.is_finished() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        } else {
            game.race_flow.endings_ahead = None;
        }
        game
    };
    let probe = start(false);
    let winner = probe.sim.drive.as_ref().unwrap().car.car_type.handling.clone();
    let table = IntroTable::load(&probe.game_data.root).unwrap();
    let mut played = 0;
    for won in [false, true] {
        for n in 1..=4u8 {
            let Some(seed) = (0..2000).find(|seed| {
                let mut rng = nfsu2::opponents::GameRng(*seed);
                table
                    .pick_ending(4014, &winner, true, won, &mut rng)
                    .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(n))
            }) else {
                continue;
            };
            let play = |ahead: bool| {
                let mut game = start(ahead);
                game.rng = nfsu2::opponents::GameRng(seed);
                game.play_race_ending(4014, Vec3::ZERO, 0.0, &winner, won, None);
                assert!(game.race_flow.endings_ahead.is_none(), "taken or not made");
                let run = game.scenes.shop.as_ref().expect("an ending plays");
                let crowd = game.scenes.crowd.as_ref().map(|c| (c.set.instances.len(), c.length));
                (format!("{:?}", run.scene), run.car_slot, run.camera.is_some(), crowd, game.rng.0)
            };
            assert_eq!(play(true), play(false), "EndingNis{n:02}, won {won}");
            played += 1;
        }
    }
    assert!(played >= 4, "endings checked: {played}");
}


/// The race start scenes' people stand on the ground at every start: each
/// skinned character's lowest vertex within a few centimetres of the
/// ground under its bone 1 (0x436050 puts its world height on that ground
/// + 0.05 every update), for every career track's start and every intro
/// scene its start can draw. Without the ground step the scene's single
/// height at the start line left people below the road or in the air
/// where the road slopes or curbs rise. Real assets, no GPU; run with
/// isolated XDG homes.
#[test]
#[ignore = "requires original assets; loads every career track (minutes)"]
fn race_start_crowd_stands_on_the_ground() {
    use nfsu2::shop_scene::{IntroTable, Scene, SceneKind, origin};
    testkit::game();
    let mut game = Game::new().unwrap();
    game.settings.custom[5] = 1;
    let mut profile = Profile::new("CROWD_GROUND_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    let table = IntroTable::load(&game.game_data.root).unwrap();
    let limit = std::env::var("OB_CROWD_TRACKS").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(usize::MAX);
    let mut seen = std::collections::HashSet::new();
    // Per placement the lowest and the highest feet-to-ground gap and where, and how many person frames
    // were more than 0.1 m under the ground or 0.3 m over it.
    type Worst = ((f32, String), (f32, String), usize, usize, usize);
    let fresh = || -> Worst { ((f32::INFINITY, String::new()), (f32::NEG_INFINITY, String::new()), 0, 0, 0) };
    let (mut checked, mut worst, mut flat_worst) = (0usize, fresh(), fresh());
    let events: Vec<(usize, u16)> =
        game.game_data.career.events.iter().enumerate().map(|(i, e)| (i, e.track)).collect();
    for (e, number) in events {
        if seen.len() >= limit || !seen.insert(number) {
            continue;
        }
        if game.start_race(e, false, false).is_err() {
            continue;
        }
        let Mode::Race(race) = &game.mode else { continue };
        let Some(track) = game.game_data.career.tracks.get(&number) else { continue };
        let (Some(at), Some(heading)) = (race.route.start_line, race.route.start_heading) else { continue };
        let picks = table.race_candidates(
            track.number,
            !race.reversed,
            race.racers.rivals.len() as u32 + 1,
            race.grid_slot as u8 + 1,
            track.start_kind as u8,
        );
        let d = game.sim.drive.as_ref().unwrap();
        let o = origin(Vec3::new(at.x, at.y, d.world.grid_height(at)), heading);
        let mut scenes: Vec<u8> =
            picks.iter().filter_map(|&k| IntroTable::scene_number(table.race_list[k].hash)).collect();
        scenes.dedup();
        for n in scenes {
            let Ok(scene) = Scene::load(&game.game_data.root, SceneKind::Intro(n)) else { continue };
            let Some(set) =
                nfsu2::game_fs::read(&game.game_data.root, &format!("NIS/Scene_{}_BundleB.bun", scene.kind.name()))
                    .ok()
                    .and_then(|b| nfsu2::character::SceneCharacters::parse(&b).ok())
            else {
                continue;
            };
            let mut crowd = SceneCrowd::new(std::sync::Arc::new(set), o, scene.length, &d.world);
            while crowd.frame < crowd.length {
                let t = crowd.frame / nfsu2::shop_scene::FPS;
                let probes = crowd.set.ground_probe(t, o, &crowd.lifts);
                for (lifts, out) in [(&crowd.lifts[..], &mut worst), (&[][..], &mut flat_worst)] {
                    // `posed` leaves out exactly the characters `ground_probe` has no point for.
                    let points = probes.iter().flatten();
                    for (i, ((model, palette, world), p)) in
                        crowd.set.posed(t, o, lifts).into_iter().zip(points).enumerate()
                    {
                        let Some(g) = d.world.grid_ground(*p) else { continue };
                        let low = crowd.set.models[model]
                            .skin(&palette, world)
                            .iter()
                            .map(|v| v.0.z)
                            .fold(f32::INFINITY, f32::min);
                        let gap = low - g;
                        let at = || format!("track {number} IntroNis{n:02} person {i} frame {:.0}", crowd.frame);
                        if gap < out.0.0 {
                            out.0 = (gap, at());
                        }
                        if gap > out.1.0 {
                            out.1 = (gap, at());
                        }
                        out.2 += usize::from(gap < -0.1);
                        out.3 += usize::from(gap > 0.3);
                        out.4 += 1;
                    }
                }
                checked += 1;
                crowd.advance(10.0 / nfsu2::shop_scene::FPS, &d.world);
            }
        }
    }
    eprintln!(
        "{} tracks, {checked} scene frames; feet to ground (lowest, highest, frames under / over, frames): {worst:?}",
        seen.len()
    );
    eprintln!("without the ground step: {flat_worst:?}");
    assert!(checked > 100, "scene frames checked: {checked}");
    // 0.05 over the ground, the lowest vertex a sole's thickness lower; a few dance steps lift a foot.
    assert!(worst.0.0 > -0.1, "a person's feet {} m under the ground: {}", worst.0.0, worst.0.1);
    assert!(worst.1.0 < 0.25, "a person's feet {} m over the ground: {}", worst.1.0, worst.1.1);
}

/// A race's ending scene and its characters end together (issue #337):
/// leaving the post-race flow for the city (0x603EC0, every way out of
/// post-race state 7, runs the NIS manager's unload 0x43B640: scene stop
/// 0x439070, scene freed 0x43B1F0) leaves no crowd behind. A crowd kept
/// into free roam was drawn again by the next menu over the world (the
/// race's Engage) through a character renderer whose texture slots were
/// another world renderer's (a region left), which panicked. Real
/// assets, no GPU.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_ending_crowd_goes_with_its_scene_at_the_city_start() {
    testkit::game();
    super::test_isolation::exclusive();
    use nfsu2::shop_scene::{IntroTable, SceneKind};
    let mut game = Game::new().unwrap();
    let mode = screens::QrMode::Circuit;
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    let event = game.quick_event(mode);
    game.quick.car = Some("240SX".into());
    game.start_race(event, true, false).unwrap();
    let winner = game.sim.drive.as_ref().unwrap().car.car_type.handling.clone();
    // An ending with characters (EndingNis04: the crowd round the car).
    let table = IntroTable::load(&game.game_data.root).unwrap();
    let seed = (0..2000)
        .find(|seed| {
            let mut rng = nfsu2::opponents::GameRng(*seed);
            table
                .pick_ending(4014, &winner, true, true, &mut rng)
                .is_some_and(|i| IntroTable::ending_number(table.ending_list[i].hash) == Some(4))
        })
        .expect("a seed for EndingNis04");
    game.rng = nfsu2::opponents::GameRng(seed);
    game.play_race_ending(4014, Vec3::ZERO, 0.0, &winner, true, None);
    assert!(matches!(game.scenes.shop.as_ref().map(|r| r.scene.kind), Some(SceneKind::Ending(4))));
    assert!(game.scenes.crowd.is_some(), "the ending's characters play");
    // Out of the post-race screens to the city.
    game.explore(None).unwrap();
    assert!(game.scenes.shop.is_none(), "the ending scene ended");
    assert!(game.scenes.crowd.is_none(), "the ending's characters went with their scene");
}
