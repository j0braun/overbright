use glam::Vec3;

use super::lights::CarExtra;
use crate::render::enh::materials::world::puddle::{level, puddle_at, smoothstep};
use crate::render::enh::weather::spray::CarSample;

// ---------------------------------------------------------------------
// Spray (weather_fx's pool).

/// What the spray needs of a car beyond [`CarSample`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SprayCar {
    /// |slip| per wheel (m/s, along and across combined), lateral slip.
    pub slip: [f32; 4],
    pub lateral: [f32; 4],
    pub down: [bool; 4],
    /// The puddle under each wheel (0..1).
    pub puddle: [f32; 4],
    /// The body's water film (`Weather::car_wet`).
    pub film: f32,
    /// The rain amount now (weather +0x1F0).
    pub rain: f32,
}


impl SprayCar {
    pub fn of(car: &CarSample, extra: Option<&CarExtra>, rain: f32, road_wet: f32, film: f32) -> SprayCar {
        let lv = level(rain, road_wet);
        let mut s = SprayCar { film, rain, down: [true; 4], ..Default::default() };
        for i in 0..4 {
            s.puddle[i] = puddle_at(car.contacts[i].truncate(), lv);
            if let Some(e) = extra {
                let (along, across, down) = e.wheels[i];
                s.slip[i] = (along * along + across * across).sqrt();
                s.lateral[i] = across;
                s.down[i] = down;
            }
        }
        s
    }
}

