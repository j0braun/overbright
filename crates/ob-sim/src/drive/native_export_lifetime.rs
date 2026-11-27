//! Actual production mover/contact boundaries; native captures define publication order.
use super::*;
use std::cell::RefCell;
#[derive(Clone, Copy, Debug)]
struct Observation {
    car: usize,
    entry: bool,
    restep: bool,
    retained: f32,
    projection: f32,
}
thread_local! { static TRACE: RefCell<Option<Vec<Observation>>> = const { RefCell::new(None) }; }
// `entry` is the force-loop consumer boundary after frame setup, not the
// native mover-entry hook. Exit is immediately after scalar publication.
pub(super) fn observe(state: &CarState, entry: bool) {
    TRACE.with(|trace| {
        if let Some(rows) = trace.borrow_mut().as_mut() {
            let v = state.velocity;
            let f = state.forward();
            rows.push(Observation {
                car: state as *const _ as usize,
                entry,
                restep: state.restep,
                retained: state.exported_forward_speed(),
                projection: (v.z * f.z + v.y * f.y) + v.x * f.x,
            });
        }
    });
}
/// The original 240SX (the tests that call it need the game).
fn car() -> Car {
    Car::load(&crate::testkit::game_root(), "240SX").unwrap()
}
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn actual_world_mover_publishes_before_post_motion_callback() {
    let car = car();
    let world = World::flat(2000.0);
    let mut state = CarState::new(Vec3::new(0.0, 0.0, 100.0), 0.0);
    state.velocity = Vec3::X * 20.0;
    state.tyre_speed = 20.0; // supplied initial exported state
    let mut observed = None;
    step_world_with_post_motion(
        &mut [Racer {
            car: &car,
            state: &mut state,
            input: Input::Player(Controls::default()),
        }],
        &mut CarField::default(),
        &mut crate::smackable::Props::default(),
        &world,
        0.05,
        &mut ContactSounds::default(),
        |_, r| {
            let v = r.state.velocity;
            let f = r.state.forward();
            observed = Some((
                r.state.exported_forward_speed(),
                (v.z * f.z + v.y * f.y) + v.x * f.x,
            ));
        },
    );
    assert_eq!(
        state.resolved, 0,
        "ordinary control has no contact response"
    );
    let (retained, projection) = observed.unwrap();
    assert!((projection - 20.0).abs() > 1e-4);
    assert_eq!(
        retained.to_bits(),
        projection.to_bits(),
        "post-motion cache={retained}, projection={projection}"
    );
}
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn actual_world_contact_restep_consumes_previous_export() {
    let car = car();
    let world = World::flat(2000.0);
    let mut a = CarState::placed(&car.car_type, &world, Vec3::new(-3.0, 0.0, 0.0), 0.0);
    let mut b = CarState::placed(
        &car.car_type,
        &world,
        Vec3::new(3.0, 0.0, 0.0),
        std::f32::consts::PI,
    );
    a.velocity = Vec3::X * 15.0;
    b.velocity = -Vec3::X * 15.0;
    a.tyre_speed = 15.0;
    b.tyre_speed = 15.0; // supplied initial exported states
    let mut field = CarField::default();
    TRACE.with(|t| *t.borrow_mut() = Some(Vec::new()));
    for _ in 0..60 {
        step_cars(
            &mut [
                Racer {
                    car: &car,
                    state: &mut a,
                    input: Input::Player(Controls::default()),
                },
                Racer {
                    car: &car,
                    state: &mut b,
                    input: Input::Player(Controls::default()),
                },
            ],
            &mut field,
            &world,
            1.0 / 120.0,
        );
    }
    let rows = TRACE.with(|t| t.borrow_mut().take().unwrap());
    let mut exports = std::collections::HashMap::new();
    let mut resteps = 0;
    let mut changed = 0;
    for r in rows {
        if !r.entry {
            exports.insert(r.car, r.projection);
        } else if r.restep {
            resteps += 1;
            let previous = *exports
                .get(&r.car)
                .expect("contact follows a completed mover");
            if previous.to_bits() != r.projection.to_bits() {
                changed += 1;
            }
            assert_eq!(
                r.retained.to_bits(),
                previous.to_bits(),
                "contact restep: {r:?}, prior export={previous}"
            );
        }
    }
    assert!(
        resteps > 0 && changed > 0,
        "real collision must create a distinguishing re-step: {resteps}/{changed}"
    );
    eprintln!("actual world contact: {resteps} re-steps, {changed} differing live projections");
}

