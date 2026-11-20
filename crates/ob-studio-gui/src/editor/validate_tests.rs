//! Race validation on a road network written in the test: a straight
//! road along +x in two segments, the second behind a barrier.

use glam::{Vec2, Vec3};

use super::*;
use std::collections::HashMap;

use crate::formats::roads::{RoadNet, Segment};
use crate::tools::map::race;
use crate::tools::map::edit::RacePart;
use crate::tools::map::ops::RacePoint;
use crate::tools::map::{CustomRace, MAX_CHECKPOINTS, MAX_LAPS, MapMod, RaceKind, Spot};

/// Two segments end to end along +x: 0..200 m and 200..400 m. With
/// `barrier`, a closed race barrier shuts the second.
pub(crate) fn straight(barrier: bool) -> RoadNet {
    let xs = [0.0, 100.0, 200.0, 200.0, 300.0, 400.0];
    let mut net = RoadNet {
        nodes: xs.iter().enumerate().map(|(i, &x)| (Vec2::new(x, 0.0), i / 3)).collect(),
        links: vec![(0xFF, 0, [-1; 3]); 6],
        along: vec![0.0; 6],
        ..RoadNet::default()
    };
    let seg = |first, exits: [Vec<(usize, u8)>; 2], barriers| Segment {
        first,
        end: first + 3,
        length: 200.0,
        spline_length: 200.0,
        exits,
        barriers,
    };
    net.segments = vec![seg(0, [vec![], vec![(1, 0)]], vec![]), seg(3, [vec![(0, 1)], vec![]], vec![0])];
    net.closed = vec![barrier];
    net.barrier_ends = vec![[Vec2::new(200.0, -5.0), Vec2::new(200.0, 5.0)]];
    net.barrier_held = vec![barrier];
    net.barrier_names = vec![vec![]];
    net
}

pub(crate) fn sprint(points: &[(f32, f32)]) -> CustomRace {
    let mut r = CustomRace::new("sprint", 4001, RaceKind::Sprint);
    r.name = "Sprint".into();
    r.checkpoints = points.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
    r
}

fn map_of(races: Vec<CustomRace>) -> MapMod {
    MapMod { races, ..MapMod::default() }
}

fn run(map: &MapMod, net: &RoadNet) -> Vec<Check> {
    race_checks(map, 0, Some(net), &[], &Tested::default())
}

fn errors(c: &[Check]) -> Vec<&Check> {
    c.iter().filter(|c| c.level == Level::Error).collect()
}

fn warnings(c: &[Check]) -> Vec<&Check> {
    c.iter().filter(|c| c.level == Level::Warning).collect()
}

#[test]
fn a_race_on_the_road_has_no_errors_and_says_its_length() {
    let net = straight(false);
    let mut r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 1.0)]);
    r.start = Some(Spot { position: Vec3::new(30.0, 0.0, 0.0), heading: 0.0 });
    r.finish = Some(Spot { position: Vec3::new(390.0, 0.0, 0.0), heading: 0.0 });
    let c = run(&map_of(vec![r]), &net);
    assert!(errors(&c).is_empty() && warnings(&c).is_empty(), "{c:#?}");
    assert!(c.iter().any(|c| c.ok && c.text.contains("sprint on track 4001, 400 m, 3 checkpoints")), "{c:#?}");
    assert!(c.iter().any(|c| c.ok && c.text.contains("finish line after the last checkpoint")), "{c:#?}");
    assert!(c.iter().any(|c| !c.ok && c.text.ends_with("not tested since the last change")));
}

