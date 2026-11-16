//! Clone deviation, Modern + RT only (docs/perf-budget.md, the new
//! effects' costs): Modern Lighting's ray-traced shadows at half the scene's
//! resolution. Before, 2 + 1 + 1 + 1 rays per pixel were traced inside the world's
//! colour shader (`ml_ray`), which also held every world pixel's shader to
//! the ray queries' registers. Now a compute pass (`rt_shadow.wgsl`)
//! between the main view's depth pre-pass and its colour pass traces the
//! two strongest lights of each 2 × 2 pixel block (the area-light rays,
//! the block's quarter of the disc set) and the colour pass gathers the
//! 2 × 2 blocks around each pixel, weighted by its own plane and normal
//! (`ml_vis` in world.wgsl). Nothing native is involved (SPEED2.EXE has no
//! ray tracing); the Original path and every other tier are untouched.

use crate::render::Gpu;
use glam::{DMat4, DVec3, Mat4, Vec3};

/// The visibility blocks' format: (light + 1, visibility) × 2, view depth,
/// normal.
pub const VIS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Uint;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    inv: [[f32; 4]; 4],
    sizes: [f32; 4],
    misc: [f32; 4],
}

/// The compute module: world.wgsl's Frame / Lamp structs and its Modern
/// Lighting blocks marked `// BEGIN RTVIS …` (C: the rays), then rt_shadow.wgsl.
pub fn source(depth_ms: bool) -> String {
    let block = |name: &str| -> &str { crate::render::core::shader::registry::world_section(&format!("RTVIS {name}")) };
    let parts = ["FRAME", "LAMP", "A", "B", "C", "D"].map(block).concat();
    let own = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/rt_shadow.wgsl"))
        .replace("DEPTH_TEXTURE", if depth_ms { "texture_depth_multisampled_2d" } else { "texture_depth_2d" });
    format!("enable wgpu_ray_query;\n{parts}{own}")
}

/// The scene's size → the blocks' (half, rounded up).
pub fn blocks(size: (u32, u32)) -> (u32, u32) {
    (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1))
}

/// Clip → world position relative to `eye` for `view_proj` (in f64: the
/// world's coordinates reach thousands of metres).
pub fn inverse_relative(view_proj: Mat4, eye: Vec3) -> Mat4 {
    let m = view_proj.as_dmat4() * DMat4::from_translation(DVec3::new(eye.x as f64, eye.y as f64, eye.z as f64));
    m.inverse().as_mat4()
}

fn layout(device: &wgpu::Device, ms: bool) -> wgpu::BindGroupLayout {
    use wgpu::{BindGroupLayoutEntry as E, BindingType as B, ShaderStages as S};
    let c = S::COMPUTE;
    let storage = |binding| E {
        binding,
        visibility: c,
        ty: B::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let uniform = |binding| E {
        binding,
        visibility: c,
        ty: B::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
        label: Some("rt shadow"),
        entries: &[
            uniform(0),
            storage(1),
            storage(2),
            E { binding: 3, visibility: c, ty: B::AccelerationStructure { vertex_return: false }, count: None },
            E {
                binding: 4,
                visibility: c,
                ty: B::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: ms,
                },
                count: None,
            },
            E {
                binding: 5,
                visibility: c,
                ty: B::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: VIS_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            uniform(6),
        ],
    })
}

/// The world's group 0 entry the colour pass reads the blocks from
/// (Modern + RT's layout only).
pub fn layout_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 15,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Uint,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// A world renderer's pass: its pipelines (plain / multisampled depth),
/// uniform and blocks target.
pub struct RtShadow {
    layouts: [wgpu::BindGroupLayout; 2],
    pipelines: [Option<wgpu::ComputePipeline>; 2],
    uniform: wgpu::Buffer,
    vis: wgpu::TextureView,
    size: (u32, u32),
}

