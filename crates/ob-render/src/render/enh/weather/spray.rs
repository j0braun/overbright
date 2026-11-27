use glam::{Mat4, Vec3};

/// A drawn car at a tick (built by the tick, [`CarSample::of`]).
#[derive(Clone, Copy, Debug)]
pub struct CarSample {
    pub world: Mat4,
    /// The wheels' ground contact points (world).
    pub contacts: [Vec3; 4],
    /// m/s, world.
    pub velocity: Vec3,
}

impl CarSample {
    /// A car as the spray sees it (built on the tick from the
    /// car's matrix at the tick, its wheels' centres and its velocity,
    /// `CarRenderer::tick_wet`).
    pub fn of(world: Mat4, wheel_centres: &[Vec3; 4], velocity: Vec3) -> CarSample {
        let contacts = wheel_centres.map(|c| world.transform_point3(Vec3::new(c.x, c.y, 0.05)));
        CarSample { world, contacts, velocity }
    }
}

/// A tick's cars for the spray (`cars`), in drawing order: the same car
/// given twice (the mirror's) counts once, 64 at most.
pub fn add_car(cars: &mut Vec<CarSample>, car: CarSample) {
    let at = car.world.w_axis.truncate();
    if cars.iter().any(|c| c.world.w_axis.truncate().distance_squared(at) < 1e-4) {
        return;
    }
    if cars.len() < 64 {
        cars.push(car);
    }
}

/// A car's body as the spray sees it: a box in the car's frame from
/// its wheels (the axles ± the overhangs, the track + the flanks, from
/// the sills to the roof). Under the sills the spray may stay (it is
/// thrown out from under the car).
#[derive(Clone, Copy, Debug)]
pub struct Body {
    inv: Mat4,
    lo: Vec3,
    hi: Vec3,
}

/// The overhangs past the axles, the flanks past the wheels' contact
/// lines, the sill and roof heights over the contacts (m; **inferred**, a
/// typical NFSU2 car: the Skyline's, the 350Z's, the Corolla's boxes lie
/// within).
pub const BODY_OVERHANG: f32 = 1.05;
pub const BODY_FLANK: f32 = 0.22;
pub const BODY_SILL: f32 = 0.32;
pub const BODY_ROOF: f32 = 1.5;
/// A puff's radius may reach this far into the margin around the box
/// (m): the smallest radius kept next to the body.
pub const BODY_MIN_RADIUS: f32 = 0.08;

