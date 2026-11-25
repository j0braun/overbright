//! Issue #236: the Performance Tuning / Dyno entry from free roam (the
//! career performance shop, the safe house) with an encounter racer about.
//! The dyno track is another region (L4RG); a racer left over from the
//! city's encounters was drawn on it (the dyno's scene runs in Explore
//! mode) and its shadow asked the dyno's ground with its city ground
//! state: a collision pack index past the dyno's packs, a panic in
//! `TrackGround::query`. Real assets, no GPU: the draw's own car set
//! (`DrawnCars`) and the shadow's ground query (`car_shadow_in_game`).
use super::*;
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// What the frame's shadows ask of the ground for every car they draw
/// besides the player's: each wheel's ground, with a copy of the car's
/// ground state (as `render::native::shadows::car_shadow_in_game`).
fn drawn_cars_ground(game: &Game) -> usize {
    use crate::render::frame::cars::{CarSet, DrawnCars, TrafficSet};
    let d = game.sim.drive.as_ref().unwrap();
    let poses = crate::render::snapshot::FramePoses::live(Some(d));
    let cars = DrawnCars::of_parts(
        &game.mode,
        game.sim.paused_race.as_ref(),
        game.sim.traffic.as_ref(),
        game.sim.encounters.as_ref(),
        game.scenes.shop.as_ref(),
        &poses,
        CarSet { paused_race: true, traffic: TrafficSet::Live },
    );
    let mut n = 0;
    for (car, state) in cars.companion.into_iter().chain(cars.on_road()) {
        let mut q = state.ground.clone();
        for w in &car.car_type.wheels {
            let _ = d.world.ground_for(&mut q, state.transform().transform_point3(w.position));
        }
        n += 1;
    }
    n
}

#[test]
#[cfg_attr(
    not(all(ob_game, ob_private)),
    ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)"
)]
fn dyno_entry_with_an_encounter_racer_about() {
    testkit::game();
    let save = nfsu2::formats::native_save::NativeSave::parse(&testkit::private(
        "tests/fixtures/native_saves/career_complete.sav",
    ))
    .unwrap();
    let mut game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    game.settings.apply_native(&save);
    game.profile = Some(Profile::from_native(&save, ctx));
    game.explore(None).unwrap();
    // City floors spread over the map: the racer's ground state ends up
    // on many packs, most of them past the dyno's count.
    let (floors, city_packs) = {
        let d = game.sim.drive.as_ref().unwrap();
        let floors: Vec<Vec3> = d
            .world
            .ground
            .faces()
            .filter(|f| f.flags & 0x0B == 0)
            .map(|f| (f.vertices[0] + f.vertices[1] + f.vertices[2]) / 3.0)
            .collect();
        (floors, d.world.ground.grids().count())
    };
    let runs: usize = std::env::var("B236_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(24);
    let mut past_dyno = 0;
    for i in 0..runs {
        let at = floors[(i * 7919 + 13) % floors.len()];
        game.explore(Some((at + Vec3::Z, i as f32 * 0.7))).unwrap();
        game.mode = Mode::Explore;
        // An encounter racer next to the player, its ground found there.
        {
            let d = game.sim.drive.as_ref().unwrap();
            let e = game.sim.encounters.as_mut().expect("free roam's encounters");
            e.ready();
            let r = &mut e.cars[0];
            r.active = true;
            r.state = CarState::placed(&r.car.car_type, &d.world, d.state.position + d.state.forward() * 6.0, 0.0);
            let p = r.state.position;
            let _ = d.world.ground_for(&mut r.state.ground, p);
        }
        game.advance_for(0.1).unwrap();
        let pack = game.sim.encounters.as_ref().unwrap().cars[0].state.ground.pack();
        // Alternately the performance shop's and the safe house's entry.
        game.dyno.from = if i % 2 == 0 { 1 } else { 0 };
        if i % 2 == 1 {
            game.mode = game.icon_menu(MenuKind::Crib);
        }
        game.dyno.tuning_mode = 0;
        game.customize_dyno_yes().unwrap();
        assert!(game.on_dyno_track(), "run {i}: on the dyno");
        let dyno_packs = game.sim.drive.as_ref().unwrap().world.ground.grids().count();
        past_dyno += pack.is_some_and(|p| p >= dyno_packs) as usize;
        // The DynoEnter scene and the dyno's first menu, frame by frame.
        for _ in 0..60 {
            drawn_cars_ground(&game);
            game.advance_for(1.0 / 30.0).unwrap();
        }
        assert!(
            game.sim.encounters.as_ref().is_none_or(|e| e.cars.iter().all(|c| !c.active)),
            "run {i}: the city's encounter racers stay in the city"
        );
        game.quit_tuning();
        game.mode = game.after_tuning();
        eprintln!("run {i}: racer on pack {pack:?} of {city_packs}, the dyno has {dyno_packs}");
    }
    assert!(past_dyno > runs / 4, "only {past_dyno} of {runs} runs put the racer on a pack the dyno lacks");
}
