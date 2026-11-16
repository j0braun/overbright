//! The dyno's tuning reaches the driving physics (docs/formats.md, "Tuning
//! in the physics"): the car's set-up 0x5B4DD0 runs 0x5B4790 on the
//! tuning object, so a slider moves its field of the car's tuning block,
//! which the physics reads. Ignored without NFSU2_ROOT.
mod common;

use glam::Vec3;
use nfsu2::car::Car;
use nfsu2::drive::{AiControls, CarField, CarState, Input, Racer, step_cars};
use nfsu2::tuning::Settings;

struct Data {
    root: std::path::PathBuf,
    packages: Vec<nfsu2::performance::Package>,
    table: Vec<nfsu2::tuning::Unlock>,
}

fn data() -> Data {
    let root = common::game_root();
    let career = nfsu2::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc")).expect("reading GLOBAL/GlobalB.lzc")).expect("decompressing GlobalB.lzc");
    let group = nfsu2::formats::bundle::chunks(&career, 0).find(|c| c.id == 0x8003_4A10).expect("career chunk 0x34A10");
    let packages = nfsu2::performance::parse(group.child(0x34A15).expect("performance packages (0x34A15)").body);
    let table = nfsu2::tuning::parse(group.child(0x34A1A).expect("tuning table (0x34A1A)").body);
    Data { root, packages, table }
}

/// The Peugeot with `parts` fitted and one slider (all nine points for a
/// curve) at `t`.
fn car(d: &Data, parts: &[u8], param: u8, t: f32) -> Car {
    let mut s = Settings::default();
    (0..9).for_each(|i| s.set_t(param, i, t));
    Car::load_physics(&d.root, "PEUGOT", &d.packages, &|id| parts.contains(&id), Some((&d.table, &s))).unwrap()
}