impl Body {
    pub fn of(c: &CarSample) -> Body {
        let inv = c.world.inverse();
        let l = c.contacts.map(|p| inv.transform_point3(p));
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in l {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let ground = (lo.z + hi.z) * 0.5;
        Body {
            inv,
            lo: Vec3::new(lo.x - BODY_OVERHANG, lo.y - BODY_FLANK, ground + BODY_SILL),
            hi: Vec3::new(hi.x + BODY_OVERHANG, hi.y + BODY_FLANK, ground + BODY_ROOF),
        }
    }

    /// The distance from world point `p` to the box (0 inside).
    pub fn distance(&self, p: Vec3) -> f32 {
        let q = self.inv.transform_point3(p);
        (self.lo - q).max(q - self.hi).max(Vec3::ZERO).length()
    }
}

/// A puff at `p` of radius `r` next to the cars' bodies: its radius
/// capped by the distance to the nearest body (a ball never reaches into
/// it), and how much of its opacity is kept (none with its centre inside a
/// body, full from 0.25 m out).
pub fn body_clearance(bodies: &[Body], p: Vec3, r: f32) -> (f32, f32) {
    let d = bodies.iter().map(|b| b.distance(p)).fold(f32::MAX, f32::min);
    if d == f32::MAX {
        return (r, 1.0);
    }
    let keep = (d / 0.25).clamp(0.0, 1.0);
    (r.min(d.max(BODY_MIN_RADIUS)), keep * keep * (3.0 - 2.0 * keep))
}

/// Spray particles per second per wheel (droplets, mist) on a road of
/// wetness `road_wet` at `speed` m/s: none below 4 m/s or on a dry road.
/// **Inferred**, by eye.
pub fn spray_rates(road_wet: f32, speed: f32) -> (f32, f32) {
    let t = ((speed - 4.0) / 21.0).clamp(0.0, 1.0);
    let w = road_wet.clamp(0.0, 1.0) * t * t * (3.0 - 2.0 * t);
    (36.0 * w, 8.0 * w)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Particle {
    pub(super) pos: Vec3,
    pub(super) vel: Vec3,
    ground: f32,
    pub(super) age: f32,
    pub(super) life: f32,
    pub(super) size: (f32, f32),
    pub(super) mist: bool,
    pub(super) seed: f32,
    pub(super) alpha: f32,
    /// A volume puff (wet_spray's volume, not a billboard).
    cloud: bool,
    /// The car (index in `step`'s cars) that threw it.
    src: u16,
}

/// The CPU spray pool, stepped at the 60 Hz cadence of the world clock.
#[derive(Default)]
pub struct Spray {
    pub(super) particles: Vec<Particle>,
    rng: u32,
    last_clock: Option<f32>,
    /// Wet Road Interaction (`wet_road::emit`): the cars' wheels, in
    /// the order of the cars given to `step` (empty: the row off), and its
    /// tier.
    pub interaction: Vec<super::wet_road::SprayCar>,
    pub tier: u8,
    /// The next slot to reuse once the pool is full.
    next: usize,
    /// Wet Road Interaction's volume ( `wet_spray`): the volume puffs
    /// (rooster tails, wake mist, splash sheets), their own pool.
    clouds: Vec<Particle>,
    next_cloud: usize,
}

/// The elements a job takes at once (`par::chunks_mut`).
const CHUNK: usize = 512;

/// The volume puffs' pool size.
pub const CLOUD_MAX: usize = 12000;

/// The spray pool's size.
pub const SPRAY_MAX: usize = 6000;

impl Spray {
    fn rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn len(&self) -> usize {
        self.particles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    /// A digest of the pool (the rate tests compare it at equal ticks).
    pub fn digest(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64 ^ u64::from(self.rng);
        for p in self.particles.iter().chain(&self.clouds) {
            for v in [p.pos.x, p.pos.y, p.pos.z, p.vel.x, p.vel.y, p.vel.z, p.age] {
                h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
            }
        }
        h
    }

    /// The live volume puffs.
    pub fn clouds(&self) -> usize {
        self.clouds.len()
    }

    fn push_cloud(&mut self, p: Particle) {
        if self.clouds.len() >= CLOUD_MAX {
            self.next_cloud %= CLOUD_MAX;
            self.clouds[self.next_cloud] = p;
            self.next_cloud += 1;
        } else {
            self.clouds.push(p);
        }
    }

    /// How much spray the eye at `eye` sits in (0..1), leaving out
    /// the car nearest to it (the chase camera's own car: its spray flies
    /// past the camera, not at it).
    pub fn hit(&self, cars: &[CarSample], eye: Vec3) -> f32 {
        let own = cars
            .iter()
            .enumerate()
            .min_by(|a, b| {
                a.1.world
                    .w_axis
                    .truncate()
                    .distance_squared(eye)
                    .total_cmp(&b.1.world.w_axis.truncate().distance_squared(eye))
            })
            .filter(|c| c.1.world.w_axis.truncate().distance(eye) < 12.0)
            .map(|c| c.0 as u16);
        // PERF-RAIN: `spray_volume::hit`'s sum over the puffs in order,
        // a puff out of its reach (its radius as `puff` caps it, + 2.5 m)
        // skipped before it is made (it adds nothing there): the same
        // value, no list of every puff a tick.
        let mut s = 0.0;
        for p in self.clouds.iter().filter(|p| Some(p.src) != own) {
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let r = p.size.0 + (p.size.1 - p.size.0) * t.sqrt();
            let reach = r.min(super::spray_volume::MAX_RADIUS) + super::spray_volume::HIT_REACH;
            if !(p.pos.distance_squared(eye) < reach * reach) {
                continue;
            }
            if let Some(q) = super::spray_volume::puff(p.pos, r, p.alpha, t, p.seed, true) {
                super::spray_volume::hit_add(&mut s, &q, eye);
            }
        }
        super::spray_volume::hit_total(s)
    }

    /// The volume's particles this frame (the puffs, and with the
    /// volume on the mist too), eye-independent world positions.
    /// Puffs near the eye fade out (`wet_spray::near_fade`: the chase
    /// camera's own car throws its spray past the lens).
    /// Against a glow shell around the car at speed: a
    /// puff keeps clear of every drawn car's body ([`body_clearance`]):
    /// its radius is capped by its distance to the body box and it fades
    /// out inside it, so the spray stays behind and below the wheels and
    /// nothing outlines the body (the depth test cut the balls that grew
    /// into the rear overhang down to a rim around the silhouette).
    pub fn volume_particles(&self, eye: Vec3, cars: &[CarSample]) -> Vec<crate::render::volume_fx::Particle> {
        let bodies: Vec<Body> =
            cars.iter().filter(|c| c.world.w_axis.truncate().distance(eye) < 120.0).map(Body::of).collect();
        let one = |p: &Particle| {
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let r = p.size.0 + (p.size.1 - p.size.0) * t.sqrt();
            let (r, keep) = body_clearance(&bodies, p.pos, r);
            super::spray_volume::puff(
                p.pos,
                r,
                p.alpha * keep * super::spray_volume::near_fade(p.pos.distance(eye) - r),
                t,
                p.seed,
                p.cloud,
            )
        };
        // PERF-RAIN: the puffs, then the mist, each made on its own on the
        // job pool (joined here) and kept in that order: the serial list.
        let mut made: Vec<Option<crate::render::volume_fx::Particle>> =
            vec![None; self.clouds.len() + self.particles.len()];
        let (clouds, particles) = (&self.clouds, &self.particles);
        let n = clouds.len();
        super::par::chunks_mut(&mut made, CHUNK, |start, part| {
            for (k, out) in part.iter_mut().enumerate() {
                let i = start + k;
                *out = if i < n { one(&clouds[i]) } else { Some(&particles[i - n]).filter(|p| p.mist).and_then(one) };
            }
        });
        made.into_iter().flatten().collect()
    }

    /// Add a particle; a full pool reuses its slots in turn (the oldest
    /// first, mostly).
    fn push(&mut self, p: Particle) {
        if self.particles.len() >= SPRAY_MAX {
            self.next %= SPRAY_MAX;
            self.particles[self.next] = p;
            self.next += 1;
        } else {
            self.particles.push(p);
        }
    }

    /// Step by the world clock's advance since the last call (none while
    /// paused; a jump of more than half a second restarts).
    pub fn advance(&mut self, cars: &[CarSample], road_wet: f32, clock: f32) {
        let dt = match self.last_clock {
            Some(c) if clock > c && clock - c < 0.5 => clock - c,
            Some(c) if clock == c => 0.0,
            _ => {
                self.particles.clear();
                self.clouds.clear();
                0.0
            }
        };
        self.last_clock = Some(clock);
        let steps = (dt * 60.0).round() as u32;
        for _ in 0..steps.min(30) {
            self.step(cars, road_wet, 1.0 / 60.0);
        }
    }

    /// One step: emit behind every wheel of every car on a wet road, then
    /// move: droplets fly and fall, mist slows to the still air and grows.
    pub fn step(&mut self, cars: &[CarSample], road_wet: f32, dt: f32) {
        for car in cars {
            let v = car.velocity;
            let speed = v.length();
            let (drops, mut mist) = spray_rates(road_wet, speed);
            // With Wet Road Interaction the volume's rooster tails
            // carry the spray's body; this row's mist joins it at half rate
            // (no fog bank of overlapping balls).
            if self.tier > 0 {
                mist *= 0.5;
            }
            if drops <= 0.0 {
                continue;
            }
            let back = -v / speed;
            let side = Vec3::new(-back.y, back.x, 0.0).normalize_or(Vec3::X);
            let s = (speed / 25.0).clamp(0.3, 1.5);
            for c in car.contacts {
                for (rate, is_mist) in [(drops, false), (mist, true)] {
                    let n = (rate * dt + self.rand()) as u32;
                    for _ in 0..n {
                        let (r0, r1, r2, r3) = (self.rand(), self.rand(), self.rand(), self.rand());
                        let pos = c + back * (0.25 + 0.3 * r0) + side * (r1 - 0.5) * 0.3;
                        let p = if is_mist {
                            Particle {
                                pos: pos + Vec3::Z * 0.1,
                                vel: v * 0.55 + Vec3::Z * (0.6 + 1.0 * r2) * s + side * (r3 - 0.5) * 1.6,
                                ground: c.z,
                                age: 0.0,
                                life: 0.9 + 0.9 * r0,
                                size: (0.2, 0.7 + 0.5 * r1),
                                mist: true,
                                seed: r2,
                                alpha: 0.09,
                                cloud: false,
                                src: 0,
                            }
                        } else {
                            Particle {
                                pos,
                                vel: v * 0.7
                                    + back * (1.0 + 2.0 * r0) * s
                                    + Vec3::Z * (1.0 + 2.5 * r2) * s
                                    + side * (r3 - 0.5) * 2.4 * s,
                                ground: c.z,
                                age: 0.0,
                                life: 0.35 + 0.35 * r1,
                                size: (0.018, 0.03),
                                mist: false,
                                seed: r3,
                                alpha: 0.4,
                                cloud: false,
                                src: 0,
                            }
                        };
                        self.push(p);
                    }
                }
            }
        }
        // Wet Road Interaction: rooster tails, puddle splashes, wake
        // mist, arch spray and body sheeting.
        if !self.interaction.is_empty() {
            let fx = std::mem::take(&mut self.interaction);
            for (k, (car, f)) in cars.iter().zip(&fx).enumerate() {
                let mut new = Vec::new();
                super::wet_road::emit(car, f, road_wet, self.tier, dt, &mut || self.rand(), &mut |p| new.push(p));
                for p in new {
                    let seed = self.rand();
                    let q = Particle {
                        pos: p.pos,
                        vel: p.vel,
                        ground: p.ground,
                        age: 0.0,
                        life: p.life,
                        size: p.size,
                        mist: p.mist,
                        seed,
                        alpha: p.alpha,
                        cloud: p.cloud,
                        src: k as u16,
                    };
                    if p.cloud {
                        self.push_cloud(q);
                    } else {
                        self.push(q);
                    }
                }
            }
            self.interaction = fx;
        }
        // The volume puffs (and with the row the mist) ride the air
        // the cars drag along (volume_fx's wake), slowed hard (fine spray),
        // a little gravity; they spread on the road instead of sinking.
        if self.tier > 0 {
            // Per car: its position, velocity and the wake's reach² (its
            // length behind thrice, `volume_fx::wake`, and a margin; 30 m
            // at most).
            let near: Vec<(Vec3, Vec3, f32)> = cars
                .iter()
                .map(|c| {
                    (
                        c.world.w_axis.truncate(),
                        c.velocity,
                        (4.0 + 3.0 * (2.5 + 0.35 * c.velocity.length())).min(30.0).powi(2),
                    )
                })
                .collect();
            let air = |pos: Vec3| -> Vec3 {
                let (mut sw, mut sv) = (0.0, Vec3::ZERO);
                for (at, cv, reach) in &near {
                    let r = pos - *at;
                    if r.length_squared() > *reach {
                        continue;
                    }
                    let (w, v) = crate::render::volume_fx::wake(r, *cv);
                    sw += w;
                    sv += v * w;
                }
                super::spray_volume::WIND * (1.0 - sw.min(1.0)) + sv / sw.max(1.0)
            };
            let ride = |p: &mut Particle| {
                p.age += dt;
                let a = air(p.pos);
                let drag = if p.cloud { super::spray_volume::CLOUD_DRAG } else { 2.5 };
                p.vel += (a - p.vel) * (drag * dt).min(1.0);
                p.vel.z -= if p.cloud { super::spray_volume::CLOUD_FALL * (1.0 - p.age / p.life) } else { -0.15 } * dt;
                p.pos += p.vel * dt;
                let floor = p.ground + 0.08;
                if p.pos.z < floor {
                    p.pos.z = floor;
                    p.vel.z = p.vel.z.max(0.0);
                }
            };
            // PERF-RAIN: each puff on its own (it reads the cars alone), so
            // the chunks on the job pool, joined here, give the serial
            // loop's values bit for bit (`par::chunks_mut`).
            super::par::chunks_mut(&mut self.clouds, CHUNK, |_, part| part.iter_mut().for_each(ride));
            super::par::chunks_mut(&mut self.particles, CHUNK, |_, part| {
                part.iter_mut().filter(|p| p.mist).for_each(ride)
            });
            self.clouds.retain(|p| p.age < p.life);
        }
        let interaction = self.tier > 0;
        for p in self.particles.iter_mut().filter(|p| !(interaction && p.mist)) {
            p.age += dt;
            if p.mist {
                p.vel -= p.vel * (2.5 * dt).min(1.0);
                p.vel.z += 0.15 * dt;
            } else {
                p.vel.z -= 9.8 * dt;
                p.vel -= p.vel * (0.8 * dt).min(1.0);
            }
            p.pos += p.vel * dt;
        }
        self.particles.retain(|p| p.age < p.life && (p.mist || p.pos.z > p.ground - 0.05));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(speed: f32) -> CarSample {
        let world = Mat4::IDENTITY;
        CarSample {
            world,
            contacts: [
                Vec3::new(1.3, 0.8, 0.0),
                Vec3::new(1.3, -0.8, 0.0),
                Vec3::new(-1.3, 0.8, 0.0),
                Vec3::new(-1.3, -0.8, 0.0),
            ],
            velocity: Vec3::X * speed,
        }
    }

    /// No spray on a dry road or standing still; spray behind a moving car
    /// on a wet road, and it settles and dies.
    #[test]
    fn spray_only_on_wet_roads_at_speed() {
        assert_eq!(spray_rates(0.0, 30.0), (0.0, 0.0));
        assert_eq!(spray_rates(1.0, 3.0), (0.0, 0.0));
        let mut s = Spray::default();
        for _ in 0..120 {
            s.step(&[car(30.0)], 0.0, 1.0 / 60.0);
            s.step(&[car(2.0)], 1.0, 1.0 / 60.0);
        }
        assert!(s.is_empty());
        for _ in 0..60 {
            s.step(&[car(30.0)], 1.0, 1.0 / 60.0);
        }
        assert!(s.len() > 60, "{}", s.len());
        // Behind the car: everything spawned behind its wheels (moving +x
        // at 30 m/s, the spray trails it).
        assert!(s.particles.iter().all(|p| p.pos.z > -0.2));
        for _ in 0..180 {
            s.step(&[], 1.0, 1.0 / 60.0);
        }
        assert!(s.is_empty());
    }

    /// Against a blueish smoke tint at max settings: at
    /// 217 km/h the volume's puffs never reach into the car's body box
    /// above the sills (no glow shell hugging the silhouette), the spray
    /// still flies behind and below it, and another car's spray is kept
    /// clear of that car too.
    #[test]
    fn spray_keeps_clear_of_the_body() {
        let mut s = Spray { tier: 2, ..Default::default() };
        let wheels = super::super::wet_road::SprayCar { down: [true; 4], rain: 0.8, ..Default::default() };
        let fast = car(60.0);
        let mut worst_before = 0.0f32;
        for _ in 0..120 {
            s.interaction = vec![wheels];
            s.step(&[fast], 1.0, 1.0 / 60.0);
        }
        let body = Body::of(&fast);
        // Without the clearance the balls reach into the body (the bug).
        for p in &s.clouds {
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let r = p.size.0 + (p.size.1 - p.size.0) * t.sqrt();
            worst_before = worst_before.max(r - body.distance(p.pos));
        }
        assert!(worst_before > 0.3, "the test sees the overlap it guards against: {worst_before}");
        let eye = Vec3::new(-6.0, 0.0, 1.6);
        let ps = s.volume_particles(eye, &[fast]);
        assert!(ps.len() > 100, "{}", ps.len());
        let mut behind = 0.0;
        for p in &ps {
            let c = Vec3::new(p.pos_r[0], p.pos_r[1], p.pos_r[2]);
            let d = body.distance(c);
            assert!(p.pos_r[3] <= d.max(BODY_MIN_RADIUS) + 1e-4, "a puff of {} m at {d} m from the body", p.pos_r[3]);
            if d <= 0.0 {
                assert_eq!(p.albedo_sigma[3], 0.0, "a puff inside the body is not drawn");
            }
            if d > 0.25 {
                behind += p.albedo_sigma[3] * p.pos_r[3];
            }
        }
        // (The test's car stands at the origin while its puffs take its
        // velocity: they leave the box ahead; in game it moves on.)
        assert!(behind > 0.0, "the spray clear of the body is still drawn");
        // A body box from the wheels: sills to roof, overhangs past the axles.
        assert!(body.distance(Vec3::new(0.0, 0.0, 0.8)) == 0.0 && body.distance(Vec3::new(0.0, 0.0, 0.1)) > 0.2);
        assert!(body.distance(Vec3::new(-1.3 - BODY_OVERHANG - 1.0, 0.0, 0.8)) > 0.99);
        // No car: unchanged.
        assert_eq!(body_clearance(&[], Vec3::ZERO, 0.7), (0.7, 1.0));
    }

    /// With Wet Road Interaction the wheels throw volume puffs; they
    /// stay on or over the road, the pool is bounded, the volume gets them
    /// (faded near the eye), and the eye in the car's own spray is not hit
    /// (a car ahead's is).
    #[test]
    fn interaction_clouds_for_the_volume() {
        let mut s = Spray { tier: 1, ..Default::default() };
        let wheels = super::super::wet_road::SprayCar { down: [true; 4], rain: 0.7, ..Default::default() };
        for _ in 0..90 {
            s.interaction = vec![wheels];
            s.step(&[car(30.0)], 1.0, 1.0 / 60.0);
        }
        assert!(s.clouds() > 200, "{}", s.clouds());
        assert!(s.clouds.iter().all(|p| p.cloud && p.pos.z >= p.ground + 0.079));
        let far = s.volume_particles(Vec3::new(0.0, 0.0, 200.0), &[]);
        let near = s.volume_particles(Vec3::new(-3.0, 0.0, 1.0), &[]);
        assert!(far.len() > 200 && near.len() <= far.len());
        let sum =
            |v: &[crate::render::volume_fx::Particle]| v.iter().map(|p| p.albedo_sigma[3] * p.pos_r[3]).sum::<f32>();
        assert!(sum(&near) < sum(&far), "faded near the eye");
        // The camera 4 m behind the car: its own spray; another car's
        // spray at the same spot hits.
        let eye = Vec3::new(-4.0, 0.0, 1.0);
        assert_eq!(s.hit(&[car(30.0)], eye), 0.0);
        let ahead = CarSample { world: Mat4::from_translation(Vec3::new(30.0, 0.0, 0.0)), ..car(30.0) };
        assert!(s.hit(&[ahead], eye) > 0.0 || s.hit(&[ahead], Vec3::new(2.0, 0.0, 0.6)) > 0.0);
        for _ in 0..400 {
            s.interaction = vec![wheels; 8];
            s.step(&[car(40.0); 8], 1.0, 1.0 / 60.0);
            assert!(s.clouds() <= CLOUD_MAX);
        }
        // The row off: none.
        let mut off = Spray::default();
        for _ in 0..60 {
            off.step(&[car(30.0)], 1.0, 1.0 / 60.0);
        }
        assert_eq!(off.clouds(), 0);
    }

    /// The pool is bounded and stepping follows the world clock (paused:
    /// nothing moves; a jump restarts).
    #[test]
    fn spray_follows_the_clock() {
        let mut s = Spray::default();
        s.advance(&[car(30.0)], 1.0, 10.0);
        assert!(s.is_empty());
        s.advance(&[car(30.0)], 1.0, 10.5 - 0.01);
        let n = s.len();
        assert!(n > 0);
        s.advance(&[car(30.0)], 1.0, 10.5 - 0.01);
        assert_eq!(s.len(), n);
        s.advance(&[car(30.0)], 1.0, 20.0);
        assert!(s.is_empty());
        for i in 0..2000 {
            s.step(&[car(40.0); 8], 1.0, 1.0 / 60.0);
            assert!(s.len() <= SPRAY_MAX, "{i}");
        }
    }
}
