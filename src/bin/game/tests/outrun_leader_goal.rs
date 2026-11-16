//! The other encounter cars' goal 5 on the outrun's
//! leader.

use crate::*;
use nfsu2::ai::encounter_controller::EncounterGoal::{Chase, Lead, RoadRace};

/// Native (0x42EDF0 state 4 while race type 7, 0x42EF04..0x42EF36 →
/// 0x4285A0; **verified**): on every manager update, L = race
/// order 1; every racer at order ≥ 2 that is out and of car kind 2–4 gets
/// goal 5 on L — a new goal 5 when its goal is not type 5 (0x402070 drops
/// the old goal and its action), else only the target changes (the
/// action is kept). The challenged car keeps goal 6 while it leads.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn outrun_cars_out_chase_the_leader() {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("OutrunGoal5");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let e = game.sim.encounters.as_mut().unwrap();
    e.ready();
    assert!(e.cars.len() >= 3, "three encounter racers");
    let routes = e.routes.clone();
    let spots: Vec<(Vec3, Vec2)> = routes
        .file
        .records
        .iter()
        .enumerate()
        .flat_map(|(i, record)| {
            record.points.iter().enumerate().filter(|(_, p)| p.flags & 0x800 != 0).map(move |(cp, _)| (i, cp))
        })
        .step_by(7)
        .take(3)
        .map(|(rec, cp)| (routes.file.records[rec].points[cp].position, routes.file.direction(rec, cp, false)))
        .collect();
    for (r, &(position, direction)) in e.cars.iter_mut().zip(&spots) {
        r.active = true;
        r.state = CarState::new(position, direction.y.atan2(direction.x));
        r.state.velocity = direction.extend(0.0) * 20.0;
        r.state.publish_forward_speed();
        r.route = nfsu2::ai::route::RacerRoute::start(&routes, position, direction);
        r.controller = Default::default();
    }
    let d = game.sim.drive.as_ref().unwrap();
    let ai = game.sim.ai.as_ref().unwrap();
    // Before the challenge (state 3): goal 9 for all (no goal 5 / 6).
    e.state = 3;
    let node = e.player_node(&d.state);
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    assert!(e.cars.iter().all(|c| c.controller.goal.is_none()));
    // Racing, the challenged car (0) leads: it holds goal 6, the others
    // goal 5 (on it).
    e.state = 4;
    e.outrun = Some(Outrun { opponent: 0, clock: 0.0, start_tick: 4000, player: 10.0, rival: 0.0, player_leads: false });
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    let goals: Vec<_> = e.cars.iter().map(|c| c.controller.goal).collect();
    assert_eq!(goals[..3], [Some(Lead), Some(Chase), Some(Chase)]);
    let kept: Vec<bool> = e.cars.iter().map(|c| c.controller.actions.is_some()).collect();
    // The player takes the lead: goal 5 for every AI car; the others'
    // goal 5 is only retargeted (their actions stay).
    e.outrun.as_mut().unwrap().player_leads = true;
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    let goals: Vec<_> = e.cars.iter().map(|c| c.controller.goal).collect();
    assert_eq!(goals[..3], [Some(Chase), Some(Chase), Some(Chase)]);
    for i in 1..3 {
        if kept[i] {
            assert!(e.cars[i].controller.actions.is_some(), "car {i}: a retarget keeps the action");
        }
    }
    // A parked car (not out) is skipped.
    e.cars[2].active = false;
    e.cars[2].controller = Default::default();
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    assert_eq!(e.cars[2].controller.goal, None);
}

/// Native (**verified**): the outrun's end (0x5F84A0) calls 0x42EFB0,
/// whose first call 0x42CD60 gives every AI racer out (0x5ED440) whose goal
/// is not of type 1 the goal "Win road race" (0x42AA20(1) → 0x40CA20, type
/// 1, vtable 0x784768; 0x402070 drops the old goal and its action). The cars
/// drive on with it until 0x40BF60 / 0x40BFB0 park them. **Observed** under
/// Wine (outrun_brake_p14 b1–b3): goal 1 on the end's frame, action 6 the
/// next, the catch-up 0.75 and the target at three quarters of the race
/// pace (b1 43 m/s, speed 57) until the car is parked about 2 s later. The
/// clone had kept goals 5 / 6 at the outrun's pace.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn outrun_cars_take_goal_1_after_the_end() {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("OutrunGoal5");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let e = game.sim.encounters.as_mut().unwrap();
    e.ready();
    assert!(e.cars.len() >= 3, "three encounter racers");
    let routes = e.routes.clone();
    let spots: Vec<(Vec3, Vec2)> = routes
        .file
        .records
        .iter()
        .enumerate()
        .flat_map(|(i, record)| {
            record.points.iter().enumerate().filter(|(_, p)| p.flags & 0x800 != 0).map(move |(cp, _)| (i, cp))
        })
        .step_by(7)
        .take(3)
        .map(|(rec, cp)| (routes.file.records[rec].points[cp].position, routes.file.direction(rec, cp, false)))
        .collect();
    for (r, &(position, direction)) in e.cars.iter_mut().zip(&spots) {
        r.active = true;
        r.state = CarState::new(position, direction.y.atan2(direction.x));
        r.state.velocity = direction.extend(0.0) * 20.0;
        r.state.publish_forward_speed();
        r.route = nfsu2::ai::route::RacerRoute::start(&routes, position, direction);
        r.controller = Default::default();
    }
    let d = game.sim.drive.as_ref().unwrap();
    let ai = game.sim.ai.as_ref().unwrap();
    let node = e.player_node(&d.state);
    e.state = 4;
    e.outrun = Some(Outrun { opponent: 0, clock: 0.0, start_tick: 4000, player: 0.0, rival: 10.0, player_leads: true });
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    let racing: Vec<_> = e.cars.iter().map(|c| c.controller.goal).collect();
    assert_eq!(racing[..3], [Some(Chase), Some(Chase), Some(Chase)]);
    // The end (`Encounters::race`: the result taken, `ended` kept), then
    // state 5: goal 1 for every car out, its action dropped once and then
    // chosen afresh (a same-goal think keeps it).
    e.ended = e.outrun.take();
    e.state = 5;
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    let goals: Vec<_> = e.cars.iter().map(|c| c.controller.goal).collect();
    assert_eq!(goals[..3], [Some(RoadRace); 3], "goal 1 after the end");
    for _ in 0..30 {
        e.think(ai, 1.0 / 60.0, d, node, &[], None);
        let goals: Vec<_> = e.cars.iter().map(|c| c.controller.goal).collect();
        assert_eq!(goals[..3], [Some(RoadRace); 3], "goal 1 stays");
    }
    // Goal 1's speed: the offline racer's × 0.75 with no race on (no
    // leader's 0.84, no pursuit cap).
    for c in &e.cars[..3] {
        let route = c.route.as_ref().unwrap();
        let mut probe = c.speed.clone();
        let mut rng = nfsu2::opponents::GameRng(7);
        let full = probe.update(&ai.tuning, route, &mut rng, 1e6, None, 1.0 / 60.0);
        assert!(c.driver_plan.1 <= full * 0.75 * 1.05 + 0.5, "target {} against 0.75 × {}", c.driver_plan.1, full);
    }
    // Back in state 0 (`ended` cleared there): goal 9 again.
    e.ended = None;
    e.state = 3;
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    assert!(e.cars.iter().all(|c| c.controller.goal.is_none()));
}
