//! Runs against the installed game: the tests are ignored (with the reason)
//! unless `NFSU2_ROOT` is set (docs/testing.md).

mod common;

use nfsu2::world::World;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn bayview_loads_completely() {
    let root = common::game_root();
    let world = World::load(std::path::Path::new(&root), "L4RA").unwrap();

    // Every scenery instance resolves to a solid in the stream file.
    assert_eq!(world.missing_solids, 0);
    assert!(world.placements.len() > 40_000, "{} placements", world.placements.len());
    // All 393 streaming sections and 255 visibility lists are read.
    assert_eq!(world.sections.len(), 393);
    assert_eq!(world.visibility.lists.len(), 255);

    // A known spot (a shop sign in Bayview) sits inside drivable section 307,
    // whose list includes itself.
    let (section, visible) = world.visible_sections_at(glam::Vec3::new(-1405.9, 250.0, 20.0)).unwrap();
    assert_eq!(section, 307);
    assert!(visible.contains(&307));

    // The JDLZ-compressed dynamic textures decode, including the night sky.
    // HUFF-coded ones too.
    for name in ["SKY_NIGHT_A", "SKY_NIGHT_A_CAP", "SKY_SUNRISE_A", "SKY_SUNRISE_A_CAP", "SKY_SUNSET_A"] {
        assert!(world.textures.contains_key(&nfsu2::formats::hash::bin_hash(name)), "{name} missing");
    }

    // Fog comes from the "Normal Fog" parameter map: a start distance
    // inside the 250-unit far plane, strength in 0..1, a 0..255 colour.
    let fog = nfsu2::fog::FogState::default()
        .update(&world, glam::Vec2::new(-1405.9, 250.0))
        .expect("fog map present");
    assert!(fog.value[1] > 0.0 && fog.value[2] <= 0.6, "{fog:?}");
    assert!(fog.color.iter().all(|c| (0.0..=1.0).contains(c)), "{fog:?}");

    // The screen tint (0x5D7800): "Sunrise Screen Tint" alone (weight 1).
    // Row 0 of both maps is all zero (slot off); at the explore shot's
    // camera the row is (189, 62, 113, 0.88, 0.42, 0.77, 0.091).
    let mut tint = nfsu2::screen_tint::ScreenTintState::default();
    let t = tint.update(&world, glam::Vec2::new(1706.87, -885.67), 1.0);
    assert!(t.enabled && (t.bleach - 0.88).abs() < 1e-4 && (t.grey - 0.091).abs() < 1e-4, "{t:?}");
    assert!(t.color.iter().all(|c| (0.0..=1.0).contains(c)), "{t:?}");

    // The emitter descriptions: GLOBALB's 92, then Bayview's 37; the
    // contact groups gather theirs, and their textures load.
    let fx = &world.emitters;
    assert_eq!(fx.descriptions.len(), 92 + 37);
    assert_eq!(fx.descriptions[0].name, "XEBO_ROAD_DirtA01");
    let names = |g: &str| -> Vec<String> { fx.group(nfsu2::formats::hash::bin_hash(g)).into_iter().map(|i| fx.descriptions[i].name.clone()).collect() };
    assert_eq!(names("XECS_SOLID_WALL"), ["XECS_SolidWall_GlowA", "XECS_SolidWall_SprksA"]);
    assert_eq!(names("XECI_CAR"), ["XECI_Car_SmokeA", "XECI_Car_SprksA"]);
    assert_eq!(names("XETT_ROAD_SK"), ["XETT_RoadSk_SmkA"]);
    assert_eq!(names("CARFX_NITRO"), ["XES_NOS_FlameA"]);
    for g in ["XEBO_ROAD", "XECS_SOLID_WALL", "XECI_CAR", "XETT_ROAD_SK", "XETT_ROAD_SP", "CARFX_NITRO"] {
        for i in fx.group(nfsu2::formats::hash::bin_hash(g)) {
            let d = &fx.descriptions[i];
            assert_eq!(d.hash, nfsu2::formats::hash::bin_hash(&d.name));
            assert!(world.textures.contains_key(&d.texture), "{}'s texture missing", d.name);
        }
    }
    let sparks = &fx.descriptions[fx.by_name("XECS_SolidWall_SprksA").unwrap()];
    assert_eq!((sparks.life[0], sparks.cone, sparks.gravity, sparks.fps), (0.5, 10.0, 1.0, 22));
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn stock_240sx_assembles() {
    let root = common::game_root();
    let car = nfsu2::car::Car::load(std::path::Path::new(&root), "240SX").unwrap();
    // Body, base, 4 wheels + 4 brakes.
    assert_eq!(car.parts.iter().filter(|p| p.is_wheel && p.lods.is_none_or(|m| m & 1 != 0)).count(), 4);
    assert!(car.parts.len() >= 20, "{} parts", car.parts.len());
    // Wheel table: front axle at x = 1.39, track ±0.73, radius ~0.3135.
    let w = car.car_type.wheels[0];
    assert!((w.position.x - 1.39).abs() < 1e-3 && (w.position.y - 0.73).abs() < 1e-3);
    assert!((w.radius - 0.3135).abs() < 1e-3);
    // The generated skin exists and every lit car mesh has a normal per
    // vertex (the prelit TRUNK_UNDER has none).
    assert!(car.textures.contains_key(&nfsu2::car::skin_hash("240SX")));
    assert!(car.parts.iter().all(|p| {
        let s = &car.solids[p.solid];
        s.normals.is_empty() || s.normals.len() == s.vertices.len()
    }));
    // The stock parts come from the part database: the stock spoiler slot
    // and the door sills are there.
    let names: Vec<&str> = car.parts.iter().map(|p| car.solids[p.solid].name.as_str()).collect();
    assert!(names.contains(&"240SX_KIT00_DOOR_SILL_LEFT_A"), "{names:?}");
    assert!(names.contains(&"240SX_BASE_A"));
    // The hatch glass draws GLOBALB's REAR_DEFROSTER (0x615290), blended,
    // and the glass keeps the stock tint's light material, WINDSHIELD
    // (0x638DC5: the slot 0x99 part's attribute 0x6BA02C05).
    let defroster = car.texture_for(nfsu2::formats::hash::bin_hash("REAR_DEFROSTER")).expect("REAR_DEFROSTER");
    assert_eq!(defroster.blend, nfsu2::formats::texture::Blend::Alpha);
    assert_eq!(car.light_material_for(nfsu2::formats::hash::bin_hash("WINDSHIELD")).unwrap().name, "WINDSHIELD");
}

#[test]
fn scenery_cull_flags_follow_the_views() {
    use nfsu2::render::WorldView;
    // 0x4888A0 as 0x5B9C30 sets the views up in game.
    assert_eq!(WorldView::Main.cull_flags(false), 0x601A);
    assert_eq!(WorldView::RoadReflection.cull_flags(true), 0x854);
    assert_eq!(WorldView::Environment.cull_flags(false), 0x1058);
    // Traffic lights (0x02) stay out of the player's view, the backward
    // race's signs (0x08) out of a forward race, 0x10 out of every view.
    assert!(WorldView::Main.culls(0x0062, false));
    assert!(WorldView::Main.culls(0x0068, false) && !WorldView::Main.culls(0x0068, true));
    assert!(WorldView::Environment.culls(0x0070, false));
    // The cube and the road reflection need 0x40; the reflection also
    // drops 0x0400.
    assert!(!WorldView::Main.culls(0x0000, false));
    assert!(WorldView::Environment.culls(0x0020, false) && !WorldView::Environment.culls(0x0040, false));
    assert!(WorldView::RoadReflection.culls(0x0440, false) && !WorldView::Environment.culls(0x0440, false));
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn front_end_light_setups_load() {
    let root = common::game_root();
    let raw = std::fs::read(std::path::Path::new(&root).join("FRONTEND/FrontB.lzc")).unwrap();
    let setups = nfsu2::render::lighting::parse_light_setups(&nfsu2::formats::jdlz::decompress(&raw).unwrap());
    assert_eq!(setups.len(), 10);
    let showroom = setups[&nfsu2::formats::hash::bin_hash("SHOWROOM")];
    assert_eq!((showroom[0].theta, showroom[0].phi), (149.0, 100.0));
    assert!((showroom[1].intensity - 0.5).abs() < 1e-5);
    assert!(setups.contains_key(&nfsu2::formats::hash::bin_hash("CRIB01")));
}

/// A car at rest on the road at `start`, facing `heading` (radians).
fn car_on_road(world: &nfsu2::world::World, car: &nfsu2::car::Car, start: glam::Vec3, heading: f32) -> nfsu2::drive::CarState {
    let road = world.road_height_below(start).expect("road under start");
    let w = &car.car_type.wheels[0];
    nfsu2::drive::CarState::new(glam::Vec3::new(start.x, start.y, road - (w.position.z - w.radius)), heading)
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn driving_stays_on_the_road() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    // Down a Chinatown street.
    let start = glam::Vec3::new(361.0, -462.0, 30.0);
    let mut state = car_on_road(&world, &car, start, (-113.0f32).to_radians());
    let throttle = nfsu2::drive::Controls { throttle: 1.0, ..Default::default() };
    for _ in 0..2000 {
        state.step(&car, &world, throttle, 0.001);
    }
    assert!(state.speed() > 5.0, "speed {}", state.speed());
    let moved = state.position.distance(start.truncate().extend(state.position.z));
    assert!(moved > 4.0, "moved {moved} m");
    // Still on a road surface.
    let under = world.road_height_below(state.position + glam::Vec3::Z * 3.0).expect("still on road");
    assert!((state.position.z - under).abs() < 1.0);
    // Steering right turns clockwise.
    let before = state.heading();
    let right = nfsu2::drive::Controls { throttle: 0.5, steer: 1.0, ..Default::default() };
    for _ in 0..500 {
        state.step(&car, &world, right, 0.001);
    }
    assert!(state.heading() < before);
}

/// Drive with `controls` for `seconds`; panics if the car's centre crosses
/// a wall.
fn drive_without_crossing(
    world: &nfsu2::world::World,
    car: &nfsu2::car::Car,
    state: &mut nfsu2::drive::CarState,
    controls: nfsu2::drive::Controls,
    seconds: f32,
) {
    for _ in 0..(seconds / nfsu2::drive::STEP) as u32 {
        let before = state.position + glam::Vec3::Z * 0.6;
        state.step(car, world, controls, nfsu2::drive::STEP);
        let after = state.position + glam::Vec3::Z * 0.6;
        assert!(world.wall_hit(before, after).is_none(), "went through a wall at {before}");
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn buildings_stop_the_car() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    // Square on to a building front about 13 m ahead.
    let start = glam::Vec3::new(355.0, -472.0, 30.0);
    let mut state = car_on_road(&world, &car, start, (-28.6f32).to_radians());
    let throttle = nfsu2::drive::Controls { throttle: 1.0, ..Default::default() };
    drive_without_crossing(&world, &car, &mut state, throttle, 4.0);
    let travelled = state.position.truncate().distance(start.truncate());
    assert!(travelled > 5.0 && travelled < 13.0, "travelled {travelled} m");
    assert!(state.speed().abs() < 0.5, "pinned at {} m/s", state.speed());
    assert!(world.road_height_below(state.position + glam::Vec3::Z * 3.0).is_some_and(|z| (state.position.z - z).abs() < 1.0));
    // Holding the brake selects reverse and backs away.
    let brake = nfsu2::drive::Controls { brake: 1.0, ..Default::default() };
    drive_without_crossing(&world, &car, &mut state, brake, 2.0);
    assert_eq!(state.gear_name(), "R");
    assert!(state.speed() < -2.0, "reversing at {} m/s", state.speed());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_glancing_wall_turns_the_car() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    // Facing -y, a wall at 35° to the street a few metres ahead.
    let start = glam::Vec3::new(365.0, -470.0, 30.0);
    let mut state = car_on_road(&world, &car, start, -std::f32::consts::FRAC_PI_2);
    let throttle = nfsu2::drive::Controls { throttle: 1.0, ..Default::default() };
    drive_without_crossing(&world, &car, &mut state, throttle, 4.0);
    let turned = (state.heading().to_degrees() + 90.0).abs();
    assert!(turned > 10.0, "turned {turned}°");
    assert!(state.speed() > 5.0, "kept going at {} m/s", state.speed());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drivetrain_launches_shifts_and_reverses() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    let h = &car.car_type.handling;
    assert_eq!(h.gearbox.ratios.len(), 7, "reverse, neutral, 5 forward");
    assert_eq!(h.gearbox.rear_split, 1.0, "240SX is rear-wheel drive");
    assert!((h.engine.torque.at(4000.0) - 0.2).abs() < 0.05);

    // Full throttle down the Chinatown street: 1st, then 2nd, under the
    // limiter.
    let start = glam::Vec3::new(361.0, -462.0, 30.0);
    let mut state = car_on_road(&world, &car, start, (-113.0f32).to_radians());
    let throttle = nfsu2::drive::Controls { throttle: 1.0, ..Default::default() };
    let mut top_gear = 0;
    for _ in 0..(6.0 / nfsu2::drive::STEP) as u32 {
        state.step(&car, &world, throttle, nfsu2::drive::STEP);
        let d = state.drivetrain.as_ref().unwrap();
        top_gear = top_gear.max(d.gear);
        assert!(d.rpm() <= h.engine.max_rpm + 1.0);
    }
    assert!(top_gear >= 3, "shifted into 2nd");
    assert!(state.speed() > 10.0, "speed {}", state.speed());

    // Brake to a stop, then hold it: reverse.
    let brake = nfsu2::drive::Controls { brake: 1.0, ..Default::default() };
    for _ in 0..(3.5 / nfsu2::drive::STEP) as u32 {
        state.step(&car, &world, brake, nfsu2::drive::STEP);
    }
    assert_eq!(state.gear_name(), "R");
    assert!(state.speed() < -1.0, "reversing at {} m/s", state.speed());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn collision_floor_matches_the_drawn_street() {
    let root = common::game_root();
    let world = nfsu2::world::World::load(std::path::Path::new(&root), "L4RA").unwrap();
    // The drive start: the drawn street is at about 17.8 m.
    let z = world.road_height_below(glam::Vec3::new(361.0, -462.0, 30.0)).expect("floor");
    assert!((z - 17.8).abs() < 0.2, "floor at {z}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn car_collision_hull_loads() {
    let root = common::game_root();
    let car = nfsu2::car::Car::load(std::path::Path::new(&root), "240SX").unwrap();
    // `240SX_CV`: 37 points around the body.
    assert_eq!(car.hull.len(), 37);
    let (lo, hi) = car.hull.iter().fold((glam::Vec3::MAX, glam::Vec3::MIN), |(l, h), p| (l.min(*p), h.max(*p)));
    eprintln!("hull {lo} .. {hi}; wheel {:?}", car.car_type.wheels[0]);
    assert!(lo.x < -1.8 && hi.x > 1.8 && hi.y > 0.7 && lo.y < -0.7, "{lo} {hi}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn ground_query_finds_the_street() {
    let root = common::game_root();
    let world = nfsu2::world::World::load(std::path::Path::new(&root), "L4RA").unwrap();
    // The game's query (0x5E72F0) through section 1601's pack.
    let mut query = nfsu2::ground::GroundQuery::default();
    let (z, n, _) = world.ground_for(&mut query, glam::Vec3::new(361.0, -462.0, 18.3)).expect("ground");
    assert!((z - 17.8).abs() < 0.2 && n.z > 0.9, "ground {z} {n}");
    // It agrees with the plain floor lookup down the street (here a
    // single level, where both rules pick the same floor).
    let mut compared = 0;
    for k in 0..40 {
        let p = glam::Vec3::new(361.0 - k as f32 * 1.0, -462.0 - k as f32 * 2.25, 19.0);
        let Some(plain) = world.road_height_below(p) else { continue };
        let Some((z, _, _)) = world.ground_for(&mut query, p) else {
            panic!("no ground at {p}: {}", world.ground.describe(&world.visibility, &query));
        };
        assert!((z - plain).abs() < 0.05, "at {p}: {z} vs {plain}");
        compared += 1;
    }
    assert!(compared > 25, "{compared} points on the street");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_modes_drive() {
    use nfsu2::race::RaceMode;
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let start = glam::Vec3::new(361.0, -462.0, 30.0);
    let throttle = nfsu2::drive::Controls { throttle: 1.0, ..Default::default() };
    for race in [RaceMode::FreeRoam, RaceMode::Drag, RaceMode::Drift, RaceMode::StreetX, RaceMode::Url] {
        let mut car = nfsu2::car::Car::load(root, "240SX").unwrap();
        car.set_race(race);
        let (h, stock) = (&car.car_type.handling, &car.car_type.stock);
        // The 240SX is rear-wheel drive: the tunes' third rows.
        let mass = match race {
            RaceMode::Drag => 0.95,
            _ => 1.0,
        };
        assert!((h.mass - stock.mass * mass).abs() < 1e-4, "{race:?} mass");
        let mut state = car_on_road(&world, &car, start, (-113.0f32).to_radians());
        state.start_race(race);
        // URL events ramp the gas in at 1/s.
        state.step(&car, &world, throttle, 0.1);
        let gas = state.last_gas;
        if race == RaceMode::Url {
            assert!(gas <= 0.1 + 1e-4, "URL gas {gas}");
        } else if race == RaceMode::Drag {
            // In neutral the drag gas ramps at 1/s too, with a 0.6 floor
            // under idle + 1000 rpm (0x407A30).
            assert!((gas - 0.6).abs() < 1e-6, "drag gas {gas}");
        } else {
            assert_eq!(gas, 1.0);
        }
        // A drag race's gearbox is manual (0x5EB440): shift up as the
        // engine nears its maximum.
        for _ in 0..(4.0 / nfsu2::drive::STEP) as u32 {
            if state.manual && (state.gear_name() == "N" || state.rpm() > car.car_type.handling.engine.redline_rpm - 200.0) {
                state.shift(&car, true);
            }
            state.step(&car, &world, throttle, nfsu2::drive::STEP);
            assert!(state.position.is_finite() && state.velocity.is_finite());
        }
        eprintln!("{race:?}: class {} speed {:.2} m/s gear {}", h.performance_class, state.speed(), state.gear_name());
        assert!(state.speed() > 8.0, "{race:?} speed {}", state.speed());
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drift_assist_damps_a_slide() {
    use nfsu2::race::RaceMode;
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let start = glam::Vec3::new(361.0, -462.0, 30.0);
    let mut yaw = Vec::new();
    // The drift tune also scales record +0x270, the ground yaw-inertia
    // factor; both runs take the drift car's so only the assist differs.
    let mut drift_car = nfsu2::car::Car::load(root, "240SX").unwrap();
    drift_car.set_race(RaceMode::Drift);
    let k = drift_car.car_type.handling.unknown_270;
    for race in [RaceMode::FreeRoam, RaceMode::Drift] {
        let mut car = nfsu2::car::Car::load(root, "240SX").unwrap();
        car.set_race(race);
        car.car_type.handling.unknown_270 = k;
        let mut state = car_on_road(&world, &car, start, (-113.0f32).to_radians());
        // Moving along the street but already yawing hard: a slide.
        state.velocity = state.forward() * 15.0;
        // Synthetic initial/test motion explicitly establishes its exported scalar.
        state.publish_forward_speed();
        let rot = glam::Mat3::from_quat(state.orientation);
        // On the ground the rigid body's yaw inertia is × record +0x270
        // (0x59B950), which the race tunes change.
        let h = &car.car_type.handling;
        state.angular_momentum = rot * glam::Vec3::new(0.0, 0.0, 1.5 * h.inertia[2] * h.unknown_270);
        state.ground_inertia = true;
        state.spin = glam::Vec3::Z * 1.5;
        let coast = nfsu2::drive::Controls::default();
        for _ in 0..(0.3 / nfsu2::drive::STEP) as u32 {
            state.step(&car, &world, coast, nfsu2::drive::STEP);
        }
        if race == RaceMode::Drift {
            assert!(state.wheels[2].assist_grip != 1.0, "rear assist grip set");
            assert_eq!(state.wheels[0].assist_grip, 1.0);
        } else {
            assert_eq!(state.wheels[2].assist_grip, 1.0);
        }
        yaw.push(state.spin.z.abs());
        eprintln!("{race:?}: yaw rate {:.3} rad/s, rear assist grip {:.3}", state.spin.z, state.wheels[2].assist_grip);
    }
    assert!(yaw[1] < yaw[0], "the assist damps the yaw: {yaw:?}");
}

/// Two 240SXs driven into each other head on down the Chinatown street
/// (the world collision step 0x5FFA60 with car pairs): a gentle bump
/// trades momentum between the equal cars and leaves them apart; a hard
/// one (an impulse of 30 or more, 0x59FB80) puts both in crash mode.
fn head_on(speed: f32) -> (nfsu2::drive::CarState, nfsu2::drive::CarState, glam::Vec3) {
    use nfsu2::drive::{CarField, Controls, Racer, Touching, step_cars};
    let root = common::game_root();
    let world = nfsu2::world::World::load(&root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(&root, "240SX").unwrap();
    let heading = (-113.0f32).to_radians();
    let dir = glam::Vec3::new(heading.cos(), heading.sin(), 0.0);
    let start = glam::Vec3::new(361.0, -462.0, 30.0);
    let mut a = car_on_road(&world, &car, start, heading);
    let mut b = car_on_road(&world, &car, start + dir * 7.0, heading + std::f32::consts::PI);
    // Settle on the springs, then set them rolling at each other.
    let mut field = CarField::new(1);
    for _ in 0..30 {
        step_cars(&mut [Racer { car: &car, state: &mut a, input: nfsu2::drive::Input::Player(Controls::default()) }, Racer { car: &car, state: &mut b, input: nfsu2::drive::Input::Player(Controls::default()) }], &mut field, &world, nfsu2::drive::STEP);
    }
    // ContactNote.impulse sums pushes for that pair. Ground impulses also
    // update hardest_impulse, so that field cannot establish that cars met.
    let car_push = |s: &nfsu2::drive::CarState| s.contacts.iter().any(|c| matches!(c.touching, Touching::Car { .. }) && c.impulse > 0.0);
    assert!(!car_push(&a) && !car_push(&b), "cars must start without a car-pair push");
    a.velocity = dir * speed;
    // Synthetic initial/test motion explicitly establishes its exported scalar.
    a.publish_forward_speed();
    b.velocity = -dir * speed;
    // Synthetic initial/test motion explicitly establishes its exported scalar.
    b.publish_forward_speed();
    let before = a.velocity + b.velocity;
    let mut hit = false;
    for _ in 0..90 {
        step_cars(&mut [Racer { car: &car, state: &mut a, input: nfsu2::drive::Input::Player(Controls::default()) }, Racer { car: &car, state: &mut b, input: nfsu2::drive::Input::Player(Controls::default()) }], &mut field, &world, nfsu2::drive::STEP);
        hit |= car_push(&a) && car_push(&b);
        if hit {
            break;
        }
    }
    assert!(hit, "both cars must record a positive car-pair push");
    // A frame later: no overlap.
    step_cars(&mut [Racer { car: &car, state: &mut a, input: nfsu2::drive::Input::Player(Controls::default()) }, Racer { car: &car, state: &mut b, input: nfsu2::drive::Input::Player(Controls::default()) }], &mut field, &world, nfsu2::drive::STEP);
    let hull = |s: &nfsu2::drive::CarState| -> Vec<glam::Vec3> { car.hull.iter().map(|p| s.position + s.orientation * *p).collect() };
    let gap = nfsu2::narrow::distance(&hull(&a), &hull(&b), nfsu2::narrow::EXACT, &mut None).0;
    // The native query deliberately returns zero when the initial support
    // points' squared gap <= 1e-5 (0x5B36CE / 0x7A1934). That includes
    // separated hulls about 3 mm apart; zero is not proof of penetration.
    // An independent strict separating-plane test along the head-on axis
    // catches the previously misreported 1.7 mm separation without changing
    // collision tolerances or allowing overlap.
    let projection_gap = hull(&b).iter().map(|p| p.dot(dir)).fold(f32::INFINITY, f32::min)
        - hull(&a).iter().map(|p| p.dot(dir)).fold(f32::NEG_INFINITY, f32::max);
    assert!(gap > 0.0 || projection_gap > 0.0,
        "apart after the hit: query {gap}, separating plane {projection_gap}");
    (a, b, before)
}

/// Native 0x5ABB10 refreshes components again with dt=0 after holding
/// the car. The clutch therefore leaves Changing in this single pass.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn held_car_refreshes_clutch_without_advancing_time() {
    use nfsu2::drive::Controls;
    use nfsu2::handling::Shift;
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    let mut state = car_on_road(&world, &car, glam::Vec3::new(361.0, -462.0, 30.0), (-113.0f32).to_radians());
    for _ in 0..120 { state.step(&car, &world, Controls::default(), nfsu2::drive::STEP); }
    assert_eq!(state.rest, 2);
    let t = state.drivetrain.as_mut().unwrap();
    t.gear = 2; t.target_gear = 2; t.pending = true;
    t.shift = Shift::Disengaging; t.clutch = 0.0;
    t.heat = 0.5;
    let pose = state.position;
    state.step(&car, &world, Controls::default(), 0.0);
    let t = state.drivetrain.as_ref().unwrap();
    assert_eq!(state.rest, 2);
    assert_eq!(t.shift, Shift::Engaging, "held refresh must advance both discrete transitions");
    assert_eq!(t.clutch, 0.0);
    assert_eq!(t.heat, 0.5);
    assert_eq!(state.position, pose);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn opening_bump_stop_still_refreshes_components() {
    use nfsu2::drive::Controls;
    use nfsu2::handling::Shift;
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    let mut state = car_on_road(&world, &car, glam::Vec3::new(361.0, -462.0, 30.0), (-113.0f32).to_radians());
    state.step(&car, &world, Controls::default(), 0.0);
    state.position.z -= 0.5;
    state.velocity = glam::Vec3::Z;
    // Synthetic initial/test motion explicitly establishes its exported scalar.
    state.publish_forward_speed();
    state.rest = 0;
    let t = state.drivetrain.as_mut().unwrap();
    t.gear = 2; t.target_gear = 2; t.pending = true;
    t.shift = Shift::Disengaging; t.clutch = 0.0;
    state.step(&car, &world, Controls::default(), 0.0);
    assert_eq!(state.rest, 0);
    assert_eq!(state.velocity, glam::Vec3::Z, "opening bump stop applies no impulse");
    // The car was moved, so the step takes a force pass first, and it ends
    // with the next one (0x5B04E0 → 0x5B0530); both find the suspension at
    // full compression and refresh the components (0x5ABB10 → 0x5A0D90):
    // Disengaging → Changing (the first pass), → Engaging (the sub-step's
    // own tidy-up), → Disengaging (the last pass: no gas at idle, the
    // anti-stall test 0x594EA0). Without the refreshes it would stop in
    // Changing.
    assert_eq!(state.drivetrain.as_ref().unwrap().shift, Shift::Disengaging,
        "full compression requests component refresh even without closing velocity");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn cars_bump_and_part() {
    let (a, b, before) = head_on(4.0);
    let after = a.velocity + b.velocity;
    // Equal cars take equal and opposite pushes: the summed velocity along
    // the street barely moves (the road's friction aside).
    eprintln!("bump: a {:?} b {:?}", a.velocity, b.velocity);
    assert!((after - before).truncate().length() < 1.0, "momentum kept: {before:?} → {after:?}");
    assert!(!a.crash && !b.crash, "a bump is no crash");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn hard_hits_crash_both_cars() {
    let (a, b, _) = head_on(20.0);
    eprintln!("crash: a {:?} {} b {:?} {}", a.velocity, a.hardest_impulse, b.velocity, b.hardest_impulse);
    assert!(a.crash && b.crash, "crash mode on both");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drag_engine_blows_over_the_redline() {
    use nfsu2::race::RaceMode;
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let mut car = nfsu2::car::Car::load(root, "240SX").unwrap();
    car.set_race(RaceMode::Drag);
    let mut state = car_on_road(&world, &car, glam::Vec3::new(361.0, -462.0, 30.0), (-113.0f32).to_radians());
    state.start_race(RaceMode::Drag);
    let throttle = nfsu2::drive::Controls { throttle: 1.0, ..Default::default() };
    // Into 1st and held there on full gas: past the redline the heat
    // climbs 0.45/s and the engine blows at 1.2 (0x5A11F0, 0x5942D0).
    state.shift(&car, true);
    let (mut red, mut blown_at) = (false, None);
    for i in 0..(8.0 / nfsu2::drive::STEP) as u32 {
        state.step(&car, &world, throttle, nfsu2::drive::STEP);
        let t = state.drivetrain.as_ref().unwrap();
        red |= t.drag_shift_light(&car.car_type.handling, t.gear) == Some(-1.0);
        if t.blown() {
            blown_at = Some(i as f32 * nfsu2::drive::STEP);
            break;
        }
    }
    assert!(red, "the shift light never went red");
    let at = blown_at.expect("engine never blew");
    assert!(at > 2.6, "blew after {at} s");
}

/// The scenery groups (`0x34108`): `SMOKEABLE` lists every prop's
/// scenery entry; no other group lists one.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn smokeable_group_lists_every_prop() {
    let root = common::game_root();
    let world = nfsu2::world::World::load(std::path::Path::new(&root), "L4RA").unwrap();
    let props: std::collections::HashSet<u16> = world.smackables.iter().map(|p| p.scenery).collect();
    assert_eq!(props.len(), 514);
    let smokeable = world.scenery_groups.iter().find(|g| g.name == "SMOKEABLE").expect("SMOKEABLE");
    assert_eq!(smokeable.members.len(), 601);
    assert!(props.iter().all(|p| smokeable.members.contains(p)));
    for g in world.scenery_groups.iter().filter(|g| g.name != "SMOKEABLE") {
        assert!(!g.on, "{}", g.name);
        assert!(g.members.iter().all(|m| !props.contains(m)), "{}", g.name);
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn props_carry_their_contact_data() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    assert!(world.smackables.len() > 400, "{}", world.smackables.len());
    let mut seen = std::collections::BTreeMap::new();
    for p in &world.smackables {
        let i = &p.info;
        assert!(i.mass > 0.0 && i.restitution <= 0.005 && i.friction >= 0.3, "{i:?}");
        assert!(i.inertia.determinant() > 0.0, "{i:?}");
        seen.entry(i.name.clone()).or_insert((i.mass, i.break_impulse, i.restitution, i.friction));
    }
    for (name, v) in seen {
        println!("{name}: mass {} break {} e {} mu {}", v.0, v.1, v.2, v.3);
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn props_stand_in_for_their_scenery_instances() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    // Paired through the 0x34107 table (0x5D6440), each prop's placement
    // stands where the prop's own transform says.
    assert!(world.smackables.len() > 500, "{}", world.smackables.len());
    let mut shaped = 0;
    for p in &world.smackables {
        let placed = world.placements[p.placement].transform.w_axis.truncate();
        assert!(placed.distance(p.rest.w_axis.truncate()) < 1e-3);
        let centre = (p.world_min + p.world_max) * 0.5;
        assert!(placed.distance(centre) < p.radius + 1.0, "{} at {placed} box centre {centre}", p.info.name);
        assert!(!p.shape.is_empty() && p.reach > 0.0);
        shaped += (p.shape.len() != 8) as usize;
    }
    println!("{} props, {shaped} with a shape of other than 8 points", world.smackables.len());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_car_knocks_a_prop_over_and_it_comes_to_rest() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    // A light prop with no break impulse, standing on a road with room to
    // drive at it along x.
    let (k, prop) = world
        .smackables
        .iter()
        .enumerate()
        .find(|(_, p)| {
            let at = p.rest.w_axis.truncate();
            p.info.break_impulse == 0.0
                && p.info.mass < 0.3
                && (0..=8).all(|i| {
                    let q = at - glam::Vec3::X * i as f32 + glam::Vec3::Z * 2.0;
                    world.road_height_below(q).is_some_and(|z| (z - at.z).abs() < 0.3)
                })
        })
        .expect("a prop on open road");
    let at = prop.rest.w_axis.truncate();
    let mut state = car_on_road(&world, &car, at - glam::Vec3::X * 8.0 + glam::Vec3::Z * 2.0, 0.0);
    state.velocity = glam::Vec3::X * 12.0;
    // Synthetic initial/test motion explicitly establishes its exported scalar.
    state.publish_forward_speed();
    let mut props = nfsu2::smackable::Props::new(&world);
    let mut last = None;
    let mut last_at = 0;
    let dt = 0.004;
    for i in 0..(6.0 / dt) as u32 {
        for (placement, m) in state.step_with_props(&car, &world, &mut props, nfsu2::drive::Controls::default(), dt, &mut Default::default()) {
            assert_eq!(placement, prop.placement);
            assert!(m.is_finite());
            if last != Some(m) {
                last_at = i;
            }
            last = Some(m);
        }
    }
    let _ = k;
    assert_eq!(props.knocked(), 1, "{} knocked", props.knocked());
    let end = last.expect("the prop moved");
    let moved = end.w_axis.truncate().distance(at);
    assert!(moved > 0.5, "moved {moved} m");
    // Resting on the ground, not under it.
    let ground = world.road_height_below(end.w_axis.truncate() + glam::Vec3::Z * 2.0).unwrap_or(at.z);
    let centre = end.transform_point3(prop.info.centre_of_mass);
    assert!(centre.z > ground - 0.05, "centre {centre} below ground {ground}");
    // And it stops ("No activity", 0x5A8DA0) well before the end.
    assert!((last_at as f32) * dt < 5.0, "still moving at {} s", last_at as f32 * dt);
    println!("{} pushed {moved:.2} m, centre {:.2} m over the ground, still from {:.2} s", prop.info.name, centre.z - ground, last_at as f32 * dt);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn camera_tracks_parse() {
    use nfsu2::formats::bundle::chunks;
    use nfsu2::formats::camera_tracks::{CameraKind, CameraKind::*, parse_camera_tracks};
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    for (file, kind, groups) in [("TRACKS/L4RA.BUN", Route, 35), ("GLOBAL/InGameRace.bun", Generic, 38), ("GLOBAL/InGameCommon.bun", Typed, 6)] {
        let data = std::fs::read(root.join(file)).unwrap();
        let chunk = chunks(&data, 0).find(|c| CameraKind::of_chunk(c.id).is_some()).expect(file);
        let parsed = parse_camera_tracks(chunk).unwrap();
        assert_eq!(parsed.len(), groups, "{file}");
        for g in &parsed {
            assert_eq!(g.kind, kind);
            for t in &g.tracks {
                assert!(t.name.starts_with("Track "), "{file}: {:?}", t.name);
                // Entries start at 0 and run in order inside 0..1.
                assert_eq!(t.entries.first().map(|e| e.start), Some(0.0), "{file} {}", t.name);
                assert!(t.entries.windows(2).all(|w| w[0].start <= w[1].start && w[1].start <= 1.0));
                // Movers: 0 (no track camera: the director keeps a chase
                // view), 0x12 or 0x13.
                assert!(t.entries.iter().all(|e| matches!(e.mover, 0 | 0x12 | 0x13)), "{file} {}", t.name);
            }
        }
    }
}

/// The track director on 4001's route cameras: a group whose record the race
/// has, with a track camera at the record's middle, gives a camera with the
/// eye finite and the field of view a real angle, and the shot is held.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn track_director_follows_a_route_camera() {
    use nfsu2::ai::route::AiRoutes;
    use nfsu2::formats::bundle::chunks;
    use nfsu2::formats::camera_tracks::{ROUTE_CAMERAS, parse_camera_tracks};
    use nfsu2::frontend::career::{self, CareerData};
    use nfsu2::opponents::GameRng;
    use nfsu2::track_camera::{TrackDirector, route_group};
    let root = common::game_root();
    let data = CareerData::load(&root).unwrap();
    let track = &data.tracks[&4001];
    let routes = AiRoutes::load(&root, track, false).unwrap();
    let length = career::load_route(&root, track, false).unwrap().road.length;
    let bundle = std::fs::read(root.join("TRACKS").join(format!("{}.BUN", track.region))).unwrap();
    let groups = chunks(&bundle, 0).find(|c| c.id == ROUTE_CAMERAS).map(|c| parse_camera_tracks(c).unwrap()).unwrap();
    let car = glam::Mat4::IDENTITY;
    let mut rng = GameRng::default();
    // A view looking straight up passes 0x448280's direction tests for
    // every candidate that looks down (e·d in (−0.5, 0]); the sight test
    // finds nothing in the way.
    let up = nfsu2::track_camera::View { eye: glam::Vec3::new(0.0, 0.0, 2.0), forward: glam::Vec3::Z, fov: 1.2, velocity: glam::Vec3::ZERO };
    let mut shown = 0;
    for group in &groups {
        let (id, bit) = group.route_key();
        let Some(record) = routes.file.records.iter().find(|r| r.id == id && (r.flags >> 1 & 1) as u16 == bit) else { continue };
        let along = (record.coord_range.0 + record.coord_range.1) * 0.5;
        let Some((g, _)) = route_group(&groups, &routes.file.records, length, along, false) else { continue };
        let mut dir = TrackDirector::default();
        let Some(cam) = dir.direct(&groups, &routes.file.records, length, along, false, car, 0.0, &mut rng, &up, true, &mut |_, _| false) else { continue };
        assert!(cam.position.is_finite(), "group {id}: eye {:?}", cam.position);
        assert!(cam.fov_y > 0.1 && cam.fov_y < 3.0, "group {id}: fov {}", cam.fov_y);
        assert!(dir.shot.as_ref().is_some_and(|s| s.group == g));
        shown += 1;
    }
    // At rest every track runs long enough. Records overlap, and the group
    // the car is on is inferred (the first covering record), so the check
    // is on 448, whose middle is a track camera (mover 0x12) of its own.
    assert!(shown >= 1, "no route group gave a camera");
    let mut dir = TrackDirector::default();
    let along = 2491.8;
    let cam = dir.direct(&groups, &routes.file.records, length, along, false, car, 0.0, &mut rng, &up, true, &mut |_, _| false).expect("448 shows a track camera");
    assert_eq!(dir.shot.as_ref().map(|s| groups[s.group].route_key()), Some((448, 0)));
    assert!(cam.fov_y > 0.1 && cam.fov_y < 3.0);
}

/// A race switches its barriers on (0x578070): its groups' collision and
/// scenery. Its racers' own lists switch the collision off again for
/// everyone else once they have looked (0x5E76B2), its end hides the
/// scenery (0x5711E0); free roam then switches the career's on. A backward
/// race mirrors the race-only scenery (0x5DB4C0); a region load switches
/// everything off (0x5D8360).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn races_switch_their_barriers_on_and_free_roam_off() {
    use nfsu2::world::{BarrierSetup, HIDDEN_FLAG};
    let root = common::game_root();
    let mut world = World::load(std::path::Path::new(&root), "L4RA").unwrap();
    let group = |w: &World, name: &str| w.barrier_groups.iter().position(|g| g.name == name).unwrap() as u8;
    let scenery = |w: &World, name: &str| w.scenery_groups.iter().position(|g| g.name == name).unwrap();
    let shown = |w: &World, g: usize| {
        let placed: Vec<usize> = w.scenery_groups[g].members.iter().filter_map(|&m| w.scenery_placement(m)).collect();
        assert!(!placed.is_empty());
        placed.iter().filter(|&&p| w.placements[p].flags & HIDDEN_FLAG == 0).count() * 100 / placed.len()
    };
    let (race, own) = (group(&world, "BARRIERS_4014"), group(&world, "PLAYER_BARRIERS_4014"));
    let (career2, other) = (group(&world, "BARRIERS_CAREER2"), group(&world, "BARRIERS_4002"));
    // As loaded every group is off and its members hidden.
    assert!(!(1..world.barriers.len()).any(|g| world.barriers.is_on(g as u8)));
    assert_eq!(shown(&world, scenery(&world, "BARRIERS_4014")), 0);

    let setup = BarrierSetup { track: 4014, free_roam: false, career: true, stage: 2, reversed: false, owns_car: true };
    world.set_up_barriers(&setup);
    assert!(world.barriers.is_on(race) && world.barriers.is_on(own));
    assert!(!world.barriers.is_on(career2) && !world.barriers.is_on(other));
    assert_eq!(shown(&world, scenery(&world, "BARRIERS_4014")), 100);
    assert_eq!(shown(&world, scenery(&world, "PLAYER_BARRIERS_4014")), 100);
    assert_eq!(shown(&world, scenery(&world, "BARRIERS_4002")), 0);
    assert!(!world.take_changed().is_empty());
    // A racer's height query takes its list on and leaves it off.
    let mut q = nfsu2::ground::GroundQuery::default();
    q.barriers = world.racer_barriers(&setup);
    assert_eq!(q.barriers, vec![own, race]);
    assert!(world.ground_for(&mut q, glam::Vec3::new(361.0, -462.0, 18.3)).is_some());
    assert!(!world.barriers.is_on(race) && !world.barriers.is_on(own));

    let roam = BarrierSetup { track: 4000, free_roam: true, career: true, stage: 2, reversed: false, owns_car: true };
    world.set_up_barriers(&roam);
    assert!(!world.barriers.is_on(race) && world.barriers.is_on(career2));
    assert_eq!(world.racer_barriers(&roam), vec![career2]);
    assert_eq!(shown(&world, scenery(&world, "BARRIERS_4014")), 0);
    assert_eq!(shown(&world, scenery(&world, "FREE_ROAM")), 100);
    assert_eq!(shown(&world, scenery(&world, "BARRIERS_CAREER2")), 100);

    // Run backward, the race-only barriers' local x is negated; forward
    // again, it is back.
    let barrier = world.placements.iter().position(|p| p.flags & 0x0200 != 0).unwrap();
    let before = world.placements[barrier].transform;
    world.set_up_barriers(&BarrierSetup { reversed: true, ..setup });
    assert_eq!(world.placements[barrier].transform.x_axis, -before.x_axis);
    assert_eq!(world.placements[barrier].transform.w_axis, before.w_axis);
    world.set_up_barriers(&setup);
    assert_eq!(world.placements[barrier].transform, before);

    // A region load: all off, the scenery as on disk.
    world.reset_groups();
    assert!(!(1..world.barriers.len()).any(|g| world.barriers.is_on(g as u8)));
    assert_eq!(shown(&world, scenery(&world, "BARRIERS_4014")), 0);
}

/// docs/prop-hits.md: a PEUGOT rolling north at `kmh` through the
/// highway's yellow barrels (ZCS_Barrel_A, L4RA 379.4 −1477), throttle held,
/// 5 s at 1/60 s, from the original's state a frame before its first query
/// with the first barrel (the 60 km/h run of docs/prop-hits.md §7a, the contact
/// log's transform: centre of mass 378.834 −1480.284 36.210, x and y rows;
/// the scenario's start, 379.4 −1500 north, comes there through the
/// original's hold and roll, which the clone's start does not replay).
/// The outcome hangs on that start frame: from it the first barrel leaves
/// at −36°, the original's clean contact; started on the logged pose
/// itself (a frame later: the step moves the car before the pair's first
/// query) the pair's cold query comes 0.27 m nearer, its fresh support
/// lands early and the barrel leaves at −1.4° — keep the start as it is.
/// Per barrel that moved: the heading of its
/// first motion off the car's (degrees, + = left), its highest rise, the
/// distance to where it ends and how high its centre of mass then rests
/// above its base.
fn barrel_run(kmh: f32) -> Vec<(f32, f32, f32, f32)> {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "PEUGOT").unwrap();
    let group: Vec<usize> = (0..world.smackables.len())
        .filter(|&k| {
            let p = &world.smackables[k];
            p.info.name == "ZCS_Barrel_A" && p.rest.w_axis.truncate().distance(glam::Vec3::new(379.4, -1477.0, 35.8)) < 5.0
        })
        .collect();
    assert_eq!(group.len(), 3, "the barrel group");
    let x = glam::Vec3::new(-0.11544, 0.99268, -0.03547);
    let y = glam::Vec3::new(-0.99063, -0.11768, -0.06938);
    let rot = glam::Mat3::from_cols(x, y, x.cross(y));
    // A frame before it: the step moves the car, then the pair's first query.
    let com = glam::Vec3::new(378.83444, -1480.28394, 36.21029) - x * kmh / 3.6 / 60.0;
    let heading = x.y.atan2(x.x);
    let mut state = car_on_road(&world, &car, com, heading);
    state.orientation = glam::Quat::from_mat3(&rot).normalize();
    state.position = com - rot * car.hull_offset();
    let forward = glam::Vec3::new(x.x, x.y, 0.0).normalize();
    state.velocity = x * kmh / 3.6;
    state.publish_forward_speed();
    let mut props = nfsu2::smackable::Props::new(&world);
    let throttle = nfsu2::drive::Controls { throttle: 1.0, ..Default::default() };
    let centre = |k: usize, m: glam::Mat4| m.transform_point3(world.smackables[k].info.centre_of_mass);
    let mut track: Vec<Vec<glam::Vec3>> = vec![Vec::new(); group.len()];
    for _ in 0..300 {
        state.step_with_props(&car, &world, &mut props, throttle, 1.0 / 60.0, &mut Default::default());
        for (placement, m, _) in props.poses(&world) {
            if let Some(i) = group.iter().position(|&k| world.smackables[k].placement == placement) {
                let c = centre(group[i], m);
                if track[i].last() != Some(&c) {
                    track[i].push(c);
                }
            }
        }
    }
    let mut out = Vec::new();
    for (i, t) in track.iter().enumerate() {
        let start = centre(group[i], world.smackables[group[i]].rest);
        let Some(first) = t.iter().find(|c| c.distance(start) > 0.02) else { continue };
        let d = *first - start;
        let angle = (d.truncate().perp_dot(forward.truncate())).atan2(d.truncate().dot(forward.truncate())).to_degrees() * -1.0;
        let up = t.iter().map(|c| c.z - start.z).fold(0.0, f32::max);
        let end = *t.last().unwrap();
        let base = start.z - world.smackables[group[i]].info.centre_of_mass.z;
        out.push((angle, up, end.truncate().distance(start.truncate()), end.z - base));
        println!("{kmh} km/h barrel {i}: angle {angle:.1} up {up:.2} dist {:.2} rest height {:.2}", out.last().unwrap().2, end.z - base);
    }
    out
}

/// The original under Wine (docs/prop-hits.md §3; native runs n60a–c,
/// two more runs): at 60 km/h the first barrel leaves 20–40° right
/// of the car's heading, no barrel rises over 0.25 m, travels over 15 m,
/// or rests with its centre of mass under 0.55 m or over 1.0 m above its
/// base (lying 0.64–0.73, upright 0.95). The clone's car once
/// pushed the barrels on without their contacts with each other.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn barrels_at_60_kmh_stay_in_the_originals_range() {
    let run = barrel_run(60.0);
    assert!(!run.is_empty(), "no barrel moved");
    assert!((-40.0..=-15.0).contains(&run[0].0), "first barrel's heading {}", run[0].0);
    for (i, &(_, up, dist, rest)) in run.iter().enumerate() {
        assert!(up < 0.25 && dist < 15.0 && (0.55..1.0).contains(&rest), "barrel {i}: up {up} dist {dist} rest {rest}");
    }
}

/// The original's distance queries of a car–car pair, replayed (the
/// original under Wine, scenario `car_rear_end_200`: the player's PEUGOT
/// rolled at 195 km/h past an encounter FOCUS, the closest 8.8 cm, no
/// contact): per query the gap and both bodies' transforms (A the PEUGOT,
/// hull offset (0.3, 0, 0.3); B the FOCUS, (0.19, 0, 0.27)), the first the
/// pair's first (cold) query.
const CAR_CAR_QUERIES: [(f32, [f32; 9], [f32; 9]); 19] = [
    (0.407650, [496.05328, 354.13382, 12.50517, -0.14749, 0.98862, 0.02965, -0.98494, -0.14407, -0.09559], [497.27731, 358.61371, 12.96542, -0.01955, 0.99805, 0.05927, -0.99185, -0.01190, -0.12682]),
    (0.088249, [495.91916, 354.99719, 12.55533, -0.14763, 0.98859, 0.02985, -0.98618, -0.14484, -0.08047], [497.26758, 358.98596, 12.98879, -0.02242, 0.99801, 0.05887, -0.99182, -0.01481, -0.12677]),
    (0.119873, [495.77615, 355.92026, 12.60832, -0.14777, 0.98858, 0.02959, -0.98716, -0.14560, -0.06564], [497.25656, 359.38434, 13.01369, -0.02561, 0.99796, 0.05843, -0.99178, -0.01804, -0.12669]),
    (0.131435, [495.63818, 356.81357, 12.65802, -0.14795, 0.98854, 0.03004, -0.98776, -0.14617, -0.05451], [497.24527, 359.77029, 13.03771, -0.02882, 0.99790, 0.05798, -0.99173, -0.02129, -0.12660]),
    (0.161311, [495.50519, 357.67719, 12.70417, -0.14811, 0.98850, 0.03048, -0.98804, -0.14657, -0.04782], [497.23376, 360.14380, 13.06086, -0.03203, 0.99783, 0.05753, -0.99166, -0.02454, -0.12653]),
    (0.221813, [495.36301, 358.60031, 12.75175, -0.14825, 0.98849, 0.03008, -0.98818, -0.14687, -0.04387], [497.22079, 360.54343, 13.08553, -0.03556, 0.99774, 0.05703, -0.99157, -0.02811, -0.12647]),
    (0.266729, [495.22534, 359.49356, 12.79643, -0.14836, 0.98851, 0.02887, -0.98823, -0.14710, -0.04189], [497.20746, 360.93054, 13.10933, -0.03908, 0.99764, 0.05653, -0.99147, -0.03168, -0.12642]),
    (0.311218, [495.09229, 360.35696, 12.83773, -0.14850, 0.98851, 0.02814, -0.98819, -0.14724, -0.04254], [497.19379, 361.30511, 13.13218, -0.04256, 0.99752, 0.05600, -0.99140, -0.03522, -0.12600]),
    (0.376055, [494.94992, 361.27979, 12.88177, -0.14868, 0.98848, 0.02817, -0.98815, -0.14741, -0.04287], [497.17831, 361.70587, 13.15657, -0.04636, 0.99738, 0.05546, -0.99136, -0.03912, -0.12519]),
    (0.450439, [494.81210, 362.17273, 12.92332, -0.14888, 0.98843, 0.02886, -0.98799, -0.14747, -0.04605], [497.16241, 362.09406, 13.18028, -0.05014, 0.99723, 0.05501, -0.99127, -0.04297, -0.12462]),
    (0.542965, [494.67883, 363.03583, 12.96290, -0.14903, 0.98843, 0.02832, -0.98778, -0.14749, -0.05039], [497.14615, 362.46960, 13.20328, -0.05389, 0.99705, 0.05465, -0.99115, -0.04676, -0.12426]),
    (0.637512, [494.53619, 363.95831, 13.00632, -0.14915, 0.98845, 0.02700, -0.98762, -0.14758, -0.05312], [497.12775, 362.87131, 13.22790, -0.05799, 0.99684, 0.05435, -0.99098, -0.05089, -0.12399]),
    (0.739432, [494.39798, 364.85089, 13.04881, -0.14933, 0.98842, 0.02692, -0.98748, -0.14768, -0.05536], [497.10889, 363.26028, 13.25175, -0.06204, 0.99660, 0.05413, -0.99078, -0.05495, -0.12382]),
    (0.827724, [494.26425, 365.71359, 13.08997, -0.14954, 0.98837, 0.02765, -0.98734, -0.14778, -0.05759], [497.08966, 363.63657, 13.27480, -0.06602, 0.99636, 0.05398, -0.99056, -0.05893, -0.12374]),
    (0.917788, [494.12115, 366.63568, 13.13397, -0.14971, 0.98835, 0.02734, -0.98717, -0.14787, -0.06034], [497.06802, 364.03915, 13.29944, -0.07034, 0.99607, 0.05387, -0.99036, -0.06327, -0.12326]),
    (0.985479, [493.98251, 367.52789, 13.17671, -0.14980, 0.98839, 0.02539, -0.98698, -0.14796, -0.06312], [497.04590, 364.42914, 13.32326, -0.07458, 0.99576, 0.05379, -0.99015, -0.06753, -0.12261]),
    (1.163509, [493.84836, 368.39047, 13.21713, -0.14987, 0.98843, 0.02340, -0.98665, -0.14799, -0.06800], [497.02332, 364.80652, 13.34641, -0.07869, 0.99546, 0.05360, -0.98991, -0.07167, -0.12224]),
    (1.434257, [493.70486, 369.31262, 13.25897, -0.15001, 0.98844, 0.02215, -0.98610, -0.14796, -0.07561], [496.99792, 365.21033, 13.37121, -0.08310, 0.99511, 0.05331, -0.98960, -0.07610, -0.12209]),
    (1.749352, [493.56586, 370.20514, 13.29838, -0.15019, 0.98842, 0.02183, -0.98535, -0.14785, -0.08498], [496.97202, 365.60153, 13.39520, -0.08738, 0.99477, 0.05296, -0.98926, -0.08040, -0.12208]),
];

/// 0x5B33A0 between two car hulls, both offsets on: the clone's query on
/// the original's transforms, in order with one pair history, gives the
/// original's gaps.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn car_car_queries_match_the_originals() {
    use glam::{Mat3, Vec3};
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let (a, b) = (nfsu2::car::Car::load(root, "PEUGOT").unwrap(), nfsu2::car::Car::load(root, "FOCUS").unwrap());
    assert!((a.hull_offset() - Vec3::new(0.3, 0.0, 0.3)).length() < 1e-4, "{}", a.hull_offset());
    assert!((b.hull_offset() - Vec3::new(0.19, 0.0, 0.27)).length() < 1e-4, "{}", b.hull_offset());
    let at = |t: &[f32; 9]| -> (Vec3, Mat3) {
        let (x, y) = (Vec3::new(t[3], t[4], t[5]), Vec3::new(t[6], t[7], t[8]));
        (Vec3::new(t[0], t[1], t[2]), Mat3::from_cols(x, y, x.cross(y)))
    };
    let world = |car: &nfsu2::car::Car, p: Vec3, r: Mat3| -> Vec<Vec3> {
        car.hull.iter().map(|v| p + r * (*v - car.hull_offset())).collect()
    };
    let mut cache = None;
    for (k, (want, ta, tb)) in CAR_CAR_QUERIES.iter().enumerate() {
        let ((pa, ra), (pb, rb)) = (at(ta), at(tb));
        let raw = [
            nfsu2::narrow::Side::hull(ra * a.hull_offset(), &a.hull_links),
            nfsu2::narrow::Side::hull(rb * b.hull_offset(), &b.hull_links),
        ];
        let d = nfsu2::narrow::distance(&world(&a, pa, ra), &world(&b, pb, rb), raw, &mut cache).0;
        println!("query {k}: original {want:.6} clone {d:.6}");
        assert!((d - want).abs() < 2e-3, "query {k}: original {want} clone {d}");
    }
}

/// The original's queries of one car–wall pair, in order from the pair's
/// first (the original under Wine, free roam on the highway north of the
/// yellow barrel group, the PEUGOT transplanted at 120 km/h and drifting
/// into the wall on its right; contact log with every pair, exact floats):
/// the gap and the car's transform (centre of mass, x and y rows). The last
/// eight are the time of impact's search of the first contact; its moment
/// is the search's clear query (0.003174, the 14th), its last query the
/// overlapping one after it.
const WALL_QUERIES: [(f32, [f32; 9]); 15] = [
    (0.994781, [379.5216979980469, -1485.687744140625, 36.30189514160156, -0.0004363470070529729, 0.9990929961204529, -0.04257920756936073, -0.9998726844787598, -0.0011150817153975368, -0.015918070450425148]), // detect
    (0.904816, [379.5213317871094, -1485.1253662109375, 36.28586959838867, -0.0004797823785338551, 0.9991006851196289, -0.042397815734148026, -0.999875545501709, -0.0011476976796984673, -0.015730591490864754]), // detect
    (0.718536, [379.5209655761719, -1484.5811767578125, 36.27069854736328, -0.0005209037917666137, 0.9991064667701721, -0.04226287081837654, -0.999881386756897, -0.0011712731793522835, -0.01536536868661642]), // detect
    (0.519196, [379.5204772949219, -1483.9989013671875, 36.25481033325195, -0.0005638060392811894, 0.9991103410720825, -0.04217115417122841, -0.9998893737792969, -0.0011904311832040548, -0.014835457317531109]), // detect
    (0.32605, [379.5199890136719, -1483.434814453125, 36.239715576171875, -0.0006043165922164917, 0.9991118311882019, -0.0421343669295311, -0.9998989701271057, -0.0012023437302559614, -0.014169437810778618]), // detect
    (0.139099, [379.5195007324219, -1482.8890380859375, 36.225242614746094, -0.0006445039180107415, 0.999116837978363, -0.04201487451791763, -0.9999117255210876, -0.001201722538098693, -0.013238507322967052]), // detect
    (0.0, [379.5188903808594, -1482.30517578125, 36.20987319946289, -0.0006882812012918293, 0.9991253614425659, -0.04181065037846565, -0.9999273419380188, -0.0011909990571439266, -0.011999961920082569]), // detect
    (0.039154, [379.5191955566406, -1482.59716796875, 36.217559814453125, -0.0006663925596512854, 0.9991210699081421, -0.04191276431083679, -0.9999195337295532, -0.0011963607976213098, -0.01261923462152481]), // toi
    (0.0, [379.51904296875, -1482.451171875, 36.213714599609375, -0.0006773368804715574, 0.999123215675354, -0.04186170548200607, -0.9999234676361084, -0.0011936798691749573, -0.012309597805142403]), // toi
    (0.01413, [379.51910400390625, -1482.524169921875, 36.21563720703125, -0.0006718647200614214, 0.999122142791748, -0.04188723489642143, -0.9999215006828308, -0.0011950203916057944, -0.012464416213333607]), // toi
    (0.0, [379.5190734863281, -1482.4876708984375, 36.21467590332031, -0.0006746008293703198, 0.999122679233551, -0.0418744720518589, -0.9999224543571472, -0.0011943501885980368, -0.012387007474899292]), // toi
    (0.007843, [379.51910400390625, -1482.505859375, 36.21515655517578, -0.0006732327747158706, 0.9991224408149719, -0.04188085347414017, -0.999921977519989, -0.0011946852318942547, -0.012425712309777737]), // toi
    (0.0047, [379.51910400390625, -1482.4967041015625, 36.21491622924805, -0.0006739167729392648, 0.9991225600242615, -0.041877660900354385, -0.9999222159385681, -0.0011945177102461457, -0.012406359426677227]), // toi
    (0.003174, [379.5190734863281, -1482.4921875, 36.21479797363281, -0.0006742588011547923, 0.9991226196289062, -0.041876066476106644, -0.9999223351478577, -0.0011944338912144303, -0.012396683916449547]), // toi
    (0.0, [379.5190734863281, -1482.4898681640625, 36.21473693847656, -0.0006744298152625561, 0.999122679233551, -0.04187526926398277, -0.9999223947525024, -0.0011943919816985726, -0.01239184569567442]), // toi
];
const WALL_FACE: [[f32; 3]; 4] = [[385.25, -1494.625, 36.171875], [385.25, -1494.625, 40.9921875], [379.5, -1478.875, 40.5078125], [379.5, -1478.875, 35.6875]];


/// The explore bench's wall (Bayview, the face through (1716.625, −892.25)
/// and (1724.125, −850.125), normal (−0.9845, 0.1753, 0)): the PEUGOT at full
/// throttle square against it, its front edge (`_CV` vertices 21 and 31, one
/// above the other) kept by the pair from the frame before, the pose of a
/// push-out (0x5B4300) taken from the clone's run of the transplanted
/// scenario (docs/prop-hits.md §7c). The original's query rebuilds the kept
/// vertices about the centre of mass (0x5A1980, 0x43C9C0 → 0x441000): here
/// vertex 21 comes out one float step nearer the wall than 31, the query
/// keeps it and its next support (the climb from 21, 21 again) ends it, so
/// the push-out clears after its first 5 mm step, as every one of the
/// original's push-outs at this wall did (one query each, e.g. gap 0.0161 m).
/// Built as origin + R·v instead, the two tie: the reduction keeps the first
/// (31), the climb from the last kept (21) stops at 21, which is then new to
/// the set and placed R·c (0.3 m) off, so the query reports an overlap until
/// the car has been pushed 0.32 m back.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn wall_push_out_rebuilds_the_kept_edge_as_the_original() {
    use glam::{Mat3, Quat, Vec3};
    let root = common::game_root();
    let car = nfsu2::car::Car::load(std::path::Path::new(&root), "PEUGOT").unwrap();
    let f = f32::from_bits;
    let origin = Vec3::new(f(0x44d6_7122), f(0xc45d_d260), f(0x41be_6bdb));
    let rot = Mat3::from_quat(Quat::from_xyzw(f(0x3814_8d68), f(0xb7cc_7e56), f(0xbdb4_cceb), f(0x3f7f_001f)));
    let wall = [
        Vec3::new(1716.625, -892.25, -2048.0),
        Vec3::new(1716.625, -892.25, 2048.0),
        Vec3::new(1724.125, -850.125, 2048.0),
        Vec3::new(1724.125, -850.125, -2048.0),
    ];
    let normal = Vec3::new(-0.98451763, 0.17528504, 0.0);
    let com = origin + rot * Vec3::from(car.car_type.handling.centre_of_mass);
    let side = nfsu2::narrow::Side::hull(rot * car.hull_offset(), &car.hull_links);
    let push = |hull: &[Vec3]| {
        let mut cache = Some(nfsu2::narrow::PairCache::warm(vec![31, 21], vec![2, 0, 1]));
        let offset = nfsu2::contact::push_out(hull, &wall, com, normal, side, &mut cache);
        (offset.length(), cache.unwrap().kept().0.to_vec())
    };
    let toward = |p: Vec3| -(p - wall[0]).dot(normal);

    let native = car.hull_points(origin, rot);
    assert!(toward(native[21]) > toward(native[31]), "{} {}", native[21], native[31]);
    let (moved, kept) = push(&native);
    assert!((moved - 0.005).abs() < 1e-6 && kept == [21], "moved {moved} kept {kept:?}");

    let summed: Vec<Vec3> = car.hull.iter().map(|v| origin + rot * *v).collect();
    assert_eq!(toward(summed[31]), toward(summed[21]));
    let (moved, _) = push(&summed);
    assert!(moved > 0.3, "origin + R·v: moved {moved}");
}

/// 0x5B2620 → 0x5AF0F0 → 0x594010: the contact point is the midpoint of
/// the pair's closest points as the last query of the time of impact's
/// search left them (+0xC0 / +0xD0), not a query at the moment the car is
/// moved back to; with it the impulse is the original's (its rigid body
/// before and after 0x5AF0F0, read under Wine).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn wall_contact_takes_the_searchs_last_points() {
    use glam::{Mat3, Vec3};
    let root = common::game_root();
    let car = nfsu2::car::Car::load(std::path::Path::new(&root), "PEUGOT").unwrap();
    let h = &car.car_type.handling;
    let at = |t: &[f32; 9]| -> (Vec3, Mat3) {
        let (x, y) = (Vec3::new(t[3], t[4], t[5]), Vec3::new(t[6], t[7], t[8]));
        (Vec3::new(t[0], t[1], t[2]), Mat3::from_cols(x, y, x.cross(y)))
    };
    let face: Vec<Vec3> = WALL_FACE.iter().map(|p| Vec3::from(*p)).collect();
    let query = |t: &[f32; 9], cache: &mut Option<nfsu2::narrow::PairCache>| {
        let (p, r) = at(t);
        let hull: Vec<Vec3> = car.hull.iter().map(|v| p + r * (*v - car.hull_offset())).collect();
        let raw = [nfsu2::narrow::Side::hull(r * car.hull_offset(), &car.hull_links), nfsu2::narrow::Side::NONE];
        nfsu2::narrow::distance_at(&hull, &face, p, Vec3::ZERO, raw, cache)
    };
    let mut cache = None;
    for (k, (want, t)) in WALL_QUERIES.iter().enumerate() {
        let d = query(t, &mut cache).0;
        println!("query {k}: original {want:.6} clone {d:.6}");
        assert!((d - want).abs() < 1e-4, "query {k}: original {want} clone {d}");
    }
    // The contact point: the last query's points, as the original's 0x594010 read them.
    let (_, x, y) = cache.as_ref().unwrap().last().unwrap();
    let point = (x + y) * 0.5;
    let want = Vec3::new(380.33917, -1481.17700, 36.47986);
    assert!(point.distance(want) < 5e-5, "point {point} against the original's {want}");
    // Measured again at the moment (the 14th query's pose) it lies 2 mm off.
    let (_, x2, y2) = query(&WALL_QUERIES[13].1, &mut None);
    assert!(((x2 + y2) * 0.5).distance(want) > 1e-3);
    // The impulse from there (0x5AF0F0 against a wall: e 0.05, μ 0.09).
    let (com, rot) = at(&WALL_QUERIES[13].1);
    // The ground tensor (0x59B950: yaw term × record +0x270), the car on its wheels.
    let inertia = Vec3::from(h.inertia) * Vec3::new(1.0, 1.0, h.unknown_270);
    let (e, mu) = nfsu2::contact::materials(0.0, 4);
    let body = nfsu2::contact::Body {
        velocity: Vec3::new(-0.03429770842194557, 33.91706466674805, -0.8905678391456604),
        spin: Vec3::new(0.013581253588199615, 0.0778525322675705, -0.0007016605231910944),
        mass: h.mass,
        inverse_inertia: Box::new(move |v| v / inertia),
        up: rot * Vec3::Z,
        all_wheels_down: false,
        crash: false,
    };
    let normal = Vec3::new(-0.939357340335846, -0.34294000267982483, 0.0);
    let (dp, dl) = nfsu2::contact::impulse(&body, point - com, normal, e, mu).unwrap();
    // The original's rigid body: momentum and angular momentum before and after.
    let want_dp = Vec3::new(-9.436842918395996, 31.359561920166016, -0.9121862053871155)
        - Vec3::new(-0.03618408739566803, 35.78250503540039, -0.9395491480827332);
    let want_dl = Vec3::new(0.5033341646194458, -0.9727269411087036, 3.5015482902526855)
        - Vec3::new(0.020000308752059937, 0.03294619917869568, 0.007004398386925459);
    println!("dP {dp} original {want_dp}; dL {dl} original {want_dl}");
    assert!((dp - want_dp).length() < 2e-3, "dP {dp} against {want_dp}");
    assert!((dl - want_dl).length() < 2e-3, "dL {dl} against {want_dl}");
}

/// 0x5B33A0 / 0x595050 in the original's float order: the original's
/// queries of the player's PEUGOT against one highway wall at 120 km/h and
/// the yellow barrels at 60, 120 and 200 km/h (the contact log under Wine,
/// every float as bits,
/// the maintainer's private capture), each run from the pair's
/// logged state before it on the logged transforms, give the original's
/// distance, closest points and kept vertices to the last bit — among them
/// the time-of-impact query 12 µm over the warm boundary (0.003174 m) that
/// sets the wall's second contact moment.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn wall_and_barrel_queries_match_the_original_to_the_bit() {
    use glam::{Mat3, Vec3};
    use nfsu2::narrow::{self, Body, PairCache, Side};
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "PEUGOT").unwrap();
    let text = common::private_str("docs/evidence/prop-hits/query-bits.tsv");
    let bits = |s: &str| -> Vec<f32> { s.split(',').filter(|t| !t.is_empty()).map(|t| f32::from_bits(u32::from_str_radix(t, 16).unwrap())).collect() };
    let v3 = |s: &str| -> Vec3 { Vec3::from_slice(&bits(s)) };
    let ints = |s: &str| -> Vec<usize> { s.split(',').filter(|t| !t.is_empty()).map(|t| t.parse().unwrap()).collect() };
    let frame = |s: &str| -> (Vec3, Mat3) {
        let w: Vec<Vec3> = s.split(' ').map(v3).collect();
        (w[0], Mat3::from_cols(w[1], w[2], w[3]))
    };
    let hull = |stored: &[Vec3], c: Vec3, p: Vec3, r: Mat3| -> Vec<Vec3> { stored.iter().map(|v| narrow::body_point(r, p, *v - c)).collect() };
    let mut last: std::collections::HashMap<String, (Vec3, Vec3)> = Default::default();
    let (mut n, mut boundary) = (0, 0);
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        let (want_d, want_x, want_y) = (bits(c[8])[0], v3(c[9]), v3(c[10]));
        let before = last.insert(c[1].to_string(), (want_x, want_y));
        if c[14] != "1" {
            continue; // not measured in that call (the pair was not dirty)
        }
        let (pa, ra) = frame(c[6]);
        let offset = car.hull_offset();
        let a = hull(&car.hull, offset, pa, ra);
        let side_a = Side::hull(ra * offset, &car.hull_links).with_body(Body { stored: &car.hull, offset, rot: ra, at: pa });
        let (b, side_b, prop);
        if c[0] == "wall" {
            b = c[7].split(' ').map(v3).collect::<Vec<_>>();
            side_b = Side::NONE;
        } else {
            let (pb, rb) = frame(c[7]);
            prop = world
                .smackables
                .iter()
                .filter(|p| p.info.name == "ZCS_Barrel_A")
                .min_by(|p, q| {
                    let d = |p: &nfsu2::smackable::Prop| p.rest.transform_point3(p.info.centre_of_mass).distance(pb);
                    d(p).total_cmp(&d(q))
                })
                .unwrap();
            b = hull(&prop.raw_shape, prop.offset, pb, rb);
            side_b = Side::hull(rb * prop.offset, &prop.links).with_body(Body { stored: &prop.raw_shape, offset: prop.offset, rot: rb, at: pb });
        }
        let mut cache = (c[3] == "1").then(|| PairCache::warm_at(ints(c[4]), ints(c[5]), before));
        let (d, x, y) = narrow::distance(&a, &b, [side_a, side_b], &mut cache);
        let at = format!("{} query {n} ({})", c[0], c[2]);
        assert_eq!(d.to_bits(), want_d.to_bits(), "{at}: distance {d} against {want_d}");
        assert_eq!((x.to_array().map(f32::to_bits), y.to_array().map(f32::to_bits)), (want_x.to_array().map(f32::to_bits), want_y.to_array().map(f32::to_bits)), "{at}: closest points");
        if c[13] == "1" {
            let kept = cache.as_ref().map(|k| (k.kept().0.to_vec(), k.kept().1.to_vec())).unwrap_or_default();
            assert_eq!(kept, (ints(c[11]), ints(c[12])), "{at}: kept vertices");
        }
        boundary += (want_d == 0.003_173_828_1) as usize;
        n += 1;
    }
    assert!(n > 250 && boundary > 0, "{n} queries, {boundary} at the boundary");
}

/// 0x5B2620 → 0x5AF0F0 → 0x594010 for two cars: the contact point is the
/// midpoint of the pair's closest points (+0xC0 / +0xD0) as its last query
/// left them, the same rule as a car–track pair (observed under Wine in
/// every one of 77 car–car contacts, docs/prop-hits.md §7f). One rear
/// contact (the PEUGOT into a PICKUP) and one side contact (into an SUV,
/// the PEUGOT's centre of mass moved after an earlier hit: its offset as
/// logged), the maintainer's private capture: the pair's
/// queries of the contact's frame, each from its logged entry state, give
/// the original's bits, and the last one's midpoint is the original's point.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn car_car_contacts_take_the_last_querys_points() {
    use glam::{Mat3, Vec3};
    use nfsu2::narrow::{self, Body, PairCache, Side};
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let player = nfsu2::car::Car::load(root, "PEUGOT").unwrap();
    let text = common::private_str("docs/evidence/prop-hits/carcar-contacts.tsv");
    let bits = |s: &str| -> Vec<f32> { s.split(',').filter(|t| !t.is_empty()).map(|t| f32::from_bits(u32::from_str_radix(t, 16).unwrap())).collect() };
    let v3 = |s: &str| -> Vec3 { Vec3::from_slice(&bits(s)) };
    let ints = |s: &str| -> Vec<usize> { s.split(',').filter(|t| !t.is_empty()).map(|t| t.parse().unwrap()).collect() };
    let same = |a: Vec3, b: Vec3| a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits);
    let lines: Vec<Vec<&str>> = text.lines().filter(|l| !l.starts_with('#')).map(|l| l.split('\t').collect()).collect();
    let mut models: std::collections::HashMap<&str, nfsu2::car::Car> = Default::default();
    for c in &lines {
        models.entry(c[1]).or_insert_with(|| nfsu2::car::Car::load(root, c[1]).unwrap());
    }
    let side = |car: &'_ nfsu2::car::Car, f: &str| -> (Vec<Vec3>, Mat3, Vec3, Vec3) {
        let w: Vec<Vec3> = f.split(' ').map(v3).collect();
        let (off, at, rot) = (w[0], w[1], Mat3::from_cols(w[2], w[3], w[4]));
        (car.hull.iter().map(|v| narrow::body_point(rot, at, *v - off)).collect(), rot, at, off)
    };
    let mut last: Option<(Vec3, Vec3)> = None;
    let mut contacts = 0;
    for c in &lines {
        let other = &models[c[1]];
        if c[0] == "contact" {
            let (x, y) = last.take().expect("a query before the contact");
            let point = (x + y) * 0.5;
            assert!(same(point, v3(c[2])), "{}: point {point} against the original's {}", c[1], v3(c[2]));
            contacts += 1;
            continue;
        }
        let (want_d, want_x, want_y) = (bits(c[8])[0], v3(c[9]), v3(c[10]));
        let before = last.replace((want_x, want_y));
        if c[14] != "1" {
            continue;
        }
        let ((a, ra, ta, oa), (b, rb, tb, ob)) = (side(&player, c[6]), side(other, c[7]));
        let sa = Side::hull(ra * oa, &player.hull_links).with_body(Body { stored: &player.hull, offset: oa, rot: ra, at: ta });
        let sb = Side::hull(rb * ob, &other.hull_links).with_body(Body { stored: &other.hull, offset: ob, rot: rb, at: tb });
        let mut cache = (c[3] == "1").then(|| PairCache::warm_at(ints(c[4]), ints(c[5]), before));
        let (d, x, y) = narrow::distance(&a, &b, [sa, sb], &mut cache);
        assert_eq!(d.to_bits(), want_d.to_bits(), "{} {}: distance {d} against {want_d}", c[1], c[2]);
        assert!(same(x, want_x) && same(y, want_y), "{} {}: closest points", c[1], c[2]);
        if c[13] == "1" {
            let kept = cache.as_ref().map(|k| (k.kept().0.to_vec(), k.kept().1.to_vec())).unwrap_or_default();
            assert_eq!(kept, (ints(c[11]), ints(c[12])), "{} {}: kept vertices", c[1], c[2]);
        }
        last = Some((x, y));
    }
    assert_eq!(contacts, 2);
}
