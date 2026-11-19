//! Diagnostic: drive at a wall (default: a 35° glancing one; `square`: head
//! on; `street`: down a street), hold the brake to reverse, then drive off.
use glam::Vec3;
use nfsu2::drive::{CarState, Controls, STEP};
fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/developing/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let root = nfsu2::game_root();
    let world = nfsu2::world::World::load(&root, "L4RA")?;
    let car = nfsu2::car::Car::load(&root, "240SX")?;
    let arg = std::env::args().nth(1).unwrap_or_default();
    let wall = arg != "street";
    let square = arg == "square";
    let start = if square { Vec3::new(355.0, -472.0, 30.0) } else if wall { Vec3::new(365.0, -470.0, 30.0) } else { Vec3::new(361.0, -462.0, 30.0) };
    let road = world.road_height_below(start).unwrap();
    let w = &car.car_type.wheels[0];
    let mut s = CarState::new(Vec3::new(start.x, start.y, road - (w.position.z - w.radius)), if square { (-28.6f32).to_radians() } else if wall { -std::f32::consts::FRAC_PI_2 } else { (-113.0f32).to_radians() });
    for (label, c, secs) in [
        ("throttle", Controls { throttle: 1.0, ..Default::default() }, 6.0),
        ("brake", Controls { brake: 1.0, ..Default::default() }, 8.0),
        ("forward", Controls { throttle: 1.0, ..Default::default() }, 4.0),
    ] {
        let n = (secs / STEP) as u32;
        for i in 1..=n {
            let before = s.position + Vec3::Z * 0.6;
            s.step(&car, &world, c, STEP);
            if let Some((f, nrm)) = world.wall_hit(before, s.position + Vec3::Z * 0.6) {
                println!("  crossed a wall at {:.2} n {nrm:.2} ({f:.2})", before);
            }
            if i % 30 == 0 {
                let d = s.drivetrain.as_ref().unwrap();
                println!("{label} t {:.1} v {:.2} gear {} target {} shift {:?} clutch {:.2} rpm {:.0} at {:.2} heading {:.0}", i as f32 * STEP, s.speed(), d.gear, d.target_gear, d.shift, d.clutch, d.rpm(), s.position, s.heading().to_degrees());
            }
        }
    }
    Ok(())
}
