//! Particles High / Ultra (docs/particles-smoke.md; a **clone
//! deviation**, opt-in; nothing here exists in SPEED2.EXE). The native
//! particle system (`crate::particles`, 0x491E50 / 0x492E10 / 0x48BDF0)
//! keeps running exactly as before: it decides where, when and how many
//! particles the original spawns. With the Graphics row "Particles" on
//! High or Ultra, each particle the native emitters spawn from a **smoke**
//! description (tyre / burnout / drift smoke, the nitrous purge, crash and
//! contact smoke; [`is_smoke`]) is handed to this GPU system, which expands
//! it into several smaller puffs (compute spawn), moves them on the GPU
//! (compute update: the description's own drag and gravity, plus noise
//! turbulence, a light wind and the drag of passing cars) and draws them
//! as instanced billboards with procedural noise detail, a depth fade
//! against the scene (soft particles, no hard edges on the road or the
//! cars), light from the street lamps (the dynamic lamps' light-pack list), the cars'
//! headlights and tail lights, and a thickness term. The native quad of a
//! smoke particle is then not drawn; sparks, nitrous flames, the event
//! markers and every other particle keep the native quads.
//!
//! Spawning follows the native births tick by tick ([`Feeder::record`],
//! queued per 1/60 s world update and replayed by [`Detail::prepare`] at the
//! next draw), so the emit rules and timing stay native.

use crate::particles::{Description, Library, Particle};
use crate::render::Gpu;
use crate::render::camera::Camera;
use glam::{Mat4, Vec3};
use std::collections::VecDeque;