/// A custom race has only its base track's roads: a checkpoint beyond
/// them, or a leg they do not join, says so.
#[test]
fn leaving_the_base_tracks_area_is_named_plainly() {
    let net = straight(false);
    let c = run(&map_of(vec![sprint(&[(20.0, 0.0), (380.0, 0.0)])]), &net);
    assert!(c.iter().any(|c| c.ok && c.text.contains("stays on that track's roads (an area of 400 m × 0 m)")), "{c:#?}");
    let c = run(&map_of(vec![sprint(&[(20.0, 0.0), (380.0, 0.0), (390.0, 600.0)])]), &net);
    let w = warnings(&c);
    assert!(
        w.iter().any(|w| w.text.contains("checkpoint 3 is 600 m from base track 4001's roads")
            && w.text.ends_with(AREA_NOTE)),
        "{c:#?}"
    );
    // Two roads that do not meet: no barrier to blame, the area is.
    let mut apart = straight(false);
    for s in &mut apart.segments {
        s.exits = [vec![], vec![]];
    }
    let c = run(&map_of(vec![sprint(&[(20.0, 0.0), (380.0, 0.0)])]), &apart);
    let e = errors(&c);
    assert_eq!(e.len(), 1, "{c:#?}");
    assert!(
        e[0].text.contains("no road from checkpoint 1 to 2 on base track 4001's roads") && e[0].text.ends_with(AREA_NOTE),
        "{}",
        e[0].text
    );
}

#[test]
fn too_few_checkpoints_and_a_missing_network_are_errors() {
    let map = map_of(vec![sprint(&[(20.0, 0.0)])]);
    let c = run(&map, &straight(false));
    assert!(errors(&c).iter().any(|c| c.text.contains("at least 2")), "{c:#?}");
    let c = race_checks(&map, 0, None, &[], &Tested::default());
    assert!(errors(&c).iter().any(|c| c.text.contains("no road network")), "{c:#?}");
}

#[test]
fn a_leg_behind_a_closed_barrier_is_named_and_another_track_offered() {
    let closed = straight(true);
    let open = straight(false);
    let map = map_of(vec![sprint(&[(20.0, 0.0), (380.0, 0.0)])]);
    let c = race_checks(&map, 0, Some(&closed), &[(4001, &closed), (4002, &open)], &Tested::default());
    let e = errors(&c);
    assert_eq!(e.len(), 1, "{c:#?}");
    assert!(
        e[0].text.contains("no road from checkpoint 1 to 2") && e[0].text.contains("race barrier of track 4001"),
        "{}",
        e[0].text
    );
    assert_eq!(e[0].fix, Some(Fix::TryTrack(4002)));
    assert_eq!(e[0].point, Some(RacePoint { part: RacePart::Checkpoint, index: 0 }));
    assert_eq!(e[0].at, Some(Vec2::new(20.0, 0.0)));
    assert!(blocks(&c, 0));
    // The fix moves the race onto that track, and the route builds there.
    let mut fixed = map.clone();
    assert!(fix_race(&mut fixed, 0, &Fix::TryTrack(4002), Some(&closed), &|_| 0.0));
    assert_eq!(fixed.races[0].track, 4002);
    assert!(errors(&race_checks(&fixed, 0, Some(&open), &[], &Tested::default())).is_empty());
    // No track that helps: no fix offered.
    let c = race_checks(&map, 0, Some(&closed), &[(4001, &closed)], &Tested::default());
    assert_eq!(errors(&c)[0].fix, None);
    // Nor a track whose roads are far from the checkpoints (its area is
    // elsewhere), even though a route builds from its nearest nodes.
    let mut far = open.clone();
    far.nodes.iter_mut().for_each(|n| n.0.y += 500.0);
    let c = race_checks(&map, 0, Some(&closed), &[(4001, &closed), (4003, &far)], &Tested::default());
    assert_eq!(errors(&c)[0].fix, None);
}

#[test]
fn grid_slots_off_the_road_are_snapped_onto_it() {
    let net = straight(false);
    let mut r = sprint(&[(100.0, 0.0), (380.0, 0.0)]);
    r.start = Some(Spot { position: Vec3::new(60.0, 0.0, 0.0), heading: 0.0 });
    r.grid = race::default_grid(r.start.unwrap());
    r.grid[6].position.y = 25.5;
    let map = map_of(vec![r]);
    let c = run(&map, &net);
    let w = warnings(&c);
    assert_eq!(w.len(), 1, "{c:#?}");
    assert!(w[0].text.contains("grid slot 7 is 26 m from the road network"), "{}", w[0].text);
    assert_eq!(w[0].fix, Some(Fix::SnapGrid));
    assert_eq!(w[0].point, Some(RacePoint { part: RacePart::Grid, index: 6 }));
    let mut fixed = map.clone();
    assert!(fix_race(&mut fixed, 0, &Fix::SnapGrid, Some(&net), &|_| 2.0));
    let g = fixed.races[0].grid[6];
    assert_eq!(g.position, Vec3::new(map.races[0].grid[6].position.x, 2.0, 2.0));
    // The other slots stay where they were.
    assert_eq!(fixed.races[0].grid[5], map.races[0].grid[5]);
    assert!(warnings(&run(&fixed, &net)).is_empty());
}

