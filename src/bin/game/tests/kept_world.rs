//! A world kept for the next drive draws what a fresh load of the same spot
//! draws: Lamp Shadows' layers and Light Probes' field follow the new
//! drive's barriers (docs/ambient-detail.md "Kept-world caches",
//! docs/lamps-shadows.md, docs/lighting2.md).
use super::*;

const SIZE: (u32, u32) = (320, 240);

fn frame(game: &mut Game) {
    game.step_for(1.0 / 60.0).unwrap();
    game.draw().unwrap();
}

/// The frame just drawn, read back once its end is submitted (as
/// `first_use::picture`).
fn picture(game: &mut Game) -> Vec<u8> {
    let w = game.platform.window.as_mut().unwrap();
    w.settle();
    nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), SIZE.0, SIZE.1)
}

/// Offscreen at the High preset (Lamp Shadows on) with Light Probes off
/// (their field after a switch is checked on its own,
/// [`light_probes_follow_a_kept_drives_barriers`]).
fn game_high() -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("KEPT_WORLD_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    nfsu2::qol::Preset::Modern.apply(&mut game.settings.enh);
    game.settings.enh.light_probes = false;
    game.settings.apply_enhancements();
    assert!(game.settings.enh.lamp_shadows > 0, "Lamp Shadows on at High");
    game
}

/// The lamps holding a shadow layer, with when each layer was drawn.
fn layers(game: &Game) -> Vec<(usize, u64)> {
    let l = game.render.world.as_ref().and_then(|r| r.lamp_shadow_layers()).expect("Lamp Shadows' layers");
    l.holders.iter().zip(&l.drawn).filter_map(|(h, &d)| h.map(|h| (h, d))).collect()
}

/// Each lamp's shadow layer as drawn now (its depth), by lamp.
fn layer_depths(game: &mut Game) -> std::collections::BTreeMap<usize, Vec<u8>> {
    let w = game.platform.window.as_mut().unwrap();
    w.settle();
    let gpu = w.gpu.clone();
    let r = game.render.world.as_ref().unwrap();
    let l = r.lamp_shadow_layers().expect("Lamp Shadows' layers");
    let held: Vec<(usize, usize)> =
        l.holders.iter().enumerate().filter_map(|(layer, h)| h.map(|h| (h, layer))).collect();
    held.into_iter()
        .map(|(lamp, layer)| (lamp, r.lamp_shadow_layer_depth(&gpu, layer as u32).expect("a layer")))
        .collect()
}

/// Each placement's flags and transform (as bits).
fn placements(game: &Game) -> Vec<(u16, [u32; 16])> {
    let w = &game.sim.drive.as_ref().unwrap().world;
    w.placements.iter().map(|p| (p.flags, p.transform.to_cols_array().map(f32::to_bits))).collect()
}

/// What [`kept_and_afresh`] saw.
struct Seen {
    /// Layers held over the switch: drawn again, kept as drawn.
    redrawn: usize,
    kept: usize,
    /// The frame a second after the switch, and the same with the lamps'
    /// state dropped after it.
    frame: Vec<u8>,
    afresh: Vec<u8>,
    /// The lamps holding a layer in both, and those whose layer's depth
    /// differs between them.
    compared: usize,
    maps_differ: Vec<usize>,
}

