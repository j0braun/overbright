//! Stage-4 drag launch parity (docs/notes/parity/drag-stage4-parity.md): the
//! original's launch as the maintainer's tool `subtrap.py` captured it sub-step by
//! sub-step under Wine. Ignored without NFSU2_ROOT.
mod common;

use glam::Vec3;
use nfsu2::car::Car;
use nfsu2::drive::{AiControls, CarField, CarState, Input, Racer, step_cars};

fn skyline() -> Car {
    let root = common::game_root();
    let career = nfsu2::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc")).expect("reading GLOBAL/GlobalB.lzc")).expect("decompressing GlobalB.lzc");
    let group = nfsu2::formats::bundle::chunks(&career, 0).find(|c| c.id == 0x8003_4A10).expect("career chunk 0x34A10");
    let packages = nfsu2::performance::parse(group.child(0x34A15).expect("performance packages (0x34A15)").body);
    Car::load_physics(&root, "SKYLINE", &packages, &|_| false, None).expect("loading the SKYLINE")
}

fn ai(gas: f32, brake: f32, gear: i32) -> Input {
    Input::Ai(AiControls { steer_degrees: 0.0, gas, brake, handbrake: 0.0, gear, nitrous: false, mover_held: false })
}

fn frame(car: &Car, state: &mut CarState, world: &nfsu2::world::World, input: Input) {
    step_cars(&mut [Racer { car, state, input }], &mut CarField::default(), world, 1.0 / 60.0);
}

fn start(car: &Car) -> (CarState, nfsu2::world::World) {
    let w = car.car_type.wheels[0];
    (CarState::new(Vec3::new(-900.0, 0.0, w.radius - w.position.z), 0.0), nfsu2::world::World::flat(2000.0))
}

/// A car braked to a stop has its front wheels locked (0x59D410); the
/// replay's snapshot take (the wheel serialiser's take branch, 0x59CA6A /
/// 0x59CAC6) frees every wheel, so the original's Skyline left the drag's
/// grid with four free wheels.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_snapshot_take_frees_wheels_the_brakes_locked() {
    let car = skyline();
    let (mut state, world) = start(&car);
    for _ in 0..60 {
        frame(&car, &mut state, &world, ai(1.0, 0.0, -1));
    }
    let mut frames = 0;
    while state.speed() > 0.01 {
        frame(&car, &mut state, &world, ai(0.0, 1.0, -1));
        frames += 1;
        assert!(frames < 600, "never stopped");
    }
    assert!(state.wheels.iter().any(|w| w.locked), "no wheel locked braking to a stop");
    state.snapshot_take();
    assert!(state.wheels.iter().all(|w| !w.locked && !w.sliding));
}

/// The rest test (0x5992C0 at 0x599324) reads the car's gear +0x434, which
/// Car_SpeedAndWheelSpin (0x5A5924) copies from the drivetrain once a frame
/// after the sub-steps: the frame in which the clutch puts first gear in
/// still holds the car (observed under Wine: the Skyline's launch frame 4,
/// clutch 0.080, drive torque sent, speed and wheel spins exactly 0).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_frame_that_engages_first_gear_still_holds_the_car() {
    let car = skyline();
    let (mut state, world) = start(&car);
    // Revving in neutral on the line.
    for _ in 0..90 {
        frame(&car, &mut state, &world, ai(1.0, 0.0, 1));
    }
    assert_eq!(state.speed(), 0.0);
    let mut engaged = None;
    for f in 0..60 {
        let before = state.drivetrain.as_ref().unwrap().gear;
        frame(&car, &mut state, &world, ai(1.0, 0.0, 2));
        let after = state.drivetrain.as_ref().unwrap().gear;
        if before == nfsu2::handling::NEUTRAL && after == 2 {
            engaged = Some(f);
            assert_eq!(state.velocity, Vec3::ZERO, "the engaging frame moved the car");
            assert!(state.wheels.iter().all(|w| w.spin == 0.0));
            frame(&car, &mut state, &world, ai(1.0, 0.0, 2));
            assert!(state.speed() > 0.0, "the next frame does not hold it");
            break;
        }
    }
    assert!(engaged.is_some(), "first gear never engaged");
}

/// The per-frame readers see the slip of the state the frame ended in (the
/// original's integrator evaluates the tyre force after each integration):
/// on a straight launch on the flat, r·ω − v.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_frame_leaves_the_end_states_slip() {
    let car = skyline();
    let (mut state, world) = start(&car);
    for _ in 0..30 {
        frame(&car, &mut state, &world, ai(1.0, 0.0, -1));
        let v = state.velocity.dot(state.forward());
        for (i, w) in state.wheels.iter().enumerate() {
            let r = car.car_type.wheels[i].radius;
            assert!((w.slip - (r * w.spin - v)).abs() < 0.05, "wheel {i}: {} against {}", w.slip, r * w.spin - v);
        }
    }
}
