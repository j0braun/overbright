//! Runs the vehicle model through a few manoeuvres in Bayview and prints
//! what happens: settling at rest, full throttle, braking, a turn.
//!
//!     cargo run --release --example drive_probe -- 240SX

use glam::Vec3;
use nfsu2::drive::{CarState, Controls, STEP};

fn report(label: &str, t: f32, s: &CarState) {
    let w = &s.wheels;
    println!(
        "{label:9} t {t:5.2}  {:6.1} km/h  gear {} {:5.0} rpm  z {:7.3}  pitch {:5.2}° roll {:5.2}°  yaw/s {:6.1}°  \
         slip {:6.1}° steer {:5.1}°{}  loads {:4.1} {:4.1} {:4.1} {:4.1}  comp {:5.3} {:5.3} {:5.3} {:5.3}  slide {}{}{}{}",
        s.speed() * 3.6,
        s.gear_name(),
        s.rpm(),
        s.position.z,
        (s.orientation * Vec3::X).z.asin().to_degrees(),
        (s.orientation * Vec3::Y).z.asin().to_degrees(),
        s.spin.z.to_degrees(),
        s.slip_degrees(),
        s.steer_degrees,
        if s.drifting { " drift" } else { "" },
        w[0].load,
        w[1].load,
        w[2].load,
        w[3].load,
        w[0].compression,
        w[1].compression,
        w[2].compression,
        w[3].compression,
        w[0].sliding as u8,
        w[1].sliding as u8,
        w[2].sliding as u8,
        w[3].sliding as u8,
    );
}

fn run(label: &str, s: &mut CarState, car: &nfsu2::car::Car, world: &nfsu2::world::World, c: Controls, secs: f32, every: f32) {
    let n = (secs / STEP) as u32;
    let k = (every / STEP) as u32;
    for i in 1..=n {
        s.step(car, world, c, STEP);
        if i % k == 0 {
            report(label, i as f32 * STEP, s);
        }
    }
}

fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/developing/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let root = nfsu2::game_root();
    let name = std::env::args().nth(1).unwrap_or_else(|| "240SX".into());
    let world = nfsu2::world::World::load(&root, "L4RA")?;
    let car = nfsu2::car::Car::load(&root, &name)?;
    let w = &car.car_type.wheels[0];
    let start = Vec3::new(361.0, -462.0, 30.0);
    let road = world.road_height_below(start).expect("road");
    let heading = (-90.0f32 - 23.0).to_radians();
    let mut s = CarState::new(Vec3::new(start.x, start.y, road - (w.position.z - w.radius)), heading);

    run("rest", &mut s, &car, &world, Controls::default(), 1.0, 0.5);
    run("throttle", &mut s, &car, &world, Controls { throttle: 1.0, ..Default::default() }, 4.0, 1.0);
    // A handbrake flick to the left, then counter-steer (right) on the throttle.
    let mut drift = s.clone();
    run("handbrake", &mut drift, &car, &world, Controls { handbrake: 1.0, steer: -1.0, ..Default::default() }, 0.4, 0.1);
    run("counter", &mut drift, &car, &world, Controls { throttle: 1.0, steer: 1.0, ..Default::default() }, 0.8, 0.1);
    run("right", &mut s, &car, &world, Controls { throttle: 0.5, steer: 1.0, ..Default::default() }, 1.5, 0.25);
    run("brake", &mut s, &car, &world, Controls { brake: 1.0, ..Default::default() }, 1.0, 0.5);
    Ok(())
}
