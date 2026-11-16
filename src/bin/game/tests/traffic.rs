use crate::*;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn traffic_lane_keeping() {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    // A career's free roam (see `traffic_probe`): the stage's wall-offs
    // count for the player alone.
    let mut profile = Profile::new("Lanes");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let mut offs = Vec::new();
    for _ in 0..3600 {
        game.advance_for(1.0 / 60.0).unwrap();
        let t = game.sim.traffic.as_ref().unwrap();
        for c in t.cars.iter().filter(|c| c.mover.is_some()) {
            if let Some(lane) = c.lane.as_ref().filter(|l| !l.avoiding()) {
                offs.extend(lane.off_centre(&t.routes, c.state.position.truncate()).map(f32::abs));
            }
        }
    }
    offs.sort_by(f32::total_cmp);
    let q = |p: f64| offs[((offs.len() - 1) as f64 * p) as usize];
    let out = offs.iter().filter(|&&o| o > 1.5).count() as f64 / offs.len() as f64;
    println!(
        "lane offsets over {} car-frames: median {:.2} m, p90 {:.2}, p99 {:.2}, max {:.2}; over 1.5 m {:.1}%",
        offs.len(),
        q(0.5),
        q(0.9),
        q(0.99),
        q(1.0),
        out * 100.0
    );
    assert!(offs.len() > 1000 && q(0.5) < 0.6 && q(0.9) < 1.0 && out < 0.05);
}

/// A free roam load that takes the world kept in memory (a car change
/// dropped the drive after a city race) leaves the same world as the
/// first, fresh load: every field of the cache's dump.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn resident_world_reload_equals_fresh_load() {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("Player");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.explore(None).unwrap();
    let fresh = game.sim.drive.as_ref().unwrap().world.to_sections();
    let race = game
        .game_data.career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| {
            e.kind == career::EventKind::Circuit
                && game.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
        })
        .map(|e| e.index)
        .unwrap();
    game.start_race(race, false, false).unwrap();
    for _ in 0..120 {
        game.advance_for(1.0 / 60.0).unwrap();
    }
    game.explore(None).unwrap();
    let at = game.here();
    game.drop_drive();
    assert!(game.loads.resident.is_some());
    game.explore(at).unwrap();
    assert!(game.loads.resident.is_none(), "the kept world was taken");
    let again = game.sim.drive.as_ref().unwrap().world.to_sections();
    assert!(
        fresh.len() == again.len() && fresh.iter().zip(&again).all(|(a, b)| a.0 == b.0 && a.1 == b.1),
        "the resident reload differs from the fresh load"
    );
}

/// Free-roam traffic probe (run by hand: `cargo test --release --bin
/// game traffic_probe -- --ignored --nocapture`; `PROBE_SECONDS`, default
/// 120): the player stands at the free-roam start while the traffic
/// runs; per car it logs spawns / despawns with the model, hits (the
/// TrafficMover handed to the MeloMvr), wall and car contacts, teleports,
/// long stops, junction turns and lane changes, and sums up the lane
/// offsets, heading errors and speeds against the lane's.
/// Race traffic as measured under Wine (`cargo test --release --bin game
/// race_traffic_probe -- --ignored --nocapture`; `PROBE_MODE` circuit,
/// sprint or drag, `PROBE_LEVEL` 0–3, `PROBE_SECONDS`): a Quick Race on
/// the mode's first track with the player standing on the grid, the
/// placed traffic cars counted every frame (1/20 s, the Wine copy's
/// rate) and their distances from the player.
/// Diagnostic only: reports rival rollover onset without changing physics.
#[test]
#[ignore = "requires game assets; diagnostic, not a native parity assertion"]
fn drag_rival_rollover_probe() {
    assert!(std::env::var_os("NFSU2_ROOT").is_some());
    let mut game = Game::new().unwrap();
    game.rng = nfsu2::opponents::GameRng(20261103);
    game.quick.tracks = game.quick_track_select(screens::QrMode::Drag, 0.0);
    let tracks: Vec<_> = game.quick.tracks.as_ref().unwrap().tracks.iter().map(|t| t.number).collect();
    println!("drag regression tracks={tracks:?}");
    if let Ok(track) = std::env::var("ROLLOVER_TRACK") {
        let track: u16 = track.parse().expect("numeric ROLLOVER_TRACK");
        assert!(tracks.contains(&track), "track must belong to drag mode");
        game.settings.qr[nfsu2::frontend::quick_race::race_type(screens::QrMode::Drag)].track = track;
    }
    let event = game.quick_event(screens::QrMode::Drag);
    game.start_race(event, true, false).unwrap();
    let hz: f32 = std::env::var("ROLLOVER_HZ").ok().and_then(|s| s.parse().ok()).unwrap_or(60.0);
    assert!(hz.is_finite() && hz >= 20.0 && hz <= 1000.0);
    let mut reported = std::collections::BTreeSet::new();
    let mut pregreen_rollovers = std::collections::BTreeSet::new();
    let mut reached_green = false;
    let mut history = std::collections::VecDeque::new();
    for frame in 0..(hz * 60.0) as usize {
        game.advance_for(1.0 / hz).unwrap();
        let Mode::Race(race) = &game.mode else { break };
        assert_eq!(race.racers.rivals.len(), 3, "regression requires three opponents");
        reached_green |= race.timing.clock >= 0.0;
        let rows: Vec<_> = race
            .racers.rivals
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let upright = (r.state.orientation * Vec3::Z).z;
                (
                    i,
                    r.car.name.clone(),
                    r.state.position,
                    r.state.velocity,
                    upright,
                    r.state.wheels_down(),
                    r.state.totaled,
                    r.progress.dropped,
                    r.state.spin,
                    r.state.angular_momentum,
                    r.state.hardest_impulse,
                    r.state.contacts.clone(),
                )
            })
            .collect();
        history.push_back(format!("frame={frame} clock={} rows={rows:?}", race.timing.clock));
        if history.len() > 30 {
            history.pop_front();
        }
        for row in &rows {
            if row.4 < 0.5 && race.timing.clock < 0.0 {
                pregreen_rollovers.insert(row.0);
            }
            if row.4 < 0.5 && reported.insert(row.0) {
                println!(
                    "ROLLOVER track={} hz={hz} rival={}\n{}",
                    game.game_data.career.events[race.event].track,
                    row.0,
                    history.iter().cloned().collect::<Vec<_>>().join("\n")
                );
            }
        }
    }
    assert!(reached_green, "must exercise scene-to-physics handoff");
    assert!(pregreen_rollovers.is_empty(), "stationary rivals rolled during countdown: {pregreen_rollovers:?}");
    println!(
        "diagnostic completed hz={hz}, rivals tilting beyond60deg={reported:?}; player remains on grid, not a reproduction of user driving"
    );
}