mod feeder;
pub use feeder::*;
mod gpu_common;
pub use gpu_common::*;
mod detail;
pub use detail::*;
mod half_size;
pub use half_size::*;
mod gpu_smoke;
use gpu_smoke::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::{Cubic, Emitter, System};

    fn smoke_library() -> Library {
        let mut d = Description { name: "XETT_RoadSk_SmkA".into(), ..Default::default() };
        d.rate = [30.0, 0.2];
        d.life = [2.0, 0.3];
        d.speed = [1.5, 0.4];
        d.extent = Vec3::splat(0.2);
        d.drag = 0.4;
        d.gravity = -0.3;
        d.inherit = [0.3, 0.5];
        d.color = Cubic([[0.0; 4], [0.0; 4], [0.0, 0.0, 0.0, -120.0], [150.0, 150.0, 175.0, 120.0]]);
        d.size = Cubic([[0.0; 4], [0.0; 4], [6.0, 0.0, 0.0, 0.0], [0.5, 0.0, 0.0, 0.0]]);
        d.range = 200.0;
        let mut spark = d.clone();
        spark.name = "XEBO_ROAD_SprksA".into();
        Library { descriptions: vec![d, spark], templates: Vec::new() }
    }

    /// Recording the births (Particles High / Ultra) does not change the
    /// native system: same spawns, same particles, tick by tick.
    #[test]
    fn emit_counts_unchanged_vs_native() {
        let lib = smoke_library();
        let run = |record: bool| {
            let mut sys = System::default();
            let mut e = Emitter::new(0, Mat4::from_translation(Vec3::new(10.0, 5.0, 0.0)), 7);
            let mut s = Emitter::new(1, Mat4::IDENTITY, 9);
            let mut feeder = Feeder::default();
            let mut spawned = 0;
            let mut snapshots = Vec::new();
            for tick in 0..240 {
                if record {
                    sys.births = Some(Vec::new());
                }
                e.velocity = Vec3::new(20.0, 0.0, 0.0);
                sys.emit(&lib, &mut e, 1.0 / 60.0 * (1.0 + (tick % 7) as f32));
                sys.emit(&lib, &mut s, -1.0);
                sys.update(&lib, 1.0 / 60.0, |_| None);
                if let Some(b) = sys.births.take() {
                    spawned += b.len();
                    feeder.record(&lib, b, vec![None], 1.0 / 60.0, ULTRA, 0.0);
                }
                snapshots.push(
                    sys.particles
                        .iter()
                        .map(|p| (p.description, p.life, p.speed, p.direction, p.position.to_array().map(f32::to_bits)))
                        .collect::<Vec<_>>(),
                );
            }
            let smoke_births: usize = feeder.ticks.iter().map(|t| t.births.len()).sum();
            (snapshots, spawned, smoke_births)
        };
        let (native, n0, _) = run(false);
        let (detailed, n1, smoke) = run(true);
        assert_eq!(native, detailed, "the native particles are untouched");
        assert_eq!(n0, 0);
        assert!(n1 > 100, "births recorded: {n1}");
        // Every smoke spawn is handed over once; the sparks are not.
        assert!(smoke > 0 && smoke < n1, "{smoke} of {n1}");
    }

    /// The half-size draw's two targets, blended as the pipeline sets
    /// them up and composited as smoke_half.wgsl does (scene · T + C,
    /// alpha · T + A), give the native per-puff "over" (colour One /
    /// InvSrcAlpha, alpha SrcAlpha / InvSrcAlpha) of the same puffs, and
    /// the composite's module validates (single and multisampled depth).
    #[test]
    fn half_size_targets_composite_to_the_native_over() {
        let puffs = [
            (0.3, [0.2, 0.1, 0.05]),
            (0.8, [0.5, 0.4, 0.3]),
            (0.05, [0.9, 0.9, 0.9]),
            (1.0, [0.1, 0.2, 0.3]),
            (0.42, [0.0, 0.7, 0.2]),
        ];
        for (scene, scene_a) in [([0.25, 0.5, 0.75], 0.6), ([0.0, 0.0, 0.0], 0.0), ([2.0, 1.0, 0.5], 1.0)] {
            let mut native = (scene, scene_a);
            let (mut c, mut t, mut a_sum) = ([0.0f64; 3], 1.0f64, 0.0f64);
            for &(a, rgb) in &puffs[..] {
                let src: [f64; 3] = rgb.map(|v: f64| v * a);
                for k in 0..3 {
                    native.0[k] = src[k] + (1.0 - a) * native.0[k];
                    c[k] = src[k] + (1.0 - a) * c[k];
                }
                native.1 = a * a + (1.0 - a) * native.1;
                t *= 1.0 - a;
                a_sum = a * a + (1.0 - a) * a_sum;
            }
            for k in 0..3 {
                assert!((c[k] + t * scene[k] - native.0[k]).abs() < 1e-12);
            }
            assert!((a_sum + t * scene_a - native.1).abs() < 1e-12);
        }
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/smoke_half.wgsl"));
        for src in [source.to_string(), source.replace("texture_depth_2d", "texture_depth_multisampled_2d")] {
            let module = wgpu::naga::front::wgsl::parse_str(&src).expect("parses");
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("valid");
        }
    }

    #[test]
    fn soft_particle_math() {
        let mut c = Camera::new(Vec3::new(1200.0, -800.0, 20.0), 0.4, -0.05);
        c.near = 0.25;
        c.far = 5000.0;
        for reversed in [false, true] {
            let p = c.projection_depth(16.0 / 9.0, reversed) * Mat4::look_to_rh(Vec3::ZERO, c.forward(), c.up);
            for z in [0.5f32, 2.0, 10.0, 75.0, 900.0] {
                let clip = p * (c.forward() * z).extend(1.0);
                let d = clip.z / clip.w;
                let back = linear_depth(d, c.near, c.far, reversed);
                assert!((back - z).abs() < 2e-3 * z.max(1.0) + 1e-3 * z * z / 100.0, "{reversed} {z}: {back}");
            }
        }
        // Behind the surface: hidden; in front by the soft distance: full.
        assert_eq!(soft_fade(10.0, 10.5, 0.5), 0.0);
        assert_eq!(soft_fade(10.0, 10.0, 0.5), 0.0);
        assert!((soft_fade(10.0, 9.75, 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(soft_fade(10.0, 9.0, 0.5), 1.0);
        assert_eq!(softness(0.2), 0.15);
        assert_eq!(softness(2.0), 0.5);
        assert_eq!(softness(8.0), 0.6);
    }

    #[test]
    fn original_tier_takes_nothing() {
        assert_eq!(tier_params(0), None);
        assert_eq!(tier_params(1), Some(HIGH));
        assert_eq!(tier_params(2), Some(ULTRA));
        // Ultra: more, smaller, longer-lived puffs than High.
        assert!(ULTRA.children > HIGH.children && ULTRA.size < HIGH.size && ULTRA.linger > HIGH.linger);
        // The quads' filter with nothing replaced is the native list.
        let lib = smoke_library();
        let mut sys = System::default();
        let mut e = Emitter::new(0, Mat4::IDENTITY, 3);
        sys.emit(&lib, &mut e, 1.0);
        let all = sys.quads(&lib, Vec3::new(0.0, -5.0, 0.0), Vec3::X, Vec3::Z, Vec3::Y, 500.0);
        let kept = sys.quads_where(&lib, Vec3::new(0.0, -5.0, 0.0), Vec3::X, Vec3::Z, Vec3::Y, 500.0, |_| true);
        assert_eq!(all.len(), kept.len());
        assert!(all.iter().zip(&kept).all(|(a, b)| a.corners == b.corners && a.uv == b.uv && a.rgba == b.rgba));
    }

    #[test]
    fn ring_wraps_at_the_pool() {
        let slots: Vec<u32> = ring_slots(14, 2, 4, 16).collect();
        assert_eq!(slots, [14, 15, 0, 1, 2, 3, 4, 5]);
    }

    /// GLOBALB's smoke: tyre, purge and crash smoke go to the detailed
    /// path; sparks, flames and the markers' smoke stay native.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn smoke_classification_over_globalb() {
        let root = crate::testkit::game_root();
        let data = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let mut lib = Library::default();
        lib.add(&data);
        let smoke = |n: &str| lib.by_name(n).map(|i| is_smoke(&lib.descriptions[i]));
        for n in [
            "XETT_RoadSk_SmkA",
            "XETT_RoadSp_SmkA",
            "XES_NOSBO_A_Hood_SmokeA_01",
            "XES_NOSBO_C_Wheel_SmokeB_01",
            "XECI_Car_SmokeA",
            "XEBO_ROAD_SmokeA",
            "XETT_GrassSK_Smoke",
        ] {
            assert_eq!(smoke(n), Some(true), "{n}");
        }
        for n in [
            "XES_NOS_FlameA",
            "XEBO_ROAD_SprksA",
            "XECI_Car_SprksA",
            "XEE_DVD_SmokeA_01",
            "XECH_Flag_Girl_Flare_SmokeA",
        ] {
            assert_eq!(smoke(n), Some(false), "{n}");
        }
    }

    /// The GPU spawn and update: every birth makes its puffs, they move
    /// and age, and die after their (lingering) life.
    #[test]
    #[ignore = "requires a GPU adapter; run serially with --ignored"]
    fn gpu_spawn_and_update() {
        let gpu = pollster::block_on(Gpu::new(
            None,
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        ))
        .unwrap();
        let lib = smoke_library();
        let mut g = GpuSmoke::new(&gpu, HIGH, &lib, &Default::default());
        g.tables(&gpu, &lib, &[]);
        let births: Vec<Birth> = (0..10)
            .map(|i| Birth {
                position: [i as f32, 0.0, 1.0],
                life: 1.0,
                velocity: [0.0, 0.0, 1.0],
                share: 0.0,
                ids: [0, 0, i, u32::MAX],
            })
            .collect();
        let tick = |b: Vec<Birth>| Tick {
            dt: 1.0 / 60.0,
            births: b,
            cars: vec![Some((Vec3::new(100.0, 0.0, 0.0), Vec3::ZERO))],
            time: 0.0,
            wet: 0.0,
        };
        let read = |g: &GpuSmoke| {
            let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: g.params.pool as u64 * 64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut e = gpu.device.create_command_encoder(&Default::default());
            e.copy_buffer_to_buffer(&g.pool, 0, &staging, 0, g.params.pool as u64 * 64);
            gpu.queue.submit([e.finish()]);
            staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let v: Vec<[f32; 16]> =
                bytemuck::cast_slice(&staging.slice(..).get_mapped_range().expect("mapped")).to_vec();
            v
        };
        let mut e = gpu.device.create_command_encoder(&Default::default());
        g.simulate(&gpu, &mut e, &[tick(births)]);
        gpu.queue.submit([e.finish()]);
        let pool = read(&g);
        let alive: Vec<_> = pool.iter().filter(|p| p[7] > 0.0).collect();
        assert_eq!(alive.len(), 10 * HIGH.children as usize);
        assert!(alive.iter().all(|p| p[3] > 0.0 && p[2] > 0.5 && (p[7] - HIGH.linger).abs() <= 0.21 * HIGH.linger));
        let ticks: Vec<Tick> = (0..150).map(|_| tick(Vec::new())).collect();
        let mut e = gpu.device.create_command_encoder(&Default::default());
        g.simulate(&gpu, &mut e, &ticks);
        gpu.queue.submit([e.finish()]);
        assert!(read(&g).iter().all(|p| p[7] == 0.0), "all puffs dead after 2.5 s");
    }
}
