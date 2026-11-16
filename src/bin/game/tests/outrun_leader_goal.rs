//! The other encounter cars' goal 5 on the outrun's
//! leader.

use crate::*;

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
    assert_eq!(goals[..3], [Some(false), Some(true), Some(true)]);
    let kept: Vec<bool> = e.cars.iter().map(|c| c.controller.actions.is_some()).collect();
    // The player takes the lead: goal 5 for every AI car; the others'
    // goal 5 is only retargeted (their actions stay).
    e.outrun.as_mut().unwrap().player_leads = true;
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    let goals: Vec<_> = e.cars.iter().map(|c| c.controller.goal).collect();
    assert_eq!(goals[..3], [Some(true), Some(true), Some(true)]);
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

/// Native (**traced**): after an outrun ends (0x5F84A0 →
/// 0x42EFB0, the manager in DEACTIVATE_OPPONENT, state 5) nothing assigns a
/// goal — goals 5 / 6 are built only in state 4 (0x42A9E0 / 0x42EF36) and
/// goal 9 only in state 3 (0x42A860) — so the cars still out keep their
/// goal 5 / 6 until 0x40BF60 / 0x40BFB0 park them; the clone put them back
/// on goal 9 (`goal9_think`) once `outrun` was cleared.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn outrun_cars_keep_their_goals_after_the_end() {
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
    assert_eq!(racing[..3], [Some(true), Some(true), Some(true)]);
    // The end (`Encounters::race`: the result taken, `ended` kept), then
    // state 5: the same goals, think after think.
    e.ended = e.outrun.take();
    e.state = 5;
    for _ in 0..30 {
        e.think(ai, 1.0 / 60.0, d, node, &[], None);
        let goals: Vec<_> = e.cars.iter().map(|c| c.controller.goal).collect();
        assert_eq!(goals[..3], racing[..3], "the goals stay after the end");
    }
    // Back in state 0 (`ended` cleared there): goal 9 again.
    e.ended = None;
    e.state = 3;
    e.think(ai, 1.0 / 60.0, d, node, &[], None);
    assert!(e.cars.iter().all(|c| c.controller.goal.is_none()));
}
