//! The cars' ground effects (0x5F5740, `particles::GroundFx`) on Bayview:
//! the tyre smoke's rates at a burnout, the deep water's spray and the
//! bottom-out sparks of a landing, counted in the particle system after
//! the native update cadence (one car update per 1/60 s world update).
use super::*;

fn count(fx: &Effects, world: &nfsu2::world::World, prefix: &str) -> usize {
    fx.system
        .particles
        .iter()
        .filter(|p| world.emitters.descriptions[p.description as usize].name.starts_with(prefix))
        .count()
}

/// A floor face's centre of a surface (the first in the track's packs).
fn floor_of(world: &nfsu2::world::World, surface: u8) -> Vec3 {
    let f = world.ground.faces().find(|f| f.surface == surface && !f.is_wall()).expect("a floor of the surface");
    f.vertices[..f.corners].iter().fold(Vec3::ZERO, |s, v| s + *v) / f.corners as f32
}

fn on_ground(state: &mut CarState, surface: u8) {
    for w in &mut state.wheels {
        w.on_ground = true;
        w.surface = surface;
        w.spin = 20.0;
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn ground_effects_fire_as_the_original() {
    let root = testkit::game_root();
    let world = World::load(&root, "L4RA").unwrap();
    let car = Car::load(&root, "240SX").unwrap();
    let settings = crate::sim::TickSettings::take(&[0; 22]);
    let dt = 1.0 / 60.0;

    // A burnout: the rear wheels spin 12 m/s faster than the car rolls.
    // XETT_RoadSp_SmkA (rate 1) bursts 1 particle a rear wheel each update
    // (intensity (12 − 2) / 198); XETT_RoadSk_SmkA joins after 0.15 s of
    // the slip past 5 m/s (intensity 7 / 35 × 0.25), 1 a wheel each update.
    let road = floor_of(&world, 1);
    let mut state = CarState::new(road + Vec3::Z * 0.5, 0.0);
    on_ground(&mut state, 1);
    for w in &mut state.wheels[2..] {
        w.exported_slip = 12.0;
    }
    let mut fx = Effects::default();
    let view = state.position - state.forward() * 6.0;
    for _ in 0..60 {
        fx.step(&world, view, &[Some((&car, &state))], &[], dt, &settings);
    }
    assert_eq!(count(&fx, &world, "XETT_RoadSp"), 120, "1 a rear wheel per update");
    let skid = count(&fx, &world, "XETT_RoadSk");
    assert!((100..=104).contains(&skid), "skid smoke from the ninth update on: {skid}");
    assert_eq!(fx.ground_cars[&0].0.skid_time[0], 0.0, "the gripping front wheels never time a skid");
    // The spin smoke leaves behind the wheel: the template's z axis points
    // to the car's back (the clone had sent it up from the contact).
    let back: f32 = fx
        .system
        .particles
        .iter()
        .filter(|p| world.emitters.descriptions[p.description as usize].name.starts_with("XETT_RoadSp"))
        .map(|p| p.velocity().dot(state.forward()))
        .sum();
    assert!(back < 0.0, "{back}");
    // Out of the listener's 40 m the spin smoke stops; the skid's reach is 150 m.
    let mut far = Effects::default();
    for _ in 0..60 {
        far.step(&world, state.position + Vec3::X * 60.0, &[Some((&car, &state))], &[], dt, &settings);
    }
    assert_eq!(count(&far, &world, "XETT_RoadSp"), 0);
    assert!(count(&far, &world, "XETT_RoadSk") > 90);

    // A landing: the bump stops' sum 1.5 (event 4, 1..2) bursts XEBO_ROAD
    // at 0.5: the sparks (rate 100) 1 + 0.5 / 0.01, the flash (rate 2) 2,
    // the smoke (rate 0) 1 + 0.5 / 0.1 (the summed waits' rounding may
    // take the last one off).
    let mut land = state.clone();
    for w in &mut land.wheels {
        w.exported_slip = 0.0;
    }
    land.bump_totals = [0.375; 4];
    let mut fx = Effects::default();
    fx.step(&world, view, &[Some((&car, &land))], &[], dt, &settings);
    let sparks = count(&fx, &world, "XEBO_ROAD_SprksA");
    assert!((50..=51).contains(&sparks), "{sparks}");
    assert_eq!(count(&fx, &world, "XEBO_ROAD_FlashA"), 2);
    assert!((5..=6).contains(&count(&fx, &world, "XEBO_ROAD_SmokeA")));
    land.bump_totals = [0.0; 4];
    let before = fx.system.particles.len();
    fx.step(&world, view, &[Some((&car, &land))], &[], dt, &settings);
    assert_eq!(count(&fx, &world, "XEBO_ROAD_SprksA"), sparks, "only the update with the hit");
    assert!(fx.system.particles.len() <= before);

    // Deep water (Bayview's four DEEP_WATER floors): driving through at
    // 20 m/s (44.7 mph) fires the slow drops (DRSL 3..30 mph, cut back by
    // twice the excess over 1) and the fast ones (DR 35..100 mph).
    let water = floor_of(&world, 14);
    let mut wet = CarState::new(water + Vec3::Z * 0.4, 0.0);
    on_ground(&mut wet, 14);
    wet.velocity = wet.forward() * 20.0;
    let mut fx = Effects::default();
    let view = wet.position - wet.forward() * 6.0;
    fx.step(&world, view, &[Some((&car, &wet))], &[], dt, &settings);
    assert_eq!(fx.ground_cars[&0].0.skid_time, [0.0; 4]);
    let slow = count(&fx, &world, "XETT_DpWtrDRsl");
    let fast = count(&fx, &world, "XETT_DpWtrDRF");
    // DRSL (rate 5, four emitters): 1 + ⌊0.70 · 5⌋ = 4 each; DR (rate 2
    // and 4, three emitters): 1 each.
    assert_eq!((slow, fast), (16, 3), "the spray through deep water");
    // On the road beside it, no spray.
    let mut dry = Effects::default();
    let mut road_car = wet.clone();
    road_car.position = road + Vec3::Z * 0.5;
    on_ground(&mut road_car, 1);
    dry.step(&world, road_car.position - road_car.forward() * 6.0, &[Some((&car, &road_car))], &[], dt, &settings);
    assert_eq!(count(&dry, &world, "XETT_DpWtr") + count(&dry, &world, "XETT_Puddle"), 0);
}
