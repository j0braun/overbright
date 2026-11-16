//! Particles Cinematic, **volumetric** smoke (docs/smoke-volumetric.md;
//! a **clone deviation**, opt-in; nothing here exists in SPEED2.EXE). The
//! billboards read as cheap and dated, hence the volume. The
//! smoke-specific half over [`crate::render::volume_fx`]: the Cinematic puffs
//! (`smoke_cinematic`'s pool: the native smoke births, moved by the noise
//! flow, the wind and the cars' wakes) become volume particles on the GPU
//! (smoke_volume.wgsl `cs_smoke_particles`: the billboards' size and opacity, the
//! native colour as albedo); the lights are the street lamps, the cars'
//! headlights, tail lights and underglows ([`scene_lights`]).

use crate::render::Gpu;
use crate::render::volume_fx::{self, Lamp, SceneLights, Spot, VolumeFx};
use glam::{Mat4, Vec3};

/// The smoke's constants (inferred, by eye; no native reference exists).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// A puff's opacity (the billboard's) × this before it becomes an
    /// optical depth.
    pub opacity: f32,
    /// The same for the faint, long-lived haze puffs.
    pub haze: f32,
    /// A puff's radius against the billboard's half size.
    pub size: f32,
    /// Steam (wet road): its extinction × (1 − this × wetness).
    pub steam: f32,
    /// Steam's evaporation rate (1/s): its density × e^(−this · age).
    pub evaporate: f32,
    /// The albedo's floor (a smoke grey).
    pub grey: f32,
    /// The tail lights' and underglows' gains (against the native colours).
    pub tail: f32,
    pub neon: f32,
    /// How much of the native colour's hue the albedo keeps (0 a
    /// neutral grey; the native road smoke is a lavender 150, 150, 175 that
    /// stood in for the night's light on a flat sprite — the lit volume
    /// takes its colour from the scene's lights instead).
    pub tint: f32,
    /// Dissipation — a growing puff's optical depth × growth^−(this × its age fraction)
    /// (1: its mass spreads over its cross-section, so an old puff thins
    /// into haze instead of piling up into a wall).
    pub spread: f32,
    /// The renderer's shading.
    pub fx: volume_fx::Look,
}

/// Haze 0.6 → 0.3, size 1.15 → 0.95, spread 2.5 → 3.5 (a long
/// burnout built a flat wall over the whole view: +20 mean, blacks +10,
/// contrast −5 against Original; now dense young billows and old smoke
/// that thins out, docs/smoke-nextgen.md).
pub const LOOK: Look = Look {
    opacity: 3.0,
    haze: 0.3,
    size: 0.95,
    steam: 0.6,
    evaporate: 1.2,
    grey: 0.78,
    tail: 0.8,
    neon: 1.6,
    tint: 0.12,
    spread: 3.5,
    fx: FX,
};

/// The renderer's shading for the smoke: the fine billow octave and
/// the billow shading turned toward the lamps, the temporal resolve with
/// 1.4× longer march steps (docs/smoke-nextgen.md).
pub const FX: volume_fx::Look = volume_fx::Look {
    fine: 1.0,
    directional: 0.7,
    shade: 0.6,
    sharp: 2.0,
    temporal: 0.85,
    step: 1.4,
    ..volume_fx::LOOK
};

/// An underglow light's reach (m).
pub const NEON_RADIUS: f32 = 2.8;

/// `NFSU2_SMOKE3_LOOK=k=v,…` overrides [`LOOK`]'s fields by name (tuning
/// and comparisons only).
pub fn look() -> Look {
    static L: std::sync::OnceLock<Look> = std::sync::OnceLock::new();
    *L.get_or_init(|| {
        let mut l = LOOK;
        for kv in crate::render::debug_env::nfsu2_smoke3_look().split(',') {
            let Some((k, v)) = kv.split_once('=') else { continue };
            let Ok(v) = v.trim().parse::<f32>() else { continue };
            match k.trim() {
                "opacity" => l.opacity = v,
                "haze" => l.haze = v,
                "size" => l.size = v,
                "steam" => l.steam = v,
                "evaporate" => l.evaporate = v,
                "grey" => l.grey = v,
                "tail" => l.tail = v,
                "neon" => l.neon = v,
                "tint" => l.tint = v,
                "spread" => l.spread = v,
                "scene" => l.fx.scene = v,
                "near" => l.fx.near = v,
                "shadow" => l.fx.shadow = v,
                "ambient" => l.fx.ambient = Vec3::splat(v),
                "g" => l.fx.g = v,
                "erosion" => l.fx.erosion = v,
                "detail" => l.fx.detail = v,
                "shade" => l.fx.shade = v,
                "sharp" => l.fx.sharp = v,
                "lamp" => l.fx.lamp = v,
                "head" => l.fx.spot = v,
                "fine" => l.fx.fine = v,
                "dir" => l.fx.directional = v,
                "temporal" => l.fx.temporal = v,
                "step" => l.fx.step = v,
                _ => {}
            }
        }
        l
    })
}