/// `first` (a drive held for a second: its lamps' layers drawn), then
/// `second` (a drive on the spot on the world kept): the layers of lamps
/// whose view holds a placement the switch changed and the layers draw are
/// drawn again in the next frames, the others are kept as drawn. The frame a
/// second later (the layers handed out again faded in), and the same frame
/// of the same run whose lamps start afresh after the switch
/// (`WorldRenderer::lamps_afresh`: every layer handed out and drawn anew, as
/// a fresh load's renderer has them).
fn kept_and_afresh(what: &str, first: &dyn Fn(&mut Game), second: &dyn Fn(&mut Game)) -> Seen {
    let mut game = game_high();
    first(&mut game);
    for _ in 0..60 {
        frame(&mut game);
    }
    let before = layers(&game);
    let last_draw = before.iter().map(|l| l.1).max().unwrap_or(0);
    let was = placements(&game);
    // The bounds as the layers were drawn (a switch can move a placement:
    // the track's world animations).
    let r = game.render.world.as_ref().unwrap();
    let was_bounds: Vec<Vec<(Vec3, f32)>> = (0..was.len()).map(|p| r.lamp_shadow_bounds(p)).collect();
    second(&mut game);
    let now = placements(&game);
    // Shown or moved, or hidden, by the switch (the hidden flag 0x10 and
    // the transform are what the layers draw by).
    let switched: Vec<usize> = (0..was.len().min(now.len()))
        .filter(|&i| {
            let (a, b) = (was[i], now[i]);
            (a.0 & 0x10 == 0 || b.0 & 0x10 == 0) && ((a.0 ^ b.0) & 0x10 != 0 || a.1 != b.1)
        })
        .collect();
    assert!(!switched.is_empty(), "{what}: the set-up switched placements");
    let r = game.render.world.as_ref().unwrap();
    let bounds: Vec<(Vec3, f32)> = switched
        .iter()
        .flat_map(|&p| r.lamp_shadow_bounds(p).into_iter().chain(was_bounds[p].iter().copied()))
        .collect();
    let sees: std::collections::BTreeMap<usize, bool> =
        before.iter().map(|&(l, _)| (l, r.lamp_view_holds(l, &bounds))).collect();
    // The switch's first frames: each layer given up is handed out again
    // (at most a few a tick) and drawn.
    for _ in 0..10 {
        frame(&mut game);
    }
    let after = layers(&game);
    let (mut redrawn, mut kept) = (0, 0);
    for (lamp, drawn) in &before {
        let Some(&(_, now)) = after.iter().find(|a| a.0 == *lamp) else { continue };
        if sees[lamp] {
            assert!(
                now > last_draw,
                "{what}, lamp {lamp}: a switched placement in its view, its layer not drawn again"
            );
            redrawn += 1;
        } else {
            assert_eq!(now, *drawn, "{what}, lamp {lamp}: nothing switched in its view, its layer drawn again");
            kept += 1;
        }
    }
    for _ in 0..50 {
        frame(&mut game);
    }
    let kept_frame = picture(&mut game);
    let kept_maps = layer_depths(&mut game);
    drop(game);
    let mut game = game_high();
    first(&mut game);
    for _ in 0..60 {
        frame(&mut game);
    }
    second(&mut game);
    game.render.world.as_mut().unwrap().lamps_afresh();
    for _ in 0..60 {
        frame(&mut game);
    }
    let afresh = picture(&mut game);
    let afresh_maps = layer_depths(&mut game);
    let both: Vec<usize> = kept_maps.keys().copied().filter(|l| afresh_maps.contains_key(l)).collect();
    let maps_differ = both.iter().copied().filter(|l| kept_maps[l] != afresh_maps[l]).collect();
    Seen { redrawn, kept, frame: kept_frame, afresh, compared: both.len(), maps_differ }
}

/// A Quick Race of `mode` on `track` with the PEUGOT.
fn quick_race(game: &mut Game, mode: screens::QrMode, track: u16) {
    let ty = nfsu2::frontend::quick_race::race_type(mode);
    game.quick.tracks = game.quick_track_select(mode, 0.0);
    game.settings.qr[ty].track = track;
    game.settings.qr[ty].reversed = false;
    let e = game.quick_event(mode);
    game.quick.car = Some("PEUGOT".into());
    game.start_race(e, true, false).unwrap();
}