#[test]
#[ignore = "probe: prints traffic in a race"]
fn race_traffic_probe() {
    crate::tests::testkit::game();
    let level: u8 = std::env::var("PROBE_LEVEL").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let secs: f32 = std::env::var("PROBE_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(120.0);
    let (mode, ty) = match std::env::var("PROBE_MODE").as_deref() {
        Ok("sprint") => (screens::QrMode::Sprint, 1),
        Ok("drag") => (screens::QrMode::Drag, 2),
        _ => (screens::QrMode::Circuit, 0),
    };
    let mut game = Game::new().unwrap();
    game.settings.qr[ty].traffic = level;
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::QuickRace, MenuKind::Main).unwrap();
    game.act(Action::QrMode(mode), MenuKind::QrMode).unwrap();
    game.menu_key(Key::Accept).unwrap();
    game.menu_key(Key::Accept).unwrap();
    game.menu_key(Key::Accept).unwrap();
    if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
        game.menu_key(Key::Accept).unwrap();
    }
    let Mode::Race(r) = &game.mode else { panic!("no race ({})", mode_name(&game.mode)) };
    println!(
        "track {} level {level} pool {}",
        game.game_data.career.events.get(r.event).map_or(0, |e| e.track),
        r.racers.traffic.as_ref().map_or(0, |t| t.cars.len())
    );
    let (mut counts, mut dists, mut placements) = (Vec::new(), Vec::new(), 0usize);
    let mut was = Vec::new();
    let mut first_green = None;
    for frame in 0..(secs * 20.0) as usize {
        game.advance_for(1.0 / 20.0).unwrap();
        let Mode::Race(r) = &game.mode else { break };
        let Some(t) = r.racers.traffic.as_ref() else { break };
        let me = game.sim.drive.as_ref().unwrap().state.position.truncate();
        let now: Vec<Option<Vec2>> =
            t.cars.iter().map(|c| c.lane.is_some().then(|| c.state.position.truncate())).collect();
        if was.len() == now.len() {
            placements += now
                .iter()
                .zip(&was)
                .filter(|(n, w): &(&Option<Vec2>, &Option<Vec2>)| {
                    n.is_some() && (w.is_none() || n.unwrap().distance(w.unwrap()) > 10.0)
                })
                .count();
        }
        was = now.clone();
        if r.timing.clock > 0.0 && first_green.is_none() {
            first_green = Some(frame);
            println!("green at {:.1} s: {} placed", frame as f32 / 20.0, now.iter().flatten().count());
        }
        if frame % std::env::var("PROBE_EVERY").ok().and_then(|s| s.parse().ok()).unwrap_or(200usize) == 0 {
            let placed: Vec<&TrafficCar> = t.cars.iter().filter(|c| c.lane.is_some()).collect();
            let mut d: Vec<(u32, bool, f32, bool, f32, &str)> = placed
                .iter()
                .map(|c| {
                    let l = c.lane.as_ref().unwrap();
                    (
                        c.state.position.truncate().distance(me) as u32,
                        c.mover.is_some(),
                        c.mover.map_or(c.state.velocity.length(), |m| m.speed),
                        c.stopped,
                        c.plan.as_ref().map_or(0.0, |p| p.1),
                        if l.avoiding() {
                            "A"
                        } else if l.obeying() {
                            "O"
                        } else {
                            ""
                        },
                    )
                })
                .collect();
            d.sort_by_key(|x| x.0);
            // Each car: distance, * on the MeloMvr, S stopped for good
            // (+0x710), A avoiding / O at an intersection, speed /
            // target.
            println!(
                "{:5.1} s: {} placed, {} under 1 m/s, {} stopped; {:?}",
                frame as f32 / 20.0,
                placed.len(),
                d.iter().filter(|x| x.2 < 1.0).count(),
                d.iter().filter(|x| x.3).count(),
                d.iter()
                    .map(|x| format!(
                        "{}{}{}{} {:.1}/{:.0}",
                        x.0,
                        if x.1 { "" } else { "*" },
                        if x.3 { "S" } else { "" },
                        x.5,
                        x.2,
                        x.4
                    ))
                    .collect::<Vec<_>>()
            );
        }
        counts.push(now.iter().flatten().count());
        dists.extend(now.iter().flatten().map(|p| p.distance(me)));
    }
    dists.sort_by(f32::total_cmp);
    let q = |v: &[f32], f: f32| v.get(((v.len() as f32 - 1.0) * f) as usize).copied().unwrap_or(0.0);
    let mean = counts.iter().sum::<usize>() as f32 / counts.len().max(1) as f32;
    println!(
        "placed: mean {mean:.1} min {} max {}; placements {placements}; distance p10 {:.0} median {:.0} p90 {:.0}",
        counts.iter().min().unwrap_or(&0),
        counts.iter().max().unwrap_or(&0),
        q(&dists, 0.1),
        q(&dists, 0.5),
        q(&dists, 0.9)
    );
}

/// The traffic circling detector (player report: "sometimes
/// they drive in a circle"): per placed car, a 10 s window of its
/// unwrapped heading; a car whose heading turns more than 300° within
/// the window while it moves (mean speed over 1 m/s), or that turns
/// over 300° within 25 m of where it was 10 s earlier, is circling. The
/// lane's own direction over the same window is logged, so a lane that
/// loops (a ramp) tells itself apart.
struct CircleWatch {
    /// Per car: (time, unwrapped heading, speed, position, unwrapped lane direction).
    hist: Vec<std::collections::VecDeque<(f32, f32, f32, Vec2, f32)>>,
    /// Per car: the time it was last reported (one report per 10 s).
    last: Vec<f32>,
    found: Vec<String>,
    spins: usize,
}

impl CircleWatch {
    const WINDOW: f32 = 10.0;
    const TURN: f32 = 300.0;
    fn new(n: usize) -> CircleWatch {
        CircleWatch { hist: vec![Default::default(); n], last: vec![f32::NEG_INFINITY; n], found: Vec::new(), spins: 0 }
    }

    fn watch(&mut self, t: &Traffic, ts: f32, place: &str) {
        for (i, c) in t.cars.iter().enumerate() {
            let h = &mut self.hist[i];
            let Some(lane) = c.lane.as_ref() else {
                h.clear();
                continue;
            };
            let p = c.state.position.truncate();
            let heading = c.mover.map_or(c.state.heading(), |m| m.heading);
            let speed = c.mover.map_or(c.state.velocity.truncate().length(), |m| m.speed);
            let dir = lane.current(&t.routes).map_or(0.0, |x| x.4.y.atan2(x.4.x));
            // SOAK_TRACE=place-prefix,car,t0,t1: that car's walk per update
            // (a debugging aid).
            if let Ok(v) = std::env::var("SOAK_TRACE") {
                let f: Vec<&str> = v.split(',').collect();
                if f.len() == 4
                    && place.starts_with(f[0])
                    && f[1].parse() == Ok(i)
                    && ts >= f[2].parse().unwrap_or(0.0)
                    && ts <= f[3].parse().unwrap_or(0.0)
                {
                    if let Some(k) = lane.avoid_target() {
                        if let Some(o) = t.cars.get(k.wrapping_sub(0)).filter(|_| k < t.cars.len()) {
                            println!(
                                "  target {k}: at {:.1} {:.1} v {:.1} stopped {} mover {} belt {} melo {} walk {}",
                                o.state.position.x,
                                o.state.position.y,
                                o.belt.belt.as_ref().map_or(o.state.speed(), |b| b.speed()),
                                o.stopped,
                                o.mover.is_some(),
                                o.belt.belt.is_some(),
                                o.state.melo.is_some(),
                                o.lane.as_ref().map_or(String::new(), |l| l.walk_desc())
                            );
                        } else {
                            println!("  target {k} (not a traffic car)");
                        }
                    }
                    println!(
                        "TRACE {ts:7.2} car {i} at {:.1} {:.1} z {:.2} fz {:.2} upz {:.2} wz {:.2} con {} hdg {:.0} v {speed:.1} mover {} melo {} avoid {} obey {} tgt {:?} steer {:.1} walk {}",
                        p.x,
                        p.y,
                        c.state.position.z,
                        c.state.forward().z,
                        (c.state.orientation * Vec3::Z).z,
                        c.state.spin.z,
                        c.state.contacts.len(),
                        heading.to_degrees(),
                        c.mover.is_some(),
                        c.state.melo.is_some(),
                        lane.avoiding(),
                        lane.obeying(),
                        c.plan.as_ref().map(|pl| (pl.0.iter().take(3).map(|q| (q.x as i32, q.y as i32)).collect::<Vec<_>>(), pl.1)),
                        c.controls.steer_degrees,
                        lane.walk_desc()
                    );
                }
            }
            let wrap = |a: f32| a.sin().atan2(a.cos());
            let (hu, du) = match h.back() {
                // A jump (a new placement) starts over.
                Some(&(_, ph, _, pp, pd)) if pp.distance(p) < 10.0 => (ph + wrap(heading - ph), pd + wrap(dir - pd)),
                Some(_) => {
                    h.clear();
                    (heading, dir)
                }
                None => (heading, dir),
            };
            h.push_back((ts, hu, speed, p, du));
            while h.front().is_some_and(|x| ts - x.0 > Self::WINDOW) {
                h.pop_front();
            }
            if ts - self.last[i] < Self::WINDOW || h.len() < 2 {
                continue;
            }
            let (lo, hi) = h.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |a, x| (a.0.min(x.1), a.1.max(x.1)));
            let turned = (hi - lo).to_degrees();
            if turned <= Self::TURN {
                continue;
            }
            let mean = h.iter().map(|x| x.2).sum::<f32>() / h.len() as f32;
            let back = h.front().unwrap().3.distance(p);
            if mean < 1.0 && back > 25.0 {
                continue;
            }
            let (dlo, dhi) = h.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |a, x| (a.0.min(x.4), a.1.max(x.4)));
            self.last[i] = ts;
            let cur = lane.current(&t.routes);
            let target = c.plan.as_ref().map(|pl| (pl.0.first().map(|q| q.truncate()), pl.1));
            let to = target.and_then(|x| x.0).map(|q| {
                let d = q - p;
                let rel = wrap(d.y.atan2(d.x) - heading).to_degrees();
                (d.length(), rel)
            });
            let s = format!(
                "{place} {ts:7.1} s car {i:2} {} circling: turned {turned:.0}° in {:.1} s, mean {mean:.1} m/s, {back:.0} m from its place 10 s ago, at {:.0} {:.0}; mover {} stopped {} avoid {} obey {}; lane step {:?} walk {} lane turned {:.0}°; target point {to:?} (m, ° off heading) speed {:?}; steer {:.1}° gas {:.2} brake {:.2}; contacts {:?}",
                c.car.car_type.name,
                ts - h.front().unwrap().0,
                p.x,
                p.y,
                if c.mover.is_some() {
                    "TrafficMover"
                } else if c.state.melo.is_some() {
                    "MeloMvr"
                } else {
                    "physics"
                },
                c.stopped,
                lane.avoiding(),
                lane.obeying(),
                cur.map(|x| (x.0, x.1, x.2, x.3)),
                lane.walk_len(),
                (dhi - dlo).to_degrees(),
                target.map(|x| x.1),
                c.controls.steer_degrees,
                c.controls.gas,
                c.controls.brake,
                c.state.contacts.iter().map(|x| (x.touching, x.impulse)).collect::<Vec<_>>(),
            );
            // A car stopped for good (+0x710) no longer steers: its
            // driver holds target 0 on the heading, so a turn there is
            // a hit's spin on the MeloMvr (logged, not counted).
            if c.stopped {
                println!("(spin after a hit) {s}");
                self.spins += 1;
            } else {
                println!("{s}");
                self.found.push(s);
            }
        }
    }
}

