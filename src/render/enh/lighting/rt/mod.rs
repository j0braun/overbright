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
            assert_eq!(frag.matches("rt_keep(vec4<f32>(0.0), frag_in.xy, true)").count(), 4);
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
        let head = rays.split("fn rt_ray(frag_in: vec4<f32>, c: vec4<f32>) -> vec4<f32> {\n").nth(1).unwrap();
        let head = head.split("    return rt_fill(c, origin, r, n, car, term, false);").next().unwrap();
        let trace = ssr.split("fn fs_trace(@builtin(position) frag_in: vec4<f32>) -> @location(0) vec4<f32> {\n").nth(1).unwrap();
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

    /// The textured hit's surface record: UVs shifted by whole tiles, the
    /// baked colour turned from D3DCOLOR's B, G, R to R, G, B, the layer and
    /// the LOD term (a 1 m² triangle covering the whole layer: log2 of the
    /// layer's side texels per metre).
    #[test]
    fn surfaces_pack_uv_colour_layer_and_lod() {
        let uv = [[3.25f32, -1.5], [4.25, -1.5], [3.25, -0.5]];
        let s = Surface::new(uv, [[10, 20, 30, 40], [0, 0, 0, 0], [255, 255, 255, 255]], 7, 1.5);
        let f = |i: usize| f32::from_bits(s.words[i]);
        assert_eq!([f(0), f(1), f(2), f(3), f(4), f(5)], [0.25, 0.5, 1.25, 0.5, 0.25, 1.5]);
        assert_eq!(s.words[6].to_le_bytes(), [30, 20, 10, 255]);
        assert_eq!((s.words[9], f(10)), (7, 1.5));
        let p = [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
        let lod = lod_term(p, [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        assert!((lod - (HIT_TEXTURE as f32).log2()).abs() < 1e-5, "{lod}");
    }

    /// A hit layer: the texture box-filtered to the layer's side, then its
    /// mips down to 1 × 1 (a two-colour texture's mean at the end).
    #[test]
    fn hit_layers_filter_and_chain_mips() {
        let (w, h) = (256u32, 64u32);
        let mut data = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                data.extend_from_slice(if x < w / 2 { &[255, 0, 0, 255] } else { &[0, 0, 255, 255] });
            }
        }
        let t = Texture {
            name: "t".into(),
            hash: 1,
            width: w,
            height: h,
            format: crate::formats::texture::PixelFormat::Rgba8,
            mip_count: 1,
            data: data.into(),
            palette: vec![],
            alpha_test: false,
            blend: Blend::None,
            rendering_order: 0,
            late_buckets: false,
            scroll: Default::default(),
        };
        let l = hit_layer(&t);
        let n = HIT_TEXTURE as usize;
        assert_eq!(l.len(), (0..HIT_MIPS).map(|m| ((HIT_TEXTURE >> m) as usize).pow(2) * 4).sum::<usize>());
        assert_eq!(&l[0..4], &[255, 0, 0, 255]);
        assert_eq!(&l[(n - 1) * 4..n * 4], &[0, 0, 255, 255]);
        let last = &l[l.len() - 4..];
        assert!(last[0].abs_diff(128) <= 1 && last[2].abs_diff(128) <= 1 && last[1] == 0, "{last:?}");
    }

    /// A street lamp's glow (type 9) as the native sprite's first layer:
    /// its colour × doubled alpha, size and depth clamp; its record carries the
    /// glow mark, and its octahedron holds the sphere of its largest reach.
    #[test]
    fn lamp_glow_is_the_sprite_layer_and_its_octahedron_holds_it() {
        let g = Glow::from_flare(Vec3::new(1.0, 2.0, 8.0), Vec3::NEG_Z, 9, 2, 1.0).expect("type 9");
        let alpha = 220.0 / 255.0;
        assert!((g.rgb[0] - 110.0 / 255.0 * alpha).abs() < 1e-6 && g.exponent == 0.0);
        assert_eq!((g.size, g.scale), (30.0, [0.001, 0.1]));
        let s = g.surface();
        assert_eq!(s.words[9], GLOW_MARK);
        assert_eq!(f32::from_bits(s.words[2]), 8.0);
        let tris = g.octahedron();
        for t in tris.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(Vec3::from);
            let n = (b - a).cross(c - a).normalize();
            assert!(n.dot(a - g.centre).abs() >= g.reach() - 1e-4, "a face cuts the glow's sphere");
        }
        // A brake light faces its direction (flag 4); a type without a layer has no glow.
        let brake = Glow::from_flare(Vec3::ZERO, Vec3::Y, 1, 4, 0.85).unwrap();
        assert!(brake.exponent > 0.0);
        assert!(Glow::from_flare(Vec3::ZERO, Vec3::Y, 200, 0, 1.0).is_none());
    }

    /// The hit layers made on the GPU match the CPU's filter: a 256 × 256
    /// texture (a colour pattern) with its box-filtered mips, shrunk both
    /// ways, every level within 3 levels per channel (the GPU samples the
    /// source's own level of that size, the CPU boxes the top one).
    #[test]
    #[ignore = "requires a GPU; run explicitly for the hit layers"]
    fn hit_layers_on_the_gpu_match_the_cpu_filter() {
        let gpu = pollster::block_on(crate::render::Gpu::new(None, wgpu::Instance::default())).unwrap();
        let n = 256usize;
        let mut level: Vec<u8> = (0..n * n)
            .flat_map(|i| {
                let (x, y) = (i % n, i / n);
                [(x * 255 / n) as u8, (y * 255 / n) as u8, (((x / 8) ^ (y / 8)) % 2 * 255) as u8, 255]
            })
            .collect();
        let mut data = level.clone();
        let mut side = n;
        while side > 1 {
            let half = side / 2;
            let next: Vec<u8> = (0..half * half * 4)
                .map(|i| {
                    let (c, p) = (i % 4, i / 4);
                    let (x, y) = (p % half, p / half);
                    let at = |xx: usize, yy: usize| level[(yy * side + xx) * 4 + c] as u32;
                    ((at(2 * x, 2 * y) + at(2 * x + 1, 2 * y) + at(2 * x, 2 * y + 1) + at(2 * x + 1, 2 * y + 1) + 2) / 4) as u8
                })
                .collect();
            data.extend_from_slice(&next);
            level = next;
            side = half;
        }
        let t = Texture {
            name: "t".into(),
            hash: 1,
            width: n as u32,
            height: n as u32,
            format: crate::formats::texture::PixelFormat::Rgba8,
            mip_count: 9,
            data: data.into(),
            palette: vec![],
            alpha_test: false,
            blend: Blend::None,
            rendering_order: 0,
            late_buckets: false,
            scroll: Default::default(),
        };
        let cpu = hit_layer(&t);
        let view = hit_texture_gpu(&gpu, &[Some(&t)]);
        let mut at = 0usize;
        for m in 0..HIT_MIPS {
            let side = HIT_TEXTURE >> m;
            let got = read_level(&gpu, view.texture(), m, side);
            let want = &cpu[at..at + (side * side * 4) as usize];
            let worst = got.iter().zip(want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 3, "level {m}: {worst}");
            at += (side * side * 4) as usize;
        }
    }

    /// Level `m` (side × side RGBA8) of layer 0 of `t`, read back.
    fn read_level(gpu: &crate::render::Gpu, t: &wgpu::Texture, m: u32, side: u32) -> Vec<u8> {
        let row = (side * 4).div_ceil(256) * 256;
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row * side),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: t, mip_level: m, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(side) },
            },
            wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let v = buf.slice(..).get_mapped_range().expect("mapped").to_vec();
        (0..side as usize).flat_map(|y| v[y * row as usize..y * row as usize + side as usize * 4].to_vec()).collect()
    }

    /// The list's compute pass sums the glow on its one query (rt.wgsl's
    /// `rt_glow` inlined on `rq`): no second query is reached from the pass.
    #[test]
    fn glow_rides_the_list_query() {
        let ssr = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
        let rays = list_rays_source(ssr);
        let fill = rays.split("fn rt_fill(").nth(1).unwrap().split("\nfn ").next().unwrap();
        assert!(!fill.contains("rt_glow("), "{fill}");
        assert!(fill.contains("rayQueryInitialize(&rq, rt_tlas, RayDesc(RAY_FLAG_NONE, 0x04u"));
        assert!(fill.contains("rayQueryGetCandidateIntersection(&rq)"));
        // The opaque casts leave the glowing instance out.
        assert!(!rays.contains("0xFFu, select("));
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
