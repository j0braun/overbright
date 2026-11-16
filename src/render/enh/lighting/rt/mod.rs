//! Ray-traced reflections (the Graphics page's Screen-Space
//! Reflections row at "Ray Traced (experimental)"; a **clone deviation**,
//! off by default; docs/rt-reflections.md). Nothing here is native: the
//! game has its planar road capture and the car's cube only.
//!
//! wgpu's `EXPERIMENTAL_RAY_QUERY` (Vulkan only, "may have major bugs"):
//! the device asks for it only on a discrete Vulkan GPU that offers it
//! ([`wanted`]); elsewhere the tier is greyed out.
//!
//! The scene: one BLAS of the world, made once per region from each
//! placement's low LOD (`Placement::low_solid`, what the native road
//! reflection and cube faces draw) in world space, and one BLAS per car
//! model (its parts at LOD C, car space), cached; a TLAS rebuilt every
//! frame (the world once, the cars at their poses), from a ring of three
//! (frames in flight). Each triangle has one colour (`colors`): the world's
//! baked vertex colour × its texture's mean (world.efx `tex × v0 × 2`), a
//! car's texture mean × its light material's mid diffuse.
//!
//! The trace: the SSR trace pass ([`super::ssr`]) in a variant with this
//! module's group 1 (`rt.wgsl` appended to `ssr.wgsl`): where its screen
//! march misses or is unsure (off screen, behind the camera, rays at the
//! camera), one ray query per trace pixel fills the rest with the hit's
//! colour; the SSR filter and composite then run as before (never
//! stronger than native on a dry road: the chrome-flicker fix's luminance hold applies to RT
//! hits as well).

use std::collections::HashMap;
use std::sync::Mutex;

use glam::{Mat4, Vec3};

use crate::car::Car;
use crate::formats::texture::{Blend, Texture};
use crate::render::Gpu;
use crate::world::World;