/// The new spray this row adds for one car over `dt`, through `push(rand,
/// particle)`. Rates are per second; **inferred**, by eye. `tier` 1 / 2.
pub fn emit(
    car: &CarSample,
    fx: &SprayCar,
    road_wet: f32,
    tier: u8,
    dt: f32,
    rand: &mut dyn FnMut() -> f32,
    push: &mut dyn FnMut(SprayParticle),
) {
    let v = car.velocity;
    let speed = v.length();
    // The spray follows the standing water: the one wetness's film
    // (`Wetness::film`: full in the shower, a third once the rain
    // stops: a damp film throws a thin mist, not a rooster tail; as the
    // references FH5 and GT7 throw little spray once the rain stops).
    let w = road_wet.clamp(0.0, 1.0);
    if w <= 0.0 || speed < 1.0 {
        return;
    }
    let m = if tier >= 2 { 1.6 } else { 1.0 };
    let fwd = car.world.x_axis.truncate().normalize_or(Vec3::X);
    let back = -v / speed;
    let side = Vec3::new(-back.y, back.x, 0.0).normalize_or(Vec3::Y);
    let t = smoothstep(4.0, 30.0, speed);
    let count = |rate: f32, r: f32| (rate * dt + r) as u32;
    for i in 0..4 {
        if !fx.down[i] {
            continue;
        }
        let c = car.contacts[i];
        let rear = i >= 2;
        let slip = (fx.slip[i] / 6.0).clamp(0.0, 1.5);
        // Sideways with the slide (the patch sweeps across the water).
        let lat = side * (fx.lateral[i] / 8.0).clamp(-1.0, 1.0);
        // Rooster tail: the rear wheels lift a plume with speed and slip.
        let rooster = if rear { 40.0 } else { 14.0 } * w * m * (t * t + 0.8 * slip * smoothstep(1.0, 6.0, speed));
        for _ in 0..count(rooster, rand()) {
            let (a, b, c2, d) = (rand(), rand(), rand(), rand());
            let lift = (1.5 + 3.0 * b) * (0.4 + t + 0.4 * slip);
            push(SprayParticle {
                pos: c + back * (0.3 + 0.2 * a) + side * (c2 - 0.5) * 0.25 + Vec3::Z * 0.05,
                vel: v * 0.65
                    + back * (1.5 + 3.0 * a) * (0.5 + t)
                    + Vec3::Z * lift
                    + lat * 3.0
                    + side * (d - 0.5) * 1.8,
                ground: c.z,
                life: 0.45 + 0.45 * c2,
                size: (0.025, 0.045 + 0.025 * a),
                mist: false,
                alpha: 0.55,
                cloud: false,
            });
        }
        // (smaller and fewer; the volume's rooster tail carries the
        // body of the spray, this is its fine mist.)
        let plume = if rear { 5.0 } else { 2.0 } * w * m * (t + 0.6 * slip);
        for _ in 0..count(plume, rand()) {
            let (a, b, c2) = (rand(), rand(), rand());
            push(SprayParticle {
                pos: c + back * (0.5 + 0.4 * a) + Vec3::Z * 0.25,
                vel: v * 0.5 + Vec3::Z * (0.8 + 1.2 * b) * (0.5 + t) + lat * 2.0 + side * (c2 - 0.5) * 1.2,
                ground: c.z,
                life: 0.8 + 0.6 * a,
                size: (0.3, 0.8 + 0.4 * b),
                mist: true,
                alpha: 0.14,
                cloud: false,
            });
        }
        // (the volume's rooster tail grows from 3 m/s, full at 25.)
        let tv = smoothstep(3.0, 25.0, speed);
        // The rooster tail as a volume (wet_spray): puffs thrown up
        // and back off the contact patch on ballistic arcs, slowed by the
        // air the car drags along, fanned out sideways by a slide.
        let rooster_v = if rear { 46.0 } else { 16.0 } * w * m * (tv + 0.9 * slip * smoothstep(1.0, 6.0, speed));
        for _ in 0..count(rooster_v, rand()) {
            let (a, b, c2, d) = (rand(), rand(), rand(), rand());
            let up = (1.2 + 2.8 * b) * (0.35 + tv + 0.35 * slip);
            push(SprayParticle {
                pos: c + back * (0.35 + 0.25 * a) + side * (c2 - 0.5) * 0.3 + Vec3::Z * 0.12,
                vel: v * 0.6 + back * (1.5 + 3.5 * a) * (0.4 + tv) + Vec3::Z * up + lat * 3.5 + side * (d - 0.5) * 2.0,
                ground: c.z,
                // A thrown sheet, not a cloud bank: smaller, shorter
                // lived (it falls back onto the road), a little denser.
                life: 0.55 + 0.45 * c2,
                size: (0.16, 0.5 + 0.35 * a + 0.3 * tv),
                mist: false,
                alpha: 0.34,
                cloud: true,
            });
        }
        // The fan right behind the tyre: many short-lived puffs (the
        // tread's water torn off the contact patch), so the tail reads as
        // one continuous sheet near the wheel.
        // (half as many, denser puffs: they still overlap several
        // times over at speed, and the splat's cost went with the count.)
        let fan = if rear { 140.0 } else { 50.0 } * w * m * (tv + 0.7 * slip * smoothstep(1.0, 6.0, speed));
        for _ in 0..count(fan, rand()) {
            let (a, b, c2, d) = (rand(), rand(), rand(), rand());
            push(SprayParticle {
                pos: c + back * (0.25 + 0.2 * a) + side * (c2 - 0.5) * 0.28 + Vec3::Z * 0.08,
                vel: v * 0.75
                    + back * (1.0 + 2.0 * a) * (0.4 + tv)
                    + Vec3::Z * (0.8 + 1.8 * b) * (0.4 + tv + 0.3 * slip)
                    + lat * 3.0
                    + side * (d - 0.5) * 1.2,
                ground: c.z,
                life: 0.3 + 0.25 * c2,
                size: (0.1, 0.38 + 0.15 * a),
                mist: false,
                alpha: 0.42,
                cloud: true,
            });
        }
        // Through a puddle: the sheet thrown out to both sides as a volume.
        let sheet_v = 50.0 * fx.puddle[i] * w * m * smoothstep(2.0, 18.0, speed);
        for _ in 0..count(sheet_v, rand()) {
            let (a, b, c2, d) = (rand(), rand(), rand(), rand());
            let out = if d < 0.5 { -1.0 } else { 1.0 };
            let s = (speed / 20.0).clamp(0.3, 1.6);
            push(SprayParticle {
                pos: c + side * out * 0.2 + Vec3::Z * 0.1,
                vel: v * 0.7 + side * out * (1.5 + 3.0 * b) * s + Vec3::Z * (1.0 + 2.2 * c2) * s,
                ground: c.z,
                life: 0.5 + 0.4 * a,
                size: (0.1, 0.55 + 0.3 * b),
                mist: false,
                alpha: 0.25,
                cloud: true,
            });
        }
        // Wheel-spin and drifts: the slipping tyre churns a low cloud.
        let spin = if rear { 34.0 } else { 12.0 } * w * m * smoothstep(2.0, 8.0, fx.slip[i]);
        for _ in 0..count(spin, rand()) {
            let (a, b, c2) = (rand(), rand(), rand());
            push(SprayParticle {
                pos: c + back * 0.3 + side * (a - 0.5) * 0.4 + Vec3::Z * 0.1,
                vel: v * 0.35 + lat * 2.5 + side * (b - 0.5) * 1.5 + Vec3::Z * (0.5 + 1.0 * c2),
                ground: c.z,
                life: 1.0 + 0.8 * a,
                size: (0.25, 1.0 + 0.6 * b),
                mist: false,
                alpha: 0.14,
                cloud: true,
            });
        }
        // Through a puddle: a sheet of water thrown out to both sides.
        let splash = 140.0 * fx.puddle[i] * w * m * smoothstep(2.0, 18.0, speed);
        for _ in 0..count(splash, rand()) {
            let (a, b, c2, d) = (rand(), rand(), rand(), rand());
            let out = if d < 0.5 { -1.0 } else { 1.0 };
            let s = (speed / 20.0).clamp(0.3, 1.6);
            push(SprayParticle {
                pos: c + side * out * 0.15 + back * 0.1 * a,
                vel: v * 0.75 + side * out * (1.5 + 3.5 * b) * s + Vec3::Z * (1.2 + 2.8 * c2) * s + back * a * s,
                ground: c.z,
                life: 0.4 + 0.4 * b,
                size: (0.025, 0.05),
                mist: false,
                alpha: 0.55,
                cloud: false,
            });
        }
        // Off the wheel arch: the tread's water flung up and back off the
        // top of the tyre.
        let arch = 22.0 * w * m * t;
        for _ in 0..count(arch, rand()) {
            let (a, b, c2) = (rand(), rand(), rand());
            push(SprayParticle {
                pos: c + Vec3::Z * (0.45 + 0.1 * a) - fwd * 0.15 + side * (b - 0.5) * 0.2,
                vel: v * 0.85 + back * (0.6 + 1.5 * c2) + Vec3::Z * (0.3 + 1.2 * a),
                ground: c.z,
                life: 0.3 + 0.3 * b,
                size: (0.015, 0.025),
                mist: false,
                alpha: 0.35,
                cloud: false,
            });
        }
    }
    // Wake mist: a fast car leaves a cloud in its slipstream.
    let wake = 9.0 * w * m * smoothstep(18.0, 45.0, speed);
    let mid = (car.contacts[2] + car.contacts[3]) * 0.5;
    for _ in 0..count(wake, rand()) {
        let (a, b, c2) = (rand(), rand(), rand());
        push(SprayParticle {
            pos: mid + back * (0.8 + 0.8 * a) + side * (b - 0.5) * 1.6 + Vec3::Z * 0.4,
            vel: v * 0.45 + Vec3::Z * (0.3 + 0.5 * c2),
            ground: mid.z,
            life: 1.2 + 0.8 * a,
            size: (0.5, 1.2 + 0.4 * b),
            mist: true,
            alpha: 0.06,
            cloud: false,
        });
    }
    // The mist cloud behind a fast car, filling its slipstream (the
    // puffs follow the car's wake, wet_spray / volume_fx::wake).
    let wake_v = 16.0 * w * m * smoothstep(12.0, 40.0, speed);
    for _ in 0..count(wake_v, rand()) {
        let (a, b, c2) = (rand(), rand(), rand());
        push(SprayParticle {
            pos: mid + back * (1.0 + 1.5 * a) + side * (b - 0.5) * 2.0 + Vec3::Z * 0.5,
            vel: v * 0.5 + Vec3::Z * (0.2 + 0.4 * c2),
            ground: mid.z,
            life: 1.2 + 0.8 * a,
            size: (0.5, 1.1 + 0.4 * b),
            mist: false,
            alpha: 0.06,
            cloud: true,
        });
    }
    // Water sheeting off the body: drops leave the roof's and the boot's
    // rear edge with speed while the film is thick.
    let sheet = 30.0 * m * smoothstep(0.15, 0.6, fx.film) * smoothstep(8.0, 30.0, speed);
    for _ in 0..count(sheet, rand()) {
        let (a, b, c2) = (rand(), rand(), rand());
        let local = Vec3::new(-2.0 - 0.2 * a, (b - 0.5) * 1.4, 0.95 + 0.35 * c2);
        push(SprayParticle {
            pos: car.world.transform_point3(local),
            vel: v * 0.8 + Vec3::Z * 0.4 + side * (b - 0.5),
            ground: mid.z,
            life: 0.35 + 0.3 * a,
            size: (0.012, 0.02),
            mist: false,
            alpha: 0.35,
            cloud: false,
        });
    }
}

