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

/// The blocks for a scene of `size` shown at `screen`, and the scene
/// pixels per grid pixel. A supersampled scene (SSAA, Render Scale above
/// 100 %: larger than the screen both ways) takes one block per 2 × 2
/// screen pixels, the block sampling the scene pixel under its first screen
/// pixel: the soft shadows are a smooth signal, and the scene's extra
/// pixels added rays without adding shadow detail the screen shows (each
/// pixel still gathers its own blocks on its own plane). Otherwise one per
/// 2 × 2 scene pixels and a scale of 1 (the grid as before, bit for bit).
pub fn grid(size: (u32, u32), screen: (u32, u32)) -> ((u32, u32), [f32; 2]) {
    if screen.0 > 0 && screen.1 > 0 && screen.0 < size.0 && screen.1 < size.1 {
        (blocks(screen), [size.0 as f32 / screen.0 as f32, size.1 as f32 / screen.1 as f32])
    } else {
        (blocks(size), [1.0, 1.0])
    }
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
    screen: (u32, u32),
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

/// The pass's uniform, its grid scale 1 until the first `prepare` (the
/// colour pass divides by it).
fn uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let u = Uniform { inv: [[0.0; 4]; 4], sizes: [0.0; 4], misc: [0.0, 1.0, 1.0, 0.0] };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rt shadow"),
        contents: bytemuck::bytes_of(&u),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

/// The world's group 0 entry the colour pass reads the grid's scale from
/// (the pass's uniform; Modern + RT's layout only).
pub fn layout_entry_uniform() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 16,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

/// A uniform with the grid's scale 1 (no traced frame yet).
pub fn placeholder_uniform(gpu: &Gpu) -> wgpu::Buffer {
    static T: std::sync::Mutex<Option<wgpu::Buffer>> = std::sync::Mutex::new(None);
    T.lock().unwrap().get_or_insert_with(|| uniform_buffer(&gpu.device)).clone()
}

/// An empty 1 × 1 blocks target (every pixel keeps its fallback).
pub fn placeholder(gpu: &Gpu) -> wgpu::TextureView {
    static T: std::sync::Mutex<Option<wgpu::TextureView>> = std::sync::Mutex::new(None);
    T.lock().unwrap().get_or_insert_with(|| vis_target(&gpu.device, (1, 1))).clone()
}

impl RtShadow {
    pub fn new(gpu: &Gpu) -> RtShadow {
        let device = &gpu.device;
        let uniform = uniform_buffer(device);
        RtShadow {
            layouts: [layout(device, false), layout(device, true)],
            pipelines: [None, None],
            uniform,
            vis: vis_target(device, (1, 1)),
            size: (0, 0),
            screen: (0, 0),
        }
    }

    /// The blocks the colour pass reads (a 1 × 1 empty target before the
    /// first sized frame: every pixel keeps its fallback).
    pub fn vis(&self) -> &wgpu::TextureView {
        &self.vis
    }

    /// The uniform the colour pass reads too (the grid's scale, `mlrt.wgsl`
    /// `ml_vis_grid`).
    pub fn uniform(&self) -> &wgpu::Buffer {
        &self.uniform
    }

    /// This frame's values: the scene's `size` shown at `screen`, the main
    /// view's `view_proj` and `eye`, the depth cleared to `clear`. The blocks
    /// are remade when their grid changed (before the frame's group 0 binds
    /// them).
    pub fn prepare(&mut self, gpu: &Gpu, size: (u32, u32), screen: (u32, u32), view_proj: Mat4, eye: Vec3, clear: f32) {
        let (b, scale) = grid(size, screen);
        if (size, screen) != (self.size, self.screen) {
            self.vis = vis_target(&gpu.device, b);
            self.size = size;
            self.screen = screen;
        }
        let u = Uniform {
            inv: inverse_relative(view_proj, eye).to_cols_array_2d(),
            sizes: [size.0 as f32, size.1 as f32, b.0 as f32, b.1 as f32],
            misc: [clear, scale[0], scale[1], 0.0],
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
            blocks: grid(self.size, self.screen).0,
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

    /// Modern + RT's colour functions take the traced blocks first
    /// (`ml_vis_early`, before Surface Detail and the lighting) and the light
    /// loop reads only its results: the gather's sums are not held across
    /// the loop, where they took registers (waves) from every world pixel
    /// (docs/shadow-softness.md, "The gather first").
    #[test]
    fn colour_pass_gathers_first() {
        use crate::render::core::shader::registry::{self, TextureBinding, WorldKey};
        for (binding, subgroups) in TextureBinding::ALL.into_iter().flat_map(|b| [(b, false), (b, true)]) {
            let key = WorldKey { lamps: true, modern: 2, binding, multiview: false, biased: false, subgroups };
            let src = registry::world(key);
            let body = |name: &str| -> &str {
                let a = src.find(&format!("fn {name}(")).unwrap_or_else(|| panic!("{name}"));
                &src[a..a + src[a..].find("\n}\n").unwrap()]
            };
            for f in ["world_colour", "reflect_colour"] {
                let b = body(f);
                let early = b.find("ml_vis_early(").unwrap_or_else(|| panic!("{f}: no early gather"));
                assert!(early < b.find("surface_at(").unwrap(), "{f}: the gather after Surface Detail");
            }
            assert!(!body("ml_relight").contains("ml_vis_find("), "the gather in the light loop's function");
            assert_eq!(src.matches("ml_vis_find(").count(), 2, "{binding:?}: one definition, one call");
        }
    }

    /// The gather's exact short cuts (`ml_vis_taps`: uniform blocks, no
    /// light) make the full gather's ids and sums bit for bit: today's
    /// `ml_vis_taps` and the gather without them (below, as it was) run on
    /// the GPU over blocks holding every case (lit, shadowed, penumbra, no
    /// light, a second light, a light's edge, depth and normal steps) for
    /// pixels on and off the blocks' plane.
    #[test]
    #[ignore = "requires a GPU adapter; run serially with --ignored"]
    fn gather_short_cuts_are_exact() {
        use wgpu::util::DeviceExt;
        let text = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        let cut = |from: &str, to: &str| -> &str {
            let a = text.find(from).unwrap_or_else(|| panic!("{from}"));
            &text[a..a + text[a..].find(to).unwrap_or_else(|| panic!("{to}"))]
        };
        // Today's gather (its grid, private state, `ml_vis_oct`, the taps; the
        // grid's scale 1, the scene's) and the full one as it was.
        let grid = cut("struct MlRtVis", "// The NIS characters' pipelines");
        let today = cut("var<private> g_vis_ids", "// The blocks for this pixel at p (face normal ng");
        let src = format!(
            "@group(0) @binding(15) var ml_vis_blocks: texture_2d<u32>;\n\
             @group(0) @binding(0) var<storage, read> pixels: array<vec4<f32>>;\n\
             @group(0) @binding(1) var<storage, read_write> out: array<vec4<u32>>;\n\
             {grid}{today}{reference}\n\
             @compute @workgroup_size(64)\n\
             fn main(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
                 let i = id.x;\n\
                 if i >= arrayLength(&pixels) / 2u {{ return; }}\n\
                 let a = pixels[2u * i];\n\
                 let b = pixels[2u * i + 1u];\n\
                 ml_vis_taps(a.xy, a.z, vec2<f32>(a.w, b.x), b.yzw);\n\
                 out[4u * i] = g_vis_ids;\n\
                 out[4u * i + 1u] = bitcast<vec4<u32>>(g_vis_sum);\n\
                 out[4u * i + 2u] = bitcast<vec4<u32>>(g_vis_wt);\n\
                 ref_taps(a.xy, a.z, vec2<f32>(a.w, b.x), b.yzw);\n\
                 out[4u * i + 3u] = g_vis_ids;\n\
                 out[4u * arrayLength(&pixels) / 2u + 2u * i] = bitcast<vec4<u32>>(g_vis_sum);\n\
                 out[4u * arrayLength(&pixels) / 2u + 2u * i + 1u] = bitcast<vec4<u32>>(g_vis_wt);\n\
             }}\n",
            reference = REFERENCE
        );
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()))).unwrap();
        let device = &gpu.device;
        // 32 x 32 blocks: depth on a sloped plane with a step, normals with a
        // crease, and the light slots by region.
        let (bw, bh) = (32u32, 32u32);
        let oct = |n: Vec3| -> u32 {
            let n = n / (n.x.abs() + n.y.abs() + n.z.abs());
            let mut e = glam::Vec2::new(n.x, n.y);
            if n.z < 0.0 {
                e = (glam::Vec2::ONE - glam::Vec2::new(e.y.abs(), e.x.abs())) * glam::Vec2::new(e.x.signum(), e.y.signum());
            }
            let q = |v: f32| ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
            q(e.x) | (q(e.y) << 16)
        };
        let plane = |x: f32, y: f32| 10.0 + 0.05 * x + 0.02 * y + if x > 24.0 { 3.0 } else { 0.0 };
        let mut blocks = Vec::new();
        for by in 0..bh {
            for bx in 0..bw {
                let (x, y) = (bx as f32 * 2.0 + 0.5, by as f32 * 2.0 + 0.5);
                let n = if by > 20 { Vec3::new(0.0, 0.6, 0.8) } else { Vec3::Z };
                let slot = |light: u32, vis: u32| ((light + 1) << 16) | vis;
                let (sx, sy) = match (bx / 8, by / 8) {
                    (0, _) => (0, 0),
                    (1, 0) => (slot(4, 65535), 0),
                    (1, _) => (slot(4, 0), slot(9, 65535)),
                    (2, 0) => (slot(4, if (bx + by) % 3 == 0 { 32768 } else { 65535 }), 0),
                    (2, _) => (slot(if bx % 2 == 0 { 4 } else { 7 }, 65535), 0),
                    _ => (slot(6, [0, 32768, 65535][((bx * 7 + by * 3) % 3) as usize]), slot(2, [0, 65535][(bx % 2) as usize])),
                };
                blocks.extend_from_slice(&[sx, sy, plane(x, y).to_bits(), oct(n)]);
            }
        }
        let tex = device.create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: bw, height: bh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: VIS_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&blocks),
        );
        // Every scene pixel twice: on the blocks' plane and 0.5 m off it
        // (a decal's height), the second half with a tilted face.
        let mut pixels: Vec<[f32; 4]> = Vec::new();
        for off in [0.0f32, 0.5] {
            for py in 0..bh * 2 {
                for px in 0..bw * 2 {
                    let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
                    let n = if py > 40 { Vec3::new(0.0, 0.6, 0.8) } else if off > 0.0 { Vec3::new(0.3, 0.0, 0.95).normalize() } else { Vec3::Z };
                    pixels.push([x, y, plane(x - 0.5, y - 0.5) + off, 0.025]);
                    pixels.push([0.01, n.x, n.y, n.z]);
                }
            }
        }
        let n = pixels.len() / 2;
        let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&pixels),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let size = (n * 6 * 16) as u64;
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(src.into()) });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: input.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: output.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 15, resource: wgpu::BindingResource::TextureView(&tex.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 16, resource: uniform_buffer(device).as_entire_binding() },
            ],
        });
        let mut e = device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((n as u32).div_ceil(64), 1, 1);
        }
        e.copy_buffer_to_buffer(&output, 0, &staging, 0, size);
        gpu.queue.submit([e.finish()]);
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let v: Vec<[u32; 4]> = bytemuck::cast_slice(&staging.slice(..).get_mapped_range().expect("mapped")).to_vec();
        let (mut uniform, mut none, mut lit) = (0, 0, 0);
        for i in 0..n {
            let (ids, sum, wt, ref_ids) = (v[4 * i], v[4 * i + 1], v[4 * i + 2], v[4 * i + 3]);
            let (ref_sum, ref_wt) = (v[4 * n + 2 * i], v[4 * n + 2 * i + 1]);
            // (No light in the centre and side blocks: only the ids are read.)
            if ref_ids == [0; 4] {
                assert_eq!(ids, ref_ids, "pixel {i} ({:?})", pixels[2 * i]);
                none += 1;
                continue;
            }
            assert_eq!((ids, sum, wt), (ref_ids, ref_sum, ref_wt), "pixel {i} ({:?})", pixels[2 * i]);
            uniform += usize::from(ids[0] != 0 && ids[0] == ids[2] && ids[1] == ids[3]);
            lit += usize::from(sum[0] != 0);
        }
        assert!(uniform > n / 4 && none > n / 8 && lit > n / 4, "the cases covered: {uniform} {none} {lit} of {n}");
    }

    /// The gather as it was before its short cuts (the reference above).
    const REFERENCE: &str = r#"
