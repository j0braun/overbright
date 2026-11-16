//! Modern Lighting, phase 2 "bold" (clone deviation, opt-in: the
//! Graphics → Lighting page's "Lighting" row and its "Lighting Strength";
//! docs/modern-lighting.md). Nothing here is native: SPEED2.EXE lights the
//! world with its baked vertex colours only (world.efx `tex × v0 × 2`) and
//! the cars with the light packs' harmonic (0x48E3D0 → 0x4893A0).
//!
//! Phase 1 (`modern_light.rs`) relights only the street lamps' share of
//! the bake. Phase 2 lights the whole city from dynamic lights:
//!
//! * every light-pack lamp as a per-pixel physical pool (the fitted colour
//!   and falloff of phase 1, sharper with the strength), its share of the
//!   bake taken out and the rest of the bake (the artists' paint, sky,
//!   bounce) kept as the ambient term, darkened by the strength
//!   ([`Params::ambient`]: "painted darkness");
//! * the neon and the lit signs (the world's additive textures named
//!   SGN_* / *NEON*, merged per [`EMITTER_CELL`] m cell; [`emitters`]) as
//!   coloured area-ish lights;
//! * every car's tail lights and the other cars' headlights ([`car_lights`]);
//! * shadows: up to [`layers`] cached lamp depth layers (the Lamp Shadows' layer cache),
//!   the cars as soft ellipsoids; with "Modern + RT" a ray query toward
//!   each of the [`Params::rays`] strongest lights per pixel (the ray-traced reflections'
//!   scene: the world's low LOD and the cars);
//! * a GGX specular from all of them on the roads (wet or dry), and the
//!   cars lit per pixel by the four strongest of the same lights
//!   ([`car_lamps`]).
//!
//! Strength 0 ("Classic") is phase 1's relight exactly (plus nothing new);
//! 100 ("Bold") the full dynamic look. The constants are a judgement call
//! (no native counterpart); they are named here and in world.wgsl.

use std::sync::{Mutex, RwLock};

use glam::{Mat4, Vec3};

use crate::formats::lights::WorldLight;
use crate::render::lamps::{GpuLamp, Lamp, Occluder};

