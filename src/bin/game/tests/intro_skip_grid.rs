use super::*;

#[derive(Debug)]
struct Handoff {
    scene: String,
    cut: f32,
    slots: Vec<usize>,
    at_cut: Vec<Vec3>,
    at_green: Vec<Vec3>,
    cut_centres: Vec<Vec3>,
    targets: Vec<Vec3>,
    progress: Vec<f32>,
}
fn run(scene_number: u8, skip: Option<f32>, hz: f32) -> Handoff {
    let mut game = Game::new().unwrap();
    game.settings.custom[5] = 1;
    let mut profile = Profile::new("INTRO_GRID_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    // Actual eligible picks from the first-career drag candidate list.
    // The pick (0x436A26) draws on the shared generator after the
    // set-up's and the world build's draws (with the render infos and the
    // traffic fill in the load), so the seeds that give these scenes move
    // with them (25: 0; 3: 4).
    game.rng = nfsu2::opponents::GameRng(match scene_number {
        3 => 4,
        25 => 0,
        _ => unreachable!(),
    });
    let event =
        game.game_data.career.stage_events(&game.game_data.root, 1).into_iter().find(|e| e.kind == career::EventKind::Drag).unwrap().index;
    game.start_race(event, false, false).unwrap();
    let scene = game.scenes.shop.as_ref().unwrap();
    assert_eq!(
        scene.scene.kind,
        nfsu2::shop_scene::SceneKind::Intro(scene_number),
        "explicit moving/stationary scene setup; unset NFSU2_INTRO_NIS"
    );
    let cut = scene.scene.marks[1] as f32;
    let name = scene.scene.kind.name();
    let Mode::Race(race) = &game.mode else { panic!("race") };
    let slots: Vec<_> = race.racers.rivals.iter().map(|r| r.slot + 1).collect();
    let targets: Vec<_> =
        slots.iter().map(|&slot| scene.origin.transform_point3(scene.scene.racer(slot, cut).unwrap().0)).collect();
    let skip_frame = skip.map(|fraction| fraction * cut);
    let mut skipped = false;
    let mut at_cut = None;
    let mut cut_centres = None;
    for _ in 0..(30.0 * hz) as usize {
        if !skipped && skip_frame.is_some_and(|f| game.scenes.shop.as_ref().is_some_and(|s| s.frame >= f)) {
            let controller_state = |game: &Game| {
                let Mode::Race(race) = &game.mode else { panic!("race") };
                race.racers.rivals
                    .iter()
                    .map(|r| {
                        (
                            r.route.as_ref().and_then(|route| route.node()),
                            r.rng.0,
                            format!("{:?}/{:?}", r.driver, r.controls),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let before = controller_state(&game);
            game.key(KeyCode::Enter, &mut || {}).unwrap();
            assert_eq!(controller_state(&game), before, "seek must preserve existing route/driver state");
            let Mode::Race(race) = &game.mode else { panic!("race") };
            let d = game.sim.drive.as_ref().unwrap();
            let run = game.scenes.shop.as_ref().unwrap();
            for (car, state, slot) in std::iter::once((&d.car, &d.state, race.grid_slot + 1))
                .chain(race.racers.rivals.iter().map(|r| (&r.car, &r.state, r.slot + 1)))
            {
                let target = run.origin.transform_point3(run.scene.racer(slot, cut).unwrap().0);
                // Authored scene points are centres of mass; CarState.position
                // is the model origin. Check the data target independently.
                let centre = state.position + state.orientation * Vec3::from(car.car_type.handling.centre_of_mass);
                let error = centre.truncate().distance(target.truncate());
                eprintln!("SEEK_TARGET scene={scene_number} hz={hz} slot={slot} error={error}");
                assert!(error <= 2e-4, "seek missed authored slot{slot}: {error}");
                assert_eq!(state.velocity, Vec3::ZERO);
                assert_eq!(state.spin, Vec3::ZERO);
                assert_eq!(state.angular_momentum, Vec3::ZERO);
            }
            for r in &race.racers.rivals {
                assert_eq!(r.state.velocity, Vec3::ZERO, "seek is not elapsed travel");
                assert_eq!(r.state.spin, Vec3::ZERO);
                assert_eq!(r.state.angular_momentum, Vec3::ZERO, "preserve momentum handoff fix");
                assert!(r.progress.dropped.is_none() && r.progress.finished.is_none());
            }
            skipped = true;
        }
        // A drawn frame at that display rate, the window's path.
        game.advance(1.0 / hz).unwrap();
        let Mode::Race(race) = &game.mode else { panic!("race changed") };
        let poses = || race.racers.rivals.iter().map(|r| r.state.position).collect::<Vec<_>>();
        if at_cut.is_none() && game.scenes.shop.as_ref().is_some_and(|s| s.frame >= cut) {
            at_cut = Some(poses());
            cut_centres = Some(
                race.racers.rivals
                    .iter()
                    .map(|r| {
                        r.state.position + r.state.orientation * Vec3::from(r.car.car_type.handling.centre_of_mass)
                    })
                    .collect(),
            );
        }
        for r in &race.racers.rivals {
            assert!((r.state.orientation * Vec3::Z).z > 0.95, "preserve upright momentum handoff");
        }
        if race.timing.clock >= 0.0 {
            return Handoff {
                scene: name,
                cut,
                slots,
                at_cut: at_cut.unwrap(),
                at_green: poses(),
                cut_centres: cut_centres.unwrap(),
                targets,
                progress: race.racers.rivals.iter().map(|r| r.progress.along).collect(),
            };
        }
    }
    panic!("no green");
}

/// Original assets (ignored without NFSU2_ROOT); `Game::new` isolates
/// XDG_DATA_HOME under `cfg(test)`.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn intro_skip_preserves_ai_grid_handoff() {
    crate::tests::testkit::game();
    let mut errors = Vec::new();
    for scene_number in [3, 25] {
        for hz in [60.0, 240.0] {
            let full = run(scene_number, None, hz);
            eprintln!("FULL hz={hz} {full:?}");
            for fraction in [0.0, 0.1, 0.5, 0.95] {
                let skipped = run(scene_number, Some(fraction), hz);
                assert_eq!(full.scene, skipped.scene);
                assert_eq!(full.slots, skipped.slots);
                for i in 0..full.slots.len() {
                    let cut_gap = full.at_cut[i].truncate().distance(skipped.at_cut[i].truncate());
                    let green_gap = full.at_green[i].truncate().distance(skipped.at_green[i].truncate());
                    let progress_gap = (full.progress[i] - skipped.progress[i]).abs();
                    assert!(progress_gap < 0.5, "seek progress disagrees with complete intro: {progress_gap}");
                    let target_gap = skipped.cut_centres[i].truncate().distance(skipped.targets[i].truncate());
                    assert!(target_gap <= 0.001, "post-key update moved car off authored target: {target_gap}");
                    eprintln!(
                        "SKIP hz={hz} fraction={fraction} scene={} cut={} slot={} cut_gap={cut_gap} green_gap={green_gap} progress_gap={progress_gap} target_gap={target_gap} cut_pose={:?} green_pose={:?}",
                        full.scene, full.cut, full.slots[i], skipped.at_cut[i], skipped.at_green[i]
                    );
                    if cut_gap > 0.5 || green_gap > 0.5 {
                        errors.push((hz, fraction, i, cut_gap, green_gap));
                    }
                }
            }
        }
    }
    assert!(errors.is_empty(), "intro-skip AI handoff mismatch: {errors:?}");
}