fn ref_add(q: vec2<i32>, dims: vec2<i32>, fw: f32, want: f32, tol: f32, e: vec2<f32>) {
    let t = textureLoad(ml_vis_blocks, clamp(q, vec2<i32>(0), dims - 1), 0);
    let de = unpack2x16snorm(t.w) - e;
    let wd = 1.0 - smoothstep(tol, 2.0 * tol, abs(bitcast<f32>(t.z) - want));
    let wn = 1.0 - smoothstep(0.1, 0.2, abs(de.x) + abs(de.y));
    let inside = all(q >= vec2<i32>(0)) && all(q < dims);
    let w = select(0.0, fw * wd * wn, inside);
    let mx = select(vec4<f32>(0.0), vec4<f32>(w), g_vis_ids == vec4<u32>(t.x >> 16u));
    let my = select(vec4<f32>(0.0), vec4<f32>(w), g_vis_ids == vec4<u32>(t.y >> 16u));
    g_vis_sum += mx * (f32(t.x & 0xFFFFu) / 65535.0) + my * (f32(t.y & 0xFFFFu) / 65535.0);
    g_vis_wt += mx + my;
}
fn ref_taps(pix: vec2<f32>, z: f32, dz: vec2<f32>, ng: vec3<f32>) {
    let dims = vec2<i32>(textureDimensions(ml_vis_blocks));
    let u = (pix - 0.5) * 0.5 + 0.5;
    let k = floor(u);
    let f = u - k;
    let ax = vec3<f32>(0.5 - 0.5 * f.x, 0.5, 0.5 * f.x);
    let ay = vec3<f32>(0.5 - 0.5 * f.y, 0.5, 0.5 * f.y);
    let c = vec2<i32>(k);
    g_vis_cb = c;
    g_vis_c = textureLoad(ml_vis_blocks, clamp(c, vec2<i32>(0), dims - 1), 0);
    let side = select(vec2<i32>(-1), vec2<i32>(1), f >= vec2<f32>(0.5));
    let d = textureLoad(ml_vis_blocks, clamp(c + side, vec2<i32>(0), dims - 1), 0);
    g_vis_ids = vec4<u32>(g_vis_c.x >> 16u, g_vis_c.y >> 16u, d.x >> 16u, d.y >> 16u);
    g_vis_sum = vec4<f32>(0.0);
    g_vis_wt = vec4<f32>(0.0);
    let tol = 0.004 * z + 0.02;
    let e = ml_vis_oct(ng);
    let z0 = z + dot(dz, vec2<f32>(c * 2) + 0.5 - pix);
    let sx = dz.x * 2.0;
    let sy = dz.y * 2.0;
    ref_add(c + vec2<i32>(-1, -1), dims, ax.x * ay.x, z0 - sx - sy, tol, e);
    ref_add(c + vec2<i32>(0, -1), dims, ax.y * ay.x, z0 - sy, tol, e);
    ref_add(c + vec2<i32>(1, -1), dims, ax.z * ay.x, z0 + sx - sy, tol, e);
    ref_add(c + vec2<i32>(-1, 0), dims, ax.x * ay.y, z0 - sx, tol, e);
    ref_add(c, dims, ax.y * ay.y, z0, tol, e);
    ref_add(c + vec2<i32>(1, 0), dims, ax.z * ay.y, z0 + sx, tol, e);
    ref_add(c + vec2<i32>(-1, 1), dims, ax.x * ay.z, z0 - sx + sy, tol, e);
    ref_add(c + vec2<i32>(0, 1), dims, ax.y * ay.z, z0 + sy, tol, e);
    ref_add(c + vec2<i32>(1, 1), dims, ax.z * ay.z, z0 + sx + sy, tol, e);
}
"#;

    /// A supersampled scene's blocks follow the screen (2 x 2 screen pixels
    /// each, every block's scene pixel inside the scene); otherwise the
    /// scene's, at a scale of exactly 1 (the grid as before).
    #[test]
    fn grid_follows_the_screen_when_supersampled() {
        assert_eq!(grid((5431, 3055), (3840, 2160)).0, (1920, 1080));
        assert_eq!(grid((7680, 4320), (3840, 2160)), ((1920, 1080), [2.0, 2.0]));
        for (size, screen) in [((3840, 2160), (3840, 2160)), ((2880, 1620), (3840, 2160)), ((1281, 721), (0, 0))] {
            assert_eq!(grid(size, screen), (blocks(size), [1.0, 1.0]), "{size:?} at {screen:?}");
        }
        for (size, screen) in [((5431, 3055), (3840, 2160)), ((2715, 1527), (1920, 1080)), ((7680, 4320), (3841, 2161))] {
            let ((bw, bh), s) = grid(size, screen);
            let q = |b: u32, s: f32| ((b as f32 * 2.0 + 0.5) * s).floor() as u32;
            assert!(q(bw - 1, s[0]) < size.0 && q(bh - 1, s[1]) < size.1, "{size:?} at {screen:?}");
            assert!(bw * 2 >= screen.0 && bh * 2 >= screen.1);
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
