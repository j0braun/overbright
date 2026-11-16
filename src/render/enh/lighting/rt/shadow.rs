//! Clone deviation, Modern + RT only (docs/perf-budget.md, the new
//! effects' costs): Modern Lighting's ray-traced shadows at half the scene's
//! resolution. Before, 2 + 1 + 1 + 1 rays per pixel were traced inside the world's
//! colour shader (`ml_ray`), which also held every world pixel's shader to
//! the ray queries' registers. Now a compute pass (`rt_shadow.wgsl`)
//! between the main view's depth pre-pass and its colour pass traces the
//! two strongest lights of each 2 × 2 pixel block (the area-light rays,
//! the block's quarter of the disc set) and the colour pass gathers the
//! 2 × 2 blocks around each pixel, weighted by its own plane and normal
//! (`ml_vis` in world.wgsl). The lights past those two without a shadow
//! layer (neon, signs, emitters) share a resampled visibility, one ray a
//! block to a light picked by its share of their light, averaged over the
//! block's history (the last frame's blocks, reprojected); the two traced
//! lights keep a history by light (docs/shadow-softness.md "Every light: the
//! rest resampled"). Nothing native is involved (SPEED2.EXE has no ray
//! tracing); the Original path and every other tier are untouched.

use crate::render::Gpu;
use glam::{DMat4, DVec3, Mat4, Vec3};

/// The visibility blocks' format: (light + 1, visibility) × 2, the rest's
/// visibility, view depth and normal.
pub const VIS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Uint;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    inv: [[f32; 4]; 4],
    sizes: [f32; 4],
    misc: [f32; 4],
    prev: [[f32; 4]; 4],
    hist: [f32; 4],
    skip: [f32; 4],
}

/// A box of blocks, `lo..hi`.
type BlockBox = ((u32, u32), (u32, u32));

/// The history's least blend toward this frame per world tick (1 / 60 s):
/// 1 / 16 (about a quarter second to settle; each block's sample pixel and
/// disc quarter turn over four frames, so the history is what smooths the
/// blocks' steps), never below [`BLEND_MIN`] a frame (a paused world's view
/// still settles). A moving shadow leaves no trail: the history is held to
/// the range this frame's rays show around the block first.
const BLEND_TICK: f32 = 1.0 / 16.0;
const BLEND_MIN: f32 = 1.0 / 32.0;
/// A cut (the eye this far from the last frame's, m) drops the history.
const CUT: f32 = 20.0;

/// The history's least blend for a frame `dt` seconds of world time after
/// the last (the same time constant at any frame rate).
pub fn blend(dt: f32) -> f32 {
    let ticks = (dt * 60.0).clamp(0.0, 8.0);
    (1.0 - (1.0 - BLEND_TICK).powf(ticks)).max(BLEND_MIN)
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
    relative_f64(view_proj, eye).inverse().as_mat4()
}

/// World position relative to `eye` → clip for `view_proj` (in f64).
pub fn relative(view_proj: Mat4, eye: Vec3) -> Mat4 {
    relative_f64(view_proj, eye).as_mat4()
}

fn relative_f64(view_proj: Mat4, eye: Vec3) -> DMat4 {
    view_proj.as_dmat4() * DMat4::from_translation(DVec3::new(eye.x as f64, eye.y as f64, eye.z as f64))
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
            E {
                binding: 8,
                visibility: c,
                ty: B::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            E {
                binding: 7,
                visibility: c,
                ty: B::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            },
        ],
    })
}

