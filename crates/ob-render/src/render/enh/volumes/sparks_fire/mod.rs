//! Sparks & Fire, Next-Gen tier (clone deviation, opt-in;
//! docs/notes/graphics/sparks-fire.md).
//!
//! The native emitters still decide **when, where and how many**: the
//! contact bursts (0x5A9630 → 0x5DC4D0, `particles::post_contact`), the
//! nitrous flame (0x5F5F0B → 0x61FB10(10)) and the turbo's flame pops
//! (`carfx_pops`). Every particle they spawn is still spawned and aged by
//! the native system (`particles::System`); this tier only changes how the
//! spark and flame descriptions look:
//! * **Sparks** (descriptions named `*Sprks*`): each native birth becomes
//!   a handful of hot fragments thrown with its velocity, pulled by gravity
//!   and air drag, bouncing off the road under the contact (restitution
//!   and friction), cooling along a black-body ramp (white → yellow →
//!   orange → dull red), drawn as motion streaks (their own velocity over
//!   a shutter), emissive into the HDR scene so the glow passes bloom them.
//! * **Flames** (`*NOS_Flame*`): the native flame particles themselves
//!   (same life, motion, owner) drawn as a procedural fire — a teardrop
//!   stretched along the particle's motion, licked by scrolling noise, blue
//!   core at the exhaust through white to an orange, smoky tip — plus a
//!   **heat haze** that refracts the scene behind and around it.
//! * **Light**: the flames of each car and each cluster of hot sparks are
//!   short-lived point lights applied to the scene (road, walls, cars)
//!   through the depth buffer (a deferred splat: position from depth,
//!   normal from its derivatives, Lambert × the surface's own colour).
//!
//! Original: nothing here runs, nothing is recorded; the native quads are
//! drawn as before (byte-identical).

use glam::{Mat4, Vec3, Vec4};

use crate::particles::{Description, Library, Particle};

use crate::render::Gpu;