mod params;
pub use params::*;
mod emitters;
pub use emitters::*;
mod car_lights;
pub use car_lights::*;
mod state;
pub use state::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// world.wgsl's MODERN / MLRT blocks: the params index matches, every
    /// tier's module (with the arrays, multiview and mip-bias forms of the
    /// lamps' module) parses and validates, the ray-query lines only in
    /// Modern + RT, and Modern off is the lamps' module as before Modern Lighting.
    /// docs/look-direction.md rule "darken only where the original is dark": the strength's darkening, boost
    /// and emitter light fade out where the bake itself is bright, so the
    /// artists' lit surfaces and the highlights (and the glow they feed) are
    /// the original's; halving the bake everywhere took p95 −34 at Bold.
    #[test]
    fn darkening_is_anchored_to_the_bake() {
        let wgsl = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        assert!(wgsl.contains("const ML_KEEP_LO: f32 = 0.06;") && wgsl.contains("const ML_KEEP_HI: f32 = 0.22;"));
        assert!(wgsl.contains("let keep = smoothstep(ML_KEEP_LO, ML_KEEP_HI,"));
        for t in [
            "let amb = mix(P.ambient, 1.0, keep);",
            "let boost = mix(P.boost, 1.0, keep);",
            "let emit = P.emit * (1.0 - keep);",
            "let rest = (bake - share * k) * amb;",
        ] {
            assert!(wgsl.contains(t), "{t}");
        }
        assert!(!wgsl.contains("* P.boost +"), "every relit term takes the anchored boost");
    }

    #[test]
    fn world_modules_per_tier() {
        let wgsl = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        assert!(wgsl.contains(&format!("const ML_PARAMS: u32 = {PARAMS_INDEX}u;")));
        // MLRT nests inside MODERN (the composer's sections, core::shader):
        // MLRT without MODERN adds nothing.
        use crate::render::core::shader::{compose::VariantFlag, registry};
        let lamps = |extra: &[VariantFlag]| {
            registry::compose(
                registry::ShaderId::World,
                [VariantFlag::Lamps, VariantFlag::TextureBindingPlain]
                    .into_iter()
                    .chain(extra.iter().copied())
                    .collect(),
            )
            .unwrap()
        };
        assert_eq!(lamps(&[VariantFlag::Mlrt]), lamps(&[]), "an MLRT section outside MODERN");
        let off = crate::render::world_source_modern(true, 0);
        assert!(!off.contains("ml_relight"));
        let m1 = crate::render::world_source_modern(true, 1);
        assert!(m1.contains("ml_relight") && !m1.contains("ml_tlas") && !m1.contains("ray_query"));
        let m2 = crate::render::world_source_modern(true, 2);
        assert!(m2.starts_with("enable wgpu_ray_query;") && m2.contains("ml_tlas"));
        use wgpu::naga;
        for modern in 0..=2u8 {
            for (arrays, multiview, biased) in [(false, false, false), (true, false, false), (false, false, true)] {
                let src =
                    crate::render::world_module_source_modern(true, modern, arrays, multiview, biased, false);
                let module = naga::front::wgsl::parse_str(&src)
                    .unwrap_or_else(|e| panic!("modern {modern}: {}", e.emit_to_string(&src)));
                naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                    .validate(&module)
                    .unwrap_or_else(|e| panic!("modern {modern} arrays {arrays}: {e:?}"));
            }
        }
    }

    /// The cars' Modern module: car.wgsl untouched natively, every form
    /// (paint, wet, surface) takes the hook once and validates.
    #[test]
    fn car_modules() {
        let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        assert!(!native.contains("car_modern") && !native.contains("g_car_modern_spec"));
        use wgpu::naga;
        for (paint, wet, surface) in
            [(false, false, false), (true, false, false), (true, true, false), (false, true, true)]
        {
            let src = car_source(&crate::render::car::source_for_tests(paint, wet, surface)).unwrap();
            assert_eq!(src.matches("car_modern(in)").count(), 1);
            let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .expect("valid");
        }
    }

    #[test]
    fn params_span_classic_to_bold() {
        let c = Params::new(Tier::Modern, 0.0);
        assert_eq!(
            (c.ambient, c.boost, c.uncap, c.emit, c.spec),
            (1.0, 1.0, 0.0, 0.0, 1.0),
            "classic is phase 1's relight"
        );
        assert_eq!(c.soft, crate::render::modern_light::POOL_SOFT);
        let b = Params::new(Tier::Rt, 1.0);
        assert_eq!((b.ambient, b.boost, b.soft, b.emit), (AMBIENT_BOLD, BOOST_BOLD, SOFT_BOLD, EMIT_BOLD));
        assert!(b.ambient < 1.0 && b.boost > 1.0, "bold: darker gaps, brighter pools");
        assert_eq!((b.rays, b.tier), (4, 3));
        let low = Params::new(Tier::Low, 0.75);
        assert_eq!((low.shadowed, low.rays, low.tier, low.lights), (2, 0, 1, 4));
        let g = b.gpu();
        assert_eq!(g.position, [1.0, AMBIENT_BOLD, BOOST_BOLD, SOFT_BOLD]);
        assert_eq!(g.colour[0], 3.0);
    }

    #[test]
    fn layers_per_tier() {
        assert_eq!(layers(Tier::Off), None);
        assert_eq!(layers(Tier::Low), Some((512, 4)));
        assert_eq!(layers(Tier::Modern), Some((1024, 24)));
        assert_eq!(layers(Tier::Rt), layers(Tier::Modern));
        // The views buffer holds them and the headlight's.
        assert!(24 < crate::render::lamps::MAX_LAYERS);
    }

    #[test]
    fn emissive_names() {
        for n in ["SGN_NEONCASINO_A_LL", "SGN_HOTEL", "ARC_AUDIOSHOPNEON_A_RB", "ARC_RBLDING_SIGN"] {
            assert!(emissive_name(n), "{n}");
        }
        for n in [
            "SFX_FLAREMISTA_DM",
            "SFX_LIGHT_BEAMA",
            "TRN_CITYGRIDE",
            "HEADLIGHTGLOW",
            "ARC_OFFICEWINDOW_D_BK",
            "ARC_STREETLIGHTS_A_DM",
            "OBJ_TRACKBARRIERB_NEON_",
        ] {
            assert!(!emissive_name(n), "{n}");
        }
    }

    #[test]
    fn emitter_light_grows_with_area_and_keeps_the_hue() {
        assert!(emitter_light(0.01, Vec3::ZERO, Vec3::ONE, Vec3::Z).is_none(), "too small");
        assert!(emitter_light(4.0, Vec3::ZERO, Vec3::splat(0.01), Vec3::Z).is_none(), "too dim");
        let small = emitter_light(1.0, Vec3::ZERO, Vec3::new(0.4, 0.1, 0.5), Vec3::X * 0.9).unwrap();
        let big = emitter_light(16.0, Vec3::ZERO, Vec3::new(0.4, 0.1, 0.5), Vec3::X * 0.1).unwrap();
        assert!(big.outer > small.outer && big.colour[2] > small.colour[2]);
        // Magenta stays magenta: blue the strongest, green the weakest.
        assert!(small.colour[2] > small.colour[0] && small.colour[0] > small.colour[1]);
        assert_eq!(small.sub, SUB_EMITTER);
        assert!(big.outer <= 24.0);
        assert_eq!(small.direction, Vec3::X, "faces one way");
        assert_eq!(big.direction, Vec3::ZERO, "all around");
    }

    #[test]
    fn car_lights_sit_behind_the_box_and_skip_their_own_car() {
        let o = Occluder {
            transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            min: Vec3::new(-2.2, -0.9, 0.0),
            max: Vec3::new(2.2, 0.9, 1.3),
            glow: 0.0,
        };
        let pools = [
            crate::render::HeadlightPool { transform: Mat4::IDENTITY, color: [255, 255, 255] },
            crate::render::HeadlightPool { transform: o.transform, color: [200, 200, 180] },
        ];
        let l = car_lights(&[o], &pools, false);
        assert_eq!(l.len(), 2, "one tail light, one headlight (not the player's pool)");
        assert!(l[0].light.position.x < 10.0 - 2.2 && l[0].light.sub == SUB_TAIL && l[0].light.inner == 0.0);
        assert!(l[1].light.position.x > 12.0 && l[1].light.sub == SUB_HEAD && l[1].light.inner == 0.0);
        assert_eq!(car_lights(&[o], &pools, true).len(), 1, "Headlight Beams draws the headlights");
        let mut g = vec![crate::render::lamps::gpu_lamp(&l[0].light, -1); 1];
        tag(&mut g, &[0], &l, crate::render::modern_light::CITY_CORE);
        assert_eq!((g[0].colour[3], g[0].shadow[0]), (-2.0, -2.0));
        assert!(g[0].shadow[1] > g[0].shadow[2]);
    }

    #[test]
    fn light_pool_shapes() {
        // Pack lamps: phase 1's pool.
        let rel = Vec3::new(3.0, 0.0, 9.0);
        assert_eq!(
            light_pool(rel, Vec3::Z, 30.0, 0.0, 150.0),
            crate::render::modern_light::pool_soft(rel, Vec3::Z, 30.0, 150.0)
        );
        // Emitters: 0 on their own surface, near 1 a few metres off,
        // falling, 0 at the reach.
        assert_eq!(light_pool(Vec3::new(0.0, 0.0, 0.5), Vec3::Z, 12.0, -1.0, 0.0), 0.0);
        let near = light_pool(Vec3::new(0.0, 0.0, 3.5), Vec3::Z, 12.0, -1.0, 0.0);
        assert!(near > 0.3 && near < 1.0, "{near}");
        assert!(light_pool(Vec3::new(0.0, 0.0, 6.0), Vec3::Z, 12.0, -1.0, 0.0) < 0.2);
        assert_eq!(light_pool(Vec3::new(0.0, 0.0, 12.0), Vec3::Z, 12.0, -1.0, 0.0), 0.0);
    }

    #[test]
    fn car_lamps_off_without_params() {
        publish(&[], None);
        assert!(car_lamps(Vec3::ZERO).is_none());
        let p = Params::new(Tier::Modern, 1.0);
        let mut lamp = GpuLamp {
            position: [0.0, 0.0, 5.0, 30.0],
            colour: [0.0; 4],
            direction: [0.0, 0.0, -1.0, -2.0],
            shadow: [-1.0, 0.5, 0.4, 0.3],
        };
        publish(&[lamp], Some(p));
        let l = car_lamps(Vec3::ZERO).unwrap();
        assert_eq!(l[0], [0.0, 0.0, 5.0, 30.0]);
        assert!(l[1][3] < 0.0 && (l[1][0] - 0.5 * BOOST_BOLD).abs() < 1e-5);
        // A car's own tail light does not light it.
        lamp.colour[3] = -2.0;
        lamp.position = [0.0, -2.5, 0.5, 6.0];
        publish(&[lamp], Some(p));
        let l = car_lamps(Vec3::ZERO).unwrap();
        assert_eq!(l[1][0], 0.0);
        publish(&[], None);
    }
}