/// The lane pursuit the probes' player drives (as `traffic_probe`'s
/// PROBE_DRIVE): the nearest traffic lane's walk, about 20 m/s.
fn lane_pursuit(game: &mut Game, walk: &mut Option<(usize, usize, bool)>, rng: &mut nfsu2::opponents::GameRng) {
    let Some(t) = game.sim.traffic.as_ref() else { return };
    let f = &t.routes;
    let st = &game.sim.drive.as_ref().unwrap().state;
    let (pos, fwd) = (st.position.truncate(), st.forward().truncate().normalize_or_zero());
    if walk.is_none() {
        *walk = f
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.lanes[2..6].iter().any(|&l| l))
            .flat_map(|(ri, r)| r.points.iter().enumerate().map(move |(ci, c)| (ri, ci, c.position.truncate())))
            .min_by(|a, b| a.2.distance_squared(pos).total_cmp(&b.2.distance_squared(pos)))
            .map(|(r, c, _)| (r, c, f.direction(r, c, false).dot(fwd) < 0.0));
    }
    let mut target = None;
    if let Some(mut w) = *walk {
        while f.along(w.0, w.1, w.2, pos) > f.segment(w.0, w.1, w.2) {
            match nfsu2::ai::traffic::road_walk(f, w.0, w.1, w.2, 0.01, rng) {
                Some(n) => w = n,
                None => break,
            }
        }
        *walk = Some(w);
        let ahead = nfsu2::ai::traffic::road_walk(f, w.0, w.1, w.2, 12.0, &mut rng.clone()).unwrap_or(w);
        let r = &f.records[ahead.0];
        let l = (2..6).find(|&l| r.lanes[nfsu2::formats::ai_routes::lane(l, ahead.2)]).unwrap_or(2);
        let off =
            nfsu2::formats::ai_routes::lane_offset(&r.points[ahead.1], nfsu2::formats::ai_routes::lane(l, ahead.2));
        target = Some(f.point(ahead.0, ahead.1, ahead.2, 0.0, off).truncate());
        // Lost (off the walk by far): take the nearest lane again.
        if target.is_some_and(|t| t.distance(pos) > 60.0) {
            *walk = None;
        }
    }
    let speed = st.velocity.truncate().length();
    let (mut gas, brake) = (speed < 18.0, speed > 23.0);
    let mut steer = 0.0;
    if let Some(tp) = target {
        let to = (tp - pos).normalize_or_zero();
        let a = fwd.y.atan2(fwd.x) - to.y.atan2(to.x);
        steer = (a.sin().atan2(a.cos()) * 3.0).clamp(-1.0, 1.0);
    }
    // Stuck against something: back off.
    if speed < 0.5 && st.wall_contact.is_some() {
        gas = false;
    }
    game.platform.keys.clear();
    for (on, k) in [
        (gas, KeyCode::ArrowUp),
        (brake || (speed < 0.5 && !gas), KeyCode::ArrowDown),
        (steer < -0.15, KeyCode::ArrowLeft),
        (steer > 0.15, KeyCode::ArrowRight),
    ] {
        if on {
            game.platform.keys.insert(k);
        }
    }
}

/// Spread spots on the free-roam traffic lanes (farthest-point sampling
/// over the lanes' control points, from the free-roam start).
/// `near`: only points within that radius of that place (x, y, r).
fn lane_spots(
    f: &nfsu2::formats::ai_routes::RouteFile,
    start: Vec3,
    n: usize,
    near: Option<(Vec2, f32)>,
) -> Vec<(Vec3, f32)> {
    let pos = |&(r, c): &(usize, usize)| f.records[r].points[c].position;
    let pts: Vec<(usize, usize)> = f
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.lanes[2..6].iter().any(|&l| l))
        .flat_map(|(ri, r)| (0..r.points.len()).map(move |ci| (ri, ci)))
        .filter(|p| near.is_none_or(|(c, r)| pos(p).truncate().distance(c) < r))
        .collect();
    let mut chosen: Vec<Vec3> = vec![start];
    let mut out = Vec::new();
    for _ in 0..n {
        let Some(best) = pts.iter().max_by(|a, b| {
            let d = |p: &(usize, usize)| {
                chosen.iter().map(|c| c.truncate().distance(pos(p).truncate())).fold(f32::INFINITY, f32::min)
            };
            d(a).total_cmp(&d(b))
        }) else {
            break;
        };
        let (r, c) = *best;
        let rec = &f.records[r];
        let l = (2..6).map(|l| nfsu2::formats::ai_routes::lane(l, false)).find(|&l| rec.lanes[l]).unwrap_or(2);
        let at = f.point(r, c, false, 0.0, nfsu2::formats::ai_routes::lane_offset(&rec.points[c], l));
        let dir = f.direction(r, c, false);
        chosen.push(pos(best));
        out.push((at + Vec3::Z, dir.y.atan2(dir.x)));
    }
    out
}

/// Long traffic soak with the circling detector (run by hand: `cargo
/// test --release --bin game traffic_circling_soak -- --ignored
/// --nocapture`): `SOAK_SPOTS` spots spread over the free-roam lanes
/// (default 12) for `SOAK_SECONDS` each (default 240), the player
/// standing at even spots and driving the lanes at odd ones, then
/// `SOAK_RACES` Quick Races (default 6: circuit, sprint and drag, the
/// first two tracks of each) at traffic level 3 with the player on the autopilot. Fails on
/// any circling car.
#[test]
#[ignore = "soak: long traffic run"]
fn traffic_circling_soak() {
    crate::tests::testkit::game();
    let env = |k: &str, d: f32| std::env::var(k).ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(d);
    let (spots, secs, races) =
        (env("SOAK_SPOTS", 12.0) as usize, env("SOAK_SECONDS", 240.0), env("SOAK_RACES", 6.0) as usize);
    let found = traffic_soak(spots, secs, races, env("SOAK_RACE_SECONDS", 240.0));
    assert!(found.is_empty(), "{} circling traffic cars:\n{}", found.len(), found.join("\n"));
}