/// Speed (m/s) after `secs` at full throttle from rest on a flat floor.
fn launch(car: &Car, secs: f32) -> f32 {
    let w = car.car_type.wheels[0];
    let mut state = CarState::new(Vec3::new(-900.0, 0.0, w.radius - w.position.z), 0.0);
    let world = nfsu2::world::World::flat(2000.0);
    let mut field = CarField::default();
    let dt = 1.0 / 60.0;
    let steps = (dt / nfsu2::drive::STEP).ceil().max(1.0) as u32;
    for _ in 0..(secs * 60.0) as usize {
        let input = Input::Ai(AiControls { steer_degrees: 0.0, gas: 1.0, brake: 0.0, handbrake: 0.0, gear: -1, nitrous: false, mover_held: false });
        for _ in 0..steps {
            step_cars(&mut [Racer { car, state: &mut state, input }], &mut field, &world, dt / steps as f32);
        }
    }
    state.speed()
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_tuned_final_drive_changes_the_launch() {
    let d = data();
    // Part 11 (level-3 transmission) opens the final drive (param 9, kind 0).
    let (short, mid, long) = (car(&d, &[11], 9, 1.0), car(&d, &[11], 9, 0.0), car(&d, &[11], 9, -1.0));
    let fd = |c: &Car| c.car_type.handling.gearbox.final_drive_rear;
    assert!(fd(&short) > fd(&mid) && fd(&mid) > fd(&long), "the final drive moves with the slider: {} {} {}", fd(&short), fd(&mid), fd(&long));
    // Without the part the slider does nothing (0x59B0F0 counts no tunable).
    assert_eq!(fd(&car(&d, &[], 9, 1.0)), fd(&car(&d, &[], 9, 0.0)));
    // The physics reads it: a shorter final drive gets off the line harder.
    let (a, b) = (launch(&short, 3.0), launch(&long, 3.0));
    assert!(a > b + 0.2, "3 s from rest: short {a:.2} m/s, long {b:.2} m/s");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_tuned_ecu_reshapes_the_torque_and_the_launch() {
    let d = data();
    // Part 51 (level-3 ECU) opens param 23. The low points raised: they
    // go to 0x5B2C10 one by one without spreading, then 0x59B3D0 shares
    // the change of the total back evenly, so the curve tilts towards low
    // rpm and keeps its total.
    let ecu = |t: f32| {
        let mut s = Settings::default();
        (0..3).for_each(|i| s.set_t(23, i, t));
        Car::load_physics(&d.root, "PEUGOT", &d.packages, &|id| id == 51, Some((&d.table, &s))).unwrap()
    };
    let (up, mid) = (ecu(1.0), ecu(0.0));
    let v = |c: &Car| c.car_type.handling.engine.torque.values;
    let (a, b) = (v(&up), v(&mid));
    assert!(a[1] > b[1] && a[8] < b[8], "ECU low points +1: {a:?} vs {b:?}");
    assert!((a.iter().sum::<f32>() - b.iter().sum::<f32>()).abs() < 0.002, "the total is kept");
    let (x, y) = (launch(&up, 4.0), launch(&mid, 4.0));
    assert!((x - y).abs() > 0.01, "the launch changes: {x} vs {y}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn tuned_chassis_fields_reach_the_handling() {
    let d = data();
    let h = |parts: &[u8], p: u8, t: f32| car(&d, parts, p, t).car_type.handling;
    // Brake bias (18, part 44), front spring (0, part 35), front ride
    // height (3, part 35: the wheel's z), steering ratio (8, part 35),
    // downforce (21, part 58), nitrous (16, part 14).
    let bias = (h(&[44], 18, -1.0).brake_balance, h(&[44], 18, 1.0).brake_balance);
    assert!(bias.0 != bias.1, "brake bias {bias:?}");
    let spring = (h(&[35], 0, -1.0).suspension[0].spring, h(&[35], 0, 1.0).suspension[0].spring);
    assert!(spring.0 < spring.1, "spring {spring:?}");
    let ride = |t: f32| car(&d, &[35], 3, t).car_type.wheels[0].position.z;
    assert!(ride(-1.0) != ride(1.0), "ride height {} {}", ride(-1.0), ride(1.0));
    let steer = (h(&[35], 8, -1.0).steering_ratio, h(&[35], 8, 1.0).steering_ratio);
    assert!(steer.0 != steer.1, "steering {steer:?}");
    let df = (h(&[58], 21, -1.0).downforce[0], h(&[58], 21, 1.0).downforce[0]);
    assert!(df.0 < df.1, "downforce {df:?}");
    let nos = (h(&[14], 16, -1.0).nitrous_capacity, h(&[14], 16, 1.0).nitrous_capacity);
    assert!(nos.0 < nos.1, "nitrous {nos:?}");
}

/// The replay manager's DUMMY_SNAPSHOT (every 100 world updates, 0x57D010
/// → 0x57C190; docs/tuning-physics-parity.md) empties the rigid body's
/// force sums before the next integration: the car drops for one sub-step.
/// Under Wine every car at a launch lost 0.031–0.036 m/s of vertical speed
/// in that update (circuit_s1g frames 17 / 117 / 217, the clone's CORSA
/// 0.032 against the original's 0.033); the same frame without the
/// snapshot changes it by under 0.002.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_dummy_snapshot_drops_the_car_for_a_sub_step() {
    let d = data();
    let car = car(&d, &[], 9, 0.0);
    let w = car.car_type.wheels[0];
    let mut state = CarState::new(Vec3::new(-900.0, 0.0, w.radius - w.position.z), 0.0);
    let world = nfsu2::world::World::flat(2000.0);
    let mut field = CarField::default();
    let input = Input::Ai(AiControls { steer_degrees: 0.0, gas: 1.0, brake: 0.0, handbrake: 0.0, gear: -1, nitrous: false, mover_held: false });
    for _ in 0..45 {
        step_cars(&mut [Racer { car: &car, state: &mut state, input }], &mut field, &world, 1.0 / 60.0);
    }
    let step = |mut s: CarState, snap: bool| {
        if snap {
            s.snapshot_take();
        }
        let vz = s.velocity.z;
        step_cars(&mut [Racer { car: &car, state: &mut s, input }], &mut CarField::default(), &world, 1.0 / 60.0);
        s.velocity.z - vz
    };
    let (plain, snap) = (step(state.clone(), false), step(state.clone(), true));
    assert!(plain.abs() < 0.01, "a plain launch frame: {plain}");
    let jolt = snap - plain;
    eprintln!("snapshot jolt {jolt:.4} (plain {plain:.4})");
    assert!((-0.045..=-0.025).contains(&jolt), "the snapshot's drop {jolt} (Wine: -0.031..-0.036)");
}

/// The same take clears the tyres' sliding flag (+0x160) and their drive
/// torque and force (+0x108, +0x124): the wheel's derivative (0x59D360)
/// takes the whole slip out within the first sub-step, then the wheels spin
/// up again. Captured sub-step by sub-step under Wine (`substep.py`,
/// docs/evidence/tuning-physics-parity/substep-capture.txt): a COROLLA's
/// spinning rear wheel 47.3 → 1.25 rad/s, 23.4 at the frame's end; in the
/// circuit runs the driven wheels end the frame at 0.54–0.55 of the
/// plain frame's spin (CORSA 21.6 / 39.5, 240SX 22.1 / 40.0).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_dummy_snapshot_lets_spinning_wheels_grip_for_a_sub_step() {
    let d = data();
    let car = car(&d, &[], 9, 0.0);
    let w = car.car_type.wheels[0];
    let mut state = CarState::new(Vec3::new(-900.0, 0.0, w.radius - w.position.z), 0.0);
    let world = nfsu2::world::World::flat(2000.0);
    let mut field = CarField::default();
    let input = Input::Ai(AiControls { steer_degrees: 0.0, gas: 1.0, brake: 0.0, handbrake: 0.0, gear: -1, nitrous: false, mover_held: false });
    // Revved in neutral as on the grid, then into the launch's wheelspin
    // (the clutch closing, the front wheels of the front-driven Peugeot
    // spinning).
    let rev = Input::Ai(AiControls { steer_degrees: 0.0, gas: 1.0, brake: 1.0, handbrake: 0.0, gear: 1, nitrous: false, mover_held: false });
    for _ in 0..90 {
        step_cars(&mut [Racer { car: &car, state: &mut state, input: rev }], &mut field, &world, 1.0 / 60.0);
    }
    let mut frames = 0;
    while state.wheels[0].slip < 5.0 {
        step_cars(&mut [Racer { car: &car, state: &mut state, input }], &mut field, &world, 1.0 / 60.0);
        frames += 1;
        assert!(frames < 120, "no wheelspin");
    }
    let step = |mut s: CarState, snap: bool| {
        if snap {
            s.snapshot_take();
        }
        step_cars(&mut [Racer { car: &car, state: &mut s, input }], &mut CarField::default(), &world, 1.0 / 60.0);
        s.wheels[0].spin
    };
    let (plain, snap) = (step(state.clone(), false), step(state.clone(), true));
    let share = snap / plain;
    eprintln!("snapshot spin share {share:.3} ({snap:.2} / {plain:.2})");
    assert!((0.4..=0.7).contains(&share), "the snapshot frame's spin {snap} against {plain} (Wine: 0.54-0.55)");
}

/// Two bodies overlapping for the whole frame are pushed apart (0x5B4300)
/// along the line between their positions at the moment (vt+0x34 →
/// 0x593290 from vt+0x08 = 0x593B50: car +0x60, the centre of mass), not
/// between the car origins. Two cars stacked at the same x and y, the top
/// one turned round: their origins are one above the other, their centres
/// of mass are not (the centre of mass sits off the origin along the car's
/// forward axis), so the push has a sideways part along x.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn stuck_cars_are_pushed_apart_between_their_centres_of_mass() {
    let d = data();
    let car = car(&d, &[], 9, 0.0);
    let c = Vec3::from(car.car_type.handling.centre_of_mass);
    assert!(c.x.abs() > 0.05, "the centre of mass is off the origin along forward: {c:?}");
    let w = car.car_type.wheels[0];
    let z = w.radius - w.position.z;
    let mut a = CarState::new(Vec3::new(0.0, 0.0, z), 0.0);
    let mut b = CarState::new(Vec3::new(0.0, 0.0, z + 0.6), std::f32::consts::PI);
    let world = nfsu2::world::World::flat(2000.0);
    let mut field = CarField::default();
    let input = Input::Ai(AiControls { steer_degrees: 0.0, gas: 0.0, brake: 1.0, handbrake: 0.0, gear: 1, nitrous: false, mover_held: false });
    let (ax, bx) = (a.position.x, b.position.x);
    step_cars(&mut [Racer { car: &car, state: &mut a, input }, Racer { car: &car, state: &mut b, input }], &mut field, &world, 1.0 / 60.0);
    let (da, db) = (a.position.x - ax, b.position.x - bx);
    eprintln!("centre of mass {c:?}: A moved {da:.4} m along x, B {db:.4} m");
    // B's centre of mass is at −c.x, A's at +c.x: B goes towards −c.x.
    assert!((db - da) * c.x.signum() < -0.01, "the push runs between the centres of mass: A {da}, B {db}");
}

