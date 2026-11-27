//! Particles Cinematic (docs/notes/graphics/smoke.md "Particles Cinematic: the simulation and the billboard drawing"; a **clone
//! deviation**, opt-in; nothing here exists in SPEED2.EXE). The Particles
//! row's top tier, separate from High / Ultra's code path
//! (`render::gpu_particles`, whose [`Feeder`] it shares unchanged): the
//! native emitters still decide where, when and how many smoke particles
//! spawn; each native smoke birth becomes a few large **billows** and one
//! faint, long-lived **haze** puff, moved on the GPU by a divergence-free
//! noise flow (rolling eddies), buoyancy, the wind and the cars' wakes,
//! and drawn
//!
//! * at a **reduced resolution** (a half or a third of the scene's, by
//!   [`lowres_div`]) into a weighted-blended OIT pair (order-independent:
//!   no sorting, no popping), soft against a nearest-depth downsample of
//!   the scene;
//! * with a billow density from a tiling 3-D noise volume (value and
//!   gradient in one fetch), whose slice rolls with age; a normal from the
//!   density gradient, a self-shadow step toward the light, an ambient
//!   from above, the street lamps (Modern Lighting's physical falloff
//!   when it is on), the cars' headlights (forward scattering) and tail
//!   lights;
//! * composited over the scene with a **depth-aware (bilateral) upsample**
//!   (no halos around the car or poles).
//!
//! On a wet road (rain) the smoke turns to steam: whiter, thinner,
//! shorter-lived, rising faster. The native quads of the smoke particles
//! are not drawn (as High / Ultra); sparks, flames, markers stay native.

use crate::particles::Library;
use crate::render::Gpu;
use crate::render::camera::Camera;
use crate::render::gpu_particles::{Birth, Feeder, MAX_BIRTHS, MAX_CARS, MAX_TICKS, Tick, TierParams, is_smoke};
use glam::{Mat4, Vec3, Vec4};