/// A short soak (two spots, 90 s each, one race) for every test run.
///
/// KNOWN DEFECT, open: on the default stream one traffic car (a PARCELVAN)
/// "circles" about 80 s into the race on track 4014; other world streams
/// (`NFSU2_TRAFFIC_SEED` 1-5) pass. Traced (`SOAK_TRACE`,
/// docs/race-ai-parity.md "The traffic walk's turns"): not the walk — the car's walk runs
/// straight down record 11 throughout. The car is on the BeltMover (mode 4,
/// the world under it not loaded) in "Avoid Cars", which asks for 0 (a car
/// on the next lane within 15 m, 0x424CB0); the belt built with target 0
/// decelerates at v0 / 0.5 s (0x5916A0) and its heading, taken from the
/// velocity 0.4 s ahead (0x59EC90 → 0x591B80), turns round 0.1 s after
/// each think's rebuild — the original does the same (observed: belt cars
/// reaching 0 flip by 180° and run backwards). What differs is how often:
/// measured alike (throttle only from the green, docs/race-ai-parity.md
/// "Belt cars in "Avoid Cars"") the clone's race belt cars spend 38 % of their time in "Avoid
/// Cars", 10 % of that under 3 m/s; the original's 9 % and 4 % (four runs,
/// 1.6–17 %). The rules match (0x4145B0, 0x424CB0, 0x41CD10, 0x402ED0 re-read);
/// the clone's belt cars run in denser platoons (open). With the placer starting from the human's road node
/// (the human controller's road node) the test passes on the default stream and on
/// streams 1–5, because other cars are placed; the cause stands (the long
/// soak `traffic_circling_soak` still finds belt avoiders turning round).
/// The test runs in every test run again: if a change of the random streams
/// makes it fail, that is this open defect showing, not the change.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn traffic_does_not_circle() {
    crate::tests::testkit::game();
    let found = traffic_soak(2, 90.0, 1, 90.0);
    assert!(found.is_empty(), "{} circling traffic cars:\n{}", found.len(), found.join("\n"));
}

fn traffic_soak(spots: usize, secs: f32, races: usize, race_secs: f32) -> Vec<String> {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("Soak");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let start = game.sim.drive.as_ref().unwrap().state.position;
    // SOAK_NEAR=x,y,r: the spots around that place (the first one too).
    let near = std::env::var("SOAK_NEAR")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<_>>())
        .filter(|v| v.len() == 3)
        .map(|v| (Vec2::new(v[0], v[1]), v[2]));
    let mut places = lane_spots(
        &game.sim.traffic.as_ref().unwrap().routes,
        near.map_or(start, |n| n.0.extend(0.0)),
        spots.saturating_sub(1) + usize::from(near.is_some()),
        near,
    );
    if near.is_some() && !places.is_empty() {
        game.explore(Some(places.remove(0))).unwrap();
    }
    let mut found = Vec::new();
    let mut rng = nfsu2::opponents::GameRng(11);
    let mut frames_total = 0usize;
    for (k, at) in std::iter::once(None).chain(places.into_iter().map(Some)).enumerate().take(spots) {
        if at.is_some() {
            game.explore(at).unwrap();
        }
        let n = game.sim.traffic.as_ref().unwrap().cars.len();
        let mut w = CircleWatch::new(n);
        let p0 = game.sim.drive.as_ref().unwrap().state.position;
        let place = format!("spot {k} ({:.0} {:.0}, {})", p0.x, p0.y, if k % 2 == 1 { "driving" } else { "standing" });
        let mut walk = None;
        for frame in 0..(secs * 60.0) as usize {
            if k % 2 == 1 {
                lane_pursuit(&mut game, &mut walk, &mut rng);
            } else {
                game.platform.keys.clear();
            }
            game.advance_for(1.0 / 60.0).unwrap();
            if !matches!(game.mode, Mode::Explore) {
                break;
            }
            w.watch(game.sim.traffic.as_ref().unwrap(), frame as f32 / 60.0, &place);
            frames_total += 1;
        }
        println!("{place}: {} circling, {} spins after hits", w.found.len(), w.spins);
        found.extend(w.found);
    }
    game.platform.keys.clear();
    for r in 0..races {
        let (mode, ty) =
            [(screens::QrMode::Circuit, 0), (screens::QrMode::Sprint, 1), (screens::QrMode::Drag, 2)][r % 3];
        game.settings.qr[ty].traffic = 3;
        game.drop_drive();
        game.mode = game.icon_menu(MenuKind::Main);
        game.act(Action::QuickRace, MenuKind::Main).unwrap();
        game.act(Action::QrMode(mode), MenuKind::QrMode).unwrap();
        for _ in 0..r / 3 {
            game.menu_key(Key::Right).unwrap();
        }
        game.menu_key(Key::Accept).unwrap();
        game.menu_key(Key::Accept).unwrap();
        game.menu_key(Key::Accept).unwrap();
        if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
            game.menu_key(Key::Accept).unwrap();
        }
        let Mode::Race(rc) = &game.mode else {
            println!("race {r}: no race ({})", mode_name(&game.mode));
            continue;
        };
        let place = format!("race {r} (event track {})", game.game_data.career.events.get(rc.event).map_or(0, |e| e.track));
        let n = rc.racers.traffic.as_ref().map_or(0, |t| t.cars.len());
        let mut w = CircleWatch::new(n);
        let mut on = false;
        for frame in 0..((race_secs + 30.0) * 60.0) as usize {
            game.advance_for(1.0 / 60.0).unwrap();
            let Mode::Race(rc) = &mut game.mode else { break };
            if !on && rc.timing.clock >= 0.0 {
                if let (Some(routes), Some(d)) = (rc.racers.ai_routes.as_ref(), game.sim.drive.as_ref()) {
                    rc.autopilot = Some(Autopilot::new(d, routes, d.car.car_type.handling.race.drift()));
                    on = true;
                }
            }
            if let Some(t) = rc.racers.traffic.as_ref() {
                w.watch(t, frame as f32 / 60.0, &place);
            }
            frames_total += 1;
        }
        println!("{place}: {} circling, {} spins after hits", w.found.len(), w.spins);
        found.extend(w.found);
    }
    println!("soak: {frames_total} frames, {} circling", found.len());
    found
}

