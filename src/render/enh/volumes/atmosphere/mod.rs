//! The atmosphere (clone deviation, opt-in: every Volumetric Fog tier, `enh.volumetric_fog 1..3`, Low and High
//! a coarser grid with fewer lights than Next-Gen, [`tier`]; docs/atmosphere.md). SPEED2.EXE has no such pass: its fog is world.efx's per-vertex fog (FogValue /
//! FogColor, applied unchanged by the clone) and the horizon fog grid (0x5C7480). Everything here is an
//! **inferred** look (tuned by eye against the native palette), nothing was traced.
//!
//! A froxel medium (`atmosphere.wgsl`): height haze in the native fog colour that thickens in rain and
//! drifts with the wind, lit by the street lamps (the native weight² falloff, light shafts through their
//! Lamp Shadows layers), the neon and lit signs (Modern Lighting's emitters, which exist for every world),
//! spot lights (the player's headlight; **the Next-Gen car lights own the cones' look**: they fill [`NextGen::spots`]), and the
//! city's ambient; temporally accumulated; integrated front to back with the volumetric smoke (volume_fx)
//! hiding the haze behind it; applied to the scene before the post chain (exposure, grade, HDR as the rest).
//!
//! The narrow interface for the other effects: [`medium`] (the haze's density and height for a rain amount:
//! the rain haze and the car lights' cones read it), [`NextGen::spots`] (the car lights push their cones), [`NextGen::smoke`]
//! (any volume_fx medium).

use crate::render::Gpu;
use crate::render::camera::Camera;
use glam::{Mat4, Vec3};