/// The smoke's lights: the street lamps; per car within 80 m its
/// headlights (a cone ahead, the car's pool colour) and tail lights; the
/// underglows (two lights under the sills, `neons`: car matrix and linear
/// R, G, B × alpha; ground lights, held under the car's body — a
/// coloured glow in the low smoke round the car, never the whole cloud);
/// `points`: any further point lights of the scene (the way in for
/// e.g. Modern Lighting's lights).
pub fn scene_lights(
    lamps: &[Lamp],
    cars: &[(Mat4, [u8; 3])],
    neons: &[(Mat4, Vec3)],
    points: &[Lamp],
    eye: Vec3,
    modern: bool,
    look: &Look,
) -> SceneLights {
    let mut l = SceneLights { lamps: lamps.to_vec(), modern, points: points.to_vec(), ..Default::default() };
    let tail = [Vec3::new(-2.1, 0.6, 0.85), Vec3::new(-2.1, -0.6, 0.85)];
    for (m, colour) in cars {
        let at = m.w_axis.truncate();
        if at.distance(eye) > 80.0 {
            continue;
        }
        let forward = m.x_axis.truncate().normalize_or(Vec3::X);
        let up = m.z_axis.truncate().normalize_or(Vec3::Z);
        let c = colour.map(|b| (f32::from(b) / 128.0).min(1.0));
        l.spots.push(Spot {
            position: at + forward * 2.0 + up * 0.7,
            reach: 28.0,
            direction: (forward - Vec3::Z * 0.1).normalize_or(forward),
            cos_outer: 0.82,
            cos_inner: 0.94,
            colour: Vec3::from(c),
        });
        for t in tail {
            l.points.push((
                m.transform_point3(t),
                crate::render::gpu_particles::TAIL_RADIUS,
                crate::render::gpu_particles::TAIL_COLOUR * look.tail,
            ));
        }
    }
    for (m, c) in neons {
        if m.w_axis.truncate().distance(eye) > 60.0 {
            continue;
        }
        for x in [-1.1f32, 1.1] {
            // (A negative radius: a ground light, volume_fx `SceneLights::points`.)
            l.points.push((m.transform_point3(Vec3::new(x, 0.0, 0.1)), -NEON_RADIUS, *c * look.neon));
        }
    }
    l
}

fn e(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource: buffer.as_entire_binding() }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SmokeUniform {
    counts: [u32; 4],
    look2: [f32; 4],
    look: [f32; 4],
}

/// The renderer and the puff → particle pass.
pub struct SmokeVolume {
    fx: VolumeFx,
    look: Look,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    convert: wgpu::ComputePipeline,
}