/// Lamp Shadows' layers over a kept drive's barrier switch, on the spot:
/// free roam → a city circuit → free roam where its car stands (the drive
/// kept: the same car and region; the barriers the switch shows and hides
/// there are blended, barrier tape and the start line's graphics, which no
/// layer draws, so every layer is kept), and Quick Races from one grid on
/// the world kept: L4RG's URL circuits 4701 → 4703 and L4RB's drag strips
/// 4222 → 4223, whose set-ups switch pieces some lamps' layers draw (those
/// are drawn again, the others kept). Each time a second
/// later the frame equals the frame with the lamps started afresh after the
/// switch, and each lamp's layer holds the depth the layer drawn afresh
/// holds, at the High preset.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn lamp_layers_follow_a_kept_drives_barriers() {
    super::test_isolation::exclusive();
    let circuit = {
        let game = Game::new().unwrap();
        let data = &game.game_data;
        data.career
            .stage_events(&data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .expect("a city circuit")
    };
    let spot = std::sync::Mutex::new((Vec3::ZERO, 0.0f32));
    let city = kept_and_afresh(
        "free roam after a city circuit",
        &|g| {
            g.explore(None).unwrap();
            for _ in 0..5 {
                frame(g);
            }
            g.start_race(circuit, false, false).unwrap();
            let d = g.sim.drive.as_ref().unwrap();
            let f = d.state.forward();
            *spot.lock().unwrap() = (d.state.position, f.y.atan2(f.x));
        },
        &|g| {
            let (at, heading) = *spot.lock().unwrap();
            g.explore(Some((at, heading))).unwrap();
        },
    );
    let url = kept_and_afresh("a URL circuit after another", &|g| quick_race(g, screens::QrMode::Url, 4701), &|g| {
        quick_race(g, screens::QrMode::Url, 4703)
    });
    let drag = kept_and_afresh("a drag strip after another", &|g| quick_race(g, screens::QrMode::Drag, 4222), &|g| {
        quick_race(g, screens::QrMode::Drag, 4223)
    });
    let all = [
        ("free roam after a city circuit", &city),
        ("a URL circuit after another", &url),
        ("a drag strip after another", &drag),
    ];
    for (what, s) in all {
        let differ = s.frame.chunks_exact(4).zip(s.afresh.chunks_exact(4)).filter(|(a, b)| a != b).count();
        eprintln!(
            "{what}: layers drawn again {}, kept {}; {differ} pixels differ; layers compared {}, differ {:?}",
            s.redrawn, s.kept, s.compared, s.maps_differ
        );
        assert!(
            s.compared > 0 && s.maps_differ.is_empty(),
            "{what}: lamps' layers differ from those drawn afresh: {:?}",
            s.maps_differ
        );
        assert!(differ == 0, "{what}: the frame differs from the one with the lamps afresh in {differ} pixels");
    }
    assert!(city.redrawn == 0 && city.kept > 0, "the city's switch keeps every layer");
    for (what, s) in [("the URL circuits", &url), ("the drag strips", &drag)] {
        assert!(s.redrawn > 0 && s.kept > 0, "{what}: layers drawn again {}, kept {}: both kinds", s.redrawn, s.kept);
    }
}

/// Offscreen with Light Probes on (the rest as the defaults).
fn game_probes() -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("KEPT_WORLD_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
    game.settings.enh.parse("enh.light_probes", "1");
    game.settings.apply_enhancements();
    game
}

/// What [`probes_kept_and_afresh`] saw.
struct ProbesSeen {
    /// Probes the second drive's set-up gave up.
    released: usize,
    /// Probes baked in both fields, and those whose cubes differ.
    compared: usize,
    differ: Vec<usize>,
}

/// The probe field a second after `second` (a drive on the world kept
/// after `first`, held for a second), and the same with the field dropped
/// after the switch (`WorldRenderer::probes_afresh`: made and baked anew,
/// as a fresh load's renderer has it), compared probe by probe.
fn probes_kept_and_afresh(first: &dyn Fn(&mut Game), second: &dyn Fn(&mut Game)) -> ProbesSeen {
    let run = |afresh: bool| {
        let mut game = game_probes();
        first(&mut game);
        for _ in 0..60 {
            frame(&mut game);
        }
        second(&mut game);
        let r = game.render.world.as_mut().unwrap();
        assert!(r.probes_released().is_some(), "a probe field after the first drive");
        if afresh {
            r.probes_afresh();
        }
        for _ in 0..60 {
            frame(&mut game);
        }
        game.platform.window.as_mut().unwrap().settle();
        let released = game.render.world.as_ref().unwrap().probes_released().unwrap_or(0);
        (released, nfsu2::render::probes::published_baked().expect("the probe field"))
    };
    let (released, kept) = run(false);
    let (_, afresh) = run(true);
    assert_eq!(kept.len(), afresh.len());
    let both: Vec<usize> = (0..kept.len()).filter(|&i| kept[i].is_some() && afresh[i].is_some()).collect();
    let differ = both.iter().copied().filter(|&i| kept[i] != afresh[i]).collect();
    ProbesSeen { released, compared: both.len(), differ }
}