#[test]
#[ignore = "probe: prints free-roam traffic"]
fn traffic_probe() {
    crate::tests::testkit::game();
    let secs: f32 = std::env::var("PROBE_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(120.0);
    let verbose = std::env::var_os("PROBE_VERBOSE").is_some();
    let mut game = Game::new().unwrap();
    // A career's free roam, as measured under Wine: the player's own
    // barrier list then holds BARRIERS_CAREER<stage> (0x5F54B0), so
    // the group counts for the player alone (0x5E7403 / 0x5E76B2).
    // PROBE_FREE_RUN: Quick Race's Free Run instead (no profile, its
    // mode record's level, the track's start).
    if std::env::var_os("PROBE_FREE_RUN").is_some() {
        game.race_flow.quick_free_run = true;
    } else {
        let mut profile = Profile::new("Probe");
        profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        game.profile = Some(profile);
    }
    game.explore(None).unwrap();
    // PROBE_AT=x,y,z[,heading°]: stand there instead (a traffic zone's
    // lane point, as the Wine copy's teleport).
    if let Ok(v) = std::env::var("PROBE_AT") {
        let f: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if f.len() >= 3 {
            game.explore(Some((Vec3::new(f[0], f[1], f[2]), f.get(3).copied().unwrap_or(0.0).to_radians()))).unwrap();
        }
    }
    if std::env::var_os("PROBE_DRIVE").is_some() {
        // Off the parking spot onto the nearest traffic lane, along it.
        let p = game.sim.drive.as_ref().unwrap().state.position;
        let f = &game.sim.traffic.as_ref().unwrap().routes;
        let (r, c) = f
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.lanes[2..6].iter().any(|&l| l))
            .flat_map(|(ri, r)| r.points.iter().enumerate().map(move |(ci, c)| (ri, ci, c.position)))
            .min_by(|a, b| a.2.distance_squared(p).total_cmp(&b.2.distance_squared(p)))
            .map(|(r, c, _)| (r, c))
            .unwrap();
        let back = std::env::var_os("PROBE_BACK").is_some();
        let rec = &f.records[r];
        let l = (2..6).map(|l| nfsu2::formats::ai_routes::lane(l, back)).find(|&l| rec.lanes[l]).unwrap_or(2);
        let at = f.point(r, c, back, 0.0, nfsu2::formats::ai_routes::lane_offset(&rec.points[c], l));
        let dir = f.direction(r, c, back);
        game.explore(Some((at + Vec3::Z, dir.y.atan2(dir.x)))).unwrap();
    }
    let start = game.sim.drive.as_ref().unwrap().state.position;
    println!("player at {:.0} {:.0}", start.x, start.y);
    let n = game.sim.traffic.as_ref().unwrap().cars.len();
    #[derive(Default, Clone)]
    struct Track {
        placed: bool,
        on_mover: bool,
        pos: Vec3,
        record: Option<(usize, usize, bool, usize)>,
        stopped: f32,
        stuck_logged: bool,
        wall: bool,
        touch: bool,
        bad: bool,
    }
    let mut tr = vec![Track::default(); n];
    let (mut offs, mut head, mut ratio, mut tgt_ratio) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut ev: HashMap<&str, usize> = HashMap::new();
    let frames = (secs * 60.0) as usize;
    let mut dens: Vec<(usize, usize)> = Vec::new();
    let mut models: HashMap<String, usize> = HashMap::new();
    for c in &game.sim.traffic.as_ref().unwrap().cars {
        *models.entry(c.car.car_type.name.clone()).or_default() += 1;
    }
    println!("pool {n}: {models:?}");
    // PROBE_DRIVE: the player drives the traffic lanes at about 20 m/s
    // (a crude pursuit on the walk's points, lane 2 or the first its
    // record has), as a player did when traffic looked wrong.
    let drive = std::env::var_os("PROBE_DRIVE").is_some();
    let mut walk: Option<(usize, usize, bool)> = None;
    let mut auto_rng = nfsu2::opponents::GameRng(7);
    let mut player_hits = 0usize;
    let mut player_touch = false;
    for frame in 0..frames {
        if drive {
            let f = &game.sim.traffic.as_ref().unwrap().routes;
            let st = &game.sim.drive.as_ref().unwrap().state;
            let (pos, fwd) = (st.position.truncate(), st.forward().truncate().normalize_or_zero());
            if walk.is_none() {
                walk = f
                    .records
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.lanes[2..6].iter().any(|&l| l))
                    .flat_map(|(ri, r)| r.points.iter().enumerate().map(move |(ci, c)| (ri, ci, c.position.truncate())))
                    .min_by(|a, b| a.2.distance_squared(pos).total_cmp(&b.2.distance_squared(pos)))
                    .map(|(r, c, _)| (r, c, f.direction(r, c, false).dot(fwd) < 0.0));
            }
            let mut target = None;
            if let Some(mut w) = walk {
                while f.along(w.0, w.1, w.2, pos) > f.segment(w.0, w.1, w.2) {
                    match nfsu2::ai::traffic::road_walk(f, w.0, w.1, w.2, 0.01, &mut auto_rng) {
                        Some(n) => w = n,
                        None => break,
                    }
                }
                walk = Some(w);
                let ahead = nfsu2::ai::traffic::road_walk(f, w.0, w.1, w.2, 12.0, &mut auto_rng.clone()).unwrap_or(w);
                let r = &f.records[ahead.0];
                let l = (2..6).find(|&l| r.lanes[nfsu2::formats::ai_routes::lane(l, ahead.2)]).unwrap_or(2);
                let off = nfsu2::formats::ai_routes::lane_offset(
                    &r.points[ahead.1],
                    nfsu2::formats::ai_routes::lane(l, ahead.2),
                );
                target = Some(f.point(ahead.0, ahead.1, ahead.2, 0.0, off).truncate());
            }
            let d = game.sim.drive.as_mut().unwrap();
            let speed = d.state.velocity.truncate().length();
            let mut c = Controls {
                throttle: if speed < 18.0 { 1.0 } else { 0.0 },
                brake: if speed > 23.0 { 0.5 } else { 0.0 },
                handbrake: 0.0,
                steer: 0.0,
                nitrous: false,
                ..Default::default()
            };
            if let Some(tp) = target {
                let to = (tp - pos).normalize_or_zero();
                let a = fwd.y.atan2(fwd.x) - to.y.atan2(to.x);
                c.steer = (a.sin().atan2(a.cos()) * 3.0).clamp(-1.0, 1.0);
            }
            if verbose && frame % 300 == 0 {
                println!(
                    "{:7.2} player at {:.0} {:.0} speed {speed:.1} walk {walk:?} target {target:?} steer {:.2} wall {:?} gear {:?} mode {}",
                    frame as f32 / 60.0,
                    pos.x,
                    pos.y,
                    c.steer,
                    d.state.wall_contact,
                    d.state.drivetrain.as_ref().map(|t| t.gear),
                    mode_name(&game.mode)
                );
            }
            game.platform.keys.clear();
            for (on, k) in [
                (c.throttle > 0.0, KeyCode::ArrowUp),
                (c.brake > 0.0, KeyCode::ArrowDown),
                (c.steer < -0.15, KeyCode::ArrowLeft),
                (c.steer > 0.15, KeyCode::ArrowRight),
            ] {
                if on {
                    game.platform.keys.insert(k);
                }
            }
            game.advance_for(1.0 / 60.0).unwrap();
            let d = game.sim.drive.as_ref().unwrap();
            let touch = d.state.contacts.iter().any(|x| matches!(x.touching, nfsu2::drive::Touching::Car { .. }));
            if touch && !player_touch {
                player_hits += 1;
                if verbose {
                    println!(
                        "{:7.2} player touches a car at {:.0} {:.0} speed {:.1}",
                        frame as f32 / 60.0,
                        d.state.position.x,
                        d.state.position.y,
                        speed
                    );
                }
            }
            player_touch = touch;
        } else {
            game.advance_for(1.0 / std::env::var("PROBE_HZ").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(60.0))
                .unwrap();
        }
        let t = game.sim.traffic.as_ref().unwrap();
        let me = game.sim.drive.as_ref().unwrap().state.position;
        let ts = frame as f32 / 60.0;
        dens.push((
            t.cars.iter().filter(|c| c.lane.is_some()).count(),
            t.cars.iter().filter(|c| c.lane.is_some() && c.mover.is_some()).count(),
        ));
        if let Some(w) = std::env::var("PROBE_CAR").ok().and_then(|s| s.parse::<usize>().ok()) {
            let c = &t.cars[w];
            if let Some(l) = c.lane.as_ref() {
                if frame % 6 == 0 {
                    println!(
                        "{ts:7.2} car {w} at {:.1?} v {:.1} target {:?} obey {} avoid {:?} mover {} step {:?}",
                        c.state.position.truncate(),
                        c.state.velocity.length(),
                        c.plan.as_ref().map(|x| x.1),
                        l.obeying(),
                        l.avoid_target().map(|id| if id < t.cars.len() {
                            format!("car {id} at {:.0?}", t.cars[id].state.position.truncate())
                        } else {
                            format!("id {}", usize::MAX - id)
                        }),
                        c.mover.is_some(),
                        l.current(&t.routes).map(|x| (x.0, x.1, x.2, x.3))
                    );
                }
            }
        }
        for (i, c) in t.cars.iter().enumerate() {
            let k = &mut tr[i];
            let placed = c.lane.is_some();
            let p = c.state.position;
            let jumped = placed && k.placed && p.truncate().distance(k.pos.truncate()) > 10.0;
            if k.placed && (!placed || jumped) {
                *ev.entry("despawn").or_default() += 1;
                if verbose {
                    println!(
                        "{ts:7.2} car {i:2} despawn at {:.0} {:.0} ({:.0} m from player){}",
                        k.pos.x,
                        k.pos.y,
                        k.pos.truncate().distance(me.truncate()),
                        if k.on_mover { "" } else { " [melo]" }
                    );
                }
            }
            if placed && (!k.placed || jumped) {
                *ev.entry("spawn").or_default() += 1;
                let cur = c.lane.as_ref().and_then(|l| l.current(&t.routes));
                let dir = cur.map_or(Vec2::ZERO, |x| x.4);
                let toward = (me.truncate() - p.truncate()).normalize_or_zero().dot(dir) > 0.0;
                if verbose {
                    println!(
                        "{ts:7.2} car {i:2} spawn {} at {:.0} {:.0} ({:.0} m, {}) step {:?} speed {:.1}",
                        c.car.car_type.name,
                        p.x,
                        p.y,
                        p.truncate().distance(me.truncate()),
                        if toward { "towards player" } else { "away" },
                        cur.map(|x| (x.0, x.1, x.2, x.3)),
                        c.state.velocity.length()
                    );
                }
                k.record = cur.map(|x| (x.0, x.1, x.2, x.3));
                k.stopped = 0.0;
                k.stuck_logged = false;
            }
            if placed {
                let lane = c.lane.as_ref().unwrap();
                if k.on_mover && c.mover.is_none() && !jumped && k.placed {
                    *ev.entry("hit->melo").or_default() += 1;
                    let other = t
                        .cars
                        .iter()
                        .enumerate()
                        .filter(|&(j, o)| j != i && o.lane.is_some())
                        .map(|(j, o)| (j, o.state.position.distance(p)))
                        .min_by(|a, b| a.1.total_cmp(&b.1));
                    let pd = p.distance(me);
                    if verbose {
                        println!(
                            "{ts:7.2} car {i:2} hit -> MeloMvr at {:.0} {:.0}; nearest traffic {:?}, player {:.1} m; contacts {:?}",
                            p.x,
                            p.y,
                            other,
                            pd,
                            c.state.contacts.iter().map(|x| (x.touching, x.point)).collect::<Vec<_>>()
                        );
                    }
                }
                let wall = c.state.wall_contact.is_some()
                    && c.state.contacts.iter().any(|x| matches!(x.touching, nfsu2::drive::Touching::Track { .. }));
                if wall && !k.wall {
                    *ev.entry(if c.mover.is_some() { "wall (mover)" } else { "wall (melo)" }).or_default() += 1;
                    if verbose {
                        println!(
                            "{ts:7.2} car {i:2} wall contact at {:.0} {:.0} speed {:.1} off {:?}",
                            p.x,
                            p.y,
                            c.state.velocity.length(),
                            lane.off_centre(&t.routes, p.truncate())
                        );
                    }
                }
                k.wall = wall;
                let touch = c.state.contacts.iter().any(|x| matches!(x.touching, nfsu2::drive::Touching::Car { .. }));
                if touch && !k.touch {
                    *ev.entry("car contact").or_default() += 1;
                }
                k.touch = touch;
                let cur = lane.current(&t.routes);
                if let (Some(a), Some(b)) = (k.record, cur) {
                    if a.0 != b.0 && !jumped {
                        *ev.entry("record change").or_default() += 1;
                        if verbose {
                            println!(
                                "{ts:7.2} car {i:2} junction r{}:{} -> r{}:{} lane {} -> {}",
                                a.0, a.1, b.0, b.1, a.3, b.3
                            );
                        }
                    } else if a.3 != b.3 && !jumped {
                        *ev.entry("lane change").or_default() += 1;
                        if verbose {
                            println!("{ts:7.2} car {i:2} lane {} -> {} (avoid {})", a.3, b.3, lane.avoiding());
                        }
                    }
                }
                k.record = cur.map(|x| (x.0, x.1, x.2, x.3));
                let speed = c.mover.map_or(c.state.velocity.length(), |m| m.speed);
                if speed < 0.5 {
                    k.stopped += 1.0 / 60.0;
                    if k.stopped > 10.0 && !k.stuck_logged {
                        k.stuck_logged = true;
                        *ev.entry("stopped >10 s").or_default() += 1;
                        if verbose {
                            println!(
                                "{ts:7.2} car {i:2} stopped 10 s at {:.0} {:.0} obey {} avoid {} mover {} target {:?} tracking {:?}",
                                p.x,
                                p.y,
                                lane.obeying(),
                                lane.avoiding(),
                                c.mover.is_some(),
                                c.plan.as_ref().map(|x| x.1),
                                lane.avoid_target().map(|id| if id < t.cars.len() {
                                    format!("car {id} at {:.0?}", t.cars[id].state.position.truncate())
                                } else {
                                    format!("id {}", usize::MAX - id)
                                })
                            );
                        }
                    }
                } else {
                    k.stopped = 0.0;
                }
                if c.mover.is_some() && !lane.avoiding() && k.placed && !jumped {
                    let o = lane.off_centre(&t.routes, p.truncate());
                    if let Some(o) = o {
                        offs.push(o.abs());
                    }
                    let mut err = 0.0;
                    if let Some(x) = cur {
                        let h = c.state.heading();
                        let d = x.4.y.atan2(x.4.x) - h;
                        err = d.sin().atan2(d.cos()).abs().to_degrees();
                        head.push(err);
                    }
                    let bad = o.is_some_and(|o| o.abs() > 2.0) || err > 30.0;
                    if bad && !k.bad {
                        *ev.entry("off lane >2 m or >30 deg").or_default() += 1;
                        if verbose {
                            println!(
                                "{ts:7.2} car {i:2} off lane {o:?} heading err {err:.0} at {:.0} {:.0} step {:?} speed {speed:.1} obey {}",
                                p.x,
                                p.y,
                                cur.map(|x| (x.0, x.1, x.2, x.3)),
                                lane.obeying()
                            );
                        }
                    }
                    k.bad = bad;
                    let ls = lane.lane_speed(&t.routes);
                    if ls > 1.0 && !lane.obeying() {
                        ratio.push(speed / ls);
                        if let Some((_, target)) = c.plan.as_ref().filter(|x| x.1 > 1.0) {
                            tgt_ratio.push(speed / target);
                        }
                    }
                }
            }
            k.placed = placed;
            k.on_mover = c.mover.is_some();
            k.pos = p;
        }
    }
    let q = |v: &mut Vec<f32>, p: f64| {
        v.sort_by(f32::total_cmp);
        if v.is_empty() { f32::NAN } else { v[((v.len() - 1) as f64 * p) as usize] }
    };
    let placed_mean = dens.iter().map(|d| d.0).sum::<usize>() as f32 / dens.len().max(1) as f32;
    let mover_mean = dens.iter().map(|d| d.1).sum::<usize>() as f32 / dens.len().max(1) as f32;
    println!(
        "placed per frame: mean {placed_mean:.1} min {} max {}; on the TrafficMover mean {mover_mean:.1}",
        dens.iter().map(|d| d.0).min().unwrap_or(0),
        dens.iter().map(|d| d.0).max().unwrap_or(0)
    );
    println!(
        "placement tries {} placed {}",
        game.sim.traffic.as_ref().unwrap().tries.0,
        game.sim.traffic.as_ref().unwrap().tries.1
    );
    println!("events over {secs} s: {:?}", {
        let mut e: Vec<_> = ev.into_iter().collect();
        e.sort();
        e
    });
    println!(
        "lane offset |m| (mover, not avoiding) n={}: median {:.2} p90 {:.2} p99 {:.2} max {:.2}",
        offs.len(),
        q(&mut offs, 0.5),
        q(&mut offs, 0.9),
        q(&mut offs, 0.99),
        q(&mut offs, 1.0)
    );
    println!(
        "heading error deg: median {:.1} p90 {:.1} p99 {:.1} max {:.1}",
        q(&mut head, 0.5),
        q(&mut head, 0.9),
        q(&mut head, 0.99),
        q(&mut head, 1.0)
    );
    println!(
        "speed / lane speed: p10 {:.2} median {:.2} p90 {:.2}; speed / target: p10 {:.2} median {:.2} p90 {:.2}",
        q(&mut ratio, 0.1),
        q(&mut ratio, 0.5),
        q(&mut ratio, 0.9),
        q(&mut tgt_ratio, 0.1),
        q(&mut tgt_ratio, 0.5),
        q(&mut tgt_ratio, 0.9)
    );
    let t = game.sim.traffic.as_ref().unwrap();
    let p = game.sim.drive.as_ref().unwrap().state.position;
    println!(
        "player: drove {:.0} m from the start, {player_hits} car contacts",
        p.truncate().distance(start.truncate())
    );
    {
        // Why placements fail where the player ended: the walk from the
        // nearest control point of a record with traffic lanes.
        let f = &t.routes;
        let near = f
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.lanes[2..].iter().any(|&l| l))
            .flat_map(|(ri, r)| r.points.iter().enumerate().map(move |(ci, c)| (ri, ci, c.position)))
            .min_by(|a, b| a.2.distance_squared(p).total_cmp(&b.2.distance_squared(p)))
            .unwrap();
        println!(
            "player's node r{}:{} of {} points ({:.0} m away)",
            near.0,
            near.1,
            f.records[near.0].points.len(),
            near.2.distance(p)
        );
        for back in [false, true] {
            let mut rng = nfsu2::opponents::GameRng(3);
            let ok = (0..20)
                .filter(|_| nfsu2::ai::traffic::road_walk(f, near.0, near.1, back, 250.0, &mut rng).is_some())
                .count();
            println!("  walk 250 m {}: {ok}/20 reach", if back { "backward" } else { "forward" });
        }
    }
    println!(
        "end: {} placed, {} on mover",
        t.cars.iter().filter(|c| c.lane.is_some()).count(),
        t.cars.iter().filter(|c| c.mover.is_some()).count()
    );
}

