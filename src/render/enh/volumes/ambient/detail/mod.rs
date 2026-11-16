//! Ambient Detail (docs/ambient-detail.md; a **clone deviation**,
//! opt-in; nothing here exists in SPEED2.EXE and nothing was traced: it is
//! visual only, the simulation and every native system are untouched).
//!
//! Graphics → Effects → **Ambient Detail** (`enh.ambient_detail`):
//!
//! * **On** — the street props ([`crate::render::ambient_props`]): low-poly cans,
//!   bottles, cups, cardboard boxes, crumpled paper, leaves and newspaper,
//!   oil stains and kerb-side drain grates, placed deterministically on the
//!   pavements and in the gutters (never on the lanes or the racing line),
//!   drawn at the end of the main view's world pass (depth-tested and
//!   written: SSAO, contact shadows and the temporal pass take them like
//!   the world), lit like the road under them (world.efx: colour × baked v0
//!   × 2, fogged) plus the scene's direct lights: the nearest street lamps
//!   (the light packs' falloff or Modern Lighting's), each car's headlight
//!   cone and tail / brake lights.
//! * **Next-Gen** — also simulated, on the CPU (visual only, per drawn
//!   frame): leaves and paper **blown up by passing cars** (each car's bow
//!   push, side wash and trailing wake with its updraft) and by **gusts**
//!   (a slow noise wind), fluttering, tumbling, then **settling** on the
//!   road or pavement where they land (they stay moved); **grit** chips
//!   flicked up by the tyres; **moths** round the street lamps near the
//!   view. And through the volumetric module ([`crate::render::volume_fx`], its
//!   own grid and [`crate::render::volume_fx::ParticlePool`]): lit **tyre dust**
//!   behind the wheels on a dry road (much more on gravel) and **steam**
//!   rising from the drain grates and Bayview's road grills, drifting with
//!   the wind and stirred by the cars' wakes, lit by the lamps, headlights
//!   and tail lights with self-shadowing and multiple scattering,
//!   composited with the other particles (before the post chain; in the
//!   temporal pass's late path with TAA).

use glam::{Mat4, Quat, Vec3, Vec4};

use crate::render::Gpu;
use crate::render::ambient_props::{self as props, Inst, Kind, Mesh, Placed};
use crate::render::volume_fx::{self, Emit, Fields, ParticlePool, SceneLights, Spot, VolumeFx};

