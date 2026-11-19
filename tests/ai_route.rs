//! The AI route layer against the shipped data.

mod common;

use glam::Vec2;
use nfsu2::ai::route::{AiRoutes, AiTuning, CatchUp, LineSpeed, Presets, RacerRoute, SpeedTarget, catch_up_multiplier, fit_performance, handling_score};
use nfsu2::frontend::career::{self, CareerData};
use nfsu2::opponents::{Bounds, GameRng};

fn root() -> std::path::PathBuf {
    common::game_root()
}

/// The grid slots come in marker +0x0C order: 4001's pole at the front
/// middle, slots 4–7 a row behind.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn grid_slots_follow_the_markers() {
    let root = root();
    let data = CareerData::load(&root).unwrap();
    let route = career::load_route(&root, &data.tracks[&4001], false).unwrap();
    assert_eq!(route.grid.len(), 8);
    assert!(route.grid[0].0.truncate().distance(Vec2::new(-397.3, 270.3)) < 0.2);
    assert!(route.grid[4].0.truncate().distance(Vec2::new(-391.7, 271.2)) < 0.2);
}

/// A racer walks 4001 from the pole along the racing line: holding back
/// the branches (type-4 junctions), the walk comes round to the grid after
/// the track's 6274 m (+0x98), never dead-ends, and the racing-line speeds
/// stay within the data's 100 m/s.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn racers_walk_a_lap_of_4001() {
    let root = root();
    let data = CareerData::load(&root).unwrap();
    let track = &data.tracks[&4001];
    let routes = AiRoutes::load(&root, track, false).unwrap();
    let grid = career::load_route(&root, track, false).unwrap().grid;
    let (pos, heading) = grid[0];
    let mut r = RacerRoute::start(&routes, pos, Vec2::new(heading.cos(), heading.sin())).expect("start");
    r.branch_hold = f32::MAX;
    let accel = |v: f32| (6.0 - v * 0.08).max(0.5);
    let mut at = pos;
    let mut walked = 0.0f32;
    let mut lap = None;
    for frame in 0..6000 {
        r.clock = frame as f32 * 0.05;
        r.update(&routes, at, 30.0, glam::Vec2::X, 2, false, &accel);
        assert!(!r.dead_end, "dead end at frame {frame}");
        let path = r.path(&routes);
        let v = r.speed_at(r.distance() + 20.0);
        assert!((0.0..=100.5).contains(&v), "speed {v}");
        let next = *path.iter().find(|p| p.truncate().distance(at.truncate()) > 5.0).expect("a point ahead");
        at += ((next - at).truncate().normalize_or_zero() * 1.5).extend(0.0);
        at.z = next.z;
        walked += 1.5;
        if walked > 1000.0 && at.truncate().distance(pos.truncate()) < 10.0 {
            lap = Some(walked);
            break;
        }
    }
    let lap = lap.expect("came round");
    // The probe walks the path in 1.5 m steps towards points 5 m ahead.
    assert!((lap - 6274.0).abs() < 6274.0 * 0.05, "lap {lap}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn tuning_rows_and_catch_up() {
    let root = root();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let t = AiTuning::parse(&globalb).expect("0x34B00");
    assert_eq!(t.speed_factor(0), 0.74);
    assert_eq!(t.speed_factor(9), 1.0);
    assert_eq!(t.jitter(0), 40.0);
    assert_eq!(t.jitter_period(), 5.0);
    let base = CatchUp { skill: 5, laps: 2, race_time: 60.0, progress: 0.2, ..Default::default() };
    let behind = catch_up_multiplier(&t, &CatchUp { gap: -200.0, ..base });
    let ahead = catch_up_multiplier(&t, &CatchUp { gap: 300.0, ..base });
    assert!(behind > 1.05 && ahead < 0.95, "{behind} {ahead}");
    // Faded out at the end of the race.
    assert_eq!(catch_up_multiplier(&t, &CatchUp { gap: -200.0, progress: 1.0, ..base }), 1.0);
    // Read under Wine (Quick Race circuit, Resort Loop, 3 laps, traffic
    // minimum, catch-up on; the human parked at the start, 4th, line-speed
    // mean 0): rivals of skill 3 and 4, 1035 and 1130 m ahead, 0.115 and
    // 0.125 of the race done, controller +0x770 = 0.6267 and 0.6514.
    let line = Some(LineSpeed { ratio: 0.0, place: 4 });
    for (gap, skill, want) in [(1035.0, 3, 0.6267), (1130.0, 4, 0.6514)] {
        let m = catch_up_multiplier(&t, &CatchUp { gap, skill, laps: 3, race_time: 60.0, progress: gap / 3001.1 / 3.0, own_speed: 17.5, line, ..Default::default() });
        assert!((m - want).abs() < 2e-4, "{gap}: {m} vs {want}");
    }
    // Target speed: route speed × row factor.
    let data = CareerData::load(&root).unwrap();
    let routes = AiRoutes::load(&root, &data.tracks[&4001], false).unwrap();
    let grid = career::load_route(&root, &data.tracks[&4001], false).unwrap().grid;
    let mut r = RacerRoute::start(&routes, grid[0].0, Vec2::new(grid[0].1.cos(), grid[0].1.sin())).unwrap();
    r.update(&routes, grid[0].0, 0.0, glam::Vec2::X, 2, false, &|_| 5.0);
    let mut st = SpeedTarget::new(0);
    let v = st.update(&t, &r, &mut GameRng::default(), 0.0, None, 0.016);
    assert!(v > 0.0 && v <= 100.0 * 0.74 + 0.01, "{v}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn presets_fit_opponents() {
    let root = root();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let presets = Presets::parse(&globalb);
    assert_eq!(presets.0.len(), 46);
    assert!((presets.0[2][0].rating - 144.12).abs() < 0.01);
    assert!(presets.0[2][0].parts.is_empty() || presets.0[2][0].rating < 145.0);
    let bounds = Bounds::parse(&globalb).unwrap();
    let data = CareerData::load(&root).unwrap();
    let records = nfsu2::car::car_type_records(&globalb).unwrap();
    let stock = handling_score(&records[3].1, &bounds);
    assert!((0.03..=1.0).contains(&stock));
    let low = fit_performance(&presets, &bounds, 3, 3, 0.0, &records[3].1, &data.packages, false);
    let high = fit_performance(&presets, &bounds, 3, 3, 10.0, &records[3].1, &data.packages, false);
    assert!(high.len() > low.len(), "{low:?} {high:?}");
}

/// A drag race's skill-source rivals (0x5260C0, 0x526309: score −1 when
/// 0x89E7D8 / 0x890108 is set) keep the nearest preset to the skill's
/// rating: observed under Wine, stage-4 drags against the finished-career save's
/// Skyline at skill 4 (rating 400.5): the COROLLA, PEUGOT and RX7 came out
/// at 397.5, 398.4 and 402.4 (classes 2, 2, 1).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drag_rivals_keep_the_nearest_preset() {
    let root = root();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let presets = Presets::parse(&globalb);
    let bounds = Bounds::parse(&globalb).unwrap();
    let data = CareerData::load(&root).unwrap();
    let records = nfsu2::car::car_type_records(&globalb).unwrap();
    let index = |name: &str| records.iter().position(|(n, _)| n.eq_ignore_ascii_case(name)).unwrap();
    let skyline = index("SKYLINE");
    for (name, rating) in [("COROLLA", 397.45), ("PEUGOT", 398.39), ("RX7", 402.36)] {
        let car = index(name);
        let fitted = fit_performance(&presets, &bounds, car, skyline, 4.0, &records[car].1, &data.packages, true);
        let preset = presets.0[car].iter().find(|p| p.parts == fitted).unwrap();
        assert!((preset.rating - rating).abs() < 0.05, "{name}: {}", preset.rating);
        // Outside a drag race the second fit moves it.
        let street = fit_performance(&presets, &bounds, car, skyline, 4.0, &records[car].1, &data.packages, false);
        assert_ne!(street, fitted, "{name}");
    }
}