impl SmokeVolume {
    /// Over the Cinematic pool and its tables (`pool`, `descs`, `colors`).
    pub fn new(gpu: &Gpu, pool: &wgpu::Buffer, descs: &wgpu::Buffer, colors: &wgpu::Buffer) -> SmokeVolume {
        let look = look();
        let fx = VolumeFx::new(gpu, volume_fx::BUDGET_HIGH, look.fx);
        let device = &gpu.device;
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("smoke3"),
                size: std::mem::size_of::<SmokeUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let cs = wgpu::ShaderStages::COMPUTE;
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: cs,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("smoke3"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: cs,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, true),
                storage(3, true),
                storage(4, false),
            ],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smoke3"),
            layout: &layout,
            entries: &[e(0, &uniform), e(1, pool), e(2, descs), e(3, colors), e(4, fx.particles())],
        });
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("smoke3"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/smoke_volume.wgsl")).into(),
            ),
        });
        let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("smoke3"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let convert = crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
            label: Some("smoke3 particles"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("cs_smoke_particles"),
            compilation_options: Default::default(),
            cache: gpu.disk_pipeline_cache(),
        });
        SmokeVolume { fx, look, uniform, group, convert }
    }

    pub fn look(&self) -> &Look {
        &self.look
    }

    /// The volume's medium ([`VolumeFx::medium`]).
    pub fn medium(&self) -> Option<(wgpu::TextureView, glam::Vec3, f32, [u32; 3])> {
        self.fx.medium()
    }

    /// The frame's volume: the first `puffs` pool entries as particles,
    /// then the renderer's build.
    pub fn build(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        view: volume_fx::View,
        lights: &SceneLights,
        puffs: u32,
    ) {
        let puffs = puffs.min(self.fx.budget().particles);
        let l = self.look;
        gpu.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&SmokeUniform {
                counts: [puffs, 0, 0, 0],
                look2: [l.haze, l.evaporate, l.tint.clamp(0.0, 1.0), l.spread.max(0.0)],
                look: [l.opacity, l.size, l.steam, l.grey],
            }),
        );
        if puffs > 0 {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("smoke3 particles"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.convert);
            pass.set_bind_group(0, &self.group, &[]);
            pass.dispatch_workgroups(puffs.div_ceil(64), 1, 1);
        }
        self.fx.frame(gpu, view, lights);
        self.fx.build(gpu, encoder, puffs);
    }

    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32) {
        self.fx.warm(gpu, format, samples, depth_samples);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        viewport: Option<[f32; 4]>,
    ) {
        self.fx.draw(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_lights() {
        let car = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
        let far = Mat4::from_translation(Vec3::new(500.0, 0.0, 0.0));
        let l = scene_lights(
            &[(Vec3::Z * 6.0, 12.0, Vec3::ONE)],
            &[(car, [0x90, 0xAA, 0x90]), (far, [255; 3])],
            &[(car, Vec3::new(1.0, 0.0, 1.0))],
            &[],
            Vec3::ZERO,
            false,
            &LOOK,
        );
        assert_eq!(l.lamps.len(), 1);
        // The near car only: one headlight cone ahead, two tail lights, two
        // underglow lights under it.
        assert_eq!(l.spots.len(), 1);
        assert!(l.spots[0].position.x > 5.0 && l.spots[0].direction.x > 0.9);
        assert_eq!(l.points.len(), 4);
        assert!(l.points[..2].iter().all(|p| p.0.x < 5.0), "tail lights behind");
        assert!(
            l.points[2..].iter().all(|p| p.0.z < 0.5 && p.2.x > 1.0 && p.1 < 0.0),
            "underglow under the car, a ground light"
        );
        // The colour goes through as given (R, G, B), only scaled.
        assert!(l.points[2..].iter().all(|p| p.2.y == 0.0 && (p.2.x - p.2.z).abs() < 1e-6));
        // Further scene lights come first, kept as given.
        let extra = scene_lights(&[], &[], &[], &[(Vec3::X, 4.0, Vec3::new(0.0, 0.0, 2.0))], Vec3::ZERO, true, &LOOK);
        assert_eq!(extra.points, vec![(Vec3::X, 4.0, Vec3::new(0.0, 0.0, 2.0))]);
    }

    #[test]
    fn look_overrides() {
        // Without the variable: the constants.
        if !crate::render::debug_env::nfsu2_smoke3_look_set() {
            assert_eq!(look(), LOOK);
        }
        assert!(LOOK.opacity > 1.0 && LOOK.size > 0.8 && LOOK.steam < 1.0);
        // The smoke takes the fine octave, the directional shading
        // and the temporal resolve (the longer steps need it); old smoke
        // thins out faster than before (no wall).
        assert!(LOOK.fx.fine > 0.0 && LOOK.fx.directional > 0.0 && LOOK.fx.temporal > 0.5 && LOOK.fx.step > 1.0);
        assert!(LOOK.spread > 2.5 && LOOK.haze < 0.6);
    }
}
