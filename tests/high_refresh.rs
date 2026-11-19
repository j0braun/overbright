//! The simulation at any display rate (docs/original/uncapped-fps.md).
//!
//! The original does not run at high frame rates, so the game must behave
//! as at 30 or 60 fps whatever the display rate. The world therefore
//! advances on a fixed cadence (`nfsu2::frame_clock::Cadence`, used by
//! `Game::redraw`): one update per 1/60 s of real time, each the original's
//! 60 fps frame (0x57EAE0: 30, 29, 31 ticks of 1/1800 s, split by the
//! mover 0x5B72E0 into ⌊dt / 4.4 ms⌋ + 1 sub-steps), input read once per
//! update; frames drawn in between only interpolate. So at 120, 144, 240
//! and 500 Hz the world state is bit for bit the 60 Hz one — including the
//! at-rest clutch cycle that decides a reverse start (always state 0 at
//! 60 fps in the original: −3.16 m/s 1 s after the press) and the race
//! clock (the sum of the updates' dts).

mod common;

use glam::Vec3;
use nfsu2::drive::{CarState, Controls};
use nfsu2::frame_clock::Cadence;

/// The display rates every run is compared at (30 and 200 Hz
/// added; the first is the reference), and a jitter schedule
/// ([`JITTER`]).
const DISPLAYS: [f32; 7] = [60.0, 30.0, 120.0, 144.0, 200.0, 240.0, 500.0];

/// An uneven display: frames of 4–40 ms (xorshift, fixed seed) and a
/// 120 ms stall every 300 frames (the rate gate's `jitter:<seed>` shape),
/// as 0.0 in [`DISPLAYS`]' place.
const JITTER: f32 = 0.0;

/// The `k`th frame's interval of display `hz` (s); [`JITTER`]: the
/// uneven schedule.
fn interval(hz: f32, k: usize) -> f32 {
    if hz != JITTER {
        return 1.0 / hz;
    }
    if k % 300 == 299 {
        return 0.120;
    }
    let mut x = (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    0.004 + (x % 3601) as f32 * 1e-5
}

fn on_road(world: &nfsu2::world::World, car: &nfsu2::car::Car, start: Vec3, heading: f32) -> CarState {
    let w = car.car_type.wheels[0];
    let road = world.road_height_below(start).unwrap();
    CarState::new(Vec3::new(start.x, start.y, road - (w.position.z - w.radius)), heading)
}

/// `updates` world updates on a `hz` display: the car, the world time (the
/// race clock's sum) and the updates' count. `controls(n)` is what the
/// input read gives at update n.
fn run(world: &nfsu2::world::World, car: &nfsu2::car::Car, mut state: CarState, hz: f32, updates: usize, controls: &dyn Fn(usize) -> Controls) -> (CarState, f32) {
    let mut cadence = Cadence::default();
    let (mut n, mut clock, mut k) = (0, 0.0f32, 0);
    while n < updates {
        let (dts, _) = cadence.frame(interval(hz, k));
        k += 1;
        for dt in dts {
            if n == updates {
                break;
            }
            state.step(car, world, controls(n), dt);
            clock += dt;
            n += 1;
        }
    }
    (state, clock)
}

fn same_as_60(name: &str, world: &nfsu2::world::World, car: &nfsu2::car::Car, start: CarState, updates: usize, controls: &dyn Fn(usize) -> Controls) -> CarState {
    let (s60, c60) = run(world, car, start.clone(), 60.0, updates, controls);
    let want = format!("{s60:?}");
    for &hz in DISPLAYS[1..].iter().chain([&JITTER]) {
        let (s, c) = run(world, car, start.clone(), hz, updates, controls);
        let at = if hz == JITTER { "jitter".to_string() } else { format!("{hz} Hz") };
        assert_eq!(c.to_bits(), c60.to_bits(), "{name}: world clock at {at}");
        assert!(format!("{s:?}") == want, "{name}: world state at {at} differs from 60 Hz (speed {} vs {})", s.speed(), s60.speed());
    }
    println!("{name}: identical at {DISPLAYS:?} Hz and jitter; speed {:.3} m/s, world time {c60:.4} s", s60.speed());
    s60
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn driving_is_the_60_fps_drive_at_every_display_rate() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "240SX").unwrap();
    let start = on_road(&world, &car, Vec3::new(361.0, -462.0, 30.0), (-113.0f32).to_radians());
    same_as_60("full throttle 4 s", &world, &car, start.clone(), 240, &|_| Controls { throttle: 1.0, ..Default::default() });
    same_as_60("throttle and a turn 3 s", &world, &car, start.clone(), 180, &|_| Controls { throttle: 0.6, steer: 0.4, ..Default::default() });
    same_as_60("launch 3 s, brake 3 s", &world, &car, start, 360, &|n| if n < 180 { Controls { throttle: 1.0, ..Default::default() } } else { Controls { brake: 1.0, ..Default::default() } });
}

/// Rachel's 350Z (DDAY_PLAYER_CAR) where the original was measured.
fn rachel(root: &std::path::Path) -> nfsu2::car::Car {
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let raw = nfsu2::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc")).unwrap()).unwrap();
    let career = nfsu2::formats::bundle::chunks(&raw, 0).find(|c| c.id == 0x8003_4A10).unwrap();
    let packages = nfsu2::performance::parse(career.child(0x34A15).unwrap().body);
    let preset = nfsu2::car::parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
    let perf = preset.perf(&packages);
    nfsu2::car::Car::load_upgraded(root, &preset.car, &preset.installed(), &packages, &|id| perf.contains(&id)).unwrap()
}

/// Reverse from rest: 3 s settled, then the brake held 1 s, at every
/// display rate the 60 fps result; and as in the original at 60 fps the
/// gearbox rests in state 0 (clutch open), so the press reverses at once
/// (original −3.159 / −3.130 m/s after 1 s; the maintainer's private captures).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn reversing_from_rest_is_the_60_fps_start_at_every_display_rate() {
    let root = common::game_root();
    let root = std::path::PathBuf::from(root);
    let world = nfsu2::world::World::load(&root, "L4RA").unwrap();
    let car = rachel(&root);
    let start = on_road(&world, &car, Vec3::new(356.61, -478.40, 18.5), (-119.38f32).to_radians());
    for press in 180..186 {
        let s = same_as_60(&format!("reverse, press at update {press}"), &world, &car, start.clone(), press + 60,
            &|n| if n < press { Controls::default() } else { Controls { brake: 1.0, ..Default::default() } });
        assert!((s.speed() + 3.15).abs() < 0.05, "press at {press}: {} m/s, original at 60 fps −3.13..−3.16", s.speed());
    }
}
