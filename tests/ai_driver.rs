//! The AI driver (src/ai/driver/) driving the clone's cars.

mod common;

use glam::Vec3;
use nfsu2::ai::driver::{AiDriver, DriverInputs};

fn on_road(world: &nfsu2::world::World, car: &nfsu2::car::Car, start: Vec3, heading: f32) -> nfsu2::drive::CarState {
    let w = car.car_type.wheels[0];
    let road = world.road_height_below(start).unwrap();
    nfsu2::drive::CarState::new(Vec3::new(start.x, start.y, road - (w.position.z - w.radius)), heading)
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn steers_toward_a_point_on_the_left() {
    let root = common::game_root();
    let car = nfsu2::car::Car::load(std::path::Path::new(&root), "240SX").unwrap();
    let h = &car.car_type.handling;
    let mut state = nfsu2::drive::CarState::new(Vec3::ZERO, 0.0);
    state.velocity = Vec3::new(15.0, 0.0, 0.0);
    for w in &mut state.wheels {
        w.on_ground = true;
    }
    // A path bending off to the left (+y).
    let path: Vec<Vec3> = (1..=20).map(|i| Vec3::new(i as f32 * 5.0, (i as f32 * 0.6).powi(2), 0.0)).collect();
    let mut driver = AiDriver::new(h, false, 1.0);
    let mut out = Default::default();
    for f in 0..30 {
        let input = DriverInputs { path: &path, target_speed: 15.0, enabled: true, green: true, countdown: false, since_green: 10.0, clock: 10.0 + f as f64 / 60.0 };
        out = driver.update(&state, h, false, &input, 1.0 / 60.0);
    }
    let out: nfsu2::drive::AiControls = out;
    assert!(out.steer_degrees > 0.0, "steer {}", out.steer_degrees);
    assert_eq!(out.gear, -1);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drives_a_straight_path_at_the_target_speed() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    let h = &car.car_type.handling;
    let heading = (-113.0f32).to_radians();
    let start = Vec3::new(361.0, -462.0, 30.0);
    let mut state = on_road(&world, &car, start, heading);
    let dir = Vec3::new(heading.cos(), heading.sin(), 0.0);
    let mut driver = AiDriver::new(h, false, 1.0);
    let dt = nfsu2::drive::STEP;
    let mut clock = 100.0f64;
    let mut speeds = Vec::new();
    for f in 0..(6.0 / dt) as u32 {
        // The next 20 points 8 m apart ahead of the car along the street.
        let along = (state.position - start).dot(dir).max(0.0);
        let path: Vec<Vec3> = (1..=20).map(|i| start + dir * (along + i as f32 * 8.0)).collect();
        let since = f as f32 * dt;
        let input = DriverInputs { path: &path, target_speed: 12.0, enabled: true, green: true, countdown: false, since_green: since, clock };
        let c = driver.update(&state, h, false, &input, dt);
        state.step_ai(&car, &world, c, dt);
        clock += dt as f64;
        speeds.push(state.speed());
    }
    let late = &speeds[speeds.len() - 60..];
    let mean = late.iter().sum::<f32>() / late.len() as f32;
    assert!((mean - 12.0).abs() < 2.0, "speed {mean}");
    // Still on the street's line.
    let off = (state.position - start).cross(dir).z.abs();
    assert!(off < 3.0, "{off} m off the line");
}

/// Explore traffic (src/ai/traffic/): the traffic models (+0x844 = 2)
/// load, and one spawned on an L4RA lane keeps to its lane for 10 s.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn traffic_car_follows_its_lane() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let types = nfsu2::car::parse_car_types(&globalb).unwrap();
    let traffic: Vec<&str> = types.iter().filter(|t| t.traffic).map(|t| t.name.as_str()).collect();
    assert!(traffic.contains(&"TAXI") && !traffic.contains(&"240SX"), "{traffic:?}");
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "TAXI").unwrap();
    let h = &car.car_type.handling;
    let routes = nfsu2::formats::ai_routes::RouteFile::parse(&std::fs::read(root.join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).unwrap());
    let mut rng = nfsu2::opponents::GameRng(7);
    // Career free roam's start (the placer's search as measured under Wine).
    let player = Vec3::new(1695.25, -883.64, 24.1);
    let (mut lane, at, heading) = (0..50).find_map(|_| nfsu2::ai::traffic::TrafficLane::spawn(&routes, player, glam::Vec2::X, 30.0, 0.5, 0.0, &[], &|a, b| a.distance(b), &mut rng).filter(|s| world.road_height_below(s.1 + Vec3::Z * 3.0).is_some())).expect("a spawn");
    let mut state = on_road(&world, &car, at + Vec3::Z * 3.0, heading);
    let mut driver = AiDriver::new(h, car.car_type.suv, 1.0);
    let dt = nfsu2::drive::STEP;
    let mut worst: f32 = 0.0;
    for f in 0..(10.0 / dt) as u32 {
        let Some((path, target)) = lane.update(&routes, state.position, state.velocity.length()) else { break };
        let input = DriverInputs { path: &path, target_speed: target, enabled: true, green: true, countdown: false, since_green: f as f32 * dt, clock: f as f64 * dt as f64 };
        let c = driver.update(&state, h, car.car_type.suv, &input, dt);
        state.step_ai(&car, &world, c, dt);
        // The car's offset off its lane's centre (at its control point).
        if f > 200 {
            if let Some(d) = lane.off_centre(&routes, state.position.truncate()) {
                worst = worst.max(d.abs());
            }
        }
    }
    assert!(state.position.distance(at) > 30.0, "moved {}", state.position.distance(at));
    assert!(worst < 6.0, "{worst} m off the lane");

    // The same car on the game's TrafficMover (0x408820) instead of the
    // physics, as explore mode moves unhit traffic (the racers' driver,
    // which the traffic controller builds too, steering the mover by its
    // wheel angle): it keeps near the lane and never passes 55 mph.
    let (mut lane, at, heading) = (0..50).find_map(|_| nfsu2::ai::traffic::TrafficLane::spawn(&routes, player, glam::Vec2::X, 30.0, 0.5, 0.0, &[], &|a, b| a.distance(b), &mut rng).filter(|s| world.road_height_below(s.1 + Vec3::Z * 3.0).is_some())).expect("a spawn");
    let mut state = on_road(&world, &car, at + Vec3::Z * 3.0, heading);
    let mut mover = nfsu2::ai::traffic::TrafficMover { position: state.position.truncate(), heading, speed: 0.0 };
    let mut driver = AiDriver::new(h, car.car_type.suv, 1.0);
    let w = &car.car_type.wheels;
    let mut worst: f32 = 0.0;
    for f in 0..(10.0 / dt) as u32 {
        let Some((path, target)) = lane.update(&routes, state.position, mover.speed) else { break };
        let input = DriverInputs { path: &path, target_speed: target, enabled: true, green: true, countdown: false, since_green: f as f32 * dt, clock: f as f64 * dt as f64 };
        let c = driver.update(&state, h, car.car_type.suv, &input, dt);
        let old = mover.heading;
        mover.step(w[3].position.x, w[0].position.x - w[3].position.x, c.steer_degrees.to_radians(), c.gas, c.brake, dt);
        state.spin = Vec3::Z * (mover.heading - old) / dt;
        assert!(mover.speed <= nfsu2::ai::traffic::MOVER_TOP_SPEED);
        let road = world.road_height_below(mover.position.extend(state.position.z + 3.0)).unwrap_or(state.position.z);
        state.position = mover.position.extend(road - (w[0].position.z - w[0].radius));
        state.orientation = glam::Quat::from_rotation_z(mover.heading);
        state.velocity = Vec3::new(mover.heading.cos(), mover.heading.sin(), 0.0) * mover.speed;
        for ws in &mut state.wheels {
            ws.on_ground = true;
        }
        // Off the lane's centre line from the walk point behind the car
        // (the path starts after the car's nearest walk point, 0x41D530,
        // so its first piece can lie a whole segment ahead of the car).
        if f > 200 {
            if let Some(d) = lane.off_centre(&routes, state.position.truncate()) {
                worst = worst.max(d.abs());
            }
        }
    }
    assert!(state.position.distance(at) > 30.0, "moved {}", state.position.distance(at));
    println!("worst {worst:.2} m off the lane on the mover");
    assert!(worst < 10.0, "{worst} m off the lane on the mover");
}