fn vis_target(device: &wgpu::Device, blocks: (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("rt shadow blocks"),
            size: wgpu::Extent3d { width: blocks.0, height: blocks.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: VIS_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// An empty 1 × 1 blocks target (every pixel keeps its fallback).
pub fn placeholder(gpu: &Gpu) -> wgpu::TextureView {
    static T: std::sync::Mutex<Option<wgpu::TextureView>> = std::sync::Mutex::new(None);
    T.lock().unwrap().get_or_insert_with(|| vis_target(&gpu.device, (1, 1))).clone()
}

impl RtShadow {
    pub fn new(gpu: &Gpu) -> RtShadow {
        let device = &gpu.device;
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("rt shadow"),
                size: std::mem::size_of::<Uniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        RtShadow {
            layouts: [layout(device, false), layout(device, true)],
            pipelines: [None, None],
            uniform,
            vis: vis_target(device, (1, 1)),
            size: (0, 0),
        }
    }

    /// The blocks the colour pass reads (a 1 × 1 empty target before the
    /// first sized frame: every pixel keeps its fallback).
    pub fn vis(&self) -> &wgpu::TextureView {
        &self.vis
    }

    /// This frame's values: the scene's `size`, the main view's
    /// `view_proj` and `eye`, the depth cleared to `clear`. The blocks are
    /// remade when the size changed (before the frame's group 0 binds them).
    pub fn prepare(&mut self, gpu: &Gpu, size: (u32, u32), view_proj: Mat4, eye: Vec3, clear: f32) {
        if size != self.size {
            self.vis = vis_target(&gpu.device, blocks(size));
            self.size = size;
        }
        let b = blocks(size);
        let u = Uniform {
            inv: inverse_relative(view_proj, eye).to_cols_array_2d(),
            sizes: [size.0 as f32, size.1 as f32, b.0 as f32, b.1 as f32],
            misc: [clear, 0.0, 0.0, 0.0],
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// The traced visibility's pipeline for a depth multisampled when
    /// `ms`, made now: a drive's warm-up (docs/first-use-builds.md).
    pub fn warm(&mut self, gpu: &Gpu, ms: bool) {
        self.pipeline(gpu, ms);
    }

    fn pipeline(&mut self, gpu: &Gpu, ms: bool) -> wgpu::ComputePipeline {
        let i = usize::from(ms);
        if self.pipelines[i].is_none() {
            let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("rt shadow"),
                source: wgpu::ShaderSource::Wgsl(source(ms).into()),
            });
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("rt shadow"),
                bind_group_layouts: &[Some(&self.layouts[i])],
                immediate_size: 0,
            });
            self.pipelines[i] = Some(crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
                label: Some("rt shadow"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some("cs_rt_vis"),
                compilation_options: Default::default(),
                cache: gpu.disk_pipeline_cache(),
            }));
        }
        self.pipelines[i].clone().unwrap()
    }

    /// What recording the pass needs (the depth view is bound then).
    pub fn plan(
        &mut self,
        gpu: &Gpu,
        samples: u32,
        frame: &wgpu::Buffer,
        lights: &wgpu::Buffer,
        clusters: &wgpu::Buffer,
        tlas: &wgpu::Tlas,
    ) -> Plan {
        let ms = samples > 1;
        Plan {
            device: gpu.device.clone(),
            pipeline: self.pipeline(gpu, ms),
            layout: self.layouts[usize::from(ms)].clone(),
            frame: frame.clone(),
            lights: lights.clone(),
            clusters: clusters.clone(),
            tlas: tlas.clone(),
            uniform: self.uniform.clone(),
            vis: self.vis.clone(),
            blocks: blocks(self.size),
            ms,
        }
    }
}

/// One frame's pass, recorded on a render thread after the pre-pass.
pub struct Plan {
    device: wgpu::Device,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    frame: wgpu::Buffer,
    lights: wgpu::Buffer,
    clusters: wgpu::Buffer,
    tlas: wgpu::Tlas,
    uniform: wgpu::Buffer,
    vis: wgpu::TextureView,
    blocks: (u32, u32),
    ms: bool,
}

impl Plan {
    /// The blocks from the pre-pass's `depth` (skipped when its sample
    /// count is not the planned one).
    pub fn record(&self, encoder: &mut wgpu::CommandEncoder, depth: &wgpu::TextureView) {
        if (depth.texture().sample_count() > 1) != self.ms {
            return;
        }
        use wgpu::BindingResource as R;
        let entry = |binding, resource| wgpu::BindGroupEntry { binding, resource };
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt shadow"),
            layout: &self.layout,
            entries: &[
                entry(0, self.frame.as_entire_binding()),
                entry(1, self.lights.as_entire_binding()),
                entry(2, self.clusters.as_entire_binding()),
                entry(3, R::AccelerationStructure(&self.tlas)),
                entry(4, R::TextureView(depth)),
                entry(5, R::TextureView(&self.vis)),
                entry(6, self.uniform.as_entire_binding()),
            ],
        });
        let mut pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("rt shadow"), timestamp_writes: None });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(self.blocks.0.div_ceil(8), self.blocks.1.div_ceil(8), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::naga;

    /// The compute module (both depth forms) validates with naga.
    #[test]
    fn module_validates() {
        for ms in [false, true] {
            let src = source(ms);
            let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|e| panic!("ms {ms}: {e:?}"));
        }
    }

    /// Clip → eye-relative world: a point projected and brought back.
    #[test]
    fn inverse_relative_round_trips() {
        let eye = Vec3::new(-1800.0, 2400.0, 35.0);
        let view = Mat4::look_at_rh(eye, eye + Vec3::new(0.3, 1.0, -0.1), Vec3::Z);
        let proj = Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.5, 4000.0);
        let vp = proj * view;
        let inv = inverse_relative(vp, eye);
        for p in [Vec3::new(-1790.0, 2430.0, 30.0), Vec3::new(-1700.0, 2900.0, 2.0)] {
            let c = vp * p.extend(1.0);
            let ndc = c / c.w;
            let r = inv * ndc;
            let back = r.truncate() / r.w + eye;
            assert!((back - p).length() < 0.05, "{back} vs {p}");
        }
    }

    /// The blocks cover the scene (odd sizes rounded up).
    #[test]
    fn blocks_cover() {
        assert_eq!(blocks((3840, 2160)), (1920, 1080));
        assert_eq!(blocks((2715, 1527)), (1358, 764));
        assert_eq!(blocks((0, 0)), (1, 1));
    }
}