mod params;
pub use params::*;
mod noise;
pub use noise::*;
mod cinematic;
pub use cinematic::*;
mod gpu_common;
use gpu_common::*;
mod gpu;
use gpu::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_routing() {
        assert!(!active(0) && !active(1) && !active(2));
        assert!(active(TIER));
        let f = feed_params();
        assert_eq!(f.children, CINEMATIC.billows + 1);
        assert!(f.linger >= CINEMATIC.haze_life && f.linger >= CINEMATIC.linger);
        // Fewer, larger puffs than Ultra (the cost is fill, not count).
        let u = crate::render::gpu_particles::ULTRA;
        assert!(CINEMATIC.billows + 1 < u.children && CINEMATIC.size > u.size);
    }

    #[test]
    fn reduced_resolution() {
        if crate::render::debug_env::nfsu2_smoke2_div_set() {
            return;
        }
        assert_eq!(lowres_div(720), 2);
        assert_eq!(lowres_div(1080), 2);
        assert_eq!(lowres_div(1440), 2);
        assert_eq!(lowres_div(2160), 3);
        assert_eq!(lowres_div(4320), 4);
        assert_eq!(lowres_size((1920, 1080), 2), (960, 540));
        assert_eq!(lowres_size((3840, 2160), 3), (1280, 720));
        assert_eq!(lowres_size((1281, 721), 2), (641, 361));
    }

    #[test]
    fn bilateral_prefers_matching_depth() {
        // Same depth: full weight; a car edge 10 m nearer: tiny.
        assert!((bilateral(20.0, 20.0) - 1.0).abs() < 1e-3);
        assert!(bilateral(10.0, 20.0) < 0.01);
        // Relative: a 0.1 m step far away still counts as the same surface.
        assert!(bilateral(200.1, 200.0) > 0.99);
        assert!(bilateral(10.0, 20.0) > 0.0, "never zero (normalisable)");
    }

    #[test]
    fn wake_trails_behind_the_car() {
        let v = Vec3::new(20.0, 0.0, 0.0);
        let (behind, air) = wake(Vec3::new(-3.0, 0.0, 0.5), v);
        let (ahead, _) = wake(Vec3::new(4.0, 0.0, 0.5), v);
        let (side, _) = wake(Vec3::new(-3.0, 6.0, 0.5), v);
        assert!(behind > 0.5, "{behind}");
        assert!(ahead < 0.01 && side < 0.05, "{ahead} {side}");
        // Dragged forward at part of the car's speed, rolling up.
        assert!(air.x > 5.0 && air.x < 20.0 && air.z > 0.0);
        // Out to the side it is on.
        let (_, left) = wake(Vec3::new(-3.0, 1.0, 0.5), v);
        let (_, right) = wake(Vec3::new(-3.0, -1.0, 0.5), v);
        assert!(left.y > 0.0 && right.y < 0.0);
        // Parked: only the old local pull.
        let (w, a) = wake(Vec3::new(0.5, 0.0, 0.0), Vec3::ZERO);
        assert!(w > 0.9 && a == Vec3::ZERO);
    }

    #[test]
    fn lamp_falloff_windows() {
        for modern in [false, true] {
            assert_eq!(lamp_falloff(100.0, 10.0, modern), 0.0, "{modern}: zero at the outer radius");
            assert!(lamp_falloff(1.0, 10.0, modern) > lamp_falloff(50.0, 10.0, modern));
            assert!(lamp_falloff(0.0, 10.0, modern) <= 1.6);
        }
        assert_eq!(lamp_falloff(0.0, 10.0, false), 1.0);
    }

    #[test]
    fn noise_volume_tiles_and_has_gradients() {
        let v = noise_volume();
        let n = NOISE;
        assert_eq!(v.len(), n * n * n * 4);
        // Periodic: the density across the edge continues.
        for p in [Vec3::new(0.0, 5.0, 9.0), Vec3::new(13.0, 0.0, 40.0), Vec3::new(3.0, 22.0, 0.0)] {
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                let a = billow(p);
                let b = billow(p + axis * n as f32);
                assert!((a - b).abs() < 1e-4, "{p} {axis}: {a} {b}");
            }
        }
        // Spread over the range, not flat.
        let r: Vec<u8> = v.chunks(4).map(|c| c[0]).collect();
        let (lo, hi) = (*r.iter().min().unwrap(), *r.iter().max().unwrap());
        assert!(lo < 5 && hi > 250, "{lo}..{hi}");
        let mean = r.iter().map(|&x| x as f32).sum::<f32>() / r.len() as f32;
        assert!((120.0..135.0).contains(&mean), "equalised: {mean}");
        // The stored gradient points uphill.
        let at = |x: usize, y: usize, z: usize| ((z % n * n + y % n) * n + x % n) * 4;
        let mut agree = 0;
        let mut total = 0;
        for i in 0..2000 {
            let (x, y, z) = ((i * 7) % n, (i * 13) % n, (i * 29) % n);
            let g = v[at(x, y, z) + 1] as i32 - 128;
            let d = v[at(x + 1, y, z)] as i32 - v[at(x + n - 1, y, z)] as i32;
            if d.abs() > 4 {
                total += 1;
                agree += i32::from(g.signum() == d.signum());
            }
        }
        assert!(agree * 10 >= total * 9, "{agree}/{total}");
    }

    /// The GPU spawn and update: births make billows and a haze puff
    /// each, they move, and all die after the haze's life.
    #[test]
    #[ignore = "requires a GPU adapter; run serially with --ignored"]
    fn gpu_spawn_update_and_draw() {
        use crate::particles::{Cubic, Description};
        let gpu = pollster::block_on(Gpu::new(
            None,
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        ))
        .unwrap();
        let mut d = Description { name: "XETT_RoadSk_SmkA".into(), ..Default::default() };
        d.life = [2.0, 0.3];
        d.drag = 0.4;
        d.gravity = -0.3;
        d.color = Cubic([[0.0; 4], [0.0; 4], [0.0, 0.0, 0.0, -120.0], [150.0, 150.0, 175.0, 120.0]]);
        d.size = Cubic([[0.0; 4], [0.0; 4], [6.0, 0.0, 0.0, 0.0], [0.5, 0.0, 0.0, 0.0]]);
        let lib = Library { descriptions: vec![d], templates: Vec::new() };
        let mut g = GpuSmoke2::new(&gpu, CINEMATIC, &lib, &Default::default());
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
            cars: vec![Some((Vec3::new(3.0, 0.5, 0.5), Vec3::new(15.0, 0.0, 0.0)))],
            time: 0.0,
            wet: 0.5,
        };
        let read = |g: &GpuSmoke2| {
            let size = g.params.pool as u64 * PUFF_SIZE;
            let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut e = gpu.device.create_command_encoder(&Default::default());
            e.copy_buffer_to_buffer(&g.pool, 0, &staging, 0, size);
            gpu.queue.submit([e.finish()]);
            staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let v: Vec<[f32; 20]> =
                bytemuck::cast_slice(&staging.slice(..).get_mapped_range().expect("mapped")).to_vec();
            v
        };
        let mut e = gpu.device.create_command_encoder(&Default::default());
        g.simulate(&gpu, &mut e, &[tick(births)]);
        gpu.queue.submit([e.finish()]);
        let pool = read(&g);
        let alive: Vec<_> = pool.iter().filter(|p| p[7] > 0.0).collect();
        assert_eq!(alive.len(), 10 * (CINEMATIC.billows as usize + 1));
        let haze = alive.iter().filter(|p| p[19] > 0.5).count();
        assert_eq!(haze, 10);
        // Wet (0.5): every puff carries its steam share.
        assert!(
            alive.iter().all(|p| (p[16] - 0.5).abs() < 1e-4),
            "steam share {:?}",
            alive.iter().map(|p| p[16]).take(4).collect::<Vec<_>>()
        );
        assert!(alive.iter().all(|p| p[7] <= CINEMATIC.haze_life * 1.2 && p[3] > 0.0));
        // Draw once into a small target (validation only).
        let view = View {
            camera: Camera::new(Vec3::new(-10.0, 0.0, 2.0), 0.0, 0.0),
            aspect: 1.0,
            reversed: false,
            pixels_per_metre: 500.0,
            lamps: vec![(Vec3::new(0.0, 0.0, 6.0), 12.0, Vec3::ONE)],
            cars: vec![],
            wet: 0.0,
            modern: false,
            neons: vec![],
            fog: None,
            points: vec![],
        };
        g.frame(&gpu, &view, 0.1);
        let mk = |format, usage| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let colour = mk(wgpu::TextureFormat::Rgba16Float, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let depth = mk(
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let mut e = gpu.device.create_command_encoder(&Default::default());
        // The volume built (splat, resolve, light) before the draw.
        if let Some((v, fv)) = g.volume.as_mut().zip(g.frame_view.take()) {
            let lights = crate::render::smoke_volume::scene_lights(
                &view.lamps,
                &[],
                &[],
                &[],
                view.camera.position,
                false,
                v.look(),
            );
            v.build(&gpu, &mut e, fv, &lights, g.written);
        }
        g.draw(
            &gpu,
            &mut e,
            &colour.create_view(&Default::default()),
            wgpu::TextureFormat::Rgba16Float,
            1,
            &depth.create_view(&Default::default()),
            1,
            None,
        );
        gpu.queue.submit([e.finish()]);
        let ticks: Vec<Tick> = (0..60 * 9).map(|_| tick(Vec::new())).collect();
        for chunk in ticks.chunks(MAX_TICKS) {
            let mut e = gpu.device.create_command_encoder(&Default::default());
            g.simulate(&gpu, &mut e, chunk);
            gpu.queue.submit([e.finish()]);
        }
        assert!(read(&g).iter().all(|p| p[7] == 0.0), "all puffs dead after 9 s");
    }
}
