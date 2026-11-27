//! Volumetric effects (docs/notes/graphics/smoke.md "Volumetric effects and the volumetric Cinematic smoke"; a **clone
//! deviation**, opt-in; nothing here exists in SPEED2.EXE): a reusable
//! renderer for smoke, steam, spray, mist and fire, and a GPU particle
//! pool to drive it. Review 2026-11-05: enhanced effects must be simulated,
//! volumetric and lit by the scene (the "next-gen" bar); first used by the
//! Cinematic smoke (`smoke_volume`).
//!
//! **Renderer** ([`VolumeFx`]). Producers write [`Particle`]s (a position,
//! radius, albedo, extinction, emission, lumpiness, age, thinness) into
//! its particle buffer, from the CPU ([`VolumeFx::write_particles`]) or a
//! GPU pass ([`VolumeFx::particles`], e.g. [`ParticlePool::export`]); each
//! frame [`VolumeFx::build`]
//!
//! 1. **splats** them as lumpy density balls (their own noise, turning
//!    with them) into a world-aligned voxel grid around the view
//!    ([`Budget`]; snapped to the world so it never swims), with
//!    fixed-point atomics (density, thinness, age, albedo, emission);
//! 2. **resolves** the accumulators into density, albedo and emission
//!    volumes, only over the region particles touched this frame or the
//!    last, marking the occupied 8³ bricks (empty-space skipping) and
//!    their bounds;
//! 3. **lights** a half-resolution light volume: single scattering from
//!    the [`SceneLights`] (street lamps with the light packs' or Modern
//!    Lighting's falloff, spot lights such as headlights, point lights
//!    such as tail lights and underglows) with a Henyey–Greenstein phase
//!    toward the eye (halos when looking into a light through the medium),
//!    **self-shadowed** by marching the density toward each light, plus
//!    the sky from above occluded by the medium over it; a
//!    multiple-scattering approximation ([`multi_scatter`]) keeps thick
//!    media bright and soft inside; plus the emission;
//!
//! and [`VolumeFx::draw`] **ray-marches** it at a reduced resolution
//! ([`march_div`]) to the scene's depth (soft against every surface),
//! eroding thin parts with drifting detail noise and shading each billow
//! (the eroded density a step up), then composites it with a depth-aware
//! upsample. Into the scene before the post chain: exposure, grade, HDR
//! and the temporal pass's late path (after TAA, no ghosting) apply.
//!
//! **Simulation** ([`ParticlePool`]): emitters push [`Emit`]s (position,
//! velocity, radius, growth, albedo, extinction, temperature, life); a GPU
//! ring advances them by [`Fields`] (drag, buoyancy from the temperature,
//! which cools and glows, divergence-free turbulence, wind, car wakes) and
//! exports them as [`Particle`]s.

use crate::render::Gpu;
use glam::{Mat4, Vec3};