/// The tuning works on the block with the performance uniques taken off
/// and puts them back after (0x5B4790: 0x599DA0 / 0x5997F0), so neutral
/// sliders leave a unique's bonus in place. Read under Wine (save
/// community_perf_after_buys.sav, free roam, tuning block at the engine data
/// − 0x1F0): the LANCEREVO8 with these parts, uniques 0–8 and every
/// slider at 100 has tyre grip / sliding 1.9 / 1.65 front, 1.95 / 1.7
/// rear, the untuned values; the clone's tuning had clamped the tyre
/// unique's +0.05 off (1.85 / 1.6, 1.9 / 1.65).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn neutral_tuning_keeps_the_uniques_bonus() {
    let d = data();
    let parts: [u8; 51] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x11, 0x13, 0x14, 0x15, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x23, 0x24, 0x25, 0x26, 0x27,
        0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x3a, 0x3d, 0x40, 0x43,
    ];
    let fitted = |id: u8| parts.contains(&id) || (nfsu2::performance::UNIQUE_BASE..nfsu2::performance::UNIQUE_BASE + 9).contains(&id);
    let tuned = Car::load_physics(&d.root, "LANCEREVO8", &d.packages, &fitted, Some((&d.table, &Settings::default()))).unwrap();
    let plain = Car::load_physics(&d.root, "LANCEREVO8", &d.packages, &fitted, None).unwrap();
    let t = |c: &Car| {
        let h = &c.car_type.handling;
        [h.tyres[0].grip, h.tyres[0].sliding, h.tyres[1].grip, h.tyres[1].sliding]
    };
    assert_eq!(t(&tuned), t(&plain));
    for (a, b) in t(&tuned).iter().zip([1.9f32, 1.65, 1.95, 1.7]) {
        assert!((a - b).abs() < 1e-5, "{:?}", t(&tuned));
    }
}

