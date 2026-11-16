//! Tests of the physics rules ported from the
//! binary audit (docs/native-fixes/N-PHYS.tsv), each stating the native
//! rule and its address.
use super::*;

/// The original 240SX (rear-wheel drive: rear share 1.0 > 0.8).
fn car() -> Car {
    Car::load(&crate::testkit::game_root(), "240SX").unwrap()
}

fn record(upright: bool) -> ContactRecord {
    ContactRecord {
        age: 0.0,
        life: 0.0,
        point: Vec3::ZERO,
        upright,
        wall: upright,
        sounded: false,
        slide: 0.0,
        id: 0,
        impulse: 0.0,
        sound: (9, Vec3::ZERO),
    }
}

/// The wall drive (0x5A49B0) reads car +0x430 = |v|
/// (`fld [ecx+0x430]` at 0x5A4D2A) × 2.237 against 60 mph, not the signed
/// forward speed: a car reversing at 30 m/s (67 mph) along a wall keeps
/// its own rear share; at 20 m/s (45 mph) it gets 0.65.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn wall_drive_reads_the_speed_not_the_forward_speed() {
    let car = car();
    let h = &car.car_type.handling;
    assert!(h.gearbox.rear_split > 0.8);
    for (v, want) in [(-30.0, h.gearbox.rear_split), (-20.0, 0.65), (30.0, h.gearbox.rear_split)] {
        let mut state = CarState::new(Vec3::ZERO, 0.0);
        state.velocity = Vec3::X * v;
        state.records.insert((0, 0), record(true));
        let mut dt = Drivetrain::new(h);
        state.wall_drive(h, &mut dt);
        assert_eq!(dt.rear_share, want, "v {v}");
    }
}

/// Observed under Wine (0 pair tests 0x5B5060 and 0 contacts
/// 0x5B2620 in the race start scene and countdown): two cars held by the
/// start take no car pair; the same two after the green collide.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn no_car_pairs_while_the_race_start_holds() {
    let car = car();
    let world = World::flat(2000.0);
    for hold in [true, false] {
        // Head on, 6 m apart, closing at 20 m/s.
        let mut a = CarState::new(Vec3::new(0.0, 0.0, 0.0), 0.0);
        let mut b = CarState::new(Vec3::new(6.0, 0.0, 0.0), std::f32::consts::PI);
        a.velocity = Vec3::X * 10.0;
        b.velocity = -Vec3::X * 10.0;
        a.start_hold = hold;
        b.start_hold = hold;
        let mut field = CarField::default();
        let mut props = crate::smackable::Props::default();
        let mut paired = false;
        for _ in 0..30 {
            step_world(
                &mut [
                    Racer { car: &car, state: &mut a, input: Input::Player(Controls::default()) },
                    Racer { car: &car, state: &mut b, input: Input::Player(Controls::default()) },
                ],
                &mut field,
                &mut props,
                &world,
                1.0 / 60.0,
                &mut ContactSounds::default(),
            );
            paired |= !field.pairs.is_empty();
        }
        assert_eq!(paired, !hold, "hold {hold}");
        // Held, they pass through each other; free, the push turns them back.
        assert_eq!(a.position.x > b.position.x, hold, "hold {hold}: {} {}", a.position.x, b.position.x);
    }
}

/// Observed under Wine: the jump camera's flight starts from
/// the wheel matrices 0x5921C0 builds, the wheel's place lifted by its
/// suspension offset (body +0x340 + 4·i), not the static place.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn jump_flight_starts_from_the_lifted_wheels() {
    let car = car();
    let mut state = CarState::new(Vec3::new(10.0, 5.0, 1.0), 0.3);
    let stat = car.car_type.wheels.iter().map(|w| state.position + state.orientation * (w.position - Vec3::Z * w.radius)).sum::<Vec3>() * 0.25;
    assert!((state.flight_start(&car) - stat).length() < 1e-5);
    for (i, w) in state.wheels.iter_mut().enumerate() {
        w.compression = [0.04, 0.04, -0.01, -0.01][i];
    }
    let lifted = state.flight_start(&car);
    assert!((lifted.z - stat.z - 0.015).abs() < 1e-5, "{} {}", lifted.z, stat.z);
}