/// A new spray particle (weather_fx's pool makes the rest).
#[derive(Clone, Copy, Debug)]
pub struct SprayParticle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub ground: f32,
    pub life: f32,
    pub size: (f32, f32),
    pub mist: bool,
    pub alpha: f32,
    /// A volume puff (`wet_spray`), moved by the cars' air.
    pub cloud: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    fn car(at: Vec3, speed: f32) -> CarSample {
        let world = Mat4::from_translation(at);
        CarSample {
            world,
            contacts: [
                Vec3::new(1.3, 0.8, 0.0),
                Vec3::new(1.3, -0.8, 0.0),
                Vec3::new(-1.3, 0.8, 0.0),
                Vec3::new(-1.3, -0.8, 0.0),
            ]
            .map(|c| c + at),
            velocity: Vec3::X * speed,
        }
    }

    /// No spray on a dry road or standing; rooster tails grow with speed
    /// and slip; puddles splash; a thick film sheets off at speed.
    #[test]
    fn spray_follows_speed_slip_and_puddles() {
        let mut seed = 1u32;
        let mut rand = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let mut count = |c: &CarSample, fx: &SprayCar, wet: f32| {
            let mut n = 0;
            for _ in 0..120 {
                emit(c, fx, wet, 1, 1.0 / 60.0, &mut rand, &mut |_| n += 1);
            }
            n
        };
        let fx = SprayCar { down: [true; 4], rain: 0.7, ..Default::default() };
        assert_eq!(count(&car(Vec3::ZERO, 30.0), &fx, 0.0), 0);
        // Once the rain has stopped, a wet road throws a thin mist
        // (a third of the shower's), not a rooster tail.
        let film = |rain: f32| crate::render::enh::materials::world::wetness::Wetness::of(rain, 1.0).film;
        let damp = SprayCar { rain: 0.0, ..fx };
        let (shower, after) =
            (count(&car(Vec3::ZERO, 30.0), &fx, film(0.7)), count(&car(Vec3::ZERO, 30.0), &damp, film(0.0)));
        assert!(after > 0 && after * 2 < shower, "{after} {shower}");
        assert_eq!(film(0.7), 1.0);
        assert!((film(0.0) - 0.35).abs() < 1e-6 && film(0.3) > 0.6);
        assert_eq!(count(&car(Vec3::ZERO, 0.5), &fx, 1.0), 0);
        let slow = count(&car(Vec3::ZERO, 10.0), &fx, 1.0);
        let fast = count(&car(Vec3::ZERO, 35.0), &fx, 1.0);
        assert!(fast > slow * 2, "{slow} {fast}");
        let sliding = SprayCar { slip: [0.0, 0.0, 8.0, 8.0], lateral: [0.0, 0.0, 7.0, 7.0], ..fx };
        assert!(count(&car(Vec3::ZERO, 10.0), &sliding, 1.0) > slow * 3 / 2);
        let puddle = SprayCar { puddle: [0.8; 4], ..fx };
        assert!(count(&car(Vec3::ZERO, 10.0), &puddle, 1.0) > slow * 2);
        let film = SprayCar { film: 0.8, ..fx };
        assert!(count(&car(Vec3::ZERO, 30.0), &film, 1.0) > count(&car(Vec3::ZERO, 30.0), &fx, 1.0));
        // A wheel in the air throws nothing.
        let air = SprayCar { down: [false; 4], ..fx };
        let mut n = 0;
        emit(&car(Vec3::ZERO, 30.0), &air, 1.0, 1, 1.0, &mut rand, &mut |p| {
            // Only the wake mist (its volume cloud) and the body sheeting.
            assert!((p.mist || p.cloud) && p.size.1 > 1.0 || p.pos.z > 0.9, "{p:?}");
            n += 1;
        });
        let _ = n;
    }
}