/// The first blocks' buffer ([`Plan::record_region`]): 0 at offset 0, the
/// characters' box at [`REGION_OFFSET`].
const REGION_OFFSET: u32 = 256;

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
    /// The dispatches' first blocks (rt_shadow.wgsl `rv_first`).
    first: wgpu::Buffer,
    /// The blocks, written in turn: this frame's ([`Self::cur`]) and the
    /// last one's (the history).
    vis: [wgpu::TextureView; 2],
    cur: usize,
    size: (u32, u32),
    screen: (u32, u32),
    /// The last prepared frame: its view-projection, eye and world time.
    last: Option<(Mat4, Vec3, f32)>,
    /// This frame's box left to the characters' pass.
    skip: Option<BlockBox>,
    frames: u32,
    /// The NIS characters' box of blocks the last frame's characters pass
    /// traced again (shared with the frame's [`Plan`]): this frame's first
    /// pass leaves it out and the characters' pass covers it.
    boxes: std::sync::Arc<std::sync::Mutex<Option<BlockBox>>>,
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
    let u = Uniform { inv: [[0.0; 4]; 4], sizes: [0.0; 4], misc: [0.0, 1.0, 1.0, 0.0], prev: [[0.0; 4]; 4], hist: [0.0; 4], skip: [0.0; 4] };
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
            first: crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor {
                    label: Some("rt shadow first block"),
                    size: u64::from(REGION_OFFSET) + 16,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            ),
            vis: [vis_target(device, (1, 1)), vis_target(device, (1, 1))],
            cur: 0,
            size: (0, 0),
            screen: (0, 0),
            last: None,
            skip: None,
            frames: 0,
            boxes: Default::default(),
        }
    }

    /// The blocks the colour pass reads (a 1 × 1 empty target before the
    /// first sized frame: every pixel keeps its fallback).
    pub fn vis(&self) -> &wgpu::TextureView {
        &self.vis[self.cur]
    }

    /// The uniform the colour pass reads too (the grid's scale, `mlrt.wgsl`
    /// `ml_vis_grid`).
    pub fn uniform(&self) -> &wgpu::Buffer {
        &self.uniform
    }

    /// This frame's values: the scene's `size` shown at `screen`, the main
    /// view's `view_proj` and `eye`, the depth cleared to `clear`, the world
    /// `time` (s). The blocks are remade when their grid changed (before the
    /// frame's group 0 binds them), and this frame's blocks are the other
    /// pair's (the last frame's become the history).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(&mut self, gpu: &Gpu, size: (u32, u32), screen: (u32, u32), view_proj: Mat4, eye: Vec3, clear: f32, time: f32) {
        let (b, scale) = grid(size, screen);
        let still = crate::render::debug_env::nfsu2_rt_shadow_still();
        let mut valid = !still && self.last.is_some_and(|(_, last_eye, _)| last_eye.distance(eye) < CUT);
        if (size, screen) != (self.size, self.screen) {
            self.vis = [vis_target(&gpu.device, b), vis_target(&gpu.device, b)];
            self.size = size;
            self.screen = screen;
            valid = false;
        }
        self.cur ^= 1;
        self.frames = self.frames.wrapping_add(1);
        let (prev, dt) = match self.last {
            Some((vp, _, t)) if valid => (relative(vp, eye), time - t),
            _ => (Mat4::IDENTITY, 0.0),
        };
        let blend = blend(dt);
        // The characters' box of the last frame (dropped with the grid).
        let skip = self.boxes.lock().unwrap().take().filter(|_| valid && size.0 > 0);
        self.skip = skip;
        let u = Uniform {
            inv: inverse_relative(view_proj, eye).to_cols_array_2d(),
            sizes: [size.0 as f32, size.1 as f32, b.0 as f32, b.1 as f32],
            misc: [clear, scale[0], scale[1], 0.0],
            prev: prev.to_cols_array_2d(),
            hist: [
                if valid { 1.0 } else { 0.0 },
                blend,
                if still || crate::render::debug_env::nfsu2_rt_shadow_unturned() { 0.0 } else { (self.frames % (1 << 24)) as f32 },
                blend,
            ],
            skip: skip.map_or([0.0; 4], |((x0, y0), (x1, y1))| [x0 as f32, y0 as f32, x1 as f32, y1 as f32]),
        };
        self.last = (size.0 > 0).then_some((view_proj, eye, time));
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
            queue: gpu.queue.clone(),
            first: self.first.clone(),
            pipeline: self.pipeline(gpu, ms),
            layout: self.layouts[usize::from(ms)].clone(),
            frame: frame.clone(),
            lights: lights.clone(),
            clusters: clusters.clone(),
            tlas: tlas.clone(),
            uniform: self.uniform.clone(),
            vis: self.vis[self.cur].clone(),
            prev: self.vis[self.cur ^ 1].clone(),
            blocks: grid(self.size, self.screen).0,
            skip: self.skip,
            boxes: self.boxes.clone(),
            ms,
        }
    }
}