/// The MeloMvr (src/melo.rs): a hit traffic car on flat ground settles on
/// its springs, pulls away on the gas through the automatic gears and
/// stops on the brake with the wheels locked. (Away from the flat
/// world's diagonal seam, which the hull catches at the origin.)
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn melo_mover_settles_drives_and_stops() {
    use nfsu2::drive::{AiControls, CarField, Input, Racer, step_cars};
    let root = common::game_root();
    let car = nfsu2::car::Car::load(std::path::Path::new(&root), "TAXI").unwrap();
    let world = nfsu2::world::World::flat(1000.0);
    let w = car.car_type.wheels[0];
    let mut state = nfsu2::drive::CarState::new(Vec3::new(0.0, 300.0, w.radius - w.position.z), 0.0);
    state.melo = Some(nfsu2::melo::Melo::new(&car, &state, 5.0));
    let mut field = CarField::default();
    let mut run = |state: &mut nfsu2::drive::CarState, c: AiControls, frames: usize| {
        for _ in 0..frames {
            step_cars(&mut [Racer { car: &car, state, input: Input::Melo(c) }], &mut field, &world, 1.0 / 60.0);
        }
    };
    let idle = AiControls { gear: -1, ..Default::default() };
    run(&mut state, idle, 120);
    assert!(state.wheels.iter().all(|w| w.on_ground), "{:?}", state.wheels);
    assert!(state.velocity.length() < 0.5, "settled at {:?}", state.velocity);
    run(&mut state, AiControls { gas: 1.0, ..idle }, 300);
    let melo = state.melo.as_ref().unwrap();
    assert!(state.speed() > 5.0, "speed {}", state.speed());
    assert!(melo.gear >= 2, "gear {}", melo.gear);
    run(&mut state, AiControls { brake: 1.0, ..idle }, 300);
    assert!(state.speed().abs() < 0.5, "speed after braking {}", state.speed());
    assert!(state.position.z.is_finite());
}

/// On the MeloMvr 0x402590 reads the chassis' own down / up rpm
/// (+0x1C0 / +0x19C by gear, 0x4025D1) and an AI shot may run only with
/// the rpm inside [A, A + 0.95 (B − A)] (0x40D850 wants 0x402590 false).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn melo_nitrous_band_uses_the_chassis_shift_rpm() {
    let root = common::game_root();
    let car = nfsu2::car::Car::load(std::path::Path::new(&root), "TAXI").unwrap();
    let state = nfsu2::drive::CarState::new(Vec3::new(0.0, 300.0, 1.0), 0.0);
    let mut melo = nfsu2::melo::Melo::new(&car, &state, 5.0);
    let (up, down) = nfsu2::handling::shift_points(&car.car_type.handling);
    melo.gear = 3;
    let (a, b) = (down[3], up[3]);
    assert!(b > a, "{a} {b}");
    for (rpm, inside) in [(a, true), ((a + b) / 2.0, true), (a + 0.95 * (b - a), true), (a - 1.0, false), (b, false)] {
        melo.rpm = rpm;
        assert_eq!(melo.in_nitrous_band(), inside, "rpm {rpm}, band {a}..{b}");
    }
}