mod types;
pub use types::*;
mod math;
pub use math::*;
mod programs;
use programs::*;
mod renderer;
pub use renderer::*;
mod pool;
pub use pool::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_is_snapped_and_ahead() {
        let b = BUDGET_HIGH;
        let eye = Vec3::new(1003.37, -2207.91, 12.6);
        let o = grid_origin(b, eye, Vec3::new(0.0, 1.0, -0.2));
        for c in o.to_array() {
            let k = c / b.voxel;
            assert!((k - k.round()).abs() < 1e-3, "{c}: whole voxels in world space (no swimming)");
        }
        let ext = Vec3::new(b.dims[0] as f32, b.dims[1] as f32, b.dims[2] as f32) * b.voxel;
        assert!(eye.y - o.y < 20.0 && o.y + ext.y - eye.y > 40.0, "more ahead than behind");
        assert!((eye.x - o.x - ext.x * 0.5).abs() < 1.0);
        assert!(eye.z - o.z >= BELOW - 0.01 && eye.z - o.z < BELOW + b.voxel + 0.01);
        assert_eq!(
            grid_origin(b, eye + Vec3::new(0.0, 0.0, 0.01), Vec3::new(0.0, 1.0, -0.2)),
            o,
            "a small move keeps the grid"
        );
    }

    #[test]
    fn phase_and_multiple_scattering() {
        for g in [0.0f32, 0.4, 0.75] {
            let n = 2000;
            let s: f32 = (0..n)
                .map(|i| hg(g, -1.0 + 2.0 * (i as f32 + 0.5) / n as f32) * 2.0 * std::f32::consts::PI * 2.0 / n as f32)
                .sum();
            assert!((s - 1.0).abs() < 0.01, "{g}: {s}");
        }
        assert!(hg(0.6, 1.0) > 10.0 * hg(0.6, -1.0), "forward scattering");
        let single = (-6.0f32).exp() * hg(0.5, 0.0) * 2.0 * std::f32::consts::PI;
        assert!(multi_scatter(6.0, 0.0, 0.5) > 20.0 * single, "thick media stay lit");
        assert!(multi_scatter(0.0, 1.0, 0.5) > multi_scatter(0.0, -1.0, 0.5));
        assert!(multi_scatter(0.0, 0.0, 0.5) > multi_scatter(2.0, 0.0, 0.5));
    }

    #[test]
    fn opacity_to_extinction() {
        // The chord through the centre gives back the opacity.
        for (a, r) in [(0.5f32, 1.0f32), (0.9, 0.3), (0.2, 2.5)] {
            let s = sigma_for_opacity(a, r);
            assert!(((1.0 - (-s * 0.9 * r).exp()) - a).abs() < 1e-4);
        }
        assert!(sigma_for_opacity(1.0, 1.0).is_finite());
    }

    #[test]
    fn lights_nearest_and_capped() {
        let eye = Vec3::new(10.0, 0.0, 0.0);
        let lamps: Vec<Lamp> = (0..100).map(|i| (Vec3::new(i as f32, 0.0, 5.0), 10.0, Vec3::ONE)).collect();
        let u = lights_uniform(&SceneLights { lamps, modern: true, ..Default::default() }, eye);
        assert_eq!(u.counts, [MAX_LAMPS as u32, 0, 0, 1]);
        // Eye-relative, the nearest first.
        assert_eq!(u.lamps[0][..3], [0.0, 0.0, 5.0]);
        assert!(u.lamps.chunks(2).take(MAX_LAMPS).all(|l| (l[0][0]).abs() <= 54.0));
    }

    #[test]
    fn wake_trails_behind_the_car() {
        let v = Vec3::new(20.0, 0.0, 0.0);
        let (behind, air) = wake(Vec3::new(-3.0, 0.0, 0.5), v);
        let (ahead, _) = wake(Vec3::new(4.0, 0.0, 0.5), v);
        assert!(behind > 0.5 && ahead < 0.01);
        assert!(air.x > 5.0 && air.x < 20.0 && air.z > 0.0);
    }

    #[test]
    fn march_tuning_defaults_are_inert() {
        assert_eq!(march_tuning(1.0, 1.0), [0.0, 0.0]);
        assert_eq!(march_tuning(0.5, 0.25), [0.5, 1.25]);
        assert_eq!(march_tuning(0.5, 0.0), [0.5, 1.0]);
    }

    #[test]
    fn march_divisor() {
        if crate::render::debug_env::nfsu2_vfx_div_set() {
            return;
        }
        assert_eq!((march_div(1080), march_div(1440), march_div(2160), march_div(4320)), (2, 2, 3, 4));
        assert!(bilateral(10.0, 20.0) < 0.01 && (bilateral(20.0, 20.0) - 1.0).abs() < 1e-3);
    }

    /// Every user's look takes a march whose override set the warm-up makes ([`WARM_LOOKS`]): no march is
    /// made in a drawn frame. And the smoke's set is the march with every branch in.
    #[test]
    fn march_override_sets_are_warmed() {
        let warmed: Vec<_> = WARM_LOOKS.iter().map(march_constants).collect();
        for look in [
            LOOK,
            Look { scene: 2.2, ..LOOK },
            crate::render::smoke_volume::FX,
            crate::render::wet_spray::LOOK,
            crate::render::water_ng::MIST_LOOK,
        ] {
            assert!(warmed.contains(&march_constants(&look)), "{look:?}");
        }
        assert_eq!(march_constants(&crate::render::smoke_volume::FX).map(|c| c.1), [1.0; 3]);
    }

    #[test]
    fn particle_layouts() {
        assert_eq!(std::mem::size_of::<Particle>(), 80);
        assert_eq!(std::mem::size_of::<Emit>(), 64);
        assert!(std::mem::size_of::<FieldsUniform>() as u64 <= FIELDS_STRIDE);
        let b = empty_bounds();
        assert!(b[0] > b[3] && b[12] > b[15], "empty bounds: min > max");
    }

    /// The pool and the renderer on a GPU: emits spawn, rise when hot,
    /// export, splat, light and draw without validation errors.
    #[test]
    #[ignore = "requires a GPU adapter; run serially with --ignored"]
    fn gpu_pool_and_render() {
        let gpu = pollster::block_on(Gpu::new(
            None,
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        ))
        .unwrap();
        let mut fx = VolumeFx::new(&gpu, Budget { dims: [64, 64, 32], voxel: 0.4, particles: 4096, steps: 64 }, LOOK);
        let mut pool = ParticlePool::new(&gpu, 1024);
        let emits: Vec<Emit> = (0..100)
            .map(|i| Emit {
                pos_life: [i as f32 * 0.1, 5.0, 1.0, 3.0],
                vel_r: [0.0, 0.0, 0.5, 0.6],
                albedo_sigma: [0.8, 0.8, 0.8, 3.0],
                heat: [0.8, 0.3, 0.7, 0.0],
            })
            .collect();
        pool.step(
            1.0 / 60.0,
            emits,
            Fields { cars: vec![(Vec3::new(2.0, 5.0, 0.5), Vec3::new(10.0, 0.0, 0.0))], ..Default::default() },
        );
        for _ in 0..30 {
            pool.step(1.0 / 60.0, Vec::new(), Fields::default());
        }
        let mut e = gpu.device.create_command_encoder(&Default::default());
        let n = pool.export(&gpu, &mut e, &fx, 16);
        assert_eq!(n, 1024);
        fx.write_particles(
            &gpu,
            0,
            &[Particle {
                pos_r: [0.0, 4.0, 1.0, 1.0],
                albedo_sigma: [0.9, 0.9, 0.9, 2.0],
                emit_lump: [0.0, 0.0, 0.0, 1.0],
                ..Default::default()
            }],
        );
        let cam = crate::render::camera::Camera::new(Vec3::new(0.0, -5.0, 2.0), 0.0, 0.0);
        let view = View {
            eye: cam.position,
            forward: cam.forward(),
            view_proj: cam.projection_depth(1.0, false) * Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up),
            near: cam.near,
            far: cam.far,
            reversed: false,
            time: 0.5,
            fog: None,
        };
        fx.frame(
            &gpu,
            view,
            &SceneLights {
                lamps: vec![(Vec3::new(0.0, 5.0, 6.0), 12.0, Vec3::ONE)],
                points: vec![(Vec3::new(0.0, 4.0, 0.7), 2.5, Vec3::X)],
                ..Default::default()
            },
        );
        fx.build(&gpu, &mut e, 16 + n);
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
        fx.draw(
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
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

        // A readable target: the scene ambient is its mean (a flat
        // colour here), with the native fog and a ground light (an
        // underglow) — and no validation error.
        let readable = mk(
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let rv = readable.create_view(&Default::default());
        let mut e = gpu.device.create_command_encoder(&Default::default());
        {
            let att = wgpu::RenderPassColorAttachment {
                view: &rv,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.5, g: 0.25, b: 0.125, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            };
            e.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(att)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        fx.frame(
            &gpu,
            View { fog: Some(([250.0, 1.0 / 200.0, 0.4], [0.2, 0.1, 0.3])), ..view },
            &SceneLights {
                points: vec![(Vec3::new(0.0, 4.0, 0.1), -2.5, Vec3::new(1.0, 0.8, 0.0))],
                ..Default::default()
            },
        );
        fx.build(&gpu, &mut e, 16 + n);
        fx.draw(
            &gpu,
            &mut e,
            &rv,
            wgpu::TextureFormat::Rgba16Float,
            1,
            &depth.create_view(&Default::default()),
            1,
            None,
        );
        let size = ((AMBIENT_W * AMBIENT_H + 1) * 16) as u64;
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        e.copy_buffer_to_buffer(&fx.ambient, 0, &read, 0, size);
        gpu.queue.submit([e.finish()]);
        read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let v: Vec<f32> = bytemuck::cast_slice(&read.slice(..).get_mapped_range().unwrap()).to_vec();
        let mean = &v[(AMBIENT_W * AMBIENT_H * 4) as usize..];
        assert!(
            (mean[0] - 0.5).abs() < 0.01
                && (mean[1] - 0.25).abs() < 0.01
                && (mean[2] - 0.125).abs() < 0.01
                && mean[3] == 1.0,
            "{mean:?}"
        );

        // The smoke's look (fine octave, directional shading, the
        // temporal resolve, longer steps): three frames, the history
        // ping-pongs and the view is kept for the next reprojection.
        let mut fx = VolumeFx::new(
            &gpu,
            Budget { dims: [64, 64, 32], voxel: 0.4, particles: 4096, steps: 64 },
            Look { fine: 1.0, directional: 0.7, temporal: 0.85, step: 1.4, ..LOOK },
        );
        for k in 0..3 {
            let mut e = gpu.device.create_command_encoder(&Default::default());
            fx.write_particles(
                &gpu,
                0,
                &[Particle {
                    pos_r: [0.0, 4.0, 1.0, 1.5],
                    albedo_sigma: [0.9, 0.9, 0.9, 2.0],
                    emit_lump: [0.0, 0.0, 0.0, 1.0],
                    ..Default::default()
                }],
            );
            fx.frame(
                &gpu,
                View { eye: view.eye + Vec3::X * 0.2 * k as f32, ..view },
                &SceneLights { lamps: vec![(Vec3::new(3.0, 5.0, 6.0), 12.0, Vec3::ONE)], ..Default::default() },
            );
            fx.build(&gpu, &mut e, 1);
            fx.draw(
                &gpu,
                &mut e,
                &rv,
                wgpu::TextureFormat::Rgba16Float,
                1,
                &depth.create_view(&Default::default()),
                1,
                None,
            );
            gpu.queue.submit([e.finish()]);
        }
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let t = fx.targets.lock().unwrap();
        let t = t.as_ref().unwrap();
        assert_eq!(t.frame, 3);
        assert!(t.prev.is_some_and(|p| (p.1 - (view.eye + Vec3::X * 0.4)).length() < 1e-5));
    }

    /// The new knobs are off for the other users (spray, mist keep
    /// the earlier march), the uniform's tail stays aligned, the temporal
    /// resolve has its own shader section.
    #[test]
    fn g10d_knobs_and_layout() {
        assert_eq!((LOOK.fine, LOOK.directional, LOOK.temporal, LOOK.step), (0.0, 0.0, 0.0, 1.0));
        for l in [crate::render::wet_spray::LOOK, crate::render::water_ng::MIST_LOOK] {
            assert_eq!((l.fine, l.directional, l.temporal, l.step), (0.0, 0.0, 0.0, 1.0));
        }
        assert_eq!(std::mem::offset_of!(VolUniform, prev_vp) % 16, 0);
        assert_eq!(std::mem::offset_of!(VolUniform, temporal), std::mem::offset_of!(VolUniform, prev_vp) + 64 + 32);
        let s = sections();
        assert!(s[6].contains("fn fs_temporal") && !s[4].contains("fn fs_temporal") && s[4].contains("fn fs_march"));
    }

    /// The integration constants: scene ambient on, a near fade, a
    /// partial surface shadow; the View carries the native fog.
    #[test]
    fn integration_look() {
        assert!(LOOK.scene > 0.0 && LOOK.near > 0.3 && (0.0..1.0).contains(&LOOK.shadow) && LOOK.shade < 0.5);
        assert!(std::mem::size_of::<VolUniform>() % 16 == 0);
        assert_eq!(std::mem::offset_of!(VolUniform, look5) % 16, 0);
    }
}