/// The reference lap (0x5F6E80) on circuits and sprints: the walk from the
/// start line fills most of the lap's bins from the racing lines.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn reference_laps_walk_the_racing_lines() {
    use nfsu2::standings::{LapReference, RaceRoad, walk};
    let root = root();
    let data = CareerData::load(&root).unwrap();
    for number in [4001u16, 4101, 4213] {
        let Some(track) = data.tracks.get(&number) else { continue };
        let route = career::load_route(&root, track, false).unwrap();
        let routes = AiRoutes::load(&root, track, false).unwrap();
        let road = &route.road;
        let finish_at = if road.loops { route.start_at } else { route.finish_at };
        let race = RaceRoad { road, start_at: route.start_at, finish_at };
        let start = route.start_line.or(route.grid.first().map(|g| g.0)).unwrap();
        let found = walk(&routes.file, &race, start);
        let bins: std::collections::HashSet<usize> = found.iter().map(|f| (f.0 * 256.0) as usize).collect();
        let reference = LapReference::new(road.length, found.iter().copied());
        println!("{number}: {} rates, {} bins, lap {:.1} s over {:.0} m", found.len(), bins.len(), reference.lap_time, road.length);
        assert!(bins.len() > 64, "{number}: {} bins", bins.len());
    }
}

/// The encounter racers' racing-line speeds (0x5DDFD0 → 0x5E8790) for the
/// best class of the *upgraded* racers. Read under Wine (2026-11-04, the
/// finished-career save, career stage 6, encounter COROLLA / CORSA / PEUGOT
/// matched to the player's GT-R): the route manager's class +0x4674
/// (0x88E994) was 0 and record 1095's lane-0/1 bytes (control point
/// +0x2C / +0x2D) were these; the clone's free-roam network with class 0
/// gives them exactly, class 3 (the stock cars' class, which the clone
/// used before) 20–30 % less (docs/notes/outrun/ai-strength.md).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn encounter_racing_speeds_match_the_original_for_class_0() {
    let root = root();
    let data = CareerData::load(&root).unwrap();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let curves = nfsu2::ai::route::ClassCurves::parse(&globalb).expect("class curves");
    let track = &data.tracks[&4000];
    let mode = nfsu2::ai::route::speed_mode(track.flags, track.number);
    let native: [(usize, [u8; 2]); 7] = [(10, [56, 63]), (14, [60, 72]), (18, [64, 74]), (21, [67, 74]), (22, [68, 74]), (24, [70, 73]), (26, [72, 71])];
    for stage in [5, 6] {
        let mut file = nfsu2::formats::ai_routes::RouteFile::parse(&std::fs::read(root.join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).unwrap());
        file.finalize_free_roam_paths(&std::fs::read(root.join("TRACKS/ROUTESL4RA/PathsFreeRoam.bin")).unwrap(), 4000, stage);
        let bytes = |class: usize| {
            let mut routes = AiRoutes { file: file.clone(), track: 4000, backward: false, race_coords: false, resets: Vec::new(), blocks: Vec::new() };
            routes.racing_speeds(&curves, class, false, mode, None);
            let r = routes.file.record(1095).expect("record 1095");
            native.map(|(cp, _)| { let s = routes.file.records[r].points[cp].lane_speed; [s[0], s[1]] })
        };
        assert_eq!(bytes(0), native.map(|n| n.1), "stage {stage}: class 0 as the original's memory");
        assert!(bytes(3).iter().zip(native).all(|(c, n)| c[1] < n.1[1]), "stage {stage}: class 3 is slower");
    }
}

