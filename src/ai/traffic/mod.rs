//! Traffic in explore mode: cars placed around the player on the route
//! file's traffic lanes (2–5 forward, 6–9 backward), each following its
//! lane through the traffic junctions at the lane's speed. See
//! docs/formats.md, "Traffic". The placement follows the game's traffic
//! placer (0x42F910 → 0x42F800 → 0x42E9F0), the pool and the models its
//! preload (0x4FE9F0, 0x412BE0), and a placed car moves by the game's
//! TrafficMover (0x408820) until it is hit. The lane's path and target
//! speed follow the traffic lane action "Drive Traffic Lane" (0x4242D0 →
//! 0x420D40 → 0x41D530), the intersection action (0x425150) and the
//! traffic goal (0x41C380, junctions 0x413BB0) for the racers' own
//! low-level driver, which the traffic controller builds too (0x426580 →
//! 0x425AB0); the controller thinks in turns (0x429D90).

use std::collections::VecDeque;

use glam::{Vec2, Vec3};

use crate::formats::ai_routes::{RouteFile, lane, lane_offset};
use crate::opponents::GameRng;

mod avoid;
mod crossings;
mod mover;
mod place;
mod plan;
mod pool;
mod stream;
mod think;
mod walk;

pub use avoid::*;
pub use crossings::*;
pub use mover::*;
pub use place::*;
use plan::*;
pub use pool::*;
pub use stream::*;
pub use think::*;
pub use walk::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// 0x420D40 keeps a traffic car's path front between thinks (the
    /// driver's +0x1D9, observed 1 for every traffic car under Wine): a
    /// car heading south on record 538's backward lane (L4RA free roam,
    /// the street by the stage-1 garage exit, docs/world-systems-parity.md)
    /// keeps steering for control point 21 until it is past it. The fresh
    /// plan from the point after the nearest, which the clone made every
    /// think before, aimed at point 20 from 20 m short of point 21 and cut
    /// the bend 2.5–4 m inside the original's lane.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn traffic_path_keeps_its_front_until_passed() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let rec = f.records.iter().position(|r| r.id == 734).expect("record id 734");
        let start = Step { record: rec, cp: 22, backward: true, lane: 4 };
        let mut t = TrafficLane {
            steps: VecDeque::from([start]),
            rng: GameRng(1),
            obey: None,
            avoid: None,
            roomed: 0,
            took_avoid: false,
            behind: None,
            kept: Vec::new(),
            heading: None,
            walked: 0.0,
            last_turn: -99_999.0,
            turned: None,
            lead: (0, 0.0),
            drag: false,
        };
        t.extend(&f, ENTRY_REACH);
        let on_lane = |cp: usize| point(&f, Step { cp, ..start }, 0.0);
        let (p21, p20) = (on_lane(21), on_lane(20));
        // Driving the lane south: at each place the car is on the lane line
        // from point 22 towards 21, heading along it.
        let dir = (p21 - on_lane(22)).truncate().normalize();
        t.heading = Some(dir);
        // The path runs through every walk point (0x41D530 → 0x415D50),
        // from the one after the car's nearest (at most two 8 m steps on):
        // the 8 m points between the control points, then point 21 itself
        // (a traffic walk's control points carry no offset).
        let car = on_lane(22) + (p21 - on_lane(22)) * 0.3;
        let (path, _) = t.update(&f, car, 15.0).expect("a path");
        let ahead = |q: Vec3, from: Vec3, d: Vec2| (q - from).truncate().dot(d);
        assert!(
            ahead(path[0], car, dir) > 0.0 && path[0].truncate().distance(car.truncate()) < 16.5,
            "fresh plan from the car's next walk point: {:?} (car {car:?})",
            path[0]
        );
        let has = |path: &[Vec3], q: Vec3| path.iter().any(|x| x.truncate().distance(q.truncate()) < 0.5);
        assert!(has(&path, p21), "the path holds point 21: {path:?}");
        let gaps: Vec<f32> = path.windows(2).map(|w| w[0].truncate().distance(w[1].truncate())).collect();
        // Every 8 m along a control point, the remainder (under 16 m)
        // before the next one.
        assert!(
            gaps.iter().all(|&g| g < 16.5) && gaps.iter().filter(|&&g| (g - 8.0).abs() < 0.2).count() * 2 > gaps.len(),
            "8 m points: {gaps:?}"
        );
        // 18 m short of point 21 the kept path still runs through it.
        let near = p21 - (p21 - on_lane(22)).normalize() * 18.0;
        let (path, _) = t.update(&f, near, 15.0).expect("a path");
        assert!(has(&path, p21), "kept through 21: {path:?}");
        // Past point 21: nothing behind the car, point 20 ahead.
        let d20 = (p20 - p21).truncate().normalize();
        t.heading = Some(d20);
        let car = p21 + (p20 - p21).normalize() * 2.0;
        let (path, _) = t.update(&f, car, 15.0).expect("a path");
        assert!(path.iter().all(|&q| ahead(q, car, d20) > -0.5), "past 21: {path:?}");
        assert!(has(&path, p20), "past 21, 20 ahead: {path:?}");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// The original's traffic goes straight on through intersection 3
    /// (record 734 cp 15 / record 492 cp 5; 0 turns in 18 passes under
    /// Wine, and 0 at record 734 cp 14 / 16 in 77 passes)
    /// although cp 16's junction 949 to record 492 is a branch taken half
    /// the time: a turn the walk takes as it reaches its length is dropped
    /// by the next extension (0x40A620) and not considered again within
    /// 30 m (walk +0x354). Driving down record 734 from far off at 14 m/s,
    /// a think every 0.53 s (16 cars, 0x429D90), so 7.4 m a pass, the walk
    /// never turns there whatever the draws; built at once from close by (a
    /// placement), a turn stays with some draws. (The clone once
    /// moved a whole control point a pass here, 10–15 m: the walk's end then
    /// fell further short of the reach than the original's ever does.)
    fn the_walk_drops_a_turn_taken_at_its_end() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let rec = f.records.iter().position(|r| r.id == 734).expect("record id 734");
        let turned = |t: &TrafficLane| t.steps.iter().any(|s| s.record != rec);
        let new = |cp: usize, seed: u32| {
            let start = Step { record: rec, cp, backward: true, lane: 4 };
            let mut t = TrafficLane {
                steps: VecDeque::from([start]),
                rng: GameRng(seed),
                obey: None,
                avoid: None,
                roomed: 0,
                took_avoid: false,
                behind: None,
                kept: Vec::new(),
                heading: None,
                walked: 0.0,
                last_turn: -99_999.0,
                turned: None,
                lead: (0, 0.0),
                drag: false,
            };
            t.extend(&f, ENTRY_REACH);
            t
        };
        for seed in 1..40 {
            // From cp 24, the record's last (about 150 m north): the walk
            // reaches cp 16 a step at a time.
            let mut t = new(24, seed);
            assert!(!turned(&t), "seed {seed}: the first build already reaches the junction");
            let mut went = Vec::new();
            // Down record 734 (backward) 7.4 m a think.
            let (mut cp, mut along) = (24usize, 0.0f32);
            for _ in 0..200 {
                along += 7.4;
                while cp > 13 && along > f.segment(rec, cp, true) {
                    along -= f.segment(rec, cp, true);
                    cp -= 1;
                }
                t.heading = Some(f.direction(rec, cp, true));
                if t.update(&f, f.point(rec, cp, true, along, 0.0), 14.0).is_none() {
                    break;
                }
                went.push((t.steps[0].record, t.steps[0].cp));
                if cp <= 13 {
                    break;
                }
            }
            assert!(
                went.iter().all(|&(r, _)| r == rec) && went.contains(&(rec, 15)),
                "seed {seed}: the car left record 734 at the intersection: {went:?}"
            );
        }
        // Built at once from cp 19 (cp 16 well inside the reach): some draws keep the turn.
        let kept = (1..40).filter(|&seed| turned(&new(19, seed))).count();
        assert!(kept > 0, "a placement's walk never turns at cp 16");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x422220, 0x422668..0x4226DF (verified): after a turn the
    /// extension measures its length from the walk's first point (the new
    /// last point's +0x54, which 0x419670 rebases as points drop behind the
    /// car) plus jt, so a turn well inside the reach is followed by more of
    /// the walk and stays, however far the car has driven. With the
    /// distance walked added (as the clone once did) the extension
    /// stopped right after every turn once the car had moved, and the next
    /// pass dropped it.
    fn a_turn_inside_the_reach_stays_after_a_long_drive() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let rec = f.records.iter().position(|r| r.id == 734).expect("record id 734");
        let mut turned = 0;
        // From cp 20 the walk holds its 10 points (0x422300's least) before
        // it reaches cp 16, still inside the reach.
        for seed in 1..40 {
            let mut t = TrafficLane {
                steps: VecDeque::from([Step { record: rec, cp: 20, backward: true, lane: 4 }]),
                rng: GameRng(seed),
                obey: None,
                avoid: None,
                roomed: 0,
                took_avoid: false,
                behind: None,
                kept: Vec::new(),
                heading: None,
                // 300 m driven since the walk's start, the last turn long behind.
                walked: 300.0,
                last_turn: -99_999.0,
                turned: None,
                lead: (0, 0.0),
                drag: false,
            };
            t.extend(&f, ENTRY_REACH);
            let Some(k) = t.steps.iter().position(|s| s.record != rec) else { continue };
            turned += 1;
            assert!(
                t.turned.is_none() && t.steps.len() > k + 1,
                "seed {seed}: the extension stopped at the turn's target (step {k} of {})",
                t.steps.len()
            );
        }
        assert!(turned > 0, "no draw takes the turn at cp 16");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x42E4A0 on the Bayview Bridge drag (Routes4201F): with the start
    /// line at 308.0 and the start gap 100 refusing up to 408, and the fill
    /// step 31 m as the 2D spacing, ten cars take ten distinct control
    /// points from 452.48 on (under Wine: 452.5, 502.3, … one a point),
    /// all on record 1, the one whose points carry bits 25 / 26.
    fn drag_fill_lines_the_bridge() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RB/Routes4201F.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let mut rng = GameRng(12345);
        let mut placed: Vec<(Vec2, f32)> = Vec::new();
        for j in 0..10 {
            let gate = |at: Vec2, c: f32, coordinate: bool| {
                !coordinate && c - 308.0 >= 100.0 && !placed.iter().any(|p| p.0.distance(at) < 31.0)
            };
            let (_, at, _) = TrafficLane::fill_drag(&f, j, 31.0, None, 0.5, &gate, &mut rng).expect("placed");
            let c = f
                .records
                .iter()
                .flat_map(|r| r.points.iter())
                .filter(|c| c.position.truncate().distance(at.truncate()) < 8.0)
                .map(|c| c.coord)
                .next()
                .expect("on a point");
            placed.push((at.truncate(), c));
        }
        let mut coords: Vec<f32> = placed.iter().map(|p| p.1).collect();
        coords.sort_by(f32::total_cmp);
        assert!((coords[0] - 452.48).abs() < 0.1, "{coords:?}");
        assert!(coords.windows(2).all(|w| w[1] - w[0] > 40.0), "{coords:?}");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// The stage-4 drag S4_H_DRAG_15 (track 4213 backward,
    /// ROUTESL4RB/Routes4213B) has no race-record point the drag fill
    /// 0x42E4A0 can take (0x417C60: a point with flag bit 25 or 26 on a
    /// record with a lane on that side), whatever the reach or the flag, so
    /// none of its 12 pool cars is placed and, the world list holding no
    /// traffic, the placer never runs (observed under Wine: the race's world
    /// list held its 4 racers throughout). The Bayview Bridge drag (4201F)
    /// has such points.
    fn drag_fill_finds_no_point_on_4213_backward() {
        let read = |p: &str| {
            let path = crate::testkit::game_root().join(p);
            RouteFile::parse(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        };
        let (h15, bridge) = (read("TRACKS/ROUTESL4RB/Routes4213B.bin"), read("TRACKS/ROUTESL4RB/Routes4201F.bin"));
        let any = |f: &RouteFile| {
            (0..112).any(|k| {
                [false, true].into_iter().any(|flag| drag_point(f, None, 0.0, k as f32 * 31.0, flag).is_some())
            })
        };
        // No point of 4213B's race records carries bit 25 or 26.
        assert!(
            h15.records.iter().filter(|r| r.is_race()).flat_map(|r| r.points.iter()).all(|c| c.flags >> 25 & 3 == 0)
        );
        assert!(!any(&h15), "4213B: a drag fill point");
        assert!(any(&bridge), "4201F: no drag fill point");
        let mut rng = GameRng(7);
        assert!((0..12).all(|j| TrafficLane::fill_drag(&h15, j, 31.0, None, 0.5, &|_, _, _| true, &mut rng).is_none()));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x42E2C0 on an L4RA circuit (Routes4001F): ten fills land on
    /// distinct race-record points that carry bits 25 and 26 and are
    /// neither of a record's last two, kept 31 m apart by the gate.
    fn race_traffic_stops_at_the_dead_end() {
        // Quick Race circuit 4014 forward: the traffic lanes running to
        // (−246, 307) end on a point with a low-nibble-3 junction (the
        // original's cars stop there for good under Wine, +0x710 = 1; the
        // clone's 0x417750 port, `ends_ahead`).
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/Routes4014F.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let end = Vec2::new(-246.0, 307.0);
        let mut found = false;
        for (ri, r) in f.records.iter().enumerate() {
            for (k, cp) in r.points.iter().enumerate() {
                for backward in [false, true] {
                    let ends =
                        f.junctions_at(cp).map(|j| f.junctions[j]).any(|j| j.direction() == backward && j.kind() == 3);
                    if !ends || cp.position.truncate().distance(end) > 15.0 {
                        continue;
                    }
                    let Some(lane) = (2..=5).find(|&l| has_lane(&f, Step { record: ri, cp: k, backward, lane: 0 }, l))
                    else {
                        continue;
                    };
                    let Some(prev) =
                        (if backward { Some(k + 1).filter(|&c| c < r.points.len()) } else { k.checked_sub(1) })
                    else {
                        continue;
                    };
                    let t = TrafficLane {
                        steps: VecDeque::from([
                            Step { record: ri, cp: prev, backward, lane },
                            Step { record: ri, cp: k, backward, lane },
                        ]),
                        rng: GameRng(1),
                        obey: None,
                        avoid: None,
                        roomed: 0,
                        took_avoid: false,
                        behind: None,
                        kept: Vec::new(),
                        heading: None,
                        walked: 0.0,
                        last_turn: -99_999.0,
                        turned: None,
                        lead: (0, 0.0),
                        drag: false,
                    };
                    let from = r.points[prev].position.truncate();
                    let gap = f.segment(ri, prev, backward);
                    // At 14 m/s: stopped within 14 m of the point, not before.
                    assert!(!t.ends_ahead(&f, from, 14.0) || gap < 14.0);
                    let near = from + (cp.position.truncate() - from) * ((gap - 10.0).max(0.0) / gap.max(1e-3));
                    assert!(t.ends_ahead(&f, near, 14.0));
                    found = true;
                }
            }
        }
        assert!(found, "no dead end near {end}");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn race_fill_takes_race_points() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/Routes4001F.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let mut rng = GameRng(777);
        let mut placed: Vec<Vec2> = Vec::new();
        for _ in 0..10 {
            let gate = |at: Vec2, _: f32, _: bool| !placed.iter().any(|p| p.distance(at) < 31.0);
            let Some((t, at, _)) = TrafficLane::fill_race(&f, 0.5, &gate, &mut rng) else { continue };
            let s = t.steps[0];
            let r = &f.records[s.record];
            assert!(r.is_race() && s.cp >= 1 && s.cp + 2 <= r.points.len());
            assert_eq!(r.points[s.cp].flags >> 25 & 3, 3);
            placed.push(at.truncate());
        }
        assert!(placed.len() >= 5, "{}", placed.len());
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x417D80: from a point of L4RA's free-roam roads the placer also
    /// starts on a parallel road 2–30 m beside it (a divided road's other
    /// carriageway), one per record, at most three starts.
    fn placer_starts_beside_the_road() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let (mut points, mut beside) = (0, 0);
        for (ri, r) in f.records.iter().enumerate().filter(|(_, r)| r.lanes[2..].iter().any(|&l| l)) {
            for (ci, c) in r.points.iter().enumerate() {
                let own = Step { record: ri, cp: ci, backward: ci % 2 == 0, lane: 0 };
                let all = starts(&f, own, PLACER_STARTS);
                assert!(all.len() <= PLACER_STARTS && all[0].record == ri && all[0].cp == ci);
                points += 1;
                if all.len() > 1 {
                    beside += 1;
                }
                for s in &all[1..] {
                    let o = &f.records[s.record].points[s.cp];
                    let cos = o.tangent.dot(c.tangent);
                    assert!(s.record != ri && cos.abs() > 0.85);
                    assert_eq!(s.backward, own.backward ^ (cos < 0.0));
                    let d = (o.position - c.position).truncate();
                    let across = d.x * c.tangent.y - d.y * c.tangent.x;
                    assert!(
                        across.abs() >= c.half_width[0].min(c.half_width[1]) + 2.0 - 1e-3
                            && across.abs() <= c.half_width[0].max(c.half_width[1]) + 30.0 + 1e-3,
                        "{across}"
                    );
                }
            }
        }
        eprintln!("{beside} of {points} traffic points have a road beside them");
        assert!(beside > 0 && beside < points);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x42E9F0 starts its search from the player's road node (controller
    /// +0x0C: the human controller's, kept by 0x42A190 / searched by
    /// 0x411490, `HumanRoad`), handed to `spawn_from`; only without one the
    /// nearest segment's start (`road_node`). The same node gives the same
    /// placements draw for draw; a node 1.5 km off (record 734 cp 16) puts
    /// every walk's first point beyond SPAWN_MAX of the player: no placement.
    fn the_placer_starts_from_the_players_road_node() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let player = Vec3::new(1695.25, -883.64, 24.1);
        let own = road_node(&f, player.truncate());
        let far = (f.records.iter().position(|r| r.id == 734).expect("record id 734"), 16);
        let run = |node: Option<(usize, usize)>| {
            let mut rng = GameRng(1234);
            let mut cursor = usize::MAX;
            (0..40)
                .map(|i| {
                    let speed = if i % 2 == 0 { 30.0 } else { 0.0 };
                    TrafficLane::spawn_from(&f, player, Vec2::X, speed, 0.5, 0.0, &[], &|a, b| a.distance(b), &mut rng, &mut cursor, false, node)
                        .map(|(_, p, h)| (p, h))
                })
                .collect::<Vec<_>>()
        };
        let none = run(None);
        assert!(none.iter().any(|x| x.is_some()), "no placement from the nearest segment");
        assert_eq!(run(own), none, "the node the nearest-segment rule finds places the same cars");
        assert!(run(Some(far)).iter().all(|x| x.is_none()), "a node 1.5 km off still placed a car near the player");
    }

    #[test]
    /// 0x42F800 (0x42F812, 0x42F852; verified): each car a pass tries
    /// costs the world generator one integer draw (which human) and one
    /// float (the teleporter odds) before 0x42E9F0's own draws; before
    /// the clone once drew neither, so its placements read the stream two
    /// draws early.
    fn the_placer_draws_the_racer_and_the_odds_first() {
        let mut a = GameRng(0x1234_5678);
        let mut b = a.clone();
        placer_try_draws(&mut a, 1);
        let _ = b.below(1);
        let _ = b.unit(1.0);
        assert_eq!(a.0, b.0);
        assert_ne!(a.0, 0x1234_5678);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x42E9F0 at 0x42EAB3..0x42EAFF (verified, the asm; observed
    /// under Wine, docs/evidence/traf-5): at 55 mph or more the walk byte is
    /// set only while the human's wrong-way byte (player object +0x248,
    /// 0x600720) is clear; a coordinate race then flips it on a node record
    /// with +0x70 bit 1 (0x42EB04). Race 4014's record 475 carries bit 1 and
    /// runs against its control-point order, so a player going the right way
    /// gets traffic walked backward along it, one going the wrong way
    /// forward: ahead of him either way. The clone once ignored
    /// +0x248 and placed a wrong-way player's traffic behind him.
    fn the_placer_turns_round_for_a_wrong_way_player() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RA/Routes4014F.bin")).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let rec = f.records.iter().position(|r| r.id == 475).expect("record 475");
        assert_ne!(f.records[rec].flags & 2, 0, "record 475 carries +0x70 bit 1");
        let cp = 10;
        let player = f.records[rec].points[cp].position;
        let diff = |a: f32, b: f32| a - b;
        let open = |at: Vec2, _: f32, _: bool| at.distance(player.truncate()) >= DESPAWN;
        let side = |wrong_way: bool| {
            let coord = f.records[rec].points[cp].coord;
            let race = RacePlace { diff: &diff, coord, own: None, gate: &open, wrong_way };
            let mut rng = GameRng(77);
            // The player's own road (start 0 of 0x417D80's list).
            let mut cursor = 2usize;
            let (lane, _, _) = (0..20)
                .find_map(|_| {
                    cursor = 2;
                    TrafficLane::spawn_race(&f, player, Vec2::X, 30.0, 0.5, 0.0, &[], &|a, b| a.distance(b), &mut rng, &mut cursor, true, Some((rec, cp)), Some(&race))
                })
                .expect("a placement");
            let (r, k, ..) = lane.current(&f).expect("a walk");
            (f.records[r].id, k)
        };
        // The right way: 250 m backward along 475 (cp 10 → its start) and on
        // past 557 / 454 to record 452; the wrong way: forward along 475 and
        // off it at its junction to record 435. Under Wine (n1, at speed)
        // the original placed 475:15 → 454:2 → 452:6 going the right way
        // and 475:19 → 435:12 going the wrong way.
        let (right, wrong) = (side(false), side(true));
        assert_eq!(right.0, 452, "the right way: placed along the race ({right:?})");
        assert_eq!(wrong.0, 435, "the wrong way: placed along the player's way ({wrong:?})");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x42E9F0 at 0x42ED4C..0x42EDA9 (verified, the asm): in a
    /// coordinate race a car that is not parked (controller +0x77C clear)
    /// and lies ahead of the player (car +0x8FC − racer +0x8FC > 0) is not
    /// placed ahead farther than it is; the placement fails (and 0x42F910
    /// parks it). A parked car, or one behind the player, is placed as
    /// before. The clone once moved such a car on at once.
    fn a_car_ahead_is_not_placed_farther_ahead() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let player = Vec3::new(1695.25, -883.64, 24.1);
        let diff = |a: f32, b: f32| a - b;
        // 0x418110 in 2D (free roam's points): none within 250 m of the player.
        let open = |at: Vec2, _: f32, _: bool| at.distance(player.truncate()) >= DESPAWN;
        // At 30 m/s the placer always goes ahead (0x42EAC9, 24.5865 m/s).
        let run = |own: Option<f32>| {
            let mut rng = GameRng(77);
            let mut cursor = usize::MAX;
            let race = RacePlace { diff: &diff, coord: 0.0, own, gate: &open, wrong_way: false };
            (0..60)
                .filter(|_| {
                    TrafficLane::spawn_race(&f, player, Vec2::X, 30.0, 0.5, 0.0, &[], &|a, b| a.distance(b), &mut rng, &mut cursor, false, None, Some(&race))
                        .is_some()
                })
                .count()
        };
        let parked = run(None);
        assert!(parked > 0, "no placement at all");
        // 100 m ahead of the player: every place ahead lies 250 m or more off.
        assert_eq!(run(Some(100.0)), 0, "a car 100 m ahead was placed farther ahead");
        // Behind the player, or ahead beyond the 500 m reach: no limit.
        assert_eq!(run(Some(-100.0)), parked);
        assert_eq!(run(Some(600.0)), parked);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// 0x42E9F0's reach test (0x42EBBB..0x42EC19, verified): a point
    /// measured by race coordinate (0x5DCC90) takes the signed difference
    /// point +0x08 − racer +0x8FC (0x5D5CE0, no FABS) and refuses it over
    /// 500 m, so a point behind the player never fails it; any other point
    /// its 2D distance. The clone once took the larger of the 2D
    /// distance and |Δ coordinate| for every point.
    fn the_placers_reach_is_signed_on_race_coordinate() {
        let data = std::fs::read(crate::testkit::game_root().join("TRACKS/ROUTESL4RA/Routes4014F.bin")).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let all: Vec<(usize, usize)> =
            f.records.iter().enumerate().flat_map(|(ri, r)| (0..r.points.len()).map(move |k| (ri, k))).collect();
        let by_coord = *all.iter().find(|&&(ri, k)| coordinate_point(&f, ri, k)).expect("a coordinate point");
        let flat = *all.iter().find(|&&(ri, k)| !coordinate_point(&f, ri, k)).expect("a 2D point");
        let diff = |a: f32, b: f32| a - b;
        let open = |_: Vec2, _: f32, _: bool| true;
        let c = f.records[by_coord.0].points[by_coord.1].coord;
        let race = |coord: f32| RacePlace { diff: &diff, coord, own: None, gate: &open, wrong_way: false };
        let player = f.records[by_coord.0].points[by_coord.1].position.truncate();
        // The point 800 m behind the player: −800, under the 500 m reach
        // (the clone's old measure, |Δ| or 2D, refused it).
        assert_eq!(reach_measure(&f, by_coord.0, by_coord.1, player, &race(c + 800.0)), -800.0);
        assert!(reach_measure(&f, by_coord.0, by_coord.1, player, &race(c + 800.0)) <= SPAWN_MAX);
        // 800 m ahead: refused.
        assert!(reach_measure(&f, by_coord.0, by_coord.1, player, &race(c - 800.0)) > SPAWN_MAX);
        // A point without a race coordinate: its 2D distance, whatever the coordinates.
        let q = f.records[flat.0].points[flat.1].position.truncate();
        let d = reach_measure(&f, flat.0, flat.1, player, &race(c + 1.0e6));
        assert!((d - q.distance(player)).abs() < 1e-3);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn spawns_and_follows_free_roam_lanes() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let mut rng = GameRng(1234);
        // Record lane bytes: some records carry traffic lanes, not all.
        assert!(f.records.iter().any(|r| r.lanes[2..].iter().any(|&l| l)));
        assert!(f.records.iter().any(|r| !r.lanes[2..].iter().any(|&l| l)));
        // Career free roam's start, where the placer's search is measured
        // under Wine (its road node, record 515, has no traffic lanes).
        let player = Vec3::new(1695.25, -883.64, 24.1);
        let mut spawned = 0;
        for i in 0..40 {
            let speed = if i % 2 == 0 { 30.0 } else { 0.0 };
            let Some((mut t, p, _)) =
                TrafficLane::spawn(&f, player, Vec2::X, speed, 0.5, 0.0, &[], &|a, b| a.distance(b), &mut rng)
            else {
                continue;
            };
            let d = p.truncate().distance(player.truncate());
            assert!(d <= SPAWN_MAX + 30.0 && d >= DESPAWN - 30.0, "{d}");
            let placed = t.lane_speed(&f);
            assert!(placed > 0.0 && placed < 60.0, "{placed}");
            let Some((path, speed)) = t.update(&f, p, placed) else { continue };
            assert!(!path.is_empty() && speed > 0.0 && speed <= MOVER_TOP_SPEED && speed <= placed, "{speed}");
            // Consecutive lane points are a road segment apart, not a jump.
            for w in path.windows(2) {
                assert!(w[0].distance(w[1]) < 150.0);
            }
            spawned += 1;
        }
        assert!(spawned > 0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn placer_search_spawns_where_the_original_does() {
        // Career free roam's start: under Wine (2026-10-31)
        // the placer's tries placed 25–40 % of the time, at these points
        // (the first one or two past 250 m each way along the road).
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        assert_eq!(road_node(&f, Vec2::new(1695.25, -883.64)).map(|(r, c)| (f.records[r].id, c)), Some((515, 6)));
        let seen = [
            Vec2::new(1641.0, -611.0),
            Vec2::new(1657.0, -609.0),
            Vec2::new(1640.0, -572.0),
            Vec2::new(1634.0, -535.0),
            Vec2::new(1629.0, -498.0),
            Vec2::new(1545.0, -1100.0),
            Vec2::new(1516.0, -1126.0),
            Vec2::new(1480.0, -1155.0),
        ];
        let player = Vec3::new(1695.25, -883.64, 24.1);
        let mut rng = GameRng(5);
        let (mut placed, mut near) = (0, 0);
        for _ in 0..200 {
            let Some((_, p, _)) =
                TrafficLane::spawn(&f, player, Vec2::X, 0.0, 0.5, 60.0, &[], &|a, b| a.distance(b), &mut rng)
            else {
                continue;
            };
            placed += 1;
            let d = p.truncate().distance(player.truncate());
            assert!((DESPAWN..=SPAWN_MAX).contains(&d), "{d}");
            near += usize::from(seen.iter().any(|s| s.distance(p.truncate()) < 40.0));
        }
        assert!((30..=140).contains(&placed) && near * 2 >= placed, "{placed} {near}");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn avoidance_tracks_and_ends() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let mut rng = GameRng(91);
        // Career free roam's start, where the placer's search is measured
        // under Wine (its road node, record 515, has no traffic lanes).
        let player = Vec3::new(1695.25, -883.64, 24.1);
        let (mut t, p, heading) = (0..400)
            .find_map(|i| {
                TrafficLane::spawn(
                    &f,
                    player,
                    Vec2::X,
                    if i % 2 == 0 { 30.0 } else { 0.0 },
                    0.5,
                    0.0,
                    &[],
                    &|a, b| a.distance(b),
                    &mut rng,
                )
            })
            .expect("a spawn");
        t.update(&f, p, 10.0).expect("lane");
        let dir = Vec2::from_angle(heading);
        let s0 = t.steps[0];
        let along = f.direction(s0.record, s0.cp, s0.backward);
        // A standing car 10 m ahead on the lane: the action takes it and
        // the car stops (within 15 m) or changes lanes (0x4146C0).
        let ahead = Other {
            position: p.truncate() + along * 10.0,
            z: p.z,
            velocity: Vec2::ZERO,
            forward: along,
            half: Vec2::new(2.2, 0.9),
            traffic: None,
            human: false,
            id: 7,
        };
        let own = Other {
            position: p.truncate(),
            z: p.z,
            velocity: dir * 10.0,
            forward: dir,
            half: Vec2::new(2.2, 0.9),
            traffic: Some(0),
            human: false,
            id: 0,
        };
        let mut x = Crossings::new(&f, 1);
        let lane0 = t.steps[0].lane;
        if f.intersection_at(&f.records[s0.record].points[s0.cp]).is_some()
            || t.steps.iter().any(|s| f.intersection_at(&f.records[s.record].points[s.cp]).is_some())
        {
            return;
        }
        let fresh = (t.clone(), x.clone());
        let mut rng = GameRng(1);
        let (_, target) = t.think(&f, 0, p, 10.0, &own, &mut x, &[ahead], &mut rng).expect("lane");
        let a = t.avoid.expect("avoiding");
        assert_eq!(a.target, 7);
        // 0x40D5C0 at 0x426CFF draws only at the human's car.
        assert_eq!(rng.0, 1, "no honk draws at a traffic car");
        let (mut t2, mut x2) = fresh;
        let human = Other { human: true, id: usize::MAX, ..ahead };
        let mut rng = GameRng(1);
        t2.think(&f, 0, p, 10.0, &own, &mut x2, &[human], &mut rng).expect("lane");
        assert_eq!(t2.avoid.map(|a| a.target), Some(usize::MAX));
        let draws = (1..=8).find(|&n| {
            let mut r = GameRng(1);
            (0..n).for_each(|_| {
                r.unit(1.0);
            });
            r.0 == rng.0
        });
        assert!(matches!(draws, Some(5) | Some(6)), "honk draws at the human: {draws:?}");
        assert!(target == 0.0 || t.steps[0].lane != lane0, "{target}");
        // Gone from the road: done, back to the lane's speed.
        let behind = Other { position: p.truncate() - dir * 50.0, ..ahead };
        t.think(&f, 0, p, 10.0, &own, &mut x, &[behind], &mut GameRng(1)).expect("lane");
        assert!(t.avoid.is_none());
    }

    #[test]
    fn crest_cap_pulls_towards_10_ms() {
        let at = [0.0, 10.0, 25.0, 40.0, 60.0];
        // Flat road: the cap stands.
        assert_eq!(crest_cap(24.0, &[0.0; 5], &at, 30.0), 24.0);
        // A crest (grade falling by 0.2) starting at the car: 10 m/s.
        assert_eq!(crest_cap(24.0, &[0.1, -0.1, -0.1, -0.1, -0.1], &at, 30.0), 10.0);
        // Starting 25 m on: half way between 10 m (1) and 40 m (0).
        let c = crest_cap(24.0, &[0.1, 0.1, 0.1, -0.1, -0.1], &at, 30.0);
        assert!((c - 17.0).abs() < 1e-4, "{c}");
        // Beyond the look: not seen.
        assert_eq!(crest_cap(24.0, &[0.1, 0.1, 0.1, 0.1, -0.1], &at, 30.0), 24.0);
        // A drop of exactly 0.1 does not count.
        assert_eq!(crest_cap(24.0, &[0.1, 0.0, 0.0, 0.0, 0.0], &at, 30.0), 24.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn lateral_returns_to_the_lane() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let mut rng = GameRng(77);
        // Career free roam's start, where the placer's search is measured
        // under Wine (its road node, record 515, has no traffic lanes).
        let player = Vec3::new(1695.25, -883.64, 24.1);
        let (mut t, p, heading) = (0..400)
            .find_map(|i| {
                TrafficLane::spawn(
                    &f,
                    player,
                    Vec2::X,
                    if i % 2 == 0 { 30.0 } else { 0.0 },
                    0.5,
                    0.0,
                    &[],
                    &|a, b| a.distance(b),
                    &mut rng,
                )
            })
            .expect("a spawn");
        // On the lane's centre the path is the lane; 3 m to the right it
        // comes back at 0.151 m per metre.
        let (centre, _) = t.clone().update(&f, p, 10.0).expect("lane");
        let right = Vec3::new(heading.sin(), -heading.cos(), 0.0) * 3.0;
        let (off, _) = t.update(&f, p + right, 10.0).expect("lane");
        let first = off[0].truncate().distance(centre[0].truncate());
        // The first point is the walk point after the car's (an 8 m point
        // or the next control point), the offset moved by the rate × the
        // step to it.
        let walk = t.walk_points(&f);
        let lead = t.lead.0;
        let expect = (3.0 - LATERAL_RATE * (walk[lead + 1].s - walk[lead].s)).max(0.0);
        assert!((first - expect).abs() < 0.05, "{first} {expect}");
        let last = off.last().unwrap().truncate().distance(centre.last().unwrap().truncate());
        assert!(last <= first, "{last} {first}");
        // The traffic junction types: the high nibble, 2 always, 5/7/10 never.
        let mut taken = [0u32; 16];
        for j in &f.junctions {
            taken[(j.flags >> 4 & 0xF) as usize] += 1;
        }
        assert!(taken[2] > 0 && taken[4] > 0 && taken[7] > 0);
    }

    /// A traffic walk under Wine (`walkprobe1.tsv`, free roam stage 1,
    /// car 0x3e95ff0 on lane 4, forward on record 734): its FuturePoints
    /// (+0x5E, +0x58) and lane 4's offset (+0x10) at the points 0x418DA0
    /// put between the control points, against the clone's walk.
    const NATIVE_WALK_734: [(usize, f32, f32); 12] = [
        (19, 8.0, 0.71),
        (19, 16.0, 0.42),
        (19, 24.0, -0.14),
        (20, 8.0, -0.86),
        (20, 16.0, -1.70),
        (20, 24.0, -2.22),
        (20, 32.0, -2.10),
        (21, 8.0, -0.85),
        (21, 16.0, -1.32),
        (21, 24.0, -1.39),
        (21, 32.0, -1.05),
        (18, 8.0, 0.93),
    ];

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_walk_holds_the_originals_points_and_smooth_lane_offsets() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let Some(r) = f.record(734) else { panic!("record 734") };
        let steps: VecDeque<Step> = (17..=22).map(|cp| Step { record: r, cp, backward: false, lane: 4 }).collect();
        let t = TrafficLane {
            steps,
            rng: GameRng(1),
            obey: None,
            avoid: None,
            roomed: 0,
            took_avoid: false,
            behind: None,
            kept: Vec::new(),
            heading: None,
            walked: 0.0,
            last_turn: -99_999.0,
            turned: None,
            lead: (0, 0.0),
            drag: false,
        };
        let walk = t.walk_points(&f);
        // 0x418DA0: trunc(segment / 8) − 1 points every 8 m: the original's
        // walk had 4 on control points 20 and 21, 3 on 19, 1 on 18.
        let count = |cp: usize| walk.iter().filter(|w| t.steps[w.step].cp == cp && w.d > 0.0).count();
        assert_eq!((count(18), count(19), count(20), count(21)), (1, 3, 4, 4));
        for &(cp, d, x) in &NATIVE_WALK_734 {
            let w = walk.iter().find(|w| t.steps[w.step].cp == cp && (w.d - d).abs() < 1e-3).expect("the point");
            // Within 0.02 m; 18's run starts the clone's walk, where the
            // original's tangent came from a point it had already dropped.
            let tol = if cp == 18 { 0.05 } else { 0.02 };
            assert!((w.x[3] - x).abs() < tol, "cp {cp} +{d}: {} against the original's {x}", w.x[3]);
        }
        // The control points' own offsets stay 0 (0x426810).
        assert!(walk.iter().filter(|w| w.d == 0.0).all(|w| w.x == [0.0; 5]));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_walk_reaches_as_far_as_the_originals() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let mut rng = GameRng(5);
        let player = Vec3::new(1695.25, -883.64, 24.1);
        let mut spans = Vec::new();
        for i in 0..200 {
            let Some((mut t, p, _)) = TrafficLane::spawn(
                &f,
                player,
                Vec2::X,
                if i % 2 == 0 { 30.0 } else { 0.0 },
                0.5,
                0.0,
                &[],
                &|a, b| a.distance(b),
                &mut rng,
            ) else {
                continue;
            };
            for speed in [10.0, 40.0] {
                if t.update(&f, p, speed).is_none() {
                    continue;
                }
                let walk = t.walk_points(&f);
                let last = walk.last().unwrap().s;
                // 0x428030 → 0x422220: at least max(125 m, 4 s × speed) and
                // more than 9 points, at most 40 (unless the road ends).
                assert!(walk.len() <= WALK_MAX + 8, "{}", walk.len());
                if walk.len() < WALK_MAX - 8 {
                    let need = (ENTRY_REACH.max(ENTRY_REACH_TIME * speed)).min(last + 1.0);
                    assert!(last >= need - 1e-3, "{last} {need}");
                    assert!(walk.len() >= WALK_MIN || last >= ENTRY_REACH.max(ENTRY_REACH_TIME * speed));
                }
                if speed == 10.0 {
                    spans.push(last);
                }
            }
        }
        spans.sort_by(f32::total_cmp);
        assert!(spans.len() > 20);
        // Under Wine the walks spanned 126 / 135 / 150 m (p10 / 50 / 90).
        let median = spans[spans.len() / 2];
        assert!((125.0..175.0).contains(&median), "{median}");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_walk_steps_by_the_jump_at_a_junction() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let r = |id: i16| f.record(id).expect("record");
        // Observed under Wine (walks): 540:1 → 495:1 backward at s + 4.9,
        // 731:10 → 477:24 at s + 13.3; the records' own segments are 25.1 / 5.0.
        let a = Step { record: r(540), cp: 1, backward: true, lane: 4 };
        let b = Step { record: r(495), cp: 1, backward: true, lane: 4 };
        assert!((gap(&f, a, b) - 4.9).abs() < 0.1, "{}", gap(&f, a, b));
        let a = Step { record: r(731), cp: 10, backward: false, lane: 4 };
        let b = Step { record: r(477), cp: 24, backward: false, lane: 4 };
        assert!((gap(&f, a, b) - 13.3).abs() < 0.15, "{}", gap(&f, a, b));
        // Within a record the step is about the point's segment.
        let a = Step { record: r(540), cp: 3, backward: true, lane: 4 };
        let b = Step { record: r(540), cp: 2, backward: true, lane: 4 };
        assert!((gap(&f, a, b) - f.segment(a.record, a.cp, a.backward)).abs() < 0.3);
        // 540:6 → 734:23 lies 3.7 m on: no 8 m points on 540:6 (its own
        // segment is 41 m), so none of them lie past the junction.
        let a = Step { record: r(540), cp: 6, backward: false, lane: 4 };
        let b = Step { record: r(734), cp: 23, backward: true, lane: 4 };
        let mut t = TrafficLane {
            steps: VecDeque::from([a, b, Step { cp: 22, ..b }]),
            rng: GameRng(1),
            obey: None,
            avoid: None,
            roomed: 0,
            took_avoid: false,
            behind: None,
            kept: Vec::new(),
            heading: None,
            walked: 0.0,
            last_turn: -99_999.0,
            turned: None,
            lead: (0, 0.0),
            drag: false,
        };
        let walk = t.walk_points(&f);
        assert_eq!(walk.iter().filter(|w| w.step == 0).count(), 1);
        assert!((walk[1].s - 3.7).abs() < 0.2, "{}", walk[1].s);
        // The car 20 m past 734:23 on its way down: its walk starts at 734:23's 16 m point.
        let p = f.point(b.record, b.cp, true, 20.0, 0.0);
        assert!(t.update(&f, p, 0.0).is_some());
        assert_eq!((t.steps[0].record, t.steps[0].cp), (b.record, 23));
        assert_eq!(t.lead, (2, 16.0));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_walk_passes_over_points_under_3_m() {
        // 0x422220: a new point less than 3 m on from the last is passed over.
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        let mut rng = GameRng(9);
        let player = Vec3::new(460.0, -46.0, 18.0);
        let mut n = 0;
        for i in 0..200 {
            let Some((mut t, p, _)) = TrafficLane::spawn(
                &f,
                player,
                Vec2::X,
                if i % 2 == 0 { 15.0 } else { 0.0 },
                0.5,
                0.0,
                &[],
                &|a, b| a.distance(b),
                &mut rng,
            ) else {
                continue;
            };
            if t.update(&f, p, 10.0).is_none() {
                continue;
            }
            for k in 1..t.steps.len() {
                let g = gap(&f, t.steps[k - 1], t.steps[k]);
                // The placement's own first point is not gated (0x426810).
                assert!(k == 1 || g >= MIN_STEP, "{g} at {k}");
            }
            n += 1;
        }
        assert!(n > 50);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_standing_avoider_takes_the_lane_it_stands_in() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).expect("reading the game's data file");
        let f = RouteFile::parse(&data);
        // Record id 401 (index 222), cp 13: lanes 2 / 3 / 4 at −5.77 / −2.00 / +1.92.
        let st = Step { record: f.record(401).expect("record"), cp: 13, backward: false, lane: 4 };
        let at = |l: f32| f.point(st.record, st.cp, false, 2.0, l).truncate();
        assert_eq!(standing_lane(&f, st, at(-2.4)), Some(3));
        assert_eq!(standing_lane(&f, st, at(-4.5)), Some(2));
        assert_eq!(standing_lane(&f, st, at(1.0)), Some(4));
        assert_eq!(standing_lane(&f, st, at(-0.1)), Some(3), "nearer −2.00 than +1.92");
        assert_eq!(standing_lane(&f, st, at(0.1)), Some(4));
    }

    #[test]
    fn dense_pool_scales_free_roam_only_by_choice() {
        assert_eq!(dense_pool(16, 0), 16);
        assert_eq!(dense_pool(16, 1), 24);
        assert_eq!(dense_pool(16, 2), 32);
        assert_eq!(dense_pool(0, 2), 0);
        assert_eq!(dense_pool(20, 2), MAX_DENSE_CARS);
    }

    #[test]
    fn pool_and_models() {
        // Track 4000's bytes: level 3 forward is 16; none at level 0.
        let t = [12, 12, 10, 10, 12, 12, 16, 16];
        assert_eq!(pool_size(&t, 3, false), 16);
        assert_eq!(pool_size(&t, 1, true), 10);
        assert_eq!(pool_size(&t, 0, false), 0);
        // Caps of zone 0: three types never drive (0), the rest 3 or 5.
        let caps = [5, 5, 0, 5, 3, 5, 0, 3, 5, 5, 0, 5, 5, 5, 5];
        let models = assign_models(&caps, 16, &mut GameRng(99));
        let mut seen: Vec<usize> = models.clone();
        seen.sort();
        seen.dedup();
        assert!(seen.len() <= MAX_MODELS);
        assert!(models.iter().all(|&m| caps[m] > 0));
        for &m in &seen {
            assert!(models.iter().filter(|&&x| x == m).count() <= caps[m] as usize);
        }
    }

    #[test]
    fn mover_turns_and_holds_its_speed() {
        assert_eq!(mover_table(&MOVER_ACCEL, 0.0), MOVER_ACCEL[0]);
        assert_eq!(mover_table(&MOVER_BRAKE, 30.0), MOVER_BRAKE[3]);
        let mid = mover_table(&MOVER_ACCEL, 0.5 / 0.122_018_18);
        assert!((mid - (MOVER_ACCEL[0] + MOVER_ACCEL[1]) / 2.0).abs() < 1e-4);
        // Straight at full gas: capped at 55 mph.
        let mut m = TrafficMover { position: Vec2::ZERO, heading: 0.0, speed: 20.0 };
        for _ in 0..600 {
            m.step(-1.3, 2.6, 0.0, 1.0, 0.0, 1.0 / 60.0);
        }
        assert_eq!(m.speed, MOVER_TOP_SPEED);
        assert!(m.position.y.abs() < 1e-3 && m.position.x > 200.0, "{}", m.position);
        // A left turn at 10 m/s: radius wheelbase / steer about the rear axle.
        let mut m = TrafficMover { position: Vec2::ZERO, heading: 0.0, speed: 10.0 };
        let steer = 0.1f32;
        let r = 2.6 / steer;
        let pivot = Vec2::new(-1.3, r);
        for _ in 0..60 {
            m.step(-1.3, 2.6, steer, 0.0, 0.0, 1.0 / 60.0);
        }
        assert!((m.heading - 10.0 / r).abs() < 1e-3);
        assert!((m.position.distance(pivot) - Vec2::new(1.3, r).length()).abs() < 1e-3);
        // Braking stops at 0.
        let mut m = TrafficMover { position: Vec2::ZERO, heading: 1.0, speed: 1.0 };
        m.step(-1.3, 2.6, 0.0, 0.0, 1.0, 1.0);
        assert_eq!(m.speed, 0.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn intersections_and_zones() {
        let root = crate::testkit::game_root().join("TRACKS/ROUTESL4RA");
        let routes = std::fs::read(root.join("RoutesFreeRoam.bin")).expect("RoutesFreeRoam.bin");
        let paths = std::fs::read(root.join("PathsFreeRoam.bin")).expect("PathsFreeRoam.bin");
        let f = RouteFile::parse(&routes);
        // 38 intersections, all kind 1; each road's crossing point carries
        // the intersection's index in its flags.
        assert_eq!(f.intersections.len(), 38);
        assert!(f.intersections.iter().all(|x| x.kind == 1));
        for (k, x) in f.intersections.iter().enumerate() {
            for road in 0..2 {
                let (r, c) = f.crossing_point(x, road).expect("crossing point");
                assert_eq!(f.intersection_at(&f.records[r].points[c]), Some(k));
            }
            // The other road's edge lies before the crossing point.
            let back = stop_back(&f, k, Some(0), false);
            assert!(back.abs() < 40.0, "{back}");
        }
        // Four traffic zones; level 3 reads rows 6–8, caps zones 0–3.
        let zones = TrafficZone::parse(&paths);
        assert_eq!(zones.len(), 4);
        let mut rows: Vec<usize> = zones.iter().filter_map(|z| z.row(3)).collect();
        rows.sort();
        assert_eq!(rows, vec![6, 6, 7, 8]);
        assert_eq!(zones[0].row(1), Some(0));
        assert!(zones.iter().all(|z| z.caps < 5));
        let cp =
            f.records.iter().flat_map(|r| &r.points).find(|c| TrafficZone::at(&zones, c.position.truncate()).is_some());
        let table = [
            (240.0, 300.0),
            (110.0, 145.0),
            (80.0, 95.0),
            (90.0, 110.0),
            (70.0, 100.0),
            (50.0, 65.0),
            (50.0, 70.0),
            (35.0, 45.0),
            (20.0, 25.0),
        ];
        let p = cp.expect("a point in a zone").position.truncate();
        let row = TrafficZone::at(&zones, p).and_then(|z| z.row(3)).unwrap();
        let mut rng = GameRng(5);
        let g = spacing(&zones, &table, 3, p, false, &mut rng).unwrap();
        assert!(g >= table[row].0 && g <= table[row].1, "{g}");
        assert_eq!(spacing(&zones, &table, 3, p, true, &mut rng), None);
        assert_eq!(spacing(&zones, &table, 3, Vec2::splat(1e6), false, &mut rng), None);
    }

    #[test]
    fn crossings_take_turns() {
        let f = RouteFile {
            intersections: vec![crate::formats::ai_routes::Intersection { kind: 1, roads: [1, 2], points: [0, 0] }],
            ..Default::default()
        };
        let mut x = Crossings::new(&f, 4);
        // Free: road 0 may go; road 1 only while road 0's nearest car is
        // 3 s or more away.
        assert!(x.may_go(0, 1, 0) && x.may_go(0, 1, 1));
        x.slots[0][0] = Some(Slot { road: 0, dist: 20.0, time: 2.0 });
        assert!(x.may_go(0, 1, 0) && !x.may_go(0, 1, 1));
        x.slots[0][0] = Some(Slot { road: 0, dist: 40.0, time: 4.0 });
        assert!(x.may_go(0, 1, 1));
        // Road 1 goes and holds it while its car is within 12 m.
        x.claim(0, 1);
        x.slots[0][1] = Some(Slot { road: 1, dist: 5.0, time: 1.0 });
        x.tick();
        assert_eq!(x.holds(0), Some(1));
        assert!(!x.may_go(0, 1, 0));
        x.leave(1);
        x.tick();
        assert_eq!(x.holds(0), None);
        // Kind 3 never lets a car go by itself.
        assert!(!x.may_go(0, 3, 0));
        // Thinks take turns: one car every other update.
        let slots: Vec<Option<usize>> = (0..8).map(|u| think_slot(u, 4)).collect();
        assert_eq!(slots, vec![None, Some(0), None, Some(1), None, Some(2), None, Some(3)]);
    }
}