/// The dyno pull's reset holds the car (0x56D350 → 0x599210(1)
/// at 0x56D415). The position spring 0x5A5C30 (k 150, damping 15, x/y)
/// keeps the centre of mass where it stood while the velocity runs on,
/// and the undriven wheels (the 240SX's front) turn with the held point,
/// not with the road speed.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_dyno_hold_keeps_the_car_in_place() {
    let car = car();
    let world = World::flat(2000.0);
    let w = car.car_type.wheels[0];
    let mut state = CarState::new(Vec3::new(0.0, 0.0, w.radius - w.position.z), 0.0);
    let settle = |s: &mut CarState, gas: f32, n: usize| {
        for _ in 0..n {
            let input = Input::Ai(AiControls { gas, gear: if gas > 0.0 { 2 } else { crate::handling::NEUTRAL as i32 }, ..Default::default() });
            step_cars(&mut [Racer { car: &car, state: s, input }], &mut CarField::default(), &world, 1.0 / 60.0);
        }
    };
    settle(&mut state, 0.0, 90);
    let com = |s: &CarState| s.position + s.orientation * Vec3::from(car.car_type.handling.centre_of_mass);
    let at = com(&state);
    state.set_hold(&car);
    settle(&mut state, 1.0, 120);
    assert!(state.velocity.length() > 3.0, "{}", state.velocity.length());
    assert!(com(&state).truncate().distance(at.truncate()) < 0.1, "{:?} {:?}", com(&state), at);
    assert!(state.wheels[0].spin.abs() < 1.0 && state.wheels[2].spin.abs() > 10.0, "{} {}", state.wheels[0].spin, state.wheels[2].spin);
}

/// The original's mover takes the force pass at the end of every sub-step,
/// after the integration and the tidy-ups (0x5B04E0 → vt+8 = 0x5B0530), and
/// runs the frame's pedal, brake and grip set-up (0x5ABC00, 0x5A4D90)
/// before its sub-step loop with no force pass in between (0x5B72E0). So a
/// frame's first sub-step integrates forces taken before that frame's input:
/// a one-sub-step frame (dt under 4.4 ms, 0x7FC070) with the brake pressed
/// moves the body and turns the wheels exactly as one without it, and the
/// brake shows from the next frame on. The engine's derivative takes its
/// torque at the integration (0x5A12A0 → 0x594310), so the throttle acts on
/// the crank in that same first sub-step.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_frames_first_substep_integrates_the_forces_taken_before_its_input() {
    let car = car();
    let world = World::flat(2000.0);
    let w = car.car_type.wheels[0];
    let mut base = CarState::new(Vec3::new(0.0, 0.0, w.radius - w.position.z), 0.0);
    let step = |s: &mut CarState, c: Controls, dt: f32| {
        step_cars(&mut [Racer { car: &car, state: s, input: Input::Player(c) }], &mut CarField::default(), &world, dt);
    };
    // Rolling at 20 m/s in gear, the gas held.
    base.velocity = Vec3::X * 20.0;
    for wheel in &mut base.wheels {
        wheel.spin = 20.0 / w.radius;
    }
    let gas = Controls { throttle: 1.0, ..Default::default() };
    for _ in 0..60 {
        step(&mut base, gas, 1.0 / 60.0);
    }
    let one = 1.0 / 240.0;
    let (mut held, mut braked, mut lifted) = (base.clone(), base.clone(), base.clone());
    step(&mut held, gas, one);
    step(&mut braked, Controls { brake: 1.0, ..gas }, one);
    step(&mut lifted, Controls { throttle: 0.0, ..gas }, one);
    for (s, what) in [(&braked, "brake"), (&lifted, "lift")] {
        assert_eq!(s.velocity, held.velocity, "{what}: the body in the first sub-step");
        assert_eq!(s.angular_momentum, held.angular_momentum, "{what}");
        for i in 0..4 {
            assert_eq!(s.wheels[i].spin, held.wheels[i].spin, "{what}: wheel {i}");
        }
    }
    let rpm = |s: &CarState| s.drivetrain.as_ref().unwrap().engine_w;
    assert!(rpm(&lifted) < rpm(&held), "the crank takes the frame's throttle: {} {}", rpm(&lifted), rpm(&held));
    step(&mut held, gas, one);
    step(&mut braked, Controls { brake: 1.0, ..gas }, one);
    assert!(braked.wheels[0].spin < held.wheels[0].spin - 0.05, "{} {}", braked.wheels[0].spin, held.wheels[0].spin);
    assert!(braked.velocity.x < held.velocity.x, "{} {}", braked.velocity.x, held.velocity.x);
}