/// The control points a walk passed, in order, with a break (None) at a
/// new placement.
type Passed = Vec<Option<(usize, usize)>>;

/// Per control point that holds a turn junction (bit 26) or a branch
/// (high nibble 4): how many walks
/// passed it, how many took a turn there and how many another junction
/// (a link or a branch without bit 26). A pass is the walk's first point
/// moving from A to the next control point B: B a junction target of A
/// (taken), or A's neighbour on its record (straight on).
fn turn_tally(
    f: &nfsu2::formats::ai_routes::RouteFile,
    walks: &[Passed],
) -> std::collections::BTreeMap<(u16, usize), (usize, usize, usize)> {
    let mut out = std::collections::BTreeMap::new();
    for w in walks {
        for pair in w.windows(2) {
            let (Some(a), Some(b)) = (pair[0], pair[1]) else { continue };
            let point = &f.records[a.0].points[a.1];
            let js: Vec<_> = f.junctions_at(point).map(|j| f.junctions[j]).collect();
            if !js.iter().any(|j| j.kind() != 3 && (j.turn() || j.flags >> 4 & 0xF == 4)) {
                continue;
            }
            let hit = js.iter().find(|j| {
                let (rid, tcp) = j.target();
                f.record(rid) == Some(b.0) && usize::try_from(tcp).ok() == Some(b.1)
            });
            let straight = b.0 == a.0 && (b.1 + 1 == a.1 || a.1 + 1 == b.1);
            let e = out.entry((f.records[a.0].id, a.1)).or_insert((0, 0, 0));
            match hit {
                Some(j) if j.turn() => {
                    e.0 += 1;
                    e.1 += 1;
                }
                Some(_) => {
                    e.0 += 1;
                    e.2 += 1;
                }
                None if straight => e.0 += 1,
                None => {}
            }
        }
    }
    out
}