mod sim;
pub use sim::*;
mod draw;
pub use draw::*;
mod volume;
pub use volume::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn car_air_pushes_out_and_drags_behind() {
        let car = Vec3::ZERO;
        let v = Vec3::new(30.0, 0.0, 0.0);
        // Beside the body: pushed outward.
        let side = car_air(Vec3::new(0.0, 2.0, 0.0), car, v);
        assert!(side.y > 2.0, "{side}");
        // Behind: dragged along and lifted.
        let wake = car_air(Vec3::new(-5.0, 0.5, 0.0), car, v);
        assert!(wake.x > 3.0 && wake.z > 0.5, "{wake}");
        // Far away or a standing car: nothing.
        assert_eq!(car_air(Vec3::new(0.0, 20.0, 0.0), car, v).length() < 0.01, true);
        assert_eq!(car_air(Vec3::new(0.0, 1.5, 0.0), car, Vec3::new(1.0, 0.0, 0.0)), Vec3::ZERO);
    }

    #[test]
    fn wind_is_calm_mostly_and_gusts_sometimes() {
        let p = Vec3::new(100.0, -50.0, 0.0);
        let speeds: Vec<f32> = (0..2000).map(|i| wind(p, i as f32 * 0.25).length()).collect();
        let base = Fields::default().wind.length();
        let calm = speeds.iter().filter(|s| (**s - base).abs() < 0.2).count();
        let gusty = speeds.iter().filter(|s| **s > 2.5).count();
        assert!(calm > 800 && gusty > 20, "calm {calm} gusty {gusty}");
        assert_eq!(wind(p, 3.0), wind(p, 3.0));
    }

    /// A leaf hit by a car's wash lifts, flies and settles on the ground
    /// again; a leaf in still air stays.
    #[test]
    fn leaf_lifts_and_settles() {
        let flat = Some((0.0, Vec3::Z, false));
        let rest = (0.0, Vec3::Z);
        let mut l =
            Loose { pos: Vec3::ZERO, vel: Vec3::ZERO, rot: Quat::IDENTITY, spin: Vec3::ZERO, flying: false, calm: 0.0 };
        assert!(!step_loose(&mut l, Kind::Leaf, 0.5, Vec3::new(0.5, 0.0, 0.0), 0.0, 1.0 / 60.0, flat, rest));
        let mut top: f32 = 0.0;
        for k in 0..30 {
            step_loose(&mut l, Kind::Leaf, 0.5, Vec3::new(6.0, 0.0, 1.0), k as f32 / 60.0, 1.0 / 60.0, flat, rest);
            top = top.max(l.pos.z);
        }
        assert!(l.flying && top > 0.05 && l.pos.x > 0.1, "{l:?} top {top}");
        for k in 0..900 {
            step_loose(&mut l, Kind::Leaf, 0.5, Vec3::ZERO, k as f32 / 60.0, 1.0 / 60.0, flat, rest);
        }
        assert!(!l.flying && l.pos.z < 0.01 && l.pos.z >= 0.0, "{l:?}");
        assert!((l.rot * Vec3::Z).z > 0.99);
    }

    #[test]
    fn frustum_keeps_what_is_ahead() {
        let cam = crate::render::camera::Camera::looking_at(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0));
        let planes = frustum_planes(cam.view_projection(16.0 / 9.0));
        let inside = |p: Vec3| planes.iter().all(|pl| pl.truncate().dot(p) + pl.w > 0.0);
        assert!(inside(Vec3::new(10.0, 0.0, 0.0)));
        assert!(!inside(Vec3::new(-10.0, 0.0, 0.0)));
        assert!(!inside(Vec3::new(1.0, 30.0, 0.0)));
    }

    #[test]
    fn off_by_default() {
        assert_eq!(crate::qol::Enhancements::default().ambient_detail, 0);
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;

    /// The steam's pool exports live particles where the vent is (GPU).
    #[test]
    #[ignore = "requires a GPU; run explicitly"]
    fn steam_pool_exports() {
        let gpu = pollster::block_on(Gpu::new(
            None,
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        ))
        .unwrap();
        let fx = VolumeFx::new(&gpu, BUDGET, volume_fx::LOOK);
        let mut pool = ParticlePool::new(&gpu, 4096);
        let vent = Vec3::new(-302.9, -292.9, 22.9);
        for k in 0..15 {
            let emits: Vec<Emit> = (0..3)
                .map(|i| Emit {
                    pos_life: (vent + Vec3::X * i as f32 * 0.1).extend(5.0).to_array(),
                    vel_r: [0.0, 0.0, 1.0, 0.27],
                    albedo_sigma: [0.9, 0.9, 0.9, 2.9],
                    heat: [0.55, 0.25, 0.6, 0.6],
                })
                .collect();
            pool.step(
                0.3,
                emits,
                Fields {
                    wind: wind(vent, k as f32),
                    turbulence: 1.4,
                    drag: 0.9,
                    buoyancy: 1.6,
                    cooling: 0.35,
                    glow: 0.0,
                    cars: Vec::new(),
                },
            );
        }
        let mut e = gpu.device.create_command_encoder(&Default::default());
        let n = pool.export(&gpu, &mut e, &fx, 0);
        let size = n as u64 * 80;
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        e.copy_buffer_to_buffer(fx.particles(), 0, &read, 0, size);
        gpu.queue.submit([e.finish()]);
        read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = read.slice(..).get_mapped_range().unwrap();
        let ps: &[volume_fx::Particle] = bytemuck::cast_slice(&data);
        let live: Vec<_> = ps.iter().filter(|p| p.pos_r[3] > 0.0).collect();
        for p in &live {
            eprintln!(
                "{:?} r {:.2} sigma {:.3} t {:.2}",
                (Vec3::from_slice(&p.pos_r[..3]) - vent).to_array(),
                p.pos_r[3],
                p.albedo_sigma[3],
                p.misc[0]
            );
        }
        assert!(live.len() > 20);
        assert!(live.iter().all(|p| Vec3::from_slice(&p.pos_r[..3]).distance(vent) < 20.0 && p.albedo_sigma[3] > 0.0));
    }
}