/// docs/notes/outrun/ai-strength.md §3: the chaser at (−220.8, −1577.1)
/// heading +x, 24 m behind the player (clone lead run L3, traffic draw 45,
/// the finished-career save). The goal-5 search's origin is record 720's first point
/// travelled backward, whose segment holds both cars; 0x428720 keeps that
/// node (`node_holds`), the list cannot go on past the record's first
/// point and the car's own entry lies past its last: 0x428720 returns
/// false (a full think's failure: no suspension in race 7), not the
/// 0x42B002 suspension the clone applied. The next update without the
/// search (the ready fallback's 0x428720) searches the node again
/// (0x42CDC0) and the car is inside the new list.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn chase_search_at_a_record_start_fails_the_update_and_recovers() {
    let root = root();
    let mut file = nfsu2::formats::ai_routes::RouteFile::parse(&std::fs::read(root.join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).unwrap());
    file.finalize_free_roam_paths(&std::fs::read(root.join("TRACKS/ROUTESL4RA/PathsFreeRoam.bin")).unwrap(), 4000, STAGE);
    let routes = AiRoutes { file, track: 4000, backward: false, race_coords: false, resets: Vec::new(), blocks: Vec::new() };
    let position = glam::Vec3::new(-220.79, -1577.06, 13.61);
    let direction = Vec2::new(0.9994, -0.0357);
    // The target is the player's road point (0x5EC0C0: the nearest
    // control point, `PointGrid`, as game.rs `player_node`).
    let player = glam::Vec3::new(-197.13, -1579.65, 13.61);
    let (pr, pc) = nfsu2::formats::ai_routes::PointGrid::new(&routes.file).nearest(&routes.file, player).unwrap();
    let target = routes.file.records[pr].points[pc].position;
    let mut route = RacerRoute::start(&routes, position, direction).expect("a route");
    route.set_outrun_chaser(true);
    assert!(route.search_outrun_chaser(&routes, position, direction, target));
    let (record, cp, backward) = route.node().unwrap();
    assert_eq!((record, cp, backward), (720, 0, true), "record index 720 (id {})", routes.file.records[record].id);
    assert!(nfsu2::ai::route::node_holds(&routes.file, record, cp, position));
    route.update(&routes, position, 33.0, glam::Vec2::X, 0, false, &|_| 1.0);
    assert!(route.node().is_some() && route.past_end(), "full think: 0x428720 returns false");
    route.update(&routes, position, 33.0, glam::Vec2::X, 0, false, &|_| 1.0);
    assert!(route.node().is_some_and(|n| n.0 != record) && !route.past_end(), "ready fallback: a new node");
}
const STAGE: u32 = 5;

/// A drag AI racer's shift offset ranges (0x5B6214..0x5B626A: AI tuning
/// +0x550..+0x564 by skill row). Read under Wine in drag_s1: a CORSA
/// of row 3 had +0xCC..+0xE0 = −500, 140, 0, 0.01, −500, 140, a FOCUS and
/// a PEUGOT106 of row 2 −650, 200, 0, 0.01, −650, 200.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drag_ai_shift_ranges_by_skill_row() {
    let root = root();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let t = AiTuning::parse(&globalb).expect("0x34B00");
    assert_eq!(t.shift_ranges(3), Some([[-500.0, 140.0], [0.0, 0.01], [-500.0, 140.0]]));
    assert_eq!(t.shift_ranges(2), Some([[-650.0, 200.0], [0.0, 0.01], [-650.0, 200.0]]));
    assert_eq!(t.shift_ranges(0).unwrap()[0], [-900.0, 300.0]);
    assert_eq!(t.shift_ranges(9).unwrap()[0], [-110.0, 15.0]);
    // A row past 9 keeps the drivetrain's own (the clamp then fails the
    // equality test at 0x5B6210).
    assert_eq!(t.shift_ranges(10), None);
}