/// Per (mover, avoiding): car-frames, under 3 m/s, negative speed.
type Modes = std::collections::BTreeMap<(u8, bool), (usize, usize, usize)>;

/// One frame of [`traffic_turn_stats`]: each placed car's walk front and
/// its mover (2 TrafficMover, 3 MeloMvr, 4 BeltMover with the belt's
/// speed, 0 the physics).
/// What [`watch_walks`]' dump adds per row: the player's place,
/// facing (x, y) and speed, the race road (None in free roam) and the
/// player's race coordinate on it.
#[derive(Clone, Copy)]
struct DumpPlayer<'a> {
    at: Vec3,
    fwd: Vec2,
    speed: f32,
    road: Option<&'a nfsu2::formats::roads::RoadRoute>,
    along: f32,
    /// The human controller's road node (record index, cp), the
    /// placer's start (0x42E9F0 on controller +0x0C).
    node: Option<(usize, usize)>,
    /// The race's wrong-way byte (+0x248).
    wrong_way: bool,
}

fn watch_walks(t: &Traffic, walks: &mut [(Passed, Option<Vec2>)], modes: &mut Modes, pl: Option<DumpPlayer>) {
    // TURN_DUMP=<file>: every second frame (≈ 30 a second, as
    // tools/parity/trafwalk.py samples the original) one trafwalk-format
    // row per placed car — frame, slot, mover, place, heading (°), speed,
    // action (2 Avoid Cars, 17 Obey, 7 Drive Traffic Lane) — and the
    // tracked object's id (the player usize::MAX, racers below it).
    thread_local! {
        static DUMP: std::cell::RefCell<Option<(std::io::BufWriter<std::fs::File>, u64)>> =
            std::cell::RefCell::new(std::env::var("TURN_DUMP").ok().and_then(|p| std::fs::File::create(p).ok()).map(|f| (std::io::BufWriter::new(f), 0)));
    }
    DUMP.with(|d| {
        use std::io::Write;
        let mut d = d.borrow_mut();
        let Some((w, n)) = d.as_mut() else { return };
        *n += 1;
        if *n % 2 != 0 {
            return;
        }
        // Placement columns (appended): player x,y · fwd x,y · player speed ·
        // car race coordinate · player race coordinate · the walk's front
        // record id:cp:backward:lane · held. A parked car (no lane) is a
        // mode 0 row, as trafwalk lists the original's parked cars.
        let extra = |c: &TrafficCar| {
            let Some(pl) = pl else { return String::new() };
            let p = c.state.position.truncate();
            let cc = pl.road.map_or(0.0, |r| r.locate_once(p));
            let front = c.lane.as_ref().or(c.coast.as_ref()).and_then(|l| l.current(&t.routes)).map_or("-".to_string(), |(r, cp, b, lane, _)| {
                format!("{}:{cp}:{}:{lane}", t.routes.records[r].id, u8::from(b))
            });
            let node = pl.node.map_or("-".to_string(), |(r, cp)| {
                format!("{}:{cp}:{:#x}:{:.2}:{}", t.routes.records[r].id, t.routes.records[r].flags, pl.speed, u8::from(pl.wrong_way))
            });
            format!(
                "\t{:.2},{:.2}\t{:.3},{:.3}\t{:.2}\t{cc:.1}\t{:.1}\t{front}\t{}\t{node}",
                pl.at.x,
                pl.at.y,
                pl.fwd.x,
                pl.fwd.y,
                pl.speed,
                pl.along,
                u8::from(c.held)
            )
        };
        for (i, c) in t.cars.iter().enumerate() {
            // A paused car with its walk is listed by its mover, as
            // trafwalk lists the original's coasting parked cars.
            let Some(l) = c.lane.as_ref().or(c.coast.as_ref()) else {
                if pl.is_some() {
                    let p = c.state.position;
                    let _ = writeln!(
                        w,
                        "0\t{n}\t0\t{i}\t{i}\t0\t{:.2},{:.2},{:.2}\t0.0\t0.000\t-1\t-1\t0.00\t0.00\t0.0\t0\t0.00,0.00\t{}{}",
                        p.x,
                        p.y,
                        p.z,
                        u8::from(c.active),
                        extra(c)
                    );
                }
                continue;
            };
            let (mode, speed) = match (&c.mover, &c.belt.belt, &c.state.melo) {
                (Some(m), _, _) => (2, m.speed),
                (_, Some(b), _) => (4, b.speed()),
                (_, _, Some(_)) => (3, c.state.speed()),
                _ => (0, c.state.speed()),
            };
            let act = if l.avoiding() { 2 } else if l.obeying() { 17 } else { 7 };
            let p = c.state.position;
            let st = l.avoid_state();
            let tgt = st.map_or(-1, |(t, ..)| if t >= usize::MAX - 64 { -((usize::MAX - t) as i64) - 1 } else { t as i64 });
            let (tm, look, asked) = st.map_or((0.0, 0.0, 0.0), |(_, tm, look, asked)| (tm, look, asked));
            let _ = writeln!(
                w,
                "0\t{n}\t0\t{i}\t{i}\t{mode}\t{:.2},{:.2},{:.2}\t{:.1}\t{speed:.3}\t{act}\t{tgt}\t{asked:.2}\t{tm:.2}\t{look:.1}\t{}\t{:.2},{:.2}\t{}{}",
                p.x,
                p.y,
                p.z,
                c.state.heading().to_degrees(),
                u8::from(c.stopped),
                c.state.velocity.x,
                c.state.velocity.y,
                u8::from(c.active),
                extra(c)
            );
        }
        let _ = w.flush();
    });
    for (i, c) in t.cars.iter().enumerate() {
        let Some(cur) = c.lane.as_ref().and_then(|l| l.current(&t.routes)) else { continue };
        let (mode, speed) = match (&c.mover, &c.belt.belt, &c.state.melo) {
            (Some(m), _, _) => (2, m.speed),
            (_, Some(b), _) => (4, b.speed()),
            (_, _, Some(_)) => (3, c.state.speed()),
            _ => (0, c.state.speed()),
        };
        let e = modes.entry((mode, c.lane.as_ref().unwrap().avoiding())).or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += usize::from(speed < 3.0);
        e.2 += usize::from(speed < 0.0);
        let p = c.state.position.truncate();
        let w = &mut walks[i];
        if w.1.is_some_and(|q| q.distance(p) > 50.0) {
            w.0.push(None);
        }
        w.1 = Some(p);
        if w.0.last() != Some(&Some((cur.0, cur.1))) {
            w.0.push(Some((cur.0, cur.1)));
        }
    }
}