#[test]
fn a_default_grid_off_the_road_is_fixed_by_moving_the_start() {
    let net = straight(false);
    let mut r = sprint(&[(100.0, 0.0), (380.0, 0.0)]);
    r.start = Some(Spot { position: Vec3::new(60.0, 30.0, 0.0), heading: 170.0 });
    let map = map_of(vec![r]);
    let c = run(&map, &net);
    assert!(warnings(&c).iter().any(|w| w.fix == Some(Fix::SnapStart)), "{c:#?}");
    assert!(warnings(&c).iter().any(|w| w.fix == Some(Fix::SnapGrid)), "{c:#?}");
    let mut fixed = map.clone();
    assert!(fix_race(&mut fixed, 0, &Fix::SnapGrid, Some(&net), &|_| 0.0));
    let s = fixed.races[0].start.unwrap();
    // On the road, turned along it the way it nearly faced (-x).
    assert!(s.position.distance(Vec3::new(60.0, 0.0, 0.0)) < 1e-3, "{}", s.position);
    assert!((s.heading.abs() - 180.0).abs() < 1e-3, "{}", s.heading);
    assert!(fixed.races[0].grid.is_empty(), "the grid stays the default one");
    assert!(warnings(&run(&fixed, &net)).is_empty(), "{:#?}", run(&fixed, &net));
}

#[test]
fn a_finish_before_the_last_checkpoint_or_off_the_route_is_a_warning() {
    let net = straight(false);
    let mut r = sprint(&[(20.0, 0.0), (380.0, 0.0)]);
    r.finish = Some(Spot { position: Vec3::new(100.0, 0.0, 0.0), heading: 0.0 });
    let c = run(&map_of(vec![r.clone()]), &net);
    assert!(warnings(&c).iter().any(|w| w.text.contains("finish line comes before the last checkpoint")), "{c:#?}");
    r.finish = Some(Spot { position: Vec3::new(390.0, 40.0, 0.0), heading: 0.0 });
    let map = map_of(vec![r]);
    let c = run(&map, &net);
    assert!(warnings(&c).iter().any(|w| w.fix == Some(Fix::SnapFinish)), "{c:#?}");
    let mut fixed = map.clone();
    assert!(fix_race(&mut fixed, 0, &Fix::SnapFinish, Some(&net), &|_| 0.0));
    assert_eq!(fixed.races[0].finish.unwrap().position, Vec3::new(390.0, 0.0, 0.0));
    assert!(warnings(&run(&fixed, &net)).is_empty());
}

#[test]
fn ids_laps_and_spacing_are_checked() {
    let net = straight(false);
    let a = sprint(&[(20.0, 0.0), (35.0, 0.0), (380.0, 0.0)]);
    let mut b = sprint(&[(20.0, 0.0), (380.0, 0.0)]);
    b.kind = RaceKind::Circuit;
    b.laps = 30;
    let map = map_of(vec![a, b]);
    let c = run(&map, &net);
    assert!(warnings(&c).iter().any(|w| w.text.contains("checkpoints 1 and 2 are 15 m apart")), "{c:#?}");
    let c = race_checks(&map, 1, Some(&net), &[], &Tested::default());
    let e = errors(&c);
    assert!(e.iter().any(|e| e.fix == Some(Fix::Rename("sprint-2".into()))), "{c:#?}");
    assert!(e.iter().any(|e| e.fix == Some(Fix::Laps(MAX_LAPS))), "{c:#?}");
    let mut far = sprint(&[(0.0, 0.0), (2000.0, 0.0)]);
    far.id = "Bad Id".into();
    let c = run(&map_of(vec![far]), &net);
    assert!(warnings(&c).iter().any(|w| w.text.contains("2.0 km apart")), "{c:#?}");
    assert!(errors(&c).iter().any(|e| e.fix == Some(Fix::Rename("badid".into()))), "{c:#?}");
}