/// One frame's pass, recorded on a render thread after the pre-pass.
#[derive(Clone)]
pub struct Plan {
    device: wgpu::Device,
    queue: wgpu::Queue,
    first: wgpu::Buffer,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    frame: wgpu::Buffer,
    lights: wgpu::Buffer,
    clusters: wgpu::Buffer,
    tlas: wgpu::Tlas,
    uniform: wgpu::Buffer,
    vis: wgpu::TextureView,
    /// The last frame's blocks (the history).
    prev: wgpu::TextureView,
    blocks: (u32, u32),
    ms: bool,
    /// The box this frame's first pass leaves to the characters' pass.
    skip: Option<BlockBox>,
    boxes: std::sync::Arc<std::sync::Mutex<Option<BlockBox>>>,
}

impl Plan {
    /// The blocks from the pre-pass's `depth` (skipped when its sample
    /// count is not the planned one).
    pub fn record(&self, encoder: &mut wgpu::CommandEncoder, depth: &wgpu::TextureView) {
        self.dispatch(encoder, depth, 0, (0, 0), self.blocks);
    }

    /// The blocks again over `lo..hi` (blocks), from the `depth` that now
    /// holds the NIS characters too: their pixels read their own lights'
    /// rays as the world's do (no rays in their colour shader). Once a frame.
    /// The box is joined with the one the first pass left out (the last
    /// frame's), and kept for the next frame's first pass to leave out: the
    /// blocks there are traced once a frame, not twice.
    pub fn record_region(&self, encoder: &mut wgpu::CommandEncoder, depth: &wgpu::TextureView, lo: (u32, u32), hi: (u32, u32)) {
        let lo = (lo.0.min(self.blocks.0), lo.1.min(self.blocks.1));
        let hi = (hi.0.min(self.blocks.0), hi.1.min(self.blocks.1));
        if hi.0 <= lo.0 || hi.1 <= lo.1 {
            return;
        }
        *self.boxes.lock().unwrap() = Some((lo, hi));
        let (lo, hi) = match self.skip {
            Some((a, b)) => ((lo.0.min(a.0), lo.1.min(a.1)), (hi.0.max(b.0), hi.1.max(b.1))),
            None => (lo, hi),
        };
        // (z 1: the characters' pass, which leaves nothing out.)
        self.queue.write_buffer(&self.first, u64::from(REGION_OFFSET), bytemuck::cast_slice(&[lo.0, lo.1, 1, 0]));
        self.dispatch(encoder, depth, REGION_OFFSET, lo, (hi.0 - lo.0, hi.1 - lo.1));
    }

    /// The blocks' grid.
    pub fn blocks(&self) -> (u32, u32) {
        self.blocks
    }

    fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, depth: &wgpu::TextureView, offset: u32, _lo: (u32, u32), n: (u32, u32)) {
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
                entry(8, R::TextureView(&self.prev)),
                entry(
                    7,
                    R::Buffer(wgpu::BufferBinding { buffer: &self.first, offset: 0, size: wgpu::BufferSize::new(16) }),
                ),
            ],
        });
        let mut pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("rt shadow"), timestamp_writes: None });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[offset]);
        pass.dispatch_workgroups(n.0.div_ceil(8), n.1.div_ceil(8), 1);
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

    /// The gather's short cut (`ml_vis_taps`: uniform blocks) makes the full
    /// gather's ids and, up to rounding, its sums (each light's, the rest's,
    /// the surface's weight): today's `ml_vis_taps` and the same without the
    /// short cut run on the GPU over blocks holding every case (lit,
    /// shadowed, penumbra, no light, a second light, a light's edge, a rest
    /// or none, depth and normal steps) for pixels on and off the blocks'
    /// plane.
    #[test]
    #[ignore = "requires a GPU adapter; run serially with --ignored"]
    fn gather_short_cut_matches_the_full_gather() {
        use wgpu::util::DeviceExt;
        let text = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        let cut = |from: &str, to: &str| -> &str {
            let a = text.find(from).unwrap_or_else(|| panic!("{from}"));
            &text[a..a + text[a..].find(to).unwrap_or_else(|| panic!("{to}"))]
        };
        // Today's gather (its grid, private state, `ml_vis_oct`, the taps; the
        // grid's scale 1, the scene's) and the same without its short cut.
        let grid = cut("struct MlRtVis", "// The NIS characters' pipelines");
        let today = cut("var<private> g_vis_ids", "// The blocks for this pixel at p (face normal ng");
        let taps = cut("fn ml_vis_taps(", "\n}\n");
        const SHORT_CUT: &str =
            "if (vx == 0u || vx == 0x7FFFu) && (vy == 0u || vy == 0x7FFFu) && (g_vis_c.x >> 16u) != (g_vis_c.y >> 16u) {";
        assert_eq!(taps.matches(SHORT_CUT).count(), 1);
        let reference = format!("{}\n}}\n", taps.replace("fn ml_vis_taps(", "fn ref_taps(").replace(SHORT_CUT, "if false {"));
        let src = format!(
            "@group(0) @binding(15) var ml_vis_blocks: texture_2d<u32>;\n\
             @group(0) @binding(0) var<storage, read> pixels: array<vec4<f32>>;\n\
             @group(0) @binding(1) var<storage, read_write> out: array<vec4<u32>>;\n\
             {grid}{today}{reference}\n\
             @compute @workgroup_size(64)\n\
             fn main(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
                 let i = id.x;\n\
                 let n = arrayLength(&pixels) / 2u;\n\
                 if i >= n {{ return; }}\n\
                 let a = pixels[2u * i];\n\
                 let b = pixels[2u * i + 1u];\n\
                 ml_vis_taps(a.xy, a.z, vec2<f32>(a.w, b.x), b.yzw);\n\
                 out[8u * i] = g_vis_ids;\n\
                 out[8u * i + 1u] = bitcast<vec4<u32>>(g_vis_sum);\n\
                 out[8u * i + 2u] = bitcast<vec4<u32>>(g_vis_wt);\n\
                 out[8u * i + 3u] = bitcast<vec4<u32>>(vec4<f32>(g_vis_all, g_vis_rest, 0.0));\n\
                 ref_taps(a.xy, a.z, vec2<f32>(a.w, b.x), b.yzw);\n\
                 out[8u * i + 4u] = g_vis_ids;\n\
                 out[8u * i + 5u] = bitcast<vec4<u32>>(g_vis_sum);\n\
                 out[8u * i + 6u] = bitcast<vec4<u32>>(g_vis_wt);\n\
                 out[8u * i + 7u] = bitcast<vec4<u32>>(vec4<f32>(g_vis_all, g_vis_rest, 0.0));\n\
             }}\n"
        );
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()))).unwrap();
        let device = &gpu.device;
        // 32 x 32 blocks: depth on a sloped plane with a step, normals with a
        // crease, the light slots and the rest by region.
        let (bw, bh) = (32u32, 32u32);
        let oct = |n: Vec3| -> u32 {
            let n = n / (n.x.abs() + n.y.abs() + n.z.abs());
            let mut e = glam::Vec2::new(n.x, n.y);
            if n.z < 0.0 {
                e = (glam::Vec2::ONE - glam::Vec2::new(e.y.abs(), e.x.abs())) * glam::Vec2::new(e.x.signum(), e.y.signum());
            }
            let q = |v: f32| ((v.clamp(-1.0, 1.0) * 127.0).round() as i8) as u8 as u32;
            q(e.x) | (q(e.y) << 8)
        };
        // An f32 (positive, normal range) as f16 bits, rounded to nearest.
        let half = |v: f32| -> u32 {
            let b = v.to_bits();
            let e = ((b >> 23) & 0xFF) as i32 - 127 + 15;
            let m = (b & 0x7F_FFFF) + 0x1000;
            ((e as u32) << 10) + (m >> 13)
        };
        let plane = |x: f32, y: f32| 10.0 + 0.05 * x + 0.02 * y + if x > 24.0 { 3.0 } else { 0.0 };
        let mut blocks = Vec::new();
        for by in 0..bh {
            for bx in 0..bw {
                let (x, y) = (bx as f32 * 2.0 + 0.5, by as f32 * 2.0 + 0.5);
                let n = if by > 20 { Vec3::new(0.0, 0.6, 0.8) } else { Vec3::Z };
                // (light + 1) << 16, the rest's kind (1 << 15), visibility (15 bits).
                let slot = |light: u32, vis: u32| ((light + 1) << 16) | if light % 2 == 0 { 0x8000 } else { 0 } | vis;
                let (sx, sy) = match (bx / 8, by / 8) {
                    (0, _) => (0, 0),
                    (1, 0) => (slot(4, 32767), 0),
                    (1, _) => (slot(4, 0), slot(9, 32767)),
                    (2, 0) => (slot(4, if (bx + by) % 3 == 0 { 16384 } else { 32767 }), 0),
                    (2, _) => (slot(if bx % 2 == 0 { 4 } else { 7 }, 32767), 0),
                    _ => (slot(6, [0, 16384, 32767][((bx * 7 + by * 3) % 3) as usize]), slot(2, [0, 32767][(bx % 2) as usize])),
                };
                let rest = match (bx + by / 4) % 4 {
                    0 => 0,
                    1 => 40000 | (12 << 16) | (130 << 24),
                    _ => [0u32, 20000, 65535][(by % 3) as usize] | (3 << 16) | (128 << 24),
                };
                blocks.extend_from_slice(&[sx, sy, if by < 4 { 0 } else { rest }, half(plane(x, y)) | (oct(n) << 16)]);
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
        let size = (n * 8 * 16) as u64;
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
        let f = |a: [u32; 4]| a.map(f32::from_bits);
        let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-5 * (1.0 + y.abs()));
        let (mut uniform, mut rest, mut lit) = (0, 0, 0);
        for i in 0..n {
            let got = &v[8 * i..8 * i + 4];
            let want = &v[8 * i + 4..8 * i + 8];
            let what = format!("pixel {i} ({:?})", pixels[2 * i]);
            // (No light nor rest in the centre and side blocks: the ids alone.)
            if want[0] == [0; 4] && f(want[3])[2] == 0.0 {
                assert_eq!(got[0], want[0], "{what}");
                continue;
            }
            assert_eq!(got[0], want[0], "{what}");
            for k in 1..4 {
                assert!(close(f(got[k]), f(want[k])), "{what}: {:?} vs {:?}", f(got[k]), f(want[k]));
            }
            uniform += usize::from(got[0][0] != 0 && got[0][0] == got[0][2] && got[0][1] == got[0][3]);
            rest += usize::from(f(want[3])[2] > 0.0);
            lit += usize::from(f(want[1])[0] > 0.0);
        }
        assert!(uniform > n / 8 && rest > n / 4 && lit > n / 4, "the cases covered: {uniform} {rest} {lit} of {n}");
    }

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