/// The parts sweep: the original's tuning blocks for cars from
/// stock to maxed (tests/fixtures/native_blocks: the 350Z of the career
/// start, a community save's LANCEREVO8 with every unique, an outrun 240SX with 39
/// parts, circuit rivals) against the clone's upgraded record from the
/// same fitted parts and uniques, word for word (`performance::upgrade`,
/// 0x5AC500 + 0x5997F0). Over the harness's 144 recorded cars and six
/// saves' players the sweep (`devprobes/src/bin/block_sweep`) found no difference.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn upgraded_blocks_match_the_originals() {
    let d = data();
    let globalb = std::fs::read(d.root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let dir = common::private_root().join("tests/fixtures/native_blocks");
    let f = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "bin") {
            continue;
        }
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        let name = stem.split_once('-').unwrap().1.to_string();
        let native = std::fs::read(&p).unwrap();
        let state = &native[0x3C0..0x3C0 + 0x94];
        let fitted = |id: u8| {
            if id >= nfsu2::performance::UNIQUE_BASE {
                let i = (id - nfsu2::performance::UNIQUE_BASE) as usize;
                i < 10 && u32::from_le_bytes(state[0x28 + 4 * i..0x2C + 4 * i].try_into().unwrap()) != 0
            } else {
                id < 0x44 && state[0x50 + id as usize] != 0
            }
        };
        let clone = nfsu2::car::tuned_record(&globalb, &name, &d.packages, &fitted).unwrap();
        for o in (0x110..0x110 + 0x2B0).step_by(4) {
            let (a, b) = (f(&native, o), f(&clone, o));
            assert!((a - b).abs() <= 1e-5 * a.abs().max(1.0), "{stem}: record +{o:#x} native {a} clone {b}");
        }
        n += 1;
    }
    assert!(n >= 5, "{n} fixtures");
}