mod geometry;
pub use geometry::*;
mod scene;
pub use scene::*;
mod trace;
pub use trace::*;
pub mod shadow;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_source_hooks_every_anchor_once() {
        let ssr = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
        for (from, _) in ANCHORS {
            assert_eq!(ssr.matches(from).count(), 1, "{from}");
        }
        let s = trace_source(ssr);
        assert_eq!(s.matches("rt_fill(").count(), 6); // 5 calls + the definition
        assert!(s.starts_with("enable wgpu_ray_query;"));
        // It parses and validates (naga, no GPU).
        let module = wgpu::naga::front::wgsl::parse_str(&s).expect("parse");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("valid");
    }

    fn validate(s: &str) -> wgpu::naga::Module {
        let module = wgpu::naga::front::wgsl::parse_str(s).unwrap_or_else(|e| panic!("{}", e.emit_to_string(s)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("valid");
        module
    }

    /// The ray list's two modules (docs/rt-reflections.md "The ray
    /// list"), for both depth forms: the fragment pass is the RT form with
    /// each `rt_fill` call's result sent to `rt_keep` instead (the same
    /// five places, zero for the four whose result is 0) and holds no ray
    /// query; the compute pass's `rt_ray` is fs_trace's head as written,
    /// then `rt_fill`; both parse and validate (naga, no GPU).
    #[test]
    fn list_sources_follow_the_rt_form() {
        let ssr = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
        for src in [ssr.to_string(), crate::render::ssr::multisampled(ssr)] {
            let frag = list_trace_source(&src);
            assert_eq!(frag.matches("rt_keep(").count(), 6); // 5 calls + the definition
            assert_eq!(frag.matches("rt_keep(vec4<f32>(0.0), frag.xy, true)").count(), 4);
            assert!(!frag.contains("rayQuery") && !frag.contains("ray_query;") && !frag.contains("rt_fill("));
            for ((from, rt), (from_list, list)) in ANCHORS.iter().zip(LIST_ANCHORS.iter()) {
                assert_eq!(from, from_list);
                // The same line, its call swapped.
                let call = |s: &str| s.split("return ").nth(1).unwrap().split('(').next().unwrap().to_string();
                assert_eq!(call(rt), "rt_fill");
                assert_eq!(call(list), "rt_keep");
            }
            validate(&frag).entry_points.iter().find(|e| e.name == "fs_trace").expect("fs_trace");
        }
        let rays = list_rays_source(ssr);
        assert!(rays.starts_with("enable wgpu_ray_query;"));
        // The head is fs_trace's, every line up to the ray's reach.
        let head = rays.split("fn rt_ray(frag: vec4<f32>, c: vec4<f32>) -> vec4<f32> {\n").nth(1).unwrap();
        let head = head.split("    return rt_fill(c, origin, r, n, car, term, false);").next().unwrap();
        let trace = ssr.split("fn fs_trace(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {\n").nth(1).unwrap();
        assert!(trace.starts_with(head) && head.ends_with("    let r = reflect(v, n);\n"), "{head}");
        // rt_fill's two casts on one query (rt_cast's own is left unused).
        assert_eq!(rays.matches("rt_cast(").count(), 1, "rt_cast's definition only");
        assert_eq!(rays.matches("var rq: ray_query;").count(), 2);
        assert_eq!(rays.matches("rayQueryInitialize(&rq, rt_tlas, RayDesc(RAY_FLAG_FORCE_OPAQUE, ").count(), 3);
        let module = validate(&rays);
        assert!(module.entry_points.iter().any(|e| e.name == "cs_rt_rays"));
    }

    /// A listed pixel's packing covers the largest trace the list takes.
    #[test]
    fn list_pixels_pack_in_15_bits() {
        assert!(MAX_SIDE > 16384, "the largest 2-D texture of the devices it runs on");
        let (x, y) = (MAX_SIDE - 1, MAX_SIDE - 1);
        let e = x | (y << 15) | 0x8000_0000;
        assert_eq!((e & 0x7FFF, (e >> 15) & 0x7FFF, e >> 31), (x, y, 1));
        assert!(u64::from(MAX_GROUPS) * 64 >= 3840 * 2160 / 2, "a round covers half a 4K trace");
    }

    #[test]
    fn instance_transform_is_row_major_3x4() {
        let m = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)) * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let t = instance_transform(m);
        let p = m.transform_point3(Vec3::new(1.0, 0.0, 0.0));
        let q = [t[0] + t[3], t[4] + t[7], t[8] + t[11]];
        assert!((Vec3::from(q) - p).length() < 1e-5, "{q:?} vs {p}");
    }

    #[test]
    fn pack_halves_and_clamps() {
        assert_eq!(pack([0.0, 0.5, 1.0]), 0xFF_FF_80_00);
        assert_eq!(pack([2.0, -1.0, 0.25]) & 0xFF_FFFF, 0x40_00_FF);
    }

    /// Bayview's traced world (game data; ignored without NFSU2_ROOT): the
    /// low LODs give a few hundred thousand to a few million triangles,
    /// under the colour index's 24 bits with the cars' room, every colour
    /// opaque, no sky.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bayview_world_geometry_fits() {
        let root = crate::testkit::game_root();
        let world = World::load(std::path::Path::new(&root), "L4RA").expect("world");
        let t = std::time::Instant::now();
        let mut means = HashMap::new();
        let g = world_geometry(&world, &mut means);
        eprintln!(
            "bayview rt world: {} triangles, {} textures, {:.0} ms",
            g.triangles(),
            means.len(),
            t.elapsed().as_secs_f64() * 1e3
        );
        assert_eq!(g.positions.len(), g.triangles() * 3);
        assert!(g.triangles() > 100_000, "{}", g.triangles());
        assert!((g.triangles() as u32) + CAR_COLOR_ROOM < 1 << 24);
        assert!(g.colors.iter().all(|c| c >> 24 == 0xFF));
    }
}