/// Turns per junction point, the clone against the original
/// (`cargo test --release --features dev-harness --bin game -- --ignored
/// traffic_turn_stats --nocapture`). `TURN_NATIVE` = the original's
/// `native-walk.tsv` (tools/parity/trafwalk.py: each traffic car's walk
/// by record id and control point); `TURN_AT=x,y,z[,heading°]` the
/// player's stand (free roam, a career at stage 1), `TURN_SECONDS` (300),
/// `TURN_SEEDS` (1: the default stream; n: also NFSU2_TRAFFIC_SEED
/// 1..n-1, a harness knob read at the world build).
#[test]
#[ignore = "probe: turns per junction, the clone against a native walk log"]
fn traffic_turn_stats() {
    crate::tests::testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("Turns");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let f = game.sim.traffic.as_ref().unwrap().routes.clone();
    let at: Vec<f32> = std::env::var("TURN_AT")
        .unwrap_or_else(|_| "450,-60,10,90".into())
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    let secs: f32 = std::env::var("TURN_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(300.0);
    let seeds: u64 = std::env::var("TURN_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut clone: Vec<Passed> = Vec::new();
    let mut modes: Modes = Default::default();
    // TURN_RACE=1: the soak's Quick Race circuit instead (the mode's first
    // track, traffic level TURN_TRAFFIC (3), the autopilot from the green).
    let race = std::env::var_os("TURN_RACE").is_some();
    let throttle = std::env::var_os("TURN_THROTTLE").is_some();
    let mut f = f;
    for seed in 0..seeds {
        if seed > 0 {
            // SAFETY: the test thread alone reads the knob at the world build.
            unsafe { std::env::set_var("NFSU2_TRAFFIC_SEED", seed.to_string()) };
        }
        if race {
            game.settings.qr[0].traffic = std::env::var("TURN_TRAFFIC").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
            game.drop_drive();
            game.mode = game.icon_menu(MenuKind::Main);
            game.act(Action::QuickRace, MenuKind::Main).unwrap();
            game.act(Action::QrMode(screens::QrMode::Circuit), MenuKind::QrMode).unwrap();
            for _ in 0..3 {
                game.menu_key(Key::Accept).unwrap();
            }
            if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
                game.menu_key(Key::Accept).unwrap();
            }
            let Mode::Race(rc) = &game.mode else { panic!("no race ({})", mode_name(&game.mode)) };
            let n = rc.racers.traffic.as_ref().map_or(0, |t| t.cars.len());
            println!("race track {}", game.game_data.career.events.get(rc.event).map_or(0, |e| e.track));
            let mut walks: Vec<(Passed, Option<Vec2>)> = vec![(Vec::new(), None); n];
            let mut on = false;
            for _ in 0..((secs + 30.0) * 60.0) as usize {
                // TURN_THROTTLE=1: throttle only from the green, as the
                // original's qr_walk_4014 drives (no autopilot).
                if throttle {
                    game.platform.keys.insert(KeyCode::ArrowUp);
                }
                game.advance_for(1.0 / 60.0).unwrap();
                let Mode::Race(rc) = &mut game.mode else { break };
                if !on && !throttle && rc.timing.clock >= 0.0 {
                    if let (Some(routes), Some(d)) = (rc.racers.ai_routes.as_ref(), game.sim.drive.as_ref()) {
                        rc.autopilot = Some(Autopilot::new(d, routes, d.car.car_type.handling.race.drift()));
                        on = true;
                    }
                }
                // From the green, as the original's poll (qr_walk_4014 arms at the green).
                if let Some(t) = rc.racers.traffic.as_ref().filter(|_| rc.timing.clock >= 0.0) {
                    f = t.routes.clone();
                    let pl = game.sim.drive.as_ref().map(|d| DumpPlayer {
                        at: d.state.position,
                        fwd: (d.state.orientation * Vec3::X).truncate(),
                        speed: d.state.speed(),
                        road: Some(&rc.route.road),
                        along: rc.racers.me.along,
                        node: d.human_road.node,
                        wrong_way: rc.wrong_way,
                    });
                    watch_walks(t, &mut walks, &mut modes, pl);
                }
            }
            clone.extend(walks.into_iter().map(|w| w.0));
            continue;
        }
        game.explore(Some((Vec3::new(at[0], at[1], at[2]), at.get(3).copied().unwrap_or(0.0).to_radians()))).unwrap();
        let n = game.sim.traffic.as_ref().unwrap().cars.len();
        let mut walks: Vec<(Passed, Option<Vec2>)> = vec![(Vec::new(), None); n];
        for _ in 0..(secs * 60.0) as usize {
            game.platform.keys.clear();
            game.advance_for(1.0 / 60.0).unwrap();
            let pl = game.sim.drive.as_ref().map(|d| DumpPlayer {
                at: d.state.position,
                fwd: (d.state.orientation * Vec3::X).truncate(),
                speed: d.state.speed(),
                road: None,
                along: 0.0,
                node: d.human_road.node,
                wrong_way: false,
            });
            watch_walks(game.sim.traffic.as_ref().unwrap(), &mut walks, &mut modes, pl);
        }
        clone.extend(walks.into_iter().map(|w| w.0));
    }
    unsafe { std::env::remove_var("NFSU2_TRAFFIC_SEED") };
    let mut native = Vec::new();
    if let Ok(p) = std::env::var("TURN_NATIVE") {
        let mut by: HashMap<String, (Passed, Option<Vec2>)> = HashMap::new();
        for line in std::fs::read_to_string(&p).unwrap().lines().skip(1).filter(|l| !l.starts_with('#')) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() < 13 {
                continue;
            }
            let pos: Vec<f32> = c[6].split(',').filter_map(|x| x.parse().ok()).collect();
            let p2 = Vec2::new(pos[0], pos[1]);
            let first = c[12].split(' ').next().and_then(|w| {
                let mut it = w.split(':');
                let rid: i16 = it.next()?.parse().ok()?;
                let cp: usize = it.next()?.parse().ok()?;
                f.record(rid).filter(|&r| cp < f.records[r].points.len()).map(|r| (r, cp))
            });
            let e = by.entry(c[4].to_string()).or_default();
            if e.1.is_some_and(|q| q.distance(p2) > 50.0) {
                e.0.push(None);
            }
            e.1 = Some(p2);
            if let Some(x) = first
                && e.0.last() != Some(&Some(x))
            {
                e.0.push(Some(x));
            }
        }
        native = by.into_values().map(|v| v.0).collect();
    }
    let (tn, tc) = (turn_tally(&f, &native), turn_tally(&f, &clone));
    println!("junction point (record id:cp): original passes/turned/other-junction | clone passes/turned/other ({seeds} streams, {secs} s each)");
    let keys: std::collections::BTreeSet<_> = tn.keys().chain(tc.keys()).copied().collect();
    let (mut sn, mut sc) = ((0, 0), (0, 0));
    for k in keys {
        let a = tn.get(&k).copied().unwrap_or_default();
        let b = tc.get(&k).copied().unwrap_or_default();
        sn = (sn.0 + a.0, sn.1 + a.1);
        sc = (sc.0 + b.0, sc.1 + b.1);
        if a.0 + b.0 >= 3 {
            println!("TURNS {}:{}\t{}/{}/{}\t|\t{}/{}/{}", k.0, k.1, a.0, a.1, a.2, b.0, b.1, b.2);
        }
    }
    println!("TURNS total\toriginal {}/{} | clone {}/{}", sn.1, sn.0, sc.1, sc.0);
    for ((m, a), (n, slow, neg)) in modes {
        println!("MODES mode {m} avoid {a}: {n} car-frames, under 3 m/s {slow}, negative {neg}");
    }
}