mod sim;
pub use sim::*;
mod instances;
pub use instances::*;
mod gpu;
pub use gpu::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::{Emitter, System};

    fn library() -> Library {
        let mut lib = Library::default();
        let spark = Description {
            name: "XECS_SolidWall_SprksA".into(),
            life: [0.5, 0.0],
            rate: [100.0, 0.0],
            speed: [2.0, 0.8],
            range: 1000.0,
            ..Default::default()
        };
        let flame = Description {
            name: "XES_NOS_FlameA".into(),
            life: [0.2, 0.8],
            rate: [8.0, 0.0],
            speed: [6.0, 0.6],
            range: 1000.0,
            ..Default::default()
        };
        let smoke = Description {
            name: "XECS_SolidWall_SmokeA".into(),
            life: [1.0, 0.0],
            rate: [10.0, 0.0],
            range: 1000.0,
            ..Default::default()
        };
        lib.descriptions = vec![spark, flame, smoke];
        lib
    }

    #[test]
    fn kinds_by_name() {
        let lib = library();
        assert_eq!(kind_of(&lib.descriptions[0]), Some(Kind::Spark));
        assert_eq!(kind_of(&lib.descriptions[1]), Some(Kind::Flame));
        assert_eq!(kind_of(&lib.descriptions[2]), None);
    }

    #[test]
    fn sparks_fall_bounce_and_cool() {
        let lib = library();
        let mut system = System { births: Some(Vec::new()), ..Default::default() };
        let mut e = Emitter::new(0, Mat4::from_translation(Vec3::new(0.0, 0.0, 0.3)), 7);
        e.velocity = Vec3::new(20.0, 0.0, 0.0);
        system.emit(&lib, &mut e, -0.2);
        let births = system.births.take().unwrap();
        assert!(!births.is_empty());
        let mut sim = Sim::default();
        // A hard contact: about FRAGMENTS_MAX a birth (within MAX_NEW).
        let hard = [Contact { births: 0..births.len(), slide: 30.0, impulse: 20.0, normal: Vec3::Y, two_sided: false }];
        sim.record(&lib, &births, &hard, &|_| Some(0.0));
        assert!(
            !sim.sparks.is_empty() && sim.sparks.len() <= MAX_NEW,
            "{} fragments of {} births",
            sim.sparks.len(),
            births.len()
        );
        let mut lowest = f32::MAX;
        let mut bounced = false;
        let start_t: f32 = sim.sparks.iter().map(|s| s.temperature()).sum();
        for _ in 0..30 {
            let before: Vec<f32> = sim.sparks.iter().map(|s| s.velocity.z).collect();
            sim.step(1.0 / 60.0);
            for (s, vz) in sim.sparks.iter().zip(before) {
                lowest = lowest.min(s.position.z);
                bounced |= vz < 0.0 && s.velocity.z > 0.0;
            }
        }
        assert!(lowest >= -1e-4, "never under the road: {lowest}");
        assert!(bounced, "some fragment bounced");
        let end_t: f32 = sim.sparks.iter().map(|s| s.temperature()).sum();
        assert!(end_t < start_t);
        // All gone by their longest life.
        for _ in 0..120 {
            sim.step(1.0 / 60.0);
        }
        assert!(sim.sparks.is_empty());
    }

    /// The drawn fragments are S_n at α = 1, S_{n−1} at α = 0
    /// (bit for bit), in between for the ones that live on; a fragment
    /// born in the tick waits for α = 1, one that died in it is drawn
    /// where it was.
    #[test]
    fn drawn_fragments_blend_the_last_two_ticks() {
        let lib = library();
        let mut sim = Sim::default();
        let mut system = System { births: Some(Vec::new()), ..Default::default() };
        let mut e = Emitter::new(0, Mat4::IDENTITY, 7);
        system.emit(&lib, &mut e, -1.0);
        let births = system.births.take().unwrap();
        sim.record(&lib, &births, &[], &|_| Some(0.0));
        sim.step(1.0 / 60.0);
        assert!(!sim.sparks.is_empty());
        let first = sim.sparks.len();
        // The next tick: no births; the fragments move.
        sim.record(&lib, &[], &[], &|_| Some(0.0));
        sim.step(1.0 / 60.0);
        let mut scratch = Vec::new();
        let at = |v: &[Spark]| v.iter().map(|s| (s.id, s.position.to_array().map(f32::to_bits))).collect::<Vec<_>>();
        assert_eq!(at(sim.drawn(1.0, &mut scratch)), at(&sim.sparks));
        assert_eq!(at(sim.drawn(0.0, &mut scratch)), at(&sim.prev));
        assert_eq!(sim.prev.len(), first);
        let mid = sim.drawn(0.5, &mut scratch).to_vec();
        for (m, (a, b)) in mid.iter().zip(sim.prev.iter().zip(&sim.sparks)) {
            assert_eq!((m.id, a.id), (b.id, b.id));
            assert!((m.position - (a.position + b.position) * 0.5).length() < 1e-4);
        }
        // A new fragment (only in S_n) is not drawn before α = 1.
        let mut births = Vec::new();
        let mut system = System { births: Some(Vec::new()), ..Default::default() };
        system.emit(&lib, &mut e, -1.0);
        births.extend(system.births.take().unwrap());
        sim.record(&lib, &births, &[], &|_| Some(0.0));
        sim.step(1.0 / 60.0);
        let born = sim.sparks.iter().filter(|s| !sim.prev.iter().any(|p| p.id == s.id)).count();
        assert!(born > 0);
        assert_eq!(sim.drawn(0.5, &mut scratch).len(), sim.prev.len());
    }

    #[test]
    fn smoke_and_flames_are_not_thrown() {
        let lib = library();
        let mut sim = Sim::default();
        let mut system = System { births: Some(Vec::new()), ..Default::default() };
        for d in [1, 2] {
            let mut e = Emitter::new(d, Mat4::IDENTITY, 3);
            system.emit(&lib, &mut e, -1.0);
        }
        let births = system.births.take().unwrap();
        assert!(!births.is_empty());
        sim.record(&lib, &births, &[], &|_| None);
        assert!(sim.sparks.is_empty());
    }

    #[test]
    fn flames_follow_the_native_particles_and_light() {
        let lib = library();
        let mut sim = Sim::default();
        let _ = sim.kind(&lib, 0);
        let mut system = System::default();
        let mut e = Emitter::new(1, Mat4::from_translation(Vec3::new(5.0, 0.0, 0.4)), 11);
        e.key = 42;
        system.emit(&lib, &mut e, -1.0);
        let mut out = Vec::new();
        flame_instances(&lib, &sim, &system.particles, &mut out);
        assert_eq!(out.len(), system.particles.len());
        assert!(
            out.iter()
                .all(|i| i.b[3] == 1.0 && (Vec3::from_slice(&i.a[..3]) - Vec3::new(5.0, 0.0, 0.4)).length() < 1e-3)
        );
        let l = lights(&lib, &sim, &sim.sparks, &system.particles);
        assert_eq!(l.len(), 1);
        assert!((Vec3::from_slice(&l[0].position[..3]) - Vec3::new(5.0, 0.0, 0.4)).length() < 1e-3);
    }

    /// Against too many sparks: a light touch makes
    /// a few fragments, a hard scrape more but never a shower; the
    /// fragments stay low and near the contact and die within LIFE_MAX.
    #[test]
    fn light_touches_make_no_shower() {
        assert!(intensity(2.0, 1.0) < 0.05, "a nudge");
        assert!(intensity(8.0, 2.0) < 0.15, "a light brush: {}", intensity(8.0, 2.0));
        assert!(intensity(30.0, 15.0) > 0.95, "a hard scrape");
        assert!(intensity(2.0, 30.0) > 0.4, "a hard hit with little slide");
        let lib = library();
        let births: Vec<Particle> = (0..40)
            .map(|i| {
                let mut p = Particle {
                    description: 0,
                    palette: 0,
                    key: 0,
                    life: 512,
                    angle: 0,
                    speed: 0,
                    carried: 0,
                    position: Vec3::new(i as f32 * 0.01, 0.0, 0.3),
                    direction: [0; 3],
                    frame: 0,
                    id: i,
                };
                p.set_velocity_for_test(Vec3::new(30.0, 0.0, 2.0));
                p
            })
            .collect();
        let count = |slide: f32, impulse: f32| {
            let mut sim = Sim::default();
            let c = [Contact { births: 0..births.len(), slide, impulse, normal: Vec3::Y, two_sided: false }];
            sim.record(&lib, &births, &c, &|_| Some(0.0));
            sim
        };
        let light = count(6.0, 1.0).sparks.len();
        let hard = count(30.0, 20.0);
        assert!(light <= 14, "light touch: {light} fragments of 40 births");
        assert!(hard.sparks.len() >= 30 && hard.sparks.len() <= MAX_NEW, "hard: {}", hard.sparks.len());
        // Thrown with the contact's motion, no fountain: the scatter off
        // the wall and up stays within a few m/s.
        for s in &hard.sparks {
            assert!(s.velocity.z < 2.0 * 0.95 + 1.8 + 1.2 + 0.01, "lift {}", s.velocity.z);
            assert!(s.velocity.y > -1.0 && s.velocity.y < 1.5 + 2.4 + 0.01, "off the wall {}", s.velocity.y);
            assert!(s.velocity.x > 12.0, "with the car {}", s.velocity.x);
            assert!(s.life <= LIFE_MAX * 1.2 + 1e-4);
        }
        let mut sim = hard;
        let mut top: f32 = 0.0;
        for _ in 0..60 {
            sim.step(1.0 / 60.0);
            top = top.max(sim.sparks.iter().map(|s| s.position.z).fold(0.0, f32::max));
        }
        assert!(top < 0.3 + 1.3, "stays low: {top}");
        assert!(sim.sparks.is_empty(), "gone within {} s", LIFE_MAX * 1.2);
    }

    /// The contact glow replaces the native sprite: soft, small, fading
    /// with the native alpha; the sparks' light is a small pool.
    #[test]
    fn contact_glow_and_its_light() {
        let mut lib = library();
        let mut glow =
            Description { name: "XECS_SolidWall_GlowA".into(), life: [0.3, 0.0], range: 1000.0, ..Default::default() };
        glow.size.0[3] = [1.2, 0.0, 0.0, 0.0];
        glow.color.0[3] = [200.0, 200.0, 200.0, 250.0];
        lib.descriptions.push(glow);
        let mut sim = Sim::default();
        assert_eq!(sim.kind(&lib, 3), Some(Kind::Glow));
        let mut system = System::default();
        let mut e = Emitter::new(3, Mat4::from_translation(Vec3::new(2.0, 0.0, 0.5)), 5);
        system.emit(&lib, &mut e, -1.0);
        assert!(!system.particles.is_empty());
        let mut out = Vec::new();
        glow_instances(&lib, &sim, &system.particles, &mut out);
        assert_eq!(out.len(), system.particles.len());
        assert!(out.iter().all(|i| i.b[3] == 2.0 && i.a[3] <= 0.5 && i.c[1] > 0.5));
        let l = lights(&lib, &sim, &sim.sparks, &system.particles);
        assert_eq!(l.len(), 1);
        assert!(l[0].position[3] <= 2.2 && l[0].colour[0] <= SPARK_LIGHT + 1e-6, "{:?}", l[0]);
    }

    #[test]
    fn ramps() {
        assert_eq!(falloff(0.0, 4.0), 1.0);
        assert_eq!(falloff(4.0, 4.0), 0.0);
        let hot = heat_colour(1.0);
        let cold = heat_colour(0.2);
        assert!(hot.min_element() > 1.5 && cold.x > cold.y && cold.z == 0.0);
    }
}