/// Light Probes' field over a kept drive's set-up switch: Quick Races from
/// one grid on the world kept (L4RG's URL circuits 4701 → 4703, L4RB's
/// drag strips 4222 → 4223, whose set-ups switch road pieces, walls and
/// markers by the grid) give up the probes that see a switched placement
/// and bake them again; free roam → a city circuit → free roam where its
/// car stands (the city's common transition) gives up none. Each time the
/// probes baked a second later are those of the field made afresh after the
/// switch, probe by probe.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn light_probes_follow_a_kept_drives_barriers() {
    super::test_isolation::exclusive();
    let circuit = {
        let game = Game::new().unwrap();
        let data = &game.game_data;
        data.career
            .stage_events(&data.root, 1)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Circuit
                    && data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .expect("a city circuit")
    };
    let spot = std::sync::Mutex::new((Vec3::ZERO, 0.0f32));
    let city = probes_kept_and_afresh(
        &|g| {
            g.explore(None).unwrap();
            for _ in 0..5 {
                frame(g);
            }
            g.start_race(circuit, false, false).unwrap();
            let d = g.sim.drive.as_ref().unwrap();
            let f = d.state.forward();
            *spot.lock().unwrap() = (d.state.position, f.y.atan2(f.x));
        },
        &|g| {
            let (at, heading) = *spot.lock().unwrap();
            g.explore(Some((at, heading))).unwrap();
        },
    );
    let url = probes_kept_and_afresh(&|g| quick_race(g, screens::QrMode::Url, 4701), &|g| {
        quick_race(g, screens::QrMode::Url, 4703)
    });
    let drag = probes_kept_and_afresh(&|g| quick_race(g, screens::QrMode::Drag, 4222), &|g| {
        quick_race(g, screens::QrMode::Drag, 4223)
    });
    let all = [
        ("free roam after a city circuit", &city),
        ("a URL circuit after another", &url),
        ("a drag strip after another", &drag),
    ];
    for (what, s) in all {
        eprintln!(
            "{what}: probes given up {}, compared {}, differ {} {:?}",
            s.released,
            s.compared,
            s.differ.len(),
            &s.differ[..s.differ.len().min(12)]
        );
    }
    for (what, s) in all {
        assert!(s.compared > 0, "{what}: no probe baked in both fields");
        assert!(s.differ.is_empty(), "{what}: {} probes differ from the field made afresh", s.differ.len());
    }
    assert_eq!(city.released, 0, "the city's switch gives up no probe");
    for (what, s) in [("the URL circuits", &url), ("the drag strips", &drag)] {
        assert!(s.released > 0, "{what}: the set-up gives up probes");
    }
}

/// The GPU time Light Probes take (GPU timestamps, `prof::gpu`) in a kept
/// drive's first frame and its next nine, after the set-ups of
/// [`light_probes_follow_a_kept_drives_barriers`] and after the same
/// set-up again (nothing switched). A measuring aid (docs/lighting2.md).
#[test]
#[ignore = "a measuring aid (GPU timestamps); run with --ignored and NFSU2_ROOT"]
fn light_probes_rebake_cost() {
    super::test_isolation::exclusive();
    nfsu2::prof::gpu::want(true);
    let probe_ms = |game: &mut Game| {
        // The frame's timing closed at its end.
        game.platform.window.as_mut().unwrap().settle();
        let gpu = &game.platform.window.as_ref().unwrap().gpu;
        nfsu2::prof::gpu::finish(&gpu.device);
        nfsu2::prof::gpu::take().into_iter().filter(|e| e.0 == "world: light probes").map(|e| e.1).sum::<f64>()
    };
    let cases: [(&str, (screens::QrMode, u16), (screens::QrMode, u16)); 4] = [
        ("URL 4701 -> 4703", (screens::QrMode::Url, 4701), (screens::QrMode::Url, 4703)),
        ("URL 4701 -> 4701", (screens::QrMode::Url, 4701), (screens::QrMode::Url, 4701)),
        ("drag 4222 -> 4223", (screens::QrMode::Drag, 4222), (screens::QrMode::Drag, 4223)),
        ("drag 4222 -> 4222", (screens::QrMode::Drag, 4222), (screens::QrMode::Drag, 4222)),
    ];
    for (what, a, b) in cases {
        let mut game = game_probes();
        {
            let gpu = &game.platform.window.as_ref().unwrap().gpu;
            assert!(nfsu2::prof::gpu::init(&gpu.device, &gpu.queue), "GPU timestamps on this device");
        }
        quick_race(&mut game, a.0, a.1);
        for _ in 0..60 {
            frame(&mut game);
        }
        probe_ms(&mut game);
        quick_race(&mut game, b.0, b.1);
        frame(&mut game);
        let first = probe_ms(&mut game);
        for _ in 0..9 {
            frame(&mut game);
        }
        let next = probe_ms(&mut game);
        let released = game.render.world.as_ref().unwrap().probes_released().unwrap_or(0);
        // The drive start's check with nothing switched (the set-up again).
        let world = &game.sim.drive.as_ref().unwrap().world;
        let r = game.render.world.as_mut().unwrap();
        let t = std::time::Instant::now();
        r.new_drive_probes(world, true);
        let check = t.elapsed().as_secs_f64() * 1e3;
        eprintln!(
            "PROBE_COST {what}: given up {released}; first frame {first:.3} ms, next 9 frames {next:.3} ms; \
             the check alone {check:.3} ms CPU ({} placements)",
            world.placements.len()
        );
    }
}