#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn native_export_and_placement_writers_are_bit_exact() {
    let rows = &crate::testkit::private("docs/evidence/tyre-export-lifetime/exports.bin");
    assert_eq!(rows.len(), 3514 * 36);
    for row in rows.chunks_exact(36) {
        let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap());
        let v = |i| Vec3::new(f(i), f(i + 4), f(i + 8));
        assert_eq!(
            exported_forward_speed(v(8), v(20)).to_bits(),
            f(32).to_bits(),
            "native capture/index {:?}",
            &row[..8]
        );
    }
    let row = &crate::testkit::private("docs/evidence/tyre-export-lifetime/placements.bin");
    assert_eq!(row.len(), 40);
    let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap());
    let axis = Vec3::new(f(12), f(16), f(20));
    assert_eq!(
        placement_velocity(axis, f(8)).to_array().map(f32::to_bits),
        [f(24), f(28), f(32)].map(f32::to_bits)
    );
    let mut state = CarState::new(Vec3::ZERO, 0.0);
    state.set_placement_speed(f(8));
    assert_eq!(state.exported_forward_speed().to_bits(), f(36).to_bits());
    assert_eq!(state.velocity.x.to_bits(), 8.1f32.to_bits());
    assert_ne!(
        state.exported_forward_speed().to_bits(),
        state.speed().to_bits()
    );
}
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn actual_world_placement_argument_survives_until_mover_export() {
    let car = car();
    let world = World::flat(2000.0);
    let mut state = CarState::new(Vec3::new(0.0, 0.0, 100.0), 0.0);
    state.set_placement_speed(8.0);
    TRACE.with(|t| *t.borrow_mut() = Some(Vec::new()));
    state.step(&car, &world, Controls::default(), 1.0 / 120.0);
    let rows = TRACE.with(|t| t.borrow_mut().take().unwrap());
    assert_eq!(rows.len(), 2);
    assert!(rows[0].entry && !rows[1].entry);
    assert_eq!(rows[0].retained.to_bits(), 8.0f32.to_bits());
    assert_eq!(rows[0].projection.to_bits(), 8.1f32.to_bits());
    assert_eq!(rows[1].retained.to_bits(), rows[1].projection.to_bits());
}
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn explicit_script_or_traffic_export_survives_physics_handoff() {
    let car = car();
    let world = World::flat(2000.0);
    for traffic in [false, true] {
        let mut state = CarState::new(Vec3::new(0.0, 0.0, 100.0), 0.2);
        state.velocity = Vec3::new(12.0, 3.0, 1.0);
        if traffic {
            state.set_exported_forward_speed(12.5);
        } else {
            state.publish_forward_speed();
        }
        let expected = state.exported_forward_speed();
        TRACE.with(|t| *t.borrow_mut() = Some(Vec::new()));
        state.step(&car, &world, Controls::default(), 1.0 / 120.0);
        let rows = TRACE.with(|t| t.borrow_mut().take().unwrap());
        assert_eq!(rows[0].retained.to_bits(), expected.to_bits());
    }
}
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn actual_melo_export_is_retained_on_physics_handoff() {
    let car = car();
    let world = World::flat(2000.0);
    let mut state = CarState::new(Vec3::new(0.0, 0.0, 100.0), 0.2);
    state.velocity = Vec3::X * 7.0;
    state.publish_forward_speed();
    step_cars(
        &mut [Racer {
            car: &car,
            state: &mut state,
            input: Input::Melo(AiControls::default()),
        }],
        &mut CarField::default(),
        &world,
        1.0 / 60.0,
    );
    assert!(state.melo.is_some());
    let expected = exported_forward_speed(state.velocity, state.forward());
    assert_eq!(state.exported_forward_speed().to_bits(), expected.to_bits());
    state.leave_melo();
    TRACE.with(|t| *t.borrow_mut() = Some(Vec::new()));
    state.step(&car, &world, Controls::default(), 1.0 / 120.0);
    let rows = TRACE.with(|t| t.borrow_mut().take().unwrap());
    assert_eq!(rows[0].retained.to_bits(), expected.to_bits());
}