#[test]
fn checks_sort_errors_first() {
    let net = straight(true);
    let mut nets = HashMap::new();
    nets.insert(4001u16, Some(net));
    let map = map_of(vec![sprint(&[(20.0, 0.0), (35.0, 0.0)]), sprint(&[(20.0, 0.0), (380.0, 0.0)])]);
    let c = checks(&map, &nets, &[4001], &Tested::default());
    let levels: Vec<Level> = c.iter().map(|c| c.level).collect();
    let mut sorted = levels.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(levels, sorted);
    assert_eq!(c[0].level, Level::Error);
}

#[test]
fn tested_races_are_remembered_until_they_change() {
    let s = crate::testkit::Scratch::new("editor-tested");
    let mut r = sprint(&[(20.0, 0.0), (380.0, 0.0)]);
    assert!(!Tested::load(s.path()).is_tested(&r));
    mark_tested(s.path(), &r).unwrap();
    let t = Tested::load(s.path());
    assert!(t.is_tested(&r));
    // A new name keeps it; a moved checkpoint does not.
    r.name = "Other".into();
    assert!(t.is_tested(&r));
    let net = straight(false);
    let c = race_checks(&map_of(vec![r.clone()]), 0, Some(&net), &[], &t);
    assert!(c.iter().any(|c| c.ok && c.text.ends_with(": tested")), "{c:#?}");
    r.checkpoints[1].x += 1.0;
    assert!(!t.is_tested(&r));
    assert!(!s.path().join(format!("{TESTED_FILE}.tmp")).exists());
}

#[test]
fn snap_finds_the_nearest_open_road_point() {
    let net = straight(true);
    let rp = race::snap(&net, Vec2::new(150.0, 7.0)).unwrap();
    assert_eq!((rp.at, rp.segment, rp.distance), (Vec2::new(150.0, 0.0), 0, 7.0));
    assert_eq!(rp.dir, Vec2::X);
    // The closed segment is not snapped to.
    let rp = race::snap(&net, Vec2::new(350.0, 0.0)).unwrap();
    assert_eq!((rp.at, rp.segment), (Vec2::new(200.0, 0.0), 0));
}

/// The game's own races, as custom races (their checkpoints, grid and
/// start line), validate without errors and with their grids on the road.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_games_own_races_validate_cleanly() {
    let root = crate::testkit::game_root();
    let career = crate::frontend::career::CareerData::load(&root).unwrap();
    let mut checked = 0;
    let mut worst = 0.0f32;
    let mut off = Vec::new();
    let mut numbers: Vec<u16> = career.tracks.keys().copied().collect();
    numbers.sort();
    for number in numbers {
        let t = &career.tracks[&number];
        if !career.events.iter().any(|e| e.races.first().map_or(e.track, |r| r.0) == number) {
            continue;
        }
        let Ok(native) = crate::frontend::career::load_route(&root, t, false) else { continue };
        let Ok(net) = race::road_net(&root, t) else { continue };
        let mut r = CustomRace::new("native", number, if native.loops { RaceKind::Circuit } else { RaceKind::Sprint });
        r.checkpoints = native.checkpoints.clone();
        if native.loops && r.checkpoints.len() > 2 && r.checkpoints.first() == r.checkpoints.last() {
            r.checkpoints.pop();
        }
        if race::check(&net, &r).is_err() || r.checkpoints.len() > MAX_CHECKPOINTS {
            continue;
        }
        r.grid = native.grid.iter().map(|(p, h)| Spot { position: *p, heading: h.to_degrees() }).collect();
        for g in &r.grid {
            worst = worst.max(race::snap(&net, g.position.truncate()).map_or(0.0, |rp| rp.distance));
        }
        let c = race_checks(&map_of(vec![r]), 0, Some(&net), &[], &Tested::default());
        assert!(errors(&c).is_empty(), "{number}: {c:#?}");
        if c.iter().any(|c| c.fix == Some(Fix::SnapGrid)) {
            off.push(number);
        }
        checked += 1;
    }
    eprintln!("native races checked: {checked}, grid slots at most {worst:.1} m from the road line");
    // A handful start away from their network (on a lot beside it).
    assert!(checked >= 20 && off.len() * 10 <= checked, "{checked} checked, grids off the network: {off:?}");
}