mod look;
pub use look::*;
mod lights;
pub use lights::*;
mod pass;
pub use pass::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_validates() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/atmosphere.wgsl"));
        for src in [source.to_string(), ms_source(source)] {
            let module = wgpu::naga::front::wgsl::parse_str(&src).expect("parses");
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("valid");
            for e in ["cs_inject", "cs_integrate", "fs_apply"] {
                assert!(module.entry_points.iter().any(|x| x.name == e), "{e}");
            }
        }
        assert_ne!(ms_source(source), source, "the multisampled variant differs");
        assert_eq!(std::mem::size_of::<Uniform>() % 16, 0);
    }

    #[test]
    fn slices_round_trip_and_span_the_range() {
        assert!((slice_depth(0.0) - NEAR).abs() < 1e-5);
        assert!((slice_depth(1.0) - FAR).abs() < 1e-2);
        for z in [0.6f32, 3.0, 25.0, 140.0, 300.0] {
            assert!((slice_depth(depth_slice(z)) - z).abs() < 1e-3 * z, "{z}");
        }
        // Exponential: the near slices are thin (shafts close by stay sharp).
        let first = slice_depth(1.0 / DIMS[2] as f32) - NEAR;
        assert!(first < 0.1, "{first}");
    }

    #[test]
    fn rain_thickens_and_raises_the_haze() {
        let (d0, h0) = medium(0.0);
        let (d1, h1) = medium(1.0);
        assert!(d1 > d0 * 2.0 && h1 > h0);
        assert_eq!(medium(3.0), medium(1.0));
    }

    #[test]
    fn jitter_covers_the_slice() {
        let mut v: Vec<f32> = (1..=16).map(jitter).collect();
        assert!(v.iter().all(|x| (0.0..1.0).contains(x)));
        v.sort_by(f32::total_cmp);
        assert!(v.windows(2).all(|w| w[1] - w[0] < 0.2), "{v:?}");
    }

    #[test]
    fn nx6_cones_become_spots() {
        let c = crate::render::car_lights::GpuSpot {
            position: [1.0, 2.0, 0.5, 0.7],
            right: [0.0, -1.0, 0.0, 1.8],
            up: [0.0, 0.0, 1.0, 3.5],
            dir: [1.0, 0.0, 0.0, 0.0],
            colour: [0.9, 0.85, 0.7, 0.0],
        };
        let s = spots_from_car_lights(&[c, c], Vec3::new(100.0, 50.0, 10.0), Some(12), MAX_SPOTS);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].eye, Vec3::new(101.0, 52.0, 10.5));
        assert_eq!((s[0].across, s[0].down, s[0].layer, s[1].layer), (1.8, 3.5, 12, -1));
        assert_eq!(NX6_SPOT_SCALE, 1.0);
    }

    #[test]
    fn car_lamps_join_the_medium() {
        let lamps: Vec<(Vec3, Vec3, Vec3)> =
            (0..40).map(|i| (Vec3::new(i as f32, 0.0, 0.5), Vec3::new(0.5, 0.03, 0.01), -Vec3::X)).collect();
        let l = car_lamps(&lamps, CAR_LAMPS);
        assert_eq!(l.len(), CAR_LAMPS);
        assert!(l.iter().all(|x| x.kind == LightKind::Point && x.layer == -1 && x.axis == -Vec3::X));
        assert_eq!(l[0].radius, crate::render::car_lights::HALO);
        assert!(MAX_LIGHTS >= LAMPS + EMITTERS + CAR_LAMPS);
    }

    /// docs/atmosphere.md "The tiers": one medium at every fog tier, each cheaper tier a coarser grid,
    /// a shorter range and fewer lights; the tiers a preset sets (Low, High) keep no history, so their
    /// picture at a tick is the same at every display rate (the rate gate's Extreme frames).
    #[test]
    fn tiers_get_cheaper() {
        assert_eq!(tier(0), None);
        let (low, high, ng) = (tier(1).unwrap(), tier(2).unwrap(), tier(3).unwrap());
        assert_eq!(tier(9), Some(ng));
        let cells = |t: &Tier| t.dims.iter().map(|&d| d as u64).product::<u64>();
        for (a, b) in [(low, high), (high, ng)] {
            assert!(cells(&a) < cells(&b) && a.far <= b.far, "{a:?} {b:?}");
            assert!(a.lamps <= b.lamps && a.emitters <= b.emitters && a.car_lamps <= b.car_lamps && a.spots <= b.spots);
        }
        assert!(!low.history && !high.history && ng.history);
        assert!(!low.shafts && high.shafts && ng.shafts);
        assert!(ng.lamps <= LAMPS && ng.emitters <= EMITTERS && ng.car_lamps <= CAR_LAMPS && ng.spots <= MAX_SPOTS);
        assert!(ng.far <= FAR && ng.dims == DIMS);
    }

    /// A sign lights the half-space in front of it whatever its light's kind (Modern Lighting's emitters
    /// are kind 2), and its glow in the air keeps to a share of its light's reach.
    #[test]
    fn signs_glow_in_front_within_their_reach() {
        use crate::formats::lights::WorldLight;
        let sign = crate::render::lamps::Lamp {
            section: 0,
            light: WorldLight {
                kind: 2,
                sub: 2,
                on: true,
                colour: [200, 40, 160],
                position: Vec3::new(20.0, 0.0, 5.0),
                outer: 24.0,
                direction: Vec3::NEG_X,
                intensity: 1.0,
                inner: 0.0,
                cone: 0.0,
            },
        };
        let picked = pick_lights(&[], std::slice::from_ref(&sign), None, Vec3::ZERO, Vec3::X, &TIER_LOW);
        assert_eq!(picked.len(), 1);
        assert_eq!((picked[0].kind, picked[0].axis), (LightKind::Emitter, Vec3::NEG_X));
        let full = crate::render::lamps::gpu_lamp(&sign.light, -1).position[3];
        assert!(picked[0].radius <= full * 0.5 + 1e-4, "{} {full}", picked[0].radius);
    }

    #[test]
    fn look_overrides() {
        let l = look_from("haze=0.01, lamp=3,bogus=2,g=x");
        assert_eq!(l.haze, 0.01);
        assert_eq!(l.lamp, 3.0);
        assert_eq!(l.g, LOOK.g);
        assert_eq!(look_from("veil=0.5").veil, 0.5);
    }

    /// docs/look-direction.md rules "no double fog" / "count the glow twice": the medium never puts the
    /// native fog colour back on top of world.efx's fog, does not veil the
    /// scene a second time, and its halos stay inside the air-light budget
    /// measured at the look spots (the first look: +14 mean luma, p5 +7.7).
    #[test]
    fn look_keeps_the_native_fog_and_glow() {
        assert!(LOOK.ambient <= 0.1, "rule E1: no double fog (ambient {})", LOOK.ambient);
        // No veil at all (a black fog over the distance the lamps' light only paid back near).
        assert_eq!(LOOK.veil, 0.0, "rule E1: the native fog owns the attenuation");
        assert!(LOOK.lamp <= 50.0 && LOOK.emitter <= 15.0 && LOOK.knee <= 2.0, "rule E4: the halo budget");
        let wgsl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/atmosphere.wgsl"));
        assert!(wgsl.contains("mix(1.0, v.a, u.prev_forward.x)"), "fs_apply applies the veil");
    }

    #[test]
    fn lights_pick_nearest_ahead_with_layers() {
        use crate::formats::lights::WorldLight;
        let lamp = |x: f32| crate::render::lamps::Lamp {
            section: 0,
            light: WorldLight {
                kind: 2,
                sub: 2,
                on: true,
                colour: [150, 150, 150],
                position: Vec3::new(x, 0.0, 8.0),
                outer: 30.0,
                direction: Vec3::NEG_Z,
                intensity: 1.0,
                inner: 10.0,
                cone: 0.0,
            },
        };
        let lamps: Vec<_> = [10.0, -200.0, 50.0, 1000.0].into_iter().map(lamp).collect();
        let picked = pick_lights(&lamps, &[], None, Vec3::ZERO, Vec3::X, &TIER_NEXT_GEN);
        // Behind (−200 m) and too far (1000 m) are left out; the nearest first.
        assert_eq!(picked.iter().map(|l| l.position.x).collect::<Vec<_>>(), vec![10.0, 50.0]);
        assert!(picked.iter().all(|l| l.kind == LightKind::Lamp && l.layer == -1));
    }
}
